# C-API Unsafe Reduction — Design

> Status: Approved
> Scope: `crates/zerodds-c-api/`
> Konsumenten betroffen: keine (ABI bleibt bit-identisch)
> Konsumenten konsumieren: C++ (`crates/cpp/`), C# (`crates/cs/`), TypeScript-Node (`crates/ts-node/`), ROS2 RMW Shim (`crates/rmw-zerodds-shim/`)

## 1 Ziel

Die 1082 `unsafe`-Tokens im `zerodds-c-api`-Crate werden auf ~720 reduziert, indem die fünf wiederkehrenden FFI-Boundary-Pattern in eine zentrale `ffi_helpers`-Newtype-Schicht kapseln. ABI bleibt unverändert. SAFETY-Audit-Fläche schrumpft auf ~5 kuratierte Helper-Sites + ~166 standardisierte Call-Site-Marker.

## 2 Status-Quo

10 369 LOC über 13 Files, 204 `pub unsafe extern "C" fn`, 705 SAFETY-Kommentare auf 893 inline `unsafe`-Vorkommen → ~188 nicht dokumentierte unsafe-Stellen.

### 2.1 Klassifizierung der unsafe-Semantik

| Klasse | Operation | Sites |
|---|---|---:|
| A | `unsafe { &*ptr }` — immutable Deref | 158 |
| B | `*out = …` / `ptr::write(out, …)` — Out-Pointer | 57 |
| C | `Box::into_raw` / `Box::from_raw` — Lifecycle | 68 |
| D | `slice::from_raw_parts{,_mut}` — Byte-Buffer | 24 |
| E | `CStr::from_ptr(...).to_str()` — C-String | 15 |
| _Restkategorien_ | `transmute`, `ptr.cast` | **0** |

Null `transmute` und null `.cast()` bedeutet: der Code ist semantisch sauber, alle unsafe-Ops haben sichere Rust-Äquivalente. Es ist ein Boilerplate-Problem, kein Typ-Punning-Problem.

### 2.2 Drei Datei-Gruppen

- **Gruppe I (Boilerplate, reduzierbar)** — 9 Files, 862 Tokens, 80% der Masse: `extra_ffi`, `subscriber_ffi`, `participant_ffi`, `publisher_ffi`, `condition_ffi`, `lib`, `factory_ffi`, `topic_ffi`, `builtin_ffi`.
- **Gruppe II (Function-Pointer, irreduzibel unsafe)** — 2 Files, 120 Tokens: `listener_ffi` (C→Rust→C-Callbacks), `xcdr2` (Codegen-Function-Pointer-Calls). Rust hat keine sichere Wrapper-Form für indirekte fn-Pointer-Calls.
- **Gruppe III (Interner Konversion-Layer, bereits gut)** — 2 Files, 100 Tokens: `qos_ffi`, `entities`. Kein Touch.

### 2.3 Multiplikator-Achsen

- **M1 Pro-Argument**: typische fn hat 3-5 Pointer-Args → 3-5 unsafe-Sites pro fn
- **M2 Roundtrip-Spiegelung**: QoS-`get`/`set` × 6 Entity-Typen = 48 nahezu identische fns
- **M3 Container-Iteration**: outer Deref + Lock + per-Element-Deref in Lookup-fns
- **M4 Spec-Vendor-Operationen**: read/take × {_instance, _next_instance, _w_condition} × {Bytes,Shape} = 20 read/take-Varianten

### 2.4 Selbst-Referenzen

- `_w_timestamp`-Varianten delegieren oft 1:1 an Non-Timestamp-Sibling — werden nach Migration trivial safe
- Inter-FFI-Aufrufe (`zerodds_dr_take_w_condition` ruft `zerodds_dr_take`) → können als interne safe Helpers parallel zur extern fn angelegt werden
- `listener_ffi`-Callbacks und `xcdr2`-Codegen-Pointer sind echte unvermeidbare Self-References — bleiben Gruppe II

## 3 Architektur

### 3.1 Module-Layout

Neu: `crates/zerodds-c-api/src/ffi_helpers.rs` (~200 LOC, `pub(crate)`).

Niemals in `cbindgen` exponiert, nie Teil der `zerodds.h`-API. ABI bleibt bit-identisch.

### 3.2 Newtype-Familie

