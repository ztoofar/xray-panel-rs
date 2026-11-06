use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use axum::{
    body::to_bytes,
    extract::{ConnectInfo, FromRequest, Multipart, Path, Query, Request, State},
    http::{
        header::{CONTENT_TYPE, SET_COOKIE},
        HeaderMap, HeaderValue,
    },
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Map, Value};

use crate::{
    api::public_ip,
    auth::{self, SESSION_COOKIE, SESSION_TTL},
    db::{
        self,
        models::{now, Client, Inbound},
    },
    error::AppResult,
    service,
    state::{AppState, SpeedSample},
    sys,
    xray::link::client_link,
};

const CSRF_COOKIE: &str = "xpanel_csrf";
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn ok(obj: Value) -> Json<Value> {
    Json(json!({ "success": true, "msg": "", "obj": obj }))
}

pub(crate) fn fail(msg: impl Into<String>) -> Json<Value> {
    Json(json!({ "success": false, "msg": msg.into(), "obj": null }))
}

pub(crate) async fn read_params(req: Request) -> AppResult<HashMap<String, String>> {
    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    // 3x-ui forms (inbound enable switch, backup upload) post multipart, which
    // the urlencoded fallback cannot parse.
    if content_type.contains("multipart/form-data") {
        let mut multipart = Multipart::from_request(req, &()).await.map_err(|err| {
            crate::error::AppError::BadRequest(format!("读取 multipart 失败：{err}"))
        })?;
        let mut params = HashMap::new();
        while let Some(field) = multipart.next_field().await.map_err(|err| {
            crate::error::AppError::BadRequest(format!("读取 multipart 字段失败：{err}"))
        })? {
            let name = field.name().unwrap_or_default().to_string();
            let value = field.text().await.map_err(|err| {
                crate::error::AppError::BadRequest(format!("读取 multipart 值失败：{err}"))
            })?;
            params.insert(name, value);
        }
        return Ok(params);
    }
    let bytes = to_bytes(req.into_body(), 2 << 20)
        .await
        .map_err(|err| crate::error::AppError::BadRequest(format!("读取请求内容失败：{err}")))?;
    if content_type.contains("application/json") {
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|err| crate::error::AppError::BadRequest(format!("JSON 格式无效：{err}")))?;
        let mut params = HashMap::new();
        if let Some(object) = value.as_object() {
            for (key, value) in object {
                let text = match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                params.insert(key.clone(), text);
            }
        }
        Ok(params)
    } else {
        let text = String::from_utf8_lossy(&bytes);
        serde_urlencoded::from_str(&text)
            .map_err(|err| crate::error::AppError::BadRequest(format!("表单格式无效：{err}")))
    }
}

pub async fn csrf_token(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = auth::cookie(&headers, SESSION_COOKIE) {
        if let Ok(Some(session)) = db::sessions::find(&state.db, &auth::hash_token(&token)).await {
            if session.expires_at > now() {
                return ok(Value::String(session.csrf)).into_response();
            }
        }
    }
    let token = auth::cookie(&headers, CSRF_COOKIE)
        .filter(|token| !token.is_empty())
        .unwrap_or_else(|| auth::random_token(24));
    let mut response = ok(Value::String(token.clone())).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{CSRF_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax"
        ))
        .expect("csrf cookie"),
    );
    response
}

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response {
    let ip = addr.ip();
    let headers = req.headers().clone();
    let real_ip = crate::notify::real_ip(&headers, ip);
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return err.into_response(),
    };
    let username = params.get("username").cloned().unwrap_or_default();
    let password = params.get("password").cloned().unwrap_or_default();
    if username.trim().is_empty() || password.is_empty() {
        tracing::debug!(%ip, "compat login rejected: empty credentials");
        return fail("用户名或密码不能为空").into_response();
    }
    if super::auth::is_rate_limited(&state, ip).await {
        tracing::warn!(%ip, "panel login rate limited");
        return fail("登录尝试过于频繁，请稍后再试").into_response();
    }

    let user = match db::users::find_by_username(&state.db, username.trim()).await {
        Ok(user) => user,
        Err(err) => {
            tracing::error!(error = %err, "compat login lookup failed");
            return fail("服务器内部错误").into_response();
        }
    };
    // Hash even for an unknown user so latency cannot reveal which usernames exist.
    let verified = match &user {
        Some(user) => auth::verify_password(&password, &user.password_hash),
        None => {
            let _ = auth::verify_password(&password, super::auth::dummy_hash());
            false
        }
    };
    let Some(user) = user.filter(|_| verified) else {
        tracing::warn!(username = ?username.trim(), %ip, "panel login failed");
        super::auth::record_failure(&state, ip).await;
        crate::notify::spawn_failed_login(state.clone(), real_ip, username.trim(), "兼容登录");
        return fail("用户名或密码错误").into_response();
    };
    super::auth::clear_failures(&state, ip).await;

    let csrf = auth::cookie(&headers, CSRF_COOKIE)
        .filter(|token| !token.is_empty())
        .unwrap_or_else(|| auth::random_token(24));
    let token = auth::random_token(32);
    let ts = now();
    let ttl = db::settings::session_ttl_secs(&state.db).await;
    if let Err(err) = db::sessions::create(
        &state.db,
        &auth::hash_token(&token),
        user.id,
        &csrf,
        ts,
        ts + ttl,
    )
    .await
    {
        tracing::error!(error = %err, "compat session create failed");
        return fail("服务器内部错误").into_response();
    }
    tracing::info!(username = ?user.username, %ip, "panel login succeeded");

    let mut response = ok(Value::Null).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&auth::session_cookie(&token, ttl, state.cfg.secure_cookie))
            .expect("session cookie"),
    );
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{CSRF_COOKIE}={csrf}; Path=/; HttpOnly; SameSite=Lax"
        ))
        .expect("csrf cookie"),
    );
    response
}

pub async fn logout(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Some(token) = auth::cookie(&headers, SESSION_COOKIE) {
        let hash = auth::hash_token(&token);
        if let Ok(Some(session)) = db::sessions::find(&state.db, &hash).await {
            tracing::info!(username = ?session.username, ip = %addr.ip(), "panel logout");
        }
        let _ = db::sessions::delete(&state.db, &hash).await;
    }
    let mut response = ok(Value::Null).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&auth::clear_cookie()).expect("cookie"),
    );
    response
}

pub async fn status() -> Json<Value> {
    ok(json!({ "version": VERSION, "dbType": "sqlite" }))
}

pub async fn setting_all(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let values = db::settings::get_all(&state.db).await?;
    let mut object = stored_settings(&values);
    let traffic_poll_secs = values
        .get("traffic_poll_secs")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(5);
    let template = template_string(&state).await;
    // Seed the new setting from the legacy native public_address row so
    // upgrades keep using the address the admin already configured.
    let legacy_public_address = values.get("public_address").cloned().unwrap_or_default();
    let seeded_node_address = object
        .get("nodeAddress")
        .and_then(Value::as_str)
        .map(str::trim)
        .map(str::to_string)
        .or_else(|| {
            object
                .get("webDomain")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or(legacy_public_address);
    let defaults = json!({
        "webListen": state.cfg.listen.ip().to_string(),
        "webPort": state.cfg.listen.port() as i64,
        "webDomain": "",
        "nodeAddress": seeded_node_address,
        "webBasePath": state.base_path(),
        "webCertFile": "",
        "webKeyFile": "",
        "xrayBin": state.cfg.xray_bin.to_string_lossy(),
        "xrayConfig": state.cfg.xray_config.to_string_lossy(),
        "xrayApiAddr": state.cfg.xray_api_addr,
        "xrayLogFile": state.cfg.xray_log_file.to_string_lossy(),
        "secureCookie": state.cfg.secure_cookie,
        "sessionMaxAge": SESSION_TTL / 60,
        "pageSize": 25,
        "expireDiff": 0,
        "trafficDiff": 0,
        "xrayTemplateConfig": template,
        "tgBotEnable": false,
        "discordEnabled": false,
        "remarkTemplate": "{{INBOUND}}-{{EMAIL}}",
        "loginNotifyWebhook": "",
        "updateProxy": "",
        "outboundCheckEnabled": false,
        "outboundCheckTags": "",
        "outboundCheckIntervalMinutes": 5,
        "ipDataCloudKey": "",
    });
    if let Some(defaults) = defaults.as_object() {
        for (key, value) in defaults {
            object.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    // Saved values must win: overwriting them here made the settings page
    // snap back to the running config and look like the save never happened.
    object.insert("xrayTemplateConfig".into(), json!(template));
    // The live prefix is authoritative even if an old row predates validation.
    object.insert("webBasePath".into(), json!(state.base_path()));
    // The poll interval lives in its own settings row because the traffic job
    // reads it directly; keep the UI value in sync with it.
    object.insert("trafficPollSecs".into(), json!(traffic_poll_secs));
    Ok(ok(Value::Object(object)))
}

pub async fn setting_update(State(state): State<AppState>, req: Request) -> AppResult<Json<Value>> {
    let mut params = read_params(req).await?;
    let mut base_path_update = None;
    let mut outbound_check_changed = false;
    for key in [
        crate::outbound_check::ENABLED_KEY,
        crate::outbound_check::TAGS_KEY,
        crate::outbound_check::INTERVAL_KEY,
    ] {
        if params.contains_key(key) {
            outbound_check_changed = true;
        }
    }
    if let Some(raw) = params.get("webBasePath") {
        match crate::web::base_path::normalize(raw) {
            Some(path) => {
                params.insert("webBasePath".into(), path.clone());
                base_path_update = Some(path);
            }
            None => return Ok(fail("面板路径格式必须类似 /secret/")),
        }
    }
    if let Some(port) = params.get("webPort") {
        match port.trim().parse::<u16>() {
            Ok(value) if value > 0 => {}
            _ => return Ok(fail("面板端口必须为 1-65535")),
        }
    }
    if let Some(listen) = params.get("webListen") {
        let listen = listen.trim();
        if !listen.is_empty() && listen.parse::<std::net::IpAddr>().is_err() {
            return Ok(fail("监听地址必须为 IP 地址或留空"));
        }
    }
    if let Some(url) = params.get("loginNotifyWebhook") {
        let url = url.trim();
        if !url.is_empty() && !(url.starts_with("http://") || url.starts_with("https://")) {
            return Ok(fail("微信 Webhook 必须以 http:// 或 https:// 开头"));
        }
    }
    if let Some(raw) = params.get("ipDataCloudKey") {
        let key = raw.trim();
        if key.chars().any(char::is_whitespace) || key.len() > 256 {
            return Ok(fail("IP数据云 API Key 格式无效"));
        }
        params.insert("ipDataCloudKey".into(), key.to_string());
    }
    if let Some(url) = params.get("updateProxy") {
        if let Err(err) = crate::outbound_subs::validate_proxy(url) {
            return Ok(fail(format!("更新代理设置无效：{err}")));
        }
    }
    if let Some(raw) = params.get("nodeAddress").cloned() {
        let address = raw.trim();
        let bracketed_v6 = address.starts_with('[') && address.ends_with(']');
        if address.contains("://")
            || address.contains('/')
            || address.chars().any(char::is_whitespace)
            || (address.matches(':').count() == 1 && !bracketed_v6)
            || address.contains("]:")
        {
            return Ok(fail("节点地址必须为不含协议、端口、路径和空格的主机名"));
        }
        params.insert("nodeAddress".into(), address.to_string());
    }
    if let Some(raw) = params.get("trafficPollSecs") {
        match raw.trim().parse::<u64>() {
            Ok(value) if (2..=300).contains(&value) => {
                db::settings::set(&state.db, "traffic_poll_secs", &value.to_string()).await?;
                state.refresh_notify.notify_one();
            }
            _ => return Ok(fail("trafficPollSecs 必须为 2-300")),
        }
    }
    if let Some(raw) = params.get(crate::outbound_check::INTERVAL_KEY) {
        match raw.trim().parse::<i64>() {
            Ok(value) if (1..=1440).contains(&value) => {}
            _ => return Ok(fail("出站检测频率必须为 1-1440 分钟")),
        }
    }
    if let Some(raw) = params.get(crate::outbound_check::TAGS_KEY) {
        let mut tags: Vec<String> = Vec::new();
        for tag in raw.split(',') {
            let tag = tag.trim();
            if !tag.is_empty() && !tags.iter().any(|known| known == tag) {
                tags.push(tag.to_string());
            }
        }
        params.insert(crate::outbound_check::TAGS_KEY.into(), tags.join(","));
    }
    let mut object = stored_settings(&db::settings::get_all(&state.db).await?);

    // Saving panel settings never restarts anything. The Xray page has its
    // own save endpoint; here we only persist values that take effect on the
    // next "restart panel" click or on the next background tick.
    for (key, value) in params {
        if key == "xrayTemplateConfig" {
            continue;
        }
        // Keep the API key a string even when it is all digits.
        let stored = if key == "ipDataCloudKey" {
            json!(value)
        } else {
            coerce_form_value(&value)
        };
        object.insert(key, stored);
    }
    let serialized = serde_json::to_string(&Value::Object(object)).unwrap_or_else(|_| "{}".into());
    db::settings::set(&state.db, "x3ui_settings", &serialized).await?;
    tracing::info!(
        outbound_check = outbound_check_changed,
        "panel settings saved"
    );
    if outbound_check_changed {
        state.outbound_check_notify.notify_one();
    }
    if let Some(path) = base_path_update {
        state.set_base_path(path);
    }
    Ok(ok(Value::Null))
}

pub async fn setting_defaults(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let values = db::settings::get_all(&state.db).await?;
    let stored = stored_settings(&values);
    let traffic_poll_secs = values
        .get("traffic_poll_secs")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(5);
    // The clients page reads pageSize/expireDiff/trafficDiff from this endpoint;
    // omitting pageSize made it fall back to its "one long page" mode.
    let value_or = |key: &str, fallback: Value| {
        stored
            .get(key)
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or(fallback)
    };
    // nodeAddress is the canonical share/QR host. webDomain stays in the
    // payload for older code paths that still read it.
    let node_address = stored
        .get("nodeAddress")
        .and_then(Value::as_str)
        .or_else(|| stored.get("webDomain").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string();
    Ok(ok(json!({
        "pageSize": value_or("pageSize", json!(25)),
        "sessionMaxAge": value_or("sessionMaxAge", json!(SESSION_TTL / 60)),
        "datepicker": value_or("datepicker", json!("gregorian")),
        "timeLocation": value_or("timeLocation", json!("")),
        "restartXrayOnClientDisable": value_or("restartXrayOnClientDisable", json!(true)),
        "trafficPollSecs": traffic_poll_secs,
        "expireDiff": value_or("expireDiff", json!(0)),
        "trafficDiff": value_or("trafficDiff", json!(0)),
        "tgBotEnable": value_or("tgBotEnable", json!(false)),
        "ipLimitEnable": value_or("ipLimitEnable", json!(false)),
        "webDomain": node_address.clone(),
        "nodeAddress": node_address,
    })))
}

pub async fn factory_defaults() -> Json<Value> {
    // Values are strings; the settings page compares them against the current
    // form value to decide whether to show its "default" tag.
    ok(json!({
        "webListen": "0.0.0.0",
        "webPort": "2053",
        "webBasePath": "/",
        "nodeAddress": "",
        "sessionMaxAge": (SESSION_TTL / 60).to_string(),
        "pageSize": "25",
        "restartXrayOnClientDisable": "true",
        "loginNotifyWebhook": "",
        "updateProxy": "",
        "xrayBin": "/usr/local/bin/xray",
        "xrayConfig": "/usr/local/etc/xray/config.json",
        "xrayApiAddr": "127.0.0.1:10085",
        "xrayLogFile": "",
        "secureCookie": "false",
        "datepicker": "gregorian",
    }))
}

pub async fn default_json_config() -> AppResult<Json<Value>> {
    Ok(ok(crate::xray::config::default_extra()))
}

pub async fn server_status(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let system = sys::snapshot();
    let xray = state.xray.status().await;
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let state_name = if xray.running { "running" } else { "stop" };
    let color = if xray.running { "green" } else { "orange" };
    let public_ip = public_ip::public_ip_json().await;
    Ok(ok(json!({
        "cpu": system.cpu_percent,
        "cpuCores": cores,
        "logicalPro": cores,
        "cpuSpeedMhz": system.cpu_speed_mhz,
        "loads": [system.load1, system.load5, system.load15],
        "mem": { "current": system.mem_used, "total": system.mem_total },
        "swap": { "current": system.swap_used, "total": system.swap_total },
        "disk": { "current": system.disk_used, "total": system.disk_total },
        "netIO": {
            "up": system.net_tx_rate.round() as u64,
            "down": system.net_rx_rate.round() as u64,
        },
        "netTraffic": { "sent": system.net_tx_bytes, "recv": system.net_rx_bytes },
        "tcpCount": system.tcp_count,
        "udpCount": system.udp_count,
        "publicIP": public_ip,
        "uptime": system.uptime,
        "appUptime": now() - state.started_at,
        "appStats": {
            "threads": system.threads,
            "mem": system.process_rss,
            "uptime": now() - state.started_at,
        },
        "xray": {
            "state": state_name,
            "errorMsg": xray.last_error.unwrap_or_default(),
            "version": xray.version.unwrap_or_default(),
            "color": color,
        }
    })))
}

pub async fn server_logs(
    State(state): State<AppState>,
    Path(rows): Path<String>,
    req: Request,
) -> Json<Value> {
    let params = read_params(req).await.unwrap_or_default();
    let limit = rows.parse::<usize>().unwrap_or(20).clamp(1, 5000);
    // The log viewer asks for one origin at a time. Without `source` this
    // endpoint keeps its historical meaning: the captured Xray process log.
    match params.get("source").map(String::as_str) {
        Some("panel") => {
            if let Some(name) = params
                .get("file")
                .map(String::as_str)
                .filter(|value| !value.is_empty())
            {
                let Some(path) = crate::panel_log::resolve_log_file(&state.cfg.panel_log_dir, name)
                else {
                    return fail("日志文件名无效");
                };
                return match super::logs::tail_lines(&path, limit).await {
                    Some(lines) => ok(Value::Array(lines.into_iter().map(Value::String).collect())),
                    None => fail("日志文件不存在或无法读取"),
                };
            }
            return ok(Value::Array(
                super::logs::panel_lines(&state, limit)
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ));
        }
        Some("xray") => {
            let file = params.get("file").map(String::as_str).unwrap_or("error");
            return ok(super::logs::xray_payload(&state, file, limit).await);
        }
        _ => {}
    }
    if let Some(lines) = super::logs::tail_lines(&state.cfg.xray_log_file, limit).await {
        if !lines.is_empty() {
            return ok(Value::Array(lines.into_iter().map(Value::String).collect()));
        }
    }
    let buffer = state.xray_logs.lock().await;
    let skip = buffer.len().saturating_sub(limit);
    ok(Value::Array(
        buffer
            .iter()
            .skip(skip)
            .cloned()
            .map(Value::String)
            .collect(),
    ))
}

pub async fn server_panel_log_files(State(state): State<AppState>) -> Json<Value> {
    ok(json!(crate::panel_log::list_log_files(
        &state.cfg.panel_log_dir
    )))
}

pub async fn server_restart_xray(State(state): State<AppState>) -> Json<Value> {
    tracing::info!("xray restart requested from dashboard");
    match service::apply(&state).await {
        Ok(()) => ok(Value::Null),
        Err(err) => fail(format!("重启 Xray 失败：{err}")),
    }
}

pub async fn server_stop_xray(State(state): State<AppState>) -> Json<Value> {
    tracing::info!("xray stop requested from dashboard");
    state.xray.stop().await;
    ok(Value::Null)
}

pub async fn setting_update_user(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let old_username = params.get("oldUsername").cloned().unwrap_or_default();
    let old_password = params.get("oldPassword").cloned().unwrap_or_default();
    let new_username = params
        .get("newUsername")
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let new_password = params.get("newPassword").cloned().unwrap_or_default();
    let user = match db::users::find_by_username(&state.db, old_username.trim()).await? {
        Some(user) => user,
        None => return Ok(fail("用户名或密码错误")),
    };
    if !auth::verify_password(&old_password, &user.password_hash) {
        return Ok(fail("用户名或密码错误"));
    }
    let username = if new_username.is_empty() {
        user.username.clone()
    } else {
        new_username
    };
    if new_password.is_empty() && username == user.username {
        return Ok(fail("没有需要更新的内容"));
    }
    if username != user.username
        && db::users::find_by_username(&state.db, &username)
            .await?
            .is_some()
    {
        return Ok(fail("用户名已存在"));
    }
    let password_hash = if new_password.is_empty() {
        user.password_hash.clone()
    } else {
        match auth::hash_password(&new_password) {
            Ok(hash) => hash,
            Err(err) => return Ok(fail(format!("生成密码哈希失败：{err}"))),
        }
    };
    db::users::update(&state.db, user.id, &username, &password_hash).await?;
    db::sessions::delete_for_user(&state.db, user.id).await?;
    Ok(ok(Value::Null))
}

pub async fn setting_restart_panel() -> Json<Value> {
    tracing::info!("panel restart requested");
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        std::process::exit(1);
    });
    ok(Value::Null)
}

pub async fn server_config_json(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let config = service::config_value(&state)
        .await
        .map_err(crate::error::AppError::Internal)?;
    Ok(ok(config))
}

pub async fn server_xray_versions(State(state): State<AppState>) -> Json<Value> {
    match crate::updates::xray_versions(&state).await {
        Ok(versions) => ok(json!(versions)),
        Err(err) => fail(format!("获取 Xray 版本失败：{err:#}")),
    }
}

pub async fn server_install_xray(
    State(state): State<AppState>,
    Path(version): Path<String>,
) -> Json<Value> {
    if version.trim().is_empty() {
        return fail("缺少 Xray 版本参数");
    }
    match crate::updates::install_xray(&state, version.trim()).await {
        Ok(()) => ok(Value::Null),
        Err(err) => fail(format!("安装 Xray {version} 失败：{err:#}")),
    }
}

pub async fn panel_update_check(State(state): State<AppState>) -> Json<Value> {
    match crate::panel_update::check(&state).await {
        Ok(info) => ok(info),
        Err(err) => fail(format!("检查面板更新失败：{err:#}")),
    }
}

pub async fn panel_update_run(State(state): State<AppState>) -> Json<Value> {
    match crate::panel_update::install(&state).await {
        Ok(info) => ok(info),
        Err(err) => fail(format!("更新面板失败：{err:#}")),
    }
}

pub async fn server_update_geofile_all(State(state): State<AppState>) -> Json<Value> {
    match crate::updates::update_geofile(&state, None).await {
        Ok(count) => ok(json!({ "installed": count })),
        Err(err) => fail(format!("更新 Geofiles 失败：{err:#}")),
    }
}

pub async fn server_update_geofile_one(
    State(state): State<AppState>,
    Path(file): Path<String>,
) -> Json<Value> {
    match crate::updates::update_geofile(&state, Some(file.trim())).await {
        Ok(count) => ok(json!({ "installed": count })),
        Err(err) => fail(format!("更新 Geofile 失败：{err:#}")),
    }
}

pub async fn server_new_uuid() -> Json<Value> {
    ok(json!(uuid::Uuid::new_v4().to_string()))
}

pub async fn server_new_x25519(State(state): State<AppState>) -> Json<Value> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        tokio::process::Command::new(&state.cfg.xray_bin)
            .arg("x25519")
            .output(),
    )
    .await;
    let output = match output {
        Ok(Ok(output)) if output.status.success() => output,
        _ => return fail("Xray x25519 生成失败"),
    };
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
        (Some(private_key), Some(public_key)) => {
            ok(json!({ "privateKey": private_key, "publicKey": public_key }))
        }
        _ => fail("无法解析 Xray x25519 输出"),
    }
}

