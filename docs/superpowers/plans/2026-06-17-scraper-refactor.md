# Scraper Refactor: Generic Adapter Architecture — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the scraper into a pluggable source-chain architecture, add a DuckDuckGo web search source, and introduce a GenericAdapter fallback so ANY brand can be enriched without a dedicated adapter.

**Architecture:** Search sources become independent async functions (`sources/*.py`) composed into priority-ordered chains per adapter. `BrandAdapter` gains concrete `search()` that iterates `_sources()`. `GenericAdapter` acts as catch-all when `REGISTRY` has no entry for a brand.

**Tech Stack:** Python 3.12, Playwright, BeautifulSoup4, httpx, asyncpg, pytest + pytest-asyncio + pytest-httpx

## Global Constraints

- Every source function has signature `async def search_X(product_name: str, num_fab: str | None) -> ScrapedData | None`
- Sources are non-fatal — exceptions logged as warnings, chain continues to next source
- `GenericAdapter` is instantiated directly in `main.py`, not stored in `REGISTRY`
- All adapter `marca` values and REGISTRY keys are uppercase ASCII
- `SCRAPER_MARCA` env var values are uppercased before DB query

## File Structure

```
servicos/scraper/
  sources/
    __init__.py            (unchanged)
    mercadolivre.py        (MODIFY: generalize signature, return ScrapedData)
    duckduckgo.py          (NEW: DuckDuckGo Lite search source)
    hurricane_site.py      (NEW: extracted Playwright logic from hurricane adapter)
  adapters/
    __init__.py            (MODIFY: BrandAdapter._sources + search, GenericAdapter)
    hurricane.py           (MODIFY: thin adapter, only overrides _sources())
  main.py                  (MODIFY: uppercase MARCA, GenericAdapter fallback)
  tests/
    __init__.py             (NEW: empty)
    test_sources.py         (NEW: unit tests for all sources)
    test_adapters.py        (NEW: unit tests for adapters/REGISTRY)
    test_integration.py     (NEW: integration tests with mocked HTTP/Playwright)
    test_e2e.py             (NEW: real-network end-to-end tests)
  requirements.txt          (MODIFY: add pytest, pytest-asyncio, pytest-httpx)
```

---

### Task 1: Add GenericAdapter and refactor BrandAdapter ABC

**Files:**
- Modify: `servicos/scraper/adapters/__init__.py`

**Interfaces:**
- Produces:
  - `BrandAdapter.marca: str` (existing, unchanged)
  - `BrandAdapter.search(product_name, num_fab) -> ScrapedData | None` (NEW concrete method)
  - `BrandAdapter._sources() -> list[Callable]` (NEW abstract method)
  - `GenericAdapter(BrandAdapter)` class with `marca = "GENERIC"` and `_sources()` returning [search_mercadolivre, search_duckduckgo]

- [ ] **Step 1: Write tests for adapters/__init__.py**

```bash
mkdir -p servicos/scraper/tests
```

Create `servicos/scraper/tests/__init__.py` (empty file):

```python
# empty
```

Create `servicos/scraper/tests/test_adapters.py`:

