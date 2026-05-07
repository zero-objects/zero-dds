// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMI4CCM Connector-Fragment — Spec §7.6 + Annex A.
//!
//! Spec §7.6 (S. 11-12) und Annex A (S. 17) definieren das
//! Templated-Module `CCM_AMI::Connector_T<interface T, interface
//! AMI4CCM_T>` mit:
//!
//! ```idl
//! porttype AMI4CCM_Port_Type {
//!     // Deliver the asynchronous interface with its sendc_ operations
//!     provides AMI4CCM_T ami4ccm_provides;
//!     // Delivers the original interface for synchronous invocations
//!     // through the connector
//!     provides T ami4ccm_sync_provides;
//!     // Use the port of the target component
//!     uses T ami4ccm_uses;
//! };
//! connector AMI4CCM_Connector {
//!     port AMI4CCM_Port_Type ami4ccm_port;
//! };
//! ```
//!
//! Wir produzieren auf der AST-Ebene zwei Strukturen:
//!
//! 1. [`PortType`] — `AMI4CCM_Port_Type<T, AMI4CCM_T>` mit zwei
//!    Facets (`ami4ccm_provides` AMI4CCM_T, `ami4ccm_sync_provides` T)
//!    und einem Receptacle (`ami4ccm_uses` T).
//! 2. [`Connector`] — Concrete `AMI4CCM_<Iface>_Connector` mit
//!    instanziierten Type-Parametern und einem Port.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// PortType-Facet — repräsentiert eine `provides`-Klausel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facet {
    /// Lokaler Facet-Name (im IDL: `provides AMI4CCM_T ami4ccm_provides;`
    /// → `ami4ccm_provides`).
    pub name: String,
    /// Repository-/Scoped-Name des facetierten Interfaces.
    pub interface_type: String,
}

/// PortType-Receptacle — repräsentiert eine `uses`-Klausel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsesPort {
    /// Receptacle-Name (im IDL: `uses T ami4ccm_uses;` → `ami4ccm_uses`).
    pub name: String,
    /// Repository-/Scoped-Name des verwendeten Interfaces.
    pub interface_type: String,
}

/// `porttype AMI4CCM_Port_Type` — Spec §7.6, S. 11 + Annex A, S. 17.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortType {
    /// Templated-Name (typisch `AMI4CCM_Port_Type`).
    pub name: String,
    /// Type-Parameter `T` (Original-Iface) und `AMI4CCM_T`
    /// (transformiertes Iface).
    pub type_params: Vec<String>,
    /// Facets — laut Spec zwei: `provides AMI4CCM_T ami4ccm_provides;`
    /// und `provides T ami4ccm_sync_provides;`.
    pub provides: Vec<Facet>,
    /// Receptacle — laut Spec genau eines: `uses T ami4ccm_uses;`.
    pub uses: Vec<UsesPort>,
}

impl PortType {
    /// Standard-`AMI4CCM_Port_Type<T, AMI4CCM_T>` aus Spec Annex A (S. 17):
    ///
    /// ```idl
    /// porttype AMI4CCM_Port_Type {
    ///     provides AMI4CCM_T ami4ccm_provides;
    ///     provides T ami4ccm_sync_provides;
    ///     uses T ami4ccm_uses;
    /// };
    /// ```
    #[must_use]
    pub fn ami4ccm_port_type() -> Self {
        Self {
            name: "AMI4CCM_Port_Type".to_string(),
            type_params: vec!["T".to_string(), "AMI4CCM_T".to_string()],
            provides: vec![
                Facet {
                    name: "ami4ccm_provides".to_string(),
                    interface_type: "AMI4CCM_T".to_string(),
                },
                Facet {
                    name: "ami4ccm_sync_provides".to_string(),
                    interface_type: "T".to_string(),
                },
            ],
            uses: vec![UsesPort {
                name: "ami4ccm_uses".to_string(),
                interface_type: "T".to_string(),
            }],
        }
    }
}

/// Receptacle-Klausel im Connector — `uses [multiple]
/// AMI4CCM_Port_Type<T,AMI4CCM_T> ami4ccm_port;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorPort {
    /// Receptacle-Name (Spec §7.6 default: `ami4ccm_port`).
    pub name: String,
    /// Type-Reference auf den `PortType` (instanziiert).
    pub port_type: String,
    /// Multiplex (Spec §7.7, S. 13): `uses multiple` vs `uses`.
    pub multiplex: bool,
}

