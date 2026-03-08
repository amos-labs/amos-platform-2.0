pub mod middleware;
pub mod routes;
pub mod state;

use axum::Router;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use std::time::Duration;

pub use state::AppState;

/// Start the Axum HTTP server with all routes and middleware
pub async fn start_server(
    state: AppState,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = build_router(state);

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Starting AMOS API server on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Build the complete Axum router with all middleware
fn build_router(state: AppState) -> Router {
    let state = std::sync::Arc::new(state);
    let app = routes::build_routes()
        .with_state(state)
        .layer(
            CorsLayer::permissive() // TODO: Configure for production
        )
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(TraceLayer::new_for_http());

    app
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_builds() {
        // Basic smoke test that router can be constructed
        // Actual state would be needed for full testing
    }
}
