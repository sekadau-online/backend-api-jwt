use axum::{
    Extension,
    Json,
    http::StatusCode,
};
use sqlx::MySqlPool;
use validator::Validate;
use serde_json::json;
use crate::utils::handler::HandlerResult;
// Import schemas request and response register
use crate::schemas::register_schema::{RegisterSchema, RegisterResponseSchema};
// Import util response API
use crate::utils::response::ApiResponse;

// Handler for user registration
pub async fn register_handler(
    Extension(db_pool): Extension<MySqlPool>,
    Json(payload): Json<RegisterSchema>,
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

    // Normalize email for consistent duplicate checks and storage
    let email_normalized = payload.email.trim().to_lowercase();
    // Check if the email already exists to avoid duplicate registrations
    let existing_count: i64 = sqlx::query_scalar("SELECT COUNT(1) FROM users WHERE email = ?")
        .bind(&email_normalized)
        .fetch_one(&db_pool)
        .await
        .map_err(|e| {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to check existing user", "details": e.to_string() }));
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        })?;

    if existing_count > 0 {
        let response = ApiResponse::error_with_data("Conflict", json!({ "error": "Email already registered", "field": "email" }));
        return Err((StatusCode::CONFLICT, Json(response)));
    }

    // Hash the password in a blocking thread to avoid blocking the async runtime
    let pw_to_hash = payload.password.clone();
    let hashed_password = match tokio::task::spawn_blocking(move || bcrypt::hash(&pw_to_hash, 4)).await {
        Ok(Ok(h)) => h,
        Ok(Err(_)) => {
            let response = ApiResponse::error_with_data("Hash error", json!({ "error": "Failed to hash password" }));
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)));
        }
        Err(join_err) => {
            let response = ApiResponse::error_with_data("Hash error", json!({ "error": "Failed to hash password", "details": join_err.to_string() }));
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)));
        }
    };

    // Insert the new user into the database
    let res = sqlx::query(
        r#"
        INSERT INTO users (name, email, password)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(&payload.name)
    .bind(&email_normalized)
    .bind(&hashed_password)
    .execute(&db_pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if let Some(code) = db_err.code() && code == "1062" {
                let response = ApiResponse::error_with_data("Conflict", json!({ "error": "Email already registered", "field": "email" }));
                return (StatusCode::CONFLICT, Json(response));
            }
            let msg = db_err.message().to_string();
            if msg.to_lowercase().contains("duplicate") {
                let response = ApiResponse::error_with_data("Conflict", json!({ "error": "Email already registered", "field": "email" }));
                return (StatusCode::CONFLICT, Json(response));
            }
        }
        let e_str = e.to_string();
        if e_str.contains("1062") || e_str.to_lowercase().contains("duplicate") {
            let response = ApiResponse::error_with_data("Conflict", json!({ "error": "Email already registered", "field": "email" }));
            return (StatusCode::CONFLICT, Json(response));
        }
        let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to register user", "details": e.to_string() }));
        (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
    })?;

    let user_id = res.last_insert_id() as i64;
    // Fetch the newly created user
    let user = sqlx::query_as::<_, RegisterResponseSchema>(
        r#"
        SELECT id, name, email, created_at, updated_at
        FROM users
        WHERE id = ?
        "#,
    )
    .bind(user_id)
    .fetch_one(&db_pool)
    .await
    .map_err(|e| {
        let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to fetch registered user", "details": e.to_string() }));
        (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
    })?;

    // Serialize user into JSON value
    let user_value = serde_json::to_value(user).map_err(|e| {
        let response = ApiResponse::error_with_data("Serialization error", json!({ "error": "Failed to serialize user", "details": e.to_string() }));
        (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
    })?;
    let response = ApiResponse::success_with_data("User registered", user_value);
    Ok((StatusCode::CREATED, Json(response)))
}