/// `connector AMI4CCM_<Iface>_Connector` — Spec §7.6 + Annex A
/// `formal/2015-08-03` S. 17.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    /// Connector-Name (typisch `AMI4CCM_<Iface>_Connector`).
    pub name: String,
    /// Repository-ID (Spec `omg.org/CCM_AMI/...`).
    pub repository_id: String,
    /// Erbt von `Components::EnterpriseComponent` (Spec §7.6 IDL2-
    /// Lowering).
    pub base: String,
    /// Templated-Type-Args — ([T-Original-Iface, AMI4CCM_T-Iface]).
    pub type_args: Vec<String>,
    /// Concrete-Port (Spec: `port AMI4CCM_Port_Type ami4ccm_port;`).
    pub ports: Vec<ConnectorPort>,
}

impl Connector {
    /// Konstruktor fuer einen Connector zum Original-Interface
    /// `<original_iface>` und seinem AMI4CCM-Pendant
    /// `AMI4CCM_<original_iface>`. Spec §7.6 (S. 11-12).
    #[must_use]
    pub fn for_interface(original_iface: &str) -> Self {
        let connector_name = format!("AMI4CCM_{original_iface}_Connector");
        let ami_iface = format!("AMI4CCM_{original_iface}");
        Self {
            name: connector_name.clone(),
            repository_id: format!("IDL:omg.org/CCM_AMI/{connector_name}:1.0"),
            base: "Components::EnterpriseComponent".to_string(),
            type_args: vec![original_iface.to_string(), ami_iface],
            ports: vec![ConnectorPort {
                name: "ami4ccm_port".to_string(),
                port_type: "AMI4CCM_Port_Type".to_string(),
                multiplex: false,
            }],
        }
    }

    /// Aktiviert Multiplex-Mode auf dem `ami4ccm_port` (Spec §7.7, S. 13).
    pub fn enable_multiplex(&mut self) {
        if let Some(port) = self.ports.iter_mut().find(|p| p.name == "ami4ccm_port") {
            port.multiplex = true;
        }
    }

    /// Spec §7.6 — Connector ist ein `local interface` nach IDL2-Lowering.
    #[must_use]
    pub fn is_local(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn port_type_matches_spec_annex_a_idl() {
        let pt = PortType::ami4ccm_port_type();
        assert_eq!(pt.type_params, vec!["T", "AMI4CCM_T"]);

        // Annex A: zwei Facets in der vorgeschriebenen Reihenfolge.
        assert_eq!(pt.provides.len(), 2);
        assert_eq!(pt.provides[0].name, "ami4ccm_provides");
        assert_eq!(pt.provides[0].interface_type, "AMI4CCM_T");
        assert_eq!(pt.provides[1].name, "ami4ccm_sync_provides");
        assert_eq!(pt.provides[1].interface_type, "T");

        // Annex A: ein Receptacle `uses T ami4ccm_uses`.
        assert_eq!(pt.uses.len(), 1);
        assert_eq!(pt.uses[0].name, "ami4ccm_uses");
        assert_eq!(pt.uses[0].interface_type, "T");
    }

    #[test]
    fn connector_for_interface_uses_correct_naming() {
        let c = Connector::for_interface("StockManager");
        assert_eq!(c.name, "AMI4CCM_StockManager_Connector");
        assert_eq!(c.type_args, vec!["StockManager", "AMI4CCM_StockManager"]);
        assert_eq!(
            c.repository_id,
            "IDL:omg.org/CCM_AMI/AMI4CCM_StockManager_Connector:1.0"
        );
    }

    #[test]
    fn connector_default_is_simplex() {
        let c = Connector::for_interface("Foo");
        assert!(!c.ports[0].multiplex);
    }

    #[test]
    fn enable_multiplex_marks_port() {
        let mut c = Connector::for_interface("Foo");
        c.enable_multiplex();
        assert!(c.ports[0].multiplex);
    }

    #[test]
    fn connector_inherits_enterprise_component() {
        let c = Connector::for_interface("Foo");
        assert_eq!(c.base, "Components::EnterpriseComponent");
        assert!(c.is_local());
    }
}
