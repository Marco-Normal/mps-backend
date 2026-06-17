# Docker Build Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate redundant Rust compilation — 4 docker builds → 2 — by deduplicating build blocks in docker-compose.yml.

**Architecture:** One Docker image per Rust package (`mps-produtos:latest`, `mps-pedidos:latest`), referenced by multiple Compose services via `image:`. Init services move to `profiles: ["init"]`. Root `.dockerignore` shrinks build context.

**Tech Stack:** Docker + BuildKit, Docker Compose, Rust workspace with cargo-chef, Cargo `[[bin]]` targets

## Global Constraints

- `docker compose up --build` must remain the daily developer workflow
- `produtos-scraper` (Python) and `evolution-api` (pre-built image) must not change
- Init services are one-shot migrations, run only via `--profile init`
- Pedidos must produce `api` and `init` binaries to match Dockerfile expectations

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `.dockerignore` | Create | Exclude non-build files from Docker context |
| `servicos/pedidos/Cargo.toml` | Modify | Add `[[bin]]` entries for `api` and `init` |
| `servicos/pedidos/src/bin/init.rs` | Create | Minimal migrations-only init binary (no CSV seed) |
| `docker-compose.yml` | Modify | Image tags, init profiles, remove redundant `build:` blocks |

The `Dockerfile` itself does not change — after standardizing pedidos binary names, the existing COPY lines (`api` and `init`) work correctly for both packages.

---

### Task 1: Add root `.dockerignore`

**Files:**
- Create: `.dockerignore`

**Interfaces:**
- Consumes: nothing
- Produces: `.dockerignore` — shrinks Docker build context for all `docker build` invocations

- [ ] **Step 1: Create `.dockerignore`**

```dockerignore
target/
raw/
docs/
.git/
.env
token-optimizer/
servicos/scraper/
*.md
```

- [ ] **Step 2: Verify file was created**

```bash
cat .dockerignore
```
Expected: prints the exclusion patterns above.

- [ ] **Step 3: Commit**

```bash
git add .dockerignore
git commit -m "feat: add root .dockerignore to shrink build context"
```

---

### Task 2: Standardize pedidos binary targets and create init stub

**Files:**
- Modify: `servicos/pedidos/Cargo.toml:1-5` — add `[[bin]]` entries
- Create: `servicos/pedidos/src/bin/init.rs`

**Interfaces:**
- Consumes: `common::db_utils::create_pool`, `common::db_utils::table_exists` (same as produtos init)
- Produces: `pedidos` package now compiles to `api` and `init` binaries; Docker `COPY --from=builder /app/target/release/api` and `COPY .../init` both succeed

- [ ] **Step 1: Add `[[bin]]` entries to pedidos Cargo.toml**

Replace the first 5 lines of `servicos/pedidos/Cargo.toml`:

**Before:**
```toml
[package]
name = "pedidos"
version.workspace = true
edition.workspace = true
```

**After:**
```toml
[package]
name = "pedidos"
version.workspace = true
edition.workspace = true

[[bin]]
name = "api"
path = "src/main.rs"

[[bin]]
name = "init"
path = "src/bin/init.rs"
```

Keep all `[dependencies]` unchanged.

- [ ] **Step 2: Create `servicos/pedidos/src/bin/init.rs`**

Minimal init binary — runs SQL migrations and grants permissions. No CSV seeding (pedidos has no seed data).

```rust
use common::db_utils::{create_pool, table_exists};
use miette::IntoDiagnostic;
use tracing::info;

#[tokio::main]
pub async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Initializing pedidos database pool");
    let pool = create_pool(1).await;
    info!("Checking if migrations are needed...");
    if !table_exists(&pool, "pedidos").await? {
        sqlx::migrate!().run(&pool).await.into_diagnostic()?;
        let app_user = dotenvy::var("APP_USER").expect("APP_USER must be set");
        sqlx::raw_sql(&format!(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO {app_user}"
        ))
        .execute(&pool)
        .await
        .into_diagnostic()?;
        sqlx::raw_sql(&format!(
            "GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO {app_user}"
        ))
        .execute(&pool)
        .await
        .into_diagnostic()?;
        info!("Initialization complete");
        return Ok(());
    }
    info!("Migrations already applied, nothing to do");
    Ok(())
}
```

- [ ] **Step 3: Verify pedidos compiles both binaries**

```bash
cargo check --package pedidos 2>&1
```
Expected: compiles without errors. Both `api` and `init` targets should appear in output.

- [ ] **Step 4: Commit**

