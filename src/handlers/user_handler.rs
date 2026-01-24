use axum::{
    Extension,
    Json,
    http::StatusCode,
};
use axum::extract::Path;

use sqlx::MySqlPool;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::utils::handler::HandlerResult;use bcrypt::hash;
use validator::Validate;

// Import Models User
use crate::models::user::User;

// Import schemas for creating a user
use crate::schemas::user_schema::{UserResponseSchema, UserStoreRequestSchema, UserUpdateRequestSchema};
// use crate::schemas::user_schema::UserUpdateRequestSchema;

// Import util response API
use crate::utils::response::ApiResponse;

#[derive(Deserialize)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub async fn index(
    Extension(db): Extension<MySqlPool>,
    axum::extract::Query(params): axum::extract::Query<PaginationParams>,
) -> HandlerResult {
    // Bound and default pagination values
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) as i64 * per_page as i64;

    // Fetch paginated users
    let users_result = sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, email, created_at, updated_at
        FROM users
        ORDER BY id
        LIMIT ? OFFSET ?
        "#
    )
    .bind(per_page as i64)
    .bind(offset)
    .fetch_all(&db)
    .await;

    // Fetch total count for pagination metadata
    let total_result: Result<i64, sqlx::Error> = sqlx::query_scalar("SELECT COUNT(1) FROM users")
        .fetch_one(&db)
        .await;

    match (users_result, total_result) {
        (Ok(users), Ok(total)) => {
            let response = ApiResponse::success_with_data("Users fetched successfully", json!({
                "users": users,
                "meta": { "page": page, "per_page": per_page, "total": total }
            }));
            Ok((StatusCode::OK, Json(response)))
        },
        (Err(e), _) | (_, Err(e)) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to fetch users", "details": e.to_string() }));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)))
        }
    }
}

pub async fn store(
    Extension(db_pool): Extension<MySqlPool>,
    Json(payload): Json<UserStoreRequestSchema>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), (StatusCode, Json<ApiResponse<Value>>)> {
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

    // Normalize email
    let email_normalized = payload.email.trim().to_lowercase();

    // Check if the email already exists
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

    // Hash the password
    let hashed_password = hash(&payload.password, bcrypt::DEFAULT_COST).map_err(|_| {
        let response = ApiResponse::error_with_data("Hash error", json!({ "error": "Failed to hash password" }));
        (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
    })?;

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
    let user = match sqlx::query_as::<_, UserResponseSchema>(
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
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)));
        }
    };

    let user_value = match serde_json::to_value(user) {
        Ok(v) => v,
        Err(e) => {
            let response = ApiResponse::error_with_data("Serialization error", json!({ "error": "Failed to serialize user", "details": e.to_string() }));
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)));
        }
    };
    let response = ApiResponse::success_with_data("User created", user_value);
    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn show(
    Extension(db_pool): Extension<MySqlPool>,
    axum::extract::Path(user_id): axum::extract::Path<i64>,
) -> HandlerResult {
    // Fetch user by ID
    let user_result = sqlx::query_as::<_, UserResponseSchema>(
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
            let user_value = match serde_json::to_value(user) {
                Ok(v) => v,
                Err(e) => {
                    let response = ApiResponse::error_with_data("Serialization error", json!({ "error": "Failed to serialize user", "details": e.to_string() }));
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)));
                }
            };
            let response = ApiResponse::success_with_data("User fetched successfully", user_value);
            Ok((StatusCode::OK, Json(response)))
        },
        Ok(None) => {
            let response = ApiResponse::error_with_data("Not Found", json!({ "error": "User not found" }));
            Err((StatusCode::NOT_FOUND, Json(response)))
        },
        Err(e) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to fetch user", "details": e.to_string() }));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)))
        }
    }
} 

pub async fn update(
    Path(id): Path<i64>,
    Extension(db_pool): Extension<MySqlPool>,
    Json(payload): Json<UserUpdateRequestSchema>,
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
                return Err((StatusCode::NOT_FOUND, Json(response)));
            }
            let response = ApiResponse::success_with_data("User updated successfully", json!({ "user_id": id }));
            Ok((StatusCode::OK, Json(response)))
        },
        Err(e) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to update user", "details": e.to_string() }));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)))
        }
    }
}

pub async fn destroy(
    Path(id): Path<i64>,
    Extension(db_pool): Extension<MySqlPool>,
) -> HandlerResult {
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
                return Err((StatusCode::NOT_FOUND, Json(response)));
            }
            let response = ApiResponse::success_with_data("User deleted successfully", json!({ "user_id": id }));
            Ok((StatusCode::OK, Json(response)))
        },
        Err(e) => {
            let response = ApiResponse::error_with_data("Database error", json!({ "error": "Failed to delete user", "details": e.to_string() }));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)))
        }
    }
}