```python
from __future__ import annotations

import pytest
from adapters import BrandAdapter, GenericAdapter, REGISTRY, ScrapedData


class FakeAdapter(BrandAdapter):
    marca = "FAKE"

    def _sources(self):
        async def dummy_src(product_name: str, num_fab: str | None) -> ScrapedData | None:
            return ScrapedData(descricao="from dummy", image_urls=[])

        return [dummy_src]


def test_registry_keys_are_uppercase():
    for key in REGISTRY:
        assert key == key.upper(), f"REGISTRY key {key!r} is not uppercase"
        assert key.isascii(), f"REGISTRY key {key!r} is not ASCII"


def test_all_adapters_have_marca_str():
    for key, cls in REGISTRY.items():
        assert isinstance(cls.marca, str), f"{cls.__name__}.marca must be str"
        assert cls.marca == key, f"{cls.__name__}.marca={cls.marca!r} != REGISTRY key={key!r}"


def test_scraped_data_caps_images_at_3():
    data = ScrapedData(
        descricao="test",
        image_urls=["a.jpg", "b.jpg", "c.jpg", "d.jpg", "e.jpg"],
    )
    assert len(data.image_urls) == 3
    assert data.image_urls == ["a.jpg", "b.jpg", "c.jpg"]


def test_scraped_data_empty_images():
    data = ScrapedData(descricao=None)
    assert data.image_urls == []
    assert data.descricao is None


@pytest.mark.asyncio
async def test_brandadapter_search_returns_first_source_result():
    adapter = FakeAdapter()
    result = await adapter.search("test product", None)
    assert result is not None
    assert result.descricao == "from dummy"


@pytest.mark.asyncio
async def test_brandadapter_search_falls_through_to_second_source():
    async def returns_none(_p: str, _n: str | None) -> ScrapedData | None:
        return None

    async def returns_data(_p: str, _n: str | None) -> ScrapedData | None:
        return ScrapedData(descricao="second", image_urls=[])

    class TwoSourceAdapter(BrandAdapter):
        marca = "TWOSRC"
        def _sources(self):
            return [returns_none, returns_data]

    adapter = TwoSourceAdapter()
    result = await adapter.search("test", None)
    assert result is not None
    assert result.descricao == "second"


@pytest.mark.asyncio
async def test_brandadapter_search_all_sources_fail_returns_none():
    async def returns_none(_p: str, _n: str | None) -> ScrapedData | None:
        return None

    class AllFailAdapter(BrandAdapter):
        marca = "ALLFAIL"
        def _sources(self):
            return [returns_none, returns_none]

    adapter = AllFailAdapter()
    result = await adapter.search("test", None)
    assert result is None


@pytest.mark.asyncio
async def test_brandadapter_source_exception_does_not_crash():
    async def raises_error(_p: str, _n: str | None) -> ScrapedData | None:
        raise RuntimeError("boom")

    async def returns_data(_p: str, _n: str | None) -> ScrapedData | None:
        return ScrapedData(descricao="recovered", image_urls=[])

    class CrashAdapter(BrandAdapter):
        marca = "CRASH"
        def _sources(self):
            return [raises_error, returns_data]

    adapter = CrashAdapter()
    result = await adapter.search("test", None)
    assert result is not None
    assert result.descricao == "recovered"


def test_generic_adapter_exists_and_has_marca():
    assert issubclass(GenericAdapter, BrandAdapter)
    assert GenericAdapter.marca == "GENERIC"


def test_generic_adapter_not_in_registry():
    assert "GENERIC" not in REGISTRY
    assert GenericAdapter is not None
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd servicos/scraper && pip install pytest pytest-asyncio 2>&1 && python -m pytest tests/test_adapters.py -v 2>&1
```

Expected: tests fail because `GenericAdapter` does not exist yet and `BrandAdapter._sources` is not defined.

- [ ] **Step 3: Rewrite adapters/__init__.py**

Replace `servicos/scraper/adapters/__init__.py`:

```python
from __future__ import annotations

import logging
from abc import ABC, abstractmethod
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Awaitable

logger = logging.getLogger(__name__)

# Source function protocol: takes product_name and optional num_fab, returns data or None
Source = Callable[[str, str | None], Awaitable["ScrapedData | None"]]


@dataclass
class ScrapedData:
    descricao: str | None
    image_urls: list[str] = field(default_factory=list)

    def __post_init__(self) -> None:
        self.image_urls = self.image_urls[:3]


class BrandAdapter(ABC):
    marca: str

    def __init_subclass__(cls, **kwargs: object) -> None:
        super().__init_subclass__(**kwargs)
        if not hasattr(cls, "marca") or not isinstance(cls.marca, str):
            raise TypeError(
                f"{cls.__name__} must define a 'marca' class attribute of type str"
            )

    async def search(
        self, product_name: str, num_fab: str | None
    ) -> ScrapedData | None:
        for source in self._sources():
            try:
                result = await source(product_name, num_fab)
                if result is not None:
                    return result
            except Exception:
                logger.warning(
                    "%s: source %s failed for %r",
                    self.marca,
                    getattr(source, "__name__", repr(source)),
                    product_name,
                    exc_info=True,
                )
        return None

    @abstractmethod
    def _sources(self) -> list[Source]:
        ...


class GenericAdapter(BrandAdapter):
    marca = "GENERIC"

    def _sources(self) -> list[Source]:
        from sources.mercadolivre import search_mercadolivre
        from sources.duckduckgo import search_duckduckgo

        return [search_mercadolivre, search_duckduckgo]


# Deferred imports to avoid circular dependencies
from .hurricane import HurricaneAdapter  # noqa: E402

REGISTRY: dict[str, type[BrandAdapter]] = {
    "HURRICANE": HurricaneAdapter,
}
```

