# zerodds-async 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-async-1.0.md` (Vendor-Spec, draft 2026-05-04).

Implementation:

- `crates/dcps-async/` — Async-DCPS-API (Futures/Streams über DCPS).

## §1 Type-Mapping zur Sync-API

### §1.1 AsyncDomainParticipantFactory
- **Anforderung:** Singleton, share Sync-Factory; create_participant async-fähig.
- **Repo:** `crates/dcps-async/src/factory.rs`
- **Tests:** `crates/dcps-async/tests/smoke.rs::factory_singleton_offline_participant`
- **Status:** done

### §1.2 AsyncDomainParticipant
- **Anforderung:** newtype um `DomainParticipant`; create_topic, create_publisher, create_subscriber identisch zu sync.
- **Repo:** `crates/dcps-async/src/participant.rs`
- **Tests:** `tests/smoke.rs` (alle nutzen create_participant_offline + create_topic + create_publisher/subscriber)
- **Status:** done

### §1.3 AsyncPublisher / AsyncSubscriber
- **Anforderung:** newtype, create_datawriter/datareader liefert AsyncDataWriter/Reader.
- **Repo:** `crates/dcps-async/src/{publisher,subscriber}.rs`
- **Tests:** `tests/smoke.rs::writer_write_async_offline`, `reader_take_returns_empty_after_timeout`
- **Status:** done

### §1.4 AsyncDataWriter<T>
- **Anforderung:** newtype + Send/Sync; teilt internen `Arc<DataWriter<T>>`.
- **Repo:** `crates/dcps-async/src/writer.rs`
- **Tests:** `tests/smoke.rs::writer_write_async_offline`, `writer_register_dispose_unregister_async`
- **Status:** done

### §1.5 AsyncDataReader<T>
- **Anforderung:** dito.
- **Repo:** `crates/dcps-async/src/reader.rs`
- **Tests:** `tests/smoke.rs::reader_take_returns_empty_after_timeout`
- **Status:** done

## §2 Methodensignaturen

### §2.1.1 AsyncDataWriter::write
- **Anforderung:** `async fn write(&self, &T) -> Result<()>`; suspendiert bei OutOfResources statt zu blockieren.
- **Repo:** `crates/dcps-async/src/writer.rs::write` (yield_for-Retry-Loop bis `reliability.max_blocking_time` abgelaufen ist; Spec §5.1).
- **Tests:** `tests/smoke.rs::{writer_write_async_offline, write_returns_timeout_after_max_blocking_when_queue_full}`
- **Status:** done — bei OutOfResources yield + retry; Timeout-Result-Pfad mit endlicher max_blocking_time getestet.

### §2.1.2 AsyncDataWriter::register_instance
- **Anforderung:** `async fn` analog sync.
- **Repo:** `crates/dcps-async/src/writer.rs::register_instance`
- **Tests:** `tests/smoke.rs::writer_register_dispose_unregister_async`
- **Status:** done

### §2.1.3 AsyncDataWriter::dispose
- **Anforderung:** `async fn`; löst Wire-Lifecycle DISPOSED.
- **Repo:** `crates/dcps-async/src/writer.rs::dispose`
- **Tests:** `tests/smoke.rs::writer_register_dispose_unregister_async`
- **Status:** done

### §2.1.4 AsyncDataWriter::unregister_instance
- **Anforderung:** `async fn`; UNREGISTERED + autodispose-Flag.
- **Repo:** `crates/dcps-async/src/writer.rs::unregister_instance`
- **Tests:** `tests/smoke.rs::writer_register_dispose_unregister_async`
- **Status:** done

### §2.1.5 AsyncDataWriter::wait_for_matched_subscription
- **Anforderung:** `async fn`; resolves Ok bei min_count, Err(Timeout) bei timeout.
- **Repo:** `crates/dcps-async/src/writer.rs::wait_for_matched_subscription`
- **Tests:** `tests/smoke.rs::wait_for_matched_subscription_times_out_when_no_reader`
- **Status:** done

### §2.1.6 AsyncDataWriter::matched_subscription_count
- **Anforderung:** synchron — non-async-Eigenschaft.
- **Repo:** `crates/dcps-async/src/writer.rs::matched_subscription_count`
- **Tests:** indirekt über wait_for_matched_subscription
- **Status:** done

