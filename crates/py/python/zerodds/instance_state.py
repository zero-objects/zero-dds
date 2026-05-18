"""InstanceStateMask-Konstanten (DDS 1.4 §2.2.2.5.1.4)."""
from __future__ import annotations

from . import _core

ALIVE: int = _core.INSTANCE_STATE_ALIVE
NOT_ALIVE_DISPOSED: int = _core.INSTANCE_STATE_NOT_ALIVE_DISPOSED
NOT_ALIVE_NO_WRITERS: int = _core.INSTANCE_STATE_NOT_ALIVE_NO_WRITERS
NOT_ALIVE: int = _core.INSTANCE_STATE_NOT_ALIVE
ANY: int = _core.INSTANCE_STATE_ANY

__all__ = ["ALIVE", "NOT_ALIVE_DISPOSED", "NOT_ALIVE_NO_WRITERS", "NOT_ALIVE", "ANY"]
