use axum::middleware;
use axum::{Router, body::Body, http::Request, routing::get};
use backend_api_jwt::middlewares::rate_limiter::rate_limiter;
use tower::util::ServiceExt; // brings .oneshot()

#[tokio::test]
async fn request_cost_allows_multiple_quick_requests() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "1");
        std::env::set_var("RATE_LIMIT_BURST", "1");
        std::env::set_var("RATE_LIMIT_REQUEST_COST", "0.2");
    }

    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn(rate_limiter));

    // With burst=1 and cost=0.2, we should allow 5 immediate requests
    let mut ok_count = 0;
    for _ in 0..5 {
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        if resp.status().as_u16() == 200 {
            ok_count += 1;
        }
    }
    assert_eq!(ok_count, 5);

    // Sixth should be blocked
    let req6 = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp6 = app.oneshot(req6).await.unwrap();
    assert_eq!(resp6.status().as_u16(), 429);
}
