// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-TSN Configuration Model PIM — Spec §7.2 + §7.3.
//!
//! Voll spec-konforme Repraesentation der Tab 7.1-7.14:
//!
//! * **§7.2.1 Application Configuration** (`application`) — Tab 7.1-7.9:
//!   QosLibrary / QosProfile / DomainLibrary / Domain / RegisteredType
//!   / DomainParticipantLibrary / DomainParticipant /
//!   ApplicationLibrary / Application.
//! * **§7.2.2 Deployment Configuration** (`deployment`) — Tab 7.10-7.14:
//!   NodeLibrary / Node / DeploymentLibrary / Deployment /
//!   DeploymentConfiguration.
//! * **§7.3 Configuration PSM** (`xml`, `json`) — XML- und JSON-
//!   Loader/Renderer fuer das normative XSD-Schema.
//!
//! Das alte simplifizierte Modell in `crate::config` bleibt fuer
//! Backwards-Compat-Loader bestehen; neue Caller sollten `pim`
//! benutzen, weil es Tab 7.1-7.14 1:1 abdeckt.

pub mod application;
pub mod deployment;

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

#[cfg(feature = "std")]
pub use json::{RenderJsonError, render_dds_tsn_json};

#[cfg(feature = "std")]
pub use xml::{ParseXmlError, parse_dds_tsn_xml};

/// Top-Level — eine vollstaendige DDS-TSN-Konfiguration mit allen
/// fuenf Library-Sections. Spec §7.2 Figur 7.1 + 7.2.
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
