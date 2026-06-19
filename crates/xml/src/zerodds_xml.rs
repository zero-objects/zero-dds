// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Top-level building-block loader for DDS-XML 1.0.
//!
//! Aggregates the four library types (QoS, Domain, Domain-Participant,
//! Application) from a single `<dds>` root element into a
//! [`DdsXml`] snapshot. Provides cross-library resolve helpers that
//! resolve a participant incl. its inheritance chain and the referenced
//! domain/topic/QoS items.
//!
//! Spec sources: OMG DDS-XML 1.0 §7.3.2 - §7.3.6 together.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::application::{ApplicationLibrary, parse_app_library_element};
use crate::domain::{DomainEntry, DomainLibrary, TopicEntry, parse_domain_library_element};
use crate::errors::XmlError;
use crate::inheritance::resolve_chain;
use crate::parser::parse_xml_tree;
use crate::participant::{
    DataReaderEntry, DataWriterEntry, DomainParticipantEntry, DomainParticipantLibrary,
    PublisherEntry, SubscriberEntry, parse_dp_library_element,
};
use crate::qos::{EntityQos, QosLibrary};
use crate::qos_inheritance::resolve_profile;
use crate::qos_parser::parse_qos_library_element_public;
use crate::resolver::parse_library_ref;
use crate::xtypes_def::{TypeDef, TypeLibrary};
use crate::xtypes_parser::parse_types_element;

/// Aggregated top-level snapshot of a `<dds>` document.
///
/// All four library types from DDS-XML 1.0 §7.3.2-7.3.6 are gathered here
/// in their parsed form. Cross-library references are resolved
/// **lazily** when the `resolve_*` methods are called — the
/// constructor only performs well-formedness and schema checks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DdsXml {
    /// All `<qos_library>` entries.
    pub qos_libraries: Vec<QosLibrary>,
    /// All `<domain_library>` entries.
    pub domain_libraries: Vec<DomainLibrary>,
    /// All `<domain_participant_library>` entries.
    pub participant_libraries: Vec<DomainParticipantLibrary>,
    /// All `<application_library>` entries.
    pub application_libraries: Vec<ApplicationLibrary>,
    /// All `<types>` top-level blocks (Spec §7.3.3).
    pub type_libraries: Vec<TypeLibrary>,
}

/// Resolved snapshot of a domain participant after applying:
/// 1. Multi-level `base_name` inheritance (participant chain).
/// 2. Domain lookup via `domain_ref`.
/// 3. Topic lookup via `topic_ref` of the children writers/readers.
/// 4. Optional: QoS profile materialization when `qos_profile_ref`
///    was set or inline QoS was merged via inheritance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedParticipant {
    /// Full lookup path (`library::name`).
    pub lookup_path: String,
    /// Effective name.
    pub name: String,
    /// Resolved numeric domain ID.
    pub domain_id: u32,
    /// Full domain snapshot incl. topics + type registrations.
    pub domain: DomainEntry,
    /// Inheritance chain of the participant definition (base-first).
    pub inheritance_chain: Vec<String>,
    /// Effective participant QoS after merging the chain.
    pub qos: Option<EntityQos>,
    /// Resolved topic references.
    pub topics: Vec<ResolvedTopic>,
    /// Resolved publishers.
    pub publishers: Vec<ResolvedPublisher>,
    /// Resolved subscribers.
    pub subscribers: Vec<ResolvedSubscriber>,
}

/// Resolved topic snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedTopic {
    /// Topic name.
    pub name: String,
    /// Type name (from the domain's `register_type_ref`).
    pub type_name: String,
    /// Effective topic QoS (inline from the topic, or via `qos_profile_ref`,
    /// or `None` if nothing is set).
    pub qos: Option<EntityQos>,
    /// Topic filter glob.
    pub topic_filter: Option<String>,
}

/// Resolved publisher snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedPublisher {
    /// Publisher name.
    pub name: String,
    /// Effective publisher QoS (or inherited from the participant).
    pub qos: Option<EntityQos>,
    /// Resolved DataWriters.
    pub data_writers: Vec<ResolvedDataWriter>,
}

/// Resolved subscriber snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedSubscriber {
    /// Subscriber name.
    pub name: String,
    /// Effective subscriber QoS (or inherited from the participant).
    pub qos: Option<EntityQos>,
    /// Resolved DataReaders.
    pub data_readers: Vec<ResolvedDataReader>,
}

/// Resolved DataWriter snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedDataWriter {
    /// Writer name.
    pub name: String,
    /// Resolved topic reference.
    pub topic: ResolvedTopic,
    /// Effective writer QoS (inline via inheritance, or publisher QoS,
    /// or via `qos_profile_ref`).
    pub qos: Option<EntityQos>,
}

/// Resolved DataReader snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedDataReader {
    /// Reader name.
    pub name: String,
    /// Resolved topic reference.
    pub topic: ResolvedTopic,
    /// Effective reader QoS.
    pub qos: Option<EntityQos>,
}

/// Adapter trait for binding a resolved participant to a
/// real DCPS `DomainParticipantFactory`. This crate
/// **deliberately implements only the trait skeleton** — a concrete wire-up
/// implementation lives in a separate crate (e.g. `zerodds-dcps-xml-bridge`),
/// to preserve the layer discipline (`zerodds-xml` does **not** depend on
/// `zerodds-dcps`).
///
/// Spec reference: DDS-XML 1.0 §7.3.5 (Domain Participant Library) provides
/// the configuration; binding to the DDS 1.4 §2.2.2 DCPS factory
/// is the task of the higher-level adapter.
pub trait ParticipantFactoryAdapter {
    /// Apply a resolved participant snapshot to a DCPS
    /// factory. An adapter MUST:
    /// 1. Create the `DomainParticipant` object with `domain_id`,
    /// 2. Create the topics and their type registrations,
    /// 3. Instantiate publishers/subscribers with the effective QoS,
    /// 4. Bind DataWriters/DataReaders to the topics.
    ///
    /// # Errors
    /// Implementation-defined.
    fn apply(&self, participant: &ResolvedParticipant) -> Result<(), XmlError>;
}

/// Convenience function: forwards a resolved participant to an
/// adapter. The implementation is trivial forwarding and exists
/// only to make the top-level API ergonomic.
///
/// # Errors
/// As [`ParticipantFactoryAdapter::apply`].
pub fn apply_to_factory(
    participant: &ResolvedParticipant,
    factory: &dyn ParticipantFactoryAdapter,
) -> Result<(), XmlError> {
    factory.apply(participant)
}

/// Parses a complete `<dds>` document and returns the aggregated
/// building-block snapshot.
///
/// Accepts documents that contain *any arbitrary subset* of the four library
/// types — even an empty `<dds/>` is a valid document.
///
/// # Errors
/// * [`XmlError::InvalidXml`] — no `<dds>` root or XML not
///   well-formed.
/// * Further errors from the per-library decoder paths.
pub fn parse_dds_xml(xml: &str) -> Result<DdsXml, XmlError> {
    let doc = parse_xml_tree(xml)?;
    if doc.root.name != "dds" {
        return Err(XmlError::InvalidXml(format!(
            "expected <dds> root, got <{}>",
            doc.root.name
        )));
    }
    let mut out = DdsXml::default();
    for child in &doc.root.children {
        match child.name.as_str() {
            "qos_library" => out
                .qos_libraries
                .push(parse_qos_library_element_public(child)?),
            "domain_library" => out
                .domain_libraries
                .push(parse_domain_library_element(child)?),
            "domain_participant_library" => out
                .participant_libraries
                .push(parse_dp_library_element(child)?),
            "application_library" => out
                .application_libraries
                .push(parse_app_library_element(child)?),
            "types" => out.type_libraries.push(parse_types_element(child)?),
            _ => {}
        }
    }
    Ok(out)
}

impl DdsXml {
    /// Looks up a participant via the `library::participant` path.
    ///
    /// # Errors
    /// [`XmlError::UnresolvedReference`] if the library or participant
    /// is missing.
    pub fn find_participant(&self, path: &str) -> Result<&DomainParticipantEntry, XmlError> {
        let r = parse_library_ref(path)?;
        if !r.is_qualified() {
            return Err(XmlError::UnresolvedReference(format!(
                "participant ref `{path}` must be qualified `library::name`"
            )));
        }
        let lib = self
            .participant_libraries
            .iter()
            .find(|l| l.name == r.library)
            .ok_or_else(|| {
                XmlError::UnresolvedReference(format!("participant_library `{}`", r.library))
            })?;
        lib.participant(&r.name)
            .ok_or_else(|| XmlError::UnresolvedReference(format!("participant `{path}`")))
    }

