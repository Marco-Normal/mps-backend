use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Deserialize, Debug, Serialize, sqlx::FromRow)]
pub struct Product {
    #[serde(rename = "Idproduto")]
    pub id: i32,
    #[serde(rename = "Descricao")]
    pub nome: String,
    #[serde(rename = "Marca")]
    pub marca: String,
    #[serde(rename = "Num_fab")]
    pub num_fab: Option<String>,
    #[serde(rename = "idunidade")]
    pub unidade: String,
    #[serde(rename = "VLR_VENDA1")]
    pub valor: Decimal,
}

#[derive(Debug)]
pub struct AppState {
    pub db: PgPool,
}
