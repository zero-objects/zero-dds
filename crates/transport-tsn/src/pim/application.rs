// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS Application Configuration — Spec §7.2.1 Tab 7.1-7.9.
//!
//! Represents the five library concepts from Figure 7.1:
//!
//! 1. `QosLibrary` (Tab 7.1) - QosProfile (Tab 7.2)
//! 2. `DomainLibrary` (Tab 7.3) - Domain (Tab 7.4) - RegisteredType (Tab 7.5)
//! 3. `DomainParticipantLibrary` (Tab 7.6) - DomainParticipant (Tab 7.7)
//! 4. `ApplicationLibrary` (Tab 7.8) - Application (Tab 7.9)
//!
//! QoS contents are not modeled inline here — we reference them
//! via `*QosRef` structs with a profile name. The caller (e.g. an
//! XML loader) materializes the DDS standard QoS structs from
//! `crates/qos/`. This aspect is explicitly recorded in Spec §7.2.1 Tab 7.2
//! as "0..* multiplicity with a name identifier".

use alloc::string::String;
use alloc::vec::Vec;

// -------------------------------------------------------------------
// Tab 7.1 — QosLibrary.
// -------------------------------------------------------------------

/// Spec Tab 7.1 — `QosLibrary`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QosLibrary {
    /// `name` (`String8`, Mult 1).
    pub name: String,
    /// `qos_profile` (`QosProfile`, Mult 0..*).
    pub qos_profiles: Vec<QosProfile>,
}

/// Spec Tab 7.2 — `QosProfile`. Per QoS category, 0..* references with
/// a name identifier (Spec: "shall provide a name to identify the
/// specific configuration").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QosProfile {
    /// `name` (caller identifier, the spec implies uniqueness
    /// within the library).
    pub name: String,
    /// `domain_participant_qos` (Mult 0..*).
    pub domain_participant_qos: Vec<DomainParticipantQosRef>,
    /// `publisher_qos` (Mult 0..*).
    pub publisher_qos: Vec<PublisherQosRef>,
    /// `subscriber_qos` (Mult 0..*).
    pub subscriber_qos: Vec<SubscriberQosRef>,
    /// `topic_qos` (Mult 0..*).
    pub topic_qos: Vec<TopicQosRef>,
    /// `datawriter_qos` (Mult 0..*).
    pub datawriter_qos: Vec<DataWriterQosRef>,
    /// `datareader_qos` (Mult 0..*).
    pub datareader_qos: Vec<DataReaderQosRef>,
}

/// QoS reference with a named identifier (the caller layer materializes
/// the DDS PSM contents).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainParticipantQosRef {
    /// Name of this specific `DomainParticipantQos`.
    pub name: String,
    /// Optional: XML snippet/JSON content for the caller (empty = default QoS).
    pub raw_qos: String,
}

/// QoS reference with a named identifier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublisherQosRef {
    /// Identifier.
    pub name: String,
    /// Content (caller form).
    pub raw_qos: String,
}

/// QoS reference with a named identifier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriberQosRef {
    /// Identifier.
    pub name: String,
    /// Content.
    pub raw_qos: String,
}

/// QoS reference with a named identifier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicQosRef {
    /// Identifier.
    pub name: String,
    /// Content.
    pub raw_qos: String,
}

/// QoS reference with a named identifier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataWriterQosRef {
    /// Identifier.
    pub name: String,
    /// Content.
    pub raw_qos: String,
}

/// QoS reference with a named identifier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataReaderQosRef {
    /// Identifier.
    pub name: String,
    /// Content.
    pub raw_qos: String,
}

// -------------------------------------------------------------------
// Tab 7.3-7.5 — DomainLibrary / Domain / RegisteredType.
// -------------------------------------------------------------------

/// Spec Tab 7.3 — `DomainLibrary`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainLibrary {
    /// `name` (`String8`).
    pub name: String,
    /// `domain` (`Domain`, Mult 0..*).
    pub domains: Vec<Domain>,
}

/// Spec Tab 7.4 — `Domain`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Domain {
    /// `name` (`String8`, Mult 1).
    pub name: String,
    /// `registered_type` (Mult 0..*).
    pub registered_types: Vec<RegisteredType>,
    /// `Topic` (Mult 0..*) — Spec Tab 7.4: only the subset attributes
    /// `topic_name` + `type_name` from DDS.
    pub topics: Vec<TopicLibraryEntry>,
}

