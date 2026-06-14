use std::sync::Arc;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
};

use crate::{
    handlers::{
        create_product_handler, delete_product_by_id, delete_product_image,
        get_product_by_id, get_product_images, get_products_by_query,
        update_product_by_id, upload_product_image,
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
        .route(
            "/api/products/{id}/imagens",
            post(upload_product_image)
                .layer(DefaultBodyLimit::max(5 * 1024 * 1024))
                .get(get_product_images),
        )
        .route(
            "/api/products/{id}/imagens/{img_id}",
            delete(delete_product_image),
        )
        .with_state(app_state)
}
