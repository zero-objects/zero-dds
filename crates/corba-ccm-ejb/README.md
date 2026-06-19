# `zerodds-corba-ccm-ejb`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-ccm-ejb/badge.svg)](https://docs.rs/zerodds-corba-ccm-ejb)

CCM↔EJB-Bridge: CosTransactions↔JTA-UserTransaction-Status,
ConnectorBean-Lifecycle, JNDI↔CosNaming-Glue, Java-CCM-Bean-Stub-
Codegen. `no_std + alloc`, `forbid(unsafe_code)`. Safety
classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|---------|
| OMG CCM 4.0 | §16 (CCM↔EJB equivalents, ConnectorBean lifecycle) |
| OMG Transaction Service 1.4 | §10 (CosTransactions::Status, Coordinator/Resource) |
| JEE JTA 1.3 | §3.2 (`javax.transaction.Status` constants) |
| JNDI 1.2 | Sub-context naming |

## What's inside

- **`tx`** — bijective mapping `TxStatus` ↔ `JtaStatus` (all 10
  values), `TxBridge` trait, `InMemoryTxBridge` impl for test hosting.
- **`connector_bean`** — `ConnectorBean` JEE EJB 3 model with CCM
  lifecycle mapping (`@PostConstruct`, `@PreDestroy`, `@Resource`,
  `@TransactionAttribute`).
- **`stub_gen`** — `generate_bean_stub(component, kind)` emits
  `<Comp>Bean.java` from an AST component (CCM 4.0 Annex A Java PSM).
- **`naming_glue`** — `cos_naming_to_jndi` + `jndi_to_cos_naming`
  bidirectional namespace mapping.

## What's not covered

- **JNI/JVM bindings** — concrete container bindings (JBoss EAP,
  WildFly, GlassFish, Open Liberty) are caller-layer; this crate
  provides the mapping layer at the model level.
- **EJB container hosting** — we do not start a JVM; the ConnectorBean
  lifecycle is driven by the external JEE container.

## Example

```rust
use zerodds_corba_ccm_ejb::{JtaStatus, TxStatus, jta_status_from_cos};

// CosTransactions::Status::Active ↔ JTA STATUS_ACTIVE.
assert_eq!(jta_status_from_cos(TxStatus::Active), JtaStatus::Active);
```

## Tests

```bash
cargo test -p zerodds-corba-ccm-ejb
```

## See also

- [Architecture](../../docs/architecture/02_architecture.md)
- [Components](../../documentation/02-architecture/components.md)
- [`zerodds-ccm`](../ccm/README.md) — CCM equivalent IDL layer (model
  input for the stub codegen).
