//! Authentication middleware

use crate::state::AppState;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// JWT claims (matches platform's auth::Claims)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub tenant_id: String,
    pub role: String,
    pub tenant_slug: String,
    pub iat: i64,
    pub exp: i64,
}

/// Authentication middleware (JWT + API key)
pub async fn authenticate(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    // Check X-API-Key header
    if let Some(api_key) = headers.get("X-API-Key") {
        if let Ok(key_str) = api_key.to_str() {
            if is_valid_api_key(key_str, &state).await {
                return Ok(next.run(request).await);
            }
        }
    }

    // Check Authorization: Bearer <JWT>
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if let Ok(claims) = validate_jwt(token, &state) {
                    let mut request = request;
                    request.extensions_mut().insert(claims);
                    return Ok(next.run(request).await);
                }
            }
        }
    }

    Err((
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "error": "Missing or invalid authentication",
            "code": "unauthorized",
            "hint": "Provide 'Authorization: Bearer <jwt>' or 'X-API-Key: <key>' header"
        })),
    )
        .into_response())
}

fn validate_jwt(token: &str, state: &AppState) -> Result<Claims, ()> {
    let jwt_secret = state.config.auth.jwt_secret.expose_secret();
    let validation = Validation::default();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| ())
}

/// Validate API key against database (check api_keys table)
async fn is_valid_api_key(api_key: &str, state: &AppState) -> bool {
    if api_key.is_empty() {
        return false;
    }
    // Hash the key and look it up in the database
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    let key_hash = hex::encode(hasher.finalize());

    sqlx::query("SELECT 1 FROM api_keys WHERE key_hash = $1 AND revoked = FALSE AND (expires_at IS NULL OR expires_at > NOW())")
        .bind(&key_hash)
        .fetch_optional(&state.db_pool)
        .await
        .ok()
        .flatten()
        .is_some()
}
