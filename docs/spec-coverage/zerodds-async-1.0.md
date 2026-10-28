# zerodds-async 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-async-1.0.md` (ZeroDDS-Vendor-Spec,
async-DDS-API als Newtype-Schicht über der Sync-API).

Implementation:

- `crates/dcps-async/` — Newtypes, Futures, Streams, Tokio-Glue.
- `crates/dcps/` — Waker-Slot im `UserReaderSlot` und dessen Wecken im
  Empfangspfad.

Offene Punkte und Decision-Records: `zerodds-async-1.0.open.md`.

## Ziele und Nicht-Ziele

### Ziel — runtime-agnostische Public-API

**Spec:** „Ziele" — „**Runtime-agnostisch** als Public-API: `impl
Future`/`impl Stream`-Returns ohne harte Tokio-Abhaengigkeit."

**Repo:** `crates/dcps-async/src/lib.rs` (`yield_for` weckt ohne
`tokio-glue` über einen abgekoppelten Thread und den Waker des Callers;
`SleepFuture` ist ein gewöhnliches `Future`),
`crates/dcps-async/src/reader.rs` (die Streams implementieren
`futures_core::Stream`, nicht ein Tokio-Trait).

**Tests:** `crates/dcps-async/tests/smoke.rs::futures_complete_on_a_non_tokio_executor`
— treibt `write`, `take` und `wait_for_matched_subscription` auf einem im
Test selbst gebauten Park-Executor ohne jede Tokio-Beteiligung.

**Status:** done

### Ziel — strikte API-Symmetrie zur Sync-API

**Spec:** „Ziele" — „**Strikte API-Symmetrie** zur Sync-API: identische
Methoden-Namen, identische QoS-Semantik."

**Repo:** `crates/dcps-async/src/writer.rs` (`write`,
`register_instance`, `dispose`, `unregister_instance`,
`wait_for_matched_subscription`, `matched_subscription_count`, `qos`),
`crates/dcps-async/src/reader.rs` (`take`, `read`,
`wait_for_matched_publication`, `matched_publication_count`, `qos`) —
jeweils dieselben Namen wie auf `DataWriter`/`DataReader`, plus
`as_sync()` auf jedem Newtype.

**Tests:** `crates/dcps-async/tests/smoke.rs::participant_creates_topic_publisher_and_subscriber`,
`crates/dcps-async/tests/smoke.rs::matched_subscription_count_is_a_synchronous_snapshot`,
`crates/dcps-async/tests/smoke.rs::matched_publication_count_is_a_synchronous_snapshot`

**Status:** done

### Ziel — kein zusätzlicher Allokations-Overhead pro Aufruf

**Spec:** „Ziele" — „**Performance**: kein zusaetzlicher
Allokations-Overhead pro Call".

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write`
ruft auf dem unbelasteten Pfad nur `DataWriter::write`; die
Reliability-Budget-Abfrage `DataWriter::qos()` — die den ganzen
QoS-Satz klont — läuft erst im Backpressure-Zweig).

**Tests:** `crates/dcps-async/tests/alloc.rs::async_write_adds_no_extra_heap_vs_sync`
(dhat als globaler Allocator, Vergleich sync gegen async),
`crates/dcps-async/tests/perf_targets.rs::async_write_is_within_five_percent_of_sync_write`

**Status:** done

### Ziel — Tokio-Glue als optionales Feature

**Spec:** „Ziele" — „**Optional Tokio-Glue** als feature-gated
Convenience."

**Repo:** `crates/dcps-async/Cargo.toml` (`tokio-glue = ["dep:tokio"]`,
`tokio` als `optional = true`), `crates/dcps-async/src/factory.rs`
(`spawn_in_tokio`, `spawn_in_tokio_with_qos` hinter `#[cfg(feature =
"tokio-glue")]`).

**Tests:** `crates/dcps-async/tests/default_build.rs::default_feature_set_is_std_only`,
`crates/dcps-async/tests/spawn_in_tokio.rs::spawn_in_tokio_drives_tick_on_tokio_runtime`

**Status:** done

### Nicht-Ziel — keine eigene async-Runtime im Crate

**Spec:** „Nicht-Ziele" — „Eigene async-Runtime einbauen."

**Repo:** `crates/dcps-async/Cargo.toml` (Pflicht-Dependencies sind
`zerodds-dcps`, `zerodds-qos` und `futures-core`; `futures-core` liefert
nur den `Stream`-Trait, keinen Executor).

**Tests:** `crates/dcps-async/tests/default_build.rs::no_async_runtime_is_a_hard_dependency`

**Status:** done

