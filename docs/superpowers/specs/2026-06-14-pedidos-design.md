# Pedidos Microservice — Design Spec

**Date:** 2026-06-14
**Status:** Approved

---

## Context

Rust workspace using Axum + SQLx + PostgreSQL. Two existing services: `produtos` (complete) and `pedidos` (scaffolded, incomplete). Two shared libs: `common` (ApiResponse) and `errors` (AppError).

The `pedidos` service has partial models, schemas, and migrations already written but contains SQL syntax errors, wrong column types, and no handlers or router yet.

---

## Scope

1. Complete the `pedidos` service with full CRUD + search
2. Extend the `produtos` service with stock, description, and image support
3. Extend `libs/errors` to support UUID/i64 IDs and batch validation errors

Out of scope: real-time stock reservation, event-driven messaging, customer service implementation.

---

## `produtos` Extensions

### New columns (migration)

```sql
ALTER TABLE produtos
  ADD COLUMN descricao TEXT,
  ADD COLUMN estoque INTEGER NOT NULL DEFAULT 0 CHECK (estoque >= 0);
```

- `descricao` — optional free-text product description
- `estoque` — current stock quantity; enforced non-negative at DB level

### New table — `imagens_produto`

```sql
CREATE TABLE imagens_produto (
  id         BIGSERIAL PRIMARY KEY,
  id_produto INTEGER NOT NULL REFERENCES produtos(id) ON DELETE CASCADE,
  path       TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Images are stored on local disk under a `static/` directory within the `produtos` service. The `path` column holds the relative filesystem path. Deleting an image record also deletes the file from disk.

### New endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/produtos/:id/imagens` | Multipart upload; saves file to `static/`, stores path in DB |
| `GET` | `/produtos/:id/imagens` | List all image records for a product |
| `DELETE` | `/produtos/:id/imagens/:img_id` | Delete DB record and file from disk |

---

## `libs/errors` Changes

### `AppError::NotFound` — change `id` field

Current: `id: i32`
New: `id: String`

Reason: pedidos uses `i64` order IDs and `Uuid` customer IDs. Using `String` unifies all ID types without a breaking change to callers (pass `.to_string()` at call sites).

### New variants

```rust
// Returned when one or more items fail product validation
ValidationFailed { items: Vec<ItemValidationError> }

// Returned when JWT is missing, invalid, or customer_id does not match order owner
Unauthorized

// Returned on invalid status transitions or attempts to modify a non-editable order
UnprocessableEntity(String)
```

`ItemValidationError` is defined in the `pedidos` crate:

```rust
pub struct ItemValidationError {
    pub id_product: i32,
    pub reason: String,   // e.g. "product not found", "insufficient stock (requested 5, available 2)"
}
```

HTTP status mappings:
- `ValidationFailed` → `422 Unprocessable Entity`
- `Unauthorized` → `401 Unauthorized`
- `UnprocessableEntity` → `422 Unprocessable Entity`

---

## `pedidos` Database Schema

The existing migrations have syntax errors and must be replaced with corrected versions.

### `pedidos` table

```sql
CREATE TYPE status AS ENUM (
  'processando',
  'confirmado',
  'enviado',
  'entregue',
  'cancelado',
  'rejeitado'
);

CREATE TABLE pedidos (
  id          BIGSERIAL PRIMARY KEY,
  customer_id UUID NOT NULL,
  stat        status NOT NULL DEFAULT 'processando',
  created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pedidos_customer ON pedidos(customer_id);
CREATE INDEX idx_pedidos_stat     ON pedidos(stat);
```

`customer_id` is a UUID extracted from the caller's JWT. It is never accepted from the request body.

### `items_pedidos` table

```sql
CREATE TABLE items_pedidos (
  id         BIGSERIAL PRIMARY KEY,
  id_order   BIGINT NOT NULL REFERENCES pedidos(id) ON DELETE CASCADE,
  id_product INTEGER NOT NULL,
  quantity   INTEGER NOT NULL CHECK (quantity > 0),
  unit_price DECIMAL(10,2) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_items_pedidos_order ON items_pedidos(id_order);
```

`unit_price` is captured at order creation time from the current `produtos.valor`. Price changes in `produtos` do not retroactively affect existing orders.

`id_product` is a reference to the `produtos` service — no foreign key constraint since the services have independent databases.

---

## `pedidos` Module Structure

```
servicos/pedidos/src/
├── main.rs            -- server bootstrap; builds AppState (db pool, http client, produtos URL)
├── lib.rs             -- module declarations
├── router.rs          -- route definitions, applies JWT middleware
├── models.rs          -- Order, OrderItem, CompleteOrder, Status (fix customer_id to Uuid)
├── schema.rs          -- request/response DTOs
├── handlers.rs        -- thin HTTP handlers; extract JWT, delegate to service
├── service.rs         -- business logic: state machine enforcement, item mutation rules,
│                         validation orchestration
└── produto_client.rs  -- async HTTP client wrapping all calls to the produtos service
```

### `AppState`

```rust
pub struct AppState {
    pub db: PgPool,
    pub http: reqwest::Client,    // shared client reused across requests
    pub produtos_url: String,     // base URL, e.g. "http://localhost:3001", from env
}
```

---

## `pedidos` API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/pedidos` | JWT | Create order; validates all items concurrently before persisting |
| `GET` | `/pedidos` | JWT | List orders; filter by `?customer_id=<uuid>`, `?status=<status>`, `?limit=<n>` |
| `GET` | `/pedidos/:id` | JWT | Get full order with items list and computed total |
| `PATCH` | `/pedidos/:id/status` | JWT | Update status (validated against state machine) |
| `PATCH` | `/pedidos/:id/items` | JWT | Add/remove/update item quantities (only when `Processando`) |
| `DELETE` | `/pedidos/:id` | JWT | Delete order (only when `Processando` or `Cancelado`) |

### Request/response shapes

**`POST /pedidos` body:**
```json
{
  "items": [
    { "id_product": 1, "quantity": 3 },
    { "id_product": 5, "quantity": 1 }
  ]
}
```

**`GET /pedidos/:id` response:**
```json
{
  "status": "ok",
  "data": {
    "order": {
      "id": 42,
      "customer_id": "uuid-here",
      "stat": "processando",
      "created_at": "...",
      "updated_at": "..."
    },
    "items": [
      { "id": 1, "id_product": 1, "quantity": 3, "unit_price": "15.00" }
    ],
    "total": "45.00"
  }
}
```

**`PATCH /pedidos/:id/status` body:**
```json
{ "status": "confirmado" }
```

**`PATCH /pedidos/:id/items` body:**
```json
{
  "add": [{ "id_product": 7, "quantity": 2 }],
  "update": [{ "id": 1, "quantity": 5 }],
  "remove": [3]
}
```
All three fields are optional and can be combined in a single request.

- `add` — new products to append; triggers concurrent product validation against `produtos`; `unit_price` is fetched from `produtos` at this point
- `update` — change `quantity` of existing items by their `items_pedidos.id`; `unit_price` is **not** changed (price was locked at creation)
- `remove` — delete existing items by their `items_pedidos.id` (not by `id_product`)

---

## Status State Machine

Valid transitions:

```
Processando → Confirmado
Processando → Cancelado
Confirmado  → Enviado
Confirmado  → Rejeitado
Confirmado  → Cancelado
Enviado     → Entregue
Enviado     → Cancelado
Rejeitado   → Cancelado
```

`Entregue` and `Cancelado` are terminal states — no further transitions.
`Rejeitado` is also terminal except it can be cancelled.

Any attempted transition not listed above returns `422 UnprocessableEntity` with a message indicating the invalid transition.

Item modification (`PATCH /pedidos/:id/items`) is only permitted when `stat == Processando`. Any other state returns `422`.

---

## Product Validation Flow

Triggered on `POST /pedidos` and on `PATCH /pedidos/:id/items` when `add` items are present.

1. Collect all `id_product` values from the request
2. Spawn one async task per product calling `GET {produtos_url}/produtos/{id}`
3. `futures::future::join_all(...)` — all tasks run concurrently
4. For each result:
   - If HTTP 404 or error → record `ItemValidationError { id_product, reason: "product not found" }`
   - If `requested_quantity > product.estoque` → record `ItemValidationError { id_product, reason: "insufficient stock (requested N, available M)" }`
   - If valid → capture `unit_price = product.valor` for use in INSERT
5. If any errors collected → return `422 ValidationFailed { items: errors }`
6. If all pass → proceed to DB transaction

The `unit_price` for each item is locked in at validation time (step 4), not re-fetched during INSERT.

---

## JWT Handling

A custom Axum extractor reads `Authorization: Bearer <token>`, validates the JWT signature and expiry, and extracts `customer_id: Uuid` from the claims.

- `customer_id` is **never** accepted from the request body
- The JWT claims must contain a `customer_id` field (UUID string). This format must be agreed upon with the customer service team. Example claims: `{ "customer_id": "550e8400-...", "exp": 1234567890 }`
- On `PATCH /pedidos/:id/*` and `DELETE /pedidos/:id`: the service fetches the order and asserts `order.customer_id == jwt.customer_id` before proceeding; mismatch returns `401 Unauthorized`
- The JWT secret is read from the environment variable `JWT_SECRET`
- The JWT library to use is `jsonwebtoken` (to be added to workspace dependencies)

---

## Dependencies to Add

| Crate | Where | Purpose |
|-------|-------|---------|
| `reqwest` | `pedidos` | HTTP client for produtos calls |
| `futures` | `pedidos` | `join_all` for concurrent validation |
| `jsonwebtoken` | `pedidos` | JWT validation and claims extraction |
| `uuid` | `pedidos` + workspace | UUID type with serde + sqlx support |
| `tokio-multipart` (via `axum::extract::Multipart`) | `produtos` | Built into Axum 0.8 — no extra crate needed |

---

## Error Handling Summary

| Scenario | HTTP Status | `AppError` variant |
|----------|-------------|---------------------|
| Order/product not found | 404 | `NotFound { service, id }` |
| JWT missing or invalid | 401 | `Unauthorized` |
| Customer does not own order | 401 | `Unauthorized` |
| Invalid status transition | 422 | `UnprocessableEntity` |
| Items modified outside `Processando` | 422 | `UnprocessableEntity` |
| One or more items fail validation | 422 | `ValidationFailed { items }` |
| DB error | 500 | `DbError` |
