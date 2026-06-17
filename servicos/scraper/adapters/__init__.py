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