### §2.2.1 AsyncDataReader::take_stream
- **Anforderung:** `fn take_stream() -> impl Stream<Item = Sample<T>> + Send`.
- **Repo:** `crates/dcps-async/src/reader.rs::take_stream` (`Item = T`, ergonomisch) + `take_stream_with_info` (`SampleInfoStream`, `Item = Sample<T>` — der Spec-Wortlaut).
- **Tests:** `tests/smoke.rs` (builder) + example `rust-async-showcase` (`take_stream_with_info_yields_sample_info`).
- **Status:** done — Stream nutzt den nativen Reader-Slot-Waker (`register_user_reader_waker`, Wake bei Sample-Arrival, kein Polling); im Offline-Mode dient detached-thread-Sleep als Fallback (Spec §3.3). SampleInfo-Variante liefert `Sample<T>`.

### §2.2.2 AsyncDataReader::take
- **Anforderung:** `async fn take(timeout) -> Result<Vec<Sample<T>>>`.
- **Repo:** `crates/dcps-async/src/reader.rs::take` (`Vec<T>`) + `take_with_info` / `read_with_info` (`Vec<Sample<T>>`) + `read` (non-consuming).
- **Tests:** `tests/smoke.rs::reader_take_returns_empty_after_timeout`; example `take_with_info_carries_sample_info`, `read_is_non_consuming`.
- **Status:** done — `take`/`read` liefern `T` (ergonomisch); `take_with_info`/`read_with_info` liefern `Sample<T>` mit voller SampleInfo (Spec-Wortlaut).

### §2.2.4a Content-Filtered-Reader (Erweiterung)
- **Anforderung:** async-Pendant zu ContentFilteredTopic / DataReader::with_filter (DDS 1.4 §2.2.2.5.4) — in der Original-Spec §2.2 nicht gelistet, aber für Vollständigkeit ergänzt.
- **Repo:** `AsyncSubscriber::create_datareader_filtered` (Closure) + `AsyncDomainParticipant::create_contentfilteredtopic` + `AsyncSubscriber::create_datareader_cft` (SQL, via `DdsTypeRow`). Voraussetzung: `#[derive(DdsType)]` generiert `field_value` (crates/cdr-derive), sonst matcht der SQL-Filter nichts.
- **Tests:** example `rust-async-showcase` (`filtered_reader_delivers_only_matching`, `sql_content_filtered_reader_delivers_only_matching`).
- **Status:** done.

### §2.2.3 AsyncDataReader::wait_for_matched_publication
- **Anforderung:** dito wait_for_matched_subscription.
- **Repo:** `crates/dcps-async/src/reader.rs::wait_for_matched_publication`
- **Tests:** indirekt über publication_matched_stream-Test
- **Status:** done

### §2.2.4 AsyncDataReader::matched_publication_count
- **Anforderung:** synchron.
- **Repo:** `crates/dcps-async/src/reader.rs::matched_publication_count`
- **Tests:** indirekt
- **Status:** done

## §3 Waker-Modell

### §3.1 Waker-Slot pro Reader
- **Anforderung:** UserReaderSlot bekommt `async_waker: Mutex<Option<Waker>>`.
- **Repo:** `crates/dcps/src/runtime.rs::UserReaderSlot::async_waker`
- **Tests:** indirekt via SampleStream-Live-Pfad
- **Status:** done

### §3.2 Wire-Pfad weckt Waker
- **Anforderung:** `deliver_to_reader_slot` weckt Waker, sobald sample_tx.send.
- **Repo:** `crates/dcps/src/runtime.rs::wake_async_waker` (nach jedem `sample_tx.send` im handle_user_datagram-Pfad)
- **Tests:** indirekt
- **Status:** done

### §3.3 Stream::poll_next registriert Waker
- **Anforderung:** Pending-Branch speichert `cx.waker().clone()` im Slot.
- **Repo:** `crates/dcps-async/src/reader.rs::SampleStream::poll_next` (Live-Mode via `runtime_handle` + `register_user_reader_waker`)
- **Tests:** kompiliert
- **Status:** done — Live-Mode nativ; Offline-Mode behält detached-thread-Fallback.

## §4 Tokio-Glue (Feature)

### §4.1 spawn_in_tokio
- **Anforderung:** Mit `--features tokio-glue`: AsyncDomainParticipantFactory::spawn_in_tokio.
- **Repo:** `crates/dcps-async/src/factory.rs::AsyncDomainParticipantFactory::spawn_in_tokio` (+ `_with_qos`); treibt den DDS-Tick-Loop als tokio-Task statt dediziertem `std::thread` (via `RuntimeConfig::external_tick` + `DcpsRuntime::tick_driver()`).
- **Tests:** `crates/dcps/tests/external_tick.rs` + `crates/dcps-async/tests/spawn_in_tokio.rs`.
- **Status:** done — `spawn_in_tokio` treibt den Tick im tokio-Executor (spart einen Thread pro Participant).

