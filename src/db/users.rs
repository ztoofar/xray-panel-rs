use super::models::{now, User};
use sqlx::SqlitePool;

pub async fn count(db: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(db)
        .await
}

pub async fn create(
    db: &SqlitePool,
    username: &str,
    password_hash: &str,
) -> Result<i64, sqlx::Error> {
    let res =
        sqlx::query("INSERT INTO users (username, password_hash, created_at) VALUES (?, ?, ?)")
            .bind(username)
            .bind(password_hash)
            .bind(now())
            .execute(db)
            .await?;
    Ok(res.last_insert_rowid())
}

pub async fn find_by_username(
    db: &SqlitePool,
    username: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT id, username, password_hash FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(db)
        .await
}

pub async fn get(db: &SqlitePool, id: i64) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT id, username, password_hash FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn update(
    db: &SqlitePool,
    id: i64,
    username: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET username = ?, password_hash = ? WHERE id = ?")
        .bind(username)
        .bind(password_hash)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}
