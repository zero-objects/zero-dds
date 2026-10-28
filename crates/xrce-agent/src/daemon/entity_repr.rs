// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What the DDS side needs out of a `CREATE`'s representation.
//!
//! DDS-XRCE 1.0 §7.7.3: a client describes each participant, topic,
//! publisher, subscriber and endpoint by a `RepresentationFormat` —
//! `BY_REFERENCE` (a profile the agent holds), `AS_XML_STRING` or
//! `IN_BINARY`. This module reads the two self-contained ones into the
//! names, container relations and QoS the DDS side is created with:
//!
//! * **XML, attribute form** — this crate's own §9.3 dialect
//!   (`crates/xrce/src/xml_config.rs` emits it):
//!   `<topic name="Square" type_name="ShapeType"/>`,
//!   `<data_writer topic_ref="0x0102" reliability="BEST_EFFORT"/>`,
//!   `<participant><domain_id>7</domain_id></participant>`.
//! * **XML, element form** — the Fast DDS profile syntax the eProsima
//!   Micro-XRCE-DDS Client 3.0.1 examples send:
//!   `<dds><topic><name>HelloWorldTopic</name><dataType>HelloWorld</dataType></topic></dds>`,
//!   `<dds><data_writer><topic><kind>NO_KEY</kind><name>…</name><dataType>…</dataType></topic><qos>…</qos></data_writer></dds>`.
//! * **binary** — the `OBJK_*_Binary` structs exactly as Micro-XRCE-DDS
//!   Client 3.0.1 serializes them (`include/uxr/client/core/type/xrce_types.h`,
//!   `src/c/core/serialization/xrce_types.c`: classic CDR, natural
//!   alignment from the start of the blob, the submessage's byte order),
//!   checked against the `BinaryEntityCreation` datagram of
//!   `tools/wire-capture/xrce-sessions/client-to-agent.hex`.
//!
//! Every element the element form or the binary form carries is either
//! applied or named in [`EndpointDescription::not_applied`], which the
//! session reports as an event; nothing is dropped without a trace.
//! A representation that names no topic — a `BY_REFERENCE` topic without
//! a profile store, an XML string without a name — is refused rather than
//! given an invented name.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_types::{CompleteTypeObject, TypeObject};
use zerodds_xml::XmlElement;
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_repr::ObjectVariant;

use crate::daemon::binary_forms;
use crate::daemon::dds_port::ParticipantQos;
use crate::daemon::dds_port::{Durability, EndpointQos, History};
use crate::daemon::definitions::{ParticipantDef, ParticipantScope, SystemDefinitions};
use crate::daemon::profiles::AgentProfiles;

/// Why a representation cannot be turned into a DDS entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReprError {
    /// The XML does not parse, the binary blob is truncated or carries
    /// trailing bytes, or a value is outside its range —
    /// `STATUS_ERR_INVALID_DATA`.
    Malformed(String),
    /// The representation refers to something the agent does not hold —
    /// a `BY_REFERENCE` profile name, no topic name at all —
    /// `STATUS_ERR_UNKNOWN_REFERENCE`.
    Unresolved(String),
    /// The representation asks for something the agent cannot do:
    /// `STATUS_ERR_INCOMPATIBLE` ("an incompatibility between the Client and
    /// the Agent", DDS-XRCE 1.0 §7.8.2).
    Incompatible(String),
}

impl ReprError {
    /// The message, for the event the session logs.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Malformed(m) | Self::Unresolved(m) | Self::Incompatible(m) => m,
        }
    }
}

/// A topic, as the DDS side names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicDescription {
    /// DDS topic name.
    pub topic_name: String,
    /// DDS type name; empty when the representation names the type only by
    /// its `type_identifier`, which the session resolves against the types it
    /// knows.
    pub type_name: String,
    /// The XTypes `TypeIdentifier` the representation names the type with
    /// (`OBJK_Topic_Binary.type_identifier`, §7.7.3.7.3); it takes precedence
    /// over the name.
    pub type_identifier: Option<zerodds_types::TypeIdentifier>,
    /// The type definition `type_name` is registered for, when it is not the
    /// type of that name: a `<register_type name="ShapeType"
    /// type_ref="ShapesDemoTypes::ShapeType"/>` names the type the topic goes
    /// by (`name`) and the definition it is (`type_ref`). `None`: the type
    /// definition is the one named `type_name`.
    pub definition: Option<String>,
    /// The QoS of the Topic (`<topic_qos>`, `qos_profile`): the policies its
    /// DataWriters and DataReaders take where their own representation does
    /// not state them (DDS 1.4 §2.2.2.3.1, the QoS of the Topic is what an
    /// endpoint created for it defaults to), and TOPIC_DATA.
    pub qos: TopicQos,
    /// Elements the representation carries that the agent does not apply.
    pub not_applied: Vec<String>,
}

/// The policies a Topic states (DDS 1.4 §2.2.3, `TopicQos`): each `None` is a
/// policy it leaves to the endpoint.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopicQos {
    /// RELIABILITY: `true` = RELIABLE.
    pub reliable: Option<bool>,
    /// DURABILITY.
    pub durability: Option<Durability>,
    /// HISTORY.
    pub history: Option<History>,
    /// DEADLINE; the inner `None` is infinite.
    pub deadline: Option<Option<Duration>>,
    /// LIFESPAN; the inner `None` is infinite.
    pub lifespan: Option<Option<Duration>>,
    /// OWNERSHIP: `true` = EXCLUSIVE.
    pub exclusive_ownership: Option<bool>,
    /// LIVELINESS.
    pub liveliness: Option<zerodds_qos::LivelinessQosPolicy>,
    /// LATENCY_BUDGET.
    pub latency_budget: Option<zerodds_qos::LatencyBudgetQosPolicy>,
    /// DESTINATION_ORDER.
    pub destination_order: Option<zerodds_qos::DestinationOrderQosPolicy>,
    /// TRANSPORT_PRIORITY.
    pub transport_priority: Option<i32>,
    /// DURABILITY_SERVICE.
    pub durability_service: Option<zerodds_qos::DurabilityServiceQosPolicy>,
    /// RESOURCE_LIMITS.
    pub resource_limits: Option<zerodds_qos::ResourceLimitsQosPolicy>,
    /// TOPIC_DATA.
    pub topic_data: Option<Vec<u8>>,
}

impl TopicQos {
    /// The policies `other` states over those of `self`.
    fn overlay(&mut self, other: Self) {
        macro_rules! over {
            ($($f:ident),*) => { $( if other.$f.is_some() { self.$f = other.$f; } )* };
        }
        over!(
            reliable,
            durability,
            history,
            deadline,
            lifespan,
            exclusive_ownership,
            liveliness,
            latency_budget,
            destination_order,
            transport_priority,
            durability_service,
            resource_limits,
            topic_data
        );
    }

    /// Whether the Topic states no policy.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The policies of the Topic onto the QoS of an endpoint that has not
    /// stated its own yet.
    pub fn apply_to(&self, qos: &mut EndpointQos) {
        if let Some(v) = self.reliable {
            qos.reliable = v;
        }
        if let Some(v) = self.durability {
            qos.durability = v;
        }
        if let Some(v) = self.history {
            qos.history = v;
        }
        if let Some(v) = self.deadline {
            qos.deadline = v;
        }
        if let Some(v) = self.lifespan {
            qos.lifespan = v;
        }
        if let Some(v) = self.exclusive_ownership {
            qos.exclusive_ownership = v;
        }
        if let Some(v) = self.liveliness {
            qos.liveliness = v;
        }
        if let Some(v) = self.latency_budget {
            qos.latency_budget = v;
        }
        if let Some(v) = self.destination_order {
            qos.destination_order = v;
        }
        if let Some(v) = self.transport_priority {
            qos.transport_priority = v;
        }
        if let Some(v) = self.durability_service {
            qos.durability_service = v;
        }
        if let Some(v) = self.resource_limits {
            qos.resource_limits = v;
        }
        if let Some(v) = &self.topic_data {
            qos.topic_data.clone_from(v);
        }
    }

    /// The policies of a `<topic_qos>` of a profile.
    ///
    /// # Errors
    /// [`ReprError::Malformed`] for a KEEP_LAST depth of 0 or less.
    fn from_entity_qos(
        reference: &str,
        q: &zerodds_xml::qos::EntityQos,
    ) -> Result<Self, ReprError> {
        let mut out = Self::default();
        if let Some(r) = &q.reliability {
            out.reliable = Some(r.kind == zerodds_qos::ReliabilityKind::Reliable);
        }
        out.durability = q.durability.as_ref().map(|d| match d.kind {
            zerodds_qos::DurabilityKind::Volatile => Durability::Volatile,
            zerodds_qos::DurabilityKind::TransientLocal => Durability::TransientLocal,
            zerodds_qos::DurabilityKind::Transient => Durability::Transient,
            zerodds_qos::DurabilityKind::Persistent => Durability::Persistent,
        });
        if let Some(h) = &q.history {
            out.history = Some(match h.kind {
                zerodds_qos::HistoryKind::KeepAll => History::KeepAll,
                zerodds_qos::HistoryKind::KeepLast => {
                    History::KeepLast(u32::try_from(h.depth).ok().filter(|d| *d > 0).ok_or_else(
                        || {
                            ReprError::Malformed(format!(
                                "topic qos `{reference}`: KEEP_LAST depth {}",
                                h.depth
                            ))
                        },
                    )?)
                }
            });
        }
        out.deadline = q.deadline.as_ref().map(|d| std_duration(d.period));
        out.lifespan = q.lifespan.as_ref().map(|l| std_duration(l.duration));
        out.exclusive_ownership = q
            .ownership
            .as_ref()
            .map(|o| o.kind == zerodds_qos::OwnershipKind::Exclusive);
        out.liveliness = q.liveliness;
        out.latency_budget = q.latency_budget;
        out.destination_order = q.destination_order;
        out.transport_priority = q.transport_priority.as_ref().map(|t| t.value);
        out.durability_service = q.durability_service;
        out.resource_limits = q.resource_limits;
        out.topic_data = q.topic_data.as_ref().map(|t| t.value.clone());
        Ok(out)
    }
}

/// How an endpoint names its topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopicRef {
    /// By the topic's `ObjectId` — the attribute form's `topic_ref`, the
    /// binary form's `topic_id`.
    Object(ObjectId),
    /// By the topic's DDS name — the element form's `<topic><name>`.
    Name(String),
}

/// Whether an endpoint is a writer or a reader; the defaults and the
/// fields that exist differ (DDS 1.4 §2.2.3: RELIABILITY defaults to
/// RELIABLE for a DataWriter, BEST_EFFORT for a DataReader).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointRole {
    /// `OBJK_DATAWRITER`.
    Writer,
    /// `OBJK_DATAREADER`.
    Reader,
}

/// A DataWriter or DataReader, as the DDS side creates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointDescription {
    /// The topic, if the representation names one; `None` leaves the
    /// choice to the session (the client's only topic).
    pub topic: Option<TopicRef>,
    /// The type name, if the representation repeats it (element form).
    pub type_name: Option<String>,
    /// `<kind>WITH_KEY</kind>` / `NO_KEY`, if the representation says.
    pub keyed: Option<bool>,
    /// QoS: the DDS defaults for the role, overridden by what the
    /// representation carries.
    pub qos: EndpointQos,
    /// Elements the representation carries that the agent does not apply,
    /// by their path in the XML (`qos/publishMode`).
    pub not_applied: Vec<String>,
}

/// A Publisher or Subscriber: what its endpoints inherit (DDS 1.4
/// §2.2.3.13 PARTITION, §2.2.3.2 GROUP_DATA) and the policies of the
/// entity itself (§2.2.3.6 PRESENTATION, §2.2.3.20 ENTITY_FACTORY).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerDescription {
    /// PARTITION names.
    pub partitions: Vec<String>,
    /// GROUP_DATA.
    pub group_data: Vec<u8>,
    /// PRESENTATION; the DDS default is INSTANCE scope, no coherent or
    /// ordered access.
    pub presentation: zerodds_qos::PresentationQosPolicy,
    /// ENTITY_FACTORY `autoenable_created_entities`; the DDS default is
    /// `true`.
    pub autoenable_created_entities: bool,
    /// The `<data_writer>` or `<data_reader>` elements the XML holds: nested
    /// objects the CREATE of the Publisher or Subscriber makes too
    /// (§7.7.3.8.1, §7.7.3.9.1).
    pub embedded: Vec<XmlElement>,
    /// Elements not applied, as for an endpoint.
    pub not_applied: Vec<String>,
}

impl Default for ContainerDescription {
    fn default() -> Self {
        Self {
            partitions: Vec::new(),
            group_data: Vec::new(),
            presentation: zerodds_qos::PresentationQosPolicy::default(),
            autoenable_created_entities: true,
            embedded: Vec::new(),
            not_applied: Vec::new(),
        }
    }
}

/// The element a `BY_REFERENCE` representation names in the agent's
/// profiles (`crate::daemon::profiles`).
fn referenced<'p>(
    profiles: &'p AgentProfiles,
    element: &str,
    name: &str,
) -> Result<&'p XmlElement, ReprError> {
    profiles.entity(element, name).ok_or_else(|| {
        ReprError::Unresolved(format!(
            "no <{element} profile_name=\"{name}\"> in the agent's xml_profile_files"
        ))
    })
}

/// A participant, as the DDS side creates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantDescription {
    /// The DDS domain.
    pub domain_id: i32,
    /// What the Domain a binary `domain_reference` names puts in the scope of
    /// the participant (its topics and type registrations); `None` for the
    /// other forms, whose scope is the one of their definition.
    pub scope: Option<ParticipantScope>,
    /// The policies of the participant (`qos_profile_reference`).
    pub qos: ParticipantQos,
    /// Elements the representation carries that the agent does not apply.
    pub not_applied: Vec<String>,
}

