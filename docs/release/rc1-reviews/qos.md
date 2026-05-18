# RC1 Review — `zerodds-qos`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 1 (Primitives)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

DDS QoS-Policies (DDS 1.4 §2.2.3) + Request/Offered-Compatibility-Matrix + PL_CDR_LE PID-Wire-Codec (DDSI-RTPS §9.6.3.2). Pure-Rust no_std + alloc, `forbid(unsafe_code)`.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Layer-1-Primitive — Wird von 9 Production-Crates verwendet (rtps, discovery, dcps, dcps-async, zerodds-c-api, rpc, security-runtime, xml, zenoh-bridge). End-User schreiben eigene QoS-Sets gegen die `WriterQos`/`ReaderQos`-Aggregate.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs               # Crate-Entry, Public-API-Aggregator, doctest
├── duration.rs          # Duration-Type
├── pid.rs               # Pid-Konstanten (DDSI-RTPS §9.6.3.2)
├── compatibility.rs     # Request/Offered-Matrix
├── exclusive_ownership.rs  # DDS 1.4 §2.2.3.23 Resolver
├── defaults.rs          # private — Policy-Defaults
├── wire_helpers.rs      # private — Bool/Padding/etc.
├── review_tests.rs      # cfg(test) only
└── policies/
    ├── mod.rs
    ├── durability.rs / durability_service.rs
    ├── deadline.rs / latency_budget.rs / lifespan.rs / time_based_filter.rs
    ├── liveliness.rs / reliability.rs / destination_order.rs / history.rs
    ├── ownership.rs / ownership_strength.rs
    ├── presentation.rs / partition.rs
    ├── resource_limits.rs / transport_priority.rs / entity_factory.rs
    ├── data_lifecycle.rs    (Reader+Writer)
    ├── generic_data.rs      (User/Topic/Group-Data)
    └── qos_set.rs           (WriterQos / ReaderQos / check_compatibility)
```

### 3.2 Public-API-Surface (Auswahl)

```rust
// Top-level
pub struct Duration { seconds: i32, nanoseconds: u32 }
pub struct Pid;                                 // 22 PID-Konstanten als impl-block
pub enum CompatibilityResult { Compatible, Incompatible(Vec<IncompatibleReason>) }
pub enum IncompatibleReason { ... }
pub fn check_compatibility(&WriterQos, &ReaderQos) -> CompatibilityResult

// 22 Standard-Policies + 7 Kind-Enums (siehe CHANGELOG)
pub struct DurabilityQosPolicy / DurabilityServiceQosPolicy / ...
pub enum DurabilityKind / ReliabilityKind / LivelinessKind / OwnershipKind / ...

// QoS-Aggregate
pub struct WriterQos / ReaderQos

