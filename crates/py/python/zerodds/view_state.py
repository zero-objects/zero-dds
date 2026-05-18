"""ViewStateMask-Konstanten (DDS 1.4 §2.2.2.5.1.4)."""
from __future__ import annotations

from . import _core

NEW: int = _core.VIEW_STATE_NEW
NOT_NEW: int = _core.VIEW_STATE_NOT_NEW
ANY: int = _core.VIEW_STATE_ANY

__all__ = ["NEW", "NOT_NEW", "ANY"]
