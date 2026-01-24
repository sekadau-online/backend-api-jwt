use axum::{
    Extension,
    Json,
    http::StatusCode,
};

use sqlx::MySqlPool;
use bcrypt::verify;
use validator::Validate;
use serde_json::json;
use crate::utils::handler::HandlerResult;
//import schemas for login request and response
use crate::schemas::login_schema::{LoginSchema, LoginResponseSchema, UserLoginResponseSchema};
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
            return Err((StatusCode::UNAUTHORIZED, Json(response)));
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
            return Err((StatusCode::UNAUTHORIZED, Json(response)));
        }
    };
    let is_password_valid = verify(&payload.password, &stored_password).unwrap_or_default();
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
