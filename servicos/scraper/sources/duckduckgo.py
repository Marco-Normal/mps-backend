from __future__ import annotations

import logging

import httpx
from bs4 import BeautifulSoup

from adapters import ScrapedData

logger = logging.getLogger(__name__)

_DDG_URL = "https://html.duckduckgo.com/html/"
_MIN_DESCRIPTION_LEN = 20
_TIMEOUT = 15.0


async def search_duckduckgo(
    product_name: str, num_fab: str | None,
) -> ScrapedData | None:
    query = num_fab if num_fab else product_name

    async with httpx.AsyncClient() as client:
        try:
            resp = await client.get(
                _DDG_URL,
                params={"q": query},
                timeout=_TIMEOUT,
            )
            resp.raise_for_status()
        except (httpx.HTTPError, ValueError) as exc:
            logger.warning("DuckDuckGo search failed for %r: %s", query, exc)
            return None

    soup = BeautifulSoup(resp.text, "html.parser")
    snippets: list[str] = []
    for el in soup.select(".result__snippet"):
        text = el.get_text(separator=" ", strip=True)
        if text:
            snippets.append(text)

    if not snippets:
        logger.debug("DuckDuckGo: no snippets found for %r", query)
        return None

    description = "\n".join(snippets[:3])
    if len(description) < _MIN_DESCRIPTION_LEN:
        logger.debug("DuckDuckGo: description too short (%d chars) for %r", len(description), query)
        return None

    return ScrapedData(descricao=description, image_urls=[])