pub async fn server_new_vless_enc(State(state): State<AppState>) -> Json<Value> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        tokio::process::Command::new(&state.cfg.xray_bin)
            .arg("vlessenc")
            .output(),
    )
    .await;
    let output = match output {
        Ok(Ok(output)) if output.status.success() => output,
        _ => return fail("Xray vlessenc 执行失败（版本过低或文件缺失）"),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    ok(json!({ "auths": vless_enc_auths(&text) }))
}

// `xray vlessenc` prints an "Authentication: <label>" line followed by a
// {decryption, encryption} block, one pair per auth kind.
fn vless_enc_auths(text: &str) -> Vec<Value> {
    let mut auths = Vec::new();
    let mut label: Option<String> = None;
    let mut body = String::new();
    let push_block = |label: &str, body: &str, auths: &mut Vec<Value>| {
        let (Some(decryption), Some(encryption)) = (
            json_string_field(body, "decryption"),
            json_string_field(body, "encryption"),
        ) else {
            return;
        };
        let normalized: String = label
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        let base_id = if normalized.contains("mlkem768") {
            "mlkem768"
        } else if normalized.contains("x25519") {
            "x25519"
        } else {
            return;
        };
        for (suffix, mode, mode_label) in [
            ("", "native", ""),
            ("_xorpub", "xorpub", " (xorpub)"),
            ("_random", "random", " (random)"),
        ] {
            auths.push(json!({
                "id": format!("{base_id}{suffix}"),
                "label": format!("{label}{mode_label}"),
                "decryption": decryption.replacen(".native.", &format!(".{mode}."), 1),
                "encryption": encryption.replacen(".native.", &format!(".{mode}."), 1),
            }));
        }
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Authentication:") {
            if let Some(previous) = label.take() {
                push_block(&previous, &body, &mut auths);
            }
            body.clear();
            label = Some(rest.trim().to_string());
        } else if label.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some(previous) = label {
        push_block(&previous, &body, &mut auths);
    }
    auths
}

fn json_string_field(body: &str, field: &str) -> Option<String> {
    let start = body.find(&format!("\"{field}\""))?;
    let rest = &body[start..];
    let rest = &rest[rest.find(':')? + 1..];
    let rest = &rest[rest.find('"')? + 1..];
    Some(rest[..rest.find('"')?].to_string())
}

async fn run_xray_generator(state: &AppState, args: &[&str]) -> Result<String, String> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new(&state.cfg.xray_bin)
            .args(args)
            .output(),
    )
    .await
    .map_err(|_| "Xray 命令执行超时".to_string())?
    .map_err(|err| format!("运行 Xray {} 失败：{err}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "Xray {} 执行失败：{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// Both `xray mldsa65` and `xray mlkem768` print two labelled "Label: value"
// lines (Seed plus the public half).
fn parse_key_pair_output(text: &str) -> Option<(String, String)> {
    let mut values = text.lines().filter_map(|line| {
        line.split_once(':')
            .map(|(_, value)| value.trim())
            .filter(|value| !value.is_empty())
    });
    Some((values.next()?.to_string(), values.next()?.to_string()))
}

pub async fn server_new_mldsa65(State(state): State<AppState>) -> Json<Value> {
    match run_xray_generator(&state, &["mldsa65"]).await {
        Ok(text) => match parse_key_pair_output(&text) {
            Some((seed, verify)) => ok(json!({ "seed": seed, "verify": verify })),
            None => fail("无法解析 Xray ML-DSA-65 输出"),
        },
        Err(err) => fail(err),
    }
}

pub async fn server_new_mlkem768(State(state): State<AppState>) -> Json<Value> {
    match run_xray_generator(&state, &["mlkem768"]).await {
        Ok(text) => match parse_key_pair_output(&text) {
            Some((seed, client)) => ok(json!({ "seed": seed, "client": client })),
            None => fail("无法解析 Xray ML-KEM-768 输出"),
        },
        Err(err) => fail(err),
    }
}

pub async fn server_new_ech_cert(State(state): State<AppState>, req: Request) -> Json<Value> {
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    let sni = params.get("sni").map(|value| value.trim()).unwrap_or("");
    if sni.is_empty() {
        return fail("生成 ECH 需要填写 SNI（serverName）");
    }
    match run_xray_generator(&state, &["tls", "ech", "--serverName", sni]).await {
        Ok(text) => {
            let lines: Vec<&str> = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect();
            // Stock xray prints a short two-line banner before the two keys:
            // config list on line 2 and server keys on line 4.
            if lines.len() < 4 {
                return fail("Xray TLS ECH 输出格式异常");
            }
            ok(json!({
                "echConfigList": lines[1],
                "echServerKeys": lines[3],
            }))
        }
        Err(err) => fail(err),
    }
}

pub async fn server_cert_hash(State(state): State<AppState>, req: Request) -> Json<Value> {
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    let cert_file = params
        .get("certFile")
        .map(|value| value.trim())
        .unwrap_or("");
    let cert_content = params.get("certContent").cloned().unwrap_or_default();
    let bytes = if !cert_file.is_empty() {
        let known = match known_cert_files(&state).await {
            Ok(known) => known,
            Err(err) => return fail(format!("读取面板设置失败：{err}")),
        };
        let requested = std::path::PathBuf::from(cert_file);
        let canonical = match requested.canonicalize() {
            Ok(path) => path,
            Err(err) => return fail(format!("证书文件错误：{err}")),
        };
        if !known.contains(&canonical) {
            return fail("证书文件未被任何入站或面板设置引用");
        }
        match tokio::fs::read(&canonical).await {
            Ok(bytes) if bytes.len() <= 1 << 20 => bytes,
            Ok(_) => return fail("证书文件超过 1 MiB"),
            Err(err) => return fail(format!("读取证书失败：{err}")),
        }
    } else if !cert_content.trim().is_empty() {
        cert_content.into_bytes()
    } else {
        return fail("未提供证书");
    };
    match certificate_der_hashes(&bytes) {
        Ok(hashes) => ok(json!(hashes)),
        Err(err) => fail(err),
    }
}

// `xray tls ping` prints one section without SNI and one with it. The SNI
// handshake is the one the inbound will actually present, so prefer it.
fn remote_cert_hashes(stdout: &str) -> Vec<String> {
    let (section, has_sni) = match stdout.rsplit_once("Pinging with SNI") {
        Some((_, rest)) => (rest, true),
        None => (stdout, false),
    };
    let mut hashes = extract_cert_hashes(section);
    if has_sni && hashes.is_empty() {
        hashes = extract_cert_hashes(stdout);
    }
    hashes
}

fn extract_cert_hashes(text: &str) -> Vec<String> {
    let mut hashes: Vec<String> = Vec::new();
    for line in text.lines() {
        let Some((label, value)) = line.split_once("SHA256:") else {
            continue;
        };
        if !label.contains("Cert's ") {
            continue;
        }
        let hash = value.trim().to_ascii_lowercase();
        if hash.len() == 64
            && hash.chars().all(|c| c.is_ascii_hexdigit())
            && !hashes.contains(&hash)
        {
            hashes.push(hash);
        }
    }
    hashes
}

fn split_host_port(raw: &str) -> Option<(String, u16)> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 300 {
        return None;
    }
    if let Some(rest) = raw.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = if after.is_empty() {
            443
        } else {
            after.strip_prefix(':')?.parse::<u16>().ok()?
        };
        return (!host.is_empty() && port > 0).then(|| (host.to_string(), port));
    }
    if !raw.contains(':') || raw.parse::<std::net::IpAddr>().is_ok() {
        return Some((raw.to_string(), 443));
    }
    let (host, port) = raw.rsplit_once(':')?;
    if host.contains(':') {
        return None;
    }
    let port = port.parse::<u16>().ok()?;
    (!host.is_empty() && port > 0).then(|| (host.to_string(), port))
}

fn valid_remote_host(host: &str) -> bool {
    !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
}

pub async fn server_remote_cert_hash(State(state): State<AppState>, req: Request) -> Json<Value> {
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    let raw = params.get("server").map(|value| value.trim()).unwrap_or("");
    let allow_private = params
        .get("allowPrivate")
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
    let Some((host, port)) = split_host_port(raw) else {
        return fail("目标地址无效，应为域名或 host:port");
    };
    if !valid_remote_host(&host) {
        return fail("目标地址包含无效字符");
    }
    let addrs = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::lookup_host((host.as_str(), port)),
    )
    .await
    {
        Err(_) => return fail("解析目标主机超时"),
        Ok(Err(err)) => return fail(format!("无法解析目标主机：{err}")),
        Ok(Ok(addrs)) => addrs.collect::<Vec<_>>(),
    };
    let Some(address) = addrs.first().copied() else {
        return fail("目标主机没有可用地址");
    };
    if !allow_private && crate::outbound_subs::blocked_ip(address.ip()) {
        // The frontend confirms with allowPrivate=true after showing this flag.
        return Json(json!({
            "success": false,
            "msg": "目标地址属于内网或回环地址，确认后才能访问",
            "obj": { "privateTarget": true },
        }));
    }
    let target = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let mut command = tokio::process::Command::new(&state.cfg.xray_bin);
    // The resolved IP is pinned so the TLS handshake cannot be rebound to a
    // different address between the guard and the probe.
    command
        .args(["tls", "ping", "-ip", &address.ip().to_string(), &target])
        .kill_on_drop(true);
    let output =
        match tokio::time::timeout(std::time::Duration::from_secs(20), command.output()).await {
            Err(_) => return fail("获取远程证书超时"),
            Ok(Err(err)) => return fail(format!("运行 Xray TLS 探测失败：{err}")),
            Ok(Ok(output)) => output,
        };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let hashes = remote_cert_hashes(&stdout);
    if hashes.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return fail(format!(
            "未能从远程服务获取证书 SHA-256{}",
            if detail.is_empty() {
                String::new()
            } else {
                format!("：{detail}")
            }
        ));
    }
    ok(json!(hashes))
}

