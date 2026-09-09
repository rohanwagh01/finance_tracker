//! Research commands: watchlist CRUD, a preview of exactly what would leave the
//! machine, cached news, and the LLM analysis run.

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::research::llm::{self, LlmProvider, ResearchAnalysis};
use crate::research::news::{self, NewsItem, NewsProvider};
use crate::research::safe_context::{build_safe_context, tickers, RawHolding, SafePortfolioContext};
use crate::secrets::keys;
use crate::state::AppState;
use crate::util::{new_id, now};

const NEWS_LOOKBACK_DAYS: i64 = 14;
const MAX_NEWS_TICKERS: usize = 20;
const PER_TICKER_HEADLINES: usize = 6;
const KEEP_REPORTS: i64 = 10;

// --- watchlist -------------------------------------------------------------

fn clean_ticker(raw: &str) -> AppResult<String> {
    let t = raw.trim().to_ascii_uppercase();
    if t.is_empty() || t.len() > 12 || !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return Err(AppError::Invalid(format!("not a ticker: {raw:?}")));
    }
    Ok(t)
}

#[tauri::command]
pub async fn research_watchlist(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let db = state.db().await?;
    Ok(
        sqlx::query_scalar("SELECT ticker FROM research_watchlist ORDER BY ticker")
            .fetch_all(&db.pool)
            .await?,
    )
}

#[tauri::command]
pub async fn research_watchlist_add(state: State<'_, AppState>, ticker: String) -> AppResult<()> {
    let t = clean_ticker(&ticker)?;
    let db = state.db().await?;
    sqlx::query("INSERT OR IGNORE INTO research_watchlist (ticker, created_at) VALUES (?1, ?2)")
        .bind(&t)
        .bind(now())
        .execute(&db.pool)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn research_watchlist_remove(state: State<'_, AppState>, ticker: String) -> AppResult<()> {
    let db = state.db().await?;
    sqlx::query("DELETE FROM research_watchlist WHERE ticker = ?1")
        .bind(ticker.trim().to_ascii_uppercase())
        .execute(&db.pool)
        .await?;
    Ok(())
}

// --- safe context ---------------------------------------------------------

async fn raw_holdings(pool: &SqlitePool) -> AppResult<Vec<RawHolding>> {
    // `gain_frac` is computed over just the lots that report a cost basis
    // (value_with_basis - cost_basis) / cost_basis — so a partial cost basis
    // still yields a meaningful number for the part that's known.
    let rows: Vec<(Option<String>, Option<String>, f64, Option<f64>)> = sqlx::query_as(
        "SELECT s.ticker, s.sector,
                COALESCE(SUM(COALESCE(h.value, h.price * h.quantity, 0.0)), 0.0) AS value,
                CASE WHEN SUM(h.cost_basis) > 0 THEN
                    (SUM(CASE WHEN h.cost_basis IS NOT NULL
                              THEN COALESCE(h.value, h.price * h.quantity, 0.0) END)
                     - SUM(h.cost_basis)) / SUM(h.cost_basis)
                END AS gain_frac
         FROM holdings h
         JOIN accounts a ON a.id = h.account_id
         JOIN securities s ON s.id = h.security_id
         WHERE a.type = 'investment' AND a.is_hidden = 0
           AND s.ticker IS NOT NULL AND s.ticker <> '' AND s.ticker NOT LIKE 'CUR:%'
           AND COALESCE(s.type, '') <> 'cash'
         GROUP BY s.ticker, s.sector",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(ticker, sector, value, gain_frac)| RawHolding { ticker, sector, value, gain_frac })
        .collect())
}

/// Whether the user has opted to share rounded gain/loss % with the LLM.
async fn share_gains(state: &AppState) -> AppResult<bool> {
    Ok(state.config_get_or("research_share_gains", "false").await? == "true")
}

async fn safe_context(pool: &SqlitePool, include_gains: bool) -> AppResult<SafePortfolioContext> {
    let raw = raw_holdings(pool).await?;
    let watchlist: Vec<String> =
        sqlx::query_scalar("SELECT ticker FROM research_watchlist ORDER BY ticker")
            .fetch_all(pool)
            .await?;
    Ok(build_safe_context(&raw, &watchlist, include_gains))
}

/// Exactly the payload that would be sent to the news APIs / research LLM.
#[tauri::command]
pub async fn research_safe_context(state: State<'_, AppState>) -> AppResult<SafePortfolioContext> {
    let include_gains = share_gains(&state).await?;
    safe_context(&state.db().await?.pool, include_gains).await
}

// --- news ---------------------------------------------------------------

async fn news_config(state: &AppState) -> AppResult<Option<(NewsProvider, String)>> {
    let provider = NewsProvider::parse(&state.config_get_or("news_provider", "finnhub").await?);
    let key_name = match provider {
        NewsProvider::Finnhub => keys::FINNHUB_API_KEY,
        NewsProvider::Marketaux => keys::MARKETAUX_API_KEY,
        NewsProvider::None => return Ok(None),
    };
    match state.secrets().await?.get(key_name).await? {
        Some(k) => Ok(Some((provider, k))),
        None => Ok(None),
    }
}

async fn cached(pool: &SqlitePool, key: &str, today: &str) -> AppResult<Option<Vec<NewsItem>>> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT payload, fetched_at FROM research_news_cache WHERE cache_key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    match row {
        Some((payload, fetched_at)) if fetched_at.starts_with(today) => {
            Ok(serde_json::from_str(&payload).ok())
        }
        _ => Ok(None),
    }
}

