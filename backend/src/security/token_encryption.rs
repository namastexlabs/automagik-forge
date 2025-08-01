use std::env;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Error, Debug)]
pub enum TokenEncryptionError {
    #[error("Failed to encrypt token: {0}")]
    EncryptionFailed(String),
    #[error("Failed to decrypt token: {0}")]
    DecryptionFailed(String),
    #[error("Invalid key configuration: {0}")]
    InvalidKey(String),
    #[error("Base64 decode error: {0}")]
    Base64DecodeError(#[from] base64::DecodeError),
}

/// Token encryption service for securing sensitive tokens in database
pub struct TokenEncryption {
    cipher: Aes256Gcm,
}

impl TokenEncryption {
    /// Create new token encryption service
    /// Uses GITHUB_TOKEN_ENCRYPTION_KEY environment variable for key
    /// Falls back to generating a key from JWT_SECRET if not set (not recommended for production)
    pub fn new() -> Result<Self, TokenEncryptionError> {
        let key = Self::get_encryption_key()?;
        let cipher = Aes256Gcm::new(&key);
        Ok(Self { cipher })
    }

    /// Get encryption key from environment
    fn get_encryption_key() -> Result<Key<Aes256Gcm>, TokenEncryptionError> {
        // Try to get dedicated encryption key first
        if let Ok(key_base64) = env::var("GITHUB_TOKEN_ENCRYPTION_KEY") {
            let key_bytes = STANDARD
                .decode(key_base64)
                .map_err(TokenEncryptionError::Base64DecodeError)?;
            
            if key_bytes.len() != 32 {
                return Err(TokenEncryptionError::InvalidKey(
                    "Encryption key must be 32 bytes (256 bits)".to_string(),
                ));
            }
            
            let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
            return Ok(*key);
        }

        // Fallback to deriving key from JWT secret (not ideal but better than no encryption)
        if let Ok(jwt_secret) = env::var("JWT_SECRET") {
            let mut key_bytes = [0u8; 32];
            
            // Use PBKDF2-like derivation (simplified for this case)
            let secret_bytes = jwt_secret.as_bytes();
            let salt = b"automagik-forge-token-encryption-salt";
            
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(secret_bytes);
            hasher.update(salt);
            let hash = hasher.finalize();
            
            key_bytes.copy_from_slice(&hash[..32]);
            let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
            
            tracing::warn!("Using JWT_SECRET for token encryption. Set GITHUB_TOKEN_ENCRYPTION_KEY for better security");
            return Ok(*key);
        }

        Err(TokenEncryptionError::InvalidKey(
            "No encryption key available. Set GITHUB_TOKEN_ENCRYPTION_KEY or JWT_SECRET".to_string(),
        ))
    }

    /// Encrypt a GitHub token for database storage
    pub fn encrypt_github_token(&self, token: &str) -> Result<String, TokenEncryptionError> {
        if token.is_empty() {
            return Ok(String::new());
        }

        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, token.as_bytes())
            .map_err(|e| TokenEncryptionError::EncryptionFailed(e.to_string()))?;

        // Combine nonce and ciphertext for storage
        let mut combined = Vec::with_capacity(nonce.len() + ciphertext.len());
        combined.extend_from_slice(&nonce);
        combined.extend_from_slice(&ciphertext);

        // Base64 encode for database storage
        Ok(STANDARD.encode(&combined))
    }

    /// Decrypt a GitHub token from database storage
    pub fn decrypt_github_token(&self, encrypted_token: &str) -> Result<String, TokenEncryptionError> {
        if encrypted_token.is_empty() {
            return Ok(String::new());
        }

        // Base64 decode
        let combined = STANDARD
            .decode(encrypted_token)
            .map_err(TokenEncryptionError::Base64DecodeError)?;

        if combined.len() < 12 {
            return Err(TokenEncryptionError::DecryptionFailed(
                "Invalid encrypted token format".to_string(),
            ));
        }

        // Split nonce and ciphertext (nonce is first 12 bytes for AES-GCM)
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| TokenEncryptionError::DecryptionFailed(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| TokenEncryptionError::DecryptionFailed(format!("Invalid UTF-8: {}", e)))
    }

    /// Generate a new encryption key (for key rotation)
    pub fn generate_new_key() -> String {
        let key = Aes256Gcm::generate_key(&mut OsRng);
        STANDARD.encode(key.as_slice())
    }

    /// Validate encryption key format
    pub fn validate_key(key_base64: &str) -> Result<(), TokenEncryptionError> {
        let key_bytes = STANDARD
            .decode(key_base64)
            .map_err(TokenEncryptionError::Base64DecodeError)?;
        
        if key_bytes.len() != 32 {
            return Err(TokenEncryptionError::InvalidKey(
                "Encryption key must be 32 bytes (256 bits)".to_string(),
            ));
        }
        
        Ok(())
    }
}

