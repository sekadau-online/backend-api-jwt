use axum::{Router, routing::get, http::Request, body::Body};
use backend_api_jwt::middlewares::rate_limiter::rate_limiter;
use axum::middleware;
use tower::util::ServiceExt; // brings .oneshot()

#[tokio::test]
#[serial_test::serial]
async fn rate_limiter_prefers_auth_key_when_configured() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1000");
        std::env::set_var("RATE_LIMIT_BURST", "1000");
        std::env::set_var("RATE_LIMIT_KEY_PRIORITY", "auth");
    }

    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    // request with Authorization header
    let req = Request::builder()
        .uri("/")
        .header("authorization", "Bearer token-abc-123")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let key_type = resp.headers().get("x-key-type").unwrap().to_str().unwrap();
    assert_eq!(key_type, "auth");
}

#[tokio::test]
#[serial_test::serial]
async fn rate_limiter_uses_auth_plus_ip_when_configured() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1000");
        std::env::set_var("RATE_LIMIT_BURST", "1000");
        std::env::set_var("RATE_LIMIT_KEY_PRIORITY", "auth+ip");
    }

    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    let req = Request::builder()
        .uri("/")
        .header("authorization", "Bearer token-one")
        .header("x-forwarded-for", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let key_type = resp.headers().get("x-key-type").unwrap().to_str().unwrap();
    let key_source = resp.headers().get("x-key-source").unwrap().to_str().unwrap();
    println!("DEBUG: key_type={} key_source={}", key_type, key_source);
    assert_eq!(key_type, "auth+ip");
}