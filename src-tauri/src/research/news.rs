//! Market news for the tickers in the safe context. Two providers, both
//! read-only and both fed nothing but ticker symbols + a date range.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppResult;
use crate::http::HttpClient;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewsItem {
    pub ticker: String,
    pub headline: String,
    pub summary: String,
    pub source: String,
    pub url: String,
    /// ISO date (`YYYY-MM-DD`) — day precision is all the UI needs.
    pub published_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NewsProvider {
    Finnhub,
    Marketaux,
    None,
}

impl NewsProvider {
    pub fn parse(s: &str) -> Self {
        match s {
            "finnhub" => Self::Finnhub,
            "marketaux" => Self::Marketaux,
            _ => Self::None,
        }
    }
}

fn unix_to_date(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn iso_day(s: &str) -> String {
    // Marketaux returns e.g. "2026-09-01T13:00:00.000000Z"
    s.split(['T', ' ']).next().unwrap_or(s).to_string()
}

/// Fetch recent company news for one ticker from Finnhub.
pub async fn finnhub_company_news(
    http: &HttpClient,
    api_key: &str,
    ticker: &str,
    from: &str,
    to: &str,
    limit: usize,
) -> AppResult<Vec<NewsItem>> {
    let url = format!(
        "https://finnhub.io/api/v1/company-news?symbol={ticker}&from={from}&to={to}&token={api_key}"
    );
    let resp = http.get(&url)?.send().await?;
    if !resp.status().is_success() {
        return Ok(vec![]);
    }
    let arr: Value = resp.json().await?;
    let mut out = Vec::new();
    for it in arr.as_array().into_iter().flatten() {
        let headline = it.get("headline").and_then(|v| v.as_str()).unwrap_or("").trim();
        let url = it.get("url").and_then(|v| v.as_str()).unwrap_or("").trim();
        if headline.is_empty() || url.is_empty() {
            continue;
        }
        out.push(NewsItem {
            ticker: ticker.to_string(),
            headline: headline.to_string(),
            summary: it.get("summary").and_then(|v| v.as_str()).unwrap_or("").trim().to_string(),
            source: it.get("source").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            url: url.to_string(),
            published_at: unix_to_date(it.get("datetime").and_then(|v| v.as_i64()).unwrap_or(0)),
        });
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

/// Fetch news for several tickers at once from Marketaux.
pub async fn marketaux_news(
    http: &HttpClient,
    api_key: &str,
    tickers: &[String],
    limit: usize,
) -> AppResult<Vec<NewsItem>> {
    if tickers.is_empty() {
        return Ok(vec![]);
    }
    let symbols = tickers.join(",");
    let url = format!(
        "https://api.marketaux.com/v1/news/all?symbols={symbols}&filter_entities=true&language=en&limit={limit}&api_token={api_key}"
    );
    let resp = http.get(&url)?.send().await?;
    if !resp.status().is_success() {
        return Ok(vec![]);
    }
    let body: Value = resp.json().await?;
    let mut out = Vec::new();
    for a in body.get("data").and_then(|v| v.as_array()).into_iter().flatten() {
        let headline = a.get("title").and_then(|v| v.as_str()).unwrap_or("").trim();
        let link = a.get("url").and_then(|v| v.as_str()).unwrap_or("").trim();
        if headline.is_empty() || link.is_empty() {
            continue;
        }
        let summary = a
            .get("description")
            .and_then(|v| v.as_str())
            .or_else(|| a.get("snippet").and_then(|v| v.as_str()))
            .unwrap_or("")
            .trim()
            .to_string();
        let source = a.get("source").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let day = iso_day(a.get("published_at").and_then(|v| v.as_str()).unwrap_or(""));

        // attribute the article to each of our tickers it mentions
        let mentioned: Vec<String> = a
            .get("entities")
            .and_then(|v| v.as_array())
            .map(|es| {
                es.iter()
                    .filter_map(|e| e.get("symbol").and_then(|v| v.as_str()))
                    .map(|s| s.to_ascii_uppercase())
                    .filter(|s| tickers.contains(s))
                    .collect()
            })
            .unwrap_or_default();

        for ticker in mentioned {
            out.push(NewsItem {
                ticker,
                headline: headline.to_string(),
                summary: summary.clone(),
                source: source.clone(),
                url: link.to_string(),
                published_at: day.clone(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_round_trips() {
        assert_eq!(NewsProvider::parse("finnhub"), NewsProvider::Finnhub);
        assert_eq!(NewsProvider::parse("marketaux"), NewsProvider::Marketaux);
        assert_eq!(NewsProvider::parse("whatever"), NewsProvider::None);
    }

    #[test]
    fn dates_normalize_to_days() {
        assert_eq!(iso_day("2026-09-01T13:00:00.000000Z"), "2026-09-01");
        assert_eq!(iso_day("2026-09-01"), "2026-09-01");
        assert_eq!(unix_to_date(1_756_684_800), "2025-09-01");
    }
}
