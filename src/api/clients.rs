use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::{distributions::Alphanumeric, Rng, RngCore};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::{
    auth::AuthCtx,
    db::{self, clients::ClientInput, models::Client},
    error::{AppError, AppResult},
    service,
    state::AppState,
    xray::identity::traffic_pattern,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientBody {
    pub inbound_id: Option<i64>,
    pub inbound_ids: Option<Vec<i64>>,
    pub email: String,
    pub enable: Option<bool>,
    pub total: Option<i64>,
    pub expiry_time: Option<i64>,
    pub settings: Option<Value>,
    pub comment: Option<String>,
    pub group_name: Option<String>,
    pub limit_ip: Option<i64>,
    pub limit_hwid: Option<i64>,
    pub reset_days: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ToggleBody {
    pub enable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub inbound_id: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    let mut clients = match query.inbound_id {
        Some(inbound_id) => db::clients::list_by_inbound(&state.db, inbound_id).await?,
        None => db::clients::list_all(&state.db).await?,
    };
    let associations = db::client_inbounds::all(&state.db).await?;
    for client in &mut clients {
        client.inbound_ids = associations
            .get(&client.id)
            .cloned()
            .unwrap_or_else(|| vec![client.inbound_id]);
    }
    Ok(Json(
        serde_json::to_value(clients).unwrap_or_else(|_| json!([])),
    ))
}

pub async fn online(State(state): State<AppState>) -> Json<Value> {
    let online = state.online_clients.lock().await;
    let mut users: Vec<_> = online.values().cloned().collect();
    users.sort_by(|left, right| left.email.cmp(&right.email));
    Json(serde_json::to_value(users).unwrap_or_else(|_| json!([])))
}

pub async fn get(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let client = db::clients::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("client not found".into()))?;
    let client = with_ids(&state, client).await?;
    Ok(Json(
        serde_json::to_value(&client).unwrap_or_else(|_| json!({})),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(_ctx): Extension<AuthCtx>,
    Json(body): Json<ClientBody>,
) -> AppResult<Json<Value>> {
    let inbound_ids = normalize_inbound_ids(&body, None)?;
    let inbounds = load_inbounds(&state, &inbound_ids).await?;
    let email = validate_email(&body.email)?;
    if db::clients::email_exists(&state.db, &email, None).await? {
        return Err(AppError::BadRequest("email already exists".into()));
    }

    let provided = json_object(body.settings.unwrap_or_else(|| json!({})), "settings")?;
    let settings = prepare_for_inbounds(
        &inbounds,
        &email,
        provided.as_object().cloned().unwrap_or_default(),
    )?;

    let input = ClientInput {
        inbound_id: inbound_ids[0],
        email,
        enable: body.enable.unwrap_or(true),
        total: body.total.unwrap_or(0),
        expiry_time: body.expiry_time.unwrap_or(0),
        settings,
        comment: body.comment.unwrap_or_default(),
        group_name: body.group_name.unwrap_or_default(),
        limit_ip: body.limit_ip.unwrap_or(0).max(0),
        limit_hwid: body.limit_hwid.unwrap_or(0).max(0),
        reset_days: body.reset_days.unwrap_or(0).max(0),
    };
    let id = db::clients::create(&state.db, &input).await?;
    db::client_inbounds::set_for_client(&state.db, id, &inbound_ids).await?;
    if let Err(err) = service::apply(&state).await {
        return Err(AppError::Apply(format!(
            "client saved but xray apply failed: {err}"
        )));
    }
    let client = db::clients::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("client not found".into()))?;
    let client = with_ids(&state, client).await?;
    Ok(Json(
        serde_json::to_value(&client).unwrap_or_else(|_| json!({})),
    ))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<ClientBody>,
) -> AppResult<Json<Value>> {
    let existing = db::clients::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("client not found".into()))?;
    let existing = with_ids(&state, existing).await?;
    let inbound_ids = normalize_inbound_ids(&body, Some(&existing))?;
    let inbounds = load_inbounds(&state, &inbound_ids).await?;
    let email = validate_email(&body.email)?;
    if db::clients::email_exists(&state.db, &email, Some(id)).await? {
        return Err(AppError::BadRequest("email already exists".into()));
    }

    let mut map = existing.settings.0.as_object().cloned().unwrap_or_default();
    if let Some(provided) = body.settings {
        let provided = json_object(provided, "settings")?;
        if let Some(provided) = provided.as_object() {
            for (key, value) in provided {
                map.insert(key.clone(), value.clone());
            }
        }
    }
    let settings = prepare_for_inbounds(&inbounds, &email, map)?;

    let input = ClientInput {
        inbound_id: inbound_ids[0],
        email,
        enable: body.enable.unwrap_or(existing.enable),
        total: body.total.unwrap_or(existing.total),
        expiry_time: body.expiry_time.unwrap_or(existing.expiry_time),
        settings,
        comment: body.comment.unwrap_or(existing.comment),
        group_name: body.group_name.unwrap_or(existing.group_name),
        limit_ip: body.limit_ip.unwrap_or(existing.limit_ip).max(0),
        limit_hwid: body.limit_hwid.unwrap_or(existing.limit_hwid).max(0),
        reset_days: body.reset_days.unwrap_or(existing.reset_days).max(0),
    };
    db::clients::update(&state.db, id, &input).await?;
    db::client_inbounds::set_for_client(&state.db, id, &inbound_ids).await?;
    if let Err(err) = service::apply(&state).await {
        return Err(AppError::Apply(format!(
            "client saved but xray apply failed: {err}"
        )));
    }
    let client = db::clients::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("client not found".into()))?;
    let client = with_ids(&state, client).await?;
    Ok(Json(
        serde_json::to_value(&client).unwrap_or_else(|_| json!({})),
    ))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let removed = db::clients::delete(&state.db, id).await?;
    if !removed {
        return Err(AppError::NotFound("client not found".into()));
    }
    if let Err(err) = service::apply(&state).await {
        return Err(AppError::Apply(format!(
            "client deleted but xray apply failed: {err}"
        )));
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn toggle(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<ToggleBody>,
) -> AppResult<Json<Value>> {
    if db::clients::get(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound("client not found".into()));
    }
    db::clients::set_enable(&state.db, id, body.enable).await?;
    if let Err(err) = service::apply(&state).await {
        return Err(AppError::Apply(format!(
            "client updated but xray apply failed: {err}"
        )));
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn reset(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let client = db::clients::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("client not found".into()))?;

    let mut inbound_ids = db::client_inbounds::ids_for_client(&state.db, id).await?;
    if inbound_ids.is_empty() {
        inbound_ids.push(client.inbound_id);
    }
    for inbound_id in inbound_ids {
        let pattern = traffic_pattern(inbound_id, &client.email);
        if let Err(err) = state.stats.reset_pattern(&pattern).await {
            if state.xray.status().await.running {
                return Err(AppError::Apply(format!("reset xray stats failed: {err}")));
            }
            tracing::debug!(error = %err, "xray down, resetting database counters only");
        }
    }
    db::clients::reset_traffic(&state.db, id).await?;
    let client = db::clients::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("client not found".into()))?;
    let client = with_ids(&state, client).await?;
    Ok(Json(
        serde_json::to_value(&client).unwrap_or_else(|_| json!({})),
    ))
}

async fn with_ids(state: &AppState, mut client: Client) -> AppResult<Client> {
    let ids = db::client_inbounds::ids_for_client(&state.db, client.id).await?;
    client.inbound_ids = if ids.is_empty() {
        vec![client.inbound_id]
    } else {
        ids
    };
    Ok(client)
}

fn normalize_inbound_ids(body: &ClientBody, existing: Option<&Client>) -> AppResult<Vec<i64>> {
    let mut ids = body
        .inbound_ids
        .clone()
        .or_else(|| body.inbound_id.map(|id| vec![id]))
        .or_else(|| existing.map(|client| client.inbound_ids.clone()))
        .unwrap_or_default();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|id| *id > 0);
    if ids.is_empty() {
        return Err(AppError::BadRequest(
            "at least one inbound is required".into(),
        ));
    }
    Ok(ids)
}

