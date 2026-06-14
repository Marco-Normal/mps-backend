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
