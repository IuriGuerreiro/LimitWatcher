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