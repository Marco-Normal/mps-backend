# Pedidos Microservice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the `pedidos` microservice with full CRUD, JWT auth, concurrent product validation via HTTP, and a status state machine; extend `produtos` with stock, description, and image support.

**Architecture:** Service-layer pattern — thin Axum handlers delegate to a `service.rs` that owns business logic (state machine, item rules, validation orchestration). A dedicated `produto_client.rs` fires concurrent HTTP calls to the `produtos` service to validate product existence and stock before persisting an order. JWT middleware extracts `customer_id: Uuid` from every request.

**Tech Stack:** Rust, Axum 0.8, SQLx 0.8 (PostgreSQL), `reqwest`, `futures`, `jsonwebtoken`, `uuid`, `tokio`, `serde`

---

## File Map

### Modified files
| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `uuid`, `reqwest`, `futures`, `jsonwebtoken` to workspace deps |
| `libs/errors/src/errors.rs` | `NotFound.id: i32` → `String`; add `Unauthorized`, `UnprocessableEntity`, `ValidationFailed` |
| `servicos/produtos/src/models.rs` | Add `descricao`, `estoque` to `Product`; add `ProductImage` struct |
| `servicos/produtos/src/schema.rs` | Add `descricao`, `estoque` to `ProductSchema` / `UpdateProductSchema` |
| `servicos/produtos/src/handlers.rs` | Update all handlers for new fields; add image handlers |
| `servicos/produtos/src/router.rs` | Add image routes |
| `servicos/produtos/Cargo.toml` | Add `uuid` dep |
| `servicos/pedidos/Cargo.toml` | Add `uuid`, `reqwest`, `futures`, `jsonwebtoken`, `dotenvy`, `serde_json`, `tracing`, `tracing-subscriber`, `tracing-appender` |
| `servicos/pedidos/src/models.rs` | Fix `customer_id` to `Uuid`, fix `i64` types, add proper derives |
| `servicos/pedidos/src/schema.rs` | Fix `OrderListQuery`; add `UpdateOrderItemsSchema` |
| `servicos/pedidos/src/lib.rs` | Add all new module declarations |
| `servicos/pedidos/src/main.rs` | Full server bootstrap with `AppState` |

### Created files
| File | Purpose |
|------|---------|
| `servicos/produtos/migrations/TIMESTAMP_add_descricao_estoque.up.sql` | Add `descricao`, `estoque` columns |
| `servicos/produtos/migrations/TIMESTAMP_add_descricao_estoque.down.sql` | Reverse above |
| `servicos/produtos/migrations/TIMESTAMP_add_imagens_produto.up.sql` | Create `imagens_produto` table |
| `servicos/produtos/migrations/TIMESTAMP_add_imagens_produto.down.sql` | Drop `imagens_produto` |
| `servicos/pedidos/migrations/` | All new correct migrations (old broken ones deleted) |
| `servicos/pedidos/src/router.rs` | Route definitions |
| `servicos/pedidos/src/handlers.rs` | Thin HTTP handlers |
| `servicos/pedidos/src/service.rs` | Business logic, state machine, item rules |
| `servicos/pedidos/src/produto_client.rs` | Concurrent HTTP validation client |

---

## Task 1: Update Workspace Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add workspace dependencies**

Open `Cargo.toml` at the workspace root and add the following to `[workspace.dependencies]`:

```toml
uuid = { version = "1.17.0", features = ["v4", "serde"] }
reqwest = { version = "0.12", features = ["json"] }
futures = "0.3"
jsonwebtoken = "9.3"
dotenvy = "0.15.7"
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"
```

- [ ] **Step 2: Verify workspace compiles**

```bash
cargo check --workspace
```

Expected: no errors (new deps not yet used, but they should resolve).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add uuid, reqwest, futures, jsonwebtoken to workspace deps"
```

---

## Task 2: Extend `libs/errors`

**Files:**
- Modify: `libs/errors/src/errors.rs`

The current `AppError::NotFound` uses `id: i32` which breaks for UUID customer IDs and `i64` order IDs. We also need new error variants.

- [ ] **Step 1: Write the unit test for new error variants**

Add a `#[cfg(test)]` block at the bottom of `libs/errors/src/errors.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn not_found_accepts_string_id() {
        let e = AppError::NotFound {
            service: "Order".to_string(),
            id: "42".to_string(),
        };
        assert!(e.to_string().contains("Order"));
        assert!(e.to_string().contains("42"));
    }

    #[test]
    fn unauthorized_returns_401() {
        let e = AppError::Unauthorized;
        let resp = e.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn unprocessable_entity_returns_422() {
        let e = AppError::UnprocessableEntity("bad transition".to_string());
        let resp = e.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test -p errors
```

Expected: compile error — new variants don't exist yet.

- [ ] **Step 3: Rewrite `errors.rs` with new variants**

Replace the full file content:

```rust
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use miette::Diagnostic;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Serialize)]
pub struct ItemValidationError {
    pub id_product: i32,
    pub reason: String,
}

#[derive(Debug, Diagnostic, Error)]
pub enum AppError {
    #[error("{service} with ID {id} not found")]
    NotFound { service: String, id: String },
    #[error("Internal server error: {0}")]
    Internal(String),
    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),
    #[error("{0} already exists")]
    Conflict(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Unprocessable: {0}")]
    UnprocessableEntity(String),
    #[error("Validation failed")]
    ValidationFailed { items: Vec<ItemValidationError> },
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            AppError::NotFound { service, id } => (
                StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({
                    "status": "error",
                    "message": format!("{service} with ID {id} not found"),
                })),
            ),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "status": "error", "message": msg })),
            ),
            AppError::DbError(e) => {
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "status": "error",
                        "message": "Internal server error",
                    })),
                )
            }
            AppError::Conflict(service) => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("{service} already exists."),
                })),
            ),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Unauthorized",
                })),
            ),
            AppError::UnprocessableEntity(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "status": "error",
                    "message": msg,
                })),
            ),
            AppError::ValidationFailed { items } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Product validation failed",
                    "items": items,
                })),
            ),
        };

        (status, body).into_response()
    }
}
```

Also add `serde_json` and `serde` to `libs/errors/Cargo.toml`:

```toml
[dependencies]
axum.workspace = true
miette = { version = "7.6.0", features = ["fancy"] }
thiserror = "2.0.18"
sqlx.workspace = true
tracing = "0.1"
serde = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 4: Fix `produtos` call sites for `NotFound`**

