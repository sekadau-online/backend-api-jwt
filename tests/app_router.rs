use axum::body::Body;
use axum::http::{Method, Request};
use backend_api_jwt::app::build_router;
use tower::util::ServiceExt; // for oneshot

#[test]
fn build_router_smoke() {
    let _router = build_router();
}

#[tokio::test]
async fn cors_preflight_wildcard_allows_origin() {
    let prev = std::env::var("ENABLE_CORS").ok();
    unsafe {
        std::env::set_var("ENABLE_CORS", "true");
    }

    let app = build_router();
    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/register")
        .header("Origin", "http://example.com")
        .header("Access-Control-Request-Method", "POST")
        .header(
            "Access-Control-Request-Headers",
            "Authorization,Content-Type",
        )
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.expect("request failed");
    assert!(resp.status().is_success());
    let allowed = resp
        .headers()
        .get("access-control-allow-origin")
        .map(|v| v.to_str().unwrap_or(""));
    let allow_methods = resp
        .headers()
        .get("access-control-allow-methods")
        .map(|v| v.to_str().unwrap_or(""));
    let allow_headers = resp
        .headers()
        .get("access-control-allow-headers")
        .map(|v| v.to_str().unwrap_or(""));
    if allowed.is_none() && allow_methods.is_none() {
        panic!(
            "No ACAO or ACA-Methods header. status={} headers={:?}",
            resp.status(),
            resp.headers()
        );
    }
    if let Some(a) = allowed {
        assert!(a == "*" || a == "http://example.com");
    }
    if let Some(m) = allow_methods {
        assert!(m.to_uppercase().contains("POST"));
    }
    if let Some(h) = allow_headers {
        let h_trim = h.trim();
        if h_trim != "*" {
            let h_lc = h_trim.to_lowercase();
            assert!(
                h_lc.contains("authorization"),
                "ACAH exists but does not include Authorization: {}",
                h
            );
        }
    }

    if let Some(v) = prev {
        unsafe {
            std::env::set_var("ENABLE_CORS", v);
        }
    } else {
        unsafe {
            std::env::remove_var("ENABLE_CORS");
        }
    }
}

#[tokio::test]
async fn cors_specific_origin_allowed() {
    let prev = std::env::var("CORS_ALLOWED_ORIGINS").ok();
    unsafe {
        std::env::set_var("CORS_ALLOWED_ORIGINS", "http://allowed.example.com");
    }

    let app = build_router();
    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/register")
        .header("Origin", "http://allowed.example.com")
        .header("Access-Control-Request-Method", "POST")
        .header(
            "Access-Control-Request-Headers",
            "Authorization,Content-Type",
        )
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.expect("request failed");
    assert!(resp.status().is_success());
    let allowed = resp
        .headers()
        .get("access-control-allow-origin")
        .map(|v| v.to_str().unwrap_or(""));
    let allow_methods = resp
        .headers()
        .get("access-control-allow-methods")
        .map(|v| v.to_str().unwrap_or(""));
    let allow_headers = resp
        .headers()
        .get("access-control-allow-headers")
        .map(|v| v.to_str().unwrap_or(""));
    if allowed.is_none() && allow_methods.is_none() {
        panic!(
            "No ACAO or ACA-Methods header. status={} headers={:?}",
            resp.status(),
            resp.headers()
        );
    }
    let allowed_val = allowed.unwrap_or_default();
    assert!(allowed_val == "*" || allowed_val == "http://allowed.example.com");
    if let Some(m) = allow_methods {
        assert!(m.to_uppercase().contains("POST"));
    }
    if let Some(h) = allow_headers {
        let h_trim = h.trim();
        if h_trim != "*" {
            let h_lc = h_trim.to_lowercase();
            assert!(
                h_lc.contains("authorization"),
                "ACAH exists but does not include Authorization: {}",
                h
            );
        }
    }

    if let Some(v) = prev {
        unsafe {
            std::env::set_var("CORS_ALLOWED_ORIGINS", v);
        }
    } else {
        unsafe {
            std::env::remove_var("CORS_ALLOWED_ORIGINS");
        }
    }
}
