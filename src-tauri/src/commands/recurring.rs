//! Recurring payments and income — manually entered or auto-detected from the
//! transaction history.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::{new_id, now};

/// SQL: monthly-equivalent of `amount` given `cadence`.
const MONTHLY_EQUIV: &str = "(amount * CASE cadence
    WHEN 'weekly'    THEN 52.0/12.0
    WHEN 'biweekly'  THEN 26.0/12.0
    WHEN 'monthly'   THEN 1.0
    WHEN 'quarterly' THEN 1.0/3.0
    WHEN 'yearly'    THEN 1.0/12.0
    ELSE 1.0 END)";

const VALID_CADENCE: &[&str] = &["weekly", "biweekly", "monthly", "quarterly", "yearly"];

#[derive(Serialize, sqlx::FromRow)]
pub struct Recurring {
    pub id: String,
    pub label: String,
    pub amount: f64,
    pub cadence: String,
    pub kind: String, // expense | income
    pub monthly_equiv: f64,
    pub category_id: Option<String>,
    pub category_label: Option<String>,
    pub source: String, // manual | detected
    pub active: bool,
    pub next_due: Option<String>,
}

#[derive(Deserialize)]
pub struct RecurringInput {
    pub label: String,
    pub amount: f64,
    pub cadence: String,
    pub kind: String,
    pub category_id: Option<String>,
    pub next_due: Option<String>,
    pub active: Option<bool>,
}

