// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Plan data model — OMG D&C 4.0 §6 + §7.
//!
//! We model the core plan structures without the full UML stack: only
//! what is relevant for a deployable CCM application.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// `ImplementationDescription` (IDD) — D&C §6.4.
///
/// Describes a concrete component implementation: which artifact,
/// which component repository ID it realizes, and which configuration
/// properties it has.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImplementationDescription {
    /// Unique label (spec D&C §6.4 `label`).
    pub label: String,
    /// UUID (spec D&C §6.4 `UUID`).
    pub uuid: String,
    /// List of artifact paths (`.so`/`.dll`/`.jar`).
    pub artifacts: Vec<String>,
    /// Repository ID of the component that this implementation
    /// realizes.
    pub realizes: String,
    /// Property map (D&C §6.4 `execParameter`).
    pub exec_params: BTreeMap<String, String>,
    /// Implementation dependencies (D&C §6.4 `dependsOn`).
    pub depends_on: Vec<ImplementationDependency>,
}

/// Implementation dependency — D&C §6.5.
///
/// Spec §6.5: `name` + `version` + required capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImplementationDependency {
    /// Required capability name (e.g. `Java_Runtime`).
    pub name: String,
    /// Minimum version.
    pub min_version: String,
}

/// `PackagedComponentImplementation` — D&C §6.6.
///
/// Links a component repository ID with the available
/// implementations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackagedComponentImplementation {
    /// Component repository ID.
    pub component_id: String,
    /// List of implementations (e.g. one for Linux, one for
    /// Windows).
    pub implementations: Vec<ImplementationDescription>,
}

/// `ComponentPackageDescription` (CPD) — D&C §6.6.
///
/// Bundle description for a component plus all of its
/// implementations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentPackageDescription {
    /// Unique label.
    pub label: String,
    /// UUID.
    pub uuid: String,
    /// Repository ID of the component.
    pub component_id: String,
    /// Implementations (with selection requirements).
    pub implementations: Vec<ImplementationDescription>,
}

/// `PackageConfiguration` (PSD) — D&C §6.7.
///
/// A package that has already been selected (implementation selection
/// complete).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageConfiguration {
    /// Unique label.
    pub label: String,
    /// UUID.
    pub uuid: String,
    /// Selected component package.
    pub package: ComponentPackageDescription,
    /// Selected implementation (index into package.implementations).
    pub selected_implementation: usize,
}

/// `InstanceDeploymentDescription` (IDD instance) — D&C §7.5.
///
/// A concrete instance of a component, with node assignment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstanceDeploymentDescription {
    /// Instance name.
    pub name: String,
    /// Reference to an ImplementationDescription (label).
    pub implementation: String,
    /// Target node name.
    pub node: String,
    /// Configured properties (D&C §7.5 `configProperty`).
    pub config_props: BTreeMap<String, String>,
    /// Co-location constraint (spec D&C §7.7) — list of other
    /// instance names that this instance must run in the same process
    /// as.
    pub co_locate_with: Vec<String>,
}

/// `DeploymentPlan` (DPD) — D&C §7.4.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeploymentPlan {
    /// Unique label.
    pub label: String,
    /// UUID.
    pub uuid: String,
    /// Resource requirements (e.g. minimum RAM).
    pub realizes: String,
    /// Implementations (D&C §7.4 `implementation`).
    pub implementations: Vec<ImplementationDescription>,
    /// Instances (D&C §7.4 `instance`).
    pub instances: Vec<InstanceDeploymentDescription>,
    /// Connection definitions (D&C §7.6 `connection`).
    pub connections: Vec<PlanConnection>,
}

/// Plan connection — connects the receptacle of one instance to the
/// facet of another.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanConnection {
    /// Connection name.
    pub name: String,
    /// Source instance.
    pub source_instance: String,
    /// Source port (receptacle name).
    pub source_port: String,
    /// Target instance.
    pub target_instance: String,
    /// Target port (facet name).
    pub target_port: String,
}

