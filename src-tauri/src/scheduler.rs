//! Background refresh scheduler with configurable intervals

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tauri::{AppHandle, Manager, Runtime, Emitter};

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

#[derive(Clone)]
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
    app.manage(Arc::new(scheduler.clone())); // Store Arc<Scheduler> directly? No, manage handles T. 
                                            // The code in PHASE-1-CORE uses Arc<Scheduler> but the method signature of manage is manage<T>(state: T).
                                            // Wait, `app.manage` takes T. If I pass Arc<Scheduler>, retrieving it needs to be app.state::<Arc<Scheduler>>().
                                            // The code below uses app.manage(Arc::new(scheduler)). 
                                            // Then in loop it accesses scheduler directly. 
                                            // To access from other commands, we'll need to know the type.
                                            // Let's assume Arc<Scheduler> is the intended state type.
    
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
        
        for (name, provider_arc) in registry.enabled_providers() {
            let provider = provider_arc.read().await;
            match provider.fetch_usage().await {
                Ok(usage) => {
                    // Update cache
                    if let Some(cache) = app.try_state::<Arc<RwLock<CacheManager>>>() {
                        let mut cache = cache.write().await;
                        cache.set(name.as_str(), usage.clone());
                        let _ = cache.save();
                    }
                    
                    // Check for low usage warnings
                    check_usage_warnings(app, &name, &usage).await;
                    
                    // Emit update event
                    let _ = app.emit("provider-updated", (name.as_str(), &usage));
                }
                Err(e) => {
                    log::error!("Failed to refresh {}: {}", name, e);
                    let _ = app.emit("provider-error", (name.as_str(), e.to_string()));
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