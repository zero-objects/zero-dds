# `zerodds-async` v1.0 — async DDS API spec

ZeroDDS vendor spec. Status: **Draft 2026-05-04**, implemented in
`crates/dcps-async` (to be created).

## Motivation

There is no OMG spec for an async DDS API. The sync API (DDS-DCPS
1.4 §2.2.2) is thread-pool-friendly, but blocks the caller on
`wait_for_data` / `wait_for_matched_subscription`. In the Tokio ecosystem
this means a `spawn_blocking` round trip per call — it costs
latency and blocks the worker pool.

This spec defines a **runtime-agnostic** async API as a
newtype wrapper around the sync API.

## Goals

- **Runtime-agnostic** as a public API: `impl Future`/`impl Stream`
  returns without a hard Tokio dependency.
- **Strict API symmetry** with the sync API: identical method names,
  identical QoS semantics.
- **Performance**: no additional allocation overhead per call;
  a waker pattern equivalent to `tokio::sync::Notify`.
- **Optional Tokio glue** as a feature-gated convenience.

## Non-goals

- Building in our own async runtime.
- Replacing the sync API — sync stays first-class.
- Converting `WaitSet` to async — `WaitSet` is a spec DCPS construct;
  an async version would be `Stream<Item = ConditionEvent>` as a separate API.

## Module layout

```text
crates/
  dcps-async/                       # new crate
    Cargo.toml                      # default = ["std"], optional "tokio-glue"
    src/
      lib.rs                        # re-exports
      writer.rs                     # AsyncDataWriter
      reader.rs                     # AsyncDataReader
      stream.rs                     # SampleStream<T>
      participant.rs                # AsyncDomainParticipant (thin)
      waker.rs                      # Spec §4 — waker wiring
      tokio_glue.rs                 # feature "tokio-glue"
    tests/
      ...
```

## §1 Type mapping to the sync API

| Sync type | Async type | Wire identity |
|----------|-----------|-----------------|
| `DomainParticipantFactory` | `AsyncDomainParticipantFactory` | same singleton |
| `DomainParticipant` | `AsyncDomainParticipant` | newtype around `Arc<DomainParticipantInner>` |
| `Topic<T>` | identical — topics are data model, not I/O |
| `Publisher` / `Subscriber` | `AsyncPublisher` / `AsyncSubscriber` | newtype |
| `DataWriter<T>` | `AsyncDataWriter<T>` | newtype |
| `DataReader<T>` | `AsyncDataReader<T>` | newtype |

All async newtypes are `Clone` + `Send` + `Sync`. They share the
internal `Arc<...Inner>` with the sync variant — the same writer
can be used at runtime as sync OR async.

## §2 Method signatures

### §2.1 AsyncDataWriter

```rust
impl<T: DdsType + Send + Sync> AsyncDataWriter<T> {
    /// Spec §2.2.2.4.2.16. Async variant: does not block the caller
    /// thread; under RESOURCE_LIMITS pressure the future is suspended
    /// until drain notify.
    pub async fn write(&self, sample: &T) -> Result<()>;

    /// Spec §2.2.2.4.2.6.
    pub async fn register_instance(&self, sample: &T) -> Result<InstanceHandle>;

    /// Spec §2.2.2.4.2.10. Wire lifecycle marker with DISPOSED bit.
    pub async fn dispose(&self, sample: &T, handle: InstanceHandle) -> Result<()>;

    /// Spec §2.2.2.4.2.7.
    pub async fn unregister_instance(&self, sample: &T, handle: InstanceHandle) -> Result<()>;

    /// Spec §2.2.2.4.2.11. Resolves Ok when `min_count` subscribers
    /// have matched; Err(Timeout) on timeout.
    pub async fn wait_for_matched_subscription(
        &self,
        min_count: usize,
        timeout: Duration,
    ) -> Result<()>;

    /// Snapshot — non-async property.
    pub fn matched_subscription_count(&self) -> usize;
}
```

### §2.2 AsyncDataReader

