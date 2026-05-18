# RC1 Review — `zerodds-corba-ccm-ejb`

> **Layer:** 8 (CORBA-Stack, Tier-A) | **Reviewer:** claude | **Public-Strategy:** 🌐 public
> **Status:** ✅ rc1-ready

## 1 Purpose

CCM↔EJB-Bridge auf Modell-Ebene: bijektives `CosTransactions::Status` ↔ `javax.transaction.Status`-Mapping (CCM 4.0 §16 + JEE JTA 1.3 §3.2), ConnectorBean-Lifecycle, JNDI↔CosNaming-Glue, Java-CCM-Bean-Stub-Codegen (CCM 4.0 Annex A Java-PSM). `no_std + alloc`, keine Workspace-Deps (Substrate-Crate).

## 2-3 Inhalt

- 5 src-Files (lib + 4 Module: connector_bean, naming_glue, stub_gen, tx).
- 0 tests-Files (Tests inline pro Modul).
- **25 Tests grün** (24 unit + 1 doc).

## 3.4 Coherence-Audit (Cross-Crate × Spec)

**Verifizierung:** `rg 'zerodds_corba_ccm_ejb|zerodds-corba-ccm-ejb' --type rust crates/ -g '!crates/corba-ccm-ejb/**'` → 0 externe Konsumenten heute (JEE-Container-Konsumenten leben extern in Java-Schicht).

| Item-Familie | Spec-Anker | External Production-Refs | Klassifikation |
|---|---|---|---|
| `tx::TxStatus` (10 Werte) + `JtaStatus` (10 Werte) | OMG TS 1.4 §10 + JEE JTA 1.3 §3.2 | 0 | OPTIONAL-HOOK (Spec-MUST Status-Enum-Modell fuer Bridge-Hosts) |
| `tx::jta_status_from_cos` + `jta_status_to_cos` | CCM 4.0 §16 (Equivalents) | 0 | OPTIONAL-HOOK (bijektive Mapping-API) |
| `tx::TxBridge`-Trait + `InMemoryTxBridge`-Impl | CCM 4.0 §16 + OMG TS 1.4 §10 | 0 | OPTIONAL-HOOK (Plugin-API fuer Container-Vendoren; In-Memory-Impl als Test-Hosting) |
| `connector_bean::{ConnectorBean, LifecycleCallback, LifecyclePhase}` | CCM 4.0 §16 (CCM↔EJB Lifecycle-Equivalents) | 0 | OPTIONAL-HOOK (JEE-Bean-Lifecycle-Mapping fuer Container-Vendoren) |
| `stub_gen::{generate_bean_stub, StubKind}` | CCM 4.0 Annex A Java-PSM | 0 | OPTIONAL-HOOK (Java-Codegen-Hook fuer Bean-Stubs) |
| `naming_glue::{JndiContext, JndiBinding, cos_naming_to_jndi, jndi_to_cos_naming}` | JNDI 1.2 + CosNaming 1.3 | 0 | OPTIONAL-HOOK (Namespace-Mapping fuer JEE-Hosts) |

