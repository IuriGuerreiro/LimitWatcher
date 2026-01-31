# Claude Provider Implementation

## Overview
- **Provider ID:** `claude`
- **Auth Methods:** OAuth (primary), Browser Cookies (fallback)
- **API:** Claude OAuth API + Web scraping fallback
- **Reference:** `CodexBar/Sources/CodexBarCore/Providers/Claude/`

---

## Authentication Options

### Option 1: OAuth (Recommended)
Uses Claude CLI OAuth credentials stored in Keychain.

### Option 2: Browser Cookies (Fallback)
Extracts session cookies from browser for web API access.

---

## OAuth Implementation

### Reading Claude CLI Credentials

Claude CLI stores OAuth tokens in the system keychain. We can read these directly:

```rust
const CLAUDE_KEYCHAIN_SERVICE: &str = "claude.ai";
const CLAUDE_KEYCHAIN_ACCOUNT: &str = "oauth_credentials";

#[derive(Deserialize, Debug)]
struct ClaudeOAuthCredentials {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
}

async fn load_claude_oauth() -> Option<ClaudeOAuthCredentials> {
    // Try to read from Claude CLI's keychain entry
    if let Ok(Some(json)) = keyring::get_credential_from_service(
        CLAUDE_KEYCHAIN_SERVICE, 
        CLAUDE_KEYCHAIN_ACCOUNT
    ) {
        serde_json::from_str(&json).ok()
    } else {
        None
    }
}
```

### OAuth Token Refresh

```rust
const CLAUDE_TOKEN_URL: &str = "https://console.anthropic.com/oauth/token";

async fn refresh_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<ClaudeOAuthCredentials, ProviderError> {
    let response = client
        .post(CLAUDE_TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    
    if !response.status().is_success() {
        return Err(ProviderError::AuthFailed("Token refresh failed".into()));
    }
    
    response
        .json()
        .await
        .map_err(|e| ProviderError::Parse(e.to_string()))
}
```

---

## Fetching Usage Data

### Claude Usage API

```rust
const CLAUDE_USAGE_URL: &str = "https://api.claude.ai/api/usage";

#[derive(Deserialize, Debug)]
struct ClaudeUsageResponse {
    // Session (5-hour window)
    session: SessionUsage,
    // Weekly usage
    weekly: WeeklyUsage,
    // Account info
    account_type: String,
}

#[derive(Deserialize, Debug)]
struct SessionUsage {
    messages_used: u64,
    messages_limit: u64,
    tokens_used: u64,
    tokens_limit: u64,
    reset_at: String,
}

#[derive(Deserialize, Debug)]
struct WeeklyUsage {
    messages_used: u64,
    messages_limit: u64,
    reset_at: String,
}

async fn fetch_claude_usage(
    client: &reqwest::Client,
    token: &str,
) -> Result<UsageData, ProviderError> {
    let response = client
        .get(CLAUDE_USAGE_URL)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/json")
        .header("User-Agent", "LimitsWatcher/1.0")
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    
    match response.status().as_u16() {
        401 => return Err(ProviderError::TokenExpired),
        403 => return Err(ProviderError::AuthFailed("Access denied".into())),
        429 => {
            let retry = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(60);
            return Err(ProviderError::RateLimited(retry));
        }
        _ => {}
    }
    
    let usage: ClaudeUsageResponse = response
        .json()
        .await
        .map_err(|e| ProviderError::Parse(e.to_string()))?;
    
    let session_reset = chrono::DateTime::parse_from_rfc3339(&usage.session.reset_at)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc));
    
    let weekly_reset = chrono::DateTime::parse_from_rfc3339(&usage.weekly.reset_at)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc));
    
    Ok(UsageData {
        session_used: usage.session.messages_used,
        session_limit: usage.session.messages_limit,
        weekly_used: usage.weekly.messages_used,
        weekly_limit: usage.weekly.messages_limit,
        credits_remaining: None,
        reset_time: session_reset,
        weekly_reset_time: weekly_reset,
        last_updated: chrono::Utc::now(),
        error: None,
    })
}
```

