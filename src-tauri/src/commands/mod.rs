use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsage {
    pub provider: String,
    pub enabled: bool,
    pub session_used: u32,
    pub session_limit: u32,
    pub weekly_used: u32,
    pub weekly_limit: u32,
    pub reset_time: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn get_provider_status(provider: String) -> Result<ProviderUsage, String> {
    // TODO: Implement actual provider status fetching
    Ok(ProviderUsage {
        provider,
        enabled: true,
        session_used: 0,
        session_limit: 0,
        weekly_used: 0,
        weekly_limit: 0,
        reset_time: None,
        error: None,
    })
}

#[tauri::command]
pub async fn refresh_provider(provider: String) -> Result<(), String> {
    // TODO: Implement provider refresh
    println!("Refreshing provider: {}", provider);
    Ok(())
}

#[tauri::command]
pub async fn save_credentials(provider: String, _credentials: String) -> Result<(), String> {
    // TODO: Implement credential storage
    println!("Saving credentials for provider: {}", provider);
    Ok(())
}

#[tauri::command]
pub async fn get_all_usage() -> Result<Vec<ProviderUsage>, String> {
    // TODO: Implement fetching all provider usage
    Ok(vec![
        ProviderUsage {
            provider: "GitHub Copilot".to_string(),
            enabled: true,
            session_used: 0,
            session_limit: 0,
            weekly_used: 0,
            weekly_limit: 0,
            reset_time: None,
            error: Some("Not configured".to_string()),
        },
        ProviderUsage {
            provider: "Claude".to_string(),
            enabled: true,
            session_used: 0,
            session_limit: 0,
            weekly_used: 0,
            weekly_limit: 0,
            reset_time: None,
            error: Some("Not configured".to_string()),
        },
        ProviderUsage {
            provider: "Gemini".to_string(),
            enabled: true,
            session_used: 0,
            session_limit: 0,
            weekly_used: 0,
            weekly_limit: 0,
            reset_time: None,
            error: Some("Not configured".to_string()),
        },
        ProviderUsage {
            provider: "Antigravity".to_string(),
            enabled: false,
            session_used: 0,
            session_limit: 0,
            weekly_used: 0,
            weekly_limit: 0,
            reset_time: None,
            error: Some("Not configured".to_string()),
        },
    ])
}

#[tauri::command]
pub async fn set_provider_enabled(provider: String, enabled: bool) -> Result<(), String> {
    // TODO: Implement enabling/disabling providers
    println!("Setting provider {} enabled: {}", provider, enabled);
    Ok(())
}
