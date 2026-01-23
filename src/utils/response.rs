use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    /// Convenience constructor when you have a concrete `T` value
    pub fn success_with_data(message: &str, data: T) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            data: Some(data),
        }
    }

    /// Error constructor that includes structured `data` (e.g. validation errors)
    pub fn error_with_data(message: &str, data: T) -> Self {
        Self {
            success: false,
            message: message.to_string(),
            data: Some(data),
        }
    }
}  