use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Diagnostic, Error)]
pub enum AppError {
    #[error("{service} with ID {id} not found")]
    NotFound { service: String, id: i32 },
    #[error("Internal server error: {0}")]
    Internal(String),
    #[error("Database error {0}")]
    DbError(#[from] sqlx::Error),
    #[error("{0} already exists")]
    Conflict(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound { service, id } => (
                StatusCode::NOT_FOUND,
                format!("{service} with ID {id} not found"),
            ),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::DbError(e) => {
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::Conflict(service) => {
                (StatusCode::CONFLICT, format!("{service} already exists."))
            }
        };

        let body = Json(serde_json::json!({
            "status": "error",
            "message": message,
        }));

        (status, body).into_response()
    }
}
