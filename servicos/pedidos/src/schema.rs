use serde::Deserialize;
use uuid::Uuid;

use crate::models::Status;

/// Single item in a new order
#[derive(Deserialize, Debug)]
pub struct OrderItemSchema {
    pub id_product: i32,
    pub quantity: i32,
}

/// Body for POST /pedidos
#[derive(Deserialize, Debug)]
pub struct CreateOrderSchema {
    pub items: Vec<OrderItemSchema>,
}

/// Body for PATCH /pedidos/:id/status
#[derive(Deserialize, Debug)]
pub struct UpdateStatusSchema {
    pub status: Status,
}

/// Query params for GET /pedidos
#[derive(Deserialize, Debug)]
pub struct OrderListQuery {
    pub customer_id: Option<Uuid>,
    pub status: Option<Status>,
    pub limit: Option<i64>,
}

/// Item to add in PATCH /pedidos/:id/items
#[derive(Deserialize, Debug, Clone)]
pub struct AddItemSchema {
    pub id_product: i32,
    pub quantity: i32,
}

/// Item to update quantity (by items_pedidos.id)
#[derive(Deserialize, Debug)]
pub struct UpdateItemSchema {
    pub id: i64,
    pub quantity: i32,
}

/// Body for PATCH /pedidos/:id/items
#[derive(Deserialize, Debug)]
pub struct UpdateOrderItemsSchema {
    pub add: Option<Vec<AddItemSchema>>,
    pub update: Option<Vec<UpdateItemSchema>>,
    pub remove: Option<Vec<i64>>,
}
