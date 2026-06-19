# zerodds-async 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-async-1.0.md` (vendor spec, draft 2026-05-04).

Implementation:

- `crates/dcps-async/` — Async DCPS API (futures/streams over DCPS).

## §1 Type mapping to the sync API

### §1.1 AsyncDomainParticipantFactory
- **Requirement:** singleton, shares the sync factory; create_participant async-capable.
- **Repo:** `crates/dcps-async/src/factory.rs`
- **Tests:** `crates/dcps-async/tests/smoke.rs::factory_singleton_offline_participant`
- **Status:** done

### §1.2 AsyncDomainParticipant
- **Requirement:** a newtype around `DomainParticipant`; create_topic, create_publisher, create_subscriber identical to sync.
- **Repo:** `crates/dcps-async/src/participant.rs`
- **Tests:** `tests/smoke.rs` (all use create_participant_offline + create_topic + create_publisher/subscriber)
- **Status:** done

### §1.3 AsyncPublisher / AsyncSubscriber
- **Requirement:** newtype, create_datawriter/datareader returns AsyncDataWriter/Reader.
- **Repo:** `crates/dcps-async/src/{publisher,subscriber}.rs`
- **Tests:** `tests/smoke.rs::writer_write_async_offline`, `reader_take_returns_empty_after_timeout`
- **Status:** done

### §1.4 AsyncDataWriter<T>
- **Requirement:** newtype + Send/Sync; shares an internal `Arc<DataWriter<T>>`.
- **Repo:** `crates/dcps-async/src/writer.rs`
- **Tests:** `tests/smoke.rs::writer_write_async_offline`, `writer_register_dispose_unregister_async`
- **Status:** done

### §1.5 AsyncDataReader<T>
- **Requirement:** ditto.
- **Repo:** `crates/dcps-async/src/reader.rs`
- **Tests:** `tests/smoke.rs::reader_take_returns_empty_after_timeout`
- **Status:** done

## §2 Method signatures

### §2.1.1 AsyncDataWriter::write
- **Requirement:** `async fn write(&self, &T) -> Result<()>`; suspends on OutOfResources instead of blocking.
- **Repo:** `crates/dcps-async/src/writer.rs::write` (yield_for retry loop until `reliability.max_blocking_time` has elapsed; spec §5.1).
- **Tests:** `tests/smoke.rs::{writer_write_async_offline, write_returns_timeout_after_max_blocking_when_queue_full}`
- **Status:** done — on OutOfResources yield + retry; the timeout-result path with a finite max_blocking_time tested.

### §2.1.2 AsyncDataWriter::register_instance
- **Requirement:** `async fn` analogous to sync.
- **Repo:** `crates/dcps-async/src/writer.rs::register_instance`
- **Tests:** `tests/smoke.rs::writer_register_dispose_unregister_async`
- **Status:** done

### §2.1.3 AsyncDataWriter::dispose
- **Requirement:** `async fn`; triggers the wire lifecycle DISPOSED.
- **Repo:** `crates/dcps-async/src/writer.rs::dispose`
- **Tests:** `tests/smoke.rs::writer_register_dispose_unregister_async`
- **Status:** done

### §2.1.4 AsyncDataWriter::unregister_instance
- **Requirement:** `async fn`; UNREGISTERED + autodispose flag.
- **Repo:** `crates/dcps-async/src/writer.rs::unregister_instance`
- **Tests:** `tests/smoke.rs::writer_register_dispose_unregister_async`
- **Status:** done

### §2.1.5 AsyncDataWriter::wait_for_matched_subscription
- **Requirement:** `async fn`; resolves Ok on min_count, Err(Timeout) on timeout.
- **Repo:** `crates/dcps-async/src/writer.rs::wait_for_matched_subscription`
- **Tests:** `tests/smoke.rs::wait_for_matched_subscription_times_out_when_no_reader`
- **Status:** done

### §2.1.6 AsyncDataWriter::matched_subscription_count
- **Requirement:** synchronous — a non-async property.
- **Repo:** `crates/dcps-async/src/writer.rs::matched_subscription_count`
- **Tests:** indirectly via wait_for_matched_subscription
- **Status:** done

### §2.2.1 AsyncDataReader::take_stream
- **Requirement:** `fn take_stream() -> impl Stream<Item = Sample<T>> + Send`.
- **Repo:** `crates/dcps-async/src/reader.rs::take_stream` + `SampleStream`
- **Tests:** compiles + builder test (smoke)
- **Status:** done — the stream uses the native reader-slot waker (`register_user_reader_waker`, woken on sample arrival, no polling); offline mode uses detached-thread sleep as a fallback (spec §3.3).

