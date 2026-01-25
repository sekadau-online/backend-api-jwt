use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct UserStoreRequestSchema {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,

    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(length(min = 6, message = "Password must be at least 6 characters long"))]
    pub password: String,
}

#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
pub struct UserResponseSchema {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UserUpdateRequestSchema {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: Option<String>,

    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,

    #[validate(length(min = 6, message = "Password must be at least 6 characters long"))]
    pub password: Option<String>,
}
