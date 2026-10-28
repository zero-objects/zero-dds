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
use crate::domain::{
    DomainEntry, DomainLibrary, RegisterType, TopicEntry, parse_domain_library_element,
};
use crate::errors::XmlError;
use crate::inheritance::resolve_chain;
use crate::options::{Ctx, Parsed, ReadOptions};
use crate::parser::parse_xml_tree;
use crate::participant::{
    DataReaderEntry, DataWriterEntry, DomainParticipantEntry, DomainParticipantLibrary,
    PublisherEntry, SubscriberEntry, parse_dp_library_element,
};
use crate::qos::{EntityQos, QosLibrary, default_qos};
use crate::qos_inheritance::{ResolvedQos, resolve_profile};
use crate::qos_parser::{parse_ccm_top_level_profile, parse_qos_library_element};
use crate::resolver::parse_library_ref;
use crate::xtypes_def::{TypeDef, TypeLibrary};
use crate::xtypes_parser::parse_types_element_ctx;

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
    /// Full domain snapshot incl. topics + type registrations: the
    /// referenced domain's, plus the participant's own `<topic>` /
    /// `<register_type>` definitions (§7.3.5.4.2).
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

/// The name of the library that holds the profiles written directly under `<dds>`
/// (DDS4CCM 1.1 §7.4.2.3, read under [`crate::VendorForms::Accept`]): a profile of
/// it is reached as `(dds)::Profile`, and `base_name` without a library refers
/// within it. A document that declares a library of the same name is not
/// distinguished from it.
pub const CCM_LIBRARY: &str = "(dds)";

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
    Ok(parse_dds_xml_with(xml, &ReadOptions::default())?.value)
}