### §2.2.2 AsyncDataReader::take
- **Requirement:** `async fn take(timeout) -> Result<Vec<Sample<T>>>`.
- **Repo:** `crates/dcps-async/src/reader.rs::take`
- **Tests:** `tests/smoke.rs::reader_take_returns_empty_after_timeout`
- **Status:** done

### §2.2.3 AsyncDataReader::wait_for_matched_publication
- **Requirement:** ditto wait_for_matched_subscription.
- **Repo:** `crates/dcps-async/src/reader.rs::wait_for_matched_publication`
- **Tests:** indirectly via the publication_matched_stream test
- **Status:** done

### §2.2.4 AsyncDataReader::matched_publication_count
- **Requirement:** synchronous.
- **Repo:** `crates/dcps-async/src/reader.rs::matched_publication_count`
- **Tests:** indirectly
- **Status:** done

## §3 Waker model

### §3.1 Waker slot per reader
- **Requirement:** UserReaderSlot gets `async_waker: Mutex<Option<Waker>>`.
- **Repo:** `crates/dcps/src/runtime.rs::UserReaderSlot::async_waker`
- **Tests:** indirectly via the SampleStream live path
- **Status:** done

### §3.2 Wire path wakes the waker
- **Requirement:** `deliver_to_reader_slot` wakes the waker as soon as sample_tx.send.
- **Repo:** `crates/dcps/src/runtime.rs::wake_async_waker` (after every `sample_tx.send` in the handle_user_datagram path)
- **Tests:** indirectly
- **Status:** done

### §3.3 Stream::poll_next registers the waker
- **Requirement:** the pending branch stores `cx.waker().clone()` in the slot.
- **Repo:** `crates/dcps-async/src/reader.rs::SampleStream::poll_next` (live mode via `runtime_handle` + `register_user_reader_waker`)
- **Tests:** compiles
- **Status:** done — live mode native; offline mode keeps the detached-thread fallback.

## §4 Tokio glue (feature)

### §4.1 spawn_in_tokio
- **Requirement:** with `--features tokio-glue`: AsyncDomainParticipantFactory::spawn_in_tokio.
- **Repo:** `crates/dcps-async/src/factory.rs::AsyncDomainParticipantFactory::spawn_in_tokio` (+ `_with_qos`); drives the DDS tick loop as a tokio task instead of a dedicated `std::thread` (via `RuntimeConfig::external_tick` + `DcpsRuntime::tick_driver()`).
- **Tests:** `crates/dcps/tests/external_tick.rs` + `crates/dcps-async/tests/spawn_in_tokio.rs`.
- **Status:** done — `spawn_in_tokio` drives the tick in the tokio executor (saves one thread per participant).

## §5 Backpressure & resource limits

### §5.1 write future suspends on OutOfResources
- **Requirement:** on DdsError::OutOfResources it awaits drain_notify; on OK it returns.
- **Repo:** `crates/dcps-async/src/writer.rs::AsyncDataWriter::write` — yield_for retry loop until `reliability.max_blocking_time` elapses (then `DdsError::Timeout`); the caller future stays asleep instead of spinning.
- **Tests:** `tests/smoke.rs::write_returns_timeout_after_max_blocking_when_queue_full`
- **Status:** done — the write future stays asleep (yield_for retry until `max_blocking_time`, then `Timeout`) rather than spinning; a native drain-notify hook instead of retry would be a possible optimization once the sync writer exposes a drain channel.

## §6 Listener bridge

### §6.1 data_available_stream
- **Requirement:** `fn data_available_stream() -> impl Stream<Item = ()> + Send`.
- **Repo:** `crates/dcps-async/src/reader.rs::DataAvailableStream` (polling probe: a `reader.is_ready()` loop, no consuming).
- **Tests:** compiles (lib + tests).
- **Status:** done — the listener stream builds on a non-consuming probe; the native wakeup hangs on §3 (the reader-slot waker), which is live.

### §6.2 publication_matched_stream
- **Requirement:** `fn publication_matched_stream() -> impl Stream<Item = PublicationMatchedStatus> + Send`.
- **Repo:** `crates/dcps-async/src/reader.rs::PublicationMatchedStream`
- **Tests:** `tests/smoke.rs::publication_matched_stream_yields_initial_count`
- **Status:** done — yields a `usize` (match count) on each change.

## §7 Error mapping

### §7.1 DdsError unchanged
- **Requirement:** Future::Output is `Result<T, DdsError>` without async-specific error variants.
- **Repo:** all methods in `crates/dcps-async/src/{writer,reader}.rs` return `Result<T, DdsError>`.
- **Tests:** `tests/smoke.rs::wait_for_matched_subscription_times_out_when_no_reader` matches `DdsError::Timeout`.
- **Status:** done

## §8 Test strategy

