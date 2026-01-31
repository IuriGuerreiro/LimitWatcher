# Phase 2: Provider Framework

## Overview
Create the provider abstraction layer and registry system that allows multiple AI service providers to be implemented independently.

---

## Provider Trait

**File:** `src-tauri/src/providers/traits.rs`

```rust
//! Provider trait definition - implement this for each AI service

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::storage::UsageData;

/// Result type for provider operations
pub type ProviderResult<T> = Result<T, ProviderError>;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Authentication required")]
    AuthRequired,
    
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u64),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Provider error: {0}")]
    Provider(String),
    
    #[error("Not configured")]
    NotConfigured,
}

/// Authentication method supported by a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// OAuth2 device flow (user visits URL, enters code)
    DeviceFlow,
    /// Standard OAuth2 with redirect
    OAuth2,
    /// API key/token
    ApiKey,
    /// Browser cookies
    Cookies,
    /// Local service (no auth needed)
    Local,
    /// CLI-based (reads from CLI config)
    Cli,
}

/// Provider metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Unique identifier (lowercase, no spaces)
    pub id: String,
    /// Display name
    pub name: String,
    /// Provider website
    pub website: String,
    /// Supported auth methods (in order of preference)
    pub auth_methods: Vec<AuthMethod>,
    /// Whether provider supports session limits
    pub has_session_limits: bool,
    /// Whether provider supports weekly limits
    pub has_weekly_limits: bool,
    /// Whether provider supports credits
    pub has_credits: bool,
    /// Icon name (for frontend)
    pub icon: String,
}

/// Main provider trait - implement for each AI service
#[async_trait]
pub trait Provider: Send + Sync {
    /// Get provider metadata
    fn info(&self) -> ProviderInfo;
    
    /// Check if provider is authenticated
    async fn is_authenticated(&self) -> bool;
    
    /// Fetch current usage data
    async fn fetch_usage(&self) -> ProviderResult<UsageData>;
    
    /// Start authentication flow (returns URL for user if needed)
    async fn start_auth(&mut self) -> ProviderResult<Option<AuthFlow>>;
    
    /// Complete authentication (with code/token from user)
    async fn complete_auth(&mut self, response: AuthResponse) -> ProviderResult<()>;
    
    /// Logout / clear credentials
    async fn logout(&mut self) -> ProviderResult<()>;
    
    /// Get current auth status for display
    fn auth_status(&self) -> AuthStatus;
}

/// Authentication flow information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFlow {
    /// URL for user to visit
    pub url: String,
    /// Code to display (for device flow)
    pub user_code: Option<String>,
    /// Instructions for user
    pub instructions: String,
    /// Poll interval in seconds (for device flow)
    pub poll_interval: Option<u64>,
}

/// Response from user completing auth
#[derive(Debug, Clone, Deserialize)]
pub enum AuthResponse {
    /// OAuth callback code
    OAuthCode(String),
    /// Device flow completed (just poll for token)
    DeviceFlowComplete,
    /// API key entered
    ApiKey(String),
    /// Cookies pasted
    Cookies(String),
}

/// Current authentication status
#[derive(Debug, Clone, Serialize)]
pub enum AuthStatus {
    /// Not authenticated
    NotAuthenticated,
    /// Authentication in progress
    Authenticating { message: String },
    /// Authenticated successfully
    Authenticated { user: Option<String>, expires: Option<String> },
    /// Auth error
    Error { message: String },
}
```

---

## Provider Registry

**File:** `src-tauri/src/providers/mod.rs`

```rust
pub mod traits;
pub mod copilot;
pub mod claude;
pub mod gemini;
pub mod antigravity;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use traits::*;

/// Registry of all available providers
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<RwLock<dyn Provider>>>,
    enabled: HashMap<String, bool>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
            enabled: HashMap::new(),
        };
        
        // Register all providers
        registry.register(copilot::CopilotProvider::new());
        registry.register(claude::ClaudeProvider::new());
        registry.register(gemini::GeminiProvider::new());
        registry.register(antigravity::AntigravityProvider::new());
        
        registry
    }
    
    fn register<P: Provider + 'static>(&mut self, provider: P) {
        let info = provider.info();
        let id = info.id.clone();
        self.providers.insert(id.clone(), Arc::new(RwLock::new(provider)));
        self.enabled.insert(id, false); // Disabled by default
    }
    
    pub fn get_provider(&self, id: &str) -> Option<Arc<RwLock<dyn Provider>>> {
        self.providers.get(id).cloned()
    }
    
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.get(id).copied().unwrap_or(false)
    }
    
    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        if self.providers.contains_key(id) {
            self.enabled.insert(id.to_string(), enabled);
        }
    }
    
    pub fn all_provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
    
    pub fn enabled_providers(&self) -> Vec<(String, Arc<RwLock<dyn Provider>>)> {
        self.providers
            .iter()
            .filter(|(id, _)| self.is_enabled(id))
            .map(|(id, p)| (id.clone(), p.clone()))
            .collect()
    }
    
    pub async fn get_all_info(&self) -> Vec<(ProviderInfo, bool)> {
        let mut result = Vec::new();
        for (id, provider) in &self.providers {
            let p = provider.read().await;
            let enabled = self.is_enabled(id);
            result.push((p.info(), enabled));
        }
        result
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## Provider Implementation Template

Use this template when implementing a new provider:

```rust
//! [Provider Name] implementation
//! 
//! Auth method: [OAuth2/DeviceFlow/Cookies/etc]
//! API docs: [URL]
//! 
//! ## Authentication Flow
//! [Describe the auth flow]
//! 
//! ## Usage Data
//! [What data is available: session limits, weekly limits, credits, etc]

