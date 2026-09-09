//! Spending analytics: totals, monthly series, the category → subcategory →
//! merchant → transaction drill-down, and a filterable transaction list.
//!
//! "Spending" everywhere means: money out (`amount > 0`), not a transfer, not
//! excluded from review, on a visible account, and not in a transfer / loan-
//! payment category (those would double-count credit-card payments).

use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::now;

const EXCLUDED_PRIMARY: &str = "'TRANSFER_IN','TRANSFER_OUT','LOAN_PAYMENTS'";

const FROM_CLAUSE: &str = "
    FROM transactions t
    JOIN accounts a ON a.id = t.account_id
    LEFT JOIN categories c ON c.id = t.category_id
    LEFT JOIN categories p ON p.id = c.parent_id
";

#[derive(Deserialize, Default)]
pub struct SpendingFilter {
    pub from: String, // YYYY-MM-DD
    pub to: String,
    pub account_ids: Option<Vec<String>>,
    pub person_id: Option<String>,
    /// Show only transactions the user excluded from their spending. When set,
    /// `person_id` is ignored.
    pub excluded_only: Option<bool>,
}

impl SpendingFilter {
    fn excluded(&self) -> bool {
        self.excluded_only == Some(true)
    }
}

/// Whether `filter.person_id` (if any) refers to the primary "self" person.
/// The self person also owns everything that never needed review
/// (`review_status = 'not_required'`).
async fn person_is_self(pool: &SqlitePool, f: &SpendingFilter) -> AppResult<bool> {
    if f.excluded() {
        return Ok(false);
    }
    match &f.person_id {
        Some(id) => Ok(
            sqlx::query_scalar::<_, bool>("SELECT is_self FROM people WHERE id = ?1")
                .bind(id)
                .fetch_optional(pool)
                .await?
                .unwrap_or(false),
        ),
        None => Ok(false),
    }
}

fn push_where(qb: &mut QueryBuilder<Sqlite>, f: &SpendingFilter, person_self: bool) {
    // Attribution (owner / excluded) only counts on accounts that are still
    // marked shared. On a non-shared account every charge is the primary
    // person's, regardless of any decision stored from when it was shared.
    qb.push(" WHERE t.amount > 0 AND t.is_transfer = 0 AND a.is_hidden = 0 ");
    if f.excluded() {
        qb.push(" AND a.is_shared = 1 AND t.review_status = 'excluded' ");
    } else {
        qb.push(" AND NOT (a.is_shared = 1 AND t.review_status = 'excluded') ");
    }
    qb.push(format!(
        " AND COALESCE(p.id, c.id, '') NOT IN ({EXCLUDED_PRIMARY}) "
    ));
    qb.push(" AND t.posted_date >= ").push_bind(f.from.clone());
    qb.push(" AND t.posted_date <= ").push_bind(f.to.clone());
    if let Some(ids) = f.account_ids.as_ref().filter(|v| !v.is_empty()) {
        qb.push(" AND t.account_id IN (");
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(id.clone());
        }
        qb.push(")");
    }
    if !f.excluded() {
        if let Some(pid) = &f.person_id {
            if person_self {
                qb.push(" AND (a.is_shared = 0 OR t.owner_person_id = ")
                    .push_bind(pid.clone())
                    .push(" OR t.review_status = 'not_required') ");
            } else {
                qb.push(" AND a.is_shared = 1 AND t.owner_person_id = ")
                    .push_bind(pid.clone());
            }
        }
    }
}

// --- summary ---------------------------------------------------------------

