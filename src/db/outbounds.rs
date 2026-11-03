use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use super::models::{now, OutboundTraffic};

pub async fn list_traffic(db: &SqlitePool) -> Result<Vec<OutboundTraffic>, sqlx::Error> {
    sqlx::query_as::<_, OutboundTraffic>("SELECT * FROM outbound_traffics ORDER BY tag")
        .fetch_all(db)
        .await
}

pub async fn upsert_traffic(
    db: &SqlitePool,
    tag: &str,
    up_delta: i64,
    down_delta: i64,
    last_up: i64,
    last_down: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO outbound_traffics (tag, up, down, last_up, last_down, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(tag) DO UPDATE SET
           up = up + excluded.up,
           down = down + excluded.down,
           last_up = excluded.last_up,
           last_down = excluded.last_down,
           updated_at = excluded.updated_at",
    )
    .bind(tag)
    .bind(up_delta)
    .bind(down_delta)
    .bind(last_up)
    .bind(last_down)
    .bind(now())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn reset_traffic(db: &SqlitePool, tag: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE outbound_traffics SET up = 0, down = 0, last_up = 0, last_down = 0,
         updated_at = ? WHERE tag = ?",
    )
    .bind(now())
    .bind(tag)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn reset_all(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE outbound_traffics SET up = 0, down = 0, last_up = 0, last_down = 0,
         updated_at = ?",
    )
    .bind(now())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn retain_tags(db: &SqlitePool, tags: &[String]) -> Result<(), sqlx::Error> {
    if tags.is_empty() {
        sqlx::query("DELETE FROM outbound_traffics")
            .execute(db)
            .await?;
        return Ok(());
    }
    let mut query = QueryBuilder::<Sqlite>::new("DELETE FROM outbound_traffics WHERE tag NOT IN (");
    let mut separated = query.separated(", ");
    for tag in tags {
        separated.push_bind(tag);
    }
    query.push(")");
    query.build().execute(db).await?;
    Ok(())
}
