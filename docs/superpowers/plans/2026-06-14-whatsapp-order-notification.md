# WhatsApp Order Notification — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Send a WhatsApp message to the company seller via Evolution API whenever a new order is placed, containing customer (mocked), items, and total.

**Architecture:** Notification is fire-and-forget — spawned as a `tokio::spawn` task inside `service::create_order` after the DB transaction commits, so order creation always succeeds regardless of notification outcome. `ValidatedItem` is extended with `nome` (already available from the produtos API) to populate item names in the message. `AppState` gains four Evolution API fields read from env vars at startup.

**Tech Stack:** Rust (tokio, reqwest, chrono), Evolution API v2 (Docker), existing `pedidos` crate.

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `servicos/pedidos/src/produto_client.rs` | Modify | Add `nome: String` to `ProdutoDto` and `ValidatedItem`; derive `Clone` on `ValidatedItem` |
| `servicos/pedidos/src/notificacao.rs` | Create | `ClienteInfo`, `ClientesMock`, `build_message`, `notify_order` |
| `servicos/pedidos/src/models.rs` | Modify | Add 4 Evolution API fields to `AppState` |
| `servicos/pedidos/src/main.rs` | Modify | Read 4 new env vars, populate `AppState` |
| `servicos/pedidos/src/lib.rs` | Modify | Declare `pub mod notificacao` |
| `servicos/pedidos/src/service.rs` | Modify | Accept `Arc<AppState>` in `create_order`; spawn notification task after commit |
| `servicos/pedidos/src/handlers.rs` | Modify | Pass `Arc::clone(&state)` to `create_order` |
| `docker-compose.yml` | Modify | Add `evolution-api` service + volume; add 4 env vars to `pedidos-api` |

---

## Task 1: Extend `ValidatedItem` with `nome`

The produtos API already returns `nome` in the product JSON. We just aren't deserialising it. Add it to `ProdutoDto` and surface it in `ValidatedItem` so the notification has product names without an extra HTTP round-trip.

**Files:**
- Modify: `servicos/pedidos/src/produto_client.rs`

- [ ] **Step 1: Add `nome` to `ProdutoDto` and `ValidatedItem`, derive `Clone`**

Replace the current `ProdutoDto` and `ValidatedItem` definitions:

```rust
/// Successful validation result for one item
#[derive(Clone)]
pub struct ValidatedItem {
    pub id_product: i32,
    pub nome: String,
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
    nome: String,
    #[serde(rename = "VLR_VENDA1")]
    valor: Decimal,
    estoque: i32,
}
```

- [ ] **Step 2: Propagate `nome` into the `ValidatedItem` constructor**

Find the `Ok(ValidatedItem { ... })` block (around line 63) and add `nome`:

```rust
Ok(ValidatedItem {
    id_product,
    nome: p.nome,
    quantity,
    unit_price: p.valor,
})
```

- [ ] **Step 3: Build to verify no compile errors**

```bash
cargo build --package pedidos 2>&1 | grep -E "^error"
```

Expected: no output (zero errors).

- [ ] **Step 4: Commit**

```bash
git add servicos/pedidos/src/produto_client.rs
git commit -m "feat(pedidos): add nome field to ValidatedItem for notification use"
```

---

## Task 2: Create `notificacao.rs`

All notification logic lives here. `build_message` is a pure function and gets unit tests. `ClientesMock` is clearly marked for future replacement. `notify_order` is the public entry point called via `tokio::spawn`.

**Files:**
- Create: `servicos/pedidos/src/notificacao.rs`

- [ ] **Step 1: Write `servicos/pedidos/src/notificacao.rs`**

