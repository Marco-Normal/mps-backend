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
    #[serde(default)]
    pub descricao: Option<String>,
    #[serde(default)]
    pub estoque: i32,
}

#[derive(Deserialize, Debug, Serialize, sqlx::FromRow)]
pub struct ProductImage {
    pub id: i64,
    pub id_produto: i32,
    pub path: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug)]
pub struct AppState {
    pub db: PgPool,
    pub static_dir: std::path::PathBuf,
    pub frontend_url: axum::http::HeaderValue,
}
