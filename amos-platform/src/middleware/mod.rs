//! HTTP middleware for authentication and error handling.

use amos_core::AmosError;
use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::error;

/// Global error handler middleware.
///
/// Maps AmosError variants to appropriate HTTP status codes and JSON responses.
pub async fn error_handler(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    next.run(req).await
}

/// JSON error response format.
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}

pub fn amos_error_to_response(err: &AmosError) -> (StatusCode, Json<ErrorResponse>) {
    let status = StatusCode::from_u16(err.status_code())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let code = match &err {
        AmosError::Unauthorized(_) => "unauthorized",
        AmosError::Forbidden(_) => "forbidden",
        AmosError::NotFound { .. } => "not_found",
        AmosError::Validation(_) => "validation_error",
        AmosError::Database(_) => "database_error",
        AmosError::SolanaRpc(_) => "solana_error",
        AmosError::InsufficientStake { .. } => "insufficient_stake",
        AmosError::NoRevenueToClaim => "no_revenue",
        _ => "internal_error",
    };

    let response = ErrorResponse {
        error: err.to_string(),
        code: code.to_string(),
        details: None,
    };

    (status, Json(response))
}

/// Authentication middleware using API keys.
///
/// TODO: Implement actual API key validation against database.
pub async fn require_api_key(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // Extract Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    if let Some(auth) = auth_header {
        if auth.starts_with("Bearer ") {
            let _api_key = &auth[7..];
            // TODO: Validate API key against database
            // For now, accept any Bearer token
            return next.run(req).await;
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: "Missing or invalid API key".into(),
            code: "unauthorized".into(),
            details: Some("Provide 'Authorization: Bearer <api_key>' header".into()),
        }),
    ).into_response()
}

/// Admin authentication middleware.
///
/// TODO: Implement role-based access control.
pub async fn require_admin(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // TODO: Check if authenticated user has admin role
    next.run(req).await
}

/// Harness service authentication.
///
/// Validates that the request comes from a legitimate harness container.
/// TODO: Implement JWT or mTLS validation.
pub async fn require_harness_auth(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let auth_header = req
        .headers()
        .get("X-Harness-Token")
        .and_then(|h| h.to_str().ok());

    if let Some(_token) = auth_header {
        // TODO: Validate harness token
        return next.run(req).await;
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: "Missing or invalid harness token".into(),
            code: "unauthorized".into(),
            details: Some("Provide 'X-Harness-Token' header".into()),
        }),
    ).into_response()
}
