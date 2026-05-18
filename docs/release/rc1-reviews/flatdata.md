# RC1 Review — `zerodds-flatdata`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 4 (Core Services)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public (`publish = true` — keine Embargo-Deps).

---

## 1 Purpose

Zero-Copy Same-Host-Pub/Sub-Primitive: `FlatStruct`-Trait, Slot-Layout per Spec, drei produktive `SlotBackend`-Implementationen (in-memory + POSIX shm/mmap + Iceoryx2-Bridge).

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Keine ZeroDDS-Crate-Deps, keine Embargo-Pfad-Deps. End-User koennen die Crate direkt fuer Custom-Same-Host-Pub/Sub-Stacks nutzen, unabhaengig von DCPS.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs          # Crate-Entry, FlatStruct-Trait, Re-Exports
├── allocator.rs    # InMemorySlotAllocator + SlotError + SlotHandle
├── backend.rs      # SlotBackend-Trait
├── iceoryx.rs      # Iceoryx2SlotAdapter (Feature iceoryx2-bridge)
├── locator.rs      # ShmLocator + is_same_host + fnv1a_32
├── posix.rs        # PosixSlotAllocator + PosixSlotError (Feature posix-mmap)
├── pubsub.rs       # FlatWriter / FlatSlot / FlatReader / FlatSampleRef
└── slot.rs         # SlotHeader + SLOT_HEADER_SIZE + ReaderMask + align_up
```

### 3.2 Public-API-Surface

```rust
pub unsafe trait FlatStruct: Copy + 'static + Send + Sync;
pub struct SlotHeader; pub const SLOT_HEADER_SIZE: usize; pub type ReaderMask;
pub trait SlotBackend: Send + Sync;
pub struct SlotHandle; pub enum SlotError;
pub struct InMemorySlotAllocator;
#[cfg(feature = "posix-mmap")] pub struct PosixSlotAllocator; pub enum PosixSlotError;
#[cfg(feature = "iceoryx2-bridge")] pub struct Iceoryx2SlotAdapter;
pub struct FlatWriter<T>; pub struct FlatSlot<'a, T>;
pub struct FlatReader<T>; pub struct FlatSampleRef<T>;
pub struct ShmLocator; pub enum LocatorError;
pub fn is_same_host(...) -> bool; pub fn fnv1a_32(bytes: &[u8]) -> u32;
```

### 3.3 Tests

- `cargo test -p zerodds-flatdata`: ✅ **38 passed**, 0 failed (Default-Features).
- `cargo test -p zerodds-flatdata --all-features`: ✅ **48 passed**, 0 failed (POSIX-Tests + 4 echte iceoryx2-Bridge-Tests inkl. Pub/Sub-Roundtrip im selben Process).

### 3.4 Coherence-Audit

| Public-Item | Spec-Anker | Klassifikation | Decision |
|---|---|---|---|
| `FlatStruct` (Trait) | flatdata-1.0 §1.1 | CONNECTED (von dcps + Tests) | — |
| `SlotHeader` + `SLOT_HEADER_SIZE` + `ReaderMask` | flatdata-1.0 §2 | CONNECTED | — |
| `SlotBackend`-Trait | flatdata-1.0 §4.1 (Backend-Abstraktion) | CONNECTED | — |
| `SlotHandle` + `SlotError` | flatdata-1.0 §4 | CONNECTED | — |
| `InMemorySlotAllocator` + `with_type_hash` | flatdata-1.0 §4.1 + §6.1 | CONNECTED | — |
| `PosixSlotAllocator` + `PosixSlotError` (Feature) | flatdata-1.0 §4.1 + §7.1 (POSIX-Permissions) | CONNECTED | — |
| `Iceoryx2Publisher<T>` + `Iceoryx2Subscriber<T>` + `Iceoryx2Error` (Feature) | ADR-0004 + zerodds-flatdata-1.0 §4 (alternative Pub/Sub-API gegen Eclipse iceoryx2 v0.8) | CONNECTED | — (F-FLATDATA-iceoryx2-bridge-stub wire-up) |
| `FlatWriter<T>` + `FlatSlot<T>` | flatdata-1.0 §8.1 + §8.2 | CONNECTED | — |
| `FlatReader<T>` + `FlatSampleRef<T>` | flatdata-1.0 §9.1 + §6.1 | CONNECTED | — (F-FLATDATA-pubsub-typehash wire-up) |
| `ShmLocator` + `LocatorError` + `is_same_host` + `fnv1a_32` | flatdata-1.0 §3 (PID_SHM_LOCATOR) | CONNECTED | — |

Ergebnis: **0 ❌-Klassen offen**.

## 4 Wiring

### 4.1 Dependencies

```toml
[dependencies]
shared_memory = { version = "0.12", optional = true }   # nur posix-mmap-Feature
```

Keine ZeroDDS-Crate-Deps.

### 4.2 Dependents

`zerodds-dcps` (Feature `flatdata-integration`); end-user-Builds direkt.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | Standard-Library + Mutex + Threads |
| `alloc` | ✅ (via std) | `Vec`/`Arc` |
| `posix-mmap` | ✅ | `PosixSlotAllocator` |
| `iceoryx2-bridge` | ❌ | Iceoryx2-Adapter |

## 5 Spec-Relevanz

- **Spec(s):** `docs/specs/zerodds-flatdata-1.0.md` §1–§9 (komplett); ADR-0003 (Drei-Backend-Architektur); ADR-0004 (Iceoryx2-Bridge optional).

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

```bash
rg -i -e 'llvm@llvm' -e 'sandra-kessler' -e 'fishermen21' \
  -e '/Users/sandrakessler' -e 'PDE-Spec' -e 'zero-principle' \
  -e 'Ghost-Inject' -e '/tmp/cyc\.xml' \
  crates/flatdata/
