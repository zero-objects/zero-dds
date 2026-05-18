# RC1 Review — `zerodds-time-service`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 1 (Primitives)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

OMG Time Service 1.1 (formal/2002-05-07) — Datentypen + Operationen + TimeService API. Standalone-Library. Pure-Rust no_std + alloc, `forbid(unsafe_code)`. Safety classification: STANDARD.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Standalone-Library für End-User-Applikationen, die OMG-Time-Service-1.1-Konformität brauchen (Distributed-Time-Sync mit Inaccuracy-Tracking, TIO-Overlap-Detection, etc.). Ein Tutorial-Konsument existiert bereits (`examples/tutorials/dds-warehouse/stations/02-time-sync/`).

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs           # Crate-Entry, Public-API-Aggregator, doctest
├── time_base.rs     # TimeBase-Modul (TimeT/InaccuracyT/TdfT/UtcT/IntervalT + current_time)
├── uto.rs           # Universal Time Object (Spec §1.3.4)
├── tio.rs           # Time Interval Object (Spec §1.3.5)
└── service.rs       # TimeService-Interface (Spec §2.1)
```

### 3.2 Public-API-Surface

```rust
// time_base
pub type TimeT = u64;
pub type InaccuracyT = u64;
pub type TdfT = i16;
pub struct UtcT { time, inacclo, inacchi, tdf }
pub struct IntervalT { lower_bound, upper_bound }
pub fn current_time() -> TimeT;   // std + no_std-Stub
pub const UTC_EPOCH_TO_UNIX_TICKS: TimeT;
pub const TICKS_PER_SECOND: u64;

// uto
pub struct Uto { ... }
pub enum ComparisonType { IntervalC, MidC }
pub enum TimeComparison { EqualTo, LessThan, GreaterThan, Indeterminate }

// tio
pub struct Tio { ... }
pub enum OverlapType { ... }

// service
pub struct TimeService { default_tdf, default_inaccuracy, secure_source }
pub struct TimeUnavailable;
```

### 3.3 Tests

- `cargo test -p zerodds-time-service`: ✅ **36 Tests** (35 unit + 1 doctest).

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `TimeT` / `InaccuracyT` / `TdfT` | Spec §1.3.2.1-3 | 0 production / 1 tutorial | SPEC-MANDATED-OPEN | doc-as-hook (siehe F-TIMESVC-2) |
| `UtcT` (16-byte wire) | Spec §1.3.2.4 | 0 production / 1 tutorial | SPEC-MANDATED-OPEN | doc-as-hook |
| `IntervalT` | Spec §1.3.2.5 | 0 production / 0 tutorial | SPEC-MANDATED-OPEN | doc-as-hook |
| `current_time` | (Wall-Clock-Helfer) | 0 production / 1 tutorial | CONNECTED via tutorial | — |
| `Uto` + `ComparisonType` + `TimeComparison` | Spec §1.3.4 | 0 production / 1 tutorial | CONNECTED via tutorial | — |
| `Tio` + `OverlapType` | Spec §1.3.5 | 0 production / 0 tutorial | SPEC-MANDATED-OPEN | doc-as-hook |
| `TimeService` + `TimeUnavailable` | Spec §2.1 + §1.3.3.1 | 0 production / 1 tutorial | CONNECTED via tutorial | — |
| `TICKS_PER_SECOND` (= 10_000_000) | Spec §1.3.2.4 (UtcT 100ns-Ticks) | 0 production / 0 tutorial | SPEC-MANDATED Public-Constant | doc-as-hook (Wire-Constant für End-User-Time-Inspect) |
| `UTC_EPOCH_TO_UNIX_TICKS` | Spec §1.3.2.4 (Epoch-Konstante 1582→1970) | 1 tutorial | CONNECTED via tutorial | — |

### 3.4.1 Sweep-Verifikation (§1.5b Pass 2)

`/tmp/zerodds-audit/time-service.tsv` enthält 15 Public-Items nach
Drop von `_wire_compat_check` (war `#[doc(hidden)]` Doc-Test-Helper
ohne Konsumenten — bei Layer-1-Pass-2 entfernt). Alle 15 Items sind in
der Tabelle oben oder durch Family-Rows abgedeckt. **0 DEAD nach Pass 2.**

**Befund:** ZeroDDS-DDS-DCPS verbraucht `zerodds-time-service` nicht intern. Das ist **by design** — DDS-DCPS 1.4 §2.3.3 verlangt sein eigenes 8-byte `Time_t` mit 1970-Unix-Epoch und 1ns-Auflösung, byte-distinkt zum 16-byte `UtcT` mit 1582-Epoch und 100ns-Ticks aus OMG-Time-Service 1.1. Beide Specs sind orthogonal.

## 4 Wiring

### 4.1 Dependencies