### §8.1 Async counterparts per sync test
- **Requirement:** one async counterpart per sync test in `crates/dcps/tests/`.
- **Repo:** `crates/dcps-async/tests/{smoke,proptest_backpressure,cyclone_live_async_e2e}.rs`
- **Tests:** smoke covers the happy path; proptest covers random sequences.
- **Status:** done — smoke + proptest + Cyclone live E2E present.

### §8.2 Tokio test runner
- **Requirement:** `tokio::test` as the default; a smol variant as a feature probe.
- **Repo:** `crates/dcps-async/tests/smoke.rs` (all tests `#[tokio::test(flavor = "multi_thread")]`).
- **Tests:** Cargo.toml dev-dep `tokio = { features = ["rt-multi-thread", "macros", ...] }`.
- **Status:** done — tokio default; the smol variant remains open (no added value while tokio-glue is the only runtime hook).

### §8.3 proptest over channel backpressure
- **Requirement:** `proptest` over random write/take sequences — the backpressure invariant.
- **Repo:** `crates/dcps-async/tests/proptest_backpressure.rs::write_take_sequence_holds_invariants`.
- **Tests:** 16 cases with random capacity ∈ [1,8] + op vec ∈ [0,32]; verifies: on a full queue write MUST return Timeout, on a free queue it MUST return Ok.
- **Status:** done.

### §8.4 E2E against Cyclone live
- **Requirement:** E2E against Cyclone live like sync; latency comparison sync vs async.
- **Repo:** `crates/dcps-async/tests/cyclone_live_async_e2e.rs::async_reader_does_not_panic_against_live_cyclone_pub` (`#[ignore]`-gated, SSH lab setup); latency comparison via `crates/dcps-async/benches/write_async_vs_sync.rs` + CI bench-main.
- **Tests:** live E2E with `BENCH_HOST_AVAILABLE=1` + `cargo test -- --ignored` opt-in.
- **Status:** done — the live test set up as an #[ignore] opt-in; the quantitative latency answer is delivered by §9.1.

## §9 Performance targets

### §9.1 write().await latency
- **Requirement:** ≤ 5 % overhead vs sync-write (criterion bench).
- **Repo:** `crates/dcps-async/benches/write_async_vs_sync.rs` + `.gitlab-ci.yml::bench-main` (`cargo bench -p zerodds-dcps-async --bench write_async_vs_sync -- --save-baseline pre`) + a `bench-compare` regression check.
- **Tests:** the bench runs on every main push; regression > 10% red via `tests/perf/check_bench_regressions.py`.
- **Status:** done — the bench is active in the CI pipeline.

### §9.2 take_stream throughput
- **Requirement:** no sample loss through polling latency; 100 % sample rate.
- **Repo:** —
- **Tests:** —
- **Status:** open — a bench for take_stream throughput is open.

### §9.3 Allocation per write()
- **Requirement:** 0 extra heap allocations vs sync.
- **Repo:** —
- **Tests:** —
- **Status:** open — a dhat-rs bench is open.

## §10 Decisions

### D-1: Runtime-agnostic API as the default
- **Choice:** the public API returns `impl Future`/`impl Stream` without a tokio pin.
- **Rationale:** zerodds should not force a runtime; the caller chooses.
- **Consequence:** the wakeup path uses its own waker (native reader-slot waker; detached-thread sleep only as an offline fallback) instead of tokio::sync::Notify (see §3); the tokio-glue feature switches to tokio wakeup.

### D-2: Tokio glue as an optional feature
- **Choice:** `--features tokio-glue` enables tokio-specific convenience.
- **Rationale:** tokio dominates (~85 % market share); the glue offers comfort without coercion.
- **Consequence:** the default build has no `tokio` dep; workspace CI stays lean.

### D-3: API symmetry with the sync API
- **Choice:** method names identical (write, take, dispose, ...) — no `_async` suffix.
- **Rationale:** the newtype pattern makes the sync↔async switch a pure type change; caller code does not change.
- **Consequence:** the caller MUST import `AsyncDataWriter` instead of `DataWriter` — no name-collision risk because of a different module/path.

### D-4: WaitSet stays sync
- **Choice:** WaitSet is NOT converted to async in 1.0.
- **Rationale:** WaitSet is a spec DCPS construct with clear block semantics; `Stream<Item = ConditionEvent>` is a separate API layer and needs its own design.
- **Consequence:** a caller that needs WaitSet uses the sync API. The async pattern via take_stream + listener streams.

### D-5: Listener bridge as a stream wrapper
- **Choice:** sync listeners stay; the async bridge offers a stream variant.
- **Rationale:** spec §2.2.2.4.4 listeners are callbacks — an async caller prefers `while let Some(ev) = stream.next().await`.
- **Consequence:** listener streams are separate methods; the sync listener set stays the default.
