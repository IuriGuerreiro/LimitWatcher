use async_trait::async_trait;
use crate::providers::traits::*;
use crate::storage::UsageData;

pub struct AntigravityProvider;

impl AntigravityProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Provider for AntigravityProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "antigravity".to_string(),
            name: "Antigravity".to_string(),
            website: "https://antigravity.ai".to_string(), // Placeholder URL
            auth_methods: vec![AuthMethod::Local],
            has_session_limits: false,
            has_weekly_limits: false,
            has_credits: true,
            icon: "antigravity".to_string(),
        }
    }
    
    async fn is_authenticated(&self) -> bool {
        false
    }
    
    async fn fetch_usage(&self) -> ProviderResult<UsageData> {
        Err(ProviderError::NotConfigured)
    }
    
    async fn start_auth(&mut self) -> ProviderResult<Option<AuthFlow>> {
        Err(ProviderError::NotConfigured)
    }
    
    async fn complete_auth(&mut self, _response: AuthResponse) -> ProviderResult<()> {
        Err(ProviderError::NotConfigured)
    }
    
    async fn logout(&mut self) -> ProviderResult<()> {
        Ok(())
    }
    
    fn auth_status(&self) -> AuthStatus {
        AuthStatus::NotAuthenticated
    }
}
