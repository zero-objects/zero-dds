// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-TSN Configuration Model PIM — Spec §7.2 + §7.3.
//!
//! Fully spec-conformant representation of Tab 7.1-7.14:
//!
//! * **§7.2.1 Application Configuration** (`application`) — Tab 7.1-7.9:
//!   QosLibrary / QosProfile / DomainLibrary / Domain / RegisteredType
//!   / DomainParticipantLibrary / DomainParticipant /
//!   ApplicationLibrary / Application.
//! * **§7.2.2 Deployment Configuration** (`deployment`) — Tab 7.10-7.14:
//!   NodeLibrary / Node / DeploymentLibrary / Deployment /
//!   DeploymentConfiguration.
//! * **§7.3 Configuration PSM** (`xml`, `json`) — XML and JSON
//!   loaders/renderers for the normative XSD schema.
//!
//! The old simplified model in `crate::config` remains for
//! backwards-compat loaders; new callers should use `pim`,
//! because it covers Tab 7.1-7.14 one-to-one.

pub mod application;
pub mod deployment;
pub mod yang;

#[cfg(feature = "std")]
pub mod json;
#[cfg(feature = "std")]
pub mod xml;

pub use application::{
    Application, ApplicationLibrary, DataReaderQosRef, DataWriterQosRef, Domain, DomainLibrary,
    DomainParticipant, DomainParticipantLibrary, DomainParticipantQosRef, PublisherQosRef,
    QosLibrary, QosProfile, RegisteredType, SubscriberQosRef, TopicLibraryEntry, TopicQosRef,
};
pub use deployment::{
    Deployment, DeploymentConfiguration, DeploymentLibrary, IpV4, IpV6, MacAddr, Node, NodeLibrary,
};
pub use yang::{
    EndStationInterface, GroupDataFrameSpecification, GroupListener, GroupTalker,
    InterfaceCapabilities, StreamId, TalkerListenerGroups, UserToNetworkRequirements,
    talker_listener_groups,
};

#[cfg(feature = "std")]
pub use json::{RenderJsonError, render_dds_tsn_json};

#[cfg(feature = "std")]
pub use xml::{ParseXmlError, parse_dds_tsn_xml};

/// Top level — a complete DDS-TSN configuration with all
/// five library sections. Spec §7.2 Figure 7.1 + 7.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DdsTsnConfig {
    /// `qos_library` — Spec Tab 7.1.
    pub qos_libraries: alloc::vec::Vec<QosLibrary>,
    /// `domain_library` — Spec Tab 7.3.
    pub domain_libraries: alloc::vec::Vec<DomainLibrary>,
    /// `domain_participant_library` — Spec Tab 7.6.
    pub domain_participant_libraries: alloc::vec::Vec<DomainParticipantLibrary>,
    /// `application_library` — Spec Tab 7.8.
    pub application_libraries: alloc::vec::Vec<ApplicationLibrary>,
    /// `node_library` — Spec Tab 7.10.
    pub node_libraries: alloc::vec::Vec<NodeLibrary>,
    /// `deployment_library` — Spec Tab 7.12.
    pub deployment_libraries: alloc::vec::Vec<DeploymentLibrary>,
}
