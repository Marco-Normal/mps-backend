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

# Playwright launch args required when running as non-root inside Docker
_PLAYWRIGHT_ARGS = ["--no-sandbox", "--disable-setuid-sandbox"]


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
            browser = await p.chromium.launch(headless=True, args=_PLAYWRIGHT_ARGS)
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
