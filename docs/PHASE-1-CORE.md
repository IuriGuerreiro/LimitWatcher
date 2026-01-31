# Phase 1: Core Infrastructure

## Overview
Implement the foundational systems: secure storage, system tray, settings management, and background scheduler.

---

## 1. Storage Layer

### 1.1 Keyring (OS Keychain)

**File:** `src-tauri/src/storage/keyring.rs`

```rust
//! OS Keychain integration for secure credential storage
//! - Windows: Credential Manager
//! - macOS: Keychain
//! - Linux: Secret Service (GNOME Keyring / KWallet)

use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE_NAME: &str = "com.limitswatcher";

#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, KeyringError>;

/// Store a credential in the OS keychain
pub fn store_credential(key: &str, value: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, key)?;
    entry.set_password(value)?;
    Ok(())
}

/// Retrieve a credential from the OS keychain
pub fn get_credential(key: &str) -> Result<Option<String>> {
    let entry = Entry::new(SERVICE_NAME, key)?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Delete a credential from the OS keychain
pub fn delete_credential(key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, key)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
        Err(e) => Err(e.into()),
    }
}

/// Store a structured credential (serialized as JSON)
pub fn store_credential_json<T: Serialize>(key: &str, value: &T) -> Result<()> {
    let json = serde_json::to_string(value)?;
    store_credential(key, &json)
}

/// Retrieve a structured credential (deserialized from JSON)
pub fn get_credential_json<T: for<'de> Deserialize<'de>>(key: &str) -> Result<Option<T>> {
    match get_credential(key)? {
        Some(json) => Ok(Some(serde_json::from_str(&json)?)),
        None => Ok(None),
    }
}

// Provider-specific key helpers
pub mod keys {
    pub const COPILOT_TOKEN: &str = "copilot_access_token";
    pub const CLAUDE_OAUTH: &str = "claude_oauth_token";
    pub const CLAUDE_COOKIES: &str = "claude_cookies";
    pub const GEMINI_OAUTH: &str = "gemini_oauth_token";
    pub const ANTIGRAVITY_CONFIG: &str = "antigravity_config";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_roundtrip() {
        let key = "test_credential";
        let value = "test_value_12345";
        
        store_credential(key, value).unwrap();
        let retrieved = get_credential(key).unwrap();
        assert_eq!(retrieved, Some(value.to_string()));
        
        delete_credential(key).unwrap();
        let deleted = get_credential(key).unwrap();
        assert_eq!(deleted, None);
    }
}
```

---

### 1.2 Encrypted Storage

**File:** `src-tauri/src/storage/encrypted.rs`

