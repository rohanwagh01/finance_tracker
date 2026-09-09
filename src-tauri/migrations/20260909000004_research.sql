-- Milestone 8: research feature — watchlist, cached news, saved LLM reports.

CREATE TABLE research_watchlist (
    ticker     TEXT PRIMARY KEY,
    created_at TEXT NOT NULL
);

-- Raw provider news responses, keyed by "<provider>:<ticker>:<yyyy-mm-dd>".
CREATE TABLE research_news_cache (
    cache_key  TEXT PRIMARY KEY,
    payload    TEXT NOT NULL,
    fetched_at TEXT NOT NULL
);

-- Saved analysis runs (newest is shown on the Research page by default).
CREATE TABLE research_reports (
    id          TEXT PRIMARY KEY,
    created_at  TEXT NOT NULL,
    provider    TEXT NOT NULL,
    model       TEXT NOT NULL,
    report_json TEXT NOT NULL
);