#[tauri::command]
pub async fn list_recurring(state: State<'_, AppState>) -> AppResult<Vec<Recurring>> {
    let db = state.db().await?;
    Ok(sqlx::query_as::<_, Recurring>(&format!(
        "SELECT r.id, r.label, r.amount, r.cadence, r.kind, {MONTHLY_EQUIV} AS monthly_equiv,
                r.category_id, c.label AS category_label, r.source, r.active, r.next_due
         FROM recurring_payments r
         LEFT JOIN categories c ON c.id = r.category_id
         ORDER BY r.kind, monthly_equiv DESC"
    ))
    .fetch_all(&db.pool)
    .await?)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RecurringSummary {
    pub monthly_expense: f64,
    pub monthly_income: f64,
}

#[tauri::command]
pub async fn recurring_summary(state: State<'_, AppState>) -> AppResult<RecurringSummary> {
    let db = state.db().await?;
    Ok(sqlx::query_as::<_, RecurringSummary>(&format!(
        "SELECT COALESCE(SUM(CASE WHEN kind = 'expense' THEN {MONTHLY_EQUIV} END), 0.0) AS monthly_expense,
                COALESCE(SUM(CASE WHEN kind = 'income'  THEN {MONTHLY_EQUIV} END), 0.0) AS monthly_income
         FROM recurring_payments WHERE active = 1"
    ))
    .fetch_one(&db.pool)
    .await?)
}

fn validate(input: &RecurringInput) -> AppResult<()> {
    if input.label.trim().is_empty() {
        return Err(AppError::Invalid("label is required".into()));
    }
    if !VALID_CADENCE.contains(&input.cadence.as_str()) {
        return Err(AppError::Invalid(format!("bad cadence: {}", input.cadence)));
    }
    if !["expense", "income"].contains(&input.kind.as_str()) {
        return Err(AppError::Invalid("kind must be expense or income".into()));
    }
    Ok(())
}

#[tauri::command]
pub async fn create_recurring(
    state: State<'_, AppState>,
    input: RecurringInput,
) -> AppResult<String> {
    validate(&input)?;
    let db = state.db().await?;
    let id = new_id();
    sqlx::query(
        "INSERT INTO recurring_payments (id, label, amount, cadence, kind, category_id, source, active, next_due, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'manual', ?7, ?8, ?9, ?9)",
    )
    .bind(&id)
    .bind(input.label.trim())
    .bind(input.amount.abs())
    .bind(&input.cadence)
    .bind(&input.kind)
    .bind(&input.category_id)
    .bind(input.active.unwrap_or(true))
    .bind(&input.next_due)
    .bind(now())
    .execute(&db.pool)
    .await?;
    Ok(id)
}

#[tauri::command]
pub async fn update_recurring(
    state: State<'_, AppState>,
    id: String,
    input: RecurringInput,
) -> AppResult<()> {
    validate(&input)?;
    let db = state.db().await?;
    let res = sqlx::query(
        "UPDATE recurring_payments SET label = ?2, amount = ?3, cadence = ?4, kind = ?5,
             category_id = ?6, active = ?7, next_due = ?8, updated_at = ?9 WHERE id = ?1",
    )
    .bind(&id)
    .bind(input.label.trim())
    .bind(input.amount.abs())
    .bind(&input.cadence)
    .bind(&input.kind)
    .bind(&input.category_id)
    .bind(input.active.unwrap_or(true))
    .bind(&input.next_due)
    .bind(now())
    .execute(&db.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("recurring {id}")));
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_recurring(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let db = state.db().await?;
    sqlx::query("DELETE FROM recurring_payments WHERE id = ?1")
        .bind(&id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

// --- detection -------------------------------------------------------

fn normalize_merchant(s: &str) -> String {
    let cleaned: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
        .collect();
    cleaned
        .split_whitespace()
        .filter(|w| !w.chars().all(|c| c.is_ascii_digit()))
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn classify_cadence(median_gap_days: f64) -> Option<(&'static str, f64)> {
    let g = median_gap_days;
    if (5.0..=9.0).contains(&g) {
        Some(("weekly", 7.0))
    } else if (11.0..=17.0).contains(&g) {
        Some(("biweekly", 14.0))
    } else if (25.0..=35.0).contains(&g) {
        Some(("monthly", 30.44))
    } else if (84.0..=98.0).contains(&g) {
        Some(("quarterly", 91.3))
    } else if (350.0..=380.0).contains(&g) {
        Some(("yearly", 365.25))
    } else {
        None
    }
}

#[derive(Serialize)]
pub struct DetectResult {
    pub added: usize,
}

/// Scan ~15 months of transactions for repeating merchant + amount patterns and
/// insert them as `detected` recurring rows (skipping any that already exist).
#[tauri::command]
pub async fn detect_recurring(state: State<'_, AppState>) -> AppResult<DetectResult> {
    let db = state.db().await?;
    let added = detect(&db.pool).await?;
    Ok(DetectResult { added })
}

pub async fn detect(pool: &SqlitePool) -> AppResult<usize> {
    let rows: Vec<(String, f64, String, Option<String>)> = sqlx::query_as(
        "SELECT COALESCE(NULLIF(t.merchant_name, ''), t.description) AS merchant,
                t.amount, t.posted_date, t.category_id
         FROM transactions t
         JOIN accounts a ON a.id = t.account_id
         WHERE t.is_transfer = 0 AND t.review_status != 'excluded' AND a.is_hidden = 0
           AND t.posted_date >= date('now', '-460 days')",
    )
    .fetch_all(pool)
    .await?;

    let epoch = chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    // group: normalized merchant -> list of (amount, day-offset from epoch, category)
    let mut groups: std::collections::HashMap<String, Vec<(f64, i64, Option<String>)>> =
        Default::default();
    for (merch, amount, date, cat) in rows {
        let Ok(d) = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d") else { continue };
        groups
            .entry(normalize_merchant(&merch))
            .or_default()
            .push((amount, (d - epoch).num_days(), cat));
    }

    let ts = now();
    let mut added = 0usize;

    for (merch, mut txns) in groups {
        if merch.is_empty() || txns.len() < 3 {
            continue;
        }
        txns.sort_by_key(|t| t.1);

        // cluster by amount (same sign, within 8% or $1 of the running median)
        let mut clusters: Vec<Vec<(f64, i64, Option<String>)>> = Vec::new();
        for t in txns {
            let hit = clusters.iter_mut().find(|c| {
                let m = median(c.iter().map(|x| x.0).collect());
                m.signum() == t.0.signum()
                    && (m - t.0).abs() <= (m.abs() * 0.08).max(1.0)
            });
            match hit {
                Some(c) => c.push(t),
                None => clusters.push(vec![t]),
            }
        }

        for c in clusters {
            if c.len() < 3 {
                continue;
            }
            let gaps: Vec<f64> = c.windows(2).map(|w| (w[1].1 - w[0].1) as f64).collect();
            let mgap = median(gaps.clone());
            let Some((cadence, cadence_days)) = classify_cadence(mgap) else { continue };
            let span = (c.last().unwrap().1 - c[0].1) as f64;
            if span < cadence_days * 1.8 {
                continue;
            }
            // most gaps should be near the cadence
            let ok = gaps.iter().filter(|g| (**g - cadence_days).abs() <= cadence_days * 0.4).count();
            if (ok as f64) < gaps.len() as f64 * 0.6 {
                continue;
            }

            let amount = median(c.iter().map(|x| x.0.abs()).collect());
            let avg: f64 = c.iter().map(|x| x.0).sum::<f64>() / c.len() as f64;
            let kind = if avg > 0.0 { "expense" } else { "income" };
            let cat = c.iter().rev().find_map(|x| x.2.clone());
            let last_day = c.last().unwrap().1;
            let next_due = Some(
                (epoch + chrono::Duration::days(last_day + cadence_days.round() as i64))
                    .format("%Y-%m-%d")
                    .to_string(),
            );
            let label = c
                .last()
                .map(|_| titlecase(&merch))
                .unwrap_or_else(|| merch.clone());

            let res = sqlx::query(
                "INSERT OR IGNORE INTO recurring_payments
                   (id, label, amount, cadence, kind, category_id, source, detected_merchant, active, next_due, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'detected', ?7, 1, ?8, ?9, ?9)",
            )
            .bind(new_id())
            .bind(&label)
            .bind(amount)
            .bind(cadence)
            .bind(kind)
            .bind(&cat)
            .bind(&merch)
            .bind(&next_due)
            .bind(&ts)
            .execute(pool)
            .await?;
            added += res.rows_affected() as usize;
        }
    }
    Ok(added)
}

fn titlecase(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut ch = w.chars();
            match ch.next() {
                Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[tokio::test]
    async fn detects_a_monthly_subscription_and_skips_one_offs() {
        let db = Db::connect_in_memory().await.unwrap();
        let p = &db.pool;
        let ts = now();
        sqlx::query("INSERT INTO items (id,provider,secret_ref,status,created_at,updated_at) VALUES ('i','plaid','x','active',?1,?1)").bind(&ts).execute(p).await.unwrap();
        sqlx::query("INSERT INTO accounts (id,item_id,provider_account_id,name,type,currency,is_hidden,created_at,updated_at) VALUES ('a','i','a','A','depository','USD',0,?1,?1)").bind(&ts).execute(p).await.unwrap();

        let ins = |id: &str, merch: &str, amt: f64, date: &str| {
            let q = sqlx::query("INSERT INTO transactions (id,account_id,provider_txn_id,posted_date,amount,currency,description,merchant_name,category_source,pending,review_status,is_transfer,created_at,updated_at) VALUES (?1,'a',?1,?2,?3,'USD',?4,?4,'provider',0,'not_required',0,?5,?5)")
                .bind(id.to_string()).bind(date.to_string()).bind(amt).bind(merch.to_string()).bind(ts.clone());
            async move { q.execute(p).await.unwrap(); }
        };
        // Netflix ~monthly for 5 months
        ins("n1", "Netflix", 15.99, "2026-04-05").await;
        ins("n2", "Netflix", 15.99, "2026-05-06").await;
        ins("n3", "Netflix", 15.99, "2026-06-05").await;
        ins("n4", "Netflix", 15.99, "2026-07-06").await;
        ins("n5", "Netflix", 15.99, "2026-08-05").await;
        // paycheck biweekly (income)
        ins("p1", "ACME PAYROLL", -2000.0, "2026-06-01").await;
        ins("p2", "ACME PAYROLL", -2000.0, "2026-06-15").await;
        ins("p3", "ACME PAYROLL", -2000.0, "2026-06-29").await;
        ins("p4", "ACME PAYROLL", -2000.0, "2026-07-13").await;
        // one-off
        ins("o1", "Best Buy", 400.0, "2026-06-10").await;

        let n = detect(p).await.unwrap();
        assert_eq!(n, 2);

        let rows: Vec<(String, String, String, f64)> = sqlx::query_as(
            "SELECT label, cadence, kind, amount FROM recurring_payments ORDER BY kind",
        )
        .fetch_all(p)
        .await
        .unwrap();
        let netflix = rows.iter().find(|r| r.2 == "expense").unwrap();
        assert_eq!(netflix.1, "monthly");
        assert!((netflix.3 - 15.99).abs() < 0.01);
        let pay = rows.iter().find(|r| r.2 == "income").unwrap();
        assert_eq!(pay.1, "biweekly");

        // idempotent
        assert_eq!(detect(p).await.unwrap(), 0);
    }
}
