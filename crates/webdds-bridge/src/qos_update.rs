// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Turns a REST/SOAP `PUT` body's inline `<qos>` container into a live
//! `zerodds_dcps::qos` value, for [`crate::gateway::Gateway`]'s
//! `update_*` operations — Spec §7.4 `update_topic`/`update_publisher`/
//! `update_subscriber`/`update_data_writer`/`update_data_reader`/
//! `update_participant`, all of which "change the QoS" of the
//! underlying DDS entity (the exact wording §7.4.4.4/§7.4.5.2/§7.4.6.2
//! use).
//!
//! `zerodds_xml::EntityQos` carries every DDS 1.4 §2.2.3 policy as an
//! `Option` — `None` meaning "not named in this `<qos>`", `Some`
//! meaning explicitly set. Every `<entity>_qos_from` function here
//! **merges** onto `base` — the entity's own current, live QoS
//! (`Entity::get_qos()`), not a fresh `<Entity>Qos::default()` — so a
//! `<qos>` that names one policy leaves every other one exactly as the
//! entity already holds it.
//!
//! This is a deliberate correctness choice, not the more literal
//! reading of Spec §8.3.3 Tab 5 ("the same format as when the object
//! was created"): `zerodds_qos`'s own policy defaults
//! (`ResourceLimitsQosPolicy::default()` = `LENGTH_UNLIMITED` fields)
//! differ from `zerodds_dcps::DataWriterQos::default()`'s
//! (`max_samples=1000`, …) — a real, measured mismatch (this module's
//! own `writer_qos_from`/`reader_qos_from` hit it building on
//! `<Entity>Qos::default()` in an earlier version of this file: a
//! `<qos>` naming only `<deadline>` still tripped `ImmutablePolicy` on
//! `RESOURCE_LIMITS`, a policy the caller never mentioned).
//! Changeable=NO-post-enable policies (Spec §2.2.3) make that
//! consequential, not cosmetic: a "replace the whole representation"
//! `update_*` would refuse on any field the two default sets disagree
//! on, for every entity this daemon creates (which enables every
//! entity immediately, Spec §7.4.4.7/§7.4.5.1). Merging onto the
//! entity's own current QoS has no such mismatch to trip over.
//!
//! **`<qos_profile>library::profile</qos_profile>`** — a *reference*
//! to a stored `QosLibrary`/`QosProfile` (Spec §7.4.4.4's "a QoS or
//! QosProfile") — also resolves, through [`resolve_qos_profile_ref`]:
//! every configured `QosLibrary`'s stored profiles (`crate::metadata`
//! stores the posted XML opaquely — Spec §7.3.1 is metadata-only, no
//! DCPS round trip — so they are re-parsed on each reference rather
//! than kept in a second, synchronized index) go through
//! `zerodds_xml::qos_inheritance::resolve_profile`, which also
//! resolves `base_name` inheritance, cross-library included. Where a
//! body names both an inline `<qos>` and a `<qos_profile>`, the inline
//! one wins per field (`EntityQos::merge`'s own "override wins" rule)
//! — Spec §7.4.4.8 rule for `update_publisher` reads the same way for
//! its own QoS-vs-QosProfile case.
//!
//! `create_topic`/`create_publisher`/`create_subscriber`/
//! `create_data_writer`/`create_data_reader`/`create_participant` take
//! no QoS at all today (inline or by reference), so resolving one on
//! `update_*` is a net addition, not a narrowing of what a client
//! could already rely on — see the crate README "Open points".

use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_web::bridge::BackendError;
use zerodds_xml::{EntityQos, FilteredEntityQos};

use crate::metadata::MetadataStore;

/// The kind of entity a QoS is for: it fixes the container of a profile
/// that applies and the policies an inline `<qos>` may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    /// Topic (`topic_qos`).
    Topic,
    /// Publisher (`publisher_qos`).
    Publisher,
    /// Subscriber (`subscriber_qos`).
    Subscriber,
    /// DataWriter (`datawriter_qos`).
    DataWriter,
    /// DataReader (`datareader_qos`).
    DataReader,
    /// DomainParticipant (`domain_participant_qos`).
    DomainParticipant,
}