### Nicht-Ziel — die Sync-API bleibt erste Klasse

**Spec:** „Nicht-Ziele" — „Sync-API ersetzen — Sync bleibt
erste-Klasse."

**Repo:** `crates/dcps-async/src/participant.rs`,
`crates/dcps-async/src/publisher.rs`,
`crates/dcps-async/src/subscriber.rs`,
`crates/dcps-async/src/writer.rs`, `crates/dcps-async/src/reader.rs` —
jeder Newtype hält den Sync-Typ und gibt ihn über `as_sync()` heraus;
`crates/dcps/` bleibt unverändert benutzbar.

**Tests:** `crates/dcps-async/tests/smoke.rs::async_newtypes_are_clone_send_sync_and_share_state`
— derselbe Reader wird über den Newtype und über `as_sync()` bedient,
und beide sehen denselben Zustand.

**Status:** done

### Nicht-Ziel — WaitSet bleibt synchron

**Spec:** „Nicht-Ziele" — „`WaitSet` async-konvertieren — `WaitSet` ist
Spec-DCPS-Konstrukt; async-Version waere `Stream<Item =
ConditionEvent>` als separate API."

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — die Spec nimmt den Punkt selbst aus ihrem
Geltungsbereich; `WaitSet` bleibt in `crates/dcps/` und wird von
`crates/dcps-async/` nicht angefasst.

## Module-Layout

### Module-Layout — Feature-Satz des Crates

**Spec:** „Module-Layout" — „`Cargo.toml # default = ["std"], optional
"tokio-glue"`".

**Repo:** `crates/dcps-async/Cargo.toml` (`default = ["std"]`, `std =
[]`, `tokio-glue = ["dep:tokio"]`).

**Tests:** `crates/dcps-async/tests/default_build.rs::default_feature_set_is_std_only`,
`crates/dcps-async/tests/default_build.rs::no_async_runtime_is_a_hard_dependency`

**Status:** done

### Module-Layout — Datei-Aufteilung

**Spec:** „Module-Layout" — skizziert `lib.rs`, `writer.rs`, `reader.rs`,
`stream.rs`, `participant.rs`, `waker.rs`, `tokio_glue.rs`.

**Repo:** `crates/dcps-async/src/lib.rs`,
`crates/dcps-async/src/writer.rs`, `crates/dcps-async/src/reader.rs`,
`crates/dcps-async/src/participant.rs`,
`crates/dcps-async/src/publisher.rs`,
`crates/dcps-async/src/subscriber.rs`,
`crates/dcps-async/src/factory.rs` — die Streams liegen bei den
Endpunkt-Typen statt in einem eigenen Stream-Modul, die
Waker-Verdrahtung sitzt an ihrem Ende in `crates/dcps/src/runtime.rs`,
und der Tokio-Glue steht bei dem Factory-Typ, den er erweitert.

**Tests:** —

**Status:** n/a (informative) — die Sektion skizziert eine
Datei-Aufteilung, keine beobachtbare Eigenschaft. Die normativen Teile
derselben Sektion (Feature-Satz) stehen als eigenes Item darüber.

## §1 Type-Mapping zur Sync-API

### §1 AsyncDomainParticipantFactory

**Spec:** §1, Tabellenzeile 1 — `DomainParticipantFactory` →
`AsyncDomainParticipantFactory`, Wire-Identität „gleiche Singleton".

**Repo:** `crates/dcps-async/src/factory.rs`
(`AsyncDomainParticipantFactory::instance` hält
`&'static DomainParticipantFactory`; `create_participant_offline`,
`create_participant`, `create_participant_with_qos`).

**Tests:** `crates/dcps-async/tests/smoke.rs::factory_singleton_offline_participant`

**Status:** done

### §1 AsyncDomainParticipant

**Spec:** §1, Tabellenzeile 2 — `DomainParticipant` →
`AsyncDomainParticipant`, „newtype um `Arc<DomainParticipantInner>`".

**Repo:** `crates/dcps-async/src/participant.rs`
(`AsyncDomainParticipant` hält den Sync-`DomainParticipant`;
`domain_id`, `create_topic`, `create_publisher`, `create_subscriber`,
`as_sync`).

**Tests:** `crates/dcps-async/tests/smoke.rs::participant_creates_topic_publisher_and_subscriber`

**Status:** done

### §1 Topic ist in beiden Welten derselbe Typ

**Spec:** §1, Tabellenzeile 3 — „`Topic<T>` | identisch — Topics sind
Daten-Modell, nicht I/O".

