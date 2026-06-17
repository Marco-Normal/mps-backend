from __future__ import annotations

import pytest
from adapters import ScrapedData


def test_hurricane_site_function_has_correct_signature():
    import inspect
    from sources.hurricane_site import search_hurricane_site

    sig = inspect.signature(search_hurricane_site)
    params = list(sig.parameters.keys())
    assert params == ["product_name", "num_fab"]


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


# ── DuckDuckGo ──────────────────────────────────────────────


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
        url="https://html.duckduckgo.com/html/?q=XPTO",
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
