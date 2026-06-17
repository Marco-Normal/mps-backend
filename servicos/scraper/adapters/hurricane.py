from __future__ import annotations

from adapters import BrandAdapter
from sources.hurricane_site import search_hurricane_site
from sources.mercadolivre import search_mercadolivre
from sources.duckduckgo import search_duckduckgo


class HurricaneAdapter(BrandAdapter):
    marca = "HURRICANE"

    def _sources(self):
        return [search_hurricane_site, search_mercadolivre, search_duckduckgo]
