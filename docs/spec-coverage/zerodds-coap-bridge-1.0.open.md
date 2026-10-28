# zerodds-coap-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-coap-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §4.5 Content-Format-Registry 65000/65001/65002/50/60

**Status:** `partial` — die Option selbst und der CDR2-LE-Wert 65000 sind geprüft; die vier weiteren Werte werden als §4.5a `open` geführt.

## §4.5a Content-Format-Werte 65001/65002/50/60

**Status:** `open` — der Teil von §4.5 jenseits 65000. Im Baum steht kein Registry-Typ und kein Test für die vier Werte.

## §8.1 Strukturiertes JSON-Logging

**Status:** `open` — `crates/coap-bridge/src/daemon/cli.rs::tests` prüft die Flag-Zerlegung, aber kein Test prüft die JSON-Log-Form oder die Wirkung von `--log-level`.

## §8.3 OTLP-Spans

**Status:** `open` — `crates/coap-bridge/src/daemon/runtime_common.rs` baut die `OtlpConfig`, aber kein Test prüft, dass `OTEL_EXPORTER_OTLP_ENDPOINT` gelesen wird oder dass pro CoAP-Exchange ein Span entsteht.

## §10 RTPS-Peer + libcoap/californium/aiocoap

**Status:** `partial` — libcoap ist gemessen, Californium und aiocoap nicht; der fehlende Teil wird als §10a `open` geführt.

## §10a Client-Matrix Californium/aiocoap

**Status:** `open` — der Teil von §10 und §12.3 jenseits libcoap. `crates/coap-bridge/tests/cross_vendor.rs` kennt nur libcoap.

## §12.3 Multi-Vendor cross_vendor.rs

**Status:** `partial` — libcoap und die DDS-Seite sind geprüft, die restliche Client-Matrix nicht.