async fn known_cert_files(state: &AppState) -> Result<HashSet<std::path::PathBuf>, anyhow::Error> {
    let mut known = HashSet::new();
    for inbound in db::inbounds::list(&state.db).await? {
        collect_cert_files(&inbound.stream_settings.0, &mut known);
    }
    let settings = db::settings::ui_settings(&state.db).await?;
    if let Some(path) = settings.get("webCertFile").and_then(Value::as_str) {
        if !path.trim().is_empty() {
            if let Ok(path) = std::path::PathBuf::from(path).canonicalize() {
                known.insert(path);
            }
        }
    }
    Ok(known)
}

fn collect_cert_files(stream: &Value, known: &mut HashSet<std::path::PathBuf>) {
    let Some(certs) = stream
        .get("tlsSettings")
        .and_then(|tls| tls.get("certificates"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for cert in certs {
        let Some(path) = cert.get("certificateFile").and_then(Value::as_str) else {
            continue;
        };
        if let Ok(path) = std::path::PathBuf::from(path).canonicalize() {
            known.insert(path);
        }
    }
}

#[cfg(test)]
mod security_helper_tests {
    use super::{
        certificate_der_hashes, parse_key_pair_output, remote_cert_hashes, split_host_port,
    };

    #[test]
    fn parses_hashes_from_the_sni_section() {
        let leaf = "a".repeat(64);
        let ca = "b".repeat(64);
        let other = "c".repeat(64);
        let output = format!(
            "TLS ping: example.com:443\nCert's leaf SHA256:\t{other}\n-------------------\n\
             Pinging with SNI\nHandshake succeeded\n\
             Cert's leaf SHA256:\t{leaf}\n\
             Cert's CA <Root> SHA256:\t{ca}\nTLS ping finished\n"
        );
        assert_eq!(remote_cert_hashes(&output), vec![leaf, ca]);
    }

    #[test]
    fn splits_host_port_variants() {
        assert_eq!(
            split_host_port("example.com"),
            Some(("example.com".into(), 443))
        );
        assert_eq!(
            split_host_port("example.com:8443"),
            Some(("example.com".into(), 8443))
        );
        assert_eq!(
            split_host_port("[2001:db8::1]:8443"),
            Some(("2001:db8::1".into(), 8443))
        );
        assert_eq!(
            split_host_port("2001:db8::1"),
            Some(("2001:db8::1".into(), 443))
        );
        assert_eq!(split_host_port("example.com:"), None);
        assert_eq!(split_host_port("example.com:0"), None);
        assert_eq!(split_host_port("https://example.com"), None);
    }

    #[test]
    fn parses_labelled_key_pair_output() {
        let (first, second) = parse_key_pair_output("Seed: abc\nVerify: def\n").expect("parsed");
        assert_eq!((first.as_str(), second.as_str()), ("abc", "def"));
        assert!(parse_key_pair_output("not a key").is_none());
    }

    #[test]
    fn hashes_pem_certificates_and_raw_der() {
        use sha2::{Digest, Sha256};
        let der = [0u8, 1, 2, 3];
        let expected = hex::encode(Sha256::digest(der));
        let raw = certificate_der_hashes(&der).unwrap();
        assert_eq!(raw, vec![expected.clone()]);
        let pem = b"-----BEGIN CERTIFICATE-----\nAAECAw==\n-----END CERTIFICATE-----\n";
        assert_eq!(certificate_der_hashes(pem).unwrap(), vec![expected]);
        assert!(certificate_der_hashes(b"").is_err());
    }
}

fn certificate_der_hashes(bytes: &[u8]) -> Result<Vec<String>, String> {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    let mut hashes = Vec::new();
    let text = String::from_utf8_lossy(bytes);
    let mut rest = text.as_ref();
    let begin = "-----BEGIN CERTIFICATE-----";
    let end = "-----END CERTIFICATE-----";
    while let Some(start) = rest.find(begin) {
        let after = &rest[start + begin.len()..];
        let Some(stop) = after.find(end) else {
            return Err("PEM 证书格式不完整".into());
        };
        let der = base64::engine::general_purpose::STANDARD
            .decode(after[..stop].split_whitespace().collect::<String>())
            .map_err(|err| format!("decode PEM certificate: {err}"))?;
        hashes.push(hex::encode(Sha256::digest(&der)));
        rest = &after[stop + end.len()..];
    }
    if hashes.is_empty() {
        if bytes.is_empty() {
            return Err("证书内容为空".into());
        }
        hashes.push(hex::encode(Sha256::digest(bytes)));
    }
    Ok(hashes)
}

pub async fn inbounds_slim(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let inbounds = db::inbounds::list(&state.db).await?;
    let clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    let mut members: HashMap<i64, Vec<&Client>> = HashMap::new();
    for client in &clients {
        let ids = associations
            .get(&client.id)
            .cloned()
            .unwrap_or_else(|| vec![client.inbound_id]);
        for id in ids {
            members.entry(id).or_default().push(client);
        }
    }
    let speeds = state.traffic_speeds.lock().await;
    Ok(ok(Value::Array(
        inbounds
            .iter()
            .map(|inbound| {
                let members = members.get(&inbound.id).cloned().unwrap_or_default();
                let settings = inject_clients(inbound, &members);
                let speed = speeds
                    .inbounds
                    .get(&inbound.tag)
                    .copied()
                    .unwrap_or_default();
                let stats: Vec<Value> = members
                    .iter()
                    .map(|client| {
                        json!({
                            "id": client.id,
                            "inboundId": inbound.id,
                            "enable": client.enable,
                            "email": client.email,
                            "up": client.up,
                            "down": client.down,
                            "expiryTime": client.expiry_time,
                            "total": client.total,
                            "reset": 0,
                        })
                    })
                    .collect();
                json!({
                    "id": inbound.id,
                    "remark": inbound.remark,
                    "tag": inbound.tag,
                    "protocol": inbound.protocol,
                    "port": inbound.port,
                    "listen": inbound.listen,
                    "enable": inbound.enable,
                    "up": inbound.up,
                    "down": inbound.down,
                    "upSpeed": speed.up,
                    "downSpeed": speed.down,
                    "total": inbound.total,
                    "expiryTime": inbound.expiry_time,
                    "settings": settings.to_string(),
                    "streamSettings": inbound.stream_settings.0.to_string(),
                    "sniffing": inbound.sniffing.0.to_string(),
                    "clientStats": stats,
                    "nodeId": Value::Null,
                    "originNodeGuid": "",
                    "shareAddrStrategy": inbound.share_addr_strategy,
                    "shareAddr": inbound.share_addr,
                    "subSortIndex": inbound.sub_sort_index,
                    "excludeFromSub": inbound.exclude_from_sub,
                    "disableFlow": inbound.disable_flow,
                })
            })
            .collect(),
    )))
}

pub async fn inbounds_list(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let inbounds = db::inbounds::list(&state.db).await?;
    let clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    let mut members: HashMap<i64, Vec<&Client>> = HashMap::new();
    for client in &clients {
        let ids = associations
            .get(&client.id)
            .cloned()
            .unwrap_or_else(|| vec![client.inbound_id]);
        for id in ids {
            members.entry(id).or_default().push(client);
        }
    }
    Ok(ok(Value::Array(
        inbounds
            .iter()
            .map(|inbound| {
                inbound_view(
                    inbound,
                    members.get(&inbound.id).map(Vec::as_slice).unwrap_or(&[]),
                )
            })
            .collect(),
    )))
}

pub async fn inbound_get(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let inbound = db::inbounds::get(&state.db, id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("入站不存在".into()))?;
    let clients = db::clients::list_by_inbound(&state.db, id).await?;
    let members: Vec<&Client> = clients.iter().collect();
    Ok(ok(inbound_view(&inbound, &members)))
}

pub async fn inbounds_options(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let inbounds = db::inbounds::list(&state.db).await?;
    Ok(ok(Value::Array(
        inbounds.iter().map(inbound_option).collect(),
    )))
}

// The clients page drops inbounds whose `enable` is falsy from the attach
// picker, so every field 3x-ui's InboundOption marks required must be present.
fn inbound_option(inbound: &Inbound) -> Value {
    let settings = &inbound.settings.0;
    let stream = &inbound.stream_settings.0;
    let network = stream
        .get("network")
        .and_then(Value::as_str)
        .unwrap_or("tcp");
    let security = stream
        .get("security")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let tls_flow_capable = inbound.protocol == "vless"
        && network == "tcp"
        && (security == "tls" || security == "reality");
    let ss_method = if inbound.protocol == "shadowsocks" {
        settings.get("method").and_then(Value::as_str).unwrap_or("")
    } else {
        ""
    };
    json!({
        "id": inbound.id,
        "remark": inbound.remark,
        "tag": inbound.tag,
        "protocol": inbound.protocol,
        "port": inbound.port,
        "enable": inbound.enable,
        "network": network,
        "security": security,
        "tlsFlowCapable": tls_flow_capable,
        "ssMethod": ss_method,
        "listen": inbound.listen,
        "nodeId": Value::Null,
        "shareAddrStrategy": "node",
        "shareAddr": "",
    })
}

pub async fn client_sub_links(
    State(state): State<AppState>,
    Path(sub_id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let address = crate::api::inbounds::public_address(&state, &headers).await?;
    let clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    let mut links = Vec::new();
    for client in &clients {
        let stored = client
            .settings
            .0
            .get("subId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if stored.is_empty() || stored != sub_id {
            continue;
        }
        let ids = associations
            .get(&client.id)
            .cloned()
            .unwrap_or_else(|| vec![client.inbound_id]);
        for id in ids {
            let Some(inbound) = db::inbounds::get(&state.db, id).await? else {
                continue;
            };
            if let Ok(Some(link)) = crate::xray::link::client_link(&inbound, client, &address) {
                links.push(Value::String(link));
            }
        }
    }
    Ok(ok(Value::Array(links)))
}

pub async fn clients_list(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let mut clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    for client in &mut clients {
        client.inbound_ids = associations
            .get(&client.id)
            .cloned()
            .unwrap_or_else(|| vec![client.inbound_id]);
    }
    let speeds = state.traffic_speeds.lock().await;
    Ok(ok(Value::Array(
        clients
            .iter()
            .map(|client| client_record(client, speeds.clients.get(&client.email).copied()))
            .collect(),
    )))
}

pub async fn clients_paged(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> AppResult<Json<Value>> {
    let mut clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    let inbound_protocols: HashMap<i64, String> = db::inbounds::list(&state.db)
        .await?
        .into_iter()
        .map(|inbound| (inbound.id, inbound.protocol))
        .collect();
    for client in &mut clients {
        client.inbound_ids = associations
            .get(&client.id)
            .cloned()
            .unwrap_or_else(|| vec![client.inbound_id]);
    }
    let page = query
        .get("page")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let page_size = query
        .get("pageSize")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(25)
        .clamp(1, 1000);
    let search = query
        .get("search")
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let stored = stored_settings(&db::settings::get_all(&state.db).await?);
    let expire_diff_ms = stored
        .get("expireDiff")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .saturating_mul(86_400_000);
    let traffic_diff_bytes = stored
        .get("trafficDiff")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .saturating_mul(1_073_741_824);
    let now_ms = now() * 1000;

    let csv = |key: &str| -> HashSet<String> {
        query
            .get(key)
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let buckets = csv("filter");
    let protocols = csv("protocol");
    let groups = csv("group");
    let inbound_ids: HashSet<i64> = csv("inbound")
        .iter()
        .filter_map(|value| value.parse().ok())
        .collect();
    let expiry_from = query
        .get("expiryFrom")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let expiry_to = query
        .get("expiryTo")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let usage_from = query
        .get("usageFrom")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let usage_to = query
        .get("usageTo")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let has_comment = query.get("hasComment").map(String::as_str);
    let has_tg_id = query.get("hasTgId").map(String::as_str);
    let auto_renew = query.get("autoRenew").map(String::as_str);

    let online_emails: HashSet<String> =
        state.online_clients.lock().await.keys().cloned().collect();
    let filtered: Vec<&Client> = clients
        .iter()
        .filter(|client| {
            if !search.is_empty() && !client_matches_search(client, &search) {
                return false;
            }
            if !buckets.is_empty() {
                let bucket = client_bucket(client, now_ms, expire_diff_ms, traffic_diff_bytes);
                let matches = buckets.contains(bucket)
                    || (buckets.contains("online") && online_emails.contains(&client.email));
                if !matches {
                    return false;
                }
            }
            if !groups.is_empty() && !groups.contains(&client.group_name) {
                return false;
            }
            if !protocols.is_empty()
                && !client.inbound_ids.iter().any(|id| {
                    inbound_protocols
                        .get(id)
                        .is_some_and(|protocol| protocols.contains(protocol))
                })
            {
                return false;
            }
            if !inbound_ids.is_empty()
                && !client.inbound_ids.iter().any(|id| inbound_ids.contains(id))
            {
                return false;
            }
            let expiry = expiry_ms(client.expiry_time);
            if expiry_from > 0 && (expiry <= 0 || expiry < expiry_from) {
                return false;
            }
            if expiry_to > 0 && (expiry <= 0 || expiry > expiry_to) {
                return false;
            }
            let used = client.up + client.down;
            if usage_from > 0 && used < usage_from {
                return false;
            }
            if usage_to > 0 && used > usage_to {
                return false;
            }
            let commented = !client.comment.trim().is_empty();
            if has_comment == Some("yes") && !commented {
                return false;
            }
            if has_comment == Some("no") && commented {
                return false;
            }
            // The panel does not model Telegram ids or auto-renew yet.
            if has_tg_id == Some("yes") || auto_renew == Some("on") {
                return false;
            }
            true
        })
        .collect();

    let sort = query.get("sort").map(String::as_str).unwrap_or("createdAt");
    let descending = query
        .get("order")
        .is_some_and(|order| order == "descend" || order == "desc");
    let mut sorted = filtered;
    sorted.sort_by(|a, b| {
        let ordering = match sort {
            "email" => a
                .email
                .to_ascii_lowercase()
                .cmp(&b.email.to_ascii_lowercase()),
            "updatedAt" => a.updated_at.cmp(&b.updated_at),
            "lastOnline" => a.last_online.cmp(&b.last_online),
            "traffic" => (a.up + a.down).cmp(&(b.up + b.down)),
            "remaining" => remaining_bytes(a).cmp(&remaining_bytes(b)),
            "expiryTime" => expiry_ms(a.expiry_time).cmp(&expiry_ms(b.expiry_time)),
            _ => a.created_at.cmp(&b.created_at),
        };
        if descending {
            ordering.reverse()
        } else {
            ordering
        }
    });

    let total = clients.len();
    let filtered_count = sorted.len();
    let start = (page - 1) * page_size;
    let speeds = state.traffic_speeds.lock().await;
    let items: Vec<Value> = sorted
        .iter()
        .skip(start)
        .take(page_size)
        .map(|client| client_record(client, speeds.clients.get(&client.email).copied()))
        .collect();

    let active = clients.iter().filter(|client| client.enable).count();
    let mut depleted_emails = Vec::new();
    let mut expiring_emails = Vec::new();
    let mut deactive_emails = Vec::new();
    for client in &clients {
        match client_bucket(client, now_ms, expire_diff_ms, traffic_diff_bytes) {
            "depleted" => depleted_emails.push(client.email.clone()),
            "expiring" => expiring_emails.push(client.email.clone()),
            "deactive" => deactive_emails.push(client.email.clone()),
            _ => {}
        }
    }
    let online: Vec<String> = online_emails.iter().cloned().collect();
    let mut groups: Vec<String> = clients
        .iter()
        .map(|client| client.group_name.clone())
        .filter(|name| !name.trim().is_empty())
        .collect();
    groups.sort();
    groups.dedup();

    Ok(ok(json!({
        "items": items,
        "total": total,
        "filtered": filtered_count,
        "page": page,
        "pageSize": page_size,
        "summary": {
            "total": total,
            "active": active,
            "onlineCount": online.len(),
            "depletedCount": depleted_emails.len(),
            "expiringCount": expiring_emails.len(),
            "deactiveCount": deactive_emails.len(),
            "online": online,
            "depleted": depleted_emails,
            "expiring": expiring_emails,
            "deactive": deactive_emails,
        },
        "groups": groups,
    })))
}

fn client_matches_search(client: &Client, needle: &str) -> bool {
    if client.email.to_ascii_lowercase().contains(needle)
        || client.comment.to_ascii_lowercase().contains(needle)
        || client.group_name.to_ascii_lowercase().contains(needle)
    {
        return true;
    }
    let Some(credentials) = client.settings.0.as_object() else {
        return false;
    };
    ["subId", "id", "password", "user", "pass"]
        .iter()
        .any(|key| {
            credentials
                .get(*key)
                .and_then(Value::as_str)
                .is_some_and(|value| value.to_ascii_lowercase().contains(needle))
        })
}

fn expiry_ms(value: i64) -> i64 {
    if value > 0 && value < 100_000_000_000 {
        value.saturating_mul(1000)
    } else {
        value
    }
}

fn remaining_bytes(client: &Client) -> i64 {
    if client.total <= 0 {
        i64::MAX
    } else {
        (client.total - (client.up + client.down)).max(0)
    }
}

fn client_bucket(
    client: &Client,
    now_ms: i64,
    expire_diff_ms: i64,
    traffic_diff_bytes: i64,
) -> &'static str {
    let used = client.up + client.down;
    let expiry = expiry_ms(client.expiry_time);
    let expired = expiry > 0 && expiry <= now_ms;
    let exhausted = client.total > 0 && used >= client.total;
    if expired || exhausted {
        return "depleted";
    }
    if !client.enable {
        return "deactive";
    }
    let expiring = (expiry > 0 && expiry - now_ms < expire_diff_ms)
        || (client.total > 0 && client.total - used < traffic_diff_bytes);
    if expiring {
        "expiring"
    } else {
        "active"
    }
}

pub async fn clients_onlines(State(state): State<AppState>) -> Json<Value> {
    let online = state.online_clients.lock().await;
    let emails: Vec<String> = online.keys().cloned().collect();
    ok(json!(emails))
}

pub async fn clients_onlines_by_guid(State(state): State<AppState>) -> Json<Value> {
    let online = state.online_clients.lock().await;
    let emails: Vec<String> = online.keys().cloned().collect();
    // Local inbounds carry an empty originNodeGuid, so key the local bucket ''
    // (the frontend's node-key fallback).
    ok(json!({ "": emails }))
}

pub async fn clients_online_ips(State(state): State<AppState>) -> Json<Value> {
    let online = state.online_clients.lock().await;
    let mut out = Map::new();
    for (email, user) in online.iter() {
        if !user.ips.is_empty() {
            let ips: Vec<&str> = user.ips.iter().map(|entry| entry.ip.as_str()).collect();
            out.insert(email.clone(), json!(ips));
        }
    }
    ok(Value::Object(out))
}

pub async fn clients_online_inbounds(State(state): State<AppState>) -> Json<Value> {
    let online = state.online_clients.lock().await;
    let mut out = Map::new();
    for (email, user) in online.iter() {
        if !user.inbounds.is_empty() {
            out.insert(email.clone(), json!(user.inbounds));
        }
    }
    ok(Value::Object(out))
}

pub async fn clients_active_inbounds(State(state): State<AppState>) -> Json<Value> {
    let tags = state.active_inbounds.lock().await.clone();
    ok(json!({ "": tags }))
}

pub async fn clients_last_online(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let clients = db::clients::list_all(&state.db).await?;
    let mut map = Map::new();
    for client in clients {
        if client.last_online > 0 {
            map.insert(client.email, json!(client.last_online * 1000));
        }
    }
    Ok(ok(Value::Object(map)))
}

pub async fn clients_groups(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let clients = db::clients::list_all(&state.db).await?;
    let mut names: Vec<String> = clients
        .into_iter()
        .map(|client| client.group_name)
        .filter(|name| !name.trim().is_empty())
        .collect();
    names.sort();
    names.dedup();
    Ok(ok(Value::Array(
        names
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect(),
    )))
}

pub async fn hosts_list() -> Json<Value> {
    ok(json!([]))
}

pub async fn xray_outbounds_traffic(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let rows = db::outbounds::list_traffic(&state.db).await?;
    Ok(ok(Value::Array(
        rows.into_iter()
            .map(|row| json!({ "tag": row.tag, "up": row.up, "down": row.down }))
            .collect(),
    )))
}

pub async fn xray_default_config() -> Json<Value> {
    ok(Value::String(
        crate::xray::config::DEFAULT_EXTRA_JSON.to_string(),
    ))
}

pub async fn inbounds_import(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let Some(raw) = params.get("data") else {
        return Ok(fail("缺少 data 参数"));
    };
    let parsed: Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(err) => return Ok(fail(format!("入站 JSON 无效：{err}"))),
    };
    let Some(object) = parsed.as_object() else {
        return Ok(fail("入站数据必须为 JSON 对象"));
    };
    let mut flat = HashMap::new();
    for (key, value) in object {
        let text = match value {
            Value::String(text) => text.clone(),
            Value::Null => continue,
            other => other.to_string(),
        };
        flat.insert(key.clone(), text);
    }
    let mut input = match inbound_input_from_params(&flat) {
        Ok(input) => input,
        Err(message) => return Ok(fail(message)),
    };
    if db::inbounds::tag_exists(&state.db, &input.tag, None).await? {
        return Ok(fail(format!("标签已存在：{}", input.tag)));
    }
    let settings = input.settings.clone();
    let managed = managed_protocol(&input.protocol);
    if managed {
        if let Some(settings) = input.settings.as_object_mut() {
            settings.remove("clients");
            settings.remove("accounts");
        }
    }
    // Shared clients keep the counters from their first import (3x-ui).
    let existing_emails: HashSet<String> = db::clients::list_all(&state.db)
        .await?
        .into_iter()
        .map(|client| client.email)
        .collect();
    let id = db::inbounds::create(&state.db, &input).await?;
    if managed {
        sync_clients_from_settings(&state, id, &input.protocol, &settings, true).await?;
        // 3x-ui import brings every client back enabled.
        for client in db::clients::list_by_inbound(&state.db, id).await? {
            if !client.enable {
                db::clients::set_enable(&state.db, client.id, true).await?;
            }
        }
    }
    let inbound_up = object.get("up").and_then(Value::as_i64).unwrap_or(0).max(0);
    let inbound_down = object
        .get("down")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    if inbound_up != 0 || inbound_down != 0 {
        db::inbounds::add_traffic(&state.db, id, inbound_up, inbound_down, 0, 0).await?;
    }
    // 3x-ui imports the exported traffic counters alongside the inbound.
    if let Some(stats) = object.get("clientStats").and_then(Value::as_array) {
        for stat in stats {
            let Some(email) = stat.get("email").and_then(Value::as_str) else {
                continue;
            };
            if existing_emails.contains(email) {
                continue;
            }
            let Some(client) = db::clients::find_by_email(&state.db, email).await? else {
                continue;
            };
            let up = stat.get("up").and_then(Value::as_i64).unwrap_or(0).max(0);
            let down = stat.get("down").and_then(Value::as_i64).unwrap_or(0).max(0);
            if up != 0 || down != 0 {
                db::clients::add_traffic(&state.db, client.id, up, down, 0, 0, 0).await?;
            }
        }
    }
    if let Err(err) = service::apply(&state).await {
        return Ok(fail(format!("入站已保存，但应用 Xray 配置失败：{err}")));
    }
    Ok(ok(json!({ "id": id })))
}

pub async fn inbound_add(State(state): State<AppState>, req: Request) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let input = match inbound_input_from_params(&params) {
        Ok(input) => input,
        Err(message) => return Ok(fail(message)),
    };
    let settings = input.settings.clone();
    let managed = managed_protocol(&input.protocol);
    let mut input = input;
    if managed {
        if let Some(object) = input.settings.as_object_mut() {
            object.remove("clients");
            object.remove("accounts");
        }
    }
    let id = db::inbounds::create(&state.db, &input).await?;
    if managed {
        sync_clients_from_settings(&state, id, &input.protocol, &settings, true).await?;
    }
    if let Err(err) = service::apply(&state).await {
        return Ok(fail(format!("入站已保存，但应用 Xray 配置失败：{err}")));
    }
    tracing::info!(
        inbound_id = id,
        tag = %input.tag,
        protocol = %input.protocol,
        "inbound created"
    );
    Ok(ok(json!({ "id": id })))
}

pub async fn inbound_update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    req: Request,
) -> AppResult<Json<Value>> {
    let existing = db::inbounds::get(&state.db, id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("入站不存在".into()))?;
    let params = read_params(req).await?;
    let mut input = match inbound_input_from_params(&params) {
        Ok(input) => input,
        Err(message) => return Ok(fail(message)),
    };
    input.user_id = existing.user_id;
    if input.tag.is_empty() {
        input.tag = existing.tag.clone();
    }
    let settings = input.settings.clone();
    let managed = managed_protocol(&input.protocol);
    let has_clients = managed
        && (settings.get("clients").and_then(Value::as_array).is_some()
            || settings.get("accounts").and_then(Value::as_array).is_some());
    if has_clients {
        if let Some(object) = input.settings.as_object_mut() {
            object.remove("clients");
            object.remove("accounts");
        }
    }
    db::inbounds::update(&state.db, id, &input).await?;
    if has_clients {
        sync_clients_from_settings(&state, id, &input.protocol, &settings, true).await?;
    }
    if let Err(err) = service::apply(&state).await {
        return Ok(fail(format!("入站已保存，但应用 Xray 配置失败：{err}")));
    }
    tracing::info!(
        inbound_id = id,
        tag = %input.tag,
        protocol = %input.protocol,
        "inbound updated"
    );
    Ok(ok(json!({ "id": id })))
}

pub async fn inbound_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    db::inbounds::delete(&state.db, id).await?;
    db::client_inbounds::delete_orphans(&state.db).await?;
    if let Err(err) = service::apply(&state).await {
        return Ok(fail(format!("入站已删除，但应用 Xray 配置失败：{err}")));
    }
    tracing::info!(inbound_id = id, "inbound deleted");
    Ok(ok(Value::Null))
}

