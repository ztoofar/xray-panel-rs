use std::net::{IpAddr, SocketAddr};

use axum::{
    extract::{ConnectInfo, State},
    http::{header::SET_COOKIE, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    auth::{self, AuthCtx, SESSION_COOKIE},
    db::{self, models::now},
    error::{AppError, AppResult},
    state::AppState,
};

const MAX_ATTEMPTS: usize = 5;
const WINDOW_SECS: i64 = 60;

#[derive(Deserialize)]
pub struct LoginBody {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsBody {
    pub username: Option<String>,
    pub current_password: String,
    pub new_password: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<LoginBody>,
) -> AppResult<Response> {
    let ip = addr.ip();
    let real_ip = crate::notify::real_ip(&headers, ip);
    if is_rate_limited(&state, ip).await {
        tracing::warn!(%ip, "panel login rate limited");
        return Err(AppError::RateLimited);
    }

    let user = db::users::find_by_username(&state.db, body.username.trim()).await?;
    let verified = match &user {
        Some(user) => auth::verify_password(&body.password, &user.password_hash),
        None => {
            // Burn comparable time so missing users are not distinguishable by latency.
            let _ = auth::verify_password(&body.password, dummy_hash());
            false
        }
    };

    let Some(user) = user.filter(|_| verified) else {
        tracing::warn!(username = ?body.username.trim(), %ip, "panel login failed");
        record_failure(&state, ip).await;
        crate::notify::spawn_failed_login(state.clone(), real_ip, body.username.trim(), "面板登录");
        return Err(AppError::Unauthorized);
    };

    clear_failures(&state, ip).await;
    let (cookie, csrf) = new_session(&state, user.id).await?;
    tracing::info!(username = ?user.username, %ip, "panel login succeeded");
    let mut response = Json(json!({
        "authenticated": true,
        "username": user.username,
        "csrf": csrf,
    }))
    .into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("cookie header is valid"),
    );
    Ok(response)
}

pub async fn session_info(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    if let Some(token) = auth::cookie(&headers, SESSION_COOKIE) {
        if let Ok(Some(session)) = db::sessions::find(&state.db, &auth::hash_token(&token)).await {
            if session.expires_at > now() {
                return Json(json!({
                    "authenticated": true,
                    "username": session.username,
                    "csrf": session.csrf,
                }))
                .into_response();
            }
        }
    }
    Json(json!({ "authenticated": false })).into_response()
}

pub async fn logout(State(state): State<AppState>, headers: axum::http::HeaderMap) -> Response {
    if let Some(token) = auth::cookie(&headers, SESSION_COOKIE) {
        let hash = auth::hash_token(&token);
        if let Ok(Some(session)) = db::sessions::find(&state.db, &hash).await {
            tracing::info!(username = ?session.username, "panel logout");
        }
        let _ = db::sessions::delete(&state.db, &hash).await;
    }
    let mut response = Json(json!({ "ok": true })).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&auth::clear_cookie()).expect("cookie header is valid"),
    );
    response
}

pub async fn change_credentials(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthCtx>,
    Json(body): Json<CredentialsBody>,
) -> AppResult<Response> {
    let user = db::users::get(&state.db, ctx.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !auth::verify_password(&body.current_password, &user.password_hash) {
        return Err(AppError::BadRequest("当前密码不正确".into()));
    }

    let username = body
        .username
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(&user.username);
    if username.len() > 64 {
        return Err(AppError::BadRequest("用户名过长".into()));
    }

    let password_hash = match body.new_password.as_deref().filter(|v| !v.is_empty()) {
        Some(password) => {
            if password.len() < 8 {
                return Err(AppError::BadRequest("新密码至少需要 8 个字符".into()));
            }
            auth::hash_password(password)?
        }
        None => user.password_hash.clone(),
    };

    db::users::update(&state.db, user.id, username, &password_hash).await?;
    db::sessions::delete_for_user(&state.db, user.id).await?;
    let (cookie, csrf) = new_session(&state, user.id).await?;
    tracing::info!(username = ?username, "admin credentials changed");

    let mut response = Json(json!({
        "ok": true,
        "username": username,
        "csrf": csrf,
    }))
    .into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("cookie header is valid"),
    );
    Ok(response)
}

async fn new_session(state: &AppState, user_id: i64) -> AppResult<(String, String)> {
    let token = auth::random_token(32);
    let csrf = auth::random_token(24);
    let ts = now();
    let ttl = db::settings::session_ttl_secs(&state.db).await;
    db::sessions::create(
        &state.db,
        &auth::hash_token(&token),
        user_id,
        &csrf,
        ts,
        ts + ttl,
    )
    .await?;
    let cookie = auth::session_cookie(&token, ttl, state.cfg.secure_cookie);
    Ok((cookie, csrf))
}

pub(crate) async fn is_rate_limited(state: &AppState, ip: IpAddr) -> bool {
    let mut limiter = state.login_limiter.lock().await;
    let cutoff = now() - WINDOW_SECS;
    let entry = limiter.entry(ip).or_default();
    entry.retain(|ts| *ts > cutoff);
    entry.len() >= MAX_ATTEMPTS
}

pub(crate) async fn record_failure(state: &AppState, ip: IpAddr) {
    let mut limiter = state.login_limiter.lock().await;
    // Bound the map so forged source addresses cannot grow it without limit.
    if limiter.len() > 1024 {
        let cutoff = now() - WINDOW_SECS;
        limiter.retain(|_, attempts| {
            attempts.retain(|ts| *ts > cutoff);
            !attempts.is_empty()
        });
    }
    limiter.entry(ip).or_default().push(now());
}

pub(crate) async fn clear_failures(state: &AppState, ip: IpAddr) {
    state.login_limiter.lock().await.remove(&ip);
}

pub(crate) fn dummy_hash() -> &'static str {
    static HASH: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
        auth::hash_password("not-a-real-password").expect("argon2 hash")
    });
    HASH.as_str()
}
