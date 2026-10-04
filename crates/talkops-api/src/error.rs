//! API error type: maps domain errors to HTTP status codes and a JSON body.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use talkops_core::error::CoreError;

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict(String),
    BadRequest(String),
    TooManyRequests,
    Internal(String),
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorBody {
    /// Machine-readable error code.
    pub error: &'static str,
    /// Human-readable details (English).
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "login required".to_owned(),
            ),
            ApiError::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "insufficient permissions".to_owned(),
            ),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found", "not found".to_owned()),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, "conflict", m),
            ApiError::BadRequest(m) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid", m),
            ApiError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "too many attempts, try again later".to_owned(),
            ),
            ApiError::Internal(m) => {
                tracing::error!(error = %m, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal server error".to_owned(),
                )
            }
        };
        (status, Json(ErrorBody { error, message })).into_response()
    }
}

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::NotFound => ApiError::NotFound,
            CoreError::Conflict(m) => ApiError::Conflict(format!("already exists ({m})")),
            CoreError::Validation(m) => ApiError::BadRequest(m),
            CoreError::Crypto(e) => ApiError::Internal(e.to_string()),
            CoreError::Db(e) => ApiError::Internal(e.to_string()),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        CoreError::from(err).into()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