pub async fn inbound_set_enable(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let enable = params
        .get("enable")
        .map(|value| value == "true" || value == "1")
        .unwrap_or(false);
    db::inbounds::set_enable(&state.db, id, enable).await?;
    if let Err(err) = service::apply(&state).await {
        return Ok(fail(format!("入站已更新，但应用 Xray 配置失败：{err}")));
    }
    tracing::info!(inbound_id = id, enable, "inbound toggled");
    Ok(ok(Value::Null))
}

pub async fn stub(method: axum::http::Method, Path(rest): Path<String>) -> Json<Value> {
    if !matches!(method, axum::http::Method::GET | axum::http::Method::HEAD) {
        return fail("当前面板尚未支持此功能");
    }
    let obj = if rest.contains("history")
        || rest.contains("logs")
        || rest.contains("Observatory")
        || rest.contains("apiTokens")
        || rest.contains("geodata")
        || rest.contains("CertFiles")
        || rest.ends_with("list")
    {
        json!([])
    } else {
        Value::Null
    };
    ok(obj)
}

fn stored_settings(values: &HashMap<String, String>) -> Map<String, Value> {
    values
        .get("x3ui_settings")
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn form_bool(value: Option<&String>) -> bool {
    value.is_some_and(|value| value == "true" || value == "1")
}

fn coerce_form_value(value: &str) -> Value {
    match value {
        "true" => json!(true),
        "false" => json!(false),
        _ => match value.parse::<i64>() {
            Ok(number) => json!(number),
            Err(_) => json!(value),
        },
    }
}

async fn template_string(state: &AppState) -> String {
    match service::editable_config_value(state).await {
        Ok(config) => serde_json::to_string(&config).unwrap_or_else(|_| "{}".into()),
        Err(_) => service::load_extra(state)
            .await
            .map(|extra| serde_json::to_string(&extra).unwrap_or_else(|_| "{}".into()))
            .unwrap_or_else(|_| "{}".into()),
    }
}

fn inbound_view(inbound: &Inbound, members: &[&Client]) -> Value {
    let settings = inject_clients(inbound, members);
    let stats: Vec<Value> = members
        .iter()
        .map(|client| {
            json!({
                "id": client.id,
                "inboundId": client.inbound_id,
                "enable": client.enable,
                "email": client.email,
                "up": client.up,
                "down": client.down,
                "expiryTime": client.expiry_time,
                "total": client.total,
                "reset": 0,
            })
        })
        .collect();

    json!({
        "id": inbound.id,
        "userId": inbound.user_id,
        "up": inbound.up,
        "down": inbound.down,
        "total": inbound.total,
        "remark": inbound.remark,
        "subSortIndex": inbound.sub_sort_index,
        "excludeFromSub": inbound.exclude_from_sub,
        "enable": inbound.enable,
        "expiryTime": inbound.expiry_time,
        "trafficReset": inbound.traffic_reset,
        "trafficResetDay": inbound.traffic_reset_day,
        "lastTrafficResetTime": inbound.last_traffic_reset_time,
        "listen": inbound.listen,
        "port": inbound.port,
        "protocol": inbound.protocol,
        "settings": settings.to_string(),
        "streamSettings": inbound.stream_settings.0.to_string(),
        "tag": inbound.tag,
        "sniffing": inbound.sniffing.0.to_string(),
        "clientStats": stats,
        "nodeId": Value::Null,
        "shareAddrStrategy": inbound.share_addr_strategy,
        "shareAddr": inbound.share_addr,
        "disableFlow": inbound.disable_flow,
        "originNodeGuid": "",
        "fallbackParent": Value::Null,
    })
}

fn inject_clients(inbound: &Inbound, clients: &[&Client]) -> Value {
    let mut settings = inbound.settings.0.clone();
    let Some(object) = settings.as_object_mut() else {
        return settings;
    };
    match inbound.protocol.as_str() {
        "vless" | "vmess" => {
            let list: Vec<Value> = clients
                .iter()
                .map(|client| {
                    let creds = client.settings.0.as_object();
                    let mut entry = Map::new();
                    entry.insert(
                        "id".into(),
                        creds
                            .and_then(|c| c.get("id"))
                            .cloned()
                            .unwrap_or(json!("")),
                    );
                    entry.insert("email".into(), json!(client.email));
                    entry.insert("enable".into(), json!(client.enable));
                    if inbound.protocol == "vless" {
                        if let Some(flow) =
                            creds.and_then(|c| c.get("flow")).and_then(Value::as_str)
                        {
                            if !flow.is_empty() {
                                entry.insert("flow".into(), json!(flow));
                            }
                        }
                    } else {
                        entry.insert(
                            "alterId".into(),
                            creds
                                .and_then(|c| c.get("alterId"))
                                .cloned()
                                .unwrap_or(json!(0)),
                        );
                        entry.insert(
                            "security".into(),
                            creds
                                .and_then(|c| c.get("security"))
                                .cloned()
                                .unwrap_or(json!("auto")),
                        );
                    }
                    Value::Object(entry)
                })
                .collect();
            object.insert("clients".into(), Value::Array(list));
        }
        "trojan" => {
            let list: Vec<Value> = clients
                .iter()
                .map(|client| {
                    json!({
                        "password": client.settings.0.get("password").cloned().unwrap_or(json!("")),
                        "email": client.email,
                        "enable": client.enable,
                    })
                })
                .collect();
            object.insert("clients".into(), Value::Array(list));
        }
        "shadowsocks" => {
            let method = object
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("2022-blake3-aes-128-gcm")
                .to_string();
            let list: Vec<Value> = clients
                .iter()
                .map(|client| {
                    json!({
                        "method": method,
                        "password": client.settings.0.get("password").cloned().unwrap_or(json!("")),
                        "email": client.email,
                        "enable": client.enable,
                    })
                })
                .collect();
            object.insert("clients".into(), Value::Array(list));
        }
        "http" | "mixed" => {
            let list: Vec<Value> = clients
                .iter()
                .map(|client| {
                    let creds = client.settings.0.as_object();
                    json!({
                        "user": creds.and_then(|c| c.get("user")).cloned().unwrap_or(json!(client.email)),
                        "pass": creds.and_then(|c| c.get("pass")).cloned().unwrap_or(json!("")),
                    })
                })
                .collect();
            object.insert("accounts".into(), Value::Array(list));
        }
        "hysteria" => {
            let list: Vec<Value> = clients
                .iter()
                .map(|client| {
                    let creds = client.settings.0.as_object();
                    json!({
                        "auth": creds.and_then(|c| c.get("auth")).cloned().unwrap_or(json!("")),
                        "email": client.email,
                        "enable": client.enable,
                    })
                })
                .collect();
            object.insert("clients".into(), Value::Array(list));
        }
        "wireguard" => {
            let list: Vec<Value> = clients
                .iter()
                .map(|client| {
                    let creds = client.settings.0.as_object();
                    json!({
                        "email": client.email,
                        "enable": client.enable,
                        "privateKey": creds.and_then(|c| c.get("privateKey")).cloned().unwrap_or(json!("")),
                        "publicKey": creds.and_then(|c| c.get("publicKey")).cloned().unwrap_or(json!("")),
                        "preSharedKey": creds.and_then(|c| c.get("preSharedKey")).cloned().unwrap_or(json!("")),
                        "allowedIPs": creds.and_then(|c| c.get("allowedIPs")).cloned().unwrap_or(json!([])),
                        "keepAlive": creds.and_then(|c| c.get("keepAlive")).cloned().unwrap_or(json!(0)),
                    })
                })
                .collect();
            object.insert("clients".into(), Value::Array(list));
        }
        "masque" => {
            let list: Vec<Value> = clients
                .iter()
                .map(|client| {
                    let creds = client.settings.0.as_object();
                    json!({
                        "password": creds.and_then(|c| c.get("password")).cloned().unwrap_or(json!("")),
                        "email": client.email,
                        "enable": client.enable,
                    })
                })
                .collect();
            object.insert("clients".into(), Value::Array(list));
        }
        _ => return settings,
    }
    settings
}

fn client_record(client: &Client, speed: Option<SpeedSample>) -> Value {
    let creds = client.settings.0.as_object();
    let mut record = Map::new();
    record.insert("id".into(), json!(client.id));
    record.insert("email".into(), json!(client.email));
    record.insert("enable".into(), json!(client.enable));
    record.insert("totalGB".into(), json!(client.total));
    record.insert("expiryTime".into(), json!(client.expiry_time));
    record.insert("limitIp".into(), json!(client.limit_ip));
    record.insert("limitHwid".into(), json!(client.limit_hwid));
    record.insert(
        "subId".into(),
        creds
            .and_then(|value| value.get("subId"))
            .cloned()
            .unwrap_or_else(|| json!("")),
    );
    record.insert("comment".into(), json!(client.comment));
    record.insert("group".into(), json!(client.group_name));
    record.insert("reset".into(), json!(client.reset_days));
    record.insert("lastResetTime".into(), json!(client.last_reset_time));
    let inbound_ids = if client.inbound_ids.is_empty() {
        vec![client.inbound_id]
    } else {
        client.inbound_ids.clone()
    };
    record.insert("inboundIds".into(), json!(inbound_ids));
    record.insert(
        "traffic".into(),
        json!({
            "up": client.up,
            "down": client.down,
            // Rates from the last traffic tick (bytes/s); 0 when idle. The
            // frontend no longer has to diff successive list snapshots, which
            // used to freeze the speed display on an all-idle panel.
            "upSpeed": speed.map(|value| value.up).unwrap_or(0),
            "downSpeed": speed.map(|value| value.down).unwrap_or(0),
            "total": client.total,
            "expiryTime": client.expiry_time,
            "enable": client.enable,
            // 3x-ui's clients page reads the online tooltip from here (ms).
            "lastOnline": client.last_online * 1000,
        }),
    );
    record.insert("createdAt".into(), json!(client.created_at * 1000));
    record.insert("updatedAt".into(), json!(client.updated_at * 1000));
    if let Some(creds) = creds {
        for key in [
            "id",
            "password",
            "flow",
            "auth",
            "privateKey",
            "publicKey",
            "preSharedKey",
            "allowedIPs",
            "keepAlive",
        ] {
            if let Some(value) = creds.get(key) {
                let target = if key == "id" { "uuid" } else { key };
                record.insert(target.into(), value.clone());
            }
        }
        if let Some(user) = creds.get("user") {
            record.insert("auth".into(), user.clone());
        }
        if let Some(pass) = creds.get("pass") {
            record.insert("password".into(), pass.clone());
        }
    }
    Value::Object(record)
}

fn inbound_input_from_params(
    params: &HashMap<String, String>,
) -> Result<db::inbounds::InboundInput, String> {
    let protocol = params
        .get("protocol")
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if protocol.is_empty() {
        return Err("缺少协议".into());
    }
    if !crate::xray::config::is_xray_inbound(&protocol) {
        return Err(format!("不支持的协议：{protocol}"));
    }
    let port = params
        .get("port")
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| "port is required".to_string())?;
    if !(1..=65535).contains(&port) {
        return Err("端口必须为 1-65535".into());
    }
    let settings = match params
        .get("settings")
        .filter(|value| !value.trim().is_empty())
    {
        Some(raw) => serde_json::from_str::<Value>(raw)
            .map_err(|err| format!("settings 不是有效的 JSON：{err}"))?,
        None => json!({}),
    };
    if !settings.is_object() {
        return Err("settings 必须为 JSON 对象".into());
    }
    let stream_settings = match params
        .get("streamSettings")
        .filter(|value| !value.trim().is_empty())
    {
        Some(raw) => serde_json::from_str::<Value>(raw)
            .map_err(|err| format!("streamSettings 不是有效的 JSON：{err}"))?,
        None => json!({ "network": "tcp" }),
    };
    if !stream_settings.is_object() {
        return Err("streamSettings 必须为 JSON 对象".into());
    }
    let sniffing = match params
        .get("sniffing")
        .filter(|value| !value.trim().is_empty())
    {
        Some(raw) => serde_json::from_str::<Value>(raw)
            .map_err(|err| format!("sniffing 不是有效的 JSON：{err}"))?,
        None => json!({}),
    };
    if !sniffing.is_object() {
        return Err("sniffing 必须为 JSON 对象".into());
    }
    let tag = params
        .get("tag")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("in-{protocol}-{port}"));
    if tag == "api" {
        return Err("标签 \"api\" 为保留名称".into());
    }
    let traffic_reset = params
        .get("trafficReset")
        .cloned()
        .unwrap_or_else(|| "never".into());
    let sub_sort_index = params
        .get("subSortIndex")
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value != 0)
        .unwrap_or(1);
    let share_addr_strategy = params
        .get("shareAddrStrategy")
        .map(String::as_str)
        .filter(|value| matches!(*value, "node" | "listen" | "custom"))
        .unwrap_or("node")
        .to_string();
    Ok(db::inbounds::InboundInput {
        user_id: 1,
        remark: params.get("remark").cloned().unwrap_or_default(),
        sub_sort_index,
        exclude_from_sub: form_bool(params.get("excludeFromSub")),
        enable: params
            .get("enable")
            .map(|value| value == "true" || value == "1")
            .unwrap_or(true),
        expiry_time: params
            .get("expiryTime")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        total: params
            .get("total")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        traffic_reset,
        traffic_reset_day: params
            .get("trafficResetDay")
            .and_then(|value| value.parse().ok())
            .unwrap_or(1),
        listen: params.get("listen").cloned().unwrap_or_default(),
        port,
        protocol,
        settings,
        stream_settings,
        sniffing,
        tag,
        share_addr_strategy,
        share_addr: params.get("shareAddr").cloned().unwrap_or_default(),
        disable_flow: form_bool(params.get("disableFlow")),
    })
}