async fn store_cache(pool: &SqlitePool, key: &str, items: &[NewsItem]) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO research_news_cache (cache_key, payload, fetched_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(cache_key) DO UPDATE SET payload = excluded.payload, fetched_at = excluded.fetched_at",
    )
    .bind(key)
    .bind(serde_json::to_string(items)?)
    .bind(now())
    .execute(pool)
    .await?;
    Ok(())
}

async fn gather_news(
    state: &AppState,
    ctx: &SafePortfolioContext,
    force_refresh: bool,
) -> AppResult<Vec<NewsItem>> {
    let Some((provider, api_key)) = news_config(state).await? else {
        return Ok(vec![]);
    };
    let pool = &state.db().await?.pool;
    let http = &state.http;

    let all_tickers = tickers(ctx);
    let syms: Vec<String> = all_tickers.into_iter().take(MAX_NEWS_TICKERS).collect();
    if syms.is_empty() {
        return Ok(vec![]);
    }

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let from = (chrono::Utc::now() - chrono::Duration::days(NEWS_LOOKBACK_DAYS))
        .format("%Y-%m-%d")
        .to_string();

    let mut out: Vec<NewsItem> = Vec::new();

    match provider {
        NewsProvider::Finnhub => {
            for sym in &syms {
                let key = format!("finnhub:{sym}:{today}");
                if !force_refresh {
                    if let Some(hit) = cached(pool, &key, &today).await? {
                        out.extend(hit);
                        continue;
                    }
                }
                let items = news::finnhub_company_news(
                    http, &api_key, sym, &from, &today, PER_TICKER_HEADLINES,
                )
                .await
                .unwrap_or_default();
                store_cache(pool, &key, &items).await.ok();
                out.extend(items);
            }
        }
        NewsProvider::Marketaux => {
            let key = format!("marketaux:{}:{today}", syms.join("-"));
            if !force_refresh {
                if let Some(hit) = cached(pool, &key, &today).await? {
                    return Ok(sorted(hit));
                }
            }
            let items = news::marketaux_news(http, &api_key, &syms, MAX_NEWS_TICKERS * PER_TICKER_HEADLINES)
                .await
                .unwrap_or_default();
            store_cache(pool, &key, &items).await.ok();
            out.extend(items);
        }
        NewsProvider::None => {}
    }

    Ok(sorted(out))
}

fn sorted(mut items: Vec<NewsItem>) -> Vec<NewsItem> {
    items.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    items.dedup_by(|a, b| a.url == b.url && a.ticker == b.ticker);
    items
}

#[tauri::command]
pub async fn research_news(
    state: State<'_, AppState>,
    refresh: Option<bool>,
) -> AppResult<Vec<NewsItem>> {
    // news only needs tickers, never gain figures
    let ctx = safe_context(&state.db().await?.pool, false).await?;
    gather_news(&state, &ctx, refresh.unwrap_or(false)).await
}

// --- analysis run -------------------------------------------------------

