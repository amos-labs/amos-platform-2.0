//! Middleware for authentication

pub mod auth;

pub use auth::{authenticate, Claims};
