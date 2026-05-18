# zerodds-async 1.0 — Open Items

Phase-1 + Phase-2-A geliefert. Verbleibend:

## Phase-2 (offen)

### §3 Waker-Modell — nativer Reader-Slot-Waker

- **§3.1 Waker-Slot pro Reader** — UserReaderSlot bekommt `async_waker: Mutex<Option<Waker>>`. Phase-1 nutzt detached-thread.
- **§3.2 Wire-Pfad weckt Waker** — `deliver_to_reader_slot` weckt den Waker bei `sample_tx.send`.

### §5 Backpressure

- **§5.1 write-Future suspendiert bei OutOfResources** — `drain_notify`-Pattern statt Sync-Condvar.

### §8 Test-Strategie (partial)

- **§8.3 proptest fuer Channel-Backpressure** — zufaellige write/take-Sequenzen, kein Verlust/Deadlock.
- **§8.4 Cyclone-Live-E2E** — AsyncDataWriter gegen Cyclone-Reader.
- **§8.1 Sync-Test-Spiegelung** — vollstaendige Spiegelung.

### §9 Performance-Targets (partial)

- **§9.1 write().await Latenz** — Bench-Skelett da, numerische Bestaetigung in CI-Bench-Pipeline.
- **§9.2 take_stream Throughput** — Bench offen.
- **§9.3 Allokation pro write()** — dhat-rs-Bench offen.

### §4 Tokio-Glue (partial)

- **§4.1 spawn_in_tokio** — yield_for ist tokio-aware; Tick-Loop-Spawning offen.

## Sprint-Plan Phase-2

| Sprint | Items | Aufwand |
|--------|-------|---------|
| **A8** | §3.1 + §3.2 — Native Reader-Slot-Waker | 1 PT |
| **A9** | §5.1 — drain_notify Backpressure | 1 PT |
| **A10** | §4.1 — spawn_in_tokio | 0.5 PT |
| **A11** | §8.3, §9.1-§9.3 — Bench + proptest | 1 PT |
| **A12** | §8.4 — Cyclone-Live-E2E | 0.5 PT |

Gesamt-Phase-2: ~4 PT.

## Phase-1 done (zur Referenz)

- §1.1-§1.5 Type-Mapping ✓
- §2.1.2-§2.1.6 Writer-Methoden ✓
- §2.2.2-§2.2.4 Reader-Methoden ✓
- §6.2 publication_matched_stream ✓
- §7.1 Error-Mapping ✓
- §8.2 tokio::test ✓

partial:

- §2.1.1 write (suspend-on-OutOfResources fehlt)
- §2.2.1 take_stream (Polling-Wakeup statt nativ)
- §3.3 Stream-Waker-Registrierung (Phase-1: detached thread)
- §6.1 data_available_stream (konsumiert Samples beim Probing)