/// The participant a `CREATE(OBJK_PARTICIPANT)` describes. The wire
/// `OBJK_PARTICIPANT_Representation.domain_id` (§7.7.3, Annex A) is the
/// participant's domain; an XML representation (inline or the referenced
/// profile) that names a domain of its own (`<domain_id>` in the
/// attribute form, `<domainId>` in the Fast DDS form) overrides it. A
/// binary representation (`OBJK_DomainParticipant_Binary`, §7.7.3.6.3,
/// Annex A) may repeat the domain (`long domain_id` in the form of the
/// clause; the two must agree) and names an XRCE Domain and a QoS profile
/// by reference.
///
/// # Errors
/// [`ReprError::Malformed`] if the XML does not parse or its domain is
/// not a number, if neither the wire nor the XML gives a domain, or the
/// binary form is none of the XCDR2 forms; [`ReprError::Unresolved`] for a
/// reference to a profile, a QoS profile or an XRCE Domain the agent does
/// not hold.
pub fn participant(
    variant: &ObjectVariant,
    wire_domain_id: Option<i16>,
    big_endian: bool,
    profiles: &AgentProfiles,
) -> Result<ParticipantDescription, ReprError> {
    let parsed;
    let not_applied = Vec::new();
    let mut participant_qos = ParticipantQos::default();
    let mut domain_scope = None;
    let mut binary_domain = None;
    // The domain of a DDS-XML DomainParticipant definition (§7.7.3.6.1,
    // §7.7.3.6.2): its `domain_id`, or the one of its `domain_ref` Domain.
    let mut defined_domain = None;
    let root = match variant {
        ObjectVariant::ByXmlString(xml) => {
            parsed = parse(xml)?;
            if let Some(source) = parsed
                .as_ref()
                .map(|root| participant_source(root, profiles))
                .transpose()?
                .flatten()
            {
                defined_domain = Some(source.domain_id()?);
                let el = source
                    .definitions
                    .effective_element(&source.def, 0)
                    .map_err(ReprError::Unresolved)?;
                participant_qos = definition_participant_qos(&el, profiles)?;
                None
            } else {
                parsed.as_ref()
            }
        }
        ObjectVariant::ByReference(name) => {
            if let Some(def) = profiles.definitions().participant(name) {
                defined_domain = Some(
                    profiles
                        .definitions()
                        .domain_id_of(def)
                        .map_err(ReprError::Unresolved)?,
                );
                let el = profiles
                    .definitions()
                    .effective_element(def, 0)
                    .map_err(ReprError::Unresolved)?;
                participant_qos = definition_participant_qos(&el, profiles)?;
                None
            } else {
                Some(referenced(profiles, "participant", name)?)
            }
        }
        ObjectVariant::InBinary(bytes) => {
            let binary = binary_forms::participant(bytes, big_endian)?;
            binary_domain = binary.domain_id;
            // §7.7.3.6.3: the references name definitions "known to the
            // Agent". A `domain_reference` is a Domain of a
            // `<domain_library>` the agent has read; one in no library is an
            // unknown reference.
            if let Some(reference) = &binary.domain_reference {
                let domain = profiles.definitions().domain(reference).ok_or_else(|| {
                    ReprError::Unresolved(format!(
                        "XRCE Domain `{reference}` is in no <domain_library> of this agent"
                    ))
                })?;
                if let Some(id) = domain.domain_id {
                    let id = i32::try_from(id).map_err(|_| {
                        ReprError::Malformed(format!(
                            "Domain `{reference}`: domain_id {id} is out of range"
                        ))
                    })?;
                    // The binary form repeats the domain (`long domain_id`);
                    // the two must name the same one.
                    if binary_domain.is_some_and(|b| b != id) {
                        return Err(ReprError::Malformed(format!(
                            "the representation says domain {} and Domain `{reference}` {id}",
                            binary_domain.unwrap_or_default()
                        )));
                    }
                    defined_domain = Some(id);
                }
                domain_scope = profiles.definitions().domain_scope(reference);
            }
            if let Some(qos) = &binary.qos_profile_reference {
                let resolved = profiles.qos_profile(qos).map_err(|e| {
                    ReprError::Unresolved(format!("participant qos_profile `{qos}`: {e}"))
                })?;
                participant_qos = participant_qos_of(qos, &resolved)?;
            }
            None
        }
    };
    let domain_id = 'domain: {
        if let Some(domain) = defined_domain {
            break 'domain domain;
        }
        if let Some(root) = root {
            let participant = if root.name == "participant" {
                Some(root)
            } else {
                root.child("participant")
            };
            let named = if root.name == "domain_id" {
                Some(root)
            } else {
                participant.and_then(|p| p.child("domain_id").or_else(|| p.child("domainId")))
            };
            if let Some(el) = named {
                break 'domain el.text.trim().parse().map_err(|_| {
                    ReprError::Malformed(format!(
                        "participant domain `{}` is not a number",
                        el.text
                    ))
                })?;
            }
        }
        match (wire_domain_id.map(i32::from), binary_domain) {
            (Some(wire), Some(binary)) if wire != binary => {
                return Err(ReprError::Malformed(format!(
                    "the representation says domain {wire} and its binary form {binary}"
                )));
            }
            (Some(domain), _) | (None, Some(domain)) => domain,
            (None, None) => {
                return Err(ReprError::Malformed(
                    "participant CREATE without a domain_id".into(),
                ));
            }
        }
    };
    Ok(ParticipantDescription {
        domain_id,
        scope: domain_scope,
        qos: participant_qos,
        not_applied,
    })
}

/// The QoS a `<domain_participant>` definition states: its
/// `<domain_participant_qos>` (DDS-XML §7.3.5.4.3, a vendor spelling is
/// `<domainparticipant_qos>`, refused unless `vendor_forms: accept`) with its
/// `base_name` profile, applied the way a
/// `qos_profile_reference` is.
///
/// # Errors
/// [`ReprError::Unresolved`] for a `base_name` the agent does not hold,
/// [`ReprError::Incompatible`] for a policy a DomainParticipant does not have.
fn definition_participant_qos(
    el: &XmlElement,
    profiles: &AgentProfiles,
) -> Result<ParticipantQos, ReprError> {
    let Some(qos_el) = el.children.iter().find(|c| {
        matches!(
            c.name.as_str(),
            "domain_participant_qos" | "domainparticipant_qos"
        )
    }) else {
        return Ok(ParticipantQos::default());
    };
    let resolved = profiles
        .qos_element(qos_el)
        .map_err(ReprError::Unresolved)?;
    participant_qos_of(&qos_el.name, &resolved)
}

/// The policies of a DomainParticipant a `qos_profile_reference` names (the
/// `<domain_participant_qos>` of the profile, DDS-XML §7.3.2): the ones a DDS
/// DomainParticipant has (DDS 1.4 §2.2.2.2.1.1) are applied, USER_DATA and
/// ENTITY_FACTORY; a profile that has no participant QoS applies nothing.
///
/// # Errors
/// [`ReprError::Incompatible`] for a policy the participant QoS of the profile
/// sets that a DomainParticipant does not have: it cannot be applied and is
/// not accepted and ignored.
fn participant_qos_of(
    reference: &str,
    resolved: &zerodds_xml::qos_inheritance::ResolvedQos,
) -> Result<ParticipantQos, ReprError> {
    let mut out = ParticipantQos::default();
    let Some(q) = zerodds_xml::default_qos(&resolved.domainparticipant_qos) else {
        return Ok(out);
    };
    if let Some(u) = &q.user_data {
        out.user_data.clone_from(&u.value);
    }
    if let Some(f) = &q.entity_factory {
        out.autoenable_created_entities = f.autoenable_created_entities;
    }
    let others = [
        (q.durability.is_some(), "durability"),
        (q.durability_service.is_some(), "durability_service"),
        (q.presentation.is_some(), "presentation"),
        (q.deadline.is_some(), "deadline"),
        (q.latency_budget.is_some(), "latency_budget"),
        (q.ownership.is_some(), "ownership"),
        (q.ownership_strength.is_some(), "ownership_strength"),
        (q.liveliness.is_some(), "liveliness"),
        (q.time_based_filter.is_some(), "time_based_filter"),
        (q.partition.is_some(), "partition"),
        (q.reliability.is_some(), "reliability"),
        (q.transport_priority.is_some(), "transport_priority"),
        (q.lifespan.is_some(), "lifespan"),
        (q.destination_order.is_some(), "destination_order"),
        (q.history.is_some(), "history"),
        (q.resource_limits.is_some(), "resource_limits"),
        (q.writer_data_lifecycle.is_some(), "writer_data_lifecycle"),
        (q.reader_data_lifecycle.is_some(), "reader_data_lifecycle"),
        (q.topic_data.is_some(), "topic_data"),
        (q.group_data.is_some(), "group_data"),
    ];
    let named: Vec<&str> = others
        .iter()
        .filter(|(set, _)| *set)
        .map(|(_, name)| *name)
        .collect();
    if named.is_empty() {
        Ok(out)
    } else {
        Err(ReprError::Incompatible(format!(
            "participant qos_profile `{reference}` sets {}, which a DomainParticipant does not \
             have (it has USER_DATA and ENTITY_FACTORY)",
            named.join(", ")
        )))
    }
}

/// A DomainParticipant definition and the definitions around it (its Domain,
/// the participants it inherits from), so that it can be planned.
#[derive(Debug, Clone)]
pub struct ParticipantSource {
    /// The participant.
    pub def: ParticipantDef,
    /// The definitions of the agent with the participant in them.
    pub definitions: SystemDefinitions,
}

impl ParticipantSource {
    /// The DDS domain of the participant.
    ///
    /// # Errors
    /// [`ReprError::Unresolved`] if it names no domain, or a `domain_ref` the
    /// agent does not hold.
    pub fn domain_id(&self) -> Result<i32, ReprError> {
        self.definitions
            .domain_id_of(&self.def)
            .map_err(ReprError::Unresolved)
    }

    /// What is in the scope of the participant: its type registrations and
    /// topics, those of its Domain included.
    ///
    /// # Errors
    /// [`ReprError::Unresolved`] if its `base_name` chain cannot be followed.
    pub fn scope(&self) -> Result<ParticipantScope, ReprError> {
        self.definitions
            .scope_of(&self.def)
            .map_err(ReprError::Unresolved)
    }
}

/// The DomainParticipant a `<domain_participant_library>` (the form of
/// §7.7.3.6.2) or a bare `<domain_participant>` holds; `None` for any other
/// root. The library holds a single participant.
fn participant_source(
    root: &XmlElement,
    profiles: &AgentProfiles,
) -> Result<Option<ParticipantSource>, ReprError> {
    // A DDS-XML document wraps its definitions in `<dds>` (§7.3.2); the
    // DomainParticipant definition is the element inside it.
    let inner = if root.name == "dds" {
        root.children.iter().find(|c| {
            matches!(
                c.name.as_str(),
                "domain_participant_library" | "domain_participant"
            )
        })
    } else {
        Some(root)
    };
    let Some(root) = inner else {
        return Ok(None);
    };
    let library = match root.name.as_str() {
        "domain_participant_library" => root.clone(),
        "domain_participant" => XmlElement {
            name: "domain_participant_library".into(),
            attributes: [("name".to_string(), "(inline)".to_string())].into(),
            children: vec![root.clone()],
            ..XmlElement::default()
        },
        _ => return Ok(None),
    };
    let mut definitions = profiles.definitions().clone();
    let before: Vec<String> = definitions
        .participants()
        .map(|p| p.reference.clone())
        .collect();
    definitions
        .add_participant_library(&library)
        .map_err(ReprError::Malformed)?;
    let mut added = definitions
        .participants()
        .filter(|p| !before.contains(&p.reference));
    match (added.next(), added.next()) {
        (Some(def), None) => Ok(Some(ParticipantSource {
            def: def.clone(),
            definitions: definitions.clone(),
        })),
        _ => Err(ReprError::Malformed(
            "a <domain_participant_library> holds a single <domain_participant> (§7.7.3.6.2)"
                .into(),
        )),
    }
}

/// The DomainParticipant definition a `CREATE(OBJK_PARTICIPANT)` carries: the
/// one a `BY_REFERENCE` representation names in the agent's files, or the one
/// an XML representation defines. `None` for a binary representation and for
/// a Fast DDS profile, which have no nested objects.
///
/// # Errors
/// [`ReprError::Malformed`] if the XML does not parse or is not a single
/// participant.
pub fn participant_definition(
    variant: &ObjectVariant,
    profiles: &AgentProfiles,
) -> Result<Option<ParticipantSource>, ReprError> {
    match variant {
        ObjectVariant::ByReference(name) => {
            Ok(profiles
                .definitions()
                .participant(name)
                .map(|def| ParticipantSource {
                    def: def.clone(),
                    definitions: profiles.definitions().clone(),
                }))
        }
        ObjectVariant::ByXmlString(xml) => match parse(xml)? {
            Some(root) => participant_source(&root, profiles),
            None => Ok(None),
        },
        ObjectVariant::InBinary(_) => Ok(None),
    }
}

/// The participants an Application definition holds: the one a
/// `BY_REFERENCE` representation names, or the single `<application>` of an
/// `<application_library>` (§7.7.3.5.2). `None` for XML that is no
/// application.
///
/// # Errors
/// [`ReprError::Unresolved`] for a reference to no Application;
/// [`ReprError::Malformed`] if the XML does not parse or defines other than
/// one application.
pub fn application_definition(
    variant: &ObjectVariant,
    profiles: &AgentProfiles,
) -> Result<Vec<ParticipantSource>, ReprError> {
    let (definitions, reference) = match variant {
        ObjectVariant::ByReference(name) => {
            if profiles.definitions().application(name).is_none() {
                return Err(ReprError::Unresolved(format!(
                    "no application `{name}` in the agent's definitions"
                )));
            }
            (profiles.definitions().clone(), name.clone())
        }
        ObjectVariant::ByXmlString(xml) => {
            let root = parse(xml)?
                .ok_or_else(|| ReprError::Malformed("empty application representation".into()))?;
            if root.name != "application_library" {
                return Err(ReprError::Malformed(format!(
                    "an application is an <application_library>, not <{}> (§7.7.3.5.2)",
                    root.name
                )));
            }
            let mut definitions = profiles.definitions().clone();
            let before: Vec<String> = definitions
                .applications()
                .map(|a| a.reference.clone())
                .collect();
            definitions
                .add_application_library(&root)
                .map_err(ReprError::Malformed)?;
            let added: Vec<String> = definitions
                .applications()
                .filter(|a| !before.contains(&a.reference))
                .map(|a| a.reference.clone())
                .collect();
            match added.as_slice() {
                [reference] => (definitions, reference.clone()),
                _ => {
                    return Err(ReprError::Malformed(
                        "an <application_library> holds a single <application> (§7.7.3.5.2)".into(),
                    ));
                }
            }
        }
        ObjectVariant::InBinary(_) => {
            return Err(ReprError::Malformed(
                "an application has no binary representation (§7.7.3.5)".into(),
            ));
        }
    };
    let app = definitions
        .application(&reference)
        .ok_or_else(|| ReprError::Unresolved(format!("application `{reference}`")))?
        .clone();
    app.participants
        .iter()
        .map(|p| {
            let def = definitions
                .participant(p)
                .ok_or_else(|| ReprError::Unresolved(format!("participant `{p}`")))?;
            Ok(ParticipantSource {
                def: def.clone(),
                definitions: definitions.clone(),
            })
        })
        .collect()
}

/// The topic a `CREATE(OBJK_TOPIC)` describes. `scope` is what the topic's
/// DomainParticipant defines (its type registrations and topics, those of its
/// Domain included): a `BY_REFERENCE` representation is "the bare name of an
/// XRCE Topic defined in" it (§7.7.3.7.1), and a `register_type_ref`
/// (§7.7.3.7.2) is one of its registrations.
///
/// # Errors
/// [`ReprError::Unresolved`] for a reference to a topic or profile the agent
/// does not hold, or a representation without a topic or type name;
/// [`ReprError::Malformed`] if it does not parse.
pub fn topic(
    variant: &ObjectVariant,
    big_endian: bool,
    profiles: &AgentProfiles,
    scope: Option<&ParticipantScope>,
) -> Result<TopicDescription, ReprError> {
    match variant {
        ObjectVariant::ByReference(name) => {
            // §7.7.3.7.1: a Topic of the participant or of its Domain; the
            // Fast DDS `profile_name` is the eProsima client's way.
            match scope.and_then(|s| s.topic(name)) {
                Some(el) => topic_from_xml(el, scope, profiles),
                None => topic_from_xml(
                    profiles.entity("topic", name).ok_or_else(|| {
                        ReprError::Unresolved(format!(
                            "no topic named `{name}` in the DomainParticipant or its Domain, and \
                             no <topic profile_name=\"{name}\"> in the agent's xml_profile_files"
                        ))
                    })?,
                    scope,
                    profiles,
                ),
            }
        }
        ObjectVariant::ByXmlString(xml) => {
            let root = parse(xml)?
                .ok_or_else(|| ReprError::Unresolved("empty topic representation".into()))?;
            let el = if root.name == "topic" {
                &root
            } else {
                root.child("topic")
                    .ok_or_else(|| ReprError::Unresolved("XML names no <topic>".into()))?
            };
            topic_from_xml(el, scope, profiles)
        }
        ObjectVariant::InBinary(bytes) => {
            // §7.7.3.7.3, Annex A: OBJK_Topic_Binary { string<256>
            // topic_name; @optional string<256> type_reference; @optional
            // TypeIdentifier type_identifier; } in XCDR2.
            let spec = match binary_forms::topic(bytes, big_endian) {
                Ok(t) => {
                    return Ok(TopicDescription {
                        topic_name: t.topic_name,
                        type_name: t.type_reference.unwrap_or_default(),
                        type_identifier: t.type_identifier,
                        definition: None,
                        qos: TopicQos::default(),
                        not_applied: Vec::new(),
                    });
                }
                Err(e) => e,
            };
            // The structure the eProsima client sends, in classic CDR:
            // OBJK_Topic_Binary { string topic_name; @optional string
            // type_reference; @optional string type_name; }
            let eprosima = (|| {
                let mut r = Cdr::new(bytes, big_endian);
                let topic_name = r.string()?;
                let type_reference = r.optional(Cdr::string)?;
                let type_name = r.optional(Cdr::string)?;
                r.finish("OBJK_Topic_Binary")?;
                Ok::<_, ReprError>((topic_name, type_reference, type_name))
            })();
            let (topic_name, type_reference, type_name) = eprosima.map_err(|e| {
                ReprError::Malformed(format!(
                    "{}; as the eProsima structure: {}",
                    spec.message(),
                    e.message()
                ))
            })?;
            let type_name = type_name.or(type_reference).ok_or_else(|| {
                ReprError::Malformed(format!(
                    "topic `{topic_name}` names neither a type_reference nor a type (§7.7.3.7.3)"
                ))
            })?;
            Ok(TopicDescription {
                topic_name,
                type_name,
                type_identifier: None,
                definition: None,
                qos: TopicQos::default(),
                not_applied: Vec::new(),
            })
        }
    }
}

