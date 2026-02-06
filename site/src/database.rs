use std::str::FromStr;

use sqlx::{Pool, Sqlite, query, sqlite::{self, SqlitePool}};

pub (crate) async fn initialize() -> anyhow::Result<Pool<Sqlite>> {
    let options = sqlite::SqliteConnectOptions::from_str(&std::env::var("DATABASE_URL")?)?
        .foreign_keys(true)
        .create_if_missing(false)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePool::connect_with(options)
        .await
        .map_err(anyhow::Error::from)?;

    query("PRAGMA journal_mode = WAL;")
        .execute(&pool)
        .await
        .map_err(anyhow::Error::from)?;

    Ok(pool)
}
