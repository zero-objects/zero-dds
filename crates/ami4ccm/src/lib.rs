// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG AMI4CCM 1.1 — Asynchronous Method Invocation for CORBA Component Model.
//!
//! Crate `zerodds-ami4ccm`. Safety classification: **STANDARD**.
//! Spec `formal/2015-08-03` (`docs/standards/cache/omg/ami4ccm-1.1.pdf`).
//!
//! # Scope
//!
//! AMI4CCM ist eine Transformations-Spec: aus einer normalen IDL-
//! `interface`-Definition werden zwei zusaetzliche Local-Interfaces
//! abgeleitet (Spec §7.3 + §7.5):
//!
//! 1. `AMI4CCM_<Iface>` — Async-Operations mit `sendc_`-Praefix.
//! 2. `AMI4CCM_<Iface>ReplyHandler` — Type-spezifische Callbacks +
//!    `_excep`-Operations.
//!
//! Plus `CCM_AMI::ExceptionHolder` als Datenmodell fuer Exception-
//! Lieferung (Spec §7.4.1) und Pragma-Parsing fuer
//! `#pragma ami4ccm interface "<name>"` und
//! `#pragma ami4ccm receptacle "<comp>::<recep>"` (Spec §7.7).
//!
//! Die Transformation arbeitet auf dem AST-Layer von [`zerodds_idl::ast`]:
//! Eingabe ist `InterfaceDef`, Ausgabe sind zwei neu konstruierte
//! `InterfaceDef`-Instanzen (`InterfaceKind::Local`), die jedes
//! Codegen-Backend (cpp/cs/java/ts) wie normale Interfaces behandeln
//! kann.
//!
//! # Was nicht abgedeckt ist
//!
//! * **AMI4CCM Connector + Deployment** (Spec §7.6 + §7.8) — der
//!   Connector-Fragment-Code wird via D&C in einen CCM-Container
//!   deployed (`Components::EnterpriseComponent`,
//!   `CCM_AMI::Connector_T`-Templated-Module). ZeroDDS hat keinen CCM-
//!   Container/Executor; siehe Audit-File
//!   `docs/spec-coverage/omg-ami4ccm-1.1.md`. Die **Implied-IDL-
//!   Transformation** (Conformance-Punkt 1) ist voll abgedeckt; der
//!   **Connector-Fragment** (Conformance-Punkt 2) ist `n/a` ohne
//!   CCM-Runtime.
//! * **CCM-Pragma-Pre-Processor-Integration** — Pragma-Parsing ist als
//!   eigenstaendige Funktion realisiert; Integration in den IDL-
//!   Preprocessor laesst sich darauf aufbauen, sobald CCM ein
//!   Top-Level-Vorhaben ist.
//!
//! # Beispiel
//!
//! ```
//! use zerodds_ami4ccm::{Ami4CcmPragma, parse_pragma};
//!
//! let p = parse_pragma("#pragma ami4ccm interface \"Stock::StockManager\"").unwrap();
//! assert_eq!(
//!     p,
//!     Ami4CcmPragma::Interface {
//!         name: "Stock::StockManager".into(),
//!     }
//! );
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod connector;
pub mod deployment;
pub mod exception_holder;
pub mod multiplex;
pub mod pragma;
pub mod scope_resolver;
pub mod transform;

pub use connector::{Connector, ConnectorPort, Facet, PortType};
pub use deployment::{
    ConnectorImplementation, ConnectorPlanFragment, ImplementationDescriptor, PlanInstance,
};
pub use exception_holder::{ExceptionHolder, UserExceptionBase};
pub use multiplex::{
    ReceptacleArity, context_method_for_receptacle, sequence_typedef_for_interface,
};
pub use pragma::{Ami4CcmPragma, ParsePragmaError, parse_pragma};
pub use scope_resolver::{context_from_specification, populate_from_specification};
pub use transform::{
    Ami4CcmInterfaces, TransformContext, transform_interface, transform_interface_in_context,
};
