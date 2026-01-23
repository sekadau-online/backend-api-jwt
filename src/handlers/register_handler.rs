use axum::{
    Extension,
    Json,
    http::StatusCode,
};
use sqlx::MySqlPool;
use bcrypt::hash;
use validator::Validate;
use serde_json::json;

// Import schemas request and response register
use crate::schemas::{RegisterSchema, RegisterResponseSchema};
// Import util response API
use crate::utils::response::ApiResponse;

// Handler for user registration
pub async fn register_handler(
    Extension(db_pool): Extension<MySqlPool>,
    Json(payload): Json<RegisterSchema>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
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

    // Normalize email for consistent duplicate checks and storage
    let email_normalized = payload.email.trim().to_lowercase();
    // Check if the email already exists to avoid duplicate registrations
    let existing_count: i64 = match sqlx::query_scalar("SELECT COUNT(1) FROM users WHERE email = ?")
        .bind(&email_normalized)
        .fetch_one(&db_pool)
        .await
    {
        Ok(cnt) => cnt,
        Err(e) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to check existing user", "details": e.to_string() }));
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    if existing_count > 0 {
        let response = ApiResponse::error_with_data("Conflict", json!({ "error": "Email already registered", "field": "email" }));
        return (StatusCode::CONFLICT, Json(response));
    }

    // Hash the password
    let hashed_password = match hash(&payload.password, 4) {
        Ok(hp) => hp,
        Err(_) => {
            let response = ApiResponse::error_with_data("Hash error", json!({ "error": "Failed to hash password" }));
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    // Insert the new user into the database
    let result = sqlx::query(
        r#"
        INSERT INTO users (name, email, password)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(&payload.name)
    .bind(&email_normalized)
    .bind(&hashed_password)
    .execute(&db_pool)
    .await;

    let user_id = match result {
        Ok(res) => res.last_insert_id() as i64,
        Err(e) => {
            // If another transaction inserted the same email in the meantime, translate that to Conflict if duplicate entry
            // Prefer matching the database error code when available (e.g., MySQL 1062)
            match &e {
                sqlx::Error::Database(db_err) => {
                    if let Some(code) = db_err.code() {
                        if code == "1062" {
                            let response = ApiResponse::error_with_data("Conflict", json!({ "error": "Email already registered", "field": "email" }));
                            return (StatusCode::CONFLICT, Json(response));
                        }
                    }
                    let msg = db_err.message().to_string();
                    if msg.to_lowercase().contains("duplicate") {
                        let response = ApiResponse::error_with_data("Conflict", json!({ "error": "Email already registered", "field": "email" }));
                        return (StatusCode::CONFLICT, Json(response));
                    }
                }
                _ => {}
            }

            // Fallback to string checks (older behavior)
            let e_str = e.to_string();
            if e_str.contains("1062") || e_str.to_lowercase().contains("duplicate") {
                let response = ApiResponse::error_with_data("Conflict", json!({ "error": "Email already registered", "field": "email" }));
                return (StatusCode::CONFLICT, Json(response));
            }

            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to register user", "details": e.to_string() }));
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };
    // Fetch the newly created user
    let user = match sqlx::query_as::<_, RegisterResponseSchema>(
        r#"
        SELECT id, name, email, created_at, updated_at
        FROM users
        WHERE id = ?
        "#,
    )
    .bind(user_id)
    .fetch_one(&db_pool)
    .await {
        Ok(user) => user,
        Err(e) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to fetch registered user", "details": e.to_string() }));
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };
    // Return the successful response (serialize user into a JSON value)
    let response = ApiResponse::success_with_data("User registered", serde_json::to_value(user).unwrap());
    (StatusCode::CREATED, Json(response))
}