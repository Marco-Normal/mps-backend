# User Role Separation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure both `produtos` and `pedidos` databases have proper role separation (admin/migration/CRUD) and init-user.sh works across multiple PostgreSQL instances.

**Architecture:** Single parametric `init-user.sh` mounted on both DB containers, using generic env vars (`MIGRATION_USER`, `MIGRATION_PASSWORD`, `APP_USER`, `APP_PASSWORD`). Both `init.rs` binaries add `ALTER DEFAULT PRIVILEGES FOR ROLE migrator` after migrations so future tables auto-grant DML to app_user.

**Tech Stack:** Bash (init-user.sh), Docker Compose, Rust + sqlx (init.rs)

## Global Constraints

- Migration user gets DDL (CREATE, ALL on schema) — runs SQLx migrations
- App user gets DML only (SELECT, INSERT, UPDATE, DELETE) — used by API services
- Admin is the Postgres superuser (`POSTGRES_USER`)
- No `.env` changes required
- Script must be idempotent (safe to re-run, uses `CREATE USER` with existence check or `IF NOT EXISTS`)

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `scripts/init-user.sh` | Rewrite | Create users with correct privileges, parametric via env vars |
| `docker-compose.yml` | Modify | Mount script on pedidos-db, add MIGRATION/APP env vars to both DBs |
| `servicos/produtos/src/bin/init.rs` | Modify | Add `ALTER DEFAULT PRIVILEGES FOR ROLE migrator` after existing GRANTs |
| `servicos/pedidos/src/bin/init.rs` | Modify | Add `ALTER DEFAULT PRIVILEGES FOR ROLE migrator` after existing GRANTs |

---

### Task 1: Rewrite init-user.sh as parametric script

**Files:**
- Modify: `scripts/init-user.sh` (full rewrite)

**Interfaces:**
- Consumes: `POSTGRES_USER`, `POSTGRES_DB` (standard Postgres vars), `MIGRATION_USER`, `MIGRATION_PASSWORD`, `APP_USER`, `APP_PASSWORD` (from docker-compose environment)
- Produces: Creates `migration_user` and `app_user` roles in the target database

- [ ] **Step 1: Write the parametric init-user.sh**

```bash
#!/bin/bash
set -e

# This script runs inside any PostgreSQL container when it's first initialized.
# It runs as the superuser ($POSTGRES_USER).
# Reads generic env vars set by docker-compose per service.

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    -- Create migration user (DDL: can create tables, sequences, indexes, run migrations)
    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '${MIGRATION_USER}') THEN
            CREATE USER ${MIGRATION_USER} WITH PASSWORD '${MIGRATION_PASSWORD}';
        END IF;
    END
    \$\$;
    GRANT CONNECT ON DATABASE ${POSTGRES_DB} TO ${MIGRATION_USER};
    GRANT CREATE ON DATABASE ${POSTGRES_DB} TO ${MIGRATION_USER};
    GRANT ALL PRIVILEGES ON SCHEMA public TO ${MIGRATION_USER};

    -- Create application user (DML only: no DDL, used by API services)
    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '${APP_USER}') THEN
            CREATE USER ${APP_USER} WITH PASSWORD '${APP_PASSWORD}';
        END IF;
    END
    \$\$;
    GRANT CONNECT ON DATABASE ${POSTGRES_DB} TO ${APP_USER};

    -- Default privileges for future tables created by admin (belt-and-suspenders)
    ALTER DEFAULT PRIVILEGES IN SCHEMA public
        GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO ${APP_USER};
    ALTER DEFAULT PRIVILEGES IN SCHEMA public
        GRANT USAGE, SELECT ON SEQUENCES TO ${APP_USER};
EOSQL
```

- [ ] **Step 2: Syntax-check the script**

```bash
bash -n scripts/init-user.sh
```
Expected: no output (no syntax errors).

- [ ] **Step 3: Commit**

```bash
git add scripts/init-user.sh
git commit -m "feat: rewrite init-user.sh as parametric script with role separation"
```

---

### Task 2: Mount script on both DBs and add env vars

**Files:**
- Modify: `docker-compose.yml` — produtos-db environment/volumes and pedidos-db environment/volumes

**Interfaces:**
- Consumes: `scripts/init-user.sh` (from Task 1), existing `.env` vars
- Produces: Both DB containers run init script with correct per-instance env vars

- [ ] **Step 1: Update produtos-db**

Add `MIGRATION_USER`, `MIGRATION_PASSWORD`, `APP_USER`, `APP_PASSWORD` environment variables. The volume mount already exists.

From:
```yaml
  produtos-db:
    image: postgres:17-alpine
    container_name: mps-produtos-db
    restart: unless-stopped
    env_file: .env
    environment:
      POSTGRES_USER: ${PRODUTOS_POSTGRES_USER}
      POSTGRES_PASSWORD: ${PRODUTOS_POSTGRES_PASSWORD}
      POSTGRES_DB: ${PRODUTOS_DB_NAME}
    ports:
      - "${PRODUTOS_DB_PORT}:5432"
    volumes:
      - pg_produtos_data:/var/lib/postgresql/data
      - ./scripts/init-user.sh:/docker-entrypoint-initdb.d/init-user.sh
    healthcheck:
      ...
```