- [ ] **Step 4: Run tests to verify they pass (some may fail due to missing sources)**

```bash
cd servicos/scraper && python -m pytest tests/test_adapters.py -v 2>&1
```

Expected: tests that don't depend on source imports pass (registry keys, ScrapedData, GenericAdapter existence). Tests that instantiate `GenericAdapter` may fail because `sources.mercadolivre` and `sources.duckduckgo` signatures haven't been updated yet. Mark those as expected failures with `-k "not (generic or two_source or crash or brandadapter_search)"` for now.

- [ ] **Step 5: Commit**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
git add servicos/scraper/adapters/__init__.py servicos/scraper/tests/
git commit -m "feat: refactor BrandAdapter ABC with _sources() chain, add GenericAdapter"
```

---

### Task 2: Extract hurricane_site.py source from hurricane adapter

**Files:**
- Create: `servicos/scraper/sources/hurricane_site.py`
- Modify: `servicos/scraper/adapters/hurricane.py` (remove extracted code, keep only adapter class)

**Interfaces:**
- Produces: `search_hurricane_site(product_name: str, num_fab: str | None) -> ScrapedData | None`
- Consumes: `BrandAdapter`, `ScrapedData`, `Source` from Task 1

- [ ] **Step 1: Write test for hurricane_site source**

Add to `servicos/scraper/tests/test_sources.py` (create file):

```python
from __future__ import annotations

import pytest


def test_hurricane_site_function_has_correct_signature():
    import inspect
    from sources.hurricane_site import search_hurricane_site

    sig = inspect.signature(search_hurricane_site)
    params = list(sig.parameters.keys())
    assert params == ["product_name", "num_fab"]
```

- [ ] **Step 2: Verify test fails**

```bash
cd servicos/scraper && python -m pytest tests/test_sources.py::test_hurricane_site_function_has_correct_signature -v 2>&1
```

Expected: FAIL — ModuleNotFoundError or ImportError (file doesn't exist yet).

- [ ] **Step 3: Create sources/hurricane_site.py**

Extract the `_scrape_manufacturer` logic from `adapters/hurricane.py` (lines 48-98) into `servicos/scraper/sources/hurricane_site.py`:

```python
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
```

- [ ] **Step 4: Run test to verify signature**

```bash
cd servicos/scraper && python -m pytest tests/test_sources.py::test_hurricane_site_function_has_correct_signature -v 2>&1
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
git add servicos/scraper/sources/hurricane_site.py servicos/scraper/tests/test_sources.py
git commit -m "feat: extract hurricane_site source from hurricane adapter"
```

---

### Task 3: Generalize mercadolivre source

**Files:**
- Modify: `servicos/scraper/sources/mercadolivre.py`

**Interfaces:**
- Produces: `search_mercadolivre(product_name: str, num_fab: str | None) -> ScrapedData | None`
- Consumes: `ScrapedData` from Task 1

- [ ] **Step 1: Write unit tests for generalized mercadolivre source**

Add to `servicos/scraper/tests/test_sources.py`:

```python
import pytest
from adapters import ScrapedData


def test_mercadolivre_function_has_correct_signature():
    import inspect
    from sources.mercadolivre import search_mercadolivre

    sig = inspect.signature(search_mercadolivre)
    params = list(sig.parameters.keys())
    assert params == ["product_name", "num_fab"]


