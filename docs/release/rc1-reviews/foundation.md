# RC1 Review — `zerodds-foundation`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 0 (Foundation)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public
>
> Track-Materialisierung via git: `git log docs/release/rc1-reviews/foundation.md`.

---

## 1 Purpose

Foundation-Layer-Primitive: Hot-Path-Stack-Buffer, Wire-Integrity-Hashes (CRC-32C / CRC-64-XZ / MD5), strukturierte Observability-Events + Sinks, OpenTelemetry-kompatible Tracing-Spans + Histogramme, Lock-Free-Read RCU-Cell. Pure-Rust, `no_std`-tauglich, `forbid(unsafe_code)`.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Foundation ist die Basis-Schicht und Teil der RC1-Public-API. Alle ZeroDDS-Crates bauen darauf auf; End-User können einzelne Primitives (z.B. CRCs für eigene Wire-Format-Verifikation) direkt verwenden.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs           # Crate-Entry, pub-use Aggregator
├── buffer.rs        # PoolBuffer<CAP> + PoolBufferError
├── crc.rs           # crc32c, crc64_xz, md5
├── observability.rs # Event/Sink/Component/Level + Reference-Sinks
├── rcu.rs           # RcuCell<T>
└── tracing.rs       # Span/SpanContext/Histogram/...
```

### 3.2 Public-API-Surface

```rust
// Stack-Buffer
pub struct PoolBuffer<const CAP: usize>;
pub enum PoolBufferError { Overflow, CapacityTooLarge }

// Hashes
pub fn crc32c(data: &[u8]) -> u32;
pub fn crc64_xz(data: &[u8]) -> u64;
pub fn md5(data: &[u8]) -> [u8; 16];

// Observability
pub struct Event;
pub struct Attribute;
pub enum Level;
pub enum Component;
pub trait Sink: Send + Sync;
pub struct NullSink;
pub struct StderrJsonSink;
pub struct VecSink;
pub type SharedSink;
pub fn null_sink() -> SharedSink;

// Tracing
pub struct Span;
pub struct SpanContext;
pub struct SpanId(pub [u8; 8]);
pub struct TraceId(pub [u8; 16]);
pub enum SpanKind;
pub enum SpanStatus;
pub struct Histogram;

