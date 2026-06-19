// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-corba-dnc` — OMG Deployment & Configuration 4.0
//! (`formal/2006-04-02`).
//!
//! Crate `zerodds-corba-dnc`. Safety classification: **STANDARD**.
//!
//! # Modules
//!
//! * [`plan`] — data model for DPD/CPD/IDD/PSD (D&C §6 + §7).
//! * [`xml`] — XML loader for plan files (D&C §10 XML encoding).
//! * [`repository`] — RepositoryManager (D&C §8).
//! * [`execution`] — ExecutionManager / DomainApplicationManager
//!   (D&C §9).
//! * [`node`] — NodeManager / NodeApplicationManager (D&C §9).
//! * [`container_host`] — ContainerHost: binds a
//!   `zerodds-corba-ccm::Container` to a plan application run.
//!
//! ## Example
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
