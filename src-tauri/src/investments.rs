//! Persist Plaid `/investments/holdings/get` results into `securities` +
//! `holdings`, and keep investment-account balances / snapshots current.

use std::collections::HashMap;

use serde_json::Value;
use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::util::{new_id, now};

fn f(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|x| x.as_f64())
}
fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(String::from)
}

/// `holdings` is the raw `{ accounts, securities, holdings }` object from Plaid.
pub async fn persist(pool: &SqlitePool, item_id: &str, holdings: &Value) -> AppResult<()> {
    // plaid account_id -> our account uuid
    let account_map: HashMap<String, String> =
        sqlx::query_as::<_, (String, String)>(
            "SELECT provider_account_id, id FROM accounts WHERE item_id = ?1",
        )
        .bind(item_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();

    // plaid security_id -> our security uuid
    let mut security_ids: HashMap<String, String> = HashMap::new();
    if let Some(securities) = holdings["securities"].as_array() {
        for sec in securities {
            let Some(psid) = sec["security_id"].as_str() else { continue };
            let ticker = s(sec, "ticker_symbol").map(|t| t.to_uppercase());
            let name = s(sec, "name");
            let sec_type = s(sec, "type");
            let currency = s(sec, "iso_currency_code").unwrap_or_else(|| "USD".into());

            let existing: Option<String> =
                sqlx::query_scalar("SELECT id FROM securities WHERE provider_security_id = ?1")
                    .bind(psid)
                    .fetch_optional(pool)
                    .await?;
            let id = match existing {
                Some(id) => {
                    sqlx::query("UPDATE securities SET ticker = COALESCE(?2, ticker), name = COALESCE(?3, name), type = COALESCE(?4, type), currency = ?5 WHERE id = ?1")
                        .bind(&id).bind(&ticker).bind(&name).bind(&sec_type).bind(&currency)
                        .execute(pool).await?;
                    id
                }
                None => {
                    let id = new_id();
                    sqlx::query("INSERT INTO securities (id, provider_security_id, ticker, name, type, currency) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
                        .bind(&id).bind(psid).bind(&ticker).bind(&name).bind(&sec_type).bind(&currency)
                        .execute(pool).await?;
                    id
                }
            };
            security_ids.insert(psid.to_string(), id);
        }
    }

    let ts = now();
    let mut touched_accounts: Vec<String> = Vec::new();

    if let Some(hs) = holdings["holdings"].as_array() {
        // group by account so we can prune sold positions
        let mut per_account: HashMap<String, Vec<String>> = HashMap::new();

        for h in hs {
            let (Some(pa), Some(psid)) =
                (h["account_id"].as_str(), h["security_id"].as_str())
            else {
                continue;
            };
            let (Some(account_uuid), Some(security_id)) =
                (account_map.get(pa), security_ids.get(psid))
            else {
                continue;
            };

            let quantity = f(h, "quantity").unwrap_or(0.0);
            let price = f(h, "institution_price");
            let value = f(h, "institution_value").unwrap_or_else(|| price.unwrap_or(0.0) * quantity);
            let cost_basis = f(h, "cost_basis");

            sqlx::query(
                "INSERT INTO holdings (id, account_id, security_id, quantity, cost_basis, price, price_as_of, value, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(account_id, security_id) DO UPDATE SET
                   quantity = excluded.quantity, cost_basis = excluded.cost_basis,
                   price = excluded.price, price_as_of = excluded.price_as_of,
                   value = excluded.value, updated_at = excluded.updated_at",
            )
            .bind(new_id())
            .bind(account_uuid)
            .bind(security_id)
            .bind(quantity)
            .bind(cost_basis)
            .bind(price)
            .bind(s(h, "institution_price_as_of"))
            .bind(value)
            .bind(&ts)
            .execute(pool)
            .await?;

            per_account.entry(account_uuid.clone()).or_default().push(security_id.clone());
        }

        for (account_uuid, keep) in &per_account {
            touched_accounts.push(account_uuid.clone());
            let placeholders = keep.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "DELETE FROM holdings WHERE account_id = ? AND security_id NOT IN ({placeholders})"
            );
            let mut q = sqlx::query(&sql).bind(account_uuid);
            for id in keep {
                q = q.bind(id);
            }
            q.execute(pool).await?;
        }
    }

    // Refresh balances + snapshots for investment accounts from the holdings payload.
    if let Some(accts) = holdings["accounts"].as_array() {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        for a in accts {
            let Some(pa) = a["account_id"].as_str() else { continue };
            let Some(account_uuid) = account_map.get(pa) else { continue };
            let current = f(&a["balances"], "current");
            sqlx::query("UPDATE accounts SET current_balance = COALESCE(?2, current_balance), updated_at = ?3 WHERE id = ?1")
                .bind(account_uuid)
                .bind(current)
                .bind(&ts)
                .execute(pool)
                .await?;
            if let Some(v) = current {
                sqlx::query(
                    "INSERT INTO value_snapshots (id, snapshot_date, scope, account_id, value, created_at)
                     VALUES (?1, ?2, 'account', ?3, ?4, ?5)
                     ON CONFLICT(snapshot_date, scope, account_id) DO UPDATE SET value = excluded.value",
                )
                .bind(format!("{today}:{account_uuid}"))
                .bind(&today)
                .bind(account_uuid)
                .bind(v)
                .bind(&ts)
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(())
}
