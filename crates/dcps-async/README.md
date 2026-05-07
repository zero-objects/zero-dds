# `zerodds-dcps-async`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-dcps-async/badge.svg)](https://docs.rs/zerodds-dcps-async)

Runtime-agnostische async-Wrappers um die [`zerodds-dcps`](../dcps) Sync-API. Newtypes teilen den internen `Arc<...>` mit den Sync-Pendants — kein State-Duplikat, kein Performance-Overhead. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| ZeroDDS-async 1.0 | §1–§9 (komplett) |
| OMG DDS 1.4 | §2.2.2.4–5 (DataWriter/DataReader-API) |

## Was ist drin

- **Async-Newtypes** — `AsyncDomainParticipantFactory`, `AsyncDomainParticipant`, `AsyncPublisher`, `AsyncDataWriter<T>`, `AsyncSubscriber`, `AsyncDataReader<T>`. Jeder Newtype haelt einen `Arc<...>` auf das Sync-Pendant.
- **`write().await`** — yield-basierte Retry-Schleife auf `OutOfResources`-Backpressure (Spec §5.1). `Condvar::wait_timeout`-Sync-Block durch `yield_for`-Future ersetzt; Caller-Tasks bleiben cancelable.
- **`take(timeout).await`** — Polling-Future mit Deadline; leerer `Vec<T>` bei Timeout (analog Sync-Semantik).
- **`SampleStream`** (Spec §2.2.1) — `Stream<Item = T>`. Live-Mode nutzt `register_user_reader_waker` an der Runtime; Wake erfolgt beim `sample_tx.send` (kein Polling). Offline-Mode: detached-Thread-Sleep als Polling-Fallback (Spec §3.3).
- **`DataAvailableStream`** (Spec §6.1) — `Stream<Item = ()>`. Signalisiert "neue Daten verfuegbar" pro Sample-Zufluss; konsumiert keine Samples (Caller ruft `take()` separat).
- **`PublicationMatchedStream`** (Spec §6.2) — `Stream<Item = SubscriptionMatchedStatus>`. Emittiert den vollen Reader-side Match-Status (DDS 1.4 §2.2.4.1 SUBSCRIPTION_MATCHED) bei jeder Aenderung.
- **`wait_for_matched_*`-Futures** — Async-Polling-Schleifen mit Deadline.

## Schichten-Position

Layer 4 — Core Services. Aufgesetzt auf `zerodds-dcps` (Layer 4) und `zerodds-qos` (Layer 1). Runtime-agnostisch — nutzt `futures-core::Stream`, optional `tokio::time::sleep` mit Feature `tokio-glue`. Default-Pfad ist Detached-Thread-Sleep.

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

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | Standard-Library + Threads + Detached-Thread-Sleep. |
| `tokio-glue` | ❌ | `tokio::time::sleep` als Backend fuer `yield_for` (statt detached-Thread). Reduziert Spawn-Overhead bei tokio-basiertem Caller. |

## Stabilitaet

Alle `pub`-Items sind ab `1.0.0` stabil; Breaking-Changes erfordern Major-Bump.

## Tests

```bash
cargo test -p zerodds-dcps-async
```

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`zerodds-dcps`](../dcps) — die zugrundeliegende Sync-API
- [`docs/specs/zerodds-async-1.0.md`](../../docs/specs/zerodds-async-1.0.md) — async-API-Spec