**Klassifikation:** corba-ccm-ejb ist eine reine Bridge-Modell-Library — Spec-MUST-Surface fuer JEE-Container-Hosts (JBoss EAP, WildFly, GlassFish, Open Liberty), die CCM-Components per JNI in einen JEE-Container deployen oder umgekehrt EJBs als CCM-Receptacle exposen. Externe Production-Refs leben definitionsgemaess in vendor-spezifischen JNI-Schichten ausserhalb des ZeroDDS-Workspaces. Aktuell als OPTIONAL-HOOK klassifiziert (Plugin-API fuer Container-Vendoren), nicht DEAD-as-whole-crate, weil die Crate Spec-mandatorische Bridge-Implementation (CCM 4.0 §16 + JTA 1.3 §3.2) ist.

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0 (lib.rs-Header-Marker `(Phase-3 Sprint-2 WP #25)` entfernt).
- **TODO/FIXME:** 0.
- **Stub-Wortverwendung:** `stub_gen` Modul-Name + `StubKind`/`generate_bean_stub` API + Bean-Stub-Codegen-Output sind alle CORBA-/CCM-Annex-A-Java-PSM-Standardterminologie ("IDL-Stub", "Bean-Stub"); kein Workflow-Stub.

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett (homepage/docs.rs/keywords/categories).
2. lib.rs: Doc-Test mit `jta_status_from_cos(TxStatus::Active)` ↔ `JtaStatus::Active`.
3. SPDX bereits auf allen 5 src-Files (pre-existing).
4. README + CHANGELOG nach corba-ir-Pattern.
5. Mirror unter `github/crates/corba-ccm-ejb/` (Cargo.toml + CHANGELOG + README + 5 src-Files).
6. `website/docs/corba-ccm-ejb.md` mit Frontmatter `layer: 8 / status: rc1-ready`.
7. `github/Cargo.toml` + `github/CHANGELOG.md` Layer-8-Block ergaenzt.
8. lib.rs: `(Phase-3 Sprint-2 WP #25)` aus Crate-Header entfernt.

## §1.13 Drei-Punkte-Kohärenz

- **(a) Wire + Semantik kohärent:** `TxStatus` ↔ `JtaStatus` bijektiv (10:10), Roundtrip-Garantie bewiesen via Tests `jta_status_to_cos(jta_status_from_cos(s)) == s` ∀ s. CosNaming↔JNDI-Mapping erhaelt Sub-Context-Hierarchie. ConnectorBean-Lifecycle-Phasen Spec §16-konform.
- **(b) Wire-up mit allen Modulen:** OPTIONAL-HOOK extern (JEE-Container leben ausserhalb des Workspaces). Intern voll integriert: `tx::TxBridge`-Trait kapselt das Mapping; `connector_bean` greift Lifecycle-Phasen aus `tx`-Modell; `stub_gen` emittiert Java-Source mit JTA-Annotations; `naming_glue` ist standalone JNDI-Glue.
- **(c) Getestet:** 24 Unit-Tests (Status-Mapping-Roundtrip 10x + Bidirektional + ConnectorBean-Lifecycle-Phasen + Bean-Stub-Generation 4 Varianten + JNDI-Roundtrip + Sub-Context-Hierarchie + Edge-Cases) + 1 Doc-Test.

## 10-12 Gates

- `cargo test`: ✅ 25 (24 unit + 1 doc).
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check` (pro Crate): ✅.
- `cargo doc --no-deps`: ✅.

## RC1-DoD-Status

- §1.1 Cargo.toml ✅ (publish=true + Metadata + keywords + categories)
- §1.2 lib.rs Crate-Header ✅ (mit Doc-Test; Sprint-Marker raus)
- §1.3 README ✅
- §1.4 CHANGELOG ✅
- §1.5b Coherence-Audit: ✅ (Bridge als OPTIONAL-HOOK fuer JEE-Container-Vendoren)
- §1.6 Spec-Coverage: ✅ (CCM 4.0 §16 + OMG TS 1.4 §10 + JEE JTA 1.3 §3.2 + JNDI 1.2 referenziert)
- §1.7 Forbidden-Sweep ✅
- §1.8 License-Header ✅ (alle 5 Files SPDX, pre-existing)
- §1.9 Tests/Lints/Doc ✅
- §1.10 Review-Doc ✅
- §1.12 Public-Mirror ✅ (github/crates/corba-ccm-ejb + website/docs/corba-ccm-ejb.md)
- §1.13 Inline-Deferral-Sweep ✅; Drei-Punkte-Liste ✅; Wire-up-Status: OPTIONAL-HOOK fuer JEE-Container-Vendoren in Java-Schicht.

**Crate-Version:** `1.0.0-rc.1` | **Status:** ✅ rc1-ready | **Sign-off:** claude
