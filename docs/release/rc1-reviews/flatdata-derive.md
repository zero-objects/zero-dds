# RC1 Review — `zerodds-flatdata-derive`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 4 (Core Services)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public (`publish = true`).

---

## 1 Purpose

Proc-Macro `#[derive(FlatStruct)]`, der `unsafe impl ::zerodds_flatdata::FlatStruct for T` mit einem deterministischen `TYPE_HASH = sha256(layout_signature(T))[..16]` generiert. Build-Time-Companion zu `zerodds-flatdata`.

## 2 Public-Strategy

- **Marker:** 🌐 public.
- **Begruendung:** End-User die `zerodds-flatdata` von crates.io ziehen, brauchen den Derive auf derselben Quelle. Keine Embargo- oder ZeroDDS-Pfad-Deps.

## 3 Content-Inventur

### 3.1 Module

```
src/
└── lib.rs   # einziges Modul: derive_flat_struct + expand + has_repr_c_or_transparent + layout_signature + type_signature
```

### 3.2 Public-API-Surface

```rust
#[proc_macro_derive(FlatStruct)]
pub fn derive_flat_struct(input: TokenStream) -> TokenStream;
```

Das ist die einzige Public-Surface — proc-macro-Crates exponieren keine Library-Items, nur `proc_macro_*`-Annotationen.

### 3.3 Tests

- `cargo test -p zerodds-flatdata-derive`: ✅ 0 (proc-macro-Crates koennen ihre eigene Output nicht testen — Tests stehen im Companion).
- `cargo test -p zerodds-flatdata --test derive`: ✅ **11 passed**, 0 failed.
  - `derive_generates_wire_size`, `derive_generates_type_hash_nonzero`, `derive_generates_distinct_hash_per_layout`, `derive_as_bytes_roundtrip`, `derive_works_for_tuple_struct`, `derive_works_for_unit_struct` — Smoke-Pfade.
  - `derive_accepts_repr_transparent` — Spec §1.1 erlaubt `repr(transparent)`-Wrapper.
  - `derive_detects_field_reorder`, `derive_detects_field_type_change`, `derive_detects_type_rename` — Schema-Drift-Schutz.
  - `derive_hash_is_deterministic_for_identical_layout` — Type-Identitaet, nicht nur Layout-Identitaet.

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Test-Refs | Klassifikation | Decision |
|---|---|---|---|---|---|
| `#[derive(FlatStruct)]` | zerodds-flatdata-1.0 §1.2 (Derive-Macro) | `crates/flatdata` `dev-dependencies`-Pfad ueber den Companion-Test (`tests/derive.rs`); end-user-Code via crates.io | 11 in `crates/flatdata/tests/derive.rs` | OPTIONAL-HOOK (Spec MAY: Caller koennen alternativ die `unsafe impl` von Hand schreiben) | document-as-hook |

Der Macro hat 0 ZeroDDS-Production-Refs (FlatStruct-Hand-Impls in `crates/flatdata/src/lib.rs::tests` und in `crates/dcps/...flatdata_integration.rs` schreiben den `TYPE_HASH` direkt ein). Das ist Spec-konform: §1.1 dokumentiert die `unsafe impl`-Hand-Form als gleichwertige Pfad. Der Macro ist Convenience-Hook fuer End-User, nicht Pflicht-Wire-Path. Klassifikation analog F-CDR-1 / F-CDR-2.

Ergebnis: **0 ❌-Klassen offen.**

## 4 Wiring

### 4.1 Dependencies (uses)

```toml
[dependencies]
syn = { version = "2", features = ["full"] }
quote = "1"
proc-macro2 = "1"
sha2 = { workspace = true, default-features = false }
```

Keine ZeroDDS-Crate-Deps.

### 4.2 Dependents (used-by)

```bash
$ rg -l 'zerodds-flatdata-derive|zerodds_flatdata_derive' --type rust --type toml -g '!target/' -g '!github/' -g '!Cargo.lock'
crates/flatdata/Cargo.toml          # dev-dependencies
crates/flatdata/tests/derive.rs     # Smoke-Tests
crates/flatdata-derive/Cargo.toml
crates/flatdata-derive/src/lib.rs
```

`zerodds-flatdata` zieht den Derive nur als `dev-dependency`, weil die Macro-Crate-Surface nicht im Runtime-Build der Library noetig ist. End-User koennen sie direkt deklarieren.

### 4.3 Feature-Flags

Keine.

## 5 Spec-Relevanz

- **Spec:** `docs/specs/zerodds-flatdata-1.0.md` §1.2 (Derive-Macro). Spec wurde im selben Pass aktualisiert: Macro-Crate-Name `dcps-derive` → `flatdata-derive`, Compile-Time-Checks (`enum`/`union`-Reject, `repr(C)`/`repr(transparent)`-Pflicht) explizit aufgenommen.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep (Hard, §2.1)

```bash
rg -i -e 'llvm@llvm' -e 'sandra-kessler' -e 'fishermen21' \
  -e '/Users/sandrakessler' -e 'PDE-Spec' -e 'zero-principle' \
  -e 'Ghost-Inject' -e '/tmp/cyc\.xml' -e 'IfynaNeu' -e 'paperless' \
  -e '\bglr1\b' -e '\bglr2\b' \
  crates/flatdata-derive/
```

Treffer: **0**.

### 6.2 Sprint-/Phase-Marker (§2.1b)

