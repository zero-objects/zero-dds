// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS for Lightweight CCM 1.1 — the data model of ports and connectors.
//!
//! Spec source: `omg-ccm/dds4ccm-1.1.pdf`. This module names the basic
//! port interfaces, the port types and the connectors of the DDS-DCPS
//! ports for tooling, as Annex A declares them; `zerodds-dds4ccm` checks
//! the names against the IDL of the annex. The connector fragments (§7.2.2,
//! §7.3), the QoS profiles of the two patterns (Annex E) and the IDL itself
//! are in `zerodds-dds4ccm`, over the DCPS stack (`crates/dcps/`). The
//! DDS-DLRL ports of §8 are not modelled.

use alloc::string::String;
use alloc::vec::Vec;

// ===========================================================================
// §7.2.2.1 DDS-DCPS Basic Port Interfaces
// ===========================================================================

/// Spec §7.2.2.1 — the interface of a basic port. The interfaces that
/// depend on the data type are in the template module
/// `CCM_DDS::Typed<T, TSeq>`, the others in `CCM_DDS`; `dds_entity` of a
/// port is a `DDS::DataWriter` or a `DDS::DataReader`. The connector
/// fragments behind the interfaces are in `zerodds-dds4ccm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicPortKind {
    /// `CCM_DDS::Typed::Writer` (§7.2.2.1.1).
    Writer,
    /// `CCM_DDS::Typed::Updater` (§7.2.2.1.1).
    Updater,
    /// `CCM_DDS::Typed::Reader` (§7.2.2.1.2).
    Reader,
    /// `CCM_DDS::Typed::Getter` (§7.2.2.1.2).
    Getter,
    /// `CCM_DDS::Typed::Listener` (§7.2.2.1.2).
    Listener,
    /// `CCM_DDS::Typed::StateListener` (§7.2.2.1.2).
    StateListener,
    /// `CCM_DDS::DataListenerControl` (§7.2.2.1.3).
    DataListenerControl,
    /// `CCM_DDS::StateListenerControl` (§7.2.2.1.3).
    StateListenerControl,
    /// `CCM_DDS::ContentFilterSetting` (§7.2.2.1.4).
    ContentFilterSetting,
    /// `CCM_DDS::PortStatusListener` (§7.2.2.1.5).
    PortStatusListener,
    /// `CCM_DDS::ConnectorStatusListener` (§7.2.2.1.5).
    ConnectorStatusListener,
    /// `DDS::DataWriter`, the `dds_entity` of a publishing port.
    DataWriter,
    /// `DDS::DataReader`, the `dds_entity` of a subscribing port.
    DataReader,
}

impl BasicPortKind {
    /// Every interface of a basic port.
    pub const ALL: [Self; 13] = [
        Self::Writer,
        Self::Updater,
        Self::Reader,
        Self::Getter,
        Self::Listener,
        Self::StateListener,
        Self::DataListenerControl,
        Self::StateListenerControl,
        Self::ContentFilterSetting,
        Self::PortStatusListener,
        Self::ConnectorStatusListener,
        Self::DataWriter,
        Self::DataReader,
    ];

    /// The scoped IDL name of the interface (Annex A).
    #[must_use]
    pub const fn idl_name(self) -> &'static str {
        match self {
            Self::Writer => "CCM_DDS::Typed::Writer",
            Self::Updater => "CCM_DDS::Typed::Updater",
            Self::Reader => "CCM_DDS::Typed::Reader",
            Self::Getter => "CCM_DDS::Typed::Getter",
            Self::Listener => "CCM_DDS::Typed::Listener",
            Self::StateListener => "CCM_DDS::Typed::StateListener",
            Self::DataListenerControl => "CCM_DDS::DataListenerControl",
            Self::StateListenerControl => "CCM_DDS::StateListenerControl",
            Self::ContentFilterSetting => "CCM_DDS::ContentFilterSetting",
            Self::PortStatusListener => "CCM_DDS::PortStatusListener",
            Self::ConnectorStatusListener => "CCM_DDS::ConnectorStatusListener",
            Self::DataWriter => "DDS::DataWriter",
            Self::DataReader => "DDS::DataReader",
        }
    }

    /// Whether the interface depends on the data type: it is declared in
    /// the template module `CCM_DDS::Typed<T, TSeq>`.
    #[must_use]
    pub const fn is_typed(self) -> bool {
        matches!(
            self,
            Self::Writer
                | Self::Updater
                | Self::Reader
                | Self::Getter
                | Self::Listener
                | Self::StateListener
        )
    }
}

