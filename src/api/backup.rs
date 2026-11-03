use std::{collections::HashSet, path::Path};

use axum::{
    extract::{Multipart, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};

use crate::{
    db::models::now,
    error::{AppError, AppResult},
    service,
    state::AppState,
};

const REQUIRED_TABLES: [&str; 4] = ["users", "settings", "inbounds", "clients"];
const COPY_DELETE_ORDER: [&str; 9] = [
    "client_inbound_traffic",
    "outbound_subscriptions",
    "outbound_traffics",
    "sessions",
    "clients",
    "inbounds",
    "api_tokens",
    "users",
    "settings",
];
const COPY_INSERT_ORDER: [&str; 9] = [
    "users",
    "settings",
    "api_tokens",
    "inbounds",
    "clients",
    "client_inbound_traffic",
    "outbound_subscriptions",
    "sessions",
    "outbound_traffics",
];

pub async fn download(State(state): State<AppState>) -> AppResult<Response> {
    let temp = std::env::temp_dir().join(format!("xray-panel-backup-{}.db", now()));
    let _ = tokio::fs::remove_file(&temp).await;
    let escaped = temp.to_string_lossy().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{escaped}'"))
        .execute(&state.db)
        .await
        .map_err(|err| AppError::Internal(anyhow::anyhow!("create backup: {err}")))?;
    let bytes = tokio::fs::read(&temp)
        .await
        .map_err(|err| AppError::Internal(anyhow::anyhow!("read backup: {err}")))?;
    let _ = tokio::fs::remove_file(&temp).await;

    let filename = format!(
        "xray-panel-{}.db",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let mut response = (StatusCode::OK, bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );
    Ok(response)
}

pub async fn restore(
    State(state): State<AppState>,
    multipart: Multipart,
) -> AppResult<Json<Value>> {
    let value = restore_multipart(&state, multipart).await?;
    Ok(Json(value))
}

pub async fn compat_import(State(state): State<AppState>, multipart: Multipart) -> Json<Value> {
    match restore_multipart(&state, multipart).await {
        Ok(value) => Json(json!({ "success": true, "msg": "", "obj": value })),
        Err(err) => Json(json!({ "success": false, "msg": err.to_string(), "obj": null })),
    }
}

async fn restore_multipart(state: &AppState, mut multipart: Multipart) -> AppResult<Value> {
    let mut uploaded: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::BadRequest(format!("read upload: {err}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        if matches!(name.as_str(), "db" | "file" | "database") {
            let bytes = field
                .bytes()
                .await
                .map_err(|err| AppError::BadRequest(format!("read upload: {err}")))?;
            uploaded = Some(bytes.to_vec());
            break;
        }
    }
    let data = uploaded.ok_or_else(|| AppError::BadRequest("missing .db file field".into()))?;
    if data.len() < 16 || &data[..16] != b"SQLite format 3\0" {
        return Err(AppError::BadRequest(
            "uploaded file is not a SQLite database".into(),
        ));
    }

    let temp = std::env::temp_dir().join(format!("xray-panel-restore-{}.db", now()));
    tokio::fs::write(&temp, &data)
        .await
        .map_err(|err| AppError::Internal(anyhow::anyhow!("write restore file: {err}")))?;

    let tables = match validate_db(&temp).await {
        Ok(tables) => tables,
        Err(err) => {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(AppError::BadRequest(err));
        }
    };
    let result = restore_into(state, &temp, &tables).await;
    let _ = tokio::fs::remove_file(&temp).await;
    result?;

    state.online_clients.lock().await.clear();
    state.online_last_seen.lock().await.clear();
    state.active_inbound_last_seen.lock().await.clear();
    state.active_inbounds.lock().await.clear();
    let apply_error = service::apply(state).await.err().map(|err| err.to_string());
    Ok(json!({
        "ok": true,
        "applied": apply_error.is_none(),
        "warning": apply_error,
    }))
}

async fn validate_db(path: &Path) -> Result<HashSet<String>, String> {
    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|err| format!("open backup: {err}"))?;
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table'")
            .fetch_all(&mut connection)
            .await
            .map_err(|err| format!("read backup schema: {err}"))?;
    let tables: HashSet<String> = tables.into_iter().collect();
    for required in REQUIRED_TABLES {
        if !tables.contains(required) {
            let _ = connection.close().await;
            return Err(format!("backup is missing table: {required}"));
        }
    }
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut connection)
        .await
        .map_err(|err| format!("check backup integrity: {err}"))?;
    let _ = connection.close().await;
    if integrity != "ok" {
        return Err(format!("backup integrity check failed: {integrity}"));
    }
    Ok(tables)
}

async fn restore_into(state: &AppState, path: &Path, tables: &HashSet<String>) -> AppResult<()> {
    let mut connection = state.db.acquire().await?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await?;
    sqlx::query("ATTACH DATABASE ? AS restore")
        .bind(path.to_string_lossy().to_string())
        .execute(&mut *connection)
        .await
        .map_err(|err| AppError::BadRequest(format!("attach backup: {err}")))?;

    let outcome: Result<(), sqlx::Error> = async {
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *connection)
            .await?;
        for table in COPY_DELETE_ORDER {
            if tables.contains(table) {
                sqlx::query(&format!("DELETE FROM main.\"{table}\""))
                    .execute(&mut *connection)
                    .await?;
            }
        }
        for table in COPY_INSERT_ORDER {
            if tables.contains(table) {
                copy_shared_columns(&mut connection, table).await?;
            }
        }
        // Backups made before 0.2.7 have no attachment baselines. The delete
        // loop skipped the table, so drop the current rows explicitly instead
        // of leaving baselines that may refer to a different dataset.
        if !tables.contains("client_inbound_traffic") {
            sqlx::query("DELETE FROM main.\"client_inbound_traffic\"")
                .execute(&mut *connection)
                .await?;
        }
        // Same for pre-0.2.12 backups: no subscription table means the backup
        // has none, so retain nothing from the current database.
        if !tables.contains("outbound_subscriptions") {
            sqlx::query("DELETE FROM main.\"outbound_subscriptions\"")
                .execute(&mut *connection)
                .await?;
        }
        sqlx::query("COMMIT").execute(&mut *connection).await?;
        Ok(())
    }
    .await;

    if outcome.is_err() {
        let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
    }
    let _ = sqlx::query("DETACH DATABASE restore")
        .execute(&mut *connection)
        .await;
    let _ = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await;

    outcome.map_err(|err| AppError::BadRequest(format!("restore failed: {err}")))
}

// Restoring a backup made by an older schema must not fail just because this
// build added columns: copy the intersection and let SQLite apply defaults.
async fn copy_shared_columns(
    connection: &mut SqliteConnection,
    table: &str,
) -> Result<(), sqlx::Error> {
    let main = table_columns(connection, "main", table).await?;
    let backup: HashSet<String> = table_columns(connection, "restore", table)
        .await?
        .into_iter()
        .collect();
    let shared: Vec<String> = main
        .into_iter()
        .filter(|column| backup.contains(column))
        .collect();
    if shared.is_empty() {
        return Ok(());
    }
    let list = shared
        .iter()
        .map(|column| format!("\"{}\"", column.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(", ");
    sqlx::query(&format!(
        "INSERT INTO main.\"{table}\" ({list}) SELECT {list} FROM restore.\"{table}\""
    ))
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn table_columns(
    connection: &mut SqliteConnection,
    schema: &str,
    table: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT name FROM pragma_table_info('{table}', '{schema}')"
    ))
    .fetch_all(&mut *connection)
    .await
}