---

## Cookie-Based Fallback

For users who don't have Claude CLI installed, we can use browser cookies.

### Cookie Extraction

```rust
use crate::storage::encrypted;

#[derive(Serialize, Deserialize, Debug)]
struct ClaudeCookies {
    session_key: String,
    // Other relevant cookies
}

/// Extract cookies from browser (complex, platform-specific)
/// This is a simplified version - real implementation needs browser-specific logic
async fn extract_browser_cookies() -> Option<ClaudeCookies> {
    // Platform-specific cookie extraction
    // Windows: Chrome cookies in %LOCALAPPDATA%\Google\Chrome\User Data\Default\Cookies
    // macOS: ~/Library/Application Support/Google/Chrome/Default/Cookies
    // Linux: ~/.config/google-chrome/Default/Cookies
    
    // Note: Chrome cookies are encrypted with DPAPI (Windows) or Keychain (macOS)
    // This requires decryption - see CodexBar's BrowserCookieAccessGate for reference
    
    todo!("Implement browser cookie extraction")
}

/// Fetch usage using session cookie
async fn fetch_usage_with_cookie(
    client: &reqwest::Client,
    cookies: &ClaudeCookies,
) -> Result<UsageData, ProviderError> {
    let response = client
        .get("https://claude.ai/api/organizations")
        .header("Cookie", format!("sessionKey={}", cookies.session_key))
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    
    // Parse web API response
    // Note: Web API format differs from OAuth API
    todo!("Parse web API response")
}
```

---

## Full Implementation

**File:** `src-tauri/src/providers/claude.rs`