In `servicos/produtos/src/handlers.rs`, every `AppError::NotFound { service, id }` passes `id` as `i32`. Change all occurrences to pass `id.to_string()`:

```rust
// get_product_by_id
Err(AppError::NotFound {
    service: String::from("Product"),
    id: id.to_string(),
})

// delete_product_by_id
AppError::NotFound {
    service: String::from("Product"),
    id: id.to_string(),
}

// update_product_by_id
return Err(AppError::NotFound {
    service: String::from("Product"),
    id: id.to_string(),
});
```

- [ ] **Step 5: Run tests and verify they pass**

```bash
cargo test -p errors
cargo check --workspace
```

Expected: all tests pass, workspace compiles.

- [ ] **Step 6: Commit**

```bash
git add libs/errors/ servicos/produtos/src/handlers.rs
git commit -m "feat(errors): generalize NotFound id to String, add Unauthorized/UnprocessableEntity/ValidationFailed"
```

---

## Task 3: `produtos` — Database Migrations

**Files:**
- Create: `servicos/produtos/migrations/TIMESTAMP_add_descricao_estoque.{up,down}.sql`
- Create: `servicos/produtos/migrations/TIMESTAMP_add_imagens_produto.{up,down}.sql`

Run these from the `servicos/produtos/` directory. `DATABASE_URL` must be set in `.env`.

- [ ] **Step 1: Create migration for new columns**

```bash
cd servicos/produtos
sqlx migrate add add_descricao_estoque
```

Edit the generated `up` file:

```sql
ALTER TABLE produtos
  ADD COLUMN descricao TEXT,
  ADD COLUMN estoque INTEGER NOT NULL DEFAULT 0 CHECK (estoque >= 0);
```

Edit the generated `down` file:

```sql
ALTER TABLE produtos
  DROP COLUMN IF EXISTS descricao,
  DROP COLUMN IF EXISTS estoque;
```

- [ ] **Step 2: Create migration for images table**

```bash
sqlx migrate add add_imagens_produto
```

Edit the generated `up` file:

```sql
CREATE TABLE IF NOT EXISTS imagens_produto (
  id         BIGSERIAL PRIMARY KEY,
  id_produto INTEGER NOT NULL REFERENCES produtos(id) ON DELETE CASCADE,
  path       TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_imagens_produto_produto ON imagens_produto(id_produto);
```

Edit the generated `down` file:

```sql
DROP TABLE IF EXISTS imagens_produto;
```

- [ ] **Step 3: Run migrations**

```bash
sqlx migrate run
```

Expected: `Applied N migrations.` with no errors.

- [ ] **Step 4: Commit**

```bash
cd ../..
git add servicos/produtos/migrations/
git commit -m "feat(produtos): add descricao, estoque columns and imagens_produto table"
```

---

## Task 4: `produtos` — Update Models and Schemas

**Files:**
- Modify: `servicos/produtos/src/models.rs`
- Modify: `servicos/produtos/src/schema.rs`
- Modify: `servicos/produtos/Cargo.toml`

- [ ] **Step 1: Add `uuid` dependency to produtos**

In `servicos/produtos/Cargo.toml`, add:

```toml
uuid = { workspace = true }
```

- [ ] **Step 2: Update `models.rs`**

Replace the full file:

```rust
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
    pub descricao: Option<String>,
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
    pub static_dir: String,
}
```

- [ ] **Step 3: Update `schema.rs`**

Replace the full file:

```rust
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ProductSchema {
    pub nome: String,
    pub marca: String,
    pub num_fab: Option<String>,
    pub unidade: String,
    pub valor: Decimal,
    pub descricao: Option<String>,
    pub estoque: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct UpdateProductSchema {
    pub nome: Option<String>,
    pub marca: Option<String>,
    pub num_fab: Option<String>,
    pub unidade: Option<String>,
    pub valor: Option<Decimal>,
    pub descricao: Option<String>,
    pub estoque: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct ProductSearchSchema {
    pub q: String,
    pub limit: Option<i64>,
}
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p produtos
```

Expected: may show handler errors because `AppState` changed — fix in next task.

- [ ] **Step 5: Commit**

```bash
git add servicos/produtos/src/models.rs servicos/produtos/src/schema.rs servicos/produtos/Cargo.toml
git commit -m "feat(produtos): add descricao, estoque fields and ProductImage model"
```

---

## Task 5: `produtos` — Update Existing Handlers

**Files:**
- Modify: `servicos/produtos/src/handlers.rs`
- Modify: `servicos/produtos/src/main.rs`

`AppState` now has `static_dir: String`, and `Product` has new fields. All queries must be updated.

- [ ] **Step 1: Update `create_product_handler` query**

In `handlers.rs`, update the INSERT query to include `descricao` and `estoque`:

```rust
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
```

- [ ] **Step 2: Update `get_product_by_id` query**

```rust
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
```

- [ ] **Step 3: Update `delete_product_by_id` query**

```rust
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
```

- [ ] **Step 4: Update `update_product_by_id` query**