// RCU
pub struct RcuCell<T>;
```

### 3.3 Tests

- `cargo test -p zerodds-foundation`: ✅ alle Tests grün (inkl. 2 Doc-Tests).
- Keine E2E-/Live-Tests — Foundation ist library-only.

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `PoolBuffer` / `PoolBufferError` | Vendor-Extension (Hot-Path) | dcps:1 (small-frame stack pool) | CONNECTED | — |
| `crc32c` | DDSI-RTPS 2.5 §9.4.2.15.2 messageChecksum (RFC 4960 App. B) | rtps:1 (header_extension::ChecksumValue::compute) | CONNECTED | — (F-001 wire-up) |
| `crc64_xz` | DDSI-RTPS 2.5 §9.4.2.15.2 messageChecksum (ECMA-182) | rtps:1 (header_extension::ChecksumValue::compute) | CONNECTED | — (F-001 wire-up) |
| `md5` | RFC 1321 + DDSI-RTPS §9.4.2.15.2 + XTypes §7.3.1.2.1 + §7.3.4.5 + §7.6.8.4 + RTPS §8.3.5.10 | cdr:1 (KeyHash) + types:2 (EquivalenceHash, NameHash) + rtps:1 (GroupDigest) + rtps:1 (header_extension Checksum) | CONNECTED | — (F-002 swap-in) |
| `RcuCell` | Vendor-Primitive (Lock-Free-Read) | rtps:1 (history_cache) | CONNECTED | — |
| `Event` / `Component` / `Level` / `Attribute` | Vendor-Observability | dcps + observability-otlp | CONNECTED | — |
| `Sink` (Trait) + `NullSink` / `StderrJsonSink` / `VecSink` / `SharedSink` / `null_sink` | Plugin-Hook für End-User-Custom-Sinks | dcps + dcps-tests + observability-otlp | CONNECTED + OPTIONAL-HOOK | document-as-hook (für End-User-Custom-Sinks) |
| `Span` / `SpanContext` / `SpanId` / `TraceId` / `SpanKind` / `SpanStatus` / `Histogram` | OpenTelemetry-kompatibel | observability-otlp | CONNECTED | — |
| ~~`BufferPool` / `PoolHandle`~~ | — | 0 | DEAD | drop (F-003) — entfernt |

## 4 Wiring

### 4.1 Dependencies (uses)

```toml
[dependencies]
# (keine — Foundation ist Layer 0, hat keine ZeroDDS-Crate-Deps)
```

### 4.2 Dependents (used-by)

`zerodds-cdr`, `zerodds-types`, `zerodds-rtps`, `zerodds-dcps`, `zerodds-observability-otlp`. Cross-Check via:

```bash
$ rg -l "zerodds-foundation" crates/*/Cargo.toml
crates/cdr/Cargo.toml
crates/types/Cargo.toml
crates/rtps/Cargo.toml
crates/dcps/Cargo.toml
crates/observability-otlp/Cargo.toml
```

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | Aktiviert RcuCell, StderrJsonSink, VecSink. Implies `alloc`. |
| `alloc` | ✅ (via std) | Aktiviert `observability` + `tracing` Module + MD5 mit unbeschränkter Eingabelänge. |
| `safety` | ❌ | Reserviert für zukünftige Safety-Build-Constraints. |

## 5 Spec-Relevanz

Foundation ist kein OMG-Spec-Mapping — es ist eine reine Implementation-Library für Primitives, die andere Crates konsumieren. Indirekte Spec-Referenzen über die gewireten Items:

- DDSI-RTPS 2.5 §9.4.2.15.2 (HEADER_EXTENSION messageChecksum) — via `crc32c`/`crc64_xz`/`md5` in `rtps::header_extension`.
- XTypes 1.3 §7.3.1.2.1 (EquivalenceHash) + §7.3.4.5 (NameHash) — via `md5` in `types::hash` und `types::type_object::common`.
- XTypes 1.3 §7.6.8.4 Step 5.2 (KeyHash MD5-Fallback) — via `md5` in `cdr::key_hash`.
- DDSI-RTPS 2.5 §8.3.5.10 (GroupDigest_t) — via `md5` in `rtps::group_digest`.
- OpenTelemetry-Datenmodell (Spans/Traces) — via `Span`/`SpanContext`/`TraceId`/`SpanId` in `observability-otlp`.

Keine Coverage-Doc unter `docs/spec-coverage/` für Foundation selbst — die Spec-Belege liegen in den jeweiligen Coverage-Docs der konsumierenden Crates.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

```bash
rg -g '!target/' -i \
  -e 'llvm@llvm' -e 'sandra-kessler' -e 'fishermen21' \
  -e '/Users/sandrakessler' -e 'PDE-Spec' -e 'zero_concept' \
  -e 'zero-principle' -e 'Ghost-Inject' -e 'R-09[7-9]' \
  -e 'R-10[0-4]' -e 'R-110' -e '\bseesaw\b' \
  crates/foundation/
```

Treffer: `\\uXXXX` in `observability.rs` (JSON-Escape-String, kein Forbidden-Token — false positive).

### 6.2 Sprint-/WP-/Phase-Marker-Sweep

```bash
rg -i -e '\bWP[ -]?[0-9]' -e '\bPhase[- ]?[0-9]' -e '\bCluster[- ]?[A-Z0-9]' -e '\bSprint[- ]?[0-9]' crates/foundation/
```

Treffer (vor Cleanup): 4 (`WP 5.F.3` in tracing.rs, `Phase-5 D.4 Phase C` in rcu.rs, `Phase-5 F.3 Phase A` in observability.rs, `WP 5.D.1 (Phase-5 Cluster-D)` in buffer.rs). Alle entfernt durch fachliche Umformulierung.

### 6.3 Datums-Marker

Keine.

### 6.4 Soft-Review (TODO/FIXME/HACK)

Keine.

### 6.5 Lab-Refs

Keine.

### 6.6 Public-API-Leaks

Keine — `pub use`-Statements in `lib.rs` listen alle Items explizit.

### 6.7 Dead-Code

`BufferPool` + `PoolHandle` waren seit Sprint-18-Einführung (~150 LOC) ohne externe Verwendung → entfernt (F-003).

## 7 Cleanup-Actions

1. **F-003 drop:** `BufferPool`, `PoolHandle`, `PoolBufferError::PoolExhausted`, `PoolBufferError::SlotPoisoned` aus `buffer.rs` entfernt. `lib.rs::pub use buffer::{BufferPool, PoolHandle}` entfernt. `buffer.rs` von 498 auf 271 LOC reduziert. 6 zugehörige Tests entfernt.
2. **F-001 wire-up:** `ChecksumValue::compute(kind, payload)` und `ChecksumValue::verify(payload)` in `rtps::header_extension` hinzugefügt, die `zerodds_foundation::{crc32c, crc64_xz, md5}` aufrufen. 9 neue Tests (3 RFC/ECMA-Test-Vector-Belege + 6 Round-Trip + Verify-Detect-Tampered).
3. **F-002 swap-in:** 4 use-sites in `crates/{cdr,types,rtps}/` von externer `md-5`-Crate auf `zerodds_foundation::md5` umgestellt. `md-5` aus 3 Crate-Cargo.toml und aus workspace `[workspace.dependencies]` entfernt. Cargo.lock regeneriert.
4. **Sprint-/WP-/Phase-Marker** in 4 Foundation-Source-Files durch fachliche Umformulierung ersetzt.
5. **doc-warning fix** in `crc.rs:17` (broken intra-doc-link `[\`tests\`]` → Klartext).
6. **SPDX-Headers** in allen 6 `src/*.rs`-Dateien als erste 2 Zeilen.
7. **lib.rs Crate-Header** auf RC1-Form expandiert (Layer-Position, Public-API-Aufzählung, Feature-Flags-Tabelle, Doc-Test-Beispiel).
8. **Cargo.toml-Metadata** vervollständigt (description, repository, homepage, documentation, readme, keywords, categories, publish=true).
9. **README.md** auf RC1-Form (Status-Badges, Was-ist-drin, Quickstart, Feature-Flags, Stabilität, Lizenz, Siehe-auch).
10. **CHANGELOG.md** als initial-Materialisierung erstellt (Spec-Referenzen, Public-API-Aufzählung, Implementierungs-Notizen, Architektur-Mapping, Stabilitäts-Statement).

## 8 Spec-Doc-Updates

- `docs/spec-coverage/ddsi-rtps-2.5.md` §9.4.2.15: Repo-Beleg auf `zerodds_foundation::{crc32c, crc64_xz, md5}` umformuliert (vorher `md-5`-Crate-Reference); 9 neue Tests in der Test-Liste eingetragen; Status bleibt `done`.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig
- [x] `lib.rs`-Crate-Header mit Safety-Class + Layer + API-Aufzählung + Doc-Test
- [x] `README.md` auf RC1-Form
- [x] `CHANGELOG.md` mit `[1.0.0-rc.1]`-Eintrag (initial-Materialisierung)
- [x] doc-tested Code-Example in `lib.rs` (PoolBuffer + crc32c)

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-foundation       # ✅ alle Tests + 2 Doc-Tests grün
cargo clippy -p zerodds-foundation --tests -- -D warnings   # ✅
cargo fmt -p zerodds-foundation -- --check                  # ✅
cargo doc -p zerodds-foundation --no-deps                   # ✅ keine Warnungen
cargo run --bin zerodds-lint -- check                       # ✅ workspace 103/989
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md aus Template
- [x] §1.4 CHANGELOG.md mit RC1-Entry (initial-Materialisierung-Format)
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit (Tabelle in §3.4 ausgefüllt, alle ❌ haben Decision durchgeführt)
- [x] §1.6 Spec-Coverage-Update (DDSI-RTPS §9.4.2.15)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt (= dieses Dokument)
- [x] §1.11 Tracker auf ✅
- [x] Findings-Tracker `RC1_FINDINGS.md` aktualisiert (F-001/F-002/F-003 als ✅ resolved)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1` (workspace-version wird beim r1.0.0-Tag global hochgezogen)
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅

(Sign-off-Zeitpunkt = git-commit-Zeitpunkt dieser Datei.)
