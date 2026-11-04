use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("未登录或会话已过期")]
    Unauthorized,
    #[error("CSRF 校验失败")]
    Csrf,
    #[error("尝试过于频繁，请稍后再试")]
    RateLimited,
    #[error("{0}")]
    Apply(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("服务器内部错误")]
    Internal(#[from] anyhow::Error),
    #[error("数据库错误")]
    Db(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "未登录或会话已过期".to_string()),
            AppError::Csrf => (StatusCode::FORBIDDEN, "CSRF 校验失败".to_string()),
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "尝试过于频繁，请稍后再试".to_string(),
            ),
            AppError::Apply(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Internal(err) => {
                tracing::error!(error = %err, "服务器内部错误");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "服务器内部错误".to_string(),
                )
            }
            AppError::Db(err) => {
                tracing::error!(error = %err, "数据库错误");
                (StatusCode::INTERNAL_SERVER_ERROR, "数据库错误".to_string())
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
