mod commands;
mod notifications;
mod providers;
mod scheduler;
mod storage;
mod tray;

use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            // Initialize State
            let app_data_dir = app.path().app_data_dir().expect("failed to get app data dir");
            let cache_manager = storage::CacheManager::new(app_data_dir);
            app.manage(Arc::new(RwLock::new(cache_manager)));

            let provider_registry = providers::ProviderRegistry::new();
            app.manage(Arc::new(RwLock::new(provider_registry)));

            let notification_tracker = notifications::NotificationTracker::new();
            app.manage(Arc::new(RwLock::new(notification_tracker)));

            // Initialize system tray
            tray::init(app)?;

            // Start background scheduler
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                scheduler::start(handle).await;
            });

            // Hide dock icon on macOS (menu bar app style)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_provider_status,
            commands::refresh_provider,
            commands::save_credentials,
            commands::get_all_usage,
            commands::set_provider_enabled,
            commands::start_provider_auth,
            commands::complete_provider_auth,
            commands::logout_provider,
            commands::get_provider_auth_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