use async_trait::async_trait;
use crate::providers::traits::*;
use crate::storage::{UsageData, keyring};

pub struct MyProvider {
    // Access token / credentials
    token: Option<String>,
    // HTTP client
    client: reqwest::Client,
}

impl MyProvider {
    pub fn new() -> Self {
        Self {
            token: None,
            client: reqwest::Client::new(),
        }
    }
    
    async fn load_credentials(&mut self) {
        if let Ok(Some(token)) = keyring::get_credential("my_provider_token") {
            self.token = Some(token);
        }
    }
}

#[async_trait]
impl Provider for MyProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "my_provider".to_string(),
            name: "My Provider".to_string(),
            website: "https://example.com".to_string(),
            auth_methods: vec![AuthMethod::OAuth2],
            has_session_limits: true,
            has_weekly_limits: true,
            has_credits: false,
            icon: "my_provider".to_string(),
        }
    }
    
    async fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
    
    async fn fetch_usage(&self) -> ProviderResult<UsageData> {
        let token = self.token.as_ref().ok_or(ProviderError::AuthRequired)?;
        
        // Make API call to fetch usage
        // Parse response
        // Return UsageData
        
        todo!("Implement fetch_usage")
    }
    
    async fn start_auth(&mut self) -> ProviderResult<Option<AuthFlow>> {
        // Start OAuth flow, device flow, etc.
        todo!("Implement start_auth")
    }
    
    async fn complete_auth(&mut self, response: AuthResponse) -> ProviderResult<()> {
        // Complete auth and save credentials
        todo!("Implement complete_auth")
    }
    
    async fn logout(&mut self) -> ProviderResult<()> {
        self.token = None;
        keyring::delete_credential("my_provider_token")
            .map_err(|e| ProviderError::Provider(e.to_string()))?;
        Ok(())
    }
    
    fn auth_status(&self) -> AuthStatus {
        if self.token.is_some() {
            AuthStatus::Authenticated { user: None, expires: None }
        } else {
            AuthStatus::NotAuthenticated
        }
    }
}
```

---

## Individual Provider Docs

Each provider has its own detailed implementation document:

| Provider | Document | Auth Method | Priority |
|----------|----------|-------------|----------|
| GitHub Copilot | `providers/COPILOT.md` | Device Flow | Phase 1 |
| Claude | `providers/CLAUDE.md` | OAuth / Cookies | Phase 1 |
| Gemini | `providers/GEMINI.md` | OAuth (CLI) | Phase 1 |
| Antigravity | `providers/ANTIGRAVITY.md` | Local Probe | Phase 1 |
| Cursor | `providers/CURSOR.md` | Cookies | Phase 2 |
| z.ai | `providers/ZAI.md` | API Key | Phase 2 |
| Kiro | `providers/KIRO.md` | CLI | Phase 2 |
| OpenAI | `providers/OPENAI.md` | API Key | Phase 2 |

---

## Checklist

- [ ] Provider trait defined (`traits.rs`)
- [ ] Provider registry implemented (`mod.rs`)
- [ ] Phase 1 providers:
  - [ ] Copilot - See `providers/COPILOT.md`
  - [ ] Claude - See `providers/CLAUDE.md`
  - [ ] Gemini - See `providers/GEMINI.md`
  - [ ] Antigravity - See `providers/ANTIGRAVITY.md`
- [ ] Phase 2 providers (later):
  - [ ] Cursor
  - [ ] z.ai
  - [ ] Kiro
  - [ ] OpenAI

---

## Next Steps
- Implement individual providers using the docs in `providers/`
- **Phase 3:** UI & Notifications → See `PHASE-3-UI.md`
