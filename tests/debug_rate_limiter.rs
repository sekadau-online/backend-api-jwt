use axum::{Router, routing::get, http::Request, body::Body};
use backend_api_jwt::middlewares::rate_limiter::{rate_limiter, debug_info};
use axum::middleware;
use tower::util::ServiceExt; // brings .oneshot()

#[tokio::test]
#[serial_test::serial]
async fn debug_endpoint_reports_buckets() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1000");
        std::env::set_var("RATE_LIMIT_BURST", "1000");
        std::env::set_var("RATE_LIMIT_DEBUG", "true");
    }

    // Create app that uses the limiter to populate buckets
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    // Create two buckets by making requests with distinct x-forwarded-for
    let req1 = Request::builder()
        .uri("/")
        .header("x-forwarded-for", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req1).await.unwrap();

    let req2 = Request::builder()
        .uri("/")
        .header("x-forwarded-for", "2.2.2.2")
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req2).await.unwrap();

    // Call debug handler directly via a router route with Authorization header (token set in env)
    unsafe { std::env::set_var("RATE_LIMIT_DEBUG_TOKEN", "testtoken"); }

    let dbg = Router::new().route("/debug/rate_limiter", get(debug_info));
    let req_dbg = Request::builder().uri("/debug/rate_limiter").header("authorization", "Bearer testtoken").body(Body::empty()).unwrap();
    let resp = dbg.oneshot(req_dbg).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let body_bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(v["buckets"].as_u64().unwrap() >= 2);
}