```rust
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
    let nova_descricao = body.descricao.or(product.descricao);
    let novo_estoque = body.estoque.unwrap_or(product.estoque);

    let updated = sqlx::query_as!(
        Product,
        r#"UPDATE produtos
        SET nome=$1, marca=$2, num_fab=$3, unidade=$4, valor=$5,
            nome_norm=$6, marca_norm=$7, descricao=$8, estoque=$9
        WHERE id=$10
        RETURNING id, nome, marca, num_fab, unidade, valor, descricao, estoque"#,
        &novo_nome,
        &nova_marca,
        body.num_fab,
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
```

- [ ] **Step 5: Update `get_products_by_query` SELECT columns**

Update both SELECT queries to include `descricao, estoque`:

```rust
r#"SELECT id, nome, marca, num_fab, unidade, valor, descricao, estoque
FROM produtos
WHERE nome_norm % $1 OR marca_norm % $1
ORDER BY GREATEST(similarity(nome_norm, $1), similarity(marca_norm, $1)) DESC
LIMIT $2"#
```

(Same change for the no-limit variant.)

- [ ] **Step 6: Update `main.rs` to pass `static_dir` to `AppState`**

```rust
let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "./static".to_string());
std::fs::create_dir_all(&static_dir).expect("Failed to create static dir");
let app = create_router(Arc::new(AppState { db: pool.clone(), static_dir }));
```

- [ ] **Step 7: Compile and verify**

```bash
cargo check -p produtos
```

