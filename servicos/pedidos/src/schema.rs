use rust_decimal::Decimal;
use serde::Deserialize;

use crate::models::Status;

#[derive(Deserialize, Debug)]
pub struct OrderItemSchema {
    id: i32,
    quantity: i32,
}

#[derive(Deserialize, Debug)]
pub struct OrderSchema {
    items: Vec<OrderItemSchema>,
}

#[derive(Deserialize, Debug)]
pub struct UpdateStatus {
    status: Status,
}

#[derive(Deserialize, Debug)]
pub struct OrderListQuery {
    customer: Option<i64>,
    status: Option<Status>,
    limit: Option<i64>,
}
#[derive(Deserialize, Debug)]
pub struct OrderItemUpdateSchema {
    quantity: Option<i32>,
    value: Option<Decimal>,
}
