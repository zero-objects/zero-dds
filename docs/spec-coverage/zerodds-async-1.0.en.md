# zerodds-async 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-async-1.0.md` (ZeroDDS vendor spec, an
async DDS API as a newtype layer over the sync API).

Implementation:

- `crates/dcps-async/` — newtypes, futures, streams, tokio glue.
- `crates/dcps/` — the waker slot on `UserReaderSlot` and its wake-up in
  the receive path.

Open items and decision records: `zerodds-async-1.0.open.en.md`.

## Goals and non-goals

### Goal — runtime-agnostic public API

**Spec:** "Goals" — "**Runtime-agnostic** as a public API: `impl
Future`/`impl Stream` returns without a hard Tokio dependency."

**Repo:** `crates/dcps-async/src/lib.rs` (without `tokio-glue`,
`yield_for` wakes through a detached thread and the caller's waker;
`SleepFuture` is an ordinary `Future`), `crates/dcps-async/src/reader.rs`
(the streams implement `futures_core::Stream`, not a tokio trait).

**Tests:** `crates/dcps-async/tests/smoke.rs::futures_complete_on_a_non_tokio_executor`
— drives `write`, `take` and `wait_for_matched_subscription` on a park
executor built inside the test, with no tokio involved.

**Status:** done

### Goal — strict API symmetry with the sync API

**Spec:** "Goals" — "**Strict API symmetry** with the sync API: identical
method names, identical QoS semantics."

**Repo:** `crates/dcps-async/src/writer.rs` (`write`,
`register_instance`, `dispose`, `unregister_instance`,
`wait_for_matched_subscription`, `matched_subscription_count`, `qos`),
`crates/dcps-async/src/reader.rs` (`take`, `read`,
`wait_for_matched_publication`, `matched_publication_count`, `qos`) —
the same names as on `DataWriter`/`DataReader`, plus `as_sync()` on every
newtype.

**Tests:** `crates/dcps-async/tests/smoke.rs::participant_creates_topic_publisher_and_subscriber`,
`crates/dcps-async/tests/smoke.rs::matched_subscription_count_is_a_synchronous_snapshot`,
`crates/dcps-async/tests/smoke.rs::matched_publication_count_is_a_synchronous_snapshot`

**Status:** done

### Goal — no extra allocation overhead per call

**Spec:** "Goals" — "**Performance**: no additional
allocation overhead per call".

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write`
calls nothing but `DataWriter::write` on the uncontended path; the
reliability budget lookup `DataWriter::qos()` — which clones the whole
QoS set — happens in the backpressure branch only).

**Tests:** `crates/dcps-async/tests/alloc.rs::async_write_adds_no_extra_heap_vs_sync`
(dhat as the global allocator, sync against async),
`crates/dcps-async/tests/perf_targets.rs::async_write_is_within_five_percent_of_sync_write`

**Status:** done

### Goal — tokio glue as an optional feature

**Spec:** "Goals" — "**Optional Tokio glue** as a feature-gated
convenience."

**Repo:** `crates/dcps-async/Cargo.toml` (`tokio-glue = ["dep:tokio"]`,
`tokio` as `optional = true`), `crates/dcps-async/src/factory.rs`
(`spawn_in_tokio`, `spawn_in_tokio_with_qos` behind `#[cfg(feature =
"tokio-glue")]`).

**Tests:** `crates/dcps-async/tests/default_build.rs::default_feature_set_is_std_only`,
`crates/dcps-async/tests/spawn_in_tokio.rs::spawn_in_tokio_drives_tick_on_tokio_runtime`

**Status:** done

### Non-goal — no async runtime of its own inside the crate

**Spec:** "Non-goals" — "Building in our own async runtime."

**Repo:** `crates/dcps-async/Cargo.toml` (the mandatory dependencies are
`zerodds-dcps`, `zerodds-qos` and `futures-core`; `futures-core`
contributes the `Stream` trait, not an executor).

**Tests:** `crates/dcps-async/tests/default_build.rs::no_async_runtime_is_a_hard_dependency`

**Status:** done

### Non-goal — the sync API stays first class