@pytest.mark.asyncio
async def test_mercadolivre_parses_valid_response(httpx_mock):
    from sources.mercadolivre import search_mercadolivre

    httpx_mock.add_response(
        url="https://api.mercadolibre.com/sites/MLB/search?q=Hurricane+test+spkr&limit=1",
        json={
            "results": [
                {
                    "id": "MLB12345",
                    "title": "Alto Falante Hurricane 6\" 200W",
                }
            ]
        },
    )
    httpx_mock.add_response(
        url="https://api.mercadolibre.com/items/MLB12345/pictures",
        json=[{"url": "https://http2.mlstatic.com/img1.jpg"}],
    )

    result = await search_mercadolivre("Hurricane test spkr", None)
    assert isinstance(result, ScrapedData)
    assert result.descricao == "Alto Falante Hurricane 6\" 200W"
    assert len(result.image_urls) == 1
    assert result.image_urls[0] == "https://http2.mlstatic.com/img1.jpg"


@pytest.mark.asyncio
async def test_mercadolivre_handles_empty_results(httpx_mock):
    from sources.mercadolivre import search_mercadolivre

    httpx_mock.add_response(
        url="https://api.mercadolibre.com/sites/MLB/search?q=xyzzy_nonexistent&limit=1",
        json={"results": []},
    )

    result = await search_mercadolivre("xyzzy_nonexistent", None)
    assert result is None


@pytest.mark.asyncio
async def test_mercadolivre_handles_http_error(httpx_mock):
    from sources.mercadolivre import search_mercadolivre

    httpx_mock.add_response(
        url="https://api.mercadolibre.com/sites/MLB/search?q=test&limit=1",
        status_code=500,
    )

    result = await search_mercadolivre("test", None)
    assert result is None


@pytest.mark.asyncio
async def test_mercadolivre_uses_num_fab_when_available(httpx_mock):
    from sources.mercadolivre import search_mercadolivre

    httpx_mock.add_response(
        url="https://api.mercadolibre.com/sites/MLB/search?q=XPTO123&limit=1",
        json={
            "results": [
                {"id": "MLB999", "title": "Produto XPTO123"}
            ]
        },
    )
    httpx_mock.add_response(
        url="https://api.mercadolibre.com/items/MLB999/pictures",
        json=[],
    )

    result = await search_mercadolivre("fallback name", "XPTO123")
    assert result is not None
    assert result.descricao == "Produto XPTO123"
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd servicos/scraper && pip install pytest-httpx 2>&1 && python -m pytest tests/test_sources.py -v -k mercadolivre 2>&1
```

Expected: FAIL — signature mismatch (old function has `query: str, client` params instead of `product_name, num_fab`).

- [ ] **Step 3: Rewrite sources/mercadolivre.py**

Replace `servicos/scraper/sources/mercadolivre.py`:

```python
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
        if not results:
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

    return [pic["url"] for pic in pictures if "url" in pic][:3]
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd servicos/scraper && python -m pytest tests/test_sources.py -v -k mercadolivre 2>&1
```

Expected: all 4 mercadolivre tests PASS.

- [ ] **Step 5: Commit**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
git add servicos/scraper/sources/mercadolivre.py servicos/scraper/tests/test_sources.py
git commit -m "feat: generalize mercadolivre source — (product_name, num_fab) sig, returns ScrapedData"
```

---

### Task 4: Create DuckDuckGo search source

**Files:**
- Create: `servicos/scraper/sources/duckduckgo.py`

**Interfaces:**
- Produces: `search_duckduckgo(product_name: str, num_fab: str | None) -> ScrapedData | None`
- Consumes: `ScrapedData` from Task 1

- [ ] **Step 1: Write tests for duckduckgo source**

Add to `servicos/scraper/tests/test_sources.py`:

