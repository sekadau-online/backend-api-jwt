use backend_api_jwt::create_app;
use sqlx::{MySqlPool, Executor};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn user_list_flow() {
    dotenvy::dotenv().ok();
    // require DATABASE_URL to be set for running integration tests
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Skipping integration test: set DATABASE_URL in your environment (example: mysql://user:pass@host:3306/db)");
            return;
        }
    };

    // Ensure JWT secret is set for token generation (unsafe required in tests)
    unsafe { std::env::set_var("JWT_SECRET", std::env::var("JWT_SECRET").unwrap_or_else(|_| "test_secret".to_string())); }

    let (base, _db) = database_url.rsplit_once('/').expect("DATABASE_URL should include db name");
    let admin_url = base.to_string();
    let test_db = "db_backend_api_jwt_test_user_list";

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

    // insert a few users (e.g., 3)
    for i in 0..3 {
        let email = format!("list{}@example.com", i);
        let name = format!("Lister {}", i);
        let pw = bcrypt::hash("password", bcrypt::DEFAULT_COST).expect("hash pw");
        sqlx::query("INSERT INTO users (name, email, password) VALUES (?, ?, ?)")
            .bind(name)
            .bind(email)
            .bind(pw)
            .execute(&pool)
            .await
            .expect("insert user");
    }

    // pick one user to generate token (use first user as admin)
    let admin_id: i64 = sqlx::query_scalar("SELECT id FROM users ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("fetch admin id");

    let token = backend_api_jwt::utils::jwt::generate_jwt_token(admin_id).await.expect("generate token");

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
    let url = format!("http://{}/users", addr);

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
    assert_eq!(users.len(), 3);
    let meta = &data["meta"];
    assert_eq!(meta["page"].as_u64().unwrap_or(0), 1);
    assert_eq!(meta["per_page"].as_u64().unwrap_or(0), 50);
    assert_eq!(meta["total"].as_u64().unwrap_or(0), 3);

    // cleanup: drop test db
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
}
