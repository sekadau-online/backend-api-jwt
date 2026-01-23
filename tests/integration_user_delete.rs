use backend_api_jwt::create_app;
use sqlx::{MySqlPool, Executor};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn user_delete_flow() {
    dotenvy::dotenv().ok();
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Skipping integration test: set DATABASE_URL in your environment (example: mysql://user:pass@host:3306/db)");
            return;
        }
    };

    unsafe { std::env::set_var("JWT_SECRET", std::env::var("JWT_SECRET").unwrap_or_else(|_| "test_secret".to_string())); }

    let (base, _db) = database_url.rsplit_once('/').expect("DATABASE_URL should include db name");
    let admin_url = base.to_string();
    let test_db = "db_backend_api_jwt_test_delete";

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
    let pool = MySqlPool::connect(&test_db_url).await.expect("connect test db");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrations");

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

    pool.execute("DELETE FROM users").await.ok();

    // insert a test user
    let plain_password = "password";
    let hashed = bcrypt::hash(plain_password, bcrypt::DEFAULT_COST).expect("hash pw");
    let email = "testdelete@example.com";
    let insert_res = sqlx::query("INSERT INTO users (name, email, password) VALUES (?, ?, ?)")
        .bind("Delete Test")
        .bind(email)
        .bind(hashed)
        .execute(&pool)
        .await
        .expect("insert user");
    let user_id = insert_res.last_insert_id() as i64;

    // generate token
    let token = backend_api_jwt::utils::jwt::generate_jwt_token(user_id).await.expect("generate token");

    // start app
    let app = create_app(pool.clone());
    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let listener = tokio::net::TcpListener::bind(format!("{}:0", host)).await.expect("bind");
    let addr = listener.local_addr().unwrap();
    let server = axum::serve(listener, app.into_make_service());
    let _srv = tokio::spawn(async move { server.await.unwrap(); });
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let url = format!("http://{}/users/{}", addr, user_id);

    // No auth -> 401 (not 405)
    let res_unauth = client
        .delete(&url)
        .send()
        .await
        .expect("request failed");
    assert_eq!(res_unauth.status().as_u16(), 401);

    // With auth -> 200
    let res = client
        .delete(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("request failed");
    if res.status().as_u16() != 200 {
        let status = res.status();
        let headers = format!("{:?}", res.headers());
        let body = res.text().await.unwrap_or_default();
        panic!("DELETE failed: {} headers={} body={}", status, headers, body);
    }

    // Verify deleted -> GET returns 404
    let res2 = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res2.status().as_u16(), 404);

    admin_pool
        .execute(format!("DROP DATABASE IF EXISTS {}", test_db).as_str())
        .await
        .expect("drop test db");
}
