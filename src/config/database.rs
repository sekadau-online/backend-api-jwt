use sqlx::mysql::{MySqlPoolOptions, MySqlPool};

pub async fn establish_connection() -> MySqlPool {
// Load the DATABASE_URL from environment variables 
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env file");
// Create and return the MySQL connection pool
    let pool = match MySqlPoolOptions::new()
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
    };

    // Run migrations automatically on startup
    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
        eprintln!("Failed to run database migrations: {}", e);

        // Try to detect a VersionMismatch (partially applied migration). If found,
        // remove the partial row from `_sqlx_migrations` and retry once.
        let err_str = format!("{}", e);
        if let Some(start) = err_str.find("VersionMismatch(") {
            if let Some(open_paren) = err_str[start..].find('(') {
                let rest = &err_str[start + open_paren + 1..];
                if let Some(close_paren) = rest.find(')') {
                    let version = &rest[..close_paren];
                    eprintln!("Detected VersionMismatch for migration {}, attempting recovery...", version);

                    // Attempt to delete the partial migration row
                    match sqlx::query("DELETE FROM `_sqlx_migrations` WHERE version = ?")
                        .bind(version)
                        .execute(&pool)
                        .await
                    {
                        Ok(_) => {
                            eprintln!("Deleted partial migration row for {}. Retrying migrations.", version);
                        }
                        Err(del_err) => {
                            eprintln!("Failed to delete partial migration row for {}: {}", version, del_err);
                            std::process::exit(1);
                        }
                    }

                    // Retry migrations once
                    if let Err(retry_err) = sqlx::migrate!("./migrations").run(&pool).await {
                        eprintln!("Retry failed: {}", retry_err);
                        std::process::exit(1);
                    } else {
                        println!("Database migrations applied successfully after recovery");
                    }
                } else {
                    std::process::exit(1);
                }
            } else {
                std::process::exit(1);
            }
        } else {
            std::process::exit(1);
        }
    } else {
        println!("Database migrations applied successfully");
    }

    pool
}