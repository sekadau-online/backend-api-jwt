use axum::{Router, routing::post};

// Import the register handler
use crate::handlers::register_handler::register_handler;
// Import the login handler
use crate::handlers::login_handler::login_handler;

// Function to create auth routes
pub fn auth_routes() -> Router {
    Router::new()
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
}