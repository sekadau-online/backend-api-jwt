use axum::{Router, Extension};
use std::net::SocketAddr;
use dotenvy::dotenv;

mod config;
mod routes;
mod handlers;
mod schemas;
mod utils;

#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenv().ok();

    // Initialize tracing for structured logs
    tracing_subscriber::fmt::init();

    // Establish database connection (and run migrations)
    let _db_pool = config::database::establish_connection().await;

    // Create the application router
    let app = Router::new()
        .merge(routes::auth_routes::auth_routes())
        .layer(Extension(_db_pool));

    // PORT from environment variable or default to 3000
    let port: u16 = std::env::var("APP_PORT")
        .unwrap_or_else(|_| "3002".to_string())
        .parse()
        .expect("APP_PORT must be a valid u16 number");
    
    // Address to bind the server
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    // Print the server address
    println!("Listening on http://{}", addr);
    
    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}   