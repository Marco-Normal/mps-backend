# Scraper Refactor: Generic Adapter Architecture

**Date:** 2026-06-17
**Status:** Design approved, awaiting implementation plan

## Problem Summary

1. **Case sensitivity**: `SCRAPER_MARCA` in `.env` (mixed case) doesn't match database values (uppercase) or `REGISTRY` keys (uppercase), so the DB query finds zero products.
2. **No generic fallback**: Only one brand ("HURRICANE") has an adapter in `REGISTRY`. All other brands (ENFORTH, PERMAK, FIAMON, GRID, etc.) are silently skipped with "No adapter registered for marca=..."
3. **No web search fallback**: When manufacturer-site scraping and Mercado Livre both fail, there is no broader fallback like DuckDuckGo or Google.

---

## Architecture

Each adapter is no longer a monolith. Search sources become standalone, pluggable async functions composed into a priority-ordered chain per adapter:

```
BrandAdapter.search(product_name, num_fab)
    │
    ├── source 1 ──▶ ScrapedData? ──yes──▶ return
    │                    │no
    ├── source 2 ──▶ ScrapedData? ──yes──▶ return
    │                    │no
    └── source N ──▶ ScrapedData? ──yes──▶ return
                         │no
                       return None
```

### Source Protocol

Each source is an async function with signature:

```python
async def search_<name>(product_name: str, num_fab: str | None) -> ScrapedData | None:
```

Sources live in `sources/` and have zero knowledge of brands or adapters. They receive a product name and optional manufacturer part number, and return scraped data or None.

### Adapter changes

`BrandAdapter` gains a concrete `search()` that iterates `self._sources()` and a new `_sources()` method that subclasses override:

```python
class BrandAdapter(ABC):
    marca: str

    async def search(self, product_name: str, num_fab: str | None) -> ScrapedData | None:
        for source in self._sources():
            try:
                result = await source(product_name, num_fab)
                if result is not None:
                    return result
            except Exception:
                logger.warning("%s: source %s failed", self.marca, source.__name__, exc_info=True)
        return None

    @abstractmethod
    def _sources(self) -> list[...]:
        """Return ordered list of search sources for this brand."""
```

### Source inventory

| Source | File | Method | Notes |
|--------|------|--------|-------|
| hurricane_site | `sources/hurricane_site.py` | Playwright → hurricanesound.com.br | Extracted from today's `hurricane.py` adapter |
| mercadolivre | `sources/mercadolivre.py` | REST API → api.mercadolibre.com | Generalized: no hardcoded brand prefix |
| duckduckgo | `sources/duckduckgo.py` | **NEW** — Playwright → html.duckduckgo.com/html/ | Lite HTML version, no JS rendering needed |

### Adapter chains

| Adapter | Sources (priority order) | REGISTRY key |
|---|---|---|
| `HurricaneAdapter` | hurricane_site → mercadolivre → duckduckgo | `"HURRICANE"` |
| `GenericAdapter` | mercadolivre → duckduckgo | (not in REGISTRY) |

`GenericAdapter` is not registered per-brand. It is instantiated directly in `main.py` when `REGISTRY.get(marca)` returns None — acting as a catch-all fallback for any unregistered brand. It sets `marca = "GENERIC"` only to satisfy the `BrandAdapter` ABC contract.

---

## Sources Detail

### `sources/mercadolivre.py` (modified)

**Changes from current:**
- Function signature becomes `search_mercadolivre(product_name: str, num_fab: str | None) → ScrapedData | None`
- Creates its own `httpx.AsyncClient` internally (no caller-supplied client)
- Query string is `num_fab or product_name` (no hardcoded brand prefix)
- Returns `ScrapedData` directly instead of `tuple[str | None, list[str]]`

### `sources/duckduckgo.py` (new)

Scrapes `https://html.duckduckgo.com/html/` via Playwright — the lite HTML version that does not require JavaScript. A regular HTTP GET is sufficient; Playwright is used for consistency, anti-bot header injection, and future JS-heavy search engines.

- **Query**: `f"{brand} {num_fab or product_name}"` — e.g. `"Hurricane XPT1200"`
- **Description extraction**: First 2-3 result snippets joined with newlines. Minimum 20 chars total.
- **Images**: None from DuckDuckGo Lite directly. Secondary calls are out of scope for this spec.
- **Failure cases**: HTTP errors, CAPTCHA/block page, no snippets ≥20 chars → return None.
- **Rate limiting**: Respect `SCRAPER_DELAY_MS` via `asyncio.sleep` after each search.

### `sources/hurricane_site.py` (extracted)

Same Playwright logic currently in `hurricane.py` lines 48-98. Zero behavior change — pure extraction.

---

## `main.py` Changes

1. **MARCA normalization**: Before use, uppercase all values from `SCRAPER_MARCA`:
   ```python
   marcas = [m.strip().upper() for m in marcas_raw.split(",") if m.strip()]
   ```

2. **Generic fallback in enrich loop**: When `REGISTRY.get(marca)` returns None:
   ```python
   if adapter_class is None:
       adapter = GenericAdapter()
       logger.info("No registered adapter for marca=%r — using GenericAdapter", marca)
   else:
       adapter = adapter_class()
   ```
   No more silent skipping of unregistered brands.

3. **No other changes** — the enrichment loop, image download, DB writes, and CLI remain unchanged.

---

## Error Handling

Every source is non-fatal. Failures are logged as warnings; the chain continues to the next source.

