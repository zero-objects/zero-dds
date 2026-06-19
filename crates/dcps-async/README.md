# `zerodds-dcps-async`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-dcps-async/badge.svg)](https://docs.rs/zerodds-dcps-async)

Runtime-agnostic async wrappers around the [`zerodds-dcps`](../dcps) sync API. Newtypes share the internal `Arc<...>` with the sync counterparts — no state duplicate, no performance overhead. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| ZeroDDS-async 1.0 | §1–§9 (complete) |
| OMG DDS 1.4 | §2.2.2.4–5 (DataWriter/DataReader API) |

## What's inside

- **Async newtypes** — `AsyncDomainParticipantFactory`, `AsyncDomainParticipant`, `AsyncPublisher`, `AsyncDataWriter<T>`, `AsyncSubscriber`, `AsyncDataReader<T>`. Each newtype holds an `Arc<...>` to the sync counterpart.
- **`write().await`** — yield-based retry loop on `OutOfResources` backpressure (spec §5.1). The `Condvar::wait_timeout` sync block is replaced by a `yield_for` Future; caller tasks stay cancelable.
- **`take(timeout).await`** — polling Future with deadline; empty `Vec<T>` on timeout (analogous to sync semantics).
- **`SampleStream`** (spec §2.2.1) — `Stream<Item = T>`. Live mode uses `register_user_reader_waker` on the runtime; wake happens on `sample_tx.send` (no polling). Offline mode: detached-thread sleep as polling fallback (spec §3.3).
- **`DataAvailableStream`** (spec §6.1) — `Stream<Item = ()>`. Signals "new data available" per sample inflow; consumes no samples (the caller calls `take()` separately).
- **`PublicationMatchedStream`** (spec §6.2) — `Stream<Item = SubscriptionMatchedStatus>`. Emits the full reader-side match status (DDS 1.4 §2.2.4.1 SUBSCRIPTION_MATCHED) on every change.
- **`wait_for_matched_*` Futures** — async polling loops with deadline.

## Layer position

Layer 4 — Core Services. Built on `zerodds-dcps` (layer 4) and `zerodds-qos` (layer 1). Runtime-agnostic — uses `futures-core::Stream`, optionally `tokio::time::sleep` with feature `tokio-glue`. The default path is detached-thread sleep.

## Quickstart

```rust,ignore
use zerodds_dcps_async::{
    AsyncDomainParticipantFactory, DataReaderQos, DataWriterQos,
    PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_dcps::RawBytes;

#[tokio::main]
async fn main() {
    let factory = AsyncDomainParticipantFactory::instance();
    let participant = factory.create_participant_offline(0);
    let topic = participant
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .expect("topic");

    let pub_ = participant.create_publisher(PublisherQos::default());
    let writer = pub_
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    writer.write(&RawBytes::new(vec![1, 2, 3])).await.unwrap();
}
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Standard library + threads + detached-thread sleep. |
| `tokio-glue` | ❌ | `tokio::time::sleep` as backend for `yield_for` (instead of detached thread). Reduces spawn overhead with a tokio-based caller. |

## Stability

All `pub` items are stable from `1.0.0`; breaking changes require a major bump.

## Tests

```bash
cargo test -p zerodds-dcps-async
```

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`zerodds-dcps`](../dcps) — the underlying sync API
- [`docs/specs/zerodds-async-1.0.md`](../../docs/specs/zerodds-async-1.0.md) — async API spec