#[derive(Serialize)]
pub struct SpendingSummary {
    pub total: f64,
    pub txn_count: i64,
    pub by_month: Vec<MonthTotal>,
    pub by_category: Vec<Bucket>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MonthTotal {
    pub month: String,
    pub total: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Bucket {
    pub id: String,
    pub label: String,
    pub total: f64,
    pub count: i64,
}

pub async fn summary(pool: &SqlitePool, filter: &SpendingFilter) -> AppResult<SpendingSummary> {
    let ps = person_is_self(pool, filter).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT COALESCE(SUM(t.amount), 0.0) AS total, COUNT(*) AS count",
    );
    qb.push(FROM_CLAUSE);
    push_where(&mut qb, filter, ps);
    let (total, txn_count): (f64, i64) = qb.build_query_as().fetch_one(pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT strftime('%Y-%m', t.posted_date) AS month, COALESCE(SUM(t.amount), 0.0) AS total",
    );
    qb.push(FROM_CLAUSE);
    push_where(&mut qb, filter, ps);
    qb.push(" GROUP BY 1 ORDER BY 1");
    let by_month: Vec<MonthTotal> = qb.build_query_as().fetch_all(pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT COALESCE(p.id, c.id, 'UNCATEGORIZED') AS id,
                COALESCE(p.label, c.label, 'Uncategorized') AS label,
                COALESCE(SUM(t.amount), 0.0) AS total, COUNT(*) AS count",
    );
    qb.push(FROM_CLAUSE);
    push_where(&mut qb, filter, ps);
    qb.push(" GROUP BY 1, 2 ORDER BY 3 DESC");
    let by_category: Vec<Bucket> = qb.build_query_as().fetch_all(pool).await?;

    Ok(SpendingSummary { total, txn_count, by_month, by_category })
}

#[tauri::command]
pub async fn spending_summary(
    state: State<'_, AppState>,
    filter: SpendingFilter,
) -> AppResult<SpendingSummary> {
    summary(&state.db().await?.pool, &filter).await
}

// --- monthly trends by category -------------------------------------

#[derive(Serialize)]
pub struct SpendingTrends {
    pub months: Vec<String>,
    /// One entry per category (top N by total, plus a rolled-up "Other"),
    /// each carrying a value for every month in `months` (same order).
    pub series: Vec<TrendSeries>,
}

#[derive(Serialize)]
pub struct TrendSeries {
    pub id: String,
    pub label: String,
    pub values: Vec<f64>,
}

#[derive(sqlx::FromRow)]
struct MonthCat {
    month: String,
    cat_id: String,
    label: String,
    total: f64,
}

pub async fn trends(pool: &SqlitePool, filter: &SpendingFilter) -> AppResult<SpendingTrends> {
    const TOP_N: usize = 6;
    let ps = person_is_self(pool, filter).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT strftime('%Y-%m', t.posted_date) AS month,
                COALESCE(p.id, c.id, 'UNCATEGORIZED') AS cat_id,
                COALESCE(p.label, c.label, 'Uncategorized') AS label,
                COALESCE(SUM(t.amount), 0.0) AS total",
    );
    qb.push(FROM_CLAUSE);
    push_where(&mut qb, filter, ps);
    qb.push(" GROUP BY 1, 2, 3");
    let rows: Vec<MonthCat> = qb.build_query_as().fetch_all(pool).await?;

    // ordered unique months
    let mut months: Vec<String> = rows.iter().map(|r| r.month.clone()).collect();
    months.sort();
    months.dedup();
    let month_idx: std::collections::HashMap<&str, usize> =
        months.iter().enumerate().map(|(i, m)| (m.as_str(), i)).collect();

    // category totals to pick the top N
    let mut cat_total: std::collections::HashMap<String, (String, f64)> = Default::default();
    for r in &rows {
        let e = cat_total.entry(r.cat_id.clone()).or_insert((r.label.clone(), 0.0));
        e.1 += r.total;
    }
    let mut ranked: Vec<(String, String, f64)> = cat_total
        .into_iter()
        .map(|(id, (label, tot))| (id, label, tot))
        .collect();
    ranked.sort_by(|a, b| b.2.total_cmp(&a.2));
    let top: std::collections::HashSet<String> =
        ranked.iter().take(TOP_N).map(|r| r.0.clone()).collect();

    let mut series: std::collections::HashMap<String, TrendSeries> = Default::default();
    for (id, label, _) in ranked.iter().take(TOP_N) {
        series.insert(
            id.clone(),
            TrendSeries { id: id.clone(), label: label.clone(), values: vec![0.0; months.len()] },
        );
    }
    let has_other = ranked.len() > TOP_N;
    if has_other {
        series.insert(
            "OTHER".into(),
            TrendSeries { id: "OTHER".into(), label: "Other".into(), values: vec![0.0; months.len()] },
        );
    }

