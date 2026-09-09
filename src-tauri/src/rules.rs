//! Attribution / categorization rules. Rules only *suggest* a person (and
//! optionally a category); a suggestion becomes the confirmed owner only when
//! the user clears it in the review inbox — unless it's a `high_confidence`
//! rule and the user has enabled auto-confirm.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::util::now;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Rule {
    pub id: String,
    pub priority: i64,
    pub enabled: bool,
    pub match_field: String, // description | merchant_name | amount
    pub match_op: String,    // contains | equals | regex | gt | lt | between
    pub match_value: String,
    pub match_value2: Option<String>,
    pub account_id: Option<String>,
    pub set_person_id: Option<String>,
    pub set_category_id: Option<String>,
    pub high_confidence: bool,
}

pub struct TxnFacts<'a> {
    pub description: &'a str,
    pub merchant_name: Option<&'a str>,
    pub amount: f64,
    pub account_id: &'a str,
}

pub fn rule_matches(r: &Rule, t: &TxnFacts) -> bool {
    if let Some(scope) = &r.account_id {
        if scope != t.account_id {
            return false;
        }
    }
    match r.match_field.as_str() {
        "amount" => {
            let Ok(v) = r.match_value.trim().parse::<f64>() else {
                return false;
            };
            match r.match_op.as_str() {
                "gt" => t.amount > v,
                "lt" => t.amount < v,
                "equals" => (t.amount - v).abs() < 0.005,
                "between" => {
                    let Some(v2) = r.match_value2.as_ref().and_then(|s| s.trim().parse::<f64>().ok())
                    else {
                        return false;
                    };
                    t.amount >= v.min(v2) && t.amount <= v.max(v2)
                }
                _ => false,
            }
        }
        field => {
            let hay = if field == "merchant_name" {
                t.merchant_name.unwrap_or("")
            } else {
                t.description
            };
            match r.match_op.as_str() {
                "contains" => hay.to_lowercase().contains(&r.match_value.to_lowercase()),
                "equals" => hay.eq_ignore_ascii_case(r.match_value.trim()),
                "regex" => regex::Regex::new(&r.match_value)
                    .map(|re| re.is_match(hay))
                    .unwrap_or(false),
                _ => false,
            }
        }
    }
}

async fn enabled_rules(pool: &SqlitePool) -> AppResult<Vec<Rule>> {
    Ok(sqlx::query_as::<_, Rule>(
        "SELECT id, priority, enabled, match_field, match_op, match_value, match_value2,
                account_id, set_person_id, set_category_id, high_confidence
         FROM rules WHERE enabled = 1
         ORDER BY priority ASC, created_at ASC",
    )
    .fetch_all(pool)
    .await?)
}

