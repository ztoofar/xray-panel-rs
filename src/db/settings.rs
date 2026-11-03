use std::collections::HashMap;

use serde_json::{Map, Value};
use sqlx::SqlitePool;

pub async fn seed(db: &SqlitePool, defaults: &[(&str, &str)]) -> Result<(), sqlx::Error> {
    for (key, value) in defaults {
        sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(db)
            .await?;
    }
    Ok(())
}

pub async fn get(db: &SqlitePool, key: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await
}

pub async fn get_all(db: &SqlitePool) -> Result<HashMap<String, String>, sqlx::Error> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(db)
        .await?;
    Ok(rows.into_iter().collect())
}

// Values saved by the panel settings page live in one JSON blob under
// x3ui_settings; callers that need them at startup (listen addr, session TTL,
// job behaviour) read the same shape.
pub async fn ui_settings(db: &SqlitePool) -> Result<Map<String, Value>, sqlx::Error> {
    let raw = get(db, "x3ui_settings").await?.unwrap_or_default();
    let value: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    Ok(value.as_object().cloned().unwrap_or_default())
}

pub async fn session_ttl_secs(db: &SqlitePool) -> i64 {
    let minutes = ui_settings(db)
        .await
        .ok()
        .and_then(|settings| {
            settings.get("sessionMaxAge").and_then(|value| {
                value
                    .as_i64()
                    .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
            })
        })
        .unwrap_or(crate::auth::SESSION_TTL / 60);
    minutes.clamp(1, 525_600) * 60
}

pub async fn set(db: &SqlitePool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    set_many(db, &[(key.to_string(), value.to_string())]).await
}

pub async fn set_many(db: &SqlitePool, pairs: &[(String, String)]) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    for (key, value) in pairs {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
