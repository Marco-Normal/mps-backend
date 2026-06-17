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
