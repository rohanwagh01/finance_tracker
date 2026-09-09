-- Core schema for the local finance tracker.
-- All money values are stored in the account's currency (default USD).
-- Convention: transactions.amount is POSITIVE for money leaving the account
-- (spending / outflow) and NEGATIVE for inflow (deposits, refunds, income).

PRAGMA foreign_keys = ON;

-- A linked login at one institution through one aggregator provider.
CREATE TABLE items (
    id                TEXT PRIMARY KEY,
    provider          TEXT NOT NULL,             -- 'plaid' | 'snaptrade'
    institution_id    TEXT,
    institution_name  TEXT,
    secret_ref        TEXT NOT NULL,             -- keychain entry name holding the access token
    status            TEXT NOT NULL DEFAULT 'active',  -- active | error | disconnected
    error_message     TEXT,
    cursor            TEXT,                      -- Plaid /transactions/sync cursor
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    last_synced_at    TEXT
);

CREATE TABLE accounts (
    id                  TEXT PRIMARY KEY,
    item_id             TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    provider_account_id TEXT NOT NULL,
    name                TEXT NOT NULL,
    official_name       TEXT,
    mask                TEXT,
    type                TEXT NOT NULL,           -- depository | credit | investment | loan | other
    subtype             TEXT,
    currency            TEXT NOT NULL DEFAULT 'USD',
    current_balance     REAL,
    available_balance   REAL,
    credit_limit        REAL,
    is_shared           INTEGER NOT NULL DEFAULT 0,  -- user flag: card used by multiple people
    is_hidden           INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    UNIQUE(item_id, provider_account_id)
);

CREATE TABLE people (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    is_self     INTEGER NOT NULL DEFAULT 0,
    color       TEXT,
    created_at  TEXT NOT NULL
);

CREATE TABLE categories (
    id          TEXT PRIMARY KEY,               -- slug
    parent_id   TEXT REFERENCES categories(id),
    label       TEXT NOT NULL,
    is_custom   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE transactions (
    id                  TEXT PRIMARY KEY,
    account_id          TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    provider_txn_id     TEXT NOT NULL,
    posted_date         TEXT NOT NULL,
    authorized_date     TEXT,
    amount              REAL NOT NULL,
    currency            TEXT NOT NULL DEFAULT 'USD',
    description         TEXT NOT NULL,
    merchant_name       TEXT,
    category_id         TEXT REFERENCES categories(id),
    category_source     TEXT NOT NULL DEFAULT 'provider',  -- provider | rule | user
    pending             INTEGER NOT NULL DEFAULT 0,
    -- Attribution / review inbox
    review_status       TEXT NOT NULL DEFAULT 'not_required',
                        -- not_required | pending | kept | assigned | excluded
    owner_person_id     TEXT REFERENCES people(id),
    suggested_person_id TEXT REFERENCES people(id),
    suggestion_rule_id  TEXT,
    is_transfer         INTEGER NOT NULL DEFAULT 0,
    note                TEXT,
    location_json       TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    UNIQUE(account_id, provider_txn_id)
);
CREATE INDEX idx_txn_account_date ON transactions(account_id, posted_date);
CREATE INDEX idx_txn_review ON transactions(review_status);
CREATE INDEX idx_txn_category ON transactions(category_id);

CREATE TABLE rules (
    id              TEXT PRIMARY KEY,
    priority        INTEGER NOT NULL DEFAULT 100,
    enabled         INTEGER NOT NULL DEFAULT 1,
    match_field     TEXT NOT NULL,              -- description | merchant_name | amount
    match_op        TEXT NOT NULL,              -- contains | equals | regex | gt | lt | between
    match_value     TEXT NOT NULL,
    match_value2    TEXT,
    account_id      TEXT REFERENCES accounts(id) ON DELETE CASCADE,
    set_person_id   TEXT REFERENCES people(id),
    set_category_id TEXT REFERENCES categories(id),
    high_confidence INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL
);

CREATE TABLE securities (
    id                    TEXT PRIMARY KEY,
    provider_security_id  TEXT UNIQUE,
    ticker                TEXT,
    name                  TEXT,
    type                  TEXT,                 -- equity | etf | mutual_fund | crypto | cash | option | other
    currency              TEXT NOT NULL DEFAULT 'USD',
    sector                TEXT
);

CREATE TABLE holdings (
    id            TEXT PRIMARY KEY,
    account_id    TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    security_id   TEXT NOT NULL REFERENCES securities(id),
    quantity      REAL NOT NULL,
    cost_basis    REAL,
    price         REAL,
    price_as_of   TEXT,
    value         REAL,
    updated_at    TEXT NOT NULL,
    UNIQUE(account_id, security_id)
);

-- Point-in-time values powering the over-time charts.
CREATE TABLE value_snapshots (
    id            TEXT PRIMARY KEY,
    snapshot_date TEXT NOT NULL,
    scope         TEXT NOT NULL,                -- account | portfolio | net_worth
    account_id    TEXT REFERENCES accounts(id) ON DELETE CASCADE,
    value         REAL NOT NULL,
    created_at    TEXT NOT NULL,
    UNIQUE(snapshot_date, scope, account_id)
);

CREATE TABLE manual_assets (
    id                       TEXT PRIMARY KEY,
    name                     TEXT NOT NULL,
    kind                     TEXT NOT NULL,     -- vehicle | property | cash | other
    value                    REAL NOT NULL,
    as_of                    TEXT NOT NULL,
    depreciation_annual_pct  REAL,
    note                     TEXT,
    created_at               TEXT NOT NULL,
    updated_at               TEXT NOT NULL
);

CREATE TABLE manual_liabilities (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL,              -- mortgage | auto_loan | student_loan | other
    balance         REAL NOT NULL,
    as_of           TEXT NOT NULL,
    apr             REAL,
    minimum_payment REAL,
    note            TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE recurring_payments (
    id                TEXT PRIMARY KEY,
    label             TEXT NOT NULL,
    amount            REAL NOT NULL,
    cadence           TEXT NOT NULL DEFAULT 'monthly',  -- monthly | weekly | yearly | quarterly
    kind              TEXT NOT NULL DEFAULT 'expense',  -- expense | income
    category_id       TEXT REFERENCES categories(id),
    account_id        TEXT REFERENCES accounts(id) ON DELETE SET NULL,
    source            TEXT NOT NULL DEFAULT 'manual',   -- manual | detected
    detected_merchant TEXT,
    active            INTEGER NOT NULL DEFAULT 1,
    next_due          TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

CREATE TABLE income_sources (
    id             TEXT PRIMARY KEY,
    label          TEXT NOT NULL,
    monthly_amount REAL NOT NULL,
    kind           TEXT NOT NULL DEFAULT 'salary',  -- salary | freelance | investment | other
    source         TEXT NOT NULL DEFAULT 'manual',  -- manual | detected
    active         INTEGER NOT NULL DEFAULT 1,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

CREATE TABLE app_config (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE sync_log (
    id          TEXT PRIMARY KEY,
    item_id     TEXT REFERENCES items(id) ON DELETE CASCADE,
    started_at  TEXT NOT NULL,
    finished_at TEXT,
    status      TEXT NOT NULL,                  -- running | ok | error
    detail      TEXT
);

CREATE TABLE research_cache (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,                 -- news | llm_summary | suggestions
    cache_key    TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    expires_at   TEXT,
    UNIQUE(kind, cache_key)
);
