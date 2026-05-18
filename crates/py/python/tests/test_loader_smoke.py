"""Smoke-Tests fuer §1.3.4 + §1.4 — pure-ctypes-Loader.

Der `zerodds.loader`-Pfad ist unabhaengig vom PyO3-`_core`-Pfad. Er
braucht eine gebaute `libzerodds.{so,dylib,dll}` aus `crates/zerodds-c-api`.

Tests skippen, wenn die Library nicht gefunden wird — ein Auto-Skip
deckt sowohl CI-Workflows ohne C-API-Build als auch Development-Trees
ohne `cargo build -p zerodds-c-api`-Schritt ab.
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
    reason="libzerodds nicht gefunden — `cargo build -p zerodds-c-api` oder ZERODDS_LIB setzen",
)


def test_loader_resolves_library():
    """§1.3.4 — `load_library()` liefert ein CDLL-Objekt zurueck."""
    lib = loader.load_library()
    assert isinstance(lib, ctypes.CDLL)


def test_loader_runtime_create_participant_offline():
    """§1.3.4 + §1.4 — `Runtime(domain_id=...)` kann offline einen
    Participant via libzerodds erzeugen und beim GC sauber abbauen.
    Spec: zerodds-ffi-loader-1.0 §3.1, zerodds-c-api §1."""
    rt = loader.Runtime(domain_id=190)
    # Runtime soll ein lebendiges Handle besitzen — Attribute-Check
    # bleibt impl-stabil (siehe loader.py: self._participant).
    assert rt is not None
    # Cleanup-Pfad implizit via __del__/__exit__; manueller close()
    # falls implementiert.
    close = getattr(rt, "close", None)
    if callable(close):
        close()


def test_loader_factory_singleton():
    """§1.3.4 — DomainParticipantFactory-API auch im ctypes-Pfad
    verfuegbar (zerodds-c-api §1.2)."""
    factory = loader.DomainParticipantFactory.instance()
    assert factory is not None


def test_loader_zerodds_error_is_runtime_error_subclass():
    """§1.3.4 — Loader-Errors leiten von RuntimeError ab (zerodds-c-api
    Error-Mapping)."""
    assert issubclass(loader.ZeroDdsError, RuntimeError)
