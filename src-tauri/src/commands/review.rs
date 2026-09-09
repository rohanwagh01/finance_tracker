//! The attribution review inbox: pending charges from shared cards that the
//! user clears by Keep (mine) / Assign (someone else) / Exclude, plus the
//! rules that pre-fill the suggested person.

use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::rules;
use crate::state::AppState;
use crate::util::{new_id, now};

#[derive(Serialize, sqlx::FromRow)]
pub struct ReviewRow {
    pub id: String,
    pub posted_date: String,
    pub amount: f64,
    pub currency: String,
    pub description: String,
    pub merchant_name: Option<String>,
    pub category_label: Option<String>,
    pub account_id: String,
    pub account_name: String,
    pub suggested_person_id: Option<String>,
    pub suggested_person_name: Option<String>,
    pub suggestion_rule_id: Option<String>,
}

async fn self_person_id(pool: &SqlitePool) -> AppResult<String> {
    sqlx::query_scalar("SELECT id FROM people WHERE is_self = 1 ORDER BY created_at LIMIT 1")
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Config("no primary person configured".into()))
}

#[tauri::command]
pub async fn review_inbox(state: State<'_, AppState>) -> AppResult<Vec<ReviewRow>> {
    let db = state.db().await?;
    Ok(sqlx::query_as::<_, ReviewRow>(
        "SELECT t.id, t.posted_date, t.amount, t.currency, t.description, t.merchant_name,
                COALESCE(c.label, p.label) AS category_label,
                t.account_id, a.name AS account_name,
                t.suggested_person_id, sp.name AS suggested_person_name, t.suggestion_rule_id
         FROM transactions t
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN categories c ON c.id = t.category_id
         LEFT JOIN categories p ON p.id = c.parent_id
         LEFT JOIN people sp ON sp.id = t.suggested_person_id
         WHERE t.review_status = 'pending'
         ORDER BY t.posted_date DESC, t.amount DESC",
    )
    .fetch_all(&db.pool)
    .await?)
}

#[tauri::command]
pub async fn review_count(state: State<'_, AppState>) -> AppResult<i64> {
    let db = state.db().await?;
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE review_status = 'pending'")
            .fetch_one(&db.pool)
            .await?,
    )
}

#[derive(Deserialize)]
pub struct ReviewDecision {
    pub txn_ids: Vec<String>,
    pub decision: String, // "keep" | "assign" | "exclude" | "reset"
    pub person_id: Option<String>,
}

#[tauri::command]
pub async fn review_decide(state: State<'_, AppState>, input: ReviewDecision) -> AppResult<()> {
    if input.txn_ids.is_empty() {
        return Ok(());
    }
    let db = state.db().await?;

    // "reset" always drops the transaction back into the review inbox,
    // regardless of whether its account is marked shared.
    if input.decision == "reset" {
        let mut qb = QueryBuilder::<Sqlite>::new(
            "UPDATE transactions SET owner_person_id = NULL, suggested_person_id = NULL,
                 suggestion_rule_id = NULL, review_status = 'pending', updated_at = ",
        );
        qb.push_bind(now());
        qb.push(" WHERE id IN (");
        let mut sep = qb.separated(", ");
        for id in &input.txn_ids {
            sep.push_bind(id.clone());
        }
        qb.push(")");
        qb.build().execute(&db.pool).await?;
        return Ok(());
    }

    let (status, owner): (&str, Option<String>) = match input.decision.as_str() {
        "keep" => ("kept", Some(self_person_id(&db.pool).await?)),
        "assign" => (
            "assigned",
            Some(
                input
                    .person_id
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| AppError::Invalid("assign needs a person".into()))?,
            ),
        ),
        "exclude" => ("excluded", None),
        other => return Err(AppError::Invalid(format!("unknown decision: {other}"))),
    };

    let mut qb = QueryBuilder::<Sqlite>::new("UPDATE transactions SET review_status = ");
    qb.push_bind(status);
    qb.push(", owner_person_id = ").push_bind(owner);
    qb.push(", updated_at = ").push_bind(now());
    qb.push(" WHERE id IN (");
    let mut sep = qb.separated(", ");
    for id in &input.txn_ids {
        sep.push_bind(id.clone());
    }
    qb.push(")");
    qb.build().execute(&db.pool).await?;
    Ok(())
}

/// Send confirmed/excluded transactions back to `pending` (only on shared accounts).
#[tauri::command]
pub async fn review_reopen(state: State<'_, AppState>, txn_ids: Vec<String>) -> AppResult<()> {
    if txn_ids.is_empty() {
        return Ok(());
    }
    let db = state.db().await?;
    let mut qb = QueryBuilder::<Sqlite>::new(
        "UPDATE transactions SET review_status = 'pending', owner_person_id = NULL, updated_at = ",
    );
    qb.push_bind(now());
    qb.push(
        " WHERE id IN (SELECT t.id FROM transactions t JOIN accounts a ON a.id = t.account_id
                       WHERE a.is_shared = 1 AND t.id IN (",
    );
    let mut sep = qb.separated(", ");
    for id in &txn_ids {
        sep.push_bind(id.clone());
    }
    qb.push("))");
    qb.build().execute(&db.pool).await?;
    Ok(())
}

// --- rules -------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct RuleView {
    pub id: String,
    pub priority: i64,
    pub enabled: bool,
    pub match_field: String,
    pub match_op: String,
    pub match_value: String,
    pub match_value2: Option<String>,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub set_person_id: Option<String>,
    pub set_person_name: Option<String>,
    pub set_category_id: Option<String>,
    pub high_confidence: bool,
}

