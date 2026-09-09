use std::path::PathBuf;

use tokio::sync::RwLock;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::http::HttpClient;
use crate::providers::PlaidEnv;
use crate::secrets::SecretStore;
use crate::util::now;

/// Where the encrypted database and its plaintext KDF sidecar live.
pub struct VaultPaths {
    pub meta: PathBuf,
    pub db: PathBuf,
}

/// Everything that only exists once the vault is unlocked.
struct Session {
    db: Db,
    secrets: SecretStore,
}

/// Shared application state, managed by Tauri and injected into every command.
/// Until the master password is entered, `session` is `None` and any command
/// that needs the database returns a "locked" error.
pub struct AppState {
    pub http: HttpClient,
    pub paths: VaultPaths,
    session: RwLock<Option<Session>>,
}

impl AppState {
    pub fn new(paths: VaultPaths) -> Self {
        Self {
            http: HttpClient::new(),
            paths,
            session: RwLock::new(None),
        }
    }

    // --- lock state -------------------------------------------------------

    pub async fn open_session(&self, db: Db) {
        let secrets = SecretStore::new(db.pool.clone());
        *self.session.write().await = Some(Session { db, secrets });
    }

    pub async fn close_session(&self) {
        let taken = self.session.write().await.take();
        if let Some(s) = taken {
            // Fully close the pool so the DB file is no longer held open
            // (matters before deleting it in vault_reset).
            s.db.pool.close().await;
        }
    }

    pub async fn is_unlocked(&self) -> bool {
        self.session.read().await.is_some()
    }

    pub async fn db(&self) -> AppResult<Db> {
        self.session
            .read()
            .await
            .as_ref()
            .map(|s| s.db.clone())
            .ok_or_else(|| AppError::Config("app is locked".into()))
    }

    pub async fn secrets(&self) -> AppResult<SecretStore> {
        self.session
            .read()
            .await
            .as_ref()
            .map(|s| s.secrets.clone())
            .ok_or_else(|| AppError::Config("app is locked".into()))
    }

    // --- app_config helpers ---------------------------------------------

    pub async fn config_get(&self, key: &str) -> AppResult<Option<String>> {
        let db = self.db().await?;
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_config WHERE key = ?1")
            .bind(key)
            .fetch_optional(&db.pool)
            .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn config_get_or(&self, key: &str, default: &str) -> AppResult<String> {
        Ok(self
            .config_get(key)
            .await?
            .unwrap_or_else(|| default.to_string()))
    }

    pub async fn config_set(&self, key: &str, value: &str) -> AppResult<()> {
        let db = self.db().await?;
        sqlx::query(
            "INSERT INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(now())
        .execute(&db.pool)
        .await?;
        Ok(())
    }

    pub async fn plaid_env(&self) -> AppResult<PlaidEnv> {
        Ok(PlaidEnv::parse(
            &self.config_get_or("plaid_env", "sandbox").await?,
        ))
    }

    pub async fn credential_present(&self, name: &str) -> AppResult<bool> {
        self.secrets().await?.exists(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn locked_state_refuses_db_and_secrets() {
        let st = AppState::new(VaultPaths {
            meta: "/tmp/ft-test-meta".into(),
            db: "/tmp/ft-test-db".into(),
        });
        assert!(!st.is_unlocked().await);
        assert!(matches!(st.db().await, Err(AppError::Config(_))));
        assert!(matches!(st.secrets().await, Err(AppError::Config(_))));

        let db = Db::connect_in_memory().await.unwrap();
        st.open_session(db).await;
        assert!(st.is_unlocked().await);
        assert!(st.db().await.is_ok());
        assert!(st.secrets().await.is_ok());

        st.close_session().await;
        assert!(matches!(st.db().await, Err(AppError::Config(_))));
    }
}
