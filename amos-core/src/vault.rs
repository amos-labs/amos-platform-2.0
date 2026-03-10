//! Credential Vault - AES-256-GCM encryption for sensitive secrets.
//!
//! Provides envelope encryption for credential storage. Each secret is encrypted
//! with a unique random nonce using AES-256-GCM. The master key is loaded from
//! the `AMOS__VAULT__MASTER_KEY` environment variable (base64-encoded 32 bytes).
//!
//! In production, the master key should be sourced from a KMS (AWS KMS, GCP KMS,
//! HashiCorp Vault, etc.) rather than a static env var.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, AeadCore, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::{AmosError, Result};

/// Size of AES-256-GCM nonce in bytes (96 bits).
const NONCE_SIZE: usize = 12;

/// Credential vault providing AES-256-GCM encryption/decryption.
#[derive(Clone)]
pub struct CredentialVault {
    cipher: Aes256Gcm,
}

impl CredentialVault {
    /// Create a vault from a base64-encoded 32-byte master key.
    pub fn from_base64_key(key_b64: &str) -> Result<Self> {
        let key_bytes = BASE64.decode(key_b64).map_err(|e| {
            AmosError::Internal(format!("Invalid vault master key (bad base64): {}", e))
        })?;
        if key_bytes.len() != 32 {
            return Err(AmosError::Internal(format!(
                "Vault master key must be 32 bytes, got {}",
                key_bytes.len()
            )));
        }
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        Ok(Self { cipher })
    }

    /// Create a vault from the `AMOS__VAULT__MASTER_KEY` environment variable.
    /// If the variable is not set, generates a random key (dev mode only) and
    /// logs a warning.
    pub fn from_env() -> Result<Self> {
        match std::env::var("AMOS__VAULT__MASTER_KEY") {
            Ok(key_b64) => Self::from_base64_key(&key_b64),
            Err(_) => {
                tracing::warn!(
                    "AMOS__VAULT__MASTER_KEY not set - generating ephemeral key. \
                     Encrypted credentials will NOT survive restarts!"
                );
                let key = Aes256Gcm::generate_key(OsRng);
                let cipher = Aes256Gcm::new(&key);
                Ok(Self { cipher })
            }
        }
    }

    /// Encrypt plaintext and return a base64-encoded blob (nonce || ciphertext).
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.cipher.encrypt(&nonce, plaintext).map_err(|e| {
            AmosError::Internal(format!("Encryption failed: {}", e))
        })?;

        // Concatenate nonce + ciphertext
        let mut blob = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        blob.extend_from_slice(nonce.as_slice());
        blob.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(&blob))
    }

    /// Decrypt a base64-encoded blob (nonce || ciphertext) back to plaintext.
    pub fn decrypt(&self, blob_b64: &str) -> Result<Vec<u8>> {
        let blob = BASE64.decode(blob_b64).map_err(|e| {
            AmosError::Internal(format!("Invalid encrypted blob (bad base64): {}", e))
        })?;
        if blob.len() < NONCE_SIZE {
            return Err(AmosError::Internal(
                "Encrypted blob too short (missing nonce)".into(),
            ));
        }

        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self.cipher.decrypt(nonce, ciphertext).map_err(|e| {
            AmosError::Internal(format!(
                "Decryption failed (wrong key or corrupted data): {}",
                e
            ))
        })?;

        Ok(plaintext)
    }

    /// Convenience: encrypt a string and return base64.
    pub fn encrypt_string(&self, plaintext: &str) -> Result<String> {
        self.encrypt(plaintext.as_bytes())
    }

    /// Convenience: decrypt base64 blob and return a String.
    pub fn decrypt_string(&self, blob_b64: &str) -> Result<String> {
        let bytes = self.decrypt(blob_b64)?;
        String::from_utf8(bytes).map_err(|e| {
            AmosError::Internal(format!("Decrypted data is not valid UTF-8: {}", e))
        })
    }

    /// Generate a new random 32-byte master key and return it as base64.
    /// Useful for initial setup / key rotation scripts.
    pub fn generate_master_key() -> String {
        let key = Aes256Gcm::generate_key(OsRng);
        BASE64.encode(key.as_slice())
    }
}

impl std::fmt::Debug for CredentialVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialVault")
            .field("cipher", &"<AES-256-GCM>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let key_b64 = CredentialVault::generate_master_key();
        let vault = CredentialVault::from_base64_key(&key_b64).unwrap();

        let secret = "sk_live_abc123_stripe_secret_key";
        let encrypted = vault.encrypt_string(secret).unwrap();

        // Encrypted should be different from plaintext
        assert_ne!(encrypted, secret);

        // Decrypt should recover original
        let decrypted = vault.decrypt_string(&encrypted).unwrap();
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn different_nonces_produce_different_ciphertext() {
        let key_b64 = CredentialVault::generate_master_key();
        let vault = CredentialVault::from_base64_key(&key_b64).unwrap();

        let secret = "same_plaintext";
        let enc1 = vault.encrypt_string(secret).unwrap();
        let enc2 = vault.encrypt_string(secret).unwrap();

        // Each encryption uses a random nonce, so ciphertexts differ
        assert_ne!(enc1, enc2);

        // Both decrypt to the same value
        assert_eq!(vault.decrypt_string(&enc1).unwrap(), secret);
        assert_eq!(vault.decrypt_string(&enc2).unwrap(), secret);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let vault1 = CredentialVault::from_base64_key(
            &CredentialVault::generate_master_key(),
        )
        .unwrap();
        let vault2 = CredentialVault::from_base64_key(
            &CredentialVault::generate_master_key(),
        )
        .unwrap();

        let encrypted = vault1.encrypt_string("secret").unwrap();
        assert!(vault2.decrypt_string(&encrypted).is_err());
    }

    #[test]
    fn rejects_bad_key_length() {
        let short_key = BASE64.encode(b"too_short");
        assert!(CredentialVault::from_base64_key(&short_key).is_err());
    }
}
