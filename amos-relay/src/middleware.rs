//! HTTP middleware for error handling and authentication.

use amos_core::AmosError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Error response wrapper to avoid orphan rule violations.
pub struct ErrorResponse(pub AmosError);

/// Convert ErrorResponse to HTTP response.
impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status_code = StatusCode::from_u16(self.0.status_code())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let message = self.0.to_string();

        let body = Json(json!({
            "error": message,
            "status": status_code.as_u16(),
        }));

        (status_code, body).into_response()
    }
}

/// API key authentication middleware (optional, for harness/agent endpoints).
///
/// This is a stub implementation. In production, you would:
/// 1. Extract API key from Authorization header
/// 2. Verify it against a database or hash
/// 3. Attach authenticated identity to request extensions
pub async fn api_key_auth() -> Result<(), AmosError> {
    // TODO: Implement API key verification
    // For now, just allow all requests
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_mapping() {
        let err = AmosError::NotFound {
            entity: "resource".to_string(),
            id: "123".to_string(),
        };
        let response = ErrorResponse(err).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let err = AmosError::Validation("invalid input".to_string());
        let response = ErrorResponse(err).into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let err = AmosError::Unauthorized("invalid credentials".to_string());
        let response = ErrorResponse(err).into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
