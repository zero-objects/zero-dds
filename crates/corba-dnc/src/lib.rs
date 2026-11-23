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
//! * [`property`] — `Deployment::Property` with a typed value.
//! * [`connector`] — connectors in D&C (CORBA 3.3 Part 3 §7.6): the
//!   connector package and implementation descriptions, extended port
//!   kinds, assemblies with connector connections and their flattening
//!   into a plan with fragment instances.
//! * [`xml`] — XML of the plan and of the component interface, connector
//!   package and assembly descriptions (D&C §10; Part 3 §7.6).
//! * [`repository`] — RepositoryManager (D&C §8).
//! * [`execution`] — ExecutionManager / DomainApplicationManager
//!   (D&C §9).
//! * [`node`] — NodeManager / NodeApplicationManager (D&C §9).
//! * [`launch`] — the launch of a plan's connector fragments through
//!   their "home factory" entry points (Part 3 §7.5, §7.6.3.1, §7.6.4).
//!   Behind the `std` feature.
//! * [`container_host`] — ContainerHost: binds a
//!   `zerodds-corba-ccm::Container` to a plan application run. Behind the
//!   `std` feature, like the container itself.
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

pub mod connector;
#[cfg(feature = "std")]
pub mod container_host;
pub mod execution;
#[cfg(feature = "std")]
pub mod launch;
pub mod node;
pub mod plan;
pub mod property;
pub mod repository;
pub mod xml;

pub use connector::{
    ComponentAssemblyDescription, ComponentInterfaceDescription, ConnectorPackageDescription,
    FlattenError, flatten_assembly,
};
#[cfg(feature = "std")]
pub use container_host::{ContainerHost, HostError};
pub use execution::{DomainApplication, DomainApplicationManager, ExecutionManager};
#[cfg(feature = "std")]
pub use launch::{LaunchError, PlanLauncher, ProvidedReferences, launch_plan};
pub use node::{NodeApplication, NodeApplicationManager, NodeManager};
pub use plan::{
    ComponentPackageDescription, DeploymentPlan, ImplementationDependency,
    ImplementationDescription, InstanceDeploymentDescription, PackageConfiguration,
    PackagedComponentImplementation, PlanError,
};
pub use property::Property;
pub use repository::RepositoryManager;
pub use xml::{
    ParseError, assembly_to_xml, connector_package_to_xml, interface_to_xml, parse_assembly_xml,
    parse_connector_package_xml, parse_interface_xml, parse_plan_xml, plan_to_xml,
};
