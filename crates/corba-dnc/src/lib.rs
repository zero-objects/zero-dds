// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-corba-dnc` — OMG Deployment & Configuration 4.0
//! (`formal/2006-04-02`).
//!
//! Crate `zerodds-corba-dnc`. Safety classification: **STANDARD**.
//!
//! # Module
//!
//! * [`plan`] — Datenmodell fuer DPD/CPD/IDD/PSD (D&C §6 + §7).
//! * [`xml`] — XML-Loader fuer Plan-Files (D&C §10 XML-Encoding).
//! * [`repository`] — RepositoryManager (D&C §8).
//! * [`execution`] — ExecutionManager / DomainApplicationManager
//!   (D&C §9).
//! * [`node`] — NodeManager / NodeApplicationManager (D&C §9).
//! * [`container_host`] — ContainerHost: bindet einen
//!   `zerodds-corba-ccm::Container` an einen Plan-Application-Run.
//!
//! ## Beispiel
//!
//! ```
//! use zerodds_corba_dnc::DeploymentPlan;
//! let plan = DeploymentPlan::default();
//! assert!(plan.uuid.is_empty());
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod container_host;
pub mod execution;
pub mod node;
pub mod plan;
pub mod repository;
pub mod xml;

pub use container_host::{ContainerHost, HostError};
pub use execution::{DomainApplication, DomainApplicationManager, ExecutionManager};
pub use node::{NodeApplication, NodeApplicationManager, NodeManager};
pub use plan::{
    ComponentPackageDescription, DeploymentPlan, ImplementationDependency,
    ImplementationDescription, InstanceDeploymentDescription, PackageConfiguration,
    PackagedComponentImplementation, PlanError,
};
pub use repository::RepositoryManager;
pub use xml::{ParseError, parse_plan_xml};
