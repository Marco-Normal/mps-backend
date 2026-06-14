# Product Enrichment Scraper — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone Python Docker service that scrapes descriptions and up to 3 images per Hurricane product from `hurricanesound.com.br` (Mercado Livre fallback), downloads images to a shared volume as UUID files, and writes results to `descricao` + `imagens_produto` in the `produtos` DB.

**Architecture:** Brand-pluggable orchestrator (`main.py`) queries for unenriched products (`descricao IS NULL`), calls brand adapters (Playwright + BeautifulSoup for manufacturer sites, httpx for Mercado Livre REST API), downloads images as UUID files to a shared Docker named volume, and updates the DB per-product. Each product is committed individually so partial runs resume cleanly.

**Tech Stack:** Python 3.12, asyncpg, Playwright (Chromium headless), BeautifulSoup4, httpx, Docker named volume `produtos_static`.

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `docker-compose.yml` | Modify | Fix existing YAML bugs; add `produtos_static` volume + env to `produtos-api`; add `produtos-scraper` service |
| `servicos/scraper/Dockerfile` | Create | Python 3.12-slim image with Playwright Chromium |
| `servicos/scraper/requirements.txt` | Create | Pinned Python dependencies |
| `servicos/scraper/adapters/__init__.py` | Create | `ScrapedData` dataclass, `BrandAdapter` ABC, `REGISTRY` dict |
| `servicos/scraper/adapters/hurricane.py` | Create | Hurricane adapter: Playwright scrape of hurricanesound.com.br + ML fallback |
| `servicos/scraper/sources/__init__.py` | Create | Empty package marker |
| `servicos/scraper/sources/mercadolivre.py` | Create | Generic Mercado Livre REST API source (brand-agnostic) |
| `servicos/scraper/main.py` | Create | Orchestrator: arg parsing, DB connection, image download, per-product loop |

---

## Task 1: Fix docker-compose.yml and introduce static volume

The existing file has structural YAML bugs: `pedidos-db` is indented as if it's a port entry under `produtos-api`; the last service is named `produtos-api` but should be `pedidos-api`. Fix all bugs, add `STATIC_DIR` + `produtos_static` volume mount to `produtos-api`, declare the named volume, and add the `produtos-scraper` service.

**Files:**
- Modify: `docker-compose.yml`

- [ ] **Step 1: Replace docker-compose.yml with the corrected version**

```yaml
name: mps

services:
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
      test: ["CMD-SHELL", "pg_isready -U ${PRODUTOS_POSTGRES_USER} -d ${PRODUTOS_DB_NAME}"]
      interval: 10s
      timeout: 5s
      retries: 5

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

  produtos-api:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        SERVICE_NAME: produtos
    container_name: mps-produtos-api
    depends_on:
      produtos-init:
        condition: service_completed_successfully
      produtos-db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://${APP_USER}:${APP_PASSWORD}@produtos-db:5432/${PRODUTOS_DB_NAME}
      STATIC_DIR: /static
    restart: unless-stopped
    ports:
      - "3000:3000"
    volumes:
      - produtos_static:/static

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
      test: ["CMD-SHELL", "pg_isready -U ${PEDIDOS_POSTGRES_USER} -d ${PEDIDOS_DB_NAME}"]
      interval: 10s
      timeout: 5s
      retries: 5

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

  pedidos-api:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        SERVICE_NAME: pedidos
    container_name: mps-pedidos-api
    depends_on:
      pedidos-init:
        condition: service_completed_successfully
      pedidos-db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://${APP_USER}:${APP_PASSWORD}@pedidos-db:5432/${PEDIDOS_DB_NAME}
    restart: unless-stopped
    ports:
      - "3001:3000"

  produtos-scraper:
    build:
      context: ./servicos/scraper
    container_name: mps-produtos-scraper
    restart: "no"
    environment:
      DATABASE_URL: postgres://${APP_USER}:${APP_PASSWORD}@produtos-db:5432/${PRODUTOS_DB_NAME}
      STATIC_DIR: /static
      MARCA: "Hurricane"
      SCRAPER_DELAY_MS: "500"
      LOG_LEVEL: INFO
    volumes:
      - produtos_static:/static
    depends_on:
      produtos-init:
        condition: service_completed_successfully

volumes:
  pg_produtos_data:
  pg_pedidos_data:
  produtos_static:
```

