# zerodds-monitor-1.1 — offene Punkte

Aggregat aus `zerodds-monitor-1.1.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §4 W3C-Trace-Context als RTPS-Vendor-PID (0x0D00)

**Status:** `partial` — Wire-Format, Roundtrip und Ablehnung ungültiger Payloads sind gebaut und geprüft; die PID liegt im Protokollbereich statt im Vendor-Bereich (§4.1a).

## §4.1a PID im Vendor-Bereich

**Status:** `open` — `crates/monitor/src/trace_context.rs` setzt `PID_VENDOR_TRACE_CONTEXT = 0x0D00`; eine Vendor-PID ab `0x8000` fehlt.