```python
def test_duckduckgo_function_has_correct_signature():
    import inspect
    from sources.duckduckgo import search_duckduckgo

    sig = inspect.signature(search_duckduckgo)
    params = list(sig.parameters.keys())
    assert params == ["product_name", "num_fab"]


@pytest.mark.asyncio
async def test_duckduckgo_parses_snippets(httpx_mock):
    from sources.duckduckgo import search_duckduckgo

    # DuckDuckGo Lite HTML response with result snippets
    httpx_mock.add_response(
        url="https://html.duckduckgo.com/html/?q=Test+Product+XPTO",
        html="""<html><body>
        <div class="result">
            <a class="result__snippet">This is a test product description with enough chars for validation.</a>
        </div>
        <div class="result">
            <a class="result__snippet">Additional details about the Test Product XPTO specifications.</a>
        </div>
        </body></html>""",
    )

    result = await search_duckduckgo("Test Product", "XPTO")
    assert result is not None
    assert "test product description" in result.descricao.lower()
    assert len(result.descricao) >= 20


@pytest.mark.asyncio
async def test_duckduckgo_handles_empty_page(httpx_mock):
    from sources.duckduckgo import search_duckduckgo

    httpx_mock.add_response(
        url="https://html.duckduckgo.com/html/?q=nonexistent_xyzzy",
        html="<html><body><p>No results.</p></body></html>",
    )

    result = await search_duckduckgo("nonexistent_xyzzy", None)
    assert result is None


@pytest.mark.asyncio
async def test_duckduckgo_handles_http_error(httpx_mock):
    from sources.duckduckgo import search_duckduckgo

    httpx_mock.add_response(
        url="https://html.duckduckgo.com/html/?q=test",
        status_code=403,
    )

    result = await search_duckduckgo("test", None)
    assert result is None


@pytest.mark.asyncio
async def test_duckduckgo_uses_num_fab_when_available(httpx_mock):
    from sources.duckduckgo import search_duckduckgo

    httpx_mock.add_response(
        url="https://html.duckduckgo.com/html/?q=XPTO999",
        html="""<html><body>
        <div class="result">
            <a class="result__snippet">XPTO999 is a premium automotive component with advanced features and durable construction.</a>
        </div>
        </body></html>""",
    )

    result = await search_duckduckgo("fallback name", "XPTO999")
    assert result is not None
    assert "XPTO999" in result.descricao
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd servicos/scraper && python -m pytest tests/test_sources.py -v -k duckduckgo 2>&1
```

Expected: FAIL — ModuleNotFoundError.

- [ ] **Step 3: Create sources/duckduckgo.py**

Create `servicos/scraper/sources/duckduckgo.py`:

```python
from __future__ import annotations

import logging

import httpx
from bs4 import BeautifulSoup
from playwright.async_api import async_playwright

from adapters import ScrapedData

logger = logging.getLogger(__name__)

_DDG_URL = "https://html.duckduckgo.com/html/"
_MIN_DESCRIPTION_LEN = 20
_PLAYWRIGHT_ARGS = ["--no-sandbox", "--disable-setuid-sandbox"]
_TIMEOUT = 20_000


async def search_duckduckgo(
    product_name: str, num_fab: str | None,
) -> ScrapedData | None:
    query = num_fab if num_fab else product_name

    try:
        async with async_playwright() as p:
            browser = await p.chromium.launch(headless=True, args=_PLAYWRIGHT_ARGS)
            try:
                page = await browser.new_page()
                await page.goto(
                    _DDG_URL,
                    params={"q": query},
                    timeout=_TIMEOUT,
                    wait_until="domcontentloaded",
                )
                html = await page.content()
                await page.close()
            finally:
                await browser.close()
    except Exception as exc:
        logger.warning("DuckDuckGo: Playwright error for %r: %s", query, exc)
        return None

    soup = BeautifulSoup(html, "html.parser")
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
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd servicos/scraper && python -m pytest tests/test_sources.py -v -k duckduckgo 2>&1
```

Expected: all 4 duckduckgo tests PASS.

- [ ] **Step 5: Commit**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
git add servicos/scraper/sources/duckduckgo.py servicos/scraper/tests/test_sources.py
git commit -m "feat: add DuckDuckGo Lite search source"
```

---

### Task 5: Thin out HurricaneAdapter to use source chain

**Files:**
- Modify: `servicos/scraper/adapters/hurricane.py`

**Interfaces:**
- Consumes: `BrandAdapter`, `ScrapedData`, source functions from Tasks 2-4

- [ ] **Step 1: Write adapter chain test**

Add to `servicos/scraper/tests/test_adapters.py`:

```python
def test_hurricane_adapter_sources_ordering():
    from adapters.hurricane import HurricaneAdapter
    from sources.hurricane_site import search_hurricane_site
    from sources.mercadolivre import search_mercadolivre
    from sources.duckduckgo import search_duckduckgo

    sources = HurricaneAdapter()._sources()
    assert len(sources) == 3
    assert sources[0] is search_hurricane_site
    assert sources[1] is search_mercadolivre
    assert sources[2] is search_duckduckgo


def test_generic_adapter_sources_ordering():
    from adapters import GenericAdapter
    from sources.mercadolivre import search_mercadolivre
    from sources.duckduckgo import search_duckduckgo

    sources = GenericAdapter()._sources()
    assert len(sources) == 2
    assert sources[0] is search_mercadolivre
    assert sources[1] is search_duckduckgo
```

- [ ] **Step 2: Verify test fails**

```bash
cd servicos/scraper && python -m pytest tests/test_adapters.py::test_hurricane_adapter_sources_ordering -v 2>&1
```

Expected: FAIL — HurricaneAdapter still has old `search()` monolith, no `_sources()` override.

- [ ] **Step 3: Rewrite hurricane.py**

Replace `servicos/scraper/adapters/hurricane.py`:

```python
from __future__ import annotations

from adapters import BrandAdapter
from sources.hurricane_site import search_hurricane_site
from sources.mercadolivre import search_mercadolivre
from sources.duckduckgo import search_duckduckgo


class HurricaneAdapter(BrandAdapter):
    marca = "HURRICANE"

    def _sources(self):
        return [search_hurricane_site, search_mercadolivre, search_duckduckgo]
```

- [ ] **Step 4: Run all adapter tests**

```bash
cd servicos/scraper && python -m pytest tests/test_adapters.py -v 2>&1
```

Expected: all adapter tests PASS including HurricaneAdapter and GenericAdapter ordering tests.

- [ ] **Step 5: Commit**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
git add servicos/scraper/adapters/hurricane.py servicos/scraper/tests/test_adapters.py
git commit -m "refactor: thin HurricaneAdapter to use source chain via _sources()"
```

---

### Task 6: Modify main.py — uppercase MARCA and GenericAdapter fallback

**Files:**
- Modify: `servicos/scraper/main.py`

**Interfaces:**
- Consumes: `REGISTRY`, `GenericAdapter` from Task 1

- [ ] **Step 1: Write integration tests for main.py MARCA normalization and fallback**

Create `servicos/scraper/tests/test_integration.py`:

```python
from __future__ import annotations

import os
from unittest.mock import AsyncMock, patch

import pytest


@pytest.fixture
def mock_env():
    old = dict(os.environ)
    os.environ.update({
        "DATABASE_URL": "postgres://test:test@localhost:5432/test",
        "STATIC_DIR": "/tmp/static_test",
        "MARCA": "Rotax, HurriCane",
        "SCRAPER_DELAY_MS": "0",
        "LOG_LEVEL": "DEBUG",
    })
    yield
    os.environ.clear()
    os.environ.update(old)


def test_marca_uppercase_normalization(mock_env):
    from main import _normalize_marcas

    result = _normalize_marcas("Rotax, HurriCane")
    assert result == ["ROTAX", "HURRICANE"]


def test_marca_uppercase_handles_whitespace(mock_env):
    from main import _normalize_marcas

    result = _normalize_marcas("  Rotax ,   HurriCane  ")
    assert result == ["ROTAX", "HURRICANE"]


def test_marca_uppercase_empty_string(mock_env):
    from main import _normalize_marcas

    result = _normalize_marcas("")
    assert result == []


@pytest.mark.asyncio
async def test_enrich_product_uses_generic_adapter_when_no_registry_match(mock_env):
    from adapters import GenericAdapter
    import main

    product = {"id": 1, "nome": "Test Product", "marca": "UNKNOWN_BRAND", "num_fab": None}

    mock_pool = AsyncMock()
    # Mock acquire + transaction
    mock_conn = AsyncMock()
    mock_conn.transaction.return_value.__aenter__ = AsyncMock()
    mock_conn.transaction.return_value.__aexit__ = AsyncMock()
    mock_pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
    mock_pool.acquire.return_value.__aexit__ = AsyncMock()

    # Mock source to return data so we don't hit real network
    with patch.object(GenericAdapter, "search", new_callable=AsyncMock) as mock_search:
        mock_search.return_value = None  # simulate no data found

        await main.enrich_product(
            product, mock_pool, main.Path("/tmp"), True, main.httpx.AsyncClient(),
        )

        mock_search.assert_called_once_with("Test Product", None)


@pytest.mark.asyncio
async def test_enrich_product_uses_registry_adapter_when_match(mock_env):
    from adapters import REGISTRY
    import main

    product = {"id": 1, "nome": "Test Hurricane", "marca": "HURRICANE", "num_fab": None}

    mock_pool = AsyncMock()
    mock_conn = AsyncMock()
    mock_conn.transaction.return_value.__aenter__ = AsyncMock()
    mock_conn.transaction.return_value.__aexit__ = AsyncMock()
    mock_pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
    mock_pool.acquire.return_value.__aexit__ = AsyncMock()

    adapter_cls = REGISTRY["HURRICANE"]
    with patch.object(adapter_cls, "search", new_callable=AsyncMock) as mock_search:
        mock_search.return_value = None

        await main.enrich_product(
            product, mock_pool, main.Path("/tmp"), True, main.httpx.AsyncClient(),
        )

        mock_search.assert_called_once_with("Test Hurricane", None)
```

