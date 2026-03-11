//! Site serving and management routes
//!
//! Public: Serves AI-generated websites and landing pages at /s/{slug}.
//! Management API: CRUD operations on sites and pages at /api/v1/sites.
//! Also handles form submissions that create records in collections.

use crate::{schema::SchemaEngine, sites::SiteEngine, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;

// ── Management API routes ───────────────────────────────────────────────

/// Build management API routes for /api/v1/sites
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(api_list_sites).post(api_create_site))
        .route(
            "/{slug}",
            get(api_get_site)
                .put(api_update_site)
                .delete(api_delete_site),
        )
        .route("/{slug}/publish", post(api_publish_site))
        .route("/{slug}/pages", get(api_list_pages).post(api_upsert_page))
        .route(
            "/{slug}/pages/{page_id}",
            get(api_get_page)
                .put(api_update_page)
                .delete(api_delete_page),
        )
}

// ── Request types ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSiteRequest {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub settings: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSiteRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub domain: Option<String>,
    pub settings: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub publish: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpsertPageRequest {
    pub path: String,
    pub title: String,
    pub description: Option<String>,
    pub html_content: String,
    pub css_content: Option<String>,
    pub js_content: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub form_collection: Option<String>,
}

// ── Management API handlers ─────────────────────────────────────────────

async fn api_list_sites(State(state): State<Arc<AppState>>) -> Result<Json<JsonValue>, StatusCode> {
    let engine = SiteEngine::new(state.db_pool.clone());
    let sites = engine
        .list_sites()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "sites": sites })))
}

async fn api_create_site(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSiteRequest>,
) -> Result<Json<JsonValue>, StatusCode> {
    let engine = SiteEngine::new(state.db_pool.clone());
    let site = engine
        .create_site(
            &req.name,
            &req.slug,
            req.description.as_deref(),
            req.settings,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to create site: {e}");
            StatusCode::BAD_REQUEST
        })?;
    Ok(Json(json!(site)))
}

async fn api_get_site(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Json<JsonValue>, StatusCode> {
    let engine = SiteEngine::new(state.db_pool.clone());
    let site = engine
        .get_site(&slug)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let pages = engine.list_pages(&slug).await.unwrap_or_default();
    Ok(Json(json!({ "site": site, "pages": pages })))
}

async fn api_update_site(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(req): Json<UpdateSiteRequest>,
) -> Result<Json<JsonValue>, StatusCode> {
    let engine = SiteEngine::new(state.db_pool.clone());
    let site = engine
        .update_site(
            &slug,
            req.name.as_deref(),
            req.description.as_deref(),
            req.domain.as_deref(),
            req.settings,
        )
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(json!(site)))
}

async fn api_delete_site(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Json<JsonValue>, StatusCode> {
    let result = sqlx::query("DELETE FROM sites WHERE slug = $1")
        .bind(&slug)
        .execute(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(json!({ "status": "deleted", "slug": slug })))
}

async fn api_publish_site(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(req): Json<PublishRequest>,
) -> Result<Json<JsonValue>, StatusCode> {
    let engine = SiteEngine::new(state.db_pool.clone());
    let site = engine
        .publish_site(&slug, req.publish)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(json!(site)))
}

async fn api_list_pages(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Json<JsonValue>, StatusCode> {
    let engine = SiteEngine::new(state.db_pool.clone());
    let pages = engine
        .list_pages(&slug)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(json!({ "pages": pages })))
}

async fn api_upsert_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(req): Json<UpsertPageRequest>,
) -> Result<Json<JsonValue>, StatusCode> {
    let engine = SiteEngine::new(state.db_pool.clone());
    let page = engine
        .upsert_page(
            &slug,
            &req.path,
            &req.title,
            req.description.as_deref(),
            &req.html_content,
            req.css_content.as_deref(),
            req.js_content.as_deref(),
            req.meta_title.as_deref(),
            req.meta_description.as_deref(),
            req.form_collection.as_deref(),
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to upsert page: {e}");
            StatusCode::BAD_REQUEST
        })?;
    Ok(Json(json!(page)))
}

async fn api_get_page(
    State(state): State<Arc<AppState>>,
    Path((_slug, page_id)): Path<(String, String)>,
) -> Result<Json<JsonValue>, StatusCode> {
    // page_id is the UUID; look up by id directly
    let uuid = uuid::Uuid::parse_str(&page_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let row = sqlx::query_as::<_, PageRow>(
        r#"SELECT id, site_id, path, title, description, html_content,
                  css_content, js_content, meta_title, meta_description, og_image,
                  form_collection, sort_order, is_published, created_at, updated_at
           FROM pages WHERE id = $1"#,
    )
    .bind(uuid)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!(row)))
}

