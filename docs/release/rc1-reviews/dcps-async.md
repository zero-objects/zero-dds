# RC1 Review — `zerodds-dcps-async`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 4 (Core Services)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public (Dev-Repo `publish = false` weil transitiv via `zerodds-dcps` an Embargo-Pfad-Dep haengt; Public-Mirror `publish = true`).

---

## 1 Purpose

Runtime-agnostische async-Wrappers um die `zerodds-dcps`-Sync-API (zerodds-async-1.0). Newtypes teilen den internen `Arc<...>` mit den Sync-Pendants — kein State-Duplikat, kein Performance-Overhead.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Konsumenten-API fuer DDS-User, die ihren Stack auf einem async-Executor (tokio/smol/...) bauen.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs           # Crate-Entry, yield_for + SleepFuture, Re-Exports
├── factory.rs       # AsyncDomainParticipantFactory
├── participant.rs   # AsyncDomainParticipant
├── publisher.rs     # AsyncPublisher
├── subscriber.rs    # AsyncSubscriber
├── writer.rs        # AsyncDataWriter<T>
└── reader.rs        # AsyncDataReader<T> + SampleStream/DataAvailableStream/PublicationMatchedStream
```

### 3.2 Public-API-Surface

```rust
pub use factory::AsyncDomainParticipantFactory;
pub use participant::AsyncDomainParticipant;
pub use publisher::AsyncPublisher;
pub use reader::{AsyncDataReader, DataAvailableStream, PublicationMatchedStream, SampleStream};
pub use subscriber::AsyncSubscriber;
pub use writer::AsyncDataWriter;
pub use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipantQos, InstanceHandle,
    PublisherQos, Result, SubscriberQos, Topic, TopicQos,
};
pub use zerodds_dcps::status::SubscriptionMatchedStatus;
```

### 3.3 Tests

- `cargo test -p zerodds-dcps-async`: ✅ **9 passed**, 0 failed, 2 ignored (Lab-Live-Tests + Doc-Beispiel).
- E2E-Tests: 7 smoke-Tests + 1 Doc-Test + Lab-Live-Tests in `tests/cyclone_live_async_e2e.rs` (`#[ignore]` ohne `live-interop`).

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | Klassifikation | Decision |
|---|---|---|---|
| `AsyncDomainParticipantFactory` | zerodds-async-1.0 §1.1 | CONNECTED | — |
| `AsyncDomainParticipant` + Topic/Pub/Sub-Erzeugung | zerodds-async-1.0 §2 | CONNECTED | — |
| `AsyncPublisher` + `AsyncDataWriter::{write, register_instance, dispose, unregister_instance, wait_for_matched_subscription, matched_subscription_count}` | DDS 1.4 §2.2.2.4 + zerodds-async-1.0 §2.1 + §5.1 (Backpressure) | CONNECTED | — |
| `AsyncSubscriber` + `AsyncDataReader::{take, take_stream, wait_for_matched_publication, matched_publication_count}` | DDS 1.4 §2.2.2.5 + zerodds-async-1.0 §2.2 | CONNECTED | — |
| `SampleStream<T>` | zerodds-async-1.0 §2.2.1 + §3.3 (Reader-Slot-Waker) | CONNECTED | — |
| `DataAvailableStream<T>` | zerodds-async-1.0 §6.1 | CONNECTED | — (F-DCPS-ASYNC-data-available-consumes wire-up) |
| `PublicationMatchedStream<T>` (Item = `SubscriptionMatchedStatus`) | zerodds-async-1.0 §6.2 + DDS 1.4 §2.2.4.1 (SUBSCRIPTION_MATCHED) | CONNECTED | — (F-DCPS-ASYNC-pub-matched-stream-type wire-up) |
| `yield_for` (pub(crate)) + `SleepFuture` | zerodds-async-1.0 §3.2 (Runtime-agnostisches Sleep) | CONNECTED | — |

Ergebnis: **0 ❌-Klassen offen**. Alle Public-Items sind CONNECTED.

## 4 Wiring

### 4.1 Dependencies

```toml
[dependencies]
zerodds-dcps = { path = "../dcps" }
zerodds-qos  = { path = "../qos" }
futures-core = "0.3"
tokio = { version = "1", optional = true, features = ["rt", "sync", "time"] }
```

### 4.2 Dependents

End-User-Anwendungen, Bridges (mqtt/coap/grpc-bridge konsumieren async natuerlich), tokio-basierte DDS-Stacks.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | Standard-Library + Threads. |
| `tokio-glue` | ❌ | `tokio::time::sleep` als `yield_for`-Backend. |

## 5 Spec-Relevanz

- **Spec(s):** `docs/specs/zerodds-async-1.0.md` §1–§9 (komplett); OMG DDS 1.4 §2.2.2.4–5 (DataWriter/DataReader-API gespiegelt).
- **abgedeckte §-Sektionen:** alle Async-API-Sektionen wired.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

```bash
rg -i -e 'llvm@llvm' -e 'sandra-kessler' -e 'fishermen21' \
  -e '/Users/sandrakessler' -e 'PDE-Spec' -e 'zero-principle' \
  -e 'Ghost-Inject' -e '/tmp/cyc\.xml' \
  crates/dcps-async/src/ crates/dcps-async/Cargo.toml crates/dcps-async/README.md crates/dcps-async/CHANGELOG.md
```