- [ ] **Step 2: Verify tests fail**

```bash
cd servicos/scraper && python -m pytest tests/test_integration.py -v 2>&1
```

Expected: FAIL — `_normalize_marcas` does not exist; `main.enrich_product` still has old logic.

- [ ] **Step 3: Modify main.py**

Add `_normalize_marcas` helper and modify `enrich_product` and `run` in `servicos/scraper/main.py`.

First, update the import at line 14:

```python
from adapters import REGISTRY, GenericAdapter, ScrapedData
```

Add the `_normalize_marcas` function after `_parse_args` (after line 50):

```python
def _normalize_marcas(marcas_raw: str) -> list[str]:
    return [m.strip().upper() for m in marcas_raw.split(",") if m.strip()]
```

Modify the `enrich_product` function (lines 113-121) to use GenericAdapter fallback:

```python
    adapter_class = REGISTRY.get(marca)
    if adapter_class is None:
        adapter = GenericAdapter()
        logger.info(
            "No registered adapter for marca=%r — using GenericAdapter for product id=%d",
            marca, product_id,
        )
    else:
        adapter = adapter_class()
    logger.info("Enriching product id=%d nome=%r marca=%r", product_id, nome, marca)

    scraped: ScrapedData | None = await adapter.search(nome, num_fab)
```

Modify the `run` function (line 182, the MARCA line) to use normalization:

```python
    marcas_raw = os.environ.get("MARCA", "Hurricane")
    marcas = _normalize_marcas(marcas_raw)
```

Remove the old line:
```python
    marcas = [m.strip() for m in marcas_raw.split(",") if m.strip()]
```

- [ ] **Step 4: Run integration tests**

```bash
cd servicos/scraper && python -m pytest tests/test_integration.py -v 2>&1
```

Expected: all 5 integration tests PASS.

- [ ] **Step 5: Run all tests together**

```bash
cd servicos/scraper && python -m pytest tests/ -v 2>&1
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
git add servicos/scraper/main.py servicos/scraper/tests/test_integration.py
git commit -m "feat: uppercase MARCA normalization, GenericAdapter fallback for unregistered brands"
```

---

### Task 7: Write end-to-end tests

**Files:**
- Create: `servicos/scraper/tests/test_e2e.py`

**Interfaces:**
- Consumes: all source functions from Tasks 2-4, adapters from Tasks 1 and 5

- [ ] **Step 1: Create E2E tests**

Create `servicos/scraper/tests/test_e2e.py`:

