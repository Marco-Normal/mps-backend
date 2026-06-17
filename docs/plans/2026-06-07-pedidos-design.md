# Pedidos Microservice Design Document

**Date:** 2026-06-07  
**Status:** Design Phase  
**Service Name:** `pedidos` (Portuguese for "orders")

---

## Problem

The project currently has only one microservice (`produtos` - product catalog). We need a new microservice to manage customer orders: recording which order belongs to which customer, the order items (products and quantities), and the current order status throughout its lifecycle.

---

## Findings by Branch

### 1. Database Schema Design

**Customer Reference Strategy:** Direct foreign key (`BIGINT`) to the future `customers` table. This enforces referential integrity at the database level once the customer service exists.

**Orders Table:**
| Column        | Type                    | Constraints                  | Description                   |
|---------------|-------------------------|------------------------------|-------------------------------|
| `id`          | `BIGSERIAL`             | PRIMARY KEY                  | Auto-incrementing order ID    |
| `customer_id` | `BIGINT`                | NOT NULL, FK → customers(id) | Customer who placed the order |
| `status`      | `pedidos_status` (enum) | NOT NULL, DEFAULT 'pending'  | Current order status          |
| `created_at`  | `TIMESTAMPTZ`           | NOT NULL, DEFAULT NOW()      | Order creation timestamp      |
| `updated_at`  | `TIMESTAMPTZ`           | NOT NULL, DEFAULT NOW()      | Last status change timestamp  |

**Order Items Table:**
| Column       | Type            | Constraints                                 | Description                       |
|--------------|-----------------|---------------------------------------------|-----------------------------------|
| `id`         | `BIGSERIAL`     | PRIMARY KEY                                 | Auto-incrementing item ID         |
| `order_id`   | `BIGINT`        | NOT NULL, FK → orders(id) ON DELETE CASCADE | Parent order                      |
| `product_id` | `BIGINT`        | NOT NULL                                    | Reference to produtos product     |
| `quantity`   | `INT`           | NOT NULL, CHECK (>0)                        | Number of units                   |
| `unit_price` | `DECIMAL(10,2)` | NOT NULL                                    | Price at time of order (snapshot) |
| `created_at` | `TIMESTAMPTZ`   | NOT NULL, DEFAULT NOW()                     | Item creation timestamp           |

**Status Enum (`pedidos_status`):**  
`pending` → `confirmed` → `processing` → `shipped` → `delivered`  
Any status → `cancelled` / `rejected`

**Indexes:**
- `idx_orders_customer_id` on `orders(customer_id)` — fast lookup by customer
- `idx_order_items_order_id` on `order_items(order_id)` — fast lookup by order
- `idx_orders_status` on `orders(status)` — filter by status

### 2. API Design

**Order Endpoints:**

| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/api/orders` | Create a new order (with items) |
| `GET` | `/api/orders` | List orders (filters: `?customer_id=...&status=...`) |
| `GET` | `/api/orders/{id}` | Get order by ID (includes items) |
| `PATCH` | `/api/orders/{id}/status` | Transition order status |
| `DELETE` | `/api/orders/{id}` | Cancel an order (only if not delivered) |

**Order Item Endpoints:**

| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/api/orders/{order_id}/items` | Add item to order |
| `PATCH` | `/api/orders/{order_id}/items/{item_id}` | Update item quantity/price |
| `DELETE` | `/api/orders/{order_id}/items/{item_id}` | Remove item from order |

**Status Transitions:**

| From → To | Endpoint | Constraints |
|---|---|---|
| `pending` → `confirmed` | `PATCH /status` | Order must be pending |
| `confirmed` → `processing` | `PATCH /status` | Order must be confirmed |
| `processing` → `shipped` | `PATCH /status` | Order must be processing |
| `shipped` → `delivered` | `PATCH /status` | Order must be shipped |
| Any → `cancelled` | `PATCH /status` | Not already delivered or cancelled |
| `pending` → `rejected` | `PATCH /status` | Order must be pending |

**Request/Response Format (consistent with `produtos`):**
```json
{
  "status": "ok" | "fail",
  "data": { ... },
  "message": "..." // on errors
}
```

### 3. Integration with Other Services

**Product Validation (vs `produtos` service):**  
Synchronous HTTP call to `GET /api/produtos/{product_id}` before inserting an order item. This guarantees the product exists and is available, without creating tight database coupling. If the produtos service is unavailable, the request fails with a clear error.

**Customer Authentication (vs future customer service):**  
JWT proxy pattern — the pedidos service validates the JWT token from incoming requests directly (same secret/key as the customer service). The `customer_id` is extracted from the JWT claim rather than passed as a query parameter, preventing customers from impersonating others.

### 4. Service Structure

**Directory Layout (mirroring `produtos` exactly):**
```
servicos/pedidos/
|-- Cargo.toml
|-- migrations/
|   |-- 001_create_schema.up.sql
|   |-- 001_create_schema.down.sql
|-- src/
|   |-- main.rs           # HTTP server entry point
|   |-- lib.rs            # Re-exports
|   |-- models.rs         # Order, OrderItem, AppState
|   |-- schema.rs         # Request/response DTOs
|   |-- handlers.rs       # CRUD + status transition handlers
|   |-- router.rs         # Route definitions
|   |-- bin/
|       |-- init.rs       # Migration runner
```

**Docker Compose Additions:**
- `postgres-pedidos` — PostgreSQL container for pedidos DB
- `init-pedidos` — Migration runner (reuses existing init binary pattern)
- `api-pedidos` — Axum API server on port `3001`

**Environment Variables:**
```
PEDIDOS_POSTGRES_USER=app_user
PEDIDOS_DB_NAME=pedidos_db
PEDIDOS_DB_HOST=postgres-pedidos
PEDIDOS_DB_PORT=5432
DATABASE_URL=postgresql://app_user:password@postgres-pedidos:5432/pedidos_db
PEDIDOS_API_PORT=3001
PRODUTOS_SERVICE_URL=http://produtos-api:3000
JWT_SECRET=<shared with customer service>
```

---

## Recommendation

### Approach: Follow existing `produtos` patterns exactly

This ensures consistency across services, reuses the shared `common` library (`create_pool`, `table_exists`), and minimizes new decisions. The service will be a straightforward Axum + SQLx + PostgreSQL REST API.

### Key Design Decisions:

1. **Direct FK to customers table** — Even though the customer service doesn't exist yet, use a `BIGINT` FK. This gives us referential integrity from day one and avoids a schema change later.

2. **Snapshot pricing** — Store `unit_price` in `order_items` at order time. Product prices in `produtos` may change later, but the order must reflect the price at purchase time.

3. **Cascade delete for items** — When an order is deleted, all its items are automatically removed via `ON DELETE CASCADE`.

4. **Synchronous product validation** — Call the `produtos` service before adding items to an order. This is the cleanest approach for a microservice boundary — it avoids cross-database queries while still enforcing correctness.

5. **JWT-based customer identity** — Extract `customer_id` from the JWT token in the `Authorization` header. Never trust a `customer_id` passed in request body or query params.

### Next Steps:
1. Create `servicos/pedidos/` directory structure
2. Write migration SQL (schema + enum type)
3. Implement `models.rs`, `schema.rs`, `handlers.rs`, `router.rs`
4. Add workspace entry in root `Cargo.toml`
5. Update `docker-compose.yml` with pedidos containers
6. Update `.env.example` with pedidos variables
