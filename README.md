# Quickstart Guide

How to get mps-backend running — databases, APIs, and seed data — from zero.

---

## 1. Prerequisites

| Tool       | Min Version | Check           |
|------------|-------------|-----------------|
| Docker     | 24+         | `docker --version` |
| Docker Compose | v2+     | `docker compose version` |
| Rust       | 1.85+ (edition 2024) | `rustc --version` |
| (optional) psql | 17   | `psql --version` |

---

## 2. Clone & Environment

```bash
git clone <repo-url> mps-backend
cd mps-backend
cp .env.example .env
```

Fill in `.env`. The minimal required variables:

```bash
# ── Produtos database (port 5432) ──────────────────────
PRODUTOS_POSTGRES_USER=admin
PRODUTOS_POSTGRES_PASSWORD=admin
PRODUTOS_DB_NAME=mps_produtos_db
PRODUTOS_DB_PORT=5432
PRODUTOS_MIGRATION_USER=migrator
PRODUTOS_MIGRATION_PASSWORD=choose_a_strong_password

# ── Pedidos database (port 5433) ────────────────────────
PEDIDOS_POSTGRES_USER=admin_pedidos
PEDIDOS_POSTGRES_PASSWORD=admin_pedidos
PEDIDOS_DB_NAME=mps_pedidos_db
PEDIDOS_DB_PORT=5433
PEDIDOS_MIGRATION_USER=migrator
PEDIDOS_MIGRATION_PASSWORD=choose_a_strong_password

# ── Shared app user (both databases) ────────────────────
APP_USER=app_user
APP_PASSWORD=choose_another_strong_password

# ── Pedidos service ─────────────────────────────────────
JWT_SECRET=your_secret_here_change_me
PRODUTOS_SERVICE_URL=http://produtos-api:3000
EVOLUTION_API_URL=http://evolution-api:8080
EVOLUTION_API_KEY=change_me
EVOLUTION_INSTANCE_NAME=vendas
SELLER_WHATSAPP=5511000000000

# ── CORS ────────────────────────────────────────────────
FRONTEND_URL=http://localhost:5173
```

> **Important:** `JWT_SECRET` is used to sign and verify tokens. Pick a strong random string. The same secret must be used when generating JWTs for frontend testing.

---

## 3. Running with Docker Compose (Recommended)

### 3.1 What happens step by step

The architecture uses a **two-phase initialization** pattern:

```
┌─────────────────────────────────────────────────────────┐
│ Phase 1: PostgreSQL container starts                    │
│   → init-user.sh creates two DB roles:                  │
│     • migrator  — DDL (CREATE TABLE, migrations, etc.)  │
│     • app_user  — DML only (SELECT, INSERT, UPDATE, DELETE) │
│   → Default privileges: future tables created by        │
│     migrator auto-grant DML to app_user                 │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────┐
│ Phase 2: init container (profiles: [init])              │
│   → Connects as migrator                                │
│   → Runs sqlx migrations (CREATE TABLE, indexes, etc.)  │
│   → produtos-init also seeds CSV data                   │
│   → Grants DML on ALL TABLES + SEQUENCES to app_user    │
│   → Exits (one-shot container)                          │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────┐
│ Phase 3: API container starts                           │
│   → Connects as app_user (DML-only, least privilege)    │
│   → Listens on :3000 (produtos) / :3001 (pedidos)       │
└─────────────────────────────────────────────────────────┘
```

### 3.2 First launch

```bash
# Build images
docker compose build

# Start databases
docker compose up -d produtos-db pedidos-db

# Wait for healthchecks (pg_isready)
docker compose ps  # look for "(healthy)"

# Run one-shot initialization (migrations + seed data)
docker compose --profile init up produtos-init pedidos-init

# Start APIs
docker compose up -d produtos-api pedidos-api

# Verify
curl http://localhost:3000/api/products/search?q=pioneer
curl -H "Authorization: Bearer <token>" http://localhost:3001/api/pedidos
```