/// Whether the component calls a basic port or is called through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicPortDirection {
    /// `uses`: the component calls the connector fragment.
    Uses,
    /// `provides`: the connector fragment calls the component.
    Provides,
}

/// Spec §7.2.2.1 — basic port definition (typed by T).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicPort {
    /// Port name (e.g. `"data"`).
    pub name: String,
    /// The interface of the port.
    pub kind: BasicPortKind,
    /// Type parameter T (repository ID).
    pub type_id: String,
}

// ===========================================================================
// §7.2.2.2 DDS-DCPS Extended Ports
// ===========================================================================

/// One basic port of a port type: its name, its interface and its
/// direction.
pub type BasicPortOfPortType = (&'static str, BasicPortKind, BasicPortDirection);

/// Spec §7.2.2.2 — the six normative port types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtendedPortKind {
    /// `porttype DDS_Write`.
    DdsWrite,
    /// `porttype DDS_Update`.
    DdsUpdate,
    /// `porttype DDS_Read`.
    DdsRead,
    /// `porttype DDS_Get`.
    DdsGet,
    /// `porttype DDS_Listen`.
    DdsListen,
    /// `porttype DDS_StateListen`.
    DdsStateListen,
}

impl ExtendedPortKind {
    /// The six port types, in the order of the specification.
    pub const ALL: [Self; 6] = [
        Self::DdsWrite,
        Self::DdsUpdate,
        Self::DdsRead,
        Self::DdsGet,
        Self::DdsListen,
        Self::DdsStateListen,
    ];

    /// The IDL name of the port type.
    #[must_use]
    pub const fn idl_name(self) -> &'static str {
        match self {
            Self::DdsWrite => "DDS_Write",
            Self::DdsUpdate => "DDS_Update",
            Self::DdsRead => "DDS_Read",
            Self::DdsGet => "DDS_Get",
            Self::DdsListen => "DDS_Listen",
            Self::DdsStateListen => "DDS_StateListen",
        }
    }

    /// The basic ports the port type is made of, in the order of its
    /// declaration (§7.2.2.2).
    #[must_use]
    pub const fn basic_ports(self) -> &'static [BasicPortOfPortType] {
        use BasicPortDirection::{Provides, Uses};
        use BasicPortKind as K;
        match self {
            Self::DdsWrite => &[
                ("data", K::Writer, Uses),
                ("dds_entity", K::DataWriter, Uses),
            ],
            Self::DdsUpdate => &[
                ("data", K::Updater, Uses),
                ("dds_entity", K::DataWriter, Uses),
            ],
            Self::DdsRead => &[
                ("data", K::Reader, Uses),
                ("filter_config", K::ContentFilterSetting, Uses),
                ("dds_entity", K::DataReader, Uses),
                ("status", K::PortStatusListener, Provides),
            ],
            Self::DdsGet => &[
                ("data", K::Reader, Uses),
                ("fresh_data", K::Getter, Uses),
                ("filter_config", K::ContentFilterSetting, Uses),
                ("dds_entity", K::DataReader, Uses),
                ("status", K::PortStatusListener, Provides),
            ],
            Self::DdsListen => &[
                ("data", K::Reader, Uses),
                ("data_control", K::DataListenerControl, Uses),
                ("data_listener", K::Listener, Provides),
                ("filter_config", K::ContentFilterSetting, Uses),
                ("dds_entity", K::DataReader, Uses),
                ("status", K::PortStatusListener, Provides),
            ],
            Self::DdsStateListen => &[
                ("data", K::Reader, Uses),
                ("data_control", K::StateListenerControl, Uses),
                ("data_listener", K::StateListener, Provides),
                ("filter_config", K::ContentFilterSetting, Uses),
                ("dds_entity", K::DataReader, Uses),
                ("status", K::PortStatusListener, Provides),
            ],
        }
    }

    /// Whether the port type has the attribute `filter` with
    /// `setraises(NonChangeable)`: the port types of the subscribing side.
    #[must_use]
    pub const fn has_filter_attribute(self) -> bool {
        !matches!(self, Self::DdsWrite | Self::DdsUpdate)
    }
}

