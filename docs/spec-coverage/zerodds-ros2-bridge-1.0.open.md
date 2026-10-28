# zerodds-ros2-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-ros2-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8.2 Prometheus-Metrics via separater Exporter

**Status:** `open` — `zerodds-ros2-shim metrics` druckt den Namens-Katalog, aber kein Test in `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` ruft ihn auf, und kein Zähler wird im rmw-Pfad hochgezählt.

## §8.3 OTLP rmw_publish/rmw_take Spans

**Status:** `open` — weder `crates/rmw-zerodds-shim/` noch `crates/ros2-rmw/` nennt `OTEL_EXPORTER_OTLP_ENDPOINT` oder emittiert einen Span.

## §11 librmw_zerodds_cpp.so + Diagnose-Binary

**Status:** `partial` — ohne Beschreibung im Hauptfile.
