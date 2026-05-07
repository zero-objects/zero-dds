# `zerodds-corba-ccm-ejb`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-ccm-ejb/badge.svg)](https://docs.rs/zerodds-corba-ccm-ejb)

CCM↔EJB-Bridge: CosTransactions↔JTA-UserTransaction-Status,
ConnectorBean-Lifecycle, JNDI↔CosNaming-Glue, Java-CCM-Bean-Stub-
Codegen. `no_std + alloc`, `forbid(unsafe_code)`. Safety
classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG CCM 4.0 | §16 (CCM↔EJB-Equivalents, ConnectorBean-Lifecycle) |
| OMG Transaction Service 1.4 | §10 (CosTransactions::Status, Coordinator/Resource) |
| JEE JTA 1.3 | §3.2 (`javax.transaction.Status` Constants) |
| JNDI 1.2 | Sub-Context-Naming |

## Was ist drin

- **`tx`** — bijektives Mapping `TxStatus` ↔ `JtaStatus` (alle 10
  Werte), `TxBridge`-Trait, `InMemoryTxBridge`-Impl fuer Test-Hosting.
- **`connector_bean`** — `ConnectorBean` JEE-EJB-3-Modell mit
  CCM-Lifecycle-Mapping (`@PostConstruct`, `@PreDestroy`, `@Resource`,
  `@TransactionAttribute`).
- **`stub_gen`** — `generate_bean_stub(component, kind)` emittiert
  `<Comp>Bean.java` aus AST-Component (CCM 4.0 Annex A Java-PSM).
- **`naming_glue`** — `cos_naming_to_jndi` + `jndi_to_cos_naming`
  bidirektionales Namespace-Mapping.

## Was nicht abgedeckt ist

- **JNI-/JVM-Bindings** — konkrete Container-Bindings (JBoss EAP,
  WildFly, GlassFish, Open Liberty) sind Caller-Layer; diese Crate
  liefert das Mapping-Layer auf Modell-Ebene.
- **EJB-Container-Hosting** — wir starten keine JVM; der ConnectorBean-
  Lifecycle wird vom externen JEE-Container getrieben.

## Beispiel

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
- [`zerodds-ccm`](../ccm/README.md) — CCM-Equivalent-IDL-Layer (Modell-
  Eingabe fuer den Stub-Codegen).