```rust
//! AES-256-GCM encrypted file storage for cookies and sensitive bulk data

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const NONCE_SIZE: usize = 12;
const SALT_SIZE: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum EncryptedStorageError {
    #[error("Encryption error")]
    Encryption,
    #[error("Decryption error")]
    Decryption,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("Key derivation error")]
    KeyDerivation,
}

pub type Result<T> = std::result::Result<T, EncryptedStorageError>;

#[derive(Serialize, Deserialize)]
struct EncryptedBlob {
    salt: String,      // Base64 encoded
    nonce: String,     // Base64 encoded
    ciphertext: String, // Base64 encoded
}

/// Derive a 256-bit key from a password using Argon2id
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let salt_string = SaltString::encode_b64(salt)
        .map_err(|_| EncryptedStorageError::KeyDerivation)?;
    
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt_string)
        .map_err(|_| EncryptedStorageError::KeyDerivation)?;
    
    let hash_bytes = hash.hash.ok_or(EncryptedStorageError::KeyDerivation)?;
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash_bytes.as_bytes()[..32]);
    Ok(key)
}

/// Get machine-specific identifier for key derivation
fn get_machine_id() -> String {
    // Use combination of factors for machine binding
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    
    let username = whoami::username();
    
    format!("{}@{}", username, hostname)
}

/// Encrypt data and save to file
pub fn encrypt_to_file<T: Serialize>(path: &PathBuf, data: &T, password: Option<&str>) -> Result<()> {
    let json = serde_json::to_string(data)?;
    
    // Use password or machine ID for key derivation
    let key_source = password
        .map(|p| p.to_string())
        .unwrap_or_else(get_machine_id);
    
    // Generate random salt and nonce
    let mut salt = [0u8; SALT_SIZE];
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);
    
    // Derive key and encrypt
    let key = derive_key(&key_source, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| EncryptedStorageError::Encryption)?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher
        .encrypt(nonce, json.as_bytes())
        .map_err(|_| EncryptedStorageError::Encryption)?;
    
    // Create blob and save
    let blob = EncryptedBlob {
        salt: BASE64.encode(salt),
        nonce: BASE64.encode(nonce_bytes),
        ciphertext: BASE64.encode(ciphertext),
    };
    
    let blob_json = serde_json::to_string_pretty(&blob)?;
    
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    fs::write(path, blob_json)?;
    Ok(())
}

/// Decrypt data from file
pub fn decrypt_from_file<T: for<'de> Deserialize<'de>>(path: &PathBuf, password: Option<&str>) -> Result<T> {
    let blob_json = fs::read_to_string(path)?;
    let blob: EncryptedBlob = serde_json::from_str(&blob_json)?;
    
    let salt = BASE64.decode(&blob.salt)?;
    let nonce_bytes = BASE64.decode(&blob.nonce)?;
    let ciphertext = BASE64.decode(&blob.ciphertext)?;
    
    // Use password or machine ID for key derivation
    let key_source = password
        .map(|p| p.to_string())
        .unwrap_or_else(get_machine_id);
    
    // Derive key and decrypt
    let key = derive_key(&key_source, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| EncryptedStorageError::Decryption)?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| EncryptedStorageError::Decryption)?;
    
    let json = String::from_utf8(plaintext)
        .map_err(|_| EncryptedStorageError::Decryption)?;
    
    Ok(serde_json::from_str(&json)?)
}

/// Check if encrypted file exists
pub fn exists(path: &PathBuf) -> bool {
    path.exists()
}

/// Delete encrypted file
pub fn delete(path: &PathBuf) -> Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}
```

---

### 1.3 Usage Cache

**File:** `src-tauri/src/storage/cache.rs`

```rust
//! Non-sensitive usage data cache for offline display

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageData {
    pub session_used: u64,
    pub session_limit: u64,
    pub weekly_used: u64,
    pub weekly_limit: u64,
    pub credits_remaining: Option<u64>,
    pub reset_time: Option<DateTime<Utc>>,
    pub weekly_reset_time: Option<DateTime<Utc>>,
    pub last_updated: DateTime<Utc>,
    pub error: Option<String>,
}

impl Default for UsageData {
    fn default() -> Self {
        Self {
            session_used: 0,
            session_limit: 0,
            weekly_used: 0,
            weekly_limit: 0,
            credits_remaining: None,
            reset_time: None,
            weekly_reset_time: None,
            last_updated: Utc::now(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageCache {
    pub providers: HashMap<String, UsageData>,
}

pub struct CacheManager {
    path: PathBuf,
    cache: UsageCache,
}

impl CacheManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let path = app_data_dir.join("usage_cache.json");
        let cache = Self::load_from_file(&path).unwrap_or_default();
        
        Self { path, cache }
    }
    
    fn load_from_file(path: &PathBuf) -> Option<UsageCache> {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }
    
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.cache)?;
        fs::write(&self.path, json)
    }
    
    pub fn get(&self, provider: &str) -> Option<&UsageData> {
        self.cache.providers.get(provider)
    }
    
    pub fn set(&mut self, provider: &str, data: UsageData) {
        self.cache.providers.insert(provider.to_string(), data);
    }
    
    pub fn get_all(&self) -> &HashMap<String, UsageData> {
        &self.cache.providers
    }
    
    pub fn clear_provider(&mut self, provider: &str) {
        self.cache.providers.remove(provider);
    }
    
    pub fn clear_all(&mut self) {
        self.cache.providers.clear();
    }
}
```

---

### 1.4 Storage Module

**File:** `src-tauri/src/storage/mod.rs`

```rust
pub mod keyring;
pub mod encrypted;
pub mod cache;

pub use cache::{CacheManager, UsageCache, UsageData};
```

---

## 2. System Tray

**File:** `src-tauri/src/tray.rs`