async fn load_inbounds(
    state: &AppState,
    inbound_ids: &[i64],
) -> AppResult<Vec<db::models::Inbound>> {
    let mut inbounds = Vec::with_capacity(inbound_ids.len());
    for id in inbound_ids {
        let inbound = db::inbounds::get(&state.db, *id)
            .await?
            .ok_or_else(|| AppError::BadRequest(format!("inbound not found: {id}")))?;
        inbounds.push(inbound);
    }
    Ok(inbounds)
}

fn prepare_for_inbounds(
    inbounds: &[db::models::Inbound],
    email: &str,
    mut settings: Map<String, Value>,
) -> AppResult<Value> {
    for inbound in inbounds {
        let value = prepare_settings(&inbound.protocol, &inbound.settings.0, email, settings)?;
        settings = value.as_object().cloned().unwrap_or_default();
    }
    Ok(Value::Object(settings))
}

fn validate_email(email: &str) -> AppResult<String> {
    let email = email.trim().to_string();
    if email.is_empty() || email.len() > 128 {
        return Err(AppError::BadRequest(
            "email must be 1-128 characters".into(),
        ));
    }
    if email.contains(">>>")
        || email
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(AppError::BadRequest(
            "email cannot contain whitespace, control characters or \">>>\"".into(),
        ));
    }
    Ok(email)
}