- [ ] **Step 2: Validate YAML syntax**

```bash
docker compose config --quiet
```

Expected: no output (valid, no errors)

- [ ] **Step 3: Commit**

```bash
git add docker-compose.yml
git commit -m "fix(compose): fix YAML bugs, add produtos_static volume, add scraper service"
```

---

## Task 2: Scaffold the scraper package

**Files:**
- Create: `servicos/scraper/Dockerfile`
- Create: `servicos/scraper/requirements.txt`
- Create: `servicos/scraper/adapters/__init__.py` (empty placeholder, filled in Task 3)
- Create: `servicos/scraper/sources/__init__.py` (empty)

- [ ] **Step 1: Create `servicos/scraper/Dockerfile`**

```dockerfile
FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt \
    && playwright install chromium --with-deps
COPY . .
CMD ["python", "main.py"]
```

- [ ] **Step 2: Create `servicos/scraper/requirements.txt`**

```
asyncpg==0.30.0
playwright==1.49.0
beautifulsoup4==4.12.3
httpx==0.28.0
```

- [ ] **Step 3: Create empty package markers**

`servicos/scraper/adapters/__init__.py` — leave empty (filled in Task 3).

`servicos/scraper/sources/__init__.py` — leave empty.

- [ ] **Step 4: Commit scaffold**

```bash
git add servicos/scraper/
git commit -m "feat(scraper): scaffold directory and Dockerfile"
```

---

## Task 3: Define adapter base, ScrapedData, and registry

**Files:**
- Modify: `servicos/scraper/adapters/__init__.py`

- [ ] **Step 1: Write `servicos/scraper/adapters/__init__.py`**

```python
from __future__ import annotations

import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)


@dataclass
class ScrapedData:
    """Holds scraped content for one product. image_urls is capped at 3."""

    descricao: str | None
    image_urls: list[str] = field(default_factory=list)

    def __post_init__(self) -> None:
        self.image_urls = self.image_urls[:3]


class BrandAdapter(ABC):
    """Base class for all brand-specific scrapers.

    Subclasses must set the class attribute `marca` to the exact DB string
    (e.g. "Hurricane") and implement `search`.
    """

    marca: str

    @abstractmethod
    async def search(
        self, product_name: str, num_fab: str | None
    ) -> ScrapedData | None:
        """Return ScrapedData on success, None if nothing was found."""
        ...


# Import concrete adapters AFTER the base class is defined to avoid circular imports.
from .hurricane import HurricaneAdapter  # noqa: E402

REGISTRY: dict[str, type[BrandAdapter]] = {
    "Hurricane": HurricaneAdapter,
    # Add new brand adapters here, e.g.:
    # "JBL": JBLAdapter,
}
```

- [ ] **Step 2: Commit**

```bash
git add servicos/scraper/adapters/__init__.py
git commit -m "feat(scraper): add BrandAdapter ABC, ScrapedData dataclass, and REGISTRY"
```

---

## Task 4: Implement Mercado Livre source

**Files:**
- Modify: `servicos/scraper/sources/mercadolivre.py`

Uses the public Mercado Livre REST API (no auth required):
1. `GET https://api.mercadolibre.com/sites/MLB/search?q={query}&limit=1` — find best-matching listing
2. `GET https://api.mercadolibre.com/items/{item_id}/pictures` — fetch full-resolution gallery

- [ ] **Step 1: Write `servicos/scraper/sources/mercadolivre.py`**

