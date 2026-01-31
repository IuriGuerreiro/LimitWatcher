use async_trait::async_trait;
use crate::providers::traits::*;
use crate::storage::UsageData;

pub struct ClaudeProvider;

impl ClaudeProvider {
    pub fn new() -> Self {
        Self
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
