//! The portfolio view, built from holdings synced through Plaid Investments.

use std::collections::BTreeMap;

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppResult;
use crate::state::AppState;

// --- portfolio ---------------------------------------------------------

#[derive(sqlx::FromRow)]
struct HoldingRow {
    account_id: String,
    ticker: Option<String>,
    sec_name: Option<String>,
    sec_type: Option<String>,
    quantity: f64,
    price: Option<f64>,
    value: Option<f64>,
    cost_basis: Option<f64>,
}

impl HoldingRow {
    fn is_cash(&self) -> bool {
        self.sec_type.as_deref() == Some("cash")
            || self.ticker.as_deref().map(|t| t.starts_with("CUR:")).unwrap_or(false)
    }
    fn value(&self) -> f64 {
        self.value.unwrap_or_else(|| self.price.unwrap_or(0.0) * self.quantity)
    }
}

#[derive(Serialize, Clone)]
pub struct Position {
    pub ticker: Option<String>,
    pub name: Option<String>,
    pub sec_type: Option<String>,
    pub quantity: f64,
    pub price: Option<f64>,
    pub value: f64,
    pub cost_basis: Option<f64>,
    pub gain: Option<f64>,
    pub gain_pct: Option<f64>,
    pub allocation_pct: f64,
}

#[derive(Serialize)]
pub struct PortfolioAccount {
    pub id: String,
    pub name: String,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    pub value: f64,
    pub cash: f64,
    pub positions: Vec<Position>,
}

