use axum::{http::StatusCode, Json};
use crate::utils::response::ApiResponse;

/// Generic handler result type used across HTTP handlers to simplify signatures.
///
/// Default payload type is `serde_json::Value` for flexibility.
pub type HandlerResult<T = serde_json::Value> = Result<(StatusCode, Json<ApiResponse<T>>), (StatusCode, Json<ApiResponse<T>>)>
;