/// Secure string that zeros memory on drop
#[derive(Clone)]
pub struct SecureString {
    data: String,
}

impl SecureString {
    pub fn new(data: String) -> Self {
        Self { data }
    }

    pub fn as_str(&self) -> &str {
        &self.data
    }

    pub fn into_string(self) -> String {
        self.data
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        // Zero out the string data in memory
        unsafe {
            let bytes = self.data.as_bytes_mut();
            bytes.zeroize();
        }
    }
}

impl std::fmt::Debug for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecureString")
            .field("data", &"[REDACTED]")
            .finish()
    }
}

/// Helper function to encrypt token for user model updates
pub fn encrypt_token_for_storage(token: Option<&str>) -> Result<Option<String>, TokenEncryptionError> {
    match token {
        Some(token_str) if !token_str.is_empty() => {
            let encryption = TokenEncryption::new()?;
            Ok(Some(encryption.encrypt_github_token(token_str)?))
        }
        _ => Ok(None),
    }
}

/// Helper function to decrypt token from user model
pub fn decrypt_token_from_storage(encrypted_token: Option<&str>) -> Result<Option<SecureString>, TokenEncryptionError> {
    match encrypted_token {
        Some(encrypted_str) if !encrypted_str.is_empty() => {
            let encryption = TokenEncryption::new()?;
            let decrypted = encryption.decrypt_github_token(encrypted_str)?;
            Ok(Some(SecureString::new(decrypted)))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_key_generation_and_validation() {
        let key = TokenEncryption::generate_new_key();
        assert!(TokenEncryption::validate_key(&key).is_ok());
        
        // Test invalid key lengths
        assert!(TokenEncryption::validate_key("short").is_err());
        assert!(TokenEncryption::validate_key("").is_err());
    }

    #[test]
    fn test_encryption_decryption() {
        // Set up test environment
        env::set_var("JWT_SECRET", "test-jwt-secret-for-encryption-testing");
        
        let encryption = TokenEncryption::new().unwrap();
        let original_token = "ghp_test_token_1234567890abcdefghij";
        
        // Encrypt
        let encrypted = encryption.encrypt_github_token(original_token).unwrap();
        assert_ne!(encrypted, original_token);
        assert!(!encrypted.is_empty());
        
        // Decrypt
        let decrypted = encryption.decrypt_github_token(&encrypted).unwrap();
        assert_eq!(decrypted, original_token);
        
        // Clean up
        env::remove_var("JWT_SECRET");
    }

    #[test]
    fn test_empty_token_handling() {
        env::set_var("JWT_SECRET", "test-jwt-secret-for-empty-testing");
        
        let encryption = TokenEncryption::new().unwrap();
        
        // Empty token should return empty string
        let encrypted = encryption.encrypt_github_token("").unwrap();
        assert_eq!(encrypted, "");
        
        let decrypted = encryption.decrypt_github_token("").unwrap();
        assert_eq!(decrypted, "");
        
        env::remove_var("JWT_SECRET");
    }

    #[test]
    fn test_secure_string() {
        let secure = SecureString::new("sensitive_data".to_string());
        assert_eq!(secure.as_str(), "sensitive_data");
        
        // Test that debug doesn't reveal data
        let debug_str = format!("{:?}", secure);
        assert!(!debug_str.contains("sensitive_data"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_helper_functions() {
        env::set_var("JWT_SECRET", "test-jwt-secret-for-helpers");
        
        // Test encryption helper
        let encrypted = encrypt_token_for_storage(Some("test_token")).unwrap();
        assert!(encrypted.is_some());
        assert!(!encrypted.as_ref().unwrap().is_empty());
        
        // Test decryption helper
        let decrypted = decrypt_token_from_storage(encrypted.as_deref()).unwrap();
        assert!(decrypted.is_some());
        assert_eq!(decrypted.unwrap().as_str(), "test_token");
        
        // Test None handling
        let encrypted_none = encrypt_token_for_storage(None).unwrap();
        assert!(encrypted_none.is_none());
        
        let decrypted_none = decrypt_token_from_storage(None).unwrap();
        assert!(decrypted_none.is_none());
        
        env::remove_var("JWT_SECRET");
    }

    #[test]
    fn test_dedicated_encryption_key() {
        // Test with dedicated encryption key
        let test_key = TokenEncryption::generate_new_key();
        env::set_var("GITHUB_TOKEN_ENCRYPTION_KEY", &test_key);
        
        let encryption = TokenEncryption::new().unwrap();
        let token = "test_token_with_dedicated_key";
        
        let encrypted = encryption.encrypt_github_token(token).unwrap();
        let decrypted = encryption.decrypt_github_token(&encrypted).unwrap();
        
        assert_eq!(decrypted, token);
        
        env::remove_var("GITHUB_TOKEN_ENCRYPTION_KEY");
    }
}