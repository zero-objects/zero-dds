// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG AMI4CCM 1.1 — Asynchronous Method Invocation for CORBA Component Model.
//!
//! Crate `zerodds-ami4ccm`. Safety classification: **STANDARD**.
//! Spec `formal/2015-08-03` (`docs/standards/cache/omg/ami4ccm-1.1.pdf`).
//!
//! # Scope
//!
//! AMI4CCM is a transformation spec: from a normal IDL
//! `interface` definition, two additional local interfaces are
//! derived (spec §7.3 + §7.5):
//!
//! 1. `AMI4CCM_<Iface>` — async operations with a `sendc_` prefix.
//! 2. `AMI4CCM_<Iface>ReplyHandler` — type-specific callbacks +
//!    `_excep` operations.
//!
//! Plus `CCM_AMI::ExceptionHolder` as the data model for exception
//! delivery (spec §7.4.1) and pragma parsing for
//! `#pragma ami4ccm interface "<name>"` and
//! `#pragma ami4ccm receptacle "<comp>::<recep>"` (spec §7.7).
//!
//! The transformation operates on the AST layer of [`zerodds_idl::ast`]:
//! the input is `InterfaceDef`, the output is two newly constructed
//! `InterfaceDef` instances (`InterfaceKind::Local`), which every
//! codegen backend (cpp/cs/java/ts) can treat like normal interfaces.
//!
//! # What is not covered
//!
//! * **AMI4CCM connector + deployment** (spec §7.6 + §7.8) — the
//!   connector fragment code is deployed via D&C into a CCM container
//!   (`Components::EnterpriseComponent`,
//!   `CCM_AMI::Connector_T` templated module). ZeroDDS has no CCM
//!   container/executor; see audit file
//!   `docs/spec-coverage/omg-ami4ccm-1.1.md`. The **implied-IDL
//!   transformation** (conformance point 1) is fully covered; the
//!   **connector fragment** (conformance point 2) is `n/a` without
//!   a CCM runtime.
//! * **CCM pragma pre-processor integration** — pragma parsing is
//!   realized as a standalone function; integration into the IDL
//!   preprocessor can be built on top of it once CCM is a
//!   top-level undertaking.
//!
//! # Example
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
