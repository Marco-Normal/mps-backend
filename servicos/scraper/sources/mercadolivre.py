from __future__ import annotations

import logging

import httpx

from adapters import ScrapedData

logger = logging.getLogger(__name__)

_SEARCH_URL = "https://api.mercadolibre.com/sites/MLB/search"
_PICTURES_URL = "https://api.mercadolibre.com/items/{item_id}/pictures"
_TIMEOUT = 15.0


async def search_mercadolivre(
    product_name: str, num_fab: str | None,
) -> ScrapedData | None:
    query = num_fab if num_fab else product_name

    async with httpx.AsyncClient() as client:
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
            return None

        results: list[dict] = data.get("results", [])
        if not isinstance(results, list) or not results:
            logger.info("ML: no results for %r", query)
            return None

        item = results[0]
        item_id: str | None = item.get("id")
        if not item_id:
            logger.warning("ML: item missing 'id' field for query %r", query)
            return None

        description: str | None = item.get("title") or None
        image_urls = await _fetch_pictures(item_id, client)

        if not description and not image_urls:
            return None

        return ScrapedData(descricao=description, image_urls=image_urls)


async def _fetch_pictures(item_id: str, client: httpx.AsyncClient) -> list[str]:
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

    return [pic["url"] for pic in pictures if "url" in pic]
