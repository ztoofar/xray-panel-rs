use std::collections::HashSet;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    db,
    error::{AppError, AppResult},
    outbound_subs, service,
    state::AppState,
};

// Policy is editable (connection limits, statistics switches); the panel only
// injects statsUserOnline at generation time.
const RESERVED_KEYS: [&str; 3] = ["inbounds", "stats", "api"];

#[derive(Debug, Deserialize)]
pub struct OutboundsBody {
    pub outbounds: Value,
}

#[derive(Debug, Deserialize)]
pub struct RoutingBody {
    pub routing: Value,
}

#[derive(Debug, Deserialize)]
pub struct DnsBody {
    pub dns: Value,
}

#[derive(Debug, Deserialize)]
pub struct LogBody {
    pub log: Value,
}

#[derive(Debug, Deserialize)]
pub struct TemplateBody {
    pub template: Value,
}

pub async fn template(State(state): State<AppState>) -> AppResult<Json<Value>> {
    Ok(Json(template_value(&state).await?))
}

pub async fn put_outbounds(
    State(state): State<AppState>,
    Json(body): Json<OutboundsBody>,
) -> AppResult<Json<Value>> {
    validate_outbounds(&body.outbounds)?;
    let mut extra = load_extra(&state).await?;
    extra
        .as_object_mut()
        .expect("checked object")
        .insert("outbounds".into(), body.outbounds);
    commit_extra(&state, &extra).await?;
    let mut tags = tags_of(&extra, "outbounds");
    tags.extend(outbound_subs::active_tags(&state).await?);
    db::outbounds::retain_tags(&state.db, &tags).await?;
    Ok(Json(template_value(&state).await?))
}

pub async fn put_routing(
    State(state): State<AppState>,
    Json(body): Json<RoutingBody>,
) -> AppResult<Json<Value>> {
    let extra = load_extra(&state).await?;
    let mut outbound_tags = tags_of(&extra, "outbounds");
    if outbound_tags.is_empty() {
        outbound_tags = default_tags();
    }
    outbound_tags.extend(outbound_subs::active_tags(&state).await?);
    validate_routing(&body.routing, &outbound_tags)?;
    let mut extra = extra;
    extra
        .as_object_mut()
        .expect("checked object")
        .insert("routing".into(), body.routing);
    commit_extra(&state, &extra).await?;
    Ok(Json(template_value(&state).await?))
}

pub async fn put_dns(
    State(state): State<AppState>,
    Json(body): Json<DnsBody>,
) -> AppResult<Json<Value>> {
    if !body.dns.is_object() {
        return Err(AppError::BadRequest("dns must be a JSON object".into()));
    }
    let mut extra = load_extra(&state).await?;
    extra
        .as_object_mut()
        .expect("checked object")
        .insert("dns".into(), body.dns);
    commit_extra(&state, &extra).await?;
    Ok(Json(template_value(&state).await?))
}

pub async fn put_log(
    State(state): State<AppState>,
    Json(body): Json<LogBody>,
) -> AppResult<Json<Value>> {
    if !body.log.is_object() {
        return Err(AppError::BadRequest("log must be a JSON object".into()));
    }
    let mut extra = load_extra(&state).await?;
    extra
        .as_object_mut()
        .expect("checked object")
        .insert("log".into(), body.log);
    commit_extra(&state, &extra).await?;
    Ok(Json(template_value(&state).await?))
}

pub async fn put_template(
    State(state): State<AppState>,
    Json(body): Json<TemplateBody>,
) -> AppResult<Json<Value>> {
    let template = body.template;
    let object = template
        .as_object()
        .ok_or_else(|| AppError::BadRequest("template must be a JSON object".into()))?;
    for key in RESERVED_KEYS {
        if object.contains_key(key) {
            return Err(AppError::BadRequest(format!(
                "\"{key}\" is managed by the panel and cannot appear in the template"
            )));
        }
    }
    if let Some(outbounds) = template.get("outbounds") {
        validate_outbounds(outbounds)?;
    }
    if let Some(routing) = template.get("routing") {
        let mut tags = tags_of(&template, "outbounds");
        tags.extend(default_tags());
        tags.extend(outbound_subs::active_tags(&state).await?);
        validate_routing(routing, &tags)?;
    }
    commit_extra(&state, &template).await?;
    if template.get("outbounds").is_some() {
        let mut tags = tags_of(&template, "outbounds");
        tags.extend(outbound_subs::active_tags(&state).await?);
        db::outbounds::retain_tags(&state.db, &tags).await?;
    }
    Ok(Json(template_value(&state).await?))
}

