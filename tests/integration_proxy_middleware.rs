use backend_api_jwt::create_app;
use reqwest::StatusCode;
use serial_test::serial;
use sqlx::{Executor, MySqlPool};
use tokio::time::{Duration, sleep};

// Test that when TRUSTED_PROXIES includes the local peer (127.0.0.1/32),
// a request with `cf-connecting-ip` header results in the middleware
// resolving the client IP from the header (x-key-source should be 'extension' or header-based behavior).

#[tokio::test]
#[serial]
async fn proxy_trusted_uses_header_source() {
    // Set trusted proxies to include loopback
    unsafe {
        std::env::set_var("TRUSTED_PROXIES", "127.0.0.1/32");
    }
    // also set via helper to ensure runtime update
    backend_api_jwt::middlewares::proxy::set_trusted_proxies("127.0.0.1/32");

    dotenvy::dotenv().ok();
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!(
                "Skipping integration test: set DATABASE_URL in your environment (example: mysql://user:pass@host:3306/db)"
            );
            return;
        }
    };
    // Make rate limiter permissive for this test and purge buckets
    backend_api_jwt::test_helpers::make_rate_limiter_permissive_and_purge().await;

    let (base, _db) = database_url
        .rsplit_once('/')
        .expect("DATABASE_URL should include db name");
    let admin_url = base.to_string();
    let test_db = "db_backend_api_jwt_test_proxy_trusted";

    let admin_pool = MySqlPool::connect(&format!("{}/", admin_url))
        .await
        .expect("connect admin");
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
    admin_pool
        .execute(format!("CREATE DATABASE {}", test_db).as_str())
        .await
        .expect("create test db");

    let test_db_url = format!("{}/{}", admin_url, test_db);
    let pool = MySqlPool::connect(&test_db_url)
        .await
        .expect("connect test db");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");

    // Start server
    let app = create_app(pool.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().unwrap();
    let server = axum::serve(listener, app.into_make_service());
    let _srv = tokio::spawn(async move {
        server.await.unwrap();
    });
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://{}/users", addr))
        .header("cf-connecting-ip", "203.0.113.55")
        .send()
        .await
        .expect("request failed");

    // endpoint requires auth -> 401 OK for our purpose
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    // when trusted, proxy_middleware sets the extension; rate_limiter will likely report 'extension' as source
    let xs = res
        .headers()
        .get("x-key-source")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        xs == "extension" || xs == "cf-connecting-ip",
        "unexpected x-key-source: {}",
        xs
    );

    // cleanup
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
}

#[tokio::test]
#[serial]
async fn proxy_not_trusted_uses_header_directly() {
    // Clear trusted proxies
    unsafe {
        std::env::set_var("TRUSTED_PROXIES", "");
    }
    backend_api_jwt::middlewares::proxy::set_trusted_proxies("");

    // Ensure rate limiter permissive for this test and purge any existing buckets
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "10000");
        std::env::set_var("RATE_LIMIT_BURST", "20000");
        std::env::set_var("RATE_LIMIT_REQUEST_COST", "0.2");
    }
    backend_api_jwt::middlewares::rate_limiter::purge_stale_buckets_once(0).await;

    dotenvy::dotenv().ok();
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Skipping integration test: set DATABASE_URL in your environment");
            return;
        }
    };

    let (base, _db) = database_url
        .rsplit_once('/')
        .expect("DATABASE_URL should include db name");
    let admin_url = base.to_string();
    let test_db = "db_backend_api_jwt_test_proxy_not_trusted";

    let admin_pool = MySqlPool::connect(&format!("{}/", admin_url))
        .await
        .expect("connect admin");
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
    admin_pool
        .execute(format!("CREATE DATABASE {}", test_db).as_str())
        .await
        .expect("create test db");

    let test_db_url = format!("{}/{}", admin_url, test_db);
    let pool = MySqlPool::connect(&test_db_url)
        .await
        .expect("connect test db");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");

    // Start server
    let app = create_app(pool.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().unwrap();
    let server = axum::serve(listener, app.into_make_service());
    let _srv = tokio::spawn(async move {
        server.await.unwrap();
    });
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://{}/users", addr))
        .header("cf-connecting-ip", "203.0.113.55")
        .send()
        .await
        .expect("request failed");

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let xs = res
        .headers()
        .get("x-key-source")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    // When not trusted, rate_limiter falls back to header or peer; expect cf header to be picked
    assert!(
        xs == "cf-connecting-ip" || xs == "peer",
        "unexpected x-key-source: {}",
        xs
    );

    // cleanup
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
}