/// A `<topic>` element, attribute or element form. The type is named by
/// `type_name` (attribute form), `<dataType>` (Fast DDS form) or a
/// `register_type_ref` (DDS-XML §7.3.4.4.3), which is one of the type
/// registrations in `scope`: the topic goes by the registered name, and the
/// registration's `type_ref` is the definition. The topic's keyed-ness is its
/// type's (`<kind>` restates it); everything else it carries is a topic-level
/// QoS the agent does not apply and is named.
fn topic_from_xml(
    el: &XmlElement,
    scope: Option<&ParticipantScope>,
    profiles: &AgentProfiles,
) -> Result<TopicDescription, ReprError> {
    let topic_name = attribute_or_child(el, "name", "name")
        .ok_or_else(|| ReprError::Unresolved("<topic> without a name".into()))?;
    let mut definition = el.attribute("type_ref").map(ToString::to_string);
    let type_name = match attribute_or_child(el, "type_name", "dataType") {
        Some(name) => name,
        None => {
            let registered = el.attribute("register_type_ref").ok_or_else(|| {
                ReprError::Unresolved(format!("<topic name=\"{topic_name}\"> without a type name"))
            })?;
            let registration =
                scope
                    .and_then(|s| s.register_type(registered))
                    .ok_or_else(|| {
                        ReprError::Unresolved(format!(
                            "topic `{topic_name}`: register_type_ref `{registered}` is no type \
                         registration of the DomainParticipant or its Domain"
                        ))
                    })?;
            definition = Some(registration.type_ref.clone());
            registration.name.clone()
        }
    };
    // The QoS of the Topic: a profile its `qos_profile` / `qos_profile_ref`
    // names, then the `<topic_qos>` (with its `base_name`) over it.
    let mut qos = TopicQos::default();
    let mut not_applied = Vec::new();
    for attr in ["qos_profile", "qos_profile_ref"] {
        if attr == "qos_profile_ref" {
            vendor_qos_profile_ref(el, profiles)?;
        }
        if let Some(profile) = el.attribute(attr) {
            let resolved = profiles
                .qos_profile(profile)
                .map_err(ReprError::Unresolved)?;
            // DDS-XML 1.0 §7.3.2.4.3: the container whose topic_filter
            // matches the topic, else the one without a filter.
            if let Some(t) =
                zerodds_xml::qos::select_qos_for_topic(&resolved.topic_qos, &topic_name)
            {
                qos = TopicQos::from_entity_qos(profile, t)?;
            }
        }
    }
    for child in &el.children {
        match child.name.as_str() {
            "name" | "dataType" | "kind" => {}
            "topic_qos" => {
                let resolved = profiles.qos_element(child).map_err(ReprError::Unresolved)?;
                if let Some(t) =
                    zerodds_xml::qos::select_qos_for_topic(&resolved.topic_qos, &topic_name)
                {
                    let from = TopicQos::from_entity_qos(&topic_name, t)?;
                    qos.overlay(from);
                }
            }
            other => not_applied.push(other.to_string()),
        }
    }
    Ok(TopicDescription {
        topic_name,
        type_name,
        type_identifier: None,
        definition,
        qos,
        not_applied,
    })
}

/// The DataWriter/DataReader a `CREATE(OBJK_DATAWRITER|OBJK_DATAREADER)`
/// describes.
///
/// # Errors
/// [`ReprError::Unresolved`] for a reference to a profile or a
/// `qos_profile` the agent does not hold; [`ReprError::Malformed`] if it
/// does not parse or a value is out of range.
pub fn endpoint(
    variant: &ObjectVariant,
    role: EndpointRole,
    big_endian: bool,
    profiles: &AgentProfiles,
) -> Result<EndpointDescription, ReprError> {
    endpoint_for_topic(variant, role, big_endian, profiles, None)
}

/// As [`endpoint`], for an endpoint of a Topic that states QoS of its own:
/// the policies of the Topic are the defaults of the endpoint, and whatever
/// the endpoint's representation states overrides them.
///
/// # Errors
/// As [`endpoint`].
pub fn endpoint_for_topic(
    variant: &ObjectVariant,
    role: EndpointRole,
    big_endian: bool,
    profiles: &AgentProfiles,
    topic_qos: Option<&TopicQos>,
) -> Result<EndpointDescription, ReprError> {
    let mut defaults = match role {
        EndpointRole::Writer => EndpointQos::writer_default(),
        EndpointRole::Reader => EndpointQos::reader_default(),
    };
    if let Some(t) = topic_qos {
        t.apply_to(&mut defaults);
    }
    let mut out = EndpointDescription {
        topic: None,
        type_name: None,
        keyed: None,
        qos: defaults,
        not_applied: Vec::new(),
    };
    let tag = match role {
        EndpointRole::Writer => "data_writer",
        EndpointRole::Reader => "data_reader",
    };
    match variant {
        ObjectVariant::ByReference(name) => {
            endpoint_from_xml(referenced(profiles, tag, name)?, role, profiles, &mut out)?;
            Ok(out)
        }
        ObjectVariant::ByXmlString(xml) => {
            let Some(root) = parse(xml)? else {
                return Ok(out);
            };
            let el = if root.name == tag {
                &root
            } else {
                root.child(tag).ok_or_else(|| {
                    ReprError::Malformed(format!("XML names no <{tag}> (root <{}>)", root.name))
                })?
            };
            endpoint_from_xml(el, role, profiles, &mut out)?;
            Ok(out)
        }
        ObjectVariant::InBinary(bytes) => {
            // §7.7.3.10.2, §7.7.3.11.2, Annex A: OBJK_DataWriter_Binary
            // { string topic_name; ... } in XCDR2.
            let spec = match binary_forms::endpoint(bytes, big_endian, role == EndpointRole::Writer)
            {
                Ok(b) => {
                    out.topic = Some(TopicRef::Name(b.topic_name));
                    if let Some(q) = &b.qos {
                        apply_endpoint_binary(q, b.bit1_is_keep_all, role, &mut out.qos)?;
                    }
                    return Ok(out);
                }
                Err(e) => e,
            };
            // The structure the eProsima client sends, in classic CDR:
            // OBJK_DataWriter_Binary { ObjectId topic_id; @optional
            // OBJK_DataWriter_Binary_Qos qos; } (reader likewise).
            let eprosima = (|| {
                let mut r = Cdr::new(bytes, big_endian);
                let topic = TopicRef::Object(r.object_id()?);
                let mut qos = out.qos.clone();
                if r.bool()? {
                    endpoint_qos_from_binary(&mut r, role, &mut qos)?;
                }
                r.finish(match role {
                    EndpointRole::Writer => "OBJK_DataWriter_Binary",
                    EndpointRole::Reader => "OBJK_DataReader_Binary",
                })?;
                Ok::<_, ReprError>((topic, qos))
            })();
            let (topic, qos) = eprosima.map_err(|e| {
                ReprError::Malformed(format!(
                    "{}; as the eProsima structure: {}",
                    spec.message(),
                    e.message()
                ))
            })?;
            out.topic = Some(topic);
            out.qos = qos;
            Ok(out)
        }
    }
}

/// The Publisher (`element` = `publisher`) or Subscriber (`subscriber`)
/// a CREATE describes. An empty XML string (what the eProsima examples
/// send) is a container with default QoS.
///
/// # Errors
/// [`ReprError::Unresolved`] for a reference to a profile the agent does
/// not hold; [`ReprError::Malformed`] if it does not parse.
pub fn container(
    variant: &ObjectVariant,
    element: &str,
    big_endian: bool,
    profiles: &AgentProfiles,
) -> Result<ContainerDescription, ReprError> {
    let mut out = ContainerDescription::default();
    match variant {
        ObjectVariant::ByReference(name) => {
            container_from_xml(
                referenced(profiles, element, name)?,
                element,
                profiles,
                &mut out,
            )?;
            Ok(out)
        }
        ObjectVariant::ByXmlString(xml) => {
            let Some(root) = parse(xml)? else {
                return Ok(out);
            };
            let el = if root.name == element {
                Some(&root)
            } else {
                root.child(element)
            };
            if let Some(el) = el {
                container_from_xml(el, element, profiles, &mut out)?;
            }
            Ok(out)
        }
        ObjectVariant::InBinary(bytes) => {
            // Annex A: both structures are @extensibility(APPENDABLE), a
            // DHEADER in front.
            if let Ok(c) = binary_forms::container(bytes, big_endian) {
                if let Some(p) = c.partitions {
                    out.partitions = p;
                }
                if let Some(g) = c.group_data {
                    out.group_data = g;
                }
                return Ok(out);
            }
            // OBJK_Publisher_Binary { @optional string publisher_name;
            // @optional OBJK_Publisher_Binary_Qos qos; } and
            // OBJK_Publisher_Binary_Qos { @optional StringSequence
            // partitions; @optional BinarySequence group_data; } — the
            // subscriber's is the same layout. This is the form of the
            // clauses (§7.7.3.8.2) and the one the eProsima client sends.
            let mut r = Cdr::new(bytes, big_endian);
            let _name = r.optional(Cdr::string)?;
            if r.bool()? {
                if let Some(p) = r.optional(Cdr::string_seq)? {
                    out.partitions = p;
                }
                if let Some(g) = r.optional(Cdr::octets)? {
                    out.group_data = g;
                }
            }
            r.finish("OBJK_Publisher_Binary")?;
            Ok(out)
        }
    }
}

fn container_from_xml(
    el: &XmlElement,
    element: &str,
    profiles: &AgentProfiles,
    out: &mut ContainerDescription,
) -> Result<(), ReprError> {
    let endpoint_tag = if element == "publisher" {
        "data_writer"
    } else {
        "data_reader"
    };
    for child in &el.children {
        match child.name.as_str() {
            // DDS-XML §7.3.5.4.4/5: `<publisher_qos base_name="…">`, the
            // policies over a profile (§7.7.3.8.1).
            "publisher_qos" | "subscriber_qos" => {
                let resolved = profiles.qos_element(child).map_err(ReprError::Unresolved)?;
                let qos = if child.name == "publisher_qos" {
                    resolved.publisher_qos
                } else {
                    resolved.subscriber_qos
                };
                if let Some(q) = zerodds_xml::default_qos(&qos) {
                    if let Some(p) = &q.partition {
                        out.partitions.clone_from(&p.names);
                    }
                    if let Some(g) = &q.group_data {
                        out.group_data.clone_from(&g.value);
                    }
                    if let Some(p) = &q.presentation {
                        out.presentation = *p;
                    }
                    if let Some(f) = &q.entity_factory {
                        out.autoenable_created_entities = f.autoenable_created_entities;
                    }
                    for (set, name) in [
                        (q.deadline.is_some(), "deadline"),
                        (q.latency_budget.is_some(), "latency_budget"),
                        (q.ownership.is_some(), "ownership"),
                        (q.durability.is_some(), "durability"),
                        (q.reliability.is_some(), "reliability"),
                        (q.user_data.is_some(), "user_data"),
                        (q.topic_data.is_some(), "topic_data"),
                    ] {
                        if set {
                            out.not_applied.push(format!("{}/{name}", child.name));
                        }
                    }
                }
            }
            // The data_writer / data_reader children of a publisher or
            // subscriber are objects of their own (the session creates them
            // with the object ids of their names).
            tag if tag == endpoint_tag => out.embedded.push(child.clone()),
            "data_writer" | "data_reader" => {
                return Err(ReprError::Malformed(format!(
                    "a <{element}> holds <{endpoint_tag}> elements, not <{}>",
                    child.name
                )));
            }
            "qos" => {}
            other => {
                out.not_applied.push(other.to_string());
                continue;
            }
        }
        if child.name != "qos" {
            continue;
        }
        for policy in &child.children {
            match policy.name.as_str() {
                "partition" => out.partitions = partition_names(policy),
                "presentation" => presentation(policy, out),
                "entity_factory" => {
                    if let Some(v) = policy
                        .child("autoenable_created_entities")
                        .map(|v| v.text.trim())
                    {
                        match v {
                            "true" | "TRUE" | "1" => out.autoenable_created_entities = true,
                            "false" | "FALSE" | "0" => out.autoenable_created_entities = false,
                            other => out.not_applied.push(format!(
                                "qos/entity_factory/autoenable_created_entities={other}"
                            )),
                        }
                    }
                }
                other => out.not_applied.push(format!("qos/{other}")),
            }
        }
    }
    Ok(())
}

/// `<presentation>`: `access_scope` (`INSTANCE`, `TOPIC`, `GROUP`, also
/// written `…_PRESENTATION_QOS`), `coherent_access`, `ordered_access`.
/// What it cannot read is named, not applied silently.
fn presentation(el: &XmlElement, out: &mut ContainerDescription) {
    use zerodds_qos::PresentationAccessScope as Scope;
    let flag = |name: &str| el.child(name).map(|v| v.text.trim().to_string());
    if let Some(scope) = flag("access_scope") {
        match scope.trim_end_matches("_PRESENTATION_QOS") {
            "INSTANCE" => out.presentation.access_scope = Scope::Instance,
            "TOPIC" => out.presentation.access_scope = Scope::Topic,
            "GROUP" => out.presentation.access_scope = Scope::Group,
            other => out
                .not_applied
                .push(format!("qos/presentation/access_scope={other}")),
        }
    }
    for (name, field) in [
        ("coherent_access", &mut out.presentation.coherent_access),
        ("ordered_access", &mut out.presentation.ordered_access),
    ] {
        match flag(name).as_deref() {
            None => {}
            Some("true" | "TRUE" | "1") => *field = true,
            Some("false" | "FALSE" | "0") => *field = false,
            Some(other) => out
                .not_applied
                .push(format!("qos/presentation/{name}={other}")),
        }
    }
}

