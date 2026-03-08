//! Canvas API routes

use crate::{canvas::CanvasType, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateCanvasRequest {
    pub name: String,
    pub description: Option<String>,
    pub canvas_type: String,
    pub html_content: String,
    pub js_content: Option<String>,
    pub css_content: Option<String>,
    pub data_sources: Option<JsonValue>,
    pub actions: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCanvasRequest {
    pub name: Option<String>,
    pub html_content: Option<String>,
    pub js_content: Option<String>,
    pub css_content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenderCanvasRequest {
    pub data_context: Option<JsonValue>,
}

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_canvases).post(create_canvas))
        .route("/system", get(list_system_canvases))
        .route("/{id}", get(get_canvas).put(update_canvas).delete(delete_canvas))
        .route("/{id}/render", post(render_canvas))
        .route("/{id}/publish", post(publish_canvas))
}

async fn list_canvases(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::canvas::Canvas>>, StatusCode> {
    let canvases = state.canvas_engine.list_canvases(Some(100), None).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(canvases))
}

async fn list_system_canvases(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::canvas::Canvas>>, StatusCode> {
    let canvases = state.canvas_engine.list_system_canvases().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(canvases))
}

async fn create_canvas(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCanvasRequest>,
) -> Result<Json<crate::canvas::Canvas>, StatusCode> {
    let canvas_type = CanvasType::from_str(&req.canvas_type);

    let canvas = state.canvas_engine.create_canvas(
        req.name,
        req.description,
        canvas_type,
        req.html_content,
        req.js_content,
        req.css_content,
        req.data_sources,
        req.actions,
        None,
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(canvas))
}

async fn get_canvas(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::canvas::Canvas>, StatusCode> {
    let canvas = state.canvas_engine.get_canvas(id).await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(canvas))
}

async fn update_canvas(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCanvasRequest>,
) -> Result<Json<crate::canvas::Canvas>, StatusCode> {
    let updates = crate::canvas::CanvasUpdate {
        name: req.name,
        description: None,
        html_content: req.html_content,
        js_content: req.js_content,
        css_content: req.css_content,
        data_sources: None,
        actions: None,
    };

    let canvas = state.canvas_engine.update_canvas(id, updates).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(canvas))
}

async fn delete_canvas(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    state.canvas_engine.delete_canvas(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn render_canvas(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<RenderCanvasRequest>,
) -> Result<Json<crate::canvas::CanvasResponse>, StatusCode> {
    let canvas = state.canvas_engine.get_canvas(id).await
        .map_err(|e| {
            tracing::error!("Canvas not found {}: {}", id, e);
            StatusCode::NOT_FOUND
        })?;

    let response = state.canvas_engine.render_canvas(&canvas, req.data_context).await
        .map_err(|e| {
            tracing::error!("Canvas render failed for {}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(response))
}

async fn publish_canvas(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let public_slug = state.canvas_engine.publish_canvas(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "public_slug": public_slug,
        "public_url": format!("{}/c/{}", state.config.server.rails_url, public_slug)
    })))
}

pub async fn serve_public_canvas(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let canvas = state.canvas_engine.get_public_canvas(&slug).await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let response = state.canvas_engine.render_canvas(&canvas, None).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Html(response.content))
}
