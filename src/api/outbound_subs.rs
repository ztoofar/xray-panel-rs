use std::collections::HashMap;

use axum::{
    extract::{Path, Request, State},
    Json,
};
use serde_json::Value;

use crate::{
    api::compat::{fail, ok, read_params},
    outbound_subs::{self, SubscriptionInput},
    state::AppState,
};

pub async fn list(State(state): State<AppState>) -> Json<Value> {
    match outbound_subs::list_views(&state).await {
        Ok(items) => ok(serde_json::json!(items)),
        Err(err) => fail(format!("failed to list outbound subscriptions: {err:#}")),
    }
}

pub async fn create(State(state): State<AppState>, req: Request) -> Json<Value> {
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    match outbound_subs::create(&state, input_from_params(&params)).await {
        Ok(sub) => ok(serde_json::json!(outbound_subs::view(&sub))),
        Err(err) => fail(format!("failed to create outbound subscription: {err:#}")),
    }
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    req: Request,
) -> Json<Value> {
    let Some(id) = parse_id(&id) else {
        return fail("invalid subscription id");
    };
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    match outbound_subs::update(&state, id, input_from_params(&params)).await {
        Ok(()) => {
            outbound_subs::apply_after_change(&state).await;
            ok(Value::Null)
        }
        Err(err) => fail(format!("failed to update outbound subscription: {err:#}")),
    }
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    let Some(id) = parse_id(&id) else {
        return fail("invalid subscription id");
    };
    match outbound_subs::delete(&state, id).await {
        Ok(()) => ok(Value::Null),
        Err(err) => fail(format!("failed to delete outbound subscription: {err:#}")),
    }
}

pub async fn move_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    req: Request,
) -> Json<Value> {
    let Some(id) = parse_id(&id) else {
        return fail("invalid subscription id");
    };
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    let up = params
        .get("dir")
        .map(|value| value.trim() == "up")
        .unwrap_or(false);
    match outbound_subs::move_item(&state, id, up).await {
        Ok(()) => ok(Value::Null),
        Err(err) => fail(format!("failed to reorder outbound subscription: {err:#}")),
    }
}

pub async fn refresh(State(state): State<AppState>, Path(id): Path<String>) -> Json<Value> {
    let Some(id) = parse_id(&id) else {
        return fail("invalid subscription id");
    };
    match outbound_subs::refresh_one(&state, id, true).await {
        Ok(outbounds) => ok(Value::Array(outbounds)),
        Err(err) => fail(format!("refresh failed: {err:#}")),
    }
}

pub async fn parse_preview(State(state): State<AppState>, req: Request) -> Json<Value> {
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    let url = params.get("url").map(String::as_str).unwrap_or("");
    if url.trim().is_empty() {
        return fail("url is required");
    }
    match outbound_subs::preview(
        &state,
        url,
        params.get("userAgent").map(String::as_str).unwrap_or(""),
        params.get("proxyUrl").map(String::as_str).unwrap_or(""),
        bool_param(&params, "allowPrivate", false),
        bool_param(&params, "allowInsecure", false),
    )
    .await
    {
        Ok(outbounds) => ok(Value::Array(outbounds)),
        Err(err) => fail(format!("failed to fetch/parse subscription: {err:#}")),
    }
}

fn parse_id(raw: &str) -> Option<i64> {
    raw.trim().parse().ok()
}

fn input_from_params(params: &HashMap<String, String>) -> SubscriptionInput {
    SubscriptionInput {
        remark: params.get("remark").cloned().unwrap_or_default(),
        url: params.get("url").cloned().unwrap_or_default(),
        tag_prefix: params.get("tagPrefix").cloned().unwrap_or_default(),
        user_agent: params.get("userAgent").cloned().unwrap_or_default(),
        proxy_url: params.get("proxyUrl").cloned().unwrap_or_default(),
        update_interval: params
            .get("updateInterval")
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(600),
        enabled: bool_param(params, "enabled", true),
        allow_private: bool_param(params, "allowPrivate", false),
        allow_insecure: bool_param(params, "allowInsecure", false),
        prepend: bool_param(params, "prepend", false),
    }
}

fn bool_param(params: &HashMap<String, String>, key: &str, default: bool) -> bool {
    match params
        .get(key)
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("true") | Some("1") | Some("on") | Some("yes") => true,
        Some("false") | Some("0") | Some("off") | Some("no") => false,
        _ => default,
    }
}