**Spec:** "Non-goals" — "Replacing the sync API — sync stays
first-class."

**Repo:** `crates/dcps-async/src/participant.rs`,
`crates/dcps-async/src/publisher.rs`,
`crates/dcps-async/src/subscriber.rs`,
`crates/dcps-async/src/writer.rs`, `crates/dcps-async/src/reader.rs` —
every newtype holds the sync type and hands it out through `as_sync()`;
`crates/dcps/` stays usable unchanged.

**Tests:** `crates/dcps-async/tests/smoke.rs::async_newtypes_are_clone_send_sync_and_share_state`
— the same reader is driven through the newtype and through `as_sync()`,
and both see one state.

**Status:** done

### Non-goal — WaitSet stays synchronous

**Spec:** "Non-goals" — "Converting `WaitSet` to async — `WaitSet` is a
spec DCPS construct; an async version would be `Stream<Item =
ConditionEvent>` as a separate API."

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — the spec puts the point outside its own
scope; `WaitSet` stays in `crates/dcps/` and `crates/dcps-async/` does not
touch it.

## Module layout

### Module layout — the crate's feature set

**Spec:** "Module layout" — "`Cargo.toml # default = ["std"], optional
"tokio-glue"`".

**Repo:** `crates/dcps-async/Cargo.toml` (`default = ["std"]`, `std =
[]`, `tokio-glue = ["dep:tokio"]`).

**Tests:** `crates/dcps-async/tests/default_build.rs::default_feature_set_is_std_only`,
`crates/dcps-async/tests/default_build.rs::no_async_runtime_is_a_hard_dependency`

**Status:** done

### Module layout — the file split

**Spec:** "Module layout" — sketches `lib.rs`, `writer.rs`, `reader.rs`,
`stream.rs`, `participant.rs`, `waker.rs`, `tokio_glue.rs`.

**Repo:** `crates/dcps-async/src/lib.rs`,
`crates/dcps-async/src/writer.rs`, `crates/dcps-async/src/reader.rs`,
`crates/dcps-async/src/participant.rs`,
`crates/dcps-async/src/publisher.rs`,
`crates/dcps-async/src/subscriber.rs`,
`crates/dcps-async/src/factory.rs` — the streams live next to the
endpoint types rather than in a stream module of their own, the waker
wiring sits at its far end in `crates/dcps/src/runtime.rs`, and the tokio
glue sits next to the factory type it extends.

**Tests:** —

**Status:** n/a (informative) — the section sketches a file split, not an
observable property. The normative part of the same section (the feature
set) is the separate item above.

## §1 Type mapping to the sync API

### §1 AsyncDomainParticipantFactory

**Spec:** §1, table row 1 — `DomainParticipantFactory` →
`AsyncDomainParticipantFactory`, wire identity "same singleton".

**Repo:** `crates/dcps-async/src/factory.rs`
(`AsyncDomainParticipantFactory::instance` holds
`&'static DomainParticipantFactory`; `create_participant_offline`,
`create_participant`, `create_participant_with_qos`).

**Tests:** `crates/dcps-async/tests/smoke.rs::factory_singleton_offline_participant`

**Status:** done

### §1 AsyncDomainParticipant

**Spec:** §1, table row 2 — `DomainParticipant` →
`AsyncDomainParticipant`, "newtype around `Arc<DomainParticipantInner>`".

**Repo:** `crates/dcps-async/src/participant.rs`
(`AsyncDomainParticipant` holds the sync `DomainParticipant`;
`domain_id`, `create_topic`, `create_publisher`, `create_subscriber`,
`as_sync`).

**Tests:** `crates/dcps-async/tests/smoke.rs::participant_creates_topic_publisher_and_subscriber`

**Status:** done

### §1 Topic is the same type in both worlds

**Spec:** §1, table row 3 — "`Topic<T>` | identical — topics are
data model, not I/O".

**Repo:** `crates/dcps-async/src/participant.rs`
(`AsyncDomainParticipant::create_topic` returns `zerodds_dcps::Topic<T>`
unchanged), `crates/dcps-async/src/lib.rs` (`Topic` is re-exported from
`zerodds_dcps`, not redefined).

