# mps-backend API Guide

Complete reference for the mps-backend REST API. This document covers every public-facing endpoint, request/response schemas, authentication, and error handling—everything needed to build a frontend client.

---

## 1. Overview

Two microservices, each on its own port:

| Service  | Default Port | Purpose              |
|----------|-------------|----------------------|
| produtos | `3000`      | Product catalog + images |
| pedidos  | `3001`      | Order management (JWT-protected) |

### 1.1 Base URLs

Use environment-configured values. Defaults for local development:

```
PRODUTOS_BASE = http://localhost:3000
PEDIDOS_BASE  = http://localhost:3001
```

### 1.2 Response Envelope

Every successful response uses the `ApiResponse<T>` wrapper:

```json
{
  "status": "ok",
  "data": { ... },
  "message": null
}
```

The `status` field is one of:

| Value     | Used by                              |
|-----------|--------------------------------------|
| `"ok"`    | Standard success (GET, POST)         |
| `"success"` | DELETE and PATCH operations        |

`message` is always `null` on success.

### 1.3 Error Responses

All errors return a JSON body with `"status": "error"`:

```json
{
  "status": "error",
  "message": "Human-readable error description"
}
```

Validation errors (pedidos service) also include an `items` array:

```json
{
  "status": "error",
  "message": "Product validation failed",
  "items": [
    { "id_product": 5, "reason": "product not found" },
    { "id_product": 8, "reason": "insufficient stock: requested 10, available 3" }
  ]
}
```

### 1.4 HTTP Status Codes

| Code | Meaning              | When                                          |
|------|----------------------|-----------------------------------------------|
| 200  | OK                   | Standard success                              |
| 201  | Created              | Resource created (implicit via `Json` in axum) |
| 400  | Bad Request          | Malformed multipart upload                    |
| 401  | Unauthorized         | Missing/invalid/expired JWT                   |
| 404  | Not Found            | Resource not found                            |
| 409  | Conflict             | Duplicate key (e.g. product name+marca)       |
| 422  | Unprocessable Entity | Business rule violation (see message)         |
| 500  | Internal Server Error| Unexpected server error                       |

### 1.5 CORS

Both services allow only the configured `FRONTEND_URL` origin (default `http://localhost:5173`). Allowed methods: `GET`, `POST`, `PATCH`, `DELETE`. Allowed headers: `Content-Type`, `Authorization`.

### 1.6 Body Size Limits

Only `POST /api/products/{id}/imagens` has a 5 MB body limit. All other endpoints use axum's default (~2 MB).

---

## 2. Authentication (Pedidos Service Only)

All pedidos endpoints require an `Authorization: Bearer <token>` header.

### 2.1 JWT Structure

- **Algorithm:** HS256 (HMAC-SHA256)
- **Secret:** configured via `JWT_SECRET` env var
- **Validation:** `jsonwebtoken` library defaults, which include:
  - `exp` claim is validated (token must not be expired)
  - 60-second leeway for clock skew

### 2.2 Claims Payload

```json
{
  "customer_id": "550e8400-e29b-41d4-a716-446655440000",
  "exp": 1719000000
}
```

| Claim         | Type   | Required | Description                              |
|---------------|--------|----------|------------------------------------------|
| `customer_id` | UUIDv4 | Yes      | Identifies the customer (user)           |
| `exp`         | u64    | Yes      | Expiration timestamp (Unix seconds)      |

### 2.3 Generating a JWT (Development / Testing)

The pedidos service uses the same secret to both sign and verify tokens. There is no `/login` endpoint—tokens are expected to be pre-issued.

**Node.js (jsonwebtoken):**

```js
const jwt = require('jsonwebtoken');
const { v4: uuidv4 } = require('uuid');

const secret = process.env.JWT_SECRET || 'change_me';
const token = jwt.sign(
  { customer_id: uuidv4() },
  secret,
  { expiresIn: '24h' }
);
```

**Rust (jsonwebtoken crate):**

