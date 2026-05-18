"""ROS-2-pytest-Fixture-Setup (§6.4 Vendor-Spec).

Stellt fest, ob ein lauffaehiges ROS-2-Setup vorhanden ist (rclpy +
rmw_cyclonedds_cpp). Falls nicht, werden alle Tests in diesem
Verzeichnis geskipped. Damit kann der Test-Tree im monorepo liegen,
ohne dass die normale `pytest crates/py/python/tests/`-Run scheitert.
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
    """Pruefe, ob `rmw_zerodds_shim` als RMW-Implementation registriert ist."""
    rmw = os.environ.get("RMW_IMPLEMENTATION", "")
    return rmw == "rmw_zerodds_shim"


def pytest_collection_modifyitems(config: object, items: list) -> None:
    """Markiere nur die Tests in DIESEM Unterverzeichnis (`ros2/`) als
    skip — nicht den gesamten Test-Tree. `pytest_collection_modifyitems`
    ist ansonsten project-level und wuerde alle Tests treffen."""
    here = os.path.dirname(__file__)
    skip_no_ros = pytest.mark.skip(reason="ROS-2 nicht installiert (ROS_DISTRO + rclpy noetig)")
    skip_no_rmw = pytest.mark.skip(
        reason="RMW_IMPLEMENTATION != rmw_zerodds_shim — `export RMW_IMPLEMENTATION=rmw_zerodds_shim`",
    )
    ros2_items = [it for it in items if str(it.fspath).startswith(here)]
    if not _ros2_available():
        for item in ros2_items:
            item.add_marker(skip_no_ros)
        return
    if not _zerodds_rmw_available():
        for item in ros2_items:
            item.add_marker(skip_no_rmw)
