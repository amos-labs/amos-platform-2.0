//! Authentication middleware

use crate::state::AppState;
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Cookie name for harness session (matches platform's cookie name)
pub const SESSION_COOKIE: &str = "amos_session";

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

/// Authentication middleware for API routes.
///
/// Checks (in order): X-API-Key header, Authorization: Bearer header, amos_session cookie.
/// Returns 401 JSON for API callers, or redirects browsers to platform login.
pub async fn authenticate(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    // 1. Check X-API-Key header (for programmatic access)
    if let Some(api_key) = headers.get("X-API-Key") {
        if let Ok(key_str) = api_key.to_str() {
            if is_valid_api_key(key_str, &state).await {
                return Ok(next.run(request).await);
            }
        }
    }

    // 2. Check Authorization: Bearer <JWT> header
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

    // 3. Check amos_session cookie
    if let Some(token) = extract_cookie(&headers, SESSION_COOKIE) {
        if let Ok(claims) = validate_jwt(&token, &state) {
            let mut request = request;
            request.extensions_mut().insert(claims);
            return Ok(next.run(request).await);
        }
    }

    // Not authenticated — decide response format
    let is_browser = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/html"))
        .unwrap_or(false);

    if is_browser {
        // Redirect to platform login with return URL so user comes back after auth
        let platform_url = std::env::var("AMOS__PLATFORM__URL")
            .unwrap_or_else(|_| "https://app.amoslabs.com".into());
        // Build the harness origin from the Host header
        let harness_origin = headers
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .map(|host| format!("https://{}", host))
            .unwrap_or_default();
        let redirect_url = if harness_origin.is_empty() {
            format!("{}/login", platform_url)
        } else {
            let return_url = harness_origin.replace(':', "%3A").replace('/', "%2F");
            format!("{}/login?redirect={}", platform_url, return_url)
        };
        Err(Redirect::to(&redirect_url).into_response())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Missing or invalid authentication",
                "code": "unauthorized",
                "hint": "Provide 'Authorization: Bearer <jwt>', 'X-API-Key: <key>' header, or 'amos_session' cookie"
            })),
        )
            .into_response())
    }
}

/// Token exchange: validates a JWT from query param and sets a session cookie.
/// Used when platform redirects to harness with ?token=<jwt>.
pub async fn token_exchange(State(state): State<Arc<AppState>>, uri: Uri) -> Response {
    let token = uri
        .query()
        .and_then(|q| q.split('&').find_map(|pair| pair.strip_prefix("token=")));

    let Some(token) = token else {
        return (StatusCode::BAD_REQUEST, "Missing token parameter").into_response();
    };

    let Ok(claims) = validate_jwt(token, &state) else {
        return (StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response();
    };

    // Set cookie scoped to this harness subdomain
    let max_age = claims.exp - claims.iat;
    let cookie = format!(
        "{}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}; Secure",
        SESSION_COOKIE, token, max_age
    );

    ([(header::SET_COOKIE, cookie)], Redirect::to("/")).into_response()
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

/// Extract a named cookie from the Cookie header.
fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|s| {
            let s = s.trim();
            s.strip_prefix(&format!("{}=", name)).map(|v| v.to_string())
        })
}
