# Async-DDS-Recherche — Quellen

Ausgangslage: **es gibt keine OMG-Spec für async-DDS-APIs**. Alle
existierenden Rust-DDS-Implementations definieren ihre eigene Form.
Wir definieren `zerodds-async-1.0` als Vendor-Spec mit klaren
Bezugspunkten.

## Primaerquellen (Rust-async-Runtime)

| Quelle | URL | Inhalt |
|--------|-----|--------|
| Tokio-Tutorial | <https://tokio.rs/tokio/tutorial> | Runtime-Modell, `spawn_blocking`, `Notify`, Channels |
| async-book | <https://rust-lang.github.io/async-book/> | Future-Trait, Pin/Unpin, async fn-Lowering |
| RFC 3185 (`async fn` in trait) | <https://rust-lang.github.io/rfcs/3185-static-async-fn-in-trait.html> | Stabilisiert in Rust 1.75 (Dec 2023) |
| Stream-Trait (`futures::Stream`) | <https://docs.rs/futures/0.3/futures/stream/trait.Stream.html> | Standard fuer asynchrone Iteratoren |

## DDS-Konkurrenz-Referenzen

| Implementation | Async-Pattern |
|----------------|---------------|
| **dust-dds** | `async`-Variante via `tokio` als optional feature; `DataWriter::write_async` parallel zu `write` |
| **CycloneDDS** | Pure-C, kein async; Bindings (cyclonedds-rs) bieten optional Tokio-Wrappers |
| **RTI Connext** | "Asynchronous Publication Mode" ist Spec-Begriff fuer **Background-Thread-Pool fuer Send**, nicht Rust-async. Async-API kommt aus C++-`std::future`-Wrappern |
| **Fast-DDS** | C++-`std::async`-Pattern, kein nativer async-Stream |

## Pattern-Optionen

### Option A: Tokio-only (ähnlich Iceoryx2-async)
- Hartes `tokio = "1"` als Dep, kein-Runtime-Agnostic.
- Pro: einfach, Tokio dominiert (~85% async-Code).
- Con: Wer `async-std`/`smol` nutzt, kann nicht.

### Option B: runtime-agnostic via `futures` traits
- Nur `futures::Stream` + `futures::executor` zum Polling.
- Pro: caller waehlt seine Runtime.
- Con: keine `tokio::sync::Notify`-Wakeups → Polling/CondVar-Schicht
  selbst implementieren.

### Option C: Feature-gated tokio + smol
- `--features tokio-runtime` und `--features smol-runtime` parallel.
- Pro: maximaler Reichweite-Hebel.
- Con: 2x Maintenance.

## Empfehlung fuer zerodds-async-1.0

**Option B (runtime-agnostic) als Public-API**:
- `AsyncDataWriter::write(sample) -> impl Future<Output = Result<()>>`
- `AsyncDataReader::take_stream() -> impl Stream<Item = Sample>`
- Default-Implementation nutzt einen `Waker` der vom Wire-Pfad
  geweckt wird; Caller-Runtime ist beliebig.

**Optional Tokio-Convenience** als Feature `tokio-glue`:
- `async fn DcpsRuntime::run_in_background() -> JoinHandle`
- `tokio::sync::Notify` fuer den Wakeup-Pfad als optional
  Optimization, statt CondVar.

## Bezugspunkte zur Sync-API

Die zerodds-async-Spec MUSS strikte API-Symmetrie zur Sync-API
halten:
- Methoden-Namen identisch ohne Suffix (`write` statt `write_async`).
- QoS-Semantik identisch (Reliable, Durability, etc.).
- Status-Conditions identisch — `wait_for_data` wird zu `await`.

Das macht den Wechsel sync↔async zu einem reinen Type-Wechsel:
`DataWriter` ↔ `AsyncDataWriter` (Newtype-Pattern um den gleichen
internen `Arc<DataWriterInner>`).

## Relevante OMG-Begriffe (NICHT Rust-async!)

DDS-DCPS 1.4 §2.2.4.1 verwendet "asynchronous" im Sinne von
"non-blocking notification" — das ist die `WaitSet`-Semantik
(synchron blockierend in einer eigenen Thread-Schleife) bzw.
`Listener` (synchron in Callback). Beides ist KEIN Rust-async.
