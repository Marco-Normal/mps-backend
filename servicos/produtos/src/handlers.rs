use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use common::api_response::ApiResponse;
use errors::errors::AppError;

use tracing::{info, warn};

use crate::{
    models::{AppState, Product},
    normalization::normalize_string,
    schema::{ProductSchema, ProductSearchSchema, UpdateProductSchema},
};

#[tracing::instrument(skip(state), fields(nome = %body.nome))]
pub async fn create_product_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ProductSchema>,
) -> Result<impl IntoResponse, AppError> {
    info!(new_record = ?body, "Inserting into database");
    let nome_norm = normalize_string(&body.nome);
    let marca_norm = normalize_string(&body.marca);
    let estoque = body.estoque.unwrap_or(0);
    let product = sqlx::query_as!(
        Product,
        r#"INSERT INTO produtos (nome, nome_norm, marca, marca_norm, num_fab, unidade, valor, descricao, estoque)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, nome, marca, num_fab, unidade, valor, descricao, estoque"#,
        &body.nome,
        nome_norm,
        &body.marca,
        marca_norm,
        body.num_fab,
        &body.unidade,
        &body.valor,
        body.descricao,
        estoque,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key value") {
            AppError::Conflict("Product".to_string())
        } else {
            AppError::DbError(e)
        }
    })?;

    info!("Product inserted with success");
    Ok(Json(ApiResponse::ok(serde_json::json!({"product": product}))))
}

#[tracing::instrument(skip(state), fields(id = %id))]
pub async fn get_product_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    info!(id, "Querying by id");
    let product_result = sqlx::query_as!(
        Product,
        r#"SELECT id, nome, marca, num_fab, unidade, valor, descricao, estoque
        FROM produtos WHERE id = $1"#,
        id
    )
    .fetch_one(&state.db)
    .await;
    match product_result {
        Ok(product) => {
            info!("Product found");
            Ok(Json(ApiResponse::ok(serde_json::json!({ "product": product }))))
        }
        Err(sqlx::Error::RowNotFound) => Err(AppError::NotFound {
            service: "Product".to_string(),
            id: id.to_string(),
        }),
        Err(e) => Err(AppError::DbError(e)),
    }
}

#[tracing::instrument(skip(state), fields(id = %id))]
pub async fn delete_product_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    warn!(id, "Deleting product by ID");
    let product = sqlx::query_as!(
        Product,
        r#"DELETE FROM produtos WHERE id = $1
        RETURNING id, nome, marca, num_fab, unidade, valor, descricao, estoque"#,
        id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Product".to_string(),
            id: id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    info!("Deletion complete");
    Ok(Json(ApiResponse::success(serde_json::json!({ "product": product }))))
}

#[tracing::instrument(skip(state), fields(id = %id))]
pub async fn update_product_by_id(
    Path(id): Path<i32>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateProductSchema>,
) -> Result<impl IntoResponse, AppError> {
    warn!(id, "Updating product by id");
    let product = sqlx::query_as!(
        Product,
        r#"SELECT id, nome, marca, num_fab, unidade, valor, descricao, estoque
        FROM produtos WHERE id = $1"#,
        &id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Product".to_string(),
            id: id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    let novo_nome = body.nome.unwrap_or(product.nome);
    let n_nome_normalizado = normalize_string(&novo_nome);
    let nova_marca = body.marca.unwrap_or(product.marca);
    let n_marca_normalizada = normalize_string(&nova_marca);
    let nova_unidade = body.unidade.unwrap_or(product.unidade);
    let novo_valor = body.valor.unwrap_or(product.valor);
    // Partial-update semantics: omitting descricao preserves the existing value.
    // Sending null descricao is treated the same as omitting it (cannot clear once set).
    let nova_descricao = body.descricao.or(product.descricao);
    let novo_estoque = body.estoque.unwrap_or(product.estoque);
    let novo_num_fab = body.num_fab.or(product.num_fab);

    let updated = sqlx::query_as!(
        Product,
        r#"UPDATE produtos
        SET nome=$1, marca=$2, num_fab=$3, unidade=$4, valor=$5,
            nome_norm=$6, marca_norm=$7, descricao=$8, estoque=$9
        WHERE id=$10
        RETURNING id, nome, marca, num_fab, unidade, valor, descricao, estoque"#,
        &novo_nome,
        &nova_marca,
        novo_num_fab,
        &nova_unidade,
        novo_valor,
        n_nome_normalizado,
        n_marca_normalizada,
        nova_descricao,
        novo_estoque,
        id
    )
    .fetch_one(&state.db)
    .await
    .map_err(AppError::DbError)?;

    Ok(Json(ApiResponse::success(serde_json::json!({ "product": updated }))))
}

#[tracing::instrument(fields(query = %params.q), skip(state))]
pub async fn get_products_by_query(
    state: State<Arc<AppState>>,
    Query(params): Query<ProductSearchSchema>,
) -> Result<impl IntoResponse, AppError> {
    let normalized_q = normalize_string(&params.q);
    let products: Vec<Product> = if let Some(limit) = params.limit {
        sqlx::query_as!(
            Product,
            r#"SELECT id, nome, marca, num_fab, unidade, valor, descricao, estoque
            FROM produtos
            WHERE nome_norm % $1 OR marca_norm % $1
            ORDER BY GREATEST(similarity(nome_norm, $1), similarity(marca_norm, $1)) DESC
            LIMIT $2
            "#,
            normalized_q,
            limit
        )
        .fetch_all(&state.db)
        .await
        .map_err(AppError::DbError)?
    } else {
        sqlx::query_as!(
            Product,
            r#"SELECT id, nome, marca, num_fab, unidade, valor, descricao, estoque
            FROM produtos
            WHERE nome_norm % $1 OR marca_norm % $1
            ORDER BY GREATEST(similarity(nome_norm, $1), similarity(marca_norm, $1)) DESC
            "#,
            normalized_q,
        )
        .fetch_all(&state.db)
        .await
        .map_err(AppError::DbError)?
    };
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "products": products
    }))))
}
