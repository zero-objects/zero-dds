# zerodds-grpc-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-grpc-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8.1 Strukturiertes JSON-Logging

**Status:** `open` — kein Test prüft die JSON-Log-Form oder den `--log-level`-Switch.

## §8.3 OTLP-Spans + W3C-Traceparent

**Status:** `partial` — die OTLP-Konfiguration ist geprüft, das W3C-Traceparent-Parsing nicht; der fehlende Teil wird als §8.3a `open` geführt.

## §8.3a W3C-Traceparent-Parsing

**Status:** `open` — der Traceparent-Teil von §8.3. Kein Test und keine Fundstelle im Baum nennt `traceparent`.

## §10 RTPS-Peer + grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic

**Status:** `partial` — grpcurl ist gemessen, die fünf weiteren Clients nicht; der fehlende Teil wird als §10a `open` geführt.

## §10a Client-Matrix grpc-go/grpc-java/ghz/BloomRPC/tonic

**Status:** `open` — der Teil von §10 und §12.3 jenseits grpcurl. `crates/grpc-bridge/tests/cross_vendor.rs` kennt nur grpcurl.

## §12.3 Multi-Vendor cross_vendor.rs

**Status:** `partial` — grpcurl und die DDS-Seite sind geprüft, ein tonic-Client nicht.
