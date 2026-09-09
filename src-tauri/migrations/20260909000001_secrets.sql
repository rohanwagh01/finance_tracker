-- API credentials and per-item access tokens, kept inside the encrypted DB.
CREATE TABLE secrets (
    name       TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