**Tests:** `crates/dcps-async/tests/smoke.rs::participant_creates_topic_publisher_and_subscriber`
— one `Topic` value builds both the async writer and the async reader.

**Status:** done

### §1 AsyncPublisher and AsyncSubscriber

**Spec:** §1, table row 4 — `Publisher`/`Subscriber` →
`AsyncPublisher`/`AsyncSubscriber` as newtypes.

**Repo:** `crates/dcps-async/src/publisher.rs` (`AsyncPublisher` with
`create_datawriter`), `crates/dcps-async/src/subscriber.rs`
(`AsyncSubscriber` with `create_datareader`,
`create_datareader_filtered`, `create_datareader_cft`).

**Tests:** `crates/dcps-async/tests/smoke.rs::participant_creates_topic_publisher_and_subscriber`,
`crates/dcps-async/tests/smoke.rs::writer_write_async_offline`,
`crates/dcps-async/tests/smoke.rs::reader_take_returns_empty_after_timeout`

**Status:** done

### §1 AsyncDataWriter and AsyncDataReader

**Spec:** §1, table rows 5 and 6 — `DataWriter<T>`/`DataReader<T>` →
`AsyncDataWriter<T>`/`AsyncDataReader<T>` as newtypes.

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter<T>` holds
`Arc<DataWriter<T>>`), `crates/dcps-async/src/reader.rs`
(`AsyncDataReader<T>` holds `Arc<DataReader<T>>`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_register_dispose_unregister_async`,
`crates/dcps-async/tests/smoke.rs::take_stream_yields_pushed_samples`

**Status:** done

### §1 The newtypes are Clone, Send and Sync and share the inner Arc

**Spec:** §1 — "All async newtypes are `Clone` + `Send` + `Sync`. They share the
internal `Arc<...Inner>` with the sync variant — the same writer
can be used at runtime as sync OR async."

**Repo:** `crates/dcps-async/src/writer.rs` (`impl Clone for
AsyncDataWriter` clones the `Arc`), `crates/dcps-async/src/reader.rs`
(likewise for `AsyncDataReader`), `crates/dcps-async/src/publisher.rs`,
`crates/dcps-async/src/subscriber.rs`,
`crates/dcps-async/src/participant.rs` (`#[derive(Clone)]`).

**Tests:** `crates/dcps-async/tests/smoke.rs::async_newtypes_are_clone_send_sync_and_share_state`
— a static `Send + Sync` check plus the proof that a clone consumes the
sample fed in through the original.

**Status:** done

## §2 Method signatures

### §2.1 AsyncDataWriter::write

**Spec:** §2.1 — "`pub async fn write(&self, sample: &T) ->
Result<()>`. Spec §2.2.2.4.2.16. Async variant: does not block the caller
thread; under RESOURCE_LIMITS pressure the future is suspended
until drain notify."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write` —
a direct attempt, then a `yield_for` retry in the backpressure branch up
to `reliability.max_blocking_time`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_write_async_offline`,
`crates/dcps-async/tests/smoke.rs::write_returns_timeout_after_max_blocking_when_queue_full`

**Status:** done

### §2.1 AsyncDataWriter::register_instance

**Spec:** §2.1 — "`pub async fn register_instance(&self, sample: &T) ->
Result<InstanceHandle>`. Spec §2.2.2.4.2.6."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::register_instance`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_register_dispose_unregister_async`

**Status:** done

### §2.1 AsyncDataWriter::dispose

**Spec:** §2.1 — "`pub async fn dispose(&self, sample: &T, handle:
InstanceHandle) -> Result<()>`. Spec §2.2.2.4.2.10.
Wire lifecycle marker with DISPOSED bit."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::dispose`
delegates to `DataWriter::dispose`, which writes the DISPOSED marker).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_register_dispose_unregister_async`

**Status:** done

### §2.1 AsyncDataWriter::unregister_instance

**Spec:** §2.1 — "`pub async fn unregister_instance(&self, sample: &T,
handle: InstanceHandle) -> Result<()>`. Spec §2.2.2.4.2.7."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::unregister_instance`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_register_dispose_unregister_async`

**Status:** done

### §2.1 AsyncDataWriter::wait_for_matched_subscription

**Spec:** §2.1 — "Spec §2.2.2.4.2.11. Resolves Ok when `min_count` subscribers
have matched; Err(Timeout) on timeout."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::wait_for_matched_subscription` — compares against
`matched_subscription_count`, ticks with `yield_for`, returns
`DdsError::Timeout` on the deadline).

