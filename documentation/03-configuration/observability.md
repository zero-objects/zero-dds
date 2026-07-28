# Observability

ZeroDDS exposes three layers of observability — pick the one that
matches your operational needs.

## Layer 1: lock-free atomic stats

`HistoryCacheStats` carries `len`,
`evicted`, `max_sn`, `min_sn` as atomics. A monitoring thread
holds an `Arc<HistoryCacheStats>` and polls without taking any
lock — zero impact on the writer / reader hot path.

```rust
let stats = some_cache.stats();          // Arc clone, cheap
loop {
    let snap = stats.snapshot();
    println!("cache: len={}, max_sn={:?}", snap.len, snap.max_sn);
    std::thread::sleep(std::time::Duration::from_secs(1));
}
```

## Layer 2: Sink-based events

`zerodds_foundation::observability` defines a
sink trait; `DcpsRuntime` emits coarse-grained lifecycle events
through it.

Events emitted today:

| Event | Component | Level | Attributes |
|---|---|---|---|
| `user_writer.created` | dcps | info | `topic`, `type`, `reliable` |
| `user_reader.created` | dcps | info | `topic`, `type` |
| `writer.matched_remote_reader` | discovery | info | `writer_eid` |

### Built-in sinks

| Sink | Use case |
|---|---|
| `NullSink` | Default — no-op, zero overhead |
| `VecSink` | Tests — collects events into a `Vec` |
| `StderrJsonSink` | Production — one JSON-line per event on stderr |

### Wire it up

```rust
use std::sync::Arc;
use zerodds_foundation::observability::StderrJsonSink;
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};

let cfg = RuntimeConfig {
    observability: Arc::new(StderrJsonSink::new()),
    ..Default::default()
};
let rt = DcpsRuntime::start(0, prefix, cfg)?;
```

> ▶ Runnable example: [`observability-stderr-sink`](https://github.com/zero-objects/zero-dds-snippets/tree/master/observability-stderr-sink)
> (starts a real `DcpsRuntime` with this exact config and registers a
> writer + reader, so the JSON lines below actually print).

stderr output:

```json
{"level":"info","component":"dcps","name":"user_writer.created","attrs":{"topic":"Telemetry","type":"Robot::Pose","reliable":"true"}}
{"level":"info","component":"dcps","name":"user_reader.created","attrs":{"topic":"Commands","type":"Robot::Cmd"}}
{"level":"info","component":"discovery","name":"writer.matched_remote_reader","attrs":{"writer_eid":"EntityId(...)"}}
```

This is directly consumable by Vector, fluentd, the Datadog
agent, journald, or any log shipper that understands JSON-lines.

### Custom sinks

Implement `zerodds_foundation::observability::Sink`:

```rust
use zerodds_foundation::observability::{Event, Sink};

struct MetricsSink { /* ... */ }
impl Sink for MetricsSink {
    fn record(&self, event: &Event) {
        // increment a counter, push to a queue, etc.
    }
}
```

The trait is `Send + Sync`, so wrap in `Arc<dyn Sink>` and inject.

## Layer 3: OTel bridge

`crates/observability-otlp` (`zerodds-observability-otlp`) ships
`OtlpExporter` — an OTLP/HTTP/JSON exporter for
`zerodds_foundation::tracing::{Span, Histogram}` and
`zerodds_foundation::observability::Event`. It does **not**
implement `Sink` (spans/histograms need more than `record(&Event)`
can carry), so it is wired up manually rather than through
`RuntimeConfig.observability`: buffer spans/histograms/events with
`add_span` / `add_histogram` / `add_event`, then `flush()` — on a
periodic background thread for production use (see
`spawn_otlp_flush_loop` in any bridge daemon, e.g.
`crates/mqtt-bridge/src/daemon/runtime_common.rs`).

```rust
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};

let cfg = OtlpConfig {
    host: "otelcol".into(),
    port: 4318,
    ..OtlpConfig::default()
};
let exporter = OtlpExporter::new(cfg);

exporter.add_event(event); // zerodds_foundation::observability::Event
exporter.flush()?; // POSTs the batch to /v1/traces, /v1/metrics, /v1/logs
```

> ▶ Runnable example: [`rust-audit-otlp`](https://github.com/zero-objects/zero-dds-snippets/tree/master/rust-audit-otlp)
> (constructs this exact `OtlpConfig`/`OtlpExporter` and calls `flush()`).

The collector then forwards to Jaeger / Tempo / Datadog / any OTel
backend. See the OTLP spec at
<https://opentelemetry.io/docs/specs/otlp/>, and
`crates/observability-otlp/examples/jaeger_talker.rs` for a full
runnable example against a local Jaeger stack.

## Layer 4: traceability tooling

`zerodds-traceability` is a wire-trace decoder — point it at a tcpdump
pcap and it prints submessage-level timeline. Good for forensics
when monitoring already says "something is wrong".

```bash
sudo tcpdump -i eth0 -w trace.pcap "udp and (port 7400 or portrange 7401-7900)"
zerodds-traceability trace.pcap --decode --histogram
```

## What's *not* observed (yet)

- Per-sample latency — emit via the bench tool
  (`roundtrip-1us`) until the OTel-spans bridge is wired up.
- Memory / heap — use Linux `pidstat`, `eBPF` `bpftrace` or any
  generic Rust tooling.
- CPU usage — same.

## Practical advice

- Default to `StderrJsonSink` in production. Cheap, log-shipper-
  agnostic.
- Keep `RuntimeConfig.observability` set to `null_sink()` in
  unit tests to keep test output clean.
- `VecSink::snapshot()` is the right thing for integration tests
  that assert "writer.created event fired".
