# Front-End Gotcha Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix four backend issues that block front-end integration: missing image serving, missing CORS headers, flat order list (no items/total), and inability to clear `descricao`/`num_fab` via PATCH.

**Architecture:** `tower-http` adds CORS middleware and static file serving to both APIs. `list_orders` fetches items in a single `ANY($1)` query and assembles `CompleteOrder`s. `UpdateProductSchema` uses `Option<Option<String>>` to distinguish "absent" from "explicit null".

**Tech Stack:** Rust, Axum 0.8, `tower-http 0.6` (new dep), sqlx, existing `pedidos` and `produtos` crates.

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `servicos/produtos/Cargo.toml` | Modify | Add `tower-http 0.6` with `fs`, `cors` features |
| `servicos/produtos/src/models.rs` | Modify | Add `frontend_url: String` to `AppState` |
| `servicos/produtos/src/main.rs` | Modify | Read `FRONTEND_URL` env var, pass to `AppState` |
| `servicos/produtos/src/router.rs` | Modify | Add `CorsLayer` + `ServeDir` at `/static` |
| `servicos/produtos/src/schema.rs` | Modify | `descricao` + `num_fab` → `Option<Option<String>>` with `#[serde(default)]` |
| `servicos/produtos/src/handlers.rs` | Modify | Three-state match for `descricao` and `num_fab` in update handler |
| `servicos/pedidos/Cargo.toml` | Modify | Add `tower-http 0.6` with `cors` feature |
| `servicos/pedidos/src/models.rs` | Modify | Add `frontend_url: String` to `AppState`; `Clone` on `OrderItem` |
| `servicos/pedidos/src/main.rs` | Modify | Read `FRONTEND_URL` env var, pass to `AppState` |
| `servicos/pedidos/src/router.rs` | Modify | Add `CorsLayer` |
| `servicos/pedidos/src/service.rs` | Modify | `list_orders` returns `Vec<CompleteOrder>`, fetches items via `ANY($1)` |
| `servicos/pedidos/src/handlers.rs` | Modify | Update `list_orders_handler` return type annotation |
| `docker-compose.yml` | Modify | Add `FRONTEND_URL` to both API services |
| `.env.example` | Modify | Document `FRONTEND_URL` |
| `.sqlx/` | Modify | Regenerate offline query cache after new sqlx query |
| `docs/API_GUIDE.md` | Modify | Fix image serving section, fix decimal note |

---

## Task 1: CORS + Image Serving for `produtos-api`

Add `tower-http`, wire `CorsLayer` and `ServeDir` in the produtos router, and extend `AppState`/`main.rs`.

**Files:**
- Modify: `servicos/produtos/Cargo.toml`
- Modify: `servicos/produtos/src/models.rs`
- Modify: `servicos/produtos/src/main.rs`
- Modify: `servicos/produtos/src/router.rs`

- [ ] **Step 1: Add `tower-http` to `servicos/produtos/Cargo.toml`**

Add this line to the `[dependencies]` section:

```toml
tower-http = { version = "0.6", features = ["fs", "cors"] }
```

- [ ] **Step 2: Add `frontend_url` field to `AppState` in `models.rs`**

Current `AppState`:
```rust
#[derive(Debug)]
pub struct AppState {
    pub db: PgPool,
    pub static_dir: std::path::PathBuf,
}
```

Change to:
```rust
#[derive(Debug)]
pub struct AppState {
    pub db: PgPool,
    pub static_dir: std::path::PathBuf,
    pub frontend_url: String,
}
```

- [ ] **Step 3: Read `FRONTEND_URL` in `main.rs` and populate `AppState`**

In `servicos/produtos/src/main.rs`, find the block that reads `STATIC_DIR` and constructs `AppState`. The current `AppState` construction looks like:

```rust
let static_dir = std::path::PathBuf::from(&static_dir_str);
// ...
let app = create_router(Arc::new(AppState { db: pool.clone(), static_dir }));
```

