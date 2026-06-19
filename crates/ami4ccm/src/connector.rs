// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMI4CCM connector fragment — spec §7.6 + Annex A.
//!
//! Spec §7.6 (p. 11-12) and Annex A (p. 17) define the
//! templated module `CCM_AMI::Connector_T<interface T, interface
//! AMI4CCM_T>` with:
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
//! At the AST level we produce two structures:
//!
//! 1. [`PortType`] — `AMI4CCM_Port_Type<T, AMI4CCM_T>` with two
//!    facets (`ami4ccm_provides` AMI4CCM_T, `ami4ccm_sync_provides` T)
//!    and one receptacle (`ami4ccm_uses` T).
//! 2. [`Connector`] — concrete `AMI4CCM_<Iface>_Connector` with
//!    instantiated type parameters and one port.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// PortType facet — represents a `provides` clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facet {
    /// Local facet name (in IDL: `provides AMI4CCM_T ami4ccm_provides;`
    /// → `ami4ccm_provides`).
    pub name: String,
    /// Repository/scoped name of the faceted interface.
    pub interface_type: String,
}

/// PortType receptacle — represents a `uses` clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsesPort {
    /// Receptacle name (in IDL: `uses T ami4ccm_uses;` → `ami4ccm_uses`).
    pub name: String,
    /// Repository/scoped name of the used interface.
    pub interface_type: String,
}

/// `porttype AMI4CCM_Port_Type` — spec §7.6, p. 11 + Annex A, p. 17.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortType {
    /// Templated name (typically `AMI4CCM_Port_Type`).
    pub name: String,
    /// Type parameters `T` (original iface) and `AMI4CCM_T`
    /// (transformed iface).
    pub type_params: Vec<String>,
    /// Facets — per the spec, two: `provides AMI4CCM_T ami4ccm_provides;`
    /// and `provides T ami4ccm_sync_provides;`.
    pub provides: Vec<Facet>,
    /// Receptacle — per the spec, exactly one: `uses T ami4ccm_uses;`.
    pub uses: Vec<UsesPort>,
}

impl PortType {
    /// Standard `AMI4CCM_Port_Type<T, AMI4CCM_T>` from spec Annex A (p. 17):
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

/// Receptacle clause in the connector — `uses [multiple]
/// AMI4CCM_Port_Type<T,AMI4CCM_T> ami4ccm_port;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorPort {
    /// Receptacle name (spec §7.6 default: `ami4ccm_port`).
    pub name: String,
    /// Type reference to the `PortType` (instantiated).
    pub port_type: String,
    /// Multiplex (spec §7.7, p. 13): `uses multiple` vs `uses`.
    pub multiplex: bool,
}

/// `connector AMI4CCM_<Iface>_Connector` — spec §7.6 + Annex A
/// `formal/2015-08-03` p. 17.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    /// Connector name (typically `AMI4CCM_<Iface>_Connector`).
    pub name: String,
    /// Repository ID (spec `omg.org/CCM_AMI/...`).
    pub repository_id: String,
    /// Inherits from `Components::EnterpriseComponent` (spec §7.6 IDL2
    /// lowering).
    pub base: String,
    /// Templated type args — ([T original iface, AMI4CCM_T iface]).
    pub type_args: Vec<String>,
    /// Concrete port (spec: `port AMI4CCM_Port_Type ami4ccm_port;`).
    pub ports: Vec<ConnectorPort>,
}

impl Connector {
    /// Constructor for a connector to the original interface
    /// `<original_iface>` and its AMI4CCM counterpart
    /// `AMI4CCM_<original_iface>`. Spec §7.6 (p. 11-12).
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

    /// Enables multiplex mode on the `ami4ccm_port` (spec §7.7, p. 13).
    pub fn enable_multiplex(&mut self) {
        if let Some(port) = self.ports.iter_mut().find(|p| p.name == "ami4ccm_port") {
            port.multiplex = true;
        }
    }

    /// Spec §7.6 — the connector is a `local interface` after IDL2 lowering.
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

        // Annex A: two facets in the prescribed order.
        assert_eq!(pt.provides.len(), 2);
        assert_eq!(pt.provides[0].name, "ami4ccm_provides");
        assert_eq!(pt.provides[0].interface_type, "AMI4CCM_T");
        assert_eq!(pt.provides[1].name, "ami4ccm_sync_provides");
        assert_eq!(pt.provides[1].interface_type, "T");

        // Annex A: one receptacle `uses T ami4ccm_uses`.
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
