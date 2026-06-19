"""Tests for §1.2.12 (GuardCondition), §1.2.13 (WaitSet), §1.3.1
(re-export in the outer `zerodds` namespace) and §2.6 (WaitSet API).

Cover the spec-coverage items from
`docs/spec-coverage/zerodds-py-1.0.md`.
"""

from __future__ import annotations

import threading
import time

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core not compiled — maturin develop needed",
)


def test_guard_condition_default_trigger_is_false():
    """§1.2.12 — the constructor returns untriggered."""
    gc = zerodds.GuardCondition()
    assert gc.get_trigger_value() is False


def test_guard_condition_set_trigger_roundtrip():
    """§1.2.12 — set_trigger_value(True/False) is reflected in get_trigger_value()."""
    gc = zerodds.GuardCondition()
    gc.set_trigger_value(True)
    assert gc.get_trigger_value() is True
    gc.set_trigger_value(False)
    assert gc.get_trigger_value() is False


def test_guard_condition_via_outer_namespace():
    """§1.3.1 — `zerodds.GuardCondition` is re-exported (not just `_core.GuardCondition`)."""
    assert hasattr(zerodds, "GuardCondition")
    assert zerodds.GuardCondition is not None
    gc = zerodds.GuardCondition()
    assert gc.get_trigger_value() is False


def test_waitset_attach_and_wait_raises_timeout():
    """§1.2.13 + §2.6 — WaitSet.wait raises `TimeoutError` if no trigger
    flips within the timeout (Rust `DdsError::Timeout` → Python
    `TimeoutError`, see `crates/py/src/ffi.rs::dds_err_to_py`)."""
    gc = zerodds.GuardCondition()
    ws = zerodds.WaitSet()
    ws.attach_guard_condition(gc)
    with pytest.raises(TimeoutError):
        ws.wait(0.05)


def test_waitset_wakes_when_guard_condition_fires():
    """§1.2.13 + §2.6 — WaitSet.wait returns as soon as the attached
    GuardCondition is triggered. The trigger flips asynchronously from a
    separate thread; WaitSet.wait should return >= 1."""
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
    """§1.3.1 — `zerodds.WaitSet` is re-exported."""
    assert hasattr(zerodds, "WaitSet")
    assert zerodds.WaitSet is not None
    ws = zerodds.WaitSet()
    # WaitSet without an attached condition → wait runs straight into the timeout.
    with pytest.raises((TimeoutError, RuntimeError)):
        ws.wait(0.01)
