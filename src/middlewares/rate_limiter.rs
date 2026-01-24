use axum::{http::StatusCode, middleware::Next, response::Response, Json};
use serde_json::json;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use std::time::Instant;
use crate::utils::response::ApiResponse;

struct Bucket {
    tokens: f64,
    last: Instant,
}

static BUCKETS: Lazy<DashMap<String, Mutex<Bucket>>> = Lazy::new(|| DashMap::new());

fn rate_limit_config() -> (f64, f64) {
    // returns (rate_per_sec, burst)
    let rate = std::env::var("RATE_LIMIT_RPS").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(100.0);
    let burst = std::env::var("RATE_LIMIT_BURST").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(rate * 2.0);
    (rate, burst)
}

pub async fn rate_limiter(req: axum::http::Request<axum::body::Body>, next: Next) -> Result<Response, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let (rate, burst) = rate_limit_config();

    // Extract key (ip) from headers or extensions
    let key = req.headers().get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()).map(|s| s.to_string()))
        .or_else(|| {
            // Try to get peer addr from extensions
            req.extensions().get::<std::net::SocketAddr>().map(|sa| sa.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    let now = Instant::now();
    let entry = BUCKETS.entry(key).or_insert_with(|| Mutex::new(Bucket { tokens: burst, last: now }));
    let mut bucket = entry.lock().await;

    let elapsed = now.duration_since(bucket.last).as_secs_f64();
    bucket.tokens = (bucket.tokens + elapsed * rate).min(burst);
    bucket.last = now;

    if bucket.tokens >= 1.0 {
        bucket.tokens -= 1.0;
        drop(bucket);
        Ok(next.run(req).await)
    } else {
        let response = ApiResponse::error_with_data("Too Many Requests", json!({ "error": "Rate limit exceeded" }));
        Err((StatusCode::TOO_MANY_REQUESTS, Json(response)))
    }
}