#[derive(Deserialize)]
pub struct RuleInput {
    pub match_field: String,
    pub match_op: String,
    pub match_value: String,
    pub match_value2: Option<String>,
    pub account_id: Option<String>,
    pub set_person_id: Option<String>,
    pub set_category_id: Option<String>,
    pub high_confidence: bool,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}

const VALID_FIELDS: &[&str] = &["description", "merchant_name", "amount"];
const VALID_OPS: &[&str] = &["contains", "equals", "regex", "gt", "lt", "between"];

fn validate_rule(input: &RuleInput) -> AppResult<()> {
    if !VALID_FIELDS.contains(&input.match_field.as_str()) {
        return Err(AppError::Invalid(format!("bad match_field: {}", input.match_field)));
    }
    if !VALID_OPS.contains(&input.match_op.as_str()) {
        return Err(AppError::Invalid(format!("bad match_op: {}", input.match_op)));
    }
    if input.match_value.trim().is_empty() {
        return Err(AppError::Invalid("match value is required".into()));
    }
    if input.match_op == "regex" && regex::Regex::new(&input.match_value).is_err() {
        return Err(AppError::Invalid("invalid regular expression".into()));
    }
    if input.set_person_id.is_none() && input.set_category_id.is_none() {
        return Err(AppError::Invalid("a rule must set a person and/or a category".into()));
    }
    Ok(())
}

#[tauri::command]
pub async fn list_rules(state: State<'_, AppState>) -> AppResult<Vec<RuleView>> {
    let db = state.db().await?;
    Ok(sqlx::query_as::<_, RuleView>(
        "SELECT r.id, r.priority, r.enabled, r.match_field, r.match_op, r.match_value,
                r.match_value2, r.account_id, a.name AS account_name,
                r.set_person_id, pe.name AS set_person_name, r.set_category_id, r.high_confidence
         FROM rules r
         LEFT JOIN accounts a ON a.id = r.account_id
         LEFT JOIN people pe ON pe.id = r.set_person_id
         ORDER BY r.priority ASC, r.created_at ASC",
    )
    .fetch_all(&db.pool)
    .await?)
}

#[tauri::command]
pub async fn create_rule(state: State<'_, AppState>, input: RuleInput) -> AppResult<String> {
    validate_rule(&input)?;
    let db = state.db().await?;
    let id = new_id();
    sqlx::query(
        "INSERT INTO rules
           (id, priority, enabled, match_field, match_op, match_value, match_value2,
            account_id, set_person_id, set_category_id, high_confidence, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )
    .bind(&id)
    .bind(input.priority.unwrap_or(100))
    .bind(input.enabled.unwrap_or(true))
    .bind(&input.match_field)
    .bind(&input.match_op)
    .bind(input.match_value.trim())
    .bind(&input.match_value2)
    .bind(&input.account_id)
    .bind(&input.set_person_id)
    .bind(&input.set_category_id)
    .bind(input.high_confidence)
    .bind(now())
    .execute(&db.pool)
    .await?;
    Ok(id)
}

#[tauri::command]
pub async fn update_rule(
    state: State<'_, AppState>,
    id: String,
    input: RuleInput,
) -> AppResult<()> {
    validate_rule(&input)?;
    let db = state.db().await?;
    let res = sqlx::query(
        "UPDATE rules SET priority = ?2, enabled = ?3, match_field = ?4, match_op = ?5,
             match_value = ?6, match_value2 = ?7, account_id = ?8, set_person_id = ?9,
             set_category_id = ?10, high_confidence = ?11
         WHERE id = ?1",
    )
    .bind(&id)
    .bind(input.priority.unwrap_or(100))
    .bind(input.enabled.unwrap_or(true))
    .bind(&input.match_field)
    .bind(&input.match_op)
    .bind(input.match_value.trim())
    .bind(&input.match_value2)
    .bind(&input.account_id)
    .bind(&input.set_person_id)
    .bind(&input.set_category_id)
    .bind(input.high_confidence)
    .execute(&db.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("rule {id}")));
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_rule(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let db = state.db().await?;
    sqlx::query("DELETE FROM rules WHERE id = ?1")
        .bind(&id)
        .execute(&db.pool)
        .await?;
    // drop suggestions that came from it
    sqlx::query(
        "UPDATE transactions SET suggested_person_id = NULL, suggestion_rule_id = NULL
         WHERE suggestion_rule_id = ?1 AND review_status = 'pending'",
    )
    .bind(&id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn apply_rules_now(state: State<'_, AppState>) -> AppResult<usize> {
    let db = state.db().await?;
    let auto = state
        .config_get_or("auto_confirm_high_confidence", "false")
        .await?
        == "true";
    rules::apply_to_pending(&db.pool, auto).await
}
