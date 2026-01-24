use axum::{Router, routing::get, http::Request, body::Body};
use backend_api_jwt::middlewares::rate_limiter::rate_limiter;
use axum::middleware;
use tower::util::ServiceExt; // brings .oneshot()

#[tokio::test]
async fn rate_limiter_blocks_after_burst() {
    // Set very small limits for deterministic test
    // set env vars for deterministic behavior in this test
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1");
        std::env::set_var("RATE_LIMIT_BURST", "1");
    }

    // Build a simple router that uses the rate limiter middleware and a test handler
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    // First request with same client IP should succeed (burst=1)
    let req = Request::builder()
        .uri("/")
        .header("x-forwarded-for", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    // Second immediate request should be rate-limited
    let req2 = Request::builder()
        .uri("/")
        .header("x-forwarded-for", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(resp2.status().as_u16(), 429);
}
