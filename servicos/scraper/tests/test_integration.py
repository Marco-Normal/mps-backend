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
