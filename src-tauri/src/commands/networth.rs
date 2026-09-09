//! Net worth over time.
//!
//! Cash and investments come straight from account balances. Debt:
//! * a **non-shared** credit card / loan contributes its full balance;
//! * a **shared** credit card / loan contributes only the sum of the
//!   transactions on it that are attributed to the primary person — so a
//!   partner's charges, excluded charges, and un-reviewed charges don't count.
//!
//! The history chart mirrors this: non-shared accounts use their daily
//! `value_snapshots`; shared debt uses a running sum of your attributed
//! charges by transaction date.

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::commands::investments::HistoryPoint;
use crate::error::AppResult;
use crate::state::AppState;

const SELF_ID: &str = "(SELECT id FROM people WHERE is_self = 1 ORDER BY created_at LIMIT 1)";

#[derive(Serialize)]
pub struct NetWorthNow {
    pub cash: f64,
    pub investments: f64,
    pub debt: f64,
    pub net: f64,
}

pub async fn history(pool: &SqlitePool, from: &str, to: &str) -> AppResult<Vec<HistoryPoint>> {
    let sql = format!(
        "WITH days AS (
             SELECT DISTINCT snapshot_date AS d FROM value_snapshots
             WHERE scope = 'account' AND snapshot_date >= ?1 AND snapshot_date <= ?2
         )
         SELECT days.d AS date,
                -- everything except shared debt: latest snapshot on-or-before the day
                COALESCE((
                    SELECT SUM(
                        (SELECT vs.value FROM value_snapshots vs
                         WHERE vs.account_id = a.id AND vs.scope = 'account'
                           AND vs.snapshot_date <= days.d
                         ORDER BY vs.snapshot_date DESC LIMIT 1)
                    )
                    FROM accounts a
                    WHERE a.is_hidden = 0
                      AND NOT (a.type IN ('credit','loan') AND a.is_shared = 1)
                ), 0.0)
                -- shared debt: running sum of my attributed charges up to the day
                - COALESCE((
                    SELECT SUM(t.amount) FROM transactions t
                    JOIN accounts a ON a.id = t.account_id
                    WHERE a.is_hidden = 0 AND a.type IN ('credit','loan') AND a.is_shared = 1
                      AND t.owner_person_id = {SELF_ID}
                      AND t.posted_date <= days.d
                ), 0.0) AS value
         FROM days ORDER BY days.d"
    );
    Ok(sqlx::query_as::<_, HistoryPoint>(&sql)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await?)
}

