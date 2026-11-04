use std::{collections::HashMap, net::IpAddr};

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    auth::AuthCtx,
    db::{self, inbounds::InboundInput, models::Inbound},
    error::{AppError, AppResult},
    service,
    state::AppState,
    xray::link::client_link,
};

const RESETS: [&str; 5] = ["never", "hourly", "daily", "weekly", "monthly"];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboundBody {
    pub remark: Option<String>,
    pub enable: Option<bool>,
    pub expiry_time: Option<i64>,
    pub total: Option<i64>,
    pub traffic_reset: Option<String>,
    pub traffic_reset_day: Option<i64>,
    pub listen: Option<String>,
    pub port: i64,
    pub protocol: String,
    pub settings: Option<Value>,
    pub stream_settings: Option<Value>,
    pub sniffing: Option<Value>,
    pub tag: Option<String>,
    pub sub_sort_index: Option<i64>,
    pub exclude_from_sub: Option<bool>,
    pub share_addr_strategy: Option<String>,
    pub share_addr: Option<String>,
    pub disable_flow: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ToggleBody {
    pub enable: bool,
}

pub async fn list(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let inbounds = db::inbounds::list(&state.db).await?;
    let clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    let online = state.online_clients.lock().await.clone();
    // Exact per-attachment tags when the online API reported them; otherwise
    // fall back to the inbound-level activity window, like the frontend.
    let active_tags: std::collections::HashSet<String> =
        state.active_inbounds.lock().await.iter().cloned().collect();
    let counts: HashMap<i64, i64> = db::inbounds::client_counts(&state.db)
        .await?
        .into_iter()
        .map(|count| (count.inbound_id, count.clients))
        .collect();

    let items = inbounds
        .into_iter()
        .map(|inbound| {
            let online_clients: Vec<_> = clients
                .iter()
                .filter(|client| {
                    associations
                        .get(&client.id)
                        .map(|ids| ids.contains(&inbound.id))
                        .unwrap_or(client.inbound_id == inbound.id)
                })
                .filter(|client| {
                    let Some(user) = online.get(&client.email) else {
                        return false;
                    };
                    if user.inbounds.is_empty() {
                        active_tags.contains(&inbound.tag)
                    } else {
                        user.inbounds.iter().any(|tag| tag == &inbound.tag)
                    }
                })
                .filter_map(|client| online.get(&client.email).cloned())
                .collect();
            let mut value = serde_json::to_value(&inbound).unwrap_or_else(|_| json!({}));
            value["clientCount"] = json!(counts.get(&inbound.id).copied().unwrap_or(0));
            value["onlineCount"] = json!(online_clients.len());
            value["onlineClients"] = json!(online_clients);
            value
        })
        .collect::<Vec<_>>();

    Ok(Json(Value::Array(items)))
}

pub async fn get(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let inbound = db::inbounds::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("入站不存在".into()))?;
    Ok(Json(
        serde_json::to_value(&inbound).unwrap_or_else(|_| json!({})),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthCtx>,
    Json(body): Json<InboundBody>,
) -> AppResult<Json<Value>> {
    let mut input = build_input(&state, &body, ctx.user_id, None).await?;
    let seed_settings = body.settings.clone().unwrap_or_else(|| json!({}));
    if let Some(object) = input.settings.as_object_mut() {
        object.remove("clients");
        object.remove("accounts");
    }
    let id = db::inbounds::create(&state.db, &input).await?;
    crate::api::compat::sync_clients_from_settings(
        &state,
        id,
        &input.protocol,
        &seed_settings,
        true,
    )
    .await?;
    if let Err(err) = service::apply(&state).await {
        return Err(AppError::Apply(format!(
            "入站已保存，但应用 Xray 配置失败：{err}"
        )));
    }
    let inbound = db::inbounds::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("入站不存在".into()))?;
    Ok(Json(
        serde_json::to_value(&inbound).unwrap_or_else(|_| json!({})),
    ))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthCtx>,
    Path(id): Path<i64>,
    Json(body): Json<InboundBody>,
) -> AppResult<Json<Value>> {
    let existing = db::inbounds::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("入站不存在".into()))?;
    let mut input = build_input(&state, &body, ctx.user_id, Some(&existing)).await?;
    let seed_settings = body.settings.clone().unwrap_or_else(|| json!({}));
    let has_clients = seed_settings
        .get("clients")
        .and_then(Value::as_array)
        .is_some()
        || seed_settings
            .get("accounts")
            .and_then(Value::as_array)
            .is_some();
    if let Some(object) = input.settings.as_object_mut() {
        object.remove("clients");
        object.remove("accounts");
    }
    db::inbounds::update(&state.db, id, &input).await?;
    if has_clients {
        crate::api::compat::sync_clients_from_settings(
            &state,
            id,
            &input.protocol,
            &seed_settings,
            true,
        )
        .await?;
    }
    if let Err(err) = service::apply(&state).await {
        return Err(AppError::Apply(format!(
            "入站已保存，但应用 Xray 配置失败：{err}"
        )));
    }
    let inbound = db::inbounds::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("入站不存在".into()))?;
    Ok(Json(
        serde_json::to_value(&inbound).unwrap_or_else(|_| json!({})),
    ))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let removed = db::inbounds::delete(&state.db, id).await?;
    if !removed {
        return Err(AppError::NotFound("入站不存在".into()));
    }
    db::client_inbounds::delete_orphans(&state.db).await?;
    if let Err(err) = service::apply(&state).await {
        return Err(AppError::Apply(format!(
            "入站已删除，但应用 Xray 配置失败：{err}"
        )));
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn toggle(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<ToggleBody>,
) -> AppResult<Json<Value>> {
    if db::inbounds::get(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound("入站不存在".into()));
    }
    db::inbounds::set_enable(&state.db, id, body.enable).await?;
    if let Err(err) = service::apply(&state).await {
        return Err(AppError::Apply(format!(
            "入站已更新，但应用 Xray 配置失败：{err}"
        )));
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn links(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let inbound = db::inbounds::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("入站不存在".into()))?;
    let address = public_address(&state, &headers).await?;
    let clients = db::clients::list_by_inbound(&state.db, id).await?;

    let items = clients
        .iter()
        .filter_map(|client| match client_link(&inbound, client, &address) {
            Ok(Some(link)) => Some(json!({
                "clientId": client.id,
                "email": client.email,
                "link": link,
            })),
            Ok(None) => None,
            Err(err) => Some(json!({
                "clientId": client.id,
                "email": client.email,
                "error": err.to_string(),
            })),
        })
        .collect::<Vec<_>>();

    Ok(Json(json!({ "address": address, "items": items })))
}

async fn build_input(
    state: &AppState,
    body: &InboundBody,
    user_id: i64,
    existing: Option<&Inbound>,
) -> AppResult<InboundInput> {
    let protocol = body.protocol.trim().to_ascii_lowercase();
    if !crate::xray::config::is_xray_inbound(&protocol) {
        return Err(AppError::BadRequest(format!("不支持的协议：{protocol}")));
    }
    if !(1..=65535).contains(&body.port) {
        return Err(AppError::BadRequest("端口必须为 1-65535".into()));
    }

    let listen = body.listen.clone().unwrap_or_else(|| {
        existing
            .map(|value| value.listen.clone())
            .unwrap_or_default()
    });
    if !listen.is_empty() && listen.parse::<IpAddr>().is_err() {
        return Err(AppError::BadRequest("监听地址必须为 IP 地址或留空".into()));
    }

    let traffic_reset = body.traffic_reset.clone().unwrap_or_else(|| {
        existing
            .map(|value| value.traffic_reset.clone())
            .unwrap_or_else(|| "never".into())
    });
    if !RESETS.contains(&traffic_reset.as_str()) {
        return Err(AppError::BadRequest("流量重置计划无效".into()));
    }
    let traffic_reset_day = body
        .traffic_reset_day
        .unwrap_or_else(|| existing.map(|value| value.traffic_reset_day).unwrap_or(1));
    if !(1..=31).contains(&traffic_reset_day) {
        return Err(AppError::BadRequest("流量重置日必须为 1-31".into()));
    }

    let tag = body
        .tag
        .clone()
        .filter(|tag| !tag.trim().is_empty())
        .unwrap_or_else(|| {
            existing
                .map(|value| value.tag.clone())
                .unwrap_or_else(|| format!("in-{protocol}-{}", body.port))
        });
    if tag == "api" {
        return Err(AppError::BadRequest("标签 \"api\" 为保留名称".into()));
    }
    if db::inbounds::tag_exists(&state.db, &tag, existing.map(|value| value.id)).await? {
        return Err(AppError::BadRequest(format!("标签已存在：{tag}")));
    }

    let settings = json_object(
        body.settings
            .clone()
            .or_else(|| existing.map(|value| value.settings.0.clone()))
            .unwrap_or_else(|| json!({})),
        "settings",
    )?;
    let stream_settings = json_object(
        body.stream_settings
            .clone()
            .or_else(|| existing.map(|value| value.stream_settings.0.clone()))
            .unwrap_or_else(|| json!({ "network": "tcp" })),
        "streamSettings",
    )?;
    let sniffing = json_object(
        body.sniffing
            .clone()
            .or_else(|| existing.map(|value| value.sniffing.0.clone()))
            .unwrap_or_else(|| json!({})),
        "sniffing",
    )?;

    let share_addr_strategy = body
        .share_addr_strategy
        .clone()
        .or_else(|| existing.map(|value| value.share_addr_strategy.clone()))
        .unwrap_or_else(|| "node".into());
    if !matches!(share_addr_strategy.as_str(), "node" | "listen" | "custom") {
        return Err(AppError::BadRequest("分享地址策略无效".into()));
    }

    Ok(InboundInput {
        user_id: existing.map(|value| value.user_id).unwrap_or(user_id),
        remark: body.remark.clone().unwrap_or_else(|| {
            existing
                .map(|value| value.remark.clone())
                .unwrap_or_default()
        }),
        sub_sort_index: body
            .sub_sort_index
            .filter(|value| *value != 0)
            .unwrap_or_else(|| existing.map(|value| value.sub_sort_index).unwrap_or(1)),
        exclude_from_sub: body.exclude_from_sub.unwrap_or_else(|| {
            existing
                .map(|value| value.exclude_from_sub)
                .unwrap_or(false)
        }),
        enable: body
            .enable
            .unwrap_or_else(|| existing.map(|value| value.enable).unwrap_or(true)),
        expiry_time: body
            .expiry_time
            .unwrap_or_else(|| existing.map(|value| value.expiry_time).unwrap_or(0)),
        total: body
            .total
            .unwrap_or_else(|| existing.map(|value| value.total).unwrap_or(0)),
        traffic_reset,
        traffic_reset_day,
        listen,
        port: body.port,
        protocol,
        settings,
        stream_settings,
        sniffing,
        tag,
        share_addr_strategy,
        share_addr: body
            .share_addr
            .clone()
            .or_else(|| existing.map(|value| value.share_addr.clone()))
            .unwrap_or_default(),
        disable_flow: body
            .disable_flow
            .unwrap_or_else(|| existing.map(|value| value.disable_flow).unwrap_or(false)),
    })
}

fn json_object(value: Value, field: &str) -> AppResult<Value> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!("{field} 必须为 JSON 对象")))
    }
}

