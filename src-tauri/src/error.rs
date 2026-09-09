//! Unified error type. Tauri commands return `Result<T, AppError>`; `AppError`
//! serializes to a plain `{ "message": "..." }` object for the frontend.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("blocked outbound request to host {0:?} (not on the allowlist)")]
    HostNotAllowed(String),

    #[error("provider {provider} error{}: {message}", .code.as_deref().map(|c| format!(" [{c}]")).unwrap_or_default())]
    Provider {
        provider: String,
        code: Option<String>,
        message: String,
    },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 3)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        let code = match self {
            AppError::Provider { code, .. } => code.clone(),
            _ => None,
        };
        s.serialize_field("code", &code)?;
        s.end()
    }
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::Db(_) | AppError::Migrate(_) => "db",
            AppError::Keyring(_) => "keychain",
            AppError::Http(_) => "network",
            AppError::Json(_) => "serialization",
            AppError::HostNotAllowed(_) => "host_not_allowed",
            AppError::Provider { .. } => "provider",
            AppError::NotFound(_) => "not_found",
            AppError::Invalid(_) => "invalid",
            AppError::Config(_) => "config",
            AppError::Other(_) => "other",
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
