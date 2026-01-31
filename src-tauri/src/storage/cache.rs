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