```python
from __future__ import annotations

import logging

import httpx

logger = logging.getLogger(__name__)

_SEARCH_URL = "https://api.mercadolibre.com/sites/MLB/search"
_PICTURES_URL = "https://api.mercadolibre.com/items/{item_id}/pictures"
_TIMEOUT = 15.0


async def search_mercadolivre(
    query: str, client: httpx.AsyncClient
) -> tuple[str | None, list[str]]:
    """Search Mercado Livre for a product.

    Args:
        query: Search string (e.g. "Hurricane ALTO FALANTE 6 TRIAK").
        client: Shared httpx.AsyncClient.

    Returns:
        (description, image_urls) — description may be None; image_urls has at most 3 entries.
        Returns (None, []) if nothing found or on network error.
    """
    try:
        resp = await client.get(
            _SEARCH_URL,
            params={"q": query, "limit": 1},
            timeout=_TIMEOUT,
        )
        resp.raise_for_status()
        data = resp.json()
    except httpx.HTTPError as exc:
        logger.warning("ML search failed for %r: %s", query, exc)
        return None, []

    results: list[dict] = data.get("results", [])
    if not results:
        logger.info("ML: no results for %r", query)
        return None, []

    item = results[0]
    item_id: str = item["id"]
    description: str | None = item.get("title") or None

    image_urls = await _fetch_pictures(item_id, client)
    return description, image_urls[:3]


async def _fetch_pictures(item_id: str, client: httpx.AsyncClient) -> list[str]:
    """Return up to 3 full-resolution image URLs for a Mercado Livre item."""
    try:
        resp = await client.get(
            _PICTURES_URL.format(item_id=item_id),
            timeout=_TIMEOUT,
        )
        resp.raise_for_status()
        pictures: list[dict] = resp.json()
    except httpx.HTTPError as exc:
        logger.warning("ML pictures fetch failed for item %s: %s", item_id, exc)
        return []

    return [pic["url"] for pic in pictures if "url" in pic][:3]
```

- [ ] **Step 2: Commit**

```bash
git add servicos/scraper/sources/mercadolivre.py
git commit -m "feat(scraper): add generic Mercado Livre source"
```

---

## Task 5: Implement Hurricane adapter

**Files:**
- Create: `servicos/scraper/adapters/hurricane.py`

Strategy:
1. Search `hurricanesound.com.br` using a WooCommerce search URL with `num_fab` (preferred) or `product_name`.
2. Open the first product link in the search results.
3. Extract description and gallery images using BeautifulSoup.
4. If any Playwright error or no product found, fall back to `search_mercadolivre("Hurricane {product_name}")`.

Both pages are fetched in a single browser instance per `search()` call.

- [ ] **Step 1: Create `servicos/scraper/adapters/hurricane.py`**

