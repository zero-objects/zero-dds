// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Parses the contained-entity lists Spec §7.4.3.1/§7.4.3.2
//! (`participantObjectRepresentation`), §7.4.4.7/§7.4.4.8
//! (`publisherObjectRepresentation`) and §7.4.4.11/§7.4.4.12
//! (`subscriberObjectRepresentation`) allow: a `DomainParticipant`'s
//! `<publishers>`/`<subscribers>`, a `Publisher`'s `<data_writers>`, a
//! `Subscriber`'s `<data_readers>` — each a DDS-XTYPES XML sequence
//! (`<publishers><element>...</element>...</publishers>`,
//! `XmlElement::sequence_elements`), each element the same
//! `entity`-shaped representation (`<name>`, optionally `<qos>`/
//! `<qos_profile>`) every other WebDDS entity uses.
//!
//! Unlike a top-level `create_publisher`/`create_data_writer` (whose
//! name the gateway always generates, `crate::gateway`'s own module
//! doc), a *contained* entity's `<name>` is the caller's own —
//! `update_participant`/`update_publisher`/`update_subscriber`'s own
//! reconciliation rule ("For each contained entity that already
//! exists...") only makes sense if the same name in two calls means
//! the same entity. `crate::gateway::Gateway`'s internal maps already
//! key by a plain `String`, so this is not a second naming scheme —
//! just a different source for the key.

use zerodds_web::bridge::BackendError;
use zerodds_xml::{EntityQos, XmlElement};

use crate::qos_update::EntityKind;

use crate::metadata::MetadataStore;

/// A contained `DataWriter` (Spec §7.4.4.7 rule, within a
/// `publisherObjectRepresentation`'s `<data_writers>`).
#[derive(Debug, Clone)]
pub struct ParsedDataWriter {
    /// The caller's own identifier for this writer.
    pub name: String,
    /// The `Topic` it writes — must already exist on the enclosing
    /// participant.
    pub topic_name: String,
    /// Its QoS/QosProfile, if named.
    pub qos: Option<EntityQos>,
}

/// A contained `DataReader`. See [`ParsedDataWriter`].
#[derive(Debug, Clone)]
pub struct ParsedDataReader {
    /// The caller's own identifier for this reader.
    pub name: String,
    /// The `Topic` it reads — must already exist on the enclosing
    /// participant.
    pub topic_name: String,
    /// Its QoS/QosProfile, if named.
    pub qos: Option<EntityQos>,
}

/// A contained `Publisher`, with its own contained `DataWriter`s.
#[derive(Debug, Clone)]
pub struct ParsedPublisher {
    /// The caller's own identifier for this publisher.
    pub name: String,
    /// Its QoS/QosProfile, if named.
    pub qos: Option<EntityQos>,
    /// Contained `DataWriter`s, in document order.
    pub data_writers: Vec<ParsedDataWriter>,
}

/// A contained `Subscriber`, with its own contained `DataReader`s.
#[derive(Debug, Clone)]
pub struct ParsedSubscriber {
    /// The caller's own identifier for this subscriber.
    pub name: String,
    /// Its QoS/QosProfile, if named.
    pub qos: Option<EntityQos>,
    /// Contained `DataReader`s, in document order.
    pub data_readers: Vec<ParsedDataReader>,
}

fn required_name(el: &XmlElement, what: &str) -> Result<String, BackendError> {
    el.child("name")
        .map(|e| e.text.clone())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| BackendError::BadRequest(format!("a contained {what} needs a <name>")))
}

/// Parses `root`'s `<publishers>` sequence, if present — empty where
/// `root` names none, which is a legitimate "no contained publishers",
/// not an error.
///
/// # Errors
/// [`BackendError::BadRequest`] if an element is missing `<name>`, or
/// its own QoS/QosProfile does not resolve (see
/// `crate::qos_update::resolve_entity_qos_from_element`).
pub fn parse_publishers(
    root: &XmlElement,
    metadata: &MetadataStore,
) -> Result<Vec<ParsedPublisher>, BackendError> {
    let Some(container) = root.child("publishers") else {
        return Ok(Vec::new());
    };
    container
        .sequence_elements()
        .map(|el| parse_publisher(el, metadata))
        .collect()
}

fn parse_publisher(
    el: &XmlElement,
    metadata: &MetadataStore,
) -> Result<ParsedPublisher, BackendError> {
    let name = required_name(el, "publisher")?;
    let qos =
        crate::qos_update::resolve_entity_qos_from_element(el, metadata, EntityKind::Publisher)?;
    let data_writers = parse_data_writers(el, metadata)?;
    Ok(ParsedPublisher {
        name,
        qos,
        data_writers,
    })
}

/// Parses `root`'s `<data_writers>` sequence, if present.
///
/// # Errors
/// See [`parse_publishers`].
pub fn parse_data_writers(
    root: &XmlElement,
    metadata: &MetadataStore,
) -> Result<Vec<ParsedDataWriter>, BackendError> {
    let Some(container) = root.child("data_writers") else {
        return Ok(Vec::new());
    };
    container
        .sequence_elements()
        .map(|el| {
            let name = required_name(el, "data_writer")?;
            let topic_name = el
                .child("topic_name")
                .map(|e| e.text.clone())
                .unwrap_or_default();
            // §7.3.2.4.3: the profile's `topic_filter`s are evaluated
            // against this writer's topic name.
            let qos = crate::qos_update::resolve_entity_qos_from_element_for_topic(
                el,
                metadata,
                EntityKind::DataWriter,
                &topic_name,
            )?;
            Ok(ParsedDataWriter {
                name,
                topic_name,
                qos,
            })
        })
        .collect()
}