Add the `FRONTEND_URL` read after `STATIC_DIR` and before `create_router`:

```rust
let frontend_url = std::env::var("FRONTEND_URL")
    .into_diagnostic()
    .wrap_err("FRONTEND_URL must be set")?;

let app = create_router(Arc::new(AppState {
    db: pool.clone(),
    static_dir,
    frontend_url,
}));
```

- [ ] **Step 4: Add `CorsLayer` and `ServeDir` to `router.rs`**

Replace the entire `router.rs` content with:

```rust
use std::sync::Arc;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{
        HeaderValue, Method,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    routing::{delete, get, post},
};
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::{
    handlers::{
        create_product_handler, delete_product_by_id, delete_product_image,
        get_product_by_id, get_product_images, get_products_by_query,
        update_product_by_id, upload_product_image,
    },
    models::AppState,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            app_state
                .frontend_url
                .parse::<HeaderValue>()
                .expect("FRONTEND_URL is not a valid HTTP origin header value"),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION]);

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
            post(upload_product_image)
                .layer(DefaultBodyLimit::max(5 * 1024 * 1024))
                .get(get_product_images),
        )
        .route(
            "/api/products/{id}/imagens/{img_id}",
            delete(delete_product_image),
        )
        .nest_service("/static", ServeDir::new(&app_state.static_dir))
        .layer(cors)
        .with_state(app_state)
}
```

- [ ] **Step 5: Build to verify no compile errors**

```bash
cargo build --package produtos 2>&1 | grep -E "^error"
```

Expected: no output.

- [ ] **Step 6: Commit**

```bash
git add servicos/produtos/Cargo.toml servicos/produtos/src/models.rs \
        servicos/produtos/src/main.rs servicos/produtos/src/router.rs
git commit -m "feat(produtos): add CORS middleware and static image serving"
```

---

## Task 2: CORS for `pedidos-api`

Add `tower-http`, wire `CorsLayer` in the pedidos router, extend `AppState`/`main.rs`.

**Files:**
- Modify: `servicos/pedidos/Cargo.toml`
- Modify: `servicos/pedidos/src/models.rs`
- Modify: `servicos/pedidos/src/main.rs`
- Modify: `servicos/pedidos/src/router.rs`

- [ ] **Step 1: Add `tower-http` to `servicos/pedidos/Cargo.toml`**

Add to `[dependencies]`:

```toml
tower-http = { version = "0.6", features = ["cors"] }
```

- [ ] **Step 2: Add `frontend_url` to `AppState` in `servicos/pedidos/src/models.rs`**

Current `AppState` ends with `seller_whatsapp`. Add one field:

```rust
#[derive(Debug)]
pub struct AppState {
    pub db: PgPool,
    pub http: reqwest::Client,
    pub produtos_url: String,
    pub jwt_secret: String,
    // Evolution API / WhatsApp notification
    pub evolution_url: String,
    pub evolution_key: String,
    pub evolution_instance: String,
    pub seller_whatsapp: String,
    // CORS
    pub frontend_url: String,
}
```

- [ ] **Step 3: Read `FRONTEND_URL` in `servicos/pedidos/src/main.rs`**

Find the block that reads env vars and constructs `AppState`. Add after the `seller_whatsapp` read:

```rust
let frontend_url = std::env::var("FRONTEND_URL")
    .into_diagnostic()
    .wrap_err("FRONTEND_URL must be set")?;
```

Add to the `AppState` constructor:

```rust
let state = Arc::new(AppState {
    db: pool,
    http,
    produtos_url,
    jwt_secret,
    evolution_url,
    evolution_key,
    evolution_instance,
    seller_whatsapp,
    frontend_url,
});
```

- [ ] **Step 4: Add `CorsLayer` to `servicos/pedidos/src/router.rs`**

Replace the entire file content with:

