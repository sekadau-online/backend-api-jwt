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
    use tower_http::cors::{CorsLayer, Any};
    use axum::http::Method;

    let mut app = Router::new()
        .merge(routes::auth_routes::auth_routes())
        .merge(routes::user_routes::user_routes())
        .layer(Extension(pool));

    // Configure CORS based on environment variables:
    // - If ENABLE_CORS=true, allow any origin (dev-friendly)
    // - If CORS_ALLOWED_ORIGINS is set, use that comma-separated list
    let cors_allowed = std::env::var("CORS_ALLOWED_ORIGINS").ok();
    let enable_cors = std::env::var("ENABLE_CORS").map(|v| v == "true" || v == "1").unwrap_or(false);

    if enable_cors || cors_allowed.is_some() {
        // If CORS_ALLOWED_ORIGINS is exactly "*" treat it as permissive Any. Otherwise parse a CSV of origins.
        let cors_layer = if let Some(list) = cors_allowed {
            if list.trim() == "*" {
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
                    .allow_headers(Any)
            } else {
                use axum::http::header::HeaderValue;
                use tower_http::cors::AllowOrigin;
                let origins = list
                    .split(',')
                    .filter_map(|s| HeaderValue::from_str(s.trim()).ok())
                    .collect::<Vec<HeaderValue>>();
                CorsLayer::new()
                    .allow_origin(AllowOrigin::list(origins))
                    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
                    .allow_headers(Any)
            }
        } else {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
                .allow_headers(Any)
        };
        app = app.layer(cors_layer);
    }

    app
}