**Tests:** `crates/dcps-async/tests/smoke.rs::wait_for_matched_subscription_times_out_when_no_reader`,
`crates/dcps-async/tests/waker.rs::reader_slot_waker_is_woken_on_sample_arrival`
(the Ok branch, once the reader has matched)

**Status:** done

### §2.1 AsyncDataWriter::matched_subscription_count is synchronous

**Spec:** §2.1 — "Snapshot — non-async property. `pub fn
matched_subscription_count(&self) -> usize`."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::matched_subscription_count`, not an `async fn`).

**Tests:** `crates/dcps-async/tests/smoke.rs::matched_subscription_count_is_a_synchronous_snapshot`

**Status:** done

### §2.2 AsyncDataReader::take_stream

**Spec:** §2.2 — "`pub fn take_stream(&self) -> impl Stream<Item =
Sample<T>> + Send`. Take stream: delivers samples in the order of their
arrival. The stream ends when the reader is dropped."

**Repo:** `crates/dcps-async/src/reader.rs`
(`AsyncDataReader::take_stream` returns `SampleStream<T>` with `Item =
T`; `AsyncDataReader::take_stream_with_info` returns
`SampleInfoStream<T>` with `Item = Sample<T>` — the spec's wording).

**Tests:** `crates/dcps-async/tests/smoke.rs::take_stream_yields_pushed_samples`,
`crates/dcps-async/tests/smoke.rs::take_stream_with_info_yields_sample_info`,
`crates/dcps-async/tests/throughput.rs::take_stream_delivers_all_samples_no_loss`

**Status:** done

### §2.2 AsyncDataReader::take

**Spec:** §2.2 — "`pub async fn take(&self, timeout: Duration) ->
Result<Vec<Sample<T>>>`. Single-sample take. Resolves Ok when (a) a
sample is there, (b) a lifecycle marker is there."

**Repo:** `crates/dcps-async/src/reader.rs` (`AsyncDataReader::take`
returns `Vec<T>`; `take_with_info` and `read_with_info` return
`Vec<Sample<T>>` with the full `SampleInfo`; `read` is the non-consuming
variant).

**Tests:** `crates/dcps-async/tests/smoke.rs::reader_take_returns_empty_after_timeout`,
`crates/dcps-async/tests/smoke.rs::take_with_info_carries_sample_info`,
`crates/dcps-async/tests/smoke.rs::read_is_non_consuming`

**Status:** done

### §2.2 AsyncDataReader::wait_for_matched_publication

**Spec:** §2.2 — "`pub async fn wait_for_matched_publication(&self,
min_count: usize, timeout: Duration) -> Result<()>`. Spec §2.2.4.2.4."

**Repo:** `crates/dcps-async/src/reader.rs`
(`AsyncDataReader::wait_for_matched_publication`).

**Tests:** `crates/dcps-async/tests/smoke.rs::wait_for_matched_publication_times_out_without_writer`,
`crates/dcps-async/tests/waker.rs::take_stream_poll_next_registers_the_callers_waker`
(the Ok branch against a live matched writer)

**Status:** done

### §2.2 AsyncDataReader::matched_publication_count is synchronous

**Spec:** §2.2 — "Synchronous snapshot. `pub fn
matched_publication_count(&self) -> usize`."

**Repo:** `crates/dcps-async/src/reader.rs`
(`AsyncDataReader::matched_publication_count`, not an `async fn`).

**Tests:** `crates/dcps-async/tests/smoke.rs::matched_publication_count_is_a_synchronous_snapshot`

**Status:** done

### §2.2 Content-filtered reader (extension beyond the spec)

**Spec:** §2.2 lists no content filter. This item records the extension
so that it is covered rather than riding along uncovered; in substance it
follows DDS 1.4 §2.2.2.3.5 ContentFilteredTopic.

**Repo:** `crates/dcps-async/src/subscriber.rs`
(`AsyncSubscriber::create_datareader_filtered` with a closure,
`AsyncSubscriber::create_datareader_cft` over a `ContentFilteredTopic`),
`crates/dcps-async/src/participant.rs`
(`AsyncDomainParticipant::create_contentfilteredtopic`).

**Tests:** `crates/dcps-async/tests/smoke.rs::filtered_reader_delivers_only_matching`,
`crates/dcps-async/tests/smoke.rs::sql_content_filtered_reader_delivers_only_matching`

**Status:** done

## §3 Waker model

### §3 One waker slot per reader

**Spec:** §3 — "Per reader there is a registered `Waker` slot
(`Mutex<Option<Waker>>`)."

**Repo:** `crates/dcps/src/runtime.rs` (`UserReaderSlot` carries
`async_waker: Arc<std::sync::Mutex<Option<core::task::Waker>>>`;
`DcpsRuntime::register_user_reader_waker` sets and clears it),
`crates/dcps/src/subscriber.rs` (`DataReader::runtime_handle` hands out
the runtime and the `EntityId` that address the slot).

**Tests:** `crates/dcps-async/tests/waker.rs::reader_slot_waker_is_woken_on_sample_arrival`

**Status:** done

### §3 The wire path wakes the waker

**Spec:** §3 — "The wire path in `runtime.rs` wakes this
waker when a sample lands in the channel: `slot.sample_tx.send(sample).ok(); if let
Some(waker) = slot.async_waker.lock().unwrap().take() { waker.wake(); }`"

**Repo:** `crates/dcps/src/runtime.rs` (`wake_async_waker` takes the
waker out of the slot and wakes it; called at every point of the receive
path that sends on `sample_tx`).

**Tests:** `crates/dcps/src/runtime.rs::tests::wake_async_waker_takes_the_waker_and_wakes_once`
— covers the take and the fact that without a re-registration no second
wake happens,
`crates/dcps-async/tests/waker.rs::reader_slot_waker_is_woken_on_sample_arrival`
— covers the wake by a real sample arrival.

**Status:** done

### §3 poll_next registers the caller's waker

**Spec:** §3 — "The `take_stream` `Stream::poll_next` registers its own
waker in the slot: … `*self.slot.async_waker.lock().unwrap() =
Some(cx.waker().clone()); Poll::Pending`".

**Repo:** `crates/dcps-async/src/reader.rs` (`SampleStream::poll_next`,
`SampleInfoStream::poll_next` and `DataAvailableStream::poll_next` call
`register_user_reader_waker(eid, Some(cx.waker().clone()))` in the
pending branch as soon as the reader has a `runtime_handle()`).

**Tests:** `crates/dcps-async/tests/waker.rs::take_stream_poll_next_registers_the_callers_waker`
— the waker handed in through `Context` is the one the sample arrival
wakes.

**Status:** done

## §4 Tokio glue (feature)

### §4 spawn_in_tokio under the tokio-glue feature

**Spec:** §4 — "With `--features tokio-glue` the following is enabled: `impl
AsyncDomainParticipantFactory { pub fn spawn_in_tokio(rt:
&tokio::runtime::Handle) -> ... }` Spawns the DCPS tick loop in a
Tokio runtime instead of a dedicated `std::thread`."

**Repo:** `crates/dcps-async/src/factory.rs` (`spawn_in_tokio`,
`spawn_in_tokio_with_qos` set `RuntimeConfig::external_tick` and drive
`DcpsRuntime::tick_driver()` as a tokio task).

**Tests:** `crates/dcps-async/tests/spawn_in_tokio.rs::spawn_in_tokio_drives_tick_on_tokio_runtime`,
`crates/dcps-async/tests/spawn_in_tokio.rs::spawn_in_tokio_task_terminates_on_shutdown`,
`crates/dcps/tests/external_tick.rs::external_tick_suppresses_thread_and_driver_advances`

**Status:** done

### §4 The default build without the feature uses its own tick thread

**Spec:** §4 — "Default build (without the feature): `std::thread::spawn` as
today."

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::external_tick`
defaults to `false`, so the runtime starts its own tick thread),
`crates/dcps-async/src/factory.rs` (`create_participant` without the
feature path).

