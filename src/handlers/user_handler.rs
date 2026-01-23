use axum::{
    Extension,
    Json,
    http::StatusCode,
};

use sqlx::MySqlPool;
use serde_json::{json, Value};

// Import Models User
use crate::models::user::User;

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