#[derive(Serialize)]
pub struct Portfolio {
    pub total_value: f64,
    pub total_cost_basis: Option<f64>,
    pub total_cash: f64,
    pub total_gain: Option<f64>,
    pub accounts: Vec<PortfolioAccount>,
    pub combined: Vec<Position>,
    pub last_synced_at: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct HistoryPoint {
    pub date: String,
    pub value: f64,
}

fn finish(mut p: Position, total: f64) -> Position {
    p.gain = match (p.cost_basis, p.value) {
        (Some(cb), v) if cb != 0.0 => Some(v - cb),
        _ => None,
    };
    p.gain_pct = match (p.cost_basis, p.gain) {
        (Some(cb), Some(g)) if cb != 0.0 => Some(g / cb * 100.0),
        _ => None,
    };
    p.allocation_pct = if total > 0.0 { p.value / total * 100.0 } else { 0.0 };
    p
}

#[tauri::command]
pub async fn portfolio(state: State<'_, AppState>) -> AppResult<Portfolio> {
    let db = state.db().await?;

    let rows: Vec<HoldingRow> = sqlx::query_as(
        "SELECT h.account_id, s.ticker, s.name AS sec_name, s.type AS sec_type,
                h.quantity, h.price, h.value, h.cost_basis
         FROM holdings h
         JOIN accounts a ON a.id = h.account_id
         JOIN securities s ON s.id = h.security_id
         WHERE a.type = 'investment' AND a.is_hidden = 0
         ORDER BY h.value DESC",
    )
    .fetch_all(&db.pool)
    .await?;

    // account meta + the broker's own total for the account (authoritative).
    let accts: Vec<(String, String, Option<String>, Option<String>, Option<f64>)> = sqlx::query_as(
        "SELECT id, name, official_name, mask, current_balance
         FROM accounts WHERE type = 'investment' AND is_hidden = 0",
    )
    .fetch_all(&db.pool)
    .await?;

    // The portfolio total is just the sum of each account's balance — cash is
    // already inside that number (and shown as a CUR:USD holding), so it is
    // never added again.
    let total_value: f64 = accts.iter().filter_map(|a| a.4).sum();

    let mut cost_sum = 0.0;
    let mut have_cost = false;
    for r in &rows {
        if let Some(c) = r.cost_basis {
            cost_sum += c;
            have_cost = true;
        }
    }
    let total_cost_basis = if have_cost { Some(cost_sum) } else { None };
    // gain over just the holdings that report a cost basis
    let total_gain = total_cost_basis.map(|_| {
        rows.iter()
            .filter_map(|r| r.cost_basis.map(|cb| r.value() - cb))
            .sum()
    });
    let total_cash: f64 = rows.iter().filter(|r| r.is_cash()).map(|r| r.value()).sum();

    let mut per_account: BTreeMap<String, Vec<Position>> = BTreeMap::new();
    for r in &rows {
        per_account.entry(r.account_id.clone()).or_default().push(finish(
            Position {
                ticker: r.ticker.clone(),
                name: r.sec_name.clone(),
                sec_type: r.sec_type.clone(),
                quantity: r.quantity,
                price: r.price,
                value: r.value(),
                cost_basis: r.cost_basis,
                gain: None,
                gain_pct: None,
                allocation_pct: 0.0,
            },
            total_value,
        ));
    }
    let account_cash: BTreeMap<String, f64> = rows.iter().filter(|r| r.is_cash()).fold(
        BTreeMap::new(),
        |mut m, r| {
            *m.entry(r.account_id.clone()).or_default() += r.value();
            m
        },
    );

    let mut accounts: Vec<PortfolioAccount> = accts
        .into_iter()
        .map(|(id, name, official, mask, balance)| {
            let positions = per_account.remove(&id).unwrap_or_default();
            let value = balance.unwrap_or_else(|| positions.iter().map(|p| p.value).sum());
            PortfolioAccount {
                cash: account_cash.get(&id).copied().unwrap_or(0.0),
                id,
                name,
                official_name: official,
                mask,
                value,
                positions,
            }
        })
        .collect();
    accounts.sort_by(|a, b| b.value.total_cmp(&a.value));

    // combined by ticker (fallback to security name)
    let mut combined: BTreeMap<String, Position> = BTreeMap::new();
    for r in &rows {
        let key = r
            .ticker
            .clone()
            .or_else(|| r.sec_name.clone())
            .unwrap_or_else(|| "—".into());
        let e = combined.entry(key).or_insert(Position {
            ticker: r.ticker.clone(),
            name: r.sec_name.clone(),
            sec_type: r.sec_type.clone(),
            quantity: 0.0,
            price: r.price,
            value: 0.0,
            cost_basis: None,
            gain: None,
            gain_pct: None,
            allocation_pct: 0.0,
        });
        e.quantity += r.quantity;
        e.value += r.value();
        if let Some(cb) = r.cost_basis {
            e.cost_basis = Some(e.cost_basis.unwrap_or(0.0) + cb);
        }
    }
    let mut combined: Vec<Position> = combined
        .into_values()
        .map(|p| finish(p, total_value))
        .collect();
    combined.sort_by(|a, b| b.value.total_cmp(&a.value));

    let last_synced_at: Option<String> = sqlx::query_scalar::<_, Option<String>>(
        "SELECT MAX(last_synced_at) FROM items i
         WHERE EXISTS (SELECT 1 FROM accounts a WHERE a.item_id = i.id AND a.type = 'investment')",
    )
    .fetch_optional(&db.pool)
    .await?
    .flatten();

    Ok(Portfolio {
        total_value,
        total_cost_basis,
        total_cash,
        total_gain,
        accounts,
        combined,
        last_synced_at,
    })
}

/// Portfolio value over time, from the daily `value_snapshots` written on each
/// sync. Plaid has no historical prices, so this only covers the period since
/// you started syncing — it can't be backfilled.
pub async fn history(pool: &SqlitePool, from: &str, to: &str) -> AppResult<Vec<HistoryPoint>> {
    Ok(sqlx::query_as::<_, HistoryPoint>(
        "SELECT vs.snapshot_date AS date, SUM(vs.value) AS value
         FROM value_snapshots vs
         JOIN accounts a ON a.id = vs.account_id
         WHERE vs.scope = 'account' AND a.type = 'investment' AND a.is_hidden = 0
           AND vs.snapshot_date >= ?1 AND vs.snapshot_date <= ?2
         GROUP BY 1 ORDER BY 1",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?)
}

#[tauri::command]
pub async fn portfolio_history(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> AppResult<Vec<HistoryPoint>> {
    history(&state.db().await?.pool, &from, &to).await
}

/// Value over time for one account, carrying the last known snapshot forward
/// over days it wasn't synced.
#[tauri::command]
pub async fn account_value_history(
    state: State<'_, AppState>,
    account_id: String,
    from: String,
    to: String,
) -> AppResult<Vec<HistoryPoint>> {
    let db = state.db().await?;
    Ok(sqlx::query_as::<_, HistoryPoint>(
        "WITH days AS (
             SELECT DISTINCT snapshot_date AS d FROM value_snapshots
             WHERE scope = 'account' AND account_id = ?1
               AND snapshot_date >= ?2 AND snapshot_date <= ?3
         )
         SELECT days.d AS date,
                (SELECT vs.value FROM value_snapshots vs
                 WHERE vs.account_id = ?1 AND vs.scope = 'account'
                   AND vs.snapshot_date <= days.d
                 ORDER BY vs.snapshot_date DESC LIMIT 1) AS value
         FROM days ORDER BY days.d",
    )
    .bind(&account_id)
    .bind(&from)
    .bind(&to)
    .fetch_all(&db.pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::util::now;

    #[tokio::test]
    async fn history_sums_investment_snapshots_per_day() {
        let db = Db::connect_in_memory().await.unwrap();
        let p = &db.pool;
        let ts = now();
        sqlx::query("INSERT INTO items (id,provider,secret_ref,status,created_at,updated_at) VALUES ('i','plaid','x','active',?1,?1)").bind(&ts).execute(p).await.unwrap();
        for (id, t, hidden) in [("inv1", "investment", 0), ("inv2", "investment", 0), ("bank", "depository", 0), ("hid", "investment", 1)] {
            sqlx::query("INSERT INTO accounts (id,item_id,provider_account_id,name,type,currency,is_hidden,created_at,updated_at) VALUES (?1,'i',?1,?1,?2,'USD',?3,?4,?4)")
                .bind(id).bind(t).bind(hidden).bind(&ts).execute(p).await.unwrap();
        }
        let snap = |sid: &str, date: &str, acct: &str, v: f64| {
            let (sid, date, acct, ts) = (sid.to_string(), date.to_string(), acct.to_string(), ts.clone());
            let p = p.clone();
            async move {
                sqlx::query("INSERT INTO value_snapshots (id,snapshot_date,scope,account_id,value,created_at) VALUES (?1,?2,'account',?3,?4,?5)")
                    .bind(&sid).bind(&date).bind(&acct).bind(v).bind(&ts).execute(&p).await.unwrap();
            }
        };
        snap("a", "2026-09-01", "inv1", 100.0).await;
        snap("b", "2026-09-01", "inv2", 50.0).await;
        snap("c", "2026-09-01", "bank", 999.0).await; // not investment → ignored
        snap("d", "2026-09-01", "hid", 777.0).await;  // hidden → ignored
        snap("e", "2026-09-02", "inv1", 120.0).await;

        let h = history(p, "2026-09-01", "2026-09-30").await.unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].date, "2026-09-01");
        assert_eq!(h[0].value, 150.0);
        assert_eq!(h[1].value, 120.0);
    }
}
