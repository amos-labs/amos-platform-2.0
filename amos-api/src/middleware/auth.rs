use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;

/// Authentication middleware
/// Validates Bearer tokens (JWT) or API keys
pub struct AuthMiddleware;

impl AuthMiddleware {
    /// Middleware function to validate authentication
    pub async fn validate(
        State(state): State<Arc<AppState>>,
        headers: HeaderMap,
        request: Request,
        next: Next,
    ) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
        // Extract Authorization header
        let auth_header = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok());

        if let Some(auth_value) = auth_header {
            // Check for Bearer token
            if auth_value.starts_with("Bearer ") {
                let token = &auth_value[7..];
                if validate_jwt(token).await {
                    return Ok(next.run(request).await);
                }
            }
            // Check for API key
            else if auth_value.starts_with("ApiKey ") {
                let api_key = &auth_value[7..];
                if validate_api_key(&state, api_key).await {
                    return Ok(next.run(request).await);
                }
            }
        }

        // Check for API key in X-API-Key header (alternative)
        if let Some(api_key) = headers.get("x-api-key").and_then(|h| h.to_str().ok()) {
            if validate_api_key(&state, api_key).await {
                return Ok(next.run(request).await);
            }
        }

        // No valid authentication found
        Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Unauthorized",
                "message": "Valid authentication required. Provide Bearer token or API key."
            })),
        ))
    }
}

/// Validate JWT token
/// TODO: Implement actual JWT validation with jsonwebtoken crate
async fn validate_jwt(token: &str) -> bool {
    // Placeholder validation
    // In production:
    // 1. Decode JWT
    // 2. Verify signature with public key
    // 3. Check expiration
    // 4. Validate claims (issuer, audience, etc.)

    tracing::debug!("Validating JWT token: {}", &token[..10.min(token.len())]);

    // For now, accept any non-empty token
    // TODO: Implement real JWT validation
    !token.is_empty()
}

/// Validate API key against database
/// TODO: Implement actual API key lookup in database
async fn validate_api_key(state: &AppState, api_key: &str) -> bool {
    // Placeholder validation
    // In production:
    // 1. Hash the API key
    // 2. Look up in database
    // 3. Check if active and not expired
    // 4. Update last_used timestamp
    // 5. Check rate limits

    tracing::debug!("Validating API key: {}", &api_key[..10.min(api_key.len())]);

    // For now, accept any non-empty key
    // TODO: Implement real API key validation with database lookup
    !api_key.is_empty()
}

/// Extract user ID from JWT token
/// TODO: Implement actual JWT parsing
pub fn extract_user_id_from_token(token: &str) -> Option<String> {
    // Placeholder
    // In production, decode JWT and extract user_id from claims
    Some("user_123".to_string())
}

/// Extract permissions from JWT token
/// TODO: Implement actual JWT parsing
pub fn extract_permissions_from_token(token: &str) -> Vec<String> {
    // Placeholder
    // In production, decode JWT and extract permissions from claims
    vec!["read".to_string(), "write".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_jwt() {
        assert!(validate_jwt("valid_token").await);
        assert!(!validate_jwt("").await);
    }

    #[test]
    fn test_extract_user_id() {
        let user_id = extract_user_id_from_token("token");
        assert!(user_id.is_some());
    }

    #[test]
    fn test_extract_permissions() {
        let perms = extract_permissions_from_token("token");
        assert!(!perms.is_empty());
    }
}
