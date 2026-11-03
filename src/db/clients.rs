use serde_json::Value;
use sqlx::types::Json;
use sqlx::SqlitePool;

use super::models::{now, Client};

#[derive(Debug, Clone)]
pub struct ClientInput {
    pub inbound_id: i64,
    pub email: String,
    pub enable: bool,
    pub total: i64,
    pub expiry_time: i64,
    pub settings: Value,
    pub comment: String,
    pub group_name: String,
    pub limit_ip: i64,
    pub limit_hwid: i64,
    pub reset_days: i64,
}

pub async fn list_all(db: &SqlitePool) -> Result<Vec<Client>, sqlx::Error> {
    sqlx::query_as::<_, Client>("SELECT * FROM clients ORDER BY id")
        .fetch_all(db)
        .await
}

pub async fn list_by_inbound(db: &SqlitePool, inbound_id: i64) -> Result<Vec<Client>, sqlx::Error> {
    sqlx::query_as::<_, Client>(
        "SELECT c.* FROM clients c
         JOIN client_inbounds ci ON ci.client_id = c.id
         WHERE ci.inbound_id = ? ORDER BY c.id",
    )
    .bind(inbound_id)
    .fetch_all(db)
    .await
}

pub async fn find_by_email(db: &SqlitePool, email: &str) -> Result<Option<Client>, sqlx::Error> {
    sqlx::query_as::<_, Client>("SELECT * FROM clients WHERE email = ?")
        .bind(email)
        .fetch_optional(db)
        .await
}

pub async fn get(db: &SqlitePool, id: i64) -> Result<Option<Client>, sqlx::Error> {
    sqlx::query_as::<_, Client>("SELECT * FROM clients WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn email_exists(
    db: &SqlitePool,
    email: &str,
    exclude_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let count: i64 = match exclude_id {
        Some(id) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM clients WHERE email = ? AND id != ?")
                .bind(email)
                .bind(id)
                .fetch_one(db)
                .await?
        }
        None => {
            sqlx::query_scalar("SELECT COUNT(*) FROM clients WHERE email = ?")
                .bind(email)
                .fetch_one(db)
                .await?
        }
    };
    Ok(count > 0)
}

pub async fn create(db: &SqlitePool, input: &ClientInput) -> Result<i64, sqlx::Error> {
    let ts = now();
    let res = sqlx::query(
        "INSERT INTO clients
         (inbound_id, email, enable, total, expiry_time, settings, comment, group_name,
          limit_ip, limit_hwid, reset_days, last_reset_time, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(input.inbound_id)
    .bind(&input.email)
    .bind(input.enable)
    .bind(input.total)
    .bind(input.expiry_time)
    .bind(Json(input.settings.clone()))
    .bind(&input.comment)
    .bind(&input.group_name)
    .bind(input.limit_ip)
    .bind(input.limit_hwid)
    .bind(input.reset_days)
    .bind(ts)
    .bind(ts)
    .bind(ts)
    .execute(db)
    .await?;
    Ok(res.last_insert_rowid())
}

pub async fn update(db: &SqlitePool, id: i64, input: &ClientInput) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE clients SET inbound_id = ?, email = ?, enable = ?, total = ?,
         expiry_time = ?, settings = ?, comment = ?, group_name = ?, limit_ip = ?,
         limit_hwid = ?, reset_days = ?, updated_at = ? WHERE id = ?",
    )
    .bind(input.inbound_id)
    .bind(&input.email)
    .bind(input.enable)
    .bind(input.total)
    .bind(input.expiry_time)
    .bind(Json(input.settings.clone()))
    .bind(&input.comment)
    .bind(&input.group_name)
    .bind(input.limit_ip)
    .bind(input.limit_hwid)
    .bind(input.reset_days)
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete(db: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM clients WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn set_enable(db: &SqlitePool, id: i64, enable: bool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE clients SET enable = ?, updated_at = ? WHERE id = ?")
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
    last_online: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE clients SET up = up + ?, down = down + ?, last_up = ?, last_down = ?,
         last_online = ?, updated_at = ? WHERE id = ?",
    )
    .bind(up_delta)
    .bind(down_delta)
    .bind(last_up)
    .bind(last_down)
    .bind(last_online)
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

// One UPDATE per tick for idle-but-connected clients, instead of a write per
// client. Callers only pass rows whose stored timestamp is already stale.
pub async fn touch_last_online(
    db: &SqlitePool,
    ids: &[i64],
    last_online: i64,
) -> Result<(), sqlx::Error> {
    if ids.is_empty() {
        return Ok(());
    }
    // SQLite's default host-parameter limit is 999.
    for chunk in ids.chunks(500) {
        let mut builder =
            sqlx::QueryBuilder::<sqlx::Sqlite>::new("UPDATE clients SET last_online = ");
        builder.push_bind(last_online);
        builder.push(" WHERE id IN (");
        let mut separated = builder.separated(", ");
        for id in chunk {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        builder.build().execute(db).await?;
    }
    Ok(())
}

pub async fn set_traffic(
    db: &SqlitePool,
    id: i64,
    up: i64,
    down: i64,
    last_online: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE clients SET up = ?, down = ?, last_up = 0, last_down = 0,
         last_online = ?, updated_at = ? WHERE id = ?",
    )
    .bind(up)
    .bind(down)
    .bind(last_online)
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn reset_traffic(db: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE clients SET up = 0, down = 0, last_up = 0, last_down = 0,
         last_reset_time = ?, updated_at = ? WHERE id = ?",
    )
    .bind(now())
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

// Record a scheduled reset attempt without clearing traffic. Used when at
// least one attachment reset failed: the totals stay, attachments are flagged
// for reconciliation, and the next attempt waits for the next reset period.
pub async fn touch_reset_time(db: &SqlitePool, id: i64, ts: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE clients SET last_reset_time = ?, updated_at = ? WHERE id = ?")
        .bind(ts)
        .bind(ts)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn set_group(db: &SqlitePool, id: i64, group: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE clients SET group_name = ?, updated_at = ? WHERE id = ?")
        .bind(group)
        .bind(now())
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn adjust(
    db: &SqlitePool,
    id: i64,
    add_millis: i64,
    add_bytes: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE clients SET
           expiry_time = CASE WHEN expiry_time > 0 THEN expiry_time + ? ELSE 0 END,
           total = CASE WHEN total > 0 THEN MAX(0, total + ?) ELSE 0 END,
           updated_at = ?
         WHERE id = ?",
    )
    .bind(add_millis)
    .bind(add_bytes)
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_by_inbound(db: &SqlitePool, inbound_id: i64) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM clients WHERE inbound_id = ?")
        .bind(inbound_id)
        .execute(db)
        .await?;
    Ok(res.rows_affected())
}

pub async fn delete_depleted(db: &SqlitePool) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM clients WHERE total > 0 AND up + down >= total")
        .execute(db)
        .await?;
    Ok(res.rows_affected())
}
