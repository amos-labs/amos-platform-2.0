//! Authentication middleware

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};

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

/// Validate API key
fn is_valid_api_key(api_key: &str) -> bool {
    // TODO: Implement actual API key validation
    // For now, accept any non-empty key
    !api_key.is_empty()
}

/// Validate JWT token
fn is_valid_jwt(token: &str) -> bool {
    // TODO: Implement actual JWT validation
    // For now, accept any non-empty token
    !token.is_empty()
}
