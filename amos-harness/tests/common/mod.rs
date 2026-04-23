//! Shared test helpers for HTTP integration tests.
//!
//! Builds the real Axum router via `create_server()` so tests exercise the
//! same stack production does — auth middleware, rate limiters, route wiring,
//! tool-registry dispatch. The tradeoff is a handful of background tasks
//! (automation cron, oauth refresh, relay sync sleeping-forever) per test;
//! they get cancelled when the tokio runtime drops at test end.
//!
//! Config comes from env vars (`AMOS__*`). `init_test_env()` sets the
//! essentials once per process so every test gets a consistent config without
//! needing to set them from its shell.

#![allow(dead_code)]

use amos_core::AppConfig;
use amos_harness::create_server;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::{Arc, Once};
use tower::ServiceExt;

static INIT: Once = Once::new();

/// Seed the process env with the vars `AppConfig::load()` needs to produce a
/// test-safe config. Runs exactly once per test binary.
fn init_test_env() {
    INIT.call_once(|| {
        let db = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set — point at a Postgres with pgvector");

        // SAFETY: called inside Once before any test threads touch env.
        unsafe {
            // AppConfig::load() reads AMOS__DATABASE__URL; mirror DATABASE_URL
            // into that so the same env works for sqlx and for the config loader.
            std::env::set_var("AMOS__DATABASE__URL", &db);

            // Redis URL: prefer an explicit AMOS__REDIS__URL; fall back to
            // REDIS_URL; then to localhost.
            if std::env::var("AMOS__REDIS__URL").is_err() {
                let redis_url = std::env::var("REDIS_URL")
                    .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
                std::env::set_var("AMOS__REDIS__URL", redis_url);
            }

            // Stop platform_sync from spawning (it starts iff the URL is
            // non-empty; default is http://localhost:4000 which isn't there).
            std::env::set_var("AMOS__PLATFORM__URL", "");

            // Deterministic JWT secret so `test_jwt()` and the harness agree.
            std::env::set_var(
                "AMOS__AUTH__JWT_SECRET",
                "test-jwt-secret-for-http-integration-only",
            );

            // Mark environment as non-production so TLS checks don't trip.
            std::env::set_var("AMOS__ENV", "test");
        }
    });
}

/// JWT claims matching `middleware::auth::Claims`. Kept in sync manually —
/// if the harness claims shape changes, this shape must change too.
#[derive(Debug, Serialize, Deserialize)]
struct TestClaims {
    sub: String,
    tenant_id: String,
    role: String,
    tenant_slug: String,
    iat: i64,
    exp: i64,
}

/// Build a fresh router + pool for one test.
///
/// Each `#[tokio::test]` gets its own tokio runtime, and PgPool is bound to
/// the runtime that created it, so we build from scratch per test (matches
/// `smoke_flows.rs`). Migrations are idempotent: sqlx tracks applied versions
/// in `_sqlx_migrations`, so re-running is cheap.
pub async fn build_app() -> (Router, Arc<AppConfig>, PgPool) {
    init_test_env();

    let config = Arc::new(AppConfig::load().expect("load test config"));

    let db_url = config.database.url.expose_secret().to_string();
    let pool = PgPool::connect(&db_url)
        .await
        .expect("connect to test Postgres");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("harness migrations apply cleanly");

    let redis_client = redis::Client::open(config.redis.url.as_str()).expect("build redis client");

    let router = create_server(config.clone(), pool.clone(), redis_client)
        .await
        .expect("create_server");

    (router, config, pool)
}

/// Issue a valid JWT using the same secret the harness is using.
pub fn test_jwt(config: &AppConfig) -> String {
    let secret = config.auth.jwt_secret.expose_secret().to_string();
    let now = Utc::now();
    let claims = TestClaims {
        sub: "test-user".to_string(),
        tenant_id: "test-tenant".to_string(),
        role: "admin".to_string(),
        tenant_slug: "test".to_string(),
        iat: now.timestamp(),
        exp: (now + ChronoDuration::hours(1)).timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("encode test JWT")
}

/// Drive the router with a single request. Returns (status, parsed body).
/// If the body isn't valid JSON the returned Value is Null — callers that
/// expect raw bytes should use `send_raw` instead.
pub async fn send_json(
    router: Router,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let (status, bytes) = send_raw(router, method, path, bearer, body).await;
    let parsed = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, parsed)
}

/// Like `send_json` but returns the raw body bytes (for HTML routes).
pub async fn send_raw(
    router: Router,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Vec<u8>) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {}", token));
    }
    let req_body = match body {
        Some(v) => Body::from(serde_json::to_vec(&v).expect("serialize body")),
        None => Body::empty(),
    };
    let req = builder.body(req_body).expect("build request");
    let resp = router.oneshot(req).await.expect("router oneshot");
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read response body");
    (status, bytes.to_vec())
}
