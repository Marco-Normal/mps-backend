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