    for r in &rows {
        let Some(&mi) = month_idx.get(r.month.as_str()) else { continue };
        let key = if top.contains(&r.cat_id) { r.cat_id.as_str() } else { "OTHER" };
        if let Some(s) = series.get_mut(key) {
            s.values[mi] += r.total;
        }
    }

    // return in ranked order, Other last
    let mut out: Vec<TrendSeries> = ranked
        .iter()
        .take(TOP_N)
        .filter_map(|(id, _, _)| series.remove(id))
        .collect();
    if let Some(o) = series.remove("OTHER") {
        out.push(o);
    }

    Ok(SpendingTrends { months, series: out })
}

#[tauri::command]
pub async fn spending_trends(
    state: State<'_, AppState>,
    filter: SpendingFilter,
) -> AppResult<SpendingTrends> {
    trends(&state.db().await?.pool, &filter).await
}

// --- per person -------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct PersonSpend {
    pub person_id: String,
    pub person_name: String,
    pub is_self: bool,
    pub total: f64,
    pub count: i64,
}

/// Spend grouped by owner. Unowned `not_required` transactions (everything on
/// non-shared accounts) are attributed to the primary person. `person_id` on
/// the filter is ignored here.
#[tauri::command]
pub async fn spending_by_person(
    state: State<'_, AppState>,
    filter: SpendingFilter,
) -> AppResult<Vec<PersonSpend>> {
    let db = state.db().await?;
    let f = SpendingFilter { person_id: None, excluded_only: None, ..filter };

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT CASE WHEN a.is_shared = 1 THEN COALESCE(t.owner_person_id, self.id) ELSE self.id END AS person_id,
                CASE WHEN a.is_shared = 1 THEN COALESCE(own.name, self.name) ELSE self.name END AS person_name,
                CASE WHEN a.is_shared = 1 THEN COALESCE(own.is_self, self.is_self) ELSE self.is_self END AS is_self,
                COALESCE(SUM(t.amount), 0.0) AS total, COUNT(*) AS count
         FROM transactions t
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN categories c ON c.id = t.category_id
         LEFT JOIN categories p ON p.id = c.parent_id
         LEFT JOIN people own ON own.id = t.owner_person_id
         CROSS JOIN (SELECT id, name, is_self FROM people WHERE is_self = 1 ORDER BY created_at LIMIT 1) self",
    );
    push_where(&mut qb, &f, false);
    qb.push(" AND (a.is_shared = 0 OR t.owner_person_id IS NOT NULL OR t.review_status = 'not_required')");
    qb.push(" GROUP BY 1, 2, 3 ORDER BY total DESC");
    Ok(qb.build_query_as().fetch_all(&db.pool).await?)
}

// --- drill-down ----------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct ChildRow {
    pub id: String,
    pub label: String,
    pub total: f64,
    pub count: i64,
    pub is_leaf: bool,
}

pub async fn children(
    pool: &SqlitePool,
    filter: &SpendingFilter,
    category_id: &str,
) -> AppResult<Vec<ChildRow>> {
    let ps = person_is_self(pool, filter).await?;
    let has_children = category_id != "UNCATEGORIZED"
        && sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE parent_id = ?1)",
        )
        .bind(category_id)
        .fetch_one(pool)
        .await?
            == 1;

    if has_children {
        let mut qb = QueryBuilder::<Sqlite>::new(
            "SELECT c.id AS id, c.label AS label,
                    COALESCE(SUM(t.amount), 0.0) AS total, COUNT(*) AS count, 0 AS is_leaf",
        );
        qb.push(FROM_CLAUSE);
        push_where(&mut qb, filter, ps);
        qb.push(" AND c.parent_id = ").push_bind(category_id.to_string());
        qb.push(" GROUP BY 1, 2 ORDER BY 3 DESC");
        return Ok(qb.build_query_as().fetch_all(pool).await?);
    }

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT COALESCE(NULLIF(t.merchant_name, ''), t.description) AS id,
                COALESCE(NULLIF(t.merchant_name, ''), t.description) AS label,
                COALESCE(SUM(t.amount), 0.0) AS total, COUNT(*) AS count, 1 AS is_leaf",
    );
    qb.push(FROM_CLAUSE);
    push_where(&mut qb, filter, ps);
    if category_id == "UNCATEGORIZED" {
        qb.push(" AND t.category_id IS NULL");
    } else {
        qb.push(" AND (t.category_id = ")
            .push_bind(category_id.to_string())
            .push(" OR c.parent_id = ")
            .push_bind(category_id.to_string())
            .push(")");
    }
    qb.push(" GROUP BY 1 ORDER BY 3 DESC");
    Ok(qb.build_query_as().fetch_all(pool).await?)
}

