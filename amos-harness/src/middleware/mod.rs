//! Middleware for authentication and error handling

pub mod auth;
pub mod error_handler;

pub use auth::authenticate;
pub use error_handler::handle_error;