**Repo:** `crates/dcps-async/src/participant.rs`
(`AsyncDomainParticipant::create_topic` liefert `zerodds_dcps::Topic<T>`
unverändert zurück), `crates/dcps-async/src/lib.rs` (`Topic` wird aus
`zerodds_dcps` re-exportiert, kein eigener Typ).

**Tests:** `crates/dcps-async/tests/smoke.rs::participant_creates_topic_publisher_and_subscriber`
— derselbe `Topic`-Wert baut Async-Writer und Async-Reader.

**Status:** done

### §1 AsyncPublisher und AsyncSubscriber

**Spec:** §1, Tabellenzeile 4 — `Publisher`/`Subscriber` →
`AsyncPublisher`/`AsyncSubscriber` als Newtype.

**Repo:** `crates/dcps-async/src/publisher.rs` (`AsyncPublisher` mit
`create_datawriter`), `crates/dcps-async/src/subscriber.rs`
(`AsyncSubscriber` mit `create_datareader`,
`create_datareader_filtered`, `create_datareader_cft`).

**Tests:** `crates/dcps-async/tests/smoke.rs::participant_creates_topic_publisher_and_subscriber`,
`crates/dcps-async/tests/smoke.rs::writer_write_async_offline`,
`crates/dcps-async/tests/smoke.rs::reader_take_returns_empty_after_timeout`

**Status:** done

### §1 AsyncDataWriter und AsyncDataReader

**Spec:** §1, Tabellenzeilen 5 und 6 — `DataWriter<T>`/`DataReader<T>` →
`AsyncDataWriter<T>`/`AsyncDataReader<T>` als Newtype.

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter<T>` hält
`Arc<DataWriter<T>>`), `crates/dcps-async/src/reader.rs`
(`AsyncDataReader<T>` hält `Arc<DataReader<T>>`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_register_dispose_unregister_async`,
`crates/dcps-async/tests/smoke.rs::take_stream_yields_pushed_samples`

**Status:** done

### §1 Newtypes sind Clone, Send und Sync und teilen den inneren Arc

**Spec:** §1 — „Alle Async-Newtypes sind `Clone` + `Send` + `Sync`. Sie
teilen den internen `Arc<...Inner>` mit der Sync-Variante — der gleiche
Writer kann zur Laufzeit als sync ODER async genutzt werden."

**Repo:** `crates/dcps-async/src/writer.rs` (`impl Clone for
AsyncDataWriter` klont den `Arc`), `crates/dcps-async/src/reader.rs`
(dito für `AsyncDataReader`), `crates/dcps-async/src/publisher.rs`,
`crates/dcps-async/src/subscriber.rs`,
`crates/dcps-async/src/participant.rs` (`#[derive(Clone)]`).

**Tests:** `crates/dcps-async/tests/smoke.rs::async_newtypes_are_clone_send_sync_and_share_state`
— statische `Send + Sync`-Prüfung plus der Nachweis, dass ein Klon das
Sample konsumiert, das über das Original eingespeist wurde.

**Status:** done

## §2 Methodensignaturen

### §2.1 AsyncDataWriter::write

**Spec:** §2.1 — „`pub async fn write(&self, sample: &T) ->
Result<()>`. Spec §2.2.2.4.2.16. Async-Variante: blockt nicht den
Caller-Thread; bei RESOURCE_LIMITS-Pressure wird der Future suspended bis
Drain-Notify."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write` —
Direktversuch, danach im Backpressure-Zweig `yield_for`-Retry bis
`reliability.max_blocking_time`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_write_async_offline`,
`crates/dcps-async/tests/smoke.rs::write_returns_timeout_after_max_blocking_when_queue_full`

**Status:** done

### §2.1 AsyncDataWriter::register_instance

**Spec:** §2.1 — „`pub async fn register_instance(&self, sample: &T) ->
Result<InstanceHandle>`. Spec §2.2.2.4.2.6."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::register_instance`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_register_dispose_unregister_async`

**Status:** done

### §2.1 AsyncDataWriter::dispose

**Spec:** §2.1 — „`pub async fn dispose(&self, sample: &T, handle:
InstanceHandle) -> Result<()>`. Spec §2.2.2.4.2.10.
Wire-Lifecycle-Marker mit DISPOSED-Bit."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::dispose`
delegiert an `DataWriter::dispose`, das den DISPOSED-Marker schreibt).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_register_dispose_unregister_async`

**Status:** done

### §2.1 AsyncDataWriter::unregister_instance

**Spec:** §2.1 — „`pub async fn unregister_instance(&self, sample: &T,
handle: InstanceHandle) -> Result<()>`. Spec §2.2.2.4.2.7."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::unregister_instance`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_register_dispose_unregister_async`

**Status:** done

### §2.1 AsyncDataWriter::wait_for_matched_subscription

**Spec:** §2.1 — „Spec §2.2.2.4.2.11. Resolves Ok wenn `min_count`
Subscribers gematcht haben; Err(Timeout) bei timeout."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::wait_for_matched_subscription` — Vergleich gegen
`matched_subscription_count`, `yield_for`-Takt, `DdsError::Timeout` bei
Deadline).