Treffer: **0** in src/. Lab-Refs in `tests/cyclone_live_async_e2e.rs` per Guardrails §2.1 erlaubt (Public-Mirror-Exclude).

### 6.2 Sprint-/WP-/Phase-Marker

Pre-Cleanup: `Phase-1 / Phase-2`-Markers in writer.rs (1) und reader.rs (3). Post-Cleanup: **0** — die zur Doku-Aussage passenden Implementierungen sind voll wired (SampleStream nutzt nativen Reader-Slot-Waker, DataAvailableStream konsumiert nicht mehr, PublicationMatchedStream emittiert vollen Status), die stale-Phase-Texte sind durch fachliche Beschreibung ersetzt.

### 6.3 Datums-Marker

Keine im Source. CHANGELOG.md hat Keep-a-Changelog-Konvention-Marker (per Guardrails §2.1c erlaubt).

### 6.4 Soft-Review (TODO/FIXME/HACK)

Keine.

### 6.5 Lab-Refs in src/

Keine.

### 6.6 Public-API-Leaks

Keine.

### 6.7 Dead-Code

Keine.

## 7 Cleanup-Actions

1. **F-DCPS-ASYNC-data-available-consumes** (resolved): `DataAvailableStream::poll_next` rief `take()` (konsumierend) auf der Reader-Inbox auf — Samples wurden auf den Boden geworfen, ein Caller, der `data_available_stream` mit `take_stream` koppelte, sah die Samples NIE. Behoben: jetzt `read()` (non-consuming, DDS 1.4 §2.2.2.5.3.5) mit `last_seen_count`-Delta-Tracking; bei Live-Mode-Reader registriert sich der Stream via `register_user_reader_waker` an der DcpsRuntime — Wake erfolgt direkt beim `sample_tx.send`. Offline-Mode-Polling-Fallback bleibt. Neuer E2E-Test `data_available_stream_signals_without_consuming` belegt: nach Stream-Event ist das Sample immer noch via `take()` lesbar.
2. **F-DCPS-ASYNC-pub-matched-stream-type** (resolved): `PublicationMatchedStream` emittierte `Stream<Item = usize>` (nur den Match-Count); spec zerodds-async-1.0 §6.2 verlangt einen vollen Status. Behoben: jetzt `Stream<Item = SubscriptionMatchedStatus>` (Reader-side per DDS 1.4 §2.2.4.1, weil das Item auf dem `AsyncDataReader` lebt). Felder: `total_count` (cumulative max), `total_count_change`, `current_count`, `current_count_change`, `last_publication_handle`. Initialer Aufruf liefert den Synthesized-Snapshot fuer den Initial-Count (Delta von `last_count = MAX`). Test `publication_matched_stream_yields_initial_count` angepasst.
3. **F-DCPS-ASYNC-stale-phase-docs** (resolved): writer.rs/reader.rs hatten "Phase-1 delegiert / Phase-2 baut nativ um"-Doc-Comments, die nicht mehr stimmen — die nativen Pfade (yield-basierte Retry-Loop in `write`, Reader-Slot-Waker in SampleStream) sind voll wired. Doc-Texte auf den aktuellen Implementierungs-Stand umgeschrieben mit Spec-Referenz statt Roadmap-Sprache.
4. **SPDX-Headers** in allen 7 src-Files gesetzt.
5. **Cargo.toml-Metadata**: `homepage`, `documentation`, `readme`, `keywords`, `categories` ergänzt; `publish = false`-Begründung dokumentiert.
6. **Doc-Build-Warning** behoben: `Vec<T>` → `` `Vec<T>` `` in reader.rs (HTML-tag-Parsing in rustdoc).
7. **README.md** + **CHANGELOG.md** in RC1-Form.

## 8 Spec-Doc-Updates

- `docs/specs/zerodds-async-1.0.md` §6.2 — Item-Type-Korrektur sollte mit dem Code abgeglichen werden (war bisher `PublicationMatchedStatus`, korrekter Reader-side ist `SubscriptionMatchedStatus`). Spec-Doc-Update bleibt für die Spec-Curation-Phase (kein RC1-Blocker — die Crate ist Spec-konform mit der korrekten DDS-1.4-Semantik).

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig
- [x] `lib.rs`-Crate-Header mit Safety-Class + Spec-Ref + Layer + Beispiel
- [x] `README.md` auf RC1-Form
- [x] `CHANGELOG.md` mit `[1.0.0-rc.1]`-Entry
- [x] doc-tested Code-Example in `lib.rs` (ignore wegen `#[tokio::main]`-Macro)

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-dcps-async                            # ✅ 9 passed, 0 failed, 2 ignored
cargo clippy -p zerodds-dcps-async --tests -- -D warnings   # ✅ clean
cargo fmt --all -- --check                                  # ✅ clean
cargo doc -p zerodds-dcps-async --no-deps                   # ✅ keine Warnungen
cargo run --bin zerodds-lint -- check                       # ✅ workspace clean
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md aus Template
- [x] §1.4 CHANGELOG.md mit RC1-Entry
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit
- [x] §1.6 Spec-Coverage-Update (kein blocker — Spec-Curation deferred)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File (7 src-Files)
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt (= dieses Dokument)
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts (`github/crates/dcps-async/` + `github/Cargo.toml` + `github/CHANGELOG.md` + `website/docs/dcps-async.md`)
- [x] §1.13 Spec-Conformance-Audit (3 F-DCPS-ASYNC-Findings ✅ resolved)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅
