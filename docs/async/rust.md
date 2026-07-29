<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS async — Rust

The `zerodds-dcps-async` crate (`zerodds-async-1.0`) is the reference async
surface. Runtime-agnostic: only `futures-core` at its core, so it runs on Tokio,
async-std or smol. It wraps `zerodds-dcps` without state duplication — the async
newtypes share the same `Arc`, and the reader wakes futures directly on sample
arrival (no polling in live mode). Method names match the sync API.

Runnable walkthrough covering the whole surface:
[`rust-async-showcase`](https://github.com/zero-objects/zero-dds-snippets/tree/main/rust-async-showcase).

## Setup

```toml
[dependencies]
zerodds-dcps-async = { version = "1.0.0-rc.7", features = ["tokio-glue"] }
zerodds-dcps = "1.0.0-rc.7"
```

## Participant, writer, reader

```rust
use zerodds_dcps_async::*;

let factory = AsyncDomainParticipantFactory::instance();
let p = factory.create_participant(0)?;                       // live (loopback discovery)
let topic  = p.create_topic::<MyType>("T", TopicQos::default())?;
let writer = p.create_publisher(PublisherQos::default())
              .create_datawriter::<MyType>(&topic, DataWriterQos::default())?;
let reader = p.create_subscriber(SubscriberQos::default())
              .create_datareader::<MyType>(&topic, DataReaderQos::default())?;
```

## Futures

```rust
writer.wait_for_matched_subscription(1, T).await?;
reader.wait_for_matched_publication(1, T).await?;

writer.write(&sample).await?;                                 // reliable: suspends on backpressure
let batch      = reader.take(dur).await?;                     // Vec<T>, consuming
let seen       = reader.read(dur).await?;                     // non-consuming
let with_info  = reader.take_with_info(dur).await?;           // Vec<Sample<T>> (SampleInfo)

// instance lifecycle
let h = writer.register_instance(&sample).await?;
writer.dispose(&sample, h).await?;
writer.unregister_instance(&sample, h).await?;
```

## Streams

```rust
use futures::StreamExt;

let mut s      = reader.take_stream();                        // Stream<Item = T>
let mut si     = reader.take_stream_with_info();              // Stream<Item = Sample<T>>
let mut avail  = reader.data_available_stream();              // Stream<Item = ()>
let mut matched= reader.publication_matched_stream();         // Stream<Item = SubscriptionMatchedStatus>

while let Some(sample) = s.next().await { /* … */ }
```

## Content-filtered readers

```rust
// closure filter
let r = subscriber.create_datareader_filtered::<MyType, _>(
    &topic, DataReaderQos::default(), |x| x.value > 100)?;

// SQL ContentFilteredTopic
let cft = participant.create_contentfilteredtopic::<MyType>(
    "F", &topic, "value > %0", vec!["100".to_string()])?;
let r = subscriber.create_datareader_cft::<MyType>(&cft, DataReaderQos::default())?;
```

## Tokio integration (optional `tokio-glue` feature)

```rust
let live = factory.spawn_in_tokio(0, &tokio::runtime::Handle::current())?;
```

Drives the participant's event loop on the Tokio runtime, so the streams above
become fully waker-driven (no polling).

## Guarantees

- **Zero extra heap per `write`** vs the sync path (dhat-verified).
- **No sample loss** through polling latency — a reliable reader draining
  `take_stream` surfaces 100 % of published samples.
- Byte-identical wire to the sync API; the two interoperate on the same topic.
