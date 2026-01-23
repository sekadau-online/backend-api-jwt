use axum::{
    Extension,
    Json,
    http::StatusCode,
};
use axum::extract::Path;

use sqlx::MySqlPool;
use serde_json::{json, Value};
use bcrypt::hash;
use validator::Validate;

// Import Models User
use crate::models::user::User;

// Import schemas for creating a user
use crate::schemas::{RegisterResponseSchema, UserStoreRequestSchema, UserUpdateRequestSchema};
// use crate::schemas::user_schema::UserUpdateRequestSchema;

// Import util response API
use crate::utils::response::ApiResponse;

pub async fn index(
    Extension(db): Extension<MySqlPool>,
) -> (StatusCode, Json<ApiResponse<Value>>) {
    // Fetch all users from the database
    let users_result = sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, email, created_at, updated_at
        FROM users
        "#
    )
    .fetch_all(&db)
    .await;

    match users_result {
        Ok(users) => {
            let response = ApiResponse::success_with_data("Users fetched successfully", json!({ "users": users }));
            (StatusCode::OK, Json(response))
        },
        Err(e) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to fetch users", "details": e.to_string() }));
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }
}

pub async fn store(
    Extension(db_pool): Extension<MySqlPool>,
    Json(payload): Json<UserStoreRequestSchema>,
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

    // Normalize email
    let email_normalized = payload.email.trim().to_lowercase();

    // Check if the email already exists
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
    let hashed_password = match hash(&payload.password, bcrypt::DEFAULT_COST) {
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

    let response = ApiResponse::success_with_data("User created", serde_json::to_value(user).unwrap());
    (StatusCode::CREATED, Json(response))
}

pub async fn show(
    Extension(db_pool): Extension<MySqlPool>,
    axum::extract::Path(user_id): axum::extract::Path<i64>,
) -> (StatusCode, Json<ApiResponse<Value>>) {
    // Fetch user by ID
    let user_result = sqlx::query_as::<_, RegisterResponseSchema>(
        r#"
        SELECT id, name, email, created_at, updated_at
        FROM users
        WHERE id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(&db_pool)
    .await;

    match user_result {
        Ok(Some(user)) => {
            let response = ApiResponse::success_with_data("User fetched successfully", serde_json::to_value(user).unwrap());
            (StatusCode::OK, Json(response))
        },
        Ok(None) => {
            let response = ApiResponse::error_with_data("Not Found", json!({ "error": "User not found" }));
            (StatusCode::NOT_FOUND, Json(response))
        },
        Err(e) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to fetch user", "details": e.to_string() }));
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }
} 

pub async fn update(
    Path(id): Path<i64>,
    Extension(db_pool): Extension<MySqlPool>,
    Json(payload): Json<UserUpdateRequestSchema>,
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

    // Prepare optional email (trim + lowercase if provided)
    let email_param: Option<String> = payload.email.as_ref().map(|s| s.trim().to_lowercase());

    // Update user in the database (only change fields provided)
    let result = sqlx::query(
        r#"
        UPDATE users
        SET name = COALESCE(?, name),
            email = COALESCE(?, email)
        WHERE id = ?
        "#,
    )
    .bind(payload.name.clone())
    .bind(email_param)
    .bind(id)
    .execute(&db_pool)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                let response = ApiResponse::error_with_data("Not Found", json!({ "error": "User not found" }));
                return (StatusCode::NOT_FOUND, Json(response));
            }
            let response = ApiResponse::success_with_data("User updated successfully", json!({ "user_id": id }));
            (StatusCode::OK, Json(response))
        },
        Err(e) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to update user", "details": e.to_string() }));
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }
}

pub async fn destroy(
    Path(id): Path<i64>,
    Extension(db_pool): Extension<MySqlPool>,
) -> (StatusCode, Json<ApiResponse<Value>>) {
    // Delete user from the database
    let result = sqlx::query(
        r#"
        DELETE FROM users
        WHERE id = ?
        "#,
    )
    .bind(id)
    .execute(&db_pool)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                let response = ApiResponse::error_with_data("Not Found", json!({ "error": "User not found" }));
                return (StatusCode::NOT_FOUND, Json(response));
            }
            let response = ApiResponse::success_with_data("User deleted successfully", json!({ "user_id": id }));
            (StatusCode::OK, Json(response))
        },
        Err(e) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to delete user", "details": e.to_string() }));
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }
}