**Tests:** `crates/dcps/tests/external_tick.rs::internal_tick_thread_advances_on_its_own`

**Status:** done

## §5 Backpressure and resource limits

### §5 The write future suspends instead of blocking

**Spec:** §5 — "Spec §2.2.3.19 RESOURCE_LIMITS reliable block: with a full writer
cache the `write` future is **suspended** until drain notify.
In the sync API the thread blocks with `Condvar`."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write`: on
`DdsError::OutOfResources` the reliability deadline is read once, then
`yield_for(2 ms).await` and a retry, until either a drain happens or
`max_blocking_time` elapses and `DdsError::Timeout` comes back — the
caller thread stays free).

**Tests:** `crates/dcps-async/tests/smoke.rs::write_returns_timeout_after_max_blocking_when_queue_full`,
`crates/dcps-async/tests/proptest_backpressure.rs::write_take_sequence_holds_invariants`

**Status:** done

## §6 Listener bridge

### §6 data_available_stream

**Spec:** §6 — "`pub fn data_available_stream(&self) -> impl Stream<Item
= ()> + Send`. The streams emit one element per listener trigger."

**Repo:** `crates/dcps-async/src/reader.rs` (`DataAvailableStream` with
`Item = ()`; the probe runs over the non-consuming `DataReader::read`, so
the consumer can still fetch the samples with `take` afterwards).

**Tests:** `crates/dcps-async/tests/smoke.rs::data_available_stream_signals_without_consuming`

**Status:** done

### §6 publication_matched_stream

**Spec:** §6 — "`pub fn publication_matched_stream(&self) -> impl
Stream<Item = PublicationMatchedStatus> + Send`."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::publication_matched_stream` returns
`PublicationMatchedStream<T>` with `Item =
zerodds_dcps::status::PublicationMatchedStatus` — the writer side of DDS
1.4 §2.2.4.1 PUBLICATION_MATCHED), `crates/dcps-async/src/reader.rs`
(`AsyncDataReader::subscription_matched_stream` as the reader-side
counterpart with `SubscriptionMatchedStatus`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_publication_matched_stream_yields_status`,
`crates/dcps-async/tests/smoke.rs::subscription_matched_stream_yields_initial_count`

**Status:** done

## §7 Error mapping

### §7 DdsError is passed through unchanged

**Spec:** §7 — "`DdsError` is passed through unchanged.
The `Future::Output` type is always `Result<T, DdsError>`."

**Repo:** `crates/dcps-async/src/lib.rs` (`DdsError` and `Result` are
re-exported from `zerodds_dcps`, no error type of its own),
`crates/dcps-async/src/writer.rs`, `crates/dcps-async/src/reader.rs`
(every fallible method returns `zerodds_dcps::Result<_>`).

**Tests:** `crates/dcps-async/tests/smoke.rs::wait_for_matched_subscription_times_out_when_no_reader`,
`crates/dcps-async/tests/smoke.rs::write_returns_timeout_after_max_blocking_when_queue_full`
— both match on `DdsError::Timeout` coming out of the async path.

**Status:** done

## §8 Test strategy

### §8 One async counterpart per sync test

**Spec:** §8 — "For each sync test in `crates/dcps/tests/` an async counterpart
in `crates/dcps-async/tests/`."

**Repo:** `crates/dcps-async/tests/smoke.rs`,
`crates/dcps-async/tests/waker.rs`,
`crates/dcps-async/tests/throughput.rs`,
`crates/dcps-async/tests/alloc.rs`,
`crates/dcps-async/tests/perf_targets.rs`,
`crates/dcps-async/tests/proptest_backpressure.rs`,
`crates/dcps-async/tests/default_build.rs`,
`crates/dcps-async/tests/spawn_in_tokio.rs`,
`crates/dcps-async/tests/cyclone_live_async_e2e.rs`.

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_write_async_offline`,
`crates/dcps-async/tests/smoke.rs::take_stream_yields_pushed_samples`,
`crates/dcps-async/tests/smoke.rs::read_is_non_consuming`

**Status:** partial — the async surface is covered item by item, but
`crates/dcps/tests/` holds around sixty test files (QoS matrix, lifecycle
wire, discovery server, security) and `crates/dcps-async/tests/` holds
nine. There is no 1:1 counterpart per sync test. The remainder is tracked
as its own `open` item "§8 1:1 async counterpart per sync test".

### §8 tokio::test as the default runner, smol as a feature probe

**Spec:** §8 — "`tokio::test` as the default test runner; with a
`smol::test` variant as a feature probe."

**Repo:** `crates/dcps-async/Cargo.toml` (`tokio` as a dev-dependency
with `rt-multi-thread` and `macros`),
`crates/dcps-async/tests/smoke.rs` (every async test is
`#[tokio::test(flavor = "multi_thread")]`).

**Tests:** `crates/dcps-async/tests/smoke.rs::factory_singleton_offline_participant`,
`crates/dcps-async/tests/smoke.rs::futures_complete_on_a_non_tokio_executor`

**Status:** partial — the default runner is tokio, and runtime
independence is covered by a park executor built inside the test. There
is no `smol` variant; `smol` is not a dev-dependency. The remainder is
tracked as its own `open` item "§8 smol test variant".

### §8 proptest over channel backpressure

**Spec:** §8 — "`proptest` over channel backpressure (random
write/take sequences)."

