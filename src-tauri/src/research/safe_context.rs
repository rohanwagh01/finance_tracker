//! Privacy guard for the research feature.
//!
//! This module is the ONLY path from portfolio data to any outbound service
//! (the news APIs and the research LLM). It deliberately reduces holdings to
//! `{ ticker, allocation_pct, sector }` — no dollar values, no share counts,
//! no cost basis, no account identifiers. Allocation is rounded to whole
//! percentage points so the payload can't be used to back out balances.

use serde::Serialize;

/// Raw holding as it exists inside the app. Never serialized outbound.
#[derive(Debug, Clone)]
pub struct RawHolding {
    pub ticker: Option<String>,
    pub sector: Option<String>,
    /// Market value in the account currency.
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SafeHolding {
    pub ticker: String,
    /// Whole-number percent of the total portfolio (0-100).
    pub allocation_pct: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sector: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SafePortfolioContext {
    pub holdings: Vec<SafeHolding>,
    /// User-maintained tickers to also research. Plain symbols only.
    pub watchlist: Vec<String>,
}

/// Build the outbound-safe context. `watchlist` is passed straight through
/// after upper-casing / trimming.
pub fn build_safe_context(raw: &[RawHolding], watchlist: &[String]) -> SafePortfolioContext {
    let total: f64 = raw.iter().map(|h| h.value.max(0.0)).sum();

    let mut holdings: Vec<SafeHolding> = raw
        .iter()
        .filter_map(|h| {
            let ticker = h.ticker.as_ref()?.trim().to_ascii_uppercase();
            if ticker.is_empty() {
                return None;
            }
            let pct = if total > 0.0 {
                ((h.value.max(0.0) / total) * 100.0).round().clamp(0.0, 100.0) as u8
            } else {
                0
            };
            Some(SafeHolding {
                ticker,
                allocation_pct: pct,
                sector: h.sector.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            })
        })
        .collect();

    holdings.sort_by(|a, b| b.allocation_pct.cmp(&a.allocation_pct).then(a.ticker.cmp(&b.ticker)));

    let watchlist = watchlist
        .iter()
        .map(|t| t.trim().to_ascii_uppercase())
        .filter(|t| !t.is_empty())
        .collect();

    SafePortfolioContext { holdings, watchlist }
}

/// The full list of tickers referenced by a context (holdings + watchlist),
/// deduplicated. Used to fan out news lookups.
pub fn tickers(ctx: &SafePortfolioContext) -> Vec<String> {
    let mut out: Vec<String> = ctx
        .holdings
        .iter()
        .map(|h| h.ticker.clone())
        .chain(ctx.watchlist.iter().cloned())
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(ticker: &str, value: f64) -> RawHolding {
        RawHolding { ticker: Some(ticker.into()), sector: None, value }
    }

    #[test]
    fn computes_rounded_allocation() {
        let ctx = build_safe_context(&[raw("AAPL", 1500.0), raw("VTI", 8500.0)], &[]);
        assert_eq!(ctx.holdings[0].ticker, "VTI");
        assert_eq!(ctx.holdings[0].allocation_pct, 85);
        assert_eq!(ctx.holdings[1].allocation_pct, 15);
    }

    #[test]
    fn drops_cash_and_untagged_positions() {
        let ctx = build_safe_context(
            &[
                raw("AAPL", 1000.0),
                RawHolding { ticker: None, sector: None, value: 500.0 },
                RawHolding { ticker: Some("  ".into()), sector: None, value: 10.0 },
            ],
            &[],
        );
        assert_eq!(ctx.holdings.len(), 1);
        assert_eq!(ctx.holdings[0].ticker, "AAPL");
    }

    /// The core privacy assertion: the serialized payload must not carry any
    /// value / quantity / balance field, only ticker, allocation_pct, sector.
    #[test]
    fn serialized_payload_leaks_no_amounts() {
        let ctx = build_safe_context(
            &[
                RawHolding { ticker: Some("AAPL".into()), sector: Some("Tech".into()), value: 12345.67 },
                RawHolding { ticker: Some("KO".into()), sector: Some("Consumer".into()), value: 987.65 },
            ],
            &["nvda".to_string()],
        );
        let json = serde_json::to_string(&ctx).unwrap();

        for forbidden in ["value", "quantity", "cost", "price", "balance", "shares", "amount"] {
            assert!(!json.contains(forbidden), "payload leaked field {forbidden:?}: {json}");
        }
        // No raw dollar figures.
        assert!(!json.contains("12345"));
        assert!(!json.contains("987"));
        assert!(json.contains("AAPL") && json.contains("NVDA"));
    }

    #[test]
    fn watchlist_is_normalized() {
        let ctx = build_safe_context(&[], &[" msft ".into(), "aapl".into(), "".into()]);
        assert_eq!(ctx.watchlist, vec!["MSFT", "AAPL"]);
    }
}
