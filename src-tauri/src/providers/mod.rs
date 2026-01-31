pub mod traits;
pub mod copilot;
pub mod claude;
pub mod gemini;
pub mod antigravity;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use traits::*;

/// Registry of all available providers
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<RwLock<dyn Provider>>>,
    enabled: HashMap<String, bool>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
            enabled: HashMap::new(),
        };
        
        // Register all providers
        registry.register(copilot::CopilotProvider::new());
        registry.register(claude::ClaudeProvider::new());
        registry.register(gemini::GeminiProvider::new());
        registry.register(antigravity::AntigravityProvider::new());
        
        registry
    }
    
    fn register<P: Provider + 'static>(&mut self, provider: P) {
        let info = provider.info();
        let id = info.id.clone();
        self.providers.insert(id.clone(), Arc::new(RwLock::new(provider)));
        self.enabled.insert(id, false); // Disabled by default
    }
    
    pub fn get_provider(&self, id: &str) -> Option<Arc<RwLock<dyn Provider>>> {
        self.providers.get(id).cloned()
    }
    
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.get(id).copied().unwrap_or(false)
    }
    
    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        if self.providers.contains_key(id) {
            self.enabled.insert(id.to_string(), enabled);
        }
    }
    
    pub fn all_provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
    
    pub fn enabled_providers(&self) -> Vec<(String, Arc<RwLock<dyn Provider>>)> {
        self.providers
            .iter()
            .filter(|(id, _)| self.is_enabled(id))
            .map(|(id, p)| (id.clone(), p.clone()))
            .collect()
    }
    
    pub async fn get_all_info(&self) -> Vec<(ProviderInfo, bool)> {
        let mut result = Vec::new();
        // We iterate over keys to get consistent order if needed, or just iterate map
        for (id, provider) in &self.providers {
            let p = provider.read().await;
            let enabled = self.is_enabled(id);
            result.push((p.info(), enabled));
        }
        result
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
