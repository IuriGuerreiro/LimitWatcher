# Antigravity Provider Implementation

## Overview
- **Provider ID:** `antigravity`
- **Auth Method:** None (local service probe)
- **API:** Local language server probe
- **Reference:** `CodexBar/Sources/CodexBarCore/Providers/Antigravity/`

---

## Authentication

Antigravity doesn't require authentication - it runs as a local service. We just need to detect if it's running and probe its status.

---

## Service Detection

### Finding Antigravity Process

```rust
use std::process::Command;

/// Check if Antigravity is running
fn is_antigravity_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq antigravity.exe", "/FO", "CSV", "/NH"])
            .output()
            .ok();
        
        output.map(|o| {
            String::from_utf8_lossy(&o.stdout).contains("antigravity.exe")
        }).unwrap_or(false)
    }
    
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let output = Command::new("pgrep")
            .args(["-x", "antigravity"])
            .output()
            .ok();
        
        output.map(|o| o.status.success()).unwrap_or(false)
    }
}
```

### Local Port Detection

Antigravity typically runs on a local port. We probe known ports to find it:

```rust
const ANTIGRAVITY_PORTS: &[u16] = &[8765, 8766, 8767]; // Common Antigravity ports

async fn find_antigravity_port() -> Option<u16> {
    for port in ANTIGRAVITY_PORTS {
        if is_port_antigravity(*port).await {
            return Some(*port);
        }
    }
    None
}

async fn is_port_antigravity(port: u16) -> bool {
    let url = format!("http://localhost:{}/health", port);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;
    
    match client.get(&url).send().await {
        Ok(response) => {
            // Check if it's Antigravity by looking at response
            if let Ok(text) = response.text().await {
                text.contains("antigravity") || text.contains("Antigravity")
            } else {
                false
            }
        }
        Err(_) => false,
    }
}
```

---

## Fetching Usage Data

### Probing Local Status

```rust
#[derive(Deserialize, Debug)]
struct AntigravityStatus {
    running: bool,
    version: Option<String>,
    usage: Option<AntigravityUsage>,
}

#[derive(Deserialize, Debug)]
struct AntigravityUsage {
    requests_today: u64,
    requests_limit: Option<u64>,
    tokens_used: u64,
    tokens_limit: Option<u64>,
}

async fn probe_antigravity_status(port: u16) -> Result<UsageData, ProviderError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    
    let url = format!("http://localhost:{}/status", port);
    
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    
    if !response.status().is_success() {
        return Err(ProviderError::Provider("Status endpoint returned error".into()));
    }
    
    let status: AntigravityStatus = response
        .json()
        .await
        .map_err(|e| ProviderError::Parse(e.to_string()))?;
    
    if !status.running {
        return Err(ProviderError::Provider("Antigravity not running".into()));
    }
    
    let usage = status.usage.unwrap_or(AntigravityUsage {
        requests_today: 0,
        requests_limit: None,
        tokens_used: 0,
        tokens_limit: None,
    });
    
    Ok(UsageData {
        session_used: usage.tokens_used,
        session_limit: usage.tokens_limit.unwrap_or(0),
        weekly_used: usage.requests_today,
        weekly_limit: usage.requests_limit.unwrap_or(0),
        credits_remaining: None,
        reset_time: None, // Daily reset at midnight
        weekly_reset_time: Some(get_next_midnight()),
        last_updated: chrono::Utc::now(),
        error: None,
    })
}

fn get_next_midnight() -> chrono::DateTime<chrono::Utc> {
    let now = chrono::Utc::now();
    let tomorrow = now.date_naive().succ_opt().unwrap();
    tomorrow.and_hms_opt(0, 0, 0).unwrap().and_utc()
}
```

---

## Full Implementation

**File:** `src-tauri/src/providers/antigravity.rs`

