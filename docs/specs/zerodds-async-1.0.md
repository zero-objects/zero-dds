# `zerodds-async` v1.0 — async-DDS-API-Spec

ZeroDDS Vendor-Spec. Status: **Draft 2026-05-04**, in
`crates/dcps-async` (zu erstellen) implementiert.

## Motivation

Es gibt keine OMG-Spec fuer eine async-DDS-API. Die Sync-API (DDS-DCPS
1.4 §2.2.2) ist Thread-pool-friendly, aber blockt den Caller bei
`wait_for_data` / `wait_for_matched_subscription`. Im Tokio-Oekosystem
bedeutet das einen `spawn_blocking`-Roundtrip pro Call — kostet
Latenz und blockt den Worker-Pool.

Diese Spec definiert eine **runtime-agnostische** async-API als
Newtype-Wrapper um die Sync-API.

## Ziele

- **Runtime-agnostisch** als Public-API: `impl Future`/`impl Stream`-
  Returns ohne harte Tokio-Abhaengigkeit.
- **Strikte API-Symmetrie** zur Sync-API: identische Methoden-Namen,
  identische QoS-Semantik.
- **Performance**: kein zusaetzlicher Allokations-Overhead pro Call;
  ein `tokio::sync::Notify`-aequivalentes Waker-Pattern.
- **Optional Tokio-Glue** als feature-gated Convenience.

## Nicht-Ziele

- Eigene async-Runtime einbauen.
- Sync-API ersetzen — Sync bleibt erste-Klasse.
- `WaitSet` async-konvertieren — `WaitSet` ist Spec-DCPS-Konstrukt;
  async-Version waere `Stream<Item = ConditionEvent>` als separate API.

## Module-Layout

```text
crates/
  dcps-async/                       # neuer Crate
    Cargo.toml                      # default = ["std"], optional "tokio-glue"
    src/
      lib.rs                        # Re-Exports
      writer.rs                     # AsyncDataWriter
      reader.rs                     # AsyncDataReader
      stream.rs                     # SampleStream<T>
      participant.rs                # AsyncDomainParticipant (thin)
      waker.rs                      # Spec §4 — Waker-Wiring
      tokio_glue.rs                 # feature "tokio-glue"
    tests/
      ...
```

## §1 Type-Mapping zur Sync-API

| Sync-Typ | Async-Typ | Wire-Identitaet |
|----------|-----------|-----------------|
| `DomainParticipantFactory` | `AsyncDomainParticipantFactory` | gleiche Singleton |
| `DomainParticipant` | `AsyncDomainParticipant` | newtype um `Arc<DomainParticipantInner>` |
| `Topic<T>` | identisch — Topics sind Daten-Modell, nicht I/O |
| `Publisher` / `Subscriber` | `AsyncPublisher` / `AsyncSubscriber` | newtype |
| `DataWriter<T>` | `AsyncDataWriter<T>` | newtype |
| `DataReader<T>` | `AsyncDataReader<T>` | newtype |

Alle Async-Newtypes sind `Clone` + `Send` + `Sync`. Sie teilen den
internen `Arc<...Inner>` mit der Sync-Variante — der gleiche Writer
kann zur Laufzeit als sync ODER async genutzt werden.

## §2 Methodensignaturen

### §2.1 AsyncDataWriter

```rust
impl<T: DdsType + Send + Sync> AsyncDataWriter<T> {
    /// Spec §2.2.2.4.2.16. Async-Variante: blockt nicht den Caller-
    /// Thread; bei RESOURCE_LIMITS-Pressure wird der Future suspended
    /// bis Drain-Notify.
    pub async fn write(&self, sample: &T) -> Result<()>;

    /// Spec §2.2.2.4.2.6.
    pub async fn register_instance(&self, sample: &T) -> Result<InstanceHandle>;

    /// Spec §2.2.2.4.2.10. Wire-Lifecycle-Marker mit DISPOSED-Bit.
    pub async fn dispose(&self, sample: &T, handle: InstanceHandle) -> Result<()>;

    /// Spec §2.2.2.4.2.7.
    pub async fn unregister_instance(&self, sample: &T, handle: InstanceHandle) -> Result<()>;

    /// Spec §2.2.2.4.2.11. Resolves Ok wenn `min_count` Subscribers
    /// gematcht haben; Err(Timeout) bei timeout.
    pub async fn wait_for_matched_subscription(
        &self,
        min_count: usize,
        timeout: Duration,
    ) -> Result<()>;

    /// Snapshot — non-async-Eigenschaft.
    pub fn matched_subscription_count(&self) -> usize;
}
```

### §2.2 AsyncDataReader