pub(crate) async fn public_address(state: &AppState, headers: &HeaderMap) -> AppResult<String> {
    // Share/QR links must use the admin-configured node address, never the
    // browser's LAN address or the reverse proxy's Host header.
    if let Some(address) = configured_node_address(state).await? {
        return Ok(address);
    }
    if let Some(address) = db::settings::get(&state.db, "public_address").await? {
        let address = address.trim();
        if !address.is_empty() {
            return Ok(address.to_string());
        }
    }
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .trim();
    let host = strip_port(host);
    if host.is_empty() {
        return Err(AppError::BadRequest("尚未配置公开地址".into()));
    }
    Ok(host)
}

// The 3x-ui settings JSON keeps "nodeAddress"; older rows may only have
// "webDomain", which served the same purpose in the public host fallback.
async fn configured_node_address(state: &AppState) -> AppResult<Option<String>> {
    let Some(raw) = db::settings::get(&state.db, "x3ui_settings").await? else {
        return Ok(None);
    };
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(&raw) else {
        return Ok(None);
    };
    // An explicitly empty nodeAddress means "no configured host"; do not fall
    // back to a stale webDomain the admin cleared.
    if let Some(value) = object.get("nodeAddress").and_then(Value::as_str) {
        let address = value.trim();
        return Ok((!address.is_empty()).then(|| address.to_string()));
    }
    if let Some(address) = object
        .get("webDomain")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(Some(address.to_string()));
    }
    Ok(None)
}

fn strip_port(host: &str) -> String {
    if host.starts_with('[') {
        let end = host.find(']').map(|index| index + 1).unwrap_or(host.len());
        host[..end].to_string()
    } else {
        host.split(':').next().unwrap_or(host).to_string()
    }
}