// protocols whose clients live in the panel clients table
fn managed_protocol(protocol: &str) -> bool {
    matches!(
        protocol,
        "vless"
            | "vmess"
            | "trojan"
            | "shadowsocks"
            | "http"
            | "mixed"
            | "wireguard"
            | "hysteria"
            | "masque"
    )
}

pub(crate) async fn sync_clients_from_settings(
    state: &AppState,
    inbound_id: i64,
    protocol: &str,
    settings: &Value,
    replace: bool,
) -> AppResult<()> {
    if !managed_protocol(protocol) {
        return Ok(());
    }
    let mut entries: Vec<Value> = match protocol {
        "http" | "mixed" => settings
            .get("accounts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        _ => settings
            .get("clients")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    };
    if protocol == "wireguard" {
        let mut used: Vec<String> = entries
            .iter()
            .filter_map(|entry| entry.get("allowedIPs").and_then(Value::as_array))
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        for entry in &mut entries {
            if let Some(object) = entry.as_object_mut() {
                ensure_wireguard_credentials(object, settings, &used);
                if let Some(values) = object.get("allowedIPs").and_then(Value::as_array) {
                    used.extend(values.iter().filter_map(Value::as_str).map(str::to_string));
                }
            }
        }
    }
    let existing = db::clients::list_by_inbound(&state.db, inbound_id).await?;
    if entries.is_empty() {
        if replace {
            db::client_inbounds::detach_inbound(&state.db, inbound_id).await?;
            db::client_inbounds::delete_orphans(&state.db).await?;
        }
        return Ok(());
    }

    let method = settings
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("2022-blake3-aes-128-gcm");
    let mut seen = std::collections::HashSet::new();

    for entry in entries {
        let Some(object) = entry.as_object() else {
            continue;
        };
        let email = object
            .get("email")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                object
                    .get("user")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("user-{inbound_id}-{}", random_alnum(6)));
        if !seen.insert(email.clone()) {
            continue;
        }
        let mut client_settings = match protocol {
            "vless" | "vmess" => {
                let id = object
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                if protocol == "vmess" {
                    json!({
                        "id": id,
                        "alterId": object.get("alterId").cloned().unwrap_or(json!(0)),
                        "security": object.get("security").cloned().unwrap_or(json!("auto")),
                    })
                } else {
                    json!({
                        "id": id,
                        "flow": object.get("flow").cloned().unwrap_or(json!("")),
                    })
                }
            }
            "trojan" => json!({
                "password": object
                    .get("password")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| random_alnum(20)),
            }),
            "shadowsocks" => {
                let password = object
                    .get("password")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        let bytes = if method.contains("256") { 32 } else { 16 };
                        random_b64(bytes)
                    });
                json!({ "password": password })
            }
            "http" | "mixed" => json!({
                "user": object
                    .get("user")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| email.clone()),
                "pass": object
                    .get("pass")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| random_alnum(16)),
            }),
            "hysteria" => json!({
                "auth": object
                    .get("auth")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| random_alnum(16)),
            }),
            "wireguard" => json!({
                "privateKey": object.get("privateKey").and_then(Value::as_str).unwrap_or_default(),
                "publicKey": object.get("publicKey").and_then(Value::as_str).unwrap_or_default(),
                "preSharedKey": object.get("preSharedKey").and_then(Value::as_str).unwrap_or_default(),
                "allowedIPs": object.get("allowedIPs").cloned().unwrap_or(json!([])),
                "keepAlive": object.get("keepAlive").cloned().unwrap_or(json!(0)),
            }),
            "masque" => json!({
                "password": object
                    .get("password")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| random_alnum(16)),
            }),
            _ => json!({}),
        };
        // 3x-ui exports each client's subId alongside its credentials; clients
        // created through the native panel get one on demand so QR/subLinks
        // work for every inbound import path.
        if let (Some(sub_id), Some(map)) = (
            object
                .get("subId")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty()),
            client_settings.as_object_mut(),
        ) {
            map.entry("subId".to_string())
                .or_insert_with(|| json!(sub_id));
        }
        ensure_sub_id(&mut client_settings);

        let existing_client = db::clients::find_by_email(&state.db, &email).await?;
        if let Some(client) = existing_client {
            let mut merged = client.settings.0.as_object().cloned().unwrap_or_default();
            if let Some(values) = client_settings.as_object() {
                for (key, value) in values {
                    merged.insert(key.clone(), value.clone());
                }
            }
            let input = db::clients::ClientInput {
                inbound_id: client.inbound_id,
                email: email.clone(),
                enable: object
                    .get("enable")
                    .and_then(Value::as_bool)
                    .unwrap_or(client.enable),
                total: object
                    .get("totalGB")
                    .and_then(Value::as_i64)
                    .unwrap_or(client.total),
                expiry_time: object
                    .get("expiryTime")
                    .and_then(Value::as_i64)
                    .unwrap_or(client.expiry_time),
                settings: Value::Object(merged),
                comment: object
                    .get("comment")
                    .and_then(Value::as_str)
                    .unwrap_or(&client.comment)
                    .to_string(),
                group_name: client.group_name.clone(),
                limit_ip: object
                    .get("limitIp")
                    .and_then(Value::as_i64)
                    .unwrap_or(client.limit_ip),
                limit_hwid: object
                    .get("limitHwid")
                    .and_then(Value::as_i64)
                    .unwrap_or(client.limit_hwid),
                reset_days: object
                    .get("reset")
                    .and_then(Value::as_i64)
                    .unwrap_or(client.reset_days),
            };
            db::clients::update(&state.db, client.id, &input).await?;
            db::client_inbounds::attach(&state.db, client.id, inbound_id).await?;
        } else {
            let input = db::clients::ClientInput {
                inbound_id,
                email,
                enable: object
                    .get("enable")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
                total: object.get("totalGB").and_then(Value::as_i64).unwrap_or(0),
                expiry_time: object
                    .get("expiryTime")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                settings: client_settings,
                comment: object
                    .get("comment")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                group_name: String::new(),
                limit_ip: object.get("limitIp").and_then(Value::as_i64).unwrap_or(0),
                limit_hwid: object.get("limitHwid").and_then(Value::as_i64).unwrap_or(0),
                reset_days: object.get("reset").and_then(Value::as_i64).unwrap_or(0),
            };
            let client_id = db::clients::create(&state.db, &input).await?;
            db::client_inbounds::attach(&state.db, client_id, inbound_id).await?;
        }
    }

    if replace {
        for client in existing {
            if !seen.contains(&client.email) {
                db::client_inbounds::detach(&state.db, client.id, inbound_id).await?;
            }
        }
        db::client_inbounds::delete_orphans(&state.db).await?;
    }
    Ok(())
}

// 3x-ui's add/bulkCreate posts {"client": {...}, "inboundIds": [...]}; read_params
// stringifies the nested object, so fold its fields back into the flat map.
fn merge_nested_client(params: &mut HashMap<String, String>) {
    let Some(raw) = params.get("client").cloned() else {
        return;
    };
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(&raw) else {
        return;
    };
    for (key, value) in object {
        params.entry(key).or_insert(match value {
            Value::String(text) => text,
            other => other.to_string(),
        });
    }
}

