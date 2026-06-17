# User Management: Role Separation Across PostgreSQL Instances

**Date:** 2025-06-16
**Goal:** Ensure both `produtos` and `pedidos` databases have proper role separation (admin, migration, CRUD) and that the init-user script works for multiple PostgreSQL instances.

---

## Problem

1. `scripts/init-user.sh` is only mounted on `produtos-db` — `pedidos-db` has no user initialization
2. `ALTER DEFAULT PRIVILEGES` in init-user.sh runs as admin, so it only applies to tables created by admin — the migration user creates tables, so app_user never gets DML on them
3. The inline GRANTs in `produtos/src/bin/init.rs` are a workaround for #2 but need improvement
4. No clear contractual guarantee that API services only connect as the CRUD user (no DDL access)

---

## Solution

### Single parametric init script

`scripts/init-user.sh` reads generic env vars (`MIGRATION_USER`, `MIGRATION_PASSWORD`, `APP_USER`, `APP_PASSWORD`, `POSTGRES_DB`) and creates two roles:

- **Migration user** — `CREATE` on database, `ALL` on schema public (runs SQLx migrations)
- **App user** — `CONNECT` on database, DML-only via `ALTER DEFAULT PRIVILEGES`

Mounted on both `produtos-db` and `pedidos-db` via docker-compose.

### Post-migration GRANT fix

Both `produtos` and `pedidos` init binaries run after SQLx migrations:

```sql
-- Grant on tables just created by this migration
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO app_user;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO app_user;

-- Future tables created by migrator auto-grant to app_user
ALTER DEFAULT PRIVILEGES FOR ROLE migrator IN SCHEMA public
  GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO app_user;
ALTER DEFAULT PRIVILEGES FOR ROLE migrator IN SCHEMA public
  GRANT USAGE, SELECT ON SEQUENCES TO app_user;
```

The `FOR ROLE migrator` clause is the fix — without it, `ALTER DEFAULT PRIVILEGES` only applies to the admin's own future objects. With it, any table created by the migration user (running SQLx migrations) automatically gets DML for app_user.

### docker-compose changes

Each DB container gets generic env vars mapped from existing `.env` values:

```yaml
produtos-db:
  environment:
    MIGRATION_USER: ${PRODUTOS_MIGRATION_USER}
    MIGRATION_PASSWORD: ${PRODUTOS_MIGRATION_PASSWORD}
    APP_USER: ${APP_USER}
    APP_PASSWORD: ${APP_PASSWORD}
  volumes:
    - ./scripts/init-user.sh:/docker-entrypoint-initdb.d/init-user.sh
    - pg_produtos_data:/var/lib/postgresql/data

pedidos-db:
  environment:
    MIGRATION_USER: ${PEDIDOS_MIGRATION_USER}
    MIGRATION_PASSWORD: ${PEDIDOS_MIGRATION_PASSWORD}
    APP_USER: ${APP_USER}
    APP_PASSWORD: ${APP_PASSWORD}
  volumes:
    - ./scripts/init-user.sh:/docker-entrypoint-initdb.d/init-user.sh
    - pg_pedidos_data:/var/lib/postgresql/data
```

### Role usage by service

| Service | Connects as | Privileges |
|---|---|---|
| `produtos-init` | `PRODUTOS_MIGRATION_USER` | DDL (create tables, run migrations) |
| `produtos-api` | `APP_USER` | DML only (SELECT, INSERT, UPDATE, DELETE) |
| `pedidos-init` | `PEDIDOS_MIGRATION_USER` | DDL (create tables, run migrations) |
| `pedidos-api` | `APP_USER` | DML only (SELECT, INSERT, UPDATE, DELETE) |

No `.env` changes needed — all existing vars cover both services.

---

## Files Changed

| File | Change |
|---|---|
| `scripts/init-user.sh` | Rewrite as parametric script using generic env vars |
| `docker-compose.yml` | Mount script on pedidos-db, add MIGRATION/APP env vars to both DBs |
| `servicos/produtos/src/bin/init.rs` | Update GRANT block with `FOR ROLE migrator` |
| `servicos/pedidos/src/bin/init.rs` | Update GRANT block with `FOR ROLE migrator` |

---

## Non-Goals

- Separate app users per service (both use `APP_USER` — different DBs isolate them)
- Row-level security or table-level per-user ACLs
- Password rotation or secret management