    /// Looks up a domain via `library::name`.
    ///
    /// # Errors
    /// [`XmlError::UnresolvedReference`] if the library or domain is missing.
    pub fn find_domain(&self, path: &str) -> Result<&DomainEntry, XmlError> {
        let r = parse_library_ref(path)?;
        if !r.is_qualified() {
            return Err(XmlError::UnresolvedReference(format!(
                "domain ref `{path}` must be qualified `library::name`"
            )));
        }
        let lib = self
            .domain_libraries
            .iter()
            .find(|l| l.name == r.library)
            .ok_or_else(|| {
                XmlError::UnresolvedReference(format!("domain_library `{}`", r.library))
            })?;
        lib.domain(&r.name)
            .ok_or_else(|| XmlError::UnresolvedReference(format!("domain `{path}`")))
    }

    /// Resolves a participant including its inheritance chain,
    /// referenced domain, topics and QoS profiles.
    ///
    /// # Errors
    /// * [`XmlError::UnresolvedReference`] — reference not found.
    /// * [`XmlError::CircularInheritance`] — `base_name` cycle.
    /// * [`XmlError::LimitExceeded`] — inheritance depth > 32.
    pub fn resolve_participant(&self, path: &str) -> Result<ResolvedParticipant, XmlError> {
        let r = parse_library_ref(path)?;
        if !r.is_qualified() {
            return Err(XmlError::UnresolvedReference(format!(
                "participant ref `{path}` must be qualified `library::name`"
            )));
        }
        let canonical = format!("{}::{}", r.library, r.name);

        // Resolve the inheritance chain.
        let chain = resolve_chain(&canonical, |key| {
            let kr = parse_library_ref(key)?;
            let lib = self
                .participant_libraries
                .iter()
                .find(|l| l.name == kr.library)
                .ok_or_else(|| {
                    XmlError::UnresolvedReference(format!("participant_library `{}`", kr.library))
                })?;
            let p = lib
                .participant(&kr.name)
                .ok_or_else(|| XmlError::UnresolvedReference(format!("participant `{key}`")))?;
            Ok(p.base_name.as_deref().map(|b| {
                if b.contains("::") {
                    b.to_string()
                } else {
                    format!("{}::{}", kr.library, b)
                }
            }))
        })?;

        // Resolve fields by merging the chain (base-first).
        let mut domain_ref: Option<String> = None;
        let mut qos: Option<EntityQos> = None;
        let mut publishers: Vec<PublisherEntry> = Vec::new();
        let mut subscribers: Vec<SubscriberEntry> = Vec::new();
        let mut register_types_ref: Vec<String> = Vec::new();
        let mut topics_ref: Vec<String> = Vec::new();
        let mut effective_name = r.name.clone();
        for key in &chain {
            let kr = parse_library_ref(key)?;
            let p = self.find_participant(key)?;
            domain_ref = Some(p.domain_ref.clone());
            qos = match (qos, p.qos.as_ref()) {
                (None, None) => None,
                (Some(a), None) => Some(a),
                (None, Some(c)) => Some(c.clone()),
                (Some(a), Some(c)) => Some(a.merge(c)),
            };
            // Children: append-then-dedup-by-name (child entries override
            // parent entries with the same name).
            merge_entries(&mut publishers, &p.publishers, |x| x.name.clone());
            merge_entries(&mut subscribers, &p.subscribers, |x| x.name.clone());
            merge_str_vec(&mut register_types_ref, &p.register_types_ref);
            merge_str_vec(&mut topics_ref, &p.topics_ref);
            effective_name = kr.name;
        }

        let dref = domain_ref.ok_or_else(|| {
            XmlError::UnresolvedReference(format!("participant `{canonical}` missing domain_ref"))
        })?;
        let domain = self.find_domain(&dref)?.clone();

        // Topics: only the explicitly referenced ones (via `<topic ref="…"/>`)
        // are taken into the ResolvedParticipant. If no
        // `topics_ref` are present, ALL topics of the domain are
        // considered implicitly available (Spec §7.3.5.4.2 allows
        // both readings — Annex C shows explicit selection; Cyclone
        // and FastDDS make all topics implicitly available).
        let topics: Vec<ResolvedTopic> = if topics_ref.is_empty() {
            domain
                .topics
                .iter()
                .map(|t| self.resolve_topic_entry(t, &domain))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            let mut out = Vec::new();
            for tref in &topics_ref {
                let topic = domain
                    .topic(tref)
                    .ok_or_else(|| XmlError::UnresolvedReference(format!("topic `{tref}`")))?;
                out.push(self.resolve_topic_entry(topic, &domain)?);
            }
            out
        };

        // Validate explicit register_types_ref entries exist in the domain.
        for rt in &register_types_ref {
            if domain.register_type(rt).is_none() {
                return Err(XmlError::UnresolvedReference(format!(
                    "register_type `{rt}` in domain `{dref}`"
                )));
            }
        }

        // Resolve publishers + writers.
        let resolved_pubs = publishers
            .iter()
            .map(|pub_e| self.resolve_publisher(pub_e, &domain))
            .collect::<Result<Vec<_>, _>>()?;
        let resolved_subs = subscribers
            .iter()
            .map(|sub_e| self.resolve_subscriber(sub_e, &domain))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ResolvedParticipant {
            lookup_path: canonical,
            name: effective_name,
            domain_id: domain.domain_id,
            domain,
            inheritance_chain: chain,
            qos,
            topics,
            publishers: resolved_pubs,
            subscribers: resolved_subs,
        })
    }

    fn resolve_topic_entry(
        &self,
        topic: &TopicEntry,
        domain: &DomainEntry,
    ) -> Result<ResolvedTopic, XmlError> {
        // resolve the type name via register_type_ref.
        let rt = domain
            .register_type(&topic.register_type_ref)
            .ok_or_else(|| {
                XmlError::UnresolvedReference(format!(
                    "register_type `{}`",
                    topic.register_type_ref
                ))
            })?;
        // QoS: inline wins; otherwise resolve qos_profile_ref.
        let qos: Option<EntityQos> = if let Some(q) = &topic.topic_qos {
            Some(q.clone())
        } else if let Some(profile_ref) = &topic.qos_profile_ref {
            let r = resolve_profile(&self.qos_libraries, profile_ref)?;
            r.topic_qos
        } else {
            None
        };
        Ok(ResolvedTopic {
            name: topic.name.clone(),
            type_name: rt.type_ref.clone(),
            qos,
            topic_filter: topic.topic_filter.clone(),
        })
    }

    fn resolve_publisher(
        &self,
        pub_e: &PublisherEntry,
        domain: &DomainEntry,
    ) -> Result<ResolvedPublisher, XmlError> {
        let writers = pub_e
            .data_writers
            .iter()
            .map(|dw| self.resolve_writer(dw, pub_e, domain))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ResolvedPublisher {
            name: pub_e.name.clone(),
            qos: pub_e.qos.clone(),
            data_writers: writers,
        })
    }

    fn resolve_subscriber(
        &self,
        sub_e: &SubscriberEntry,
        domain: &DomainEntry,
    ) -> Result<ResolvedSubscriber, XmlError> {
        let readers = sub_e
            .data_readers
            .iter()
            .map(|dr| self.resolve_reader(dr, sub_e, domain))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ResolvedSubscriber {
            name: sub_e.name.clone(),
            qos: sub_e.qos.clone(),
            data_readers: readers,
        })
    }

    fn resolve_writer(
        &self,
        dw: &DataWriterEntry,
        publisher: &PublisherEntry,
        domain: &DomainEntry,
    ) -> Result<ResolvedDataWriter, XmlError> {
        let topic = domain
            .topic(&dw.topic_ref)
            .ok_or_else(|| XmlError::UnresolvedReference(format!("topic `{}`", dw.topic_ref)))?;
        let resolved_topic = self.resolve_topic_entry(topic, domain)?;
        // QoS-Praezedenz: Inline > qos_profile_ref > Publisher-QoS.
        let qos: Option<EntityQos> = if let Some(q) = &dw.qos {
            Some(q.clone())
        } else if let Some(profile_ref) = &dw.qos_profile_ref {
            let r = resolve_profile(&self.qos_libraries, profile_ref)?;
            r.datawriter_qos
        } else {
            publisher.qos.clone()
        };
        Ok(ResolvedDataWriter {
            name: dw.name.clone(),
            topic: resolved_topic,
            qos,
        })
    }

    fn resolve_reader(
        &self,
        dr: &DataReaderEntry,
        subscriber: &SubscriberEntry,
        domain: &DomainEntry,
    ) -> Result<ResolvedDataReader, XmlError> {
        let topic = domain
            .topic(&dr.topic_ref)
            .ok_or_else(|| XmlError::UnresolvedReference(format!("topic `{}`", dr.topic_ref)))?;
        let resolved_topic = self.resolve_topic_entry(topic, domain)?;
        let qos: Option<EntityQos> = if let Some(q) = &dr.qos {
            Some(q.clone())
        } else if let Some(profile_ref) = &dr.qos_profile_ref {
            let r = resolve_profile(&self.qos_libraries, profile_ref)?;
            r.datareader_qos
        } else {
            subscriber.qos.clone()
        };
        Ok(ResolvedDataReader {
            name: dr.name.clone(),
            topic: resolved_topic,
            qos,
        })
    }

    /// Resolves a type name (`Module::Sub::Type` or bare `Type`) over
    /// all [`Self::type_libraries`].
    ///
    /// On multiple matching entries, the first in
    /// document order is returned.
    #[must_use]
    pub fn resolve_type(&self, name: &str) -> Option<&TypeDef> {
        let parts: Vec<&str> = name.split("::").collect();
        for lib in &self.type_libraries {
            if let Some(td) = walk_types(&lib.types, &parts) {
                return Some(td);
            }
        }
        None
    }

    /// Resolves an application into a list of ResolvedParticipants
    /// (1+ entries per `<application>`).
    ///
    /// # Errors
    /// As [`Self::resolve_participant`].
    pub fn resolve_application(&self, path: &str) -> Result<Vec<ResolvedParticipant>, XmlError> {
        let r = parse_library_ref(path)?;
        if !r.is_qualified() {
            return Err(XmlError::UnresolvedReference(format!(
                "application ref `{path}` must be qualified `library::name`"
            )));
        }
        let lib = self
            .application_libraries
            .iter()
            .find(|l| l.name == r.library)
            .ok_or_else(|| {
                XmlError::UnresolvedReference(format!("application_library `{}`", r.library))
            })?;
        let app = lib
            .application(&r.name)
            .ok_or_else(|| XmlError::UnresolvedReference(format!("application `{path}`")))?;
        app.domain_participants
            .iter()
            .map(|dp| self.resolve_participant(dp))
            .collect()
    }
}

