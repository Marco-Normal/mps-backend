use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use common::api_response::ApiResponse;
use errors::errors::AppError;
use tracing::info;

use crate::{
    auth::JwtCustomer,
    models::AppState,
    schema::{CreateOrderSchema, OrderListQuery, UpdateOrderItemsSchema, UpdateStatusSchema},
    service,
};

pub async fn create_order_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Json(body): Json<CreateOrderSchema>,
) -> Result<impl IntoResponse, AppError> {
    info!(%customer_id, "Creating order");
    let order = service::create_order(Arc::clone(&state), customer_id, body).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "order": order }))))
}

pub async fn list_orders_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Query(query): Query<OrderListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let orders = service::list_orders(&state.db, customer_id, &query).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "orders": orders }))))
}

pub async fn get_order_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    info!(id, "Fetching order");
    let order = service::get_order(&state.db, id, customer_id).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "order": order }))))
}

pub async fn update_status_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Path(id): Path<i64>,
    Json(body): Json<UpdateStatusSchema>,
) -> Result<impl IntoResponse, AppError> {
    info!(id, "Updating order status");
    let order = service::update_status(&state, id, customer_id, body).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "order": order }))))
}

pub async fn update_items_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Path(id): Path<i64>,
    Json(body): Json<UpdateOrderItemsSchema>,
) -> Result<impl IntoResponse, AppError> {
    info!(id, "Updating order items");
    let order = service::update_items(&state, id, customer_id, body).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "order": order }))))
}

pub async fn delete_order_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    info!(id, "Deleting order");
    let order = service::delete_order(&state.db, id, customer_id).await?;
    Ok(Json(ApiResponse::success(serde_json::json!({ "order": order }))))
}