/// Plan validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// An instance references an implementation that does not exist in
    /// the plan.
    UnknownImplementation(String),
    /// A connection references an instance that does not exist in the
    /// plan.
    UnknownInstance(String),
    /// A co-location list refers to an instance that does not exist in
    /// the plan.
    UnknownCoLocation(String),
}

impl core::fmt::Display for PlanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownImplementation(s) => write!(f, "unknown implementation `{s}`"),
            Self::UnknownInstance(s) => write!(f, "unknown instance `{s}`"),
            Self::UnknownCoLocation(s) => write!(f, "unknown co-location `{s}`"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PlanError {}

impl DeploymentPlan {
    /// Spec D&C §7.4 — validation: all instance implementations,
    /// connection endpoints, and co-locations must exist in the plan.
    ///
    /// # Errors
    /// Siehe [`PlanError`].
    pub fn validate(&self) -> Result<(), PlanError> {
        for inst in &self.instances {
            if !self
                .implementations
                .iter()
                .any(|i| i.label == inst.implementation)
            {
                return Err(PlanError::UnknownImplementation(
                    inst.implementation.clone(),
                ));
            }
            for c in &inst.co_locate_with {
                if !self.instances.iter().any(|i| &i.name == c) {
                    return Err(PlanError::UnknownCoLocation(c.clone()));
                }
            }
        }
        for conn in &self.connections {
            if !self
                .instances
                .iter()
                .any(|i| i.name == conn.source_instance)
            {
                return Err(PlanError::UnknownInstance(conn.source_instance.clone()));
            }
            if !self
                .instances
                .iter()
                .any(|i| i.name == conn.target_instance)
            {
                return Err(PlanError::UnknownInstance(conn.target_instance.clone()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::format;

    fn sample_plan() -> DeploymentPlan {
        DeploymentPlan {
            label: "Plan1".into(),
            uuid: "uuid1".into(),
            realizes: "Plan1".into(),
            implementations: alloc::vec![ImplementationDescription {
                label: "EchoImpl".into(),
                uuid: "echo-uuid".into(),
                artifacts: alloc::vec!["lib/echo.so".into()],
                realizes: "IDL:demo/Echo:1.0".into(),
                exec_params: BTreeMap::new(),
                depends_on: alloc::vec![],
            }],
            instances: alloc::vec![InstanceDeploymentDescription {
                name: "echo1".into(),
                implementation: "EchoImpl".into(),
                node: "Node1".into(),
                config_props: BTreeMap::new(),
                co_locate_with: alloc::vec![],
            }],
            connections: alloc::vec![],
        }
    }

    #[test]
    fn valid_plan_passes_validation() {
        let p = sample_plan();
        assert!(p.validate().is_ok());
    }

    #[test]
    fn instance_with_unknown_impl_fails() {
        let mut p = sample_plan();
        p.instances[0].implementation = "NoSuch".into();
        assert_eq!(
            p.validate(),
            Err(PlanError::UnknownImplementation("NoSuch".into()))
        );
    }

    #[test]
    fn connection_with_unknown_instance_fails() {
        let mut p = sample_plan();
        p.connections.push(PlanConnection {
            name: "c1".into(),
            source_instance: "echo1".into(),
            source_port: "p".into(),
            target_instance: "ghost".into(),
            target_port: "f".into(),
        });
        assert_eq!(
            p.validate(),
            Err(PlanError::UnknownInstance("ghost".into()))
        );
    }

    #[test]
    fn co_location_with_unknown_instance_fails() {
        let mut p = sample_plan();
        p.instances[0].co_locate_with.push("ghost".into());
        assert_eq!(
            p.validate(),
            Err(PlanError::UnknownCoLocation("ghost".into()))
        );
    }

    #[test]
    fn plan_error_display() {
        let e = PlanError::UnknownImplementation("foo".into());
        assert_eq!(format!("{e}"), "unknown implementation `foo`");
    }
}
