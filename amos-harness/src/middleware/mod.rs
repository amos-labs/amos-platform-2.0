//! Middleware for authentication

pub mod auth;

pub use auth::{authenticate, token_exchange, Claims, SESSION_COOKIE};