To:
```yaml
  produtos-db:
    image: postgres:17-alpine
    container_name: mps-produtos-db
    restart: unless-stopped
    env_file: .env
    environment:
      POSTGRES_USER: ${PRODUTOS_POSTGRES_USER}
      POSTGRES_PASSWORD: ${PRODUTOS_POSTGRES_PASSWORD}
      POSTGRES_DB: ${PRODUTOS_DB_NAME}
      MIGRATION_USER: ${PRODUTOS_MIGRATION_USER}
      MIGRATION_PASSWORD: ${PRODUTOS_MIGRATION_PASSWORD}
      APP_USER: ${APP_USER}
      APP_PASSWORD: ${APP_PASSWORD}
    ports:
      - "${PRODUTOS_DB_PORT}:5432"
    volumes:
      - pg_produtos_data:/var/lib/postgresql/data
      - ./scripts/init-user.sh:/docker-entrypoint-initdb.d/init-user.sh
    healthcheck:
      ...
```

- [ ] **Step 2: Update pedidos-db**

Add the same environment vars AND the volume mount for init-user.sh.

From:
```yaml
  pedidos-db:
    image: postgres:17-alpine
    container_name: mps-pedidos-db
    restart: unless-stopped
    env_file: .env
    environment:
      POSTGRES_USER: ${PEDIDOS_POSTGRES_USER}
      POSTGRES_PASSWORD: ${PEDIDOS_POSTGRES_PASSWORD}
      POSTGRES_DB: ${PEDIDOS_DB_NAME}
    ports:
      - "${PEDIDOS_DB_PORT}:5432"
    volumes:
      - pg_pedidos_data:/var/lib/postgresql/data
    healthcheck:
      ...
```

To:
```yaml
  pedidos-db:
    image: postgres:17-alpine
    container_name: mps-pedidos-db
    restart: unless-stopped
    env_file: .env
    environment:
      POSTGRES_USER: ${PEDIDOS_POSTGRES_USER}
      POSTGRES_PASSWORD: ${PEDIDOS_POSTGRES_PASSWORD}
      POSTGRES_DB: ${PEDIDOS_DB_NAME}
      MIGRATION_USER: ${PEDIDOS_MIGRATION_USER}
      MIGRATION_PASSWORD: ${PEDIDOS_MIGRATION_PASSWORD}
      APP_USER: ${APP_USER}
      APP_PASSWORD: ${APP_PASSWORD}
    ports:
      - "${PEDIDOS_DB_PORT}:5432"
    volumes:
      - pg_pedidos_data:/var/lib/postgresql/data
      - ./scripts/init-user.sh:/docker-entrypoint-initdb.d/init-user.sh
    healthcheck:
      ...
```

- [ ] **Step 3: Verify compose file parses**

```bash
docker compose config --no-interpolate 2>&1 | grep -E 'MIGRATION_USER|APP_USER'
```
Expected: Shows MIGRATION_USER and APP_USER for both produtos-db and pedidos-db.

- [ ] **Step 4: Commit**

```bash
git add docker-compose.yml
git commit -m "feat: mount init-user.sh on pedidos-db and add role env vars to both DBs"
```

---

### Task 3: Add ALTER DEFAULT PRIVILEGES FOR ROLE migrator to init.rs files

**Files:**
- Modify: `servicos/produtos/src/bin/init.rs:59-72`
- Modify: `servicos/pedidos/src/bin/init.rs:13-25`

**Interfaces:**
- Consumes: `APP_USER` env var (same as current), `MIGRATION_USER` env var (new — the role running init.rs)
- Produces: After migration, app_user gets DML on all existing and future tables created by migrator

- [ ] **Step 1: Update produtos init.rs GRANT block**

Replace the GRANT block (lines 59-72):

```rust
let app_user = dotenvy::var("APP_USER").expect("APP USER MUST BE SET");
let migrator = dotenvy::var("MIGRATION_USER").unwrap_or_else(|_| "migrator".to_string());
// After running migrations, grant DML to app_user on all tables
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
// Future tables created by migrator auto-grant DML to app_user
sqlx::raw_sql(&format!(
    "ALTER DEFAULT PRIVILEGES FOR ROLE {migrator} IN SCHEMA public \
     GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO {app_user}"
))
.execute(&pool)
.await
.into_diagnostic()?;
sqlx::raw_sql(&format!(
    "ALTER DEFAULT PRIVILEGES FOR ROLE {migrator} IN SCHEMA public \
     GRANT USAGE, SELECT ON SEQUENCES TO {app_user}"
))
.execute(&pool)
.await
.into_diagnostic()?;
```

- [ ] **Step 2: Update pedidos init.rs GRANT block**

Replace the GRANT block (lines 13-25):

```rust
let app_user = dotenvy::var("APP_USER").expect("APP_USER must be set");
let migrator = dotenvy::var("MIGRATION_USER").unwrap_or_else(|_| "migrator".to_string());
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
sqlx::raw_sql(&format!(
    "ALTER DEFAULT PRIVILEGES FOR ROLE {migrator} IN SCHEMA public \
     GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO {app_user}"
))
.execute(&pool)
.await
.into_diagnostic()?;
sqlx::raw_sql(&format!(
    "ALTER DEFAULT PRIVILEGES FOR ROLE {migrator} IN SCHEMA public \
     GRANT USAGE, SELECT ON SEQUENCES TO {app_user}"
))
.execute(&pool)
.await
.into_diagnostic()?;
```

- [ ] **Step 3: Verify both packages compile**

```bash
cargo check --package produtos --package pedidos 2>&1
```
Expected: both packages compile without errors.

- [ ] **Step 4: Commit**

```bash
git add servicos/produtos/src/bin/init.rs servicos/pedidos/src/bin/init.rs
git commit -m "feat: add ALTER DEFAULT PRIVILEGES FOR ROLE migrator to init binaries"
```

---

## Verification Checklist

After all tasks:

- [ ] `bash -n scripts/init-user.sh` — no syntax errors
- [ ] `docker compose config` — parses without errors, MIGRATION_USER and APP_USER present on both DBs
- [ ] `cargo check --workspace` — all packages compile
- [ ] `cargo test --workspace` — all existing tests still pass