impl EntityKind {
    /// The element name of the QoS container of this entity in the
    /// specification's schema.
    fn container(self) -> &'static str {
        match self {
            Self::Topic => "topic_qos",
            Self::Publisher => "publisher_qos",
            Self::Subscriber => "subscriber_qos",
            Self::DataWriter => "datawriter_qos",
            Self::DataReader => "datareader_qos",
            Self::DomainParticipant => "domain_participant_qos",
        }
    }

    /// The slots of a resolved profile that belong to this entity.
    fn slots(self, r: &zerodds_xml::ResolvedQos) -> &[FilteredEntityQos] {
        match self {
            Self::Topic => &r.topic_qos,
            Self::Publisher => &r.publisher_qos,
            Self::Subscriber => &r.subscriber_qos,
            Self::DataWriter => &r.datawriter_qos,
            Self::DataReader => &r.datareader_qos,
            Self::DomainParticipant => &r.domainparticipant_qos,
        }
    }
}

/// Parses `body` (a full entity object representation, e.g.
/// `<topic>...<qos>...</qos></topic>`, or
/// `<topic>...<qos_profile>Lib::Prof</qos_profile></topic>`) and
/// resolves the [`EntityQos`] it names for one entity kind — `kind`
/// picks which of [`zerodds_xml::ResolvedQos`]'s six QoS container
/// slot lists applies (e.g. `EntityKind::Topic` for
/// `update_topic`) and which policies an inline `<qos>` may hold: the
/// fields of that entity's QoS structure (DDS 1.4 §2.2.3, the normative
/// schema's `topicQos`, `datawriterQos`, …). `None` where the body is empty and names neither a
/// `<qos>` nor a `<qos_profile>` — "update, changing nothing" is a
/// legitimate no-op PUT, not an error.
///
/// The profile's slot list is narrowed to one [`EntityQos`] per
/// §7.3.2.4.3: no topic name at hand here, so the filterless container
/// applies, else the first one in document order
/// ([`zerodds_xml::default_qos`]). Where the caller knows the entity's
/// topic name, [`resolve_entity_qos_for_topic`] evaluates the
/// `topic_filter`s instead.
///
/// # Errors
/// [`BackendError::BadRequest`] if `body` is not UTF-8, not
/// well-formed XML, names a policy element this crate does not model,
/// or a `<qos_profile>` reference does not resolve (unknown library,
/// unknown profile, or an inheritance cycle).
pub fn resolve_entity_qos(
    body: &[u8],
    metadata: &MetadataStore,
    kind: EntityKind,
) -> Result<Option<EntityQos>, BackendError> {
    resolve_entity_qos_inner(body, metadata, kind, None)
}

/// Like [`resolve_entity_qos`], but the entity's `topic_name` narrows
/// the profile's slot list through §7.3.2.4.3 `topic_filter`
/// evaluation ([`zerodds_xml::select_qos_for_topic`]): the first
/// container whose filter matches wins; a filterless container applies
/// where none matches.
///
/// # Errors
/// See [`resolve_entity_qos`].
pub fn resolve_entity_qos_for_topic(
    body: &[u8],
    metadata: &MetadataStore,
    kind: EntityKind,
    topic_name: &str,
) -> Result<Option<EntityQos>, BackendError> {
    resolve_entity_qos_inner(body, metadata, kind, Some(topic_name))
}

fn resolve_entity_qos_inner(
    body: &[u8],
    metadata: &MetadataStore,
    kind: EntityKind,
    topic_name: Option<&str>,
) -> Result<Option<EntityQos>, BackendError> {
    if body.is_empty() {
        return Ok(None);
    }
    let text = core::str::from_utf8(body)
        .map_err(|e| BackendError::BadRequest(format!("body is not UTF-8: {e}")))?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    let doc = zerodds_xml::parse_xml_tree(text)
        .map_err(|e| BackendError::BadRequest(format!("malformed object body: {e}")))?;
    resolve_entity_qos_from_element_inner(&doc.root, metadata, kind, topic_name)
}

/// The element-level core of [`resolve_entity_qos`] — shared with
/// `crate::contained_entities`, whose nested `<publisher>`/
/// `<data_writer>`/... elements are already part of a parsed tree (the
/// participant's own body), not a separate `&[u8]` to reparse.
///
/// # Errors
/// See [`resolve_entity_qos`].
pub fn resolve_entity_qos_from_element(
    el: &zerodds_xml::XmlElement,
    metadata: &MetadataStore,
    kind: EntityKind,
) -> Result<Option<EntityQos>, BackendError> {
    resolve_entity_qos_from_element_inner(el, metadata, kind, None)
}

