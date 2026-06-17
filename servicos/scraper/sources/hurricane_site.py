from __future__ import annotations

import logging
import re
from urllib.parse import quote_plus

from bs4 import BeautifulSoup
from playwright.async_api import async_playwright

from adapters import ScrapedData

logger = logging.getLogger(__name__)

_BASE = "https://www.hurricanesound.com.br"
_SEARCH = _BASE + "/?s={query}"
_MIN_DESCRIPTION_LEN = 20
_PLAYWRIGHT_ARGS = ["--no-sandbox", "--disable-setuid-sandbox"]


async def search_hurricane_site(
    product_name: str, num_fab: str | None,
) -> ScrapedData | None:
    query = num_fab if num_fab else product_name
    search_url = _SEARCH.format(query=quote_plus(query))

    try:
        async with async_playwright() as p:
            browser = await p.chromium.launch(headless=True, args=_PLAYWRIGHT_ARGS)
            try:
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
                    logger.debug(
                        "Hurricane: no product link in search results for %r", query
                    )
                    return None

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


def _find_product_link(soup: BeautifulSoup) -> str | None:
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

    container = soup.find(class_=re.compile(r"\bproduct\b", re.I))
    if container:
        a = container.find("a", href=re.compile(r"^https?://"))
        if a:
            href = a.get("href")
            if href and isinstance(href, str):
                return href

    return None


def _extract_description(soup: BeautifulSoup) -> str | None:
    for selector in [
        ".woocommerce-product-details__short-description",
        "#tab-description p",
        ".product_description p",
        ".entry-content p",
    ]:
        el = soup.select_one(selector)
        if el:
            text = el.get_text(separator=" ", strip=True)
            if len(text) > _MIN_DESCRIPTION_LEN:
                return text
    return None


def _extract_images(soup: BeautifulSoup) -> list[str]:
    urls: list[str] = []

    for selector in [
        ".woocommerce-product-gallery__image a",
        ".woocommerce-product-gallery img",
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
            if re.search(r"-\d+x\d+\.(jpe?g|png|webp)", src):
                continue
            if src not in urls:
                urls.append(src)
            if len(urls) >= 3:
                return urls

    return urls