### 3.3 One-liner (after first build)

```bash
docker compose up -d produtos-db pedidos-db && \
  sleep 10 && \
  docker compose --profile init up produtos-init pedidos-init && \
  docker compose up -d
```

### 3.4 Optional services

- **evolution-api** — WhatsApp notification gateway. Only needed if pedidos-service sends seller notifications:
  ```bash
  docker compose up -d evolution-api
  ```

- **produtos-scraper** — Scrapes product data from a Hurricane distributor website:
  ```bash
  docker compose run --rm produtos-scraper
  ```

### 3.5 Data persistence

| Volume               | Contents                        |
|----------------------|---------------------------------|
| `pg_produtos_data`   | Produtos Postgres data          |
| `pg_pedidos_data`    | Pedidos Postgres data           |
| `produtos_static`    | Uploaded product images         |
| `evolution_instances`| WhatsApp session data           |

To reset everything:
```bash
docker compose down -v
```

---

## 4. Running without Docker (Bare Metal)

### 4.1 Start PostgreSQL

You need two PostgreSQL 17 databases:

```bash
# As superuser
psql -c "CREATE USER migrator WITH PASSWORD 'marco576silva' CREATEDB;"
psql -c "CREATE USER app_user WITH PASSWORD 'Ch0pin2709sonata';"
psql -c "CREATE DATABASE mps_produtos_db OWNER migrator;"
psql -c "CREATE DATABASE mps_pedidos_db OWNER migrator;"
```

### 4.2 Build

```bash
cargo build --release
```

### 4.3 Run initialization (migrations + seed)

```bash
# Produtos init
DATABASE_URL="postgres://migrator:marco576silva@localhost:5432/mps_produtos_db" \
APP_USER=app_user \
MIGRATION_USER=migrator \
cargo run --release --package produtos --bin init

# Pedidos init
DATABASE_URL="postgres://migrator:marco576silva@localhost:5433/mps_pedidos_db" \
APP_USER=app_user \
MIGRATION_USER=migrator \
cargo run --release --package pedidos --bin init
```

> **Note:** `APP_USER` and `MIGRATION_USER` env vars are only required by the init binary (neither is needed for the API itself).

### 4.4 Run APIs

```bash
# Terminal 1 — Produtos API
DATABASE_URL="postgres://app_user:Ch0pin2709sonata@localhost:5432/mps_produtos_db" \
FRONTEND_URL=http://localhost:5173 \
STATIC_DIR=./static \
cargo run --release --package produtos --bin api

# Terminal 2 — Pedidos API
DATABASE_URL="postgres://app_user:Ch0pin2709sonata@localhost:5433/mps_pedidos_db" \
JWT_SECRET=change_me \
PRODUTOS_SERVICE_URL=http://localhost:3000 \
EVOLUTION_API_URL=http://localhost:8080 \
EVOLUTION_API_KEY=change_me \
EVOLUTION_INSTANCE_NAME=vendas \
SELLER_WHATSAPP=5511000000000 \
FRONTEND_URL=http://localhost:5173 \
cargo run --release --package pedidos --bin api
```

> **Note:** `DATABASE_URL` uses `app_user` for the API, not `migrator`.

---

## 5. Populating Data

### 5.1 CSV Seed (produtos only)

The `produtos-init` binary reads `raw/data.csv` on first run. The CSV format:

```csv
,Idloja,Idproduto,Idprodutov,Variacao,Idprodutor,Descricao,Local_,Qde,Marca,Num_fab,Num_orig,idunidade,Qde_unidade1,Dt_ultatupco,VLR_VENDA1
,1,1610,1,,1610,ABA AMAROK PRETO -,,6,CAMPER,,,PC,0,12/6/22 13:55,79.9
,1,1601,1,,1601,ABA LATERAL AMAROK DUPLA PRETA - PTB 010,,1,CAMPER,,,PAR,0,9/25/23 13:41,124
```

The serde-mapped columns are:

