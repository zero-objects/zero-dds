// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Deployment-Support — Spec §7.8.
//!
//! Spec §7.8 (S. 14-15): "At runtime for the AMI4CCM connector an
//! AMI4CCM Connector fragment has to be deployed by the D&C
//! infrastructure. [...] The client component and fragment are
//! required to be deployed within the same process."
//!
//! Wir produzieren D&C-Deployment-Fragmente fuer den AMI4CCM-Connector
//! gemaess OMG D&C 4.0 (`formal/2006-04-02`):
//!
//! * **IDD** (Implementation Description Descriptor) — beschreibt das
//!   Connector-Artifact (.so/.dll) und die Component-Implementation.
//! * **CDP** (Component Deployment Plan) — Plan-Fragment fuer den
//!   Connector als zusaetzliche Instance neben der Client-Component.
//! * **Co-Location-Constraint** — Spec §7.8: Client und Connector
//!   muessen im selben Process laufen.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Connector-Implementation-Beschreibung — Eingabe fuer die Plan-
/// Generierung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorImplementation {
    /// Name des Original-Interfaces (z.B. `StockManager`).
    pub original_interface: String,
    /// Name der Client-Component-Instance (Spec §7.8: muss
    /// co-located sein).
    pub client_instance: String,
    /// Implementation-Artifact-Path (Spec D&C §7.6 ImplementationArtifact).
    pub artifact_path: String,
}

/// IDD — Implementation Description Descriptor (D&C §6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementationDescriptor {
    /// `label` — eindeutige Identifizierung.
    pub label: String,
    /// `UUID` der Implementation.
    pub uuid: String,
    /// Liste von Artifact-Paths.
    pub artifacts: Vec<String>,
    /// Component-Repository-ID, die diese Implementation realisiert.
    pub realizes: String,
}

impl ImplementationDescriptor {
    /// Erzeugt eine IDD fuer einen AMI4CCM-Connector. Spec §7.8 +
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

/// CDP-Fragment — eine Plan-Component-Instance (D&C §7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanInstance {
    /// `name` — Instance-Name innerhalb des Plans.
    pub name: String,
    /// Reference auf eine ImplementationDescription.
    pub implementation: String,
    /// Co-Location-Constraint (Spec §7.8: dasselbe ProcessId).
    pub co_locate_with: Option<String>,
}

/// Deployment-Plan-Fragment fuer den AMI4CCM-Connector neben einer
/// existierenden Client-Component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorPlanFragment {
    /// IDD fuer das Connector-Artifact.
    pub implementation: ImplementationDescriptor,
    /// PlanInstance fuer den Connector — co-located mit dem Client.
    pub instance: PlanInstance,
}

impl ConnectorPlanFragment {
    /// Erzeugt das komplette Plan-Fragment (Spec §7.8 + D&C §7.4).
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

    /// Spec §7.8 — Client und Fragment muessen im selben Process sein.
    #[must_use]
    pub fn is_co_located(&self) -> bool {
        self.instance.co_locate_with.is_some()
    }

    /// Serialisiert das Plan-Fragment zu D&C-XML (D&C §10).
    /// Minimal-Form mit `<implementation>` und `<instance>`.
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
