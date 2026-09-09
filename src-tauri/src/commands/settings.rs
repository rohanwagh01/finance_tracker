//! Onboarding + settings commands. Credential *values* are write-only from the
//! UI's perspective: they go into the OS keychain and are never returned.

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
    pub snaptrade_configured: bool,
    pub llm_provider: String,
    pub llm_configured: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub plaid_env: String,
    pub llm_provider: String,   // "anthropic" | "ollama" | "none"
    pub anthropic_model: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub news_provider: String,  // "finnhub" | "marketaux" | "none"
    pub auto_confirm_high_confidence: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            plaid_env: "sandbox".into(),
            llm_provider: "none".into(),
            anthropic_model: "claude-sonnet-5".into(),
            ollama_url: "http://localhost:11434".into(),
            ollama_model: "llama3.1".into(),
            news_provider: "finnhub".into(),
            auto_confirm_high_confidence: false,
        }
    }
}

async fn load_settings(state: &AppState) -> AppResult<Settings> {
    let d = Settings::default();
    Ok(Settings {
        plaid_env: state.config_get_or("plaid_env", &d.plaid_env).await?,
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
        "anthropic" => s.secrets.exists(keys::ANTHROPIC_API_KEY)?,
        "ollama" => true,
        _ => false,
    };
    Ok(SetupStatus {
        onboarding_complete: s.config_get("onboarding_complete").await?.as_deref() == Some("true"),
        plaid_configured: s.secrets.exists(keys::PLAID_CLIENT_ID)?
            && s.secrets.exists(keys::PLAID_SECRET)?,
        snaptrade_configured: s.secrets.exists(keys::SNAPTRADE_CLIENT_ID)?
            && s.secrets.exists(keys::SNAPTRADE_CONSUMER_KEY)?,
        llm_provider,
        llm_configured,
    })
}

#[tauri::command]
pub async fn list_credentials(state: State<'_, AppState>) -> AppResult<Vec<CredentialStatus>> {
    keys::WELL_KNOWN
        .iter()
        .map(|name| {
            Ok(CredentialStatus {
                name: (*name).to_string(),
                present: state.secrets.exists(name)?,
            })
        })
        .collect()
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
    state.secrets.set(&name, value)
}

#[tauri::command]
pub async fn delete_credential(state: State<'_, AppState>, name: String) -> AppResult<()> {
    if !keys::WELL_KNOWN.contains(&name.as_str()) {
        return Err(AppError::Invalid(format!("unknown credential name: {name}")));
    }
    state.secrets.delete(&name)
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
    let client = PlaidClient::from_secrets(state.http.clone(), &state.secrets, env)?;
    client.health_check().await
}

#[tauri::command]
pub async fn complete_onboarding(state: State<'_, AppState>) -> AppResult<()> {
    state.config_set("onboarding_complete", "true").await
}