/// As [`parse_dds_xml`], with reading options. Under
/// [`UnknownElements::ReportAndSkip`](crate::options::UnknownElements) what
/// the readers do not model — in a QoS library, a domain, a participant,
/// an application, or a `<dds>` child that is no library — is left out of
/// the result and listed in [`Parsed::diagnostics`] with its path.
///
/// # Errors
/// As [`parse_dds_xml`].
pub fn parse_dds_xml_with(xml: &str, options: &ReadOptions) -> Result<Parsed<DdsXml>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    if doc.root.name != "dds" {
        return Err(XmlError::InvalidXml(format!(
            "expected <dds> root, got <{}>",
            doc.root.name
        )));
    }
    let mut ctx = Ctx::new(*options);
    ctx.check_attributes(&doc.root, &[])?;
    let mut out = DdsXml::default();
    // DDS4CCM 1.1 §7.4.2.3: profiles directly under <dds>, which DDS-RPC 1.0
    // §7.10.1 names as its profile format. They are kept in one library of
    // the empty name, after the others in document order.
    let mut ccm_profiles = Vec::new();
    for child in &doc.root.children {
        if let Some(profile) = parse_ccm_top_level_profile(&mut ctx, child)? {
            ccm_profiles.push(profile);
            continue;
        }
        match child.name.as_str() {
            "qos_library" => out
                .qos_libraries
                .push(parse_qos_library_element(&mut ctx, child)?),
            "domain_library" => {
                let mut lib = parse_domain_library_element(&mut ctx, child)?;
                lib.type_libraries_before = Some(out.type_libraries.len());
                out.domain_libraries.push(lib);
            }
            "domain_participant_library" => out
                .participant_libraries
                .push(parse_dp_library_element(&mut ctx, child)?),
            "application_library" => {
                let mut lib = parse_app_library_element(&mut ctx, child)?;
                lib.participant_libraries_before = Some(out.participant_libraries.len());
                out.application_libraries.push(lib);
            }
            "types" => out
                .type_libraries
                .push(parse_types_element_ctx(&mut ctx, child)?),
            // A vendor block or a misspelt library used to be skipped; the
            // document then loaded with a section missing.
            _ => ctx.unknown_child(child)?,
        }
    }
    if !ccm_profiles.is_empty() {
        out.qos_libraries.push(QosLibrary {
            name: String::from(CCM_LIBRARY),
            profiles: ccm_profiles,
        });
    }
    Ok(ctx.finish(out))
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
        self.find_domain_in_library(path).map(|(_, d)| d)
    }

    /// As [`Self::find_domain`], but also hands back the library the
    /// domain lives in — `base_name` inheritance (§7.3.4.4.2) is resolved
    /// within that library.
    ///
    /// # Errors
    /// See [`Self::find_domain`].
    pub fn find_domain_in_library(
        &self,
        path: &str,
    ) -> Result<(&crate::domain::DomainLibrary, &DomainEntry), XmlError> {
        let r = parse_library_ref(path)?;
        if r.is_qualified() {
            let lib = self
                .domain_libraries
                .iter()
                .find(|l| l.name == r.library)
                .ok_or_else(|| {
                    XmlError::UnresolvedReference(format!("domain_library `{}`", r.library))
                })?;
            let entry = lib
                .domain(&r.name)
                .ok_or_else(|| XmlError::UnresolvedReference(format!("domain `{path}`")))?;
            return Ok((lib, entry));
        }
        // §7.3.5.4.2 does not fix the shape of `domain_ref`; the examples
        // of §7.3.5.4.1.1 and §7.3.5.4.2.1 write it unqualified
        // (`domain_ref="MyDomain"`). Search the libraries in document
        // order for the first domain of that name.
        for lib in &self.domain_libraries {
            if let Some(entry) = lib.domain(&r.name) {
                return Ok((lib, entry));
            }
        }
        Err(XmlError::UnresolvedReference(format!("domain `{path}`")))
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
        let chain = self.participant_chain(&canonical)?;
        let mut entries: Vec<DomainParticipantEntry> = Vec::new();
        for key in &chain {
            entries.push(self.find_participant(key)?.clone());
        }
        self.resolve_participant_chain(canonical, chain, &entries)
    }

    /// The `base_name` chain of a participant defined in a participant
    /// library, base-first and including the participant itself.
    fn participant_chain(&self, canonical: &str) -> Result<Vec<String>, XmlError> {
        let chain = resolve_chain(canonical, |key| {
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
        // §7.3.5.4.3 / §7.3.6.4.2: a participant "can only inherit from
        // DomainParticipants in DomainParticipant Libraries that have been
        // defined before its own definition."
        for pair in chain.windows(2) {
            let base = self.participant_position(&pair[0])?;
            let derived = self.participant_position(&pair[1])?;
            if base >= derived {
                return Err(XmlError::InvalidXml(format!(
                    "participant `{}` inherits from `{}`, which is defined later (§7.3.5.4.3)",
                    pair[1], pair[0]
                )));
            }
        }
        Ok(chain)
    }

    /// Document-order position of a participant: `(library index, entry
    /// index)`.
    fn participant_position(&self, canonical: &str) -> Result<(usize, usize), XmlError> {
        let r = parse_library_ref(canonical)?;
        let (li, lib) = self
            .participant_libraries
            .iter()
            .enumerate()
            .find(|(_, l)| l.name == r.library)
            .ok_or_else(|| {
                XmlError::UnresolvedReference(format!("participant_library `{}`", r.library))
            })?;
        let pi = lib
            .participants
            .iter()
            .position(|p| p.name == r.name)
            .ok_or_else(|| XmlError::UnresolvedReference(format!("participant `{canonical}`")))?;
        Ok((li, pi))
    }

    /// Merges an already-ordered chain of participant definitions
    /// (base-first, the effective definition last) into a
    /// [`ResolvedParticipant`].
    ///
    /// Shared by [`Self::resolve_participant`], whose chain comes entirely
    /// out of the participant libraries, and by
    /// [`Self::resolve_application`], whose last link is the definition
    /// written inside the `<application>` itself.
    fn resolve_participant_chain(
        &self,
        canonical: String,
        chain: Vec<String>,
        entries: &[DomainParticipantEntry],
    ) -> Result<ResolvedParticipant, XmlError> {
        // Resolve fields by merging the chain (base-first).
        let mut domain_ref: Option<String> = None;
        let mut domain_id: Option<u32> = None;
        let mut qos: Option<EntityQos> = None;
        let mut publishers: Vec<PublisherEntry> = Vec::new();
        let mut subscribers: Vec<SubscriberEntry> = Vec::new();
        let mut register_types: Vec<RegisterType> = Vec::new();
        let mut topics: Vec<TopicEntry> = Vec::new();
        let mut effective_name = String::new();
        for p in entries {
            // A derived definition that names no domain of its own keeps
            // the one it inherits; only a definition that does name one
            // overrides it (§7.3.5.4.3 — `base_name` inheritance is a
            // merge, and `domain_ref` is optional in the schema).
            if let Some(d) = p.domain_ref.as_ref() {
                domain_ref = Some(d.clone());
            }
            if let Some(id) = p.domain_id {
                domain_id = Some(id);
            }
            let resolved_qos =
                match p.qos.as_ref() {
                    Some(c) => Some(self.resolve_inline_qos(c, |r| {
                        default_qos(&r.domainparticipant_qos).cloned()
                    })?),
                    None => None,
                };
            qos = match (qos, resolved_qos) {
                (None, None) => None,
                (Some(a), None) => Some(a),
                (None, Some(c)) => Some(c),
                (Some(a), Some(c)) => Some(a.merge(&c)),
            };
            // Children: append-then-dedup-by-name (child entries override
            // parent entries with the same name).
            merge_entries(&mut publishers, &p.publishers, |x| x.name.clone());
            merge_entries(&mut subscribers, &p.subscribers, |x| x.name.clone());
            merge_entries(&mut register_types, &p.register_types, |x| x.name.clone());
            merge_entries(&mut topics, &p.topics, |x| x.name.clone());
            effective_name = p.name.clone();
        }

        // §7.3.5.4.2: "DomainParticipants may refer to a Domain [...]
        // using the domain_ref XML attribute." The reference is optional;
        // a participant may instead name its Domain numerically. Only when
        // neither is given is there no Domain to join.
        let (mut domain, effective_domain_id) = match domain_ref {
            Some(dref) => {
                let (domain_library, domain_entry) = self.find_domain_in_library(&dref)?;
                // §7.3.4.4.2: a domain inherits the topics and registered
                // types of its base chain as well as its id. Resolve the
                // effective domain once, so a writer at an inherited topic
                // resolves.
                let domain = domain_library.effective_domain(domain_entry)?;
                // §7.3.4.4.1: the types of the registrations are
                // "previously defined", in a `<types>` block before the
                // domain library.
                if let Some(before) = domain_library.type_libraries_before {
                    for rt in &domain.register_types {
                        if self.resolve_type_before(&rt.type_ref, before).is_none() {
                            return Err(XmlError::UnresolvedReference(format!(
                                "type `{}` of register_type `{}` is not defined before domain library `{}` (§7.3.4.4.1)",
                                rt.type_ref, rt.name, domain_library.name
                            )));
                        }
                    }
                }
                // A participant may name an id of its own (§7.3.5.4.1).
                let id = match domain_id {
                    Some(id) => id,
                    None => domain.domain_id.ok_or_else(|| {
                        XmlError::MissingRequiredElement(format!(
                            "domain_id for participant `{canonical}`: neither the participant nor domain `{dref}` and its base chain name one"
                        ))
                    })?,
                };
                (domain, id)
            }
            None => {
                let id = domain_id.ok_or_else(|| {
                    XmlError::UnresolvedReference(format!(
                        "participant `{canonical}` names neither domain_ref nor domain_id"
                    ))
                })?;
                (DomainEntry::default(), id)
            }
        };

        // §7.3.5.4.2: the referenced domain makes its Topics and
        // Register Types available to this participant. The own
        // `<register_type>`/`<topic>` children — typed with the Domain
        // building block's complex types — are definitions of the same
        // kind and join that set, overriding a domain entry of the same
        // name.
        merge_entries(&mut domain.register_types, &register_types, |x| {
            x.name.clone()
        });
        merge_entries(&mut domain.topics, &topics, |x| x.name.clone());

        // Every topic the participant can see: the domain's, plus its
        // own definitions. Writers and readers pick theirs from this
        // set by `topic_ref`.
        let topics: Vec<ResolvedTopic> = domain
            .topics
            .iter()
            .map(|t| self.resolve_topic_entry(t, &domain))
            .collect::<Result<Vec<_>, _>>()?;

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
            domain_id: effective_domain_id,
            domain,
            inheritance_chain: chain,
            qos,
            topics,
            publishers: resolved_pubs,
            subscribers: resolved_subs,
        })
    }

    /// Resolves an inline entity QoS container, applying the `base_name`
    /// inheritance of §7.3.5.4.4 first: the named profile's matching
    /// container is the base, the inline policies override it.
    ///
    /// `pick` selects that matching container out of the resolved
    /// profile (e.g. its `topic_qos` for an inline `<topic_qos>`).
    fn resolve_inline_qos<F>(&self, q: &EntityQos, pick: F) -> Result<EntityQos, XmlError>
    where
        F: Fn(&ResolvedQos) -> Option<EntityQos>,
    {
        let Some(base) = q.base_name.as_deref() else {
            return Ok(q.clone());
        };
        let path = self.qualify_profile_ref(base)?;
        let resolved = resolve_profile(&self.qos_libraries, &path)?;
        let mut out = match pick(&resolved) {
            Some(slot) => slot.merge(q),
            None => q.clone(),
        };
        // The base chain is spent; the result carries no unresolved name.
        out.base_name = None;
        Ok(out)
    }

    /// Turns an inline `base_name` value into a `library::profile` path.
    ///
    /// A qualified reference is used as written. An unqualified one is
    /// searched for across the libraries in document order, the same way
    /// `QosProfileRegistry` selects the first library (§7.3.2.4.2 does
    /// not fix a search order; the first match is the least surprising).
    fn qualify_profile_ref(&self, reference: &str) -> Result<String, XmlError> {
        if reference.contains("::") {
            return Ok(reference.to_string());
        }
        for lib in &self.qos_libraries {
            if lib.profile(reference).is_some() {
                return Ok(format!("{}::{}", lib.name, reference));
            }
        }
        Err(XmlError::UnresolvedReference(format!(
            "qos profile `{reference}`"
        )))
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
        // §7.3.4.4.1: the `type_ref` names a type "that [has] been
        // previously defined" in a `<types>` block. A dangling reference
        // would otherwise reach the writer as a type name nobody knows.
        if self.resolve_type(&rt.type_ref).is_none() {
            return Err(XmlError::UnresolvedReference(format!(
                "type `{}` of register_type `{}`",
                rt.type_ref, rt.name
            )));
        }
        // QoS: inline wins; otherwise resolve qos_profile_ref. The
        // profile's §7.3.2.4.3 filter selects by this topic's name.
        let qos: Option<EntityQos> = if let Some(q) = &topic.topic_qos {
            Some(self.resolve_inline_qos(q, |r| r.topic_qos_for_topic(&topic.name).cloned())?)
        } else if let Some(profile_ref) = &topic.qos_profile_ref {
            let r = resolve_profile(&self.qos_libraries, profile_ref)?;
            r.topic_qos_for_topic(&topic.name).cloned()
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
        let qos = match &pub_e.qos {
            Some(q) => {
                Some(self.resolve_inline_qos(q, |r| default_qos(&r.publisher_qos).cloned())?)
            }
            None => None,
        };
        Ok(ResolvedPublisher {
            name: pub_e.name.clone(),
            qos,
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
        let qos = match &sub_e.qos {
            Some(q) => {
                Some(self.resolve_inline_qos(q, |r| default_qos(&r.subscriber_qos).cloned())?)
            }
            None => None,
        };
        Ok(ResolvedSubscriber {
            name: sub_e.name.clone(),
            qos,
            data_readers: readers,
        })
    }

    fn resolve_writer(
        &self,
        dw: &DataWriterEntry,
        _publisher: &PublisherEntry,
        domain: &DomainEntry,
    ) -> Result<ResolvedDataWriter, XmlError> {
        let topic = domain
            .topic(&dw.topic_ref)
            .ok_or_else(|| XmlError::UnresolvedReference(format!("topic `{}`", dw.topic_ref)))?;
        let resolved_topic = self.resolve_topic_entry(topic, domain)?;
        // §7.3.5.4.4: "Inline QoS settings apply only to the entity in
        // whose context they are being defined." A DataWriter without QoS
        // of its own therefore does NOT take its Publisher's container;
        // it stays at `None` and materializes to the DDS defaults.
        // QoS: inline wins; otherwise resolve qos_profile_ref. The
        // profile's §7.3.2.4.3 filter selects by this writer's topic
        // name (`topic_ref`).
        let qos: Option<EntityQos> = if let Some(q) = &dw.qos {
            Some(
                self.resolve_inline_qos(q, |r| r.datawriter_qos_for_topic(&dw.topic_ref).cloned())?,
            )
        } else if let Some(profile_ref) = &dw.qos_profile_ref {
            let r = resolve_profile(&self.qos_libraries, profile_ref)?;
            r.datawriter_qos_for_topic(&dw.topic_ref).cloned()
        } else {
            None
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
        _subscriber: &SubscriberEntry,
        domain: &DomainEntry,
    ) -> Result<ResolvedDataReader, XmlError> {
        let topic = domain
            .topic(&dr.topic_ref)
            .ok_or_else(|| XmlError::UnresolvedReference(format!("topic `{}`", dr.topic_ref)))?;
        let resolved_topic = self.resolve_topic_entry(topic, domain)?;
        // §7.3.5.4.4: a DataReader does not take its Subscriber's QoS.
        // QoS: inline wins; otherwise resolve qos_profile_ref. The
        // profile's §7.3.2.4.3 filter selects by this reader's topic
        // name (`topic_ref`).
        let qos: Option<EntityQos> = if let Some(q) = &dr.qos {
            Some(
                self.resolve_inline_qos(q, |r| r.datareader_qos_for_topic(&dr.topic_ref).cloned())?,
            )
        } else if let Some(profile_ref) = &dr.qos_profile_ref {
            let r = resolve_profile(&self.qos_libraries, profile_ref)?;
            r.datareader_qos_for_topic(&dr.topic_ref).cloned()
        } else {
            None
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
        self.resolve_type_before(name, self.type_libraries.len())
    }

    /// As [`Self::resolve_type`], over the first `before` `<types>` blocks.
    fn resolve_type_before(&self, name: &str, before: usize) -> Option<&TypeDef> {
        let parts: Vec<&str> = name.split("::").collect();
        for lib in self.type_libraries.iter().take(before) {
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
            .map(|dp| {
                self.resolve_application_participant(path, dp, lib.participant_libraries_before)
            })
            .collect()
    }

    /// Resolves one participant written inside an `<application>`.
    ///
    /// The definition is the last link of its own chain: everything it
    /// reaches through `base_name` in a participant library comes first,
    /// and the inline definition overrides it (§7.3.6.4.2).
    fn resolve_application_participant(
        &self,
        app_path: &str,
        entry: &DomainParticipantEntry,
        libraries_before: Option<usize>,
    ) -> Result<ResolvedParticipant, XmlError> {
        let canonical = format!("{app_path}::{}", entry.name);
        let mut chain: Vec<String> = Vec::new();
        let mut entries: Vec<DomainParticipantEntry> = Vec::new();

        if let Some(base) = entry.base_name.as_deref() {
            if !base.contains("::") {
                return Err(XmlError::UnresolvedReference(format!(
                    "application participant `{canonical}` base_name `{base}` \
                     must be qualified `library::name`"
                )));
            }
            chain = self.participant_chain(base)?;
            // §7.3.6.4.2: only libraries defined before the application.
            if let Some(before) = libraries_before {
                for key in &chain {
                    let (library, _) = self.participant_position(key)?;
                    if library >= before {
                        return Err(XmlError::InvalidXml(format!(
                            "application participant `{canonical}` inherits from `{key}`, \
                             whose library is defined after the application (§7.3.6.4.2)"
                        )));
                    }
                }
            }
            for key in &chain {
                entries.push(self.find_participant(key)?.clone());
            }
        }
        chain.push(canonical.clone());
        entries.push(entry.clone());

        self.resolve_participant_chain(canonical, chain, &entries)
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
              <domain_participant name="P" base_name="dpl::P"/>
            </application>
          </application_library>
        </dds>"#;
        let d = parse_dds_xml(xml).expect("parse");
        assert_eq!(d.qos_libraries.len(), 1);
        assert_eq!(d.domain_libraries.len(), 1);
        assert_eq!(d.participant_libraries.len(), 1);
        assert_eq!(d.application_libraries.len(), 1);

        // The application's participant is a definition of its own that
        // inherits the library one; resolving it must reach the domain
        // the base names.
        let resolved = d.resolve_application("al::A").expect("resolve");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "P");
        assert_eq!(resolved[0].domain.name, "D");
        assert_eq!(resolved[0].domain_id, 0);
    }

    #[test]
    fn non_dds_root_rejected() {
        let xml = r#"<other/>"#;
        let err = parse_dds_xml(xml).expect_err("non-dds");
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    /// DDS4CCM 1.1 §7.4.2.3 writes `<qos_profile>` directly under `<dds>`, and
    /// §7.4.2.3.3 the single-QoS shortcut there as well. DDS-XML 1.0 has no such
    /// form: the default reading refuses it and names it, `VendorForms::Accept`
    /// reads it into one library and reports it.
    #[test]
    fn ccm_profiles_directly_under_dds_are_a_vendor_form() {
        let xml = r#"<dds>
          <qos_profile name="Strict">
            <datawriter_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></datawriter_qos>
          </qos_profile>
          <datawriter_qos name="KeepAll">
            <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
          </datawriter_qos>
          <qos_profile name="Child" base_name="Strict"/>
        </dds>"#;
        let refused = parse_dds_xml(xml).expect_err("DDS-XML 1.0 has no such form");
        assert!(matches!(refused, XmlError::VendorForm(_)), "{refused:?}");

        let parsed = parse_dds_xml_with(xml, &ReadOptions::accept_vendor_forms()).expect("accept");
        assert_eq!(parsed.diagnostics.len(), 3, "{:?}", parsed.diagnostics);
        assert_eq!(parsed.value.qos_libraries.len(), 1);
        let library = &parsed.value.qos_libraries[0];
        assert_eq!(library.name, crate::CCM_LIBRARY);
        let names: Vec<&str> = library.profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Strict", "KeepAll", "Child"]);
        // A profile is reached through the library, and base_name works within it.
        let child = crate::resolve_profile(&parsed.value.qos_libraries, "(dds)::Child")
            .expect("the child resolves through its base");
        let writer = crate::default_qos(&child.datawriter_qos).expect("a writer container");
        assert!(writer.reliability.is_some());
    }

    #[test]
    fn a_qos_library_and_ccm_profiles_can_share_a_document() {
        let xml = r#"<dds>
          <qos_library name="L"><qos_profile name="P"/></qos_library>
          <qos_profile name="Q"/>
        </dds>"#;
        let parsed = parse_dds_xml_with(xml, &ReadOptions::accept_vendor_forms()).expect("accept");
        let names: Vec<&str> = parsed
            .value
            .qos_libraries
            .iter()
            .map(|l| l.name.as_str())
            .collect();
        assert_eq!(names, ["L", crate::CCM_LIBRARY]);
    }
}
