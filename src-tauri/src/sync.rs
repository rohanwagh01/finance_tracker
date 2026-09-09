//! Orchestrates read-only Plaid syncs: link completion, account upserts,
//! balance refresh, and cursor-based transaction sync.

use std::collections::HashMap;

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::{AppError, AppResult};
use crate::providers::plaid::{PlaidAccount, PlaidClient, PlaidPfc, PlaidTransaction};
use crate::secrets::item_key;
use crate::state::AppState;
use crate::util::{new_id, now};

#[derive(Debug, Serialize)]
pub struct LinkedItem {
    pub item_id: String,
    pub institution_name: String,
    pub accounts_added: usize,
}

#[derive(Debug, Serialize, Default)]
pub struct SyncSummary {
    pub item_id: String,
    pub accounts_updated: usize,
    pub transactions_added: usize,
    pub transactions_modified: usize,
    pub transactions_removed: usize,
    pub transactions_pending: bool, // Plaid still preparing data
}

/// Stable per-install id sent as Plaid's `user.client_user_id`.
pub async fn plaid_user_id(state: &AppState) -> AppResult<String> {
    if let Some(v) = state.config_get("plaid_user_id").await? {
        return Ok(v);
    }
    let id = new_id();
    state.config_set("plaid_user_id", &id).await?;
    Ok(id)
}

pub async fn plaid_client(state: &AppState) -> AppResult<PlaidClient> {
    let env = state.plaid_env().await?;
    let secrets = state.secrets().await?;
    PlaidClient::from_secrets(state.http.clone(), &secrets, env).await
}

// --- link completion ---------------------------------------------------------

/// Exchange a freshly-linked public token and persist the item + its accounts.
/// Idempotent: a public token whose item we already have is ignored.
pub async fn process_public_token(
    state: &AppState,
    plaid: &PlaidClient,
    public_token: &str,
) -> AppResult<Option<LinkedItem>> {
    let db = state.db().await?;
    let (access_token, provider_item_id) = plaid.exchange_public_token(public_token).await?;

    let dupe: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM items WHERE provider = 'plaid' AND provider_item_id = ?1",
    )
    .bind(&provider_item_id)
    .fetch_optional(&db.pool)
    .await?;
    if dupe.is_some() {
        return Ok(None);
    }

    let institution_id = plaid.item_institution_id(&access_token).await.ok().flatten();
    let institution_name = match &institution_id {
        Some(id) => plaid.institution_name(id).await.ok().flatten(),
        None => None,
    }
    .unwrap_or_else(|| "Linked institution".to_string());

    let item_id = new_id();
    let secret_ref = item_key(&item_id);
    state.secrets().await?.set(&secret_ref, &access_token).await?;

    let ts = now();
    sqlx::query(
        "INSERT INTO items
           (id, provider, provider_item_id, institution_id, institution_name, secret_ref,
            status, created_at, updated_at)
         VALUES (?1, 'plaid', ?2, ?3, ?4, ?5, 'active', ?6, ?6)",
    )
    .bind(&item_id)
    .bind(&provider_item_id)
    .bind(&institution_id)
    .bind(&institution_name)
    .bind(&secret_ref)
    .bind(&ts)
    .execute(&db.pool)
    .await?;

    let accounts = plaid.accounts_get(&access_token).await?;
    let added = upsert_accounts(&db.pool, &item_id, &accounts).await?;

    Ok(Some(LinkedItem {
        item_id,
        institution_name,
        accounts_added: added,
    }))
}

// --- sync ------------------------------------------------------------------

pub async fn sync_item(state: &AppState, item_id: &str) -> AppResult<SyncSummary> {
    let db = state.db().await?;
    let plaid = plaid_client(state).await?;
    let access_token = access_token_for(state, item_id).await?;

    let log_id = new_id();
    sqlx::query(
        "INSERT INTO sync_log (id, item_id, started_at, status) VALUES (?1, ?2, ?3, 'running')",
    )
    .bind(&log_id)
    .bind(item_id)
    .bind(now())
    .execute(&db.pool)
    .await?;

    let result = sync_item_inner(state, &plaid, item_id, &access_token).await;

    match &result {
        Ok(s) => {
            sqlx::query(
                "UPDATE sync_log SET finished_at = ?2, status = 'ok', detail = ?3 WHERE id = ?1",
            )
            .bind(&log_id)
            .bind(now())
            .bind(format!(
                "+{} ~{} -{}",
                s.transactions_added, s.transactions_modified, s.transactions_removed
            ))
            .execute(&db.pool)
            .await?;
            sqlx::query("UPDATE items SET last_synced_at = ?2, status = 'active', error_message = NULL WHERE id = ?1")
                .bind(item_id)
                .bind(now())
                .execute(&db.pool)
                .await?;
        }
        Err(e) => {
            sqlx::query(
                "UPDATE sync_log SET finished_at = ?2, status = 'error', detail = ?3 WHERE id = ?1",
            )
            .bind(&log_id)
            .bind(now())
            .bind(e.to_string())
            .execute(&db.pool)
            .await?;
            sqlx::query("UPDATE items SET status = 'error', error_message = ?2 WHERE id = ?1")
                .bind(item_id)
                .bind(e.to_string())
                .execute(&db.pool)
                .await?;
        }
    }

    result
}