| Source | Failures handled |
|--------|-----------------|
| `mercadolivre` | HTTP errors, JSON decode errors, empty results |
| `duckduckgo` | HTTP errors, Playwright launch failure, CAPTCHA/block, no useful snippets |
| `hurricane_site` | Playwright errors, site unreachable, changed DOM selectors |

If all sources return None for a product, it is logged at INFO level and skipped — no retry, no error.

---

## Testing Strategy

Tests use `pytest` with `pytest-asyncio`. Markers: `@pytest.mark.e2e` for real-network tests.

### Unit tests (fast, no network)

| Test | What it verifies |
|------|-----------------|
| `test_registry_keys_uppercase` | Every key in `REGISTRY` is ASCII uppercase |
| `test_all_adapters_have_marca` | Every adapter class defines `marca` as a str |
| `test_sources_protocol` | Each source function accepts `(str, str \| None)` signature |
| `test_scraped_data_caps_images_at_3` | `ScrapedData` dataclass truncates `image_urls` to 3 |
| `test_brandadapter_sources_ordering` | `HurricaneAdapter._sources()` returns hurricane_site, mercadolivre, duckduckgo in that order |
| `test_genericadapter_sources_ordering` | `GenericAdapter._sources()` returns mercadolivre, duckduckgo in that order |
| `test_genericadapter_instantiates_when_registry_miss` | `REGISTRY.get("UNKNOWN_BRAND")` returns None → `GenericAdapter()` is instantiated without error |

### Integration tests (mock HTTP)

Use `pytest-httpx` or `responses` to mock HTTP calls. Also mock `async_playwright` for DuckDuckGo tests.

| Test | What it verifies |
|------|-----------------|
| `test_mercadolivre_parses_valid_response` | Mock ML API with a real-shaped response → `ScrapedData` with description and image URLs |
| `test_mercadolivre_handles_empty_results` | Mock ML API with `{"results": []}` → returns `None` |
| `test_mercadolivre_handles_http_error` | Mock ML API returning 500 → returns `None` |
| `test_duckduckgo_parses_snippets` | Mock DDG HTML with result snippets → description built from snippets |
| `test_duckduckgo_handles_empty_page` | Mock DDG HTML with no results → returns `None` |
| `test_duckduckgo_handles_http_error` | Mock DDG HTTP 403 → returns `None` |
| `test_fallback_chain_source1_succeeds` | Mock source 1 returns data, source 2 never called → source 1 result used |
| `test_fallback_chain_source1_fails_source2_succeeds` | Mock source 1 returns None, source 2 returns data → source 2 result used |
| `test_fallback_chain_all_sources_fail` | All sources return None → `search()` returns None |
| `test_genericadapter_sources_order_via_method` | `GenericAdapter()._sources()` returns [mercadolivre, duckduckgo] |

### End-to-end tests (real network, `@pytest.mark.e2e`)

| Test | What it verifies |
|------|-----------------|
| `test_mercadolivre_real_search` | Search for a known product on ML, verify non-empty result |
| `test_duckduckgo_real_search` | Search for a product on DuckDuckGo, verify snippets returned |
| `test_hurricane_adapter_real_product` | Search an actual Hurricane product, verify description or images |
| `test_generic_adapter_real_product` | Search a known product (e.g. "TARAMPS TS 400x4"), verify at least one source returns data |

---

## Files Changed

| File | Action | Summary |
|------|--------|---------|
| `servicos/scraper/sources/duckduckgo.py` | **NEW** | DuckDuckGo Lite search source |
| `servicos/scraper/sources/hurricane_site.py` | **NEW** | Extracted from `adapters/hurricane.py` |
| `servicos/scraper/sources/mercadolivre.py` | **MODIFY** | Generalized signature, returns `ScrapedData` |
| `servicos/scraper/adapters/__init__.py` | **MODIFY** | Add `_sources()` to `BrandAdapter`, add `GenericAdapter` class |
| `servicos/scraper/adapters/hurricane.py` | **MODIFY** | Thin adapter: only overrides `_sources()` |
| `servicos/scraper/main.py` | **MODIFY** | Uppercase MARCA, fallback to `GenericAdapter` when `REGISTRY` miss |
| `servicos/scraper/tests/` | **NEW** | `test_sources.py`, `test_adapters.py`, `test_integration.py`, `test_e2e.py` |

## Files NOT Changed

| File | Reason |
|------|--------|
| `ScrapedData` dataclass | Unchanged — same fields, same `image_urls[:3]` cap |
| `REGISTRY` dict | Same keys, same structure; `GenericAdapter` is used via code path, not registry key |
| `main.py` enrichment loop, image download, DB write | Unchanged |
| `Dockerfile`, `docker-compose.yml`, `.env` | Unchanged |
| `requirements.txt` | **MODIFY** | Add `pytest`, `pytest-asyncio`, `pytest-httpx` for test suite |

---

## Acceptance Criteria

1. Setting `SCRAPER_MARCA=Rotax` in `.env` and running the scraper finds ROTAX products in the DB (case-insensitive match).
2. The scraper attempts Mercado Livre and DuckDuckGo searches for brands without dedicated adapters (e.g., ROTAX, ENFORTH) instead of skipping them.
3. Hurricane products still go through the three-source chain (hurricane_site → mercadolivre → duckduckgo).
4. Any single source failure does not crash the scraper; the next source is attempted.
5. All unit and integration tests pass with `pytest -m "not e2e"`.
6. Docker image builds successfully.
7. Running `docker compose up -d` with `SCRAPER_MARCA` set to a brand present in the DB enriches at least some products.
