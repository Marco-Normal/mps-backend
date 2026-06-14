use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "order_status", rename_all = "lowercase")]
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
    pub id: i64,
    pub customer_id: Uuid,
    pub stat: Status,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct OrderItem {
    pub id: i64,
    pub id_order: i64,
    pub id_product: i32,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct CompleteOrder {
    pub order: Order,
    pub items: Vec<OrderItem>,
    pub total: Decimal,
}

#[derive(Debug)]
pub struct AppState {
    pub db: PgPool,
    pub http: reqwest::Client,
    pub produtos_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serializes_lowercase() {
        let s = serde_json::to_string(&Status::Confirmado).unwrap();
        assert_eq!(s, "\"confirmado\"");
    }

    #[test]
    fn status_deserializes_from_lowercase() {
        let s: Status = serde_json::from_str("\"enviado\"").unwrap();
        assert!(matches!(s, Status::Enviado));
    }
}
