use async_trait::async_trait;
use crate::providers::traits::*;
use crate::storage::UsageData;

pub struct GeminiProvider;

impl GeminiProvider {
    pub fn new() -> Self {
        Self
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