/// The types a `CREATE(OBJK_TYPE)` defines, by fully qualified name
/// (`Module::Name`, XTypes 1.3 §7.3.4.5.4). The binary form is one
/// `TypeObject` (XCDR2), which must be COMPLETE — a MINIMAL one carries
/// no name to match a topic's type name against. The XML form is a
/// DDS-XML type library (`<types>`, or this crate's §9.3 `<type>`
/// element around `<module>`/`<struct>`/…), compiled to
/// `CompleteTypeObject`s.
///
/// # Errors
/// [`ReprError::Unresolved`] for a `BY_REFERENCE` representation or a
/// MINIMAL `TypeObject`; [`ReprError::Malformed`] if it does not decode,
/// parse or compile, or defines no named type.
pub fn types(
    variant: &ObjectVariant,
    profiles: &AgentProfiles,
    known: &BTreeMap<String, CompleteTypeObject>,
) -> Result<Vec<(String, CompleteTypeObject)>, ReprError> {
    let named = match variant {
        ObjectVariant::ByReference(name) => {
            return Err(ReprError::Unresolved(format!(
                "type profile `{name}`: this agent resolves no BY_REFERENCE representation"
            )));
        }
        ObjectVariant::InBinary(bytes) => {
            let mut reader = zerodds_types::framing::xcdr2_reader(bytes);
            let object = TypeObject::decode_from(&mut reader)
                .map_err(|e| ReprError::Malformed(format!("TypeObject: {e:?}")))?;
            if reader.remaining() != 0 {
                return Err(ReprError::Malformed(format!(
                    "TypeObject: {} trailing bytes",
                    reader.remaining()
                )));
            }
            let TypeObject::Complete(root) = object else {
                return Err(ReprError::Unresolved(
                    "a MINIMAL TypeObject carries no type name to match a topic against".into(),
                ));
            };
            let name = complete_type_name(&root)
                .ok_or_else(|| ReprError::Malformed("TypeObject of an anonymous type".into()))?
                .to_string();
            vec![(name, root)]
        }
        ObjectVariant::ByXmlString(xml) => {
            let root = parse(xml)?
                .ok_or_else(|| ReprError::Malformed("empty type representation".into()))?;
            let libraries: Vec<&XmlElement> = if root.name == "types" || root.name == "type" {
                vec![&root]
            } else {
                root.children
                    .iter()
                    .filter(|c| c.name == "types" || c.name == "type")
                    .collect()
            };
            let mut out = Vec::new();
            for el in libraries {
                // The `<type>` wrapper of this crate's §9.3 form carries the
                // `object_id` of the XRCE object; it is no attribute of the
                // type library the DDS-XML reader checks.
                let mut wrapper = el.clone();
                wrapper.attributes.remove("object_id");
                let library = zerodds_xml::xtypes_parser::parse_types_element_with(
                    &wrapper,
                    &profiles.read_options(),
                )
                .map_err(|e| ReprError::Malformed(format!("type library: {e}")))?
                .value;
                // §7.7.3.3.2: the types may "reference other Types already
                // known to the Agent"; a name that is neither defined here
                // nor known is a reference to no type.
                let compiled = zerodds_xml::typeobject_bridge::bridge_library_complete_with_known(
                    &library, known,
                )
                .map_err(|e| match e {
                    zerodds_xml::typeobject_bridge::BridgeError::UnresolvedReference(name) => {
                        ReprError::Unresolved(format!(
                            "type library: `{name}` is neither defined in it nor a type known \
                             to the agent (DDS-XRCE 1.0 §7.7.3.3.2)"
                        ))
                    }
                    other => ReprError::Malformed(format!("type library: {other:?}")),
                })?;
                // §7.7.3.3.2: "Within the <types> element there may be
                // multiple types defined. In this case only one type shall
                // have the nested annotation set to false. This corresponds
                // to the XRCE Type being created."
                if el.name == "types" && compiled.len() > 1 {
                    let mut flags = Vec::new();
                    nested_flags(el, &mut Vec::new(), &mut flags);
                    let standing: Vec<&str> = flags
                        .iter()
                        .filter(|(name, nested)| {
                            !nested && compiled.iter().any(|(known, _)| known == name)
                        })
                        .map(|(name, _)| name.as_str())
                        .collect();
                    if standing.len() != 1 {
                        return Err(ReprError::Malformed(format!(
                            "{} types are defined and {} of them are not nested; exactly one is \
                             the XRCE Type, the others carry nested=\"true\" (§7.7.3.3.2): {}",
                            compiled.len(),
                            standing.len(),
                            standing.join(", ")
                        )));
                    }
                }
                out.extend(compiled);
            }
            out
        }
    };
    if named.is_empty() {
        return Err(ReprError::Malformed(
            "the type representation defines no type".into(),
        ));
    }
    Ok(named)
}

/// The type definitions of a `<types>` tree with their fully qualified names
/// and whether they carry `nested="true"`.
///
/// zerodds-lint: recursion-depth = `<module>` nesting of the tree (the tree
/// comes from `zerodds_xml::parser`, which refuses a nesting over
/// `MAX_TREE_DEPTH`).
fn nested_flags(el: &XmlElement, path: &mut Vec<String>, out: &mut Vec<(String, bool)>) {
    for child in &el.children {
        let name = child.attribute("name");
        match (child.name.as_str(), name) {
            ("module", Some(module)) => {
                path.push(module.to_string());
                nested_flags(child, path, out);
                path.pop();
            }
            ("struct" | "union" | "enum" | "bitmask" | "bitset" | "typedef", Some(name)) => {
                let mut qualified = path.join("::");
                if !qualified.is_empty() {
                    qualified.push_str("::");
                }
                qualified.push_str(name);
                out.push((qualified, child.attribute("nested") == Some("true")));
            }
            _ => {}
        }
    }
}

/// The name a named `CompleteTypeObject` carries; `None` for the
/// anonymous collection kinds.
fn complete_type_name(t: &CompleteTypeObject) -> Option<&str> {
    match t {
        CompleteTypeObject::Struct(s) => Some(&s.header.detail.type_name),
        CompleteTypeObject::Union(u) => Some(&u.header.detail.type_name),
        CompleteTypeObject::Enumerated(e) => Some(&e.header.detail.type_name),
        CompleteTypeObject::Alias(a) => Some(&a.header.detail.type_name),
        CompleteTypeObject::Bitmask(b) => Some(&b.detail.type_name),
        CompleteTypeObject::Bitset(b) => Some(&b.detail.type_name),
        _ => None,
    }
}

/// Parses `xml`; `None` for an empty or whitespace-only string.
fn parse(xml: &str) -> Result<Option<XmlElement>, ReprError> {
    let trimmed = xml.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    if trimmed.is_empty() {
        return Ok(None);
    }
    zerodds_xml::parse_xml_tree(trimmed)
        .map(|doc| Some(doc.root))
        .map_err(|e| ReprError::Malformed(format!("XML representation: {e}")))
}

/// `attr="…"` (attribute form) or `<child>…</child>` (element form).
fn attribute_or_child(el: &XmlElement, attr: &str, child: &str) -> Option<String> {
    el.attribute(attr)
        .map(ToString::to_string)
        .or_else(|| el.child(child).map(|c| c.text.trim().to_string()))
        .filter(|s| !s.is_empty())
}

/// The `qos_profile_ref` attribute is no DDS-XML 1.0 form (§7.3.5.4.4 writes
/// the profile as `base_name="library::profile"` of the QoS container); other
/// products write it. It is refused unless the agent's `vendor_forms:` setting
/// accepts it.
///
/// # Errors
/// [`ReprError::Malformed`] if `el` has the attribute and the agent does not
/// accept vendor forms.
fn vendor_qos_profile_ref(el: &XmlElement, profiles: &AgentProfiles) -> Result<(), ReprError> {
    if el.attribute("qos_profile_ref").is_some() && !profiles.vendor_forms() {
        return Err(ReprError::Malformed(format!(
            "<{}>: the qos_profile_ref attribute is not a DDS-XML 1.0 form; the specification \
             writes <qos container base_name=\"library::profile\"> (§7.3.5.4.4); the agent \
             setting `vendor_forms: accept` reads it",
            el.name
        )));
    }
    Ok(())
}

fn endpoint_from_xml(
    el: &XmlElement,
    role: EndpointRole,
    profiles: &AgentProfiles,
    out: &mut EndpointDescription,
) -> Result<(), ReprError> {
    // Attribute form. A named QoS profile comes first; what the element
    // itself says overrides it.
    if let Some(reference) = el.attribute("qos_profile") {
        let resolved = profiles
            .qos_profile(reference)
            .map_err(ReprError::Unresolved)?;
        apply_qos_profile(reference, &resolved, role, out)?;
    }
    vendor_qos_profile_ref(el, profiles)?;
    if let Some(reference) = el.attribute("qos_profile_ref") {
        let resolved = profiles
            .qos_profile(reference)
            .map_err(ReprError::Unresolved)?;
        apply_qos_profile(reference, &resolved, role, out)?;
    }
    if let Some(raw) = el.attribute("topic_ref") {
        // `0x…` is the ObjectId of the topic (this crate's own dialect and
        // the pre-planned objects of a configuration file); anything else
        // is the name of a topic of the DomainParticipant (DDS-XML
        // §7.3.5.4.6, §7.7.3.10.1: `topic_ref="Square"`).
        out.topic = Some(
            match raw
                .strip_prefix("0x")
                .or_else(|| raw.strip_prefix("0X"))
                .map(|hex| u16::from_str_radix(hex, 16))
            {
                Some(Ok(id)) => TopicRef::Object(ObjectId::from_raw(id)),
                Some(Err(_)) => {
                    return Err(ReprError::Malformed(format!(
                        "topic_ref `{raw}` is not a hex ObjectId"
                    )));
                }
                None => TopicRef::Name(raw.to_string()),
            },
        );
    }
    if let Some(kind) = el.attribute("reliability") {
        out.qos.reliable = reliability(kind)?;
    }
    // Element form.
    for child in &el.children {
        match child.name.as_str() {
            "topic" => topic_element(child, out)?,
            "qos" => {
                for policy in &child.children {
                    qos_policy(policy, role, out)?;
                }
            }
            // DDS-XML §7.3.5.4.6: `<datawriter_qos base_name="…">` states
            // QoS policies over a profile; the XRCE clauses spell it
            // `<data_writer_qos>` (§7.7.3.10.1).
            "datawriter_qos" | "data_writer_qos" if role == EndpointRole::Writer => {
                let reference = child.attribute("base_name").unwrap_or("(inline)");
                let resolved = profiles.qos_element(child).map_err(ReprError::Unresolved)?;
                apply_qos_profile(reference, &resolved, role, out)?;
            }
            "datareader_qos" | "data_reader_qos" if role == EndpointRole::Reader => {
                let reference = child.attribute("base_name").unwrap_or("(inline)");
                let resolved = profiles.qos_element(child).map_err(ReprError::Unresolved)?;
                apply_qos_profile(reference, &resolved, role, out)?;
            }
            // §7.7.3.11.1: "<content_filter name=…><expression>…</expression>".
            "content_filter" if role == EndpointRole::Reader => {
                let expression = child
                    .child("expression")
                    .map(|e| e.text.trim().to_string())
                    .filter(|e| !e.is_empty())
                    .ok_or_else(|| {
                        ReprError::Malformed("<content_filter> without an <expression>".into())
                    })?;
                out.qos.content_filter = Some(expression);
            }
            other => out.not_applied.push(other.to_string()),
        }
    }
    Ok(())
}

/// `<topic>` inside a Fast DDS endpoint profile: name, type, keyed-ness
/// and the HISTORY the endpoint uses (Fast DDS `TopicAttributes`).
fn topic_element(el: &XmlElement, out: &mut EndpointDescription) -> Result<(), ReprError> {
    for child in &el.children {
        let text = child.text.trim();
        match child.name.as_str() {
            "name" => out.topic = Some(TopicRef::Name(text.to_string())),
            "dataType" => out.type_name = Some(text.to_string()),
            "kind" => {
                out.keyed = Some(match text {
                    "WITH_KEY" => true,
                    "NO_KEY" => false,
                    _ => {
                        return Err(ReprError::Malformed(format!(
                            "topic kind `{text}` is neither WITH_KEY nor NO_KEY"
                        )));
                    }
                });
            }
            "historyQos" => out.qos.history = history(child)?,
            other => out.not_applied.push(format!("topic/{other}")),
        }
    }
    Ok(())
}

fn qos_policy(
    el: &XmlElement,
    role: EndpointRole,
    out: &mut EndpointDescription,
) -> Result<(), ReprError> {
    let kind = || el.child("kind").map(|k| k.text.trim()).unwrap_or("");
    match el.name.as_str() {
        "reliability" => {
            out.qos.reliable = reliability(kind())?;
            for child in &el.children {
                if child.name != "kind" {
                    out.not_applied
                        .push(format!("qos/reliability/{}", child.name));
                }
            }
        }
        "durability" => {
            out.qos.durability = match kind() {
                "VOLATILE" => Durability::Volatile,
                "TRANSIENT_LOCAL" => Durability::TransientLocal,
                "TRANSIENT" => Durability::Transient,
                "PERSISTENT" => Durability::Persistent,
                other => {
                    return Err(ReprError::Malformed(format!(
                        "durability kind `{other}` is not a DDS DURABILITY kind"
                    )));
                }
            };
        }
        "ownership" => {
            out.qos.exclusive_ownership = match kind() {
                "SHARED" => false,
                "EXCLUSIVE" => true,
                other => {
                    return Err(ReprError::Malformed(format!(
                        "ownership kind `{other}` is neither SHARED nor EXCLUSIVE"
                    )));
                }
            };
        }
        "ownershipStrength" if role == EndpointRole::Writer => {
            let value = el.child("value").map(|v| v.text.trim()).unwrap_or("");
            out.qos.ownership_strength = value.parse().map_err(|_| {
                ReprError::Malformed(format!("ownership strength `{value}` is not a long"))
            })?;
        }
        "deadline" => out.qos.deadline = duration(el.child("period"), "deadline")?,
        "lifespan" if role == EndpointRole::Writer => {
            out.qos.lifespan = duration(el.child("duration"), "lifespan")?;
        }
        "timeBasedFilter" if role == EndpointRole::Reader => {
            out.qos.time_based_filter =
                duration(el.child("minimum_separation"), "time based filter")?;
        }
        "partition" => out.qos.partitions = partition_names(el),
        other => out.not_applied.push(format!("qos/{other}")),
    }
    Ok(())
}

