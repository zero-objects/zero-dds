// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG RTC 1.0 §5.4 Resource Data Model + Introspection Interfaces.
//!
//! Phase-B-Cluster-10 (Spec-Cycle 5).
//!
//! Spec-Quelle: OMG RTC 1.0 §5.4.1 (S. 61-70) Resource-Datenmodell +
//! §5.4.2 (S. 71-77) Stereotypes-and-Interfaces (Introspection-Iface-
//! Operations).
//!
//! # Modell
//!
//! Der Datenstrom ist:
//!
//! ```text
//!   ComponentProfile
//!     ├── ports: Vec<PortProfile>
//!     └── connectors: Vec<ConnectorProfile>
//! ```
//!
//! Discovery-Wire (z.B. DDS-Topic-Push der Profiles) ist Caller-
//! Layer und ausserhalb dieses Crates.

use alloc::string::String;
use alloc::vec::Vec;

/// Eindeutiger Identifier eines RTC-Modells (Component / Port /
/// Connector). Als Spec-§5.4.1 vorgesehen ist eine UUID-Form;
/// wir benutzen einen 16-Byte-Opaque-Vec.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProfileId(pub [u8; 16]);

impl ProfileId {
    /// Erzeugt eine Null-ID (alle Bytes 0). Wird vom Discovery-Layer
    /// vor Vergabe einer echten UUID genutzt.
    #[must_use]
    pub const fn nil() -> Self {
        Self([0u8; 16])
    }
}

impl Default for ProfileId {
    fn default() -> Self {
        Self::nil()
    }
}

/// Port-Direction (Spec §5.4.1 Tab 5.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortDirection {
    /// Eingang.
    #[default]
    In,
    /// Ausgang.
    Out,
    /// Bidirektional (Service-Port).
    InOut,
}

/// Spec §5.4.1 — Port-Profile (Beschreibung eines Ports einer
/// Komponente).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PortProfile {
    /// UUID des Ports.
    pub id: ProfileId,
    /// Lokaler Name (z.B. `"odom_in"`).
    pub name: String,
    /// IDL-Type-Name des Port-Datenmodells (z.B. `"geometry::Pose"`).
    pub data_type: String,
    /// Direction.
    pub direction: PortDirection,
    /// User-defined Properties (Key-Value).
    pub properties: Vec<(String, String)>,
}

/// Spec §5.4.1 — Connector-Profile (Bindung zwischen 2+ Port-Profiles).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectorProfile {
    /// UUID des Connectors.
    pub id: ProfileId,
    /// Connector-Name.
    pub name: String,
    /// Liste der Port-IDs, die dieser Connector verbindet.
    pub port_ids: Vec<ProfileId>,
    /// User-defined Properties.
    pub properties: Vec<(String, String)>,
}

/// Spec §5.4.1 — Component-Profile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentProfile {
    /// UUID der Komponente.
    pub id: ProfileId,
    /// Component-Type (analog Spec §5.2 Component-Profile-Type).
    pub type_name: String,
    /// Komponenten-Instanz-Name.
    pub instance_name: String,
    /// Vendor (z.B. `"ZeroDDS"`).
    pub vendor: String,
    /// Version (Semver-String).
    pub version: String,
    /// Liste der Ports.
    pub ports: Vec<PortProfile>,
    /// Liste der Connectors.
    pub connectors: Vec<ConnectorProfile>,
    /// User-defined Properties.
    pub properties: Vec<(String, String)>,
}

/// Spec §5.4.2 — Introspection-Operations.
///
/// Die Spec definiert ein abstraktes UML-Interface mit drei Methoden;
/// wir realisieren es als Rust-Trait, sodass eine konkrete RTC-
/// Implementation es per Compile-Time-Dispatch bedient.
pub trait Introspection {
    /// Spec §5.4.2 — `ComponentProfile get_component_profile()`.
    fn get_component_profile(&self) -> &ComponentProfile;

    /// Spec §5.4.2 — `PortProfile get_port_profile(in PortId id)`.
    /// Returns `None` wenn der Port nicht zur Komponente gehoert.
    fn get_port_profile(&self, id: &ProfileId) -> Option<&PortProfile> {
        self.get_component_profile()
            .ports
            .iter()
            .find(|p| &p.id == id)
    }