```rust
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
struct Claims { customer_id: Uuid, exp: u64 }

let claims = Claims {
    customer_id: Uuid::new_v4(),
    exp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 86400,
};
let token = encode(&Header::default(), &claims,
    &EncodingKey::from_secret(b"change_me")).unwrap();
```

**curl with a pre-generated token:**

```bash
TOKEN="eyJhbGciOiJIUzI1NiIs..."
curl -H "Authorization: Bearer $TOKEN" http://localhost:3001/api/pedidos
```

**Important:** The pedidos service enforces ownership—a customer can only access their own orders. The `customer_id` in the JWT must match `pedidos.customer_id` on every request.

---

## 3. Produtos Service (`:3000`)

All product endpoints are **public** (no authentication required).

### 3.1 `POST /api/products` — Create Product

**Request body** (JSON):

```json
{
  "nome":      "string (required)",
  "marca":     "string (required, max 20 chars)",
  "num_fab":   "string | null (optional)",
  "unidade":   "string (required, max 5 chars, e.g. \"PC\", \"KG\", \"UN\")",
  "valor":     "decimal (required, e.g. 19.99)",
  "descricao": "string | null (optional)",
  "estoque":   "integer (optional, defaults to 0)"
}
```

| Field      | Type              | Required | Constraints              |
|------------|-------------------|----------|--------------------------|
| `nome`     | string            | Yes      | —                        |
| `marca`    | string            | Yes      | Max 20 chars in DB       |
| `num_fab`  | string \| null    | No       | —                        |
| `unidade`  | string            | Yes      | Max 5 chars in DB        |
| `valor`    | decimal           | Yes      | Stored as DECIMAL(10,2)  |
| `descricao`| string \| null    | No       | —                        |
| `estoque`  | integer           | No       | Defaults to 0, must be ≥0 |

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "product": {
      "Idproduto": 42,
      "Descricao": "Auto Falante Pioneer TS-A1671F",
      "Marca": "Pioneer",
      "Num_fab": "TS-A1671F",
      "idunidade": "PC",
      "VLR_VENDA1": 199.90,
      "descricao": "6.5\" 3-way coaxial speaker, 320W max",
      "estoque": 15
    }
  }
}
```

**Product object fields** (note: serde-renamed to match legacy CSV format):

| JSON Key      | Type           | DB Column   | Description              |
|---------------|----------------|-------------|--------------------------|
| `Idproduto`   | integer        | `id`        | Primary key              |
| `Descricao`   | string         | `nome`      | Product name             |
| `Marca`       | string         | `marca`     | Brand                    |
| `Num_fab`     | string \| null | `num_fab`   | Manufacturer part number |
| `idunidade`   | string         | `unidade`   | Unit (PC, KG, UN, etc.)  |
| `VLR_VENDA1`  | decimal        | `valor`     | Price                    |
| `descricao`   | string \| null | `descricao` | Description              |
| `estoque`     | integer        | `estoque`   | Stock quantity           |

**Error responses:**

| Status | Body                                                     |
|--------|----------------------------------------------------------|
| 409    | `{"status":"error","message":"Product already exists."}` |

**curl:**

```bash
curl -X POST http://localhost:3000/api/products \
  -H "Content-Type: application/json" \
  -d '{
    "nome": "Cabo USB-C 2m",
    "marca": "Baseus",
    "num_fab": "CA-USB-C-2M",
    "unidade": "PC",
    "valor": 29.90,
    "descricao": "USB-C to USB-C, 100W PD",
    "estoque": 50
  }'
```

---

### 3.2 `GET /api/products/search` — Search Products

Fuzzy search across product names and brands using PostgreSQL trigram similarity.

**Query parameters:**

| Param   | Type    | Required | Description                              |
|---------|---------|----------|------------------------------------------|
| `q`     | string  | Yes      | Search term (accent-insensitive, case-insensitive) |
| `limit` | integer | No       | Max results to return                    |

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "products": [
      { ... },
      { ... }
    ]
  }
}
```

Results are ordered by similarity (most relevant first). The search normalizes input by removing diacritics, lowercasing, and collapsing whitespace—matching against `nome_norm` and `marca_norm` columns via the `%` operator (pg_trgm).