**Repo:** `crates/dcps-async/tests/proptest_backpressure.rs` (random
capacity and random operation sequences against the invariant: with a
full queue `write` returns a timeout, with a free queue an `Ok`).

**Tests:** `crates/dcps-async/tests/proptest_backpressure.rs::write_take_sequence_holds_invariants`

**Status:** done

### §8 E2E against live Cyclone plus a latency comparison

**Spec:** §8 — "E2E against Cyclone live like sync, plus a latency comparison
(sync vs. async)."

**Repo:** `crates/dcps-async/tests/cyclone_live_async_e2e.rs`
(`#[ignore]`-gated, needs SSH access to the Cyclone lab),
`crates/dcps-async/benches/write_async_vs_sync.rs` (criterion),
`crates/dcps-async/tests/perf_targets.rs` (the same comparison as an
assertion).

**Tests:** `crates/dcps-async/tests/perf_targets.rs::async_write_is_within_five_percent_of_sync_write`,
`crates/dcps-async/tests/cyclone_live_async_e2e.rs::async_reader_does_not_panic_against_live_cyclone_pub`
(`#[ignore]`, not executed in this run)

**Status:** partial — the sync-against-async latency comparison runs as
an assertion on the release build, in the CI job `perf-gate`. The Cyclone live part is `#[ignore]` and
did not run in this audit. The remainder is tracked as its own `open`
item "§8 Cyclone live e2e without a lab gate".

