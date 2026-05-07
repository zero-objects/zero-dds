# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-corba-ccm-ejb`-Crate.

### Spec-Referenzen

- **OMG CCM 4.0** (`formal/06-04-01`) §16 — CCM↔EJB-Equivalents
  (ConnectorBean-Lifecycle, Component-zu-Bean-Mapping).
- **OMG CORBA Transaction Service 1.4** (`formal/2003-09-02`) §10 —
  CosTransactions::Status, Coordinator/Resource-Protocol.
- **JEE JTA 1.3** §3.2 — `javax.transaction.Status` Constants
  (`STATUS_ACTIVE`, `STATUS_MARKED_ROLLBACK`, `STATUS_PREPARED`,
  `STATUS_COMMITTED`, `STATUS_ROLLEDBACK`, `STATUS_UNKNOWN`,
  `STATUS_NO_TRANSACTION`, `STATUS_PREPARING`, `STATUS_COMMITTING`,
  `STATUS_ROLLING_BACK`).
- **JNDI 1.2** — Sub-Context-Naming, JNDI-Binding-Form.

### Public-API

**`tx`-Modul (CosTransactions ↔ JTA):**
- `TxStatus::{Active, MarkedRollback, Prepared, Committed, RolledBack,
  Unknown, NoTransaction, Preparing, Committing, RollingBack}` —
  CosTransactions::Status (10 Werte).
- `JtaStatus::{Active, MarkedRollback, Prepared, Committed, RolledBack,
  Unknown, NoTransaction, Preparing, Committing, RollingBack}` —
  `javax.transaction.Status`.
- `jta_status_from_cos(s) -> JtaStatus` + `jta_status_to_cos(s) -> TxStatus`
  — bijektive Mappings (1:1).
- `TxBridge`-Trait + `InMemoryTxBridge`-Impl fuer Test-Hosting.

**`connector_bean`-Modul:**
- `ConnectorBean` — JEE-EJB-3-Bean-Modell mit CCM-Lifecycle-Mapping.
- `LifecycleCallback`, `LifecyclePhase::{PostConstruct, PreDestroy, ...}`.
- `@Resource`-/`@TransactionAttribute`-Annotation-Modell.

**`stub_gen`-Modul (Java-CCM-Bean-Stub-Codegen):**
- `generate_bean_stub(component, kind) -> String` — emittiert
  `<Comp>Bean.java` aus einem AST-Component (CCM 4.0 Annex A
  Java-PSM).
- `StubKind::{SessionBean, MessageDrivenBean}`.

**`naming_glue`-Modul:**
- `JndiContext` + `JndiBinding` — abstraktes JNDI-Modell.
- `cos_naming_to_jndi(name) -> String` und
  `jndi_to_cos_naming(name) -> NameComponent[]` — bidirektionales
  Namespace-Mapping.

### Implementierung

`#![no_std]` mit `extern crate alloc`; `#![forbid(unsafe_code)]`.

Die Bridge ist abstrakt: konkrete JNI-/JVM-Bindings sind Caller-Layer
(JEE-Container-Vendor — JBoss EAP, WildFly, GlassFish, Open Liberty).
Diese Crate liefert das Mapping-Layer auf Modell-Ebene plus
Codegen-Hooks fuer das Java-CCM-Bean-Stub-Format.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-A).
- **Dependencies (in):** keine (substrat-Crate).
- **Dependents (out):** keine produktiven extern (Bridge wird via JNI
  von externen JEE-Container-Hosts konsumiert).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- CosTransactions ↔ JTA-Status-Mapping: durch OMG / JEE-Specs fixiert
  (1:1).
- Stub-Codegen-Output-Form: durch CCM 4.0 Annex A Java-PSM fixiert.
