# Docker Build Optimization — Design Spec

**Date:** 2025-06-16
**Goal:** Eliminate redundant Rust compilation in Docker builds by deduplicating build blocks in docker-compose.yml.

---

## Problem

`docker compose up --build` triggers four independent `docker build` invocations for two Rust packages:

| Service | Build args | Compilation |
|---|---|---|
| `produtos-api` | `SERVICE_NAME=produtos` | Full build |
| `produtos-init` | `SERVICE_NAME=produtos` | Full build (identical to above) |
| `pedidos-api` | `SERVICE_NAME=pedidos` | Full build |
| `pedidos-init` | `SERVICE_NAME=pedidos` | Full build (identical to above) |

Each `build:` block in Compose runs an independent `docker build` — BuildKit does not deduplicate across services.

Additionally:
- No root `.dockerignore` — entire project context (including `target/`, `raw/`, docs, scraper files) is sent to the Docker daemon for each build
- Dockerfile lines 30-31 hardcode `COPY …/api …/init` regardless of `SERVICE_NAME`, which is fragile across packages

The `produtos-scraper` (Python) and `evolution-api` (pre-built image) are unaffected — no redundant builds.

---

## Solution

### Core Idea

One Docker image per Rust package, referenced by multiple Compose services. Init services move to an `init` profile since they are one-shot migrations.

### Resulting structure

```
produtos-api    image: mps-produtos:latest   build: yes (single build)
produtos-init   image: mps-produtos:latest   profile: init (no build)
pedidos-api     image: mps-pedidos:latest    build: yes (single build)
pedidos-init    image: mps-pedidos:latest    profile: init (no build)
```

**Build count:** 4 → 2 compilations per `docker compose up --build`.

### docker-compose.yml changes

- `produtos-api` and `pedidos-api` — add `image:` directive alongside existing `build:` block; keep `build:` block; keep `ports` and `depends_on`
- `produtos-init` and `pedidos-init` — replace `build:` block with `image:` only; add `profiles: ["init"]`; keep `depends_on`, `environment`, `command`
- No changes to `produtos-db`, `pedidos-db`, `produtos-scraper`, `evolution-api`

### Dockerfile changes

- Fix binary copy stage to be service-aware — do not hardcode binary names. Each package may produce different binaries (e.g., `produtos` produces `api` + `init`; `pedidos` may differ)
- If a package does not produce an `init` binary, the init service for that package should be omitted or the binary added

### Root `.dockerignore`

Add `.dockerignore` at repo root excluding:
```
target/
raw/
docs/
.git/
.env
token-optimizer/
servicos/scraper/
*.md
```

This shrinks the build context sent to Docker on each build.

### Workflow

| Scenario | Command |
|---|---|
| First deploy | `docker compose build && docker compose --profile init up` |
| Daily start | `docker compose up --build` |
| Re-run migrations | `docker compose --profile init run produtos-init` |

---

## Files Changed

| File | Change |
|---|---|
| `docker-compose.yml` | Restructure Rust services: image tags, init profiles |
| `Dockerfile` | Service-aware binary copying |
| `.dockerignore` (new) | Exclude non-build files from context |

---

## Edge Cases

- **`pedidos` package may not produce an `api` binary** — currently `produtos` names its main binary `api` via `[[bin]]`, but `pedidos` uses the default package-name binary (`pedidos`). The Dockerfile must handle this or `pedidos/Cargo.toml` should add `[[bin]] name = "api"` for consistency.
- **`pedidos` has no `init` binary** — no `src/bin/init.rs` exists. If `pedidos-init` is needed, the binary must be implemented; otherwise the init service should be removed from Compose.
- **Cache invalidation on context change** — the `.dockerignore` avoids spurious cache busts from unrelated file changes

---

## Non-Goals

- Changing the `produtos-scraper` (Python) build — single build, no redundancy
- Changing the `evolution-api` service — pre-built image, no build
- Splitting the Rust workspace into separate repositories
- Introducing multi-stage caching beyond what `cargo-chef` already provides