async fn sync_item_inner(
    state: &AppState,
    plaid: &PlaidClient,
    item_id: &str,
    access_token: &str,
) -> AppResult<SyncSummary> {
    let db = state.db().await?;
    let mut summary = SyncSummary {
        item_id: item_id.to_string(),
        ..Default::default()
    };

    // 1. balances — try the real-time endpoint, fall back to cached balances
    //    from /accounts/get so a balance hiccup never blocks the transaction sync.
    let accounts = match plaid.accounts_balance_get(access_token).await {
        Ok(a) => a,
        Err(balance_err) => match plaid.accounts_get(access_token).await {
            Ok(a) => a,
            Err(_) => return Err(balance_err),
        },
    };
    summary.accounts_updated = upsert_accounts(&db.pool, item_id, &accounts).await?;
    snapshot_balances(&db.pool, item_id).await?;

    // 2. transactions (cursor-based, paginated)
    let account_map = account_id_map(&db.pool, item_id).await?;
    let mut cursor: Option<String> = sqlx::query_scalar("SELECT cursor FROM items WHERE id = ?1")
        .bind(item_id)
        .fetch_one(&db.pool)
        .await?;

    loop {
        let Some(page) = plaid.transactions_sync(access_token, cursor.as_deref()).await? else {
            summary.transactions_pending = true;
            break;
        };

        for t in page.added.iter().chain(page.modified.iter()) {
            let Some(account_uuid) = account_map.get(&t.account_id) else {
                continue;
            };
            let is_new = upsert_transaction(&db.pool, account_uuid, t).await?;
            if is_new {
                summary.transactions_added += 1;
            } else {
                summary.transactions_modified += 1;
            }
        }
        for r in &page.removed {
            let n = sqlx::query(
                "DELETE FROM transactions WHERE provider_txn_id = ?1
                   AND account_id IN (SELECT id FROM accounts WHERE item_id = ?2)",
            )
            .bind(&r.transaction_id)
            .bind(item_id)
            .execute(&db.pool)
            .await?
            .rows_affected();
            summary.transactions_removed += n as usize;
        }

        cursor = Some(page.next_cursor.clone());
        sqlx::query("UPDATE items SET cursor = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(item_id)
            .bind(&page.next_cursor)
            .bind(now())
            .execute(&db.pool)
            .await?;

        if !page.has_more {
            break;
        }
    }

    Ok(summary)
}

pub async fn sync_all(state: &AppState) -> AppResult<Vec<SyncSummary>> {
    let db = state.db().await?;
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM items WHERE provider = 'plaid' AND status != 'disconnected'")
            .fetch_all(&db.pool)
            .await?;
    let mut out = Vec::new();
    for id in ids {
        out.push(sync_item(state, &id).await?);
    }
    Ok(out)
}

// --- helpers -------------------------------------------------------------

async fn access_token_for(state: &AppState, item_id: &str) -> AppResult<String> {
    let db = state.db().await?;
    let secret_ref: Option<String> =
        sqlx::query_scalar("SELECT secret_ref FROM items WHERE id = ?1")
            .bind(item_id)
            .fetch_optional(&db.pool)
            .await?;
    let secret_ref = secret_ref.ok_or_else(|| AppError::NotFound(format!("item {item_id}")))?;
    state
        .secrets()
        .await?
        .get(&secret_ref)
        .await?
        .ok_or_else(|| AppError::Config(format!("missing access token for item {item_id}")))
}

async fn account_id_map(
    pool: &SqlitePool,
    item_id: &str,
) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query("SELECT provider_account_id, id FROM accounts WHERE item_id = ?1")
        .bind(item_id)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<String, _>(0), r.get::<String, _>(1)))
        .collect())
}

