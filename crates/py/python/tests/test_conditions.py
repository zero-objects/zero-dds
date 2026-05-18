"""Tests fuer §1.2.12 (GuardCondition), §1.2.13 (WaitSet), §1.3.1
(Re-Export im aeusseren `zerodds`-Namespace) und §2.6 (WaitSet-API).

Decken die Spec-Coverage-Items aus
`docs/spec-coverage/zerodds-py-1.0.md` ab.
"""

from __future__ import annotations

import threading
import time

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core nicht kompiliert — maturin develop noetig",
)


def test_guard_condition_default_trigger_is_false():
    """§1.2.12 — Konstruktor liefert ungetriggert."""
    gc = zerodds.GuardCondition()
    assert gc.get_trigger_value() is False


def test_guard_condition_set_trigger_roundtrip():
    """§1.2.12 — set_trigger_value(True/False) reflektiert in get_trigger_value()."""
    gc = zerodds.GuardCondition()
    gc.set_trigger_value(True)
    assert gc.get_trigger_value() is True
    gc.set_trigger_value(False)
    assert gc.get_trigger_value() is False


def test_guard_condition_via_outer_namespace():
    """§1.3.1 — `zerodds.GuardCondition` ist re-exportiert (nicht nur `_core.GuardCondition`)."""
    assert hasattr(zerodds, "GuardCondition")
    assert zerodds.GuardCondition is not None
    gc = zerodds.GuardCondition()
    assert gc.get_trigger_value() is False


def test_waitset_attach_and_wait_raises_timeout():
    """§1.2.13 + §2.6 — WaitSet.wait wirft `TimeoutError`, wenn kein Trigger
    innerhalb des Timeouts flippt (Rust `DdsError::Timeout` → Python
    `TimeoutError`, siehe `crates/py/src/ffi.rs::dds_err_to_py`)."""
    gc = zerodds.GuardCondition()
    ws = zerodds.WaitSet()
    ws.attach_guard_condition(gc)
    with pytest.raises(TimeoutError):
        ws.wait(0.05)


def test_waitset_wakes_when_guard_condition_fires():
    """§1.2.13 + §2.6 — WaitSet.wait kehrt zurueck, sobald die attached
    GuardCondition getriggert wird. Trigger flippt asynchron aus einem
    separaten Thread; WaitSet.wait sollte >= 1 zurueckgeben."""
    gc = zerodds.GuardCondition()
    ws = zerodds.WaitSet()
    ws.attach_guard_condition(gc)

    def fire_after_delay() -> None:
        time.sleep(0.05)
        gc.set_trigger_value(True)

    threading.Thread(target=fire_after_delay, daemon=True).start()
    triggered = ws.wait(2.0)
    assert triggered >= 1
    assert gc.get_trigger_value() is True


def test_waitset_via_outer_namespace():
    """§1.3.1 — `zerodds.WaitSet` ist re-exportiert."""
    assert hasattr(zerodds, "WaitSet")
    assert zerodds.WaitSet is not None
    ws = zerodds.WaitSet()
    # WaitSet ohne attached Condition → wait laeuft sofort ins Timeout.
    with pytest.raises((TimeoutError, RuntimeError)):
        ws.wait(0.01)