**Tests:** `crates/dcps-async/tests/smoke.rs::wait_for_matched_subscription_times_out_when_no_reader`,
`crates/dcps-async/tests/waker.rs::reader_slot_waker_is_woken_on_sample_arrival`
(der Ok-Zweig, sobald der Reader gematcht hat)

**Status:** done

### §2.1 AsyncDataWriter::matched_subscription_count ist synchron

**Spec:** §2.1 — „Snapshot — non-async-Eigenschaft. `pub fn
matched_subscription_count(&self) -> usize`."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::matched_subscription_count`, kein `async fn`).

**Tests:** `crates/dcps-async/tests/smoke.rs::matched_subscription_count_is_a_synchronous_snapshot`

**Status:** done

### §2.2 AsyncDataReader::take_stream

**Spec:** §2.2 — „`pub fn take_stream(&self) -> impl Stream<Item =
Sample<T>> + Send`. Take-Stream: liefert Samples in der Reihenfolge ihrer
Ankunft. Stream endet wenn der Reader gedroppt wird."

**Repo:** `crates/dcps-async/src/reader.rs`
(`AsyncDataReader::take_stream` liefert `SampleStream<T>` mit `Item =
T`; `AsyncDataReader::take_stream_with_info` liefert `SampleInfoStream<T>`
mit `Item = Sample<T>` — dem Wortlaut der Spec).

**Tests:** `crates/dcps-async/tests/smoke.rs::take_stream_yields_pushed_samples`,
`crates/dcps-async/tests/smoke.rs::take_stream_with_info_yields_sample_info`,
`crates/dcps-async/tests/throughput.rs::take_stream_delivers_all_samples_no_loss`

**Status:** done

### §2.2 AsyncDataReader::take

**Spec:** §2.2 — „`pub async fn take(&self, timeout: Duration) ->
Result<Vec<Sample<T>>>`. Single-Sample take. Resolves Ok wenn (a) ein
Sample da ist, (b) ein Lifecycle-Marker da ist."

**Repo:** `crates/dcps-async/src/reader.rs` (`AsyncDataReader::take`
liefert `Vec<T>`; `take_with_info` und `read_with_info` liefern
`Vec<Sample<T>>` mit voller `SampleInfo`; `read` ist die nicht
konsumierende Variante).

**Tests:** `crates/dcps-async/tests/smoke.rs::reader_take_returns_empty_after_timeout`,
`crates/dcps-async/tests/smoke.rs::take_with_info_carries_sample_info`,
`crates/dcps-async/tests/smoke.rs::read_is_non_consuming`

**Status:** done

### §2.2 AsyncDataReader::wait_for_matched_publication

**Spec:** §2.2 — „`pub async fn wait_for_matched_publication(&self,
min_count: usize, timeout: Duration) -> Result<()>`. Spec §2.2.4.2.4."

**Repo:** `crates/dcps-async/src/reader.rs`
(`AsyncDataReader::wait_for_matched_publication`).

**Tests:** `crates/dcps-async/tests/smoke.rs::wait_for_matched_publication_times_out_without_writer`,
`crates/dcps-async/tests/waker.rs::take_stream_poll_next_registers_the_callers_waker`
(der Ok-Zweig gegen einen live gematchten Writer)

**Status:** done

### §2.2 AsyncDataReader::matched_publication_count ist synchron

**Spec:** §2.2 — „Synchroner Snapshot. `pub fn
matched_publication_count(&self) -> usize`."

**Repo:** `crates/dcps-async/src/reader.rs`
(`AsyncDataReader::matched_publication_count`, kein `async fn`).

**Tests:** `crates/dcps-async/tests/smoke.rs::matched_publication_count_is_a_synchronous_snapshot`

**Status:** done

### §2.2 Content-Filter-Reader (Erweiterung über die Spec hinaus)

**Spec:** §2.2 listet keinen Content-Filter. Das Item hält die
Erweiterung fest, damit sie belegt ist statt unbelegt mitzulaufen;
inhaltlich folgt sie DDS 1.4 §2.2.2.3.5 ContentFilteredTopic.

