use super::models::SessionUser;
use sqlx::SqlitePool;

pub async fn create(
    db: &SqlitePool,
    token_hash: &str,
    user_id: i64,
    csrf: &str,
    created_at: i64,
    expires_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, csrf, created_at, expires_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(token_hash)
    .bind(user_id)
    .bind(csrf)
    .bind(created_at)
    .bind(expires_at)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn find(db: &SqlitePool, token_hash: &str) -> Result<Option<SessionUser>, sqlx::Error> {
    sqlx::query_as::<_, SessionUser>(
        "SELECT s.user_id, s.csrf, s.expires_at, u.username
         FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ?",
    )
    .bind(token_hash)
    .fetch_optional(db)
    .await
}

pub async fn delete(db: &SqlitePool, token_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(token_hash)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_for_user(db: &SqlitePool, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn purge_expired(db: &SqlitePool, before: i64) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(before)
        .execute(db)
        .await?;
    Ok(res.rows_affected())
}
