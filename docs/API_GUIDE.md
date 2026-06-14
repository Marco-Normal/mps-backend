# MPS Backend — Complete API Guide

> Written for front-end agents and developers. Covers every endpoint, request shape, success response, every failure mode, and all behavioural gotchas.

---

## Table of Contents

1. [Services & Base URLs](#services--base-urls)
2. [Authentication](#authentication)
3. [Response Envelope](#response-envelope)
4. [Error Responses](#error-responses)
5. [Produtos Service — Product Catalog](#produtos-service--product-catalog)
   - [POST /api/products](#post-apiproducts)
   - [GET /api/products/search](#get-apiproductssearch)
   - [GET /api/products/:id](#get-apiproductsid)
   - [PATCH /api/products/:id](#patch-apiproductsid)
   - [DELETE /api/products/:id](#delete-apiproductsid)
   - [POST /api/products/:id/imagens](#post-apiproductsimagensid)
   - [GET /api/products/:id/imagens](#get-apiproductsimagensid)
   - [DELETE /api/products/:id/imagens/:img_id](#delete-apiproductsimagensidimg_id)
6. [Pedidos Service — Orders](#pedidos-service--orders)
   - [POST /api/pedidos](#post-apipedidos)
   - [GET /api/pedidos](#get-apipedidos)
   - [GET /api/pedidos/:id](#get-apipedidosid)
   - [PATCH /api/pedidos/:id/status](#patch-apipedidosidstatus)
   - [PATCH /api/pedidos/:id/items](#patch-apipedidosiditems)
   - [DELETE /api/pedidos/:id](#delete-apipedidosid)
7. [Order Status State Machine](#order-status-state-machine)
8. [Data Models Reference](#data-models-reference)
9. [Known Gaps & Front-End Notes](#known-gaps--front-end-notes)

---

## Services & Base URLs

| Service | Default Port | Docker container | Purpose |
|---|---|---|---|
| `produtos-api` | `3000` | `mps-produtos-api` | Product catalog — browse, search, manage products |
| `pedidos-api` | `3001` | `mps-pedidos-api` | Orders — create, track, and manage customer orders |

All endpoints are relative to the service root, e.g.:
- `http://localhost:3000/api/products`
- `http://localhost:3001/api/pedidos`

---

## Authentication

**Produtos service:** No authentication on any endpoint. All product endpoints are public.

**Pedidos service:** Every endpoint requires a JWT Bearer token.

```
Authorization: Bearer <jwt_token>
```

### JWT Structure

**Algorithm:** HS256

**Claims:**
```json
{
  "customer_id": "550e8400-e29b-41d4-a716-446655440000",
  "exp": 1718400000
}
```

- `customer_id` — UUID identifying the customer. All orders are scoped to this ID.
- `exp` — Unix timestamp (seconds). The token is rejected if expired.

### Auth Failures

If the `Authorization` header is missing, malformed, uses the wrong secret, or the token is expired, **every** pedidos endpoint returns:

```json
HTTP 401 Unauthorized

{
  "status": "error",
  "message": "Unauthorized"
}
```

---

## Response Envelope

All **success** responses are wrapped in a consistent envelope:

```json
{
  "status": "ok",
  "data": { ... },
  "message": null
}
```

- `status` is `"ok"` for reads and creates; `"success"` for updates and deletes.
- `data` holds the payload (see each endpoint for the exact shape).
- `message` is always `null` on success.

**Example — get product:**
```json
{
  "status": "ok",
  "data": {
    "product": { ... }
  },
  "message": null
}
```

---

## Error Responses

All error responses are **flat** (no `data` wrapper):

```json
{
  "status": "error",
  "message": "Human-readable description"
}
```

One special case — product validation failure — adds an `items` array:

```json
{
  "status": "error",
  "message": "Product validation failed",
  "items": [
    { "id_product": 42, "reason": "insufficient stock (requested 5, available 2)" },
    { "id_product": 99, "reason": "product not found" }
  ]
}
```

### HTTP Status Code Reference

| Status | When |
|---|---|
| `200 OK` | Successful GET or search |
| `201 Created` | Not used — creates also return 200 |
| `401 Unauthorized` | Missing/invalid/expired JWT (pedidos only) |
| `404 Not Found` | Resource does not exist |
| `409 Conflict` | Attempt to create a duplicate resource |
| `422 Unprocessable Entity` | Invalid business logic (bad state transition, empty order, insufficient stock, etc.) |
| `500 Internal Server Error` | Database error or unexpected server failure |

---

## Produtos Service — Product Catalog

Base URL: `http://localhost:3000`

---

### POST /api/products

Create a new product.

**Auth:** None

**Request body (JSON):**

```json
{
  "nome": "ALTO FALANTE 6\" TRIAK TRIAXIAL 4 OHMS",
  "marca": "Hurricane",
  "num_fab": "F01.201",
  "unidade": "PAR",
  "valor": 67.90,
  "descricao": "Alto falante triaxial de 6 polegadas, 4 ohms, 120W RMS.",
  "estoque": 255
}
```

| Field | Type | Required | Notes |
|---|---|---|---|
| `nome` | string | Yes | Product display name |
| `marca` | string | Yes | Brand name |
| `num_fab` | string | No | Manufacturer part number |
| `unidade` | string | Yes | Unit of measure — e.g. `"PC"`, `"PAR"`, `"KT"` |
| `valor` | decimal | Yes | Sale price |
| `descricao` | string | No | Free-text description |
| `estoque` | integer | No | Stock quantity, defaults to `0` if omitted |

**Success — 200 OK:**

```json
{
  "status": "ok",
  "data": {
    "product": {
      "Idproduto": 108,
      "Descricao": "ALTO FALANTE 6\" TRIAK TRIAXIAL 4 OHMS",
      "Marca": "Hurricane",
      "Num_fab": "F01.201",
      "idunidade": "PAR",
      "VLR_VENDA1": "67.90",
      "descricao": "Alto falante triaxial de 6 polegadas...",
      "estoque": 255
    }
  },
  "message": null
}
```

> **Note:** The `Product` object uses legacy field names from the CSV import. See [Data Models Reference](#data-models-reference).

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `409` | `"Product already exists."` | Duplicate key violation |
| `500` | `"Internal server error"` | Database error |

---

### GET /api/products/search

Fuzzy full-text search across product names and brands using PostgreSQL trigram similarity.

**Auth:** None

**Query parameters:**

| Param | Type | Required | Notes |
|---|---|---|---|
| `q` | string | Yes | Search query. Normalised (NFD, lowercase, ASCII). |
| `limit` | integer | No | Max results to return. No default cap if omitted. |

**Example:**
```
GET /api/products/search?q=hurricane+triak&limit=10
```

**Success — 200 OK:**

```json
{
  "status": "ok",
  "data": {
    "products": [
      {
        "Idproduto": 108,
        "Descricao": "ALTO FALANTE 6 TRIAK TRIAXIAL 4 OHMS",
        "Marca": "Hurricane",
        "Num_fab": "F01.201",
        "idunidade": "PAR",
        "VLR_VENDA1": "67.90",
        "descricao": null,
        "estoque": 255
      }
    ]
  },
  "message": null
}
```

Returns an empty array `[]` if no products match — never a 404.

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `500` | `"Internal server error"` | Database error |

---

### GET /api/products/:id

Fetch a single product by its integer ID.

**Auth:** None

**Path param:** `id` — integer product ID

**Success — 200 OK:**

```json
{
  "status": "ok",
  "data": {
    "product": {
      "Idproduto": 108,
      "Descricao": "ALTO FALANTE 6 TRIAK TRIAXIAL 4 OHMS",
      "Marca": "Hurricane",
      "Num_fab": "F01.201",
      "idunidade": "PAR",
      "VLR_VENDA1": "67.90",
      "descricao": null,
      "estoque": 255
    }
  },
  "message": null
}
```

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `404` | `"Product with ID {id} not found"` | No product with that ID |
| `500` | `"Internal server error"` | Database error |

---

### PATCH /api/products/:id

Partially update a product. All fields are optional — omitted fields keep their current values.

**Auth:** None

**Path param:** `id` — integer product ID

**Request body (JSON) — all fields optional:**

```json
{
  "nome": "ALTO FALANTE 6 TRIAK TRIAXIAL 4 OHMS UPDATED",
  "marca": "Hurricane",
  "num_fab": "F01.201",
  "unidade": "PAR",
  "valor": 72.50,
  "descricao": "Nova descrição aqui.",
  "estoque": 200
}
```

> **Gotcha:** `descricao` cannot be cleared once set. Sending `null` is treated the same as omitting it — the existing value is preserved. There is currently no way to set `descricao` back to `null` via this endpoint.

**Success — 200 OK:**

Returns the updated product in the same envelope shape as `GET /api/products/:id`.

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `404` | `"Product with ID {id} not found"` | No product with that ID |
| `500` | `"Internal server error"` | Database error |

---

### DELETE /api/products/:id

Permanently delete a product. Cascades to its images in `imagens_produto`.

**Auth:** None

**Path param:** `id` — integer product ID

**Success — 200 OK:**

```json
{
  "status": "success",
  "data": {
    "product": { ... }
  },
  "message": null
}
```

Returns the deleted product.

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `404` | `"Product with ID {id} not found"` | No product with that ID |
| `500` | `"Internal server error"` | Database error |

---

### POST /api/products/:id/imagens

Upload an image for a product. Max file size: **5 MB**.

**Auth:** None

**Path param:** `id` — integer product ID

**Request:** `multipart/form-data` with a single file field (any field name).

The file extension is taken from the original filename if present.

**Success — 200 OK:**

```json
{
  "status": "ok",
  "data": {
    "image": {
      "id": 1,
      "id_produto": 108,
      "path": "3f2e1a4b-dead-beef-cafe-112233445566.jpg",
      "created_at": "2026-06-14T15:32:00Z"
    }
  },
  "message": null
}
```

> **Important:** `path` is just the UUID filename, **not** a full URL. See [Known Gaps](#known-gaps--front-end-notes) for how to construct the image URL.

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `404` | `"Product with ID {id} not found"` | Product does not exist |
| `422` | `"Multipart error: ..."` | No file field provided, or malformed multipart |
| `500` | `"Internal server error"` | File write failure or DB error |

---

### GET /api/products/:id/imagens

List all images for a product.

**Auth:** None

**Path param:** `id` — integer product ID

**Success — 200 OK:**

```json
{
  "status": "ok",
  "data": {
    "images": [
      {
        "id": 1,
        "id_produto": 108,
        "path": "3f2e1a4b-dead-beef-cafe-112233445566.jpg",
        "created_at": "2026-06-14T15:32:00Z"
      }
    ]
  },
  "message": null
}
```

Returns an empty array if the product has no images. Does **not** 404 if the product ID doesn't exist — it returns `[]`.

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `500` | `"Internal server error"` | Database error |

---

### DELETE /api/products/:id/imagens/:img_id

Delete one image by its ID. Also removes the file from disk.

**Auth:** None

**Path params:**
- `id` — integer product ID
- `img_id` — integer image ID (from `imagens_produto.id`)

**Success — 200 OK:**

```json
{
  "status": "success",
  "data": {
    "image": {
      "id": 1,
      "id_produto": 108,
      "path": "3f2e1a4b-dead-beef-cafe-112233445566.jpg",
      "created_at": "2026-06-14T15:32:00Z"
    }
  },
  "message": null
}
```

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `404` | `"ProductImage with ID {img_id} not found"` | Image doesn't exist or belongs to a different product |
| `500` | `"Internal server error"` | Database error |

---

## Pedidos Service — Orders

Base URL: `http://localhost:3001`

**All endpoints require `Authorization: Bearer <jwt_token>`.**

---

### POST /api/pedidos

Create a new order. Validates each item against the `produtos` service (checks existence and stock). On success, automatically sends a WhatsApp notification to the seller.

**Auth:** JWT required

**Request body (JSON):**

```json
{
  "items": [
    { "id_product": 108, "quantity": 2 },
    { "id_product": 111, "quantity": 1 }
  ]
}
```

| Field | Type | Required | Notes |
|---|---|---|---|
| `items` | array | Yes | Must have at least one item |
| `items[].id_product` | integer | Yes | Product ID from the produtos service |
| `items[].quantity` | integer | Yes | Must be > 0 and ≤ available stock |

**Success — 200 OK:**

```json
{
  "status": "ok",
  "data": {
    "order": {
      "order": {
        "id": 42,
        "customer_id": "550e8400-e29b-41d4-a716-446655440000",
        "stat": "processando",
        "created_at": "2026-06-14T15:32:00Z",
        "updated_at": "2026-06-14T15:32:00Z"
      },
      "items": [
        {
          "id": 1,
          "id_order": 42,
          "id_product": 108,
          "quantity": 2,
          "unit_price": "67.90",
          "created_at": "2026-06-14T15:32:00Z"
        },
        {
          "id": 2,
          "id_order": 42,
          "id_product": 111,
          "quantity": 1,
          "unit_price": "115.00",
          "created_at": "2026-06-14T15:32:00Z"
        }
      ],
      "total": "250.80"
    }
  },
  "message": null
}
```

> **Note:** `unit_price` is snapshotted at the time of order creation. Future price changes do not affect existing orders.

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `401` | `"Unauthorized"` | Missing/invalid/expired JWT |
| `422` | `"Order must have at least one item"` | Empty `items` array |
| `422` | `"Product validation failed"` + `items` array | One or more products not found or insufficient stock |
| `500` | `"Internal server error"` | Database error or produtos service unreachable |

**Validation failure example:**
```json
{
  "status": "error",
  "message": "Product validation failed",
  "items": [
    { "id_product": 999, "reason": "product not found" },
    { "id_product": 108, "reason": "insufficient stock (requested 500, available 255)" }
  ]
}
```

---

### GET /api/pedidos

List all orders for the authenticated customer, newest first.

**Auth:** JWT required

**Query parameters:**

| Param | Type | Required | Notes |
|---|---|---|---|
| `status` | string | No | Filter by order status. One of: `processando`, `confirmado`, `enviado`, `entregue`, `cancelado`, `rejeitado` |
| `limit` | integer | No | Max results. Default `50`, max `200`. |
| `customer_id` | UUID | No | **Ignored for security.** Orders are always scoped to the JWT's `customer_id`. |

**Success — 200 OK:**

```json
{
  "status": "ok",
  "data": {
    "orders": [
      {
        "id": 42,
        "customer_id": "550e8400-e29b-41d4-a716-446655440000",
        "stat": "processando",
        "created_at": "2026-06-14T15:32:00Z",
        "updated_at": "2026-06-14T15:32:00Z"
      }
    ]
  },
  "message": null
}
```

Returns an empty array if no matching orders. Note: this returns `Order` objects only (no items/total). Use `GET /api/pedidos/:id` for full detail.

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `401` | `"Unauthorized"` | Missing/invalid/expired JWT |
| `500` | `"Internal server error"` | Database error |

---

### GET /api/pedidos/:id

Fetch a single order with all its items and computed total.

**Auth:** JWT required

**Path param:** `id` — integer order ID (i64)

**Success — 200 OK:**

```json
{
  "status": "ok",
  "data": {
    "order": {
      "order": {
        "id": 42,
        "customer_id": "550e8400-e29b-41d4-a716-446655440000",
        "stat": "processando",
        "created_at": "2026-06-14T15:32:00Z",
        "updated_at": "2026-06-14T15:32:00Z"
      },
      "items": [
        {
          "id": 1,
          "id_order": 42,
          "id_product": 108,
          "quantity": 2,
          "unit_price": "67.90",
          "created_at": "2026-06-14T15:32:00Z"
        }
      ],
      "total": "135.80"
    }
  },
  "message": null
}
```

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `401` | `"Unauthorized"` | Missing/invalid/expired JWT |
| `404` | `"Order with ID {id} not found"` | Order doesn't exist |
| `401` | `"Unauthorized"` | Order exists but belongs to a different customer |
| `500` | `"Internal server error"` | Database error |

> **IDOR protection:** If the order exists but belongs to a different customer, returns `401`, not `404`. This prevents enumeration attacks.

---

### PATCH /api/pedidos/:id/status

Transition an order to a new status. Only specific transitions are allowed — see the [state machine](#order-status-state-machine).

**Auth:** JWT required

**Path param:** `id` — integer order ID

**Request body (JSON):**

```json
{
  "status": "confirmado"
}
```

Valid values: `confirmado`, `enviado`, `entregue`, `cancelado`, `rejeitado`

**Success — 200 OK:**

```json
{
  "status": "ok",
  "data": {
    "order": {
      "id": 42,
      "customer_id": "550e8400-e29b-41d4-a716-446655440000",
      "stat": "confirmado",
      "created_at": "2026-06-14T15:32:00Z",
      "updated_at": "2026-06-14T16:00:00Z"
    }
  },
  "message": null
}
```

Returns the `Order` object (not `CompleteOrder`) — no items or total.

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `401` | `"Unauthorized"` | Missing/invalid/expired JWT, or order belongs to another customer |
| `404` | `"Order with ID {id} not found"` | Order doesn't exist |
| `422` | `"Cannot transition from {x} to {y}"` | Invalid status transition |
| `422` | `"Order status changed concurrently, please retry"` | Optimistic concurrency conflict (race condition) |
| `500` | `"Internal server error"` | Database error |

---

### PATCH /api/pedidos/:id/items

Add, update, or remove items from an order. Only allowed while status is `processando`.

**Auth:** JWT required

**Path param:** `id` — integer order ID

**Request body (JSON) — all fields optional, but at least one must be provided:**

```json
{
  "add": [
    { "id_product": 110, "quantity": 3 }
  ],
  "update": [
    { "id": 1, "quantity": 5 }
  ],
  "remove": [2, 7]
}
```

| Field | Type | Notes |
|---|---|---|
| `add` | array | New items to add. Validated against produtos service (stock + existence). |
| `add[].id_product` | integer | Product ID |
| `add[].quantity` | integer | Must be > 0 and ≤ stock |
| `update` | array | Change quantity of existing order items (by `items_pedidos.id`, **not** `id_product`). |
| `update[].id` | integer | `items_pedidos.id` (from the `items` array in GET /pedidos/:id) |
| `update[].quantity` | integer | Must be > 0 |
| `remove` | array of integers | `items_pedidos.id` values to delete |

**Success — 200 OK:**

Returns the full `CompleteOrder` (same shape as `POST /api/pedidos`).

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `401` | `"Unauthorized"` | Missing/invalid/expired JWT, or wrong customer |
| `404` | `"Order with ID {id} not found"` | Order doesn't exist |
| `422` | `"Items can only be modified when order status is 'processando'"` | Order already confirmed/sent/etc. |
| `422` | `"At least one of add, update, or remove must be specified"` | Empty body |
| `422` | `"quantity must be positive, got {n}"` | A quantity in `update` is ≤ 0 |
| `422` | `"Product validation failed"` + `items` array | Added product not found or out of stock |
| `404` | `"OrderItem with ID {id} not found"` | Item ID in `update` or `remove` doesn't belong to this order |
| `500` | `"Internal server error"` | Database error |

---

### DELETE /api/pedidos/:id

Permanently delete an order. Only allowed when status is `processando` or `cancelado`.

**Auth:** JWT required

**Path param:** `id` — integer order ID

**Success — 200 OK:**

```json
{
  "status": "success",
  "data": {
    "order": {
      "id": 42,
      "customer_id": "550e8400-e29b-41d4-a716-446655440000",
      "stat": "processando",
      "created_at": "2026-06-14T15:32:00Z",
      "updated_at": "2026-06-14T15:32:00Z"
    }
  },
  "message": null
}
```

Returns the deleted order (items are not included).

**Possible failures:**

| Status | Message | Cause |
|---|---|---|
| `401` | `"Unauthorized"` | Missing/invalid/expired JWT, or wrong customer |
| `404` | `"Order with ID {id} not found"` | Order doesn't exist |
| `422` | `"Order can only be deleted when status is 'processando' or 'cancelado'"` | Order is confirmed, sent, etc. |
| `422` | `"Order status changed concurrently, please retry"` | Race condition — retry immediately |
| `500` | `"Internal server error"` | Database error |

---

## Order Status State Machine

All valid transitions:

```
processando ──► confirmado
processando ──► cancelado

confirmado  ──► enviado
confirmado  ──► rejeitado
confirmado  ──► cancelado

enviado     ──► entregue
enviado     ──► cancelado

rejeitado   ──► cancelado
```

**Terminal states** (no further transitions possible): `entregue`, `cancelado`

**Business rules enforced:**
- Items (`PATCH /items`) can only be modified while status is `processando`
- Orders can only be deleted while status is `processando` or `cancelado`
- Any other transition attempt returns `422 Unprocessable Entity`

---

## Data Models Reference

### Product (JSON representation)

The `Product` model uses legacy CSV field names for backwards compatibility:

| JSON key | Rust field | Type | Notes |
|---|---|---|---|
| `Idproduto` | `id` | integer | Primary key |
| `Descricao` | `nome` | string | Product display name |
| `Marca` | `marca` | string | Brand name |
| `Num_fab` | `num_fab` | string \| null | Manufacturer part number |
| `idunidade` | `unidade` | string | Unit of measure (`"PC"`, `"PAR"`, `"KT"`, etc.) |
| `VLR_VENDA1` | `valor` | decimal string | Sale price, serialised as a string decimal |
| `descricao` | `descricao` | string \| null | Free-text description |
| `estoque` | `estoque` | integer | Available stock |

### ProductImage

| JSON key | Type | Notes |
|---|---|---|
| `id` | integer (i64) | Primary key |
| `id_produto` | integer | FK to product |
| `path` | string | UUID filename only (see [Known Gaps](#known-gaps--front-end-notes)) |
| `created_at` | ISO 8601 UTC | Upload timestamp |

### Order

| JSON key | Type | Notes |
|---|---|---|
| `id` | integer (i64) | Primary key |
| `customer_id` | UUID string | From JWT claims |
| `stat` | string | One of the status values below |
| `created_at` | ISO 8601 UTC | |
| `updated_at` | ISO 8601 UTC | |

### OrderItem

| JSON key | Type | Notes |
|---|---|---|
| `id` | integer (i64) | Primary key — use this for `update[]` and `remove[]` in PATCH /items |
| `id_order` | integer (i64) | FK to order |
| `id_product` | integer | FK to product (cross-service, no enforced FK) |
| `quantity` | integer | |
| `unit_price` | decimal string | Snapshotted at order creation |
| `created_at` | ISO 8601 UTC | |

### CompleteOrder

Returned by `POST /api/pedidos`, `GET /api/pedidos/:id`, and `PATCH /api/pedidos/:id/items`:

```json
{
  "order": { Order },
  "items": [ OrderItem, ... ],
  "total": "135.80"
}
```

`total` is the sum of `quantity × unit_price` across all items.

### Order Status Values

| Value | Meaning |
|---|---|
| `processando` | Initial state — awaiting confirmation |
| `confirmado` | Seller confirmed |
| `enviado` | Shipped |
| `entregue` | Delivered |
| `cancelado` | Cancelled (terminal) |
| `rejeitado` | Rejected by seller (can still be cancelled) |

---

## Known Gaps & Front-End Notes

> **Note:** Gotchas #1 (image serving) and #5 (decimal serialization) described below have been resolved. The remaining items depend on the `clientes` microservice.

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

### 2. No customer registration/login endpoint

The `pedidos` service requires a JWT with a `customer_id` UUID, but there is **no `/auth/login` or `/auth/register` endpoint** in this backend. The `clientes` microservice (which will handle customer identity and login) does not exist yet.

For development/testing, JWTs must be minted manually:

```js
// Node.js example
const jwt = require('jsonwebtoken');
const token = jwt.sign(
  { customer_id: '550e8400-e29b-41d4-a716-446655440000' },
  process.env.JWT_SECRET,
  { expiresIn: '7d', algorithm: 'HS256' }
);
```

The `JWT_SECRET` env var is set in the backend's `.env` file.

### 3. Customer info in orders is UUID-only

`Order.customer_id` is a UUID. The back end does not store customer names, emails, or phone numbers. Those will come from the future `clientes` service.

### 4. WhatsApp notification is fire-and-forget

When an order is placed (`POST /api/pedidos`), a WhatsApp message is sent to the seller's phone via Evolution API. This notification:
- Does **not** affect the order creation response
- Does **not** retry on failure
- Currently uses a mock customer name/phone (`"Cliente de Teste"`, `"5511999999999"`) — real customer data integration is pending

### 5. Decimal values serialise as JSON numbers

`valor`, `unit_price`, and `total` are serialised as JSON **numbers** (not strings), because both services use `rust_decimal` with the `serde-float` feature. Standard `parseFloat` or `Number()` works fine for display. For financial calculations requiring exact precision, use a decimal library:

```js
import Decimal from 'decimal.js';
const price = new Decimal(product['VLR_VENDA1']); // safe
```

### 6. `GET /api/pedidos` returns `Order` not `CompleteOrder`

The list endpoint returns lightweight `Order` objects (no items, no total). To show cart contents or totals, fetch each order individually with `GET /api/pedidos/:id`.

### 7. Timestamps are UTC ISO 8601

All `created_at` / `updated_at` fields are UTC. Display in local time by converting in the front end.

### 8. `customer_id` query param in `GET /api/pedidos` is ignored

The `customer_id` query parameter is accepted but silently ignored. The JWT's `customer_id` always takes precedence. There is no admin endpoint that can list orders for an arbitrary customer.