```python
from __future__ import annotations

import logging
import re

import httpx
from bs4 import BeautifulSoup
from playwright.async_api import async_playwright

from sources.mercadolivre import search_mercadolivre

from . import BrandAdapter, ScrapedData

logger = logging.getLogger(__name__)

_BASE = "https://www.hurricanesound.com.br"
_SEARCH = _BASE + "/?s={query}"


class HurricaneAdapter(BrandAdapter):
    marca = "Hurricane"

    async def search(
        self, product_name: str, num_fab: str | None
    ) -> ScrapedData | None:
        # Prefer manufacturer part number for more precise search
        query = num_fab if num_fab else product_name
        logger.info(
            "Hurricane: searching %r (num_fab=%r)", product_name, num_fab
        )

        result = await _scrape_manufacturer(query)
        if result is not None:
            logger.info("Hurricane: manufacturer scrape succeeded for %r", product_name)
            return result

        logger.info(
            "Hurricane: manufacturer scrape found nothing for %r, trying Mercado Livre",
            product_name,
        )
        return await _scrape_mercadolivre(product_name)


async def _scrape_manufacturer(query: str) -> ScrapedData | None:
    """Fetch search results and the first product page from hurricanesound.com.br.

    Returns ScrapedData if at least one of (description, images) is found,
    otherwise None.
    """
    search_url = _SEARCH.format(query=query.replace(" ", "+"))

    try:
        async with async_playwright() as p:
            browser = await p.chromium.launch(headless=True)
            try:
                # --- Step 1: search results page ---
                page = await browser.new_page()
                await page.goto(
                    search_url, timeout=20_000, wait_until="domcontentloaded"
                )
                search_html = await page.content()
                await page.close()

                product_url = _find_product_link(
                    BeautifulSoup(search_html, "html.parser")
                )
                if product_url is None:
                    logger.debug("Hurricane: no product link in search results for %r", query)
                    return None

                # --- Step 2: product detail page ---
                page = await browser.new_page()
                await page.goto(
                    product_url, timeout=20_000, wait_until="domcontentloaded"
                )
                product_html = await page.content()
                await page.close()
            finally:
                await browser.close()
    except Exception as exc:
        logger.warning("Hurricane: Playwright error for %r: %s", query, exc)
        return None

    soup = BeautifulSoup(product_html, "html.parser")
    description = _extract_description(soup)
    image_urls = _extract_images(soup)

    if not description and not image_urls:
        logger.debug("Hurricane: product page parsed but yielded no data for %r", query)
        return None

    return ScrapedData(descricao=description, image_urls=image_urls)


async def _scrape_mercadolivre(product_name: str) -> ScrapedData | None:
    """Call the generic Mercado Livre source as fallback."""
    async with httpx.AsyncClient() as client:
        description, image_urls = await search_mercadolivre(
            f"Hurricane {product_name}", client
        )
    if not description and not image_urls:
        return None
    return ScrapedData(descricao=description, image_urls=image_urls)


# ---------------------------------------------------------------------------
# HTML parsing helpers
# ---------------------------------------------------------------------------

def _find_product_link(soup: BeautifulSoup) -> str | None:
    """Return the URL of the first product in WooCommerce search results."""
    # Standard WooCommerce product loop selectors (try most-specific first)
    for selector in [
        "ul.products li.product a.woocommerce-LoopProduct-link",
        "ul.products li.product a",
        ".products .product a",
        "article.product a",
    ]:
        el = soup.select_one(selector)
        if el:
            href = el.get("href", "")
            if isinstance(href, str) and href.startswith("http"):
                return href

    # Generic fallback: first anchor inside any element with "product" in its class
    container = soup.find(class_=re.compile(r"\bproduct\b", re.I))
    if container:
        a = container.find("a", href=re.compile(r"^https?://"))
        if a:
            return a["href"]

    return None


def _extract_description(soup: BeautifulSoup) -> str | None:
    """Extract a human-readable description from a WooCommerce product page."""
    for selector in [
        ".woocommerce-product-details__short-description",
        "#tab-description p",
        ".product_description p",
        ".entry-content p",
    ]:
        el = soup.select_one(selector)
        if el:
            text = el.get_text(separator=" ", strip=True)
            if len(text) > 20:
                return text
    return None


def _extract_images(soup: BeautifulSoup) -> list[str]:
    """Extract up to 3 full-resolution product gallery images."""
    urls: list[str] = []

    for selector in [
        ".woocommerce-product-gallery__image a",  # href = full-res
        ".woocommerce-product-gallery img",        # src / data-src
        ".product-gallery img",
    ]:
        for el in soup.select(selector):
            src = (
                el.get("href")
                or el.get("data-large_image")
                or el.get("data-src")
                or el.get("src")
            )
            if not src or not isinstance(src, str):
                continue
            if not src.startswith("http"):
                continue
            # Skip WooCommerce thumbnails (-NNNxNNN. suffix)
            if re.search(r"-\d+x\d+\.(jpe?g|png|webp)", src):
                continue
            if src not in urls:
                urls.append(src)
            if len(urls) >= 3:
                return urls

    return urls
```

- [ ] **Step 2: Commit**

```bash
git add servicos/scraper/adapters/hurricane.py
git commit -m "feat(scraper): add Hurricane adapter (Playwright + Mercado Livre fallback)"
```

---

## Task 6: Implement the orchestrator

**Files:**
- Create: `servicos/scraper/main.py`

- [ ] **Step 1: Create `servicos/scraper/main.py`**

