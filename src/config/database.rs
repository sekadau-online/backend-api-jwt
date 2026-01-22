use sqlx::mysql::{MySqlPoolOptions, MySqlPool};

pub async fn establish_connection() -> MySqlPool {
// Load the DATABASE_URL from environment variables 
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env file");
// Create and return the MySQL connection pool
    MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            println!("Successfully connected to the database");
            pool
        }
        Err(e) => {
            eprintln!("Failed to connect to the database: {}", e);
            std::process::exit(1); 
        }
    }
}