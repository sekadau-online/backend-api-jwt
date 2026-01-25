use axum::middleware;
use axum::{Router, body::Body, http::Request, routing::get};
use backend_api_jwt::middlewares::rate_limiter::{
    debug_info, purge_stale_buckets_once, rate_limiter,
};
use serial_test::serial;
use tower::util::ServiceExt;

// helper imports removed; this test uses only public API of the crate

#[tokio::test]
#[serial]
async fn purge_removes_stale_buckets() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1000");
        std::env::set_var("RATE_LIMIT_BURST", "1000");
        std::env::set_var("RATE_LIMIT_BUCKET_TTL_SECS", "1");
        std::env::set_var("RATE_LIMIT_DEBUG", "true");
    }

    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    // create bucket for 1.2.3.4
    let req = Request::builder()
        .uri("/")
        .header("x-forwarded-for", "1.2.3.4")
        .body(Body::empty())
        .unwrap();
    let _ = app.clone().oneshot(req).await.unwrap();
    println!("TEST: created bucket for 1.2.3.4");

    // Manually set bucket last_access to far past
    // Access internal BUCKETS via debug endpoint to confirm there is at least one bucket
    // Call purge once
    println!("TEST: calling purge_stale_buckets_once(0)");
    purge_stale_buckets_once(0).await; // ttl=0 will remove everything older than 0s
    println!("TEST: returned from purge_stale_buckets_once");

    // call debug endpoint with token (test-mode)
    unsafe {
        std::env::set_var("RATE_LIMIT_DEBUG_TOKEN", "testtoken");
    }
    let dbg = Router::new().route("/debug/rate_limiter", get(debug_info));
    let req_dbg = Request::builder()
        .uri("/debug/rate_limiter")
        .header("authorization", "Bearer testtoken")
        .body(Body::empty())
        .unwrap();
    let resp = dbg.oneshot(req_dbg).await.unwrap();
    let body_bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(v["buckets"].as_u64().unwrap(), 0);
}
