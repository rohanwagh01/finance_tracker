use crate::db::Db;
use crate::error::AppResult;
use crate::http::HttpClient;
use crate::providers::PlaidEnv;
use crate::secrets::SecretStore;
use crate::util::now;

/// Shared application state, managed by Tauri and injected into every command.
pub struct AppState {
    pub db: Db,
    pub secrets: SecretStore,
    pub http: HttpClient,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            secrets: SecretStore::new(),
            http: HttpClient::new(),
        }
    }

    // --- app_config helpers -------------------------------------------------

    pub async fn config_get(&self, key: &str) -> AppResult<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM app_config WHERE key = ?1")
                .bind(key)
                .fetch_optional(&self.db.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn config_get_or(&self, key: &str, default: &str) -> AppResult<String> {
        Ok(self.config_get(key).await?.unwrap_or_else(|| default.to_string()))
    }

    pub async fn config_set(&self, key: &str, value: &str) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(now())
        .execute(&self.db.pool)
        .await?;
        Ok(())
    }

    pub async fn plaid_env(&self) -> AppResult<PlaidEnv> {
        Ok(PlaidEnv::parse(&self.config_get_or("plaid_env", "sandbox").await?))
    }

    /// Whether a well-known credential is stored, answered from a non-secret
    /// registry in `app_config` so the startup UI never touches the keychain.
    /// Self-heals: the first lookup per key falls back to the vault (one
    /// keychain prompt, ever) and records the answer.
    pub async fn credential_present(&self, name: &str) -> AppResult<bool> {
        let cfg_key = format!("cred:{name}");
        match self.config_get(&cfg_key).await? {
            Some(v) => Ok(v == "1"),
            None => {
                let present = self.secrets.exists(name)?;
                self.config_set(&cfg_key, if present { "1" } else { "0" }).await?;
                Ok(present)
            }
        }
    }

    pub async fn set_credential_present(&self, name: &str, present: bool) -> AppResult<()> {
        self.config_set(&format!("cred:{name}"), if present { "1" } else { "0" })
            .await
    }
}