```rust
use std::sync::Arc;

use chrono::Local;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{models::AppState, produto_client::ValidatedItem};

// ---------------------------------------------------------------------------
// Customer data
// ---------------------------------------------------------------------------

pub struct ClienteInfo {
    pub nome: String,
    pub telefone: String,
}

// ---------------------------------------------------------------------------
// Mock clientes client
// ---------------------------------------------------------------------------

/// Stub for the future `clientes` microservice.
///
/// TODO: Replace this entire struct with a real HTTP client that calls
///       `GET {CLIENTES_SERVICE_URL}/clientes/{id}` and deserialises the
///       response into `ClienteInfo`. Remove MOCK_NOME and MOCK_TELEFONE
///       constants once the real integration is in place.
struct ClientesMock;

const MOCK_NOME: &str = "Cliente de Teste";
const MOCK_TELEFONE: &str = "5511999999999";

impl ClientesMock {
    async fn get(_id: Uuid) -> ClienteInfo {
        tracing::warn!("notificacao: using mock clientes client — replace with real service before production");
        ClienteInfo {
            nome: MOCK_NOME.to_string(),
            telefone: MOCK_TELEFONE.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Message formatting
// ---------------------------------------------------------------------------

/// Build the WhatsApp message text for a new order.
/// Pure function — no I/O, fully unit-testable.
pub fn build_message(order_id: i64, cliente: &ClienteInfo, items: &[ValidatedItem]) -> String {
    let itens_fmt: String = items
        .iter()
        .map(|i| {
            let subtotal = i.unit_price * Decimal::from(i.quantity);
            format!("• {}x {} — R$ {subtotal:.2}", i.quantity, i.nome)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let total: Decimal = items
        .iter()
        .fold(Decimal::ZERO, |acc, i| acc + i.unit_price * Decimal::from(i.quantity));

    let now = Local::now().format("%d/%m/%Y às %H:%M");

    format!(
        "🛒 *Novo Pedido #{order_id}*\n\n\
        *Cliente:* {nome}\n\
        *Telefone:* {telefone}\n\n\
        *Itens:*\n{itens_fmt}\n\n\
        *Total:* R$ {total:.2}\n\n\
        _Recebido em {now}_",
        nome = cliente.nome,
        telefone = cliente.telefone,
    )
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Fire-and-forget: fetch customer, build message, send via Evolution API.
/// All errors are logged; none propagate to the caller.
pub async fn notify_order(
    state: Arc<AppState>,
    customer_id: Uuid,
    order_id: i64,
    items: Vec<ValidatedItem>,
) {
    let cliente = ClientesMock::get(customer_id).await;
    let message = build_message(order_id, &cliente, &items);

    let url = format!(
        "{}/message/sendText/{}",
        state.evolution_url, state.evolution_instance
    );

    let body = serde_json::json!({
        "number": state.seller_whatsapp,
        "text": message,
    });

    match state
        .http
        .post(&url)
        .header("apikey", &state.evolution_key)
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(order_id, "WhatsApp notification sent successfully");
        }
        Ok(resp) => {
            tracing::error!(
                order_id,
                status = %resp.status(),
                "Evolution API returned non-success status"
            );
        }
        Err(e) => {
            tracing::error!(order_id, error = %e, "Failed to reach Evolution API");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn item(nome: &str, quantity: i32, unit_price: &str) -> ValidatedItem {
        ValidatedItem {
            id_product: 1,
            nome: nome.to_string(),
            quantity,
            unit_price: dec(unit_price),
        }
    }

    fn cliente(nome: &str, telefone: &str) -> ClienteInfo {
        ClienteInfo {
            nome: nome.to_string(),
            telefone: telefone.to_string(),
        }
    }

    #[test]
    fn build_message_contains_order_id() {
        let msg = build_message(42, &cliente("João", "5511999"), &[item("Prod A", 1, "10.00")]);
        assert!(msg.contains("Novo Pedido #42"), "missing order id in: {msg}");
    }

    #[test]
    fn build_message_contains_client_name_and_phone() {
        let msg = build_message(1, &cliente("Maria Silva", "5521888777"), &[item("X", 1, "1.00")]);
        assert!(msg.contains("Maria Silva"), "missing name in: {msg}");
        assert!(msg.contains("5521888777"), "missing phone in: {msg}");
    }

    #[test]
    fn build_message_computes_total_correctly() {
        // 2 × 10.00 + 3 × 5.00 = 35.00
        let items = vec![
            item("A", 2, "10.00"),
            item("B", 3, "5.00"),
        ];
        let msg = build_message(1, &cliente("T", "0"), &items);
        assert!(msg.contains("35.00"), "expected total 35.00 in: {msg}");
    }

    #[test]
    fn build_message_lists_all_item_names() {
        let items = vec![
            item("Alto Falante Hurricane", 1, "164.00"),
            item("Radio MP3", 2, "39.90"),
        ];
        let msg = build_message(1, &cliente("T", "0"), &items);
        assert!(msg.contains("Alto Falante Hurricane"), "missing item 1 in: {msg}");
        assert!(msg.contains("Radio MP3"), "missing item 2 in: {msg}");
    }

    #[test]
    fn build_message_per_line_shows_subtotal() {
        // 2 × 50.00 = 100.00 should appear on that line
        let msg = build_message(1, &cliente("T", "0"), &[item("Prod", 2, "50.00")]);
        assert!(msg.contains("100.00"), "expected line subtotal 100.00 in: {msg}");
    }
}
```

