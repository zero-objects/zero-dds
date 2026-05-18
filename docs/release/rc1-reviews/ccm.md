# RC1 Review — `zerodds-ccm`

> **Layer:** 8 (CORBA-Stack, Tier-A) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready

## 1 Purpose

OMG CCM 4.0 (`formal/06-04-01`) §6 Component Model — Equivalent-IDL-Transformation fuer Component / Home / EventType, Components::* Core-Type-Modelle, Lightweight-CCM-Profile-Filter (§13), DDS4CCM-Erweiterungen. `no_std + alloc`, dep `zerodds-idl` (AST-Eingabe).

## 2-3 Inhalt

- 6 src-Files (lib + 5 Module: dds4ccm, lightweight, model, transform, validate).
- 0 tests-Files (Tests inline pro Modul).
- **54 Tests grün** (53 unit + 1 doc).

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_ccm|zerodds-ccm' --type rust crates/ -g '!crates/ccm/**'` → 0 externe Konsumenten heute (corba-ccm-Wrapper ist Tier-A-pending Task #20).

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `model::Cookie` (+ `truncate_to_base`) | §6.5.2.4 | 0 | OPTIONAL-HOOK (Spec-MUST Receptacle-Identifier; benoetigt von jedem CCM-Container) |
| `model::{PortDescription, FacetDescription, ReceptacleDescription, ConsumerDescription, EmitterDescription, SubscriberDescription, PublisherDescription, ConnectionDescription, ConfigValue}` | §6.4.3.3 + §6.5.3 + §6.6.x | 0 | OPTIONAL-HOOK (Components::*-Valuetype-Modell) |
| `transform::{transform_component, transform_home, transform_event_type, scoped_name}` | §6.3.2 + §6.4.1 + §6.7.1 | 0 | OPTIONAL-HOOK (Equivalent-IDL-Codegen-Conformance-Punkt) |
| `lightweight::filter_to_lightweight` + `LightweightFilterError` | §13 | 0 | OPTIONAL-HOOK (LwCCM-Profile-Conformance) |
| `validate::{validate_primary_key, apply_factory_finder_body, InitOp, PrimaryKeyError}` | §6.4.1 | 0 | OPTIONAL-HOOK (Spec-MUST PrimaryKey-Constraints) |
| `dds4ccm::*` (IdlOutputForm + Connector-Datenmodelle) | DDS4CCM 1.1 §6 + Annex A/B | 0 | OPTIONAL-HOOK (DDS4CCM-Connector-Codegen) |

**Klassifikation:** ccm ist eine reine Spec-Implementierung — Spec-MUST-Surface fuer alle CCM-Container-Hosts und Codegen-Konsumenten. Externe Production-Refs werden bei der `corba-ccm`-/`corba-ccm-lib`-RC1-Review (Task #20/#21) hinzukommen, wenn der Wrapper-Layer das Equivalent-IDL einhängt. Aktuell als OPTIONAL-HOOK klassifiziert (Spec-MUST-Plugin-API fuer hosting-Anwendungen), nicht DEAD-as-whole-crate, weil die Crate Spec-mandatorische Equivalent-IDL-Transformation und Components::*-Datenmodell-Implementation ist.

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0 (im Header-Bereich keine Phase/Sprint-/WP-Marker).
- **TODO/FIXME:** 0.
- **Stub-Wortverwendung:** 3 Stellen (dds4ccm.rs Header "Connector-Stub-Layer" / "Stub-Layer fuer Migrations-Tooling" / "Annex A/B — IDL3+ Stub" + lightweight.rs:115 "Stub fuer Erweiterbarkeit") — alle als CORBA-/CCM-/IDL-Codegen-Terminologie ("IDL-Stub", "Spec-Hook") akzeptiert, kein Workflow-Stub. Klassifikation entsprechend §1.7-Audit-Filter.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett (homepage/docs.rs/keywords/categories).
2. lib.rs: Doc-Test mit `Cookie::new` + `truncate_to_base` (Spec §6.5.2.4-Garantie).
3. SPDX bereits auf allen 6 src-Files (pre-existing).
4. README + CHANGELOG nach corba-ir-Pattern.
5. Mirror unter `github/crates/ccm/` (Cargo.toml + CHANGELOG + README + 6 src-Files).
6. `website/docs/ccm.md` mit Frontmatter `layer: 8 / status: rc1-ready`.
7. `github/Cargo.toml` + `github/CHANGELOG.md` Layer-8-Block ergaenzt.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** Equivalent-IDL-Form Spec-treu (Spec §6.3.2 / §6.4.1 / §6.7.1). LwCCM-Filter reduziert nur Member-Operations (Spec §13), nicht die Inheritance-Beziehung. PrimaryKey-Validation Spec §6.4.1.6.
- **(b) Wire-up mit allen Modulen:** OPTIONAL-HOOK extern. Intern voll integriert: `transform`-Output nutzt `model`-Typen; `lightweight`-Filter operiert auf `transform`-Output; `validate`-Constraints sind in `transform_home` eingehängt; `dds4ccm`-Module sind die DDS4CCM-Spezialisierung.
- **(c) Getestet:** 53 Unit-Tests (Cookie-Roundtrip + Components::*-Valuetype-Konstruktion + Equivalent-IDL fuer Component/Home/EventType + LwCCM-Filter + PrimaryKey-Constraints + DDS4CCM-Connector + IdlOutputForm) + 1 Doc-Test.

## 10-12 Gates

- `cargo test`: ✅ 54 (53 unit + 1 doc).
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check` (pro Crate): ✅.
- `cargo doc --no-deps`: ✅.

## RC1-DoD-Status

- §1.1 Cargo.toml ✅ (publish=true + Metadata + keywords + categories)
- §1.2 lib.rs Crate-Header ✅ (mit Doc-Test)
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (Equivalent-IDL + Components::* + LwCCM als OPTIONAL-HOOK fuer CCM-Container/Codegen-Konsumenten)
- §1.6 Spec-Coverage: ✅ (`docs/spec-coverage/omg-ccm-4.0.md` + `docs/spec-coverage/dds4ccm-1.1.md` referenziert; §6 + §13 voll, §7-§16 als `n/a` begruendet)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅ (alle 6 Files SPDX, pre-existing)
- §1.9 Tests/Lints/Doc ✅
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅ (github/crates/ccm + website/docs/ccm.md)
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up-Status: OPTIONAL-HOOK fuer CCM-Container-Konsumenten.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
