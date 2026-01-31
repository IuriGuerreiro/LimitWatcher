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
