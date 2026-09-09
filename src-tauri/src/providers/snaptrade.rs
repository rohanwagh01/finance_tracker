//! SnapTrade client (read-only brokerage data: Robinhood, E*Trade).
//! Implemented incrementally: milestone 5 adds user registration, the
//! connection-portal URL, and account + holdings retrieval.
//!
//! SnapTrade signs requests with an HMAC of (consumerKey, path, query, body).
//! That signing helper will live here.
#![allow(dead_code)]

use crate::error::AppResult;
use crate::http::HttpClient;
use crate::secrets::{keys, SecretStore};

pub const BASE_URL: &str = "https://api.snaptrade.com/api/v1";

pub struct SnapTradeClient {
    http: HttpClient,
    client_id: String,
    consumer_key: String,
}

impl SnapTradeClient {
    pub fn from_secrets(http: HttpClient, secrets: &SecretStore) -> AppResult<Self> {
        Ok(Self {
            http,
            client_id: secrets.require(keys::SNAPTRADE_CLIENT_ID)?,
            consumer_key: secrets.require(keys::SNAPTRADE_CONSUMER_KEY)?,
        })
    }
}