#[tauri::command]
pub async fn net_worth_history(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> AppResult<Vec<HistoryPoint>> {
    history(&state.db().await?.pool, &from, &to).await
}

async fn now(pool: &SqlitePool) -> AppResult<NetWorthNow> {
    let sql = format!(
        "SELECT
           COALESCE(SUM(CASE WHEN a.type = 'depository' THEN a.current_balance END), 0.0),
           COALESCE(SUM(CASE WHEN a.type = 'investment' THEN a.current_balance END), 0.0),
           COALESCE(SUM(CASE
             WHEN a.type IN ('credit','loan') AND a.is_shared = 1 THEN
               COALESCE((SELECT SUM(t.amount) FROM transactions t
                         WHERE t.account_id = a.id AND t.owner_person_id = {SELF_ID}), 0.0)
             WHEN a.type IN ('credit','loan') THEN a.current_balance
           END), 0.0)
         FROM accounts a WHERE a.is_hidden = 0"
    );
    let (cash, investments, debt): (f64, f64, f64) =
        sqlx::query_as(&sql).fetch_one(pool).await?;
    Ok(NetWorthNow { cash, investments, debt, net: cash + investments - debt })
}

/// Current net worth split into cash / investments / debt.
#[tauri::command]
pub async fn net_worth_now(state: State<'_, AppState>) -> AppResult<NetWorthNow> {
    now(&state.db().await?.pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::util::now as ts_now;

    #[tokio::test]
    async fn net_worth_carries_forward_missing_days() {
        let db = Db::connect_in_memory().await.unwrap();
        let p = &db.pool;
        let ts = ts_now();
        sqlx::query("INSERT INTO items (id,provider,secret_ref,status,created_at,updated_at) VALUES ('i','plaid','x','active',?1,?1)").bind(&ts).execute(p).await.unwrap();
        for id in ["chk", "card"] {
            sqlx::query("INSERT INTO accounts (id,item_id,provider_account_id,name,type,currency,is_hidden,created_at,updated_at) VALUES (?1,'i',?1,?1,'depository','USD',0,?2,?2)")
                .bind(id).bind(&ts).execute(p).await.unwrap();
        }
        let snap = |sid: &str, d: &str, acct: &str, v: f64| {
            let (sid, d, acct, ts) = (sid.to_string(), d.to_string(), acct.to_string(), ts.clone());
            let p = p.clone();
            async move {
                sqlx::query("INSERT INTO value_snapshots (id,snapshot_date,scope,account_id,value,created_at) VALUES (?1,?2,'account',?3,?4,?5)")
                    .bind(&sid).bind(&d).bind(&acct).bind(v).bind(&ts).execute(&p).await.unwrap();
            }
        };
        snap("a", "2026-09-01", "chk", 1000.0).await;
        snap("b", "2026-09-01", "card", -200.0).await;
        snap("c", "2026-09-02", "chk", 1100.0).await;
        snap("d", "2026-09-03", "card", -150.0).await;

        let h = history(p, "2026-09-01", "2026-09-30").await.unwrap();
        assert_eq!(h.len(), 3);
        assert_eq!(h[0].value, 800.0);
        assert_eq!(h[1].value, 900.0);
        assert_eq!(h[2].value, 950.0);
    }

    #[tokio::test]
    async fn shared_debt_is_just_my_attributed_charges() {
        let db = Db::connect_in_memory().await.unwrap();
        let p = &db.pool;
        let ts = ts_now();
        sqlx::query("INSERT INTO items (id,provider,secret_ref,status,created_at,updated_at) VALUES ('i','plaid','x','active',?1,?1)").bind(&ts).execute(p).await.unwrap();
        sqlx::query("INSERT INTO people (id,name,is_self,created_at) VALUES ('me','Me',1,?1),('pp','P',0,?1)").bind(&ts).execute(p).await.unwrap();
        // shared card with a big carried balance ($1000) — should be ignored
        sqlx::query("INSERT INTO accounts (id,item_id,provider_account_id,name,type,currency,is_hidden,is_shared,current_balance,created_at,updated_at) VALUES ('card','i','card','Card','credit','USD',0,1,1000.0,?1,?1)").bind(&ts).execute(p).await.unwrap();
        sqlx::query("INSERT INTO accounts (id,item_id,provider_account_id,name,type,currency,is_hidden,is_shared,current_balance,created_at,updated_at) VALUES ('chk','i','chk','Chk','depository','USD',0,0,500.0,?1,?1)").bind(&ts).execute(p).await.unwrap();
        let tx = |id: &str, amt: f64, owner: Option<&str>, st: &str, date: &str| {
            let (id, owner, st, date, ts) = (id.to_string(), owner.map(String::from), st.to_string(), date.to_string(), ts.clone());
            let p = p.clone();
            async move {
                sqlx::query("INSERT INTO transactions (id,account_id,provider_txn_id,posted_date,amount,currency,description,category_source,pending,review_status,owner_person_id,is_transfer,created_at,updated_at) VALUES (?1,'card',?1,?2,?3,'USD','x','provider',0,?4,?5,0,?6,?6)")
                    .bind(&id).bind(&date).bind(amt).bind(&st).bind(&owner).bind(&ts).execute(&p).await.unwrap();
            }
        };
        tx("t1", 75.0, Some("me"), "kept", "2026-08-01").await;
        tx("t2", 25.0, Some("pp"), "assigned", "2026-08-02").await;
        tx("t3", 10.0, None, "excluded", "2026-08-03").await;
        tx("t4", 40.0, None, "pending", "2026-08-04").await;
        tx("t5", 30.0, Some("me"), "kept", "2026-08-20").await;
        tx("t6", -20.0, Some("me"), "kept", "2026-08-25").await; // a refund of mine

        let nw = now(p).await.unwrap();
        // 75 + 30 - 20 = 85 (only my attributed charges; carried $1000 ignored)
        assert_eq!(nw.debt, 85.0);
        assert_eq!(nw.cash, 500.0);
        assert_eq!(nw.net, 500.0 - 85.0);

        // history: running sum of my charges by date, negated
        for (id, d, acct, v) in [("s1", "2026-08-05", "card", -1000.0), ("s2", "2026-08-05", "chk", 500.0)] {
            sqlx::query("INSERT INTO value_snapshots (id,snapshot_date,scope,account_id,value,created_at) VALUES (?1,?2,'account',?3,?4,?5)")
                .bind(id).bind(d).bind(acct).bind(v).bind(&ts).execute(p).await.unwrap();
        }
        let h = history(p, "2026-08-01", "2026-09-01").await.unwrap();
        assert_eq!(h.len(), 1);
        // chk 500 (snapshot) - my charges up to 2026-08-05 (just t1 = 75) = 425
        assert_eq!(h[0].value, 425.0);
    }
}
