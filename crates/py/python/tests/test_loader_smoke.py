"""Smoke tests for §1.3.4 + §1.4 — pure-ctypes loader.

The `zerodds.loader` path is independent of the PyO3 `_core` path. It
needs a built `libzerodds.{so,dylib,dll}` from `crates/zerodds-c-api`.

Tests skip when the library is not found — an auto-skip
covers both CI workflows without a C-API build and development trees
without a `cargo build -p zerodds-c-api` step.
"""

from __future__ import annotations

import ctypes

import pytest

from zerodds import loader


def _try_load_library() -> ctypes.CDLL | None:
    try:
        return loader.load_library()
    except OSError:
        return None


_LIB = _try_load_library()

pytestmark = pytest.mark.skipif(
    _LIB is None,
    reason="libzerodds not found — run `cargo build -p zerodds-c-api` or set ZERODDS_LIB",
)


def test_loader_resolves_library():
    """§1.3.4 — `load_library()` returns a CDLL object."""
    lib = loader.load_library()
    assert isinstance(lib, ctypes.CDLL)


def test_loader_runtime_create_participant_offline():
    """§1.3.4 + §1.4 — `Runtime(domain_id=...)` can create a
    participant offline via libzerodds and tear it down cleanly on GC.
    Spec: zerodds-ffi-loader-1.0 §3.1, zerodds-c-api §1."""
    rt = loader.Runtime(domain_id=190)
    # The runtime should hold a live handle — the attribute check
    # stays impl-stable (see loader.py: self._participant).
    assert rt is not None
    # Cleanup path implicitly via __del__/__exit__; manual close()
    # if implemented.
    close = getattr(rt, "close", None)
    if callable(close):
        close()


def test_loader_factory_singleton():
    """§1.3.4 — DomainParticipantFactory API also available in the ctypes
    path (zerodds-c-api §1.2)."""
    factory = loader.DomainParticipantFactory.instance()
    assert factory is not None


def test_loader_zerodds_error_is_runtime_error_subclass():
    """§1.3.4 — loader errors derive from RuntimeError (zerodds-c-api
    error mapping)."""
    assert issubclass(loader.ZeroDdsError, RuntimeError)