```python
from __future__ import annotations

import argparse
import asyncio
import logging
import mimetypes
import os
import uuid
from pathlib import Path

import asyncpg
import httpx

from adapters import REGISTRY, ScrapedData

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------

def _setup_logging() -> None:
    level_name = os.environ.get("LOG_LEVEL", "INFO").upper()
    level = getattr(logging, level_name, logging.INFO)
    logging.basicConfig(
        level=level,
        format="%(asctime)s %(levelname)-8s %(name)s: %(message)s",
    )


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Produto enrichment scraper")
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print what would be done without writing to DB or disk",
    )
    parser.add_argument(
        "--product-id",
        type=int,
        default=None,
        metavar="ID",
        help="Only process a single product by its DB id (must also have descricao IS NULL)",
    )
    return parser.parse_args()


# ---------------------------------------------------------------------------
# Image download
# ---------------------------------------------------------------------------

def _ext_from_content_type(content_type: str) -> str:
    """Return a normalised file extension for the given MIME type."""
    mime = content_type.split(";")[0].strip()
    ext = mimetypes.guess_extension(mime) or ".jpg"
    # mimetypes returns .jpe on some systems
    return ".jpg" if ext in (".jpe", ".jpeg") else ext


async def download_image(
    url: str, static_dir: Path, client: httpx.AsyncClient
) -> str | None:
    """Download one image and save it under static_dir as <uuid4>.<ext>.

    Returns the filename (not the full path) on success, None on failure.
    """
    try:
        resp = await client.get(url, timeout=15.0, follow_redirects=True)
        resp.raise_for_status()
    except httpx.HTTPError as exc:
        logger.warning("Image download failed for %s: %s", url, exc)
        return None

    ext = _ext_from_content_type(resp.headers.get("content-type", "image/jpeg"))
    filename = f"{uuid.uuid4()}{ext}"
    dest = static_dir / filename
    try:
        dest.write_bytes(resp.content)
    except OSError as exc:
        logger.error("Could not write image to %s: %s", dest, exc)
        return None

    logger.debug("Saved image %s (%d bytes)", filename, len(resp.content))
    return filename


# ---------------------------------------------------------------------------
# Per-product enrichment
# ---------------------------------------------------------------------------

async def enrich_product(
    product: dict,
    pool: asyncpg.Pool,
    static_dir: Path,
    dry_run: bool,
) -> None:
    product_id: int = product["id"]
    nome: str = product["nome"]
    marca: str = product["marca"]
    num_fab: str | None = product["num_fab"]

    adapter_class = REGISTRY.get(marca)
    if adapter_class is None:
        logger.warning(
            "No adapter registered for marca=%r — skipping product id=%d",
            marca, product_id,
        )
        return

    adapter = adapter_class()
    logger.info("Enriching product id=%d nome=%r marca=%r", product_id, nome, marca)

    scraped: ScrapedData | None = await adapter.search(nome, num_fab)

    if scraped is None or (not scraped.descricao and not scraped.image_urls):
        logger.warning("No data found for product id=%d nome=%r", product_id, nome)
        return

    # Use product name as description fallback so the product is marked as
    # enriched and not retried on subsequent runs.
    description = scraped.descricao or nome

    if dry_run:
        logger.info(
            "[DRY RUN] id=%d: descricao=%r, %d image(s): %s",
            product_id, description, len(scraped.image_urls), scraped.image_urls,
        )
        return

    # Download images
    async with httpx.AsyncClient() as client:
        filenames: list[str] = []
        for url in scraped.image_urls:
            filename = await download_image(url, static_dir, client)
            if filename:
                filenames.append(filename)

    # Write to DB in a single transaction
    async with pool.acquire() as conn:
        async with conn.transaction():
            await conn.execute(
                "UPDATE produtos SET descricao = $1 WHERE id = $2",
                description,
                product_id,
            )
            for filename in filenames:
                await conn.execute(
                    "INSERT INTO imagens_produto (id_produto, path) VALUES ($1, $2)",
                    product_id,
                    filename,
                )

    logger.info(
        "Enriched id=%d: descricao set, %d image(s) saved",
        product_id, len(filenames),
    )


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

async def run(args: argparse.Namespace) -> None:
    database_url = os.environ["DATABASE_URL"]
    static_dir = Path(os.environ["STATIC_DIR"])
    marcas_raw = os.environ.get("MARCA", "Hurricane")
    delay_ms = int(os.environ.get("SCRAPER_DELAY_MS", "500"))
    marcas = [m.strip() for m in marcas_raw.split(",") if m.strip()]

    static_dir.mkdir(parents=True, exist_ok=True)

    pool: asyncpg.Pool = await asyncpg.create_pool(
        database_url, min_size=1, max_size=3
    )
    try:
        if args.product_id is not None:
            rows = await pool.fetch(
                "SELECT id, nome, marca, num_fab FROM produtos"
                " WHERE id = $1 AND descricao IS NULL",
                args.product_id,
            )
        else:
            rows = await pool.fetch(
                "SELECT id, nome, marca, num_fab FROM produtos"
                " WHERE marca = ANY($1) AND descricao IS NULL",
                marcas,
            )

        products = [dict(r) for r in rows]
        logger.info("Found %d product(s) to enrich", len(products))

        if not products:
            logger.info("Nothing to do — all target products are already enriched.")
            return

        for i, product in enumerate(products):
            await enrich_product(product, pool, static_dir, args.dry_run)
            if i < len(products) - 1:
                await asyncio.sleep(delay_ms / 1000.0)

    finally:
        await pool.close()


def main() -> None:
    _setup_logging()
    args = _parse_args()
    asyncio.run(run(args))


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Commit**

```bash
git add servicos/scraper/main.py
git commit -m "feat(scraper): add orchestrator with dry-run and --product-id flags"
```

---

## Task 7: Build and verify

- [ ] **Step 1: Build the scraper Docker image**

```bash
docker compose build produtos-scraper
```

Expected: Build completes without errors. Playwright installs Chromium. Final image ~1.5GB.

- [ ] **Step 2: Bring up the products DB and run init**

```bash
docker compose up -d produtos-db
docker compose up produtos-init
```

Check init completed successfully:

```bash
docker compose logs produtos-init | tail -5
```

Expected: last line contains "Initialization complete" (or similar from the Rust init binary).

- [ ] **Step 3: Dry-run to verify scraper can reach sources**

```bash
docker compose run --rm produtos-scraper python main.py --dry-run
```

Expected: Logs like:
```
Found 27 product(s) to enrich
[DRY RUN] id=108: descricao='...', 1 image(s): ['https://...']
...
```
No DB writes. No image files created.

- [ ] **Step 4: Enrich a single product**

Run against product id 108 (`ALTO FALANTE 6 TRIAK TRIAXIAL 4 OHMS`, num_fab `F01.201`):

```bash
docker compose run --rm produtos-scraper python main.py --product-id 108
```

Expected log: "Enriched id=108: descricao set, N image(s) saved"

Verify in DB:

```bash
docker compose exec produtos-db psql \
  -U ${APP_USER} -d ${PRODUTOS_DB_NAME} \
  -c "SELECT id, LEFT(descricao, 80) AS descricao FROM produtos WHERE id = 108;"
```

Expected: non-null `descricao` value.

```bash
docker compose exec produtos-db psql \
  -U ${APP_USER} -d ${PRODUTOS_DB_NAME} \
  -c "SELECT id_produto, path, created_at FROM imagens_produto WHERE id_produto = 108;"
```

Expected: 1–3 rows with UUID filenames like `3f2e1a4b-...-jpg`.

- [ ] **Step 5: Run the full Hurricane batch**

```bash
docker compose up produtos-scraper
```

Expected: exits with code 0 after processing all 27 Hurricane products.

- [ ] **Step 6: Verify idempotency**

```bash
docker compose up produtos-scraper
```

Expected: logs show "Found 0 product(s) to enrich — Nothing to do" and exits immediately.

- [ ] **Step 7: Final commit**

```bash
git add .
git commit -m "feat(scraper): complete product enrichment scraper"
```
