use axum::middleware;
use axum::{Router, body::Body, http::Request, routing::get};
use backend_api_jwt::middlewares::rate_limiter::{debug_info, debug_action, rate_limiter};
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
    unsafe {
        std::env::set_var("RATE_LIMIT_DEBUG_TOKEN", "testtoken");
    }

    let dbg = Router::new().route("/debug/rate_limiter", get(debug_info).post(debug_action));
    let req_dbg = Request::builder()
        .uri("/debug/rate_limiter")
        .header("authorization", "Bearer testtoken")
        .body(Body::empty())
        .unwrap();
    let resp = dbg.clone().oneshot(req_dbg).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let body_bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(v["buckets"].as_u64().unwrap() >= 2);

    // Now exercise the POST drop action by dropping the bottom key
    let bottom_key = v["bottom"][0]["key"].as_str().expect("bottom key");
    let payload = serde_json::json!({ "action": "drop", "bottom": [{ "key": bottom_key }] });

    let req_drop = Request::builder()
        .method("POST")
        .uri("/debug/rate_limiter")
        .header("authorization", "Bearer testtoken")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let resp2 = dbg.oneshot(req_drop).await.unwrap();
    assert_eq!(resp2.status().as_u16(), 200);
    let body2 = axum::body::to_bytes(resp2.into_body(), 64 * 1024)
        .await
        .unwrap();
    let v2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    assert!(v2["removed"].as_u64().unwrap() >= 1);
}
