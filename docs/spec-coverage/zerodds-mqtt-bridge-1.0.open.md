# zerodds-mqtt-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-mqtt-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8.1 Strukturiertes JSON-Logging

**Status:** `open` — `crates/mqtt-bridge/src/daemon/cli.rs::tests` prüft die Flag-Zerlegung, aber kein Test prüft die JSON-Log-Form oder die Wirkung von `--log-level`.

## §8.3 OTLP-Spans

**Status:** `partial` — die Konfiguration ist geprüft, der Span-Emit nicht; der fehlende Teil wird als §8.3a `open` geführt.

## §8.3a PUBLISH-Roundtrip-Span

**Status:** `open` — der Emit-Teil von §8.3.

## §10 RTPS-Peer + Mosquitto/EMQX/HiveMQ

**Status:** `partial` — Mosquitto ist gemessen, EMQX und HiveMQ nicht; der fehlende Teil wird als §10a `open` geführt.

## §10a Broker-Matrix EMQX/HiveMQ

**Status:** `open` — der Teil von §10 und §12.3 jenseits Mosquitto. `crates/mqtt-bridge/tests/cross_vendor.rs` kennt nur Mosquitto.

## §12.3 Multi-Vendor cross_vendor.rs

**Status:** `partial` — Mosquitto und die DDS-Seite sind geprüft, die restliche Broker-Matrix nicht.
