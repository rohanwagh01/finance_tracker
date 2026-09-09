//! Master-password key derivation and the plaintext `vault.meta` sidecar.
//!
//! The database itself is SQLCipher-encrypted; the 32-byte key is
//! `Argon2id(master password, salt)` and is never written to disk. Only the
//! salt + KDF cost parameters live in `vault.meta` (they are not secret).

use std::path::Path;

use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const META_VERSION: u32 = 1;
// ~19 MiB, 3 passes, 1 lane — unlock is well under a second, brute force is not.
const M_COST: u32 = 19_456;
const T_COST: u32 = 3;
const P_COST: u32 = 1;
const SALT_LEN: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub salt_hex: String,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMeta {
    pub version: u32,
    pub kdf: KdfParams,
}

impl VaultMeta {
    pub fn generate() -> Self {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        Self {
            version: META_VERSION,
            kdf: KdfParams {
                salt_hex: hex::encode(salt),
                m_cost: M_COST,
                t_cost: T_COST,
                p_cost: P_COST,
            },
        }
    }
}

/// `Argon2id(password, salt)` → 32-byte SQLCipher key, hex-encoded.
pub fn derive_key_hex(password: &str, kdf: &KdfParams) -> AppResult<String> {
    let salt = hex::decode(&kdf.salt_hex)
        .map_err(|_| AppError::Config("vault.meta: malformed salt".into()))?;
    let params = Params::new(kdf.m_cost, kdf.t_cost, kdf.p_cost, Some(32))
        .map_err(|e| AppError::Other(format!("argon2 params: {e}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .map_err(|e| AppError::Other(format!("key derivation failed: {e}")))?;
    Ok(hex::encode(key))
}

pub fn read_meta(path: &Path) -> AppResult<VaultMeta> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("cannot read vault.meta: {e}")))?;
    serde_json::from_str(&raw).map_err(Into::into)
}

pub fn write_meta(path: &Path, meta: &VaultMeta) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(path, serde_json::to_vec_pretty(meta)?)
        .map_err(|e| AppError::Config(format!("cannot write vault.meta: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_deterministic_and_salt_sensitive() {
        let a = VaultMeta::generate();
        let k1 = derive_key_hex("correct horse battery staple", &a.kdf).unwrap();
        let k2 = derive_key_hex("correct horse battery staple", &a.kdf).unwrap();
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 64);

        let b = VaultMeta::generate();
        let k3 = derive_key_hex("correct horse battery staple", &b.kdf).unwrap();
        assert_ne!(k1, k3, "different salt must yield a different key");

        let k4 = derive_key_hex("wrong password", &a.kdf).unwrap();
        assert_ne!(k1, k4);
    }
}
