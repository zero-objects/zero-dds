"""ROS-2 pytest fixture setup (§6.4 vendor spec).

Determines whether a runnable ROS-2 setup is present (rclpy +
rmw_cyclonedds_cpp). If not, all tests in this directory are
skipped. That way the test tree can live in the monorepo without
the normal `pytest crates/py/python/tests/` run failing.
"""

from __future__ import annotations

import os
import shutil

import pytest


def _ros2_available() -> bool:
    if "ROS_DISTRO" not in os.environ:
        return False
    if shutil.which("ros2") is None:
        return False
    try:
        import rclpy  # noqa: F401
    except ImportError:
        return False
    return True


def _zerodds_rmw_available() -> bool:
    """Check whether `rmw_zerodds_cpp` is registered as the RMW implementation."""
    rmw = os.environ.get("RMW_IMPLEMENTATION", "")
    return rmw == "rmw_zerodds_cpp"


def pytest_collection_modifyitems(config: object, items: list) -> None:
    """Mark only the tests in THIS subdirectory (`ros2/`) as
    skipped — not the entire test tree. `pytest_collection_modifyitems`
    is otherwise project-level and would hit all tests."""
    here = os.path.dirname(__file__)
    skip_no_ros = pytest.mark.skip(reason="ROS-2 not installed (ROS_DISTRO + rclpy needed)")
    skip_no_rmw = pytest.mark.skip(
        reason="RMW_IMPLEMENTATION != rmw_zerodds_cpp — `export RMW_IMPLEMENTATION=rmw_zerodds_cpp`",
    )
    ros2_items = [it for it in items if str(it.fspath).startswith(here)]
    if not _ros2_available():
        for item in ros2_items:
            item.add_marker(skip_no_ros)
        return
    if not _zerodds_rmw_available():
        for item in ros2_items:
            item.add_marker(skip_no_rmw)
