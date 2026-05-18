# RC1 Review — `zerodds-corba-codegen`

> **Layer:** 8 (CORBA-Stack, Tier-A) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready (`build_repository_id` jetzt CONNECTED via 4 corba-rust-Refs; F-CORBA-CODEGEN-NOT-WIRED ✅ resolved)

## 1 Purpose

OMG CORBA 3.3 Annex-A.1 IDL-Mapping-Codegen-Helpers — Tabellen + Helper, die die drei OMG-PSM-Crates (idl-cpp / idl-csharp / idl-java) und `corba-rust` zur Erzeugung CORBA-Stub-/Skeleton-Codes konsumieren.

## 2-3 Inhalt

- 5 src-Files (lib, repository_id, skeleton, special_types, stub).
- 0 tests-Files (Tests inline in src/).
- **17 Tests grün** (16 unit + 1 doc).

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg -l 'zerodds-corba-codegen'` workspace-weit + `rg 'build_repository_id'` für Production-Refs.

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `SpecialType` / `TargetLanguage` / `language_mapping` | OMG CORBA 3.3 Part 1 §7.15 + Annex-A.1 | 0 (Plugin-API für externes Codegen-Tooling) | OPTIONAL-HOOK |
| `build_repository_id` | §10.7.3.1 + §7.15 | **4** in `crates/corba-rust/src/{interface,valuetype,component}_emit.rs` | **CONNECTED** ✅ |
| `StubOp` / `render_stub_op` | Annex-A.1 Operation-Mapping | 0 (Plugin-API) | OPTIONAL-HOOK |
| `SkeletonOp` / `render_skeleton_dispatch` | Annex-A.1 Server-Side-Dispatch | 0 (Plugin-API) | OPTIONAL-HOOK |

**Wire-up:** `build_repository_id` ist nach Resolve von `F-CORBA-CODEGEN-NOT-WIRED` **CONNECTED** — `corba-rust` ersetzt seine 4 inline `format!("IDL:{name}:1.0")`-Patterns durch den Spec-konformen Builder. Die Stub/Skeleton-Templates und Special-Types-Tabelle bleiben Plugin-API für externes Codegen-Tooling (1× CONNECTED + 3× OPTIONAL-HOOK = ✅ per §1.5b).

**Finding:** `F-CORBA-CODEGEN-NOT-WIRED` ✅ resolved.

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0 (Crate war pre-Review pristine).
- **TODO/FIXME/Stub:** 0.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: SPDX + RC1-Header + Quickstart-Doc-Test.
3. SPDX auf alle 5 src-Files.
4. README + CHANGELOG.
5. Mirror unter `github/crates/corba-codegen/`.
6. `website/docs/corba-codegen.md`.
7. `github/Cargo.toml` + `github/CHANGELOG.md` ergaenzt.
8. **Wire-up (post-self-audit):** `build_repository_id` in 4 `corba-rust`-Files eingebaut; ersetzt vorherige inline `format!`-Patterns.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** Annex-A.1-Mapping-Tabelle ist Spec-konform; `build_repository_id` produziert das Spec-§10.7.3.1-Format `IDL:<scoped-name>:<major>.<minor>`. Tests verifizieren Roundtrip.
- **(b) Wire-up mit allen Modulen:** ✅ — `build_repository_id` jetzt 4× in `corba-rust` (interface/valuetype/component/home-emit) genutzt; ersetzt vorherige inline `format!("IDL:{name}:1.0")`-Patterns. Stub/Skeleton-Templates bleiben Plugin-API für externes Codegen-Tooling (OPTIONAL-HOOK explizit dokumentiert).
- **(c) Getestet:** 16 Unit-Tests + 1 Doc-Test (Repository-ID Format-Check); `corba-rust` 13 Tests grün nach Wire-up (Snapshot-Tests unveraendert, Format identisch zum vorherigen `format!`-Output).

## 10-12 Gates

- `cargo test`: ✅ 17 (16 unit + 1 doc).
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check`: ✅.
- `cargo doc --no-deps`: ✅.
- `cargo run --bin zerodds-lint -- check` (workspace-weit): ⚠️ 21 errors in `zerodds-c-api/src/factory_ffi.rs` (nicht von dieser Crate verursacht; pre-existing in einer noch-nicht-RC1-Crate).

## RC1-DoD-Status

- §1.1 Cargo.toml ✅
- §1.2 lib.rs Crate-Header ✅
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (`build_repository_id` CONNECTED via 4 corba-rust-Refs; F-CORBA-CODEGEN-NOT-WIRED ✅ resolved)
- §1.6 Spec-Coverage: ✅ (`corba-3.3.md` Crate-Mapping referenziert; alle Sektionen done)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅
- §1.9 Tests/Lints/Doc ✅; zerodds-lint workspace ⚠️ (Reibach aus `zerodds-c-api`)
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up: ✅ resolved.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
