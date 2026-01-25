use axum::{Json, extract::Request, http::StatusCode, middleware::Next, response::Response};

use crate::utils::jwt::verify_jwt_token;
use crate::utils::response::ApiResponse;

// Type error response alias
type AuthErrorResponse = (StatusCode, Json<ApiResponse<serde_json::Value>>);

// Authentication middleware to protect routes
pub async fn auth_middleware(mut req: Request, next: Next) -> Result<Response, AuthErrorResponse> {
    // Extract the Authorization header from the request
    let header = req.headers();
    let auth_header = match header.get("Authorization") {
        Some(h) => h.to_str().unwrap_or(""),
        None => {
            let response = ApiResponse::error_with_data(
                "Unauthorized",
                serde_json::json!({ "error": "Missing Authorization header" }),
            );
            return Err((StatusCode::UNAUTHORIZED, Json(response)));
        }
    };

    // Check if the header starts with "Bearer "
    if !auth_header.starts_with("Bearer ") {
        let response = ApiResponse::error_with_data(
            "Unauthorized",
            serde_json::json!({ "error": "Invalid Authorization header format" }),
        );
        return Err((StatusCode::UNAUTHORIZED, Json(response)));
    }

    // Extract the token part
    let token = auth_header.trim_start_matches("Bearer ").trim();

    // Verify the JWT token
    match verify_jwt_token(token).await {
        Ok(claims) => {
            // You can attach claims to request extensions if needed
            req.extensions_mut().insert(claims);
            // Proceed to the next middleware/handler
            Ok(next.run(req).await)
        }
        Err(e) => {
            let response = ApiResponse::error_with_data(
                "Unauthorized",
                serde_json::json!({ "error": "Invalid or expired token", "details": e.to_string() }),
            );
            Err((StatusCode::UNAUTHORIZED, Json(response)))
        }
    }
}
