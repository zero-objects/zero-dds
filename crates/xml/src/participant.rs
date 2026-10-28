// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.5 Building Block "Domain Participant Library".
//!
//! A `<domain_participant_library>` carries 1+ `<domain_participant>`
//! entries; each carries `domain_ref` (a reference to a domain in a
//! `<domain_library>`), optional `base_name` (participant inheritance),
//! optional inline `<domain_participant_qos>`, own `<register_type>`
//! and `<topic>` definitions (the same complex types a `<domain>`
//! uses), plus lists of publishers/subscribers with embedded
//! DataWriters/DataReaders.
//!
//! Spec source: OMG DDS-XML 1.0 §7.3.5 (Domain Participant Library Building
//! Block).
//!
//! # XML → Rust type mapping
//!
//! ```text
//! <domain_participant_library name=…> | DomainParticipantLibrary
//! <domain_participant name=… domain_ref=… base_name=…>
//!                                     | DomainParticipantEntry
//! <domain_participant_qos>            | EntityQos
//! <register_type name=… type_ref=…>   | DomainParticipantEntry.register_types
//! <topic name=… register_type_ref=…>  | DomainParticipantEntry.topics
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

use crate::domain::{RegisterType, TopicEntry, parse_register_type, parse_topic_entry};
use crate::errors::XmlError;
use crate::options::{Ctx, Parsed, ReadOptions};
use crate::parser::{XmlElement, parse_xml_tree};
use crate::qos::EntityQos;
use crate::qos_parser::set_inline_qos;
use crate::types::parse_ulong;

/// Container for 1+ participant definitions (§7.3.5.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainParticipantLibrary {
    /// Library name.
    pub name: String,
    /// Participant definitions in document order.
    pub participants: Vec<DomainParticipantEntry>,
}

impl DomainParticipantLibrary {
    /// Looks up a participant within this library.
    #[must_use]
    pub fn participant(&self, name: &str) -> Option<&DomainParticipantEntry> {
        self.participants.iter().find(|p| p.name == name)
    }
}

/// A single `<domain_participant>` (§7.3.5.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainParticipantEntry {
    /// Participant name (attribute `name`).
    pub name: String,
    /// Reference to a domain (`library::name`), when the document gives
    /// one. `dds-xml_domainparticipant_definitions_nonamespace.xsd`
    /// declares `domain_ref` without `use="required"`, and a participant
    /// may instead name its domain numerically through [`Self::domain_id`]
    /// or inherit it through [`Self::base_name`].
    pub domain_ref: Option<String>,
    /// Numeric domain ID (attribute `domain_id`), the alternative to
    /// [`Self::domain_ref`]. Also optional in the schema.
    pub domain_id: Option<u32>,
    /// Optional `base_name` reference for inheritance.
    pub base_name: Option<String>,
    /// Inline `<domain_participant_qos>`.
    pub qos: Option<EntityQos>,
    /// Own `<register_type>` definitions. §7.3.5.4.2 makes the
    /// referenced domain's registrations available; a participant may
    /// additionally register types itself, and the schema types those
    /// children with the very same `registerType` a `<domain>` uses
    /// (`dds-xml_domainparticipant_definitions_nonamespace.xsd`
    /// includes the Domain building block's schema).
    pub register_types: Vec<RegisterType>,
    /// Own `<topic>` definitions, typed as the Domain building block's
    /// `topicEntry` for the same reason as [`Self::register_types`].
    pub topics: Vec<TopicEntry>,
    /// Embedded publishers.
    pub publishers: Vec<PublisherEntry>,
    /// Embedded subscribers.
    pub subscribers: Vec<SubscriberEntry>,
}

/// `<publisher>` (§7.3.5.4.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublisherEntry {
    /// Publisher name.
    pub name: String,
    /// Inline `<publisher_qos>`.
    pub qos: Option<EntityQos>,
    /// DataWriter list.
    pub data_writers: Vec<DataWriterEntry>,
}

/// `<subscriber>` (§7.3.5.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubscriberEntry {
    /// Subscriber name.
    pub name: String,
    /// Inline `<subscriber_qos>`.
    pub qos: Option<EntityQos>,
    /// DataReader list.
    pub data_readers: Vec<DataReaderEntry>,
}

