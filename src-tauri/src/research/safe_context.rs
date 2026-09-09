//! Privacy guard for the research feature.
//!
//! This module is the ONLY path from portfolio data to any outbound service
//! (the news APIs and the research LLM). It deliberately reduces holdings to
//! `{ ticker, allocation_pct, sector }` — no dollar values, no share counts,
//! no cost basis, no account identifiers. Allocation is rounded to whole
//! percentage points so the payload can't be used to back out balances.

use serde::{Deserialize, Serialize};

/// Raw holding as it exists inside the app. Never serialized outbound.
#[derive(Debug, Clone)]
pub struct RawHolding {
    pub ticker: Option<String>,
    pub sector: Option<String>,
    /// Market value in the account currency.
    pub value: f64,
    /// Unrealized gain/loss as a fraction of cost basis (0.4 = +40%), computed
    /// upstream from cost basis. `None` when no cost basis is known. Only ever
    /// surfaced as a rounded percentage — the dollar figures never leave.
    pub gain_frac: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafeHolding {
    pub ticker: String,
    /// Whole-number percent of the total portfolio (0-100).
    pub allocation_pct: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sector: Option<String>,
    /// Rounded unrealized gain/loss percent vs. cost basis. Only present when the
    /// user has opted in via the "share gain/loss %" setting. Still no dollars.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain_pct: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafePortfolioContext {
    pub holdings: Vec<SafeHolding>,
    /// User-maintained tickers to also research. Plain symbols only.
    pub watchlist: Vec<String>,
}

/// Build the outbound-safe context. `watchlist` is passed straight through
/// after upper-casing / trimming. When `include_gains` is set, each holding also
/// carries a *rounded* gain/loss percent (derived from cost basis; no dollars).
pub fn build_safe_context(
    raw: &[RawHolding],
    watchlist: &[String],
    include_gains: bool,
) -> SafePortfolioContext {
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
            let gain_pct = if include_gains {
                h.gain_frac
                    .filter(|g| g.is_finite())
                    .map(|g| (g * 100.0).round() as i32)
            } else {
                None
            };
            Some(SafeHolding {
                ticker,
                allocation_pct: pct,
                sector: h.sector.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
                gain_pct,
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
        RawHolding { ticker: Some(ticker.into()), sector: None, value, gain_frac: None }
    }

    #[test]
    fn computes_rounded_allocation() {
        let ctx = build_safe_context(&[raw("AAPL", 1500.0), raw("VTI", 8500.0)], &[], false);
        assert_eq!(ctx.holdings[0].ticker, "VTI");
        assert_eq!(ctx.holdings[0].allocation_pct, 85);
        assert_eq!(ctx.holdings[1].allocation_pct, 15);
    }

    #[test]
    fn drops_cash_and_untagged_positions() {
        let ctx = build_safe_context(
            &[
                raw("AAPL", 1000.0),
                RawHolding { ticker: None, sector: None, value: 500.0, gain_frac: None },
                RawHolding { ticker: Some("  ".into()), sector: None, value: 10.0, gain_frac: None },
            ],
            &[],
            false,
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
                RawHolding { ticker: Some("AAPL".into()), sector: Some("Tech".into()), value: 12345.67, gain_frac: Some(1.05) },
                RawHolding { ticker: Some("KO".into()), sector: Some("Consumer".into()), value: 987.65, gain_frac: Some(0.97) },
            ],
            &["nvda".to_string()],
            false,
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
    fn gain_pct_is_opt_in_and_dollar_free() {
        let raw = [
            RawHolding { ticker: Some("AAPL".into()), sector: None, value: 14000.0, gain_frac: Some(0.4) },
            RawHolding { ticker: Some("PYPL".into()), sector: None, value: 880.0, gain_frac: Some(-0.12) },
            RawHolding { ticker: Some("XYZ".into()), sector: None, value: 500.0, gain_frac: None },
        ];
        // off by default
        let off = build_safe_context(&raw, &[], false);
        assert!(off.holdings.iter().all(|h| h.gain_pct.is_none()));

        let on = build_safe_context(&raw, &[], true);
        let aapl = on.holdings.iter().find(|h| h.ticker == "AAPL").unwrap();
        let pypl = on.holdings.iter().find(|h| h.ticker == "PYPL").unwrap();
        let xyz = on.holdings.iter().find(|h| h.ticker == "XYZ").unwrap();
        assert_eq!(aapl.gain_pct, Some(40));
        assert_eq!(pypl.gain_pct, Some(-12));
        assert_eq!(xyz.gain_pct, None); // no cost basis → nothing

        let json = serde_json::to_string(&on).unwrap();
        assert!(json.contains("gain_pct"));
        assert!(!json.contains("14000") && !json.contains("880"));
    }

    #[test]
    fn watchlist_is_normalized() {
        let ctx = build_safe_context(&[], &[" msft ".into(), "aapl".into(), "".into()], false);
        assert_eq!(ctx.watchlist, vec!["MSFT", "AAPL"]);
    }
}
