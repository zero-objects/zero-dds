# zerodds-ros2-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-ros2-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8.2 Prometheus-Metrics via separater Exporter

**Status:** `open` — `zerodds-ros2-shim metrics` druckt den Namens-Katalog, aber kein Test in `crates/rmw-zerodds-shim/tests/shim_cli_e2e.rs` ruft ihn auf, und kein Zähler wird im rmw-Pfad hochgezählt.

## §8.3 OTLP rmw_publish/rmw_take Spans

**Status:** `open` — weder `crates/rmw-zerodds-shim/` noch `crates/ros2-rmw/` nennt `OTEL_EXPORTER_OTLP_ENDPOINT` oder emittiert einen Span.

## §10 RTPS-Peer + FastRTPS/CycloneDDS/Connext

**Status:** `partial` — Fast DDS und Cyclone DDS sind gemessen; rmw_connextdds und das Build-Ziel Iron fehlen (eigene Items).

## §10.1 Cross-Vendor mit rmw_connextdds

**Status:** `open` — nicht gemessen; die Spec selbst nennt es „not measured (not installed in the test env)“. Es fehlt ein Lauf von `run_qos_translation_test.sh` und `run_ros2_services_test.sh` gegen rmw_connextdds.

## §10.2 Build-Ziel ROS-Distro Iron

**Status:** `open` — `packaging/linux/deb/build-rmw-deb.sh` kennt `humble jazzy kilted rolling` und lehnt jeden anderen Namen ab; `ci/release-candidate.yml::ros-rmw-deb` baut dieselben vier. `packaging/docker/ros2-shim/Dockerfile` nennt die Variante `iron`, deren Paketbau das Skript ablehnt (nicht gebaut, aus dem Skript gelesen).

## §11 librmw_zerodds_cpp.so + Diagnose-Binary

**Status:** `partial` — `librmw_zerodds_cpp.so` liegt im deb-Paket `ros-<distro>-rmw-zerodds-cpp` (gebaut aus `crates/rmw-zerodds-shim/ament`), das Image installiert es aus diesem Paket; Diagnose-Binary, Konfiguration und Handbuchseiten sind ausgeliefert. Der Image-Bau ist nicht gelaufen. Es fehlen die Veröffentlichung auf dem `packages.ros.org`-Spiegel (§11.1) und Paket und Image für Iron (§10.2).

## §11.1 Veröffentlichung auf dem packages.ros.org-Spiegel

**Status:** `open` — `packaging/linux/deb/build-rmw-deb.sh` baut das Paket; kein Job und kein Skript lädt es auf `packages.ros.org` hoch.
