from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field


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
    (e.g. "Hurricane") and implement `search`. The `marca` attribute is
    enforced at class-definition time via `__init_subclass__`.
    """

    marca: str

    def __init_subclass__(cls, **kwargs: object) -> None:
        super().__init_subclass__(**kwargs)
        if not hasattr(cls, "marca") or not isinstance(cls.marca, str):
            raise TypeError(
                f"{cls.__name__} must define a 'marca' class attribute of type str"
            )

    @abstractmethod
    async def search(
        self, product_name: str, num_fab: str | None
    ) -> ScrapedData | None:
        """Return ScrapedData on success, None if nothing was found."""
        ...


# Concrete adapter imports and REGISTRY are populated in Task 5 (hurricane.py).
# Each entry maps the exact DB marca string to its adapter class.
# To add a new brand: import the adapter and add a key here.
REGISTRY: dict[str, type[BrandAdapter]] = {}