async fn llm_provider(state: &AppState) -> AppResult<LlmProvider> {
    match state.config_get_or("llm_provider", "none").await?.as_str() {
        "anthropic" => {
            let api_key = state.secrets().await?.get(keys::ANTHROPIC_API_KEY).await?.ok_or_else(|| {
                AppError::Config("add your Anthropic API key in Settings to run research".into())
            })?;
            Ok(LlmProvider::Anthropic {
                api_key,
                model: state.config_get_or("anthropic_model", "claude-sonnet-5").await?,
            })
        }
        "ollama" => Ok(LlmProvider::Ollama {
            url: state.config_get_or("ollama_url", "http://localhost:11434").await?,
            model: state.config_get_or("ollama_model", "llama3.1").await?,
        }),
        _ => Err(AppError::Config(
            "choose a research LLM (Anthropic or Ollama) in Settings first".into(),
        )),
    }
}

#[derive(Serialize, serde::Deserialize, Clone)]
pub struct ResearchReport {
    pub id: String,
    pub created_at: String,
    pub provider: String,
    pub model: String,
    /// The follow-up question this report answers, if any (`None` for a fresh run).
    #[serde(default)]
    pub prompt: Option<String>,
    pub context: SafePortfolioContext,
    pub news: Vec<NewsItem>,
    pub analysis: ResearchAnalysis,
}

async fn save_report(pool: &SqlitePool, report: &ResearchReport) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO research_reports (id, created_at, provider, model, report_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(&report.id)
    .bind(&report.created_at)
    .bind(&report.provider)
    .bind(&report.model)
    .bind(serde_json::to_string(report)?)
    .execute(pool)
    .await?;

    sqlx::query(
        "DELETE FROM research_reports WHERE id NOT IN
           (SELECT id FROM research_reports ORDER BY created_at DESC LIMIT ?1)",
    )
    .bind(KEEP_REPORTS)
    .execute(pool)
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn research_run(state: State<'_, AppState>) -> AppResult<ResearchReport> {
    let pool = state.db().await?.pool;
    let ctx = safe_context(&pool, share_gains(&state).await?).await?;
    if ctx.holdings.is_empty() && ctx.watchlist.is_empty() {
        return Err(AppError::Invalid(
            "no holdings or watchlist tickers to research yet".into(),
        ));
    }
    let provider = llm_provider(&state).await?;
    let news = gather_news(&state, &ctx, false).await.unwrap_or_default();

    let analysis = llm::run_analysis(&state.http, &provider, &ctx, &news).await?;

    let report = ResearchReport {
        id: new_id(),
        created_at: now(),
        provider: provider.provider_name().to_string(),
        model: provider.model().to_string(),
        prompt: None,
        context: ctx,
        news,
        analysis,
    };
    save_report(&pool, &report).await?;
    Ok(report)
}

/// Ask a follow-up question about the most recent analysis. The model revises
/// the analysis to answer it; the new report replaces the one shown on the page.
#[tauri::command]
pub async fn research_followup(
    state: State<'_, AppState>,
    question: String,
) -> AppResult<ResearchReport> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err(AppError::Invalid("ask a question first".into()));
    }
    let pool = state.db().await?.pool;
    let prior: ResearchReport = sqlx::query_scalar::<_, String>(
        "SELECT report_json FROM research_reports ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await?
    .and_then(|j| serde_json::from_str(&j).ok())
    .ok_or_else(|| AppError::Invalid("run an analysis before asking a follow-up".into()))?;

    let provider = llm_provider(&state).await?;
    // Rebuild the context so a follow-up reflects the current data and the
    // current "share gain/loss %" setting (news is same-day cached, reuse it).
    let ctx = safe_context(&pool, share_gains(&state).await?).await?;
    let analysis = llm::run_followup(
        &state.http,
        &provider,
        &ctx,
        &prior.news,
        &prior.analysis,
        &question,
    )
    .await?;

    let report = ResearchReport {
        id: new_id(),
        created_at: now(),
        provider: provider.provider_name().to_string(),
        model: provider.model().to_string(),
        prompt: Some(question),
        context: ctx,
        news: prior.news,
        analysis,
    };
    save_report(&pool, &report).await?;
    Ok(report)
}

