use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Map AmosError to HTTP response with proper status code and JSON body
pub fn handle_error(error: amos_core::AmosError) -> Response {
    let status_code = error.status_code();
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let message = error.to_string();

    tracing::error!("API Error [{}]: {}", status_code, message);

    let error_response = json!({
        "error": status_code,
        "message": message,
    });

    (status, Json(error_response)).into_response()
}

/// Helper to create standard error responses
pub fn error_response(
    status: StatusCode,
    error_type: &str,
    message: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(json!({
            "error": error_type,
            "message": message,
            "status": status.as_u16(),
        })),
    )
}

/// Trait to make error handling more ergonomic in handlers
pub trait IntoAmosError<T> {
    fn map_amos_error(self) -> Result<T, amos_core::AmosError>;
}

impl<T, E> IntoAmosError<T> for Result<T, E>
where
    E: std::fmt::Display,
{
    fn map_amos_error(self) -> Result<T, amos_core::AmosError> {
        self.map_err(|e| amos_core::AmosError::Internal(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response() {
        let (status, json) = error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Invalid input",
        );
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_handle_validation_error() {
        let error = amos_core::AmosError::Validation("Invalid data".to_string());
        let response = handle_error(error);
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn test_handle_not_found_error() {
        let error = amos_core::AmosError::NotFound {
            entity: "User".to_string(),
            id: "123".to_string(),
        };
        let response = handle_error(error);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_handle_unauthorized_error() {
        let error = amos_core::AmosError::Unauthorized("Not authorized".to_string());
        let response = handle_error(error);
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_handle_insufficient_stake_error() {
        let error = amos_core::AmosError::InsufficientStake {
            have: 100,
            need: 500,
        };
        let response = handle_error(error);
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn test_handle_internal_error() {
        let error = amos_core::AmosError::Internal("Something went wrong".to_string());
        let response = handle_error(error);
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