    /// Spec §5.4.2 — `ConnectorProfile get_connector_profile(in
    /// ConnectorId id)`.
    fn get_connector_profile(&self, id: &ProfileId) -> Option<&ConnectorProfile> {
        self.get_component_profile()
            .connectors
            .iter()
            .find(|c| &c.id == id)
    }

    /// Spec §5.4.2 — `sequence<PortProfile> get_ports()`.
    fn get_ports(&self) -> &[PortProfile] {
        &self.get_component_profile().ports
    }

    /// Spec §5.4.2 — `sequence<ConnectorProfile> get_connectors()`.
    fn get_connectors(&self) -> &[ConnectorProfile] {
        &self.get_component_profile().connectors
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn pid(b: u8) -> ProfileId {
        let mut a = [0u8; 16];
        a[15] = b;
        ProfileId(a)
    }

    fn sample_port(b: u8, name: &str, dir: PortDirection) -> PortProfile {
        PortProfile {
            id: pid(b),
            name: name.into(),
            data_type: "geometry::Pose".into(),
            direction: dir,
            properties: Vec::new(),
        }
    }

    struct StubComponent {
        profile: ComponentProfile,
    }

    impl Introspection for StubComponent {
        fn get_component_profile(&self) -> &ComponentProfile {
            &self.profile
        }
    }

    fn build_stub() -> StubComponent {
        let p1 = sample_port(1, "in_port", PortDirection::In);
        let p2 = sample_port(2, "out_port", PortDirection::Out);
        let conn = ConnectorProfile {
            id: pid(10),
            name: "loop".into(),
            port_ids: alloc::vec![pid(1), pid(2)],
            properties: Vec::new(),
        };
        let comp = ComponentProfile {
            id: pid(99),
            type_name: "robotics::Localizer".into(),
            instance_name: "loc1".into(),
            vendor: "ZeroDDS".into(),
            version: "1.0".into(),
            ports: alloc::vec![p1, p2],
            connectors: alloc::vec![conn],
            properties: Vec::new(),
        };
        StubComponent { profile: comp }
    }

    #[test]
    fn get_component_profile_returns_component() {
        let s = build_stub();
        assert_eq!(s.get_component_profile().instance_name, "loc1");
    }

    #[test]
    fn get_port_profile_returns_some_when_known() {
        let s = build_stub();
        let p = s.get_port_profile(&pid(1)).expect("port present");
        assert_eq!(p.name, "in_port");
        assert_eq!(p.direction, PortDirection::In);
    }

    #[test]
    fn get_port_profile_returns_none_when_unknown() {
        let s = build_stub();
        assert!(s.get_port_profile(&pid(99)).is_none());
    }

    #[test]
    fn get_connector_profile_returns_known_connector() {
        let s = build_stub();
        let c = s.get_connector_profile(&pid(10)).expect("connector");
        assert_eq!(c.name, "loop");
        assert_eq!(c.port_ids.len(), 2);
    }

    #[test]
    fn get_ports_returns_all_two_ports() {
        let s = build_stub();
        assert_eq!(s.get_ports().len(), 2);
    }

    #[test]
    fn get_connectors_returns_one_connector() {
        let s = build_stub();
        assert_eq!(s.get_connectors().len(), 1);
    }

    #[test]
    fn nil_profile_id_has_zero_bytes() {
        assert_eq!(ProfileId::nil().0, [0u8; 16]);
    }

    #[test]
    fn default_port_direction_is_in() {
        assert_eq!(PortDirection::default(), PortDirection::In);
    }

    #[test]
    fn introspection_default_methods_compose_correctly() {
        // Stelle sicher, dass die Trait-Default-Implementierungen das
        // Component-Profile als Wahrheits-Quelle ehren.
        let s = build_stub();
        assert_eq!(s.get_ports().len(), s.get_component_profile().ports.len());
    }

    #[test]
    fn component_profile_field_round_trip() {
        let cp = ComponentProfile {
            id: pid(1),
            type_name: "T".into(),
            instance_name: "I".into(),
            vendor: "V".into(),
            version: "1.0".into(),
            ports: Vec::new(),
            connectors: Vec::new(),
            properties: alloc::vec![("k".into(), "v".into())],
        };
        assert_eq!(
            cp.properties[0],
            (
                alloc::string::String::from("k"),
                alloc::string::String::from("v")
            )
        );
    }
}