#[tauri::command]
pub async fn research_latest(state: State<'_, AppState>) -> AppResult<Option<ResearchReport>> {
    let db = state.db().await?;
    let row: Option<String> = sqlx::query_scalar(
        "SELECT report_json FROM research_reports ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(&db.pool)
    .await?;
    Ok(row.and_then(|j| serde_json::from_str(&j).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn ticker_validation() {
        assert_eq!(clean_ticker(" aapl ").unwrap(), "AAPL");
        assert_eq!(clean_ticker("brk.b").unwrap(), "BRK.B");
        assert!(clean_ticker("").is_err());
        assert!(clean_ticker("hello world").is_err());
        assert!(clean_ticker("way-too-long-symbol").is_err());
    }

    #[tokio::test]
    async fn safe_context_aggregates_by_ticker_and_excludes_cash() {
        let db = Db::connect_in_memory().await.unwrap();
        let p = &db.pool;
        let ts = now();
        sqlx::query("INSERT INTO items (id,provider,secret_ref,status,created_at,updated_at) VALUES ('i','plaid','x','active',?1,?1)").bind(&ts).execute(p).await.unwrap();
        for a in ["b1", "b2"] {
            sqlx::query("INSERT INTO accounts (id,item_id,provider_account_id,name,type,currency,is_hidden,created_at,updated_at) VALUES (?1,'i',?1,?1,'investment','USD',0,?2,?2)")
                .bind(a).bind(&ts).execute(p).await.unwrap();
        }
        sqlx::query("INSERT INTO securities (id,ticker,name,type) VALUES ('s1','AAPL','Apple','equity'),('s2','CUR:USD','US Dollar','cash'),('s3','VTI','Vanguard Total','etf')").execute(p).await.unwrap();
        // AAPL held in both accounts → should sum
        sqlx::query("INSERT INTO holdings (id,account_id,security_id,quantity,value,updated_at) VALUES ('h1','b1','s1',1,600,?1),('h2','b2','s1',1,400,?1),('h3','b1','s2',1,1000,?1),('h4','b1','s3',1,9000,?1)")
            .bind(&ts).execute(p).await.unwrap();
        sqlx::query("INSERT INTO research_watchlist (ticker,created_at) VALUES ('nvda',?1)").bind(&ts).execute(p).await.unwrap();

        let ctx = safe_context(p, false).await.unwrap();
        // cash excluded → total = 1000 (AAPL) + 9000 (VTI) = 10000
        assert_eq!(ctx.holdings.len(), 2);
        let aapl = ctx.holdings.iter().find(|h| h.ticker == "AAPL").unwrap();
        assert_eq!(aapl.allocation_pct, 10);
        assert_eq!(ctx.watchlist, vec!["NVDA"]);

        let json = serde_json::to_string(&ctx).unwrap();
        for forbidden in ["600", "9000", "1000", "value", "quantity"] {
            assert!(!json.contains(forbidden), "leaked {forbidden}: {json}");
        }
    }

    #[tokio::test]
    async fn gain_pct_from_cost_basis_across_accounts() {
        let db = Db::connect_in_memory().await.unwrap();
        let p = &db.pool;
        let ts = now();
        sqlx::query("INSERT INTO items (id,provider,secret_ref,status,created_at,updated_at) VALUES ('i','plaid','x','active',?1,?1)").bind(&ts).execute(p).await.unwrap();
        for a in ["b1", "b2"] {
            sqlx::query("INSERT INTO accounts (id,item_id,provider_account_id,name,type,currency,is_hidden,created_at,updated_at) VALUES (?1,'i',?1,?1,'investment','USD',0,?2,?2)")
                .bind(a).bind(&ts).execute(p).await.unwrap();
        }
        sqlx::query("INSERT INTO securities (id,ticker,name,type) VALUES ('s1','AAPL','Apple','equity'),('s2','MSFT','Microsoft','equity')").execute(p).await.unwrap();
        // AAPL held in both accounts, both report cost basis → cost 10000, value 14000 → +40%
        sqlx::query("INSERT INTO holdings (id,account_id,security_id,quantity,value,cost_basis,updated_at) VALUES ('h1','b1','s1',1,7000,6000,?1),('h2','b2','s1',1,7000,4000,?1)").bind(&ts).execute(p).await.unwrap();
        // MSFT: only one account reports cost basis (3000 → 5000) → +67% on the known part
        sqlx::query("INSERT INTO holdings (id,account_id,security_id,quantity,value,cost_basis,updated_at) VALUES ('h3','b1','s2',1,5000,3000,?1),('h4','b2','s2',1,5000,NULL,?1)").bind(&ts).execute(p).await.unwrap();

        // off unless the setting is on
        assert!(safe_context(p, false).await.unwrap().holdings.iter().all(|h| h.gain_pct.is_none()));

        let ctx = safe_context(p, true).await.unwrap();
        assert_eq!(ctx.holdings.iter().find(|h| h.ticker == "AAPL").unwrap().gain_pct, Some(40));
        assert_eq!(ctx.holdings.iter().find(|h| h.ticker == "MSFT").unwrap().gain_pct, Some(67));
    }
}
