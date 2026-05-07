# `zerodds-monitor`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-monitor/badge.svg)](https://docs.rs/zerodds-monitor)

Observability-Substrate fuer den [ZeroDDS](https://zerodds.org)-Stack:
Counter/Gauge/Histogram-Registry, Prometheus-Text-Exporter,
W3C-Trace-Context-PID-Codec, Standard-Span-Schema. Safety
classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| ZeroDDS-Monitor 1.0 | §1 (Datenmodell), §2 (Metric-Naming, 31 Metriken), §3 (Prometheus-Text-Format), §4 (PID 0x0D00 W3C-Trace-Context), §5 (Span-Schema), §6 (Lifecycle), §7 (Hook-Point-Tabelle) |
| OpenMetrics | Counter/Gauge/Histogram-Konventionen |
| W3C-Trace-Context 1.0 | `traceparent` + `tracestate` Wire-Format |

## Was ist drin

- **`Counter` / `Gauge` / `LabeledHistogram`** — atomare Metric-Typen.
- **`Labels`** — sortierte `&'static str → String`-Pairs.
- **`Registry` / `default_registry()`** — Single-Source-of-Truth, idempotenter Lookup.
- **`render_prometheus(&snapshot) -> String`** — OpenMetrics-Text-Exposition mit Label-Escaping.
- **`serve_prometheus(addr, registry)`** (Feature `prometheus-server`) — Mini-HTTP `/metrics`-Endpoint.
- **`PID_VENDOR_TRACE_CONTEXT` (0x0D00)** + `TraceContextPid::encode_inline_qos / decode_inline_qos` — RTPS-Inline-QoS-Codec fuer W3C-Trace-Context-Propagation.
- **`metric_names::*`** — 31 Standard-Metric-Konstanten (Transport / RTPS / DCPS / Discovery / Security).
- **`span_names::*`** + `attr::*` — 9 Standard-Span-Namen + DDS-Attribut-Keys.
- **`MonitorConfig` / `TraceContextEmission`** — Lifecycle-Konfiguration.

## Schichten-Position

Layer 4 — Core Services. Substrate fuer:
- `zerodds-observability-otlp` (OTLP/HTTP/JSON-Exporter)
- `tools/dashboard` (Prometheus-Scrape)
- `tools/perf` (Latenz-Histogramme)

Foundation-Substrate (`Histogram`, `Span`, `TraceId`, `SpanId`, `Event`, `Sink`-Trait) lebt weiterhin in `zerodds-foundation` — `monitor` re-exportiert das `Histogram` und nutzt die anderen via Module-Path.

## Quickstart

```rust
use zerodds_monitor::{default_registry, Labels, metric_names};

let reg = default_registry();
let counter = reg.counter(
    metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
    Labels::new().with("topic", "VehicleTracking.TrackUpdate"),
);
counter.inc();
counter.add(5);
assert_eq!(counter.get(), 6);

println!("{}", reg.render_prometheus());
```

PID 0x0D00 Roundtrip:

```rust,ignore
use zerodds_monitor::{TraceContextPid, TraceParent, TraceState};
use zerodds_foundation::tracing::{TraceId, SpanId};

let tp = TraceParent::new(
    TraceId([0x4b, 0xf9, /* ... */]),
    SpanId([0x00, 0xf0, /* ... */]),
    0x01,
);
let pid = TraceContextPid::new(tp, Some(TraceState::new("dds=topic:Foo")));
let mut buf = Vec::new();
pid.encode_inline_qos(&mut buf);
let decoded = TraceContextPid::decode_inline_qos(&buf).expect("roundtrip");
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Standard-Library + Mutex + Atomics. |
| `alloc` | ✅ (via std) | `Vec`/`Arc`/`String`. |
| `prometheus-server` | ✅ | Mini-HTTP-Server fuer `/metrics` (kein hyper-Dep). |

## Stabilitaet

`1.0.0-rc.1` ist die initiale Release-Materialisierung. Public-API,
Metric-Namen, Label-Keys, PID-0x0D00-Wire-Format und Span-Namen sind
RC1-stabil; Breaking-Changes erfordern Major-Bump.

## Tests

```bash
cargo test -p zerodds-monitor
```

40 Unit-Tests + 1 Doc-Test, davon 28 fuer den Counter/Gauge/Histogram/
Registry/Prometheus-Render-Pfad, 8 fuer den PID-0x0D00-Codec, 2 fuer
den Mini-HTTP-Server, je 2 fuer Metric-Namen + Span-Namen.

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/specs/zerodds-monitor-1.0.md`](../../docs/specs/zerodds-monitor-1.0.md) — Spec.
- [`docs/architecture/05_observability_and_tooling.md`](../../docs/architecture/05_observability_and_tooling.md) — Architektur.
- [`zerodds-observability-otlp`](../observability-otlp) — OTLP/HTTP/JSON-Exporter (konsumiert `Span`/`Histogram`/`Event` aus foundation).
- [`zerodds-foundation`](../foundation) — Substrate (`tracing`, `observability`).