/// Re-evaluate rules against every `pending` transaction: write the suggested
/// person / rule, apply a rule-set category (only over a provider category),
/// and — when `auto_confirm` is on — confirm high-confidence person matches.
/// Returns the number of transactions auto-confirmed.
pub async fn apply_to_pending(pool: &SqlitePool, auto_confirm: bool) -> AppResult<usize> {
    let rules = enabled_rules(pool).await?;

    let pending: Vec<(String, String, Option<String>, f64, String)> = sqlx::query_as(
        "SELECT id, description, merchant_name, amount, account_id
         FROM transactions WHERE review_status = 'pending'",
    )
    .fetch_all(pool)
    .await?;

    let mut confirmed = 0usize;
    for (id, description, merchant_name, amount, account_id) in pending {
        let facts = TxnFacts {
            description: &description,
            merchant_name: merchant_name.as_deref(),
            amount,
            account_id: &account_id,
        };
        let hit = rules.iter().find(|r| rule_matches(r, &facts));

        match hit {
            Some(r) => {
                if let Some(cat) = &r.set_category_id {
                    sqlx::query(
                        "UPDATE transactions SET category_id = ?2, category_source = 'rule', updated_at = ?3
                         WHERE id = ?1 AND category_source = 'provider'",
                    )
                    .bind(&id)
                    .bind(cat)
                    .bind(now())
                    .execute(pool)
                    .await?;
                }

                if auto_confirm && r.high_confidence && r.set_person_id.is_some() {
                    sqlx::query(
                        "UPDATE transactions SET owner_person_id = ?2, suggested_person_id = ?2,
                             suggestion_rule_id = ?3, review_status = 'assigned', updated_at = ?4
                         WHERE id = ?1 AND review_status = 'pending'",
                    )
                    .bind(&id)
                    .bind(&r.set_person_id)
                    .bind(&r.id)
                    .bind(now())
                    .execute(pool)
                    .await?;
                    confirmed += 1;
                } else {
                    sqlx::query(
                        "UPDATE transactions SET suggested_person_id = ?2, suggestion_rule_id = ?3, updated_at = ?4
                         WHERE id = ?1 AND review_status = 'pending'",
                    )
                    .bind(&id)
                    .bind(&r.set_person_id)
                    .bind(&r.id)
                    .bind(now())
                    .execute(pool)
                    .await?;
                }
            }
            None => {
                sqlx::query(
                    "UPDATE transactions SET suggested_person_id = NULL, suggestion_rule_id = NULL
                     WHERE id = ?1 AND review_status = 'pending'",
                )
                .bind(&id)
                .execute(pool)
                .await?;
            }
        }
    }
    Ok(confirmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(field: &str, op: &str, val: &str) -> Rule {
        Rule {
            id: "r".into(),
            priority: 100,
            enabled: true,
            match_field: field.into(),
            match_op: op.into(),
            match_value: val.into(),
            match_value2: None,
            account_id: None,
            set_person_id: Some("p".into()),
            set_category_id: None,
            high_confidence: false,
        }
    }

    fn facts<'a>(desc: &'a str, merch: Option<&'a str>, amount: f64) -> TxnFacts<'a> {
        TxnFacts { description: desc, merchant_name: merch, amount, account_id: "a" }
    }

    #[test]
    fn text_ops() {
        assert!(rule_matches(&rule("description", "contains", "uber"), &facts("UBER EATS", None, 20.0)));
        assert!(!rule_matches(&rule("description", "contains", "lyft"), &facts("UBER EATS", None, 20.0)));
        assert!(rule_matches(&rule("merchant_name", "equals", "Costco"), &facts("x", Some("costco"), 20.0)));
        assert!(rule_matches(&rule("description", "regex", r"^AMZN\s"), &facts("AMZN Mktp US", None, 5.0)));
    }

    #[test]
    fn amount_ops() {
        assert!(rule_matches(&rule("amount", "gt", "100"), &facts("x", None, 150.0)));
        assert!(rule_matches(&rule("amount", "lt", "10"), &facts("x", None, 4.0)));
        let mut r = rule("amount", "between", "10");
        r.match_value2 = Some("20".into());
        assert!(rule_matches(&r, &facts("x", None, 15.0)));
        assert!(!rule_matches(&r, &facts("x", None, 25.0)));
    }

    #[test]
    fn account_scope() {
        let mut r = rule("description", "contains", "x");
        r.account_id = Some("other".into());
        assert!(!rule_matches(&r, &facts("xyz", None, 1.0)));
        r.account_id = Some("a".into());
        assert!(rule_matches(&r, &facts("xyz", None, 1.0)));
    }

    #[tokio::test]
    async fn apply_suggests_and_auto_confirms() {
        let db = crate::db::Db::connect_in_memory().await.unwrap();
        let p = &db.pool;
        let ts = now();
        sqlx::query("INSERT INTO items (id,provider,secret_ref,status,created_at,updated_at) VALUES ('i','plaid','x','active',?1,?1)").bind(&ts).execute(p).await.unwrap();
        sqlx::query("INSERT INTO accounts (id,item_id,provider_account_id,name,type,currency,is_shared,created_at,updated_at) VALUES ('a','i','a','Card','credit','USD',1,?1,?1)").bind(&ts).execute(p).await.unwrap();
        sqlx::query("INSERT INTO people (id,name,is_self,created_at) VALUES ('me','Me',1,?1),('partner','Partner',0,?1)").bind(&ts).execute(p).await.unwrap();
        sqlx::query("INSERT INTO rules (id,priority,enabled,match_field,match_op,match_value,set_person_id,high_confidence,created_at) VALUES ('r1',10,1,'description','contains','spotify','partner',1,?1)").bind(&ts).execute(p).await.unwrap();
        for (id, desc) in [("t1", "SPOTIFY P1"), ("t2", "SPOTIFY P2"), ("t3", "RANDOM SHOP")] {
            sqlx::query("INSERT INTO transactions (id,account_id,provider_txn_id,posted_date,amount,currency,description,category_source,pending,review_status,is_transfer,created_at,updated_at) VALUES (?1,'a',?1,'2026-08-01',10.0,'USD',?2,'provider',0,'pending',0,?3,?3)").bind(id).bind(desc).bind(&ts).execute(p).await.unwrap();
        }

        // no auto-confirm: only suggestions
        let n = apply_to_pending(p, false).await.unwrap();
        assert_eq!(n, 0);
        let suggested: Option<String> = sqlx::query_scalar("SELECT suggested_person_id FROM transactions WHERE id='t1'").fetch_one(p).await.unwrap();
        assert_eq!(suggested.as_deref(), Some("partner"));
        let status: String = sqlx::query_scalar("SELECT review_status FROM transactions WHERE id='t1'").fetch_one(p).await.unwrap();
        assert_eq!(status, "pending");

        // auto-confirm on: high-confidence matches become 'assigned'
        let n = apply_to_pending(p, true).await.unwrap();
        assert_eq!(n, 2);
        let status: String = sqlx::query_scalar("SELECT review_status FROM transactions WHERE id='t1'").fetch_one(p).await.unwrap();
        assert_eq!(status, "assigned");
        let still_pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE review_status='pending'").fetch_one(p).await.unwrap();
        assert_eq!(still_pending, 1); // t3
    }
}
