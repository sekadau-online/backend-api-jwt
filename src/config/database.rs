use sqlx::mysql::{MySqlPoolOptions, MySqlPool};
use std::error::Error;

pub async fn establish_connection() -> Result<MySqlPool, Box<dyn Error + Send + Sync>> {
    // Load the DATABASE_URL from environment variables
    let database_url = std::env::var("DATABASE_URL").map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    // Create and return the MySQL connection pool
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    println!("Successfully connected to the database");

    // Run migrations automatically on startup
    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
        eprintln!("Failed to run database migrations: {}", e);

        // Try to detect a VersionMismatch (partially applied migration) in multiple
        // possible message formats and attempt recovery by removing the partial
        // `_sqlx_migrations` row and retrying once.
        let err_str = format!("{}", e);

        // Helper to perform delete+retry for a given version string
        async fn delete_and_retry(pool: &MySqlPool, version: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
            eprintln!("Detected partially applied migration {}. Attempting recovery...", version);
            match sqlx::query("DELETE FROM `_sqlx_migrations` WHERE version = ?")
                .bind(version)
                .execute(pool)
                .await
            {
                Ok(_) => {
                    eprintln!("Deleted partial migration row for {}. Retrying migrations.", version);
                }
                Err(del_err) => {
                    eprintln!("Failed to delete partial migration row for {}: {}", version, del_err);
                    return Err(Box::new(del_err));
                }
            }

            // Retry migrations once
            if let Err(retry_err) = sqlx::migrate!("./migrations").run(pool).await {
                eprintln!("Retry failed: {}", retry_err);
                return Err(Box::new(retry_err));
            } else {
                println!("Database migrations applied successfully after recovery");
            }

            Ok(())
        }

        // Try to parse `VersionMismatch(...)` pattern first
        if let Some(start) = err_str.find("VersionMismatch(") {
            if let Some(open_paren) = err_str[start..].find('(') {
                let rest = &err_str[start + open_paren + 1..];
                if let Some(close_paren) = rest.find(')') {
                    let version = &rest[..close_paren];
                    delete_and_retry(&pool, version).await?;
                } else {
                    eprintln!("Migration error (unexpected format): {}", err_str);
                    return Err(Box::new(e));
                }
            } else {
                eprintln!("Migration error (unexpected format): {}", err_str);
                return Err(Box::new(e));
            }
        }
        // Also support messages like: "migration 20260123121000 is partially applied; fix and remove row from `_sqlx_migrations` table"
        else if let Some(idx) = err_str.find("migration ") {
            let rest = &err_str[idx + "migration ".len()..];
            // take leading digits as version
            let version_digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !version_digits.is_empty() && (rest.contains("partially applied") || (rest.contains("previously applied") && rest.contains("modified"))) {
                delete_and_retry(&pool, &version_digits).await?;
            } else {
                eprintln!("Migration error (unexpected format): {}", err_str);
                return Err(Box::new(e));
            }
        } else {
            eprintln!("Failed to run database migrations: {}", err_str);
            return Err(Box::new(e));
        }
    } else {
        println!("Database migrations applied successfully");
    }

    Ok(pool)
}