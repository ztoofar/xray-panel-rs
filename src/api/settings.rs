use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    db,
    error::{AppError, AppResult},
    service,
    state::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsBody {
    pub panel_name: Option<String>,
    pub public_address: Option<String>,
    pub traffic_poll_secs: Option<u64>,
    pub xray_extra: Option<Value>,
}

pub async fn get(State(state): State<AppState>) -> AppResult<Json<Value>> {
    Ok(Json(read_settings(&state).await?))
}

pub async fn update(
    State(state): State<AppState>,
    Json(body): Json<SettingsBody>,
) -> AppResult<Json<Value>> {
    let mut changes: Vec<(String, String)> = Vec::new();
    let mut apply_xray = false;
    let mut refresh_poll = false;

    if let Some(name) = body.panel_name {
        let name = name.trim().to_string();
        if name.is_empty() || name.len() > 64 {
            return Err(AppError::BadRequest("面板名称必须为 1-64 个字符".into()));
        }
        changes.push(("panel_name".into(), name));
    }

    if let Some(address) = body.public_address {
        let address = address.trim().to_string();
        if address.contains("://") || address.contains('/') {
            return Err(AppError::BadRequest(
                "公开地址必须为不含协议和路径的主机名".into(),
            ));
        }
        changes.push(("public_address".into(), address));
    }

    if let Some(secs) = body.traffic_poll_secs {
        if !(2..=300).contains(&secs) {
            return Err(AppError::BadRequest("流量轮询间隔必须为 2-300 秒".into()));
        }
        changes.push(("traffic_poll_secs".into(), secs.to_string()));
        refresh_poll = true;
    }

    if let Some(extra) = body.xray_extra {
        if !extra.is_object() {
            return Err(AppError::BadRequest("xrayExtra 必须为 JSON 对象".into()));
        }
        changes.push((
            "xray_extra".into(),
            serde_json::to_string(&extra)
                .map_err(|err| AppError::BadRequest(format!("xrayExtra 无效：{err}")))?,
        ));
        apply_xray = true;
    }

    if changes.is_empty() {
        return Ok(Json(read_settings(&state).await?));
    }

    db::settings::set_many(&state.db, &changes).await?;
    if refresh_poll {
        state.refresh_notify.notify_one();
    }
    if apply_xray {
        if let Err(err) = service::apply(&state).await {
            return Err(AppError::Apply(format!(
                "设置已保存，但应用 Xray 配置失败：{err}"
            )));
        }
    }

    Ok(Json(read_settings(&state).await?))
}

async fn read_settings(state: &AppState) -> AppResult<Value> {
    let values = db::settings::get_all(&state.db).await?;
    let panel_name = values
        .get("panel_name")
        .cloned()
        .unwrap_or_else(|| "Xray Panel".into());
    let public_address = values.get("public_address").cloned().unwrap_or_default();
    let traffic_poll_secs = values
        .get("traffic_poll_secs")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5);
    let xray_extra = values
        .get("xray_extra")
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .unwrap_or_else(|| json!({}));

    Ok(json!({
        "panelName": panel_name,
        "publicAddress": public_address,
        "trafficPollSecs": traffic_poll_secs,
        "xrayExtra": xray_extra,
        "panelListen": state.cfg.listen.to_string(),
    }))
}
