// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Deployment Configuration — Spec §7.2.2 Tab 7.10-7.14.
//!
//! Repraesentiert das Deployment-Modell aus Figur 7.2:
//! `NodeLibrary` (Tab 7.10) - `Node` (Tab 7.11) - `DeploymentLibrary`
//! (Tab 7.12) - `Deployment` (Tab 7.13) - `DeploymentConfiguration`
//! (Tab 7.14, optionally referencing a TsnConfiguration).

use alloc::string::String;
use alloc::vec::Vec;

/// 6-Byte MAC-Adresse aus Spec Tab 7.20 (Caller-Form). Der bestehende
/// `mac::MacAddress`-Typ aus dem Transport-Layer kann hier nicht
/// re-used werden, weil dieses Modul std-feature-frei nutzbar bleibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MacAddr(pub [u8; 6]);

/// IPv4-Adresse als 4 Bytes Big-Endian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IpV4(pub [u8; 4]);

/// IPv6-Adresse als 16 Bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IpV6(pub [u8; 16]);

/// Spec Tab 7.10 — `NodeLibrary`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeLibrary {
    /// `name`.
    pub name: String,
    /// `node` (Mult 0..*).
    pub nodes: Vec<Node>,
}

/// Spec Tab 7.11 — `Node`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// `name` (Mult 1).
    pub name: String,
    /// `hostname` (Mult 1).
    pub hostname: String,
    /// `mac_address` (Mult 1).
    pub mac_address: MacAddr,
    /// `ipv4_address` (Mult 0..1).
    pub ipv4_address: Option<IpV4>,
    /// `ipv6_address` (Mult 0..1).
    pub ipv6_address: Option<IpV6>,
}

/// Spec Tab 7.12 — `DeploymentLibrary`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeploymentLibrary {
    /// `name`.
    pub name: String,
    /// `deployment` (Mult 0..*).
    pub deployments: Vec<Deployment>,
}

/// Spec Tab 7.13 — `Deployment`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Deployment {
    /// `name` (Mult 1).
    pub name: String,
    /// `node_ref` (Mult 1) — vollqualifizierter Name eines `Node`s.
    pub node_ref: String,
    /// `application_list` (Mult 1..*) — vollqualifizierte
    /// `Application.name`-Verweise.
    pub application_refs: Vec<String>,
    /// `configuration` (Mult 0..1) — `DeploymentConfiguration`.
    pub configuration: Option<DeploymentConfiguration>,
}

/// Spec Tab 7.14 — `DeploymentConfiguration`. Aktuell hat die Spec
/// nur das `tsn`-Feld; wir lassen den `TsnConfiguration`-Verweis als
/// generisches String-Feld stehen, damit dieses Modul nicht zyklisch
/// auf den TSN-Stream-Layer aus `crate::stream` zugreifen muss.
/// Caller resolved den Verweis ueber den `name`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeploymentConfiguration {
    /// `tsn` (Mult 0..1) — Verweis auf eine `TsnConfiguration` per
    /// Name (siehe `crate::stream::TsnTalker`/`TsnListener`).
    pub tsn_configuration_ref: Option<String>,
}

impl Deployment {
    /// Spec Tab 7.13 normativ: `application_list` ist Mult 1..* (mind.
    /// eine Application). Liefert `Err(())` wenn die Liste leer ist.
    ///
    /// # Errors
    /// `DeploymentValidationError::NoApplications`.
    pub fn validate(&self) -> Result<(), DeploymentValidationError> {
        if self.application_refs.is_empty() {
            Err(DeploymentValidationError::NoApplications)
        } else {
            Ok(())
        }
    }
}

/// Spec-Validierungs-Fehler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentValidationError {
    /// `application_list` muss mind. ein Element haben (Spec Tab 7.13
    /// Mult 1..*).
    NoApplications,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn node_holds_mac_and_optional_ips() {
        let n = Node {
            name: "Edge1".into(),
            hostname: "edge1.tsn.lab".into(),
            mac_address: MacAddr([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]),
            ipv4_address: Some(IpV4([10, 0, 0, 1])),
            ipv6_address: None,
        };
        assert_eq!(n.mac_address.0[0], 0xaa);
        assert!(n.ipv4_address.is_some());
        assert!(n.ipv6_address.is_none());
    }

    #[test]
    fn deployment_with_one_app_validates() {
        let d = Deployment {
            name: "main".into(),
            node_ref: "Edge1".into(),
            application_refs: alloc::vec!["App1".into()],
            configuration: None,
        };
        assert!(d.validate().is_ok());
    }

    #[test]
    fn deployment_without_apps_is_invalid_per_spec_mult_1_star() {
        // Spec Tab 7.13: application_list ist Mult 1..* (nicht 0..*).
        let d = Deployment {
            name: "main".into(),
            node_ref: "Edge1".into(),
            application_refs: alloc::vec::Vec::new(),
            configuration: None,
        };
        assert_eq!(d.validate(), Err(DeploymentValidationError::NoApplications));
    }

    #[test]
    fn deployment_configuration_can_reference_tsn_block() {
        let dc = DeploymentConfiguration {
            tsn_configuration_ref: Some("PerimeterTSN".into()),
        };
        assert_eq!(dc.tsn_configuration_ref.as_deref(), Some("PerimeterTSN"));
    }
}
