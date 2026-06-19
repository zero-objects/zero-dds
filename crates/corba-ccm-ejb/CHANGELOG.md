# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-corba-ccm-ejb` crate.

### Spec references

- **OMG CCM 4.0** (`formal/06-04-01`) §16 — CCM↔EJB equivalents
  (ConnectorBean lifecycle, component-to-bean mapping).
- **OMG CORBA Transaction Service 1.4** (`formal/2003-09-02`) §10 —
  CosTransactions::Status, Coordinator/Resource protocol.
- **JEE JTA 1.3** §3.2 — `javax.transaction.Status` constants
  (`STATUS_ACTIVE`, `STATUS_MARKED_ROLLBACK`, `STATUS_PREPARED`,
  `STATUS_COMMITTED`, `STATUS_ROLLEDBACK`, `STATUS_UNKNOWN`,
  `STATUS_NO_TRANSACTION`, `STATUS_PREPARING`, `STATUS_COMMITTING`,
  `STATUS_ROLLING_BACK`).
- **JNDI 1.2** — sub-context naming, JNDI binding form.

### Public API

**`tx` module (CosTransactions ↔ JTA):**
- `TxStatus::{Active, MarkedRollback, Prepared, Committed, RolledBack,
  Unknown, NoTransaction, Preparing, Committing, RollingBack}` —
  CosTransactions::Status (10 values).
- `JtaStatus::{Active, MarkedRollback, Prepared, Committed, RolledBack,
  Unknown, NoTransaction, Preparing, Committing, RollingBack}` —
  `javax.transaction.Status`.
- `jta_status_from_cos(s) -> JtaStatus` + `jta_status_to_cos(s) -> TxStatus`
  — bijective mappings (1:1).
- `TxBridge` trait + `InMemoryTxBridge` impl for test hosting.

**`connector_bean` module:**
- `ConnectorBean` — JEE EJB 3 bean model with CCM lifecycle mapping.
- `LifecycleCallback`, `LifecyclePhase::{PostConstruct, PreDestroy, ...}`.
- `@Resource` / `@TransactionAttribute` annotation model.

**`stub_gen` module (Java CCM bean stub codegen):**
- `generate_bean_stub(component, kind) -> String` — emits
  `<Comp>Bean.java` from an AST component (CCM 4.0 Annex A
  Java PSM).
- `StubKind::{SessionBean, MessageDrivenBean}`.

**`naming_glue` module:**
- `JndiContext` + `JndiBinding` — abstract JNDI model.
- `cos_naming_to_jndi(name) -> String` and
  `jndi_to_cos_naming(name) -> NameComponent[]` — bidirectional
  namespace mapping.

### Implementation

`#![no_std]` with `extern crate alloc`; `#![forbid(unsafe_code)]`.

The bridge is abstract: concrete JNI/JVM bindings are caller-layer
(JEE container vendor — JBoss EAP, WildFly, GlassFish, Open Liberty).
This crate provides the mapping layer at the model level plus codegen
hooks for the Java CCM bean stub format.

### Architecture

- **Layer:** 8 (CORBA stack, Tier-A).
- **Dependencies (in):** none (substrate crate).
- **Dependents (out):** none in production externally (the bridge is
  consumed via JNI by external JEE container hosts).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- CosTransactions ↔ JTA status mapping: fixed by OMG / JEE specs
  (1:1).
- Stub codegen output form: fixed by CCM 4.0 Annex A Java PSM.
