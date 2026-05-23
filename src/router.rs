use std::sync::Arc;

use axum::{Router, routing::post};

use crate::{handlers::create_product_handler, models::AppState};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/products", post(create_product_handler))
        .with_state(app_state)
}