async fn upsert_accounts(
    pool: &SqlitePool,
    item_id: &str,
    accounts: &[PlaidAccount],
) -> AppResult<usize> {
    let mut count = 0;
    for a in accounts {
        let currency = a
            .balances
            .iso_currency_code
            .clone()
            .unwrap_or_else(|| "USD".into());
        let res = sqlx::query(
            "INSERT INTO accounts
               (id, item_id, provider_account_id, name, official_name, mask, type, subtype,
                currency, current_balance, available_balance, credit_limit, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
             ON CONFLICT(item_id, provider_account_id) DO UPDATE SET
               name = excluded.name,
               official_name = excluded.official_name,
               mask = excluded.mask,
               type = excluded.type,
               subtype = excluded.subtype,
               currency = excluded.currency,
               current_balance = excluded.current_balance,
               available_balance = excluded.available_balance,
               credit_limit = excluded.credit_limit,
               updated_at = excluded.updated_at",
        )
        .bind(new_id())
        .bind(item_id)
        .bind(&a.account_id)
        .bind(&a.name)
        .bind(&a.official_name)
        .bind(&a.mask)
        .bind(&a.account_type)
        .bind(&a.subtype)
        .bind(&currency)
        .bind(a.balances.current)
        .bind(a.balances.available)
        .bind(a.balances.limit)
        .bind(now())
        .execute(pool)
        .await?;
        count += res.rows_affected() as usize;
    }
    Ok(count)
}

async fn snapshot_balances(pool: &SqlitePool, item_id: &str) -> AppResult<()> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    sqlx::query(
        "INSERT INTO value_snapshots (id, snapshot_date, scope, account_id, value, created_at)
         SELECT ?1 || a.id, ?2, 'account', a.id,
                COALESCE(a.current_balance, 0) *
                  CASE WHEN a.type IN ('credit','loan') THEN -1 ELSE 1 END,
                ?3
         FROM accounts a WHERE a.item_id = ?4
         ON CONFLICT(snapshot_date, scope, account_id) DO UPDATE SET value = excluded.value",
    )
    .bind(format!("{today}:"))
    .bind(&today)
    .bind(now())
    .bind(item_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns true if the row was newly inserted.
async fn upsert_transaction(
    pool: &SqlitePool,
    account_uuid: &str,
    t: &PlaidTransaction,
) -> AppResult<bool> {
    let (category_id, detailed_label) = match &t.personal_finance_category {
        Some(pfc) => (Some(ensure_category(pool, pfc).await?), Some(pfc.detailed.clone())),
        None => (None, None),
    };
    let _ = detailed_label;

    let currency = t.iso_currency_code.clone().unwrap_or_else(|| "USD".into());
    let location_json = t
        .location
        .as_ref()
        .map(|l| l.to_string())
        .filter(|s| s != "null");

    // Does it already exist?
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM transactions WHERE account_id = ?1 AND provider_txn_id = ?2",
    )
    .bind(account_uuid)
    .bind(&t.transaction_id)
    .fetch_optional(pool)
    .await?;

    if let Some((_id,)) = existing {
        // Update provider-owned fields only; never touch attribution/user fields.
        sqlx::query(
            "UPDATE transactions SET
               posted_date = ?3, authorized_date = ?4, amount = ?5, currency = ?6,
               description = ?7, merchant_name = ?8, pending = ?9,
               category_id = COALESCE(?10, category_id),
               location_json = ?11, updated_at = ?12
             WHERE account_id = ?1 AND provider_txn_id = ?2",
        )
        .bind(account_uuid)
        .bind(&t.transaction_id)
        .bind(&t.date)
        .bind(&t.authorized_date)
        .bind(t.amount)
        .bind(&currency)
        .bind(&t.name)
        .bind(&t.merchant_name)
        .bind(t.pending)
        .bind(&category_id)
        .bind(&location_json)
        .bind(now())
        .execute(pool)
        .await?;
        return Ok(false);
    }

    // New: shared accounts route the charge into the review inbox.
    let is_shared: bool =
        sqlx::query_scalar("SELECT is_shared FROM accounts WHERE id = ?1")
            .bind(account_uuid)
            .fetch_one(pool)
            .await?;
    let review_status = if is_shared { "pending" } else { "not_required" };

    sqlx::query(
        "INSERT INTO transactions
           (id, account_id, provider_txn_id, posted_date, authorized_date, amount, currency,
            description, merchant_name, category_id, category_source, pending,
            review_status, location_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'provider', ?11, ?12, ?13, ?14, ?14)",
    )
    .bind(new_id())
    .bind(account_uuid)
    .bind(&t.transaction_id)
    .bind(&t.date)
    .bind(&t.authorized_date)
    .bind(t.amount)
    .bind(&currency)
    .bind(&t.name)
    .bind(&t.merchant_name)
    .bind(&category_id)
    .bind(t.pending)
    .bind(review_status)
    .bind(&location_json)
    .bind(now())
    .execute(pool)
    .await?;
    Ok(true)
}

/// Ensure the primary + detailed Plaid categories exist; return the detailed id.
async fn ensure_category(
    pool: &SqlitePool,
    pfc: &PlaidPfc,
) -> AppResult<String> {
    sqlx::query("INSERT OR IGNORE INTO categories (id, parent_id, label, is_custom) VALUES (?1, NULL, ?2, 0)")
        .bind(&pfc.primary)
        .bind(prettify(&pfc.primary))
        .execute(pool)
        .await?;

    let label = prettify(pfc.detailed.strip_prefix(&format!("{}_", pfc.primary)).unwrap_or(&pfc.detailed));
    sqlx::query("INSERT OR IGNORE INTO categories (id, parent_id, label, is_custom) VALUES (?1, ?2, ?3, 0)")
        .bind(&pfc.detailed)
        .bind(&pfc.primary)
        .bind(&label)
        .execute(pool)
        .await?;
    Ok(pfc.detailed.clone())
}

fn prettify(slug: &str) -> String {
    slug.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
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

    #[test]
    fn prettify_titlecases_slugs() {
        assert_eq!(prettify("FOOD_AND_DRINK"), "Food And Drink");
        assert_eq!(prettify("ONLINE_MARKETPLACES"), "Online Marketplaces");
        assert_eq!(prettify(""), "");
    }

    fn txn(id: &str, account: &str, amount: f64, primary: &str, detailed: &str) -> PlaidTransaction {
        PlaidTransaction {
            transaction_id: id.into(),
            account_id: account.into(),
            date: "2026-09-01".into(),
            authorized_date: Some("2026-08-31".into()),
            amount,
            iso_currency_code: Some("USD".into()),
            name: "COFFEE SHOP".into(),
            merchant_name: Some("Coffee Shop".into()),
            pending: false,
            personal_finance_category: Some(PlaidPfc {
                primary: primary.into(),
                detailed: detailed.into(),
            }),
            location: None,
        }
    }

    #[tokio::test]
    async fn upsert_creates_category_and_routes_shared_charges_to_review() {
        let db = Db::connect_in_memory().await.unwrap();
        let pool = &db.pool;
        let ts = now();

        sqlx::query("INSERT INTO items (id, provider, secret_ref, status, created_at, updated_at) VALUES ('i1','plaid','item.i1','active',?1,?1)")
            .bind(&ts).execute(pool).await.unwrap();
        // one shared account, one normal
        for (aid, shared) in [("a_shared", 1), ("a_solo", 0)] {
            sqlx::query("INSERT INTO accounts (id, item_id, provider_account_id, name, type, currency, is_shared, created_at, updated_at) VALUES (?1,'i1',?1,'Card','credit','USD',?2,?3,?3)")
                .bind(aid).bind(shared).bind(&ts).execute(pool).await.unwrap();
        }

        // new spending charge on the shared account -> pending
        let new = upsert_transaction(pool, "a_shared", &txn("t1", "x", 4.50, "FOOD_AND_DRINK", "FOOD_AND_DRINK_COFFEE")).await.unwrap();
        assert!(new);
        // same on solo account -> not_required
        upsert_transaction(pool, "a_solo", &txn("t2", "x", 9.0, "FOOD_AND_DRINK", "FOOD_AND_DRINK_COFFEE")).await.unwrap();

        let statuses: Vec<(String, String)> = sqlx::query_as(
            "SELECT a.id, t.review_status FROM transactions t JOIN accounts a ON a.id = t.account_id ORDER BY a.id",
        ).fetch_all(pool).await.unwrap();
        assert_eq!(statuses, vec![
            ("a_shared".to_string(), "pending".to_string()),
            ("a_solo".to_string(), "not_required".to_string()),
        ]);

        // detailed category created with the primary as parent
        let cat: (String, Option<String>) = sqlx::query_as(
            "SELECT id, parent_id FROM categories WHERE id = 'FOOD_AND_DRINK_COFFEE'",
        ).fetch_one(pool).await.unwrap();
        assert_eq!(cat, ("FOOD_AND_DRINK_COFFEE".into(), Some("FOOD_AND_DRINK".into())));

        // re-upsert same txn id -> update, not insert, and keep it as not new
        let again = upsert_transaction(pool, "a_shared", &txn("t1", "x", 5.25, "FOOD_AND_DRINK", "FOOD_AND_DRINK_COFFEE")).await.unwrap();
        assert!(!again);
        let amt: f64 = sqlx::query_scalar("SELECT amount FROM transactions WHERE provider_txn_id = 't1'").fetch_one(pool).await.unwrap();
        assert_eq!(amt, 5.25);
    }
}
