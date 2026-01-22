use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

/// Database configuration helpers
pub mod database {
    use super::*;

    /// Establish a connection pool using the `DATABASE_URL` environment variable.
    /// Panics if `DATABASE_URL` is not set or the connection cannot be established.
    pub async fn establish_connection() -> MySqlPool {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .expect("Failed to create MySQL connection pool")
    }
}
