//! Aggregator integrations. All are strictly read-only: only balance,
//! transaction, holding, and liability endpoints are ever called.

pub mod plaid;
pub mod snaptrade;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlaidEnv {
    #[default]
    Sandbox,
    Production,
}

impl PlaidEnv {
    pub fn base_url(&self) -> &'static str {
        match self {
            PlaidEnv::Sandbox => "https://sandbox.plaid.com",
            PlaidEnv::Production => "https://production.plaid.com",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "production" | "prod" => PlaidEnv::Production,
            _ => PlaidEnv::Sandbox,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PlaidEnv::Sandbox => "sandbox",
            PlaidEnv::Production => "production",
        }
    }
}
