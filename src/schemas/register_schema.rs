use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use validator::Validate;

// Request schema: only needs Deserialize + validation
#[derive(Debug, Deserialize, Validate)]
pub struct RegisterSchema {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,

    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(length(min = 6, message = "Password must be at least 6 characters long"))]
    pub password: String,
}

// Response schema: used for returning data, does not need validation
#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
pub struct RegisterResponseSchema {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}