/// A DDS-XML QoS profile's DataWriter or DataReader QoS (and its
/// Publisher/Subscriber QoS for PARTITION and GROUP_DATA) onto `out`.
/// Policies the agent does not apply are named.
fn apply_qos_profile(
    reference: &str,
    resolved: &zerodds_xml::qos_inheritance::ResolvedQos,
    role: EndpointRole,
    out: &mut EndpointDescription,
) -> Result<(), ReprError> {
    // The agent resolves the profile without a topic name at this
    // point (the endpoint's topic is an object id, resolved later), so
    // §7.3.2.4.3's selection falls back: the filterless container,
    // else the first one in document order.
    let (endpoint, container) = match role {
        EndpointRole::Writer => (
            zerodds_xml::default_qos(&resolved.datawriter_qos),
            zerodds_xml::default_qos(&resolved.publisher_qos),
        ),
        EndpointRole::Reader => (
            zerodds_xml::default_qos(&resolved.datareader_qos),
            zerodds_xml::default_qos(&resolved.subscriber_qos),
        ),
    };
    let mut skipped: Vec<&str> = Vec::new();
    if let Some(q) = endpoint {
        let qos = &mut out.qos;
        if let Some(r) = &q.reliability {
            qos.reliable = r.kind == zerodds_qos::ReliabilityKind::Reliable;
            qos.max_blocking_time = std_duration(r.max_blocking_time);
        }
        if let Some(l) = &q.resource_limits {
            qos.resource_limits = *l;
        }
        if let Some(d) = &q.durability {
            qos.durability = match d.kind {
                zerodds_qos::DurabilityKind::Volatile => Durability::Volatile,
                zerodds_qos::DurabilityKind::TransientLocal => Durability::TransientLocal,
                zerodds_qos::DurabilityKind::Transient => Durability::Transient,
                zerodds_qos::DurabilityKind::Persistent => Durability::Persistent,
            };
        }
        if let Some(h) = &q.history {
            qos.history = match h.kind {
                zerodds_qos::HistoryKind::KeepAll => History::KeepAll,
                zerodds_qos::HistoryKind::KeepLast => {
                    History::KeepLast(u32::try_from(h.depth).ok().filter(|d| *d > 0).ok_or_else(
                        || {
                            ReprError::Malformed(format!(
                                "qos_profile `{reference}`: KEEP_LAST depth {}",
                                h.depth
                            ))
                        },
                    )?)
                }
            };
        }
        if let Some(o) = &q.ownership {
            qos.exclusive_ownership = o.kind == zerodds_qos::OwnershipKind::Exclusive;
        }
        if let Some(d) = &q.deadline {
            qos.deadline = std_duration(d.period);
        }
        if let Some(u) = &q.user_data {
            qos.user_data = u.value.clone();
        }
        if let Some(p) = &q.partition {
            qos.partitions = p.names.clone();
        }
        // The policies the runtime takes on the endpoint (DDS 1.4 §2.2.3).
        if let Some(l) = &q.liveliness {
            qos.liveliness = *l;
        }
        if let Some(l) = &q.latency_budget {
            qos.latency_budget = *l;
        }
        if let Some(d) = &q.destination_order {
            qos.destination_order = *d;
        }
        if let Some(t) = &q.topic_data {
            qos.topic_data.clone_from(&t.value);
        }
        // A policy that has no meaning for this kind of endpoint (a
        // TIME_BASED_FILTER of a DataWriter, an OWNERSHIP_STRENGTH,
        // LIFESPAN, TRANSPORT_PRIORITY or DURABILITY_SERVICE of a DataReader,
        // PRESENTATION, ENTITY_FACTORY and GROUP_DATA of an endpoint) is not
        // part of the endpoint's QoS in DDS-XML either.
        match role {
            EndpointRole::Writer => {
                if let Some(s) = &q.ownership_strength {
                    qos.ownership_strength = s.value;
                }
                if let Some(l) = &q.lifespan {
                    qos.lifespan = std_duration(l.duration);
                }
                if let Some(t) = &q.transport_priority {
                    qos.transport_priority = t.value;
                }
                if let Some(d) = &q.durability_service {
                    qos.durability_service = *d;
                }
                // WRITER_DATA_LIFECYCLE: the runtime unregisters (and, by the
                // default, disposes) on delete for all writers alike; one
                // writer cannot say otherwise.
                if let Some(w) = &q.writer_data_lifecycle {
                    if !w.autodispose_unregistered_instances {
                        skipped.push(
                            "writer_data_lifecycle (autodispose_unregistered_instances=false)",
                        );
                    }
                }
            }
            EndpointRole::Reader => {
                if let Some(t) = &q.time_based_filter {
                    qos.time_based_filter =
                        std_duration(t.minimum_separation).filter(|d| !d.is_zero());
                }
                // READER_DATA_LIFECYCLE: a DataReader of the runtime keeps no
                // instance after its writers are gone for a delay.
                if let Some(r) = &q.reader_data_lifecycle {
                    if !r.autopurge_nowriter_samples_delay.is_infinite()
                        || !r.autopurge_disposed_samples_delay.is_infinite()
                    {
                        skipped.push("reader_data_lifecycle (autopurge delay)");
                    }
                }
            }
        }
    }
    if let Some(c) = container {
        if let Some(p) = &c.partition {
            out.qos.partitions = p.names.clone();
        }
        if let Some(g) = &c.group_data {
            out.qos.group_data = g.value.clone();
        }
    }
    out.not_applied.extend(
        skipped
            .into_iter()
            .map(|p| format!("qos_profile `{reference}`/{p}")),
    );
    Ok(())
}

/// A QoS duration as a `core::time::Duration`; `None` for INFINITE.
fn std_duration(d: zerodds_qos::Duration) -> Option<Duration> {
    if d.is_infinite() {
        return None;
    }
    Some(Duration::from_nanos(
        u64::try_from(d.to_nanos()).unwrap_or(u64::MAX),
    ))
}

fn reliability(kind: &str) -> Result<bool, ReprError> {
    match kind {
        "RELIABLE" => Ok(true),
        "BEST_EFFORT" => Ok(false),
        other => Err(ReprError::Malformed(format!(
            "reliability kind `{other}` is neither RELIABLE nor BEST_EFFORT"
        ))),
    }
}

fn history(el: &XmlElement) -> Result<History, ReprError> {
    let depth = match el.child("depth") {
        Some(d) => d.text.trim().parse::<u32>().map_err(|_| {
            ReprError::Malformed(format!("history depth `{}` is not a number", d.text))
        })?,
        None => 1,
    };
    match el
        .child("kind")
        .map(|k| k.text.trim())
        .unwrap_or("KEEP_LAST")
    {
        "KEEP_LAST" if depth == 0 => Err(ReprError::Malformed(
            "KEEP_LAST history with depth 0 (DDS 1.4 §2.2.3.18 needs a positive depth)".into(),
        )),
        "KEEP_LAST" => Ok(History::KeepLast(depth)),
        "KEEP_ALL" => Ok(History::KeepAll),
        other => Err(ReprError::Malformed(format!(
            "history kind `{other}` is neither KEEP_LAST nor KEEP_ALL"
        ))),
    }
}

/// A Fast DDS duration: `<sec>`/`<nanosec>` children, or the text
/// `DURATION_INFINITY` (no bound, `None`).
fn duration(el: Option<&XmlElement>, what: &str) -> Result<Option<Duration>, ReprError> {
    let Some(el) = el else {
        return Err(ReprError::Malformed(format!("{what} without a duration")));
    };
    let field = |name: &str| el.child(name).map(|c| c.text.trim().to_string());
    let (sec, nanosec) = (field("sec"), field("nanosec"));
    if sec.is_none() && nanosec.is_none() {
        return match el.text.trim() {
            "DURATION_INFINITY" | "" => Ok(None),
            other => Err(ReprError::Malformed(format!(
                "{what} `{other}` is not a duration"
            ))),
        };
    }
    let parse = |v: Option<String>| -> Result<u64, ReprError> {
        match v.as_deref() {
            None => Ok(0),
            Some("DURATION_INFINITY") => Ok(u64::MAX),
            Some(s) => s
                .parse()
                .map_err(|_| ReprError::Malformed(format!("{what}: `{s}` is not a number"))),
        }
    };
    let (sec, nanosec) = (parse(sec)?, parse(nanosec)?);
    if sec == u64::MAX || nanosec == u64::MAX {
        return Ok(None);
    }
    let nanosec = u32::try_from(nanosec)
        .ok()
        .filter(|n| *n < 1_000_000_000)
        .ok_or_else(|| ReprError::Malformed(format!("{what}: nanosec out of range")))?;
    Ok(Some(Duration::new(sec, nanosec)))
}

