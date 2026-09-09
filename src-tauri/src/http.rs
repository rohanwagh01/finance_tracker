//! The single outbound HTTP client. Every network call in the app goes through
//! here, and every call is checked against a fixed host allowlist first. If a
//! host is not on the list the request never leaves the process.

use std::time::Duration;

use reqwest::{Client, Method, RequestBuilder};
use url::Url;

use crate::error::{AppError, AppResult};

/// Domains the app is permitted to talk to, and why.
/// A host matches if it equals an entry or is a subdomain of one.
const ALLOWED_DOMAINS: &[&str] = &[
    "plaid.com",        // bank / card / investment aggregation (read-only)
    "anthropic.com",    // research LLM (tickers + allocation % only)
    "finnhub.io",       // market news + earnings calendar
    "marketaux.com",    // market news (fallback)
];

/// Exact hosts allowed in addition to the domains above (local LLM runtimes).
const ALLOWED_HOSTS: &[&str] = &["localhost", "127.0.0.1", "::1"];

pub fn is_host_allowed(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if ALLOWED_HOSTS.contains(&host.as_str()) {
        return true;
    }
    ALLOWED_DOMAINS.iter().any(|d| {
        host == *d || host.ends_with(&format!(".{d}"))
    })
}

fn check_url(url: &str) -> AppResult<Url> {
    let parsed = Url::parse(url).map_err(|e| AppError::Invalid(format!("bad url {url:?}: {e}")))?;
    match parsed.host_str() {
        Some(host) if is_host_allowed(host) => Ok(parsed),
        Some(host) => Err(AppError::HostNotAllowed(host.to_string())),
        None => Err(AppError::HostNotAllowed(url.to_string())),
    }
}

#[derive(Clone)]
pub struct HttpClient {
    inner: Client,
}

impl HttpClient {
    pub fn new() -> Self {
        let inner = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("finance-tracker/0.1 (local; read-only)")
            .build()
            .expect("reqwest client builds");
        Self { inner }
    }

    pub fn request(&self, method: Method, url: &str) -> AppResult<RequestBuilder> {
        let url = check_url(url)?;
        Ok(self.inner.request(method, url))
    }

    #[allow(dead_code)] // used by GET-based provider endpoints (milestone 2+)
    pub fn get(&self, url: &str) -> AppResult<RequestBuilder> {
        self.request(Method::GET, url)
    }

    pub fn post(&self, url: &str) -> AppResult<RequestBuilder> {
        self.request(Method::POST, url)
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_known_hosts() {
        for h in [
            "sandbox.plaid.com",
            "production.plaid.com",
            "api.anthropic.com",
            "finnhub.io",
            "api.marketaux.com",
            "localhost",
            "127.0.0.1",
        ] {
            assert!(is_host_allowed(h), "{h} should be allowed");
        }
    }

    #[test]
    fn blocks_everything_else() {
        for h in [
            "evil.com",
            "plaid.com.evil.com",
            "notplaid.com",
            "api.openai.com",
            "api.snaptrade.com",
            "example.org",
            "10.0.0.5",
        ] {
            assert!(!is_host_allowed(h), "{h} should be blocked");
        }
    }

    #[test]
    fn request_to_blocked_host_errors() {
        let c = HttpClient::new();
        let err = c.get("https://api.openai.com/v1/models").unwrap_err();
        assert!(matches!(err, AppError::HostNotAllowed(_)));
    }
}
