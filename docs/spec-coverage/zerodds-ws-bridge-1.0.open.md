# zerodds-ws-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-ws-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §5.2 Type-Discovery Catalog-Frame + /schema/<slug>

**Status:** `partial` — der Catalog-Frame ist geprüft; der IDL-Endpoint wird als §5.2a `open` geführt.

## §5.2a `/schema/<slug>`-IDL-Endpoint

**Status:** `open` — der Teil von §5.2 jenseits des Catalog-Frames. Im Baum gibt es keinen `/schema`-Pfad.

## §8.1 Strukturiertes JSON-Logging

**Status:** `open` — `crates/websocket-bridge/src/daemon/cli.rs::tests` prüft die Flag-Zerlegung, aber kein Test prüft die JSON-Log-Form oder die Wirkung von `--log-level`.

## §8.3 OTLP-Spans

**Status:** `partial` — die Endpoint-Auflösung ist geprüft, der Span-Emit nicht.

## §8.3a Frame-Roundtrip-Span

**Status:** `open` — der Emit-Teil von §8.3.