```bash
git add servicos/pedidos/Cargo.toml servicos/pedidos/src/bin/init.rs
git commit -m "feat: standardize pedidos binary names to api and init"
```

---

### Task 3: Restructure docker-compose.yml

**Files:**
- Modify: `docker-compose.yml` — lines 28-41 (produtos-init), 43-63 (produtos-api), 88-101 (pedidos-init), 103-124 (pedidos-api)

**Interfaces:**
- Consumes: Docker images `mps-produtos:latest` and `mps-pedidos:latest` (built from Dockerfile)
- Produces: Compose services that share images, init services gated behind profile

- [ ] **Step 1: Update produtos-init (lines 28-41)**

**Before:**
```yaml
  produtos-init:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        SERVICE_NAME: produtos
    container_name: mps-produtos-init
    depends_on:
      produtos-db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://${PRODUTOS_MIGRATION_USER}:${PRODUTOS_MIGRATION_PASSWORD}@produtos-db:5432/${PRODUTOS_DB_NAME}
      APP_USER: ${APP_USER}
    command: ["./init"]
```

**After:**
```yaml
  produtos-init:
    image: mps-produtos:latest
    container_name: mps-produtos-init
    profiles:
      - init
    depends_on:
      produtos-db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://${PRODUTOS_MIGRATION_USER}:${PRODUTOS_MIGRATION_PASSWORD}@produtos-db:5432/${PRODUTOS_DB_NAME}
      APP_USER: ${APP_USER}
    command: ["./init"]
```

- [ ] **Step 2: Update produtos-api (lines 43-63)**

**Before:**
```yaml
  produtos-api:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        SERVICE_NAME: produtos
    container_name: mps-produtos-api
```

**After:**
```yaml
  produtos-api:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        SERVICE_NAME: produtos
    image: mps-produtos:latest
    container_name: mps-produtos-api
```

Keep `depends_on`, `environment`, `restart`, `ports`, `volumes` unchanged.

- [ ] **Step 3: Update pedidos-init (lines 88-101)**

**Before:**
```yaml
  pedidos-init:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        SERVICE_NAME: pedidos
    container_name: mps-pedidos-init
    depends_on:
      pedidos-db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://${PEDIDOS_MIGRATION_USER}:${PEDIDOS_MIGRATION_PASSWORD}@pedidos-db:5432/${PEDIDOS_DB_NAME}
      APP_USER: ${APP_USER}
    command: ["./init"]
```

**After:**
```yaml
  pedidos-init:
    image: mps-pedidos:latest
    container_name: mps-pedidos-init
    profiles:
      - init
    depends_on:
      pedidos-db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://${PEDIDOS_MIGRATION_USER}:${PEDIDOS_MIGRATION_PASSWORD}@pedidos-db:5432/${PEDIDOS_DB_NAME}
      APP_USER: ${APP_USER}
    command: ["./init"]
```

- [ ] **Step 4: Update pedidos-api (lines 103-124)**

**Before:**
```yaml
  pedidos-api:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        SERVICE_NAME: pedidos
    container_name: mps-pedidos-api
```

**After:**
```yaml
  pedidos-api:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        SERVICE_NAME: pedidos
    image: mps-pedidos:latest
    container_name: mps-pedidos-api
```

Keep `depends_on`, `environment`, `restart`, `ports` unchanged.

- [ ] **Step 5: Verify compose file parses**

```bash
docker compose config --no-interpolate 2>&1 | head -20
```
Expected: Prints resolved compose configuration without errors.

- [ ] **Step 6: Verify Docker images build**

```bash
docker compose build produtos-api pedidos-api 2>&1
```
Expected: Two builds (not four). Output shows "CACHED" on second build if nothing changed.

- [ ] **Step 7: Commit**

```bash
git add docker-compose.yml
git commit -m "feat: deduplicate docker builds with shared images and init profile"
```

---

## Verification Checklist

After all tasks complete:

- [ ] `docker compose build produtos-api pedidos-api` — exactly 2 Rust compilations (not 4)
- [ ] `docker compose up --build` — works as before, builds only 2 images
- [ ] `docker compose --profile init run produtos-init` — runs migrations from the shared image
- [ ] `docker compose --profile init run pedidos-init` — runs migrations from the shared image
- [ ] `docker compose up` (without `--build` or `--profile init`) — starts only api + db services

## Workflow Changes

| Scenario | Before | After |
|---|---|---|
| Daily start | `docker compose up --build` | `docker compose up --build` |
| First deploy | `docker compose up --build` | `docker compose build && docker compose --profile init up` |
| Re-run migrations | `docker compose up --build` (rebuilt all) | `docker compose --profile init run produtos-init` |
