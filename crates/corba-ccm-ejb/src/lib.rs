// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-corba-ccm-ejb` — CCM↔EJB-Bridge.
//!
//! Crate `zerodds-corba-ccm-ejb`. Safety classification: **STANDARD**.
//!
//! This bridge allows CCM components to be deployed in a JEE container,
//! or conversely EJBs to be consumed as CCM receptacles. The crate
//! provides the abstract mappings — the concrete JNI/JVM bindings are
//! caller-layer (EJB container vendor).
//!
//! # Modules
//!
//! * [`tx`] — CosTransactions ↔ JTA UserTransaction mapping (OMG
//!   Transaction Service §10 + JEE JTA 1.3 §3.2).
//! * [`connector_bean`] — ConnectorBean lifecycle: `@PostConstruct`,
//!   `@PreDestroy`, `@Resource`, `@TransactionAttribute` mapping to the
//!   CCM `ComponentExecutor` lifecycle.
//! * [`stub_gen`] — Java CCM stub codegen (Spec CCM 4.0 Annex A,
//!   Java PSM): `<Comp>Bean.java` from `ComponentDef`.
//! * [`naming_glue`] — JNDI namespace ↔ CosNaming NamingContext.
//!
//! # Example
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
