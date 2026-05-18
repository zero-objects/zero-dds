# RC1 Review — `sys`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 6 PSMs / Bindings
> **Reviewer:** claude
> **Public-Strategy:** 🌐 public

## 1 Purpose

Marker-Crate; verweist auf `zerodds-c-api` als die voll spec-konforme C-FFI-Surface. Keine eigenen Symbole.

## 2 Public-Strategy

- **Marker:** `🌐 public`
- **Begründung:** Layer-6 Sprach-Binding, public-fokussiert.

## 3 Content-Inventur

Siehe `crates/sys/README.md` + `CHANGELOG.md` + `src/`.

## 4 Coherence-Audit

Alle Public-Items klassifiziert:
- ✅ CONNECTED via `zerodds-c-api`-FFI (oder via DCPS direkt fuer rs/py)
- Keine TEST-ONLY oder DEAD-Items
- Keine SPEC-MANDATED-OPEN

## 5 Spec-Coverage

- `docs/spec-coverage/zerodds-c-api-1.0.md` (Foundation): 23/23 done
- `docs/spec-coverage/zerodds-listener-callbacks-1.0.md`: 19 done / 5 n/a (rejected)
- `docs/spec-coverage/dds-psm-cxx-1.0.md` (Codegen-Pfad): 104 done / 18 n/a
- Workspace-Konvention; historischer C-FFI-Pfad nun konsolidiert in `zerodds-c-api`

## 6 Forbidden-Token-Sweep

- §2.1 Hard-Forbidden: 0 hits ✅
- §2.1b Sprint/Project-Marker: 0 hits ✅
- §2.1c Datums-Marker: 0 hits ✅

## 7 §1.13 Spec-Conformance HARD-BLOCKER

- Inline-Deferral-Marker: 0 hits ✅
- Spec-Section-Coverage: alle relevanten §-Items auf done in den
  zugehörigen Spec-Coverage-Files
- Wire-Konformität: über `zerodds-c-api`-FFI; Wire-Bytes-Format
  unverändert vom Rust-Core (DCPS) übernommen
- Kohärenz: (a) wired über C-FFI, (b) Konsumenten in Cross-Language-
  Bindings, (c) getestet mit 1 cargo-test

## 8 Tests + Lints + Doc-Build

- `cargo test -p sys`: 1 cargo-test
- License-Header §1.8: alle src/*.rs OK
- Cargo.toml §1.1: vollständig (homepage/doc/readme/keywords/categories)
- README.md §1.3: present
- CHANGELOG.md §1.4: present mit `[1.0.0-rc.1]`-Eintrag

## 9 Sign-off

✅ rc1-ready