fn partition_names(el: &XmlElement) -> Vec<String> {
    el.child("names")
        .map(|names| {
            names
                .children_named("name")
                .map(|n| n.text.trim().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// `EndpointQosFlags` of §7.7.3.10.2 and Annex A. Bit 1 is
/// `is_history_keep_all` in Annex A and `is_history_keep_last` in the clause
/// (and for the eProsima client).
mod spec_flags {
    pub const RELIABLE: u16 = 1 << 0;
    pub const BIT1: u16 = 1 << 1;
    pub const OWNERSHIP_EXCLUSIVE: u16 = 1 << 2;
    pub const DURABILITY_TRANSIENT_LOCAL: u16 = 1 << 3;
    pub const DURABILITY_TRANSIENT: u16 = 1 << 4;
    pub const DURABILITY_PERSISTENT: u16 = 1 << 5;
    pub const KNOWN: u16 = (1 << 6) - 1;
}

/// Applies the `OBJK_Endpoint_Binary_Qos` of an XCDR2 blob (and the members
/// the role adds) to `qos`.
fn apply_endpoint_binary(
    b: &binary_forms::EndpointQosBinary,
    bit1_is_keep_all: bool,
    role: EndpointRole,
    qos: &mut EndpointQos,
) -> Result<(), ReprError> {
    use spec_flags as f;
    if b.qos_flags & !f::KNOWN != 0 {
        return Err(ReprError::Malformed(format!(
            "endpoint qos_flags {:#06x} carry bits EndpointQosFlags does not define",
            b.qos_flags
        )));
    }
    qos.reliable = b.qos_flags & f::RELIABLE != 0;
    qos.exclusive_ownership = b.qos_flags & f::OWNERSHIP_EXCLUSIVE != 0;
    qos.durability = match b.qos_flags
        & (f::DURABILITY_TRANSIENT_LOCAL | f::DURABILITY_TRANSIENT | f::DURABILITY_PERSISTENT)
    {
        0 => Durability::Volatile,
        f::DURABILITY_TRANSIENT_LOCAL => Durability::TransientLocal,
        f::DURABILITY_TRANSIENT => Durability::Transient,
        f::DURABILITY_PERSISTENT => Durability::Persistent,
        _ => {
            return Err(ReprError::Malformed(format!(
                "endpoint qos_flags {:#06x} set more than one durability kind",
                b.qos_flags
            )));
        }
    };
    let bit1 = b.qos_flags & f::BIT1 != 0;
    let keep_all = if bit1_is_keep_all { bit1 } else { !bit1 };
    qos.history = if keep_all {
        History::KeepAll
    } else {
        match b.history_depth {
            Some(0) => {
                return Err(ReprError::Malformed(
                    "KEEP_LAST history with depth 0".into(),
                ));
            }
            Some(d) => History::KeepLast(u32::from(d)),
            None => History::KeepLast(1),
        }
    };
    qos.deadline = b
        .deadline_msec
        .map(|ms| Duration::from_millis(u64::from(ms)));
    if let Some(user_data) = &b.user_data {
        qos.user_data.clone_from(user_data);
    }
    match role {
        EndpointRole::Writer => {
            qos.lifespan = b
                .lifespan_msec
                .map(|ms| Duration::from_millis(u64::from(ms)));
            if let Some(strength) = b.ownership_strength {
                qos.ownership_strength = i32::try_from(strength).map_err(|_| {
                    ReprError::Malformed(format!(
                        "ownership strength {strength} does not fit a DDS long"
                    ))
                })?;
            }
        }
        EndpointRole::Reader => {
            if b.lifespan_msec.is_some() {
                return Err(ReprError::Malformed(
                    "a DataReader carries no LIFESPAN (DDS 1.4 §2.2.3.16)".into(),
                ));
            }
            qos.time_based_filter = b
                .timebasedfilter_msec
                .map(|ms| Duration::from_millis(u64::from(ms)));
            qos.content_filter.clone_from(&b.contentbased_filter);
        }
    }
    Ok(())
}

/// eProsima `EndpointQosFlags` (`xrce_types.h`).
mod flags {
    pub const RELIABLE: u16 = 1 << 0;
    pub const HISTORY_KEEP_LAST: u16 = 1 << 1;
    pub const OWNERSHIP_EXCLUSIVE: u16 = 1 << 2;
    pub const DURABILITY_TRANSIENT_LOCAL: u16 = 1 << 3;
    pub const DURABILITY_TRANSIENT: u16 = 1 << 4;
    pub const DURABILITY_PERSISTENT: u16 = 1 << 5;
    pub const KNOWN: u16 = (1 << 6) - 1;
}

/// `OBJK_DataWriter_Binary_Qos` / `OBJK_DataReader_Binary_Qos`: the
/// shared `OBJK_Endpoint_QosBinary` base, then the role's own fields.
fn endpoint_qos_from_binary(
    r: &mut Cdr<'_>,
    role: EndpointRole,
    qos: &mut EndpointQos,
) -> Result<(), ReprError> {
    let f = r.u16()?;
    if f & !flags::KNOWN != 0 {
        return Err(ReprError::Malformed(format!(
            "endpoint qos_flags {f:#06x} carry bits EndpointQosFlags does not define"
        )));
    }
    qos.reliable = f & flags::RELIABLE != 0;
    qos.exclusive_ownership = f & flags::OWNERSHIP_EXCLUSIVE != 0;
    let durability = f
        & (flags::DURABILITY_TRANSIENT_LOCAL
            | flags::DURABILITY_TRANSIENT
            | flags::DURABILITY_PERSISTENT);
    qos.durability = match durability {
        0 => Durability::Volatile,
        flags::DURABILITY_TRANSIENT_LOCAL => Durability::TransientLocal,
        flags::DURABILITY_TRANSIENT => Durability::Transient,
        flags::DURABILITY_PERSISTENT => Durability::Persistent,
        _ => {
            return Err(ReprError::Malformed(format!(
                "endpoint qos_flags {f:#06x} set more than one durability kind"
            )));
        }
    };
    let depth = r.optional(Cdr::u16)?;
    qos.history = if f & flags::HISTORY_KEEP_LAST != 0 {
        match depth {
            Some(0) => {
                return Err(ReprError::Malformed(
                    "KEEP_LAST history with depth 0".into(),
                ));
            }
            Some(d) => History::KeepLast(u32::from(d)),
            None => History::KeepLast(1),
        }
    } else {
        History::KeepAll
    };
    qos.deadline = r
        .optional(Cdr::u32)?
        .map(|ms| Duration::from_millis(u64::from(ms)));
    let lifespan = r
        .optional(Cdr::u32)?
        .map(|ms| Duration::from_millis(u64::from(ms)));
    if let Some(user_data) = r.optional(Cdr::octets)? {
        qos.user_data = user_data;
    }
    match role {
        EndpointRole::Writer => {
            qos.lifespan = lifespan;
            if let Some(strength) = r.optional(Cdr::u64)? {
                qos.ownership_strength = i32::try_from(strength).map_err(|_| {
                    ReprError::Malformed(format!(
                        "ownership strength {strength} does not fit a DDS long"
                    ))
                })?;
            }
        }
        EndpointRole::Reader => {
            if lifespan.is_some() {
                return Err(ReprError::Malformed(
                    "a DataReader carries no LIFESPAN (DDS 1.4 §2.2.3.16)".into(),
                ));
            }
            qos.time_based_filter = r.optional(Cdr::u64)?.map(Duration::from_millis);
            qos.content_filter = r.optional(Cdr::string)?;
        }
    }
    Ok(())
}

/// A classic-CDR reader over one `IN_BINARY` blob: natural alignment
/// (up to 8) measured from the blob's first byte, as Micro CDR's
/// `ucdr_init_buffer` sets it up.
struct Cdr<'a> {
    buf: &'a [u8],
    pos: usize,
    big_endian: bool,
}

impl<'a> Cdr<'a> {
    fn new(buf: &'a [u8], big_endian: bool) -> Self {
        Self {
            buf,
            pos: 0,
            big_endian,
        }
    }

    fn take(&mut self, n: usize, align: usize) -> Result<&'a [u8], ReprError> {
        let start = self.pos.next_multiple_of(align);
        let end = start
            .checked_add(n)
            .filter(|e| *e <= self.buf.len())
            .ok_or_else(|| {
                ReprError::Malformed(format!(
                    "binary representation ends at {} of {} bytes needed",
                    self.buf.len(),
                    start.saturating_add(n)
                ))
            })?;
        self.pos = end;
        Ok(&self.buf[start..end])
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ReprError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N, N)?);
        if self.big_endian {
            out.reverse();
        }
        Ok(out)
    }

    fn bool(&mut self) -> Result<bool, ReprError> {
        match self.take(1, 1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            b => Err(ReprError::Malformed(format!("boolean octet {b:#04x}"))),
        }
    }

    fn u16(&mut self) -> Result<u16, ReprError> {
        self.array::<2>().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, ReprError> {
        self.array::<4>().map(u32::from_le_bytes)
    }

    fn u64(&mut self) -> Result<u64, ReprError> {
        self.array::<8>().map(u64::from_le_bytes)
    }

    fn length(&mut self) -> Result<usize, ReprError> {
        usize::try_from(self.u32()?).map_err(|_| ReprError::Malformed("length overflow".into()))
    }

    fn octets(&mut self) -> Result<Vec<u8>, ReprError> {
        let n = self.length()?;
        Ok(self.take(n, 1)?.to_vec())
    }

    fn string(&mut self) -> Result<String, ReprError> {
        let n = self.length()?;
        let bytes = self.take(n, 1)?;
        let Some((&0, text)) = bytes.split_last() else {
            return Err(ReprError::Malformed(
                "string without its NUL terminator".into(),
            ));
        };
        core::str::from_utf8(text)
            .map(ToString::to_string)
            .map_err(|_| ReprError::Malformed("string is not UTF-8".into()))
    }

    fn string_seq(&mut self) -> Result<Vec<String>, ReprError> {
        let n = self.length()?;
        if n > self.buf.len() {
            return Err(ReprError::Malformed(format!(
                "string sequence of {n} in a {}-byte blob",
                self.buf.len()
            )));
        }
        (0..n).map(|_| self.string()).collect()
    }

    fn object_id(&mut self) -> Result<ObjectId, ReprError> {
        let raw = self.take(2, 1)?;
        ObjectId::from_bytes(raw).map_err(|e| ReprError::Malformed(format!("ObjectId: {e:?}")))
    }

    /// An `@optional` member: a boolean, then the value if it is true.
    fn optional<T>(
        &mut self,
        read: fn(&mut Self) -> Result<T, ReprError>,
    ) -> Result<Option<T>, ReprError> {
        if self.bool()? {
            read(self).map(Some)
        } else {
            Ok(None)
        }
    }

    fn finish(&self, what: &str) -> Result<(), ReprError> {
        if self.pos == self.buf.len() {
            Ok(())
        } else {
            Err(ReprError::Malformed(format!(
                "{what}: {} trailing bytes after {}",
                self.buf.len() - self.pos,
                self.pos
            )))
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_xrce::encoding::Endianness;

    // The inline representations need no profiles; these shadow the
    // module's functions with an empty profile store.
    fn topic(v: &ObjectVariant, be: bool) -> Result<TopicDescription, ReprError> {
        super::topic(v, be, &AgentProfiles::default(), None)
    }
    fn endpoint(
        v: &ObjectVariant,
        role: EndpointRole,
        be: bool,
    ) -> Result<EndpointDescription, ReprError> {
        super::endpoint(v, role, be, &AgentProfiles::default())
    }
    fn container(v: &ObjectVariant, be: bool) -> Result<ContainerDescription, ReprError> {
        super::container(v, "publisher", be, &AgentProfiles::default())
    }
    fn participant_domain(v: &ObjectVariant, wire: Option<i16>) -> Result<i32, ReprError> {
        super::participant(v, wire, false, &AgentProfiles::default()).map(|p| p.domain_id)
    }

    /// One `CREATE` body of the `BinaryEntityCreation` datagram
    /// (`tools/wire-capture/xrce-sessions/client-to-agent.hex`, line 360),
    /// from the `ObjectVariant`'s kind octet on; little-endian (flags 0x05).
    fn captured(hex: &str) -> ObjectVariant {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let (_, variant, _, _) = ObjectVariant::decode_for_kind(&bytes, Endianness::Little)
            .expect("the captured bytes decode");
        variant
    }

    #[test]
    fn the_captured_binary_topic_names_its_topic_and_type() {
        let v = captured(
            "02030000240000000d0000004578616d706c65546f706963000001000c000000\
             4578616d706c6554797065000011",
        );
        assert_eq!(
            topic(&v, false).unwrap(),
            TopicDescription {
                topic_name: "ExampleTopic".into(),
                type_name: "ExampleType".into(),
                type_identifier: None,
                definition: None,
                qos: TopicQos::default(),
                not_applied: Vec::new(),
            }
        );
    }

    #[test]
    fn the_captured_binary_writer_is_reliable_keep_last_transient_local() {
        let v = captured("050300000b000000001201000b0000000000000013");
        let d = endpoint(&v, EndpointRole::Writer, false).unwrap();
        assert_eq!(d.topic, Some(TopicRef::Object(ObjectId::from_raw(0x0012))));
        assert!(d.qos.reliable);
        assert_eq!(d.qos.history, History::KeepLast(1));
        assert_eq!(d.qos.durability, Durability::TransientLocal);
        assert!(!d.qos.exclusive_ownership);
        assert!(d.not_applied.is_empty());
    }

    #[test]
    fn the_captured_binary_reader_is_reliable_keep_last_transient_local() {
        let v = captured("060300000c000000001201000b000000000000000014");
        let d = endpoint(&v, EndpointRole::Reader, false).unwrap();
        assert_eq!(d.topic, Some(TopicRef::Object(ObjectId::from_raw(0x0012))));
        assert!(d.qos.reliable);
        assert_eq!(d.qos.history, History::KeepLast(1));
        assert_eq!(d.qos.durability, Durability::TransientLocal);
        assert_eq!(d.qos.time_based_filter, None);
        assert_eq!(d.qos.content_filter, None);
    }

    #[test]
    fn the_captured_binary_publisher_has_default_qos() {
        let v = captured("030300000200000000000011");
        assert_eq!(
            container(&v, false).unwrap(),
            ContainerDescription::default()
        );
    }

    #[test]
    fn a_binary_writer_with_every_optional_member_reads_back() {
        // topic_id, qos present, flags reliable|keep_last|exclusive|persistent,
        // depth 7, deadline 250 ms, lifespan 1000 ms, user_data [1,2,3],
        // ownership strength 9 (u64, aligned to 8).
        let mut b = vec![0x00, 0x12, 0x01, 0x00];
        b.extend_from_slice(&0x0027u16.to_le_bytes());
        b.push(1);
        b.push(0); // pad to 2
        b.extend_from_slice(&7u16.to_le_bytes());
        b.push(1);
        b.push(0); // pad to 4
        b.extend_from_slice(&250u32.to_le_bytes());
        b.push(1);
        b.extend_from_slice(&[0, 0, 0]);
        b.extend_from_slice(&1000u32.to_le_bytes());
        b.push(1);
        b.extend_from_slice(&[0, 0, 0]);
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&[1, 2, 3]);
        b.push(1);
        while b.len() % 8 != 0 {
            b.push(0);
        }
        b.extend_from_slice(&9u64.to_le_bytes());
        let d = endpoint(&ObjectVariant::InBinary(b), EndpointRole::Writer, false).unwrap();
        assert!(d.qos.reliable);
        assert!(d.qos.exclusive_ownership);
        assert_eq!(d.qos.durability, Durability::Persistent);
        assert_eq!(d.qos.history, History::KeepLast(7));
        assert_eq!(d.qos.deadline, Some(Duration::from_millis(250)));
        assert_eq!(d.qos.lifespan, Some(Duration::from_millis(1000)));
        assert_eq!(d.qos.user_data, vec![1, 2, 3]);
        assert_eq!(d.qos.ownership_strength, 9);
    }

    #[test]
    fn a_binary_endpoint_without_qos_gets_the_role_defaults() {
        let w = endpoint(
            &ObjectVariant::InBinary(vec![0x00, 0x12, 0x00]),
            EndpointRole::Writer,
            false,
        )
        .unwrap();
        assert_eq!(w.qos, EndpointQos::writer_default());
        let r = endpoint(
            &ObjectVariant::InBinary(vec![0x00, 0x12, 0x00]),
            EndpointRole::Reader,
            false,
        )
        .unwrap();
        assert_eq!(r.qos, EndpointQos::reader_default());
        assert!(
            !r.qos.reliable,
            "DDS 1.4 §2.2.3.14: a DataReader defaults to BEST_EFFORT"
        );
    }

    #[test]
    fn a_truncated_or_padded_binary_is_refused() {
        let short = ObjectVariant::InBinary(vec![0x00, 0x12, 0x01, 0x00, 0x01]);
        assert!(matches!(
            endpoint(&short, EndpointRole::Writer, false),
            Err(ReprError::Malformed(_))
        ));
        let long = ObjectVariant::InBinary(vec![0x00, 0x12, 0x00, 0x00]);
        assert!(matches!(
            endpoint(&long, EndpointRole::Writer, false),
            Err(ReprError::Malformed(_))
        ));
        let two_durabilities = {
            let mut b = vec![0x00, 0x12, 0x01, 0x00];
            b.extend_from_slice(&0x0018u16.to_le_bytes());
            b.extend_from_slice(&[0, 0, 0, 0, 0]);
            ObjectVariant::InBinary(b)
        };
        assert!(matches!(
            endpoint(&two_durabilities, EndpointRole::Writer, false),
            Err(ReprError::Malformed(_))
        ));
    }

    #[test]
    fn a_big_endian_binary_topic_reads_back() {
        let mut b = Vec::new();
        b.extend_from_slice(&2u32.to_be_bytes());
        b.extend_from_slice(b"T\0");
        b.push(0);
        b.push(1);
        b.extend_from_slice(&3u32.to_be_bytes());
        b.extend_from_slice(b"Ty\0");
        let t = topic(&ObjectVariant::InBinary(b), true).unwrap();
        assert_eq!(t.topic_name, "T");
        assert_eq!(t.type_name, "Ty");
    }

    /// The strings the eProsima `PublishHelloWorld`/`SubscribeHelloWorld`
    /// examples send, verbatim.
    #[test]
    fn the_eprosima_element_form_names_topic_type_and_kind() {
        let t = topic(
            &ObjectVariant::ByXmlString(
                "<dds><topic><name>HelloWorldTopic</name><dataType>HelloWorld</dataType>\
                 </topic></dds>"
                    .into(),
            ),
            false,
        )
        .unwrap();
        assert_eq!(t.topic_name, "HelloWorldTopic");
        assert_eq!(t.type_name, "HelloWorld");

        let w = endpoint(
            &ObjectVariant::ByXmlString(
                "<dds><data_writer><topic><kind>NO_KEY</kind><name>HelloWorldTopic</name>\
                 <dataType>HelloWorld</dataType></topic></data_writer></dds>"
                    .into(),
            ),
            EndpointRole::Writer,
            false,
        )
        .unwrap();
        assert_eq!(w.topic, Some(TopicRef::Name("HelloWorldTopic".into())));
        assert_eq!(w.type_name.as_deref(), Some("HelloWorld"));
        assert_eq!(w.keyed, Some(false));
        assert_eq!(w.qos, EndpointQos::writer_default());
        assert!(w.not_applied.is_empty());
    }

    #[test]
    fn the_element_form_qos_is_applied_and_the_rest_is_named() {
        let r = endpoint(
            &ObjectVariant::ByXmlString(
                "<dds><data_reader><topic><kind>WITH_KEY</kind><name>T</name>\
                 <dataType>Ty</dataType><historyQos><kind>KEEP_LAST</kind><depth>5</depth>\
                 </historyQos></topic><qos><reliability><kind>RELIABLE</kind></reliability>\
                 <durability><kind>TRANSIENT_LOCAL</kind></durability>\
                 <deadline><period><sec>1</sec><nanosec>500000000</nanosec></period></deadline>\
                 <timeBasedFilter><minimum_separation><sec>0</sec><nanosec>100000000</nanosec>\
                 </minimum_separation></timeBasedFilter>\
                 <partition><names><name>a</name><name>b</name></names></partition>\
                 <dataSharing><kind>AUTOMATIC</kind></dataSharing></qos></data_reader></dds>"
                    .into(),
            ),
            EndpointRole::Reader,
            false,
        )
        .unwrap();
        assert_eq!(r.keyed, Some(true));
        assert!(r.qos.reliable);
        assert_eq!(r.qos.history, History::KeepLast(5));
        assert_eq!(r.qos.durability, Durability::TransientLocal);
        assert_eq!(r.qos.deadline, Some(Duration::new(1, 500_000_000)));
        assert_eq!(r.qos.time_based_filter, Some(Duration::from_millis(100)));
        assert_eq!(r.qos.partitions, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(r.not_applied, vec!["qos/dataSharing".to_string()]);
    }

    #[test]
    fn the_attribute_form_still_reads() {
        let t = topic(
            &ObjectVariant::ByXmlString("<dds><topic name=\"T\" type_name=\"Ty\"/></dds>".into()),
            false,
        )
        .unwrap();
        assert_eq!(t.topic_name, "T");
        let w = endpoint(
            &ObjectVariant::ByXmlString(
                "<data_writer topic_ref=\"0x0102\" reliability=\"BEST_EFFORT\"/>".into(),
            ),
            EndpointRole::Writer,
            false,
        )
        .unwrap();
        assert_eq!(w.topic, Some(TopicRef::Object(ObjectId::from_raw(0x0102))));
        assert!(!w.qos.reliable);
    }

    #[test]
    fn a_topic_without_a_name_or_a_reference_topic_is_refused() {
        assert!(matches!(
            topic(
                &ObjectVariant::ByXmlString("<dds><topic/></dds>".into()),
                false
            ),
            Err(ReprError::Unresolved(_))
        ));
        assert!(matches!(
            topic(&ObjectVariant::ByReference("profile".into()), false),
            Err(ReprError::Unresolved(_))
        ));
        assert!(matches!(
            topic(&ObjectVariant::ByXmlString("<dds><topic>".into()), false),
            Err(ReprError::Malformed(_))
        ));
    }

    #[test]
    fn the_participant_domain_comes_from_the_wire_unless_the_xml_names_one() {
        let eprosima = ObjectVariant::ByXmlString(
            "<dds><participant><rtps><name>default_xrce_participant</name></rtps>\
             </participant></dds>"
                .into(),
        );
        assert_eq!(participant_domain(&eprosima, Some(5)).unwrap(), 5);
        let own = ObjectVariant::ByXmlString(
            "<dds><participant><domain_id>42</domain_id></participant></dds>".into(),
        );
        assert_eq!(participant_domain(&own, Some(0)).unwrap(), 42);
        let fast = ObjectVariant::ByXmlString(
            "<dds><participant><domainId>9</domainId></participant></dds>".into(),
        );
        assert_eq!(participant_domain(&fast, Some(0)).unwrap(), 9);
        assert_eq!(
            participant_domain(&ObjectVariant::InBinary(vec![0, 0]), Some(3)).unwrap(),
            3
        );
    }

    #[test]
    fn a_binary_complete_type_object_is_named_by_its_own_type_name() {
        let ast = std::sync::Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "module M { @final struct P { @key long id; }; };",
                "entity-repr-types.idl",
            )
            .unwrap(),
        );
        let ty = zerodds_types::dynamic::DynamicType::find(&ast, "M::P").unwrap();
        let root = zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap();
        let bytes = TypeObject::Complete(root.clone()).to_bytes_le().unwrap();
        let named = types(
            &ObjectVariant::InBinary(bytes),
            &AgentProfiles::default(),
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(named, vec![("M::P".to_string(), root)]);

        let minimal = TypeObject::Minimal(
            zerodds_types::from_ast::minimal_type_object(&ast, ty.id()).unwrap(),
        )
        .to_bytes_le()
        .unwrap();
        assert!(matches!(
            types(
                &ObjectVariant::InBinary(minimal),
                &AgentProfiles::default(),
                &BTreeMap::new(),
            ),
            Err(ReprError::Unresolved(_))
        ));
    }

    /// The `<type>` element `crates/xrce/src/xml_config.rs` sends for an
    /// `OBJK_TYPE`.
    #[test]
    fn an_xml_type_library_compiles_to_named_complete_types() {
        let named = types(
            &ObjectVariant::ByXmlString(
                "<type object_id=\"0x000A\"><module name=\"ShapesDemoTypes\">\
                 <struct name=\"ShapeType\"><member name=\"color\" type=\"string\" key=\"true\"/>\
                 <member name=\"x\" type=\"int32\"/></struct></module></type>"
                    .into(),
            ),
            &AgentProfiles::default(),
            &BTreeMap::new(),
        )
        .unwrap();
        let names: Vec<&str> = named.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["ShapesDemoTypes::ShapeType"]);
    }

    /// DDS-XRCE 1.0 §7.7.3.3.2: the types of a `<types>` string may
    /// "reference other Types already known to the Agent". A reference to a
    /// known type is resolved; one to no type is a reference to nothing.
    #[test]
    fn a_types_string_may_reference_types_the_agent_knows() {
        let profiles = AgentProfiles::default();
        let base = types(
            &ObjectVariant::ByXmlString(
                "<types><struct name=\"Base\"><member name=\"x\" type=\"int32\"/></struct></types>"
                    .into(),
            ),
            &profiles,
            &BTreeMap::new(),
        )
        .unwrap();
        let known: BTreeMap<String, CompleteTypeObject> = base.into_iter().collect();
        let outer = ObjectVariant::ByXmlString(
            "<types><struct name=\"Outer\"><member name=\"b\" type=\"nonBasic\" \
             nonBasicTypeName=\"Base\"/></struct></types>"
                .into(),
        );
        let named = types(&outer, &profiles, &known).unwrap();
        let names: Vec<&str> = named.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["Outer"], "the known type is not defined again");
        assert!(
            matches!(
                types(&outer, &profiles, &BTreeMap::new()),
                Err(ReprError::Unresolved(m)) if m.contains("Base")
            ),
            "a reference to a type nobody knows"
        );
    }

    /// A form other DDS products write (here the IDL name `long` as a type) is
    /// refused with the form of the specification, and read under
    /// `vendor_forms: accept`.
    #[test]
    fn vendor_forms_of_a_types_string_are_a_setting() {
        let xml = ObjectVariant::ByXmlString(
            "<types><struct name=\"V\"><member name=\"x\" type=\"long\"/></struct></types>".into(),
        );
        let refused = types(&xml, &AgentProfiles::default(), &BTreeMap::new());
        assert!(
            matches!(&refused, Err(ReprError::Malformed(m)) if m.contains("int32")),
            "{refused:?}"
        );
        let accepted = types(
            &xml,
            &AgentProfiles::default().with_vendor_forms(true),
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(accepted[0].0, "V");
    }

    /// The `qos_profile_ref` attribute is no DDS-XML 1.0 form; it is refused
    /// unless `vendor_forms: accept`, and the `qos_profile` attribute of the
    /// XRCE representation is not affected.
    #[test]
    fn the_qos_profile_ref_attribute_is_a_vendor_form() {
        let xml = |attr: &str| {
            ObjectVariant::ByXmlString(format!(
                "<data_writer topic_ref=\"0x0102\" {attr}=\"Lib::P\"/>"
            ))
        };
        let load = |accept: bool| {
            let mut profiles = AgentProfiles::default().with_vendor_forms(accept);
            profiles
                .add_profiles(
                    "<dds><qos_library name=\"Lib\"><qos_profile name=\"P\"><datawriter_qos>\
                     <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>\
                     </datawriter_qos></qos_profile></qos_library></dds>",
                )
                .unwrap();
            profiles
        };
        let run = |variant: &ObjectVariant, profiles: &AgentProfiles| {
            super::endpoint(variant, EndpointRole::Writer, false, profiles)
        };
        let refused = run(&xml("qos_profile_ref"), &load(false));
        assert!(
            matches!(&refused, Err(ReprError::Malformed(m)) if m.contains("base_name")),
            "{refused:?}"
        );
        let w = run(&xml("qos_profile_ref"), &load(true)).unwrap();
        assert_eq!(w.qos.durability, Durability::TransientLocal);
        let w = run(&xml("qos_profile"), &load(false)).unwrap();
        assert_eq!(w.qos.durability, Durability::TransientLocal);
    }

    /// `<domainparticipant_qos>` is a vendor spelling of
    /// `<domain_participant_qos>` (DDS-XML 1.0 §7.3.2.4).
    #[test]
    fn the_domainparticipant_qos_spelling_is_a_vendor_form() {
        let doc = "<dds><qos_library name=\"Q\"><qos_profile name=\"P\"><domainparticipant_qos>\
                   <user_data><value>AQID</value></user_data></domainparticipant_qos>\
                   </qos_profile></qos_library></dds>";
        let mut refused = AgentProfiles::default();
        let e = refused.add_profiles(doc).unwrap_err();
        assert!(e.contains("domain_participant_qos"), "{e}");
        let mut accepted = AgentProfiles::default().with_vendor_forms(true);
        accepted.add_profiles(doc).unwrap();
        assert!(!accepted.vendor_notes().is_empty());
    }

    /// The eProsima ShapesDemo client's `uxr_buffer_create_*_ref` calls,
    /// resolved against the agent's `agent.refs` profiles.
    #[test]
    fn a_reference_is_read_like_the_profile_it_names() {
        let mut profiles = AgentProfiles::default();
        profiles
            .add_profiles(
                "<profiles><topic profile_name=\"shapetype_topic\"><kind>WITH_KEY</kind>\
                 <name>Square</name><dataType>ShapeType</dataType></topic>\
                 <data_reader profile_name=\"shapetype_data_reader\"><topic><kind>WITH_KEY</kind>\
                 <name>Square</name><dataType>ShapeType</dataType></topic>\
                 <qos><reliability><kind>RELIABLE</kind></reliability></qos></data_reader>\
                 <participant profile_name=\"p\"><domainId>7</domainId></participant>\
                 </profiles>",
            )
            .unwrap();
        let t = super::topic(
            &ObjectVariant::ByReference("shapetype_topic".into()),
            false,
            &profiles,
            None,
        )
        .unwrap();
        assert_eq!(
            (t.topic_name.as_str(), t.type_name.as_str()),
            ("Square", "ShapeType")
        );
        let r = super::endpoint(
            &ObjectVariant::ByReference("shapetype_data_reader".into()),
            EndpointRole::Reader,
            false,
            &profiles,
        )
        .unwrap();
        assert_eq!(r.topic, Some(TopicRef::Name("Square".into())));
        assert!(r.qos.reliable);
        assert_eq!(
            super::participant(
                &ObjectVariant::ByReference("p".into()),
                Some(0),
                false,
                &profiles
            )
            .unwrap()
            .domain_id,
            7
        );
        assert!(matches!(
            super::topic(
                &ObjectVariant::ByReference("missing".into()),
                false,
                &profiles,
                None
            ),
            Err(ReprError::Unresolved(_))
        ));
    }

    /// `qos_profile="Lib::Profile"` of the attribute form, with its
    /// `base_name` chain; the element's own attribute overrides it.
    #[test]
    fn a_qos_profile_reference_applies_the_profile() {
        let mut profiles = AgentProfiles::default();
        profiles
            .add_profiles(
                "<dds><qos_library name=\"Lib\"><qos_profile name=\"Base\"><datawriter_qos>\
                 <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>\
                 <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>4</depth></history>\
                 <liveliness><kind>MANUAL_BY_TOPIC_LIVELINESS_QOS</kind></liveliness>\
                 <writer_data_lifecycle><autodispose_unregistered_instances>false\
                 </autodispose_unregistered_instances></writer_data_lifecycle>\
                 </datawriter_qos><publisher_qos><partition><name><element>P1</element></name></partition>\
                 </publisher_qos></qos_profile>\
                 <qos_profile name=\"Shape\" base_name=\"Base\"><datawriter_qos>\
                 <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>\
                 </datawriter_qos></qos_profile></qos_library></dds>",
            )
            .unwrap();
        let w = super::endpoint(
            &ObjectVariant::ByXmlString(
                "<data_writer topic_ref=\"0x0102\" qos_profile=\"Lib::Shape\" \
                 reliability=\"BEST_EFFORT\"/>"
                    .into(),
            ),
            EndpointRole::Writer,
            false,
            &profiles,
        )
        .unwrap();
        assert!(!w.qos.reliable, "the element's own attribute wins");
        assert_eq!(w.qos.durability, Durability::TransientLocal);
        assert_eq!(w.qos.history, History::KeepLast(4));
        assert_eq!(w.qos.partitions, vec!["P1".to_string()]);
        assert_eq!(
            w.qos.liveliness.kind,
            zerodds_qos::LivelinessKind::ManualByTopic,
            "LIVELINESS is a policy of the runtime and is applied"
        );
        assert_eq!(
            w.not_applied,
            vec![
                "qos_profile `Lib::Shape`/writer_data_lifecycle \
                 (autodispose_unregistered_instances=false)"
                    .to_string()
            ]
        );
        assert!(matches!(
            super::endpoint(
                &ObjectVariant::ByXmlString(
                    "<data_writer topic_ref=\"0x0102\" qos_profile=\"Lib::None\"/>".into()
                ),
                EndpointRole::Writer,
                false,
                &profiles,
            ),
            Err(ReprError::Unresolved(_))
        ));
    }

    /// RESOURCE_LIMITS and RELIABILITY `max_blocking_time` of a profile
    /// reach the endpoint's QoS; they used to be reported as not applied.
    #[test]
    fn a_qos_profile_carries_resource_limits_and_max_blocking_time() {
        let mut profiles = AgentProfiles::default();
        profiles
            .add_profiles(
                "<dds><qos_library name=\"Lib\"><qos_profile name=\"Limited\"><datawriter_qos>\
                 <reliability><kind>RELIABLE_RELIABILITY_QOS</kind>\
                 <max_blocking_time><sec>2</sec><nanosec>0</nanosec></max_blocking_time>\
                 </reliability>\
                 <resource_limits><max_samples>8</max_samples><max_instances>2</max_instances>\
                 <max_samples_per_instance>4</max_samples_per_instance></resource_limits>\
                 </datawriter_qos></qos_profile></qos_library></dds>",
            )
            .unwrap();
        let w = super::endpoint(
            &ObjectVariant::ByXmlString(
                "<data_writer topic_ref=\"0x0102\" qos_profile=\"Lib::Limited\"/>".into(),
            ),
            EndpointRole::Writer,
            false,
            &profiles,
        )
        .unwrap();
        assert_eq!(
            w.qos.resource_limits,
            zerodds_qos::ResourceLimitsQosPolicy {
                max_samples: 8,
                max_instances: 2,
                max_samples_per_instance: 4,
            }
        );
        assert_eq!(w.qos.max_blocking_time, Some(Duration::from_secs(2)));
        assert!(w.not_applied.is_empty(), "{:?}", w.not_applied);
    }

    #[test]
    fn an_empty_container_xml_is_a_default_container() {
        assert_eq!(
            container(&ObjectVariant::ByXmlString(String::new()), false).unwrap(),
            ContainerDescription::default()
        );
        let p = container(
            &ObjectVariant::ByXmlString(
                "<dds><publisher><qos><partition><names><name>p</name></names></partition>\
                 </qos></publisher></dds>"
                    .into(),
            ),
            false,
        )
        .unwrap();
        assert_eq!(p.partitions, vec!["p".to_string()]);
    }

    /// DDS 1.4 §2.2.3.6 PRESENTATION and §2.2.3.20 ENTITY_FACTORY of a
    /// Publisher are read, not skipped.
    #[test]
    fn a_container_reads_presentation_and_entity_factory() {
        use zerodds_qos::PresentationAccessScope;
        let p = container(
            &ObjectVariant::ByXmlString(
                "<dds><publisher><qos><presentation><access_scope>GROUP_PRESENTATION_QOS\
                 </access_scope><coherent_access>true</coherent_access><ordered_access>true\
                 </ordered_access></presentation><entity_factory><autoenable_created_entities>\
                 false</autoenable_created_entities></entity_factory></qos></publisher></dds>"
                    .into(),
            ),
            false,
        )
        .unwrap();
        assert_eq!(p.presentation.access_scope, PresentationAccessScope::Group);
        assert!(p.presentation.coherent_access);
        assert!(p.presentation.ordered_access);
        assert!(!p.autoenable_created_entities);
        assert!(p.not_applied.is_empty(), "{:?}", p.not_applied);
        let d = ContainerDescription::default();
        assert_eq!(
            d.presentation,
            zerodds_qos::PresentationQosPolicy::default()
        );
        assert!(d.autoenable_created_entities);
    }

    // ---- binary representations of §7.7.3 and Annex A ------------------

    fn le(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }

    /// §7.7.3.1.3: the topic of the 36-octet example.
    fn clause_topic_example() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&le(7));
        b.extend_from_slice(b"Square\0");
        b.push(1);
        b.extend_from_slice(&le(19));
        b.extend_from_slice(b"MyTypes::ShapeType\0");
        b.push(0);
        b
    }

    /// §7.7.3.7.3: a topic in the XCDR2 form names its type by
    /// `type_reference`; the DHEADER form of Annex A reads the same.
    #[test]
    fn a_spec_binary_topic_names_its_topic_and_type() {
        let clause = clause_topic_example();
        assert_eq!(clause.len(), 36);
        let t = topic(&ObjectVariant::InBinary(clause.clone()), false).unwrap();
        assert_eq!(t.topic_name, "Square");
        assert_eq!(t.type_name, "MyTypes::ShapeType");
        assert_eq!(t.type_identifier, None);
        let mut annex = le(36).to_vec();
        annex.extend_from_slice(&clause);
        let t = topic(&ObjectVariant::InBinary(annex), false).unwrap();
        assert_eq!(
            (t.topic_name.as_str(), t.type_name.as_str()),
            ("Square", "MyTypes::ShapeType")
        );
    }

    /// §7.7.3.7.3: a `type_identifier` is carried as the `TypeIdentifier`
    /// and the type name is left to the session to resolve; a topic with
    /// neither member is refused.
    #[test]
    fn a_spec_binary_topic_may_name_its_type_by_identifier() {
        let bytes = [2, 0, 0, 0, b'T', 0, 0, 1, 0x70, 0x10];
        let t = topic(&ObjectVariant::InBinary(bytes.to_vec()), false).unwrap();
        assert_eq!(t.topic_name, "T");
        assert_eq!(t.type_name, "");
        assert!(t.type_identifier.is_some());
        let neither = [2, 0, 0, 0, b'T', 0, 0, 0];
        assert!(matches!(
            topic(&ObjectVariant::InBinary(neither.to_vec()), false),
            Err(ReprError::Malformed(_))
        ));
    }

    /// §7.7.3.6.3: the binary participant. A reference to a Domain or a QoS
    /// profile the agent does not hold cannot be honored and is
    /// `STATUS_ERR_UNKNOWN_REFERENCE`; the `long domain_id` of the clause has
    /// to agree with the one of the representation.
    #[test]
    fn a_spec_binary_participant_is_read() {
        let participant = |bytes: &[u8], wire: Option<i16>| {
            super::participant(
                &ObjectVariant::InBinary(bytes.to_vec()),
                wire,
                false,
                &AgentProfiles::default(),
            )
        };
        // Annex A: DHEADER 2 and two absent references: the domain is the
        // representation's.
        assert_eq!(
            participant(&[2, 0, 0, 0, 0, 0], Some(4)).unwrap().domain_id,
            4
        );
        // The clause: `long domain_id` 9, no references.
        let clause = [9, 0, 0, 0, 0, 0];
        assert_eq!(participant(&clause, Some(9)).unwrap().domain_id, 9);
        assert_eq!(participant(&clause, None).unwrap().domain_id, 9);
        assert!(matches!(
            participant(&clause, Some(3)),
            Err(ReprError::Malformed(_))
        ));
        // A domain_reference: this agent holds no XRCE Domain.
        let mut with_domain = vec![9, 0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0];
        with_domain.extend_from_slice(b"Lib\0");
        with_domain.push(0);
        assert!(matches!(
            participant(&with_domain, Some(9)),
            Err(ReprError::Unresolved(_))
        ));
        // A qos_profile_reference to nothing the agent holds.
        let mut with_qos = vec![9, 0, 0, 0, 0, 1, 0, 0, 5, 0, 0, 0];
        with_qos.extend_from_slice(b"Q::P\0");
        assert!(matches!(
            participant(&with_qos, Some(9)),
            Err(ReprError::Unresolved(_))
        ));
        // The two octets the eProsima client sends.
        assert_eq!(participant(&[0, 0], Some(0)).unwrap().domain_id, 0);
    }

    /// The bytes of the clause form of `OBJK_DomainParticipant_Binary`: a
    /// `long domain_id` and two `@optional string` references.
    fn binary_participant(domain: i32, domain_ref: Option<&str>, qos_ref: Option<&str>) -> Vec<u8> {
        let mut b = domain.to_le_bytes().to_vec();
        for r in [domain_ref, qos_ref] {
            match r {
                None => b.push(0),
                Some(text) => {
                    b.push(1);
                    while b.len() % 4 != 0 {
                        b.push(0);
                    }
                    let len = u32::try_from(text.len() + 1).unwrap();
                    b.extend_from_slice(&len.to_le_bytes());
                    b.extend_from_slice(text.as_bytes());
                    b.push(0);
                }
            }
        }
        b
    }

    fn library_profiles() -> AgentProfiles {
        let mut profiles = AgentProfiles::default();
        profiles
            .add_profiles(
                "<dds><domain_library name=\"D\"><domain name=\"Dom\" domain_id=\"9\">\
                 <register_type name=\"T\" type_ref=\"M::T\"/>\
                 <topic name=\"Sq\" register_type_ref=\"T\"/></domain>\
                 <domain name=\"NoId\"/></domain_library>\
                 <qos_library name=\"Q\"><qos_profile name=\"P\"><domain_participant_qos>\
                 <user_data><value>AQID</value></user_data>\
                 <entity_factory><autoenable_created_entities>false</autoenable_created_entities>\
                 </entity_factory></domain_participant_qos></qos_profile>\
                 <qos_profile name=\"Plain\"><datawriter_qos><reliability>\
                 <kind>RELIABLE_RELIABILITY_QOS</kind></reliability></datawriter_qos>\
                 </qos_profile>\
                 </qos_library></dds>",
            )
            .unwrap();
        profiles
    }

    /// §7.7.3.6.3: a `domain_reference` is resolved against the Domains of
    /// the `<domain_library>` the agent read; only a name in no library is an
    /// unknown reference.
    #[test]
    fn a_domain_reference_is_resolved_against_the_domain_libraries() {
        let profiles = library_profiles();
        let part = |bytes: Vec<u8>, wire: Option<i16>| {
            super::participant(&ObjectVariant::InBinary(bytes), wire, false, &profiles)
        };
        // The Domain's id is the participant's domain.
        let d = part(binary_participant(9, Some("D::Dom"), None), Some(9)).unwrap();
        assert_eq!(d.domain_id, 9);
        // The topics and type registrations of the Domain are in the scope of
        // the participant (§7.7.3.6.3), as for the XML form.
        let scope = d.scope.expect("the scope of the Domain");
        assert!(scope.topic("Sq").is_some());
        assert_eq!(scope.register_type("T").unwrap().type_ref, "M::T");
        // The Domain is in a library; a name in no library is unknown.
        assert!(matches!(
            part(binary_participant(9, Some("D::Nope"), None), Some(9)),
            Err(ReprError::Unresolved(_))
        ));
        // The binary form's domain and the Domain's id must agree.
        assert!(matches!(
            part(binary_participant(4, Some("D::Dom"), None), Some(4)),
            Err(ReprError::Malformed(_))
        ));
        // A Domain without a domain_id leaves the domain to the representation.
        let d = part(binary_participant(5, Some("D::NoId"), None), Some(5)).unwrap();
        assert_eq!(d.domain_id, 5);
    }

    /// §7.7.3.6.3: the `qos_profile_reference` of a participant is applied:
    /// USER_DATA and ENTITY_FACTORY of the profile's participant QoS reach the
    /// description.
    #[test]
    fn a_participant_qos_profile_reference_is_applied() {
        let profiles = library_profiles();
        let part = |qos: &str| {
            super::participant(
                &ObjectVariant::InBinary(binary_participant(9, None, Some(qos))),
                Some(9),
                false,
                &profiles,
            )
        };
        let d = part("Q::P").unwrap();
        assert_eq!(d.qos.user_data, vec![1, 2, 3]);
        assert!(!d.qos.autoenable_created_entities);
        assert!(d.not_applied.is_empty());
        // A profile without participant QoS applies nothing and is no error.
        assert_eq!(part("Q::Plain").unwrap().qos, ParticipantQos::default());
        // A profile in no library.
        assert!(matches!(part("Q::None"), Err(ReprError::Unresolved(_))));
    }

    /// The participant QoS of the XSD of DDS-XML 1.0 holds USER_DATA and
    /// ENTITY_FACTORY, and the agent applies both; there is no policy the
    /// schema allows there that the agent cannot apply. A file that puts
    /// another policy into it is no DDS-XML and is refused when it is read,
    /// with the path of the element. `participant_qos_of` still refuses
    /// such a policy with STATUS_ERR_INCOMPATIBLE if a resolved profile
    /// carries one, instead of accepting and ignoring it.
    #[test]
    fn a_participant_policy_outside_the_schema_is_refused() {
        let mut profiles = AgentProfiles::default();
        let err = profiles
            .add_profiles(
                "<dds><qos_library name=\"Q\"><qos_profile name=\"Odd\">\
                 <domain_participant_qos><deadline><period><sec>1</sec><nanosec>0</nanosec>\
                 </period></deadline></domain_participant_qos></qos_profile></qos_library></dds>",
            )
            .unwrap_err();
        assert!(
            err.contains("domain_participant_qos") && err.contains("deadline"),
            "{err}"
        );
        // The check in the agent, for a profile that carries the policy anyway.
        let qos = zerodds_xml::qos::EntityQos {
            deadline: Some(zerodds_qos::DeadlineQosPolicy::default()),
            ..Default::default()
        };
        let resolved = zerodds_xml::qos_inheritance::ResolvedQos {
            domainparticipant_qos: vec![zerodds_xml::qos::FilteredEntityQos {
                topic_filter: None,
                name: None,
                qos,
            }],
            ..Default::default()
        };
        let odd = participant_qos_of("Q::Odd", &resolved);
        assert!(
            matches!(&odd, Err(ReprError::Incompatible(m)) if m.contains("deadline")),
            "{odd:?}"
        );
    }

    /// The QoS of a Topic (`<topic_qos base_name=…>` of DDS-XML, `qos_profile`)
    /// is what its endpoints take where they state nothing, TOPIC_DATA among
    /// it; the endpoint's own representation wins. The policies the runtime
    /// takes (LIVELINESS, LATENCY_BUDGET, DESTINATION_ORDER, TRANSPORT_PRIORITY,
    /// DURABILITY_SERVICE) reach the endpoint from a profile.
    #[test]
    fn the_qos_of_a_topic_is_the_default_of_its_endpoints() {
        let mut profiles = AgentProfiles::default();
        profiles
            .add_profiles(
                "<dds><qos_library name=\"Q\"><qos_profile name=\"T\"><topic_qos>\
                 <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>\
                 <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>\
                 <topic_data><value>AQID</value></topic_data>\
                 </topic_qos><datawriter_qos>\
                 <liveliness><kind>MANUAL_BY_TOPIC_LIVELINESS_QOS</kind>\
                 <lease_duration><sec>3</sec><nanosec>0</nanosec></lease_duration></liveliness>\
                 <latency_budget><duration><sec>1</sec><nanosec>0</nanosec></duration></latency_budget>\
                 <destination_order><kind>BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS</kind>\
                 </destination_order><transport_priority><value>7</value></transport_priority>\
                 </datawriter_qos></qos_profile></qos_library></dds>",
            )
            .unwrap();
        let t = super::topic(
            &ObjectVariant::ByXmlString(
                "<dds><topic name=\"Sq\" type_name=\"T\" qos_profile=\"Q::T\">\
                 <topic_qos><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></topic_qos>\
                 </topic></dds>"
                    .into(),
            ),
            false,
            &profiles,
            None,
        )
        .unwrap();
        assert!(t.not_applied.is_empty(), "{:?}", t.not_applied);
        assert_eq!(t.qos.reliable, Some(true));
        assert_eq!(t.qos.durability, Some(Durability::TransientLocal));
        assert_eq!(t.qos.history, Some(History::KeepAll));
        assert_eq!(t.qos.topic_data, Some(vec![1, 2, 3]));
        // A reader states nothing: it takes the Topic's.
        let r = super::endpoint_for_topic(
            &ObjectVariant::ByXmlString("<data_reader topic_ref=\"0x0102\"/>".into()),
            EndpointRole::Reader,
            false,
            &profiles,
            Some(&t.qos),
        )
        .unwrap();
        assert!(
            r.qos.reliable,
            "the Topic's RELIABLE over the reader default"
        );
        assert_eq!(r.qos.history, History::KeepAll);
        assert_eq!(r.qos.topic_data, vec![1, 2, 3]);
        // A writer that states BEST_EFFORT keeps it.
        let w = super::endpoint_for_topic(
            &ObjectVariant::ByXmlString(
                "<data_writer topic_ref=\"0x0102\" reliability=\"BEST_EFFORT\"/>".into(),
            ),
            EndpointRole::Writer,
            false,
            &profiles,
            Some(&t.qos),
        )
        .unwrap();
        assert!(!w.qos.reliable);
        assert_eq!(w.qos.durability, Durability::TransientLocal);
        // The policies of the runtime from a profile.
        let w = super::endpoint(
            &ObjectVariant::ByXmlString(
                "<data_writer topic_ref=\"0x0102\" qos_profile=\"Q::T\"/>".into(),
            ),
            EndpointRole::Writer,
            false,
            &profiles,
        )
        .unwrap();
        assert_eq!(
            w.qos.liveliness.kind,
            zerodds_qos::LivelinessKind::ManualByTopic
        );
        assert_eq!(w.qos.transport_priority, 7);
        assert_eq!(
            w.qos.destination_order.kind,
            zerodds_qos::DestinationOrderKind::BySourceTimestamp
        );
        assert!(w.not_applied.is_empty(), "{:?}", w.not_applied);
    }

    /// The QoS of a DomainParticipant definition in XML
    /// (`<domain_participant_qos base_name=…>`) is applied as the one of a
    /// `qos_profile_reference` is: USER_DATA and ENTITY_FACTORY; another
    /// policy is refused when the file is read.
    #[test]
    fn the_qos_of_an_xml_participant_is_applied() {
        let profiles = library_profiles();
        let xml = |qos: &str| {
            ObjectVariant::ByXmlString(format!(
                "<dds><domain_participant name=\"P\" domain_id=\"3\">{qos}\
                 </domain_participant></dds>"
            ))
        };
        let d = super::participant(
            &xml("<domain_participant_qos base_name=\"Q::P\"/>"),
            Some(3),
            false,
            &profiles,
        )
        .unwrap();
        assert_eq!(d.domain_id, 3);
        assert_eq!(d.qos.user_data, vec![1, 2, 3]);
        assert!(!d.qos.autoenable_created_entities);
        let d = super::participant(
            &xml(
                "<domain_participant_qos><user_data><value>BQ==</value></user_data>\
                  </domain_participant_qos>",
            ),
            Some(3),
            false,
            &profiles,
        )
        .unwrap();
        assert_eq!(d.qos.user_data, vec![5]);
        // No QoS element: the defaults.
        let d = super::participant(&xml(""), Some(3), false, &profiles).unwrap();
        assert_eq!(d.qos, ParticipantQos::default());
    }

    /// Annex A `OBJK_DataWriter_Binary` with a `qos`: bit 1 of the flags is
    /// `is_history_keep_all`, the deadline is in milliseconds, and the topic
    /// is named.
    #[test]
    fn an_annex_a_binary_writer_names_its_topic_and_qos() {
        let mut bytes = vec![23, 0, 0, 0, 7, 0, 0, 0];
        bytes.extend_from_slice(b"Square\0");
        bytes.extend_from_slice(&[1, 3, 0, 0, 1]);
        bytes.extend_from_slice(&le(120_000));
        bytes.extend_from_slice(&[0, 0, 0]);
        let w = endpoint(&ObjectVariant::InBinary(bytes), EndpointRole::Writer, false).unwrap();
        assert_eq!(w.topic, Some(TopicRef::Name("Square".into())));
        assert!(w.qos.reliable);
        assert_eq!(w.qos.history, History::KeepAll);
        assert_eq!(w.qos.deadline, Some(Duration::from_secs(120)));
    }

    /// Annex B.2.4 and B.2.7, read as the Annex A structures they print: the
    /// members of the blob (the DHEADER counted as the figure's bytes 16 to
    /// 19, the byte 35 the table calls padding being the presence octet of
    /// `deadline_msec`). B.2.4: reliable and keep all with a deadline of two
    /// minutes (the intro of the example says BEST_EFFORT, the bytes and the
    /// table say `is_reliable`); B.2.7: keep all, three minutes (the intro
    /// says five), and the filter "x>100".
    #[test]
    fn the_annex_b_binary_examples_read_as_their_qos() {
        // B.2.4 members, DHEADER 24 (the padding octet 43 counted).
        let mut w = vec![24, 0, 0, 0, 7, 0, 0, 0];
        w.extend_from_slice(b"Square\0");
        w.extend_from_slice(&[1, 3, 0, 0, 1, 0xC0, 0xD4, 0x01, 0x00, 0, 0, 0, 0]);
        let w = endpoint(&ObjectVariant::InBinary(w), EndpointRole::Writer, false).unwrap();
        assert_eq!(w.topic, Some(TopicRef::Name("Square".into())));
        assert!(w.qos.reliable);
        assert_eq!(w.qos.history, History::KeepAll);
        assert_eq!(w.qos.deadline, Some(Duration::from_secs(120)));
        // B.2.7, DHEADER 34.
        let mut r = vec![34, 0, 0, 0, 7, 0, 0, 0];
        r.extend_from_slice(b"Square\0");
        r.extend_from_slice(&[
            1, 2, 0, 0, 1, 0x20, 0xBF, 0x02, 0x00, 0, 0, 0, 1, 6, 0, 0, 0,
        ]);
        r.extend_from_slice(b"x>100\0");
        let r = endpoint(&ObjectVariant::InBinary(r), EndpointRole::Reader, false).unwrap();
        assert_eq!(r.topic, Some(TopicRef::Name("Square".into())));
        assert!(!r.qos.reliable, "is_reliable (bit 0) is clear");
        assert_eq!(r.qos.history, History::KeepAll);
        assert_eq!(r.qos.deadline, Some(Duration::from_millis(180_000)));
        assert_eq!(r.qos.content_filter.as_deref(), Some("x>100"));
        // B.2.6: default QoS, no qos member.
        let mut d = vec![12, 0, 0, 0, 7, 0, 0, 0];
        d.extend_from_slice(b"Square\0");
        d.push(0);
        let d = endpoint(&ObjectVariant::InBinary(d), EndpointRole::Reader, false).unwrap();
        assert_eq!(d.topic, Some(TopicRef::Name("Square".into())));
        assert_eq!(d.qos.content_filter, None);
    }

    /// §7.7.3.11.2: the clause's `OBJK_DataReader_Binary`, bit 1 being
    /// `is_history_keep_last`, with the depth and the time-based filter.
    #[test]
    fn a_clause_binary_reader_names_its_topic_and_qos() {
        let bytes = [
            3, 0, 0, 0, b'S', b'q', 0, 0, 2, 0, 1, 0, 5, 0, 0, 0, 0, 1, 0, 0, 50, 0, 0, 0, 0,
        ];
        let r = endpoint(
            &ObjectVariant::InBinary(bytes.to_vec()),
            EndpointRole::Reader,
            false,
        )
        .unwrap();
        assert_eq!(r.topic, Some(TopicRef::Name("Sq".into())));
        assert!(!r.qos.reliable);
        assert_eq!(r.qos.history, History::KeepLast(5));
        assert_eq!(r.qos.time_based_filter, Some(Duration::from_millis(50)));
    }

    /// Annex A `OBJK_Publisher_Binary`: a DHEADER, then the name and the qos.
    #[test]
    fn an_annex_a_binary_publisher_gives_its_partitions() {
        let bytes = [
            27, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, b'P', b'u', 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0,
            0, b'p', 0, 0,
        ];
        let c = container(&ObjectVariant::InBinary(bytes.to_vec()), false).unwrap();
        assert_eq!(c.partitions, vec!["p".to_string()]);
        assert!(c.group_data.is_empty());
    }

    /// A blob that is none of the forms is refused, and the message names
    /// what each form found.
    #[test]
    fn a_blob_of_no_form_is_refused_naming_each_reading() {
        let err = endpoint(
            &ObjectVariant::InBinary(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]),
            EndpointRole::Writer,
            false,
        )
        .unwrap_err();
        assert!(matches!(err, ReprError::Malformed(_)));
        assert!(err.message().contains("AnnexA"), "{}", err.message());
        assert!(err.message().contains("eProsima"), "{}", err.message());
    }
}
