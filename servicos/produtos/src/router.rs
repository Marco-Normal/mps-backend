use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    handlers::{
        create_product_handler, delete_product_by_id, get_product_by_id, get_products_by_query,
        update_product_by_id,
    },
    models::AppState,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/products", post(create_product_handler))
        .route("/api/products/search", get(get_products_by_query))
        .route(
            "/api/products/{id}",
            get(get_product_by_id)
                .delete(delete_product_by_id)
                .patch(update_product_by_id),
        )
        .with_state(app_state)
}