- [ ] **Step 2: Run the unit tests**

```bash
cargo test --package pedidos notificacao 2>&1 | tail -20
```

Expected:
```
test notificacao::tests::build_message_computes_total_correctly ... ok
test notificacao::tests::build_message_contains_client_name_and_phone ... ok
test notificacao::tests::build_message_contains_order_id ... ok
test notificacao::tests::build_message_lists_all_item_names ... ok
test notificacao::tests::build_message_per_line_shows_subtotal ... ok
test result: ok. 5 passed; 0 failed
```

- [ ] **Step 3: Commit**

```bash
git add servicos/pedidos/src/notificacao.rs
git commit -m "feat(pedidos): add notificacao module with mock clientes client and build_message"
```

---

## Task 3: Extend `AppState` and `main.rs`

Add four Evolution API fields to `AppState` and read them from env at startup.

**Files:**
- Modify: `servicos/pedidos/src/models.rs`
- Modify: `servicos/pedidos/src/main.rs`

- [ ] **Step 1: Add four fields to `AppState` in `models.rs`**

The current `AppState` ends after `jwt_secret`. Add four new fields:

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
}
```

- [ ] **Step 2: Read the four new env vars in `main.rs`**

The current `AppState { ... }` constructor in `main.rs` ends after `jwt_secret`. Extend it:

```rust
let evolution_url = std::env::var("EVOLUTION_API_URL")
    .into_diagnostic()
    .wrap_err("EVOLUTION_API_URL must be set")?;

let evolution_key = std::env::var("EVOLUTION_API_KEY")
    .into_diagnostic()
    .wrap_err("EVOLUTION_API_KEY must be set")?;

let evolution_instance = std::env::var("EVOLUTION_INSTANCE_NAME")
    .into_diagnostic()
    .wrap_err("EVOLUTION_INSTANCE_NAME must be set")?;

let seller_whatsapp = std::env::var("SELLER_WHATSAPP")
    .into_diagnostic()
    .wrap_err("SELLER_WHATSAPP must be set")?;

let state = Arc::new(AppState {
    db: pool,
    http,
    produtos_url,
    jwt_secret,
    evolution_url,
    evolution_key,
    evolution_instance,
    seller_whatsapp,
});
```

- [ ] **Step 3: Build to verify**

```bash
cargo build --package pedidos 2>&1 | grep -E "^error"
```

Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add servicos/pedidos/src/models.rs servicos/pedidos/src/main.rs
git commit -m "feat(pedidos): add Evolution API fields to AppState, read from env"
```

---

## Task 4: Declare `notificacao` module and wire the spawn

Register the new module in `lib.rs`, change `create_order` to accept `Arc<AppState>` (so the spawned task can own a reference), and spawn `notify_order` after the DB commit.

**Files:**
- Modify: `servicos/pedidos/src/lib.rs`
- Modify: `servicos/pedidos/src/service.rs`
- Modify: `servicos/pedidos/src/handlers.rs`

- [ ] **Step 1: Add `pub mod notificacao` to `lib.rs`**

```rust
pub mod auth;
pub mod handlers;
pub mod models;
pub mod notificacao;
pub mod produto_client;
pub mod router;
pub mod schema;
pub mod service;
```

- [ ] **Step 2: Change `create_order` signature in `service.rs` to accept `Arc<AppState>`**

The current signature is:
```rust
pub async fn create_order(
    state: &AppState,
    customer_id: Uuid,
    body: CreateOrderSchema,
) -> Result<CompleteOrder, AppError> {
```

Change to:
```rust
pub async fn create_order(
    state: Arc<AppState>,
    customer_id: Uuid,
    body: CreateOrderSchema,
) -> Result<CompleteOrder, AppError> {
```

Add the import at the top of `service.rs` if not already present:
```rust
use std::sync::Arc;
```

All existing uses of `state` inside the function (`state.db`, `state.http`, `state.produtos_url`) continue to work — `Arc<T>` derefs to `T`.

- [ ] **Step 3: Spawn `notify_order` in `service.rs` after `tx.commit()`**

Locate the line `tx.commit().await.map_err(AppError::DbError)?;` (around line 145) and add the spawn immediately after:

