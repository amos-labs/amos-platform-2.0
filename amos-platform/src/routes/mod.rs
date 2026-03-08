//! HTTP API route definitions.

use axum::Router;

use crate::state::PlatformState;

pub mod billing;
pub mod governance;
pub mod health;
pub mod provisioning;
pub mod sync;
pub mod token;

/// Build all API routes.
pub fn api_routes() -> Router<PlatformState> {
    Router::new()
        .merge(health::routes())
        .merge(token::routes())
        .merge(governance::routes())
        .merge(billing::routes())
        .merge(provisioning::routes())
        .merge(sync::routes())
}
