//! Secrets at rest (ADR 0008).
//!
//! - [`SecretBox`] encrypts reversible secrets (SIP and trunk passwords) with
//!   XChaCha20-Poly1305 under `TALKOPS_SECRET_KEY`. Stored format:
//!   `v1:<base64 nonce>:<base64 ciphertext+tag>`.
//! - [`hash_password`] / [`verify_password`] handle web login passwords (argon2id).
//! - [`random_token`] / [`random_password`] produce session tokens and generated
//!   SIP passwords from the OS RNG.

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

const VERSION: &str = "v1";

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("TALKOPS_SECRET_KEY must be 64 hex characters (32 bytes)")]
    InvalidKey,
    #[error("malformed encrypted value")]
    Malformed,
    #[error("decryption failed (wrong TALKOPS_SECRET_KEY or tampered value)")]
    Decrypt,
    #[error("operating system RNG unavailable")]
    Rng,
    #[error("password hashing failed")]
    Hash,
}

/// Symmetric encryption for secrets stored in the database.
#[derive(Clone)]
pub struct SecretBox {
    cipher: XChaCha20Poly1305,
}

impl std::fmt::Debug for SecretBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretBox(..)")
    }
}

impl SecretBox {
    /// Creates a box from the hex-encoded 256-bit key.
    pub fn from_hex(key_hex: &str) -> Result<Self, CryptoError> {
        let key = hex::decode(key_hex.trim()).map_err(|_| CryptoError::InvalidKey)?;
        if key.len() != 32 {
            return Err(CryptoError::InvalidKey);
        }
        let cipher =
            XChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::InvalidKey)?;
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, CryptoError> {
        let mut nonce = [0u8; 24];
        getrandom::fill(&mut nonce).map_err(|_| CryptoError::Rng)?;
        let ciphertext = self
            .cipher
            .encrypt(&XNonce::from(nonce), plaintext.as_bytes())
            .map_err(|_| CryptoError::Decrypt)?;
        Ok(format!(
            "{VERSION}:{}:{}",
            STANDARD.encode(nonce),
            STANDARD.encode(ciphertext)
        ))
    }

    pub fn decrypt(&self, stored: &str) -> Result<String, CryptoError> {
        let mut parts = stored.splitn(3, ':');
        let (Some(VERSION), Some(nonce), Some(ciphertext)) =
            (parts.next(), parts.next(), parts.next())
        else {
            return Err(CryptoError::Malformed);
        };
        let nonce: [u8; 24] = STANDARD
            .decode(nonce)
            .ok()
            .and_then(|n| n.try_into().ok())
            .ok_or(CryptoError::Malformed)?;
        let ciphertext = STANDARD
            .decode(ciphertext)
            .map_err(|_| CryptoError::Malformed)?;
        let plaintext = self
            .cipher
            .decrypt(&XNonce::from(nonce), ciphertext.as_slice())
            .map_err(|_| CryptoError::Decrypt)?;
        String::from_utf8(plaintext).map_err(|_| CryptoError::Malformed)
    }
}

/// Hashes a web login password with argon2id (PHC string).
pub fn hash_password(password: &str) -> Result<String, CryptoError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|_| CryptoError::Hash)
}

/// Verifies a password against a stored PHC string. Malformed hashes never match.
pub fn verify_password(password: &str, phc: &str) -> bool {
    PasswordHash::new(phc)
        .map(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
        .unwrap_or(false)
}

/// URL-safe random token with `bytes` bytes of entropy.
/// Random bytes from the OS generator.
pub fn random_bytes(n: usize) -> Result<Vec<u8>, CryptoError> {
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf).map_err(|_| CryptoError::Rng)?;
    Ok(buf)
}

pub fn random_token(bytes: usize) -> Result<String, CryptoError> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|_| CryptoError::Rng)?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

/// Random SIP password: letters and digits only, because some phones and
/// provisioning formats choke on special characters.
pub fn random_password(len: usize) -> Result<String, CryptoError> {
    const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut out = String::with_capacity(len);
    let mut buf = [0u8; 64];
    while out.len() < len {
        getrandom::fill(&mut buf).map_err(|_| CryptoError::Rng)?;
        // Rejection sampling keeps the distribution uniform.
        let limit = 256 - (256 % ALPHABET.len());
        for b in buf.iter().map(|&b| b as usize).filter(|&b| b < limit) {
            if out.len() == len {
                break;
            }
            out.push(ALPHABET[b % ALPHABET.len()] as char);
        }
    }
    Ok(out)
}

/// SHA-256 digest of a session token, hex encoded. Only the digest is stored,
/// so a database leak does not reveal usable session tokens.
pub fn token_digest(token: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    #[test]
    fn secret_roundtrip_and_tamper_detection() {
        let sb = SecretBox::from_hex(KEY).unwrap();
        let a = sb.encrypt("geheim€").unwrap();
        let b = sb.encrypt("geheim€").unwrap();
        assert_ne!(a, b, "nonces must differ");
        assert!(a.starts_with("v1:"));
        assert_eq!(sb.decrypt(&a).unwrap(), "geheim€");

        let mut tampered = a.clone();
        let last = tampered.pop().unwrap();
        tampered.push(if last == 'A' { 'B' } else { 'A' });
        assert!(sb.decrypt(&tampered).is_err());

        let other = SecretBox::from_hex(&KEY.replace("00", "ff")).unwrap();
        assert!(matches!(other.decrypt(&a), Err(CryptoError::Decrypt)));
        assert!(matches!(sb.decrypt("garbage"), Err(CryptoError::Malformed)));
    }

    #[test]
    fn rejects_bad_keys() {
        assert!(SecretBox::from_hex("abcd").is_err());
        assert!(SecretBox::from_hex(&"zz".repeat(32)).is_err());
    }

    #[test]
    fn password_hashing() {
        let hash = hash_password("correct horse").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("correct horse", &hash));
        assert!(!verify_password("wrong", &hash));
        assert!(!verify_password("x", "not-a-hash"));
    }

    #[test]
    fn random_values() {
        let p = random_password(20).unwrap();
        assert_eq!(p.len(), 20);
        assert!(p.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(random_token(32).unwrap(), random_token(32).unwrap());
        assert_eq!(token_digest("abc").len(), 64);
    }
}
