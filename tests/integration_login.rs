use backend_api_jwt::create_app;
use serde_json::json;
use sqlx::{MySqlPool, Executor};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn login_flow() {
    dotenvy::dotenv().ok();
    // require DATABASE_URL to be set for running integration tests
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Skipping integration test: set DATABASE_URL in your environment (example: mysql://user:pass@host:3306/db)");
            return;
        }
    };
    let (base, _db) = database_url.rsplit_once('/').expect("DATABASE_URL should include db name");
    let admin_url = base.to_string();
    let test_db = "db_backend_api_jwt_test_login";

    // connect as admin and recreate a clean test database
    let admin_pool = MySqlPool::connect(&format!("{}/", admin_url)).await.expect("connect admin");
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
    admin_pool
        .execute(format!("CREATE DATABASE {}", test_db).as_str())
        .await
        .expect("create test db");

    let test_db_url = format!("{}/{}", admin_url, test_db);

    // connect to test db and run migrations
    let pool = MySqlPool::connect(&test_db_url).await.expect("connect test db");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrations");

    // ensure users table exists
    let create_sql = r#"
    CREATE TABLE IF NOT EXISTS users (
        id BIGINT AUTO_INCREMENT PRIMARY KEY,
        name VARCHAR(100) NOT NULL,
        email VARCHAR(100) NOT NULL UNIQUE,
        password TEXT NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
    );
    "#;
    pool.execute(create_sql).await.expect("ensure users table");

    // ensure clean state
    pool.execute("DELETE FROM users").await.ok();

    // insert a test user with bcrypt-hashed password
    let plain_password = "password";
    let hashed = bcrypt::hash(plain_password, bcrypt::DEFAULT_COST).expect("hash pw");
    let email = "testlogin@example.com";
    sqlx::query("INSERT INTO users (name, email, password) VALUES (?, ?, ?)")
        .bind("Login Test")
        .bind(email)
        .bind(hashed)
        .execute(&pool)
        .await
        .expect("insert user");

    // Build app and run server on ephemeral port
    // Make rate limiter permissive for this test and purge buckets
    backend_api_jwt::test_helpers::make_rate_limiter_permissive_and_purge().await;

    let app = create_app(pool.clone());

    // Bind to an ephemeral port using tokio listener (host from APP_HOST)
    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let listener = tokio::net::TcpListener::bind(format!("{}:0", host)).await.expect("bind");
    let addr = listener.local_addr().unwrap();

    // Serve the app in background
    let server = axum::serve(listener, app.into_make_service());
    let _srv = tokio::spawn(async move { server.await.unwrap(); });

    // Give the server a moment to start
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let url = format!("http://{}/login", addr);

    // Valid login
    let res = client
        .post(&url)
        .json(&json!({"email": email, "password": plain_password}))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status().as_u16(), 200);
    let body: serde_json::Value = res.json().await.expect("json");
    assert!(body["success"].as_bool().unwrap_or(false));
    assert!(body["data"]["token"].is_string());

    // Invalid password -> 401
    let res2 = client
        .post(&url)
        .json(&json!({"email": email, "password": "wrongpassword"}))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res2.status().as_u16(), 401);
    let body2: serde_json::Value = res2.json().await.expect("json2");
    assert!(!body2["success"].as_bool().unwrap_or(true));
    assert_eq!(body2["message"].as_str().unwrap_or(""), "Unauthorized");

    // cleanup: drop test db
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
}