Expected: compiles cleanly (image handlers not yet added, route still points to old handler set — that's fine).

- [ ] **Step 8: Commit**

```bash
git add servicos/produtos/src/ 
git commit -m "feat(produtos): update all handlers for descricao/estoque fields"
```

---

## Task 6: `produtos` — Image Upload Handlers and Routes

**Files:**
- Modify: `servicos/produtos/src/handlers.rs` (add three new handlers)
- Modify: `servicos/produtos/src/router.rs` (add image routes)

- [ ] **Step 1: Add image upload handler**

Add the following at the bottom of `handlers.rs`. Add these imports at the top:

```rust
use axum::extract::Multipart;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
```

Then add the handlers:

```rust
#[tracing::instrument(skip(state, multipart), fields(id = %id))]
pub async fn upload_product_image(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    // verify product exists
    sqlx::query!("SELECT id FROM produtos WHERE id = $1", id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::NotFound {
                service: "Product".to_string(),
                id: id.to_string(),
            },
            _ => AppError::DbError(e),
        })?;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("No file field in request".to_string()))?;

    let filename = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let data = field
        .bytes()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let file_path = PathBuf::from(&state.static_dir).join(&filename);
    let path_str = file_path.to_string_lossy().to_string();

    let mut file = tokio::fs::File::create(&file_path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    file.write_all(&data)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let image = sqlx::query_as!(
        crate::models::ProductImage,
        r#"INSERT INTO imagens_produto (id_produto, path) VALUES ($1, $2)
        RETURNING id, id_produto, path, created_at"#,
        id,
        path_str,
    )
    .fetch_one(&state.db)
    .await
    .map_err(AppError::DbError)?;

    info!("Image uploaded for product {id}");
    Ok(Json(ApiResponse::ok(serde_json::json!({ "image": image }))))
}

#[tracing::instrument(skip(state), fields(id = %id))]
pub async fn get_product_images(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    let images = sqlx::query_as!(
        crate::models::ProductImage,
        r#"SELECT id, id_produto, path, created_at FROM imagens_produto WHERE id_produto = $1"#,
        id
    )
    .fetch_all(&state.db)
    .await
    .map_err(AppError::DbError)?;

    Ok(Json(ApiResponse::ok(serde_json::json!({ "images": images }))))
}

#[tracing::instrument(skip(state), fields(id = %id, img_id = %img_id))]
pub async fn delete_product_image(
    State(state): State<Arc<AppState>>,
    Path((id, img_id)): Path<(i32, i64)>,
) -> Result<impl IntoResponse, AppError> {
    let image = sqlx::query_as!(
        crate::models::ProductImage,
        r#"DELETE FROM imagens_produto WHERE id = $1 AND id_produto = $2
        RETURNING id, id_produto, path, created_at"#,
        img_id,
        id,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "ProductImage".to_string(),
            id: img_id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    // Delete the file from disk (ignore error if file already gone)
    let _ = tokio::fs::remove_file(&image.path).await;

    info!("Image {img_id} deleted for product {id}");
    Ok(Json(ApiResponse::success(serde_json::json!({ "image": image }))))
}
```

- [ ] **Step 2: Update `router.rs`**

```rust
use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::{
    handlers::{
        create_product_handler, delete_product_by_id, delete_product_image,
        get_product_by_id, get_product_images, get_products_by_query,
        update_product_by_id, upload_product_image,
    },
    models::AppState,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/products", post(create_product_handler))
        .route("/api/products/search", get(get_products_by_query))
        .route(
            "/api/products/{id}",
            get(get_product_by_id)
                .delete(delete_product_by_id)
                .patch(update_product_by_id),
        )
        .route(
            "/api/products/{id}/imagens",
            post(upload_product_image).get(get_product_images),
        )
        .route(
            "/api/products/{id}/imagens/{img_id}",
            delete(delete_product_image),
        )
        .with_state(app_state)
}
```

- [ ] **Step 3: Build and verify**

```bash
cargo build -p produtos
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add servicos/produtos/src/
git commit -m "feat(produtos): add image upload, list, and delete endpoints"
```

---

## Task 7: `pedidos` — Fix Migrations

The existing migrations in `servicos/pedidos/migrations/` have SQL syntax errors and wrong column types. Delete them all and create correct replacements.

- [ ] **Step 1: Delete broken migration files**

```bash
rm servicos/pedidos/migrations/*.sql
```

- [ ] **Step 2: Create the status enum migration**

```bash
cd servicos/pedidos
sqlx migrate add create_status_enum
```

Edit the generated `up` file:

```sql
CREATE TYPE order_status AS ENUM (
  'processando',
  'confirmado',
  'enviado',
  'entregue',
  'cancelado',
  'rejeitado'
);
```

Edit the generated `down` file:

```sql
DROP TYPE IF EXISTS order_status;
```

Note: We rename the enum to `order_status` to avoid collision with any existing `status` type in other services sharing this DB instance.

- [ ] **Step 3: Create the pedidos table migration**

```bash
sqlx migrate add create_pedidos
```

Edit `up`:

```sql
CREATE TABLE IF NOT EXISTS pedidos (
  id          BIGSERIAL PRIMARY KEY,
  customer_id UUID NOT NULL,
  stat        order_status NOT NULL DEFAULT 'processando',
  created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pedidos_customer ON pedidos(customer_id);
CREATE INDEX idx_pedidos_stat     ON pedidos(stat);
```

Edit `down`:

```sql
DROP TABLE IF EXISTS pedidos;
DROP INDEX IF EXISTS idx_pedidos_customer;
DROP INDEX IF EXISTS idx_pedidos_stat;
```

- [ ] **Step 4: Create the items_pedidos table migration**

```bash
sqlx migrate add create_items_pedidos
```

Edit `up`:

```sql
CREATE TABLE IF NOT EXISTS items_pedidos (
  id         BIGSERIAL PRIMARY KEY,
  id_order   BIGINT NOT NULL REFERENCES pedidos(id) ON DELETE CASCADE,
  id_product INTEGER NOT NULL,
  quantity   INTEGER NOT NULL CHECK (quantity > 0),
  unit_price DECIMAL(10,2) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_items_pedidos_order ON items_pedidos(id_order);
```

Edit `down`:

```sql
DROP TABLE IF EXISTS items_pedidos;
DROP INDEX IF EXISTS idx_items_pedidos_order;
```

- [ ] **Step 5: Run migrations**

Ensure `DATABASE_URL` is set in `.env` and points to the pedidos DB (or a shared DB with a separate schema):

```bash
sqlx migrate run
```

Expected: `Applied 3 migrations.`

- [ ] **Step 6: Commit**

```bash
cd ../..
git add servicos/pedidos/migrations/
git commit -m "feat(pedidos): replace broken migrations with correct schema"
```

---

## Task 8: `pedidos` — Cargo.toml Dependencies

**Files:**
- Modify: `servicos/pedidos/Cargo.toml`

- [ ] **Step 1: Update Cargo.toml**

Replace the full file:

```toml
[package]
name = "pedidos"
version.workspace = true
edition.workspace = true

[dependencies]
axum.workspace = true
tokio.workspace = true
sqlx.workspace = true
serde.workspace = true
serde_json.workspace = true
common.workspace = true
errors.workspace = true
uuid.workspace = true
reqwest.workspace = true
futures.workspace = true
jsonwebtoken.workspace = true
dotenvy = "0.15.7"
miette = { version = "7.6.0", features = ["fancy"] }
chrono = { version = "0.4.45", features = ["serde"] }
rust_decimal = { version = "1.42.0", features = ["serde-float"] }
tracing.workspace = true
tracing-subscriber.workspace = true
tracing-appender.workspace = true
```

- [ ] **Step 2: Verify**

```bash
cargo check -p pedidos
```

Expected: compiles (current `main.rs` is a stub, so no errors).

- [ ] **Step 3: Commit**

```bash
git add servicos/pedidos/Cargo.toml Cargo.lock
git commit -m "chore(pedidos): add all required dependencies"
```

---

## Task 9: `pedidos` — Fix Models

**Files:**
- Modify: `servicos/pedidos/src/models.rs`

- [ ] **Step 1: Write unit test for Status display**

Add a test at the bottom of `models.rs` (once the file is updated):

```rust
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
```

- [ ] **Step 2: Rewrite `models.rs`**

```rust
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
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p pedidos
```

Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add servicos/pedidos/src/models.rs
git commit -m "feat(pedidos): fix models with correct types (Uuid, Utc, Status sqlx mapping)"
```

---

## Task 10: `pedidos` — Fix and Extend Schemas

**Files:**
- Modify: `servicos/pedidos/src/schema.rs`

- [ ] **Step 1: Rewrite `schema.rs`**

```rust
use rust_decimal::Decimal;
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
#[derive(Deserialize, Debug)]
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
```

- [ ] **Step 2: Verify**

```bash
cargo check -p pedidos
```

- [ ] **Step 3: Commit**

```bash
git add servicos/pedidos/src/schema.rs
git commit -m "feat(pedidos): rewrite schemas with correct types and item update support"
```

---

## Task 11: `pedidos` — JWT Extractor

**Files:**
- Create: `servicos/pedidos/src/auth.rs`
- Modify: `servicos/pedidos/src/lib.rs`

The extractor reads `Authorization: Bearer <token>`, validates with `JWT_SECRET` from env, and extracts `customer_id: Uuid`.

- [ ] **Step 1: Write unit test for claims parsing**

Create `servicos/pedidos/src/auth.rs` with the test block first:

```rust
use axum::{
    RequestPartsExt,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use axum_extra::{TypedHeader, headers::{Authorization, authorization::Bearer}};
use errors::errors::AppError;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub customer_id: Uuid,
    pub exp: usize,
}

pub struct JwtCustomer(pub Uuid);

#[axum::async_trait]
impl<S> FromRequestParts<S> for JwtCustomer
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::Unauthorized)?;

        let secret = std::env::var("JWT_SECRET").map_err(|_| AppError::Unauthorized)?;

        let token_data = decode::<Claims>(
            bearer.token(),
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized)?;

        Ok(JwtCustomer(token_data.claims.customer_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_token(customer_id: Uuid, secret: &str) -> String {
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize + 3600;
        let claims = Claims { customer_id, exp };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn valid_token_decodes_customer_id() {
        let id = Uuid::new_v4();
        let secret = "test_secret";
        let token = make_token(id, secret);

        let decoded = jsonwebtoken::decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .unwrap();

        assert_eq!(decoded.claims.customer_id, id);
    }

    #[test]
    fn wrong_secret_fails() {
        let id = Uuid::new_v4();
        let token = make_token(id, "secret_a");

        let result = jsonwebtoken::decode::<Claims>(
            &token,
            &DecodingKey::from_secret(b"secret_b"),
            &Validation::default(),
        );

        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Add `axum-extra` dependency**

Add to `servicos/pedidos/Cargo.toml`:

```toml
axum-extra = { version = "0.10", features = ["typed-header"] }
```

Also add to workspace `Cargo.toml`:

```toml
axum-extra = { version = "0.10", features = ["typed-header"] }
```

- [ ] **Step 3: Register module in `lib.rs`**

```rust
pub mod auth;
pub mod models;
pub mod schema;
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p pedidos auth
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add servicos/pedidos/src/auth.rs servicos/pedidos/src/lib.rs servicos/pedidos/Cargo.toml Cargo.toml Cargo.lock
git commit -m "feat(pedidos): add JWT extractor with customer_id claims"
```

---

## Task 12: `pedidos` — `produto_client.rs`

**Files:**
- Create: `servicos/pedidos/src/produto_client.rs`
- Modify: `servicos/pedidos/src/lib.rs`

Fires one HTTP GET per item concurrently, collects all validation failures.

- [ ] **Step 1: Write unit tests for validation logic**

Create `servicos/pedidos/src/produto_client.rs`:

```rust
use errors::errors::{AppError, ItemValidationError};
use futures::future::join_all;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::schema::AddItemSchema;

/// Successful validation result for one item
pub struct ValidatedItem {
    pub id_product: i32,
    pub quantity: i32,
    pub unit_price: Decimal,
}

#[derive(Deserialize)]
struct ProductResponse {
    data: ProductData,
}

#[derive(Deserialize)]
struct ProductData {
    product: ProdutoDto,
}

#[derive(Deserialize)]
struct ProdutoDto {
    #[serde(rename = "VLR_VENDA1")]
    valor: Decimal,
    estoque: i32,
}

/// Validates a list of items against the produtos service.
/// Returns Ok(Vec<ValidatedItem>) if all pass, or Err(AppError::ValidationFailed) listing all failures.
pub async fn validate_items(
    client: &Client,
    produtos_url: &str,
    items: &[AddItemSchema],
) -> Result<Vec<ValidatedItem>, AppError> {
    let tasks: Vec<_> = items
        .iter()
        .map(|item| {
            let url = format!("{produtos_url}/api/products/{}", item.id_product);
            let client = client.clone();
            let id_product = item.id_product;
            let quantity = item.quantity;
            async move {
                let resp = client.get(&url).send().await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        match r.json::<ProductResponse>().await {
                            Ok(body) => {
                                let p = body.data.product;
                                if quantity > p.estoque {
                                    Err(ItemValidationError {
                                        id_product,
                                        reason: format!(
                                            "insufficient stock (requested {quantity}, available {})",
                                            p.estoque
                                        ),
                                    })
                                } else {
                                    Ok(ValidatedItem {
                                        id_product,
                                        quantity,
                                        unit_price: p.valor,
                                    })
                                }
                            }
                            Err(_) => Err(ItemValidationError {
                                id_product,
                                reason: "failed to parse product response".to_string(),
                            }),
                        }
                    }
                    Ok(r) if r.status() == 404 => Err(ItemValidationError {
                        id_product,
                        reason: "product not found".to_string(),
                    }),
                    _ => Err(ItemValidationError {
                        id_product,
                        reason: "produtos service unavailable".to_string(),
                    }),
                }
            }
        })
        .collect();

    let results: Vec<Result<ValidatedItem, ItemValidationError>> = join_all(tasks).await;

    let mut errors = Vec::new();
    let mut validated = Vec::new();

    for r in results {
        match r {
            Ok(v) => validated.push(v),
            Err(e) => errors.push(e),
        }
    }

    if errors.is_empty() {
        Ok(validated)
    } else {
        Err(AppError::ValidationFailed { items: errors })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::AddItemSchema;

    #[test]
    fn insufficient_stock_message_format() {
        let e = ItemValidationError {
            id_product: 5,
            reason: format!("insufficient stock (requested {}, available {})", 10, 3),
        };
        assert!(e.reason.contains("requested 10"));
        assert!(e.reason.contains("available 3"));
    }
}
```

- [ ] **Step 2: Register module in `lib.rs`**

```rust
pub mod auth;
pub mod models;
pub mod produto_client;
pub mod schema;
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p pedidos produto_client
```

Expected: 1 test passes.

- [ ] **Step 4: Commit**

```bash
git add servicos/pedidos/src/produto_client.rs servicos/pedidos/src/lib.rs
git commit -m "feat(pedidos): add concurrent product validation client"
```

---

## Task 13: `pedidos` — Service Layer

**Files:**
- Create: `servicos/pedidos/src/service.rs`
- Modify: `servicos/pedidos/src/lib.rs`

This is the core of the service — state machine enforcement, DB queries, item mutation rules.

- [ ] **Step 1: Write unit tests for state machine**

Create `servicos/pedidos/src/service.rs` starting with tests:

```rust
use errors::errors::AppError;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    models::{AppState, CompleteOrder, Order, OrderItem, Status},
    produto_client::{ValidatedItem, validate_items},
    schema::{AddItemSchema, CreateOrderSchema, OrderListQuery, UpdateOrderItemsSchema, UpdateStatusSchema},
};

/// Returns true if the transition from `current` to `next` is valid.
pub fn is_valid_transition(current: &Status, next: &Status) -> bool {
    matches!(
        (current, next),
        (Status::Processando, Status::Confirmado)
        | (Status::Processando, Status::Cancelado)
        | (Status::Confirmado, Status::Enviado)
        | (Status::Confirmado, Status::Rejeitado)
        | (Status::Confirmado, Status::Cancelado)
        | (Status::Enviado, Status::Entregue)
        | (Status::Enviado, Status::Cancelado)
        | (Status::Rejeitado, Status::Cancelado)
    )
}

fn compute_total(items: &[OrderItem]) -> Decimal {
    items.iter().fold(Decimal::ZERO, |acc, item| {
        acc + item.unit_price * Decimal::from(item.quantity)
    })
}

pub async fn get_order(db: &PgPool, order_id: i64) -> Result<CompleteOrder, AppError> {
    let order = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos WHERE id = $1"#,
        order_id
    )
    .fetch_one(db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Order".to_string(),
            id: order_id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    let items = sqlx::query_as!(
        OrderItem,
        r#"SELECT id, id_order, id_product, quantity, unit_price, created_at
        FROM items_pedidos WHERE id_order = $1"#,
        order_id
    )
    .fetch_all(db)
    .await
    .map_err(AppError::DbError)?;

    let total = compute_total(&items);
    Ok(CompleteOrder { order, items, total })
}

pub async fn list_orders(
    db: &PgPool,
    query: &OrderListQuery,
) -> Result<Vec<Order>, AppError> {
    let orders = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos
        WHERE ($1::uuid IS NULL OR customer_id = $1)
          AND ($2::order_status IS NULL OR stat = $2)
        ORDER BY created_at DESC
        LIMIT $3"#,
        query.customer_id as Option<Uuid>,
        query.status as Option<Status>,
        query.limit.unwrap_or(50),
    )
    .fetch_all(db)
    .await
    .map_err(AppError::DbError)?;

    Ok(orders)
}

pub async fn create_order(
    state: &AppState,
    customer_id: Uuid,
    body: CreateOrderSchema,
) -> Result<CompleteOrder, AppError> {
    if body.items.is_empty() {
        return Err(AppError::UnprocessableEntity("Order must have at least one item".to_string()));
    }

    let add_items: Vec<AddItemSchema> = body
        .items
        .iter()
        .map(|i| AddItemSchema { id_product: i.id_product, quantity: i.quantity })
        .collect();

    let validated = validate_items(&state.http, &state.produtos_url, &add_items).await?;

    let mut tx = state.db.begin().await.map_err(AppError::DbError)?;

    let order = sqlx::query_as!(
        Order,
        r#"INSERT INTO pedidos (customer_id) VALUES ($1)
        RETURNING id, customer_id, stat as "stat: Status", created_at, updated_at"#,
        customer_id,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::DbError)?;

    let mut items = Vec::new();
    for v in &validated {
        let item = sqlx::query_as!(
            OrderItem,
            r#"INSERT INTO items_pedidos (id_order, id_product, quantity, unit_price)
            VALUES ($1, $2, $3, $4)
            RETURNING id, id_order, id_product, quantity, unit_price, created_at"#,
            order.id,
            v.id_product,
            v.quantity,
            v.unit_price,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::DbError)?;
        items.push(item);
    }

    tx.commit().await.map_err(AppError::DbError)?;

    let total = compute_total(&items);
    Ok(CompleteOrder { order, items, total })
}

pub async fn update_status(
    db: &PgPool,
    order_id: i64,
    customer_id: Uuid,
    body: UpdateStatusSchema,
) -> Result<Order, AppError> {
    let order = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos WHERE id = $1"#,
        order_id
    )
    .fetch_one(db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Order".to_string(),
            id: order_id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    if order.customer_id != customer_id {
        return Err(AppError::Unauthorized);
    }

    if !is_valid_transition(&order.stat, &body.status) {
        return Err(AppError::UnprocessableEntity(format!(
            "Cannot transition from {:?} to {:?}",
            order.stat, body.status
        )));
    }

    let updated = sqlx::query_as!(
        Order,
        r#"UPDATE pedidos SET stat = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, customer_id, stat as "stat: Status", created_at, updated_at"#,
        body.status as Status,
        order_id,
    )
    .fetch_one(db)
    .await
    .map_err(AppError::DbError)?;

    Ok(updated)
}

pub async fn update_items(
    state: &AppState,
    order_id: i64,
    customer_id: Uuid,
    body: UpdateOrderItemsSchema,
) -> Result<CompleteOrder, AppError> {
    let order = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos WHERE id = $1"#,
        order_id
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Order".to_string(),
            id: order_id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    if order.customer_id != customer_id {
        return Err(AppError::Unauthorized);
    }

    if order.stat != Status::Processando {
        return Err(AppError::UnprocessableEntity(
            "Items can only be modified when order status is 'processando'".to_string(),
        ));
    }

    // Validate new items before opening transaction
    let validated_adds: Vec<ValidatedItem> = if let Some(add) = &body.add {
        validate_items(&state.http, &state.produtos_url, add).await?
    } else {
        vec![]
    };

    let mut tx = state.db.begin().await.map_err(AppError::DbError)?;

    // Remove items
    if let Some(remove_ids) = &body.remove {
        for &item_id in remove_ids {
            sqlx::query!(
                "DELETE FROM items_pedidos WHERE id = $1 AND id_order = $2",
                item_id,
                order_id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::DbError)?;
        }
    }

    // Update quantities
    if let Some(updates) = &body.update {
        for u in updates {
            sqlx::query!(
                "UPDATE items_pedidos SET quantity = $1 WHERE id = $2 AND id_order = $3",
                u.quantity,
                u.id,
                order_id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::DbError)?;
        }
    }

    // Add new items
    for v in &validated_adds {
        sqlx::query!(
            r#"INSERT INTO items_pedidos (id_order, id_product, quantity, unit_price)
            VALUES ($1, $2, $3, $4)"#,
            order_id,
            v.id_product,
            v.quantity,
            v.unit_price,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::DbError)?;
    }

    // Update order updated_at
    sqlx::query!("UPDATE pedidos SET updated_at = NOW() WHERE id = $1", order_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::DbError)?;

    tx.commit().await.map_err(AppError::DbError)?;

    get_order(&state.db, order_id).await
}

pub async fn delete_order(
    db: &PgPool,
    order_id: i64,
    customer_id: Uuid,
) -> Result<Order, AppError> {
    let order = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos WHERE id = $1"#,
        order_id
    )
    .fetch_one(db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound {
            service: "Order".to_string(),
            id: order_id.to_string(),
        },
        _ => AppError::DbError(e),
    })?;

    if order.customer_id != customer_id {
        return Err(AppError::Unauthorized);
    }

    if !matches!(order.stat, Status::Processando | Status::Cancelado) {
        return Err(AppError::UnprocessableEntity(
            "Order can only be deleted when status is 'processando' or 'cancelado'".to_string(),
        ));
    }

    sqlx::query!("DELETE FROM pedidos WHERE id = $1", order_id)
        .execute(db)
        .await
        .map_err(AppError::DbError)?;

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions_are_accepted() {
        assert!(is_valid_transition(&Status::Processando, &Status::Confirmado));
        assert!(is_valid_transition(&Status::Processando, &Status::Cancelado));
        assert!(is_valid_transition(&Status::Confirmado, &Status::Enviado));
        assert!(is_valid_transition(&Status::Confirmado, &Status::Rejeitado));
        assert!(is_valid_transition(&Status::Confirmado, &Status::Cancelado));
        assert!(is_valid_transition(&Status::Enviado, &Status::Entregue));
        assert!(is_valid_transition(&Status::Enviado, &Status::Cancelado));
        assert!(is_valid_transition(&Status::Rejeitado, &Status::Cancelado));
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        assert!(!is_valid_transition(&Status::Entregue, &Status::Cancelado));
        assert!(!is_valid_transition(&Status::Cancelado, &Status::Confirmado));
        assert!(!is_valid_transition(&Status::Processando, &Status::Entregue));
        assert!(!is_valid_transition(&Status::Enviado, &Status::Processando));
        assert!(!is_valid_transition(&Status::Entregue, &Status::Processando));
    }

    #[test]
    fn compute_total_sums_correctly() {
        use chrono::Utc;
        let items = vec![
            OrderItem {
                id: 1, id_order: 1, id_product: 1,
                quantity: 3,
                unit_price: Decimal::new(1500, 2), // 15.00
                created_at: Utc::now(),
            },
            OrderItem {
                id: 2, id_order: 1, id_product: 2,
                quantity: 2,
                unit_price: Decimal::new(1000, 2), // 10.00
                created_at: Utc::now(),
            },
        ];
        let total = compute_total(&items);
        assert_eq!(total, Decimal::new(6500, 2)); // 65.00
    }
}
```

- [ ] **Step 2: Register module in `lib.rs`**

```rust
pub mod auth;
pub mod models;
pub mod produto_client;
pub mod schema;
pub mod service;
```

- [ ] **Step 3: Run unit tests**

```bash
cargo test -p pedidos service
```

Expected: `valid_transitions_are_accepted`, `invalid_transitions_are_rejected`, `compute_total_sums_correctly` all pass.

- [ ] **Step 4: Commit**

```bash
git add servicos/pedidos/src/service.rs servicos/pedidos/src/lib.rs
git commit -m "feat(pedidos): add service layer with state machine, CRUD, and item mutation logic"
```

---

## Task 14: `pedidos` — Handlers

**Files:**
- Create: `servicos/pedidos/src/handlers.rs`
- Modify: `servicos/pedidos/src/lib.rs`

Thin HTTP handlers — extract JWT, call service, return response.

- [ ] **Step 1: Create `handlers.rs`**

```rust
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
    auth::JwtCustomer,
    models::AppState,
    schema::{CreateOrderSchema, OrderListQuery, UpdateOrderItemsSchema, UpdateStatusSchema},
    service,
};

pub async fn create_order_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Json(body): Json<CreateOrderSchema>,
) -> Result<impl IntoResponse, AppError> {
    info!(%customer_id, "Creating order");
    let order = service::create_order(&state, customer_id, body).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "order": order }))))
}

pub async fn list_orders_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(_customer_id): JwtCustomer,
    Query(query): Query<OrderListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let orders = service::list_orders(&state.db, &query).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "orders": orders }))))
}

pub async fn get_order_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(_customer_id): JwtCustomer,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    info!(id, "Fetching order");
    let order = service::get_order(&state.db, id).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "order": order }))))
}

pub async fn update_status_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Path(id): Path<i64>,
    Json(body): Json<UpdateStatusSchema>,
) -> Result<impl IntoResponse, AppError> {
    warn!(id, "Updating order status");
    let order = service::update_status(&state.db, id, customer_id, body).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "order": order }))))
}

pub async fn update_items_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Path(id): Path<i64>,
    Json(body): Json<UpdateOrderItemsSchema>,
) -> Result<impl IntoResponse, AppError> {
    warn!(id, "Updating order items");
    let order = service::update_items(&state, id, customer_id, body).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "order": order }))))
}

pub async fn delete_order_handler(
    State(state): State<Arc<AppState>>,
    JwtCustomer(customer_id): JwtCustomer,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    warn!(id, "Deleting order");
    let order = service::delete_order(&state.db, id, customer_id).await?;
    Ok(Json(ApiResponse::success(serde_json::json!({ "order": order }))))
}
```

- [ ] **Step 2: Register module in `lib.rs`**

```rust
pub mod auth;
pub mod handlers;
pub mod models;
pub mod produto_client;
pub mod schema;
pub mod service;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p pedidos
```

- [ ] **Step 4: Commit**

```bash
git add servicos/pedidos/src/handlers.rs servicos/pedidos/src/lib.rs
git commit -m "feat(pedidos): add thin HTTP handlers"
```

---

## Task 15: `pedidos` — Router

**Files:**
- Create: `servicos/pedidos/src/router.rs`
- Modify: `servicos/pedidos/src/lib.rs`

- [ ] **Step 1: Create `router.rs`**

```rust
use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, patch, post},
};

use crate::{
    handlers::{
        create_order_handler, delete_order_handler, get_order_handler,
        list_orders_handler, update_items_handler, update_status_handler,
    },
    models::AppState,
};

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/pedidos", post(create_order_handler).get(list_orders_handler))
        .route(
            "/api/pedidos/{id}",
            get(get_order_handler).delete(delete_order_handler),
        )
        .route("/api/pedidos/{id}/status", patch(update_status_handler))
        .route("/api/pedidos/{id}/items", patch(update_items_handler))
        .with_state(state)
}
```

- [ ] **Step 2: Register module in `lib.rs`**

```rust
pub mod auth;
pub mod handlers;
pub mod models;
pub mod produto_client;
pub mod router;
pub mod schema;
pub mod service;
```

- [ ] **Step 3: Verify**

```bash
cargo check -p pedidos
```

- [ ] **Step 4: Commit**

```bash
git add servicos/pedidos/src/router.rs servicos/pedidos/src/lib.rs
git commit -m "feat(pedidos): add router"
```

---

## Task 16: `pedidos` — `main.rs` Bootstrap

**Files:**
- Modify: `servicos/pedidos/src/main.rs`

Mirrors `produtos/main.rs` structure, builds `AppState` with db pool + reqwest client + produtos URL.

- [ ] **Step 1: Rewrite `main.rs`**

```rust
use common::db_utils::create_pool;
use dotenvy::dotenv;
use miette::IntoDiagnostic;
use pedidos::{models::AppState, router::create_router};
use std::sync::Arc;
use tracing_appender::rolling::Rotation;
use tracing_subscriber::{filter, fmt, prelude::*};

#[tokio::main]
async fn main() -> miette::Result<()> {
    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(10)
        .filename_prefix("pedidos_api.log")
        .build("/var/log")
        .into_diagnostic()?;
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(fmt::layer().with_writer(non_blocking).json())
        .with(filter::EnvFilter::try_from_env("PEDIDOS_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    dotenv().ok();

    let pool = create_pool(10).await;
    let http = reqwest::Client::new();
    let produtos_url = std::env::var("PRODUTOS_SERVICE_URL")
        .expect("PRODUTOS_SERVICE_URL must be set");

    let state = Arc::new(AppState { db: pool, http, produtos_url });
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .into_diagnostic()?;

    axum::serve(listener, app).await.into_diagnostic()
}
```

- [ ] **Step 2: Add `PRODUTOS_SERVICE_URL` to `.env.example`**

Open `.env.example` and add:

```
PRODUTOS_SERVICE_URL=http://localhost:3000
```

- [ ] **Step 3: Full build**

```bash
cargo build --workspace
```

Expected: clean build, zero errors.

- [ ] **Step 4: Run all tests**

```bash
cargo test --workspace
```

Expected: all unit tests pass.

- [ ] **Step 5: Commit**

```bash
git add servicos/pedidos/src/main.rs .env.example
git commit -m "feat(pedidos): complete server bootstrap with AppState and tracing"
```

---

## Task 17: Update SQLx Offline Cache

SQLx compile-time query checking requires an offline cache (`.sqlx/` directory) or a live `DATABASE_URL`. Run this to regenerate the cache after all schema changes.

- [ ] **Step 1: Prepare SQLx cache for `produtos`**

```bash
cd servicos/produtos
cargo sqlx prepare
```

Expected: `.sqlx/` directory updated with new query metadata.

- [ ] **Step 2: Prepare SQLx cache for `pedidos`**

```bash
cd ../pedidos
cargo sqlx prepare
```

Expected: `.sqlx/` directory created/updated.

- [ ] **Step 3: Verify offline build (no DB needed)**

```bash
cd ../..
SQLX_OFFLINE=true cargo build --workspace
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add servicos/produtos/.sqlx/ servicos/pedidos/.sqlx/
git commit -m "chore: update sqlx offline query cache for new schema"
```

---

## Self-Review Checklist

- [x] `libs/errors` changes covered in Task 2
- [x] `produtos` migration for `descricao`/`estoque` covered in Task 3
- [x] `imagens_produto` table covered in Task 3
- [x] `produtos` model/schema updates covered in Task 4
- [x] `produtos` handler updates for new fields covered in Task 5
- [x] Image upload/list/delete endpoints covered in Task 6
- [x] Broken `pedidos` migrations replaced in Task 7
- [x] `pedidos` deps covered in Task 8
- [x] `pedidos` models fixed in Task 9
- [x] `pedidos` schemas in Task 10
- [x] JWT extractor in Task 11
- [x] Concurrent product validation in Task 12
- [x] Full service layer with state machine in Task 13
- [x] Handlers in Task 14
- [x] Router in Task 15
- [x] Server bootstrap in Task 16
- [x] SQLx cache in Task 17
