use crate::utils::handler::HandlerResult;
use crate::utils::response::ApiResponse;
use axum::{Extension, Json, http::StatusCode};
use serde_json::json;
use sqlx::MySqlPool;

pub async fn health(Extension(db): Extension<MySqlPool>) -> HandlerResult {
    // Try a simple DB ping/query
    let res: Result<i64, sqlx::Error> = sqlx::query_scalar("SELECT 1").fetch_one(&db).await;

    match res {
        Ok(_) => {
            let data = json!({ "db": "ok" });
            let response = ApiResponse::success_with_data("OK", data);
            Ok((StatusCode::OK, Json(response)))
        }
        Err(e) => {
            let response = ApiResponse::error_with_data(
                "Unhealthy",
                json!({ "db": "error", "details": e.to_string() }),
            );
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)))
        }
    }
}
