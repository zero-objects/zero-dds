// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CCM 4.0 — CORBA Component Model.
//!
//! Crate `zerodds-ccm`. Safety classification: **STANDARD**.
//! Spec `formal/06-04-01` (`internal/standards/cache/omg/ccm-4.0.pdf`).
//!
//! # Scope
//!
//! This crate implements the **Equivalent-IDL transformation part** of
//! the CCM 4.0 spec from §6 (Component Model). Input and output are one
//! [`zerodds_ast::Ast`]: a `component`, `home` or `eventtype` node is
//! rewritten into the `interface` / `valuetype` declarations an IDL
//! compiler "implicitly defines" per Spec §6.3.2 / §6.4.1 / §6.5.1 /
//! §6.6.x / §6.7.1, and the interfaces those clauses add are declared
//! beside it.
//!
//! Plus models of the `Components::*` core types (CCMObject, Cookie,
//! ConnectionDescription, Navigation/Receptacles/Events interfaces) as
//! Rust structs / enums, so callers can reference them directly.
//!
//! Plus Lightweight CCM Profile (§13) — a filter function that reduces
//! the Equivalent-IDL output to the LwCCM subset.
//!
//! # What's not covered
//!
//! The following spec chapters require a **CORBA-ORB** + **CCM-
//! container** and are deliberately `n/a` in ZeroDDS (rationale in the
//! audit file `docs/spec-coverage/omg-ccm-4.0.md`):
//! * §7 CIDL Syntax + Semantics — Component Implementation Definition
//!   Language; targets §8 CIF.
//! * §8 CCM Implementation Framework — servant/skeleton generator,
//!   requires POA + ORB.
//! * §9 Container Programming Model — server programming environment,
//!   requires CORBA-ORB.
//! * §10 Integrating with Enterprise JavaBeans — requires an EJB
//!   container.
//! * §11 Interface Repository Metamodel — requires CORBA-IFR.
//! * §12 CIF Metamodel — Implementation-Framework MOF model.
//! * §14 Deployment PSM for CCM — D&C subsystem (formal/2006-04-02).
//! * §15 Deployment IDL — part of §14.
//! * §16 XML Schema for CCM — XSD schema of the deployment subsystem.
//!
//! # Example
//!
//! ```
//! use zerodds_ccm::Cookie;
//!
//! let c = Cookie::new(vec![0x01, 0x02, 0x03]);
//! assert_eq!(c.cookie_value, vec![0x01, 0x02, 0x03]);
//! // Spec §6.5.2.4: truncation to Base retains cookieValue.
//! assert_eq!(c.truncate_to_base().cookie_value, c.cookie_value);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod dds4ccm;
pub mod lightweight;
pub mod model;
pub mod transform;
pub mod validate;

pub use lightweight::{LightweightFilterError, filter_to_lightweight};
pub use model::{Cookie, FailureReason, FeatureName, RepositoryId};
pub use transform::{
    ComponentEquivalent, EventTypeEquivalent, HomeEquivalent, TransformError, transform_component,
    transform_event_type, transform_home,
};
pub use validate::{InitOp, PrimaryKeyError, apply_factory_finder_body, validate_primary_key};