```rust
//! Antigravity provider implementation
//! 
//! Auth: None (local service)
//! API: Local HTTP probe
//! 
//! ## Data Available
//! - Daily request count
//! - Token usage (if available)
//! - Service status

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, NaiveDate};
use std::process::Command;

use crate::providers::traits::*;
use crate::storage::UsageData;

const ANTIGRAVITY_PORTS: &[u16] = &[8765, 8766, 8767, 3000, 3001];
const PROBE_TIMEOUT_SECS: u64 = 3;

pub struct AntigravityProvider {
    detected_port: Option<u16>,
    client: reqwest::Client,
}

#[derive(Deserialize, Debug)]
struct AntigravityStatus {
    running: Option<bool>,
    version: Option<String>,
    usage: Option<AntigravityUsage>,
    // Some versions use different field names
    status: Option<String>,
}

#[derive(Deserialize, Debug)]
struct AntigravityUsage {
    requests_today: Option<u64>,
    requests_limit: Option<u64>,
    tokens_used: Option<u64>,
    tokens_limit: Option<u64>,
    // Alternative field names
    daily_requests: Option<u64>,
    daily_limit: Option<u64>,
}

impl AntigravityProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(PROBE_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        
        Self {
            detected_port: None,
            client,
        }
    }
    
    fn is_process_running(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            Command::new("tasklist")
                .args(["/FI", "IMAGENAME eq antigravity.exe", "/FO", "CSV", "/NH"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains("antigravity"))
                .unwrap_or(false)
        }
        
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            Command::new("pgrep")
                .args(["-x", "antigravity"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    }
    
    async fn find_port(&mut self) -> Option<u16> {
        if let Some(port) = self.detected_port {
            // Verify cached port still works
            if self.probe_port(port).await.is_ok() {
                return Some(port);
            }
        }
        
        // Scan for Antigravity
        for port in ANTIGRAVITY_PORTS {
            if self.probe_port(*port).await.is_ok() {
                self.detected_port = Some(*port);
                return Some(*port);
            }
        }
        
        None
    }
    
    async fn probe_port(&self, port: u16) -> Result<(), ()> {
        let url = format!("http://localhost:{}/health", port);
        
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(()),
            _ => Err(()),
        }
    }
    
    fn get_next_midnight() -> DateTime<Utc> {
        let now = Utc::now();
        let tomorrow = (now.date_naive() + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap();
        tomorrow.and_utc()
    }
}

#[async_trait]
impl Provider for AntigravityProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "antigravity".to_string(),
            name: "Antigravity".to_string(),
            website: "https://antigravity.ai".to_string(),
            auth_methods: vec![AuthMethod::Local],
            has_session_limits: true,
            has_weekly_limits: true, // Daily, mapped to weekly
            has_credits: false,
            icon: "antigravity".to_string(),
        }
    }
    
    async fn is_authenticated(&self) -> bool {
        // For local services, "authenticated" means "service is running"
        self.is_process_running()
    }
    
    async fn fetch_usage(&self) -> ProviderResult<UsageData> {
        // First check if process is running
        if !self.is_process_running() {
            return Err(ProviderError::NotConfigured);
        }
        
        // Find the port
        let mut self_mut = Self {
            detected_port: self.detected_port,
            client: self.client.clone(),
        };
        
        let port = self_mut.find_port().await
            .ok_or(ProviderError::Provider("Antigravity service not found".into()))?;
        
        // Probe status
        let url = format!("http://localhost:{}/status", port);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        if !response.status().is_success() {
            // Try alternative endpoint
            let alt_url = format!("http://localhost:{}/api/status", port);
            let alt_response = self.client
                .get(&alt_url)
                .send()
                .await
                .map_err(|e| ProviderError::Network(e.to_string()))?;
            
            if !alt_response.status().is_success() {
                return Ok(UsageData {
                    session_used: 0,
                    session_limit: 0,
                    weekly_used: 0,
                    weekly_limit: 0,
                    credits_remaining: None,
                    reset_time: None,
                    weekly_reset_time: Some(Self::get_next_midnight()),
                    last_updated: Utc::now(),
                    error: Some("Service running but status unavailable".into()),
                });
            }
            
            // Try to parse alternative response
            if let Ok(status) = alt_response.json::<AntigravityStatus>().await {
                return self.parse_status(status);
            }
        }
        
        let status: AntigravityStatus = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        
        self.parse_status(status)
    }
    
    async fn start_auth(&mut self) -> ProviderResult<Option<AuthFlow>> {
        Ok(Some(AuthFlow {
            url: "https://antigravity.ai/download".to_string(),
            user_code: None,
            instructions: concat!(
                "Antigravity runs as a local service.\n\n",
                "1. Download and install Antigravity from antigravity.ai\n",
                "2. Start the Antigravity app\n",
                "3. Click 'Check status' below"
            ).to_string(),
            poll_interval: None,
        }))
    }
    
    async fn complete_auth(&mut self, _response: AuthResponse) -> ProviderResult<()> {
        // Just check if service is running
        if self.is_process_running() {
            // Try to find the port
            if self.find_port().await.is_some() {
                Ok(())
            } else {
                Err(ProviderError::Provider(
                    "Antigravity process found but couldn't connect to service".into()
                ))
            }
        } else {
            Err(ProviderError::NotConfigured)
        }
    }
    
    async fn logout(&mut self) -> ProviderResult<()> {
        // Nothing to clear for local service
        self.detected_port = None;
        Ok(())
    }
    
    fn auth_status(&self) -> AuthStatus {
        if self.is_process_running() {
            if self.detected_port.is_some() {
                AuthStatus::Authenticated {
                    user: Some(format!("localhost:{}", self.detected_port.unwrap())),
                    expires: None,
                }
            } else {
                AuthStatus::Authenticating {
                    message: "Searching for service...".into(),
                }
            }
        } else {
            AuthStatus::NotAuthenticated
        }
    }
}

impl AntigravityProvider {
    fn parse_status(&self, status: AntigravityStatus) -> ProviderResult<UsageData> {
        let usage = status.usage.unwrap_or(AntigravityUsage {
            requests_today: None,
            requests_limit: None,
            tokens_used: None,
            tokens_limit: None,
            daily_requests: None,
            daily_limit: None,
        });
        
        // Handle various field name formats
        let requests = usage.requests_today
            .or(usage.daily_requests)
            .unwrap_or(0);
        let requests_limit = usage.requests_limit
            .or(usage.daily_limit)
            .unwrap_or(0);
        
        Ok(UsageData {
            session_used: usage.tokens_used.unwrap_or(0),
            session_limit: usage.tokens_limit.unwrap_or(0),
            weekly_used: requests,
            weekly_limit: requests_limit,
            credits_remaining: None,
            reset_time: None,
            weekly_reset_time: Some(Self::get_next_midnight()),
            last_updated: Utc::now(),
            error: None,
        })
    }
}
```