/// Spec Tab 7.5 — `RegisteredType`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegisteredType {
    /// `name` — name under which the type is registered in the domain.
    pub name: String,
    /// `type_ref` — fully qualified name within the TypeLibrary.
    pub type_ref: String,
}

/// Spec Tab 7.4 sub — topic entry in a domain. Spec: "shall
/// represent only the attributes of the Topic type that apply (i.e.,
/// `topic_name` and `type_name`)".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicLibraryEntry {
    /// `topic_name`.
    pub topic_name: String,
    /// `type_name` — references a `RegisteredType.name`.
    pub type_name: String,
}

// -------------------------------------------------------------------
// Tab 7.6-7.7 — DomainParticipantLibrary / DomainParticipant.
// -------------------------------------------------------------------

/// Spec Tab 7.6 — `DomainParticipantLibrary`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainParticipantLibrary {
    /// `name`.
    pub name: String,
    /// `domain_participant` (Mult 0..*).
    pub domain_participants: Vec<DomainParticipant>,
}

/// Spec Tab 7.7 — `DomainParticipant`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainParticipant {
    /// `name` (Mult 1).
    pub name: String,
    /// `base_name` (Mult 0..1) — fully qualified name of another
    /// DomainParticipant for inheritance.
    pub base_name: Option<String>,
    /// `domain_ref` (Mult 0..1) — fully qualified name of the domain
    /// for inheritance.
    pub domain_ref: Option<String>,
}

// -------------------------------------------------------------------
// Tab 7.8-7.9 — ApplicationLibrary / Application.
// -------------------------------------------------------------------

/// Spec Tab 7.8 — `ApplicationLibrary`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApplicationLibrary {
    /// `name`.
    pub name: String,
    /// `application` (Mult 0..*).
    pub applications: Vec<Application>,
}

/// Spec Tab 7.9 — `Application`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Application {
    /// `name`.
    pub name: String,
    /// `domain_participant` (Mult 0..*) — fully qualified references
    /// to `DomainParticipant.name`.
    pub domain_participant_refs: Vec<String>,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn qos_profile_holds_six_qos_categories_per_spec_tab_72() {
        let p = QosProfile {
            name: "default".into(),
            domain_participant_qos: alloc::vec![DomainParticipantQosRef::default()],
            publisher_qos: alloc::vec![PublisherQosRef::default()],
            subscriber_qos: alloc::vec![SubscriberQosRef::default()],
            topic_qos: alloc::vec![TopicQosRef::default()],
            datawriter_qos: alloc::vec![DataWriterQosRef::default()],
            datareader_qos: alloc::vec![DataReaderQosRef::default()],
        };
        assert_eq!(p.domain_participant_qos.len(), 1);
        assert_eq!(p.publisher_qos.len(), 1);
        assert_eq!(p.subscriber_qos.len(), 1);
        assert_eq!(p.topic_qos.len(), 1);
        assert_eq!(p.datawriter_qos.len(), 1);
        assert_eq!(p.datareader_qos.len(), 1);
    }

    #[test]
    fn domain_carries_registered_types_and_topics() {
        let d = Domain {
            name: "PerimeterDomain".into(),
            registered_types: alloc::vec![RegisteredType {
                name: "Shape".into(),
                type_ref: "ShapeType".into(),
            }],
            topics: alloc::vec![TopicLibraryEntry {
                topic_name: "Square".into(),
                type_name: "Shape".into(),
            }],
        };
        assert_eq!(d.registered_types[0].type_ref, "ShapeType");
        assert_eq!(d.topics[0].type_name, "Shape");
    }

    #[test]
    fn domain_participant_inheritance_chain_is_optional() {
        // Spec Tab 7.7: base_name + domain_ref are optional (Mult 0..1).
        let dp = DomainParticipant {
            name: "MainDP".into(),
            base_name: None,
            domain_ref: None,
        };
        assert!(dp.base_name.is_none());
        assert!(dp.domain_ref.is_none());
    }

    #[test]
    fn application_holds_multiple_dp_refs() {
        // Spec Tab 7.9: domain_participant is Mult 0..* (not 1..*).
        let a = Application {
            name: "PerimeterApp".into(),
            domain_participant_refs: alloc::vec!["DP1".into(), "DP2".into()],
        };
        assert_eq!(a.domain_participant_refs.len(), 2);
    }
}
