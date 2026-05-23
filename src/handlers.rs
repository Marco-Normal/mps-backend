use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::{
    models::{AppState, Product},
    schema::ProductSchema,
};

pub async fn create_product_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ProductSchema>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let product = sqlx::query_as!(Product,
        r#"INSERT INTO produtos (nome, marca, num_fab, unidade, valor) VALUES ($1, $2, $3, $4, $5) RETURNING *"#,
        &body.nome,
        &body.marca,
        body.num_fab,
        &body.unidade,
        &body.valor
    ).fetch_one(&state.db).await.map_err(|e| e.to_string());
    if let Err(err) = product {
        if err.to_string().contains("duplicate key value") {
            let err_response = json!({
                "status": "error",
                "message": "Product already exists",
            });
            return Err((StatusCode::CONFLICT, Json(err_response)));
        }
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "error", "message": format!("{:?}", err)})),
        ));
    }
    let product_response = json!({
        "status": "sucess",
        "data": json!({"product": product})
    });
    Ok(Json(product_response))
}