```bash
rg -i -e '\bWP[ -]?[0-9]' -e '\bPhase[- ]?[0-9]' \
  -e '\bCluster[- ]?[A-Z0-9]' -e '\bSprint[- ]?[0-9]' \
  crates/flatdata-derive/
```

Treffer: **0**.

### 6.3 Datums-Marker im Source (§2.1c)

Keine im `src/`. CHANGELOG-Eintrag traegt `2026-05-06` (Keep-a-Changelog-Konvention, per Guardrails §2.1c erlaubt).

### 6.4 Soft-Review (TODO/FIXME/HACK)

```bash
rg -i -e 'TODO\b' -e 'FIXME\b' -e 'XXX\b' -e '\bhack\b' \
   crates/flatdata-derive/
```

Treffer: **0**.

### 6.5 Lab-Refs in src/

Keine.

### 6.6 Public-API-Leaks

Keine — die einzige Public-Surface ist `#[proc_macro_derive(FlatStruct)]`.

### 6.7 Tech-Debt + Dead-Code

- **F-FLATDATA-DERIVE-1** (✅ resolved): die alte `lib.rs`-Doc behauptete einen Compile-Time-Check (`Compile-Time-Check: T muss #[repr(C)] oder #[repr(transparent)] sein`), aber der Macro hat das Attribut nie inspiziert. Klassisches "Doc-Lie" / Hidden-TODO (Memory `feedback_no_hidden_todos_full_spec`). Wire-up: `has_repr_c_or_transparent` parst `#[repr(...)]`-Attrs via `parse_nested_meta` und wirft `compile_error!` wenn weder `C` noch `transparent` gesetzt ist. Damit wird die Doc-Promise zur Compile-Time-Garantie.

## 7 Cleanup-Actions

1. **F-FLATDATA-DERIVE-1** (resolved): `repr(C)`/`repr(transparent)`-Attribut-Check als echter Compile-Time-Check via `has_repr_c_or_transparent` ergaenzt.
2. **SPDX-Header** in `src/lib.rs`.
3. **Cargo.toml-Metadata**: `homepage`, `documentation`, `readme`, `keywords`, `categories` ergaenzt; `publish = false` → `publish = true`. Description um Compile-Time-Check-Mention erweitert.
4. **lib.rs Crate-Header**: Safety-Klassifikation, Spec-Ref, Schichten-Position, Public-API-Aufzaehlung, Compile-Time-Check-Sektion, Code-Beispiel — alle per Guardrails §1.2.
5. **Generierter Code**: `#[automatically_derived]` ergaenzt; SAFETY-Kommentar im Macro-Output prazisiert (vorher: "Caller wird vom Compiler erinnert" — falsch; jetzt: "Macro prueft repr und lehnt enum/union ab").
6. **Tests** in `crates/flatdata/tests/derive.rs` von 6 auf 11 erweitert: `repr(transparent)`-Akzeptanz, Field-Reorder, Field-Type-Change, Type-Rename, deterministischer Hash, Unit-Struct.
7. **README.md** (neu) im RC1-Format mit Spec-Mapping, Quickstart, Compile-Time-Check-Erklaerung.
8. **CHANGELOG.md** (neu) mit `[1.0.0-rc.1]`-Initial-Materialisierungs-Eintrag.
9. **Spec-Doc-Update**: `docs/specs/zerodds-flatdata-1.0.md` §1.2 — Macro-Crate-Name `dcps-derive` → `flatdata-derive`; Compile-Time-Checks dokumentiert.

## 8 Spec-Doc-Updates

`docs/specs/zerodds-flatdata-1.0.md` §1.2: Crate-Name korrigiert (`dcps-derive` → `flatdata-derive`); Compile-Time-Checks aufgefuehrt.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollstaendig (homepage, documentation, readme, keywords, categories, publish=true)
- [x] `lib.rs`-Crate-Header mit Safety-Class + Spec-Ref + Layer + Public-API-Aufzaehlung + Compile-Time-Checks + Beispiel
- [x] `README.md` aus RC1-Form
- [x] `CHANGELOG.md` mit `[1.0.0-rc.1]`-Entry
- [x] doc-tested Code-Example: das `ignore`-Beispiel im README + lib.rs ist `ignore` markiert, weil proc-macro-Doctests ohne Companion-Crate nicht kompilierbar sind. Lauffaehiger Beleg ist `crates/flatdata/tests/derive.rs`.

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-flatdata --test derive               # ✅ 11 passed
cargo clippy -p zerodds-flatdata-derive --tests -- -D warnings  # ✅
cargo fmt -p zerodds-flatdata-derive -- --check            # ✅
cargo doc -p zerodds-flatdata-derive --no-deps             # ✅
cargo run --bin zerodds-lint -- check                      # ✅ 105 crates, 1013 files, 0 errors, 0 warnings
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md
- [x] §1.5 Public-API-Audit (einziges Item: `derive_flat_struct`)
- [x] §1.5b Coherence-Audit
- [x] §1.6 Spec-Coverage-Update (zerodds-flatdata-1.0 §1.2 aktualisiert)
- [x] §1.7 Forbidden-Token-Sweep (0 Treffer)
- [x] §1.8 License-Header (1 src-File)
- [x] §1.9 Tests + Lints + Doc-Build gruen
- [x] §1.10 Review-Doc ausgefuellt
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts (`github/crates/flatdata-derive/` + `github/Cargo.toml`-Member + `website/docs/flatdata-derive.md`)
- [x] §1.13 Spec-Conformance-Audit (1 F-FLATDATA-DERIVE-Finding ✅ resolved: repr-attr-check)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅
