//! Tauri commands for frontend communication

use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::State;

use crate::providers::{ProviderRegistry, AuthFlow, AuthResponse};
use crate::storage::{CacheManager, UsageData, ModelQuota, keyring};

#[derive(serde::Serialize)]
pub struct ProviderStatus {
    pub provider: String,
    pub enabled: bool,
    pub authenticated: bool,
    pub session_used: u64,
    pub session_limit: u64,
    pub weekly_used: u64,
    pub weekly_limit: u64,
    pub reset_time: Option<String>,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_quotas: Option<Vec<ModelQuota>>,
}

impl From<(&str, &UsageData, bool, bool)> for ProviderStatus {
    fn from((name, data, enabled, authenticated): (&str, &UsageData, bool, bool)) -> Self {
        Self {
            provider: name.to_string(),
            enabled,
            authenticated,
            session_used: data.session_used,
            session_limit: data.session_limit,
            weekly_used: data.weekly_used,
            weekly_limit: data.weekly_limit,
            reset_time: data.reset_time.map(|t| t.to_rfc3339()),
            error: data.error.clone(),
            model_quotas: data.model_quotas.clone(),
        }
    }
}

#[tauri::command]
pub async fn start_provider_auth(
    provider: String,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<Option<AuthFlow>, String> {
    // Get provider from registry
    let provider_arc = {
        let registry_guard = registry.read().await;
        registry_guard.get_provider(&provider)
    };
    
    if let Some(p_arc) = provider_arc {
        let mut p = p_arc.write().await;
        p.start_auth().await.map_err(|e| e.to_string())
    } else {
        Err(format!("Provider '{}' not found", provider))
    }
}

#[tauri::command]
pub async fn complete_provider_auth(
    provider: String,
    response: AuthResponse,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<(), String> {
    // Get provider from registry
    let provider_arc = {
        let registry_guard = registry.read().await;
        registry_guard.get_provider(&provider)
    };
    
    if let Some(p_arc) = provider_arc {
        let mut p = p_arc.write().await;
        p.complete_auth(response).await.map_err(|e| e.to_string())
    } else {
        Err(format!("Provider '{}' not found", provider))
    }
}

#[tauri::command]
pub async fn get_provider_status(
    provider: String,
    cache: State<'_, Arc<RwLock<CacheManager>>>,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<ProviderStatus, String> {
    let cache = cache.read().await;
    let registry_read = registry.read().await;
    
    let enabled = registry_read.is_enabled(&provider);
    let data = cache.get(&provider).cloned().unwrap_or_default();
    
    let authenticated = if let Some(p_arc) = registry_read.get_provider(&provider) {
        let p = p_arc.read().await;
        p.is_authenticated().await
    } else {
        false
    };
    
    Ok(ProviderStatus::from((provider.as_str(), &data, enabled, authenticated)))
}

#[tauri::command]
pub async fn get_all_usage(
    cache: State<'_, Arc<RwLock<CacheManager>>>,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<Vec<ProviderStatus>, String> {
    let cache = cache.read().await;
    let registry_read = registry.read().await;
    
    let mut statuses = Vec::new();
    
    for name in registry_read.all_provider_names() {
        let enabled = registry_read.is_enabled(&name);
        let data = cache.get(&name).cloned().unwrap_or_default();
        
        let authenticated = if let Some(p_arc) = registry_read.get_provider(&name) {
            let p = p_arc.read().await;
            p.is_authenticated().await
        } else {
            false
        };
        
        statuses.push(ProviderStatus::from((name.as_str(), &data, enabled, authenticated)));
    }
    
    Ok(statuses)
}

#[tauri::command]
pub async fn refresh_provider(
    provider: String,
    cache: State<'_, Arc<RwLock<CacheManager>>>,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<ProviderStatus, String> {
    // Get provider from registry (holding lock briefly)
    let provider_arc = {
        let registry_guard = registry.read().await;
        registry_guard.get_provider(&provider)
    };
    
    if let Some(p_arc) = provider_arc {
        let p = p_arc.read().await;
        match p.fetch_usage().await {
            Ok(usage) => {
                let mut cache = cache.write().await;
                cache.set(&provider, usage.clone());
                let _ = cache.save();
                
                Ok(ProviderStatus::from((provider.as_str(), &usage, true, true)))
            }
            Err(e) => Err(e.to_string()),
        }
    } else {
        Err(format!("Provider '{}' not found", provider))
    }
}

#[tauri::command]
pub async fn save_credentials(
    provider: String,
    credential_type: String,
    value: String,
) -> Result<(), String> {
    let key = format!("{}_{}", provider, credential_type);
    keyring::store_credential(&key, &value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_provider_enabled(
    provider: String,
    enabled: bool,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<(), String> {
    let mut registry = registry.write().await;
    registry.set_enabled(&provider, enabled);
    Ok(())
}

#[tauri::command]
pub async fn logout_provider(
    provider: String,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<(), String> {
    let provider_arc = {
        let registry_guard = registry.read().await;
        registry_guard.get_provider(&provider)
    };

    if let Some(p_arc) = provider_arc {
        let mut p = p_arc.write().await;
        p.logout().await.map_err(|e| e.to_string())
    } else {
        Err(format!("Provider '{}' not found", provider))
    }
}

#[derive(serde::Serialize)]
pub struct AuthStatusResponse {
    pub authenticated: bool,
    pub user: Option<String>,
    pub plan: Option<String>,
    pub expires: Option<String>,
}

#[tauri::command]
pub async fn get_provider_auth_status(
    provider: String,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<AuthStatusResponse, String> {
    let provider_arc = {
        let registry_guard = registry.read().await;
        registry_guard.get_provider(&provider)
    };

    if let Some(p_arc) = provider_arc {
        let p = p_arc.read().await;
        let status = p.auth_status();

        match status {
            crate::providers::traits::AuthStatus::Authenticated { user, expires } => {
                // Parse user string to extract email and plan
                // Format from gemini.rs: "email@example.com (Plan)"
                let (email, plan) = if let Some(user_str) = &user {
                    if let Some(paren_pos) = user_str.rfind(" (") {
                        let email = user_str[..paren_pos].to_string();
                        let plan = user_str[paren_pos + 2..user_str.len() - 1].to_string();
                        (Some(email), Some(plan))
                    } else {
                        (Some(user_str.clone()), None)
                    }
                } else {
                    (None, None)
                };

                Ok(AuthStatusResponse {
                    authenticated: true,
                    user: email,
                    plan,
                    expires,
                })
            }
            _ => Ok(AuthStatusResponse {
                authenticated: false,
                user: None,
                plan: None,
                expires: None,
            }),
        }
    } else {
        Err(format!("Provider '{}' not found", provider))
    }
}
