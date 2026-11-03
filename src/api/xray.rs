use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    service,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    pub limit: Option<usize>,
}

pub async fn status(State(state): State<AppState>) -> Json<Value> {
    let status = state.xray.status().await;
    Json(json!({
        "status": status,
        "configPath": state.cfg.xray_config,
        "apiAddress": state.cfg.xray_api_addr,
    }))
}

pub async fn config(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let config = service::config_value(&state)
        .await
        .map_err(|err| AppError::BadRequest(format!("cannot build xray config: {err}")))?;
    Ok(Json(config))
}

pub async fn test(State(state): State<AppState>) -> AppResult<Json<Value>> {
    service::test_current(&state)
        .await
        .map_err(|err| AppError::BadRequest(format!("xray config test failed: {err}")))?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn restart(State(state): State<AppState>) -> AppResult<Json<Value>> {
    service::apply(&state)
        .await
        .map_err(|err| AppError::Apply(format!("restart failed: {err}")))?;
    let status = state.xray.status().await;
    Ok(Json(json!({ "ok": true, "status": status })))
}

pub async fn new_x25519(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        tokio::process::Command::new(&state.cfg.xray_bin)
            .arg("x25519")
            .output(),
    )
    .await
    .map_err(|_| AppError::Apply("xray x25519 timed out".into()))?
    .map_err(|err| AppError::Apply(format!("xray x25519 failed: {err}")))?;
    if !output.status.success() {
        return Err(AppError::Apply(format!(
            "xray x25519 failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let private = text
        .lines()
        .find(|line| line.to_ascii_lowercase().contains("private"))
        .and_then(|line| {
            line.split_once(':')
                .map(|(_, value)| value.trim().to_string())
        });
    let public = text
        .lines()
        .find(|line| line.to_ascii_lowercase().contains("public"))
        .and_then(|line| {
            line.split_once(':')
                .map(|(_, value)| value.trim().to_string())
        });
    match (private, public) {
        (Some(private_key), Some(public_key)) => Ok(Json(
            json!({ "privateKey": private_key, "publicKey": public_key }),
        )),
        _ => Err(AppError::Apply("could not parse xray x25519 output".into())),
    }
}

pub async fn logs(State(state): State<AppState>, Query(query): Query<LogQuery>) -> Json<Value> {
    let limit = query.limit.unwrap_or(200).clamp(1, 500);
    let buffer = state.xray_logs.lock().await;
    let skip = buffer.len().saturating_sub(limit);
    let lines = buffer.iter().skip(skip).cloned().collect::<Vec<_>>();
    Json(json!({ "lines": lines }))
}