async fn create_client_from_params(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<(Client, Vec<i64>), String> {
    let email = params
        .get("email")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "email is required".to_string())?;
    if db::clients::email_exists(&state.db, &email, None)
        .await
        .map_err(|err| err.to_string())?
    {
        return Err("邮箱已存在".into());
    }
    let inbound_ids = compat_inbound_ids(params, None);
    if inbound_ids.is_empty() {
        return Err("至少需要关联一个入站".into());
    }
    let mut settings = compat_client_settings(state, params, &email, None, &inbound_ids).await?;
    ensure_sub_id(&mut settings);
    let input = db::clients::ClientInput {
        inbound_id: inbound_ids[0],
        email: email.clone(),
        enable: params
            .get("enable")
            .map(|value| value == "true" || value == "1")
            .unwrap_or(true),
        total: params
            .get("totalGB")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        expiry_time: params
            .get("expiryTime")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        settings,
        comment: params.get("comment").cloned().unwrap_or_default(),
        group_name: params.get("group").cloned().unwrap_or_default(),
        limit_ip: params
            .get("limitIp")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        limit_hwid: params
            .get("limitHwid")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        reset_days: params
            .get("reset")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
    };
    let id = db::clients::create(&state.db, &input)
        .await
        .map_err(|err| err.to_string())?;
    db::client_inbounds::set_for_client(&state.db, id, &inbound_ids)
        .await
        .map_err(|err| err.to_string())?;
    let mut client = db::clients::get(&state.db, id)
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "客户端不存在".to_string())?;
    client.inbound_ids = inbound_ids.clone();
    Ok((client, inbound_ids))
}

pub async fn client_add(State(state): State<AppState>, req: Request) -> AppResult<Json<Value>> {
    let mut params = read_params(req).await?;
    merge_nested_client(&mut params);
    let client = match create_client_from_params(&state, &params).await {
        Ok((client, _)) => client,
        Err(message) => return Ok(fail(message)),
    };
    if let Err(err) = service::apply(&state).await {
        return Ok(fail(format!("客户端已保存，但应用 Xray 配置失败：{err}")));
    }
    tracing::info!(email = %client.email, "client created");
    Ok(ok(
        serde_json::to_value(&client).unwrap_or_else(|_| json!({}))
    ))
}

#[derive(serde::Deserialize)]
struct BulkClientItem {
    client: Map<String, Value>,
    #[serde(default, rename = "inboundIds")]
    inbound_ids: Vec<i64>,
}

pub async fn clients_bulk_create(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let bytes = to_bytes(req.into_body(), 2 << 20)
        .await
        .map_err(|err| crate::error::AppError::BadRequest(format!("读取请求内容失败：{err}")))?;
    let items: Vec<BulkClientItem> = serde_json::from_slice(&bytes)
        .map_err(|err| crate::error::AppError::BadRequest(format!("JSON 格式无效：{err}")))?;
    let mut created = 0usize;
    let mut skipped = Vec::new();
    for item in items {
        let mut params: HashMap<String, String> = item
            .client
            .iter()
            .map(|(key, value)| {
                let text = match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                (key.clone(), text)
            })
            .collect();
        if !item.inbound_ids.is_empty() {
            params.insert(
                "inboundIds".into(),
                serde_json::to_string(&item.inbound_ids).unwrap_or_default(),
            );
        }
        match create_client_from_params(&state, &params).await {
            Ok(_) => created += 1,
            Err(reason) => skipped.push(json!({
                "email": params.get("email").cloned().unwrap_or_default(),
                "reason": reason,
            })),
        }
    }
    if created > 0 {
        if let Err(err) = service::apply(&state).await {
            return Ok(fail(format!("客户端已保存，但应用 Xray 配置失败：{err}")));
        }
    }
    tracing::info!(created, skipped = skipped.len(), "clients bulk created");
    Ok(ok(json!({ "created": created, "skipped": skipped })))
}

fn portable_client_record(client: &Client) -> Value {
    let creds = client.settings.0.as_object();
    let mut record = Map::new();
    record.insert(
        "id".into(),
        creds
            .and_then(|value| value.get("id"))
            .cloned()
            .unwrap_or_else(|| json!("")),
    );
    record.insert("email".into(), json!(client.email));
    record.insert("enable".into(), json!(client.enable));
    record.insert("totalGB".into(), json!(client.total));
    record.insert("expiryTime".into(), json!(client.expiry_time));
    record.insert("limitIp".into(), json!(client.limit_ip));
    record.insert("limitHwid".into(), json!(client.limit_hwid));
    record.insert(
        "subId".into(),
        creds
            .and_then(|value| value.get("subId"))
            .cloned()
            .unwrap_or_else(|| json!("")),
    );
    record.insert("comment".into(), json!(client.comment));
    record.insert("group".into(), json!(client.group_name));
    record.insert("reset".into(), json!(client.reset_days));
    record.insert("lastResetTime".into(), json!(client.last_reset_time));
    record.insert("createdAt".into(), json!(client.created_at * 1000));
    record.insert("updatedAt".into(), json!(client.updated_at * 1000));
    if let Some(creds) = creds {
        for (key, value) in creds {
            record.entry(key.clone()).or_insert(value.clone());
        }
    }
    Value::Object(record)
}

pub async fn clients_export(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    let items = clients
        .iter()
        .map(|client| {
            let inbound_ids = associations.get(&client.id).cloned().unwrap_or_default();
            json!({
                "client": portable_client_record(client),
                "inboundIds": inbound_ids,
                "traffic": {
                    "up": client.up,
                    "down": client.down,
                    "lastOnline": client.last_online * 1000,
                },
            })
        })
        .collect::<Vec<_>>();
    Ok(ok(Value::Array(items)))
}

fn portable_inbound_ids(value: Option<&Value>) -> Vec<i64> {
    let mut ids: Vec<i64> = value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_i64()
                        .or_else(|| item.as_str().and_then(|text| text.trim().parse().ok()))
                })
                .collect()
        })
        .unwrap_or_default();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn portable_settings(client: &Map<String, Value>) -> Value {
    const CANONICAL_KEYS: [&str; 14] = [
        "email",
        "enable",
        "totalGB",
        "expiryTime",
        "limitIp",
        "limitHwid",
        "comment",
        "group",
        "reset",
        "lastResetTime",
        "createdAt",
        "updatedAt",
        "uuid",
        "traffic",
    ];
    let mut settings = Map::new();
    for (key, value) in client {
        if CANONICAL_KEYS.contains(&key.as_str()) {
            continue;
        }
        settings.insert(key.clone(), value.clone());
    }
    // 3x-ui exports the credential as "id"; a numeric "id" is a database row
    // and must not become a VLESS/Vmess credential.
    if !matches!(settings.get("id"), Some(Value::String(text)) if !text.is_empty()) {
        settings.remove("id");
        if let Some(uuid) = client
            .get("uuid")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            settings.insert("id".into(), json!(uuid));
        }
    }
    Value::Object(settings)
}

async fn create_portable_client(
    state: &AppState,
    client: &Map<String, Value>,
    email: &str,
    inbound_ids: &[i64],
) -> Result<Client, String> {
    if db::clients::email_exists(&state.db, email, None)
        .await
        .map_err(|err| err.to_string())?
    {
        return Err("邮箱已存在".into());
    }
    let mut params: HashMap<String, String> = client
        .iter()
        .map(|(key, value)| {
            let text = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            (key.clone(), text)
        })
        .collect();
    if let Some(credential) = client
        .get("uuid")
        .and_then(Value::as_str)
        .or_else(|| client.get("id").and_then(Value::as_str))
        .filter(|value| !value.is_empty())
    {
        params.insert("uuid".into(), credential.to_string());
    }
    params.insert(
        "inboundIds".into(),
        serde_json::to_string(inbound_ids).unwrap_or_default(),
    );
    let defaults = compat_client_settings(state, &params, email, None, inbound_ids).await?;
    let mut settings = portable_settings(client);
    if let (Some(settings), Some(defaults)) = (settings.as_object_mut(), defaults.as_object()) {
        for (key, value) in defaults {
            settings.insert(key.clone(), value.clone());
        }
    }
    ensure_sub_id(&mut settings);
    let input = db::clients::ClientInput {
        inbound_id: inbound_ids[0],
        email: email.to_string(),
        enable: client
            .get("enable")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        total: client.get("totalGB").and_then(Value::as_i64).unwrap_or(0),
        expiry_time: client
            .get("expiryTime")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        settings,
        comment: client
            .get("comment")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        group_name: client
            .get("group")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        limit_ip: client.get("limitIp").and_then(Value::as_i64).unwrap_or(0),
        limit_hwid: client.get("limitHwid").and_then(Value::as_i64).unwrap_or(0),
        reset_days: client.get("reset").and_then(Value::as_i64).unwrap_or(0),
    };
    let id = db::clients::create(&state.db, &input)
        .await
        .map_err(|err| err.to_string())?;
    db::client_inbounds::set_for_client(&state.db, id, inbound_ids)
        .await
        .map_err(|err| err.to_string())?;
    db::clients::get(&state.db, id)
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "客户端不存在".to_string())
}

pub async fn clients_import(State(state): State<AppState>, req: Request) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let Some(raw) = params.get("data") else {
        return Ok(fail("缺少 data 参数"));
    };
    let items: Vec<Value> = match serde_json::from_str(raw) {
        Ok(Value::Array(items)) => items,
        Ok(_) => return Ok(fail("客户端数据必须为 JSON 数组")),
        Err(err) => return Ok(fail(format!("客户端 JSON 无效：{err}"))),
    };
    let mut created = 0usize;
    let mut skipped = Vec::new();
    for item in items {
        let Some(object) = item.as_object() else {
            skipped.push(json!({ "email": "", "reason": "item must be a JSON object" }));
            continue;
        };
        let Some(client) = object.get("client").and_then(Value::as_object) else {
            skipped.push(json!({ "email": "", "reason": "item is missing client" }));
            continue;
        };
        let email = client
            .get("email")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        if email.is_empty() {
            skipped.push(json!({ "email": "", "reason": "client email is required" }));
            continue;
        }
        let inbound_ids = portable_inbound_ids(object.get("inboundIds"));
        if inbound_ids.is_empty() {
            skipped.push(json!({
                "email": email,
                "reason": "至少需要关联一个入站",
            }));
            continue;
        }
        match create_portable_client(&state, client, &email, &inbound_ids).await {
            Ok(client) => {
                created += 1;
                if let Some(traffic) = object.get("traffic") {
                    let up = traffic
                        .get("up")
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                        .max(0);
                    let down = traffic
                        .get("down")
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                        .max(0);
                    let last_online = traffic
                        .get("lastOnline")
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                        .max(0)
                        / 1000;
                    if up != 0 || down != 0 || last_online != 0 {
                        db::clients::set_traffic(&state.db, client.id, up, down, last_online)
                            .await?;
                    }
                }
            }
            Err(reason) => skipped.push(json!({ "email": email, "reason": reason })),
        }
    }
    if created > 0 {
        if let Err(err) = service::apply(&state).await {
            return Ok(fail(format!("客户端已导入，但应用 Xray 配置失败：{err}")));
        }
    }
    Ok(ok(json!({ "created": created, "skipped": skipped })))
}

pub async fn client_update(
    State(state): State<AppState>,
    Path(email): Path<String>,
    req: Request,
) -> AppResult<Json<Value>> {
    let existing = match db::clients::find_by_email(&state.db, &email).await? {
        Some(client) => client,
        None => return Ok(fail("客户端不存在")),
    };
    let existing_ids = db::client_inbounds::ids_for_client(&state.db, existing.id).await?;
    let mut params = read_params(req).await?;
    merge_nested_client(&mut params);
    let next_email = params
        .get("email")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| existing.email.clone());
    if next_email != existing.email
        && db::clients::email_exists(&state.db, &next_email, Some(existing.id)).await?
    {
        return Ok(fail("邮箱已存在"));
    }
    let inbound_ids = compat_inbound_ids(&params, Some(&existing_ids));
    if inbound_ids.is_empty() {
        return Ok(fail("至少需要关联一个入站"));
    }
    let mut settings =
        match compat_client_settings(&state, &params, &next_email, Some(&existing), &inbound_ids)
            .await
        {
            Ok(settings) => settings,
            Err(message) => return Ok(fail(message)),
        };
    ensure_sub_id(&mut settings);
    let input = db::clients::ClientInput {
        inbound_id: inbound_ids[0],
        email: next_email,
        enable: params
            .get("enable")
            .map(|value| value == "true" || value == "1")
            .unwrap_or(existing.enable),
        total: params
            .get("totalGB")
            .and_then(|value| value.parse().ok())
            .unwrap_or(existing.total),
        expiry_time: params
            .get("expiryTime")
            .and_then(|value| value.parse().ok())
            .unwrap_or(existing.expiry_time),
        settings,
        comment: params.get("comment").cloned().unwrap_or(existing.comment),
        group_name: params.get("group").cloned().unwrap_or(existing.group_name),
        limit_ip: params
            .get("limitIp")
            .and_then(|value| value.parse().ok())
            .unwrap_or(existing.limit_ip),
        limit_hwid: params
            .get("limitHwid")
            .and_then(|value| value.parse().ok())
            .unwrap_or(existing.limit_hwid),
        reset_days: params
            .get("reset")
            .and_then(|value| value.parse().ok())
            .unwrap_or(existing.reset_days),
    };
    db::clients::update(&state.db, existing.id, &input).await?;
    db::client_inbounds::set_for_client(&state.db, existing.id, &inbound_ids).await?;
    if let Err(err) = service::apply(&state).await {
        return Ok(fail(format!("客户端已保存，但应用 Xray 配置失败：{err}")));
    }
    let mut client = db::clients::get(&state.db, existing.id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("客户端不存在".into()))?;
    client.inbound_ids = inbound_ids;
    tracing::info!(
        email = %client.email,
        previous_email = %existing.email,
        "client updated"
    );
    Ok(ok(
        serde_json::to_value(&client).unwrap_or_else(|_| json!({}))
    ))
}

pub async fn client_delete(
    State(state): State<AppState>,
    Path(email): Path<String>,
) -> AppResult<Json<Value>> {
    let Some(client) = db::clients::find_by_email(&state.db, &email).await? else {
        return Ok(fail("客户端不存在"));
    };
    db::clients::delete(&state.db, client.id).await?;
    if let Err(err) = service::apply(&state).await {
        return Ok(fail(format!("客户端已删除，但应用 Xray 配置失败：{err}")));
    }
    tracing::info!(email = %client.email, "client deleted");
    Ok(ok(Value::Null))
}

