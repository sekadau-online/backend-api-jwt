use axum::{
    Router,
    routing::{get, post},
    middleware,
};

// Import user-related handlers
use crate::handlers::user_handler::{ index, store, show, update, destroy };

// Import middleware for authentication
use crate::middlewares::auth_middleware::auth_middleware;

pub fn user_routes() -> Router {
    Router::new()
        .route("/users", get(index))
        .route("/users", post(store))
        .route("/users/{id}", get(show).put(update).delete(destroy))
        // Apply authentication middleware to user routes
        .layer(middleware::from_fn(auth_middleware))
}