//! Error handling middleware

use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

/// Global error handler middleware
pub async fn handle_error(request: Request, next: Next) -> Response {
    next.run(request).await
}

// Note: IntoResponse for AmosError should be implemented in amos-core
// as it owns the AmosError type. This is not the right place for that impl.