/// Like [`resolve_entity_qos_from_element`], but the entity's
/// `topic_name` narrows the profile's slot list through §7.3.2.4.3
/// `topic_filter` evaluation.
///
/// # Errors
/// See [`resolve_entity_qos`].
pub fn resolve_entity_qos_from_element_for_topic(
    el: &zerodds_xml::XmlElement,
    metadata: &MetadataStore,
    kind: EntityKind,
    topic_name: &str,
) -> Result<Option<EntityQos>, BackendError> {
    resolve_entity_qos_from_element_inner(el, metadata, kind, Some(topic_name))
}

fn resolve_entity_qos_from_element_inner(
    el: &zerodds_xml::XmlElement,
    metadata: &MetadataStore,
    kind: EntityKind,
    topic_name: Option<&str>,
) -> Result<Option<EntityQos>, BackendError> {
    let profile_eq = match el.child("qos_profile") {
        Some(el) => {
            let path = el.text.trim();
            if path.is_empty() {
                None
            } else {
                let resolved = resolve_qos_profile_ref(metadata, path)?;
                match topic_name {
                    Some(topic) => {
                        zerodds_xml::select_qos_for_topic(kind.slots(&resolved), topic).cloned()
                    }
                    None => zerodds_xml::default_qos(kind.slots(&resolved)).cloned(),
                }
            }
        }
        None => None,
    };
    let inline_eq = match el.child("qos") {
        Some(el) => {
            // The body names the QoS `<qos>`; the policies it may hold are
            // those of the entity's QoS structure, so it is read as that
            // container: a policy the entity has not is refused, not
            // dropped by the conversion.
            let mut as_container = el.clone();
            as_container.name = kind.container().to_string();
            let mut inline = zerodds_xml::parse_entity_qos_public(&as_container)
                .map_err(|e| BackendError::BadRequest(format!("<qos>: {e}")))?;
            // DDS-XML 1.0 §7.3.5.4.4: "Inline entities may inherit from an
            // existing QoS Profile using the base_name XML attribute." The
            // named profile's container is the base, the inline policies
            // override it.
            match inline.base_name.take() {
                Some(base) => {
                    let resolved = resolve_qos_profile_ref(metadata, &base)?;
                    let slots = kind.slots(&resolved);
                    let base_eq = match topic_name {
                        Some(topic) => zerodds_xml::select_qos_for_topic(slots, topic),
                        None => zerodds_xml::default_qos(slots),
                    };
                    Some(match base_eq {
                        Some(b) => b.clone().merge(&inline),
                        None => inline,
                    })
                }
                None => Some(inline),
            }
        }
        None => None,
    };
    Ok(match (profile_eq, inline_eq) {
        (None, None) => None,
        (Some(p), None) => Some(p),
        (None, Some(i)) => Some(i),
        // `EntityQos::merge(self, override_)`: "child wins where the
        // child explicitly set a value" — the inline `<qos>` is the
        // override here, per this module's own doc.
        (Some(p), Some(i)) => Some(p.merge(&i)),
    })
}

/// Resolves `library::profile` against every `QosLibrary` this daemon
/// has stored (`metadata.qos_libraries`, plus each library's separately
/// stored `QosProfile`s under `metadata.qos_profiles`) — cross-library
/// `base_name` inheritance needs every library in scope, not just the
/// one `path` names.
///
/// # Errors
/// [`BackendError::BadRequest`] if `path` is not `library::profile`,
/// names an unknown library/profile, or the inheritance chain cycles.
pub fn resolve_qos_profile_ref(
    metadata: &MetadataStore,
    path: &str,
) -> Result<zerodds_xml::ResolvedQos, BackendError> {
    let libraries = all_qos_libraries(metadata);
    zerodds_xml::resolve_profile(&libraries, path)
        .map_err(|e| BackendError::BadRequest(format!("<qos_profile>{path}</qos_profile>: {e}")))
}

