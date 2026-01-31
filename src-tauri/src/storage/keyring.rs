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
