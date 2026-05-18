# OASIS MQTT v5.0 — Open + Partial Items

Aggregat aus `mqtt-5.0.md`. Nicht von Hand pflegen — vor jedem Audit-
Lauf löschen und aus dem Hauptfile neu generieren.

## Open

— keine.

## Partial

### §2.2.2 Properties — Wert-Decoding

**Status:** `partial` — Identifier-Registry + Wire-Length-Prefix done;
Identifier-spezifische Wert-Decode-Helper (z.B. `Property::ContentType
-> String`, `Property::SessionExpiry -> u32`) fehlen. Codec liefert
raw Bytes; Caller muss decoden. Aufwand 0.25 PW.

### §4.1-§4.13 Operational Behavior

**Status:** `partial` — Session-Manager, Subscription-Tree, Retained-
Messages, Will-Handling, QoS-Levels (0/1/2) als Datenmodell + Logik
in `crates/mqtt-bridge/src/broker.rs` + `topic_filter.rs`
implementiert; Keep-Alive-Timer als Wall-Clock-Tick fehlt (statisches
Modell). Aufwand 0.5 PW.

## Decision-Records (`n/a (rejected)`)

— keine.
