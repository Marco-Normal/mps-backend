# Product Enrichment Scraper — Design Spec

**Date:** 2026-06-14
**Status:** Approved
**Scope:** Standalone Python Docker service that scrapes product images and descriptions for a configurable set of brands and writes them to the `produtos` database.

---

## Problem

The `produtos` table has `descricao` (TEXT, nullable) and a related `imagens_produto` table introduced in the latest migrations. Both are empty after the CSV seed. This service fills them in by scraping manufacturer sites and Mercado Livre, downloading images to the shared static volume, and updating the DB.

Initial target: `marca = 'Hurricane'` (27 products). The architecture is brand-pluggable so any future marca can be added with a new adapter.

---

## Architecture

Three layers inside `servicos/scraper/`:

### 1. Brand Adapters (`adapters/`)

Each adapter is a class with the interface:

```python
class BrandAdapter:
    marca: str  # matches DB value exactly, e.g. "Hurricane"

    async def search(
        self, product_name: str, num_fab: str | None
    ) -> ScrapedData | None:
        ...
```

`ScrapedData` is a dataclass:

```python
@dataclass
class ScrapedData:
    descricao: str | None
    image_urls: list[str]  # max 3 entries
```

Adapters own all scraping logic for their brand. The Hurricane adapter tries `hurricanesound.com.br` first (via Playwright for JS rendering), then falls back to the generic Mercado Livre source.

The adapter registry lives in `adapters/__init__.py`:

```python
REGISTRY: dict[str, type[BrandAdapter]] = {
    "Hurricane": HurricaneAdapter,
    # "JBL": JBLAdapter,  # future
}
```

Unknown marcas are skipped with a warning log.

### 2. Generic Mercado Livre Source (`sources/mercadolivre.py`)

Brand-agnostic, shared fallback. Uses the public ML REST API:
- `GET /sites/MLB/search?q={product_name}` — finds best-matching listing
- Fetches the listing's image gallery
- Returns up to 3 image URLs and the listing title as description

No authentication required. Uses `httpx` (async).

### 3. Orchestrator (`main.py`)

1. Reads `MARCA` env var (comma-separated, e.g. `"Hurricane"` or `"Hurricane,JBL"`)
2. Queries DB: `SELECT id, nome, marca, num_fab FROM produtos WHERE marca = ANY($1) AND descricao IS NULL`
3. For each unenriched product:
   a. Looks up adapter by `marca`; skips if none registered
   b. Calls `adapter.search(nome, num_fab)`
   c. Downloads up to 3 images as `<uuid4>.jpg` to `STATIC_DIR`
   d. Inserts rows into `imagens_produto`
   e. `UPDATE produtos SET descricao = ... WHERE id = ...`
   f. Waits `SCRAPER_DELAY_MS` ms before next product (politeness)
4. Each product committed individually — partial runs resume from where they left off

---

## Idempotency

The idempotency gate is the DB query itself: `descricao IS NULL`. A product with a non-null `descricao` is never touched again. No external state file or tracking table needed.

- **Resume after crash:** already-enriched products are skipped, unenriched ones are retried
- **Re-scrape a product:** `UPDATE produtos SET descricao = NULL WHERE id = X` then re-run

---

## Docker Setup

### Directory structure

```
servicos/scraper/
├── Dockerfile
├── requirements.txt
├── main.py
├── adapters/
│   ├── __init__.py      # REGISTRY dict
│   └── hurricane.py
└── sources/
    ├── __init__.py
    └── mercadolivre.py
```

### `Dockerfile`

```dockerfile
FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt \
    && playwright install chromium --with-deps
COPY . .
CMD ["python", "main.py"]
```

### `requirements.txt`

```
asyncpg
playwright
beautifulsoup4
httpx
```

### `docker-compose.yml` addition

```yaml
produtos-scraper:
  build:
    context: ./servicos/scraper
  restart: "no"
  environment:
    DATABASE_URL: ${PRODUTOS_DATABASE_URL}
    STATIC_DIR: /static
    MARCA: "Hurricane"
    SCRAPER_DELAY_MS: 500
    LOG_LEVEL: INFO
  volumes:
    - produtos_static:/static
  depends_on:
    produtos-init:
      condition: service_completed_successfully
```

`produtos_static` is a named volume also mounted by `produtos-api` so the API can serve the downloaded images.

---

## Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | yes | — | PostgreSQL connection string (same as `produtos` service) |
| `STATIC_DIR` | yes | — | Path where image files are saved (UUID filenames) |
| `MARCA` | no | `Hurricane` | Comma-separated list of brand names to enrich |
| `SCRAPER_DELAY_MS` | no | `500` | Milliseconds to wait between products |
| `LOG_LEVEL` | no | `INFO` | Python logging level |

---

## Error Handling

Errors are isolated per product. The run never aborts due to a single failure.

| Failure | Behaviour |
|---|---|
| Manufacturer site network/timeout | Log warning, try Mercado Livre |
| Mercado Livre network/timeout | Log warning, skip product (stays unenriched) |
| Image download fails for one URL | Skip that image, continue with remaining |
| DB write failure | Log error, skip product (retried on next run) |
| No adapter for marca | Log warning, skip entire marca |

All logs go to stdout. Log level controlled by `LOG_LEVEL`.

---

## Developer Tools

- `python main.py --dry-run` — prints what would be done without writing to DB or disk
- `python main.py --product-id 2591` — runs against a single product for manual verification

---

## Verification

After deploying, manually verify:
1. `docker compose up produtos-scraper` completes with exit code 0
2. `SELECT id, descricao FROM produtos WHERE marca = 'Hurricane' LIMIT 5;` shows non-null descriptions
3. `SELECT * FROM imagens_produto LIMIT 10;` shows rows with UUID paths
4. Image files exist under `STATIC_DIR` with those UUID filenames
5. Re-running `docker compose up produtos-scraper` exits immediately (all products already enriched)

---

## Out of Scope

- Scraping any marca other than Hurricane in this iteration (architecture supports it, adapters not yet written)
- Periodic re-enrichment / refresh of existing data
- Image resizing or format conversion
- Admin UI for triggering scrapes
