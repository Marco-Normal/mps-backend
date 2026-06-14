from __future__ import annotations

import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)


@dataclass
class ScrapedData:
    """Holds scraped content for one product. image_urls is capped at 3."""

    descricao: str | None
    image_urls: list[str] = field(default_factory=list)

    def __post_init__(self) -> None:
        self.image_urls = self.image_urls[:3]


class BrandAdapter(ABC):
    """Base class for all brand-specific scrapers.

    Subclasses must set the class attribute `marca` to the exact DB string
    (e.g. "Hurricane") and implement `search`.
    """

    marca: str

    @abstractmethod
    async def search(
        self, product_name: str, num_fab: str | None
    ) -> ScrapedData | None:
        """Return ScrapedData on success, None if nothing was found."""
        ...


# Import concrete adapters AFTER the base class is defined to avoid circular imports.
from .hurricane import HurricaneAdapter  # noqa: E402

REGISTRY: dict[str, type[BrandAdapter]] = {
    "Hurricane": HurricaneAdapter,
    # Add new brand adapters here, e.g.:
    # "JBL": JBLAdapter,
}