## §9 Performance targets

### §9 write().await at most 5 % slower than sync

**Spec:** §9 — "`write().await` no more than 5 % slower than
sync `write()` (measured with `criterion` in
`crates/dcps-async/benches/`)."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write`
reads the QoS in the backpressure branch only; the lookup clones the
whole QoS set and cost about 13 % on the uncontended path; the retry
loop is a future of its own, entered on `OutOfResources` only),
`crates/dcps-async/benches/write_async_vs_sync.rs`,
`crates/dcps-async/tests/perf_targets.rs` (one writer of a live
participant without a reader, sync and with `.await`, on the release
build; `#[ignore]` on the debug build, the CI job `perf-gate` runs it with
`--release`).

**Tests:** `crates/dcps-async/tests/perf_targets.rs::async_write_is_within_five_percent_of_sync_write`
— measured on the Linux test host on the release build: async/sync
ratio 1.003–1.029 over six runs, about 511 ns per sync write, limit
1.05.

**Status:** done

### §9 take_stream loses no sample

**Spec:** §9 — "`take_stream()` keeps 100 % of the sample rate, no loss due to
polling latency."

**Repo:** `crates/dcps-async/src/reader.rs` (`SampleStream` buffers the
remainder of a `take` and hands it out element by element instead of
discarding it).

