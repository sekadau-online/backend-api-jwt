pub mod config;
pub mod routes;
pub mod handlers;
pub mod schemas;
pub mod utils;
pub mod middlewares;
pub mod models;

use axum::Router;
use sqlx::MySqlPool;
use axum::Extension;

pub fn create_app(pool: MySqlPool) -> Router {
    Router::new()
        .merge(routes::auth_routes::auth_routes())
        .merge(routes::user_routes::user_routes())
        .layer(Extension(pool))
}