async fn api_update_page(
    State(state): State<Arc<AppState>>,
    Path((slug, _page_id)): Path<(String, String)>,
    Json(req): Json<UpsertPageRequest>,
) -> Result<Json<JsonValue>, StatusCode> {
    // Use upsert_page which handles ON CONFLICT
    let engine = SiteEngine::new(state.db_pool.clone());
    let page = engine
        .upsert_page(
            &slug,
            &req.path,
            &req.title,
            req.description.as_deref(),
            &req.html_content,
            req.css_content.as_deref(),
            req.js_content.as_deref(),
            req.meta_title.as_deref(),
            req.meta_description.as_deref(),
            req.form_collection.as_deref(),
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to update page: {e}");
            StatusCode::BAD_REQUEST
        })?;
    Ok(Json(json!(page)))
}

async fn api_delete_page(
    State(state): State<Arc<AppState>>,
    Path((_slug, page_id)): Path<(String, String)>,
) -> Result<Json<JsonValue>, StatusCode> {
    let uuid = uuid::Uuid::parse_str(&page_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let result = sqlx::query("DELETE FROM pages WHERE id = $1")
        .bind(uuid)
        .execute(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(json!({ "status": "deleted", "page_id": page_id })))
}

/// Row type for direct page queries (sqlx::FromRow)
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
struct PageRow {
    pub id: uuid::Uuid,
    pub site_id: uuid::Uuid,
    pub path: String,
    pub title: String,
    pub description: Option<String>,
    pub html_content: String,
    pub css_content: Option<String>,
    pub js_content: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub og_image: Option<String>,
    pub form_collection: Option<String>,
    pub sort_order: i32,
    pub is_published: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Serve a site's index page: GET /s/{slug}
pub async fn serve_site_index(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let engine = SiteEngine::new(state.db_pool.clone());

    let site = engine
        .get_site(&slug)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if !site.is_published {
        return Err(StatusCode::NOT_FOUND);
    }

    let (site, page) = engine
        .get_page(&slug, "/")
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if !page.is_published {
        return Err(StatusCode::NOT_FOUND);
    }

    let html = engine.render_page(&site, &page);
    Ok(Html(html))
}

/// Serve a site sub-page: GET /s/{slug}/{*path}
pub async fn serve_site_page(
    State(state): State<Arc<AppState>>,
    Path((slug, path)): Path<(String, String)>,
) -> Result<Html<String>, StatusCode> {
    let engine = SiteEngine::new(state.db_pool.clone());

    let site = engine
        .get_site(&slug)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if !site.is_published {
        return Err(StatusCode::NOT_FOUND);
    }

    let page_path = format!("/{}", path);
    let (site, page) = engine
        .get_page(&slug, &page_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if !page.is_published {
        return Err(StatusCode::NOT_FOUND);
    }

    let html = engine.render_page(&site, &page);
    Ok(Html(html))
}

/// Handle form submissions: POST /s/{slug}/submit/{collection}
///
/// Creates a record in the specified collection from the form data.
pub async fn handle_form_submit(
    State(state): State<Arc<AppState>>,
    Path((slug, collection)): Path<(String, String)>,
    Json(data): Json<JsonValue>,
) -> Result<Json<JsonValue>, StatusCode> {
    // Verify the site exists and is published
    let site_engine = SiteEngine::new(state.db_pool.clone());
    let site = site_engine
        .get_site(&slug)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if !site.is_published {
        return Err(StatusCode::NOT_FOUND);
    }

    // Verify that a published page on this site has form_collection matching the requested collection
    let has_form_collection: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
            SELECT 1 FROM pages
            WHERE site_id = $1
              AND form_collection = $2
              AND is_published = true
        )"#,
    )
    .bind(site.id)
    .bind(&collection)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_form_collection {
        return Err(StatusCode::FORBIDDEN);
    }

    // Create a record in the target collection
    let schema_engine = SchemaEngine::new(state.db_pool.clone());
    let record = schema_engine
        .create_record(&collection, data)
        .await
        .map_err(|e| {
            tracing::error!(
                "Form submission failed for site '{}', collection '{}': {}",
                slug,
                collection,
                e
            );
            StatusCode::BAD_REQUEST
        })?;

    Ok(Json(json!({
        "success": true,
        "record_id": record.id.to_string(),
        "message": "Submitted successfully!"
    })))
}
