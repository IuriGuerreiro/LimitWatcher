use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

pub struct NotificationManager {
    app: AppHandle,
}

impl NotificationManager {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
    
    /// Send a usage alert notification
    pub fn send_usage_alert(&self, provider: &str, percentage: u32, level: AlertLevel) {
        let (title, body) = match level {
            AlertLevel::Info => (
                format!("{} Usage Update", provider),
                format!("{}% of your limit used", percentage),
            ),
            AlertLevel::Warning => (
                format!("⚠️ {} Usage Warning", provider),
                format!("{}% of your limit used - consider slowing down", percentage),
            ),
            AlertLevel::Critical => (
                format!("🚨 {} Usage Critical", provider),
                format!("{}% of your limit used - approaching limit!", percentage),
            ),
        };
        
        if let Err(e) = self.app.notification()
            .builder()
            .title(&title)
            .body(&body)
            .show()
        {
            eprintln!("Failed to send notification: {}", e);
        }
    }
    
    /// Send a generic notification
    pub fn send(&self, title: &str, body: &str) {
        if let Err(e) = self.app.notification()
            .builder()
            .title(title)
            .body(body)
            .show()
        {
            eprintln!("Failed to send notification: {}", e);
        }
    }
}
