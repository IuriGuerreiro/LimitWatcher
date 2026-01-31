# Gemini Provider Implementation

## Overview
- **Provider ID:** `gemini`
- **Auth Method:** OAuth via Gemini CLI credentials
- **API:** Google Generative Language API
- **Reference:** `CodexBar/Sources/CodexBarCore/Providers/Gemini/`

---

## Authentication

Gemini uses OAuth credentials stored by the Gemini CLI (`gemini`). We read these credentials directly - no browser cookies needed.

### Credential Locations

```rust
/// Get Gemini CLI credentials path based on platform
fn get_gemini_credentials_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|p| p.join("gemini").join("credentials.json"))
    }
    
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|p| p.join(".config").join("gemini").join("credentials.json"))
    }
    
    #[cfg(target_os = "linux")]
    {
        dirs::home_dir().map(|p| p.join(".config").join("gemini").join("credentials.json"))
    }
}
```

### Credential Structure

```rust
#[derive(Deserialize, Debug)]
struct GeminiCredentials {
    access_token: String,
    refresh_token: String,
    token_uri: String,
    client_id: String,
    client_secret: String,
    expiry: String, // ISO 8601 timestamp
}

async fn load_gemini_credentials() -> Option<GeminiCredentials> {
    let path = get_gemini_credentials_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}
```

### Token Refresh

```rust
async fn refresh_gemini_token(
    client: &reqwest::Client,
    credentials: &GeminiCredentials,
) -> Result<String, ProviderError> {
    let response = client
        .post(&credentials.token_uri)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &credentials.refresh_token),
            ("client_id", &credentials.client_id),
            ("client_secret", &credentials.client_secret),
        ])
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
    }
    
    let token: TokenResponse = response
        .json()
        .await
        .map_err(|e| ProviderError::Parse(e.to_string()))?;
    
    Ok(token.access_token)
}
```

---

## Fetching Usage Data

### Gemini Quota API

```rust
const GEMINI_QUOTA_URL: &str = "https://generativelanguage.googleapis.com/v1beta/quota";

#[derive(Deserialize, Debug)]
struct GeminiQuotaResponse {
    /// Requests per minute
    requests_per_minute: QuotaInfo,
    /// Tokens per minute
    tokens_per_minute: QuotaInfo,
    /// Daily requests (if applicable)
    daily_requests: Option<QuotaInfo>,
}

#[derive(Deserialize, Debug)]
struct QuotaInfo {
    used: u64,
    limit: u64,
    reset_time: Option<String>,
}

async fn fetch_gemini_quota(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<UsageData, ProviderError> {
    let response = client
        .get(GEMINI_QUOTA_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    
    match response.status().as_u16() {
        401 => return Err(ProviderError::TokenExpired),
        403 => return Err(ProviderError::AuthFailed("Access denied".into())),
        429 => return Err(ProviderError::RateLimited(60)),
        _ => {}
    }
    
    let quota: GeminiQuotaResponse = response
        .json()
        .await
        .map_err(|e| ProviderError::Parse(e.to_string()))?;
    
    // Map to UsageData
    // Session = per-minute limits, Weekly = daily limits if available
    Ok(UsageData {
        session_used: quota.requests_per_minute.used,
        session_limit: quota.requests_per_minute.limit,
        weekly_used: quota.daily_requests.as_ref().map(|d| d.used).unwrap_or(0),
        weekly_limit: quota.daily_requests.as_ref().map(|d| d.limit).unwrap_or(0),
        credits_remaining: None,
        reset_time: parse_reset_time(&quota.requests_per_minute.reset_time),
        weekly_reset_time: quota.daily_requests.as_ref().and_then(|d| parse_reset_time(&d.reset_time)),
        last_updated: chrono::Utc::now(),
        error: None,
    })
}

fn parse_reset_time(time_str: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
    time_str.as_ref().and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc))
    })
}
```

---

## Full Implementation

**File:** `src-tauri/src/providers/gemini.rs`

