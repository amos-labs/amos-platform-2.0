//! API route definitions for the AMOS Network Relay.

pub mod agents;
pub mod bounties;
pub mod harnesses;
pub mod health;
pub mod reputation;

use crate::state::RelayState;
use axum::Router;

/// Build the API routes (v1).
pub fn api_routes() -> Router<RelayState> {
    Router::new()
        .nest("/bounties", bounties::routes())
        .nest("/agents", agents::routes())
        .nest("/reputation", reputation::routes())
        .nest("/harnesses", harnesses::routes())
}
