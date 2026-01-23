use serde::{Serialize, Deserialize};
use validator::Validate;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct LoginSchema {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponseSchema {
    pub user: UserResponseSchema,
    pub token: String,
}