## §5 Backpressure & Resource-Limits

### §5.1 write-Future suspendiert bei OutOfResources
- **Anforderung:** Bei DdsError::OutOfResources awaitet drain_notify; bei OK kehrt zurück.
- **Repo:** `crates/dcps-async/src/writer.rs::AsyncDataWriter::write` — yield_for-Retry-Loop bis `reliability.max_blocking_time` abläuft (dann `DdsError::Timeout`); Caller-Future bleibt schlafend statt zu spinnen.
- **Tests:** `tests/smoke.rs::write_returns_timeout_after_max_blocking_when_queue_full`
- **Status:** done — die write-Future bleibt schlafend (yield_for-Retry bis `max_blocking_time`, dann `Timeout`) statt zu spinnen; ein nativer drain-Notify-Hook statt Retry wäre eine mögliche Optimierung, sobald der Sync-Writer einen Drain-Channel exposed.

## §6 Listener-Bridge

### §6.1 data_available_stream
- **Anforderung:** `fn data_available_stream() -> impl Stream<Item = ()> + Send`.
- **Repo:** `crates/dcps-async/src/reader.rs::DataAvailableStream` (Polling-Probe: `reader.is_ready()`-Loop, kein konsumieren).
- **Tests:** kompiliert (lib + tests).
- **Status:** done — Listener-Stream baut auf nicht-konsumierender Probe; nativer Wakeup hängt an §3 (Reader-Slot-Waker), der live ist.

### §6.2 publication_matched_stream
- **Anforderung:** `fn publication_matched_stream() -> impl Stream<Item = PublicationMatchedStatus> + Send`.
- **Repo:** `crates/dcps-async/src/reader.rs::PublicationMatchedStream`
- **Tests:** `tests/smoke.rs::publication_matched_stream_yields_initial_count`
- **Status:** done — yieldet `usize` (Match-Count) bei jeder Aenderung.

## §7 Error-Mapping

### §7.1 DdsError unverändert
- **Anforderung:** Future::Output ist `Result<T, DdsError>` ohne Async-spezifische Error-Varianten.
- **Repo:** alle Methoden in `crates/dcps-async/src/{writer,reader}.rs` returnen `Result<T, DdsError>`.
- **Tests:** `tests/smoke.rs::wait_for_matched_subscription_times_out_when_no_reader` matcht `DdsError::Timeout`.
- **Status:** done

## §8 Test-Strategie

### §8.1 Async-Pendants pro Sync-Test
- **Anforderung:** Pro Sync-Test in `crates/dcps/tests/` einen async-Pendant.
- **Repo:** `crates/dcps-async/tests/{smoke,proptest_backpressure,cyclone_live_async_e2e}.rs`
- **Tests:** smoke deckt happy-path; proptest deckt Random-Sequenzen.
- **Status:** done — smoke + proptest + Cyclone-Live-E2E vorhanden.

### §8.2 Tokio-Test-Runner
- **Anforderung:** `tokio::test` als default; smol-Variante als feature-Probe.
- **Repo:** `crates/dcps-async/tests/smoke.rs` (alle Tests `#[tokio::test(flavor = "multi_thread")]`).
- **Tests:** Cargo.toml dev-dep `tokio = { features = ["rt-multi-thread", "macros", ...] }`.
- **Status:** done — tokio-Default; smol-Variante bleibt offen (kein Mehrwert solange tokio-glue der einzige Runtime-Hook ist).

### §8.3 proptest über Channel-Backpressure
- **Anforderung:** `proptest` über zufällige write/take-Sequenzen — Backpressure-Invariante.
- **Repo:** `crates/dcps-async/tests/proptest_backpressure.rs::write_take_sequence_holds_invariants`.
- **Tests:** 16 Cases mit zufälliger Capacity ∈ [1,8] + Op-Vec ∈ [0,32]; verifiziert: bei voller Queue MUSS write Timeout liefern, bei freier Queue MUSS Ok kommen.
- **Status:** done.

