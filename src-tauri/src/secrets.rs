//! API credentials and per-item access tokens. These live in a `secrets` table
//! *inside* the SQLCipher-encrypted database — so they're covered by the same
//! master-password encryption as everything else, with no OS keychain and no
//! separate file.

use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::util::now;

/// Well-known credential names. Per-item access tokens use `item.<uuid>`.
pub mod keys {
    pub const PLAID_CLIENT_ID: &str = "plaid.client_id";
    pub const PLAID_SECRET: &str = "plaid.secret";
    pub const ANTHROPIC_API_KEY: &str = "anthropic.api_key";
    pub const FINNHUB_API_KEY: &str = "finnhub.api_key";
    pub const MARKETAUX_API_KEY: &str = "marketaux.api_key";

    /// Credential names the settings UI knows how to collect.
    pub const WELL_KNOWN: &[&str] = &[
        PLAID_CLIENT_ID,
        PLAID_SECRET,
        ANTHROPIC_API_KEY,
        FINNHUB_API_KEY,
        MARKETAUX_API_KEY,
    ];
}

pub fn item_key(item_id: &str) -> String {
    format!("item.{item_id}")
}

#[derive(Clone)]
pub struct SecretStore {
    pool: SqlitePool,
}

impl SecretStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, name: &str) -> AppResult<Option<String>> {
        Ok(
            sqlx::query_scalar("SELECT value FROM secrets WHERE name = ?1")
                .bind(name)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn require(&self, name: &str) -> AppResult<String> {
        self.get(name)
            .await?
            .ok_or_else(|| AppError::Config(format!("missing credential: {name}")))
    }

    pub async fn exists(&self, name: &str) -> AppResult<bool> {
        Ok(self.get(name).await?.is_some())
    }

    pub async fn set(&self, name: &str, value: &str) -> AppResult<()> {
        if value.is_empty() {
            return Err(AppError::Invalid("secret value must not be empty".into()));
        }
        sqlx::query(
            "INSERT INTO secrets (name, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(name)
        .bind(value)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, name: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM secrets WHERE name = ?1")
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[tokio::test]
    async fn round_trips_secrets() {
        let db = Db::connect_in_memory().await.unwrap();
        let s = SecretStore::new(db.pool.clone());

        assert!(!s.exists("plaid.client_id").await.unwrap());
        assert!(s.require("plaid.client_id").await.is_err());

        s.set("plaid.client_id", "abc123").await.unwrap();
        assert_eq!(s.get("plaid.client_id").await.unwrap().as_deref(), Some("abc123"));
        assert!(s.exists("plaid.client_id").await.unwrap());

        s.set("plaid.client_id", "def456").await.unwrap();
        assert_eq!(s.require("plaid.client_id").await.unwrap(), "def456");

        s.delete("plaid.client_id").await.unwrap();
        assert!(!s.exists("plaid.client_id").await.unwrap());

        assert!(s.set("x", "").await.is_err());
    }
}