/// Parses `root`'s `<subscribers>` sequence, if present.
///
/// # Errors
/// See [`parse_publishers`].
pub fn parse_subscribers(
    root: &XmlElement,
    metadata: &MetadataStore,
) -> Result<Vec<ParsedSubscriber>, BackendError> {
    let Some(container) = root.child("subscribers") else {
        return Ok(Vec::new());
    };
    container
        .sequence_elements()
        .map(|el| parse_subscriber(el, metadata))
        .collect()
}

fn parse_subscriber(
    el: &XmlElement,
    metadata: &MetadataStore,
) -> Result<ParsedSubscriber, BackendError> {
    let name = required_name(el, "subscriber")?;
    let qos =
        crate::qos_update::resolve_entity_qos_from_element(el, metadata, EntityKind::Subscriber)?;
    let data_readers = parse_data_readers(el, metadata)?;
    Ok(ParsedSubscriber {
        name,
        qos,
        data_readers,
    })
}

/// Parses `root`'s `<data_readers>` sequence, if present.
///
/// # Errors
/// See [`parse_publishers`].
pub fn parse_data_readers(
    root: &XmlElement,
    metadata: &MetadataStore,
) -> Result<Vec<ParsedDataReader>, BackendError> {
    let Some(container) = root.child("data_readers") else {
        return Ok(Vec::new());
    };
    container
        .sequence_elements()
        .map(|el| {
            let name = required_name(el, "data_reader")?;
            let topic_name = el
                .child("topic_name")
                .map(|e| e.text.clone())
                .unwrap_or_default();
            // §7.3.2.4.3: the profile's `topic_filter`s are evaluated
            // against this reader's topic name.
            let qos = crate::qos_update::resolve_entity_qos_from_element_for_topic(
                el,
                metadata,
                EntityKind::DataReader,
                &topic_name,
            )?;
            Ok(ParsedDataReader {
                name,
                topic_name,
                qos,
            })
        })
        .collect()
}

/// Parses the whole `body` (a `participant`/`publisher`/`subscriber`
/// object representation) into its root element, or `None` for an
/// empty body — the same "no representation named, nothing to do"
/// no-op every `update_*` already treats as legitimate.
///
/// # Errors
/// [`BackendError::BadRequest`] if `body` is not UTF-8 or not
/// well-formed XML.
pub fn parse_body(body: &[u8]) -> Result<Option<zerodds_xml::DdsXmlDocument>, BackendError> {
    if body.is_empty() {
        return Ok(None);
    }
    let text = core::str::from_utf8(body)
        .map_err(|e| BackendError::BadRequest(format!("body is not UTF-8: {e}")))?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    zerodds_xml::parse_xml_tree(text)
        .map(Some)
        .map_err(|e| BackendError::BadRequest(format!("malformed object body: {e}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_body_with_no_publishers_element_has_none() {
        let doc = parse_body(b"<domain_participant><domain_id>0</domain_id></domain_participant>")
            .unwrap()
            .unwrap();
        assert!(
            parse_publishers(&doc.root, &MetadataStore::new())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn nested_publishers_and_data_writers_parse() {
        let body = b"<domain_participant><publishers><element><name>pub1</name><data_writers><element><name>dw1</name><topic_name>Sensor</topic_name></element></data_writers></element></publishers></domain_participant>";
        let doc = parse_body(body).unwrap().unwrap();
        let pubs = parse_publishers(&doc.root, &MetadataStore::new()).unwrap();
        assert_eq!(pubs.len(), 1);
        assert_eq!(pubs[0].name, "pub1");
        assert_eq!(pubs[0].data_writers.len(), 1);
        assert_eq!(pubs[0].data_writers[0].name, "dw1");
        assert_eq!(pubs[0].data_writers[0].topic_name, "Sensor");
    }

    #[test]
    fn a_contained_entity_without_a_name_is_a_bad_request() {
        let body =
            b"<domain_participant><publishers><element><data_writers/></element></publishers></domain_participant>";
        let doc = parse_body(body).unwrap().unwrap();
        let err = parse_publishers(&doc.root, &MetadataStore::new()).unwrap_err();
        assert!(matches!(err, BackendError::BadRequest(_)));
    }

    #[test]
    fn a_publishers_qos_profile_reference_resolves() {
        let metadata = MetadataStore::new();
        metadata
            .qos_libraries
            .create(
                "Lib",
                crate::metadata::StoredDocument {
                    content_type: "application/dds-web+xml".to_string(),
                    body: b"<qos_library><name>Lib</name></qos_library>".to_vec(),
                },
            )
            .unwrap();
        metadata
            .qos_profiles
            .create(
                &MetadataStore::profile_key("Lib", "P"),
                crate::metadata::StoredDocument {
                    content_type: "application/dds-web+xml".to_string(),
                    body: b"<qos_profile><name>P</name><publisher_qos><partition><name><element>g1</element></name></partition></publisher_qos></qos_profile>".to_vec(),
                },
            )
            .unwrap();
        let body = b"<domain_participant><publishers><element><name>pub1</name><qos_profile>Lib::P</qos_profile></element></publishers></domain_participant>";
        let doc = parse_body(body).unwrap().unwrap();
        let pubs = parse_publishers(&doc.root, &metadata).unwrap();
        assert_eq!(
            pubs[0]
                .qos
                .as_ref()
                .unwrap()
                .partition
                .as_ref()
                .unwrap()
                .names,
            vec!["g1".to_string()]
        );
    }
}
