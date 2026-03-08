//! Revision and template management routes.
//!
//! - Revision endpoints: list, get, create, revert for any entity type
//! - Template endpoints: list, get, versions, check updates, subscriptions

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::revisions::{
    CreateRevisionRequest, RevisionService, TemplateService,
};
use crate::state::AppState;

// ── Query / Body types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListRevisionsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
struct ListTemplatesQuery {
    entity_type: Option<String>,
}

#[derive(Deserialize)]
struct CreateRevisionBody {
    snapshot: serde_json::Value,
    change_type: Option<String>,
    changed_by: Option<String>,
    change_summary: Option<String>,
}

#[derive(Deserialize)]
struct RevertRequestBody {
    target_version: i32,
    changed_by: Option<String>,
}

// ── Error helper ────────────────────────────────────────────────────────

fn map_err(e: amos_core::AmosError) -> (StatusCode, Json<serde_json::Value>) {
    let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(json!({ "error": e.to_string(), "status": status.as_u16() })))
}

// ── Revision handlers ───────────────────────────────────────────────────

async fn list_revisions(
    Path((entity_type, entity_id)): Path<(String, Uuid)>,
    Query(params): Query<ListRevisionsQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let service = RevisionService::new(state.db_pool.clone());
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let response = service
        .list_revisions(&entity_type, entity_id, limit, offset)
        .await
        .map_err(map_err)?;

    Ok(Json(json!({
        "revisions": response.revisions,
        "total": response.total,
        "limit": limit,
        "offset": offset,
    })))
}

async fn get_latest_revision(
    Path((entity_type, entity_id)): Path<(String, Uuid)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let service = RevisionService::new(state.db_pool.clone());

    let revision = service
        .get_latest_revision(&entity_type, entity_id)
        .await
        .map_err(map_err)?;

    match revision {
        Some(rev) => Ok(Json(json!({ "revision": rev }))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": format!("No revisions found for {} {}", entity_type, entity_id),
                "status": 404
            })),
        )),
    }
}

async fn get_revision(
    Path((entity_type, entity_id, version)): Path<(String, Uuid, i32)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let service = RevisionService::new(state.db_pool.clone());

    let revision = service
        .get_revision(&entity_type, entity_id, version)
        .await
        .map_err(map_err)?;

    Ok(Json(json!({ "revision": revision })))
}

async fn create_revision(
    Path((entity_type, entity_id)): Path<(String, Uuid)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRevisionBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let service = RevisionService::new(state.db_pool.clone());

    let request = CreateRevisionRequest {
        entity_type,
        entity_id,
        snapshot: body.snapshot,
        change_type: body.change_type.unwrap_or_else(|| "manual".to_string()),
        changed_by: body.changed_by.unwrap_or_else(|| "system".to_string()),
        change_summary: body.change_summary,
        template_id: None,
        template_version: None,
    };

    let revision = service.create_revision(request).await.map_err(map_err)?;

    Ok((StatusCode::CREATED, Json(json!({ "revision": revision }))))
}

async fn revert_to_version(
    Path((entity_type, entity_id)): Path<(String, Uuid)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<RevertRequestBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let service = RevisionService::new(state.db_pool.clone());

    let request = crate::revisions::RevertRequest {
        entity_type,
        entity_id,
        target_version: body.target_version,
        changed_by: body.changed_by.unwrap_or_else(|| "system".to_string()),
    };

    let revision = service.revert_to_version(request).await.map_err(map_err)?;

    Ok(Json(json!({ "revision": revision })))
}

// ── Template handlers ───────────────────────────────────────────────────

async fn list_templates(
    Query(params): Query<ListTemplatesQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let service = TemplateService::new(state.db_pool.clone());

    let templates = service
        .list_templates(params.entity_type.as_deref())
        .await
        .map_err(map_err)?;

    Ok(Json(json!({ "templates": templates, "total": templates.len() })))
}

async fn get_template(
    Path((entity_type, slug)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let service = TemplateService::new(state.db_pool.clone());

    let template = service
        .get_template(&entity_type, &slug)
        .await
        .map_err(map_err)?;

    Ok(Json(json!({ "template": template })))
}

async fn get_template_versions(
    Path((entity_type, slug)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let service = TemplateService::new(state.db_pool.clone());

    let template = service
        .get_template(&entity_type, &slug)
        .await
        .map_err(map_err)?;

    let versions = service
        .get_template_versions(template.id)
        .await
        .map_err(map_err)?;

    Ok(Json(json!({ "versions": versions, "total": versions.len() })))
}

async fn check_for_updates(
    Path((entity_type, entity_id)): Path<(String, Uuid)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let service = TemplateService::new(state.db_pool.clone());

    let result = service
        .check_for_updates(&entity_type, entity_id)
        .await
        .map_err(map_err)?;

    Ok(Json(json!({ "update_check": result })))
}

async fn get_subscription(
    Path((entity_type, entity_id)): Path<(String, Uuid)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let service = TemplateService::new(state.db_pool.clone());

    let subscription = service
        .get_subscription(&entity_type, entity_id)
        .await
        .map_err(map_err)?;

    Ok(Json(json!({ "subscription": subscription })))
}

// ── Router ──────────────────────────────────────────────────────────────

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Revision endpoints
        .route(
            "/revisions/{entity_type}/{entity_id}",
            get(list_revisions).post(create_revision),
        )
        .route(
            "/revisions/{entity_type}/{entity_id}/latest",
            get(get_latest_revision),
        )
        .route(
            "/revisions/{entity_type}/{entity_id}/{version}",
            get(get_revision),
        )
        .route(
            "/revisions/{entity_type}/{entity_id}/revert",
            post(revert_to_version),
        )
        // Template endpoints
        .route("/templates", get(list_templates))
        .route("/templates/{entity_type}/{slug}", get(get_template))
        .route(
            "/templates/{entity_type}/{slug}/versions",
            get(get_template_versions),
        )
        .route(
            "/templates/check-updates/{entity_type}/{entity_id}",
            get(check_for_updates),
        )
        .route(
            "/templates/subscription/{entity_type}/{entity_id}",
            get(get_subscription),
        )
}