/// Every stored `QosLibrary`, each with its stored `QosProfile`s
/// attached — built from `metadata`'s opaque XML storage on each call
/// (Spec §7.3.1's QosLibrary/QosProfile CRUD keeps no other index).
fn all_qos_libraries(metadata: &MetadataStore) -> Vec<zerodds_xml::QosLibrary> {
    metadata
        .qos_libraries
        .names()
        .into_iter()
        .map(|lib_name| {
            let profiles = metadata
                .qos_profiles
                .names()
                .into_iter()
                .filter_map(|key| {
                    let (lib, _profile) = key.split_once('/')?;
                    (lib == lib_name).then_some(key)
                })
                .filter_map(|key| {
                    let doc = metadata.qos_profiles.get(&key).ok()?;
                    let text = core::str::from_utf8(&doc.body).ok()?;
                    parse_stored_qos_profile(text).ok()
                })
                .collect();
            zerodds_xml::QosLibrary {
                name: lib_name,
                profiles,
            }
        })
        .collect()
}

/// Parses a `QosProfile` from the same body shape `CreateQosProfile`
/// stored it in — the DDS-WEB REST/SOAP object representation
/// (`<qos_profile><name>…</name><datawriter_qos>…</datawriter_qos>…
/// </qos_profile>`, fields as *child elements*), not DDS-XML 1.0's own
/// config-file convention (`<qos_profile name="…">`, an *attribute*) —
/// `zerodds_xml::qos_parser`'s own parser expects the latter and cannot
/// read the former; this daemon's stored bodies are always the former,
/// so this reads them directly rather than reusing that parser.
fn parse_stored_qos_profile(xml: &str) -> Result<zerodds_xml::QosProfile, BackendError> {
    let doc = zerodds_xml::parse_xml_tree(xml)
        .map_err(|e| BackendError::BadRequest(format!("stored qos_profile: {e}")))?;
    let root = &doc.root;
    let mut profile = zerodds_xml::QosProfile {
        name: root
            .child("name")
            .map(|e| e.text.clone())
            .unwrap_or_default(),
        base_name: root.child("base_name").map(|e| e.text.clone()),
        topic_filter: root.child("topic_filter").map(|e| e.text.clone()),
        datawriter_qos: Vec::new(),
        datareader_qos: Vec::new(),
        topic_qos: Vec::new(),
        publisher_qos: Vec::new(),
        subscriber_qos: Vec::new(),
        domainparticipant_qos: Vec::new(),
    };
    // The REST/SOAP object representation has one container per kind
    // and no `topic_filter` attribute — each container becomes the
    // single, filterless slot of its kind.
    let slot =
        |el: &zerodds_xml::XmlElement, kind: &str| -> Result<FilteredEntityQos, BackendError> {
            Ok(FilteredEntityQos {
                topic_filter: None,
                name: None,
                qos: zerodds_xml::parse_entity_qos_public(el)
                    .map_err(|e| BackendError::BadRequest(format!("{kind}: {e}")))?,
            })
        };
    if let Some(el) = root.child("datawriter_qos") {
        profile.datawriter_qos.push(slot(el, "datawriter_qos")?);
    }
    if let Some(el) = root.child("datareader_qos") {
        profile.datareader_qos.push(slot(el, "datareader_qos")?);
    }
    if let Some(el) = root.child("topic_qos") {
        profile.topic_qos.push(slot(el, "topic_qos")?);
    }
    if let Some(el) = root.child("publisher_qos") {
        profile.publisher_qos.push(slot(el, "publisher_qos")?);
    }
    if let Some(el) = root.child("subscriber_qos") {
        profile.subscriber_qos.push(slot(el, "subscriber_qos")?);
    }
    if let Some(el) = root.child("domainparticipant_qos") {
        profile
            .domainparticipant_qos
            .push(slot(el, "domainparticipant_qos")?);
    }
    Ok(profile)
}

