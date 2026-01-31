use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::storage::UsageData;

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
    async fn fetch_usage(&self) -> Result<UsageData, String>; // Using String for error for now, ideally strictly typed
    
    /// Refresh authentication if needed
    async fn refresh_auth(&self) -> Result<(), String>;
}