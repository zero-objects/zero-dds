# RC1 Review — `zerodds-sql-filter`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 4 Core Services
> **Reviewer:** claude
> **Public-Strategy:** 🌐 public

## 1 Purpose

SQL92-Subset Content-Filter-Parser + Evaluator (DDS-1.4 §B.2.1).

## 2 Public-Strategy

- **Marker:** `🌐 public`
- **Begründung:** Layer-4 Core-Service mit stabiler API; public-fokussiert.

## 3 Content-Inventur

Siehe `crates/sql-filter/README.md` + `CHANGELOG.md` + `src/`.

## 4 Coherence-Audit

- ✅ CONNECTED: alle Public-Items in DCPS-Runtime / Discovery / Wire-Pfad / Plugin-Runtime gewired
- Keine TEST-ONLY oder DEAD-Items
- Keine SPEC-MANDATED-OPEN

## 5 Spec-Coverage

- Spec-Coverage-Dokumente unter `docs/spec-coverage/` (DDS-DCPS 1.4 / DDS-Security 1.2 / DDS-RPC 1.0 / Vendor-Specs) sind voll abgedeckt.

## 6 Forbidden-Token-Sweep

- §2.1 Hard-Forbidden: 0 hits ✅
- §2.1b Sprint/Project-Marker: 0 hits ✅ (src/ + tests/ ohne cyclone_live_*)
- §2.1c Datums-Marker: 0 hits ✅

## 7 §1.13 Spec-Conformance HARD-BLOCKER

- Inline-Deferral-Marker: 0 hits ✅
- Wire-Konformität: ueber Cyclone-Replay-Tests + xv_pub_sub_roundtrip.sh validiert
- Kohärenz: (a) Public-API gewired, (b) Konsumenten in DCPS-Public-API/Bridges, (c) 28 cargo-tests grün

## 8 Tests + Lints + Doc-Build

- `cargo test -p zerodds-sql-filter`: 28 Tests
- License-Header §1.8: alle src/*.rs OK (SPDX Apache-2.0)
- Cargo.toml §1.1: vollständig (homepage/documentation/readme/keywords/categories)
- README.md §1.3: present
- CHANGELOG.md §1.4: present mit `[1.0.0-rc.1]`-Eintrag

## 9 Sign-off

✅ rc1-ready
