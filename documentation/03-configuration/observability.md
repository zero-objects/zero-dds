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

`crates/observability-otlp` (work-in-progress) wraps the Sink
trait into OTLP-HTTP-JSON shipping to an OTel collector. When
released:

```rust
use zerodds_observability_otlp::OtlpHttpSink;

let cfg = RuntimeConfig {
    observability: Arc::new(OtlpHttpSink::new("http://otelcol:4318/v1/traces")?),
    ..Default::default()
};
```

The collector then forwards to Jaeger / Tempo / Datadog / any OTel
backend. See the OTLP spec at
<https://opentelemetry.io/docs/specs/otlp/>.

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