| CSV Column    | DB Column   | Type        |
|---------------|-------------|-------------|
| `Idproduto`   | `id`        | INTEGER PK  |
| `Descricao`   | `nome`      | TEXT        |
| `Marca`       | `marca`     | VARCHAR(20) |
| `Num_fab`     | `num_fab`   | VARCHAR(20) |
| `idunidade`   | `unidade`   | VARCHAR(5)  |
| `VLR_VENDA1`  | `valor`     | DECIMAL(10,2) |

If you have your own CSV, place it at `raw/data.csv` before running init. The init binary is **idempotent** — it checks `information_schema.tables` for the `produtos` table and skips migration+seed if it exists.

### 5.2 Import via SQL

```bash
psql "postgres://app_user:Ch0pin2709sonata@localhost:5432/mps_produtos_db" \
  -c "\copy produtos(nome, nome_norm, marca, marca_norm, num_fab, unidade, valor, descricao, estoque)
       FROM 'products.csv' CSV HEADER;"
```

> Remember to pre-compute `nome_norm` and `marca_norm` (lowercase, ASCII-only, collapsed whitespace — see `servicos/produtos/src/normalization.rs`). The API does this automatically on `POST /api/products`.

### 5.3 Populate via API

The simplest way to add products at runtime:

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

### 5.4 Pedidos data

Pedidos has **no seed data**. Orders are created at runtime via `POST /api/pedidos`. To test:

```bash
# 1. Generate a JWT
TOKEN="<see Section 6>"

# 2. Create an order
curl -X POST http://localhost:3001/api/pedidos \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"items": [{"id_product": 1, "quantity": 2}]}'
```

---

## 6. Generating a JWT for Testing

The pedidos service has no `/login` endpoint — JWTs must be pre-issued. They must be signed with the same `JWT_SECRET` configured in `.env`.

**Node.js one-liner:**

```js
node -e "
const jwt = require('jsonwebtoken');
const {v4: uuidv4} = require('uuid');
console.log(jwt.sign({customer_id: uuidv4()}, 'change_me', {expiresIn: '24h'}));
"
```

**Using `jose` (no dependencies beyond Node 15+):**

```js
node --experimental-vm-modules -e "
const crypto = require('crypto');
const secret = 'change_me';
const header = Buffer.from(JSON.stringify({alg:'HS256',typ:'JWT'})).toString('base64url');
const payload = Buffer.from(JSON.stringify({
  customer_id: crypto.randomUUID(),
  exp: Math.floor(Date.now()/1000) + 86400
})).toString('base64url');
const sig = crypto.createHmac('sha256', secret)
  .update(header + '.' + payload).digest('base64url');
console.log(header + '.' + payload + '.' + sig);
"
```

**Rust:**

```rust
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct Claims { customer_id: Uuid, exp: u64 }

fn main() {
    let claims = Claims {
        customer_id: Uuid::new_v4(),
        exp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 86400,
    };
    let token = encode(&Header::default(), &claims,
        &EncodingKey::from_secret(b"change_me")).unwrap();
    println!("{token}");
}
```

Save the token:

```bash
export TOKEN="eyJhbGciOiJIUzI1NiIs..."
```

---

## 7. Verifying Everything Works

```bash
# ── Produtos service ────────────────────────────────────
# Search
curl -s "http://localhost:3000/api/products/search?q=pionner&limit=3" | jq .

# Get by ID
curl -s http://localhost:3000/api/products/1610 | jq .

# Create
curl -s -X POST http://localhost:3000/api/products \
  -H "Content-Type: application/json" \
  -d '{"nome":"Test Product","marca":"TestBrand","unidade":"PC","valor":9.99,"estoque":10}' | jq .

# Upload image
curl -s -X POST http://localhost:3000/api/products/1610/imagens \
  -F "image=@/path/to/some/image.jpg" | jq .

# ── Pedidos service ─────────────────────────────────────
# List orders (empty initially)
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://localhost:3001/api/pedidos" | jq .

# Create order
curl -s -X POST http://localhost:3001/api/pedidos \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"items":[{"id_product":1610,"quantity":2}]}' | jq .

# Get order
curl -s -H "Authorization: Bearer $TOKEN" \
  http://localhost:3001/api/pedidos/1 | jq .

# Update status
curl -s -X PATCH http://localhost:3001/api/pedidos/1/status \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"status":"confirmado"}' | jq .
```

