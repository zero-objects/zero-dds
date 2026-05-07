# Rust

The native API. Use this when your application is Rust.

## Cargo

```toml
[dependencies]
zerodds-dcps = { git = "https://github.com/zero-objects/zero-dds.git" }
zerodds-rtps = { git = "https://github.com/zero-objects/zero-dds.git" }
zerodds-qos  = { git = "https://github.com/zero-objects/zero-dds.git" }
```

The `zerodds-rs` re-export crate (idiomatic facade) is in progress;
until then use the lower-level crates directly.

## Hello, world

See [01 Getting Started → first publisher](../01-getting-started/first-publisher.md)
for the canonical example.

## Threading model

`DcpsRuntime` is `Arc`-shared and thread-safe. All public methods
take `&self`. Spawn as many publisher / subscriber threads as
you like.

```rust
let rt = Arc::new(DcpsRuntime::start(0, prefix, cfg)?);

let p = Arc::clone(&rt);
std::thread::spawn(move || {
    let eid = p.register_user_writer(/* … */).unwrap();
    p.write_user_sample(eid, b"hello".to_vec()).unwrap();
});
```

## Error handling

Public methods return `Result<T, zerodds_dcps::DdsError>`. Match on
the enum variants (e.g., `BadParameter`, `PreconditionNotMet`)
to differentiate.

## Async vs sync

The DCPS API is synchronous. The `mpsc::Receiver` returned by
`register_user_reader` blocks on `recv()` (with timeout via
`recv_timeout`). For async (`tokio` / `async-std`), wrap the
receiver in a channel adapter — there is no built-in async
support yet.

## Working with typed payloads

Use `zerodds-idlc` to generate Rust stubs from `.idl` files; the
generated `encode_cdr()` / `decode_cdr()` produce / consume the
`Vec<u8>` that `write_user_sample` and `mpsc::Receiver<Vec<u8>>`
exchange.

```rust
use robot::Telemetry;

// Publish:
let sample = Telemetry { robot_id: "r1".into(), pose: ..., t_nanos: 0 };
let bytes = sample.encode_cdr();
rt.write_user_sample(eid, bytes)?;

// Subscribe:
if let Ok(bytes) = rx.recv_timeout(Duration::from_secs(1)) {
    let sample = Telemetry::decode_cdr(&bytes)?;
}
```

## Listeners (status callbacks)

DCPS-spec listeners (e.g., `on_data_available`,
`on_offered_deadline_missed`) are exposed via the
`crates/dcps/src/listener.rs` API. Today these are polled via
status-snapshot methods (`user_writer_offered_deadline_missed`,
`user_reader_liveliness_status`). Native callback registration
is in progress.

## Configuration

See [03 Configuration](../03-configuration/README.md). The most
common setup:

```rust
use std::sync::Arc;
use zerodds_foundation::observability::StderrJsonSink;

let cfg = RuntimeConfig {
    observability: Arc::new(StderrJsonSink::new()),
    ..Default::default()
};
```

## Real-time

Apply CPU pinning + RT-scheduler from the `zerodds-rt-linux` crate
to the threads you spawn for publishing / subscribing:

```rust
use zerodds_rt_linux::{SchedulerProfile, pin_current_thread_to_cpus};

std::thread::spawn(|| {
    pin_current_thread_to_cpus(&[3]).unwrap();
    SchedulerProfile::RealtimeFifo { priority: 60 }
        .apply_to_current_thread().unwrap();
    // … hot-path loop
});
```

See `docs/REALTIME_DEPLOYMENT.md` (internal repo only) for the
kernel-tuning side.

## Reading further

- `crates/dcps/src/runtime.rs` — the rustdoc has every public
  method documented.
- `crates/qos/src/lib.rs` — every QoS policy.
- `crates/types/` — TopicType trait + TypeObject.
