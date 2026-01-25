use backend_api_jwt::create_app;
use serde_json::Value;
use sqlx::{Executor, MySqlPool};
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn health_check_flow() {
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

    let (base, _db) = database_url
        .rsplit_once('/')
        .expect("DATABASE_URL should include db name");
    let admin_url = base.to_string();
    let test_db = "db_backend_api_jwt_test_health";

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
    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let listener = tokio::net::TcpListener::bind(format!("{}:0", host))
        .await
        .expect("bind");
    let addr = listener.local_addr().unwrap();
    let server = axum::serve(listener, app.into_make_service());
    let _srv = tokio::spawn(async move {
        server.await.unwrap();
    });

    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let url = format!("http://{}/health", addr);

    let res = client.get(&url).send().await.expect("request failed");
    assert_eq!(res.status().as_u16(), 200);
    let body: Value = res.json().await.expect("json");
    assert!(body["success"].as_bool().unwrap_or(false));
    assert_eq!(body["data"]["db"].as_str().unwrap_or(""), "ok");

    // cleanup
    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
}