/// Merges `eq` onto `base` (the `Topic`'s own current QoS) — Spec
/// §2.2.2.3.2's 13 Topic policies, unset in `eq` left exactly as `base`
/// held it.
#[must_use]
pub fn topic_qos_from(eq: &EntityQos, mut base: TopicQos) -> TopicQos {
    if let Some(p) = eq.durability {
        base.durability = p;
    }
    if let Some(p) = eq.durability_service {
        base.durability_service = p;
    }
    if let Some(p) = eq.deadline {
        base.deadline = p;
    }
    if let Some(p) = eq.latency_budget {
        base.latency_budget = p;
    }
    if let Some(p) = eq.liveliness {
        base.liveliness = p;
    }
    if let Some(p) = eq.reliability {
        base.reliability = p;
    }
    if let Some(p) = eq.destination_order {
        base.destination_order = p;
    }
    if let Some(p) = eq.history {
        base.history = p;
    }
    if let Some(p) = eq.resource_limits {
        base.resource_limits = p;
    }
    if let Some(p) = eq.transport_priority {
        base.transport_priority = p;
    }
    if let Some(p) = eq.lifespan {
        base.lifespan = p;
    }
    if let Some(p) = eq.ownership {
        base.ownership = p;
    }
    if let Some(p) = eq.topic_data.clone() {
        base.topic_data = p;
    }
    base
}

/// Merges `eq` onto `base` (the `Publisher`'s own current QoS) — Spec
/// §2.2.2.4.1's 4 Publisher policies.
#[must_use]
pub fn publisher_qos_from(eq: &EntityQos, mut base: PublisherQos) -> PublisherQos {
    if let Some(p) = eq.presentation {
        base.presentation = p;
    }
    if let Some(p) = eq.partition.clone() {
        base.partition = p;
    }
    if let Some(p) = eq.group_data.clone() {
        base.group_data = p;
    }
    if let Some(p) = eq.entity_factory {
        base.entity_factory = p;
    }
    base
}

/// Merges `eq` onto `base` (the `Subscriber`'s own current QoS) — Spec
/// §2.2.2.5.1's 4 Subscriber policies (same set as
/// [`publisher_qos_from`]).
#[must_use]
pub fn subscriber_qos_from(eq: &EntityQos, mut base: SubscriberQos) -> SubscriberQos {
    if let Some(p) = eq.presentation {
        base.presentation = p;
    }
    if let Some(p) = eq.partition.clone() {
        base.partition = p;
    }
    if let Some(p) = eq.group_data.clone() {
        base.group_data = p;
    }
    if let Some(p) = eq.entity_factory {
        base.entity_factory = p;
    }
    base
}

/// Merges `eq` onto `base` (the `DomainParticipant`'s own current QoS)
/// — Spec §2.2.2.2.2's `user_data`/`entity_factory` (`property`, a
/// ZeroDDS-security-only field with no `EntityQos` counterpart, is
/// never touched).
#[must_use]
pub fn participant_qos_from(
    eq: &EntityQos,
    mut base: DomainParticipantQos,
) -> DomainParticipantQos {
    if let Some(p) = eq.user_data.clone() {
        base.user_data = p;
    }
    if let Some(p) = eq.entity_factory {
        base.entity_factory = p;
    }
    base
}

/// Merges `eq` onto `base` (the `DataWriter`'s own current QoS) — Spec
/// §2.2.2.4.2's 19 DataWriter policies (`data_representation` has no
/// `EntityQos` counterpart and is never touched).
#[must_use]
pub fn writer_qos_from(eq: &EntityQos, mut base: DataWriterQos) -> DataWriterQos {
    if let Some(p) = eq.reliability {
        base.reliability = p;
    }
    if let Some(p) = eq.durability {
        base.durability = p;
    }
    if let Some(p) = eq.durability_service {
        base.durability_service = p;
    }
    if let Some(p) = eq.deadline {
        base.deadline = p;
    }
    if let Some(p) = eq.latency_budget {
        base.latency_budget = p;
    }
    if let Some(p) = eq.liveliness {
        base.liveliness = p;
    }
    if let Some(p) = eq.destination_order {
        base.destination_order = p;
    }
    if let Some(p) = eq.lifespan {
        base.lifespan = p;
    }
    if let Some(p) = eq.ownership {
        base.ownership = p;
    }
    if let Some(p) = eq.ownership_strength {
        base.ownership_strength = p;
    }
    if let Some(p) = eq.partition.clone() {
        base.partition = p;
    }
    if let Some(p) = eq.presentation {
        base.presentation = p;
    }
    if let Some(p) = eq.history {
        base.history = p;
    }
    if let Some(p) = eq.resource_limits {
        base.resource_limits = p;
    }
    if let Some(p) = eq.transport_priority {
        base.transport_priority = p;
    }
    if let Some(p) = eq.writer_data_lifecycle {
        base.writer_data_lifecycle = p;
    }
    if let Some(p) = eq.user_data.clone() {
        base.user_data = p;
    }
    if let Some(p) = eq.topic_data.clone() {
        base.topic_data = p;
    }
    if let Some(p) = eq.group_data.clone() {
        base.group_data = p;
    }
    base
}

