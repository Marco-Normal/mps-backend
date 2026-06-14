use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use miette::Diagnostic;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Serialize)]
pub struct ItemValidationError {
    pub id_product: i32,
    pub reason: String,
}

#[derive(Debug, Diagnostic, Error)]
pub enum AppError {
    #[error("{service} with ID {id} not found")]
    NotFound { service: String, id: String },
    #[error("Internal server error: {0}")]
    Internal(String),
    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),
    #[error("{0} already exists")]
    Conflict(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Unprocessable: {0}")]
    UnprocessableEntity(String),
    #[error("Validation failed")]
    ValidationFailed { items: Vec<ItemValidationError> },
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            AppError::NotFound { service, id } => (
                StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({
                    "status": "error",
                    "message": format!("{service} with ID {id} not found"),
                })),
            ),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "status": "error", "message": msg })),
            ),
            AppError::DbError(e) => {
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "status": "error",
                        "message": "Internal server error",
                    })),
                )
            }
            AppError::Conflict(service) => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("{service} already exists."),
                })),
            ),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Unauthorized",
                })),
            ),
            AppError::UnprocessableEntity(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "status": "error",
                    "message": msg,
                })),
            ),
            AppError::ValidationFailed { items } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Product validation failed",
                    "items": items,
                })),
            ),
        };

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn not_found_accepts_string_id() {
        let e = AppError::NotFound {
            service: "Order".to_string(),
            id: "42".to_string(),
        };
        assert!(e.to_string().contains("Order"));
        assert!(e.to_string().contains("42"));
    }

    #[test]
    fn unauthorized_returns_401() {
        let e = AppError::Unauthorized;
        let resp = e.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn unprocessable_entity_returns_422() {
        let e = AppError::UnprocessableEntity("bad transition".to_string());
        let resp = e.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    }
}
