//! Authentication middleware

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::Deserialize;
use std::sync::OnceLock;

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

/// Authentication middleware (JWT + API key)
pub async fn authenticate(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Check for API key in header
    if let Some(api_key) = headers.get("X-API-Key") {
        if is_valid_api_key(api_key.to_str().unwrap_or("")) {
            return Ok(next.run(request).await);
        }
    }

    // Check for JWT token
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if is_valid_jwt(token) {
                    return Ok(next.run(request).await);
                }
            }
        }
    }

    // No valid authentication found
    Err(StatusCode::UNAUTHORIZED)
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

/// Validate API key using constant-time comparison against configured key.
/// Fails closed: if AMOS__AUTH__API_KEY is not set, all keys are rejected.
fn is_valid_api_key(api_key: &str) -> bool {
    match get_api_key() {
        Some(configured_key) => constant_time_eq(api_key.as_bytes(), configured_key.as_bytes()),
        None => {
            tracing::warn!("AMOS__AUTH__API_KEY not set; rejecting all API key auth");
            false
        }
    }
}

/// Validate JWT token by decoding and verifying signature, expiration, and issuer.
/// Fails closed: if AMOS__AUTH__JWT_SECRET is not set, all tokens are rejected.
fn is_valid_jwt(token: &str) -> bool {
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

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .is_ok()
}