/// Spec §7.2.2.2 — extended port definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedPort {
    /// Port name.
    pub name: String,
    /// Port type.
    pub kind: ExtendedPortKind,
    /// Type parameter T.
    pub type_id: String,
}

// ===========================================================================
// §7.3 Connectors
// ===========================================================================

/// An attribute of a connector. Every attribute of the normative
/// connectors is `setraises(NonChangeable)`: "Any attempt to change those
/// attributes once the configuration is complete will raise a
/// NonChangeable exception" (§7.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectorAttribute {
    /// The name of the attribute.
    pub name: &'static str,
    /// Its IDL type.
    pub idl_type: &'static str,
}

/// One mirror port of a connector: its name and the port type it mirrors.
pub type MirrorPort = (&'static str, ExtendedPortKind);

/// Spec §7.3 — the normative DDS-DCPS connectors. The runtime behind
/// them is in `zerodds-dds4ccm` (`connector`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectorKind {
    /// `DDS_Base` (§7.3.1): "All DDS connectors should inherit from that
    /// base."
    Base,
    /// `DDS_TopicBase : DDS_Base` (§7.3.1): "the base for all mono-topic
    /// connectors".
    TopicBase,
    /// `DDS_State : DDS_TopicBase` (§7.3.2), pattern State Transfer.
    State,
    /// `DDS_Event : DDS_TopicBase` (§7.3.3), pattern Event Transfer.
    Event,
}

impl ConnectorKind {
    /// Every normative DDS-DCPS connector.
    pub const ALL: [Self; 4] = [Self::Base, Self::TopicBase, Self::State, Self::Event];

    /// `uses ConnectorStatusListener error_listener` of `DDS_Base`, which
    /// every connector inherits.
    pub const ERROR_LISTENER: BasicPortOfPortType = (
        "error_listener",
        BasicPortKind::ConnectorStatusListener,
        BasicPortDirection::Uses,
    );

