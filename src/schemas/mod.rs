pub mod register_schema;
pub mod login_schema;

// Re-export common schemas for easier imports
pub use crate::schemas::register_schema::{RegisterSchema, RegisterResponseSchema};
pub use crate::schemas::login_schema::{LoginSchema, LoginResponseSchema, UserResponseSchema};