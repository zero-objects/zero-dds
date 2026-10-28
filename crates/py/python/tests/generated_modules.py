"""Loads the committed `zerodds-idlc --lang python` modules of `_generated/`."""

from __future__ import annotations

import importlib.util
import pathlib
import sys

import pytest

GEN_DIR = pathlib.Path(__file__).parent / "_generated"


def load_generated(name: str):
    path = GEN_DIR / f"{name}.py"
    if not path.exists():
        pytest.fail(f"{path} is missing — the generated modules are committed")
    spec = importlib.util.spec_from_file_location(f"_gen_{name}", path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    # Register before exec: the regenerated modules use
    # `from __future__ import annotations` (deferred/stringified
    # annotations), and dataclasses.dataclass on 3.10+ resolves those
    # strings via `sys.modules[cls.__module__]` — a module never entered
    # into sys.modules has no entry to resolve against and every
    # `@dataclass` in it fails at class-body-execution time.
    sys.modules[spec.name] = mod
    spec.loader.exec_module(mod)
    return mod