---

## Frontend Integration

```tsx
// src/components/providers/AntigravityAuth.tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";

export function AntigravityAuth({ onComplete }: { onComplete: () => void }) {
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<"unknown" | "running" | "stopped">("unknown");

  async function checkStatus() {
    setChecking(true);
    setError(null);
    
    try {
      await invoke("complete_provider_auth", {
        provider: "antigravity",
        response: { OAuthCode: "" }
      });
      setStatus("running");
      onComplete();
    } catch (e) {
      const msg = String(e);
      if (msg.includes("NotConfigured")) {
        setStatus("stopped");
        setError("Antigravity is not running. Please start it first.");
      } else {
        setError(msg);
      }
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="auth-panel">
      <h3>Antigravity</h3>
      
      <div className={`status-indicator ${status}`}>
        {status === "running" && "✓ Running"}
        {status === "stopped" && "✗ Not running"}
        {status === "unknown" && "? Unknown"}
      </div>
      
      {error && <p className="error">{error}</p>}
      
      <div className="local-service">
        <p>Antigravity runs as a local service.</p>
        <ol>
          <li>
            <a href="#" onClick={() => open("https://antigravity.ai/download")}>
              Download Antigravity
            </a>
          </li>
          <li>Install and start the application</li>
          <li>Click below to check status</li>
        </ol>
        <button onClick={checkStatus} disabled={checking}>
          {checking ? "Checking..." : "Check Status"}
        </button>
      </div>
    </div>
  );
}
```

---

## Checklist

- [ ] Process detection (all platforms)
- [ ] Port scanning and caching
- [ ] Status endpoint probing
- [ ] Handle various response formats
- [ ] Graceful handling when service unavailable
- [ ] Frontend status component
- [ ] Tests passing

---

## Notes from CodexBar

From `CodexBar/Sources/CodexBarCore/Providers/Antigravity/`:
- Marked as "experimental" in CodexBar
- Uses local HTTP probe, no external auth
- Status endpoint format may vary by version
- Daily limits reset at midnight local time
