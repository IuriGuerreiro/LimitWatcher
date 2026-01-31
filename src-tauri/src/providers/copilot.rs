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
const COPILOT_USAGE_URL: &str = "https://api.github.com/copilot_internal/user";
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
struct QuotaSnapshot {
    entitlement: f64,
    remaining: f64,
    percent_remaining: f64,
    quota_id: String,
}

#[derive(Deserialize, Debug)]
struct QuotaSnapshots {
    premium_interactions: Option<QuotaSnapshot>,
    chat: Option<QuotaSnapshot>,
}

#[derive(Deserialize, Debug)]
struct CopilotUsageResponse {
    quota_snapshots: QuotaSnapshots,
    copilot_plan: String,
    assigned_date: String,
    quota_reset_date: String,
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
            .header("Authorization", format!("token {}", token))
            .header("Accept", "application/json")
            .header("Editor-Version", "vscode/1.96.2")
            .header("Editor-Plugin-Version", "copilot-chat/0.26.7")
            .header("User-Agent", "GitHubCopilotChat/0.26.7")
            .header("X-Github-Api-Version", "2025-04-01")
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
        
        // Parse reset time
        let reset_time = DateTime::parse_from_rfc3339(&usage.quota_reset_date)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));
        
        // Extract chat quota (primary limit)
        let (chat_used, chat_limit) = if let Some(chat) = &usage.quota_snapshots.chat {
            let limit = chat.entitlement as u64;
            let used = (chat.entitlement - chat.remaining) as u64;
            (used, limit)
        } else {
            (0, 0)
        };
        
        // Extract premium interactions quota (secondary limit)
        let (premium_used, premium_limit) = if let Some(premium) = &usage.quota_snapshots.premium_interactions {
            let limit = premium.entitlement as u64;
            let used = (premium.entitlement - premium.remaining) as u64;
            (used, limit)
        } else {
            (0, 0)
        };
        
        Ok(UsageData {
            session_used: chat_used,
            session_limit: chat_limit,
            weekly_used: premium_used,
            weekly_limit: premium_limit,
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