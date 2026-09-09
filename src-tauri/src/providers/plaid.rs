//! Plaid client (read-only). Implemented incrementally:
//! milestone 2 adds Link token creation, public-token exchange, and
//! `/accounts/balance/get` + `/transactions/sync` + `/liabilities/get`.

use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::http::HttpClient;
use crate::secrets::{keys, SecretStore};

use super::PlaidEnv;

pub struct PlaidClient {
    http: HttpClient,
    env: PlaidEnv,
    client_id: String,
    secret: String,
}

impl PlaidClient {
    /// Build from stored credentials. Errors if Plaid isn't configured yet.
    pub fn from_secrets(http: HttpClient, secrets: &SecretStore, env: PlaidEnv) -> AppResult<Self> {
        Ok(Self {
            http,
            env,
            client_id: secrets.require(keys::PLAID_CLIENT_ID)?,
            secret: secrets.require(keys::PLAID_SECRET)?,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.env.base_url(), path)
    }

    /// POST a JSON body with the client_id/secret injected, return parsed JSON.
    async fn post(&self, path: &str, mut body: serde_json::Value) -> AppResult<serde_json::Value> {
        if let Some(obj) = body.as_object_mut() {
            obj.insert("client_id".into(), json!(self.client_id));
            obj.insert("secret".into(), json!(self.secret));
        }
        let resp = self.http.post(&self.url(path))?.json(&body).send().await?;
        let status = resp.status();
        let value: serde_json::Value = resp.json().await?;
        if !status.is_success() {
            let message = value
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown Plaid error")
                .to_string();
            return Err(AppError::Provider { provider: "plaid".into(), message });
        }
        Ok(value)
    }

    /// Lightweight connectivity + credential check used by the settings screen.
    /// `/institutions/get` validates the client_id/secret without needing an Item.
    pub async fn health_check(&self) -> AppResult<()> {
        self.post(
            "/institutions/get",
            json!({ "count": 1, "offset": 0, "country_codes": ["US"] }),
        )
        .await
        .map(|_| ())
    }
}