fn compat_inbound_ids(params: &HashMap<String, String>, existing: Option<&[i64]>) -> Vec<i64> {
    let raw = params.get("inboundIds").or_else(|| params.get("inboundId"));
    let mut ids: Vec<i64> = Vec::new();
    if let Some(raw) = raw {
        let mut text = raw.trim().to_string();
        if let Ok(Value::String(inner)) = serde_json::from_str::<Value>(&text) {
            text = inner;
        }
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text) {
            ids = items
                .iter()
                .filter_map(|item| {
                    item.as_i64()
                        .or_else(|| item.as_str().and_then(|value| value.parse().ok()))
                })
                .collect();
        } else {
            ids = text
                .split(',')
                .filter_map(|value| value.trim().parse().ok())
                .collect();
        }
    }
    if ids.is_empty() {
        if let Some(existing) = existing {
            ids = existing.to_vec();
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

async fn compat_client_settings(
    state: &AppState,
    params: &HashMap<String, String>,
    email: &str,
    existing: Option<&Client>,
    inbound_ids: &[i64],
) -> Result<Value, String> {
    let mut map = existing
        .and_then(|client| client.settings.0.as_object().cloned())
        .unwrap_or_default();
    for (param, key) in [
        ("uuid", "id"),
        ("flow", "flow"),
        ("password", "password"),
        ("privateKey", "privateKey"),
        ("publicKey", "publicKey"),
        ("preSharedKey", "preSharedKey"),
    ] {
        if let Some(value) = params.get(param).filter(|value| !value.is_empty()) {
            map.insert(key.to_string(), json!(value));
        }
    }
    for (param, key) in [("allowedIPs", "allowedIPs"), ("keepAlive", "keepAlive")] {
        if let Some(raw) = params.get(param).filter(|value| !value.is_empty()) {
            let value = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| json!(raw));
            map.insert(key.to_string(), value);
        }
    }
    if let Some(auth) = params.get("auth").filter(|value| !value.is_empty()) {
        map.insert("user".into(), json!(auth));
        if let Some(password) = params.get("password").filter(|value| !value.is_empty()) {
            map.insert("pass".into(), json!(password));
        }
    }

    for inbound_id in inbound_ids {
        let inbound = db::inbounds::get(&state.db, *inbound_id)
            .await
            .map_err(|err| err.to_string())?
            .ok_or_else(|| format!("入站不存在: {inbound_id}"))?;
        match inbound.protocol.as_str() {
            "vless" | "vmess" => {
                let missing = map
                    .get("id")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty);
                if missing {
                    map.insert("id".into(), json!(uuid::Uuid::new_v4().to_string()));
                }
            }
            "trojan" => {
                if map
                    .get("password")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    map.insert("password".into(), json!(random_alnum(20)));
                }
            }
            "shadowsocks" => {
                if map
                    .get("password")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    let method = inbound
                        .settings
                        .0
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or("2022-blake3-aes-128-gcm");
                    let bytes = if method.contains("256") { 32 } else { 16 };
                    map.insert("password".into(), json!(random_b64(bytes)));
                }
            }
            "http" | "mixed" => {
                if map
                    .get("user")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    map.insert("user".into(), json!(email));
                }
                if map
                    .get("pass")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    map.insert("pass".into(), json!(random_alnum(16)));
                }
            }
            "hysteria" => {
                if let Some(auth) = params.get("auth").filter(|value| !value.is_empty()) {
                    map.insert("auth".into(), json!(auth));
                }
                if map
                    .get("auth")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    map.insert("auth".into(), json!(random_alnum(16)));
                }
            }
            "wireguard" => {
                let used: Vec<String> = db::clients::list_by_inbound(&state.db, *inbound_id)
                    .await
                    .unwrap_or_default()
                    .iter()
                    .flat_map(|client| {
                        client
                            .settings
                            .0
                            .get("allowedIPs")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
                    .collect();
                ensure_wireguard_credentials(&mut map, &inbound.settings.0, &used);
            }
            "masque"
                if map
                    .get("password")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty) =>
            {
                map.insert("password".into(), json!(random_alnum(16)));
            }
            _ => {}
        }
    }
    Ok(Value::Object(map))
}

fn ensure_sub_id(settings: &mut Value) {
    let Some(object) = settings.as_object_mut() else {
        return;
    };
    let missing = object
        .get("subId")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty);
    if missing {
        object.insert("subId".into(), json!(random_alnum(16).to_ascii_lowercase()));
    }
}

fn random_alnum(len: usize) -> String {
    use rand::{distributions::Alphanumeric, Rng};
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn random_b64(len: usize) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use rand::RngCore;
    let mut buffer = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut buffer);
    STANDARD.encode(buffer)
}

fn wireguard_keypair() -> (String, String) {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use rand::RngCore;
    use x25519_dalek::{PublicKey, StaticSecret};
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes[0] &= 248;
    bytes[31] &= 127;
    bytes[31] |= 64;
    let secret = StaticSecret::from(bytes);
    let public = PublicKey::from(&secret);
    (STANDARD.encode(bytes), STANDARD.encode(public.as_bytes()))
}

fn wireguard_public_from_private(private_key: &str) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use x25519_dalek::{PublicKey, StaticSecret};
    let bytes = STANDARD.decode(private_key).ok()?;
    let array: [u8; 32] = bytes.try_into().ok()?;
    Some(STANDARD.encode(PublicKey::from(&StaticSecret::from(array)).as_bytes()))
}

fn ipv4_number(text: &str) -> Option<u32> {
    let mut octets = text.split('.');
    let mut value = 0u32;
    for _ in 0..4 {
        let octet: u32 = octets.next()?.parse().ok()?;
        if octet > 255 {
            return None;
        }
        value = (value << 8) | octet;
    }
    if octets.next().is_some() {
        return None;
    }
    Some(value)
}

fn ipv4_text(value: u32) -> String {
    format!(
        "{}.{}.{}.{}",
        (value >> 24) & 0xff,
        (value >> 16) & 0xff,
        (value >> 8) & 0xff,
        value & 0xff
    )
}

// Mirrors 3x-ui: allocate the next free tunnel address inside the inbound's
// configured subnet (default 10.0.0.0/24), skipping the network and gateway.
fn allocate_wireguard_ip(settings: &Value, used: &[String]) -> String {
    let base = settings
        .get("subnetIp")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("10.0.0.0");
    let cidr = settings
        .get("subnetCidr")
        .and_then(Value::as_i64)
        .unwrap_or(24)
        .clamp(8, 30) as u32;
    let network = ipv4_number(base).unwrap_or(0x0a00_0000) & (u32::MAX << (32 - cidr));
    let size = 1u32 << (32 - cidr);
    let used: std::collections::HashSet<u32> = used
        .iter()
        .filter_map(|entry| entry.split('/').next().and_then(ipv4_number))
        .collect();
    for offset in 2..size.saturating_sub(1) {
        let candidate = network.wrapping_add(offset);
        if !used.contains(&candidate) {
            return format!("{}/32", ipv4_text(candidate));
        }
    }
    format!("{}/32", ipv4_text(network.wrapping_add(2)))
}

fn ensure_wireguard_credentials(
    client: &mut Map<String, Value>,
    inbound_settings: &Value,
    used: &[String],
) {
    let private = client
        .get("privateKey")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let (private_key, public_key) = match private {
        Some(private_key) => {
            let public = client
                .get("publicKey")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| wireguard_public_from_private(&private_key))
                .unwrap_or_default();
            (private_key, public)
        }
        None => wireguard_keypair(),
    };
    client.insert("privateKey".into(), json!(private_key));
    client.insert("publicKey".into(), json!(public_key));
    let has_allowed = client
        .get("allowedIPs")
        .and_then(Value::as_array)
        .is_some_and(|values| !values.is_empty());
    if !has_allowed {
        client.insert(
            "allowedIPs".into(),
            json!([allocate_wireguard_ip(inbound_settings, used)]),
        );
    }
    client.entry("keepAlive").or_insert_with(|| json!(0));
}

pub async fn xray_config(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let mut config = service::config_value(&state)
        .await
        .map_err(crate::error::AppError::Internal)?;
    let inbound_tags = config
        .get("inbounds")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("tag").and_then(Value::as_str))
                .filter(|tag| *tag != crate::xray::config::API_TAG)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(object) = config.as_object_mut() {
        object.remove("inbounds");
        object.remove("api");
        object.remove("stats");
    }
    let test_url = db::settings::get(&state.db, "outbound_test_url")
        .await
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://www.google.com/generate_204".into());
    // Subscription outbounds are injected at runtime, not into the editable
    // template; the outbounds tab shows this read-only copy.
    let (subscription_outbounds, subscription_tags) =
        match crate::outbound_subs::active_outbounds(&state).await {
            Ok(outbounds) => {
                let tags = outbounds
                    .iter()
                    .filter_map(|outbound| outbound.get("tag").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                (Value::Array(outbounds), json!(tags))
            }
            Err(err) => {
                tracing::warn!(error = %err, "read outbound subscriptions failed");
                (Value::Array(Vec::new()), json!([]))
            }
        };
    let payload = json!({
        "xraySetting": config,
        "inboundTags": inbound_tags,
        "clientReverseTags": [],
        "outboundTestUrl": test_url,
        "subscriptionOutbounds": subscription_outbounds,
        "subscriptionOutboundTags": subscription_tags,
        "geodataSources": crate::updates::standard_geodata_sources(),
    });
    let text = serde_json::to_string(&payload)
        .map_err(|err| crate::error::AppError::Internal(err.into()))?;
    Ok(ok(Value::String(text)))
}

const PANEL_OWNED_XRAY_KEYS: [&str; 3] = ["inbounds", "api", "stats"];

// Save the whole editable template like 3x-ui does. Only the blocks the
// panel generates are excluded; policy/metrics/observatory/... round-trip.
fn editable_extra_from_setting(parsed: &Value) -> Value {
    let mut extra = Map::new();
    if let Some(object) = parsed.as_object() {
        for (key, value) in object {
            if value.is_null() || PANEL_OWNED_XRAY_KEYS.contains(&key.as_str()) {
                continue;
            }
            extra.insert(key.clone(), value.clone());
        }
    }
    Value::Object(extra)
}

pub async fn xray_balancer_status(State(state): State<AppState>, req: Request) -> Json<Value> {
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    let tags: Vec<String> = params
        .get("tags")
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if tags.is_empty() {
        return ok(json!({}));
    }
    let url = state.cfg.xray_api_url();
    match crate::xray::router::balancer_status(&url, &tags).await {
        Ok(statuses) => {
            let mut by_tag = Map::new();
            for status in statuses {
                by_tag.insert(
                    status.tag.clone(),
                    serde_json::to_value(status).unwrap_or(Value::Null),
                );
            }
            ok(Value::Object(by_tag))
        }
        Err(err) => {
            tracing::debug!(error = %err, "balancer status unavailable");
            // A stopped core or one without RoutingService is not an error the
            // UI needs to shout about; report every balancer as not running.
            let mut by_tag = Map::new();
            for tag in tags {
                by_tag.insert(
                    tag.clone(),
                    json!({ "tag": tag, "running": false, "override": "", "selected": [] }),
                );
            }
            ok(Value::Object(by_tag))
        }
    }
}

pub async fn xray_balancer_override(State(state): State<AppState>, req: Request) -> Json<Value> {
    let params = match read_params(req).await {
        Ok(params) => params,
        Err(err) => return fail(err.to_string()),
    };
    let tag = params.get("tag").map(|value| value.trim()).unwrap_or("");
    if tag.is_empty() {
        return fail("缺少标签");
    }
    let target = params.get("target").map(|value| value.trim()).unwrap_or("");
    match crate::xray::router::override_balancer(&state.cfg.xray_api_url(), tag, target).await {
        Ok(()) => ok(Value::Null),
        Err(err) => fail(format!("切换负载均衡目标失败：{err:#}")),
    }
}

pub async fn xray_update(State(state): State<AppState>, req: Request) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let Some(raw) = params.get("xraySetting") else {
        return Ok(fail("缺少 xraySetting 参数"));
    };
    let parsed: Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(err) => return Ok(fail(format!("xraySetting JSON 无效：{err}"))),
    };
    if !parsed.is_object() {
        return Ok(fail("xraySetting 必须为 JSON 对象"));
    }
    let extra = editable_extra_from_setting(&parsed);
    if let Some(url) = params.get("outboundTestUrl") {
        let url = if url.trim().is_empty() {
            "https://www.google.com/generate_204"
        } else {
            url.trim()
        };
        db::settings::set(&state.db, "outbound_test_url", url).await?;
    }
    service::test_extra(&state, &extra)
        .await
        .map_err(|err| crate::error::AppError::BadRequest(format!("Xray 配置校验失败：{err}")))?;
    let serialized = serde_json::to_string(&extra)
        .map_err(|err| crate::error::AppError::BadRequest(format!("序列化模板失败：{err}")))?;
    db::settings::set(&state.db, "xray_extra", &serialized).await?;
    let mut tags = crate::api::xray_settings::tags_of(&extra, "outbounds");
    // Subscription outbounds have traffic counters and routing rules too, and
    // they are intentionally absent from the template being saved here.
    tags.extend(crate::outbound_subs::active_tags(&state).await?);
    if !tags.is_empty() {
        db::outbounds::retain_tags(&state.db, &tags).await?;
    }
    service::apply(&state)
        .await
        .map_err(|err| crate::error::AppError::Apply(format!("已保存，但应用配置失败：{err}")))?;
    Ok(ok(Value::Null))
}

pub async fn xray_reset_outbound_traffic(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    if let Some(tag) = params.get("tag").filter(|value| !value.trim().is_empty()) {
        db::outbounds::reset_traffic(&state.db, tag).await?;
    } else {
        db::outbounds::reset_all(&state.db).await?;
    }
    Ok(ok(Value::Null))
}

fn json_string_list(params: &HashMap<String, String>, key: &str) -> Vec<String> {
    let Some(raw) = params.get(key) else {
        return Vec::new();
    };
    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) {
        return items
            .into_iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect();
    }
    raw.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn json_i64_list(params: &HashMap<String, String>, key: &str) -> Vec<i64> {
    let Some(raw) = params.get(key) else {
        return Vec::new();
    };
    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) {
        return items
            .into_iter()
            .filter_map(|item| {
                item.as_i64()
                    .or_else(|| item.as_str().and_then(|v| v.parse().ok()))
            })
            .collect();
    }
    raw.split(',')
        .filter_map(|item| item.trim().parse().ok())
        .collect()
}

pub async fn client_get(
    State(state): State<AppState>,
    Path(email): Path<String>,
) -> AppResult<Json<Value>> {
    let Some(client) = db::clients::find_by_email(&state.db, &email).await? else {
        return Ok(fail("客户端不存在"));
    };
    let mut client = client;
    let ids = db::client_inbounds::ids_for_client(&state.db, client.id).await?;
    if !ids.is_empty() {
        client.inbound_ids = ids;
    }
    let speed = state
        .traffic_speeds
        .lock()
        .await
        .clients
        .get(&client.email)
        .copied();
    Ok(ok(json!({
        "client": client_record(&client, speed),
        "inboundIds": client.inbound_ids,
        "tunnelAllowedIPs": {},
    })))
}

pub async fn client_reset_traffic(
    State(state): State<AppState>,
    Path(email): Path<String>,
) -> AppResult<Json<Value>> {
    let Some(client) = db::clients::find_by_email(&state.db, &email).await? else {
        return Ok(fail("客户端不存在"));
    };
    db::clients::reset_traffic(&state.db, client.id).await?;
    Ok(ok(Value::Null))
}

pub async fn clients_reset_all_traffic(State(state): State<AppState>) -> AppResult<Json<Value>> {
    for client in db::clients::list_all(&state.db).await? {
        db::clients::reset_traffic(&state.db, client.id).await?;
    }
    Ok(ok(Value::Null))
}

async fn set_clients_enable(state: &AppState, emails: &[String], enable: bool) -> AppResult<Value> {
    let mut affected = 0;
    for email in emails {
        if let Some(client) = db::clients::find_by_email(&state.db, email).await? {
            db::clients::set_enable(&state.db, client.id, enable).await?;
            affected += 1;
        }
    }
    if affected > 0 {
        let _ = service::apply(state).await;
    }
    Ok(json!({ "affected": affected }))
}

pub async fn clients_bulk_enable(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let emails = json_string_list(&params, "emails");
    Ok(ok(set_clients_enable(&state, &emails, true).await?))
}

pub async fn clients_bulk_disable(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let emails = json_string_list(&params, "emails");
    Ok(ok(set_clients_enable(&state, &emails, false).await?))
}

pub async fn clients_bulk_del(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let emails = json_string_list(&params, "emails");
    let mut affected = 0;
    for email in &emails {
        if let Some(client) = db::clients::find_by_email(&state.db, email).await? {
            db::clients::delete(&state.db, client.id).await?;
            affected += 1;
        }
    }
    db::client_inbounds::delete_orphans(&state.db).await?;
    if affected > 0 {
        let _ = service::apply(&state).await;
    }
    Ok(ok(json!({ "affected": affected, "deleted": affected })))
}

pub async fn clients_bulk_adjust(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let emails = json_string_list(&params, "emails");
    let add_days = params
        .get("addDays")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let add_bytes = params
        .get("addBytes")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let mut affected = 0;
    for email in &emails {
        if let Some(client) = db::clients::find_by_email(&state.db, email).await? {
            db::clients::adjust(&state.db, client.id, add_days * 86_400_000, add_bytes).await?;
            affected += 1;
        }
    }
    if affected > 0 {
        let _ = service::apply(&state).await;
    }
    Ok(ok(json!({ "affected": affected })))
}

async fn attach_emails(
    state: &AppState,
    emails: &[String],
    inbound_ids: &[i64],
    attach: bool,
) -> AppResult<Value> {
    let mut affected = 0;
    for email in emails {
        let Some(client) = db::clients::find_by_email(&state.db, email).await? else {
            continue;
        };
        for inbound_id in inbound_ids {
            if attach {
                db::client_inbounds::attach(&state.db, client.id, *inbound_id).await?;
            } else {
                db::client_inbounds::detach(&state.db, client.id, *inbound_id).await?;
            }
        }
        affected += 1;
    }
    db::client_inbounds::delete_orphans(&state.db).await?;
    if affected > 0 {
        let _ = service::apply(state).await;
    }
    Ok(json!({ "affected": affected }))
}

