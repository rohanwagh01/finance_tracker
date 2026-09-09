//! SQLite connection pool + embedded migrations.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::error::AppResult;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct Db {
    pub pool: SqlitePool,
}

impl Db {
    /// Open (creating if needed) the database at `path` and run pending migrations.
    pub async fn connect(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(10))
            .pragma("journal_mode", "WAL")
            .pragma("synchronous", "NORMAL");

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    /// In-memory database, used by tests.
    #[cfg(test)]
    pub async fn connect_in_memory() -> AppResult<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_apply_and_seed_categories() {
        let db = Db::connect_in_memory().await.expect("migrate");

        let cats: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM categories")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert!(cats.0 >= 17, "expected seeded categories, got {}", cats.0);

        // Exercise a representative insert path (config upsert).
        sqlx::query(
            "INSERT INTO app_config (key, value, updated_at) VALUES ('k', 'v', 'now')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .execute(&db.pool)
        .await
        .unwrap();

        let v: (String,) = sqlx::query_as("SELECT value FROM app_config WHERE key = 'k'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(v.0, "v");
    }
}
