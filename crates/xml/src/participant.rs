// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.5 Building Block "Domain Participant Library".
//!
//! Ein `<domain_participant_library>` traegt 1+ `<domain_participant>`-
//! Eintraege; jeder traegt `domain_ref` (Verweis auf eine Domain in einer
//! `<domain_library>`), optional `base_name` (Participant-Inheritance),
//! optional inline `<domain_participant_qos>`, sowie Listen von
//! Publishers/Subscribers mit eingebetteten DataWriters/DataReaders.
//!
//! Spec-Quelle: OMG DDS-XML 1.0 §7.3.5 (Domain Participant Library Building
//! Block).
//!
//! # XML → Rust-Type Mapping
//!
//! ```text
//! <domain_participant_library name=…> | DomainParticipantLibrary
//! <domain_participant name=… domain_ref=… base_name=…>
//!                                     | DomainParticipantEntry
//! <domain_participant_qos>            | EntityQos
//! <register_type ref=…/>              | DomainParticipantEntry.register_types_ref
//! <topic ref=…/>                      | DomainParticipantEntry.topics_ref
//! <publisher name=…>                  | PublisherEntry
//! <publisher_qos>                     | PublisherEntry.qos
//! <data_writer name=… topic_ref=…>    | DataWriterEntry
//! <datawriter_qos>                    | DataWriterEntry.qos
//! <subscriber name=…>                 | SubscriberEntry
//! <data_reader name=… topic_ref=…>    | DataReaderEntry
//! ```

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::parser::{XmlElement, parse_xml_tree};
use crate::qos::EntityQos;
use crate::qos_parser::parse_entity_qos_public;

/// Container fuer 1+ Participant-Definitionen (§7.3.5.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainParticipantLibrary {
    /// Library-Name.
    pub name: String,
    /// Participant-Definitionen in Dokument-Reihenfolge.
    pub participants: Vec<DomainParticipantEntry>,
}

impl DomainParticipantLibrary {
    /// Sucht einen Participant innerhalb dieser Library.
    #[must_use]
    pub fn participant(&self, name: &str) -> Option<&DomainParticipantEntry> {
        self.participants.iter().find(|p| p.name == name)
    }
}

/// Einzelner `<domain_participant>` (§7.3.5.4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainParticipantEntry {
    /// Participant-Name (Attribut `name`).
    pub name: String,
    /// Verweis auf eine Domain (`library::name`).
    pub domain_ref: String,
    /// Optionaler `base_name`-Verweis fuer Inheritance.
    pub base_name: Option<String>,
    /// Inline-`<domain_participant_qos>`.
    pub qos: Option<EntityQos>,
    /// Verweise auf `<register_type>`-Eintraege in der Domain
    /// (`<register_type ref="…"/>`-Children).
    pub register_types_ref: Vec<String>,
    /// Verweise auf `<topic>`-Eintraege in der Domain (`<topic ref="…"/>`).
    pub topics_ref: Vec<String>,
    /// Eingebettete Publisher.
    pub publishers: Vec<PublisherEntry>,
    /// Eingebettete Subscriber.
    pub subscribers: Vec<SubscriberEntry>,
}

/// `<publisher>` (§7.3.5.4.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublisherEntry {
    /// Publisher-Name.
    pub name: String,
    /// Inline-`<publisher_qos>`.
    pub qos: Option<EntityQos>,
    /// DataWriter-Liste.
    pub data_writers: Vec<DataWriterEntry>,
}

/// `<subscriber>` (§7.3.5.4.5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubscriberEntry {
    /// Subscriber-Name.
    pub name: String,
    /// Inline-`<subscriber_qos>`.
    pub qos: Option<EntityQos>,
    /// DataReader-Liste.
    pub data_readers: Vec<DataReaderEntry>,
}

/// `<data_writer>` (§7.3.5.4.6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataWriterEntry {
    /// Writer-Name.
    pub name: String,
    /// Verweis auf einen `<topic>`-Eintrag in der Domain.
    pub topic_ref: String,
    /// Inline-`<datawriter_qos>`.
    pub qos: Option<EntityQos>,
    /// Optionaler `qos_profile_ref`.
    pub qos_profile_ref: Option<String>,
}

/// `<data_reader>` (§7.3.5.4.7).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataReaderEntry {
    /// Reader-Name.
    pub name: String,
    /// Verweis auf einen `<topic>`-Eintrag in der Domain.
    pub topic_ref: String,
    /// Inline-`<datareader_qos>`.
    pub qos: Option<EntityQos>,
    /// Optionaler `qos_profile_ref`.
    pub qos_profile_ref: Option<String>,
}

/// Parsed alle `<domain_participant_library>`-Eintraege aus einem
/// `<dds>`-Wurzel-Element.
///
/// # Errors
/// Wie [`crate::parse_xml_tree`] plus Spec-Validierung.
pub fn parse_domain_participant_libraries(
    xml: &str,
) -> Result<Vec<DomainParticipantLibrary>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    if doc.root.name != "dds" {
        return Err(XmlError::InvalidXml(format!(
            "expected <dds> root, got <{}>",
            doc.root.name
        )));
    }
    let mut libs = Vec::new();
    for lib_node in doc.root.children_named("domain_participant_library") {
        libs.push(parse_dp_library_element(lib_node)?);
    }
    Ok(libs)
}

pub(crate) fn parse_dp_library_element(
    el: &XmlElement,
) -> Result<DomainParticipantLibrary, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain_participant_library@name".into()))?
        .to_string();
    let mut participants = Vec::new();
    for p in el.children_named("domain_participant") {
        participants.push(parse_dp_element(p)?);
    }
    Ok(DomainParticipantLibrary { name, participants })
}