pub async fn clients_bulk_attach(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let emails = json_string_list(&params, "emails");
    let ids = json_i64_list(&params, "inboundIds");
    Ok(ok(attach_emails(&state, &emails, &ids, true).await?))
}

pub async fn clients_bulk_detach(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let emails = json_string_list(&params, "emails");
    let ids = json_i64_list(&params, "inboundIds");
    Ok(ok(attach_emails(&state, &emails, &ids, false).await?))
}

pub async fn client_attach(
    State(state): State<AppState>,
    Path(email): Path<String>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let ids = json_i64_list(&params, "inboundIds");
    Ok(ok(attach_emails(&state, &[email], &ids, true).await?))
}

pub async fn client_detach(
    State(state): State<AppState>,
    Path(email): Path<String>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let ids = json_i64_list(&params, "inboundIds");
    Ok(ok(attach_emails(&state, &[email], &ids, false).await?))
}

pub async fn clients_groups_bulk_add(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let emails = json_string_list(&params, "emails");
    let group = params.get("group").cloned().unwrap_or_default();
    let mut affected = 0;
    for email in &emails {
        if let Some(client) = db::clients::find_by_email(&state.db, email).await? {
            db::clients::set_group(&state.db, client.id, &group).await?;
            affected += 1;
        }
    }
    Ok(ok(json!({ "affected": affected })))
}

pub async fn clients_groups_bulk_remove(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let emails = json_string_list(&params, "emails");
    let mut affected = 0;
    for email in &emails {
        if let Some(client) = db::clients::find_by_email(&state.db, email).await? {
            db::clients::set_group(&state.db, client.id, "").await?;
            affected += 1;
        }
    }
    Ok(ok(json!({ "affected": affected })))
}

pub async fn clients_del_depleted(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let deleted = db::clients::delete_depleted(&state.db).await?;
    db::client_inbounds::delete_orphans(&state.db).await?;
    if deleted > 0 {
        let _ = service::apply(&state).await;
    }
    Ok(ok(json!({ "deleted": deleted })))
}

pub async fn clients_del_orphans(State(state): State<AppState>) -> AppResult<Json<Value>> {
    db::client_inbounds::delete_orphans(&state.db).await?;
    let clients = db::clients::list_all(&state.db).await?;
    let mut deleted = 0;
    for client in clients {
        let ids = db::client_inbounds::ids_for_client(&state.db, client.id).await?;
        if ids.is_empty() {
            db::clients::delete(&state.db, client.id).await?;
            deleted += 1;
        }
    }
    Ok(ok(json!({ "deleted": deleted })))
}

pub async fn inbounds_all_links(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let public = crate::api::inbounds::public_address(&state, &headers).await?;
    let mut links = Vec::new();
    for inbound in db::inbounds::list(&state.db).await? {
        // Same precedence as the frontend link builder: custom address, then
        // the listen address, then the panel's public host.
        let address = match inbound.share_addr_strategy.as_str() {
            "custom" if !inbound.share_addr.trim().is_empty() => inbound.share_addr.trim(),
            "listen" if !inbound.listen.is_empty() => inbound.listen.trim(),
            _ => public.as_str(),
        };
        let clients = db::clients::list_by_inbound(&state.db, inbound.id).await?;
        for client in &clients {
            if let Ok(Some(link)) = client_link(&inbound, client, address) {
                links.push(Value::String(link));
            }
        }
    }
    Ok(ok(Value::Array(links)))
}

pub async fn inbound_fallbacks_get(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let Some(inbound) = db::inbounds::get(&state.db, id).await? else {
        return Ok(fail("入站不存在"));
    };
    let fallbacks = inbound
        .settings
        .0
        .get("fallbacks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let items = fallbacks
        .into_iter()
        .map(|item| {
            json!({
                "childId": 0,
                "name": item.get("name").cloned().unwrap_or(json!("")),
                "alpn": item.get("alpn").cloned().unwrap_or(json!("")),
                "path": item.get("path").cloned().unwrap_or(json!("")),
                "dest": item.get("dest").cloned().unwrap_or(json!("")),
                "xver": item.get("xver").cloned().unwrap_or(json!(0)),
            })
        })
        .collect::<Vec<_>>();
    Ok(ok(Value::Array(items)))
}

pub async fn inbound_fallbacks_put(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let Some(raw) = params.get("fallbacks") else {
        return Ok(fail("缺少 fallbacks 参数"));
    };
    let parsed: Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(err) => return Ok(fail(format!("fallbacks JSON 无效：{err}"))),
    };
    let Some(rows) = parsed.as_array() else {
        return Ok(fail("fallbacks 必须为数组"));
    };
    let fallbacks = rows
        .iter()
        .filter(|row| {
            row.get("dest")
                .and_then(Value::as_str)
                .is_some_and(|dest| !dest.trim().is_empty())
                || row
                    .get("childId")
                    .and_then(Value::as_i64)
                    .is_some_and(|id| id > 0)
        })
        .map(|row| {
            json!({
                "name": row.get("name").and_then(Value::as_str).unwrap_or_default(),
                "alpn": row.get("alpn").and_then(Value::as_str).unwrap_or_default(),
                "path": row.get("path").and_then(Value::as_str).unwrap_or_default(),
                "dest": row.get("dest").and_then(Value::as_str).unwrap_or_default(),
                "xver": row.get("xver").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect::<Vec<_>>();
    let Some(inbound) = db::inbounds::get(&state.db, id).await? else {
        return Ok(fail("入站不存在"));
    };
    let mut settings = inbound.settings.0.clone();
    if let Some(object) = settings.as_object_mut() {
        object.insert("fallbacks".into(), Value::Array(fallbacks));
    }
    db::inbounds::set_settings(&state.db, id, &settings).await?;
    if let Err(err) = service::apply(&state).await {
        return Ok(fail(format!("fallbacks 已保存，但应用配置失败：{err}")));
    }
    Ok(ok(Value::Null))
}

pub async fn inbound_reset_traffic(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    if db::inbounds::get(&state.db, id).await?.is_none() {
        return Ok(fail("入站不存在"));
    }
    // A client's stored total aggregates every inbound it is attached to, so
    // an inbound reset clears the whole total (3x-ui does the same); the
    // baselines stay and only new bytes are billed afterwards.
    for client in db::clients::list_by_inbound(&state.db, id).await? {
        db::clients::reset_traffic(&state.db, client.id).await?;
    }
    db::inbounds::reset_traffic(&state.db, id).await?;
    Ok(ok(Value::Null))
}

pub async fn inbound_del_all_clients(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let deleted = db::clients::delete_by_inbound(&state.db, id).await?;
    db::client_inbounds::delete_orphans(&state.db).await?;
    if deleted > 0 {
        let _ = service::apply(&state).await;
    }
    Ok(ok(json!({ "deleted": deleted })))
}

pub async fn inbounds_bulk_del(
    State(state): State<AppState>,
    req: Request,
) -> AppResult<Json<Value>> {
    let params = read_params(req).await?;
    let ids = json_i64_list(&params, "ids");
    let mut deleted = 0;
    for id in ids {
        if db::inbounds::delete(&state.db, id).await? {
            deleted += 1;
        }
    }
    db::client_inbounds::delete_orphans(&state.db).await?;
    if deleted > 0 {
        let _ = service::apply(&state).await;
    }
    Ok(ok(json!({ "deleted": deleted })))
}

pub async fn inbounds_reset_all_traffic(State(state): State<AppState>) -> AppResult<Json<Value>> {
    for inbound in db::inbounds::list(&state.db).await? {
        db::inbounds::reset_traffic(&state.db, inbound.id).await?;
    }
    for client in db::clients::list_all(&state.db).await? {
        db::clients::reset_traffic(&state.db, client.id).await?;
    }
    Ok(ok(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(protocol: &str, settings: Value, stream: Value, enable: bool) -> Inbound {
        Inbound {
            id: 1,
            user_id: 1,
            up: 0,
            down: 0,
            last_up: 0,
            last_down: 0,
            total: 0,
            remark: "sample".into(),
            sub_sort_index: 1,
            exclude_from_sub: false,
            enable,
            expiry_time: 0,
            traffic_reset: "never".into(),
            traffic_reset_day: 0,
            last_traffic_reset_time: 0,
            listen: "".into(),
            port: 443,
            protocol: protocol.into(),
            settings: sqlx::types::Json(settings),
            stream_settings: sqlx::types::Json(stream),
            sniffing: sqlx::types::Json(json!({})),
            tag: "in-443-tcp".into(),
            share_addr_strategy: "node".into(),
            share_addr: String::new(),
            disable_flow: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn inbound_input_rejects_malformed_json_instead_of_dropping_it() {
        let mut params = HashMap::new();
        params.insert("protocol".into(), "vless".into());
        params.insert("port".into(), "443".into());
        params.insert("settings".into(), "{not json".into());
        let err = inbound_input_from_params(&params).expect_err("malformed settings");
        assert!(err.contains("settings 不是有效的 JSON"), "{err}");

        let mut params = HashMap::new();
        params.insert("protocol".into(), "vless".into());
        params.insert("port".into(), "443".into());
        params.insert("streamSettings".into(), "[]".into());
        let err = inbound_input_from_params(&params).expect_err("non-object stream");
        assert!(err.contains("streamSettings 必须为 JSON 对象"), "{err}");
    }

    #[test]
    fn inbound_input_accepts_every_xray_native_protocol() {
        for protocol in crate::xray::config::XRAY_INBOUND_PROTOCOLS {
            let mut params = HashMap::new();
            params.insert("protocol".into(), protocol.to_string());
            params.insert("port".into(), "443".into());
            let input = inbound_input_from_params(&params)
                .unwrap_or_else(|err| panic!("{protocol}: {err}"));
            assert_eq!(input.protocol, protocol);
        }
        for rejected in ["mtproto", "tuic", "amneziawg", "socks", ""] {
            let mut params = HashMap::new();
            params.insert("protocol".into(), rejected.to_string());
            params.insert("port".into(), "443".into());
            assert!(
                inbound_input_from_params(&params).is_err(),
                "{rejected} must be rejected"
            );
        }
    }

    #[test]
    fn wireguard_credentials_are_generated_and_addresses_unique() {
        let settings = json!({"subnetIp":"10.9.0.0","subnetCidr":24});
        let mut first = Map::new();
        ensure_wireguard_credentials(&mut first, &settings, &[]);
        let private = first["privateKey"].as_str().unwrap().to_string();
        let public = first["publicKey"].as_str().unwrap().to_string();
        assert!(!private.is_empty() && !public.is_empty());
        assert_eq!(first["allowedIPs"][0], "10.9.0.2/32");

        let used = vec!["10.9.0.2/32".to_string()];
        let mut second = Map::new();
        ensure_wireguard_credentials(&mut second, &settings, &used);
        assert_eq!(second["allowedIPs"][0], "10.9.0.3/32");

        // A supplied private key derives the matching public key.
        let mut derived = Map::new();
        derived.insert("privateKey".into(), json!(private));
        ensure_wireguard_credentials(&mut derived, &settings, &[]);
        assert_eq!(derived["publicKey"], json!(public));
    }

    #[test]
    fn editable_extra_round_trips_user_blocks_and_drops_panel_blocks() {
        let parsed = json!({
            "inbounds": [{ "tag": "api" }],
            "api": { "tag": "api" },
            "stats": {},
            "policy": { "levels": { "0": { "connIdle": 123 } } },
            "metrics": { "listen": "127.0.0.1:11111" },
            "log": { "loglevel": "info" },
            "customBlock": { "keep": true }
        });
        let extra = editable_extra_from_setting(&parsed);
        assert!(extra.get("inbounds").is_none());
        assert!(extra.get("api").is_none());
        assert!(extra.get("stats").is_none());
        assert_eq!(extra["policy"]["levels"]["0"]["connIdle"], 123);
        assert_eq!(extra["metrics"]["listen"], "127.0.0.1:11111");
        assert_eq!(extra["customBlock"]["keep"], true);
    }

    #[test]
    fn default_extra_seeds_policy_and_connection_stats() {
        let extra = crate::xray::config::default_extra();
        assert_eq!(extra["policy"]["levels"]["0"]["statsUserUplink"], true);
        assert_eq!(extra["policy"]["system"]["statsInboundUplink"], true);
        assert!(extra.get("inbounds").is_none());
    }

    #[tokio::test]
    async fn reads_multipart_form_fields() {
        use axum::body::Body;

        let boundary = "lumen-boundary";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"enable\"\r\n\r\ntrue\r\n--{boundary}--\r\n"
        );
        let request = Request::builder()
            .header(
                CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();
        let params = read_params(request).await.unwrap();
        assert_eq!(params.get("enable").map(String::as_str), Some("true"));
    }

    #[tokio::test]
    async fn tails_last_lines_from_log_file() {
        let path = std::env::temp_dir().join(format!("lumen-log-{}.log", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, "a\nb\nc\nd\n").await.unwrap();
        assert_eq!(
            crate::api::logs::tail_lines(&path, 2).await.unwrap(),
            ["c", "d"]
        );
        assert_eq!(
            crate::api::logs::tail_lines(&path, 10).await.unwrap(),
            ["a", "b", "c", "d"]
        );
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[test]
    fn ensure_sub_id_fills_missing_values_only() {
        let mut settings = json!({});
        ensure_sub_id(&mut settings);
        let generated = settings["subId"].as_str().unwrap();
        assert_eq!(generated.len(), 16);

        let mut existing = json!({ "subId": "keepme" });
        ensure_sub_id(&mut existing);
        assert_eq!(existing["subId"], json!("keepme"));
    }

    #[test]
    fn vless_enc_parses_auth_blocks_and_derives_wire_modes() {
        let text = "Authentication: X25519, not Post-Quantum\n\
{\n  \"decryption\": \"mlkem768x25519plus.native.600s.SERVERX\",\n  \"encryption\": \"mlkem768x25519plus.native.0rtt.CLIENTX\"\n}\n\n\
Authentication: ML-KEM-768, Post-Quantum\n\
{\n  \"decryption\": \"mlkem768x25519plus.native.600s.SERVERM\",\n  \"encryption\": \"mlkem768x25519plus.native.0rtt.CLIENTM\",\n}\n";
        let auths = vless_enc_auths(text);
        let ids: Vec<_> = auths.iter().map(|a| a["id"].as_str().unwrap()).collect();
        assert_eq!(
            ids,
            [
                "x25519",
                "x25519_xorpub",
                "x25519_random",
                "mlkem768",
                "mlkem768_xorpub",
                "mlkem768_random"
            ]
        );
        assert_eq!(
            auths[1]["decryption"],
            json!("mlkem768x25519plus.xorpub.600s.SERVERX")
        );
        assert_eq!(
            auths[2]["encryption"],
            json!("mlkem768x25519plus.random.0rtt.CLIENTX")
        );
        assert_eq!(
            auths[4]["decryption"],
            json!("mlkem768x25519plus.xorpub.600s.SERVERM")
        );
    }

    #[test]
    fn inbound_option_carries_fields_the_client_picker_filters_on() {
        let vless = sample(
            "vless",
            json!({}),
            json!({"network": "tcp", "security": "reality"}),
            true,
        );
        let option = inbound_option(&vless);
        assert_eq!(option["enable"], json!(true));
        assert_eq!(option["tlsFlowCapable"], json!(true));
        assert_eq!(option["ssMethod"], json!(""));

        let ss = sample(
            "shadowsocks",
            json!({"method": "2022-blake3-aes-128-gcm"}),
            json!({}),
            false,
        );
        let option = inbound_option(&ss);
        assert_eq!(option["enable"], json!(false));
        assert_eq!(option["network"], json!("tcp"));
        assert_eq!(option["security"], json!("none"));
        assert_eq!(option["tlsFlowCapable"], json!(false));
        assert_eq!(option["ssMethod"], json!("2022-blake3-aes-128-gcm"));
    }
}
