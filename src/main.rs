use std::net::SocketAddr;
use dotenvy::dotenv;

use backend_api_jwt::config;
use backend_api_jwt::create_app;

#[tokio::main]
async fn main() {
    // Load environment variables from .env file (if present)
    dotenv().ok();

    // Friendly check for required env before we attempt to connect
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("Error: DATABASE_URL is not set. Copy `.env.test` to `.env` and update credentials, or set DATABASE_URL in your environment. See README.md for details.");
        std::process::exit(1);
    }

    // Initialize tracing for structured logs
    tracing_subscriber::fmt::init();

    // Establish database connection (and run migrations)
    let db_pool = config::database::establish_connection().await;

    // Create the application router using library helper (CORS will be configured there based on env)
    let app = create_app(db_pool.clone());

    // PORT from environment variable or default to 3000
    let port: u16 = std::env::var("APP_PORT")
        .unwrap_or_else(|_| "3002".to_string())
        .parse()
        .expect("APP_PORT must be a valid u16 number");

    // Host to bind to (from env), default to 127.0.0.1 for safety
    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

    // Address to bind the server
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("APP_HOST:APP_PORT must form a valid socket address");

    // Print the server address
    println!("Listening on http://{}", addr);
    
    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await.expect("failed to bind to address");
    axum::serve(listener, app.into_make_service()).await.unwrap();
}   