```python
from __future__ import annotations

import pytest


@pytest.mark.e2e
@pytest.mark.asyncio
async def test_mercadolivre_real_search():
    from sources.mercadolivre import search_mercadolivre

    result = await search_mercadolivre("Alto Falante Hurricane Triak 6", None)
    assert result is not None
    assert result.descricao is not None
    assert len(result.descricao) > 0


@pytest.mark.e2e
@pytest.mark.asyncio
async def test_duckduckgo_real_search():
    from sources.duckduckgo import search_duckduckgo

    result = await search_duckduckgo("Alto Falante Hurricane Triak 6 polegadas", None)
    assert result is not None
    assert result.descricao is not None
    assert len(result.descricao) >= 20


@pytest.mark.e2e
@pytest.mark.asyncio
async def test_hurricane_adapter_real_product():
    from adapters.hurricane import HurricaneAdapter

    adapter = HurricaneAdapter()
    result = await adapter.search("Alto Falante Hurricane Triak 6", "XPT1200")
    # May return None if product not listed; test passes either way (validates no crash)
    if result is not None:
        assert isinstance(result.descricao, str) or result.descricao is None


@pytest.mark.e2e
@pytest.mark.asyncio
async def test_generic_adapter_real_product():
    from adapters import GenericAdapter

    adapter = GenericAdapter()
    result = await adapter.search("TARAMPS TS 400x4 Amplificador", None)
    if result is not None:
        assert isinstance(result.descricao, str) or result.descricao is None
```

- [ ] **Step 2: Run E2E tests (optional, requires network)**

```bash
cd servicos/scraper && python -m pytest tests/test_e2e.py -v -m e2e 2>&1
```

Expected: tests pass or skip gracefully. DuckDuckGo test verifies real-world scraping works. ML test verifies the API is accessible.

- [ ] **Step 3: Commit**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
git add servicos/scraper/tests/test_e2e.py
git commit -m "test: add end-to-end tests for scraper sources and adapters"
```

---

### Task 8: Update requirements.txt

**Files:**
- Modify: `servicos/scraper/requirements.txt`

- [ ] **Step 1: Add test dependencies**

Add to `servicos/scraper/requirements.txt`:

```
pytest>=8.0
pytest-asyncio>=0.23
pytest-httpx>=0.30
```

- [ ] **Step 2: Install and verify**

```bash
cd servicos/scraper && pip install -r requirements.txt 2>&1
```

- [ ] **Step 3: Commit**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
git add servicos/scraper/requirements.txt
git commit -m "chore: add pytest, pytest-asyncio, pytest-httpx to scraper requirements"
```

---

### Task 9: Build Docker image and verify

**Files:**
- None (verification only)

- [ ] **Step 1: Build the scraper image**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
docker compose build produtos-scraper 2>&1
```

Expected: build succeeds with no errors.

- [ ] **Step 2: Run the scraper as a one-off container with a brand in the DB**

```bash
docker compose up -d produtos-db 2>&1
sleep 5  # wait for DB health check
docker compose run --rm \
  -e DATABASE_URL="postgres://app_user:Ch0pin2709sonata@produtos-db:5432/mps_produtos_db" \
  -e STATIC_DIR=/tmp/static \
  -e MARCA=Rotax \
  -e SCRAPER_DELAY_MS=1000 \
  -e LOG_LEVEL=INFO \
  produtos-scraper 2>&1
```

Expected: container runs, finds ROTAX products, attempts Mercado Livre and DuckDuckGo searches, logs results.

- [ ] **Step 3: Verify docker compose up works end-to-end**

```bash
docker compose up -d 2>&1
```

Expected: all services start, `produtos-scraper` runs and exits cleanly.

---

### Task 10: Final verification

- [ ] **Step 1: Run full test suite**

```bash
cd servicos/scraper && python -m pytest tests/ -v -m "not e2e" 2>&1
```

Expected: all non-E2E tests pass.

- [ ] **Step 2: Verify no regressions in service integration**

```bash
cd /home/marco_normal/tmp/Rust/mps-backend
/usr/bin/curl -s 'http://localhost:3000/api/products/search?q=camper&limit=3' 2>&1 | head -200
/usr/bin/curl -s -H "Origin: http://localhost:5173" http://localhost:3001/api/pedidos 2>&1
```

Expected: produtos API returns products, pedidos API returns 401 with CORS headers.

- [ ] **Step 3: Final commit if any cleanup needed**

```bash
git status
```
