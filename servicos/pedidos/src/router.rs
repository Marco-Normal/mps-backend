use std::sync::Arc;

use axum::{
    Router,
    http::{
        Method,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    routing::{get, patch, post},
};
use tower_http::cors::CorsLayer;

use crate::{
    admin::{dashboard_stats_handler, sales_data_handler, top_products_handler},
    handlers::{
        create_order_handler, delete_order_handler, get_order_handler,
        list_orders_handler, update_items_handler, update_status_handler,
    },
    models::AppState,
};

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(state.frontend_url.clone())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION]);

    Router::new()
        .route("/api/pedidos", post(create_order_handler).get(list_orders_handler))
        .route(
            "/api/pedidos/{id}",
            get(get_order_handler).delete(delete_order_handler),
        )
        .route("/api/pedidos/{id}/status", patch(update_status_handler))
        .route("/api/pedidos/{id}/items", patch(update_items_handler))
        .route("/api/admin/dashboard/stats", get(dashboard_stats_handler))
        .route("/api/admin/dashboard/sales", get(sales_data_handler))
        .route("/api/admin/dashboard/top-products", get(top_products_handler))
        .layer(cors)
        .with_state(state)
}
