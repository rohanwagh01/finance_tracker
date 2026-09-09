//! OS keychain wrapper. Every API credential and per-item access token lives
//! here, never in SQLite and never sent to the webview.

use keyring::Entry;

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.financetracker.app";

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

#[allow(dead_code)] // used by the item sync flow (milestone 2)
pub fn item_key(item_id: &str) -> String {
    format!("item.{item_id}")
}

#[derive(Clone, Default)]
pub struct SecretStore;

impl SecretStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(&self, name: &str) -> AppResult<Entry> {
        Ok(Entry::new(SERVICE, name)?)
    }

    pub fn set(&self, name: &str, value: &str) -> AppResult<()> {
        if value.is_empty() {
            return Err(AppError::Invalid("secret value must not be empty".into()));
        }
        self.entry(name)?.set_password(value)?;
        Ok(())
    }

    pub fn get(&self, name: &str) -> AppResult<Option<String>> {
        match self.entry(name)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Fetch a required secret or fail with a configuration error.
    pub fn require(&self, name: &str) -> AppResult<String> {
        self.get(name)?
            .ok_or_else(|| AppError::Config(format!("missing credential: {name}")))
    }

    pub fn exists(&self, name: &str) -> AppResult<bool> {
        Ok(self.get(name)?.is_some())
    }

    pub fn delete(&self, name: &str) -> AppResult<()> {
        match self.entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