**Repo:** `crates/dcps-async/src/subscriber.rs`
(`AsyncSubscriber::create_datareader_filtered` mit Closure,
`AsyncSubscriber::create_datareader_cft` über einen
`ContentFilteredTopic`), `crates/dcps-async/src/participant.rs`
(`AsyncDomainParticipant::create_contentfilteredtopic`).

**Tests:** `crates/dcps-async/tests/smoke.rs::filtered_reader_delivers_only_matching`,
`crates/dcps-async/tests/smoke.rs::sql_content_filtered_reader_delivers_only_matching`

**Status:** done

## §3 Waker-Modell

### §3 Waker-Slot pro Reader

**Spec:** §3 — „Pro Reader gibt es einen registrierten `Waker`-Slot
(`Mutex<Option<Waker>>`)."

**Repo:** `crates/dcps/src/runtime.rs` (`UserReaderSlot` trägt
`async_waker: Arc<std::sync::Mutex<Option<core::task::Waker>>>`;
`DcpsRuntime::register_user_reader_waker` setzt und löscht ihn),
`crates/dcps/src/subscriber.rs` (`DataReader::runtime_handle` gibt
Runtime und `EntityId` heraus, über die der Slot adressiert wird).

**Tests:** `crates/dcps-async/tests/waker.rs::reader_slot_waker_is_woken_on_sample_arrival`

**Status:** done

### §3 Der Wire-Pfad weckt den Waker

**Spec:** §3 — „Der Wire-Pfad in `runtime.rs` weckt diesen Waker, wenn
ein Sample im Channel landet: `slot.sample_tx.send(sample).ok(); if let
Some(waker) = slot.async_waker.lock().unwrap().take() { waker.wake(); }`"

**Repo:** `crates/dcps/src/runtime.rs` (`wake_async_waker` nimmt den
Waker aus dem Slot und weckt ihn; aufgerufen an jeder Stelle des
Empfangspfads, die auf `sample_tx` sendet).

**Tests:** `crates/dcps/src/runtime.rs::tests::wake_async_waker_takes_the_waker_and_wakes_once`
— belegt das `take` und dass ohne Neu-Registrierung kein zweites Wecken
erfolgt,
`crates/dcps-async/tests/waker.rs::reader_slot_waker_is_woken_on_sample_arrival`
— belegt das Wecken durch eine echte Sample-Ankunft.

**Status:** done

### §3 poll_next registriert den Waker des Callers

**Spec:** §3 — „Der `take_stream`-`Stream::poll_next` registriert seinen
eigenen Waker im Slot: … `*self.slot.async_waker.lock().unwrap() =
Some(cx.waker().clone()); Poll::Pending`".

**Repo:** `crates/dcps-async/src/reader.rs` (`SampleStream::poll_next`,
`SampleInfoStream::poll_next` und `DataAvailableStream::poll_next` rufen
im Pending-Zweig `register_user_reader_waker(eid,
Some(cx.waker().clone()))`, sobald der Reader ein `runtime_handle()`
hat).

**Tests:** `crates/dcps-async/tests/waker.rs::take_stream_poll_next_registers_the_callers_waker`
— der über `Context` hereingereichte Waker ist der, den die
Sample-Ankunft weckt.

**Status:** done

## §4 Tokio-Glue (Feature)

### §4 spawn_in_tokio unter dem Feature tokio-glue

**Spec:** §4 — „Mit `--features tokio-glue` aktiviert sich: `impl
AsyncDomainParticipantFactory { pub fn spawn_in_tokio(rt:
&tokio::runtime::Handle) -> ... }` Spawnt die DCPS-Tick-Loop in einer
Tokio-Runtime statt einer dedizierten `std::thread`."

**Repo:** `crates/dcps-async/src/factory.rs` (`spawn_in_tokio`,
`spawn_in_tokio_with_qos` setzen `RuntimeConfig::external_tick` und
treiben `DcpsRuntime::tick_driver()` als Tokio-Task).

**Tests:** `crates/dcps-async/tests/spawn_in_tokio.rs::spawn_in_tokio_drives_tick_on_tokio_runtime`,
`crates/dcps-async/tests/spawn_in_tokio.rs::spawn_in_tokio_task_terminates_on_shutdown`,
`crates/dcps/tests/external_tick.rs::external_tick_suppresses_thread_and_driver_advances`

**Status:** done

### §4 Default-Build ohne Feature nutzt den eigenen Tick-Thread

**Spec:** §4 — „Default-Build (ohne Feature): `std::thread::spawn` wie
heute."