/// Merges `eq` onto `base` (the `DataReader`'s own current QoS) — Spec
/// §2.2.2.5.2's 16 DataReader policies (`data_representation`/
/// `type_consistency` have no `EntityQos` counterpart and are never
/// touched).
#[must_use]
pub fn reader_qos_from(eq: &EntityQos, mut base: DataReaderQos) -> DataReaderQos {
    if let Some(p) = eq.reliability {
        base.reliability = p;
    }
    if let Some(p) = eq.durability {
        base.durability = p;
    }
    if let Some(p) = eq.deadline {
        base.deadline = p;
    }
    if let Some(p) = eq.latency_budget {
        base.latency_budget = p;
    }
    if let Some(p) = eq.liveliness {
        base.liveliness = p;
    }
    if let Some(p) = eq.destination_order {
        base.destination_order = p;
    }
    if let Some(p) = eq.ownership {
        base.ownership = p;
    }
    if let Some(p) = eq.partition.clone() {
        base.partition = p;
    }
    if let Some(p) = eq.presentation {
        base.presentation = p;
    }
    if let Some(p) = eq.history {
        base.history = p;
    }
    if let Some(p) = eq.resource_limits {
        base.resource_limits = p;
    }
    if let Some(p) = eq.time_based_filter {
        base.time_based_filter = p;
    }
    if let Some(p) = eq.reader_data_lifecycle {
        base.reader_data_lifecycle = p;
    }
    if let Some(p) = eq.user_data.clone() {
        base.user_data = p;
    }
    if let Some(p) = eq.topic_data.clone() {
        base.topic_data = p;
    }
    if let Some(p) = eq.group_data.clone() {
        base.group_data = p;
    }
    base
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn topic(body: &[u8]) -> Option<EntityQos> {
        resolve_entity_qos(body, &MetadataStore::new(), EntityKind::Topic).unwrap()
    }

    fn publisher(body: &[u8]) -> Option<EntityQos> {
        resolve_entity_qos(body, &MetadataStore::new(), EntityKind::Publisher).unwrap()
    }

    fn participant(body: &[u8]) -> Option<EntityQos> {
        resolve_entity_qos(body, &MetadataStore::new(), EntityKind::DomainParticipant).unwrap()
    }

    fn writer(body: &[u8]) -> Option<EntityQos> {
        resolve_entity_qos(body, &MetadataStore::new(), EntityKind::DataWriter).unwrap()
    }

    fn reader(body: &[u8]) -> Option<EntityQos> {
        resolve_entity_qos(body, &MetadataStore::new(), EntityKind::DataReader).unwrap()
    }

    #[test]
    fn empty_body_has_no_qos() {
        assert_eq!(topic(b""), None);
    }

    #[test]
    fn a_body_with_no_qos_element_has_no_qos() {
        assert_eq!(topic(b"<topic><name>t</name></topic>"), None);
    }

    #[test]
    fn malformed_xml_is_a_bad_request() {
        let err = resolve_entity_qos(b"<topic><qos>", &MetadataStore::new(), EntityKind::Topic)
            .unwrap_err();
        assert!(matches!(err, BackendError::BadRequest(_)));
    }

    #[test]
    fn an_unmodelled_policy_element_is_a_bad_request() {
        let err = resolve_entity_qos(
            b"<topic><qos><not_a_policy/></qos></topic>",
            &MetadataStore::new(),
            EntityKind::Topic,
        )
        .unwrap_err();
        assert!(matches!(err, BackendError::BadRequest(_)));
    }

    #[test]
    fn topic_qos_merges_reliability_onto_base_and_leaves_the_rest() {
        let body = b"<topic><qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></qos></topic>";
        let eq = topic(body).expect("qos present");
        let mut base = TopicQos::default();
        base.lifespan.duration = zerodds_qos::Duration::from_secs(42_i32);
        let q = topic_qos_from(&eq, base.clone());
        assert_eq!(
            q.reliability.kind,
            zerodds_dcps::qos::ReliabilityKind::Reliable
        );
        // Untouched by `eq` — carried over from `base`, not reset to
        // `TopicQos::default()`.
        assert_eq!(q.lifespan, base.lifespan);
    }

    #[test]
    fn publisher_qos_reads_partition() {
        let body = b"<publisher><qos><partition><name><element>g1</element></name></partition></qos></publisher>";
        let eq = publisher(body).expect("qos present");
        let q = publisher_qos_from(&eq, PublisherQos::default());
        assert_eq!(q.partition.names, vec!["g1".to_string()]);
    }

    #[test]
    fn participant_qos_reads_user_data() {
        let body =
            b"<domain_participant><qos><user_data><value>aGk=</value></user_data></qos></domain_participant>";
        let eq = participant(body).expect("qos present");
        let q = participant_qos_from(&eq, DomainParticipantQos::default());
        assert_eq!(q.user_data.value, b"hi".to_vec());
    }

    #[test]
    fn writer_qos_merges_history_and_leaves_resource_limits_from_base() {
        let body = b"<data_writer><qos><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></qos></data_writer>";
        let eq = writer(body).expect("qos present");
        // A base whose `resource_limits` differs from
        // `DataWriterQos::default()` — regression guard for the bug a
        // "replace from `zerodds_qos` defaults" approach hit: an
        // unrelated field's default disagreeing with `base`'s current
        // value must never surface as a spurious change.
        let mut base = DataWriterQos::default();
        base.resource_limits.max_samples = 42;
        let q = writer_qos_from(&eq, base.clone());
        assert_eq!(q.history.kind, zerodds_dcps::qos::HistoryKind::KeepAll);
        assert_eq!(q.resource_limits, base.resource_limits);
    }

    #[test]
    fn reader_qos_reads_reliability_and_leaves_the_rest_from_base() {
        let body = b"<data_reader><qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></qos></data_reader>";
        let eq = reader(body).expect("qos present");
        let base = DataReaderQos::default();
        let q = reader_qos_from(&eq, base.clone());
        assert_eq!(
            q.reliability.kind,
            zerodds_dcps::qos::ReliabilityKind::Reliable
        );
        assert_eq!(q.resource_limits, base.resource_limits);
    }

    fn store_profile(metadata: &MetadataStore, library: &str, profile_xml: &str) {
        if metadata.qos_libraries.get(library).is_err() {
            metadata
                .qos_libraries
                .create(
                    library,
                    crate::metadata::StoredDocument {
                        content_type: "application/dds-web+xml".to_string(),
                        body: format!("<qos_library><name>{library}</name></qos_library>")
                            .into_bytes(),
                    },
                )
                .unwrap();
        }
        let name_start = profile_xml.find("<name>").expect("profile has a name") + 6;
        let name_end = profile_xml.find("</name>").expect("profile has a name");
        let profile_name = &profile_xml[name_start..name_end];
        metadata
            .qos_profiles
            .create(
                &MetadataStore::profile_key(library, profile_name),
                crate::metadata::StoredDocument {
                    content_type: "application/dds-web+xml".to_string(),
                    body: profile_xml.as_bytes().to_vec(),
                },
            )
            .unwrap();
    }

    #[test]
    fn qos_profile_reference_resolves_a_stored_profile() {
        let metadata = MetadataStore::new();
        store_profile(
            &metadata,
            "Lib",
            "<qos_profile><name>Reliable</name><topic_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></topic_qos></qos_profile>",
        );
        let body = b"<topic><qos_profile>Lib::Reliable</qos_profile></topic>";
        let eq = resolve_entity_qos(body, &metadata, EntityKind::Topic)
            .unwrap()
            .expect("profile resolved");
        assert_eq!(
            eq.reliability.expect("reliability set").kind,
            zerodds_dcps::qos::ReliabilityKind::Reliable
        );
    }

    #[test]
    fn qos_profile_reference_to_an_unknown_profile_is_a_bad_request() {
        let metadata = MetadataStore::new();
        let body = b"<topic><qos_profile>NoSuchLib::NoSuchProfile</qos_profile></topic>";
        let err = resolve_entity_qos(body, &metadata, EntityKind::Topic).unwrap_err();
        assert!(matches!(err, BackendError::BadRequest(_)));
    }

    #[test]
    fn inline_qos_overrides_the_profiles_value_for_the_same_policy() {
        let metadata = MetadataStore::new();
        store_profile(
            &metadata,
            "Lib",
            "<qos_profile><name>P</name><topic_qos><reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></topic_qos></qos_profile>",
        );
        let body = b"<topic><qos_profile>Lib::P</qos_profile><qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></qos></topic>";
        let eq = resolve_entity_qos(body, &metadata, EntityKind::Topic)
            .unwrap()
            .expect("resolved");
        // Inline `<qos>` wins on `reliability`...
        assert_eq!(
            eq.reliability.expect("reliability set").kind,
            zerodds_dcps::qos::ReliabilityKind::Reliable
        );
        // ...but `history`, which only the profile named, still applies.
        assert_eq!(
            eq.history.expect("history set").kind,
            zerodds_dcps::qos::HistoryKind::KeepAll
        );
    }

    /// DDS-XML 1.0 §7.3.5.4.4: an inline `<qos>` may inherit from a stored
    /// profile through its `base_name` attribute; the inline policies
    /// override the profile's.
    #[test]
    fn inline_qos_base_name_inherits_from_a_stored_profile() {
        let metadata = MetadataStore::new();
        store_profile(
            &metadata,
            "Lib",
            "<qos_profile><name>P</name><topic_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></topic_qos></qos_profile>",
        );
        let body = b"<topic><qos base_name=\"Lib::P\"><history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>3</depth></history></qos></topic>";
        let eq = resolve_entity_qos(body, &metadata, EntityKind::Topic)
            .unwrap()
            .expect("resolved");
        // Inherited from the profile.
        assert_eq!(
            eq.reliability.expect("reliability set").kind,
            zerodds_dcps::qos::ReliabilityKind::Reliable
        );
        // Overridden inline.
        let history = eq.history.expect("history set");
        assert_eq!(history.kind, zerodds_dcps::qos::HistoryKind::KeepLast);
        assert_eq!(history.depth, 3);
        // The base is spent: nothing unresolved is passed on.
        assert!(eq.base_name.is_none());
    }

    #[test]
    fn inline_qos_base_name_of_an_unknown_profile_is_a_bad_request() {
        let body = b"<topic><qos base_name=\"Lib::Missing\"/></topic>";
        let err = resolve_entity_qos(body, &MetadataStore::new(), EntityKind::Topic).unwrap_err();
        assert!(matches!(err, BackendError::BadRequest(_)));
    }

    /// An inline `<qos>` holds the policies of its entity's QoS structure
    /// (DDS 1.4 §2.2.3 / the schema's `datareaderQos`, …); a policy the
    /// entity has not is refused, not dropped by the conversion.
    #[test]
    fn inline_qos_policy_the_entity_lacks_is_a_bad_request() {
        let metadata = MetadataStore::new();
        for (kind, body) in [
            (
                EntityKind::DataReader,
                &b"<data_reader><qos><ownership_strength><value>3</value></ownership_strength></qos></data_reader>"[..],
            ),
            (
                EntityKind::DataWriter,
                &b"<data_writer><qos><time_based_filter/></qos></data_writer>"[..],
            ),
            (
                EntityKind::Publisher,
                &b"<publisher><qos><reliability/></qos></publisher>"[..],
            ),
            (
                EntityKind::Topic,
                &b"<topic><qos><partition/></qos></topic>"[..],
            ),
            (
                EntityKind::DomainParticipant,
                &b"<domain_participant><qos><history/></qos></domain_participant>"[..],
            ),
        ] {
            let err = resolve_entity_qos(body, &metadata, kind).unwrap_err();
            assert!(matches!(err, BackendError::BadRequest(_)), "{kind:?}: {err:?}");
        }
        // The same policy in the entity that has it is read.
        let ok = resolve_entity_qos(
            b"<data_writer><qos><ownership_strength><value>3</value></ownership_strength></qos></data_writer>",
            &metadata,
            EntityKind::DataWriter,
        )
        .unwrap()
        .expect("qos");
        assert_eq!(ok.ownership_strength.unwrap().value, 3);
    }
}
