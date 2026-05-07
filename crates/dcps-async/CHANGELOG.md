# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-dcps-async`-Crate.

### Spec-Referenzen

- **`docs/specs/zerodds-async-1.0.md`** §1–§9 — komplette async-Wrapper-API: §2 (Newtypes), §3 (Stream-Semantik + Reader-Slot-Waker), §5 (Backpressure-Retry), §6 (Listener-Bridge: `data_available_stream`/`publication_matched_stream`), §7 (Error-Mapping), §8 (Test-Strategie), §9 (Performance-Targets).
- **OMG DDS 1.4** §2.2.2.4 (DataWriter) + §2.2.2.5 (DataReader) — gespiegelte Sync-API. §2.2.4.1 SUBSCRIPTION_MATCHED — Status-Struct, der vom `PublicationMatchedStream` emittiert wird.

### Public-API

- `AsyncDomainParticipantFactory` — Singleton-Wrapper um `DomainParticipantFactory`. `instance()`, `create_participant_offline`, `create_participant`, `create_participant_with_qos`.
- `AsyncDomainParticipant` — Topic/Publisher/Subscriber-Erzeugung; teilt Arc-State mit dem Sync-Pendant.
- `AsyncPublisher` / `AsyncDataWriter<T>` — `write(&sample).await`, `register_instance`, `dispose`, `unregister_instance`, `wait_for_matched_subscription(min_count, timeout).await`, `matched_subscription_count`, `qos`, `as_sync`.
- `AsyncSubscriber` / `AsyncDataReader<T>` — `take(timeout).await`, `take_stream() -> SampleStream<T>`, `wait_for_matched_publication`, `matched_publication_count`, `data_available_stream() -> DataAvailableStream<T>`, `publication_matched_stream() -> PublicationMatchedStream<T>`, `qos`, `as_sync`.
- Streams: `SampleStream<T>`, `DataAvailableStream<T>`, `PublicationMatchedStream<T>` — alle implementieren `futures_core::Stream`.
- Re-Exports: `DataReaderQos`, `DataWriterQos`, `DdsError`, `DdsType`, `DomainParticipantQos`, `InstanceHandle`, `PublisherQos`, `Result`, `SubscriberQos`, `Topic`, `TopicQos`, `SubscriptionMatchedStatus`.

### Implementierung

`AsyncDataWriter::write` ist eine Future-Form ueber `DataWriter::write` mit yield-basierter Retry-Schleife: bei `OutOfResources` (Queue voll + Reliable + `max_blocking_time > 0`) suspendiert der Future via `yield_for(2 ms)` und retried bis Drain oder Deadline. Statt `Condvar::wait_timeout` (Sync-Pfad) bleibt der Caller-Task cancelable.

`SampleStream::poll_next` registriert sich im Live-Mode mit `register_user_reader_waker` an der `DcpsRuntime` — der Waker wird beim `sample_tx.send` direkt gefeuert (kein Polling). Im Offline-Mode greift ein detached-Thread-Sleep als Polling-Fallback. Buffered Samples werden eins-pro-Poll yielded.

`DataAvailableStream::poll_next` ruft den nicht-konsumierenden `DataReader::read()` (DDS 1.4 §2.2.2.5.3.5) und vergleicht die Sample-Anzahl mit der bei der letzten Emission. Steigender Count → emit `()`-Event. Samples bleiben im Reader-Cache; Caller muss sie via `take()`/`take_stream` separat konsumieren. Live-Mode-Wake via `register_user_reader_waker`, Offline-Mode-Polling als Fallback.

`PublicationMatchedStream::poll_next` ueberwacht `matched_publication_count` und emittiert pro Aenderung einen `SubscriptionMatchedStatus`-Snapshot mit synthetisierten `total_count`/`current_count`/`*_change`-Feldern (Reader-side Counter, da der Sync-Pfad keinen direkten `subscription_matched_status()`-Getter exponiert).

`yield_for` ist runtime-agnostisch: ohne `tokio-glue`-Feature spawnt ein detached-Thread, der den Waker nach Ablauf weckt; mit `tokio-glue` nutzt es `tokio::time::sleep`.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-dcps`, `zerodds-qos`, `futures-core`. Optional: `tokio` (Feature `tokio-glue`).
- **Dependents (out):** End-User-Anwendungen, die DDS via async-Pfad konsumieren wollen; Bridges (mqtt-/coap-/grpc-bridge) wo die Async-Form fuer das jeweilige Bridge-Backend natuerlich ist.
- **Feature-Flags:** `std` (default), `tokio-glue`.

### Stabilitaet

Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump auf `2.0.0`.
