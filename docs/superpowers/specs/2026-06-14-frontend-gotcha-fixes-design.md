# Front-End Gotcha Fixes — Design Spec

**Date:** 2026-06-14
**Status:** Approved
**Scope:** Four backend fixes that unblock front-end integration without depending on the clientes microservice.

---

## Problem

The API guide (`docs/API_GUIDE.md`) identified several front-end gotchas. This spec addresses the four that are fully within backend control:

1. Product images are stored on disk but no HTTP route serves them
2. No CORS headers — a browser-based front-end cannot call either API directly
3. `GET /api/pedidos` returns flat `Order` objects with no items or total (N+1 problem)
4. `PATCH /api/products/:id` cannot clear `descricao` once set — `null` silently preserves the existing value

---

## Fix 1 — Image Serving

**File changed:** `servicos/produtos/src/router.rs`

Add `tower-http` to `produtos/Cargo.toml` with `fs` and `cors` features. Mount `ServeDir` at `/static` in the router using the existing `AppState.static_dir` path:

```rust
.nest_service("/static", ServeDir::new(&app_state.static_dir))
```

Images are then accessible at:
```
GET http://localhost:3000/static/{uuid}.jpg
```

`ServeDir` handles `Content-Type`, `ETag`, `Last-Modified`, and `Range` headers automatically. No new handler is needed. No new env vars — `STATIC_DIR` is already read by `main.rs`. No `docker-compose.yml` changes — `produtos_static` is already mounted at `/static` on the `produtos-api` container.

**API guide update:** Replace the "images not served" warning in `docs/API_GUIDE.md` with the new URL pattern.

---

## Fix 2 — CORS

**Files changed:** both `router.rs` files, both `AppState` structs, both `main.rs` files, `docker-compose.yml`, `.env.example`

Add `tower-http::CorsLayer` to both routers. The allowed origin is read from a `FRONTEND_URL` env var at startup. Startup panics (returns error) if the var is missing — consistent with how all other required vars are handled in `main.rs`.

**`AppState` change (both services):** new field `frontend_url: String`.

**Router pattern (applied to both services):**
```rust
let cors = CorsLayer::new()
    .allow_origin(state.frontend_url.parse::<HeaderValue>()?)
    .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
    .allow_headers([CONTENT_TYPE, AUTHORIZATION]);

Router::new()
    // ... routes ...
    .layer(cors)
    .with_state(state)
```

**New env var:**

| Variable | Example | Description |
|---|---|---|
| `FRONTEND_URL` | `http://localhost:5173` | Single origin allowed by CORS |

Added to:
- Both services' `environment:` blocks in `docker-compose.yml`
- `.env` (appended)
- `.env.example` (appended)

---

## Fix 3 — List Orders Returns `CompleteOrder`

**Files changed:** `servicos/pedidos/src/service.rs`, `servicos/pedidos/src/handlers.rs`

`service::list_orders` currently returns `Vec<Order>`. Change it to return `Vec<CompleteOrder>`.

To avoid N+1 queries, items for all returned orders are fetched in one additional query:

```sql
SELECT id, id_order, id_product, quantity, unit_price, created_at
FROM items_pedidos
WHERE id_order = ANY($1)
```

where `$1` is the array of order IDs from the first query. Items are grouped by `id_order` in Rust, totals computed with `compute_total`, and `CompleteOrder`s assembled.

**Response change:** The response key stays `"orders"`, but each element gains `items` and `total`:

```json
{
  "status": "ok",
  "data": {
    "orders": [
      {
        "order": { "id": 42, "stat": "processando", ... },
        "items": [{ "id": 1, "id_product": 108, "quantity": 2, "unit_price": "67.90", ... }],
        "total": "135.80"
      }
    ]
  },
  "message": null
}
```

If no orders match the query, `orders` is an empty array — not a 404.

---

## Fix 4 — `descricao` Clearing

**Files changed:** `servicos/produtos/src/schema.rs`, `servicos/produtos/src/handlers.rs`

Change `UpdateProductSchema.descricao` from `Option<String>` to `Option<Option<String>>` with `#[serde(default)]`. This creates three distinct states:

| Client sends | Rust value | DB result |
|---|---|---|
| Field absent from JSON | `None` | Keep existing `descricao` |
| `"descricao": null` | `Some(None)` | Set column to `NULL` |
| `"descricao": "some text"` | `Some(Some("some text"))` | Set column to `"some text"` |

**Handler update:**
```rust
let nova_descricao = match body.descricao {
    None => product.descricao,       // keep
    Some(None) => None,              // clear
    Some(Some(d)) => Some(d),        // update
};
```

The same three-state pattern applies to `num_fab` for consistency (it is already `Option<String>` and has the same "cannot clear" bug). Both are fixed together.

---

## Files Touched Summary

| File | Service | Change |
|---|---|---|
| `servicos/produtos/Cargo.toml` | produtos | Add `tower-http` with `fs`, `cors` features |
| `servicos/produtos/src/models.rs` | produtos | Add `frontend_url: String` to `AppState` |
| `servicos/produtos/src/main.rs` | produtos | Read `FRONTEND_URL`, pass to `AppState` |
| `servicos/produtos/src/router.rs` | produtos | Add `ServeDir` at `/static`, add `CorsLayer` |
| `servicos/produtos/src/schema.rs` | produtos | `descricao` and `num_fab` → `Option<Option<String>>` |
| `servicos/produtos/src/handlers.rs` | produtos | Three-state match for `descricao` and `num_fab` |
| `servicos/pedidos/Cargo.toml` | pedidos | Add `tower-http` with `cors` feature |
| `servicos/pedidos/src/models.rs` | pedidos | Add `frontend_url: String` to `AppState` |
| `servicos/pedidos/src/main.rs` | pedidos | Read `FRONTEND_URL`, pass to `AppState` |
| `servicos/pedidos/src/router.rs` | pedidos | Add `CorsLayer` |
| `servicos/pedidos/src/service.rs` | pedidos | `list_orders` returns `Vec<CompleteOrder>` |
| `servicos/pedidos/src/handlers.rs` | pedidos | Update return type in `list_orders_handler` |
| `docker-compose.yml` | both | Add `FRONTEND_URL` to both API services |
| `.env.example` | both | Document `FRONTEND_URL` |
| `docs/API_GUIDE.md` | — | Fix image serving section, correct decimal note |

---

## Out of Scope

- Making `CORS` permissive for all origins (user chose specific-origin mode)
- Embedding image URLs directly inside `Product` responses
- Fixing `GET /api/pedidos` query param `customer_id` (intentional security behaviour)
- Any changes dependent on the clientes microservice