```

Treffer: **0**.

### 6.2 Sprint-/Phase-Marker

Pre-Cleanup: 8 Phase-1/2/3-Marker in 4 Files (backend.rs, allocator.rs, pubsub.rs, posix.rs). Post-Cleanup: **0**. Phase-Sprache durch fachliche Beschreibung der drei Backends ersetzt; Phase-3-Cross-Process-Loaning-Marker durch Architektur-Begründung "Loan-API ist Owner-zentrisch" ersetzt.

### 6.3 Datums-Marker

Keine im Source. CHANGELOG.md hat Keep-a-Changelog-Marker (per Guardrails §2.1c erlaubt).

### 6.4 Soft-Review (TODO/FIXME/HACK)

Keine.

### 6.5 Lab-Refs in src/

Keine.

### 6.6 Public-API-Leaks

Keine.

### 6.7 Dead-Code

Keine.

## 7 Cleanup-Actions

1. **F-FLATDATA-pubsub-typehash** (resolved): `FlatReader::read` validiert jetzt `T::TYPE_HASH` gegen `SlotBackend::type_hash()` per Spec §6.1. Bei Mismatch wird kein Slot dereferenziert (Schema-Drift-Schutz). 3 neue Tests: `reader_rejects_type_hash_mismatch`, `reader_accepts_matching_type_hash`, `reader_without_backend_hash_does_not_reject`.
2. **F-FLATDATA-stale-phase-docs** (resolved): 8 stale Phase-1/2/3-Marker in 4 Files durch fachliche Texte ersetzt. backend.rs Modul-Doc beschreibt jetzt die drei produktiven Backends ohne Roadmap-Sprache. allocator.rs erklaert "In-Memory als Referenz-Implementation". posix.rs argumentiert Owner-zentrische Loan-API statt Phase-3-Lock-Free-Allocator-Versprechen. pubsub.rs read-Comment dokumentiert Spec §6.1 Type-Hash-Validation.
3. **F-FLATDATA-iceoryx2-bridge-stub** (resolved): User-Pushback ("müssen wir noch fertig bauen") gegen den ersten Review-Pass deckte auf, dass der `Iceoryx2SlotAdapter` ein klassisches Stub-Artefakt war (`v1.0: delegiert`). Echtes Wire-up: `iceoryx2 = "0.8"` als optional-dep, `Iceoryx2SlotAdapter` ersetzt durch `Iceoryx2Publisher<T>` + `Iceoryx2Subscriber<T>` als separate Pub/Sub-API (kein `SlotBackend`-Match — iceoryx2's FIFO-Modell ist mit dem Random-Access-Slot-Pool-Trait nicht vereinbar). Spec §6.1 Type-Hash-Cross-Validation per Service-Name-Komposition `<base>#<hex(TYPE_HASH)>` realisiert.
4. **SPDX-Headers** in allen 8 src-Files.
5. **Cargo.toml-Metadata**: `homepage`, `documentation`, `readme`, `keywords`, `categories` ergänzt; `publish = true` (keine Embargo-Pfad-Dep).
6. **README.md** + **CHANGELOG.md** in RC1-Form (mit dokumentierter Iceoryx2-Bridge-Architektur).

## 8 Spec-Doc-Updates

Keine — `docs/specs/zerodds-flatdata-1.0.md` bleibt als Spec-Quelle, Crate ist konsistent mit §6.1 Type-Hash-Validation und §8/§9 API-Form.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig
- [x] `lib.rs`-Crate-Header mit Safety-Class + Spec-Ref
- [x] `README.md` auf RC1-Form
- [x] `CHANGELOG.md` mit `[1.0.0-rc.1]`-Entry
- [x] doc-tested Code-Example via Quickstart in README

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-flatdata --all-features          # ✅ 45 passed, 0 failed, 1 ignored
cargo clippy -p zerodds-flatdata --tests --all-features -- -D warnings   # ✅ clean
cargo fmt --all -- --check                             # ✅ clean
cargo doc -p zerodds-flatdata --no-deps                # ✅ clean
cargo run --bin zerodds-lint -- check                  # ✅ workspace clean
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit
- [x] §1.6 Spec-Coverage-Update (kein Update nötig)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File (8 src-Files)
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts (`github/crates/flatdata/` + `github/Cargo.toml` + `github/CHANGELOG.md` + `website/docs/flatdata.md`)
- [x] §1.13 Spec-Conformance-Audit (3 F-FLATDATA-Findings ✅ resolved: pubsub-typehash + stale-phase-docs + iceoryx2-bridge-stub)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅
