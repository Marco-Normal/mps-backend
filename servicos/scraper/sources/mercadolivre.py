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
    except (httpx.HTTPError, ValueError) as exc:
        logger.warning("ML search failed for %r: %s", query, exc)
        return None, []

    results: list[dict] = data.get("results", [])
    if not results:
        logger.info("ML: no results for %r", query)
        return None, []

    item = results[0]
    item_id: str | None = item.get("id")
    if not item_id:
        logger.warning("ML: item missing 'id' field for query %r", query)
        return None, []
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
    except (httpx.HTTPError, ValueError) as exc:
        logger.warning("ML pictures fetch failed for item %s: %s", item_id, exc)
        return []

    if not isinstance(pictures, list):
        logger.warning("ML: unexpected pictures response for item %s", item_id)
        return []

    return [pic["url"] for pic in pictures if "url" in pic][:3]
