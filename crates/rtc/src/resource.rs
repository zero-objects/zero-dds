// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG RTC 1.0 §5.4 Resource Data Model + Introspection Interfaces.
//!
//! Phase-B-Cluster-10 (Spec-Cycle 5).
//!
//! Spec source: OMG RTC 1.0 §5.4.1 (pp. 61-70) resource data model +
//! §5.4.2 (S. 71-77) Stereotypes-and-Interfaces (Introspection-Iface-
//! Operations).
//!
//! # Model
//!
//! The data flow is:
//!
//! ```text
//!   ComponentProfile
//!     ├── ports: Vec<PortProfile>
//!     └── connectors: Vec<ConnectorProfile>
//! ```
//!
//! The discovery wire (e.g. a DDS topic push of the profiles) is the caller
//! layer and outside this crate.

use alloc::string::String;
use alloc::vec::Vec;

/// Unique identifier of an RTC model (component / port /
/// connector). Spec §5.4.1 envisages a UUID form;
/// we use a 16-byte opaque vec.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProfileId(pub [u8; 16]);

impl ProfileId {
    /// Creates a null ID (all bytes 0). Used by the discovery layer
    /// before a real UUID is assigned.
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

/// Spec §5.4.1 — port profile (description of a port of a
/// component).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PortProfile {
    /// UUID of the port.
    pub id: ProfileId,
    /// Local name (e.g. `"odom_in"`).
    pub name: String,
    /// IDL type name of the port data model (e.g. `"geometry::Pose"`).
    pub data_type: String,
    /// Direction.
    pub direction: PortDirection,
    /// User-defined properties (key-value).
    pub properties: Vec<(String, String)>,
}

/// Spec §5.4.1 — connector profile (binding between 2+ port profiles).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectorProfile {
    /// UUID of the connector.
    pub id: ProfileId,
    /// Connector name.
    pub name: String,
    /// List of port IDs that this connector connects.
    pub port_ids: Vec<ProfileId>,
    /// User-defined properties.
    pub properties: Vec<(String, String)>,
}

/// Spec §5.4.1 — component profile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentProfile {
    /// UUID of the component.
    pub id: ProfileId,
    /// Component type (analogous to spec §5.2 component-profile type).
    pub type_name: String,
    /// Component instance name.
    pub instance_name: String,
    /// Vendor (e.g. `"ZeroDDS"`).
    pub vendor: String,
    /// Version (semver string).
    pub version: String,
    /// List of ports.
    pub ports: Vec<PortProfile>,
    /// List of connectors.
    pub connectors: Vec<ConnectorProfile>,
    /// User-defined properties.
    pub properties: Vec<(String, String)>,
}

/// Spec §5.4.2 — Introspection-Operations.
///
/// The spec defines an abstract UML interface with three methods;
/// we realize it as a Rust trait, so that a concrete RTC
/// implementation serves it via compile-time dispatch.
pub trait Introspection {
    /// Spec §5.4.2 — `ComponentProfile get_component_profile()`.
    fn get_component_profile(&self) -> &ComponentProfile;

    /// Spec §5.4.2 — `PortProfile get_port_profile(in PortId id)`.
    /// Returns `None` if the port does not belong to the component.
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
        // Make sure the trait default implementations honor the
        // component profile as the source of truth.
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