```rust
tx.commit().await.map_err(AppError::DbError)?;

// Notify seller via WhatsApp — fire-and-forget, order already committed
{
    let state_n = Arc::clone(&state);
    let validated_n = validated.clone();
    let order_id = order.id;
    tokio::spawn(async move {
        crate::notificacao::notify_order(state_n, customer_id, order_id, validated_n).await;
    });
}
```

- [ ] **Step 4: Update the call site in `handlers.rs`**

The handler currently calls:
```rust
let order = service::create_order(&state, customer_id, body).await?;
```

Change to (pass a clone of the Arc, not a borrow):
```rust
let order = service::create_order(Arc::clone(&state), customer_id, body).await?;
```

Add `use std::sync::Arc;` to the imports in `handlers.rs` if not already present.

- [ ] **Step 5: Build and run all tests**

```bash
cargo build --package pedidos 2>&1 | grep -E "^error"
cargo test --package pedidos 2>&1 | tail -20
```

Expected: zero build errors; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add servicos/pedidos/src/lib.rs servicos/pedidos/src/service.rs servicos/pedidos/src/handlers.rs
git commit -m "feat(pedidos): spawn WhatsApp notification on order creation"
```

---

## Task 5: Add Evolution API to `docker-compose.yml` and `.env`

**Files:**
- Modify: `docker-compose.yml`
- Modify: `.env` (gitignored — also update `.env.example` so the vars are documented)

- [ ] **Step 1: Add `evolution-api` service and `evolution_instances` volume to `docker-compose.yml`**

Add this service block before the `volumes:` section:

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

Add `evolution_instances:` to the top-level `volumes:` block:

```yaml
volumes:
  pg_produtos_data:
  pg_pedidos_data:
  produtos_static:
  evolution_instances:
```

- [ ] **Step 2: Add the four new env vars to `pedidos-api` in `docker-compose.yml`**

Find the `pedidos-api` service's `environment:` block and add:

```yaml
      EVOLUTION_API_URL: http://evolution-api:8080
      EVOLUTION_API_KEY: ${EVOLUTION_API_KEY}
      EVOLUTION_INSTANCE_NAME: ${EVOLUTION_INSTANCE_NAME}
      SELLER_WHATSAPP: ${SELLER_WHATSAPP}
```

- [ ] **Step 3: Add the new vars to `.env`**

Append to `.env`:

```
# Evolution API / WhatsApp notifications
EVOLUTION_API_KEY=change_me_in_production
EVOLUTION_INSTANCE_NAME=vendas
SELLER_WHATSAPP=5511999991234
```

Note: `EVOLUTION_API_URL` is not needed in `.env` because the compose file hardcodes it as `http://evolution-api:8080` (internal Docker network address).

- [ ] **Step 4: Update `.env.example` to document the new vars**

Add to `.env.example`:

```
# Evolution API / WhatsApp notifications
EVOLUTION_API_KEY=change_me
EVOLUTION_INSTANCE_NAME=vendas
SELLER_WHATSAPP=5511000000000
```

- [ ] **Step 5: Validate compose YAML**

```bash
docker compose config --quiet
```

Expected: no YAML parse errors (env-var warnings for unset vars are acceptable).

- [ ] **Step 6: Commit**

```bash
git add docker-compose.yml .env.example
git commit -m "feat(compose): add Evolution API service and WhatsApp env vars"
```

---

## Task 6: End-to-end smoke test

- [ ] **Step 1: Run the full test suite one final time**

```bash
cargo test --package pedidos 2>&1 | tail -20
```

Expected: all tests pass, zero failures.

- [ ] **Step 2: Verify compose config is valid**

```bash
docker compose config --quiet
```

Expected: no errors.

- [ ] **Step 3: Check the notification spawn compiles correctly**

```bash
cargo clippy --package pedidos -- -D warnings 2>&1 | grep -E "^error"
```

Expected: no output.

- [ ] **Step 4: Manual verification checklist (requires running stack)**

When the full stack is up (`docker compose up -d`):

1. Navigate to `http://localhost:8080` — Evolution API UI loads
2. Create instance named `vendas`, scan the QR code with the seller's phone
3. POST a new order via the pedidos API (with a valid JWT)
4. Check pedidos logs: `docker compose logs pedidos-api | grep -i whatsapp`  
   Expected: `WhatsApp notification sent successfully` or Evolution API error (if not yet connected)
5. Seller's phone receives the WhatsApp message with correct order details

- [ ] **Step 5: Final commit**

```bash
git add .
git commit -m "feat(pedidos): WhatsApp order notification — complete"
```
