// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Plan-Datenmodell — OMG D&C 4.0 §6 + §7.
//!
//! Wir modellieren die zentralen Plan-Strukturen ohne den vollen
//! UML-Stack: nur das, was fuer eine deploybare CCM-Application
//! relevant ist.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// `ImplementationDescription` (IDD) — D&C §6.4.
///
/// Beschreibt eine konkrete Component-Implementation: welches
/// Artifact, welche Component-Repository-ID realisiert sie, und
/// welche Konfigurations-Properties hat sie.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImplementationDescription {
    /// Eindeutiger Label (Spec D&C §6.4 `label`).
    pub label: String,
    /// UUID (Spec D&C §6.4 `UUID`).
    pub uuid: String,
    /// Liste von Artifact-Pfaden (`.so`/`.dll`/`.jar`).
    pub artifacts: Vec<String>,
    /// Repository-ID des Components, das diese Implementation
    /// realisiert.
    pub realizes: String,
    /// Property-Map (D&C §6.4 `execParameter`).
    pub exec_params: BTreeMap<String, String>,
    /// Implementation-Dependencies (D&C §6.4 `dependsOn`).
    pub depends_on: Vec<ImplementationDependency>,
}

/// Implementation-Dependency — D&C §6.5.
///
/// Spec §6.5: `name` + `version` + Required-Capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImplementationDependency {
    /// Required-Capability-Name (z.B. `Java_Runtime`).
    pub name: String,
    /// Mindestversion.
    pub min_version: String,
}

/// `PackagedComponentImplementation` — D&C §6.6.
///
/// Verknuepft eine Component-Repository-ID mit den verfuegbaren
/// Implementations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackagedComponentImplementation {
    /// Component-Repository-ID.
    pub component_id: String,
    /// Liste der Implementations (z.B. eine fuer Linux, eine fuer
    /// Windows).
    pub implementations: Vec<ImplementationDescription>,
}

/// `ComponentPackageDescription` (CPD) — D&C §6.6.
///
/// Bundle-Beschreibung fuer eine Component plus alle ihre
/// Implementations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentPackageDescription {
    /// Eindeutiger Label.
    pub label: String,
    /// UUID.
    pub uuid: String,
    /// Repository-ID des Components.
    pub component_id: String,
    /// Implementations (mit Selection-Requirements).
    pub implementations: Vec<ImplementationDescription>,
}

/// `PackageConfiguration` (PSD) — D&C §6.7.
///
/// Ein bereits selektiertes Package (Implementation-Selection
/// abgeschlossen).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageConfiguration {
    /// Eindeutiger Label.
    pub label: String,
    /// UUID.
    pub uuid: String,
    /// Selected Component-Package.
    pub package: ComponentPackageDescription,
    /// Selected Implementation (Index in package.implementations).
    pub selected_implementation: usize,
}

/// `InstanceDeploymentDescription` (IDD-Instance) — D&C §7.5.
///
/// Eine konkrete Instance einer Component, mit Node-Assignment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstanceDeploymentDescription {
    /// Instance-Name.
    pub name: String,
    /// Reference auf eine ImplementationDescription (label).
    pub implementation: String,
    /// Target-Node-Name.
    pub node: String,
    /// Configured-Properties (D&C §7.5 `configProperty`).
    pub config_props: BTreeMap<String, String>,
    /// Co-Location-Constraint (Spec D&C §7.7) — Liste anderer
    /// Instance-Names, mit denen diese Instance im selben Process
    /// laufen muss.
    pub co_locate_with: Vec<String>,
}

/// `DeploymentPlan` (DPD) — D&C §7.4.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeploymentPlan {
    /// Eindeutiger Label.
    pub label: String,
    /// UUID.
    pub uuid: String,
    /// Ressourcen-Requirements (z.B. min RAM).
    pub realizes: String,
    /// Implementations (D&C §7.4 `implementation`).
    pub implementations: Vec<ImplementationDescription>,
    /// Instances (D&C §7.4 `instance`).
    pub instances: Vec<InstanceDeploymentDescription>,
    /// Connection-Definitions (D&C §7.6 `connection`).
    pub connections: Vec<PlanConnection>,
}

/// Plan-Connection — verbindet Receptacle einer Instance an Facet
/// einer anderen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanConnection {
    /// Connection-Name.
    pub name: String,
    /// Source-Instance.
    pub source_instance: String,
    /// Source-Port (Receptacle-Name).
    pub source_port: String,
    /// Target-Instance.
    pub target_instance: String,
    /// Target-Port (Facet-Name).
    pub target_port: String,
}

/// Plan-Validation-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// Instance referenziert eine Implementation, die nicht im Plan
    /// existiert.
    UnknownImplementation(String),
    /// Connection referenziert eine Instance, die nicht im Plan
    /// existiert.
    UnknownInstance(String),
    /// Co-Location-Liste verweist auf eine Instance, die nicht im
    /// Plan existiert.
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
    /// Spec D&C §7.4 — Validation: alle Instance-Implementations,
    /// Connection-Endpoints und Co-Locations muessen im Plan
    /// existieren.
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