// ============================================================================
// Internal merge helpers for participant inheritance
// ============================================================================

fn merge_entries<T, K, F>(acc: &mut Vec<T>, override_: &[T], key: F)
where
    T: Clone,
    K: Eq,
    F: Fn(&T) -> K,
{
    for item in override_ {
        let k = key(item);
        if let Some(pos) = acc.iter().position(|x| key(x) == k) {
            acc[pos] = item.clone();
        } else {
            acc.push(item.clone());
        }
    }
}

/// zerodds-lint: recursion-depth = number of `::` segments in the lookup path +
/// module nesting depth (effectively bounded by the `MAX_TOTAL_ELEMENTS` DoS cap
/// of the XML foundation; realistically ≤ 16).
fn walk_types<'a>(types: &'a [TypeDef], parts: &[&str]) -> Option<&'a TypeDef> {
    if parts.is_empty() {
        return None;
    }
    let head = parts[0];
    for t in types {
        if t.name() == head {
            if parts.len() == 1 {
                return Some(t);
            }
            if let TypeDef::Module(m) = t {
                if let Some(found) = walk_types(&m.types, &parts[1..]) {
                    return Some(found);
                }
            }
        }
    }
    if parts.len() == 1 {
        for t in types {
            if let TypeDef::Module(m) = t {
                if let Some(found) = walk_types(&m.types, parts) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn merge_str_vec(acc: &mut Vec<String>, override_: &[String]) {
    for s in override_ {
        if !acc.contains(s) {
            acc.push(s.clone());
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_dds() {
        let xml = r#"<dds/>"#;
        let d = parse_dds_xml(xml).expect("parse");
        assert!(d.qos_libraries.is_empty());
        assert!(d.domain_libraries.is_empty());
        assert!(d.participant_libraries.is_empty());
        assert!(d.application_libraries.is_empty());
    }

    #[test]
    fn parse_mixed_top_level() {
        let xml = r#"<dds>
          <qos_library name="ql"><qos_profile name="P"/></qos_library>
          <domain_library name="dl">
            <domain name="D" domain_id="0"/>
          </domain_library>
          <domain_participant_library name="dpl">
            <domain_participant name="P" domain_ref="dl::D"/>
          </domain_participant_library>
          <application_library name="al">
            <application name="A">
              <domain_participant ref="dpl::P"/>
            </application>
          </application_library>
        </dds>"#;
        let d = parse_dds_xml(xml).expect("parse");
        assert_eq!(d.qos_libraries.len(), 1);
        assert_eq!(d.domain_libraries.len(), 1);
        assert_eq!(d.participant_libraries.len(), 1);
        assert_eq!(d.application_libraries.len(), 1);
    }

    #[test]
    fn non_dds_root_rejected() {
        let xml = r#"<other/>"#;
        let err = parse_dds_xml(xml).expect_err("non-dds");
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }
}