**curl:**

```bash
curl "http://localhost:3000/api/products/search?q=pioneer&limit=10"
```

---

### 3.3 `GET /api/products/{id}` — Get Product by ID

**Path parameters:**

| Param | Type    | Description   |
|-------|---------|---------------|
| `id`  | integer | Product ID    |

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "product": { ... }
  }
}
```

**Error:** 404 if not found.

**curl:**

```bash
curl http://localhost:3000/api/products/42
```

---

### 3.4 `PATCH /api/products/{id}` — Update Product

**Path parameters:**

| Param | Type    | Description   |
|-------|---------|---------------|
| `id`  | integer | Product ID    |

**Request body** (JSON, all fields optional):

```json
{
  "nome":      "string (optional)",
  "marca":     "string (optional)",
  "num_fab":   "string | null (optional — see note)",
  "unidade":   "string (optional)",
  "valor":     "decimal (optional)",
  "descricao": "string | null (optional — see note)",
  "estoque":   "integer (optional)"
}
```

**Tri-state nullable fields** (`num_fab`, `descricao`):

| JSON value      | Behavior                                         |
|-----------------|--------------------------------------------------|
| Field absent    | Keep existing value unchanged                    |
| `null`          | Clear the field (set to NULL in DB)              |
| `"some string"` | Update to the new value                          |

Example to clear description while updating price:

```json
{ "descricao": null, "valor": 19.90 }
```

**Success response** (200):

```json
{
  "status": "success",
  "data": {
    "product": { ... }
  }
}
```

**Error:** 404 if not found.

**curl:**

```bash
curl -X PATCH http://localhost:3000/api/products/42 \
  -H "Content-Type: application/json" \
  -d '{"estoque": 100, "valor": 179.90}'
```

---

### 3.5 `DELETE /api/products/{id}` — Delete Product

**Path parameters:**

| Param | Type    | Description   |
|-------|---------|---------------|
| `id`  | integer | Product ID    |

**Success response** (200):

```json
{
  "status": "success",
  "data": {
    "product": { ... }
  }
}
```

Returns the deleted product object. Cascade-deletes all associated images (both DB records and files on disk).

**Error:** 404 if not found.

**curl:**

```bash
curl -X DELETE http://localhost:3000/api/products/42
```

---

### 3.6 Images

#### `POST /api/products/{id}/imagens` — Upload Image

**Path parameters:**

| Param | Type    | Description   |
|-------|---------|---------------|
| `id`  | integer | Product ID    |

**Request body:** `multipart/form-data` with a single file field (any field name). Max size: 5 MB.

The file is saved to `{STATIC_DIR}/{uuid}.{ext}` and served at `/static/{uuid}.{ext}`.

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "image": {
      "id": 15,
      "id_produto": 42,
      "path": "a1b2c3d4-e5f6-7890-abcd-ef1234567890.jpg",
      "created_at": "2025-06-16T12:00:00Z"
    }
  }
}
```

**Image object:**

| Field        | Type     | Description                                  |
|--------------|----------|----------------------------------------------|
| `id`         | integer  | Image ID (BIGSERIAL)                         |
| `id_produto` | integer  | Parent product ID                            |
| `path`       | string   | Filename relative to `/static/`              |
| `created_at` | datetime | ISO 8601 timestamp with timezone             |

**Error:** 404 if product not found, 400 if multipart is malformed.

**curl:**

```bash
curl -X POST http://localhost:3000/api/products/42/imagens \
  -F "image=@/path/to/photo.jpg"
```

**Frontend URL construction:** `{PRODUTOS_BASE}/static/{image.path}`

Example: `http://localhost:3000/static/a1b2c3d4-e5f6-7890-abcd-ef1234567890.jpg`

---

#### `GET /api/products/{id}/imagens` — List Images

**Path parameters:**

