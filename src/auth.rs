use argon2::{
    password_hash::{PasswordHash, PasswordHasher, SaltString},
    Algorithm, Argon2, Params, PasswordVerifier, Version,
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use rand::RngCore;
use serde_json::json;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{
    db::{self, models::now},
    error::AppError,
    state::AppState,
};

pub const SESSION_COOKIE: &str = "xpanel_session";
pub const SESSION_TTL: i64 = 7 * 24 * 3600;

#[derive(Debug, Clone)]
pub struct AuthCtx {
    pub user_id: i64,
}

/// 8 MiB memory cost keeps a login affordable on a 512 MiB VPS.
fn argon2() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(8192, 2, 1, None).expect("valid argon2 params"),
    )
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Ok(argon2()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    argon2()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn random_token(bytes: usize) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let mut buf = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            if key == name {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub fn session_cookie(token: &str, max_age: i64, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}")
}

pub fn clear_cookie() -> String {
    format!(
        "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; \
         Expires=Thu, 01 Jan 1970 00:00:00 GMT"
    )
}

fn ct_eq(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

fn requires_csrf(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

pub async fn require_auth(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let Some(token) = cookie(req.headers(), SESSION_COOKIE) else {
        return AppError::Unauthorized.into_response();
    };
    let session = match db::sessions::find(&state.db, &hash_token(&token)).await {
        Ok(Some(session)) if session.expires_at > now() => session,
        Ok(_) => return AppError::Unauthorized.into_response(),
        Err(err) => {
            tracing::error!(error = %err, "session lookup failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
                .into_response();
        }
    };

    if requires_csrf(req.method()) {
        let provided = req
            .headers()
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if !ct_eq(provided, &session.csrf) {
            return AppError::Csrf.into_response();
        }
    }

    req.extensions_mut().insert(AuthCtx {
        user_id: session.user_id,
    });
    next.run(req).await
}
