use axum::{
    Extension,
    Json,
    http::StatusCode,
};

use sqlx::MySqlPool;
use validator::Validate;
use serde_json::json;
use crate::utils::handler::HandlerResult;
//import schemas for login request and response
use crate::schemas::login_schema::{LoginSchema, LoginResponseSchema};
//import util response API
use crate::utils::response::ApiResponse; 
//import util JWT generation
use crate::utils::jwt::generate_jwt_token;
// Handler for user login
pub async fn login_handler(
    Extension(db_pool): Extension<MySqlPool>,
    Json(payload): Json<LoginSchema>,
) -> HandlerResult {
    // Validate the incoming payload
    if let Err(errors) = payload.validate() {
        // Build a structured map: field -> [messages]
        let mut errors_map = serde_json::Map::new();
        for (field, errs) in errors.field_errors().iter() {
            let msgs: Vec<String> = errs.iter()
                .map(|e| e.message.clone().unwrap_or_else(|| "Invalid input".into()).to_string())
                .collect();
            errors_map.insert(field.to_string(), json!(msgs));
        }
        let response = ApiResponse::error_with_data("Validation error", json!({ "errors": serde_json::Value::Object(errors_map) }));
        return Err((StatusCode::BAD_REQUEST, Json(response)));
    }
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
        "#
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
        },
        _ => {
            let response = ApiResponse::error_with_data("Unauthorized", json!({ "error": "Invalid email or password" }));
            return Err((StatusCode::UNAUTHORIZED, Json(response)));
        }
    };

    // Verify password in a blocking thread to avoid blocking the async runtime
    let pw = payload.password.clone();
    let stored_pw = stored_password.clone();
    let is_password_valid = match tokio::task::spawn_blocking(move || bcrypt::verify(&pw, &stored_pw)).await {
        Ok(Ok(valid)) => valid,
        Ok(Err(_)) => false,
        Err(join_err) => {
            let response = ApiResponse::error_with_data("Hash error", json!({ "error": "Failed to verify password", "details": join_err.to_string() }));
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)));
        }
    };

    if !is_password_valid {
        let response = ApiResponse::error_with_data("Unauthorized", json!({ "error": "Invalid email or password" }));
        return Err((StatusCode::UNAUTHORIZED, Json(response)));
    }
    // Generate JWT token
    let token = generate_jwt_token(user.id).await.map_err(|e| {
        let response = ApiResponse::error_with_data("Token error", json!({ "error": "Failed to generate token", "details": e.to_string() }));
        (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
    })?;
    // Build response schema
    let login_response = LoginResponseSchema {
        user,
        token,
    };
    let response = ApiResponse::success_with_data("Login successful", json!(login_response));
    Ok((StatusCode::OK, Json(response)))
}
// Note: In a real application, consider logging failed login attempts for security monitoring.