```rust
use std::sync::Arc;

use axum::{
    Router,
    http::{
        HeaderValue, Method,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    routing::{get, patch, post},
};
use tower_http::cors::CorsLayer;

use crate::{
    handlers::{
        create_order_handler, delete_order_handler, get_order_handler,
        list_orders_handler, update_items_handler, update_status_handler,
    },
    models::AppState,
};

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            state
                .frontend_url
                .parse::<HeaderValue>()
                .expect("FRONTEND_URL is not a valid HTTP origin header value"),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION]);

    Router::new()
        .route("/api/pedidos", post(create_order_handler).get(list_orders_handler))
        .route(
            "/api/pedidos/{id}",
            get(get_order_handler).delete(delete_order_handler),
        )
        .route("/api/pedidos/{id}/status", patch(update_status_handler))
        .route("/api/pedidos/{id}/items", patch(update_items_handler))
        .layer(cors)
        .with_state(state)
}
```

- [ ] **Step 5: Build to verify**

```bash
cargo build --package pedidos 2>&1 | grep -E "^error"
```

Expected: no output.

- [ ] **Step 6: Commit**

```bash
git add servicos/pedidos/Cargo.toml servicos/pedidos/src/models.rs \
        servicos/pedidos/src/main.rs servicos/pedidos/src/router.rs
git commit -m "feat(pedidos): add CORS middleware"
```

---

## Task 3: Fix `list_orders` to return `Vec<CompleteOrder>`

Fetch all items for returned orders in one `ANY($1)` query, group by order, assemble `CompleteOrder`s.

**Files:**
- Modify: `servicos/pedidos/src/models.rs` (add `Clone` to `OrderItem`)
- Modify: `servicos/pedidos/src/service.rs`
- Modify: `servicos/pedidos/src/handlers.rs`

- [ ] **Step 1: Add `Clone` to `OrderItem` in `models.rs`**

Change:

```rust
#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct OrderItem {
```

To:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, sqlx::FromRow)]
pub struct OrderItem {
```

- [ ] **Step 2: Write a unit test for the new `list_orders` assembly logic**

Add to the `#[cfg(test)]` module at the bottom of `servicos/pedidos/src/service.rs`:

```rust
#[test]
fn complete_orders_assembled_correctly() {
    use chrono::Utc;
    use uuid::Uuid;

    let customer = Uuid::new_v4();
    let now = Utc::now();

    let orders = vec![
        Order {
            id: 1,
            customer_id: customer,
            stat: Status::Processando,
            created_at: now,
            updated_at: now,
        },
        Order {
            id: 2,
            customer_id: customer,
            stat: Status::Confirmado,
            created_at: now,
            updated_at: now,
        },
    ];

    let all_items = vec![
        OrderItem { id: 10, id_order: 1, id_product: 100, quantity: 2, unit_price: Decimal::new(1000, 2), created_at: now },
        OrderItem { id: 11, id_order: 2, id_product: 200, quantity: 1, unit_price: Decimal::new(5000, 2), created_at: now },
        OrderItem { id: 12, id_order: 1, id_product: 101, quantity: 3, unit_price: Decimal::new(500, 2),  created_at: now },
    ];

    let complete: Vec<CompleteOrder> = orders
        .into_iter()
        .map(|order| {
            let items: Vec<OrderItem> = all_items
                .iter()
                .filter(|i| i.id_order == order.id)
                .cloned()
                .collect();
            let total = compute_total(&items);
            CompleteOrder { order, items, total }
        })
        .collect();

    // Order 1: 2×10.00 + 3×5.00 = 35.00
    assert_eq!(complete[0].items.len(), 2);
    assert_eq!(complete[0].total, Decimal::new(3500, 2));

    // Order 2: 1×50.00 = 50.00
    assert_eq!(complete[1].items.len(), 1);
    assert_eq!(complete[1].total, Decimal::new(5000, 2));
}
```

- [ ] **Step 3: Run the test to verify it passes with current code (it should — it tests the assembly logic directly)**

```bash
cargo test --package pedidos complete_orders_assembled_correctly 2>&1 | tail -5
```

Expected: `test service::tests::complete_orders_assembled_correctly ... ok`

- [ ] **Step 4: Rewrite `list_orders` in `service.rs`**

Replace the current `list_orders` function (lines 70–92) with:

```rust
pub async fn list_orders(
    db: &PgPool,
    jwt_customer_id: Uuid,
    query: &OrderListQuery,
) -> Result<Vec<CompleteOrder>, AppError> {
    let orders = sqlx::query_as!(
        Order,
        r#"SELECT id, customer_id, stat as "stat: Status", created_at, updated_at
        FROM pedidos
        WHERE customer_id = $1
          AND ($2::order_status IS NULL OR stat = $2)
        ORDER BY created_at DESC
        LIMIT $3"#,
        jwt_customer_id,
        query.status.clone() as Option<Status>,
        query.limit.unwrap_or(50).min(200),
    )
    .fetch_all(db)
    .await
    .map_err(AppError::DbError)?;

    if orders.is_empty() {
        return Ok(vec![]);
    }

    let order_ids: Vec<i64> = orders.iter().map(|o| o.id).collect();

    let all_items = sqlx::query_as!(
        OrderItem,
        r#"SELECT id, id_order, id_product, quantity, unit_price, created_at
        FROM items_pedidos
        WHERE id_order = ANY($1)"#,
        &order_ids[..] as &[i64],
    )
    .fetch_all(db)
    .await
    .map_err(AppError::DbError)?;

    let complete_orders = orders
        .into_iter()
        .map(|order| {
            let items: Vec<OrderItem> = all_items
                .iter()
                .filter(|i| i.id_order == order.id)
                .cloned()
                .collect();
            let total = compute_total(&items);
            CompleteOrder { order, items, total }
        })
        .collect();

    Ok(complete_orders)
}
```

- [ ] **Step 5: Update `list_orders_handler` in `handlers.rs`**

The handler body does not need to change — it just passes the result through. However, the return type annotation in the `serde_json::json!` call is still correct (`"orders": orders` where `orders` is now `Vec<CompleteOrder>`). Confirm the handler still compiles as-is.

- [ ] **Step 6: Regenerate the sqlx offline query cache**

The new `ANY($1)` query is not yet in `.sqlx/`. Build against a live DB to regenerate. From the workspace root:

```bash
# Start the pedidos DB if not already running
docker compose up -d pedidos-db
# Wait ~10s for it to be healthy, then regenerate the cache
# The DATABASE_URL is read from .env automatically by sqlx
cargo sqlx prepare --workspace
```

Expected: new or updated `.sqlx/*.json` files. Stage them with `git add .sqlx/`.

If `cargo sqlx` is not installed:
```bash
cargo install sqlx-cli --no-default-features --features postgres
```

- [ ] **Step 7: Build and run all tests**

```bash
cargo build --package pedidos 2>&1 | grep -E "^error"
cargo test --package pedidos 2>&1 | grep "test result"
```

Expected: zero build errors, all tests pass.

- [ ] **Step 8: Commit**

```bash
git add servicos/pedidos/src/models.rs servicos/pedidos/src/service.rs \
        servicos/pedidos/src/handlers.rs .sqlx/
git commit -m "feat(pedidos): list_orders returns CompleteOrder with items and total"
```

---

## Task 4: Fix `descricao` and `num_fab` clearing in `produtos`

Use `Option<Option<String>>` to distinguish "absent" (keep) from explicit `null` (clear).

**Files:**
- Modify: `servicos/produtos/src/schema.rs`
- Modify: `servicos/produtos/src/handlers.rs`

- [ ] **Step 1: Write a unit test for the three-state serde deserialization**

Add to `servicos/produtos/src/schema.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descricao_absent_deserializes_to_none() {
        let s = r#"{"nome": "test", "valor": 1.0, "marca": "X", "unidade": "PC"}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert!(schema.descricao.is_none(), "absent field should be None (outer)");
    }

    #[test]
    fn descricao_null_deserializes_to_some_none() {
        let s = r#"{"descricao": null}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert_eq!(schema.descricao, Some(None), "explicit null should be Some(None)");
    }

    #[test]
    fn descricao_string_deserializes_to_some_some() {
        let s = r#"{"descricao": "hello"}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert_eq!(schema.descricao, Some(Some("hello".to_string())));
    }

    #[test]
    fn num_fab_absent_deserializes_to_none() {
        let s = r#"{}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert!(schema.num_fab.is_none());
    }

    #[test]
    fn num_fab_null_deserializes_to_some_none() {
        let s = r#"{"num_fab": null}"#;
        let schema: UpdateProductSchema = serde_json::from_str(s).unwrap();
        assert_eq!(schema.num_fab, Some(None));
    }
}
```

- [ ] **Step 2: Run the tests to verify they FAIL (schema not yet updated)**

```bash
cargo test --package produtos schema::tests 2>&1 | tail -10
```

Expected: compilation failure or test failures — `descricao` is still `Option<String>`.

- [ ] **Step 3: Update `UpdateProductSchema` in `schema.rs`**

Replace the current `UpdateProductSchema`:

```rust
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
```

With:

```rust
#[derive(Deserialize, Debug)]
pub struct UpdateProductSchema {
    pub nome: Option<String>,
    pub marca: Option<String>,
    /// `None` = absent (keep existing), `Some(None)` = explicit null (clear), `Some(Some(s))` = update
    #[serde(default)]
    pub num_fab: Option<Option<String>>,
    pub unidade: Option<String>,
    pub valor: Option<Decimal>,
    /// `None` = absent (keep existing), `Some(None)` = explicit null (clear), `Some(Some(s))` = update
    #[serde(default)]
    pub descricao: Option<Option<String>>,
    pub estoque: Option<i32>,
}
```

- [ ] **Step 4: Run the schema tests to verify they now PASS**

```bash
cargo test --package produtos schema::tests 2>&1 | tail -10
```

Expected:
```
test schema::tests::descricao_absent_deserializes_to_none ... ok
test schema::tests::descricao_null_deserializes_to_some_none ... ok
test schema::tests::descricao_string_deserializes_to_some_some ... ok
test schema::tests::num_fab_absent_deserializes_to_none ... ok
test schema::tests::num_fab_null_deserializes_to_some_none ... ok
test result: ok. 5 passed; 0 failed
```

- [ ] **Step 5: Update the handler in `handlers.rs`**

In `update_product_by_id`, find and replace the two lines:

```rust
let nova_descricao = body.descricao.or(product.descricao);
// ...
let novo_num_fab = body.num_fab.or(product.num_fab);
```

With:

```rust
let nova_descricao = match body.descricao {
    None => product.descricao,       // field absent — keep existing value
    Some(None) => None,              // explicit null — clear the field
    Some(Some(d)) => Some(d),        // new value — update
};
// ...
let novo_num_fab = match body.num_fab {
    None => product.num_fab,         // field absent — keep existing value
    Some(None) => None,              // explicit null — clear the field
    Some(Some(n)) => Some(n),        // new value — update
};
```

- [ ] **Step 6: Build and run all produto tests**

```bash
cargo build --package produtos 2>&1 | grep -E "^error"
cargo test --package produtos 2>&1 | grep "test result"
```

Expected: zero errors, all tests pass.

- [ ] **Step 7: Commit**

```bash
git add servicos/produtos/src/schema.rs servicos/produtos/src/handlers.rs
git commit -m "fix(produtos): allow explicit null to clear descricao and num_fab via PATCH"
```

---

## Task 5: Update `docker-compose.yml`, `.env`, `.env.example`

**Files:**
- Modify: `docker-compose.yml`
- Modify: `.env` (gitignored — do NOT stage)
- Modify: `.env.example`

- [ ] **Step 1: Add `FRONTEND_URL` to `produtos-api` in `docker-compose.yml`**

Find the `produtos-api` environment block and add:

```yaml
      FRONTEND_URL: ${FRONTEND_URL}
```

- [ ] **Step 2: Add `FRONTEND_URL` to `pedidos-api` in `docker-compose.yml`**

Find the `pedidos-api` environment block and add:

```yaml
      FRONTEND_URL: ${FRONTEND_URL}
```

- [ ] **Step 3: Add `FRONTEND_URL` to `.env`**

Append to `.env` (not staged):

```
# CORS — front-end origin allowed to call the APIs
FRONTEND_URL=http://localhost:5173
```

- [ ] **Step 4: Add `FRONTEND_URL` to `.env.example`**

Append to `.env.example`:

```
# CORS — front-end origin allowed to call the APIs
FRONTEND_URL=http://localhost:5173
```

- [ ] **Step 5: Validate compose**

```bash
docker compose config --quiet
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add docker-compose.yml .env.example
git commit -m "feat(compose): add FRONTEND_URL for CORS to both API services"
```

---

## Task 6: Update `API_GUIDE.md`

**Files:**
- Modify: `docs/API_GUIDE.md`

- [ ] **Step 1: Fix the image serving section**

Find the **Known Gaps** section entry for "Image URLs are not served by the API" and replace its content with:

```markdown
### 1. Image URLs

Images are served by `produtos-api` at:

```
GET http://localhost:3000/static/{path}
```

where `path` is the value of the `path` field from `GET /api/products/:id/imagens`.

**Example:** if `imagens` returns `"path": "3f2e1a4b-uuid.jpg"`, the full image URL is:
```
http://localhost:3000/static/3f2e1a4b-uuid.jpg
```

In production, replace `localhost:3000` with the actual `produtos-api` hostname.
```

- [ ] **Step 2: Fix the decimal serialization note**

Find **Gotcha #5** ("Decimal values serialise as strings") and replace it with:

```markdown
### 5. Decimal values serialise as JSON numbers

`valor`, `unit_price`, and `total` are serialised as JSON **numbers** (not strings), because both services use `rust_decimal` with the `serde-float` feature. Standard `parseFloat` or `Number()` works fine for display. For financial calculations requiring exact precision, use a decimal library:

```js
import Decimal from 'decimal.js';
const price = new Decimal(product['VLR_VENDA1']); // safe
```
```

- [ ] **Step 3: Commit**

```bash
git add docs/API_GUIDE.md
git commit -m "docs: update API guide — image serving URL, fix decimal serialization note"
```

---

## Task 7: Final Verification

- [ ] **Step 1: Full build of both packages**

```bash
cargo build --package produtos --package pedidos 2>&1 | grep -E "^error"
```

Expected: no output.

- [ ] **Step 2: Full test suite**

```bash
cargo test --package produtos --package pedidos 2>&1 | grep "test result"
```

Expected: all tests pass, zero failures.

- [ ] **Step 3: Clippy clean**

```bash
cargo clippy --package produtos --package pedidos -- -D warnings 2>&1 | grep -E "^error"
```

Expected: no output.

- [ ] **Step 4: Verify CORS header present**

Start the stack and make a preflight request:

```bash
docker compose up -d produtos-db produtos-init produtos-api
curl -i -X OPTIONS http://localhost:3000/api/products \
  -H "Origin: http://localhost:5173" \
  -H "Access-Control-Request-Method: GET"
```

Expected response includes:
```
access-control-allow-origin: http://localhost:5173
access-control-allow-methods: GET, POST, PATCH, DELETE, OPTIONS
```

- [ ] **Step 5: Verify image serving**

```bash
# Upload a test image first, then:
curl -I http://localhost:3000/static/<uuid>.jpg
```

Expected: `HTTP/1.1 200 OK` with `content-type: image/jpeg`.

- [ ] **Step 6: Final commit**

```bash
git add .
git commit -m "chore: frontend gotcha fixes — complete"
```
