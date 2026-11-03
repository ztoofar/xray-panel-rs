use serde_json::Value;
use sqlx::types::Json;
use sqlx::SqlitePool;

use super::models::{now, Inbound, InboundCount};

#[derive(Debug, Clone)]
pub struct InboundInput {
    pub user_id: i64,
    pub remark: String,
    pub sub_sort_index: i64,
    pub exclude_from_sub: bool,
    pub enable: bool,
    pub expiry_time: i64,
    pub total: i64,
    pub traffic_reset: String,
    pub traffic_reset_day: i64,
    pub listen: String,
    pub port: i64,
    pub protocol: String,
    pub settings: Value,
    pub stream_settings: Value,
    pub sniffing: Value,
    pub tag: String,
    pub share_addr_strategy: String,
    pub share_addr: String,
    pub disable_flow: bool,
}

pub async fn list(db: &SqlitePool) -> Result<Vec<Inbound>, sqlx::Error> {
    sqlx::query_as::<_, Inbound>("SELECT * FROM inbounds ORDER BY id")
        .fetch_all(db)
        .await
}

pub async fn list_enabled(db: &SqlitePool) -> Result<Vec<Inbound>, sqlx::Error> {
    sqlx::query_as::<_, Inbound>("SELECT * FROM inbounds WHERE enable = 1 ORDER BY id")
        .fetch_all(db)
        .await
}

pub async fn get(db: &SqlitePool, id: i64) -> Result<Option<Inbound>, sqlx::Error> {
    sqlx::query_as::<_, Inbound>("SELECT * FROM inbounds WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn create(db: &SqlitePool, input: &InboundInput) -> Result<i64, sqlx::Error> {
    let ts = now();
    let res = sqlx::query(
        "INSERT INTO inbounds
         (user_id, remark, sub_sort_index, exclude_from_sub, enable, expiry_time, total,
          traffic_reset, traffic_reset_day, last_traffic_reset_time, listen, port, protocol,
          settings, stream_settings, sniffing, tag, share_addr_strategy, share_addr,
          disable_flow, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(input.user_id)
    .bind(&input.remark)
    .bind(input.sub_sort_index)
    .bind(input.exclude_from_sub)
    .bind(input.enable)
    .bind(input.expiry_time)
    .bind(input.total)
    .bind(&input.traffic_reset)
    .bind(input.traffic_reset_day)
    .bind(ts)
    .bind(&input.listen)
    .bind(input.port)
    .bind(&input.protocol)
    .bind(Json(input.settings.clone()))
    .bind(Json(input.stream_settings.clone()))
    .bind(Json(input.sniffing.clone()))
    .bind(&input.tag)
    .bind(&input.share_addr_strategy)
    .bind(&input.share_addr)
    .bind(input.disable_flow)
    .bind(ts)
    .bind(ts)
    .execute(db)
    .await?;
    Ok(res.last_insert_rowid())
}

pub async fn update(db: &SqlitePool, id: i64, input: &InboundInput) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE inbounds SET
         remark = ?, sub_sort_index = ?, exclude_from_sub = ?, enable = ?, expiry_time = ?,
         total = ?, traffic_reset = ?, traffic_reset_day = ?, listen = ?, port = ?,
         protocol = ?, settings = ?, stream_settings = ?, sniffing = ?, tag = ?,
         share_addr_strategy = ?, share_addr = ?, disable_flow = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&input.remark)
    .bind(input.sub_sort_index)
    .bind(input.exclude_from_sub)
    .bind(input.enable)
    .bind(input.expiry_time)
    .bind(input.total)
    .bind(&input.traffic_reset)
    .bind(input.traffic_reset_day)
    .bind(&input.listen)
    .bind(input.port)
    .bind(&input.protocol)
    .bind(Json(input.settings.clone()))
    .bind(Json(input.stream_settings.clone()))
    .bind(Json(input.sniffing.clone()))
    .bind(&input.tag)
    .bind(&input.share_addr_strategy)
    .bind(&input.share_addr)
    .bind(input.disable_flow)
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete(db: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM inbounds WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn set_enable(db: &SqlitePool, id: i64, enable: bool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE inbounds SET enable = ?, updated_at = ? WHERE id = ?")
        .bind(enable)
        .bind(now())
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn add_traffic(
    db: &SqlitePool,
    id: i64,
    up_delta: i64,
    down_delta: i64,
    last_up: i64,
    last_down: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE inbounds SET up = up + ?, down = down + ?, last_up = ?, last_down = ?,
         updated_at = ? WHERE id = ?",
    )
    .bind(up_delta)
    .bind(down_delta)
    .bind(last_up)
    .bind(last_down)
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn reset_traffic(db: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE inbounds SET up = 0, down = 0, last_up = 0, last_down = 0,
         last_traffic_reset_time = ?, updated_at = ? WHERE id = ?",
    )
    .bind(now())
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

// Record a scheduled reset attempt without clearing traffic. Used when a
// member reset failed: nothing is zeroed and the next attempt waits for the
// next reset period instead of retrying on every poll.
pub async fn touch_reset_time(db: &SqlitePool, id: i64, ts: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE inbounds SET last_traffic_reset_time = ?, updated_at = ? WHERE id = ?")
        .bind(ts)
        .bind(ts)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn client_counts(db: &SqlitePool) -> Result<Vec<InboundCount>, sqlx::Error> {
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT inbound_id, COUNT(*) FROM client_inbounds GROUP BY inbound_id")
            .fetch_all(db)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(inbound_id, clients)| InboundCount {
            inbound_id,
            clients,
        })
        .collect())
}

pub async fn tag_exists(
    db: &SqlitePool,
    tag: &str,
    exclude_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let count: i64 = match exclude_id {
        Some(id) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM inbounds WHERE tag = ? AND id != ?")
                .bind(tag)
                .bind(id)
                .fetch_one(db)
                .await?
        }
        None => {
            sqlx::query_scalar("SELECT COUNT(*) FROM inbounds WHERE tag = ?")
                .bind(tag)
                .fetch_one(db)
                .await?
        }
    };
    Ok(count > 0)
}

pub async fn set_settings(db: &SqlitePool, id: i64, settings: &Value) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE inbounds SET settings = ?, updated_at = ? WHERE id = ?")
        .bind(sqlx::types::Json(settings.clone()))
        .bind(super::models::now())
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}