pub async fn reset_outbound(
    State(state): State<AppState>,
    Path(tag): Path<String>,
) -> AppResult<Json<Value>> {
    let running = state.xray.status().await.running;
    if let Err(err) = state
        .stats
        .reset_pattern(&format!("outbound>>>{tag}>>>traffic>>>"))
        .await
    {
        if running {
            return Err(AppError::Apply(format!(
                "reset xray outbound stats failed: {err}"
            )));
        }
    }
    db::outbounds::reset_traffic(&state.db, &tag).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn reset_outbound_all(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let running = state.xray.status().await.running;
    if let Err(err) = state.stats.reset_pattern("outbound>>>").await {
        if running {
            return Err(AppError::Apply(format!(
                "reset xray outbound stats failed: {err}"
            )));
        }
    }
    db::outbounds::reset_all(&state.db).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn template_value(state: &AppState) -> AppResult<Value> {
    let extra = load_extra(state).await?;
    let traffic = db::outbounds::list_traffic(&state.db).await?;
    Ok(json!({ "template": extra, "traffic": traffic }))
}

async fn load_extra(state: &AppState) -> AppResult<Value> {
    service::load_extra(state).await.map_err(AppError::Internal)
}

async fn commit_extra(state: &AppState, extra: &Value) -> AppResult<()> {
    service::test_extra(state, extra)
        .await
        .map_err(|err| AppError::BadRequest(format!("xray config test failed: {err}")))?;
    let serialized = serde_json::to_string(extra)
        .map_err(|err| AppError::BadRequest(format!("serialize template: {err}")))?;
    db::settings::set(&state.db, "xray_extra", &serialized).await?;
    service::apply(state)
        .await
        .map_err(|err| AppError::Apply(format!("settings saved but xray apply failed: {err}")))?;
    Ok(())
}

fn validate_outbounds(outbounds: &Value) -> AppResult<()> {
    let items = outbounds
        .as_array()
        .ok_or_else(|| AppError::BadRequest("outbounds must be an array".into()))?;
    if items.is_empty() {
        return Err(AppError::BadRequest(
            "at least one outbound is required".into(),
        ));
    }
    let mut tags = HashSet::new();
    for (index, item) in items.iter().enumerate() {
        let object = item
            .as_object()
            .ok_or_else(|| AppError::BadRequest(format!("outbounds[{index}] must be an object")))?;
        let tag = object
            .get("tag")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .ok_or_else(|| AppError::BadRequest(format!("outbounds[{index}].tag is required")))?;
        if tag == "api" {
            return Err(AppError::BadRequest(
                "outbound tag \"api\" is reserved".into(),
            ));
        }
        if !tags.insert(tag.to_string()) {
            return Err(AppError::BadRequest(format!(
                "duplicate outbound tag: {tag}"
            )));
        }
        if object
            .get("protocol")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|protocol| !protocol.is_empty())
            .is_none()
        {
            return Err(AppError::BadRequest(format!(
                "outbounds[{index}].protocol is required"
            )));
        }
    }
    Ok(())
}

fn validate_routing(routing: &Value, outbound_tags: &[String]) -> AppResult<()> {
    let object = routing
        .as_object()
        .ok_or_else(|| AppError::BadRequest("routing must be a JSON object".into()))?;
    let rules = object
        .get("rules")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::BadRequest("routing.rules must be an array".into()))?;

    let outbound_tags: HashSet<&str> = outbound_tags.iter().map(String::as_str).collect();
    let balancer_tags: HashSet<&str> = object
        .get("balancers")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("tag").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();

    for (index, rule) in rules.iter().enumerate() {
        let rule = rule.as_object().ok_or_else(|| {
            AppError::BadRequest(format!("routing.rules[{index}] must be an object"))
        })?;
        let outbound = rule.get("outboundTag").and_then(Value::as_str);
        let balancer = rule.get("balancerTag").and_then(Value::as_str);
        if outbound.is_none() && balancer.is_none() {
            return Err(AppError::BadRequest(format!(
                "routing.rules[{index}] needs outboundTag or balancerTag"
            )));
        }
        if let Some(tag) = outbound {
            if tag == "api" {
                return Err(AppError::BadRequest(
                    "routing rule cannot target the reserved \"api\" outbound".into(),
                ));
            }
            if !outbound_tags.contains(tag) {
                return Err(AppError::BadRequest(format!(
                    "routing.rules[{index}] references unknown outbound tag: {tag}"
                )));
            }
        }
        if let Some(tag) = balancer {
            if !balancer_tags.contains(tag) {
                return Err(AppError::BadRequest(format!(
                    "routing.rules[{index}] references unknown balancer tag: {tag}"
                )));
            }
        }
        if rule
            .get("inboundTag")
            .and_then(Value::as_array)
            .is_some_and(|tags| tags.iter().any(|tag| tag.as_str() == Some("api")))
        {
            return Err(AppError::BadRequest(format!(
                "routing.rules[{index}] cannot match the reserved \"api\" inbound"
            )));
        }
    }
    Ok(())
}

pub(crate) fn tags_of(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("tag").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn default_tags() -> Vec<String> {
    vec!["direct".to_string(), "blocked".to_string()]
}
