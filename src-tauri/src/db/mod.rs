//! SQLite connection pool + embedded migrations.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, Connection, SqlitePool};

use crate::error::{AppError, AppResult};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct Db {
    pub pool: SqlitePool,
}

impl Db {
    /// Open a SQLCipher-encrypted database at `path` with the given hex key
    /// (64 hex chars = 32 bytes), then run pending migrations.
    ///
    /// `PRAGMA key` is emitted first by sqlx's connect-options handling, before
    /// any other pragma. A wrong key makes the first real statement fail with
    /// "file is not a database" — surfaced here as an `Invalid` error so the
    /// caller can show "incorrect password".
    pub async fn connect_encrypted(path: &Path, key_hex: &str) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(10))
            .pragma("key", format!("\"x'{key_hex}'\""))
            .pragma("journal_mode", "WAL")
            .pragma("synchronous", "NORMAL");

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        // Validate the key before touching migrations.
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sqlite_master")
            .fetch_one(&pool)
            .await
            .map_err(|_| AppError::Invalid("incorrect password".into()))?;

        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    /// Re-encrypt the database file with a new key. Validates `old_key_hex`
    /// first (wrong key → `Invalid("current password is incorrect")`). Run this
    /// with no other connections open to the file.
    pub async fn rekey(path: &Path, old_key_hex: &str, new_key_hex: &str) -> AppResult<()> {
        let mut conn = SqliteConnectOptions::new()
            .filename(path)
            .pragma("key", format!("\"x'{old_key_hex}'\""))
            .connect()
            .await?;

        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sqlite_master")
            .fetch_one(&mut conn)
            .await
            .map_err(|_| AppError::Invalid("current password is incorrect".into()))?;

        sqlx::query(&format!("PRAGMA rekey = \"x'{new_key_hex}'\""))
            .execute(&mut conn)
            .await?;

        conn.close().await?;
        Ok(())
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
    async fn encrypted_db_roundtrips_and_rejects_wrong_key() {
        let dir = std::env::temp_dir().join(format!("ft-enc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v.db");
        let key1 = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff0";
        let key2 = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0";

        {
            let db = Db::connect_encrypted(&path, key1).await.unwrap();
            sqlx::query("INSERT INTO app_config (key, value, updated_at) VALUES ('k','v','t')")
                .execute(&db.pool)
                .await
                .unwrap();
            db.pool.close().await;
        }

        assert!(
            Db::connect_encrypted(&path, key2).await.is_err(),
            "wrong key must be rejected"
        );

        let db = Db::connect_encrypted(&path, key1).await.unwrap();
        let v: String = sqlx::query_scalar("SELECT value FROM app_config WHERE key = 'k'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(v, "v");
        db.pool.close().await;

        let header = std::fs::read(&path).unwrap();
        assert_ne!(
            &header[0..16],
            b"SQLite format 3\0",
            "file must not be a plaintext SQLite database"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

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