    /// The scoped name in the IDL of Annex A. The two pattern connectors
    /// depend on the data type and are in `CCM_DDS::Typed<T, TSeq>`.
    #[must_use]
    pub const fn idl_name(self) -> &'static str {
        match self {
            Self::Base => "CCM_DDS::DDS_Base",
            Self::TopicBase => "CCM_DDS::DDS_TopicBase",
            Self::State => "CCM_DDS::Typed::DDS_State",
            Self::Event => "CCM_DDS::Typed::DDS_Event",
        }
    }

    /// The connector this one inherits from.
    #[must_use]
    pub const fn base(self) -> Option<Self> {
        match self {
            Self::Base => None,
            Self::TopicBase => Some(Self::Base),
            Self::State | Self::Event => Some(Self::TopicBase),
        }
    }

    /// The attributes the connector declares itself.
    #[must_use]
    pub const fn own_attributes(self) -> &'static [ConnectorAttribute] {
        match self {
            Self::Base => &[
                ConnectorAttribute {
                    name: "domain_id",
                    idl_type: "DDS::DomainId_t",
                },
                ConnectorAttribute {
                    name: "qos_profile",
                    idl_type: "string",
                },
            ],
            Self::TopicBase => &[
                ConnectorAttribute {
                    name: "topic_name",
                    idl_type: "string",
                },
                ConnectorAttribute {
                    name: "key_fields",
                    idl_type: "DDS::StringSeq",
                },
            ],
            Self::State | Self::Event => &[],
        }
    }

    /// The attributes of the connector, those of its bases first.
    #[must_use]
    pub fn attributes(self) -> Vec<ConnectorAttribute> {
        let mut attributes = self.base().map(Self::attributes).unwrap_or_default();
        attributes.extend_from_slice(self.own_attributes());
        attributes
    }

    /// The mirror ports of the connector: the roles of its pattern.
    #[must_use]
    pub const fn mirror_ports(self) -> &'static [MirrorPort] {
        match self {
            Self::Base | Self::TopicBase => &[],
            Self::State => &[
                ("observable", ExtendedPortKind::DdsUpdate),
                ("passive_observer", ExtendedPortKind::DdsRead),
                ("pull_observer", ExtendedPortKind::DdsGet),
                ("push_observer", ExtendedPortKind::DdsListen),
                ("push_state_observer", ExtendedPortKind::DdsStateListen),
            ],
            Self::Event => &[
                ("supplier", ExtendedPortKind::DdsWrite),
                ("pull_consumer", ExtendedPortKind::DdsGet),
                ("push_consumer", ExtendedPortKind::DdsListen),
            ],
        }
    }
}

// ===========================================================================
// §7.4.4 Threading Policy
// ===========================================================================

/// Spec §7.4.4 — the levels a threading policy is set at: "port (for
/// all its facets)", "component (for all the facets of its ports)",
/// "container (for all the facets of its components' ports)". The thread
/// pools behind them are in `zerodds-dds4ccm` (`threading`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreadingLevel {
    /// The policy of the port.
    Port,
    /// The policy of the component.
    Component,
    /// The policy of the container.
    Container,
}

impl ThreadingLevel {
    /// The order in which the thread pool of a facet is looked up: the
    /// port, "if there is no port's policy, the component's threadpool",
    /// "if there is no component's one, the container's threadpool". "If
    /// there is no container's policy, then the default is applied."
    pub const LOOKUP_ORDER: [Self; 3] = [Self::Port, Self::Component, Self::Container];

