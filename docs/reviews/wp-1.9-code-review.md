# WP 1.9 — SHM-Transport (Phase-1) Code Review

**Scope**: `crates/transport-shm/` + `LocatorKind::Shm` in `crates/rtps/src/wire_types.rs`
**Commit-Range**: seit `bb0799b` · **Datum**: 2026-04-20
**Overall**: Needs Work — solider SAFE-Phase-1-Stub, mehrere Soundness-/Test-Luecken.

## Findings

| # | Severity | Location | Issue & Empfehlung |
|---|----------|----------|--------------------|
| 1 | **Critical** | `shm_transport.rs:88-104` | `send_to_self`-Semantik undefiniert. UDP erlaubt Self-Loopback; hier unklar. Dokumentieren oder explizit rejecten + Test. |
| 2 | **Critical** | `shm_transport.rs:42-53,82-86` | TOCTOU: `bind()` macht `lookup` dann `register`; `register` ueberschreibt lautlos. Zweiter `bind` gleicher ID nach Drop-Race kann fremdes Segment klauen. Fix: `entry().or_insert_with` atomar. |
| 3 | **High** | `wire_types.rs:475` | `0x01000000` liegt NICHT im RTPS-Vendor-Range (§9.3.1.2: Vendor = MSB gesetzt / negative i32). Kollisionsrisiko mit OMG-Spec-Erweiterungen. Empfehlung: negativen i32 (z.B. `0xFF00_0000` als i32 = -16777216). |
| 4 | **High** | `shm_transport.rs:106-121,82-86` | Kein Shutdown-Signal. `recv` blockiert endlos nach letztem-Sender-Drop. Drop muss `closed: AtomicBool` setzen + `cv.notify_all()`; Loop prueft Flag → `RecvError::Closed`. |
| 5 | **High** | `registry.rs:38` | Prozess-globale Registry + feste Test-IDs → Kollisionen bei parallelem `cargo test`. Fix: `AtomicU64`-Counter oder `#[serial_test]`. |
| 6 | **High** | `registry.rs:43-60` | `if let Ok(mut r) = REGISTRY.lock()` verschluckt Poisoning silent → spaetere `bind` geben `AlreadyBound` fuer tote Entries. Fix: `.unwrap_or_else(PoisonError::into_inner)`. |
| 7 | **Medium** | `shm_transport.rs:42` | API-Asymmetrie zu UDP (`bind(SocketAddr)`). Empfehlung: `bind(Locator)` mit `kind==Shm`-Validation. |
| 8 | **Medium** | `ring_buffer.rs:22-27` | `VecDeque::with_capacity(capacity.min(1024))` reallokiert ueber 1024 hinaus — wirft Zero-Copy-Versprechen weg. Entweder voll prealloc oder `.min(1024)` dokumentieren. |
| 9 | **Medium** | `wire_types.rs:460,525` | Neue `Ord`-Derives machen Feld-Reihenfolge Teil der API (BTreeMap-Keys). Kein Fixture-Test stabiler Ordnung. |
| 10 | **Medium** | `shm_transport.rs:128-201` | Test-Gaps: (a) concurrent senders, (b) blocking `recv`-Wakeup im Thread, (c) drop-while-recv-blocks, (d) interleaved send/try_recv. Nur Happy-Path. |
| 11 | **Low** | `shm_transport.rs:73-79` | `dropped_frames()` → `unwrap_or_default()=0` bei Poison verschleiert Fehler. Besser `Result<u64, _>`. |
| 12 | **Low** | `shm_transport.rs:64` | `ReceivedDatagram.source = self.local_locator` ist Receiver, nicht Sender. UDP-Pendant setzt Peer-Source. Semantik dokumentieren oder Sender-Locator muxen. |

## Positive Highlights

- `#[forbid(unsafe_code)]` konsequent, Phase-1-Scope klar dokumentiert.
- Saubere Trennung Registry / RingBuffer / Transport → einfacher POSIX-Swap fuer WP 2.9.
- `drop_oldest`-Policy + `dropped`-Counter als built-in Observability.
- Condvar-Loop spurious-wakeup-safe (`while` + `cv.wait` korrekt).

## Priorisierung vor Freigabe

1. **Finding 3** (Vendor-Namespace) — pre-merge, Wire-Breaking spaeter.
2. **Finding 2 + 4** (Bind-TOCTOU + Shutdown-Signal).
3. **Finding 5 + 10** (Test-Isolation + concurrent/blocking/shutdown).
4. **Finding 1 + 12** (Self-Send- & Source-Semantik).

**Security**: B · **Maintainability**: B+ · **Test Coverage**: Happy-Path only.
