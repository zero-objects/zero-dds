# zerodds-py-1.0 — offene Punkte

Aggregat aus `zerodds-py-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## Nicht-Ziel — RTPS direkt in reinem Python

**Status:** `open` — der `ctypes`-Pfad braucht `libzerodds`; `runtime/python/zerodds_wire` kodiert CDR, sendet aber kein RTPS.

## §3.1 Mapping

**Status:** `partial` — die Elementzahl eines festen Arrays prüft der Encoder nicht: zu viele Elemente werden gekürzt, zu wenige werfen `IndexError` (`test_fixed_array_count_is_refused_on_encode`, `xfail(strict=True)`).

## §5.3 Cross-Vendor Python ↔ andere Sprachen

**Status:** `partial` — die Byte-Identität Python ↔ Rust ist im Baum gemessen. Für C++, Java und C# gibt es keine Messung im Repo: die Interop-Matrix, die diese Spalten trägt, liegt auf dem Bench-Host und ist nicht Teil dieses Baums. Rest als §5.3b geführt.

## §5.3b Byte-Identität Python ↔ C++/Java/C# im Baum

**Status:** `open` — es gibt keinen Testfall im Repo, der die vom Python-Codec geschriebenen ShapeType-Oktetten gegen die der C++-, Java- oder C#-Bindung hält. `shape_reference_octets` liefert diese Referenz bisher nur für Rust. Plan: einen Golden-Vektor je Bindung wie in `endpoints/python/test_byte_identity.py` (dort für `SensorReading`) auch für `ShapeType` erzeugen und die vier Codecs gegen denselben Vektor halten.

## §6.4 ROS-2-pytest-Integration

**Status:** `partial` — Nicht in CI gelaufen: `test_rclpy_init_succeeds_with_zerodds_rmw`, `test_rclpy_publish_subscribe_string_roundtrip`, `test_rclpy_service_call_roundtrip`, `test_rclpy_topic_graph_introspection`, `test_rclpy_node_names_graph`, `test_rclpy_endpoint_info_by_topic`. Die rclpy-Tests unter `crates/py/python/tests/ros2/` sind im Job `integration-python` deklariert übersprungen. ROS 2 ist nicht im CI-Image (OPEN-ITEMS 42; ROS 2 kommt mit RC9).

## §6.4 ROS-2-pytest-Integration — Lauf in CI

**Status:** `open` — Nicht in CI gelaufen: `test_rclpy_init_succeeds_with_zerodds_rmw`, `test_rclpy_publish_subscribe_string_roundtrip`, `test_rclpy_service_call_roundtrip`, `test_rclpy_topic_graph_introspection`, `test_rclpy_node_names_graph`, `test_rclpy_endpoint_info_by_topic`. Die rclpy-Tests unter `crates/py/python/tests/ros2/` sind im Job `integration-python` deklariert übersprungen. ROS 2 ist nicht im CI-Image (OPEN-ITEMS 42; ROS 2 kommt mit RC9).

## §8 Lizenz

**Status:** `open` — `crates/py/pyproject.toml` deklariert `license = { text = "Apache-2.0 OR MIT" }`, der Cargo-Workspace `Apache-2.0`; die Spec nennt Apache-2.0. Kein Test prüft die Angabe.
