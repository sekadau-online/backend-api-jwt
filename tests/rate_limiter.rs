use axum::middleware;
use axum::{Router, body::Body, http::Request, routing::get};
use backend_api_jwt::middlewares::rate_limiter::rate_limiter;
use tower::util::ServiceExt; // brings .oneshot()

#[tokio::test]
#[serial_test::serial]
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
    let resp2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(resp2.status().as_u16(), 429);

    // Also ensure the limiter exposes the key source header for diagnostics
    let req3 = Request::builder()
        .uri("/")
        .header("x-forwarded-for", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    let resp3 = app.clone().oneshot(req3).await.unwrap();
    // 429 response should include x-key-source header (xff)
    assert_eq!(resp3.status().as_u16(), 429);
    let header_val = resp3
        .headers()
        .get("x-key-source")
        .unwrap()
        .to_str()
        .unwrap();
    // key source reflects the specific header used (here: x-forwarded-for)
    assert_eq!(header_val, "x-forwarded-for");
}
