use std::sync::Arc;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{
        Method,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    routing::{delete, get, post},
};
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::{
    handlers::{
        count_products_handler, create_product_handler, delete_product_by_id,
        delete_product_image, get_product_by_id, get_product_images,
        get_products_by_query, list_products_handler, update_product_by_id,
        upload_product_image,
    },
    models::AppState,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(app_state.frontend_url.clone())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION]);

    Router::new()
        .route("/api/products", post(create_product_handler).get(list_products_handler))
        .route("/api/products/search", get(get_products_by_query))
        .route("/api/products/count", get(count_products_handler))
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
        .nest_service("/static", ServeDir::new(&app_state.static_dir))
        .layer(cors)
        .with_state(app_state)
}