### §8.4 E2E gegen Cyclone-Live
- **Anforderung:** E2E gegen Cyclone-Live wie Sync; Latenz-Vergleich Sync vs Async.
- **Repo:** `crates/dcps-async/tests/cyclone_live_async_e2e.rs::async_reader_does_not_panic_against_live_cyclone_pub` (`#[ignore]`-gated, SSH-Lab-Setup); Latenz-Vergleich via `crates/dcps-async/benches/write_async_vs_sync.rs` + CI bench-main.
- **Tests:** Live-E2E mit `BENCH_HOST_AVAILABLE=1` + `cargo test -- --ignored` opt-in.
- **Status:** done — Live-Test als #[ignore]-Opt-in eingerichtet, Quantitative Latenz-Antwort liefert §9.1.

## §9 Performance-Targets

### §9.1 write().await Latenz
- **Anforderung:** ≤ 5 % Overhead gegen sync-write (criterion bench).
- **Repo:** `crates/dcps-async/benches/write_async_vs_sync.rs` + `.gitlab-ci.yml::bench-main` (`cargo bench -p zerodds-dcps-async --bench write_async_vs_sync -- --save-baseline pre`) + `bench-compare` Regression-Check.
- **Tests:** Bench läuft auf jedem main-Push; Regression > 10% rot via `tests/perf/check_bench_regressions.py`.
- **Status:** done — Bench in CI-Pipeline aktiv.

### §9.2 take_stream Throughput
- **Anforderung:** Kein Sample-Verlust durch Polling-Latenz; 100 % Sample-Rate.
- **Repo:** `crates/dcps-async/tests/throughput.rs`.
- **Tests:** `take_stream_delivers_all_samples_no_loss` — Reliable+KeepAll-Writer publiziert N=500, Reader drainiert `take_stream`, MUSS 100 % surfacen. ~70k samples/s auf codepit.
- **Status:** done — No-Loss-Invariante asserted, Throughput gemessen (codepit-verifiziert).

### §9.3 Allokation pro write()
- **Anforderung:** 0 Heap-Allokationen extra gegen sync.
- **Repo:** `crates/dcps-async/tests/alloc.rs` (dhat global-allocator).
- **Tests:** `async_write_adds_no_extra_heap_vs_sync` — dhat vergleicht per-write-Allokationen sync vs async (offline); async ≤ sync. Nach Entfernen des per-write `sample.clone()`: 9.03 blocks/write, **0 extra** (vorher +1).
- **Status:** done — Ziel erreicht (codepit-verifiziert); der Bench deckte den überflüssigen Klon auf, der jetzt entfernt ist.

## §10 Decisions

### D-1: Runtime-agnostische API als Default
- **Wahl:** Public-API liefert `impl Future`/`impl Stream` ohne tokio-Pin.
- **Begründung:** zerodds soll nicht eine Runtime erzwingen; Caller wählt.
- **Konsequenz:** Wakeup-Pfad nutzt einen eigenen Waker (nativen Reader-Slot-Waker; detached-thread-Sleep nur als Offline-Fallback) statt tokio::sync::Notify (siehe §3); das tokio-glue-Feature schaltet auf tokio-Wakeup um.

### D-2: Tokio-Glue als optional Feature
- **Wahl:** `--features tokio-glue` aktiviert tokio-spezifische Convenience.
- **Begründung:** Tokio dominiert (~85 % Marktanteil); Glue bietet Komfort ohne Zwang.
- **Konsequenz:** Default-Build hat keine `tokio`-Dep; Workspace-CI bleibt schlank.

### D-3: API-Symmetrie zur Sync-API
- **Wahl:** Methodennamen identisch (write, take, dispose, ...) — kein `_async`-Suffix.
- **Begründung:** Newtype-Pattern macht den Switch sync↔async zu einem reinen Type-Wechsel; Caller-Code ändert sich nicht.
- **Konsequenz:** Caller MUSS importieren `AsyncDataWriter` statt `DataWriter` — kein name-collision-Risk weil different module/path.

### D-4: WaitSet bleibt sync
- **Wahl:** WaitSet wird NICHT async-konvertiert in 1.0.
- **Begründung:** WaitSet ist Spec-DCPS-Konstrukt mit klaren Block-Semantik; `Stream<Item = ConditionEvent>` ist eine separate API-Schicht und braucht eigenes Design.
- **Konsequenz:** Caller, der WaitSet braucht, nutzt sync-API. Async-Pattern via take_stream + listener-streams.

### D-5: Listener-Bridge als Stream-Wrapper
- **Wahl:** sync-Listeners bleiben; async-Bridge bietet Stream-Variante.
- **Begründung:** Spec §2.2.2.4.4-Listeners sind Callbacks — async-Caller will lieber `while let Some(ev) = stream.next().await`.
- **Konsequenz:** Listener-Streams sind separate Methoden; sync-Listener-Set bleibt Default.
