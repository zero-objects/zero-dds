// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG CCM 4.0 — CORBA Component Model.
//!
//! Crate `zerodds-ccm`. Safety classification: **STANDARD**.
//! Spec `formal/06-04-01` (`docs/standards/cache/omg/ccm-4.0.pdf`).
//!
//! # Scope
//!
//! Diese Crate implementiert den **Equivalent-IDL-Transformations-
//! Anteil** der CCM-4.0-Spec aus §6 (Component Model). Eingabe ist ein
//! `zerodds_idl::ast::ComponentDef` / `HomeDef` / `EventDef`; Ausgabe sind
//! die Spec-konformen `interface`-/`valuetype`-Definitionen, die ein
//! IDL-Compiler "implicitly defines" laut Spec §6.3.2 / §6.4.1 /
//! §6.5.1 / §6.6.x / §6.7.1.
//!
//! Plus Modelle der `Components::*`-Core-Types (CCMObject, Cookie,
//! ConnectionDescription, Navigation/Receptacles/Events-Interfaces) als
//! Rust-Structs / -Enums, damit Caller darauf direkt referenzieren
//! koennen.
//!
//! Plus Lightweight CCM Profile (§13) — Filter-Funktion, die den
//! Equivalent-IDL-Output auf das LwCCM-Subset reduziert.
//!
//! # Was nicht abgedeckt ist
//!
//! Die folgenden Spec-Kapitel verlangen einen **CORBA-ORB** + **CCM-
//! Container** und sind in ZeroDDS bewusst `n/a` (Begruendung im
//! Audit-File `docs/spec-coverage/omg-ccm-4.0.md`):
//! * §7 CIDL Syntax + Semantics — Component Implementation Definition
//!   Language; targets §8 CIF.
//! * §8 CCM Implementation Framework — Servant-/Skeleton-Generator,
//!   verlangt POA + ORB.
//! * §9 Container Programming Model — Server-Programming-Environment,
//!   verlangt CORBA-ORB.
//! * §10 Integrating with Enterprise JavaBeans — verlangt EJB-
//!   Container.
//! * §11 Interface Repository Metamodel — verlangt CORBA-IFR.
//! * §12 CIF Metamodel — Implementation-Framework-MOF-Modell.
//! * §14 Deployment PSM for CCM — D&C-Subsystem (formal/2006-04-02).
//! * §15 Deployment IDL — Bestandteil von §14.
//! * §16 XML Schema for CCM — XSD-Schema des Deployment-Subsystems.
//!
//! # Beispiel
//!
//! ```
//! use zerodds_ccm::Cookie;
//!
//! let c = Cookie::new(vec![0x01, 0x02, 0x03]);
//! assert_eq!(c.cookie_value, vec![0x01, 0x02, 0x03]);
//! // Spec §6.5.2.4: Truncation auf Base behaelt cookieValue.
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
    ComponentEquivalent, EventTypeEquivalent, HomeEquivalent, scoped_name, transform_component,
    transform_event_type, transform_home,
};
pub use validate::{InitOp, PrimaryKeyError, apply_factory_finder_body, validate_primary_key};
