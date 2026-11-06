# zerodds-observability-otlp-1.0 — offene Punkte

Aggregat aus `zerodds-observability-otlp-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §5 Bridge zu `zerodds-monitor::Registry`

**Status:** `partial` — Histogramm-Export und die aufrufseitige Snapshot-Brücke sind gebaut und geprüft; Counter und Gauge gehen laut Designentscheidung nur über Prometheus, der OTLP-Export dafür fehlt (§5.1).

## §5.1 Counter und Gauge über OTLP

**Status:** `open` — `OtlpExporter` nimmt nur `Histogram` an (`add_histogram`); OTLP-`Sum`- und `Gauge`-Datenpunkte fehlen.

## §6 Sicherheit

**Status:** `open` — der eingebaute HTTP/1.1-Export spricht kein TLS und setzt keine Authentifizierungs-Header; die Spec verweist TLS an einen Collector-Sidecar.

## Nicht-Ziel — OTLP/gRPC

**Status:** `open` — es gibt nur OTLP/HTTP mit JSON; OTLP/gRPC fehlt.