---

## 8. Architecture Summary

```
                    ┌──────────────────┐
                    │   Frontend       │
                    │   :5173 (Vite)   │
                    └───┬─────────┬────┘
                        │ GET/POST │ GET/POST/PATCH/DELETE
                        │ (no auth)│ (+ Authorization: Bearer)
                        ▼          ▼
              ┌─────────────┐  ┌─────────────────┐
              │ produtos-api│  │  pedidos-api    │
              │   :3000     │  │    :3001        │
              └──────┬──────┘  └────┬────────┬───┘
                     │              │        │
                     │              │  HTTP  │ HTTP
                     │              │  GET   │ POST
                     │              ▼        ▼
                     │       ┌──────────┐ ┌──────────────┐
                     │       │ produtos │ │ evolution-api│
                     │       │  -api    │ │   :8080      │
                     │       └──────────┘ │ (WhatsApp)   │
                     │                    └──────────────┘
          ┌──────────┴──────────┐
          ▼                     ▼
   ┌──────────────┐    ┌──────────────┐
   │ produtos-db  │    │ pedidos-db   │
   │   :5432      │    │   :5433      │
   │ PG 17 alpine │    │ PG 17 alpine │
   └──────────────┘    └──────────────┘
```

**Key design decisions:**

| Decision | Detail |
|----------|--------|
| Two databases | Separate Postgres instances — no cross-DB queries |
| Dual roles | `migrator` (DDL) for init, `app_user` (DML) for runtime |
| No ORM | Raw `sqlx::query_as!` with compile-time SQL checking |
| Optimistic concurrency | Status changes check `WHERE stat = $current` |
| Inter-service | Pedidos validates products by calling GET on produtos-api |
| JWT ownership | Every pedidos request verifies `order.customer_id == jwt.customer_id` |

---

## 9. Port Reference

| Service         | Port  | Auth    | Purpose                     |
|-----------------|-------|---------|-----------------------------|
| produtos-api    | 3000  | None    | Product CRUD, search, images|
| pedidos-api     | 3001  | JWT     | Order management            |
| produtos-db     | 5432  | —       | Products + images data      |
| pedidos-db      | 5433  | —       | Orders + items data         |
| evolution-api   | 8080  | API key | WhatsApp gateway            |
| frontend (Vite) | 5173  | —       | CORS allowed origin         |

---

## 10. Troubleshooting

### "Connection refused" on port 3000/3001
The APIs haven't started. Check container status: `docker compose ps`. Look for "(healthy)" on DB containers.

### "Product with ID X not found" when creating orders
Ensure the produtos database has been seeded (the `raw/data.csv` file exists and `produtos-init` ran successfully). Verify: `curl http://localhost:3000/api/products/1`.

### "Unauthorized" on all pedidos requests
- Check JWT is in `Authorization: Bearer <token>` format
- Ensure `JWT_SECRET` matches between token generation and `.env`
- Token may be expired (default 24h from generation)

### "Cannot transition from X to Y"
Check the valid transitions table in `docs/api-guide.md`. Some states are terminal (`entregue`, `cancelado`).

### "DATABASE_URL must be set" on bare-metal
Each binary reads exactly one env var: `DATABASE_URL`. The `.env` file example shows the format — but that file is only auto-loaded by Docker. For bare-metal, export it manually (or use `dotenvy` which `main.rs` calls via `dotenv().ok()`).

### Port conflicts
Stop any local Postgres: `sudo systemctl stop postgresql`. Or change the ports in `.env` and `docker-compose.yml`.
