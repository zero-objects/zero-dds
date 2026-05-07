// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-corba-ccm-ejb` — CCM↔EJB-Bridge.
//!
//! Crate `zerodds-corba-ccm-ejb`. Safety classification: **STANDARD**.
//!
//! Diese Bridge erlaubt es, CCM-Components in einem JEE-Container zu
//! deployen oder umgekehrt EJBs als CCM-Receptacle zu konsumieren. Der
//! Crate liefert die abstrakten Mappings — die konkreten JNI-/JVM-
//! Bindings sind Caller-Layer (EJB-Container-Vendor).
//!
//! # Module
//!
//! * [`tx`] — CosTransactions ↔ JTA-UserTransaction-Mapping (OMG
//!   Transaction Service §10 + JEE JTA 1.3 §3.2).
//! * [`connector_bean`] — ConnectorBean-Lifecycle: `@PostConstruct`,
//!   `@PreDestroy`, `@Resource`, `@TransactionAttribute` mapping zur
//!   CCM `ComponentExecutor`-Lifecycle.
//! * [`stub_gen`] — Java-CCM-Stub-Codegen (Spec CCM 4.0 Annex A,
//!   Java-PSM): `<Comp>Bean.java` aus `ComponentDef`.
//! * [`naming_glue`] — JNDI-Namespace ↔ CosNaming-NamingContext.
//!
//! # Beispiel
//!
//! ```
//! use zerodds_corba_ccm_ejb::{JtaStatus, TxStatus, jta_status_from_cos};
//!
//! // CosTransactions::Status::Active ↔ JTA STATUS_ACTIVE.
//! assert_eq!(jta_status_from_cos(TxStatus::Active), JtaStatus::Active);
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod connector_bean;
pub mod naming_glue;
pub mod stub_gen;
pub mod tx;

pub use connector_bean::{ConnectorBean, LifecycleCallback, LifecyclePhase};
pub use naming_glue::{JndiBinding, JndiContext, cos_naming_to_jndi, jndi_to_cos_naming};
pub use stub_gen::{StubKind, generate_bean_stub};
pub use tx::{JtaStatus, TxBridge, TxStatus, jta_status_from_cos, jta_status_to_cos};
