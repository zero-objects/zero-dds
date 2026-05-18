# RC1 Review — `zerodds-transport-shm`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 2.4 (Wire — POSIX-SHM)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

Cross-Process Shared-Memory-Transport via POSIX `shm_open` + `mmap`.
Lock-free SpSc-Ringbuffer pro `(owner, consumer)`-Paar. Crash-Recovery
via predictable `os_id` + `shm_unlink`.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Same-Host-IPC für Multi-Process DDS-Anwendungen.

## 3 Content-Inventur

### 3.1 Module

```
src/lib.rs    # Crate-Header + Re-Exports
src/posix.rs  # PosixShmTransport-Implementation (~1170 LOC)
```

### 3.2 Public-API-Surface

```rust
pub struct PosixShmTransport;
pub struct ShmConfig;
pub enum   ShmRole;
pub enum   PosixShmError;
pub const  SHM_MAGIC: u32;
pub const  SHM_VERSION: u32;
pub const  HEADER_BYTES: usize;
pub const  DEFAULT_CAPACITY: usize;
pub const  DEFAULT_FLINK_DIR: &str;
```

### 3.3 Tests

- `cargo test -p zerodds-transport-shm`: ✅ 18 passed (17 lib + 1 cross-process integration).

### 3.4 Coherence-Audit (§1.5b)

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `PosixShmTransport` | ZeroDDS-SHM-Transport 1.0 §2-§5 | 2 (tools/isolation-smoke, tools/bench-suite) | CONNECTED | — |
| `ShmConfig` | ZeroDDS-SHM-Transport 1.0 §2 (Config) | 2 (analog) | CONNECTED | — |
| `ShmRole` (Owner/Consumer) | ZeroDDS-SHM-Transport 1.0 §4.1 (SpSc-Modell) | 0 — Argument-Type für `open_owner`/`open_consumer` (öffentliche Konstruktor-Methoden) | VENDOR-EXTENSION (Public-API-Discriminator) | — |
| `PosixShmError` | ZeroDDS-Vendor-Error | 0 direkte Match-Sites; Return-Type aller pub-Konstruktor-Methoden | VENDOR-EXTENSION (Public-API-Error-Contract) | — |
| `SHM_MAGIC` (`"ZSHM"`) | ZeroDDS-SHM-Transport 1.0 §2 | 0 ext, 3 int | VENDOR-EXTENSION (Wire-Format-Konstante als Public-API-Inspekt) | — — End-User können das Magic-Discriminator-Wert prüfen |
| `SHM_VERSION` (1) | ZeroDDS-SHM-Transport 1.0 §2 | 0 ext, 3 int | VENDOR-EXTENSION (Wire-Version-Konstante) | — |
| `HEADER_BYTES` (64) | ZeroDDS-SHM-Transport 1.0 §2 (Layout-Konstante) | 0 ext, 9 int | VENDOR-EXTENSION (Layout-Sizing-Konstante für End-User) | — |
| `DEFAULT_CAPACITY` (1 MiB) | Library-Default | 0 ext, 2 int | VENDOR-EXTENSION (Public Default-Konstante) | — |
| `DEFAULT_FLINK_DIR` | Library-Default | 0 ext, 2 int | VENDOR-EXTENSION (Public Default-Konstante) | — |

**Zusammenfassung:** 9/9 Public-Items klassifiziert. 0 ❌-Klassen.
Alle "OVER-EXPOSED"-Treffer sind legit Public-API der ZeroDDS-SHM-
Transport-1.0-Spec (Magic + Version + Layout-Konstanten + Defaults +
Error-Type + Role-Discriminator).

## 4 Wiring

### 4.1 Dependencies

```toml
zerodds-rtps = { path = "../rtps" }
zerodds-transport = { path = "../transport" }
shared_memory = "0.12"
libc = "0.2"
```

### 4.2 Dependents

`tools/isolation-smoke`, `tools/bench-suite`, `tests/l1_cross_process.rs`.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | std + mmap |
| `alloc` | ✅ (via std) | — |
| `safety` | ❌ | reserved |

## 5 Spec-Relevanz

- **Spec:** ZeroDDS-SHM-Transport 1.0 (`docs/spec-coverage/zerodds-shm-transport-1.0.md`).
  - DDSI-RTPS 2.5 §9.4 (Locator-Kind vendor-reserviert).
  - Vendor-Spec für Segment-Layout + SpSc + Cleanup.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

Treffer: keine.

### 6.2 Soft-Review-Treffer

Treffer: keine.

### 6.3 Tech-Debt + Dead Code

- Intra-Process-Stub (`ShmTransport`, `registry`, `ring_buffer`-Module, ~455 LOC) entfernt — war Phase-1-Artefakt ohne externen Konsumenten (siehe Cleanup-Action #1).

### 6.4 Public-API-Leaks

Keine.

## 7 Cleanup-Actions

1. **Drop**: 3 Stub-Module entfernt (`shm_transport.rs`, `registry.rs`, `ring_buffer.rs`).
2. SPDX-Header in beiden verbleibenden Files.
3. Cargo.toml RC1-Metadata.
4. Crate-Header rewrite: ehrliche Spec-Story (DDSI-RTPS §9.4 OMG-normativ;
   Segment-Layout/SpSc/Cleanup ZeroDDS-eigen).
5. ZeroDDS-SHM-Transport-1.0-Spec materialisiert.
6. README + CHANGELOG.

## 8 Spec-Doc-Updates

Neuer Spec-Doc `docs/spec-coverage/zerodds-shm-transport-1.0.md` (§1-§8).

## 9 Doc-Artefacts

- [x] Cargo.toml RC1
- [x] lib.rs-Header mit Spec + Plattform-Tabelle
- [x] README + CHANGELOG

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-transport-shm                                # ✅ 18 passed
cargo clippy -p zerodds-transport-shm --all-targets -- -D warnings # ✅
cargo doc -p zerodds-transport-shm --no-deps                       # ✅
```

## 11 RC1-DoD-Checkliste

- [x] §1.1-§1.13 alle ✅

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer:** Claude
