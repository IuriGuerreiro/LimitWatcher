//! Native notification system for usage alerts

use tauri::{AppHandle, Runtime, Manager};
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
    
    if let Some(tracker_state) = app.try_state::<Arc<RwLock<NotificationTracker>>>() {
        let mut tracker = tracker_state.write().await;
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