/// `<data_writer>` (§7.3.5.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataWriterEntry {
    /// Writer name.
    pub name: String,
    /// Reference to a `<topic>` entry in the domain.
    pub topic_ref: String,
    /// Inline `<datawriter_qos>`.
    pub qos: Option<EntityQos>,
    /// Optional `qos_profile_ref`.
    pub qos_profile_ref: Option<String>,
}

/// `<data_reader>` (§7.3.5.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataReaderEntry {
    /// Reader name.
    pub name: String,
    /// Reference to a `<topic>` entry in the domain.
    pub topic_ref: String,
    /// Inline `<datareader_qos>`.
    pub qos: Option<EntityQos>,
    /// Optional `qos_profile_ref`.
    pub qos_profile_ref: Option<String>,
    /// `<content_filter>` (schema `dataReader`).
    pub content_filter: Option<FilterDef>,
    /// The statuses of the `<status_condition>` children (schema
    /// `statusCondition`), in document order.
    pub status_conditions: Vec<String>,
    /// `<read_condition>` children.
    pub read_conditions: Vec<ReadConditionDef>,
    /// `<query_condition>` children (same shape as a content filter).
    pub query_conditions: Vec<FilterDef>,
}

/// A `<content_filter>` or `<query_condition>` (schema `filter`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterDef {
    /// `name` attribute.
    pub name: String,
    /// `kind` attribute: `builtin.sql` or `builtin.stringMatch`.
    pub kind: String,
    /// `<expression>` child.
    pub expression: Option<String>,
    /// The `<param>` children of `<parameter_list>`, in document order.
    pub parameters: Vec<String>,
}

/// A `<read_condition>` (schema `readCondition`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadConditionDef {
    /// `name` attribute.
    pub name: String,
    /// `<sample_state>` values (`READ`, `NOT_READ`).
    pub sample_states: Vec<String>,
    /// `<view_state>` values (`NEW`, `NOT_NEW`).
    pub view_states: Vec<String>,
    /// `<instance_state>` values (`ALIVE`, `NOT_ALIVE_DISPOSED`,
    /// `NOT_ALIVE_NO_WRITERS`).
    pub instance_states: Vec<String>,
}

/// Parses all `<domain_participant_library>` entries from a `<dds>` or a
/// bare `<domain_participant_library>` document (§7.3.5.3 gives this
/// building block both roots — see
/// [`crate::parser::DdsXmlDocument::library_elements`]).
///
/// # Errors
/// As [`crate::parse_xml_tree`] plus spec validation.
pub fn parse_domain_participant_libraries(
    xml: &str,
) -> Result<Vec<DomainParticipantLibrary>, XmlError> {
    Ok(parse_domain_participant_libraries_with(xml, &ReadOptions::default())?.value)
}

/// As [`parse_domain_participant_libraries`], with reading options.
///
/// # Errors
/// As [`parse_domain_participant_libraries`].
pub fn parse_domain_participant_libraries_with(
    xml: &str,
    options: &ReadOptions,
) -> Result<Parsed<Vec<DomainParticipantLibrary>>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    let mut ctx = Ctx::new(*options);
    let mut libs = Vec::new();
    for lib_node in doc.library_elements("domain_participant_library")? {
        libs.push(parse_dp_library_element(&mut ctx, lib_node)?);
    }
    Ok(ctx.finish(libs))
}

pub(crate) fn parse_dp_library_element(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<DomainParticipantLibrary, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain_participant_library@name".into()))?
        .to_string();
    let mut participants = Vec::new();
    for p in &el.children {
        match p.name.as_str() {
            "domain_participant" => participants.push(parse_dp_element(ctx, p)?),
            _ => ctx.unknown_child(p)?,
        }
    }
    Ok(DomainParticipantLibrary { name, participants })
}

