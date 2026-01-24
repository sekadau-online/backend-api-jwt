use axum::{Router, routing::get, body::Body, http::Request};
use axum::middleware;
use tower::util::ServiceExt;
use backend_api_jwt::middlewares::{proxy, rate_limiter};

#[tokio::test]
async fn proxy_resolves_cf_connecting_ip_when_trusted() {
    // Trust all proxies for test
    unsafe { std::env::set_var("TRUSTED_PROXIES", "0.0.0.0/0"); }

    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(proxy::proxy_middleware))
        .layer(middleware::from_fn(rate_limiter::rate_limiter));

    // Two requests with same CF-Connecting-IP should trigger rate limiter (default RPS=100 etc.)
    unsafe { std::env::set_var("RATE_LIMIT_RPS", "1"); std::env::set_var("RATE_LIMIT_BURST", "1"); }

    let req = Request::builder()
        .uri("/")
        .header("cf-connecting-ip", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let req2 = Request::builder()
        .uri("/")
        .header("cf-connecting-ip", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(resp2.status().as_u16(), 429);
}