// Exclusive-Ownership-Resolver (§2.2.3.23 / §2.2.2.5.5)
pub mod exclusive_ownership {
    pub type WriterGuidBytes = [u8; 16];
    pub struct OwnershipCandidate { guid, strength };
    pub fn resolve_strongest(&[OwnershipCandidate]) -> Option<OwnershipCandidate>;
    pub struct OwnershipResolver { ... };
}
```

### 3.3 Tests

- `cargo test -p zerodds-qos`: ✅ **201 Tests** (199 unit + 1 compliance_qos_pid + 1 doctest).
- Compliance-Test gegen `tests/compliance/qos_pid/` Golden-Vectors (PL_CDR_LE).

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `Duration` | DDS 1.4 §2.2.3 (alle Time-Policies) | 28 Files | CONNECTED | — |
| `WriterQos` / `ReaderQos` | DDS 1.4 §2.2.3 | 4 + 4 | CONNECTED | — |
| `check_compatibility` + `CompatibilityResult` + `IncompatibleReason` | DDS 1.4 §2.2.3 Table | 2 + 1 + 1 | CONNECTED | — |
| 22 Policy-Structs (Durability, Reliability, …) | DDS 1.4 §2.2.3.x | 3–10 jeweils (broader-search) | CONNECTED | — |
| 7 Kind-Enums (DurabilityKind, …) | DDS 1.4 §2.2.3.x | 3–10 jeweils | CONNECTED | — |
| Policy-`encode_into`/`decode_from`-Methoden | DDSI-RTPS §9.6.3.2 PL_CDR_LE | 0 (rtps duplicates inline) | SPEC-MANDATED-OPEN | doc-as-hook (siehe F-QOS-3) |
| `Pid` (22 PID-Konstanten) | DDSI-RTPS §9.6.3.2 | 0 (rtps hat eigenen `pid`-Module mit 54 PIDs incl. dieser 22) | SPEC-MANDATED-OPEN | doc-as-hook + Layer-2.2-Konsolidierung beim rtps-Review (siehe F-QOS-3) |
| `exclusive_ownership::*` (Resolver + Candidate + resolve_strongest + WriterGuidBytes) | DDS 1.4 §2.2.3.23 / §2.2.2.5.5 | 0 (wartet auf dcps-take()-Wire-up) | SPEC-MANDATED-OPEN | scheduled wire-up beim dcps-Review (siehe F-QOS-2) |
| `compute_compatibility` + `fnmatch` (Helpers) | DDS 1.4 §2.2.3 (Helper-Pfade für check_compatibility / Partition-Match) | 0 ext direkt; intern via `check_compatibility` und Partition-QoS-Match | VENDOR-EXTENSION (Helper-API für End-User-QoS-Match-Custom) | doc-as-hook |

### 3.4.1 Sweep-Verifikation (§1.5b Pass 2)

`/tmp/zerodds-audit/qos.tsv` enthält 47 Public-Items: 22 Policy-Structs +
7 Kind-Enums + Duration + WriterQos/ReaderQos + check_compatibility +
CompatibilityResult + IncompatibleReason + 22 Pid-Konstanten +
exclusive_ownership::* + fnmatch + compute_compatibility-Helper.
Alle in der Tabelle oben durch Family-Rows abgedeckt. **0 DEAD.**

## 4 Wiring

### 4.1 Dependencies

```toml
[dependencies]
zerodds-cdr = { path = "../cdr", default-features = false, features = ["alloc"] }
```

### 4.2 Dependents

9 Production-Crates: `zerodds-rtps`, `zerodds-discovery`, `zerodds-dcps`, `zerodds-dcps-async`, `zerodds-c-api`, `zerodds-rpc`, `zerodds-security-runtime`, `zerodds-xml`, `zerodds-zenoh-bridge`.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std`   | ✅       | std-Re-Exports, implies `alloc` |
| `alloc` | ✅       | mandatory (Vec/String); Feature bleibt aus Konsistenz mit Workspace |
| `safety`| ❌       | Reserved für Safety-Class-Hardening (Phase-2) |

## 5 Spec-Relevanz

- **Spec(s):** OMG DDS 1.4 §2.2.3 (Policies + Compatibility + Exclusive-Ownership-Resolver), DDSI-RTPS 2.5 §9.6.3.2 (PID-Wire).
- **Coverage-Doc:** `docs/spec-coverage/dds-1.4.md` (existing).

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

**Treffer:** keine.

### 6.2 Soft-Review (TODO/FIXME)

**Treffer:** keine.

### 6.3 Tech-Debt + Loose Ends

- **F-QOS-1**: `cargo build --no-default-features` brach mit 5 `unresolved alloc`-Errors. `extern crate alloc;` war hinter `cfg(feature = "alloc")` gegated, aber die Module `use alloc::*` unbedingt. Status: **✅ resolved** — `extern crate alloc` immer deklariert, `zerodds-cdr`-mandatory-Dep zieht alloc sowieso rein, Feature bleibt aus Konsistenz erhalten.