```rust
// Klasse A — Read-Borrow
pub(crate) struct Borrowed<'a, T: 'a>(&'a T);
impl<'a, T> Borrowed<'a, T> {
    pub unsafe fn from_raw(ptr: *const T) -> Result<Self, ZeroDdsStatus>;
}
impl<T> Deref for Borrowed<'_, T> { ... }

// Klasse B — Write-Only Out-Pointer (konsumierend → single-write enforced)
pub(crate) struct OutPtr<T>(NonNull<T>);
impl<T> OutPtr<T> {
    pub unsafe fn from_raw(ptr: *mut T) -> Result<Self, ZeroDdsStatus>;
    pub fn write(self, value: T);
}

// Klasse C — Lifecycle-Owned Box
pub(crate) struct Owned<T>(Box<T>);
impl<T> Owned<T> {
    pub fn new(v: T) -> Self;
    pub fn into_raw(self) -> *mut T;
    pub unsafe fn from_raw_drop(ptr: *mut T);
}

// Klasse D — Byte-Buffer
pub(crate) struct BytesIn<'a>(&'a [u8]);
pub(crate) struct BytesOut<'a>(&'a mut [u8]);
impl<'a> BytesIn<'a>  { pub unsafe fn from_raw(p: *const u8, n: usize) -> Result<Self, ZeroDdsStatus>; }
impl<'a> BytesOut<'a> { pub unsafe fn from_raw(p: *mut u8,   n: usize) -> Result<Self, ZeroDdsStatus>; }

// Klasse E — C-String
pub(crate) struct CStrIn<'a>(&'a str);
impl<'a> CStrIn<'a> {
    pub unsafe fn from_raw(p: *const c_char) -> Result<Self, ZeroDdsStatus>;
}

// Status-Adapter Result<T, Status> → c_int
pub(crate) fn status<T>(r: Result<T, ZeroDdsStatus>) -> c_int;
```

**Begründungen:**
- `from_raw`-Methoden bleiben `unsafe fn`: Caller-Pledge ist nicht statisch beweisbar
- `Deref`-Impls lassen die Wrapper für Read-Pfade transparent verschwinden
- `OutPtr::write` konsumiert `self` → ein Write per Pointer am Typsystem garantiert
- `Owned::from_raw_drop` ist `unsafe` weil double-free verhindert werden muss (Caller-Vertrag: stammt aus `Owned::into_raw`)
- Alle Helpers `pub(crate)` → keine API-Oberfläche, keine SemVer-Pflicht

### 3.3 Body-Pattern

**Eine `unsafe { … }` Closure-Box pro extern fn**, danach reines safe Rust:

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_register_instance(
    dw: *mut ZeroDdsDataWriter, key: *const u8, key_len: usize, out_handle: *mut u64,
) -> c_int {
    if key_len != 16 { return ZeroDdsStatus::BadParameter as c_int; }
    // SAFETY: see fn # Safety doc — caller pledges validity for dw, key, out_handle.
    status(unsafe {
        (|| -> Result<(), ZeroDdsStatus> {
            let _dw = Borrowed::from_raw(dw)?;
            let key = BytesIn::from_raw(key, 16)?;
            let out = OutPtr::from_raw(out_handle)?;
            let mut h = [0u8; 8];
            h.copy_from_slice(&key[..8]);
            out.write(u64::from_le_bytes(h));
            Ok(())
        })()
    })
}
```

**Pattern-Regeln:**
- Genau ein `unsafe { … }` Block pro fn
- Closure liefert `Result<(), ZeroDdsStatus>` → `?` propagiert sauber
- Nicht-Pointer-Validierungen (Längen, Range-Checks) vor dem unsafe-Block
- Body innerhalb der Closure ist reines safe Rust
- `#![allow(unsafe_op_in_unsafe_fn)]` pro Datei wird gestrichen — Pattern erfüllt den Lint

## 4 SAFETY-Conventions (Drei-Schichten-Vertrag)

| Schicht | Wo | Form |
|---|---|---|
| Crate-Level | `src/lib.rs` Top-Doc | Zentraler "FFI-Boundary"-Vertrag: NULL-Tolerant, lebt für Dauer der fn, kein aliased mutation |
| Helper-Level | `ffi_helpers.rs` per Newtype | Präzise `# Safety`-Pledge-Form pro `from_raw`; Body-SAFETY-Kommentare zentral (~5) |
| Call-Site-Level | jede extern fn | `# Safety`-Doc verweist auf Crate-Level + listet Args; Body trägt `// SAFETY: see fn # Safety doc` |

→ 5 Helper-SAFETY (zentral, auditiert) + ~166 Call-Site-SAFETY (standardisiert) + 1 Crate-Vertrag, statt heute 705 verstreuter SAFETY-Kommentare mit ~188 Lücken.

`unsafe_op_in_unsafe_fn` wird per Workspace `[lints.rust]` zu `deny` — Compile-Zeit-Gate gegen Regression.

## 5 Migrations-Phasen