```rust
//! Gemini provider implementation
//! 
//! Auth: OAuth via Gemini CLI credentials
//! API: Google Generative Language API
//! 
//! ## Data Available
//! - Requests per minute (session)
//! - Daily requests (weekly equivalent)
//! - Token usage

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};

use crate::providers::traits::*;
use crate::storage::{UsageData, keyring};

const GEMINI_QUOTA_URL: &str = "https://generativelanguage.googleapis.com/v1beta/quota";

pub struct GeminiProvider {
    credentials: Option<GeminiCredentials>,
    access_token: Option<String>,
    client: reqwest::Client,
}

#[derive(Deserialize, Clone, Debug)]
struct GeminiCredentials {
    access_token: String,
    refresh_token: String,
    token_uri: String,
    client_id: String,
    client_secret: String,
    expiry: Option<String>,
}

#[derive(Deserialize, Debug)]
struct GeminiQuotaResponse {
    requests_per_minute: Option<QuotaInfo>,
    tokens_per_minute: Option<QuotaInfo>,
    daily_requests: Option<QuotaInfo>,
}

#[derive(Deserialize, Debug)]
struct QuotaInfo {
    used: u64,
    limit: u64,
    reset_time: Option<String>,
}

impl GeminiProvider {
    pub fn new() -> Self {
        let mut provider = Self {
            credentials: None,
            access_token: None,
            client: reqwest::Client::new(),
        };
        
        provider.load_credentials();
        provider
    }
    
    fn get_credentials_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            dirs::config_dir().map(|p| p.join("gemini").join("credentials.json"))
        }
        
        #[cfg(target_os = "macos")]
        {
            dirs::home_dir().map(|p| p.join(".config").join("gemini").join("credentials.json"))
        }
        
        #[cfg(target_os = "linux")]
        {
            dirs::home_dir().map(|p| p.join(".config").join("gemini").join("credentials.json"))
        }
    }
    
    fn load_credentials(&mut self) {
        if let Some(path) = Self::get_credentials_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(creds) = serde_json::from_str::<GeminiCredentials>(&content) {
                    self.access_token = Some(creds.access_token.clone());
                    self.credentials = Some(creds);
                }
            }
        }
    }
    
    async fn ensure_valid_token(&mut self) -> ProviderResult<String> {
        // Check if we have credentials
        let creds = self.credentials.as_ref()
            .ok_or(ProviderError::AuthRequired)?;
        
        // Check if token is expired
        let needs_refresh = if let Some(expiry) = &creds.expiry {
            if let Ok(exp_time) = DateTime::parse_from_rfc3339(expiry) {
                exp_time.with_timezone(&Utc) < Utc::now()
            } else {
                false
            }
        } else {
            false
        };
        
        if needs_refresh || self.access_token.is_none() {
            self.refresh_token().await?;
        }
        
        self.access_token.clone().ok_or(ProviderError::AuthRequired)
    }
    
    async fn refresh_token(&mut self) -> ProviderResult<()> {
        let creds = self.credentials.as_ref()
            .ok_or(ProviderError::AuthRequired)?;
        
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: Option<u64>,
        }
        
        let response = self.client
            .post(&creds.token_uri)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &creds.refresh_token),
                ("client_id", &creds.client_id),
                ("client_secret", &creds.client_secret),
            ])
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(ProviderError::AuthFailed("Token refresh failed".into()));
        }
        
        let token: TokenResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        
        self.access_token = Some(token.access_token);
        Ok(())
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "gemini".to_string(),
            name: "Gemini".to_string(),
            website: "https://gemini.google.com".to_string(),
            auth_methods: vec![AuthMethod::Cli],
            has_session_limits: true,
            has_weekly_limits: true,
            has_credits: false,
            icon: "gemini".to_string(),
        }
    }
    
    async fn is_authenticated(&self) -> bool {
        self.credentials.is_some()
    }
    
    async fn fetch_usage(&self) -> ProviderResult<UsageData> {
        let token = self.access_token.as_ref()
            .ok_or(ProviderError::AuthRequired)?;
        
        let response = self.client
            .get(GEMINI_QUOTA_URL)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        match response.status().as_u16() {
            401 => return Err(ProviderError::TokenExpired),
            403 => return Err(ProviderError::AuthFailed("Access denied".into())),
            429 => return Err(ProviderError::RateLimited(60)),
            _ => {}
        }
        
        // Handle case where quota endpoint might not exist
        if response.status().as_u16() == 404 {
            return Ok(UsageData {
                session_used: 0,
                session_limit: 0,
                weekly_used: 0,
                weekly_limit: 0,
                credits_remaining: None,
                reset_time: None,
                weekly_reset_time: None,
                last_updated: Utc::now(),
                error: Some("Quota API not available".into()),
            });
        }
        
        let quota: GeminiQuotaResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        
        let parse_reset = |time_str: &Option<String>| -> Option<DateTime<Utc>> {
            time_str.as_ref().and_then(|s| {
                DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            })
        };
        
        Ok(UsageData {
            session_used: quota.requests_per_minute.as_ref().map(|q| q.used).unwrap_or(0),
            session_limit: quota.requests_per_minute.as_ref().map(|q| q.limit).unwrap_or(0),
            weekly_used: quota.daily_requests.as_ref().map(|q| q.used).unwrap_or(0),
            weekly_limit: quota.daily_requests.as_ref().map(|q| q.limit).unwrap_or(0),
            credits_remaining: None,
            reset_time: quota.requests_per_minute.as_ref().and_then(|q| parse_reset(&q.reset_time)),
            weekly_reset_time: quota.daily_requests.as_ref().and_then(|q| parse_reset(&q.reset_time)),
            last_updated: Utc::now(),
            error: None,
        })
    }
    
    async fn start_auth(&mut self) -> ProviderResult<Option<AuthFlow>> {
        Ok(Some(AuthFlow {
            url: "https://ai.google.dev/gemini-api/docs/downloads".to_string(),
            user_code: None,
            instructions: concat!(
                "Gemini uses OAuth via the Gemini CLI.\n\n",
                "1. Install Gemini CLI from ai.google.dev\n",
                "2. Run 'gemini auth' to authenticate\n",
                "3. Click 'Check for credentials' below"
            ).to_string(),
            poll_interval: None,
        }))
    }
    
    async fn complete_auth(&mut self, _response: AuthResponse) -> ProviderResult<()> {
        self.load_credentials();
        
        if self.credentials.is_some() {
            Ok(())
        } else {
            Err(ProviderError::AuthFailed(
                "No Gemini CLI credentials found. Run 'gemini auth' first.".into()
            ))
        }
    }
    
    async fn logout(&mut self) -> ProviderResult<()> {
        self.credentials = None;
        self.access_token = None;
        // Note: We don't delete the CLI credentials file
        Ok(())
    }
    
    fn auth_status(&self) -> AuthStatus {
        if self.credentials.is_some() {
            AuthStatus::Authenticated {
                user: Some("via Gemini CLI".to_string()),
                expires: self.credentials.as_ref().and_then(|c| c.expiry.clone()),
            }
        } else {
            AuthStatus::NotAuthenticated
        }
    }
}
```

---

## Frontend Integration

```tsx
// src/components/providers/GeminiAuth.tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";

export function GeminiAuth({ onComplete }: { onComplete: () => void }) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function checkCredentials() {
    setLoading(true);
    setError(null);
    
    try {
      await invoke("complete_provider_auth", {
        provider: "gemini",
        response: { OAuthCode: "" }
      });
      onComplete();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="auth-panel">
      <h3>Gemini</h3>
      
      {error && <p className="error">{error}</p>}
      
      <div className="cli-auth">
        <p>Gemini uses OAuth via the Gemini CLI.</p>
        <ol>
          <li>
            <a href="#" onClick={() => open("https://ai.google.dev/gemini-api/docs/downloads")}>
              Install Gemini CLI
            </a>
          </li>
          <li>Run <code>gemini auth</code> in terminal</li>
          <li>Click below to check for credentials</li>
        </ol>
        <button onClick={checkCredentials} disabled={loading}>
          {loading ? "Checking..." : "Check for credentials"}
        </button>
      </div>
    </div>
  );
}
```

---

## Checklist

- [ ] CLI credential file detection (all platforms)
- [ ] Credential loading and parsing
- [ ] Token refresh when expired
- [ ] Quota API fetching
- [ ] Handle missing quota endpoint gracefully
- [ ] Frontend auth component
- [ ] Tests passing

---

## Notes from CodexBar

From `CodexBar/Sources/CodexBarCore/Providers/Gemini/`:
- Reads from `~/.config/gemini/credentials.json`
- OAuth is fully managed by CLI, we just read the tokens
- Token refresh uses standard Google OAuth endpoints
- Quota limits vary by plan (free tier vs paid)
