use chrono::Local;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Processando,
    Confirmado,
    Enviado,
    Entregue,
    Cancelado,
    Rejeitado,
}

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct Order {
    id: i64,
    customer_id: i64,
    stat: Status,
    created_at: chrono::DateTime<Local>,
    updated_at: chrono::DateTime<Local>,
}
#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct OrderItem {
    id: i64,
    id_order: i64,
    id_product: i32,
    quantity: i32,
    unit_price: Decimal,
    created_at: chrono::DateTime<Local>,
}

#[derive(Serialize)]
pub struct CompleteOrder {
    order: Order,
    items: Vec<OrderItem>,
    total: Decimal,
}
