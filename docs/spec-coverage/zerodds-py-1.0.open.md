# `zerodds-py` v1.0 — Open Items

Aggregat aus `zerodds-py-1.0.md` Hauptfile. Vor jedem Audit-Lauf
gelöscht und neu generiert (keine Drift).

**Stand:** 41 done / 1 partial / 0 open / 5 n/a (informative) / 0 n/a (rejected).

## §6.4 ROS-2-pytest-Integration

**Status:** `partial` — `crates/py/python/tests/ros2/{conftest.py,
test_rmw_zerodds_interop.py}` ist als Skeleton geschrieben; skippt
ohne `ROS_DISTRO + rclpy + RMW_IMPLEMENTATION=rmw_zerodds_shim`.
Lokal (macOS) und auf dem Bench-Host `llvm@llvm` (Debian 12) ist
weder ROS-2 installiert noch das `rmw_zerodds_shim` als
RMW-Implementation registriert; die zwei Tests skippen daher in
allen aktuellen Audit-Laeufen.

Plan-Hinweis: Eigener CI-Job auf ROS-2-Humble/Iron-Image, gebauter
`rmw_zerodds_shim` (siehe `crates/rmw-zerodds-shim/`) als RMW;
dann laufen die zwei Tests (`test_rclpy_init_succeeds_with_zerodds_rmw`,
`test_rclpy_publish_subscribe_string_roundtrip`). Dieses Setup
gehoert in den Workstream des `rmw-zerodds-shim`-Owners und nicht
in das `zerodds-py`-Audit selbst.