/// Parses one `<domain_participant>` element.
///
/// Shared with the Application building block: §7.3.6.3's
/// `dds-xml_application_definitions_nonamespace.xsd` includes this
/// building block's schema and types an `<application>`'s
/// `<domain_participant>` children as the very same `domainParticipant`
/// complex type, so both must read them the same way.
pub(crate) fn parse_dp_element(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<DomainParticipantEntry, XmlError> {
    ctx.check_attributes(el, &["name", "base_name", "domain_ref", "domain_id"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain_participant@name".into()))?
        .to_string();
    // `name` is the only `use="required"` attribute of `domainParticipant`;
    // `domain_ref`, `domain_id` and `base_name` are all optional, and the
    // OMG's own `dds-xml_domainparticipants_example.xml` opens with a
    // participant that carries `domain_id` and no `domain_ref`.
    let domain_ref = el.attribute("domain_ref").map(ToString::to_string);
    let domain_id = match el.attribute("domain_id") {
        Some(v) => Some(parse_ulong(v)?),
        None => None,
    };
    let base_name = el.attribute("base_name").map(ToString::to_string);

    let mut entry = DomainParticipantEntry {
        name,
        domain_ref,
        domain_id,
        base_name,
        ..DomainParticipantEntry::default()
    };

    for child in &el.children {
        match child.name.as_str() {
            "domain_participant_qos" => {
                set_inline_qos(ctx, &mut entry.qos, "domain_participant", child)?;
            }
            "register_type" => entry.register_types.push(parse_register_type(ctx, child)?),
            "topic" => entry.topics.push(parse_topic_entry(ctx, child)?),
            "publisher" => entry.publishers.push(parse_publisher_element(ctx, child)?),
            "subscriber" => entry
                .subscribers
                .push(parse_subscriber_element(ctx, child)?),
            // §7.3.5.4.1: entities nest under their parent. A child under
            // the wrong parent is reported, not dropped.
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(entry)
}

fn parse_publisher_element(ctx: &mut Ctx, el: &XmlElement) -> Result<PublisherEntry, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("publisher@name".into()))?
        .to_string();
    let mut qos: Option<EntityQos> = None;
    let mut data_writers = Vec::new();
    for child in &el.children {
        match child.name.as_str() {
            "publisher_qos" => set_inline_qos(ctx, &mut qos, "publisher", child)?,
            "data_writer" => data_writers.push(parse_dw_element(ctx, child)?),
            // §7.3.5.4.1: a DataReader belongs to a Subscriber, not a
            // Publisher. A misplaced or unknown child is reported.
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(PublisherEntry {
        name,
        qos,
        data_writers,
    })
}

fn parse_subscriber_element(ctx: &mut Ctx, el: &XmlElement) -> Result<SubscriberEntry, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("subscriber@name".into()))?
        .to_string();
    let mut qos: Option<EntityQos> = None;
    let mut data_readers = Vec::new();
    for child in &el.children {
        match child.name.as_str() {
            "subscriber_qos" => set_inline_qos(ctx, &mut qos, "subscriber", child)?,
            "data_reader" => data_readers.push(parse_dr_element(ctx, child)?),
            // §7.3.5.4.1: a DataWriter belongs to a Publisher, not a
            // Subscriber.
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(SubscriberEntry {
        name,
        qos,
        data_readers,
    })
}

fn parse_dw_element(ctx: &mut Ctx, el: &XmlElement) -> Result<DataWriterEntry, XmlError> {
    // `qos_profile_ref` is a ZeroDDS attribute, not the schema's.
    ctx.check_attributes(el, &["name", "topic_ref", "qos_profile_ref"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("data_writer@name".into()))?
        .to_string();
    let topic_ref = el
        .attribute("topic_ref")
        .ok_or_else(|| XmlError::MissingRequiredElement("data_writer@topic_ref".into()))?
        .to_string();
    let qos_profile_ref = crate::domain::qos_profile_ref(ctx, el, "datawriter_qos")?;
    let mut qos: Option<EntityQos> = None;
    for child in &el.children {
        match child.name.as_str() {
            "datawriter_qos" => set_inline_qos(ctx, &mut qos, "data_writer", child)?,
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(DataWriterEntry {
        name,
        topic_ref,
        qos,
        qos_profile_ref,
    })
}

fn parse_dr_element(ctx: &mut Ctx, el: &XmlElement) -> Result<DataReaderEntry, XmlError> {
    ctx.check_attributes(el, &["name", "topic_ref", "qos_profile_ref"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("data_reader@name".into()))?
        .to_string();
    let topic_ref = el
        .attribute("topic_ref")
        .ok_or_else(|| XmlError::MissingRequiredElement("data_reader@topic_ref".into()))?
        .to_string();
    let qos_profile_ref = crate::domain::qos_profile_ref(ctx, el, "datareader_qos")?;
    let mut entry = DataReaderEntry {
        name,
        topic_ref,
        qos_profile_ref,
        ..DataReaderEntry::default()
    };
    for child in &el.children {
        match child.name.as_str() {
            "datareader_qos" => set_inline_qos(ctx, &mut entry.qos, "data_reader", child)?,
            "content_filter" => {
                if entry.content_filter.is_some() {
                    return Err(XmlError::InvalidXml(format!(
                        "<{}> carries <content_filter> more than once",
                        el.path
                    )));
                }
                entry.content_filter = Some(parse_filter(ctx, child)?);
            }
            "query_condition" => entry.query_conditions.push(parse_filter(ctx, child)?),
            "status_condition" => parse_status_condition(ctx, child, &mut entry.status_conditions)?,
            "read_condition" => entry
                .read_conditions
                .push(parse_read_condition(ctx, child)?),
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(entry)
}

/// Checks `value` (whitespace collapsed, as the schema's `statusKind` and
/// siblings say) against the enumeration of the schema.
fn enum_value(el: &XmlElement, allowed: &[&str]) -> Result<String, XmlError> {
    let v = el.text.split_whitespace().collect::<Vec<_>>().join(" ");
    if allowed.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(XmlError::BadEnum(format!("{}: `{v}`", el.path)))
    }
}

/// Schema `filter`: `name` and `kind` attributes, `<expression>` and
/// `<parameter_list>` (up to 100 `<param>`).
fn parse_filter(ctx: &mut Ctx, el: &XmlElement) -> Result<FilterDef, XmlError> {
    ctx.check_attributes(el, &["name", "kind"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("{}@name", el.name)))?
        .to_string();
    let kind = el
        .attribute("kind")
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("{}@kind", el.name)))?;
    if !["builtin.sql", "builtin.stringMatch"].contains(&kind) {
        return Err(XmlError::BadEnum(format!("{}@kind: `{kind}`", el.path)));
    }
    let mut expression = None;
    let mut parameters = Vec::new();
    for child in &el.children {
        match child.name.as_str() {
            "expression" => {
                if expression.is_some() {
                    return Err(XmlError::InvalidXml(format!(
                        "<{}> carries <expression> more than once",
                        el.path
                    )));
                }
                expression = Some(child.text.clone());
            }
            "parameter_list" => {
                ctx.check_attributes(child, &[])?;
                for param in &child.children {
                    if param.name == "param" {
                        parameters.push(param.text.clone());
                    } else {
                        ctx.unknown_child(param)?;
                    }
                }
                if parameters.len() > 100 {
                    return Err(XmlError::LimitExceeded(format!(
                        "<{}> has {} <param>, the schema allows 100",
                        child.path,
                        parameters.len()
                    )));
                }
            }
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(FilterDef {
        name,
        kind: kind.to_string(),
        expression,
        parameters,
    })
}

const STATUS_KINDS: &[&str] = &[
    "DATA_AVAILABLE",
    "INCONSISTENT_TOPIC",
    "OFFERED_DEADLINE_MISSED",
    "REQUESTED_DEADLINE_MISSED",
    "OFFERED_INCOMPATIBLE_QOS",
    "REQUESTED_INCOMPATIBLE_QOS",
    "SAMPLE_LOST",
    "SAMPLE_REJECTED",
    "LIVELINESS_LOST",
    "LIVELINESS_CHANGED",
    "PUBLICATION_MATCHED",
    "SUBSCRIPTION_MATCHED",
    "DATA_ON_READERS",
];

/// Schema `statusCondition`: one or more `<status>`.
fn parse_status_condition(
    ctx: &mut Ctx,
    el: &XmlElement,
    out: &mut Vec<String>,
) -> Result<(), XmlError> {
    ctx.check_attributes(el, &[])?;
    for child in &el.children {
        if child.name == "status" {
            out.push(enum_value(child, STATUS_KINDS)?);
        } else {
            ctx.unknown_child(child)?;
        }
    }
    Ok(())
}

/// Schema `readCondition`: `name` and the three state lists.
fn parse_read_condition(ctx: &mut Ctx, el: &XmlElement) -> Result<ReadConditionDef, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("read_condition@name".into()))?
        .to_string();
    let mut def = ReadConditionDef {
        name,
        ..ReadConditionDef::default()
    };
    for child in &el.children {
        match child.name.as_str() {
            "sample_state" => def
                .sample_states
                .push(enum_value(child, &["READ", "NOT_READ"])?),
            "view_state" => def
                .view_states
                .push(enum_value(child, &["NEW", "NOT_NEW"])?),
            "instance_state" => def.instance_states.push(enum_value(
                child,
                &["ALIVE", "NOT_ALIVE_DISPOSED", "NOT_ALIVE_NO_WRITERS"],
            )?),
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(def)
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
        assert_eq!(
            libs[0].participants[0].domain_ref.as_deref(),
            Some("my_lib::MyDomain")
        );
    }

    #[test]
    fn parse_pub_with_writer() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant name="P" domain_ref="dl::D">
              <publisher name="Pub1">
                <data_writer name="W1" topic_ref="StateTopic">
                  <datawriter_qos>
                    <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
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

    /// `domain_ref` carries no `use="required"` in the schema, so its
    /// absence is a document to parse and not an error to raise. What
    /// cannot be done without it is *resolution*, and that is where the
    /// error belongs — `DdsXml::resolve_participant` says
    /// `missing domain_ref` with the participant's path in it.
    #[test]
    fn missing_domain_ref_parses_and_is_left_to_the_resolver() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant name="P"/>
          </domain_participant_library>
        </dds>"#;
        let libs = parse_domain_participant_libraries(xml).expect("parse");
        assert_eq!(libs[0].participants[0].name, "P");
        assert_eq!(libs[0].participants[0].domain_ref, None);
        assert_eq!(libs[0].participants[0].domain_id, None);
    }

    /// §7.3.5.4.2 plus the Domain building block's types: a
    /// participant's `<register_type>` and `<topic>` children are
    /// definitions with `name` and `type_ref`/`register_type_ref` —
    /// the same complex types a `<domain>` uses — not references into
    /// a domain.
    #[test]
    fn participant_topic_and_register_type_are_definitions() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant name="P" domain_id="0">
              <register_type name="StateType" type_ref="MyTypes::State"/>
              <topic name="StateTopic" register_type_ref="StateType"/>
            </domain_participant>
          </domain_participant_library>
        </dds>"#;
        let libs = parse_domain_participant_libraries(xml).expect("parse");
        let p = &libs[0].participants[0];
        assert_eq!(p.register_types.len(), 1);
        assert_eq!(p.register_types[0].name, "StateType");
        assert_eq!(p.register_types[0].type_ref, "MyTypes::State");
        assert_eq!(p.topics.len(), 1);
        assert_eq!(p.topics[0].name, "StateTopic");
        assert_eq!(p.topics[0].register_type_ref, "StateType");
    }

    /// The definitions carry the same required attributes as in a
    /// domain (§7.3.4.4.1); a `ref` attribute is in no schema, so the
    /// attribute check names it before the missing `name` is reached.
    #[test]
    fn participant_topic_by_ref_attribute_rejected() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant name="P" domain_id="0">
              <topic ref="Square"/>
            </domain_participant>
          </domain_participant_library>
        </dds>"#;
        let err = parse_domain_participant_libraries(xml).expect_err("ref");
        assert!(
            matches!(&err, XmlError::UnknownElement(m) if m.contains("@ref")),
            "{err:?}"
        );
    }

    #[test]
    fn participant_register_type_without_type_ref_rejected() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant name="P" domain_id="0">
              <register_type name="HelloWorld"/>
            </domain_participant>
          </domain_participant_library>
        </dds>"#;
        let err = parse_domain_participant_libraries(xml).expect_err("missing type_ref");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    /// `name` is the attribute the schema does mark required.
    #[test]
    fn missing_participant_name_rejected() {
        let xml = r#"<dds>
          <domain_participant_library name="dpl">
            <domain_participant domain_ref="dl::D"/>
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
