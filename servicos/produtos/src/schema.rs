use rust_decimal::Decimal;
use serde::Deserialize;
#[derive(Deserialize, Debug)]
pub struct ProductSchema {
    pub nome: String,
    pub marca: String,
    pub num_fab: Option<String>,
    pub unidade: String,
    pub valor: Decimal,
}
#[derive(Deserialize, Debug)]
pub struct UpdateProductSchema {
    pub nome: Option<String>,
    pub marca: Option<String>,
    pub num_fab: Option<String>,
    pub unidade: Option<String>,
    pub valor: Option<Decimal>,
}

#[derive(Deserialize, Debug)]
pub struct ProductSearchSchema {
    pub q: String,
    pub limit: Option<i64>,
}
