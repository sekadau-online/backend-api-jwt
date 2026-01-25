use axum::{http::StatusCode, Json};
use validator::Validate;
use serde_json::{json, Value};
use crate::utils::response::ApiResponse;

/// Validate a payload implementing `validator::Validate` and return an axum-compatible
/// error tuple on validation failure so handlers can `?` it.
pub fn validate_payload<T: Validate>(payload: &T) -> Result<(), (StatusCode, Json<ApiResponse<Value>>)> {
    if let Err(errors) = payload.validate() {
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use validator::Validate;

    #[derive(Deserialize, Validate)]
    struct TestPayload {
        #[validate(length(min = 1))]
        name: String,
    }

    #[test]
    fn test_validate_payload_err() {
        let p = TestPayload { name: "".into() };
        let res = validate_payload(&p);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_payload_ok() {
        let p = TestPayload { name: "ok".into() };
        let res = validate_payload(&p);
        assert!(res.is_ok());
    }
}
