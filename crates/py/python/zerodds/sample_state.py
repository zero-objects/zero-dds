"""SampleStateMask-Konstanten (DDS 1.4 §2.2.2.5.1.4).

Verwendung mit `zerodds.ReadCondition` / `zerodds.QueryCondition`::

    rc = zerodds.ReadCondition(
        zerodds.sample_state.ANY,
        zerodds.view_state.ANY,
        zerodds.instance_state.ALIVE,
    )
"""
from __future__ import annotations

from . import _core

NOT_READ: int = _core.SAMPLE_STATE_NOT_READ
READ: int = _core.SAMPLE_STATE_READ
ANY: int = _core.SAMPLE_STATE_ANY

__all__ = ["NOT_READ", "READ", "ANY"]
