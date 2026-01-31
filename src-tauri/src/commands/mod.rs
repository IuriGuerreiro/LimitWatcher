//! Tauri commands for frontend communication

use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::State;

use crate::providers::ProviderRegistry;
use crate::storage::{CacheManager, UsageData, keyring};

#[derive(serde::Serialize)]
pub struct ProviderStatus {
    pub provider: String,
    pub enabled: bool,
    pub session_used: u64,
    pub session_limit: u64,
    pub weekly_used: u64,
    pub weekly_limit: u64,
    pub reset_time: Option<String>,
    pub error: Option<String>,
}

impl From<(&str, &UsageData, bool)> for ProviderStatus {
    fn from((name, data, enabled): (&str, &UsageData, bool)) -> Self {
        Self {
            provider: name.to_string(),
            enabled,
            session_used: data.session_used,
            session_limit: data.session_limit,
            weekly_used: data.weekly_used,
            weekly_limit: data.weekly_limit,
            reset_time: data.reset_time.map(|t| t.to_rfc3339()),
            error: data.error.clone(),
        }
    }
}

#[tauri::command]
pub async fn get_provider_status(
    provider: String,
    cache: State<'_, Arc<RwLock<CacheManager>>>,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<ProviderStatus, String> {
    let cache = cache.read().await;
    let registry = registry.read().await;
    
    let enabled = registry.is_enabled(&provider);
    let data = cache.get(&provider).cloned().unwrap_or_default();
    
    Ok(ProviderStatus::from((provider.as_str(), &data, enabled)))
}

#[tauri::command]
pub async fn get_all_usage(
    cache: State<'_, Arc<RwLock<CacheManager>>>,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<Vec<ProviderStatus>, String> {
    let cache = cache.read().await;
    let registry = registry.read().await;
    
    let mut statuses = Vec::new();
    
    for name in registry.all_provider_names() {
        let enabled = registry.is_enabled(&name);
        let data = cache.get(&name).cloned().unwrap_or_default();
        statuses.push(ProviderStatus::from((name.as_str(), &data, enabled)));
    }
    
    Ok(statuses)
}

#[tauri::command]
pub async fn refresh_provider(
    provider: String,
    cache: State<'_, Arc<RwLock<CacheManager>>>,
    registry: State<'_, Arc<RwLock<ProviderRegistry>>>,
) -> Result<ProviderStatus, String> {
    let registry = registry.read().await;
    
    if let Some(p) = registry.get_provider(&provider) {
        match p.fetch_usage().await {
            Ok(usage) => {
                let mut cache = cache.write().await;
                cache.set(&provider, usage.clone());
                let _ = cache.save();
                
                Ok(ProviderStatus::from((provider.as_str(), &usage, true)))
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