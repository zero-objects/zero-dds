# RC1 Review — `zerodds-ami4ccm`

> **Layer:** 8 (CORBA-Stack, Tier-A) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready

## 1 Purpose

OMG AMI4CCM 1.1 (`formal/2015-08-03`) — Async-Method-Invocation fuer das CORBA Component Model. Implied-IDL-Transformation (Spec §7.3 + §7.5), ExceptionHolder-Datenmodell (§7.4), Pragma-Parsing (§7.7), Connector-/Deployment-Modelle (§7.6 + §7.8). `no_std + alloc`, dep `zerodds-idl` (AST-Eingabe).

## 2-3 Inhalt

- 8 src-Files (lib + 7 Module: connector, deployment, exception_holder, multiplex, pragma, scope_resolver, transform).
- 0 tests-Files (Tests inline pro Modul).
- **51 Tests grün** (50 unit + 1 doc).

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_ami4ccm|zerodds-ami4ccm' --type rust crates/ -g '!crates/ami4ccm/**'` → 0 externe Konsumenten heute.

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `pragma::Ami4CcmPragma` + `parse_pragma` + `ParsePragmaError` | §7.7 | 0 (CCM-Container-Konsumenten pending) | OPTIONAL-HOOK (Pragma-Pre-Processor-Plugin-API; Spec-MUST-Surface fuer hosting-Anwendungen) |
| `transform::transform_interface*` + `Ami4CcmInterfaces` + `TransformContext` | §7.3 + §7.5 | 0 | OPTIONAL-HOOK (Implied-IDL-Codegen-Hook; Spec-MUST-Conformance-Punkt 1) |
| `exception_holder::ExceptionHolder` + `UserExceptionBase` | §7.4.1 | 0 | OPTIONAL-HOOK (Spec-MUST Datenmodell fuer Exception-Lieferung) |
| `connector::{Connector, ConnectorPort, Facet, PortType}` | §7.6 | 0 | OPTIONAL-HOOK (Connector-Hosting-Modell fuer CCM-Container) |
| `deployment::{ConnectorImplementation, ConnectorPlanFragment, ImplementationDescriptor, PlanInstance}` | §7.8 | 0 | OPTIONAL-HOOK (D&C-Plan-Fragment-Schema fuer Deployment-Tooling) |
| `multiplex::{ReceptacleArity, context_method_for_receptacle, sequence_typedef_for_interface}` | §7.5 + §6.5 (Receptacles) | 0 | OPTIONAL-HOOK (Multi-Receptacle-Codegen-Helper) |
| `scope_resolver::{populate_from_specification, context_from_specification}` | §7.7 + §6 | 0 | OPTIONAL-HOOK (Cross-Module-Type-Resolution fuer Pragma-Konsumenten) |

**Klassifikation:** ami4ccm ist eine reine Spec-Implementierung — Spec-MUST-Surface fuer alle CCM-Container-/Codegen-Konsumenten. Externe Production-Refs werden bei der `corba-ccm`-/`corba-ccm-lib`-RC1-Review (Tasks #20/#21) hinzukommen, wenn der Container-Wrapper-Layer das Implied-IDL einhängt. Aktuell als OPTIONAL-HOOK klassifiziert (Spec-MAY-Plugin-API), nicht DEAD-as-whole-crate, weil die Crate Spec-mandatorische Implied-IDL-Transformation und Pragma-Parser-Implementation ist (AMI4CCM-Conformance-Punkt 1 voll).

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0 (lib.rs-Header-Wort "Top-Level-Sprint" ersetzt durch "Top-Level-Vorhaben").
- **TODO/FIXME/Stub:** 0 (kein Workflow-Stub im Code; "Stub" als CORBA-Codegen-Terminus akzeptiert).

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett (homepage/docs.rs/keywords/categories).
2. lib.rs: Doc-Test mit `parse_pragma` + `Ami4CcmPragma::Interface`.
3. SPDX bereits auf allen 8 src-Files (pre-existing).
4. README + CHANGELOG nach corba-ir-Pattern.
5. Mirror unter `github/crates/ami4ccm/` (Cargo.toml + CHANGELOG + README + 8 src-Files).
6. `website/docs/ami4ccm.md` mit Frontmatter `layer: 8 / status: rc1-ready`.
7. `github/Cargo.toml` + `github/CHANGELOG.md` Layer-8-Block ergaenzt.
8. lib.rs: "Top-Level-Sprint"-Wort durch "Top-Level-Vorhaben" ersetzt.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** AMI4CCM-Implied-IDL-Form Spec-treu (Spec §7.3.1 `sendc_<op>`-Praefix, Spec §7.5 ReplyHandler-Callbacks + `_excep`-Operations). Pragma-Tag-Set vollstaendig (`interface` + `receptacle`).
- **(b) Wire-up mit allen Modulen:** OPTIONAL-HOOK extern (Container-/Codegen-Konsumenten kommen). Intern voll integriert: `transform_interface` nutzt `pragma`-Output via `scope_resolver`-Kontext; `multiplex`-Helper an `transform`-Output angeschlossen.
- **(c) Getestet:** 50 Unit-Tests (Pragma-Roundtrip + Whitespace + 4 Error-Pfade + Implied-IDL-Operations + ReplyHandler + ExceptionHolder + Multiplex-Receptacle-Codegen + Connector-/Deployment-Konstruktion) + 1 Doc-Test.

## 10-12 Gates

- `cargo test`: ✅ 51 (50 unit + 1 doc).
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check` (pro Crate): ✅.
- `cargo doc --no-deps`: ✅.

## RC1-DoD-Status

- §1.1 Cargo.toml ✅ (publish=true + Metadata + keywords + categories)
- §1.2 lib.rs Crate-Header ✅ (mit Doc-Test)
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (Implied-IDL + Pragma + Connector als OPTIONAL-HOOK fuer CCM-Container-Konsumenten)
- §1.6 Spec-Coverage: ✅ (`docs/spec-coverage/omg-ami4ccm-1.1.md` referenziert; Conformance-Punkt 1 voll, Punkt 2 Modell-Ebene)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅ (alle 8 Files SPDX, pre-existing)
- §1.9 Tests/Lints/Doc ✅
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅ (github/crates/ami4ccm + website/docs/ami4ccm.md)
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up-Status: OPTIONAL-HOOK fuer CCM-Container-Konsumenten.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
