//! HTTP integration tests — Phase 1a of the test harness plan.
//!
//! Spins up the real Axum router via `create_server()` and drives every
//! Phase-1a-covered route with `tower::ServiceExt::oneshot`. Covers routing,
//! auth middleware, JSON serde, and tool-registry dispatch. No Bedrock,
//! no agent sidecar, no LLM.
//!
//! Notes on divergence from TEST_HARNESS_PLAN.md:
//! - The plan lists `POST /api/v1/collections` and `POST /api/v1/collections/{slug}/records`.
//!   The harness actually mounts the record CRUD at `/api/v1/data`, and collection
//!   creation is not exposed over HTTP (it happens via the `define_collection`
//!   agent tool or migrations). We seed a collection via `SchemaEngine` directly
//!   and cover the HTTP record-create path under `/api/v1/data/{collection}`.

mod common;

use amos_harness::schema::{FieldDefinition, FieldType, SchemaEngine};
use common::{build_app, send_json, send_raw, test_jwt};
use serde_json::json;
use uuid::Uuid;

fn unique(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4().simple())
}

// ═════════════════════════════════════════════════════════════════════════
// Public routes — no auth required.
// ═════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn health_returns_200() {
    let (router, _cfg, _pool) = build_app().await;
    let (status, _body) = send_json(router, "GET", "/health", None, None).await;
    assert_eq!(status, 200, "/health must respond 200 for load balancers");
}

#[tokio::test]
async fn list_tools_contains_create_landing_page() {
    let (router, _cfg, _pool) = build_app().await;
    let (status, body) = send_json(router, "GET", "/api/v1/tools", None, None).await;
    assert_eq!(status, 200);

    let names: Vec<&str> = body["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(
        names.contains(&"create_landing_page"),
        "tools listing should include create_landing_page, got: {:?}",
        names
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Auth enforcement — protected routes reject unauth'd requests.
// ═════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn sites_api_rejects_unauthenticated() {
    let (router, _cfg, _pool) = build_app().await;
    let (status, _body) = send_json(
        router,
        "POST",
        "/api/v1/sites",
        None,
        Some(json!({ "name": "x", "slug": "x" })),
    )
    .await;
    assert_eq!(
        status, 401,
        "protected sites route must reject unauth'd POST with 401"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Sites: create → add page → publish → render publicly.
// ═════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn site_create_page_publish_renders_publicly() {
    let (router, cfg, pool) = build_app().await;
    let token = test_jwt(&cfg);
    let slug = unique("http-site");

    // 1) Create the site.
    let (status, _body) = send_json(
        router.clone(),
        "POST",
        "/api/v1/sites",
        Some(&token),
        Some(json!({ "name": "HTTP Site", "slug": slug })),
    )
    .await;
    assert_eq!(status, 200, "POST /api/v1/sites must succeed");

    // Site row exists.
    let site_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sites WHERE slug = $1")
        .bind(&slug)
        .fetch_one(&pool)
        .await
        .expect("count sites");
    assert_eq!(
        site_count, 1,
        "site row must exist after POST /api/v1/sites"
    );

    // 2) Upsert the index page.
    let page_path = format!("/api/v1/sites/{}/pages", slug);
    let (status, _body) = send_json(
        router.clone(),
        "POST",
        &page_path,
        Some(&token),
        Some(json!({
            "path": "/",
            "title": "Home",
            "html_content": "<h1>Hello from HTTP</h1>"
        })),
    )
    .await;
    assert_eq!(
        status, 200,
        "POST /api/v1/sites/{{slug}}/pages must succeed"
    );

    // Page row exists.
    let page_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pages p JOIN sites s ON p.site_id = s.id WHERE s.slug = $1 AND p.path = '/'",
    )
    .bind(&slug)
    .fetch_one(&pool)
    .await
    .expect("count pages");
    assert_eq!(page_count, 1, "page row must exist after POST pages");

    // 3) Publish the site.
    let pub_path = format!("/api/v1/sites/{}/publish", slug);
    let (status, _body) = send_json(
        router.clone(),
        "POST",
        &pub_path,
        Some(&token),
        Some(json!({ "publish": true })),
    )
    .await;
    assert_eq!(status, 200, "POST publish must succeed");

    let is_published: bool = sqlx::query_scalar("SELECT is_published FROM sites WHERE slug = $1")
        .bind(&slug)
        .fetch_one(&pool)
        .await
        .expect("read is_published");
    assert!(is_published, "publish must flip is_published to true");

    // 4) Public render via /s/{slug} — the URL a customer actually hits.
    let public_path = format!("/s/{}", slug);
    let (status, bytes) = send_raw(router, "GET", &public_path, None, None).await;
    assert_eq!(status, 200, "/s/{{slug}} must return 200 once published");
    let html = String::from_utf8_lossy(&bytes);
    assert!(
        html.contains("Hello from HTTP"),
        "rendered page must include user-supplied body, got: {}",
        html
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Data API: seed a collection via engine, POST a record via HTTP.
// ═════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn post_data_collection_creates_record_row() {
    let (router, cfg, pool) = build_app().await;
    let token = test_jwt(&cfg);
    let collection_name = format!("http_leads_{}", Uuid::new_v4().simple());

    // Seed the collection directly — creation isn't exposed over HTTP.
    let engine = SchemaEngine::new(pool.clone());
    engine
        .define_collection(
            &collection_name,
            "HTTP Leads",
            Some("http integration test"),
            vec![FieldDefinition {
                name: "email".to_string(),
                display_name: "Email".to_string(),
                field_type: FieldType::Email,
                required: true,
                unique: false,
                default_value: None,
                description: None,
                options: json!({}),
            }],
        )
        .await
        .expect("define_collection ok");

    let url = format!("/api/v1/data/{}", collection_name);
    let (status, _body) = send_json(
        router,
        "POST",
        &url,
        Some(&token),
        Some(json!({ "email": "lead@example.com" })),
    )
    .await;
    assert_eq!(
        status, 201,
        "POST /api/v1/data/{{coll}} must return 201 Created"
    );

    let record_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM records r JOIN collections c ON r.collection_id = c.id WHERE c.name = $1",
    )
    .bind(&collection_name)
    .fetch_one(&pool)
    .await
    .expect("count records");
    assert_eq!(
        record_count, 1,
        "exactly one record should exist in the seeded collection"
    );
}
