pub mod traits;
use std::collections::HashMap;
use traits::Provider;

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
    enabled: HashMap<String, bool>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            enabled: HashMap::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let id = provider.id().to_string();
        self.enabled.insert(id.clone(), true); // Default enabled
        self.providers.insert(id, provider);
    }

    pub fn enabled_providers(&self) -> Vec<(String, &Box<dyn Provider>)> {
        self.providers.iter()
            .filter(|(id, _)| *self.enabled.get(*id).unwrap_or(&false))
            .map(|(id, p)| (id.clone(), p))
            .collect()
    }
    
    pub fn all_provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
    
    pub fn is_enabled(&self, provider: &str) -> bool {
        *self.enabled.get(provider).unwrap_or(&false)
    }
    
    pub fn get_provider(&self, provider: &str) -> Option<&Box<dyn Provider>> {
        self.providers.get(provider)
    }
    
    pub fn set_enabled(&mut self, provider: &str, enabled: bool) {
        if self.providers.contains_key(provider) {
            self.enabled.insert(provider.to_string(), enabled);
        }
    }
}