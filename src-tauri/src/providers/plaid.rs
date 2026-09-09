//! Plaid client — read-only. Milestone 2: Hosted Link, public-token exchange,
//! `/accounts/get` + `/accounts/balance/get`, cursor-based `/transactions/sync`,
//! `/item/get` + `/institutions/get_by_id`, `/item/remove`.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::http::HttpClient;
use crate::secrets::{keys, SecretStore};

use super::PlaidEnv;

const CLIENT_NAME: &str = "Finance Tracker";

pub struct PlaidClient {
    http: HttpClient,
    env: PlaidEnv,
    client_id: String,
    secret: String,
}

// --- response shapes we consume -------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PlaidAccount {
    pub account_id: String,
    pub name: String,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    #[serde(rename = "type")]
    pub account_type: String,
    pub subtype: Option<String>,
    #[serde(default)]
    pub balances: PlaidBalances,
}

#[derive(Debug, Default, Deserialize)]
pub struct PlaidBalances {
    pub current: Option<f64>,
    pub available: Option<f64>,
    pub limit: Option<f64>,
    pub iso_currency_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlaidTransaction {
    pub transaction_id: String,
    pub account_id: String,
    pub date: String,
    pub authorized_date: Option<String>,
    pub amount: f64,
    pub iso_currency_code: Option<String>,
    pub name: String,
    pub merchant_name: Option<String>,
    #[serde(default)]
    pub pending: bool,
    pub personal_finance_category: Option<PlaidPfc>,
    pub location: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct PlaidPfc {
    pub primary: String,
    pub detailed: String,
}

#[derive(Debug, Deserialize)]
pub struct TransactionsSyncPage {
    #[serde(default)]
    pub added: Vec<PlaidTransaction>,
    #[serde(default)]
    pub modified: Vec<PlaidTransaction>,
    #[serde(default)]
    pub removed: Vec<RemovedTransaction>,
    pub next_cursor: String,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct RemovedTransaction {
    pub transaction_id: String,
}

// --- client -------------------------------------------------------------------

impl PlaidClient {
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

    /// POST a JSON body with client_id/secret injected. On a Plaid API error the
    /// returned `AppError::Provider` carries Plaid's `error_code`.
    async fn post(&self, path: &str, mut body: Value) -> AppResult<Value> {
        if let Some(obj) = body.as_object_mut() {
            obj.insert("client_id".into(), json!(self.client_id));
            obj.insert("secret".into(), json!(self.secret));
        }
        let resp = self.http.post(&self.url(path))?.json(&body).send().await?;
        let status = resp.status();
        let value: Value = resp.json().await?;
        if !status.is_success() {
            return Err(AppError::Provider {
                provider: "plaid".into(),
                code: value
                    .get("error_code")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                message: value
                    .get("error_message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown Plaid error")
                    .to_string(),
            });
        }
        Ok(value)
    }

    fn de<T: for<'de> Deserialize<'de>>(value: Value) -> AppResult<T> {
        serde_json::from_value(value).map_err(Into::into)
    }

    // --- link ----------------------------------------------------------------

    /// Create a Hosted Link token. Returns `(link_token, hosted_link_url)`.
    pub async fn create_hosted_link_token(&self, client_user_id: &str) -> AppResult<(String, String)> {
        let v = self
            .post(
                "/link/token/create",
                json!({
                    "client_name": CLIENT_NAME,
                    "language": "en",
                    "country_codes": ["US"],
                    "user": { "client_user_id": client_user_id },
                    "products": ["transactions"],
                    "hosted_link": {}
                }),
            )
            .await?;
        let link_token = v["link_token"].as_str().unwrap_or_default().to_string();
        let url = v["hosted_link_url"].as_str().unwrap_or_default().to_string();
        if link_token.is_empty() || url.is_empty() {
            return Err(AppError::Provider {
                provider: "plaid".into(),
                code: None,
                message: "link/token/create did not return a hosted_link_url".into(),
            });
        }
        Ok((link_token, url))
    }

    /// Poll a Hosted Link token for completed sessions. Returns every
    /// `public_token` found (usually zero or one).
    pub async fn get_link_public_tokens(&self, link_token: &str) -> AppResult<Vec<String>> {
        let v = self
            .post("/link/token/get", json!({ "link_token": link_token }))
            .await?;

        let mut tokens = Vec::new();
        if let Some(sessions) = v["link_sessions"].as_array() {
            for s in sessions {
                if let Some(t) = s["on_success"]["public_token"].as_str() {
                    tokens.push(t.to_string());
                }
                if let Some(results) = s["results"]["item_add_results"].as_array() {
                    for r in results {
                        if let Some(t) = r["public_token"].as_str() {
                            tokens.push(t.to_string());
                        }
                    }
                }
            }
        }
        tokens.sort();
        tokens.dedup();
        Ok(tokens)
    }

    /// Exchange a public token for a long-lived access token. Returns
    /// `(access_token, item_id)`.
    pub async fn exchange_public_token(&self, public_token: &str) -> AppResult<(String, String)> {
        let v = self
            .post(
                "/item/public_token/exchange",
                json!({ "public_token": public_token }),
            )
            .await?;
        Ok((
            v["access_token"].as_str().unwrap_or_default().to_string(),
            v["item_id"].as_str().unwrap_or_default().to_string(),
        ))
    }

    // --- item / institution -------------------------------------------------

    /// Returns the institution id for an item, if Plaid knows it.
    pub async fn item_institution_id(&self, access_token: &str) -> AppResult<Option<String>> {
        let v = self
            .post("/item/get", json!({ "access_token": access_token }))
            .await?;
        Ok(v["item"]["institution_id"].as_str().map(String::from))
    }

    pub async fn institution_name(&self, institution_id: &str) -> AppResult<Option<String>> {
        let v = self
            .post(
                "/institutions/get_by_id",
                json!({
                    "institution_id": institution_id,
                    "country_codes": ["US"]
                }),
            )
            .await?;
        Ok(v["institution"]["name"].as_str().map(String::from))
    }

    pub async fn item_remove(&self, access_token: &str) -> AppResult<()> {
        self.post("/item/remove", json!({ "access_token": access_token }))
            .await
            .map(|_| ())
    }

    // --- accounts / transactions ------------------------------------------

    pub async fn accounts_get(&self, access_token: &str) -> AppResult<Vec<PlaidAccount>> {
        let v = self
            .post("/accounts/get", json!({ "access_token": access_token }))
            .await?;
        Self::de(v["accounts"].clone())
    }

    pub async fn accounts_balance_get(&self, access_token: &str) -> AppResult<Vec<PlaidAccount>> {
        // Capital One (and only Capital One) rejects the call for non-depository
        // accounts unless `min_last_updated_datetime` is present; every other
        // institution ignores it. A 30-day window is permissive enough to never
        // trip LAST_UPDATED_DATETIME_OUT_OF_RANGE while keeping balances current
        // enough for our purposes (transactions carry the real signal).
        let min_updated = (chrono::Utc::now() - chrono::Duration::days(30))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let v = self
            .post(
                "/accounts/balance/get",
                json!({
                    "access_token": access_token,
                    "options": { "min_last_updated_datetime": min_updated }
                }),
            )
            .await?;
        Self::de(v["accounts"].clone())
    }

    /// One page of `/transactions/sync`. `cursor` is `None` on the first call.
    /// Returns `Ok(None)` when Plaid is still preparing transaction data
    /// (`PRODUCT_NOT_READY`) so the caller can retry later.
    pub async fn transactions_sync(
        &self,
        access_token: &str,
        cursor: Option<&str>,
    ) -> AppResult<Option<TransactionsSyncPage>> {
        let mut body = json!({ "access_token": access_token, "count": 500 });
        if let Some(c) = cursor {
            body["cursor"] = json!(c);
        }
        match self.post("/transactions/sync", body).await {
            Ok(v) => Self::de(v).map(Some),
            Err(AppError::Provider { code: Some(c), .. }) if c == "PRODUCT_NOT_READY" => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn health_check(&self) -> AppResult<()> {
        self.post(
            "/institutions/get",
            json!({ "count": 1, "offset": 0, "country_codes": ["US"] }),
        )
        .await
        .map(|_| ())
    }
}
