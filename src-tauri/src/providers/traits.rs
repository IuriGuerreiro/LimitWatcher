use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageData {
    pub session_used: u32,
    pub session_limit: u32,
    pub weekly_used: u32,
    pub weekly_limit: u32,
    pub reset_time: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthStatus {
    NotConfigured,
    Valid,
    Expired,
    Invalid,
}

#[async_trait]
pub trait Provider: Send + Sync {
    /// Returns the display name of the provider
    fn name(&self) -> &str;
    
    /// Returns the unique identifier for the provider
    fn id(&self) -> &str;
    
    /// Check if the provider is properly configured/authenticated
    async fn check_auth(&self) -> AuthStatus;
    
    /// Fetch current usage data from the provider
    async fn fetch_usage(&self) -> Result<UsageData, String>;
    
    /// Refresh authentication if needed
    async fn refresh_auth(&self) -> Result<(), String>;
}
