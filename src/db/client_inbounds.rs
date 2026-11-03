use std::collections::HashMap;

use sqlx::SqlitePool;

pub async fn all(db: &SqlitePool) -> Result<HashMap<i64, Vec<i64>>, sqlx::Error> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT client_id, inbound_id FROM client_inbounds ORDER BY client_id, inbound_id",
    )
    .fetch_all(db)
    .await?;
    let mut map: HashMap<i64, Vec<i64>> = HashMap::new();
    for (client_id, inbound_id) in rows {
        map.entry(client_id).or_default().push(inbound_id);
    }
    Ok(map)
}

pub async fn ids_for_client(db: &SqlitePool, client_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT inbound_id FROM client_inbounds WHERE client_id = ? ORDER BY inbound_id",
    )
    .bind(client_id)
    .fetch_all(db)
    .await
}

pub async fn set_for_client(
    db: &SqlitePool,
    client_id: i64,
    inbound_ids: &[i64],
) -> Result<(), sqlx::Error> {
    let mut transaction = db.begin().await?;
    sqlx::query("DELETE FROM client_inbounds WHERE client_id = ?")
        .bind(client_id)
        .execute(&mut *transaction)
        .await?;
    for inbound_id in inbound_ids {
        sqlx::query("INSERT OR IGNORE INTO client_inbounds (client_id, inbound_id) VALUES (?, ?)")
            .bind(client_id)
            .bind(inbound_id)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn attach(db: &SqlitePool, client_id: i64, inbound_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO client_inbounds (client_id, inbound_id) VALUES (?, ?)")
        .bind(client_id)
        .bind(inbound_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn detach(db: &SqlitePool, client_id: i64, inbound_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM client_inbounds WHERE client_id = ? AND inbound_id = ?")
        .bind(client_id)
        .bind(inbound_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn detach_inbound(db: &SqlitePool, inbound_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM client_inbounds WHERE inbound_id = ?")
        .bind(inbound_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_orphans(db: &SqlitePool) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM clients WHERE id NOT IN (SELECT client_id FROM client_inbounds)")
            .execute(db)
            .await?;
    Ok(result.rows_affected())
}
