use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::{error::AppResult, state::AppState, sys};

pub async fn dashboard(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let status = state.xray.status().await;
    let system = sys::snapshot();

    let (inbounds, clients, enabled_clients): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM inbounds),
                (SELECT COUNT(*) FROM clients),
                (SELECT COUNT(*) FROM clients WHERE enable = 1)",
    )
    .fetch_one(&state.db)
    .await?;

    let online_clients = state.online_clients.lock().await.len();
    let (up, down): (i64, i64) =
        sqlx::query_as("SELECT COALESCE(SUM(up), 0), COALESCE(SUM(down), 0) FROM inbounds")
            .fetch_one(&state.db)
            .await?;

    let history = state
        .traffic_history
        .lock()
        .await
        .iter()
        .cloned()
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "xray": status,
        "system": system,
        "inbounds": inbounds,
        "clients": clients,
        "enabledClients": enabled_clients,
        "onlineClients": online_clients,
        "up": up,
        "down": down,
        "history": history,
        "startedAt": state.started_at,
        "version": env!("CARGO_PKG_VERSION"),
    })))
}
