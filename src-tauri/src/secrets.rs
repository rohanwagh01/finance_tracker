//! Secret storage. Every API credential and per-item access token is kept in a
//! SINGLE OS-keychain entry holding a JSON map, loaded once per process and
//! cached in memory. This keeps macOS keychain prompts to (at most) one per app
//! launch instead of one per credential — which matters a lot under `tauri dev`,
//! where each rebuild invalidates the keychain ACL.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use keyring::Entry;

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.financetracker.app";
const VAULT_ACCOUNT: &str = "vault";

/// Well-known credential names. Per-item access tokens use `item.<uuid>`.
pub mod keys {
    pub const PLAID_CLIENT_ID: &str = "plaid.client_id";
    pub const PLAID_SECRET: &str = "plaid.secret";
    pub const SNAPTRADE_CLIENT_ID: &str = "snaptrade.client_id";
    pub const SNAPTRADE_CONSUMER_KEY: &str = "snaptrade.consumer_key";
    pub const ANTHROPIC_API_KEY: &str = "anthropic.api_key";
    pub const FINNHUB_API_KEY: &str = "finnhub.api_key";
    pub const MARKETAUX_API_KEY: &str = "marketaux.api_key";

    /// Credential names the settings UI knows how to collect.
    pub const WELL_KNOWN: &[&str] = &[
        PLAID_CLIENT_ID,
        PLAID_SECRET,
        SNAPTRADE_CLIENT_ID,
        SNAPTRADE_CONSUMER_KEY,
        ANTHROPIC_API_KEY,
        FINNHUB_API_KEY,
        MARKETAUX_API_KEY,
    ];
}

pub fn item_key(item_id: &str) -> String {
    format!("item.{item_id}")
}

#[derive(Default)]
struct Vault {
    loaded: bool,
    map: HashMap<String, String>,
}

#[derive(Clone)]
pub struct SecretStore {
    inner: Arc<Mutex<Vault>>,
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vault::default())),
        }
    }

    fn entry() -> AppResult<Entry> {
        Ok(Entry::new(SERVICE, VAULT_ACCOUNT)?)
    }

    /// Load the vault JSON from the keychain the first time it's touched.
    fn ensure_loaded(&self, v: &mut Vault) -> AppResult<()> {
        if v.loaded {
            return Ok(());
        }
        match Self::entry()?.get_password() {
            Ok(json) => {
                v.map = serde_json::from_str(&json).unwrap_or_default();
            }
            Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(e.into()),
        }
        v.loaded = true;
        Ok(())
    }

    /// Pull a value out of a pre-vault per-key keychain entry (written by an
    /// earlier version) and fold it into the vault. Returns the value if found.
    fn migrate_legacy(v: &mut Vault, name: &str) -> AppResult<Option<String>> {
        let Ok(legacy) = Entry::new(SERVICE, name) else {
            return Ok(None);
        };
        match legacy.get_password() {
            Ok(val) => {
                v.map.insert(name.to_string(), val.clone());
                let _ = legacy.delete_credential();
                Self::persist(v)?;
                Ok(Some(val))
            }
            Err(_) => Ok(None),
        }
    }

    fn persist(v: &Vault) -> AppResult<()> {
        let json = serde_json::to_string(&v.map)?;
        Self::entry()?.set_password(&json)?;
        Ok(())
    }

    pub fn set(&self, name: &str, value: &str) -> AppResult<()> {
        if value.is_empty() {
            return Err(AppError::Invalid("secret value must not be empty".into()));
        }
        let mut v = self.inner.lock().unwrap();
        self.ensure_loaded(&mut v)?;
        v.map.insert(name.to_string(), value.to_string());
        Self::persist(&v)
    }

    pub fn get(&self, name: &str) -> AppResult<Option<String>> {
        let mut v = self.inner.lock().unwrap();
        self.ensure_loaded(&mut v)?;
        if let Some(val) = v.map.get(name) {
            return Ok(Some(val.clone()));
        }
        // Fall back to a legacy per-key entry (covers `item.*` tokens and any
        // credential written before the vault existed).
        Self::migrate_legacy(&mut v, name)
    }

    pub fn require(&self, name: &str) -> AppResult<String> {
        self.get(name)?
            .ok_or_else(|| AppError::Config(format!("missing credential: {name}")))
    }

    pub fn exists(&self, name: &str) -> AppResult<bool> {
        Ok(self.get(name)?.is_some())
    }

    pub fn delete(&self, name: &str) -> AppResult<()> {
        let mut v = self.inner.lock().unwrap();
        self.ensure_loaded(&mut v)?;
        if v.map.remove(name).is_some() {
            Self::persist(&v)?;
        }
        Ok(())
    }
}
