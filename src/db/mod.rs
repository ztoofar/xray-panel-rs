use std::{path::Path, time::Duration};

use anyhow::Context;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};

pub mod client_inbounds;
pub mod client_traffic;
pub mod clients;
pub mod inbounds;
pub mod models;
pub mod outbound_subs;
pub mod outbounds;
pub mod sessions;
pub mod settings;
pub mod users;

pub type Db = SqlitePool;

pub async fn init(path: &Path) -> anyhow::Result<Db> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create database directory {}", parent.display()))?;
        }
    }

    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true)
        // ~2 MiB page cache keeps the panel friendly to tiny VPS instances.
        .pragma("cache_size", "-2000")
        .pragma("temp_store", "MEMORY");

    // SQLite serialises writers anyway; a tiny pool avoids idle memory.
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .context("open sqlite database")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("run database migrations")?;

    Ok(pool)
}
