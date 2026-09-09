//! Master-password lifecycle: initialize, unlock, lock, change, reset.
//! Until `vault_initialize` / `vault_unlock` succeeds, no other command that
//! touches the database will work.

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::vault::{self, VaultMeta};

#[derive(Serialize)]
pub struct VaultStatus {
    /// "uninitialized" | "locked" | "unlocked"
    pub state: String,
}

fn validate_password(p: &str) -> AppResult<()> {
    if p.chars().count() < 8 {
        return Err(AppError::Invalid(
            "master password must be at least 8 characters".into(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn vault_status(state: State<'_, AppState>) -> AppResult<VaultStatus> {
    let s = if state.is_unlocked().await {
        "unlocked"
    } else if state.paths.meta.exists() {
        "locked"
    } else {
        "uninitialized"
    };
    Ok(VaultStatus { state: s.to_string() })
}

#[tauri::command]
pub async fn vault_initialize(state: State<'_, AppState>, password: String) -> AppResult<()> {
    if state.paths.meta.exists() {
        return Err(AppError::Invalid("the app is already set up".into()));
    }
    validate_password(&password)?;

    remove_db_files(&state.paths.db);

    let meta = VaultMeta::generate();
    let key = vault::derive_key_hex(&password, &meta.kdf)?;
    let db = Db::connect_encrypted(&state.paths.db, &key).await?;
    vault::write_meta(&state.paths.meta, &meta)?;
    state.open_session(db).await;
    Ok(())
}

fn remove_db_files(db: &std::path::Path) {
    let _ = std::fs::remove_file(db);
    for ext in ["-wal", "-shm", "-journal"] {
        let mut sib = db.as_os_str().to_owned();
        sib.push(ext);
        let _ = std::fs::remove_file(PathBuf::from(sib));
    }
}

#[tauri::command]
pub async fn vault_unlock(state: State<'_, AppState>, password: String) -> AppResult<()> {
    if state.is_unlocked().await {
        return Ok(());
    }
    let meta = vault::read_meta(&state.paths.meta)?;
    let key = vault::derive_key_hex(&password, &meta.kdf)?;
    // Wrong key surfaces as AppError::Invalid("incorrect password") from connect_encrypted.
    let db = Db::connect_encrypted(&state.paths.db, &key).await?;
    state.open_session(db).await;
    Ok(())
}

#[tauri::command]
pub async fn vault_lock(state: State<'_, AppState>) -> AppResult<()> {
    state.close_session().await;
    Ok(())
}

#[tauri::command]
pub async fn vault_change_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> AppResult<()> {
    validate_password(&new_password)?;
    let old_meta = vault::read_meta(&state.paths.meta)?;
    let old_key = vault::derive_key_hex(&old_password, &old_meta.kdf)?;

    let new_meta = VaultMeta::generate();
    let new_key = vault::derive_key_hex(&new_password, &new_meta.kdf)?;

    // Drop the live pool so rekey runs against the file exclusively.
    state.close_session().await;
    let rekey = Db::rekey(&state.paths.db, &old_key, &new_key).await;
    match rekey {
        Ok(()) => {
            vault::write_meta(&state.paths.meta, &new_meta)?;
            let db = Db::connect_encrypted(&state.paths.db, &new_key).await?;
            state.open_session(db).await;
            Ok(())
        }
        Err(e) => {
            // Re-open with the old key so the app stays usable.
            if let Ok(db) = Db::connect_encrypted(&state.paths.db, &old_key).await {
                state.open_session(db).await;
            }
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn vault_reset(state: State<'_, AppState>) -> AppResult<()> {
    state.close_session().await;
    remove_db_files(&state.paths.db);
    let _ = std::fs::remove_file(&state.paths.meta);
    Ok(())
}
