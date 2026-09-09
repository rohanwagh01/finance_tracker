//! Linked institutions (Plaid items) and their accounts: the Hosted Link flow,
//! syncing, and per-account flags.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::sync::{self, LinkedItem, SyncSummary};

#[derive(Serialize)]
pub struct LinkStart {
    pub link_token: String,
}

#[derive(Serialize)]
pub struct LinkPollResult {
    pub linked: Vec<LinkedItem>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ItemView {
    pub id: String,
    pub institution_name: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub last_synced_at: Option<String>,
    pub account_count: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AccountView {
    pub id: String,
    pub item_id: String,
    pub institution_name: Option<String>,
    pub name: String,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    #[sqlx(rename = "type")]
    pub account_type: String,
    pub subtype: Option<String>,
    pub currency: String,
    pub current_balance: Option<f64>,
    pub available_balance: Option<f64>,
    pub credit_limit: Option<f64>,
    pub is_shared: bool,
    pub is_hidden: bool,
}

#[tauri::command]
pub async fn plaid_link_start(app: AppHandle, state: State<'_, AppState>) -> AppResult<LinkStart> {
    let plaid = sync::plaid_client(&state).await?;
    let user_id = sync::plaid_user_id(&state).await?;
    let (link_token, url) = plaid.create_hosted_link_token(&user_id).await?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Other(format!("could not open browser: {e}")))?;
    Ok(LinkStart { link_token })
}

#[tauri::command]
pub async fn plaid_link_poll(
    state: State<'_, AppState>,
    link_token: String,
) -> AppResult<LinkPollResult> {
    let plaid = sync::plaid_client(&state).await?;
    let public_tokens = plaid.get_link_public_tokens(&link_token).await?;

    let mut linked = Vec::new();
    for pt in public_tokens {
        if let Some(item) = sync::process_public_token(&state, &plaid, &pt).await? {
            // Best-effort initial pull; ignore "not ready yet".
            let _ = sync::sync_item(&state, &item.item_id).await;
            linked.push(item);
        }
    }
    Ok(LinkPollResult { linked })
}

#[tauri::command]
pub async fn list_items(state: State<'_, AppState>) -> AppResult<Vec<ItemView>> {
    let db = state.db().await?;
    sqlx::query_as::<_, ItemView>(
        "SELECT i.id, i.institution_name, i.status, i.error_message, i.last_synced_at,
                (SELECT COUNT(*) FROM accounts a WHERE a.item_id = i.id) AS account_count
         FROM items i
         ORDER BY i.institution_name COLLATE NOCASE",
    )
    .fetch_all(&db.pool)
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> AppResult<Vec<AccountView>> {
    let db = state.db().await?;
    sqlx::query_as::<_, AccountView>(
        "SELECT a.id, a.item_id, i.institution_name, a.name, a.official_name, a.mask,
                a.type, a.subtype, a.currency, a.current_balance, a.available_balance,
                a.credit_limit, a.is_shared, a.is_hidden
         FROM accounts a JOIN items i ON i.id = a.item_id
         ORDER BY i.institution_name COLLATE NOCASE, a.name COLLATE NOCASE",
    )
    .fetch_all(&db.pool)
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn sync_item(state: State<'_, AppState>, item_id: String) -> AppResult<SyncSummary> {
    sync::sync_item(&state, &item_id).await
}

#[tauri::command]
pub async fn sync_all(state: State<'_, AppState>) -> AppResult<Vec<SyncSummary>> {
    sync::sync_all(&state).await
}

#[tauri::command]
pub async fn unlink_item(state: State<'_, AppState>, item_id: String) -> AppResult<()> {
    let db = state.db().await?;
    let secret_ref: Option<String> =
        sqlx::query_scalar("SELECT secret_ref FROM items WHERE id = ?1")
            .bind(&item_id)
            .fetch_optional(&db.pool)
            .await?;
    let Some(secret_ref) = secret_ref else {
        return Err(AppError::NotFound(format!("item {item_id}")));
    };

    // Best-effort: tell Plaid to drop the item and clear the stored token.
    // None of this should block removing the local data.
    if let (Ok(plaid), Ok(secrets)) = (sync::plaid_client(&state).await, state.secrets().await) {
        if let Ok(Some(token)) = secrets.get(&secret_ref).await {
            let _ = plaid.item_remove(&token).await;
        }
        let _ = secrets.delete(&secret_ref).await;
    }

    // Cascades to accounts / transactions / holdings / value_snapshots / sync_log.
    sqlx::query("DELETE FROM items WHERE id = ?1")
        .bind(&item_id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn set_account_shared(
    state: State<'_, AppState>,
    account_id: String,
    shared: bool,
) -> AppResult<()> {
    let db = state.db().await?;
    let res = sqlx::query("UPDATE accounts SET is_shared = ?2, updated_at = ?3 WHERE id = ?1")
        .bind(&account_id)
        .bind(shared)
        .bind(crate::util::now())
        .execute(&db.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("account {account_id}")));
    }
    // Move existing charges in/out of the review inbox without disturbing
    // decisions the user already made.
    if shared {
        sqlx::query(
            "UPDATE transactions SET review_status = 'pending'
             WHERE account_id = ?1 AND review_status = 'not_required' AND amount > 0",
        )
        .bind(&account_id)
        .execute(&db.pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE transactions SET review_status = 'not_required'
             WHERE account_id = ?1 AND review_status = 'pending'",
        )
        .bind(&account_id)
        .execute(&db.pool)
        .await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_account_hidden(
    state: State<'_, AppState>,
    account_id: String,
    hidden: bool,
) -> AppResult<()> {
    let db = state.db().await?;
    let res = sqlx::query("UPDATE accounts SET is_hidden = ?2, updated_at = ?3 WHERE id = ?1")
        .bind(&account_id)
        .bind(hidden)
        .bind(crate::util::now())
        .execute(&db.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("account {account_id}")));
    }
    Ok(())
}