```rust
//! System tray management with dynamic icons and menus

use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    image::Image,
};

pub fn init<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit LimitsWatcher", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh All", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    
    let menu = Menu::with_items(app, &[&show, &refresh, &separator, &quit])?;
    
    // Load tray icon
    let icon = Image::from_path("icons/tray.png")
        .unwrap_or_else(|_| Image::from_bytes(include_bytes!("../icons/tray.png")).unwrap());
    
    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("LimitsWatcher - AI Usage Tracker")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "refresh" => {
                    // Emit refresh event to frontend
                    let _ = app.emit("refresh-all", ());
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // Left click: show main window
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                _ => {}
            }
        })
        .build(app)?;
    
    Ok(())
}

/// Update tray icon based on usage status
pub fn update_icon<R: Runtime>(app: &tauri::AppHandle<R>, status: TrayStatus) {
    // TODO: Generate dynamic icon based on usage bars
    // For now, just update tooltip
    if let Some(tray) = app.tray_by_id("main") {
        let tooltip = match status {
            TrayStatus::Ok { summary } => format!("LimitsWatcher\n{}", summary),
            TrayStatus::Warning { message } => format!("⚠️ {}", message),
            TrayStatus::Error { message } => format!("❌ {}", message),
        };
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}

pub enum TrayStatus {
    Ok { summary: String },
    Warning { message: String },
    Error { message: String },
}
```

---

## 3. Background Scheduler

**File:** `src-tauri/src/scheduler.rs`

```rust
//! Background refresh scheduler with configurable intervals

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, MissedTickBehavior};
use tauri::{AppHandle, Manager, Runtime};

use crate::providers::ProviderRegistry;
use crate::storage::CacheManager;
use crate::notifications;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshInterval {
    Manual,
    OneMinute,
    TwoMinutes,
    FiveMinutes,
    FifteenMinutes,
}

impl RefreshInterval {
    pub fn to_duration(&self) -> Option<Duration> {
        match self {
            RefreshInterval::Manual => None,
            RefreshInterval::OneMinute => Some(Duration::from_secs(60)),
            RefreshInterval::TwoMinutes => Some(Duration::from_secs(120)),
            RefreshInterval::FiveMinutes => Some(Duration::from_secs(300)),
            RefreshInterval::FifteenMinutes => Some(Duration::from_secs(900)),
        }
    }
    
    pub fn from_str(s: &str) -> Self {
        match s {
            "1m" => RefreshInterval::OneMinute,
            "2m" => RefreshInterval::TwoMinutes,
            "5m" => RefreshInterval::FiveMinutes,
            "15m" => RefreshInterval::FifteenMinutes,
            _ => RefreshInterval::Manual,
        }
    }
}

pub struct Scheduler {
    interval: Arc<RwLock<RefreshInterval>>,
    running: Arc<RwLock<bool>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            interval: Arc::new(RwLock::new(RefreshInterval::FiveMinutes)),
            running: Arc::new(RwLock::new(true)),
        }
    }
    
    pub async fn set_interval(&self, interval: RefreshInterval) {
        *self.interval.write().await = interval;
    }
    
    pub async fn stop(&self) {
        *self.running.write().await = false;
    }
}

pub async fn start<R: Runtime>(app: AppHandle<R>) {
    let scheduler = Scheduler::new();
    
    // Store scheduler in app state
    app.manage(Arc::new(scheduler.clone()));
    
    // Main refresh loop
    loop {
        let current_interval = *scheduler.interval.read().await;
        
        if !*scheduler.running.read().await {
            break;
        }
        
        if let Some(duration) = current_interval.to_duration() {
            // Wait for interval
            tokio::time::sleep(duration).await;
            
            // Refresh all enabled providers
            refresh_all_providers(&app).await;
        } else {
            // Manual mode: just sleep and check again
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}

async fn refresh_all_providers<R: Runtime>(app: &AppHandle<R>) {
    // Get provider registry from state
    if let Some(registry) = app.try_state::<Arc<RwLock<ProviderRegistry>>>() {
        let registry = registry.read().await;
        
        for (name, provider) in registry.enabled_providers() {
            match provider.fetch_usage().await {
                Ok(usage) => {
                    // Update cache
                    if let Some(cache) = app.try_state::<Arc<RwLock<CacheManager>>>() {
                        let mut cache = cache.write().await;
                        cache.set(&name, usage.clone());
                        let _ = cache.save();
                    }
                    
                    // Check for low usage warnings
                    check_usage_warnings(app, &name, &usage).await;
                    
                    // Emit update event
                    let _ = app.emit("provider-updated", (&name, &usage));
                }
                Err(e) => {
                    log::error!("Failed to refresh {}: {}", name, e);
                    let _ = app.emit("provider-error", (&name, e.to_string()));
                }
            }
        }
    }
}

async fn check_usage_warnings<R: Runtime>(
    app: &AppHandle<R>,
    provider: &str,
    usage: &crate::storage::UsageData,
) {
    // Check if session usage is above 80%
    if usage.session_limit > 0 {
        let percent = (usage.session_used as f64 / usage.session_limit as f64) * 100.0;
        if percent >= 80.0 {
            notifications::send_warning(
                app,
                &format!("{} Usage Warning", provider),
                &format!("Session usage at {:.0}% ({}/{})", percent, usage.session_used, usage.session_limit),
            ).await;
        }
    }
    
    // Check weekly usage
    if usage.weekly_limit > 0 {
        let percent = (usage.weekly_used as f64 / usage.weekly_limit as f64) * 100.0;
        if percent >= 90.0 {
            notifications::send_warning(
                app,
                &format!("{} Weekly Limit", provider),
                &format!("Weekly usage at {:.0}%", percent),
            ).await;
        }
    }
}
```

---

## 4. Notifications

**File:** `src-tauri/src/notifications.rs`

```rust
//! Native notification system for usage alerts

use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Track sent notifications to avoid spam
pub struct NotificationTracker {
    sent: HashSet<String>,
}

impl NotificationTracker {
    pub fn new() -> Self {
        Self {
            sent: HashSet::new(),
        }
    }
    
    /// Check if notification was already sent (within current session)
    pub fn was_sent(&self, key: &str) -> bool {
        self.sent.contains(key)
    }
    
    /// Mark notification as sent
    pub fn mark_sent(&mut self, key: &str) {
        self.sent.insert(key.to_string());
    }
    
    /// Reset tracker (call on new day or manual reset)
    pub fn reset(&mut self) {
        self.sent.clear();
    }
}

/// Send a warning notification
pub async fn send_warning<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    // Check tracker to avoid spam
    let notification_key = format!("{}:{}", title, body);
    
    if let Some(tracker) = app.try_state::<Arc<RwLock<NotificationTracker>>>() {
        let mut tracker = tracker.write().await;
        if tracker.was_sent(&notification_key) {
            return;
        }
        tracker.mark_sent(&notification_key);
    }
    
    // Send notification
    if let Err(e) = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show()
    {
        log::error!("Failed to send notification: {}", e);
    }
}

/// Send an error notification
pub async fn send_error<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    if let Err(e) = app
        .notification()
        .builder()
        .title(&format!("❌ {}", title))
        .body(body)
        .show()
    {
        log::error!("Failed to send notification: {}", e);
    }
}

/// Send an info notification
pub async fn send_info<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    if let Err(e) = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show()
    {
        log::error!("Failed to send notification: {}", e);
    }
}
```

---

## 5. Commands (IPC)

**File:** `src-tauri/src/commands/mod.rs`

```rust
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
```

---

## Checklist

- [ ] Storage layer implemented
  - [ ] `keyring.rs` - OS Keychain integration
  - [ ] `encrypted.rs` - AES-256-GCM file encryption
  - [ ] `cache.rs` - Usage data cache
- [ ] System tray working
  - [ ] Basic menu (Show, Refresh, Quit)
  - [ ] Left-click shows main window
  - [ ] Tooltip updates
- [ ] Background scheduler running
  - [ ] Configurable intervals
  - [ ] Auto-refresh enabled providers
- [ ] Notifications working
  - [ ] Usage warnings (80%/90% thresholds)
  - [ ] Error notifications
  - [ ] Spam prevention
- [ ] Commands (IPC) implemented
  - [ ] `get_all_usage`
  - [ ] `refresh_provider`
  - [ ] `save_credentials`
  - [ ] `set_provider_enabled`

---

## Next Steps
- **Phase 2:** Provider implementations → See `PHASE-2-PROVIDERS.md`
- Individual providers: See `providers/` docs
