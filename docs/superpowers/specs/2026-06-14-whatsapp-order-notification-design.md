# WhatsApp Order Notification — Design Spec

**Date:** 2026-06-14
**Status:** Approved
**Scope:** On order creation, send a WhatsApp message to the company seller containing order details (customer name, phone, items, total). Uses Evolution API (self-hosted). Notification is best-effort — order always succeeds regardless of notification outcome.

---

## Problem

When a client places an order, the seller has no automated way to be notified. The desired flow is:

> Cliente places order → WhatsApp message sent to seller → Seller processes order offline → Seller updates order status

Payment is handled off-site. This feature covers the notification leg only.

---

## Architecture

Three additions, all contained within the existing `pedidos` service and `docker-compose.yml`. No new microservice.

### 1. `servicos/pedidos/src/notificacao.rs` (new file)

Owns all notification logic:

- `ClienteInfo { nome: String, telefone: String }` — customer data
- `ClientesMock` — stub for the future `clientes` microservice, clearly marked:
  ```rust
  // TODO: replace with real ClientesClient HTTP call when clientes service is available
  ```
- `NotificacaoClient` — wraps the Evolution API HTTP call (uses the `reqwest::Client` already in `AppState`)
- `build_message(order_id, cliente, items) -> String` — pure function that formats the WhatsApp text
- `notify_order(state, customer_id, order_id, items)` — orchestrates: get cliente → build message → send; errors are logged and swallowed

### 2. Extension to `produto_client.rs`

`ValidatedItem` currently returns `{ id_product, quantity, unit_price }`. Add `nome: String` from the produtos API response (which already includes it) so notification has product names without an extra HTTP round-trip.

### 3. `evolution-api` Docker service

Added to `docker-compose.yml`. Runs Evolution API v2.2.3, exposes port 8080 internally. A named volume `evolution_instances` persists the WhatsApp session so the seller only needs to scan the QR code once.

---

## Notification Flow

Triggered inside `create_order` handler in `handlers.rs`, after the DB transaction commits:

```rust
tokio::spawn(async move {
    notificacao::notify_order(state, customer_id, order_id, items).await;
});
// 201 returned immediately — notification is fire-and-forget
```

Inside `notify_order`:
1. `ClientesMock::get(customer_id)` → `ClienteInfo { nome, telefone }`
2. `build_message(order_id, cliente, items)` → formatted WhatsApp string
3. `NotificacaoClient::send(&state, telefone, message)` → POST to Evolution API
4. Any error at any step: `tracing::error!(order_id, ...)` and return — order unaffected

---

## Message Format

```
🛒 *Novo Pedido #42*

*Cliente:* João Silva
*Telefone:* (11) 98765-4321

*Itens:*
• 2x Alto Falante 6" Hurricane Triak — R$ 67,90
• 1x Alto Falante Sub Woofer 12" Class — R$ 164,00

*Total:* R$ 299,80

_Recebido em 14/06/2026 às 15:32_
```

WhatsApp markdown: `*bold*`, `_italic_`, bullet `•`. Total is computed as sum of `quantity × unit_price` across all items.

---

## Evolution API Integration

```
POST {EVOLUTION_API_URL}/message/sendText/{EVOLUTION_INSTANCE_NAME}
Headers:
  apikey: {EVOLUTION_API_KEY}
  Content-Type: application/json
Body:
  { "number": "{SELLER_WHATSAPP}", "text": "..." }
```

Response: any 2xx is success. 4xx/5xx or network error → log error, do not retry.

---

## Docker Setup

**New service in `docker-compose.yml`:**

```yaml
evolution-api:
  image: atendai/evolution-api:v2.2.3
  container_name: mps-evolution-api
  restart: unless-stopped
  ports:
    - "8080:8080"
  environment:
    AUTHENTICATION_API_KEY: ${EVOLUTION_API_KEY}
  volumes:
    - evolution_instances:/evolution/instances
```

**New named volume:** `evolution_instances` — persists the WhatsApp QR session across container restarts.

**New env vars** (added to `.env` and `pedidos-api` service):

| Variable | Example | Description |
|---|---|---|
| `EVOLUTION_API_URL` | `http://evolution-api:8080` | Internal Evolution API base URL |
| `EVOLUTION_API_KEY` | `change_me` | API key set in Evolution API |
| `EVOLUTION_INSTANCE_NAME` | `vendas` | Instance name (created once, tied to QR scan) |
| `SELLER_WHATSAPP` | `5511999991234` | Seller phone in international format (no `+` or spaces) |

---

## AppState Changes

Four new fields added to `pedidos`'s `AppState`:

```rust
pub struct AppState {
    pub db: PgPool,                   // already exists
    pub http: reqwest::Client,        // already exists
    pub produtos_url: String,         // already exists
    pub jwt_secret: String,           // already exists
    pub evolution_url: String,        // new
    pub evolution_key: String,        // new
    pub evolution_instance: String,   // new
    pub seller_whatsapp: String,      // new
}
```

All four are read from env vars at startup in `main.rs`; startup panics if any is missing.

---

## Error Handling

Notification is completely isolated from the order response:

| Failure | Behaviour |
|---|---|
| `ClientesMock` | Always returns stub data; logs `warn` once noting the mock is active |
| Evolution API unreachable | `tracing::error!` with order id; order unaffected |
| Evolution API 4xx / 5xx | `tracing::error!` with status and body; order unaffected |
| Tokio task panic | Caught at task boundary; order unaffected |

No retries. Notifications are best-effort by design.

---

## Mock Clientes Client

```rust
// servicos/pedidos/src/notificacao.rs

/// Stub for the future `clientes` microservice.
///
/// TODO: Replace this entire struct with a real HTTP client that calls
///       `GET {CLIENTES_SERVICE_URL}/clientes/{id}` and deserialises the response
///       into `ClienteInfo`. Remove MOCK_NOME and MOCK_TELEFONE once integrated.
struct ClientesMock;

const MOCK_NOME: &str = "Cliente de Teste";
const MOCK_TELEFONE: &str = "5511999999999";

impl ClientesMock {
    async fn get(_id: uuid::Uuid) -> ClienteInfo {
        tracing::warn!("Using mock clientes client — replace with real service before production");
        ClienteInfo {
            nome: MOCK_NOME.to_string(),
            telefone: MOCK_TELEFONE.to_string(),
        }
    }
}
```

---

## Out of Scope

- Retry logic for failed notifications
- Notifications on status changes (only order creation is in scope)
- Customer-facing notifications (seller only)
- Building the real `clientes` microservice
- WhatsApp instance creation / QR scan automation (done manually via Evolution API UI on first deploy)
