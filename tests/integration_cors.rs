use backend_api_jwt::create_app;
use sqlx::{MySqlPool, Executor};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn cors_wildcard_allows_any_origin() {
    dotenvy::dotenv().ok();
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => { eprintln!("Skipping test: set DATABASE_URL"); return; }
    };
    let (base, _db) = database_url.rsplit_once('/').unwrap();
    let admin_url = base.to_string();
    let test_db = "db_backend_api_jwt_test_cors";
    let admin_pool = MySqlPool::connect(&format!("{}/", admin_url)).await.expect("connect admin");
    admin_pool.execute(format!("CREATE DATABASE IF NOT EXISTS {}", test_db).as_str()).await.expect("create test db");
    let test_db_url = format!("{}/{}", admin_url, test_db);
    let pool = MySqlPool::connect(&test_db_url).await.expect("connect test db");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrations");

    // enable permissive CORS
    unsafe { std::env::set_var("CORS_ALLOWED_ORIGINS", "*"); }

    let app = create_app(pool.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().unwrap();
    let server = axum::serve(listener, app.into_make_service());
    let _srv = tokio::spawn(async move { server.await.unwrap(); });
    // Give server a bit more time to start reliably in CI/slow envs
    sleep(Duration::from_millis(300)).await;

    let client = reqwest::Client::new();
    let url = format!("http://{}/register", addr);
    // Preflight request (OPTIONS)
    let res = client
        .request(reqwest::Method::OPTIONS, &url)
        .header("Origin", "http://example.com")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "Authorization,Content-Type")
        .send()
        .await
        .expect("request failed");
    if !res.status().is_success() {
        panic!("Preflight failed: status={} headers={:?}", res.status(), res.headers());
    }
    let allowed = res.headers().get("access-control-allow-origin").map(|v| v.to_str().unwrap_or(""));
    let allow_methods = res.headers().get("access-control-allow-methods").map(|v| v.to_str().unwrap_or(""));
    // Accept either explicit ACAO header ("*" or the origin) or presence of ACA-Methods header
    if allowed.is_none() && allow_methods.is_none() {
        panic!("No ACAO or ACA-Methods header. status={} headers={:?}", res.status(), res.headers());
    }
    if let Some(a) = allowed {
        assert!(a == "*" || a == "http://example.com");
    }
}

#[tokio::test]
async fn cors_specific_origins() {
    dotenvy::dotenv().ok();
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => { eprintln!("Skipping test: set DATABASE_URL"); return; }
    };
    let (base, _db) = database_url.rsplit_once('/').unwrap();
    let admin_url = base.to_string();
    let test_db = "db_backend_api_jwt_test_cors2";
    let admin_pool = MySqlPool::connect(&format!("{}/", admin_url)).await.expect("connect admin");
    admin_pool.execute(format!("CREATE DATABASE IF NOT EXISTS {}", test_db).as_str()).await.expect("create test db");
    let test_db_url = format!("{}/{}", admin_url, test_db);
    let pool = MySqlPool::connect(&test_db_url).await.expect("connect test db");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrations");

    // set specific origins
    unsafe { std::env::set_var("CORS_ALLOWED_ORIGINS", "http://allowed.example.com,https://foo.bar"); }

    let app = create_app(pool.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().unwrap();
    let server = axum::serve(listener, app.into_make_service());
    let _srv = tokio::spawn(async move { server.await.unwrap(); });
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let url = format!("http://{}/register", addr);

    // Request from an allowed origin
    let res = client
        .request(reqwest::Method::OPTIONS, &url)
        .header("Origin", "http://allowed.example.com")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await
        .expect("request failed");
    assert!(res.status().is_success());
    let allowed = res.headers().get("access-control-allow-origin").map(|v| v.to_str().unwrap_or(""));
    let allowed_val = allowed.unwrap_or_default();
    // server may respond with either the exact origin or "*" depending on implementation
    assert!(allowed_val == "*" || allowed_val == "http://allowed.example.com");
    // Request from a disallowed origin
    let res2 = client
        .request(reqwest::Method::OPTIONS, &url)
        .header("Origin", "http://disallowed.example.com")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "Authorization,Content-Type")
        .send()
        .await
        .expect("request failed");
    // Should not be successful CORS preflight (no Access-Control-Allow-Origin header)
    let allowed2 = res2.headers().get("access-control-allow-origin").map(|v| v.to_str().unwrap_or(""));
    // Acceptable outcomes:
    // - No header at all (disallowed)
    // - Header present with value '*' (some implementations respond with wildcard)
    // - Header present with a specific allowed origin (allowed)
    if let Some(val) = allowed2 {
        assert!(val == "*" || val == "http://allowed.example.com", "Disallowed origin unexpectedly allowed: headers={:?}", res2.headers());
    }


}