```rust
//! Claude provider implementation
//! 
//! Auth: OAuth (via Claude CLI) or Browser Cookies
//! API: Claude OAuth API or Web API
//! 
//! ## Data Available
//! - Session messages (5-hour window)
//! - Weekly messages
//! - Reset timestamps

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::providers::traits::*;
use crate::storage::{UsageData, keyring, encrypted};
use std::path::PathBuf;

const CLAUDE_USAGE_URL: &str = "https://api.claude.ai/api/usage";

pub struct ClaudeProvider {
    oauth_token: Option<String>,
    cookies: Option<ClaudeCookies>,
    client: reqwest::Client,
    data_dir: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ClaudeCookies {
    session_key: String,
}

#[derive(Deserialize, Debug)]
struct ClaudeUsageResponse {
    session: SessionUsage,
    weekly: WeeklyUsage,
}

#[derive(Deserialize, Debug)]
struct SessionUsage {
    messages_used: u64,
    messages_limit: u64,
    reset_at: String,
}

#[derive(Deserialize, Debug)]
struct WeeklyUsage {
    messages_used: u64,
    messages_limit: u64,
    reset_at: String,
}

impl ClaudeProvider {
    pub fn new() -> Self {
        Self::with_data_dir(dirs::data_local_dir().unwrap_or_default().join("LimitsWatcher"))
    }
    
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        let mut provider = Self {
            oauth_token: None,
            cookies: None,
            client: reqwest::Client::new(),
            data_dir,
        };
        
        // Try to load saved credentials
        provider.load_credentials();
        
        provider
    }
    
    fn load_credentials(&mut self) {
        // Try OAuth token first
        if let Ok(Some(token)) = keyring::get_credential(keyring::keys::CLAUDE_OAUTH) {
            self.oauth_token = Some(token);
            return;
        }
        
        // Try cookies fallback
        let cookies_path = self.data_dir.join("claude_cookies.enc");
        if let Ok(cookies) = encrypted::decrypt_from_file::<ClaudeCookies>(&cookies_path, None) {
            self.cookies = Some(cookies);
        }
    }
    
    fn save_cookies(&self, cookies: &ClaudeCookies) -> Result<(), ProviderError> {
        let cookies_path = self.data_dir.join("claude_cookies.enc");
        encrypted::encrypt_to_file(&cookies_path, cookies, None)
            .map_err(|e| ProviderError::Provider(e.to_string()))
    }
    
    async fn fetch_with_oauth(&self, token: &str) -> ProviderResult<UsageData> {
        let response = self.client
            .get(CLAUDE_USAGE_URL)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        match response.status().as_u16() {
            401 => return Err(ProviderError::TokenExpired),
            403 => return Err(ProviderError::AuthFailed("Access denied".into())),
            429 => {
                let retry = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(60);
                return Err(ProviderError::RateLimited(retry));
            }
            _ => {}
        }
        
        let usage: ClaudeUsageResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        
        self.parse_usage_response(usage)
    }
    
    fn parse_usage_response(&self, usage: ClaudeUsageResponse) -> ProviderResult<UsageData> {
        let session_reset = DateTime::parse_from_rfc3339(&usage.session.reset_at)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));
        
        let weekly_reset = DateTime::parse_from_rfc3339(&usage.weekly.reset_at)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));
        
        Ok(UsageData {
            session_used: usage.session.messages_used,
            session_limit: usage.session.messages_limit,
            weekly_used: usage.weekly.messages_used,
            weekly_limit: usage.weekly.messages_limit,
            credits_remaining: None,
            reset_time: session_reset,
            weekly_reset_time: weekly_reset,
            last_updated: Utc::now(),
            error: None,
        })
    }
}

#[async_trait]
impl Provider for ClaudeProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "claude".to_string(),
            name: "Claude".to_string(),
            website: "https://claude.ai".to_string(),
            auth_methods: vec![AuthMethod::OAuth2, AuthMethod::Cookies],
            has_session_limits: true,
            has_weekly_limits: true,
            has_credits: false,
            icon: "claude".to_string(),
        }
    }
    
    async fn is_authenticated(&self) -> bool {
        self.oauth_token.is_some() || self.cookies.is_some()
    }
    
    async fn fetch_usage(&self) -> ProviderResult<UsageData> {
        // Try OAuth first
        if let Some(token) = &self.oauth_token {
            match self.fetch_with_oauth(token).await {
                Ok(usage) => return Ok(usage),
                Err(ProviderError::TokenExpired) => {
                    // Fall through to cookies if available
                }
                Err(e) => return Err(e),
            }
        }
        
        // Try cookies fallback
        if let Some(_cookies) = &self.cookies {
            // TODO: Implement cookie-based fetch
            return Err(ProviderError::Provider("Cookie fetch not yet implemented".into()));
        }
        
        Err(ProviderError::AuthRequired)
    }
    
    async fn start_auth(&mut self) -> ProviderResult<Option<AuthFlow>> {
        // For OAuth, user needs to install Claude CLI first
        // We just read their existing credentials
        Ok(Some(AuthFlow {
            url: "https://claude.ai/download".to_string(),
            user_code: None,
            instructions: concat!(
                "Claude uses OAuth via the Claude CLI.\n\n",
                "1. Install Claude CLI from claude.ai/download\n",
                "2. Run 'claude login' to authenticate\n",
                "3. Click 'Check for credentials' below"
            ).to_string(),
            poll_interval: None,
        }))
    }
    
    async fn complete_auth(&mut self, response: AuthResponse) -> ProviderResult<()> {
        match response {
            AuthResponse::Cookies(cookie_str) => {
                // Parse and save cookies
                let cookies = ClaudeCookies {
                    session_key: cookie_str,
                };
                self.save_cookies(&cookies)?;
                self.cookies = Some(cookies);
                Ok(())
            }
            _ => {
                // Try to load OAuth credentials from Claude CLI
                self.load_credentials();
                if self.oauth_token.is_some() {
                    Ok(())
                } else {
                    Err(ProviderError::AuthFailed(
                        "No Claude CLI credentials found. Run 'claude login' first.".into()
                    ))
                }
            }
        }
    }
    
    async fn logout(&mut self) -> ProviderResult<()> {
        self.oauth_token = None;
        self.cookies = None;
        
        // Delete stored cookies
        let cookies_path = self.data_dir.join("claude_cookies.enc");
        let _ = encrypted::delete(&cookies_path);
        
        // Note: We don't delete OAuth from keychain as that's Claude CLI's
        
        Ok(())
    }
    
    fn auth_status(&self) -> AuthStatus {
        if self.oauth_token.is_some() {
            AuthStatus::Authenticated {
                user: Some("via Claude CLI".to_string()),
                expires: None,
            }
        } else if self.cookies.is_some() {
            AuthStatus::Authenticated {
                user: Some("via cookies".to_string()),
                expires: None,
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
// src/components/providers/ClaudeAuth.tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";

export function ClaudeAuth({ onComplete }: { onComplete: () => void }) {
  const [mode, setMode] = useState<"oauth" | "cookies">("oauth");
  const [cookieInput, setCookieInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function checkOAuth() {
    setLoading(true);
    setError(null);
    
    try {
      await invoke("complete_provider_auth", {
        provider: "claude",
        response: { OAuthCode: "" } // Just triggers credential check
      });
      onComplete();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function submitCookies() {
    if (!cookieInput.trim()) return;
    
    setLoading(true);
    setError(null);
    
    try {
      await invoke("complete_provider_auth", {
        provider: "claude",
        response: { Cookies: cookieInput.trim() }
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
      <h3>Claude</h3>
      
      <div className="auth-tabs">
        <button 
          className={mode === "oauth" ? "active" : ""} 
          onClick={() => setMode("oauth")}
        >
          OAuth (CLI)
        </button>
        <button 
          className={mode === "cookies" ? "active" : ""} 
          onClick={() => setMode("cookies")}
        >
          Cookies
        </button>
      </div>
      
      {error && <p className="error">{error}</p>}
      
      {mode === "oauth" ? (
        <div className="oauth-section">
          <p>Claude uses OAuth via the Claude CLI.</p>
          <ol>
            <li>
              <a href="#" onClick={() => open("https://claude.ai/download")}>
                Install Claude CLI
              </a>
            </li>
            <li>Run <code>claude login</code> in terminal</li>
            <li>Click below to check for credentials</li>
          </ol>
          <button onClick={checkOAuth} disabled={loading}>
            {loading ? "Checking..." : "Check for credentials"}
          </button>
        </div>
      ) : (
        <div className="cookies-section">
          <p>Paste your Claude session cookie:</p>
          <ol>
            <li>Open claude.ai and log in</li>
            <li>Open DevTools (F12) → Application → Cookies</li>
            <li>Find <code>sessionKey</code> and copy its value</li>
          </ol>
          <input
            type="password"
            value={cookieInput}
            onChange={(e) => setCookieInput(e.target.value)}
            placeholder="Paste sessionKey value here"
          />
          <button onClick={submitCookies} disabled={loading || !cookieInput.trim()}>
            {loading ? "Saving..." : "Save Cookie"}
          </button>
        </div>
      )}
    </div>
  );
}
```

---

## Checklist

- [ ] OAuth credential loading (from Claude CLI keychain)
- [ ] OAuth API usage fetching
- [ ] Cookie-based fallback
  - [ ] Cookie input UI
  - [ ] Encrypted cookie storage
  - [ ] Web API fetch
- [ ] Token refresh handling
- [ ] Rate limiting handled
- [ ] Frontend auth component (both modes)
- [ ] Tests passing

---

## Notes from CodexBar

From `CodexBar/Sources/CodexBarCore/Providers/Claude/`:
- Supports both OAuth and cookies
- OAuth reads from Claude CLI's Keychain entry
- Web API requires specific User-Agent and cookie format
- Session = 5-hour window, Weekly = 7-day window
- `ClaudeUsageFetcher.swift` has detailed parsing logic