```rust
impl<T: DdsType + Send + Sync> AsyncDataReader<T> {
    /// Take-Stream: liefert Samples in der Reihenfolge ihrer Ankunft.
    /// Stream endet wenn der Reader gedroppt wird.
    pub fn take_stream(&self) -> impl Stream<Item = Sample<T>> + Send;

    /// Single-Sample take. Resolves Ok wenn (a) ein Sample da ist,
    /// (b) ein Lifecycle-Marker da ist. Err(Timeout) bei timeout.
    pub async fn take(&self, timeout: Duration) -> Result<Vec<Sample<T>>>;

    /// Spec §2.2.4.2.4.
    pub async fn wait_for_matched_publication(
        &self,
        min_count: usize,
        timeout: Duration,
    ) -> Result<()>;

    /// Synchroner Snapshot.
    pub fn matched_publication_count(&self) -> usize;
}
```

## §3 Waker-Modell

Pro Reader gibt es einen registrierten `Waker`-Slot
(`Mutex<Option<Waker>>`). Der Wire-Pfad in `runtime.rs` weckt diesen
Waker, wenn ein Sample im Channel landet:

```rust
fn deliver_to_reader_slot(slot: &mut UserReaderSlot, sample: UserSample) {
    slot.sample_tx.send(sample).ok();
    if let Some(waker) = slot.async_waker.lock().unwrap().take() {
        waker.wake();
    }
}
```

Der `take_stream`-`Stream::poll_next` registriert seinen eigenen
Waker im Slot:

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

Damit ist der async-Pfad **runtime-agnostisch** — funktioniert mit
Tokio, async-std, smol, embassy, etc.

## §4 Tokio-Glue (Feature)

Mit `--features tokio-glue` aktiviert sich:

```rust
impl AsyncDomainParticipantFactory {
    /// Spawnt die DCPS-Tick-Loop in einer Tokio-Runtime statt einer
    /// dedizierten std::thread. Spart Threads bei vielen Participants.
    pub fn spawn_in_tokio(rt: &tokio::runtime::Handle) -> ...
}
```

Default-Build (ohne Feature): `std::thread::spawn` wie heute.

## §5 Backpressure & Resource-Limits

Spec §2.2.3.19 RESOURCE_LIMITS Reliable-Block: bei vollem Writer-
Cache **suspendiert** der `write`-Future, bis Drain-Notify. Im
Sync-API blockt der Thread mit `Condvar`. Async-API nutzt einen
`Notify`-aequivalenten Mechanismus:

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

`drain_notify` ist ein internes `tokio::sync::Notify` ODER ein
custom Waker-Pool, abhaengig vom Feature.

## §6 Listener-Bridge (optional)

Spec §2.2.2.4.4 DataReaderListener / §2.2.2.4.4 DataWriterListener
sind sync-Callbacks. Async-Bridge:

```rust
pub fn data_available_stream(&self) -> impl Stream<Item = ()> + Send;
pub fn publication_matched_stream(&self) -> impl Stream<Item = PublicationMatchedStatus> + Send;
```

Streams emittieren ein Element pro Listener-Trigger.

## §7 Error-Mapping

`DdsError` wird unveraendert weitergereicht. `Future::Output`-Type ist
immer `Result<T, DdsError>`.

## §8 Test-Strategie

- Pro Sync-Test in `crates/dcps/tests/` einen async-Pendant in
  `crates/dcps-async/tests/`.
- `tokio::test` als default test runner; mit `smol::test`-Variante
  als feature-Probe.
- `proptest` ueber Channel-Backpressure (zufaellige write/take-
  Sequenzen).
- E2E gegen Cyclone-Live wie Sync, plus Latenz-Vergleich (sync vs.
  async).

## §9 Performance-Targets

- `write().await` nicht mehr als 5 % langsamer als `write()` sync
  (gemessen mit `criterion` in `crates/dcps-async/benches/`).
- `take_stream()` haelt 100 % der Sample-Rate, kein Verlust durch
  Polling-Latenz.
- Allokation pro `write`: 0 (gleicher Pfad wie sync; Future ist
  stack-allocated wenn `async fn` inlined).

## §10 Lieferumfang

- Crate-Struktur + Newtypes + Sync-API-Reuse
- `take_stream` + Waker-Wiring im Reader-Slot
- `write().await` mit Drain-Backpressure
- Listener-Streams
- Tokio-Glue-Feature (`spawn_in_tokio`)
- Bench-Suite + Spec-Compliance-Test

Der aktuelle Implementierungs-Stand pro Punkt steht im Coverage-Audit
`docs/spec-coverage/zerodds-async-1.0.md`.