Keine externen oder workspace-internen Dependencies — Standalone.

### 4.2 Konsumenten

- **Production:** keine.
- **Tutorial:** `examples/tutorials/dds-warehouse/stations/02-time-sync/code/src/lib.rs` (verwendet `current_time`, `UTC_EPOCH_TO_UNIX_TICKS`, `TimeService`, `UtcT`, `TimeUnavailable`).

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std`   | ✅       | `current_time()` via `SystemTime`; std-Re-Exports |
| `alloc` | ✅       | mandatory (Vec/wire-buffer) |

## 5 Spec-Relevanz

- **Spec(s):** OMG Time Service 1.1 (formal/2002-05-07).
- **Coverage-Doc:** `docs/spec-coverage/omg-time-1.1.md` (43 done / 6 n/a (informative) / 0 partial / 0 open).
- **Out-of-Scope:** §2.2 + §2.4 TimerEventService — adressiert in `corba-ccm/src/time_psm.rs` (CCM-PSM-Pfad).

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

**Treffer:** keine.

### 6.2 Soft-Review (TODO/FIXME)

**Treffer:** keine.

### 6.3 Tech-Debt + Loose Ends

- **F-TIMESVC-1**: `cargo build --no-default-features` produzierte 3 Warnings — 2× unused-import `current_time` in `service.rs`/`uto.rs` (weil std-only verwendet) + 1× missing-doc auf no_std-Stub `current_time()`. **Status:** ✅ resolved — `current_time`-Imports auf `cfg(feature = "std")` konditional gemacht; no_std-Stub mit Spec-konformer Doc versehen.

- **F-TIMESVC-2**: 0 Production-Cross-Refs. Per Coherence-Audit ist das **kein Wire-up-Gap** — DDS-DCPS hat sein eigenes `Time_t` (spec-distinkt zu OMG-Time-Service `UtcT`); ein Auto-Wire wäre spec-fremd. **Status:** ✅ resolved als SPEC-MANDATED-OPEN public-API. Tutorial-Konsument `dds-warehouse/02-time-sync` validiert die Public-API end-to-end für Drittanwender. README + lib.rs Header dokumentieren das Verhältnis zu DDS-DCPS Time_t explizit (siehe Tabelle).

### 6.4 Public-API-Leaks

- Keine Glob-Reexports.
- Keine ungewollt `pub`-markierten Helper.

## 7 Cleanup-Actions

1. `Cargo.toml`: `repository`/`homepage`/`documentation`/`readme`/`keywords`/`categories` ergänzt; `description` ausgebaut; `publish = false` → `publish = true`. `repository.workspace = true` entfernt zugunsten expliziter URL.
2. SPDX-License-Header in alle 5 `src/*.rs`-Files eingefügt.
3. **F-TIMESVC-1 fix:** `current_time`-Imports auf `cfg(feature = "std")` konditional + Doc-Comment für no_std-Stub (`time_base.rs`, `service.rs`, `uto.rs`).
4. `src/lib.rs` Crate-Header erweitert: Spec-Block, Layer-Position, vollständige Public-API-Aufzählung, doctest-Beispiel, explizite Out-of-Scope-Notiz für TimerEventService und Verhältnis-Statement zu DDS-DCPS Time_t.
5. `README.md` neu geschrieben: Status-Badges, Scope-Block, Verhältnis-zu-DDS-DCPS-Time_t-Tabelle, Quick-Start, Feature-Flags, Stability.
6. `CHANGELOG.md` neu angelegt mit `[1.0.0-rc.1]`-Initial-Release-Entry.

## 8 Spec-Doc-Updates

`docs/spec-coverage/omg-time-1.1.md` ist bereits voll grün. Keine Änderung nötig.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig
- [x] `lib.rs`-Crate-Header
- [x] `README.md`
- [x] `CHANGELOG.md`
- [x] doc-tested Code-Example (1 doctest grün)

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-time-service                                    # ✅ 36 Tests grün
cargo clippy -p zerodds-time-service --all-targets -- -D warnings     # ✅ clean
cargo fmt -p zerodds-time-service -- --check                          # ✅ clean
cargo doc -p zerodds-time-service --no-deps                           # ✅ clean (post F-TIMESVC-1 + intra-doc-link fix)
cargo build -p zerodds-time-service --no-default-features             # ✅ no_std (post F-TIMESVC-1 fix)
cargo build -p zerodds-time-service --no-default-features --features alloc  # ✅
cargo run --bin zerodds-lint -- check                                 # ✅ workspace clean
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit (siehe §3.4 — F-TIMESVC-1 ✅ + F-TIMESVC-2 ✅)
- [x] §1.6 Spec-Coverage-Update (kein Delta nötig)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts (`github/crates/time-service/` + `website/docs/time-service.md`)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅
