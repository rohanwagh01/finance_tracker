//! Onboarding + settings commands. Credential *values* are write-only from the
//! UI's perspective: they go into the encrypted `secrets` table and are never returned.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::providers::plaid::PlaidClient;
use crate::providers::PlaidEnv;
use crate::secrets::keys;
use crate::state::AppState;

#[derive(Serialize)]
pub struct CredentialStatus {
    pub name: String,
    pub present: bool,
}

#[derive(Serialize)]
pub struct SetupStatus {
    pub onboarding_complete: bool,
    pub plaid_configured: bool,
    pub llm_provider: String,
    pub llm_configured: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub plaid_env: String,
    /// Oldest date to pull / keep history for (ISO `YYYY-MM-DD`). Plaid can only
    /// go back ~730 days, so earlier dates just cap there.
    pub history_start_date: String,
    pub llm_provider: String,   // "anthropic" | "ollama" | "none"
    pub anthropic_model: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub news_provider: String,  // "finnhub" | "marketaux" | "none"
    pub auto_confirm_high_confidence: bool,
}

/// Default history window: the Plaid maximum, ~24 months.
pub const MAX_HISTORY_DAYS: i64 = 730;

fn default_history_start() -> String {
    (chrono::Utc::now() - chrono::Duration::days(MAX_HISTORY_DAYS))
        .format("%Y-%m-%d")
        .to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            plaid_env: "sandbox".into(),
            history_start_date: default_history_start(),
            llm_provider: "none".into(),
            anthropic_model: "claude-sonnet-5".into(),
            ollama_url: "http://localhost:11434".into(),
            ollama_model: "llama3.1".into(),
            news_provider: "finnhub".into(),
            auto_confirm_high_confidence: false,
        }
    }
}

fn days_from_start(start: &str) -> i64 {
    chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d")
        .ok()
        .map(|d| (chrono::Utc::now().date_naive() - d).num_days())
        .unwrap_or(MAX_HISTORY_DAYS)
        .clamp(1, MAX_HISTORY_DAYS)
}

/// Number of days of Plaid transaction history to request at link time,
/// derived from `history_start_date` and clamped to Plaid's max.
pub async fn history_days_requested(state: &AppState) -> AppResult<i64> {
    let start = state
        .config_get_or("history_start_date", &default_history_start())
        .await?;
    Ok(days_from_start(&start))
}

#[cfg(test)]
mod tests {
    use super::days_from_start;

    #[test]
    fn history_days_are_clamped_to_plaid_max() {
        assert_eq!(days_from_start("1990-01-01"), 730);
        assert_eq!(days_from_start("garbage"), 730);
        let recent = (chrono::Utc::now() - chrono::Duration::days(45))
            .format("%Y-%m-%d")
            .to_string();
        assert_eq!(days_from_start(&recent), 45);
        let future = (chrono::Utc::now() + chrono::Duration::days(10))
            .format("%Y-%m-%d")
            .to_string();
        assert_eq!(days_from_start(&future), 1);
    }
}

async fn load_settings(state: &AppState) -> AppResult<Settings> {
    let d = Settings::default();
    Ok(Settings {
        plaid_env: state.config_get_or("plaid_env", &d.plaid_env).await?,
        history_start_date: state
            .config_get_or("history_start_date", &d.history_start_date)
            .await?,
        llm_provider: state.config_get_or("llm_provider", &d.llm_provider).await?,
        anthropic_model: state.config_get_or("anthropic_model", &d.anthropic_model).await?,
        ollama_url: state.config_get_or("ollama_url", &d.ollama_url).await?,
        ollama_model: state.config_get_or("ollama_model", &d.ollama_model).await?,
        news_provider: state.config_get_or("news_provider", &d.news_provider).await?,
        auto_confirm_high_confidence: state
            .config_get_or("auto_confirm_high_confidence", "false")
            .await?
            == "true",
    })
}

#[tauri::command]
pub async fn get_setup_status(state: State<'_, AppState>) -> AppResult<SetupStatus> {
    let s = &*state;
    let llm_provider = s.config_get_or("llm_provider", "none").await?;
    let llm_configured = match llm_provider.as_str() {
        "anthropic" => s.credential_present(keys::ANTHROPIC_API_KEY).await?,
        "ollama" => true,
        _ => false,
    };
    Ok(SetupStatus {
        onboarding_complete: s.config_get("onboarding_complete").await?.as_deref() == Some("true"),
        plaid_configured: s.credential_present(keys::PLAID_CLIENT_ID).await?
            && s.credential_present(keys::PLAID_SECRET).await?,
        llm_provider,
        llm_configured,
    })
}

#[tauri::command]
pub async fn list_credentials(state: State<'_, AppState>) -> AppResult<Vec<CredentialStatus>> {
    let mut out = Vec::with_capacity(keys::WELL_KNOWN.len());
    for name in keys::WELL_KNOWN {
        out.push(CredentialStatus {
            name: (*name).to_string(),
            present: state.credential_present(name).await?,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn save_credential(
    state: State<'_, AppState>,
    name: String,
    value: String,
) -> AppResult<()> {
    if !keys::WELL_KNOWN.contains(&name.as_str()) {
        return Err(AppError::Invalid(format!("unknown credential name: {name}")));
    }
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Invalid("value must not be empty".into()));
    }
    state.secrets().await?.set(&name, value).await
}

#[tauri::command]
pub async fn delete_credential(state: State<'_, AppState>, name: String) -> AppResult<()> {
    if !keys::WELL_KNOWN.contains(&name.as_str()) {
        return Err(AppError::Invalid(format!("unknown credential name: {name}")));
    }
    state.secrets().await?.delete(&name).await
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    load_settings(&state).await
}

#[tauri::command]
pub async fn update_settings(state: State<'_, AppState>, settings: Settings) -> AppResult<()> {
    let s = &*state;
    let env = PlaidEnv::parse(&settings.plaid_env);
    s.config_set("plaid_env", env.as_str()).await?;

    let start = settings.history_start_date.trim();
    if chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").is_err() {
        return Err(AppError::Invalid("history start date must be YYYY-MM-DD".into()));
    }
    s.config_set("history_start_date", start).await?;

    if !["anthropic", "ollama", "none"].contains(&settings.llm_provider.as_str()) {
        return Err(AppError::Invalid("llm_provider must be anthropic, ollama, or none".into()));
    }
    s.config_set("llm_provider", &settings.llm_provider).await?;
    s.config_set("anthropic_model", settings.anthropic_model.trim()).await?;
    s.config_set("ollama_url", settings.ollama_url.trim()).await?;
    s.config_set("ollama_model", settings.ollama_model.trim()).await?;
    s.config_set("news_provider", &settings.news_provider).await?;
    s.config_set(
        "auto_confirm_high_confidence",
        if settings.auto_confirm_high_confidence { "true" } else { "false" },
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn test_plaid_connection(state: State<'_, AppState>) -> AppResult<()> {
    let env = state.plaid_env().await?;
    let secrets = state.secrets().await?;
    let client = PlaidClient::from_secrets(state.http.clone(), &secrets, env).await?;
    client.health_check().await
}

#[tauri::command]
pub async fn complete_onboarding(state: State<'_, AppState>) -> AppResult<()> {
    state.config_set("onboarding_complete", "true").await
}