#[tauri::command]
pub async fn spending_children(
    state: State<'_, AppState>,
    filter: SpendingFilter,
    category_id: String,
) -> AppResult<Vec<ChildRow>> {
    children(&state.db().await?.pool, &filter, &category_id).await
}

// --- transaction list --------------------------------------------------

#[derive(Deserialize, Default)]
pub struct TxnOpts {
    pub category_id: Option<String>,
    pub merchant: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize)]
pub struct TxnPage {
    pub rows: Vec<TxnRow>,
    pub total_count: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TxnRow {
    pub id: String,
    pub posted_date: String,
    pub amount: f64,
    pub currency: String,
    pub description: String,
    pub merchant_name: Option<String>,
    pub category_id: Option<String>,
    pub category_label: Option<String>,
    pub account_name: String,
    pub account_is_shared: bool,
    pub review_status: String,
    pub owner_person_id: Option<String>,
    pub owner_person_name: Option<String>,
    pub pending: bool,
}

fn push_txn_where(qb: &mut QueryBuilder<Sqlite>, f: &SpendingFilter, o: &TxnOpts, person_self: bool) {
    push_where(qb, f, person_self);
    match o.category_id.as_deref() {
        Some("UNCATEGORIZED") => {
            qb.push(" AND t.category_id IS NULL");
        }
        Some(cid) => {
            qb.push(" AND (t.category_id = ")
                .push_bind(cid.to_string())
                .push(" OR c.parent_id = ")
                .push_bind(cid.to_string())
                .push(")");
        }
        None => {}
    }
    if let Some(m) = &o.merchant {
        qb.push(" AND COALESCE(NULLIF(t.merchant_name, ''), t.description) = ")
            .push_bind(m.clone());
    }
    if let Some(s) = o.search.as_ref().filter(|s| !s.trim().is_empty()) {
        let like = format!("%{}%", s.trim());
        qb.push(" AND (t.description LIKE ")
            .push_bind(like.clone())
            .push(" OR t.merchant_name LIKE ")
            .push_bind(like)
            .push(")");
    }
}

pub async fn transactions(
    pool: &SqlitePool,
    filter: &SpendingFilter,
    opts: &TxnOpts,
) -> AppResult<TxnPage> {
    let limit = opts.limit.unwrap_or(50).clamp(1, 500);
    let offset = opts.offset.unwrap_or(0).max(0);
    let ps = person_is_self(pool, filter).await?;

    let mut qb = QueryBuilder::<Sqlite>::new("SELECT COUNT(*)");
    qb.push(FROM_CLAUSE);
    push_txn_where(&mut qb, filter, opts, ps);
    let total_count: i64 = qb.build_query_scalar().fetch_one(pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT t.id, t.posted_date, t.amount, t.currency, t.description, t.merchant_name,
                t.category_id, COALESCE(c.label, p.label) AS category_label,
                a.name AS account_name, a.is_shared AS account_is_shared,
                t.review_status, t.owner_person_id, op.name AS owner_person_name, t.pending",
    );
    qb.push(FROM_CLAUSE);
    qb.push(" LEFT JOIN people op ON op.id = t.owner_person_id ");
    push_txn_where(&mut qb, filter, opts, ps);
    qb.push(" ORDER BY t.posted_date DESC, t.amount DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows: Vec<TxnRow> = qb.build_query_as().fetch_all(pool).await?;

    Ok(TxnPage { rows, total_count })
}

#[tauri::command]
pub async fn list_transactions(
    state: State<'_, AppState>,
    filter: SpendingFilter,
    opts: Option<TxnOpts>,
) -> AppResult<TxnPage> {
    transactions(&state.db().await?.pool, &filter, &opts.unwrap_or_default()).await
}

/// Every transaction for one account (any date, inflows included) with its
/// attribution — for the account detail view.
#[tauri::command]
pub async fn account_transactions(
    state: State<'_, AppState>,
    account_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<TxnPage> {
    let db = state.db().await?;
    let limit = limit.unwrap_or(100).clamp(1, 500);
    let offset = offset.unwrap_or(0).max(0);

    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE account_id = ?1")
            .bind(&account_id)
            .fetch_one(&db.pool)
            .await?;

    let rows: Vec<TxnRow> = sqlx::query_as::<_, TxnRow>(
        "SELECT t.id, t.posted_date, t.amount, t.currency, t.description, t.merchant_name,
                t.category_id, COALESCE(c.label, p.label) AS category_label,
                a.name AS account_name, a.is_shared AS account_is_shared,
                t.review_status, t.owner_person_id, op.name AS owner_person_name, t.pending
         FROM transactions t
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN categories c ON c.id = t.category_id
         LEFT JOIN categories p ON p.id = c.parent_id
         LEFT JOIN people op ON op.id = t.owner_person_id
         WHERE t.account_id = ?1
         ORDER BY t.posted_date DESC, t.amount DESC
         LIMIT ?2 OFFSET ?3",
    )
    .bind(&account_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&db.pool)
    .await?;

    Ok(TxnPage { rows, total_count })
}

// --- categories / recategorize ---------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct CategoryRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub label: String,
}

#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> AppResult<Vec<CategoryRow>> {
    let db = state.db().await?;
    Ok(sqlx::query_as::<_, CategoryRow>(
        "SELECT id, parent_id, label FROM categories
         ORDER BY (parent_id IS NOT NULL), label COLLATE NOCASE",
    )
    .fetch_all(&db.pool)
    .await?)
}

#[tauri::command]
pub async fn set_transaction_category(
    state: State<'_, AppState>,
    txn_id: String,
    category_id: Option<String>,
) -> AppResult<()> {
    let db = state.db().await?;
    if let Some(cid) = &category_id {
        let ok = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE id = ?1)",
        )
        .bind(cid)
        .fetch_one(&db.pool)
        .await?
            == 1;
        if !ok {
            return Err(AppError::Invalid(format!("unknown category: {cid}")));
        }
    }
    let res = sqlx::query(
        "UPDATE transactions SET category_id = ?2, category_source = 'user', updated_at = ?3
         WHERE id = ?1",
    )
    .bind(&txn_id)
    .bind(&category_id)
    .bind(now())
    .execute(&db.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("transaction {txn_id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::util::now;

    async fn seed() -> Db {
        let db = Db::connect_in_memory().await.unwrap();
        let p = &db.pool;
        let ts = now();
        sqlx::query("INSERT INTO items (id, provider, secret_ref, status, created_at, updated_at) VALUES ('it','plaid','x','active',?1,?1)").bind(&ts).execute(p).await.unwrap();
        // acard is a shared card (attribution applies); achk/ahid are not.
        for (id, hidden, shared) in [("acard", 0, 1), ("achk", 0, 0), ("ahid", 1, 0)] {
            sqlx::query("INSERT INTO accounts (id,item_id,provider_account_id,name,type,currency,is_hidden,is_shared,created_at,updated_at) VALUES (?1,'it',?1,?1,'depository','USD',?2,?3,?4,?4)")
                .bind(id).bind(hidden).bind(shared).bind(&ts).execute(p).await.unwrap();
        }
        // detail categories under FOOD_AND_DRINK / GENERAL_MERCHANDISE
        for (id, parent) in [
            ("FOOD_AND_DRINK_RESTAURANT", "FOOD_AND_DRINK"),
            ("FOOD_AND_DRINK_GROCERIES", "FOOD_AND_DRINK"),
            ("GENERAL_MERCHANDISE_ONLINE", "GENERAL_MERCHANDISE"),
        ] {
            sqlx::query("INSERT INTO categories (id,parent_id,label,is_custom) VALUES (?1,?2,?1,0)").bind(id).bind(parent).execute(p).await.unwrap();
        }
        let ins = |tid: &str, acct: &str, amount: f64, cat: Option<&str>, merch: &str, transfer: i64, review: &str| {
            let (tid, acct, merch, review) = (tid.to_string(), acct.to_string(), merch.to_string(), review.to_string());
            let cat = cat.map(|s| s.to_string());
            let ts = ts.clone();
            let p = p.clone();
            async move {
                sqlx::query("INSERT INTO transactions (id,account_id,provider_txn_id,posted_date,amount,currency,description,merchant_name,category_id,category_source,pending,review_status,is_transfer,created_at,updated_at) VALUES (?1,?2,?1,?3,?4,'USD',?5,?5,?6,'provider',0,?7,?8,?9,?9)")
                    .bind(&tid).bind(&acct).bind("2026-08-15").bind(amount).bind(&merch).bind(&cat).bind(&review).bind(transfer).bind(&ts).execute(&p).await.unwrap();
            }
        };
        ins("t1", "acard", 40.0, Some("FOOD_AND_DRINK_RESTAURANT"), "Chipotle", 0, "not_required").await;
        ins("t2", "acard", 60.0, Some("FOOD_AND_DRINK_GROCERIES"), "Whole Foods", 0, "not_required").await;
        ins("t3", "achk", 25.0, Some("GENERAL_MERCHANDISE_ONLINE"), "Amazon", 0, "not_required").await;
        ins("t4", "acard", -15.0, Some("FOOD_AND_DRINK_RESTAURANT"), "Chipotle Refund", 0, "not_required").await; // inflow, excluded
        ins("t5", "acard", 500.0, Some("LOAN_PAYMENTS"), "CC Payment", 0, "not_required").await;   // excluded category
        ins("t6", "acard", 200.0, Some("TRANSFER_OUT"), "Xfer", 1, "not_required").await;          // transfer
        ins("t7", "ahid", 99.0, Some("FOOD_AND_DRINK_RESTAURANT"), "Hidden acct", 0, "not_required").await; // hidden account
        ins("t8", "acard", 33.0, Some("FOOD_AND_DRINK_RESTAURANT"), "Excluded", 0, "excluded").await;       // excluded review
        // date out of range
        sqlx::query("INSERT INTO transactions (id,account_id,provider_txn_id,posted_date,amount,currency,description,category_id,category_source,pending,review_status,is_transfer,created_at,updated_at) VALUES ('t9','acard','t9','2026-01-01',77.0,'USD','Old',NULL,'provider',0,'not_required',0,?1,?1)").bind(&ts).execute(p).await.unwrap();
        db
    }

    fn aug() -> SpendingFilter {
        SpendingFilter {
            from: "2026-08-01".into(),
            to: "2026-08-31".into(),
            account_ids: None,
            person_id: None,
            excluded_only: None,
        }
    }

    #[tokio::test]
    async fn summary_excludes_transfers_payments_hidden_refunds_and_out_of_range() {
        let db = seed().await;
        let s = summary(&db.pool, &aug()).await.unwrap();
        // only t1(40) + t2(60) + t3(25) = 125 across 3 txns
        assert_eq!(s.total, 125.0);
        assert_eq!(s.txn_count, 3);
        assert_eq!(s.by_month.len(), 1);
        assert_eq!(s.by_month[0].month, "2026-08");
        let food = s.by_category.iter().find(|b| b.id == "FOOD_AND_DRINK").unwrap();
        assert_eq!(food.total, 100.0);
        assert_eq!(food.count, 2);
    }

    #[tokio::test]
    async fn children_roll_primary_to_detail_to_merchant() {
        let db = seed().await;
        let subs = children(&db.pool, &aug(), "FOOD_AND_DRINK").await.unwrap();
        assert_eq!(subs.len(), 2);
        assert!(subs.iter().all(|c| !c.is_leaf));
        let restaurants = children(&db.pool, &aug(), "FOOD_AND_DRINK_RESTAURANT").await.unwrap();
        assert_eq!(restaurants.len(), 1);
        assert_eq!(restaurants[0].label, "Chipotle");
        assert!(restaurants[0].is_leaf);
        assert_eq!(restaurants[0].total, 40.0);
    }

    #[tokio::test]
    async fn person_scope_self_includes_unassigned_others_only_owned() {
        let db = seed().await;
        let p = &db.pool;
        let ts = now();
        sqlx::query("INSERT INTO people (id,name,is_self,created_at) VALUES ('me','Me',1,?1),('pp','Partner',0,?1)").bind(&ts).execute(p).await.unwrap();
        // t1 (40, not_required) stays self-owned implicitly; assign t2 (60) to partner
        sqlx::query("UPDATE transactions SET owner_person_id='pp', review_status='assigned' WHERE id='t2'").execute(p).await.unwrap();

        let mut f = aug();
        f.person_id = Some("me".into());
        let me = summary(p, &f).await.unwrap();
        assert_eq!(me.total, 65.0); // t1 40 + t3 25 (not_required), NOT t2

        f.person_id = Some("pp".into());
        let partner = summary(p, &f).await.unwrap();
        assert_eq!(partner.total, 60.0); // only the explicitly assigned t2
    }

    #[tokio::test]
    async fn excluded_only_shows_just_excluded() {
        let db = seed().await;
        // seed() has t8 (33.0) with review_status = 'excluded'
        let mut f = aug();
        f.excluded_only = Some(true);
        let s = summary(&db.pool, &f).await.unwrap();
        assert_eq!(s.txn_count, 1);
        assert_eq!(s.total, 33.0);

        let page = transactions(&db.pool, &f, &TxnOpts::default()).await.unwrap();
        assert_eq!(page.total_count, 1);
        assert_eq!(page.rows[0].review_status, "excluded");
    }

    #[tokio::test]
    async fn trends_pivots_months_x_categories() {
        let db = seed().await;
        let p = &db.pool;
        let ts = now();
        // add a July restaurant charge so we have two months
        sqlx::query("INSERT INTO transactions (id,account_id,provider_txn_id,posted_date,amount,currency,description,category_id,category_source,pending,review_status,is_transfer,created_at,updated_at) VALUES ('j1','achk','j1','2026-07-10',30.0,'USD','Cafe','FOOD_AND_DRINK_RESTAURANT','provider',0,'not_required',0,?1,?1)")
            .bind(&ts).execute(p).await.unwrap();

        let f = SpendingFilter {
            from: "2026-07-01".into(),
            to: "2026-08-31".into(),
            account_ids: None,
            person_id: None,
            excluded_only: None,
        };
        let t = trends(p, &f).await.unwrap();
        assert_eq!(t.months, vec!["2026-07", "2026-08"]);
        let food = t.series.iter().find(|s| s.id == "FOOD_AND_DRINK").unwrap();
        assert_eq!(food.values.len(), 2);
        assert_eq!(food.values[0], 30.0); // July: j1
        assert_eq!(food.values[1], 100.0); // Aug: t1 40 + t2 60
    }

    #[tokio::test]
    async fn transactions_filter_and_paginate() {
        let db = seed().await;
        let page = transactions(
            &db.pool,
            &aug(),
            &TxnOpts { category_id: Some("FOOD_AND_DRINK".into()), ..Default::default() },
        )
        .await
        .unwrap();
        assert_eq!(page.total_count, 2);
        assert_eq!(page.rows.len(), 2);

        let search = transactions(
            &db.pool,
            &aug(),
            &TxnOpts { search: Some("chipotle".into()), ..Default::default() },
        )
        .await
        .unwrap();
        assert_eq!(search.total_count, 1);
    }

    #[tokio::test]
    async fn recategorize_marks_source_user() {
        let db = seed().await;
        let ins = sqlx::query("UPDATE transactions SET category_id='FOOD_AND_DRINK_GROCERIES', category_source='user', updated_at=?2 WHERE id='t1'")
            .bind(0).bind(now()).execute(&db.pool).await.unwrap();
        assert_eq!(ins.rows_affected(), 1);
        let src: String = sqlx::query_scalar("SELECT category_source FROM transactions WHERE id='t1'").fetch_one(&db.pool).await.unwrap();
        assert_eq!(src, "user");
    }
}