    /// The level asked when this one has no policy; `None` after the
    /// container: the default applies.
    #[must_use]
    pub const fn fallback(self) -> Option<Self> {
        match self {
            Self::Port => Some(Self::Component),
            Self::Component => Some(Self::Container),
            Self::Container => None,
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// §7.2.2.1: the interfaces of the basic ports under their IDL names;
    /// the six that take T or TSeq are in `CCM_DDS::Typed`.
    #[test]
    fn dds4ccm_7_2_2_1_basic_port_interfaces_have_the_names_of_the_idl() {
        let names: Vec<&str> = BasicPortKind::ALL.iter().map(|k| k.idl_name()).collect();
        assert_eq!(
            names,
            [
                "CCM_DDS::Typed::Writer",
                "CCM_DDS::Typed::Updater",
                "CCM_DDS::Typed::Reader",
                "CCM_DDS::Typed::Getter",
                "CCM_DDS::Typed::Listener",
                "CCM_DDS::Typed::StateListener",
                "CCM_DDS::DataListenerControl",
                "CCM_DDS::StateListenerControl",
                "CCM_DDS::ContentFilterSetting",
                "CCM_DDS::PortStatusListener",
                "CCM_DDS::ConnectorStatusListener",
                "DDS::DataWriter",
                "DDS::DataReader",
            ]
        );
        for kind in BasicPortKind::ALL {
            assert_eq!(
                kind.is_typed(),
                kind.idl_name().starts_with("CCM_DDS::Typed::"),
                "{kind:?}"
            );
        }
        let p = BasicPort {
            name: "data".into(),
            kind: BasicPortKind::Reader,
            type_id: "IDL:demo/Sensor:1.0".into(),
        };
        assert_eq!(p.kind.idl_name(), "CCM_DDS::Typed::Reader");
    }

    /// §7.2.2.2: the six port types and the basic ports each is made of,
    /// as the `porttype` declarations list them.
    #[test]
    fn dds4ccm_7_2_2_2_port_types_are_made_of_the_basic_ports_of_the_specification() {
        use BasicPortDirection::{Provides, Uses};
        let idl = |kind: ExtendedPortKind| -> Vec<String> {
            kind.basic_ports()
                .iter()
                .map(|(name, interface, direction)| {
                    let keyword = match direction {
                        Uses => "uses",
                        Provides => "provides",
                    };
                    let interface = interface.idl_name();
                    let interface = interface
                        .strip_prefix("CCM_DDS::Typed::")
                        .or_else(|| interface.strip_prefix("CCM_DDS::"))
                        .unwrap_or(interface);
                    alloc::format!("{keyword} {interface} {name}")
                })
                .collect()
        };
        let names: Vec<&str> = ExtendedPortKind::ALL.iter().map(|k| k.idl_name()).collect();
        assert_eq!(
            names,
            [
                "DDS_Write",
                "DDS_Update",
                "DDS_Read",
                "DDS_Get",
                "DDS_Listen",
                "DDS_StateListen"
            ]
        );
        assert_eq!(
            idl(ExtendedPortKind::DdsWrite),
            ["uses Writer data", "uses DDS::DataWriter dds_entity"]
        );
        assert_eq!(
            idl(ExtendedPortKind::DdsUpdate),
            ["uses Updater data", "uses DDS::DataWriter dds_entity"]
        );
        assert_eq!(
            idl(ExtendedPortKind::DdsRead),
            [
                "uses Reader data",
                "uses ContentFilterSetting filter_config",
                "uses DDS::DataReader dds_entity",
                "provides PortStatusListener status"
            ]
        );
        assert_eq!(
            idl(ExtendedPortKind::DdsGet),
            [
                "uses Reader data",
                "uses Getter fresh_data",
                "uses ContentFilterSetting filter_config",
                "uses DDS::DataReader dds_entity",
                "provides PortStatusListener status"
            ]
        );
        assert_eq!(
            idl(ExtendedPortKind::DdsListen),
            [
                "uses Reader data",
                "uses DataListenerControl data_control",
                "provides Listener data_listener",
                "uses ContentFilterSetting filter_config",
                "uses DDS::DataReader dds_entity",
                "provides PortStatusListener status"
            ]
        );
        assert_eq!(
            idl(ExtendedPortKind::DdsStateListen),
            [
                "uses Reader data",
                "uses StateListenerControl data_control",
                "provides StateListener data_listener",
                "uses ContentFilterSetting filter_config",
                "uses DDS::DataReader dds_entity",
                "provides PortStatusListener status"
            ]
        );
        // Attribute `filter`: the port types of the subscribing side.
        let with_filter: Vec<&str> = ExtendedPortKind::ALL
            .iter()
            .filter(|k| k.has_filter_attribute())
            .map(|k| k.idl_name())
            .collect();
        assert_eq!(
            with_filter,
            ["DDS_Read", "DDS_Get", "DDS_Listen", "DDS_StateListen"]
        );
    }

    /// §7.3.1: `DDS_Base` with `error_listener`, `domain_id` and
    /// `qos_profile`; `DDS_TopicBase : DDS_Base` with `topic_name` and
    /// `key_fields`; the names and types of the IDL.
    #[test]
    fn dds4ccm_7_3_1_base_connectors_have_the_attributes_of_the_idl() {
        let names = |kind: ConnectorKind| -> Vec<(&str, &str)> {
            let attributes = kind.attributes();
            attributes.iter().map(|a| (a.name, a.idl_type)).collect()
        };
        let base = [("domain_id", "DDS::DomainId_t"), ("qos_profile", "string")];
        let topic = [("topic_name", "string"), ("key_fields", "DDS::StringSeq")];
        assert_eq!(names(ConnectorKind::Base), base);
        assert_eq!(names(ConnectorKind::TopicBase), [base, topic].concat());
        assert_eq!(ConnectorKind::Base.base(), None);
        assert_eq!(ConnectorKind::TopicBase.base(), Some(ConnectorKind::Base));
        assert_eq!(ConnectorKind::Base.idl_name(), "CCM_DDS::DDS_Base");
        assert_eq!(
            ConnectorKind::TopicBase.idl_name(),
            "CCM_DDS::DDS_TopicBase"
        );
        assert_eq!(
            ConnectorKind::ERROR_LISTENER,
            (
                "error_listener",
                BasicPortKind::ConnectorStatusListener,
                BasicPortDirection::Uses
            )
        );
        // The bases have no mirror port: they are no pattern.
        assert!(ConnectorKind::Base.mirror_ports().is_empty());
        assert!(ConnectorKind::TopicBase.mirror_ports().is_empty());
        assert_eq!(ConnectorKind::ALL.len(), 4);
    }

    /// §7.3.2 and §7.3.3: the two pattern connectors inherit
    /// `DDS_TopicBase` with its four attributes and declare the mirror
    /// ports of their roles.
    #[test]
    fn dds4ccm_7_3_2_and_7_3_3_pattern_connectors_have_the_mirror_ports_of_the_idl() {
        assert_eq!(
            ConnectorKind::State.mirror_ports(),
            [
                ("observable", ExtendedPortKind::DdsUpdate),
                ("passive_observer", ExtendedPortKind::DdsRead),
                ("pull_observer", ExtendedPortKind::DdsGet),
                ("push_observer", ExtendedPortKind::DdsListen),
                ("push_state_observer", ExtendedPortKind::DdsStateListen),
            ]
        );
        assert_eq!(
            ConnectorKind::Event.mirror_ports(),
            [
                ("supplier", ExtendedPortKind::DdsWrite),
                ("pull_consumer", ExtendedPortKind::DdsGet),
                ("push_consumer", ExtendedPortKind::DdsListen),
            ]
        );
        for pattern in [ConnectorKind::State, ConnectorKind::Event] {
            assert_eq!(pattern.base(), Some(ConnectorKind::TopicBase));
            assert!(pattern.own_attributes().is_empty());
            assert_eq!(
                pattern.attributes(),
                ConnectorKind::TopicBase.attributes(),
                "the attributes are the inherited ones"
            );
            assert!(pattern.idl_name().starts_with("CCM_DDS::Typed::"));
        }
        assert_eq!(ConnectorKind::State.idl_name(), "CCM_DDS::Typed::DDS_State");
        assert_eq!(ConnectorKind::Event.idl_name(), "CCM_DDS::Typed::DDS_Event");
    }

    /// §7.4.4: the thread pool of a facet is that of the port, else of the
    /// component, else of the container, else the default.
    #[test]
    fn dds4ccm_7_4_4_threading_levels_fall_back_from_port_to_component_to_container() {
        assert_eq!(
            ThreadingLevel::LOOKUP_ORDER,
            [
                ThreadingLevel::Port,
                ThreadingLevel::Component,
                ThreadingLevel::Container
            ]
        );
        let mut walked = alloc::vec![ThreadingLevel::Port];
        while let Some(next) = walked.last().and_then(|level| level.fallback()) {
            walked.push(next);
        }
        assert_eq!(walked, ThreadingLevel::LOOKUP_ORDER);
        assert_eq!(ThreadingLevel::Container.fallback(), None);
    }
}
