use backend_api_jwt::create_app;
use serde_json::json;
use sqlx::{MySqlPool, Executor};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn users_pagination_flow() {
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
    let test_db = "db_backend_api_jwt_test_users_pagination";

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

    // insert a bunch of users (e.g., 120)
    for i in 0..120 {
        let email = format!("user{}@example.com", i);
        let name = format!("User {}", i);
        let pw = bcrypt::hash("password", bcrypt::DEFAULT_COST).expect("hash pw");
        sqlx::query("INSERT INTO users (name, email, password) VALUES (?, ?, ?)")
            .bind(name)
            .bind(email)
            .bind(pw)
            .execute(&pool)
            .await
            .expect("insert user");
    }

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

    // Login to get an auth token (routes are protected)
    let login_url = format!("http://{}/login", addr);
    let login_res = client
        .post(&login_url)
        .json(&serde_json::json!({"email": "user0@example.com", "password": "password"}))
        .send()
        .await
        .expect("login request failed");
    assert_eq!(login_res.status().as_u16(), 200);
    let login_body: serde_json::Value = login_res.json().await.expect("login json");
    let token = login_body["data"]["token"].as_str().expect("token").to_string();

    let url = format!("http://{}/users?page=2&per_page=50", addr);

    let res = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status().as_u16(), 200);
    let body: serde_json::Value = res.json().await.expect("json");

    assert!(body["success"].as_bool().unwrap_or(false));
    let data = &body["data"];
    let users = data["users"].as_array().expect("users array");
    assert_eq!(users.len(), 50);
    let meta = &data["meta"];
    assert_eq!(meta["page"].as_u64().unwrap_or(0), 2);
    assert_eq!(meta["per_page"].as_u64().unwrap_or(0), 50);
    assert_eq!(meta["total"].as_u64().unwrap_or(0), 120);

    // cleanup: drop test db
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
}
