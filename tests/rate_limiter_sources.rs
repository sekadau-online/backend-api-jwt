use axum::{Router, routing::get, http::Request, body::Body};
use backend_api_jwt::middlewares::rate_limiter::rate_limiter;
use backend_api_jwt::middlewares::proxy::ClientIp;
use axum::middleware;
use tower::util::ServiceExt; // brings .oneshot()
use std::net::{IpAddr, SocketAddr};

#[tokio::test]
#[serial_test::serial]
async fn rate_limiter_uses_extension_source_when_present() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1000");
        std::env::set_var("RATE_LIMIT_BURST", "1000");
    }

    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    let mut req = Request::builder()
        .uri("/")
        .body(Body::empty())
        .unwrap();
    // simulate proxy setting the ClientIp extension
    req.extensions_mut().insert(ClientIp(IpAddr::from([1,2,3,4])));

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let header_val = resp.headers().get("x-key-source").unwrap().to_str().unwrap();
    assert_eq!(header_val, "extension");
    // Now assert x-key-type header reports ip/auth type if present (default is ip)
    // x-key-type will be set in a follow-up request after changes

}

#[tokio::test]
async fn rate_limiter_uses_peer_source_when_no_extension() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1000");
        std::env::set_var("RATE_LIMIT_BURST", "1000");
    }

    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    let mut req = Request::builder()
        .uri("/")
        .body(Body::empty())
        .unwrap();
    // simulate peer socket addr
    let sa = SocketAddr::new(IpAddr::from([203,0,113,5]), 12345);
    req.extensions_mut().insert(sa);

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let header_val = resp.headers().get("x-key-source").unwrap().to_str().unwrap();
    assert_eq!(header_val, "peer");
}

#[tokio::test]
async fn rate_limiter_uses_cf_connecting_ip_header_when_present() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1000");
        std::env::set_var("RATE_LIMIT_BURST", "1000");
    }

    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    let req = Request::builder()
        .uri("/")
        .header("cf-connecting-ip", "5.6.7.8")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let header_val = resp.headers().get("x-key-source").unwrap().to_str().unwrap();
    assert_eq!(header_val, "cf-connecting-ip");
}