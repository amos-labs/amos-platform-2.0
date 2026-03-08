use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::Deserialize;
use serde_json::json;
use std::sync::{Arc, OnceLock};

use crate::state::AppState;

static API_KEY: OnceLock<Option<String>> = OnceLock::new();
static JWT_SECRET: OnceLock<Option<String>> = OnceLock::new();

fn get_api_key() -> &'static Option<String> {
    API_KEY.get_or_init(|| std::env::var("AMOS__AUTH__API_KEY").ok())
}

fn get_jwt_secret() -> &'static Option<String> {
    JWT_SECRET.get_or_init(|| std::env::var("AMOS__AUTH__JWT_SECRET").ok())
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    #[serde(default)]
    iss: Option<String>,
    #[serde(default)]
    permissions: Vec<String>,
}

/// Authentication middleware
/// Validates Bearer tokens (JWT) or API keys
pub struct AuthMiddleware;

impl AuthMiddleware {
    /// Middleware function to validate authentication
    pub async fn validate(
        State(_state): State<Arc<AppState>>,
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
                if validate_api_key(api_key).await {
                    return Ok(next.run(request).await);
                }
            }
        }

        // Check for API key in X-API-Key header (alternative)
        if let Some(api_key) = headers.get("x-api-key").and_then(|h| h.to_str().ok()) {
            if validate_api_key(api_key).await {
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

/// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Validate JWT token by decoding and verifying signature, expiration, and issuer.
/// Fails closed: if AMOS__AUTH__JWT_SECRET is not set, all tokens are rejected.
async fn validate_jwt(token: &str) -> bool {
    let secret = match get_jwt_secret() {
        Some(s) => s,
        None => {
            tracing::warn!("AMOS__AUTH__JWT_SECRET not set; rejecting all JWT auth");
            return false;
        }
    };

    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&["amos"]);
    validation.validate_exp = true;

    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    ) {
        Ok(_) => true,
        Err(e) => {
            tracing::debug!("JWT validation failed: {}", e);
            false
        }
    }
}

/// Validate API key using constant-time comparison against configured key.
/// Fails closed: if AMOS__AUTH__API_KEY is not set, all keys are rejected.
async fn validate_api_key(api_key: &str) -> bool {
    match get_api_key() {
        Some(configured_key) => constant_time_eq(api_key.as_bytes(), configured_key.as_bytes()),
        None => {
            tracing::warn!("AMOS__AUTH__API_KEY not set; rejecting all API key auth");
            false
        }
    }
}

/// Extract user ID from JWT token by decoding the `sub` claim.
/// Returns None if the token is invalid or the secret is not configured.
pub fn extract_user_id_from_token(token: &str) -> Option<String> {
    let secret = get_jwt_secret().as_ref()?;

    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&["amos"]);
    validation.validate_exp = true;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .ok()
    .map(|data| data.claims.sub)
}

/// Extract permissions from JWT token by decoding the `permissions` claim.
/// Returns empty vec if the token is invalid or the secret is not configured.
pub fn extract_permissions_from_token(token: &str) -> Vec<String> {
    let secret = match get_jwt_secret().as_ref() {
        Some(s) => s,
        None => return vec![],
    };

    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&["amos"]);
    validation.validate_exp = true;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .ok()
    .map(|data| data.claims.permissions)
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_jwt_rejects_empty() {
        assert!(!validate_jwt("").await);
    }

    #[tokio::test]
    async fn test_validate_jwt_rejects_garbage() {
        assert!(!validate_jwt("not_a_valid_jwt").await);
    }

    #[test]
    fn test_extract_user_id_rejects_invalid() {
        let user_id = extract_user_id_from_token("invalid_token");
        assert!(user_id.is_none());
    }

    #[test]
    fn test_extract_permissions_rejects_invalid() {
        let perms = extract_permissions_from_token("invalid_token");
        assert!(perms.is_empty());
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
    }
}