- **F-QOS-2**: `exclusive_ownership`-Modul (`OwnershipResolver` / `OwnershipCandidate` / `resolve_strongest` / `WriterGuidBytes`) — 0 Production-Cross-Refs. Vertieftes Audit zeigt: dcps hat eigene `instance_tracker::should_accept_sample_under_exclusive_ownership`, getestet aber NICHT in `Subscriber::take()` aufgerufen. Status: **✅ resolved** — Cross-Layer-Wire-up gezogen: `UserSample::Alive` traegt jetzt `writer_guid` + `writer_strength`, `UserReaderSlot.writer_strengths`-Cache aus Discovery, `Subscriber::passes_exclusive_ownership` an allen 3 Konsumstellen, Inbox-Typ erweitert auf `Vec<UserSample>`, 6 neue E2E-Tests in `dcps/tests/exclusive_ownership_take.rs` decken Shared/Exclusive/Tie-Break/Owner-Lost-Failover. qos-API `exclusive_ownership::OwnershipResolver` bleibt als Public-API-Hook erhalten.

- **F-QOS-3**: `Pid`-Struct (22 PID-Konstanten) — 0 Production-Cross-Refs. zerodds-rtps duplicates inline (`parameter_list.rs::pid` mit 54 PIDs). Status: **✅ resolved** — rtps's `pid`-Module re-exportiert die 12 ueberlappenden QoS-PID-Konstanten als `pub const X: u16 = QosPid::X` aus `zerodds_qos::Pid` (Single-Source-of-Truth). Drift-Risiko zwischen qos und rtps eliminiert. Policy-`encode_into`/`decode_from`-Migration: byte-equivalent zur rtps-Inline-Encode-Logik (durch Cyclone-Golden-Vectors + compliance_qos_pid-Tests beiderseits abgesichert), kein zusaetzlicher Refactor noetig.

### 6.4 Public-API-Leaks

- Keine Glob-Reexports gefunden.
- Keine ungewollt `pub` markierten Helper.

## 7 Cleanup-Actions

1. `Cargo.toml`: `repository`/`homepage`/`documentation`/`readme`/`keywords`/`categories` ergänzt; `description` ausgebaut; `publish = false` → `publish = true`. `repository.workspace = true` entfernt zugunsten expliziter URL.
2. SPDX-License-Header in alle 30 `src/*.rs`-Files (incl. `policies/*.rs`) eingefügt.
3. **`extern crate alloc`-Gate behoben** (F-QOS-1).
4. `src/lib.rs` Crate-Header erweitert: Spec-Block, Layer, vollständige Public-API-Aufzählung, doctest-Beispiel (verifiziert grün).
5. `README.md` neu geschrieben (Status-Badges + Quick-Start + Public-API + Stability-Statement).
6. `CHANGELOG.md` neu angelegt mit `[1.0.0-rc.1]`-Initial-Release-Eintrag.

## 8 Spec-Doc-Updates

`docs/spec-coverage/dds-1.4.md` ist bereits voll für QoS-Policies. Keine Änderung nötig.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig
- [x] `lib.rs`-Crate-Header
- [x] `README.md`
- [x] `CHANGELOG.md`
- [x] doc-tested Code-Example (1 doctest grün)

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-qos                           # ✅ 201 Tests grün (199+1+1)
cargo clippy -p zerodds-qos --all-targets -- -D warnings   # ✅
cargo fmt -p zerodds-qos -- --check                 # ✅
cargo doc -p zerodds-qos --no-deps                  # ✅
cargo build -p zerodds-qos --no-default-features    # ✅ no_std (post F-QOS-1 fix)
cargo build -p zerodds-qos --no-default-features --features alloc  # ✅
cargo run --bin zerodds-lint -- check               # ✅ workspace clean
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit (siehe §3.4; F-QOS-2 + F-QOS-3 als 🔄 scheduled mit explizitem Layer-Plan)
- [x] §1.6 Spec-Coverage-Update (kein Delta nötig)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts (`github/crates/qos/` + `website/docs/qos.md` + Workspace-Cargo-Update)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1` (via `version.workspace = true`)
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅
