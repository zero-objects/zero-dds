"""Sphinx configuration for the ZeroDDS Python binding.

Build::

    pip install sphinx sphinx-rtd-theme
    cd crates/py/docs
    sphinx-build -b html . _build/html
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

# Source directory — so that autodoc finds the pure Python modules,
# without maturin having to have built the Rust core (CI/RTD-friendly).
DOCS_DIR = Path(__file__).parent.resolve()
sys.path.insert(0, str(DOCS_DIR.parent / "python"))

project = "ZeroDDS"
author = "ZeroDDS Authors"
copyright = "2026, ZeroDDS Authors"  # noqa: A001 — sphinx-convention
release = "0.0.0"

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
    "sphinx.ext.viewcode",
    "sphinx.ext.intersphinx",
]

autodoc_default_options = {
    "members": True,
    "undoc-members": False,
    "show-inheritance": True,
}

intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
}

# The Rust extension module `zerodds._core` may be unavailable at
# doc-build time (CI has no maturin-develop). We mock it,
# so that autodoc does not abort on the first import.
autodoc_mock_imports = ["zerodds._core"]

# The "mocked object detected" warnings are a consequence of the mock above
# (autodoc finds the symbol names but cannot load a real object
# behind them). They are harmless — we suppress them so that
# `-W` (warnings-as-errors) in the CI build does not fail.
suppress_warnings = ["autodoc.mocked_object"]

html_theme = os.environ.get("ZERODDS_SPHINX_THEME", "alabaster")
html_static_path: list[str] = []

templates_path = ["_templates"]
exclude_patterns: list[str] = ["_build"]
