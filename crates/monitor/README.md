# `zerodds-monitor`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-monitor/badge.svg)](https://docs.rs/zerodds-monitor)

Observability substrate for the [ZeroDDS](https://zerodds.org) stack:
counter/gauge/histogram registry, Prometheus text exporter,
W3C trace-context PID codec, standard span schema. Safety
classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| ZeroDDS-Monitor 1.0 | §1 (data model), §2 (metric naming, 31 metrics), §3 (Prometheus text format), §4 (PID 0x0D00 W3C trace context), §5 (span schema), §6 (lifecycle), §7 (hook-point table) |
| OpenMetrics | counter/gauge/histogram conventions |
| W3C-Trace-Context 1.0 | `traceparent` + `tracestate` wire format |

## What's inside

- **`Counter` / `Gauge` / `LabeledHistogram`** — atomic metric types.
- **`Labels`** — sorted `&'static str → String` pairs.
- **`Registry` / `default_registry()`** — single source of truth, idempotent lookup.
- **`render_prometheus(&snapshot) -> String`** — OpenMetrics text exposition with label escaping.
- **`serve_prometheus(addr, registry)`** (feature `prometheus-server`) — mini-HTTP `/metrics` endpoint.
- **`PID_VENDOR_TRACE_CONTEXT` (0x0D00)** + `TraceContextPid::encode_inline_qos / decode_inline_qos` — RTPS inline-QoS codec for W3C trace-context propagation.
- **`metric_names::*`** — 31 standard metric constants (transport / RTPS / DCPS / discovery / security).
- **`span_names::*`** + `attr::*` — 9 standard span names + DDS attribute keys.
- **`MonitorConfig` / `TraceContextEmission`** — lifecycle configuration.

## Layer position

Layer 4 — core services. Substrate for:
- `zerodds-observability-otlp` (OTLP/HTTP/JSON exporter)
- `tools/dashboard` (Prometheus scrape)
- `tools/perf` (latency histograms)

The foundation substrate (`Histogram`, `Span`, `TraceId`, `SpanId`, `Event`, `Sink` trait) still lives in `zerodds-foundation` — `monitor` re-exports the `Histogram` and uses the others via the module path.

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

PID 0x0D00 roundtrip:

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

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | standard library + Mutex + atomics. |
| `alloc` | ✅ (via std) | `Vec`/`Arc`/`String`. |
| `prometheus-server` | ✅ | mini-HTTP server for `/metrics` (no hyper dep). |

## Stability

`1.0.0-rc.1` is the initial release materialization. The public API,
metric names, label keys, PID-0x0D00 wire format and span names are
RC1-stable; breaking changes require a major bump.

## Tests

```bash
cargo test -p zerodds-monitor
```

40 unit tests + 1 doc test, of which 28 cover the counter/gauge/histogram/
registry/Prometheus render path, 8 the PID-0x0D00 codec, 2 the
mini-HTTP server, and 2 each metric names + span names.

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/specs/zerodds-monitor-1.1.md`](../../docs/specs/zerodds-monitor-1.1.md) — spec.
- [`docs/architecture/05_observability_and_tooling.md`](../../docs/architecture/05_observability_and_tooling.md) — architecture.
- [`zerodds-observability-otlp`](../observability-otlp) — OTLP/HTTP/JSON exporter (consumes `Span`/`Histogram`/`Event` from foundation).
- [`zerodds-foundation`](../foundation) — substrate (`tracing`, `observability`).
