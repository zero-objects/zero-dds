# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-dcps-async` crate.

### Spec references

- **`docs/specs/zerodds-async-1.0.md`** §1–§9 — complete async wrapper API: §2 (newtypes), §3 (stream semantics + Reader-Slot waker), §5 (backpressure retry), §6 (listener bridge: `data_available_stream`/`publication_matched_stream`), §7 (error mapping), §8 (test strategy), §9 (performance targets).
- **OMG DDS 1.4** §2.2.2.4 (DataWriter) + §2.2.2.5 (DataReader) — mirrored sync API. §2.2.4.1 SUBSCRIPTION_MATCHED — status struct emitted by the `PublicationMatchedStream`.

### Public API

- `AsyncDomainParticipantFactory` — singleton wrapper around `DomainParticipantFactory`. `instance()`, `create_participant_offline`, `create_participant`, `create_participant_with_qos`.
- `AsyncDomainParticipant` — topic/publisher/subscriber creation; shares Arc state with the sync counterpart.
- `AsyncPublisher` / `AsyncDataWriter<T>` — `write(&sample).await`, `register_instance`, `dispose`, `unregister_instance`, `wait_for_matched_subscription(min_count, timeout).await`, `matched_subscription_count`, `qos`, `as_sync`.
- `AsyncSubscriber` / `AsyncDataReader<T>` — `take(timeout).await`, `take_stream() -> SampleStream<T>`, `wait_for_matched_publication`, `matched_publication_count`, `data_available_stream() -> DataAvailableStream<T>`, `publication_matched_stream() -> PublicationMatchedStream<T>`, `qos`, `as_sync`.
- Streams: `SampleStream<T>`, `DataAvailableStream<T>`, `PublicationMatchedStream<T>` — all implement `futures_core::Stream`.
- Re-exports: `DataReaderQos`, `DataWriterQos`, `DdsError`, `DdsType`, `DomainParticipantQos`, `InstanceHandle`, `PublisherQos`, `Result`, `SubscriberQos`, `Topic`, `TopicQos`, `SubscriptionMatchedStatus`.

### Implementation

`AsyncDataWriter::write` is a Future form over `DataWriter::write` with a yield-based retry loop: on `OutOfResources` (queue full + Reliable + `max_blocking_time > 0`) the Future suspends via `yield_for(2 ms)` and retries until drain or deadline. Instead of `Condvar::wait_timeout` (sync path), the caller task stays cancelable.

`SampleStream::poll_next` registers in live mode with `register_user_reader_waker` on the `DcpsRuntime` — the waker fires directly on `sample_tx.send` (no polling). In offline mode a detached-thread sleep serves as the polling fallback. Buffered samples are yielded one per poll.

`DataAvailableStream::poll_next` calls the non-consuming `DataReader::read()` (DDS 1.4 §2.2.2.5.3.5) and compares the sample count against the one at the last emission. Rising count → emit `()` event. Samples stay in the reader cache; the caller must consume them separately via `take()`/`take_stream`. Live-mode wake via `register_user_reader_waker`, offline-mode polling as fallback.

`PublicationMatchedStream::poll_next` watches `matched_publication_count` and emits, per change, a `SubscriptionMatchedStatus` snapshot with synthesized `total_count`/`current_count`/`*_change` fields (reader-side counter, since the sync path does not expose a direct `subscription_matched_status()` getter).

`yield_for` is runtime-agnostic: without the `tokio-glue` feature it spawns a detached thread that wakes the waker after expiry; with `tokio-glue` it uses `tokio::time::sleep`.

### Architecture

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-dcps`, `zerodds-qos`, `futures-core`. Optional: `tokio` (feature `tokio-glue`).
- **Dependents (out):** end-user applications that want to consume DDS via the async path; bridges (mqtt-/coap-/grpc-bridge) where the async form is natural for the respective bridge backend.
- **Feature flags:** `std` (default), `tokio-glue`.

### Stability

All `pub` items are RC1-stable; breaking changes require a major bump to `2.0.0`.
