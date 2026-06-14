use std::sync::Arc;

use axum::{
    Router,
    routing::{get, patch, post},
};

use crate::{
    handlers::{
        create_order_handler, delete_order_handler, get_order_handler,
        list_orders_handler, update_items_handler, update_status_handler,
    },
    models::AppState,
};

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/pedidos", post(create_order_handler).get(list_orders_handler))
        .route(
            "/api/pedidos/{id}",
            get(get_order_handler).delete(delete_order_handler),
        )
        .route("/api/pedidos/{id}/status", patch(update_status_handler))
        .route("/api/pedidos/{id}/items", patch(update_items_handler))
        .with_state(state)
}
