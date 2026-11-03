use sqlx::SqlitePool;

use super::models::{now, OutboundSubscription};

#[derive(Debug, Clone)]
pub struct NewSubscription {
    pub remark: String,
    pub url: String,
    pub enabled: bool,
    pub allow_private: bool,
    pub allow_insecure: bool,
    pub user_agent: String,
    pub proxy_url: String,
    pub tag_prefix: String,
    pub update_interval: i64,
    pub priority: i64,
    pub prepend: bool,
}

pub async fn list(db: &SqlitePool) -> Result<Vec<OutboundSubscription>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM outbound_subscriptions ORDER BY priority ASC, id ASC")
        .fetch_all(db)
        .await
}

pub async fn list_enabled(db: &SqlitePool) -> Result<Vec<OutboundSubscription>, sqlx::Error> {
    sqlx::query_as(
        "SELECT * FROM outbound_subscriptions WHERE enabled = 1 ORDER BY priority ASC, id ASC",
    )
    .fetch_all(db)
    .await
}

pub async fn get(db: &SqlitePool, id: i64) -> Result<Option<OutboundSubscription>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM outbound_subscriptions WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn count(db: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM outbound_subscriptions")
        .fetch_one(db)
        .await
}

pub async fn insert(db: &SqlitePool, sub: &NewSubscription) -> Result<i64, sqlx::Error> {
    let timestamp = now();
    let result = sqlx::query(
        "INSERT INTO outbound_subscriptions
         (remark, url, enabled, allow_private, allow_insecure, user_agent, proxy_url,
          tag_prefix, update_interval, priority, prepend, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&sub.remark)
    .bind(&sub.url)
    .bind(sub.enabled)
    .bind(sub.allow_private)
    .bind(sub.allow_insecure)
    .bind(&sub.user_agent)
    .bind(&sub.proxy_url)
    .bind(&sub.tag_prefix)
    .bind(sub.update_interval)
    .bind(sub.priority)
    .bind(sub.prepend)
    .bind(timestamp)
    .bind(timestamp)
    .execute(db)
    .await?;
    Ok(result.last_insert_rowid())
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    db: &SqlitePool,
    id: i64,
    remark: &str,
    url: &str,
    enabled: bool,
    allow_private: bool,
    allow_insecure: bool,
    user_agent: &str,
    proxy_url: &str,
    tag_prefix: &str,
    update_interval: i64,
    prepend: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE outbound_subscriptions SET
           remark = ?, url = ?, enabled = ?, allow_private = ?, allow_insecure = ?,
           user_agent = ?, proxy_url = ?, tag_prefix = ?, update_interval = ?,
           prepend = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(remark)
    .bind(url)
    .bind(enabled)
    .bind(allow_private)
    .bind(allow_insecure)
    .bind(user_agent)
    .bind(proxy_url)
    .bind(tag_prefix)
    .bind(update_interval)
    .bind(prepend)
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM outbound_subscriptions WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn set_result(
    db: &SqlitePool,
    id: i64,
    outbounds: &str,
    identities: &str,
    last_error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE outbound_subscriptions SET
           last_fetched_outbounds = ?, link_identities = ?, last_updated = ?,
           last_error = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(outbounds)
    .bind(identities)
    .bind(now())
    .bind(last_error)
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn restore_result(
    db: &SqlitePool,
    id: i64,
    outbounds: &str,
    identities: &str,
    last_updated: i64,
    last_error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE outbound_subscriptions SET
           last_fetched_outbounds = ?, link_identities = ?, last_updated = ?, last_error = ?
         WHERE id = ?",
    )
    .bind(outbounds)
    .bind(identities)
    .bind(last_updated)
    .bind(last_error)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_error(db: &SqlitePool, id: i64, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE outbound_subscriptions SET last_error = ? WHERE id = ?")
        .bind(error)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn set_priorities(db: &SqlitePool, ordered_ids: &[i64]) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    for (index, id) in ordered_ids.iter().enumerate() {
        sqlx::query("UPDATE outbound_subscriptions SET priority = ? WHERE id = ?")
            .bind(index as i64)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
