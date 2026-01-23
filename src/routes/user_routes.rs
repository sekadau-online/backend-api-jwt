use axum::{
    Router,
    routing::get,
    middleware,
};

// Import user-related handlers
use crate::handlers::user_handler::index;

// Import middleware for authentication
use crate::middlewares::auth_middleware::auth_middleware;

pub fn user_routes() -> Router {
    Router::new()
        .route("/users", get(index))

        // Apply authentication middleware to user routes
        .layer(middleware::from_fn(auth_middleware))
}