fn parse_dp_element(el: &XmlElement) -> Result<DomainParticipantEntry, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain_participant@name".into()))?
        .to_string();
    let domain_ref = el
        .attribute("domain_ref")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain_participant@domain_ref".into()))?
        .to_string();
    let base_name = el.attribute("base_name").map(ToString::to_string);

    let mut entry = DomainParticipantEntry {
        name,
        domain_ref,
        base_name,
        ..DomainParticipantEntry::default()
    };

    for child in &el.children {
        match child.name.as_str() {
            "domain_participant_qos" => entry.qos = Some(parse_entity_qos_public(child)?),
            "register_type" => {
                let r = child
                    .attribute("ref")
                    .or_else(|| child.attribute("name"))
                    .ok_or_else(|| {
                        XmlError::MissingRequiredElement(
                            "domain_participant/register_type@ref".into(),
                        )
                    })?
                    .to_string();
                entry.register_types_ref.push(r);
            }
            "topic" => {
                let r = child
                    .attribute("ref")
                    .or_else(|| child.attribute("name"))
                    .ok_or_else(|| {
                        XmlError::MissingRequiredElement("domain_participant/topic@ref".into())
                    })?
                    .to_string();
                entry.topics_ref.push(r);
            }
            "publisher" => entry.publishers.push(parse_publisher_element(child)?),
            "subscriber" => entry.subscribers.push(parse_subscriber_element(child)?),
            _ => {}
        }
    }
    Ok(entry)
}

fn parse_publisher_element(el: &XmlElement) -> Result<PublisherEntry, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("publisher@name".into()))?
        .to_string();
    let mut qos: Option<EntityQos> = None;
    let mut data_writers = Vec::new();
    for child in &el.children {
        match child.name.as_str() {
            "publisher_qos" => qos = Some(parse_entity_qos_public(child)?),
            "data_writer" => data_writers.push(parse_dw_element(child)?),
            _ => {}
        }
    }
    Ok(PublisherEntry {
        name,
        qos,
        data_writers,
    })
}

fn parse_subscriber_element(el: &XmlElement) -> Result<SubscriberEntry, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("subscriber@name".into()))?
        .to_string();
    let mut qos: Option<EntityQos> = None;
    let mut data_readers = Vec::new();
    for child in &el.children {
        match child.name.as_str() {
            "subscriber_qos" => qos = Some(parse_entity_qos_public(child)?),
            "data_reader" => data_readers.push(parse_dr_element(child)?),
            _ => {}
        }
    }
    Ok(SubscriberEntry {
        name,
        qos,
        data_readers,
    })
}

fn parse_dw_element(el: &XmlElement) -> Result<DataWriterEntry, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("data_writer@name".into()))?
        .to_string();
    let topic_ref = el
        .attribute("topic_ref")
        .ok_or_else(|| XmlError::MissingRequiredElement("data_writer@topic_ref".into()))?
        .to_string();
    let qos_profile_ref = el.attribute("qos_profile_ref").map(ToString::to_string);
    let mut qos: Option<EntityQos> = None;
    for child in &el.children {
        if child.name == "datawriter_qos" {
            qos = Some(parse_entity_qos_public(child)?);
        }
    }
    Ok(DataWriterEntry {
        name,
        topic_ref,
        qos,
        qos_profile_ref,
    })
}

fn parse_dr_element(el: &XmlElement) -> Result<DataReaderEntry, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("data_reader@name".into()))?
        .to_string();
    let topic_ref = el
        .attribute("topic_ref")
        .ok_or_else(|| XmlError::MissingRequiredElement("data_reader@topic_ref".into()))?
        .to_string();
    let qos_profile_ref = el.attribute("qos_profile_ref").map(ToString::to_string);
    let mut qos: Option<EntityQos> = None;
    for child in &el.children {
        if child.name == "datareader_qos" {
            qos = Some(parse_entity_qos_public(child)?);
        }
    }
    Ok(DataReaderEntry {
        name,
        topic_ref,
        qos,
        qos_profile_ref,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_participant() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant name="P" domain_ref="my_lib::MyDomain"/>
          </domain_participant_library>
        </dds>"#;
        let libs = parse_domain_participant_libraries(xml).expect("parse");
        assert_eq!(libs[0].name, "dpl");
        assert_eq!(libs[0].participants[0].name, "P");
        assert_eq!(libs[0].participants[0].domain_ref, "my_lib::MyDomain");
    }

    #[test]
    fn parse_pub_with_writer() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant name="P" domain_ref="dl::D">
              <publisher name="Pub1">
                <data_writer name="W1" topic_ref="StateTopic">
                  <datawriter_qos>
                    <reliability><kind>RELIABLE</kind></reliability>
                  </datawriter_qos>
                </data_writer>
              </publisher>
            </domain_participant>
          </domain_participant_library>
        </dds>"#;
        let libs = parse_domain_participant_libraries(xml).expect("parse");
        let p = &libs[0].participants[0];
        let pub1 = &p.publishers[0];
        assert_eq!(pub1.name, "Pub1");
        let w = &pub1.data_writers[0];
        assert_eq!(w.name, "W1");
        assert_eq!(w.topic_ref, "StateTopic");
        assert!(w.qos.is_some());
    }

    #[test]
    fn missing_domain_ref_rejected() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant name="P"/>
          </domain_participant_library>
        </dds>"#;
        let err = parse_domain_participant_libraries(xml).expect_err("missing");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    #[test]
    fn dw_missing_topic_ref_rejected() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant name="P" domain_ref="dl::D">
              <publisher name="Pub1">
                <data_writer name="W1"/>
              </publisher>
            </domain_participant>
          </domain_participant_library>
        </dds>"#;
        let err = parse_domain_participant_libraries(xml).expect_err("missing");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }
}
