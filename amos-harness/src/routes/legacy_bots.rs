//! Legacy bots routes (messaging bots stored in `bots` table)

use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use std::sync::Arc;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_bots))
}

async fn list_bots(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    let rows = sqlx::query_as::<_, (
        sqlx::types::Uuid,
        String,
        Option<String>,
        String,
    )>(
        r#"
        SELECT id, name, description, status
        FROM bots
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let bots: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, name, description, status)| {
            serde_json::json!({
                "id": id.to_string(),
                "name": name,
                "description": description,
                "status": status,
                "platform": "messaging",
            })
        })
        .collect();

    Ok(Json(bots))
}