| Param | Type    | Description   |
|-------|---------|---------------|
| `id`  | integer | Product ID    |

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "images": [
      {
        "id": 15,
        "id_produto": 42,
        "path": "a1b2c3d4-e5f6-7890-abcd-ef1234567890.jpg",
        "created_at": "2025-06-16T12:00:00Z"
      }
    ]
  }
}
```

Returns an empty array if no images exist (does **not** return 404).

**curl:**

```bash
curl http://localhost:3000/api/products/42/imagens
```

---

#### `DELETE /api/products/{id}/imagens/{img_id}` — Delete Image

**Path parameters:**

| Param    | Type    | Description   |
|----------|---------|---------------|
| `id`     | integer | Product ID    |
| `img_id` | integer | Image ID      |

**Success response** (200):

```json
{
  "status": "success",
  "data": {
    "image": { ... }
  }
}
```

Deletes both the DB record and the file on disk. Returns the deleted image object.

**Error:** 404 if image not found (either wrong `id` or `img_id` doesn't belong to `id`).

**curl:**

```bash
curl -X DELETE http://localhost:3000/api/products/42/imagens/15
```

---

### 3.7 Static Files

`GET /static/*` — Serves uploaded images and other static assets from the configured `STATIC_DIR` directory. No auth required. This is the path used by image `path` values.

```
GET /static/a1b2c3d4-e5f6-7890-abcd-ef1234567890.jpg
→ 200 with image/jpeg content
```

---

## 4. Pedidos Service (`:3001`)

All endpoints require `Authorization: Bearer <token>` header (see [Authentication](#2-authentication-pedidos-service-only)).

### 4.1 `POST /api/pedidos` — Create Order

**Auth:** Required

**Request body** (JSON):

```json
{
  "items": [
    { "id_product": 32, "quantity": 5 },
    { "id_product": 15, "quantity": 2 }
  ]
}
```

| Field             | Type               | Required | Description                 |
|-------------------|--------------------|----------|-----------------------------|
| `items`           | array              | Yes      | At least one item required  |
| `items[].id_product` | integer         | Yes      | Product ID from produtos service |
| `items[].quantity`   | integer         | Yes      | Must be > 0                 |

**Business logic:**

1. Validates each item by calling `GET {PRODUTOS_URL}/api/products/{id}` internally
2. Checks stock: `quantity <= product.estoque`
3. Inserts order + items in a DB transaction
4. On commit: fires a WhatsApp notification to seller (fire-and-forget)
5. The order's `unit_price` is locked to the product's price at time of creation

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "order": {
      "order": {
        "id": 7,
        "customer_id": "550e8400-e29b-41d4-a716-446655440000",
        "stat": "processando",
        "created_at": "2025-06-16T12:00:00Z",
        "updated_at": "2025-06-16T12:00:00Z"
      },
      "items": [
        {
          "id": 101,
          "id_order": 7,
          "id_product": 32,
          "quantity": 5,
          "unit_price": 29.90,
          "created_at": "2025-06-16T12:00:00Z"
        },
        {
          "id": 102,
          "id_order": 7,
          "id_product": 15,
          "quantity": 2,
          "unit_price": 15.00,
          "created_at": "2025-06-16T12:00:00Z"
        }
      ],
      "total": 179.50
    }
  }
}
```

**CompleteOrder object:**

| Field   | Type          | Description                                    |
|---------|---------------|------------------------------------------------|
| `order` | Order         | The order record                               |
| `items` | OrderItem[]   | All items in the order                         |
| `total` | decimal       | Sum of `quantity × unit_price` for all items   |

**Order object:**

| Field         | Type     | Description                               |
|---------------|----------|-------------------------------------------|
| `id`          | integer  | Order ID (BIGSERIAL)                      |
| `customer_id` | UUID     | Customer who owns this order              |
| `stat`        | string   | Status (see [Status Enum](#51-order-status-enum)) |
| `created_at`  | datetime | ISO 8601 with timezone                    |
| `updated_at`  | datetime | ISO 8601 with timezone                    |

**OrderItem object:**

| Field        | Type     | Description                          |
|--------------|----------|--------------------------------------|
| `id`         | integer  | Item ID (BIGSERIAL)                  |
| `id_order`   | integer  | Parent order ID                      |
| `id_product` | integer  | Product ID from produtos service     |
| `quantity`   | integer  | Quantity ordered                     |
| `unit_price` | decimal  | Price per unit at time of creation   |
| `created_at` | datetime | ISO 8601 with timezone               |

**Error responses:**

| Status | Body                                                                              |
|--------|-----------------------------------------------------------------------------------|
| 401    | `{"status":"error","message":"Unauthorized"}`                                     |
| 422    | `{"status":"error","message":"Order must have at least one item"}`                |
| 422    | `{"status":"error","message":"Product validation failed","items":[...]}`          |

Validation failure reasons include:
- `"product not found"` — product ID doesn't exist in produtos service
- `"insufficient stock: requested X, available Y"` — not enough inventory

**curl:**

```bash
curl -X POST http://localhost:3001/api/pedidos \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "items": [
      { "id_product": 32, "quantity": 5 },
      { "id_product": 15, "quantity": 2 }
    ]
  }'
```

---

### 4.2 `GET /api/pedidos` — List Orders

**Auth:** Required. Returns only the authenticated customer's orders.

**Query parameters:**

| Param         | Type   | Required | Description                                        |
|---------------|--------|----------|----------------------------------------------------|
| `status`      | string | No       | Filter by status (e.g. `"processando"`, `"confirmado"`) |
| `limit`       | integer| No       | Max results (default 50, capped at 200)            |

**Note:** The `customer_id` query parameter is accepted by the server but **ignored**—the JWT's `customer_id` is always used.

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "orders": [
      {
        "order": { ... },
        "items": [ ... ],
        "total": 179.50
      }
    ]
  }
}
```

Orders are sorted by `created_at DESC` (newest first). Returns empty array if no orders exist.

**Error:** 401 if missing/invalid JWT.

**curl:**

```bash
curl "http://localhost:3001/api/pedidos?status=processando&limit=10" \
  -H "Authorization: Bearer $TOKEN"
```

---

### 4.3 `GET /api/pedidos/{id}` — Get Order by ID

**Auth:** Required. Customer must own the order.

**Path parameters:**

| Param | Type    | Description |
|-------|---------|-------------|
| `id`  | integer | Order ID    |

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "order": {
      "order": { ... },
      "items": [ ... ],
      "total": 179.50
    }
  }
}
```

**Error responses:**

| Status | Body                                                     |
|--------|----------------------------------------------------------|
| 401    | `{"status":"error","message":"Unauthorized"}`            |
| 404    | `{"status":"error","message":"Order with ID 7 not found"}` |

**curl:**

```bash
curl http://localhost:3001/api/pedidos/7 \
  -H "Authorization: Bearer $TOKEN"
```

---

### 4.4 `PATCH /api/pedidos/{id}/status` — Update Order Status

**Auth:** Required. Customer must own the order.

**Path parameters:**

| Param | Type    | Description |
|-------|---------|-------------|
| `id`  | integer | Order ID    |

**Request body** (JSON):

```json
{
  "status": "confirmado"
}
```

**Valid status transitions:**

| From           | Allowed `To`                            |
|----------------|-----------------------------------------|
| `processando`  | `confirmado`, `cancelado`               |
| `confirmado`   | `enviado`, `rejeitado`, `cancelado`     |
| `enviado`      | `entregue`, `cancelado`                 |
| `rejeitado`    | `cancelado`                             |
| `entregue`     | *(none — terminal)*                     |
| `cancelado`    | *(none — terminal)*                     |

Uses **optimistic concurrency**: the update checks `WHERE stat = $current_status`. If another request changed the status concurrently, the request fails.

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "order": {
      "id": 7,
      "customer_id": "550e8400-e29b-41d4-a716-446655440000",
      "stat": "confirmado",
      "created_at": "2025-06-16T12:00:00Z",
      "updated_at": "2025-06-16T12:05:00Z"
    }
  }
}
```

Note: status update returns only the `Order` object (no items, no total).

**Error responses:**

| Status | Body                                                                                         |
|--------|----------------------------------------------------------------------------------------------|
| 401    | `{"status":"error","message":"Unauthorized"}`                                                |
| 404    | `{"status":"error","message":"Order with ID 7 not found"}`                                   |
| 422    | `{"status":"error","message":"Cannot transition from processando to entregue"}`              |
| 422    | `{"status":"error","message":"Order status changed concurrently, please retry"}`             |

**curl:**

```bash
curl -X PATCH http://localhost:3001/api/pedidos/7/status \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"status": "confirmado"}'
```

---

### 4.5 `PATCH /api/pedidos/{id}/items` — Update Order Items

**Auth:** Required. Customer must own the order. Order status must be `processando`.

**Path parameters:**

| Param | Type    | Description |
|-------|---------|-------------|
| `id`  | integer | Order ID    |

**Request body** (JSON):

```json
{
  "add": [
    { "id_product": 10, "quantity": 2 }
  ],
  "update": [
    { "id": 42, "quantity": 5 }
  ],
  "remove": [41, 43]
}
```

| Field      | Type          | Required | Description                                        |
|------------|---------------|----------|----------------------------------------------------|
| `add`      | array \| null | No       | New items to add (validated against produtos service) |
| `update`   | array \| null | No       | Items to change quantity (by item `id`, quantity must be > 0) |
| `remove`   | array \| null | No       | Item IDs to remove                                 |

**At least one** of `add`, `update`, or `remove` must be present.

**`add` item:**

| Field        | Type    | Required | Description    |
|--------------|---------|----------|----------------|
| `id_product` | integer | Yes      | Product ID     |
| `quantity`   | integer | Yes      | Must be > 0    |

**`update` item:**

| Field      | Type    | Required | Description               |
|------------|---------|----------|---------------------------|
| `id`       | integer | Yes      | OrderItem ID to update    |
| `quantity` | integer | Yes      | New quantity, must be > 0 |

**Business logic:**

1. Validates order exists, belongs to customer, and has `processando` status
2. Validates new items against produtos service (stock check)
3. All operations run in a single DB transaction
4. Returns the complete updated order

**Success response** (200):

```json
{
  "status": "ok",
  "data": {
    "order": {
      "order": { ... },
      "items": [ ... ],
      "total": 199.50
    }
  }
}
```

**Error responses:**

| Status | Body                                                                                              |
|--------|---------------------------------------------------------------------------------------------------|
| 401    | `{"status":"error","message":"Unauthorized"}`                                                     |
| 404    | `{"status":"error","message":"Order with ID 7 not found"}`                                        |
| 404    | `{"status":"error","message":"OrderItem with ID 43 not found"}`                                   |
| 422    | `{"status":"error","message":"Items can only be modified when order status is 'processando'"}`    |
| 422    | `{"status":"error","message":"At least one of add, update, or remove must be specified"}`         |
| 422    | `{"status":"error","message":"quantity must be positive, got 0"}`                                 |
| 422    | `{"status":"error","message":"Product validation failed","items":[...]}`                          |

**curl:**

```bash
curl -X PATCH http://localhost:3001/api/pedidos/7/items \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "add": [{ "id_product": 20, "quantity": 1 }],
    "update": [{ "id": 101, "quantity": 3 }],
    "remove": [102]
  }'
```

---

### 4.6 `DELETE /api/pedidos/{id}` — Delete Order

**Auth:** Required. Customer must own the order. Order status must be `processando` or `cancelado`.

**Path parameters:**

| Param | Type    | Description |
|-------|---------|-------------|
| `id`  | integer | Order ID    |

Uses optimistic concurrency: checks `WHERE stat = $current_status`.

**Success response** (200):

```json
{
  "status": "success",
  "data": {
    "order": {
      "id": 7,
      "customer_id": "550e8400-e29b-41d4-a716-446655440000",
      "stat": "cancelado",
      "created_at": "2025-06-16T12:00:00Z",
      "updated_at": "2025-06-16T12:00:00Z"
    }
  }
}
```

Returns the deleted `Order` object. Cascade-deletes all associated items.

**Error responses:**

| Status | Body                                                                                              |
|--------|---------------------------------------------------------------------------------------------------|
| 401    | `{"status":"error","message":"Unauthorized"}`                                                     |
| 404    | `{"status":"error","message":"Order with ID 7 not found"}`                                        |
| 422    | `{"status":"error","message":"Order can only be deleted when status is 'processando' or 'cancelado'"}` |
| 422    | `{"status":"error","message":"Order status changed concurrently, please retry"}`                  |

**curl:**

```bash
curl -X DELETE http://localhost:3001/api/pedidos/7 \
  -H "Authorization: Bearer $TOKEN"
```

---

## 5. Appendix

### 5.1 Order Status Enum

All status values are serialized **lowercase** in JSON:

| Value          | Rust Enum      | Meaning                                    | Final? |
|----------------|----------------|--------------------------------------------|--------|
| `"processando"` | `Processando`  | Order created, being processed             | No     |
| `"confirmado"`  | `Confirmado`   | Order confirmed by seller                  | No     |
| `"enviado"`     | `Enviado`      | Order shipped                              | No     |
| `"entregue"`    | `Entregue`     | Order delivered to customer                | Yes    |
| `"rejeitado"`   | `Rejeitado`    | Order rejected by seller                   | No     |
| `"cancelado"`   | `Cancelado`    | Order cancelled (by customer or system)    | Yes    |

### 5.2 Full Status Transition Diagram

```
                    ┌─────────────┐
                    │ processando │
                    └──┬───────┬──┘
              ┌────────┘       └─────────┐
              ▼                          ▼
       ┌────────────┐            ┌────────────┐
       │ confirmado │            │ cancelado  │ (terminal)
       └──┬──┬──┬───┘            └────────────┘
          │  │  │
    ┌─────┘  │  └─────┐
    ▼        ▼        ▼
┌────────┐ ┌──────────┐
│enviado │ │rejeitado │
└──┬──┬──┘ └────┬─────┘
   │  └─────────┼──────────┐
   ▼            ▼          ▼
┌──────────┐ ┌────────────┐
│ entregue │ │ cancelado  │ (terminal)
│(terminal)│ └────────────┘
└──────────┘
```

### 5.3 Environment Variables Reference

Not needed for frontend development, but useful for understanding behavior:

| Variable              | Service  | Purpose                              |
|-----------------------|----------|--------------------------------------|
| `JWT_SECRET`          | pedidos  | HMAC secret for JWT sign/verify      |
| `PRODUTOS_SERVICE_URL`| pedidos  | URL of produtos service for validation |
| `FRONTEND_URL`        | both     | CORS allowed origin                  |
| `STATIC_DIR`          | produtos | Directory for uploaded images        |

### 5.4 Quick Reference: All Endpoints

#### Produtos (port 3000, no auth)

| Method | Path                                      | Description          |
|--------|-------------------------------------------|----------------------|
| POST   | `/api/products`                           | Create product       |
| GET    | `/api/products/search?q=&limit=`          | Search products      |
| GET    | `/api/products/{id}`                      | Get product by ID    |
| PATCH  | `/api/products/{id}`                      | Update product       |
| DELETE | `/api/products/{id}`                      | Delete product       |
| POST   | `/api/products/{id}/imagens`              | Upload image (5 MB max) |
| GET    | `/api/products/{id}/imagens`              | List product images  |
| DELETE | `/api/products/{id}/imagens/{img_id}`     | Delete image         |
| GET    | `/static/*`                               | Serve static files   |

#### Pedidos (port 3001, JWT required)

| Method | Path                          | Description              |
|--------|-------------------------------|--------------------------|
| POST   | `/api/pedidos`                | Create order             |
| GET    | `/api/pedidos?status=&limit=` | List customer's orders   |
| GET    | `/api/pedidos/{id}`           | Get order by ID          |
| PATCH  | `/api/pedidos/{id}/status`    | Update order status      |
| PATCH  | `/api/pedidos/{id}/items`     | Add/update/remove items  |
| DELETE | `/api/pedidos/{id}`           | Delete order             |
