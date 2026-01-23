use backend_api_jwt::create_app;
use serde_json::json;
use sqlx::{MySqlPool, Executor};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn register_flow() {
    dotenvy::dotenv().ok();
    // derive admin/base url from DATABASE_URL env
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Skipping integration test: set DATABASE_URL in your environment (example: mysql://user:pass@host:3306/db)");
            return;
        }
    };
    let (base, _db) = database_url.rsplit_once('/').expect("DATABASE_URL should include db name");
    let admin_url = base.to_string();
    let test_db = "db_backend_api_jwt_test";

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
    // ensure users table exists; if migration didn't create it, apply SQL directly (defensive)
    let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = ? AND table_name = 'users'")
        .bind(test_db)
        .fetch_one(&pool)
        .await
        .expect("info query");
    if exists.0 == 0 {
        // read migration SQL file and execute
        let sql = std::fs::read_to_string("migrations/20260122100826_create_users_table.sql").expect("read migration");
        pool.execute(sql.as_str()).await.expect("apply raw migration");
    }

    // Defensive: ensure users table exists (create if not)
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

    // Build app and run server on ephemeral port
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
    let url = format!("http://{}/register", addr);

    // Valid registration
    let res = client
        .post(&url)
        .json(&json!({"name": "Test User", "email": "test@example.com", "password": "password"}))
        .send()
        .await
        .expect("request failed");
    if res.status().as_u16() != 201 {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        panic!("expected 201 but got {}: {}", status, text);
    }
    let body: serde_json::Value = res.json().await.expect("json");
    assert!(body["success"].as_bool().unwrap_or(false));

    // Duplicate registration -> 409
    let res2 = client
        .post(&url)
        .json(&json!({"name": "Test User", "email": "test@example.com", "password": "password"}))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res2.status().as_u16(), 409);
    let body2: serde_json::Value = res2.json().await.expect("json2");
    assert!(!body2["success"].as_bool().unwrap_or(true));
    assert_eq!(body2["message"].as_str().unwrap_or(""), "Conflict");

    // cleanup: drop test db
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
}

#[tokio::test]
async fn register_validation_errors() {
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
    let test_db = "db_backend_api_jwt_test_validation";

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

    // Build app and run server on ephemeral port
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
    let url = format!("http://{}/register", addr);

    // Send empty payload to trigger validation errors
    let res = client
        .post(&url)
        .json(&json!({"name": "", "email": "", "password": ""}))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status().as_u16(), 400);
    let body: serde_json::Value = res.json().await.expect("json");
    assert!(!body["success"].as_bool().unwrap_or(true));
    assert_eq!(body["message"].as_str().unwrap_or(""), "Validation error");
    let errors = &body["data"]["errors"];
    assert!(errors.is_object());
    // Expect specific fields to be present
    assert!(errors.get("name").is_some());
    assert!(errors.get("email").is_some());
    assert!(errors.get("password").is_some());

    // cleanup: drop test db
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
}