use axum::{
    Extension,
    Json,
    http::StatusCode,
};

use sqlx::MySqlPool;
use bcrypt::verify;
use validator::Validate;
use serde_json::{json, Value};

//import schemas for login request and response
use crate::schemas::{LoginSchema, LoginResponseSchema, UserLoginResponseSchema};
//import util response API
use crate::utils::response::ApiResponse; 
//import util JWT generation
use crate::utils::jwt::generate_jwt_token;
// Handler for user login
pub async fn login_handler(
    Extension(db_pool): Extension<MySqlPool>,
    Json(payload): Json<LoginSchema>,
) -> (StatusCode, Json<ApiResponse<Value>>) {
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
        return (StatusCode::BAD_REQUEST, Json(response));
    }
    // Normalize email for consistent lookup
    let email_normalized = payload.email.trim().to_lowercase();
    // Fetch user by email
    let user_record = sqlx::query_as::<_, UserLoginResponseSchema>(
        r#"
        SELECT id, name, email, created_at, updated_at
        FROM users
        WHERE email = ?
        "#
    )
    .bind(&email_normalized)
    .fetch_optional(&db_pool)
    .await;
    let user = match user_record {
        Ok(Some(user)) => user,
        _ => {
            let response = ApiResponse::error_with_data("Unauthorized", json!({ "error": "Invalid email or password" }));
            return (StatusCode::UNAUTHORIZED, Json(response));
        }
    };
    // Verify password
    let stored_password: String = match sqlx::query_scalar::<_, String>(
        r#"
        SELECT password
        FROM users
        WHERE email = ?
        "#
    )
    .bind(&email_normalized)
    .fetch_one(&db_pool)
    .await {
        Ok(pw) => pw,
        Err(_) => {
            let response = ApiResponse::error_with_data("Unauthorized", json!({ "error": "Invalid email or password" }));
            return (StatusCode::UNAUTHORIZED, Json(response));
        }
    };
    let is_password_valid = match verify(&payload.password, &stored_password) {
        Ok(valid) => valid,
        Err(_) => false,
    };
    if !is_password_valid { 
        let response = ApiResponse::error_with_data("Unauthorized", json!({ "error": "Invalid email or password" }));
        return (StatusCode::UNAUTHORIZED, Json(response));
    }
    // Generate JWT token
    let token = match generate_jwt_token(user.id).await {
        Ok(t) => t,
        Err(e) => {
            let response = ApiResponse::error_with_data("Token error", json!({ "error": "Failed to generate token", "details": e.to_string() }));
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };
    // Build response schema
    let login_response = LoginResponseSchema {
        user,
        token,
    };
    let response = ApiResponse::success_with_data("Login successful", json!(login_response));
    (StatusCode::OK, Json(response))
}
// Note: In a real application, consider logging failed login attempts for security monitoring.
