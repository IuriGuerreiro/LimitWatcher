//! GitHub Copilot provider implementation
//! 
//! Auth: GitHub Device Flow OAuth
//! API: GitHub Copilot internal usage API
//! 
//! ## Data Available
//! - Chat completions (session)
//! - Premium requests (monthly for Pro)
//! - Reset timestamps

use async_trait::async_trait;
use serde::Deserialize;
use chrono::{DateTime, Utc};

use crate::providers::traits::*;
use crate::storage::{UsageData, keyring};

const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const COPILOT_USAGE_URL: &str = "https://api.github.com/copilot/usage";
const CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

pub struct CopilotProvider {
    token: Option<String>,
    client: reqwest::Client,
    pending_device_code: Option<String>,
    poll_interval: u64,
}

impl CopilotProvider {
    pub fn new() -> Self {
        let mut provider = Self {
            token: None,
            client: reqwest::Client::new(),
            pending_device_code: None,
            poll_interval: 5,
        };
        
        // Try to load saved token
        if let Ok(Some(token)) = keyring::get_credential(keyring::keys::COPILOT_TOKEN) {
            provider.token = Some(token);
        }
        
        provider
    }
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CopilotUsageResponse {
    chat_completions: Option<u64>,
    chat_completions_limit: Option<u64>,
    premium_requests: Option<u64>,
    premium_requests_limit: Option<u64>,
    resets_at: Option<String>,
    copilot_plan: Option<String>,
}

#[async_trait]
impl Provider for CopilotProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "copilot".to_string(),
            name: "GitHub Copilot".to_string(),
            website: "https://github.com/features/copilot".to_string(),
            auth_methods: vec![AuthMethod::DeviceFlow],
            has_session_limits: true,
            has_weekly_limits: true, // Monthly actually, but fits the model
            has_credits: false,
            icon: "copilot".to_string(),
        }
    }
    
    async fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
    
    async fn fetch_usage(&self) -> ProviderResult<UsageData> {
        let token = self.token.as_ref().ok_or(ProviderError::AuthRequired)?;
        
        let response = self.client
            .get(COPILOT_USAGE_URL)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "LimitsWatcher/1.0")
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        match response.status().as_u16() {
            401 => return Err(ProviderError::TokenExpired),
            403 => return Err(ProviderError::AuthFailed("Copilot not enabled".into())),
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
        
        let usage: CopilotUsageResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        
        let reset_time = usage.resets_at.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });
        
        Ok(UsageData {
            session_used: usage.chat_completions.unwrap_or(0),
            session_limit: usage.chat_completions_limit.unwrap_or(0),
            weekly_used: usage.premium_requests.unwrap_or(0),
            weekly_limit: usage.premium_requests_limit.unwrap_or(0),
            credits_remaining: None,
            reset_time,
            weekly_reset_time: reset_time,
            last_updated: Utc::now(),
            error: None,
        })
    }
    
    async fn start_auth(&mut self) -> ProviderResult<Option<AuthFlow>> {
        let response = self.client
            .post(GITHUB_DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", CLIENT_ID),
                ("scope", "read:user"),
            ])
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        let device_code: DeviceCodeResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;
        
        self.pending_device_code = Some(device_code.device_code.clone());
        self.poll_interval = device_code.interval;
        
        Ok(Some(AuthFlow {
            url: device_code.verification_uri,
            user_code: Some(device_code.user_code),
            instructions: "Visit the URL and enter the code to authenticate with GitHub.".into(),
            poll_interval: Some(device_code.interval),
        }))
    }
    
    async fn complete_auth(&mut self, _response: AuthResponse) -> ProviderResult<()> {
        let device_code = self.pending_device_code.take()
            .ok_or(ProviderError::AuthFailed("No pending auth".into()))?;
        
        // Poll for token
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(self.poll_interval)).await;
            
            let response = self.client
                .post(GITHUB_TOKEN_URL)
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", CLIENT_ID),
                    ("device_code", &device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(|e| ProviderError::Network(e.to_string()))?;
            
            let token_response: TokenResponse = response
                .json()
                .await
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            
            if let Some(token) = token_response.access_token {
                // Save token
                keyring::store_credential(keyring::keys::COPILOT_TOKEN, &token)
                    .map_err(|e| ProviderError::Provider(e.to_string()))?;
                self.token = Some(token.clone());
                return Ok(());
            }
            
            match token_response.error.as_deref() {
                Some("authorization_pending") => continue,
                Some("slow_down") => {
                    self.poll_interval += 5;
                    continue;
                }
                Some("expired_token") => {
                    return Err(ProviderError::AuthFailed("Device code expired".into()));
                }
                Some(error) => {
                    return Err(ProviderError::AuthFailed(
                        token_response.error_description.unwrap_or_else(|| error.to_string())
                    ));
                }
                None => continue,
            }
        }
    }
    
    async fn logout(&mut self) -> ProviderResult<()> {
        self.token = None;
        self.pending_device_code = None;
        keyring::delete_credential(keyring::keys::COPILOT_TOKEN)
            .map_err(|e| ProviderError::Provider(e.to_string()))?;
        Ok(())
    }
    
    fn auth_status(&self) -> AuthStatus {
        if self.pending_device_code.is_some() {
            AuthStatus::Authenticating {
                message: "Waiting for GitHub authorization...".into(),
            }
        } else if self.token.is_some() {
            AuthStatus::Authenticated {
                user: None,
                expires: None,
            }
        } else {
            AuthStatus::NotAuthenticated
        }
    }
}