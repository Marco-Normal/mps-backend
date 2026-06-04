use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use tracing::{error, info, warn};

use crate::{
    models::{AppState, Product},
    normalization::normalize_string,
    schema::{ProductSchema, UpdateProductSchema},
};
#[tracing::instrument]
pub async fn create_product_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ProductSchema>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    info!(new_record = ?body, "Inserting into database");
    let nome_norm = normalize_string(&body.nome);
    let marca_norm = normalize_string(&body.marca);
    let product = sqlx::query_as!(
        Product,
        r#"INSERT INTO produtos (nome,nome_norm, marca, marca_norm,num_fab, unidade, valor)
        VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id,nome, marca, num_fab, unidade, valor"#,
        &body.nome,
        nome_norm,
        &body.marca,
        marca_norm,
        body.num_fab,
        &body.unidade,
        &body.valor
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string());
    if let Err(err) = product {
        error!("Duplicated key value");
        if err.to_string().contains("duplicate key value") {
            let err_response = json!({
                "status": "error",
                "message": "Product already exists",
            });
            return Err((StatusCode::CONFLICT, Json(err_response)));
        }
        error!("Error inserting into database");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "error", "message": format!("{:?}", err)})),
        ));
    }
    info!("Product inserted with sucess");
    let product_response = json!({
        "status": "sucess",
        "data": json!({"product": product})
    });
    Ok(Json(product_response))
}

#[tracing::instrument]
pub async fn get_product_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    info!(id, "Querying by id");
    let product_result = sqlx::query_as!(
        Product,
        r#"SELECT id,nome,marca,num_fab,unidade,valor FROM produtos WHERE id = $1"#,
        id
    )
    .fetch_one(&state.db)
    .await;
    match product_result {
        Ok(product) => {
            info!("Product found");
            let response = serde_json::json!({
                "status": "ok",
                "data": serde_json::json!({
                    "product": product
                })
            });
            Ok(Json(response))
        }
        Err(sqlx::Error::RowNotFound) => {
            error!("Product with said ID doens't exists");
            let error_response = serde_json::json!({
                "status": "fail",
                "message": format!("Product with ID: {id} not found")
            });
            Err((StatusCode::NOT_FOUND, Json(error_response)))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"status": "error", "message": format!("{e}")})),
        )),
    }
}
#[tracing::instrument]
pub async fn delete_product_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    warn!(id, "Deleting product by ID");
    let product: Product = sqlx::query_as!(
        Product,
        r#"DELETE FROM produtos WHERE id = $1 RETURNING id, nome,marca,num_fab,unidade,valor"#,
        id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => {
            error!("Id not found");
            let error_response = serde_json::json!({
                "status": "fail",
                "message": format!("Product with ID: {id} not found")
            });
            (StatusCode::NOT_FOUND, Json(error_response))
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"status": "error", "message": format!("{e}")})),
        ),
    })?;

    let response = serde_json::json!({
        "status" : "sucess",
        "message": "Product deleted with sucess",
        "data": {
                "deleted_product": product
        }
    });
    info!("Deletion complete");
    Ok(Json(response))
}
#[tracing::instrument]
pub async fn update_product_by_id(
    Path(id): Path<i32>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateProductSchema>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    warn!(id, "Updating product by id");
    let query_result = sqlx::query_as!(
        Product,
        r#"SELECT id, nome,marca,num_fab,unidade,valor FROM produtos WHERE id = $1"#,
        &id
    )
    .fetch_one(&state.db)
    .await;

    let product = match query_result {
        Ok(product) => product,
        Err(sqlx::Error::RowNotFound) => {
            error!("Product not found");
            let error_response = serde_json::json!({
                "status": "error",
                "message": format!("Product with ID: {} not found", id)
            });
            return Err((StatusCode::NOT_FOUND, Json(error_response)));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "status": "error",
                    "message": format!("{:?}",e)
                })),
            ));
        }
    };

    let novo_nome = body.nome.unwrap_or(product.nome);
    let n_nome_normalizado = normalize_string(&novo_nome);
    let nova_marca = body.marca.unwrap_or(product.marca);
    let n_marca_normalizada = normalize_string(&nova_marca);
    let nova_unidade = body.unidade.unwrap_or(product.unidade);
    let novo_valor = body.valor.unwrap_or(product.valor);

    let updated_product = sqlx::query_as!(
        Product,
        r#"UPDATE produtos SET nome = $1, marca = $2, num_fab = $3, unidade = $4, valor = $5, nome_norm = $6, marca_norm = $7 WHERE id = $8
        RETURNING id, nome, marca, num_fab, unidade, valor"#,
        &novo_nome,
        &nova_marca,
        body.num_fab,
        &nova_unidade,
        novo_valor,
        n_nome_normalizado,
        n_marca_normalizada,
        id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "status": "error",
                "message": format!("{:?}", e)
            })),
        )
    })?;

    let response = json!({
        "status": "success",
        "data": json!({
            "product": updated_product
        })
    });
    Ok(Json(response))
}