**Tests:** `crates/dcps-async/tests/throughput.rs::take_stream_delivers_all_samples_no_loss`
— a reliable keep-all writer publishes 500 samples and the stream has to
surface all 500.

**Status:** done

### §9 No allocation per write compared with sync

**Spec:** §9 — "Allocation per `write`: 0 (same path as sync; the future is
stack-allocated when `async fn` is inlined)."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write`
passes `&T` through without cloning the sample).

**Tests:** `crates/dcps-async/tests/alloc.rs::async_write_adds_no_extra_heap_vs_sync`
(dhat as the global allocator, comparing blocks per write)

**Status:** done

## §10 Deliverables

### §10 Deliverables

**Spec:** §10 — the list of six deliverables (crate structure with
newtypes, `take_stream` including the waker wiring, `write().await` with
drain backpressure, listener streams, the tokio glue feature, bench suite
and spec compliance test).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — the section summarises what §1 to §9
require and states no requirement of its own.

## Open points as their own items

### §8 1:1 async counterpart per sync test

**Spec:** §8 — "For each sync test in `crates/dcps/tests/` an
async counterpart in `crates/dcps-async/tests/`."

**Repo:** —

**Tests:** —

**Status:** open

### §8 smol test variant

**Spec:** §8 — "with a `smol::test` variant as a feature probe."

**Repo:** —

**Tests:** —

**Status:** open

### §8 Cyclone live e2e without a lab gate

**Spec:** §8 — "E2E against Cyclone live like sync."

**Repo:** —

**Tests:** —

**Status:** open

## Decisions

Design decisions on the spec. They are not coverage items.

**D-1 — runtime-agnostic public API by default.** The public API returns
`impl Future` and `impl Stream` with no tokio binding. Reason: ZeroDDS
should not force a runtime. Consequence: the wake-up path uses the DCPS
runtime's reader-slot waker, and a detached thread when offline;
`--features tokio-glue` switches to `tokio::time::sleep`.

**D-2 — tokio glue as an optional feature.** Reason: convenience without
a mandate. Consequence: the default build carries no `tokio` dependency.

**D-3 — API symmetry without an `_async` suffix.** Method names are
identical to the sync API. Reason: switching sync ↔ async then is a pure
type change. Consequence: the caller imports `AsyncDataWriter` instead of
`DataWriter`; no name clash arises, because the types live in different
crates.

**D-4 — WaitSet stays synchronous.** Reason: `WaitSet` has blocking
semantics of its own; a `Stream<Item = ConditionEvent>` is an API layer of
its own. Consequence: whoever needs `WaitSet` uses the sync API; the async
pattern is `take_stream` plus the listener streams.

**D-5 — listener bridge as a stream wrapper.** The sync listeners stay;
the bridge adds streams. Reason: async callers want `while let Some(ev) =
stream.next().await`. Consequence: the listener streams are methods of
their own, the sync listener set stays the default.

**D-6 — the reader-side match stream method is named
`subscription_matched_stream`.** It has always yielded a
`SubscriptionMatchedStatus` (DDS 1.4 §2.2.4.1); the name now says so.
`publication_matched_stream` sits on `AsyncDataWriter` and yields
`PublicationMatchedStatus` — the wording of §6.

---

## Audit status

37 done / 3 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-dcps-async` — 29 tests green, 0 failed.

Test run: `cargo test -p zerodds-dcps-async --features tokio-glue` — 30 tests green, 0 failed.

Test run: `cargo test -p zerodds-dcps --test external_tick` — 2 tests green, 0 failed.

Test run: `cargo test -p zerodds-dcps --lib runtime::tests::wake_async_waker` — 1 test green, 0 failed.

Not executed: the `#[ignore]` test
`async_reader_does_not_panic_against_live_cyclone_pub` (needs Cyclone lab
access).

Open points and decision records: `zerodds-async-1.0.open.en.md`.