```
Phase 1 — ffi_helpers.rs + Unit-Tests
  Files: src/ffi_helpers.rs (~200 LOC), tests/ffi_helpers.rs (~150 LOC)
  cargo test -p zerodds-c-api → 63+N grün
  Commit: feat(c-api): ffi_helpers Newtype-Schicht für unsafe-Reduktion

Phase 2 — extra_ffi.rs (Exemplar, größter Payoff)
  48 fns migrieren; #![allow(unsafe_op_in_unsafe_fn)] streichen
  Erwartet: 230 → ~95 Tokens (-58%)
  Commit: refactor(c-api): extra_ffi auf ffi_helpers — 230→95 unsafe-Tokens

Phase 3 — Wave 1: subscriber, participant, publisher
  Drei separate Commits, pro File einer
  Erwartet gesamt: 350 → ~140 Tokens (-60%)

Phase 4 — Wave 2: condition, lib, factory, topic, builtin
  Fünf separate Commits
  Erwartet: 282 → ~115 Tokens (-59%)

Phase 5 — Group-II Konsolidierung (listener, xcdr2)
  KEIN Newtype-Migration. Function-Pointer-Calls bleiben unsafe.
  Aber: Sub-Modul fn_ptr_safety mit kuratierten Per-Call-Site-SAFETY-Beweisen
  Erwartet: 120 → ~110 Tokens (-8%, Doku-Konsolidierung)

Phase 6 — GitLab-Push + CI-Verifikation
  Branch: feat/c-api-unsafe-reduction
  Pre-Push lokal: cargo fmt -p zerodds-c-api, clippy -p, test -p
  Push gitlab; CI grün abwarten (alte Pipeline ggf. canceln)
  Bei rot: Root-Cause fixen, kein --no-verify

Phase 7 — Re-Baseline-Audit
  Token-Count-Sweep mit derselben Klassifizierung
  Diff-Tabelle vorher/nachher pro File
  Verifizieren: 5 SAFETY-Helper, ~166 Call-Site-SAFETY, keine Lücken
  Sign-off-Doc: docs/release/c-api-unsafe-audit-<datum>.md
  Falls neue Hotspots auftauchen → 2. Iterations-Plan
```

**Erwartete End-Zahlen** (von 1082 heute):
- Nach Phase 4: ~730 Tokens (Gruppe I + III)
- Nach Phase 5: ~720 Tokens
- Restmenge: 182 fn-Signaturen + 182 `#[unsafe(no_mangle)]` + 1 Helper-Call pro Pointer-Arg + Gruppe II + Gruppe III

## 6 Tests + ABI

**Neu** — `tests/ffi_helpers.rs`:
- `borrowed_null_returns_bad_handle`, `borrowed_valid_derefs`
- `outptr_writes_exactly_once`
- `owned_roundtrip_drops_on_from_raw`
- `bytesin_null_empty_len_zero_ok`, `bytesin_null_with_len_errors`
- `cstrin_utf8_validates`, `cstrin_invalid_utf8_errors`

**Miri-Pflicht** für ffi_helpers:
```
cargo +nightly miri test -p zerodds-c-api --test ffi_helpers
```

**Bestehende Tests bleiben Regression-Guard:**
- 63 cargo-tests in `crates/zerodds-c-api/src/*::tests`
- `tests/abi_compat.rs` + `abi.snapshot.json` (185 Symbole) → muss unverändert grün
- `tests/smoke_ffi.rs` E2E
- `tests/xcdr2_wire_vectors.rs` 79 tests
- `crates/cpp/tests/smoke_dds_psm.cpp` C++-Binding
- `crates/cs/csharp/ZeroDDS.Tests/Program.cs` C#-P/Invoke

## 7 Risk + Rollback

| Risk | Wkt | Mitigation |
|---|---|---|
| Lifetime `'a` leakt in Call-Site-Sig | mittel | `Borrowed<'static, T>` als Default — Caller-Pledge für Dauer der fn |
| Closure + `?` Codegen-Overhead | niedrig | inline'd, identischer Output zur Hand-Variante |
| `Owned::from_raw_drop` double-free | niedrig | Unit-Test + Miri-Roundtrip |
| ABI-Snapshot bricht | sehr niedrig | Helpers `pub(crate)`, cbindgen unsichtbar |
| C++/C#/TS-Bindings brechen | sehr niedrig | `zerodds.h` ändert sich nicht |

**Rollback** pro Phase: jedes Commit kann einzeln `git revert`-werden — keine Cross-File-Dependencies.

## 8 Working-Tree-Disziplin

Parallel-Agents arbeiten gleichzeitig im Repo. Regeln:
- `git add` immer mit konkreten Pfaden unter `crates/zerodds-c-api/`
- nie `git add -A` / `git add .`
- `cargo fmt -p zerodds-c-api`, nie `cargo fmt --all`
- WT-Noise anderer Agents nicht anfassen