```rust
impl<T: DdsType + Send + Sync> AsyncDataReader<T> {
    /// Take stream: delivers samples in the order of their arrival.
    /// The stream ends when the reader is dropped.
    pub fn take_stream(&self) -> impl Stream<Item = Sample<T>> + Send;

    /// Single-sample take. Resolves Ok when (a) a sample is there,
    /// (b) a lifecycle marker is there. Err(Timeout) on timeout.
    pub async fn take(&self, timeout: Duration) -> Result<Vec<Sample<T>>>;

    /// Spec §2.2.4.2.4.
    pub async fn wait_for_matched_publication(
        &self,
        min_count: usize,
        timeout: Duration,
    ) -> Result<()>;

    /// Synchronous snapshot.
    pub fn matched_publication_count(&self) -> usize;
}
```

## §3 Waker model

Per reader there is a registered `Waker` slot
(`Mutex<Option<Waker>>`). The wire path in `runtime.rs` wakes this
waker when a sample lands in the channel:

```rust
fn deliver_to_reader_slot(slot: &mut UserReaderSlot, sample: UserSample) {
    slot.sample_tx.send(sample).ok();
    if let Some(waker) = slot.async_waker.lock().unwrap().take() {
        waker.wake();
    }
}
```

The `take_stream` `Stream::poll_next` registers its own
waker in the slot:

```rust
fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<...> {
    if let Some(item) = self.try_recv() {
        Poll::Ready(Some(item))
    } else {
        *self.slot.async_waker.lock().unwrap() = Some(cx.waker().clone());
        Poll::Pending
    }
}
```

This makes the async path **runtime-agnostic** — it works with
Tokio, async-std, smol, embassy, etc.

## §4 Tokio glue (feature)

With `--features tokio-glue` the following is enabled:

```rust
impl AsyncDomainParticipantFactory {
    /// Spawns the DCPS tick loop in a Tokio runtime instead of a
    /// dedicated std::thread. Saves threads with many participants.
    pub fn spawn_in_tokio(rt: &tokio::runtime::Handle) -> ...
}
```

Default build (without the feature): `std::thread::spawn` as today.

## §5 Backpressure & resource limits

Spec §2.2.3.19 RESOURCE_LIMITS reliable block: with a full writer
cache the `write` future is **suspended** until drain notify. In the
sync API the thread blocks with `Condvar`. The async API uses a
mechanism equivalent to `Notify`:

```rust
pub async fn write(&self, sample: &T) -> Result<()> {
    loop {
        match self.try_write_nonblocking(sample) {
            Ok(()) => return Ok(()),
            Err(DdsError::OutOfResources { .. }) => {
                self.drain_notify.notified().await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}
```

`drain_notify` is an internal `tokio::sync::Notify` OR a
custom waker pool, depending on the feature.

## §6 Listener bridge (optional)

Spec §2.2.2.4.4 DataReaderListener / §2.2.2.4.4 DataWriterListener
are sync callbacks. Async bridge:

```rust
pub fn data_available_stream(&self) -> impl Stream<Item = ()> + Send;
pub fn publication_matched_stream(&self) -> impl Stream<Item = PublicationMatchedStatus> + Send;
```

The streams emit one element per listener trigger.

## §7 Error mapping

`DdsError` is passed through unchanged. The `Future::Output` type is
always `Result<T, DdsError>`.

## §8 Test strategy

- For each sync test in `crates/dcps/tests/` an async counterpart in
  `crates/dcps-async/tests/`.
- `tokio::test` as the default test runner; with a `smol::test` variant
  as a feature probe.
- `proptest` over channel backpressure (random write/take
  sequences).
- E2E against Cyclone live like sync, plus a latency comparison (sync vs.
  async).

## §9 Performance targets

- `write().await` no more than 5 % slower than sync `write()`
  (measured with `criterion` in `crates/dcps-async/benches/`).
- `take_stream()` keeps 100 % of the sample rate, no loss due to
  polling latency.
- Allocation per `write`: 0 (same path as sync; the future is
  stack-allocated when `async fn` is inlined).

## §10 Scope of delivery

- Crate structure + newtypes + sync API reuse
- `take_stream` + waker wiring in the reader slot
- `write().await` with drain backpressure
- Listener streams
- Tokio glue feature (`spawn_in_tokio`)
- Bench suite + spec compliance test

The current implementation state per item is in the coverage audit
`docs/spec-coverage/zerodds-async-1.0.md`.
