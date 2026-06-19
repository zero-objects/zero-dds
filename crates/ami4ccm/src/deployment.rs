// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Deployment support — spec §7.8.
//!
//! Spec §7.8 (p. 14-15): "At runtime for the AMI4CCM connector an
//! AMI4CCM Connector fragment has to be deployed by the D&C
//! infrastructure. [...] The client component and fragment are
//! required to be deployed within the same process."
//!
//! We produce D&C deployment fragments for the AMI4CCM connector
//! per OMG D&C 4.0 (`formal/2006-04-02`):
//!
//! * **IDD** (Implementation Description Descriptor) — describes the
//!   connector artifact (.so/.dll) and the component implementation.
//! * **CDP** (Component Deployment Plan) — plan fragment for the
//!   connector as an additional instance alongside the client component.
//! * **Co-location constraint** — spec §7.8: client and connector
//!   must run in the same process.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Connector implementation description — input for plan
/// generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorImplementation {
    /// Name of the original interface (e.g. `StockManager`).
    pub original_interface: String,
    /// Name of the client component instance (spec §7.8: must
    /// be co-located).
    pub client_instance: String,
    /// Implementation artifact path (spec D&C §7.6 ImplementationArtifact).
    pub artifact_path: String,
}

/// IDD — Implementation Description Descriptor (D&C §6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementationDescriptor {
    /// `label` — unique identification.
    pub label: String,
    /// `UUID` of the implementation.
    pub uuid: String,
    /// List of artifact paths.
    pub artifacts: Vec<String>,
    /// Component repository ID that this implementation realizes.
    pub realizes: String,
}

impl ImplementationDescriptor {
    /// Creates an IDD for an AMI4CCM connector. Spec §7.8 +
    /// D&C §6.4.
    #[must_use]
    pub fn for_connector(impl_: &ConnectorImplementation) -> Self {
        let connector_name = format!("AMI4CCM_{}_Connector", impl_.original_interface);
        Self {
            label: format!("{connector_name}_Impl"),
            uuid: format!(
                "ami4ccm-{}-impl-uuid",
                impl_.original_interface.to_lowercase()
            ),
            artifacts: vec![impl_.artifact_path.clone()],
            realizes: format!("IDL:omg.org/CCM_AMI/{connector_name}:1.0"),
        }
    }
}

/// CDP fragment — a plan component instance (D&C §7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanInstance {
    /// `name` — instance name within the plan.
    pub name: String,
    /// Reference to an ImplementationDescription.
    pub implementation: String,
    /// Co-location constraint (spec §7.8: the same ProcessId).
    pub co_locate_with: Option<String>,
}

/// Deployment plan fragment for the AMI4CCM connector alongside an
/// existing client component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorPlanFragment {
    /// IDD for the connector artifact.
    pub implementation: ImplementationDescriptor,
    /// PlanInstance for the connector — co-located with the client.
    pub instance: PlanInstance,
}

impl ConnectorPlanFragment {
    /// Creates the complete plan fragment (spec §7.8 + D&C §7.4).
    #[must_use]
    pub fn build(impl_: &ConnectorImplementation) -> Self {
        let implementation = ImplementationDescriptor::for_connector(impl_);
        let instance = PlanInstance {
            name: format!("{}_AMI4CCM_Connector", impl_.client_instance),
            implementation: implementation.label.clone(),
            co_locate_with: Some(impl_.client_instance.clone()),
        };
        Self {
            implementation,
            instance,
        }
    }

    /// Spec §7.8 — client and fragment must be in the same process.
    #[must_use]
    pub fn is_co_located(&self) -> bool {
        self.instance.co_locate_with.is_some()
    }

    /// Serializes the plan fragment to D&C XML (D&C §10).
    /// Minimal form with `<implementation>` and `<instance>`.
    #[must_use]
    pub fn to_dnc_xml(&self) -> String {
        let mut xml = String::new();
        xml.push_str("<deploymentPlan>\n");
        xml.push_str("  <implementation>\n");
        xml.push_str(&format!("    <name>{}</name>\n", self.implementation.label));
        xml.push_str(&format!("    <UUID>{}</UUID>\n", self.implementation.uuid));
        for art in &self.implementation.artifacts {
            xml.push_str(&format!("    <artifact>{art}</artifact>\n"));
        }
        xml.push_str(&format!(
            "    <realizes>{}</realizes>\n",
            self.implementation.realizes
        ));
        xml.push_str("  </implementation>\n");
        xml.push_str("  <instance>\n");
        xml.push_str(&format!("    <name>{}</name>\n", self.instance.name));
        xml.push_str(&format!(
            "    <implementation>{}</implementation>\n",
            self.instance.implementation
        ));
        if let Some(ref c) = self.instance.co_locate_with {
            xml.push_str(&format!("    <coLocateWith>{c}</coLocateWith>\n"));
        }
        xml.push_str("  </instance>\n");
        xml.push_str("</deploymentPlan>\n");
        xml
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn sample_impl() -> ConnectorImplementation {
        ConnectorImplementation {
            original_interface: "StockManager".to_string(),
            client_instance: "TraderClient".to_string(),
            artifact_path: "lib/ami4ccm_stockmanager.so".to_string(),
        }
    }

    #[test]
    fn idd_realizes_correct_repository_id() {
        let impl_ = sample_impl();
        let idd = ImplementationDescriptor::for_connector(&impl_);
        assert_eq!(
            idd.realizes,
            "IDL:omg.org/CCM_AMI/AMI4CCM_StockManager_Connector:1.0"
        );
        assert!(!idd.artifacts.is_empty());
    }

    #[test]
    fn plan_fragment_is_co_located() {
        let impl_ = sample_impl();
        let frag = ConnectorPlanFragment::build(&impl_);
        assert!(frag.is_co_located());
        assert_eq!(
            frag.instance.co_locate_with.as_deref(),
            Some("TraderClient")
        );
    }

    #[test]
    fn plan_instance_name_combines_client_and_connector() {
        let impl_ = sample_impl();
        let frag = ConnectorPlanFragment::build(&impl_);
        assert_eq!(frag.instance.name, "TraderClient_AMI4CCM_Connector");
    }

    #[test]
    fn xml_roundtrip_includes_co_location() {
        let impl_ = sample_impl();
        let frag = ConnectorPlanFragment::build(&impl_);
        let xml = frag.to_dnc_xml();
        assert!(xml.contains("<deploymentPlan>"));
        assert!(xml.contains("<coLocateWith>TraderClient</coLocateWith>"));
        assert!(xml.contains("AMI4CCM_StockManager_Connector"));
    }

    #[test]
    fn xml_lists_all_artifacts() {
        let mut impl_ = sample_impl();
        impl_.artifact_path = "lib/a.so".into();
        let frag = ConnectorPlanFragment::build(&impl_);
        let xml = frag.to_dnc_xml();
        assert!(xml.contains("<artifact>lib/a.so</artifact>"));
    }
}