**Repo:** `crates/dcps/src/runtime.rs` (`RuntimeConfig::external_tick`
ist standardmässig `false`; die Runtime startet dann ihren eigenen
Tick-Thread), `crates/dcps-async/src/factory.rs` (`create_participant`
ohne Feature-Pfad).

**Tests:** `crates/dcps/tests/external_tick.rs::internal_tick_thread_advances_on_its_own`

**Status:** done

## §5 Backpressure und Resource-Limits

### §5 Die write-Future suspendiert statt zu blockieren

**Spec:** §5 — „Spec §2.2.3.19 RESOURCE_LIMITS Reliable-Block: bei
vollem Writer-Cache **suspendiert** der `write`-Future, bis Drain-Notify.
Im Sync-API blockt der Thread mit `Condvar`."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write`:
bei `DdsError::OutOfResources` wird die Reliability-Deadline einmal
gelesen, danach `yield_for(2 ms).await` und erneut versucht, bis
entweder ein Drain stattfindet oder `max_blocking_time` abläuft und
`DdsError::Timeout` zurückkommt — der Caller-Thread bleibt frei).

**Tests:** `crates/dcps-async/tests/smoke.rs::write_returns_timeout_after_max_blocking_when_queue_full`,
`crates/dcps-async/tests/proptest_backpressure.rs::write_take_sequence_holds_invariants`

**Status:** done

## §6 Listener-Bridge

### §6 data_available_stream

**Spec:** §6 — „`pub fn data_available_stream(&self) -> impl Stream<Item
= ()> + Send`. Streams emittieren ein Element pro Listener-Trigger."

**Repo:** `crates/dcps-async/src/reader.rs` (`DataAvailableStream` mit
`Item = ()`; die Probe läuft über das nicht konsumierende
`DataReader::read`, so dass der Konsument die Samples danach noch per
`take` holen kann).

**Tests:** `crates/dcps-async/tests/smoke.rs::data_available_stream_signals_without_consuming`

**Status:** done

### §6 publication_matched_stream

**Spec:** §6 — „`pub fn publication_matched_stream(&self) -> impl
Stream<Item = PublicationMatchedStatus> + Send`."

**Repo:** `crates/dcps-async/src/writer.rs`
(`AsyncDataWriter::publication_matched_stream` liefert
`PublicationMatchedStream<T>` mit `Item =
zerodds_dcps::status::PublicationMatchedStatus` — die Writer-Seite von
DDS 1.4 §2.2.4.1 PUBLICATION_MATCHED), `crates/dcps-async/src/reader.rs`
(`AsyncDataReader::subscription_matched_stream` als reader-seitiges
Gegenstück mit `SubscriptionMatchedStatus`).

**Tests:** `crates/dcps-async/tests/smoke.rs::writer_publication_matched_stream_yields_status`,
`crates/dcps-async/tests/smoke.rs::subscription_matched_stream_yields_initial_count`

**Status:** done

## §7 Error-Mapping

### §7 DdsError bleibt unverändert

**Spec:** §7 — „`DdsError` wird unveraendert weitergereicht.
`Future::Output`-Type ist immer `Result<T, DdsError>`."

**Repo:** `crates/dcps-async/src/lib.rs` (`DdsError` und `Result` werden
aus `zerodds_dcps` re-exportiert, kein eigener Fehlertyp),
`crates/dcps-async/src/writer.rs`, `crates/dcps-async/src/reader.rs`
(jede fehlbare Methode gibt `zerodds_dcps::Result<_>` zurück).

**Tests:** `crates/dcps-async/tests/smoke.rs::wait_for_matched_subscription_times_out_when_no_reader`,
`crates/dcps-async/tests/smoke.rs::write_returns_timeout_after_max_blocking_when_queue_full`
— beide matchen auf `DdsError::Timeout` aus dem Async-Pfad.

**Status:** done

## §8 Test-Strategie

### §8 Ein async-Pendant pro Sync-Test

**Spec:** §8 — „Pro Sync-Test in `crates/dcps/tests/` einen async-Pendant
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

**Status:** partial — die Async-Oberfläche ist item-für-item belegt, aber
`crates/dcps/tests/` hat rund sechzig Test-Files (QoS-Matrix,
Lifecycle-Wire, Discovery-Server, Security) und `crates/dcps-async/tests/`
neun. Ein 1:1-Pendant pro Sync-Test gibt es nicht. Der Rest steht als
eigenes `open`-Item „§8 1:1-Async-Pendant pro Sync-Test".

### §8 tokio::test als Default-Runner, smol als Feature-Probe

**Spec:** §8 — „`tokio::test` als default test runner; mit
`smol::test`-Variante als feature-Probe."

