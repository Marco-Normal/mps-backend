from __future__ import annotations

import pytest


def test_hurricane_site_function_has_correct_signature():
    import inspect
    from sources.hurricane_site import search_hurricane_site

    sig = inspect.signature(search_hurricane_site)
    params = list(sig.parameters.keys())
    assert params == ["product_name", "num_fab"]
