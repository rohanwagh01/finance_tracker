//! Manually-entered assets (a car, a house, cash held elsewhere) and
//! liabilities (a mortgage, a car loan). Assets can straight-line depreciate.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::{new_id, now};

/// SQL expression for an asset's depreciated value today, floored at 0.
pub const ASSET_CURRENT_VALUE: &str = "MAX(0.0, value - value * COALESCE(depreciation_annual_pct, 0.0) / 100.0 \
     * (julianday('now') - julianday(as_of)) / 365.25)";

#[derive(Serialize, sqlx::FromRow)]
pub struct ManualAsset {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub value: f64,
    pub current_value: f64,
    pub as_of: String,
    pub depreciation_annual_pct: Option<f64>,
    pub note: Option<String>,
}

#[derive(Deserialize)]
pub struct AssetInput {
    pub name: String,
    pub kind: String,
    pub value: f64,
    pub as_of: String,
    pub depreciation_annual_pct: Option<f64>,
    pub note: Option<String>,
}

fn valid_date(d: &str) -> AppResult<()> {
    chrono::NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| AppError::Invalid("date must be YYYY-MM-DD".into()))
}

#[tauri::command]
pub async fn list_manual_assets(state: State<'_, AppState>) -> AppResult<Vec<ManualAsset>> {
    let db = state.db().await?;
    Ok(sqlx::query_as::<_, ManualAsset>(&format!(
        "SELECT id, name, kind, value, {ASSET_CURRENT_VALUE} AS current_value,
                as_of, depreciation_annual_pct, note
         FROM manual_assets ORDER BY current_value DESC"
    ))
    .fetch_all(&db.pool)
    .await?)
}

#[tauri::command]
pub async fn create_manual_asset(state: State<'_, AppState>, input: AssetInput) -> AppResult<String> {
    if input.name.trim().is_empty() {
        return Err(AppError::Invalid("name is required".into()));
    }
    valid_date(&input.as_of)?;
    let db = state.db().await?;
    let id = new_id();
    sqlx::query(
        "INSERT INTO manual_assets (id, name, kind, value, as_of, depreciation_annual_pct, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
    )
    .bind(&id)
    .bind(input.name.trim())
    .bind(&input.kind)
    .bind(input.value)
    .bind(input.as_of.trim())
    .bind(input.depreciation_annual_pct)
    .bind(input.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(now())
    .execute(&db.pool)
    .await?;
    Ok(id)
}

#[tauri::command]
pub async fn update_manual_asset(
    state: State<'_, AppState>,
    id: String,
    input: AssetInput,
) -> AppResult<()> {
    valid_date(&input.as_of)?;
    let db = state.db().await?;
    let res = sqlx::query(
        "UPDATE manual_assets SET name = ?2, kind = ?3, value = ?4, as_of = ?5,
             depreciation_annual_pct = ?6, note = ?7, updated_at = ?8 WHERE id = ?1",
    )
    .bind(&id)
    .bind(input.name.trim())
    .bind(&input.kind)
    .bind(input.value)
    .bind(input.as_of.trim())
    .bind(input.depreciation_annual_pct)
    .bind(input.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(now())
    .execute(&db.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("asset {id}")));
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_manual_asset(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let db = state.db().await?;
    sqlx::query("DELETE FROM manual_assets WHERE id = ?1")
        .bind(&id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

// --- liabilities -----------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct ManualLiability {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub balance: f64,
    pub as_of: String,
    pub apr: Option<f64>,
    pub minimum_payment: Option<f64>,
    pub note: Option<String>,
}

#[derive(Deserialize)]
pub struct LiabilityInput {
    pub name: String,
    pub kind: String,
    pub balance: f64,
    pub as_of: String,
    pub apr: Option<f64>,
    pub minimum_payment: Option<f64>,
    pub note: Option<String>,
}

#[tauri::command]
pub async fn list_manual_liabilities(
    state: State<'_, AppState>,
) -> AppResult<Vec<ManualLiability>> {
    let db = state.db().await?;
    Ok(sqlx::query_as::<_, ManualLiability>(
        "SELECT id, name, kind, balance, as_of, apr, minimum_payment, note
         FROM manual_liabilities ORDER BY balance DESC",
    )
    .fetch_all(&db.pool)
    .await?)
}

#[tauri::command]
pub async fn create_manual_liability(
    state: State<'_, AppState>,
    input: LiabilityInput,
) -> AppResult<String> {
    if input.name.trim().is_empty() {
        return Err(AppError::Invalid("name is required".into()));
    }
    valid_date(&input.as_of)?;
    let db = state.db().await?;
    let id = new_id();
    sqlx::query(
        "INSERT INTO manual_liabilities (id, name, kind, balance, as_of, apr, minimum_payment, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
    )
    .bind(&id)
    .bind(input.name.trim())
    .bind(&input.kind)
    .bind(input.balance)
    .bind(input.as_of.trim())
    .bind(input.apr)
    .bind(input.minimum_payment)
    .bind(input.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(now())
    .execute(&db.pool)
    .await?;
    Ok(id)
}

#[tauri::command]
pub async fn update_manual_liability(
    state: State<'_, AppState>,
    id: String,
    input: LiabilityInput,
) -> AppResult<()> {
    valid_date(&input.as_of)?;
    let db = state.db().await?;
    let res = sqlx::query(
        "UPDATE manual_liabilities SET name = ?2, kind = ?3, balance = ?4, as_of = ?5,
             apr = ?6, minimum_payment = ?7, note = ?8, updated_at = ?9 WHERE id = ?1",
    )
    .bind(&id)
    .bind(input.name.trim())
    .bind(&input.kind)
    .bind(input.balance)
    .bind(input.as_of.trim())
    .bind(input.apr)
    .bind(input.minimum_payment)
    .bind(input.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(now())
    .execute(&db.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("liability {id}")));
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_manual_liability(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let db = state.db().await?;
    sqlx::query("DELETE FROM manual_liabilities WHERE id = ?1")
        .bind(&id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[tokio::test]
    async fn straight_line_depreciation_and_floor() {
        let db = Db::connect_in_memory().await.unwrap();
        let p = &db.pool;
        let ts = now();
        let two_years_ago = (chrono::Utc::now() - chrono::Duration::days(730))
            .format("%Y-%m-%d")
            .to_string();
        // $30k car, 10%/yr → ~20% gone after 2 years → ~$24k
        sqlx::query("INSERT INTO manual_assets (id,name,kind,value,as_of,depreciation_annual_pct,created_at,updated_at) VALUES ('a','Car','vehicle',30000,?1,10,?2,?2)")
            .bind(&two_years_ago).bind(&ts).execute(p).await.unwrap();
        // fully depreciated → floors at 0, never negative
        sqlx::query("INSERT INTO manual_assets (id,name,kind,value,as_of,depreciation_annual_pct,created_at,updated_at) VALUES ('b','Junk','other',1000,?1,90,?2,?2)")
            .bind(&two_years_ago).bind(&ts).execute(p).await.unwrap();

        let v: f64 = sqlx::query_scalar(&format!("SELECT {ASSET_CURRENT_VALUE} FROM manual_assets WHERE id='a'"))
            .fetch_one(p).await.unwrap();
        assert!((v - 24000.0).abs() < 200.0, "got {v}");

        let junk: f64 = sqlx::query_scalar(&format!("SELECT {ASSET_CURRENT_VALUE} FROM manual_assets WHERE id='b'"))
            .fetch_one(p).await.unwrap();
        assert_eq!(junk, 0.0);
    }
}