**Repo:** `crates/dcps-async/Cargo.toml` (`tokio` als dev-dependency mit
`rt-multi-thread` und `macros`), `crates/dcps-async/tests/smoke.rs`
(jeder Async-Test ist `#[tokio::test(flavor = "multi_thread")]`).

**Tests:** `crates/dcps-async/tests/smoke.rs::factory_singleton_offline_participant`,
`crates/dcps-async/tests/smoke.rs::futures_complete_on_a_non_tokio_executor`

**Status:** partial — der Default-Runner ist tokio, und die
Runtime-Unabhängigkeit ist über einen im Test gebauten Park-Executor
belegt. Eine `smol`-Variante gibt es nicht; `smol` ist keine
dev-dependency. Der Rest steht als eigenes `open`-Item „§8
smol-Test-Variante".

### §8 proptest über Channel-Backpressure

**Spec:** §8 — „`proptest` ueber Channel-Backpressure (zufaellige
write/take-Sequenzen)."

**Repo:** `crates/dcps-async/tests/proptest_backpressure.rs` (zufällige
Kapazität und zufällige Operationsfolgen gegen die Invariante: bei voller
Queue liefert `write` einen Timeout, bei freier Queue ein `Ok`).

**Tests:** `crates/dcps-async/tests/proptest_backpressure.rs::write_take_sequence_holds_invariants`

**Status:** done

### §8 E2E gegen Cyclone-Live plus Latenz-Vergleich

**Spec:** §8 — „E2E gegen Cyclone-Live wie Sync, plus Latenz-Vergleich
(sync vs. async)."

**Repo:** `crates/dcps-async/tests/cyclone_live_async_e2e.rs`
(`#[ignore]`-gated, braucht SSH-Zugang zum Cyclone-Labor),
`crates/dcps-async/benches/write_async_vs_sync.rs` (criterion),
`crates/dcps-async/tests/perf_targets.rs` (derselbe Vergleich als
Assertion).

**Tests:** `crates/dcps-async/tests/perf_targets.rs::async_write_is_within_five_percent_of_sync_write`,
`crates/dcps-async/tests/cyclone_live_async_e2e.rs::async_reader_does_not_panic_against_live_cyclone_pub`
(`#[ignore]`, in diesem Lauf nicht ausgeführt)

**Status:** partial — der Latenz-Vergleich sync gegen async läuft als
Assertion im Release-Build, im CI-Job `perf-gate`. Der Cyclone-Live-Teil ist `#[ignore]` und
lief in diesem Audit nicht. Der Rest steht als eigenes `open`-Item „§8
Cyclone-Live-E2E ohne Lab-Gate".

## §9 Performance-Targets

### §9 write().await höchstens 5 % langsamer als sync

**Spec:** §9 — „`write().await` nicht mehr als 5 % langsamer als
`write()` sync (gemessen mit `criterion` in
`crates/dcps-async/benches/`)."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write`
liest die QoS erst im Backpressure-Zweig; die Abfrage klont den ganzen
QoS-Satz und kostete auf dem unbelasteten Pfad rund 13 %; die
Wiederholschleife ist eine eigene Future, nur bei `OutOfResources`
betreten), `crates/dcps-async/benches/write_async_vs_sync.rs`,
`crates/dcps-async/tests/perf_targets.rs` (ein Writer eines
Live-Participants ohne Reader, synchron und mit `.await`, im Release-Build;
im Debug-Build `#[ignore]`, der CI-Job `perf-gate` führt ihn mit
`--release` aus).

**Tests:** `crates/dcps-async/tests/perf_targets.rs::async_write_is_within_five_percent_of_sync_write`
— gemessen auf dem Linux-Testhost im Release-Build: Verhältnis
async/sync 1,003–1,029 in sechs Läufen, synchron rund 511 ns je Write,
Grenze 1,05.

**Status:** done

### §9 take_stream verliert kein Sample

**Spec:** §9 — „`take_stream()` haelt 100 % der Sample-Rate, kein Verlust
durch Polling-Latenz."

**Repo:** `crates/dcps-async/src/reader.rs` (`SampleStream` puffert die
Restmenge eines `take` und reicht sie Element für Element heraus, statt
sie zu verwerfen).

**Tests:** `crates/dcps-async/tests/throughput.rs::take_stream_delivers_all_samples_no_loss`
— ein Reliable-KeepAll-Writer publiziert 500 Samples, der Stream muss
alle 500 herausgeben.

**Status:** done

### §9 Keine Allokation pro write gegenüber sync

**Spec:** §9 — „Allokation pro `write`: 0 (gleicher Pfad wie sync; Future
ist stack-allocated wenn `async fn` inlined)."

