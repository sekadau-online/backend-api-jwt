use axum::{Extension, Json, http::StatusCode};

use crate::utils::handler::HandlerResult;
use serde_json::json;
use sqlx::MySqlPool;
//import schemas for login request and response
use crate::schemas::login_schema::{LoginResponseSchema, LoginSchema};
//import util response API
use crate::utils::response::ApiResponse;
//import util JWT generation
use crate::utils::jwt::generate_jwt_token;
// Handler for user login
pub async fn login_handler(
    Extension(db_pool): Extension<MySqlPool>,
    Json(payload): Json<LoginSchema>,
) -> HandlerResult {
    // Validate the incoming payload (reusable helper)
    crate::utils::validation::validate_payload(&payload)?;
    // Normalize email for consistent lookup
    let email_normalized = payload.email.trim().to_lowercase();
    // Fetch user by email (include password to avoid extra roundtrip)
    #[derive(sqlx::FromRow, Debug)]
    struct UserWithPassword {
        id: i64,
        name: String,
        email: String,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
        password: String,
    }

    let user_record = sqlx::query_as::<_, UserWithPassword>(
        r#"
        SELECT id, name, email, created_at, updated_at, password
        FROM users
        WHERE email = ?
        "#,
    )
    .bind(&email_normalized)
    .fetch_optional(&db_pool)
    .await;

    let (user, stored_password) = match user_record {
        Ok(Some(u)) => {
            let user_response = crate::schemas::login_schema::UserLoginResponseSchema {
                id: u.id,
                name: u.name.clone(),
                email: u.email.clone(),
                created_at: u.created_at,
                updated_at: u.updated_at,
            };
            (user_response, u.password)
        }
        _ => {
            let response = ApiResponse::error_with_data(
                "Unauthorized",
                json!({ "error": "Invalid email or password" }),
            );
            return Err((StatusCode::UNAUTHORIZED, Json(response)));
        }
    };

    // Verify password using shared helper (runs bcrypt.verify in blocking thread with timeout)
    let timeout_secs: Option<u64> = std::env::var("BCRYPT_VERIFY_TIMEOUT_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok());
    let pw = payload.password.clone();
    let stored_pw = stored_password.clone();

    match crate::utils::auth::verify_password_blocking(pw, stored_pw, timeout_secs).await {
        Ok(true) => {
            // ok
        }
        Ok(false) => {
            tracing::warn!(
                "login failed: invalid credentials for email={}",
                email_normalized
            );
            let response = ApiResponse::error_with_data(
                "Unauthorized",
                json!({ "error": "Invalid email or password" }),
            );
            return Err((StatusCode::UNAUTHORIZED, Json(response)));
        }
        Err(e) => {
            tracing::error!("hash verify error: {}", e);
            let response = ApiResponse::error_with_data(
                "Hash error",
                json!({ "error": "Failed to verify password" }),
            );
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)));
        }
    }
    // Generate JWT token
    let token = generate_jwt_token(user.id).await.map_err(|e| {
        let response = ApiResponse::error_with_data(
            "Token error",
            json!({ "error": "Failed to generate token", "details": e.to_string() }),
        );
        (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
    })?;
    // Build response schema
    let login_response = LoginResponseSchema { user, token };
    let response = ApiResponse::success_with_data("Login successful", json!(login_response));
    Ok((StatusCode::OK, Json(response)))
}
// Note: In a real application, consider logging failed login attempts for security monitoring.
