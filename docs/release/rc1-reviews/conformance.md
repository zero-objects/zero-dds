# RC1 Review — `zerodds-conformance`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 7 Profiles
> **Reviewer:** claude
> **Public-Strategy:** 🌐 public

## 1 Purpose

Conformance-Test-Vector-Runner: pro externer Conformance-Suite ein Modul, das Spec-Test-Vektoren als Konstanten haelt und gegen die Production-Implementations laufen laesst.

## 2 Public-Strategy

- **Marker:** `🌐 public`
- **Begründung:** Layer-7-Profile-Crate, public-fokussiert.

## 3 Content-Inventur

Siehe `crates/conformance/README.md` + `CHANGELOG.md` + `src/`.

## 4 Coherence-Audit

Alle Public-Items klassifiziert ✅; keine TEST-ONLY/DEAD-Items.

## 5 Spec-Coverage

Multi-Spec Test-Vektoren: Autobahn-WS, h2spec-gRPC, CoAP-PlugTest, OASIS-MQTT, DDS-XML-Cross-Vendor.

## 6 Forbidden-Token-Sweep

- §2.1 Hard-Forbidden: 0 hits ✅
- §2.1b Sprint/Project-Marker: 0 hits ✅
- §2.1c Datums-Marker: 0 hits ✅

## 7 §1.13 Spec-Conformance HARD-BLOCKER

- Inline-Deferral-Marker: 0 hits ✅
- Spec-Section-Coverage: alle relevanten §-Items auf done in den
  zugehörigen Spec-Coverage-Files
- Wire-Konformität: ueber zerodds-dcps + zerodds-rtps Wire-Bytes-Form
- Kohärenz: (a) wired, (b) Konsumenten in Cross-Crate-Tests, (c)
  getestet mit 7 tests

## 8 Tests + Lints + Doc-Build

- `cargo test -p zerodds-conformance`: 7 tests
- `cargo clippy -p zerodds-conformance --tests -- -D warnings`: clean
- License-Header §1.8: alle src/*.rs OK
- Cargo.toml §1.1: vollständig
- README.md §1.3: present
- CHANGELOG.md §1.4: present mit `[1.0.0-rc.1]`-Eintrag

## 9 Sign-off

✅ rc1-ready
