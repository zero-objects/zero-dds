"""Sphinx-Konfiguration fuer ZeroDDS-Python-Binding.

Build::

    pip install sphinx sphinx-rtd-theme
    cd crates/py/docs
    sphinx-build -b html . _build/html
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

# Quellverzeichnis — damit autodoc die reinen Python-Module findet,
# ohne dass maturin den Rust-Kern gebaut haben muss (CI/RTD-friendly).
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

# Das Rust-Extension-Module `zerodds._core` ist bei Doc-Build u.U.
# nicht verfuegbar. Wir mocken es, damit autodoc nicht beim ersten
# Import abbricht.
autodoc_mock_imports = ["zerodds._core"]

html_theme = os.environ.get("ZERODDS_SPHINX_THEME", "alabaster")
html_static_path: list[str] = []

templates_path = ["_templates"]
exclude_patterns: list[str] = ["_build"]
