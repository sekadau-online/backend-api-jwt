use axum::{Router, routing::get, http::Request, body::Body};
use backend_api_jwt::middlewares::rate_limiter::rate_limiter;
use axum::middleware;
use tower::util::ServiceExt; // brings .oneshot()

#[tokio::test]
#[serial_test::serial]
async fn rate_limiter_drop_behavior() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1");
        std::env::set_var("RATE_LIMIT_BURST", "1");
        std::env::set_var("RATE_LIMIT_ACTION", "drop");
    }

    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    // First request should succeed (burst=1)
    let req = Request::builder()
        .uri("/")
        .header("x-forwarded-for", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    // Second immediate request should be dropped (204 No Content)
    let req2 = Request::builder()
        .uri("/")
        .header("x-forwarded-for", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    let resp2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(resp2.status().as_u16(), 204);
    let header_val = resp2.headers().get("x-key-source").unwrap().to_str().unwrap();
    // key source reflects the specific header used (here: x-forwarded-for)
    assert_eq!(header_val, "x-forwarded-for");
    let key_type = resp2.headers().get("x-key-type").unwrap().to_str().unwrap();
    assert_eq!(key_type, "ip");
}