fn json_object(value: Value, field: &str) -> AppResult<Value> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "{field} must be a JSON object"
        )))
    }
}

fn prepare_settings(
    protocol: &str,
    inbound_settings: &Value,
    email: &str,
    mut settings: Map<String, Value>,
) -> AppResult<Value> {
    match protocol {
        "vless" | "vmess" => {
            let current = settings
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            let id = if current.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                current
            };
            if uuid::Uuid::parse_str(&id).is_err() {
                return Err(AppError::BadRequest("id must be a valid UUID".into()));
            }
            settings.insert("id".into(), json!(id));
            if protocol == "vless" {
                if let Some(flow) = settings.get("flow").and_then(Value::as_str) {
                    if flow.is_empty() {
                        settings.remove("flow");
                    }
                }
            } else {
                settings.remove("flow");
            }
        }
        "trojan" => {
            ensure_password(&mut settings, || random_alnum(20));
        }
        "shadowsocks" => {
            let method = inbound_settings
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("2022-blake3-aes-128-gcm");
            if settings
                .get("password")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                let password = if method.starts_with("2022-") {
                    let len = if method.contains("256") { 32 } else { 16 };
                    random_b64(len)
                } else {
                    random_alnum(20)
                };
                settings.insert("password".into(), json!(password));
            }
            if method.starts_with("2022-") {
                let password = settings
                    .get("password")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let len = if method.contains("256") { 32 } else { 16 };
                match STANDARD.decode(password) {
                    Ok(bytes) if bytes.len() == len => {}
                    _ => {
                        return Err(AppError::BadRequest(format!(
                            "shadowsocks {method} password must be base64 of {len} bytes"
                        )))
                    }
                }
            }
        }
        "http" | "mixed" => {
            if settings
                .get("user")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                let user = email
                    .split('@')
                    .next()
                    .map(sanitize_user)
                    .filter(|user| !user.is_empty())
                    .unwrap_or_else(|| "user".into());
                settings.insert("user".into(), json!(user));
            }
            ensure_password(&mut settings, || random_alnum(16));
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported protocol: {other}"
            )))
        }
    }
    Ok(Value::Object(settings))
}

fn ensure_password(settings: &mut Map<String, Value>, generate: impl Fn() -> String) {
    if settings
        .get("password")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        settings.insert("password".into(), json!(generate()));
    }
}

fn sanitize_user(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .take(32)
        .collect()
}

fn random_alnum(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn random_b64(len: usize) -> String {
    let mut buffer = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut buffer);
    STANDARD.encode(buffer)
}