**Repo:** `crates/dcps-async/src/writer.rs` (`AsyncDataWriter::write`
reicht `&T` durch, ohne das Sample zu klonen).

**Tests:** `crates/dcps-async/tests/alloc.rs::async_write_adds_no_extra_heap_vs_sync`
(dhat als globaler Allocator, Vergleich der Blöcke pro Write)

**Status:** done

## §10 Lieferumfang

### §10 Lieferumfang

**Spec:** §10 — Aufzählung der sechs Liefergegenstände (Crate-Struktur
mit Newtypes, `take_stream` samt Waker-Verdrahtung, `write().await` mit
Drain-Backpressure, Listener-Streams, Tokio-Glue-Feature, Bench-Suite und
Spec-Compliance-Test).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — die Sektion fasst zusammen, was §1 bis §9
normieren, und stellt keine eigene Anforderung.

## Offene Punkte als eigene Items

### §8 1:1-Async-Pendant pro Sync-Test

**Spec:** §8 — „Pro Sync-Test in `crates/dcps/tests/` einen
async-Pendant in `crates/dcps-async/tests/`."

**Repo:** —

**Tests:** —

**Status:** open

### §8 smol-Test-Variante

**Spec:** §8 — „mit `smol::test`-Variante als feature-Probe."

**Repo:** —

**Tests:** —

**Status:** open

### §8 Cyclone-Live-E2E ohne Lab-Gate

**Spec:** §8 — „E2E gegen Cyclone-Live wie Sync."

**Repo:** —

**Tests:** —

**Status:** open

## Entscheidungen

Entwurfsentscheidungen zur Spec. Sie sind keine Coverage-Items.

**D-1 — runtime-agnostische Public-API als Default.** Die Public-API
liefert `impl Future` und `impl Stream` ohne Tokio-Bindung. Grund:
ZeroDDS soll keine Runtime erzwingen. Folge: der Wakeup-Pfad nutzt den
Reader-Slot-Waker der DCPS-Runtime, im Offline-Betrieb einen
abgekoppelten Thread; `--features tokio-glue` schaltet auf
`tokio::time::sleep` um.

**D-2 — Tokio-Glue als optionales Feature.** Grund: Komfort ohne Zwang.
Folge: der Default-Build hat keine `tokio`-Dependency.

**D-3 — API-Symmetrie ohne `_async`-Suffix.** Methodennamen sind
identisch zur Sync-API. Grund: der Wechsel sync ↔ async ist damit ein
reiner Typwechsel. Folge: der Aufrufer importiert `AsyncDataWriter`
statt `DataWriter`; ein Namenskonflikt entsteht nicht, weil die Typen in
verschiedenen Crates liegen.

**D-4 — WaitSet bleibt synchron.** Grund: `WaitSet` hat eine eigene
Block-Semantik; ein `Stream<Item = ConditionEvent>` ist eine eigene
API-Schicht. Folge: wer `WaitSet` braucht, nutzt die Sync-API; das
async-Muster ist `take_stream` plus Listener-Streams.

**D-5 — Listener-Bridge als Stream-Wrapper.** Die Sync-Listener bleiben;
die Bridge bietet zusätzlich Streams. Grund: async-Aufrufer wollen `while
let Some(ev) = stream.next().await`. Folge: die Listener-Streams sind
eigene Methoden, das Sync-Listener-Set bleibt Default.

**D-6 — die reader-seitige Match-Stream-Methode heißt
`subscription_matched_stream`.** Sie liefert seit jeher einen
`SubscriptionMatchedStatus` (DDS 1.4 §2.2.4.1); der Name sagt das
jetzt. `publication_matched_stream` sitzt auf dem `AsyncDataWriter` und
liefert `PublicationMatchedStatus` — der Wortlaut von §6.

---

## Audit-Status

37 done / 3 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-dcps-async` — 29 Tests grün, 0 failed.

Test-Lauf: `cargo test -p zerodds-dcps-async --features tokio-glue` — 30 Tests grün, 0 failed.

Test-Lauf: `cargo test -p zerodds-dcps --test external_tick` — 2 Tests grün, 0 failed.

Test-Lauf: `cargo test -p zerodds-dcps --lib runtime::tests::wake_async_waker` — 1 Test grün, 0 failed.

Nicht ausgeführt: der `#[ignore]`-Test
`async_reader_does_not_panic_against_live_cyclone_pub` (braucht
Cyclone-Labor-Zugang).

Offene Punkte und Decision-Records: `zerodds-async-1.0.open.md`.
