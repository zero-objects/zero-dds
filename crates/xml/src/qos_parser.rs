// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Parser for the DDS-XML 1.0 §7.3.2 QoS profile library.
//!
//! Consumes raw XML, builds the foundation tree (see
//! [`crate::parser::parse_xml_tree`]) and maps it onto the typed
//! data model from [`crate::qos`].
//!
//! # Spec-conformant boolean semantics
//!
//! DDS-XML 1.0 §7.1.4 requires the case-sensitive spellings `true`/`false`
//! (and `1`/`0`). The local [`parse_bool_strict`] helper is the reader used
//! throughout this module; the foundation [`crate::types::parse_bool`] uses
//! the same rule.
//!
//! # Single-QoS shortcut (§7.3.2.4.4)
//!
//! A QoS container directly under `<qos_library>` is a shortcut for a
//! `<qos_profile>` with that single QoS (§7.3.2.4.4); `parse_single_qos_shortcut`
//! reads it.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_qos::{
    DeadlineQosPolicy, DestinationOrderKind, DestinationOrderQosPolicy, DurabilityKind,
    DurabilityQosPolicy, DurabilityServiceQosPolicy, Duration as QosDuration,
    EntityFactoryQosPolicy, GroupDataQosPolicy, HistoryKind, HistoryQosPolicy,
    LatencyBudgetQosPolicy, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy, OwnershipKind,
    OwnershipQosPolicy, OwnershipStrengthQosPolicy, PartitionQosPolicy, PresentationAccessScope,
    PresentationQosPolicy, ReaderDataLifecycleQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
    ResourceLimitsQosPolicy, TimeBasedFilterQosPolicy, TopicDataQosPolicy,
    TransportPriorityQosPolicy, UserDataQosPolicy, WriterDataLifecycleQosPolicy,
};

use crate::errors::XmlError;
use crate::options::{Ctx, Parsed, ReadOptions};
use crate::parser::{XmlElement, parse_xml_tree};
use crate::qos::{
    EntityQos, FilteredEntityQos, InitialResourceLimits, QosLibrary, QosProfile,
    merge_filtered_slots,
};
use crate::types::{
    DURATION_INFINITE_NSEC, DURATION_INFINITE_SEC, DURATION_ZERO_SEC, LENGTH_UNLIMITED,
    parse_duration_nsec, parse_duration_sec, parse_long, parse_positive_long_unlimited,
};

/// Parses a QoS document and returns **all** contained QoS libraries
/// (Spec §7.3.2.4.1 allows multiple libraries per document).
///
/// Accepts both roots the specification gives this building block: a
/// composed `<dds>` document and a bare `<qos_library>` document — see
/// `DdsXmlDocument::library_elements`.
///
/// # Errors
/// * [`XmlError::InvalidXml`] — XML not well-formed, or the root is
///   neither `<dds>` nor `<qos_library>`.
/// * Further errors from `parse_qos_library_element`, passed through.
pub fn parse_qos_libraries(xml: &str) -> Result<Vec<QosLibrary>, XmlError> {
    Ok(parse_qos_libraries_with(xml, &ReadOptions::default())?.value)
}

/// As [`parse_qos_libraries`], with reading options. Under
/// [`UnknownElements::ReportAndSkip`](crate::options::UnknownElements) an
/// element or attribute the schema does not define is left out and comes
/// back in [`Parsed::diagnostics`].
///
/// # Errors
/// As [`parse_qos_libraries`].
pub fn parse_qos_libraries_with(
    xml: &str,
    options: &ReadOptions,
) -> Result<Parsed<Vec<QosLibrary>>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    let mut ctx = Ctx::new(*options);
    let mut libs = Vec::new();
    for lib_node in doc.library_elements("qos_library")? {
        libs.push(parse_qos_library_element(&mut ctx, lib_node)?);
    }
    Ok(ctx.finish(libs))
}

/// Convenience: parses the document and returns the **first** QoS library.
/// Returns `MissingRequiredElement("qos_library")` if none is
/// present.
///
/// # Errors
/// As [`parse_qos_libraries`], plus `MissingRequiredElement` if the
/// document contains no library.
pub fn parse_qos_library(xml: &str) -> Result<QosLibrary, XmlError> {
    parse_qos_libraries(xml)?
        .into_iter()
        .next()
        .ok_or_else(|| XmlError::MissingRequiredElement("qos_library".into()))
}

/// Crate-internal `pub` wrapper for `parse_qos_library_element`, so that
/// the top-level loader (`crate::zerodds_xml::parse_dds_xml`) can decode a
/// `<qos_library>` element sub-tree directly without duplicating the
/// parser.
///
/// # Errors
/// See `parse_qos_library_element`.
pub fn parse_qos_library_element_public(el: &XmlElement) -> Result<QosLibrary, XmlError> {
    parse_qos_library_element(&mut Ctx::new(ReadOptions::default()), el)
}

/// As [`parse_qos_library_element_public`], with reading options.
///
/// # Errors
/// See `parse_qos_library_element`.
pub fn parse_qos_library_element_with(
    el: &XmlElement,
    options: &ReadOptions,
) -> Result<Parsed<QosLibrary>, XmlError> {
    let mut ctx = Ctx::new(*options);
    let library = parse_qos_library_element(&mut ctx, el)?;
    Ok(ctx.finish(library))
}

/// The names of the six QoS containers (DDS-XML 1.0 §7.3.2.4), with the
/// two spellings of the participant container other products write.
/// [`vendor_container_name`] says which of them is not the schema's.
fn container_kind(name: &str) -> Option<QosKind> {
    match name {
        "datawriter_qos" => Some(QosKind::DataWriter),
        "datareader_qos" => Some(QosKind::DataReader),
        "topic_qos" => Some(QosKind::Topic),
        "publisher_qos" => Some(QosKind::Publisher),
        "subscriber_qos" => Some(QosKind::Subscriber),
        // `domain_participant_qos` is the name the specification's own
        // schema declares; the other two are vendor spellings.
        "domain_participant_qos" | "domainparticipant_qos" | "participant_qos" => {
            Some(QosKind::DomainParticipant)
        }
        _ => None,
    }
}

/// Refuses (or reports, under `VendorForms::Accept`) a container spelling
/// the schema does not declare.
fn vendor_container_name(ctx: &mut Ctx, el: &XmlElement) -> Result<(), XmlError> {
    if matches!(
        el.name.as_str(),
        "domainparticipant_qos" | "participant_qos"
    ) {
        ctx.vendor_form(
            &el.path,
            &format!("<{}>", el.name),
            "<domain_participant_qos>",
        )?;
    }
    Ok(())
}

/// The attributes of a QoS container in the schema (`datawriterQos` and its
/// siblings).
const CONTAINER_ATTRIBUTES: &[&str] = &["name", "base_name", "topic_filter"];

pub(crate) fn parse_qos_library_element(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<QosLibrary, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("qos_library@name".into()))?
        .to_string();
    // Document order matters: a profile "can only inherit from QoS
    // profiles that have been defined" before it (§7.3.2.4.2), and a
    // single-QoS shortcut (§7.3.2.4.4) is a profile as well.
    let mut profiles = Vec::new();
    for child in &el.children {
        if child.name == "qos_profile" {
            profiles.push(parse_qos_profile_element(ctx, child)?);
        } else if container_kind(&child.name).is_some() {
            // Spec §7.3.2.4.4: "The definition of an individual QoS is a
            // shortcut for defining a QoS profile with a single QoS." The
            // schema (`qosLibrary`) does not require its `name`; one
            // without is a profile nothing can refer to, kept under the
            // empty name.
            profiles.push(parse_single_qos_shortcut(ctx, child)?);
        } else {
            ctx.unknown_child(child)?;
        }
    }
    Ok(QosLibrary { name, profiles })
}

/// A QoS profile written directly under `<dds>`, as DDS4CCM 1.1 §7.4.2.3 and
/// §7.4.2.3.3 write it (a `<qos_profile>`, or a single-QoS shortcut such as
/// `<datawriter_qos name="…">`, without a `<qos_library>` around it), which
/// DDS-RPC 1.0 §7.10.1 names as the format of its `@Qos` profile files.
///
/// DDS-XML 1.0 has no such form, so it is a vendor form: refused with an
/// error that names it, or read under [`crate::VendorForms::Accept`] and
/// reported.
///
/// # Errors
/// [`XmlError::VendorForm`] under the default reading, and the errors of
/// the profile reader.
pub(crate) fn parse_ccm_top_level_profile(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<Option<QosProfile>, XmlError> {
    if el.name != "qos_profile" && container_kind(&el.name).is_none() {
        return Ok(None);
    }
    ctx.vendor_form(
        &el.path,
        &format!("<{}> directly under <dds> (DDS4CCM 1.1 §7.4.2.3)", el.name),
        "the profile inside a <qos_library> (§7.3.2.3)",
    )?;
    let profile = if el.name == "qos_profile" {
        parse_qos_profile_element(ctx, el)?
    } else {
        parse_single_qos_shortcut(ctx, el)?
    };
    Ok(Some(profile))
}

/// Spec §7.3.2.4.4 single-QoS shortcut — a `<datawriter_qos>` (or
/// `_reader`/`_topic`/`_publisher`/`_subscriber`/`_participant`) directly
/// under `<qos_library>` is equivalent to a `<qos_profile name="…">` with
/// exactly one QoS.
fn parse_single_qos_shortcut(ctx: &mut Ctx, el: &XmlElement) -> Result<QosProfile, XmlError> {
    let kind = container_kind(&el.name)
        .ok_or_else(|| XmlError::UnknownElement(format!("{}: not a QoS container", el.path)))?;
    vendor_container_name(ctx, el)?;
    let name = el.attribute("name").unwrap_or_default().to_string();
    let base_name = el.attribute("base_name").map(ToString::to_string);
    // §7.3.2.4.3 puts the topic filter on the QoS container, and
    // §7.3.2.4.4 makes a named container a profile of its own — so the
    // filter of the shortcut form is the filter of that container (and
    // of the implicit profile).
    let topic_filter = el.attribute("topic_filter").map(ToString::to_string);
    let mut profile = QosProfile {
        name,
        base_name,
        topic_filter: topic_filter.clone(),
        ..QosProfile::default()
    };
    // `base_name` of the shortcut element is the base of the profile it
    // stands for (§7.3.2.4.4), already in `profile.base_name`.
    ctx.check_attributes(el, CONTAINER_ATTRIBUTES)?;
    let mut qos = parse_entity_qos(ctx, el)?;
    qos.base_name = None;
    let slot = FilteredEntityQos {
        topic_filter,
        name: None,
        qos,
    };
    match kind {
        QosKind::DataWriter => profile.datawriter_qos.push(slot),
        QosKind::DataReader => profile.datareader_qos.push(slot),
        QosKind::Topic => profile.topic_qos.push(slot),
        QosKind::Publisher => profile.publisher_qos.push(slot),
        QosKind::Subscriber => profile.subscriber_qos.push(slot),
        QosKind::DomainParticipant => profile.domainparticipant_qos.push(slot),
    }
    Ok(profile)
}

fn parse_qos_profile_element(ctx: &mut Ctx, el: &XmlElement) -> Result<QosProfile, XmlError> {
    ctx.check_attributes(el, &["name", "base_name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("qos_profile@name".into()))?
        .to_string();
    let base_name = el.attribute("base_name").map(ToString::to_string);

    let mut profile = QosProfile {
        name,
        base_name,
        ..QosProfile::default()
    };

    for child in &el.children {
        if let Some(kind) = container_kind(&child.name) {
            vendor_container_name(ctx, child)?;
            take_qos_container(ctx, &mut profile, child, kind)?;
        } else if child.name == "topic_filter" {
            // Vendor spelling: the filter as a child element. The
            // specification writes it as an attribute on the QoS
            // container (§7.3.2.4.3) — see `take_qos_container`.
            ctx.vendor_form(
                &child.path,
                "<topic_filter> as a child element",
                "the topic_filter attribute of a QoS container (§7.3.2.4.3)",
            )?;
            profile.topic_filter = Some(child.text.trim().to_string());
        } else {
            // An element the crate does not model (a `<data_representation>`
            // or a vendor policy) is not read: the reading's
            // `UnknownElements` setting decides between an error and a
            // diagnostic.
            ctx.unknown_child(child)?;
        }
    }
    Ok(profile)
}

/// The six QoS containers a `<qos_profile>` may carry (§7.3.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QosKind {
    DataWriter,
    DataReader,
    Topic,
    Publisher,
    Subscriber,
    DomainParticipant,
}

/// Reads one QoS container into its slot on `profile`.
///
/// §7.3.2.4.3: "A QoS Profile may contain several DataWriter,
/// DataReader, and Topic QoS settings that are selected based on the
/// evaluation of a filter expression on the topic name. The filter
/// expression is specified via the `topic_filter` XML attribute in the
/// QoS definition of the entity QoS." The schema (`qosProfile`) lets every
/// container repeat. Containers with a different filter stay apart, one per
/// filter; containers with the same filter (including none) merge, the later
/// one's policies overriding the earlier's.
///
/// A `base_name` on the container names a profile whose container of the
/// same kind is the base of this one (DDS-XML 1.0 §7.3.5.4.4, the form the
/// schema gives every QoS container); it is resolved with the profile.
fn take_qos_container(
    ctx: &mut Ctx,
    profile: &mut QosProfile,
    el: &XmlElement,
    kind: QosKind,
) -> Result<(), XmlError> {
    ctx.check_attributes(el, CONTAINER_ATTRIBUTES)?;
    let slot = FilteredEntityQos {
        topic_filter: el.attribute("topic_filter").map(ToString::to_string),
        name: el.attribute("name").map(ToString::to_string),
        qos: parse_entity_qos(ctx, el)?,
    };
    let target = match kind {
        QosKind::DataWriter => &mut profile.datawriter_qos,
        QosKind::DataReader => &mut profile.datareader_qos,
        QosKind::Topic => &mut profile.topic_qos,
        QosKind::Publisher => &mut profile.publisher_qos,
        QosKind::Subscriber => &mut profile.subscriber_qos,
        QosKind::DomainParticipant => &mut profile.domainparticipant_qos,
    };
    *target = merge_filtered_slots(core::mem::take(target), &[slot]);
    Ok(())
}

/// Crate-internal `pub` wrapper for `parse_entity_qos`, so that other
/// building-block modules (domain, participant) can decode inline QoS
/// containers without duplicating the parser.
///
/// # Errors
/// See `parse_entity_qos`.
pub fn parse_entity_qos_public(el: &XmlElement) -> Result<EntityQos, XmlError> {
    parse_entity_qos(&mut Ctx::new(ReadOptions::default()), el)
}

/// As [`parse_entity_qos_public`], with reading options.
///
/// # Errors
/// See `parse_entity_qos`.
pub fn parse_entity_qos_with(
    el: &XmlElement,
    options: &ReadOptions,
) -> Result<Parsed<EntityQos>, XmlError> {
    let mut ctx = Ctx::new(*options);
    let qos = parse_entity_qos(&mut ctx, el)?;
    Ok(ctx.finish(qos))
}

/// Reads an inline QoS container of an entity into `slot` (§7.3.5.4.4:
/// "Inline QoS settings apply only to the entity in whose context they are
/// being defined"). One container per entity: a second one would replace
/// the first without a word, so it is refused.
///
/// # Errors
/// Those of `parse_entity_qos`; [`XmlError::InvalidXml`] if `slot` is already
/// set.
pub(crate) fn set_inline_qos(
    ctx: &mut Ctx,
    slot: &mut Option<EntityQos>,
    parent: &str,
    el: &XmlElement,
) -> Result<(), XmlError> {
    if slot.is_some() {
        return Err(XmlError::InvalidXml(format!(
            "<{parent}> carries <{}> more than once; the inline QoS of an entity is one container",
            el.name
        )));
    }
    ctx.check_attributes(el, CONTAINER_ATTRIBUTES)?;
    *slot = Some(parse_entity_qos(ctx, el)?);
    Ok(())
}

/// The 22 policies of DDS 1.4 §2.2.3 by their element names.
const ALL_POLICIES: &[&str] = &[
    "durability",
    "durability_service",
    "presentation",
    "deadline",
    "latency_budget",
    "ownership",
    "ownership_strength",
    "liveliness",
    "time_based_filter",
    "partition",
    "reliability",
    "transport_priority",
    "lifespan",
    "destination_order",
    "history",
    "resource_limits",
    "entity_factory",
    "writer_data_lifecycle",
    "reader_data_lifecycle",
    "user_data",
    "topic_data",
    "group_data",
];

/// The policies each entity QoS container may hold, from the DDS 1.4 IDL
/// (§2.2.3). §7.2 maps the IDL 1:1, so a policy that is not a field of the
/// container's structure is not part of that container.
fn allowed_policies(container: &str) -> Option<&'static [&'static str]> {
    match container {
        "datawriter_qos" => Some(&[
            "durability",
            "durability_service",
            "deadline",
            "latency_budget",
            "liveliness",
            "reliability",
            "destination_order",
            "history",
            "resource_limits",
            "transport_priority",
            "lifespan",
            "user_data",
            "ownership",
            "ownership_strength",
            "writer_data_lifecycle",
        ]),
        "datareader_qos" => Some(&[
            "durability",
            "deadline",
            "latency_budget",
            "liveliness",
            "reliability",
            "destination_order",
            "history",
            "resource_limits",
            "user_data",
            "ownership",
            "time_based_filter",
            "reader_data_lifecycle",
        ]),
        "topic_qos" => Some(&[
            "topic_data",
            "durability",
            "durability_service",
            "deadline",
            "latency_budget",
            "liveliness",
            "reliability",
            "destination_order",
            "history",
            "resource_limits",
            "transport_priority",
            "lifespan",
            "ownership",
        ]),
        "publisher_qos" | "subscriber_qos" => {
            Some(&["presentation", "partition", "group_data", "entity_factory"])
        }
        "domain_participant_qos" | "domainparticipant_qos" | "participant_qos" => {
            Some(&["user_data", "entity_factory"])
        }
        // Not one of the six containers this crate names; leave it open.
        _ => None,
    }
}

pub(crate) fn parse_entity_qos(ctx: &mut Ctx, el: &XmlElement) -> Result<EntityQos, XmlError> {
    // §7.3.5.4.4: "Inline entities may inherit from an existing QoS
    // Profile using the base_name XML attribute." The attribute sits on
    // the QoS container itself (`<topic_qos base_name="…">`).
    let mut q = EntityQos {
        base_name: el.attribute("base_name").map(ToString::to_string),
        ..EntityQos::default()
    };
    let allowed = allowed_policies(&el.name);
    let mut seen: alloc::collections::BTreeSet<&str> = alloc::collections::BTreeSet::new();
    for child in &el.children {
        if let Some(allowed) = allowed {
            if !allowed.contains(&child.name.as_str()) {
                // §7.2: the IDL structure is mapped 1:1. A policy that is
                // not a field of this container would be read and then
                // dropped by materialization without a word.
                if ALL_POLICIES.contains(&child.name.as_str()) {
                    ctx.misplaced_child(
                        child,
                        &format!("not a field of the {} (DDS 1.4 §2.2.3)", el.name),
                    )?;
                } else {
                    ctx.unknown_child(child)?;
                }
                continue;
            }
        }
        if !seen.insert(child.name.as_str()) {
            return Err(XmlError::InvalidXml(format!(
                "<{}> carries <{}> more than once",
                el.name, child.name
            )));
        }
        match child.name.as_str() {
            "durability" => q.durability = Some(parse_durability(ctx, child)?),
            "durability_service" => {
                q.durability_service = Some(parse_durability_service(ctx, child)?)
            }
            "presentation" => q.presentation = Some(parse_presentation(ctx, child)?),
            "deadline" => q.deadline = Some(parse_deadline(ctx, child)?),
            "latency_budget" => q.latency_budget = Some(parse_latency_budget(ctx, child)?),
            "ownership" => q.ownership = Some(parse_ownership(ctx, child)?),
            "ownership_strength" => {
                q.ownership_strength = Some(parse_ownership_strength(ctx, child)?)
            }
            "liveliness" => q.liveliness = Some(parse_liveliness(ctx, child)?),
            "time_based_filter" => q.time_based_filter = Some(parse_time_based_filter(ctx, child)?),
            "partition" => q.partition = Some(parse_partition(ctx, child)?),
            "reliability" => {
                // §7.2.3: an omitted field has the default the DDS gives
                // it, and §2.2.3.14 gives a DataWriter RELIABLE where a
                // DataReader and a Topic get BEST_EFFORT.
                let default_kind = if el.name == "datawriter_qos" {
                    ReliabilityKind::Reliable
                } else {
                    ReliabilityKind::BestEffort
                };
                q.reliability = Some(parse_reliability(ctx, child, default_kind)?);
            }
            "transport_priority" => {
                q.transport_priority = Some(parse_transport_priority(ctx, child)?)
            }
            "lifespan" => q.lifespan = Some(parse_lifespan(ctx, child)?),
            "destination_order" => q.destination_order = Some(parse_destination_order(ctx, child)?),
            "history" => q.history = Some(parse_history(ctx, child)?),
            "resource_limits" => {
                let (limits, initial) = parse_resource_limits(ctx, child)?;
                q.resource_limits = Some(limits);
                q.initial_resource_limits = initial;
            }
            "entity_factory" => q.entity_factory = Some(parse_entity_factory(ctx, child)?),
            "writer_data_lifecycle" => {
                q.writer_data_lifecycle = Some(parse_writer_data_lifecycle(ctx, child)?);
            }
            "reader_data_lifecycle" => {
                q.reader_data_lifecycle = Some(parse_reader_data_lifecycle(ctx, child)?);
            }
            "user_data" => q.user_data = Some(parse_user_data(ctx, child)?),
            "topic_data" => q.topic_data = Some(parse_topic_data(ctx, child)?),
            "group_data" => q.group_data = Some(parse_group_data(ctx, child)?),
            // Not modelled: `representation`/`data_representation` and
            // `type_consistency` (XTypes), vendor policies. Refused by
            // name, as in `parse_qos_profile_element`.
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(q)
}

// ============================================================================
// Spec-strict boolean parser
// ============================================================================

/// DDS-XML 1.0 §7.1.4 Tab.7.1 / §7.1.5 Tab.7.2 boolean, for element
/// values and attribute values alike: "Yes: 1 or true. No: 0 or false.
/// Values are case sensitive."
///
/// So `1`, `true`, `0` and `false` are the four legal spellings, and
/// every case variant of them (`TRUE`, `True`, `False`) is not.
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] on any other spelling (incl.
/// `True`, `TRUE`, `yes`).
pub fn parse_bool_strict(s: &str) -> Result<bool, XmlError> {
    let t = s.trim();
    match t {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(XmlError::ValueOutOfRange(format!(
            "DDS-XML boolean must be `true`/`1` or `false`/`0` (case-sensitive), got `{t}`"
        ))),
    }
}

// ============================================================================
// Duration-Helper: XML-`<sec>/<nanosec>` ↔ `zerodds_qos::Duration`
// ============================================================================

/// Converts an XML `Duration_t` representation
/// (`<sec>` + optional `<nanosec>`, or inline text `DURATION_INFINITY`)
/// into a `zerodds_qos::Duration` (i32 seconds + u32 fraction).
///
/// Accepted XML formats:
/// 1. `<duration><sec>1</sec><nanosec>0</nanosec></duration>` — full.
/// 2. `<duration><sec>DURATION_INFINITY</sec></duration>` — sentinel
///    (maps to [`QosDuration::INFINITE`]).
/// 3. `<duration>DURATION_INFINITY</duration>` — inline text sentinel.
///    The schema's `duration` has no text content; this is a vendor form.
fn parse_duration_element(ctx: &mut Ctx, el: &XmlElement) -> Result<QosDuration, XmlError> {
    check_children(ctx, el, &["sec", "nanosec"])?;
    // Inline sentinel text (form 3).
    let trimmed = el.text.trim();
    if !trimmed.is_empty() && el.children.is_empty() {
        ctx.vendor_form(
            &el.path,
            &format!("the text `{trimmed}` as the whole value of a duration"),
            "<sec>DURATION_INFINITY</sec> (§7.2.6)",
        )?;
        return parse_duration_text_sentinel(trimmed);
    }
    if !trimmed.is_empty() {
        return Err(XmlError::InvalidXml(format!(
            "<{}> holds text `{trimmed}` next to <sec>/<nanosec>",
            el.name
        )));
    }

    // §7.2.6 marks both `sec` and `nanosec` `minOccurs="0"`; an absent
    // field takes the DDS default of 0.
    let sec = if let Some(t) = numeric_text(el, "sec") {
        parse_duration_sec(t)?
    } else {
        DURATION_ZERO_SEC
    };

    // nanosec optional; default 0.
    let nsec = if let Some(t) = numeric_text(el, "nanosec") {
        parse_duration_nsec(t)?
    } else {
        0
    };

    // Spec sentinel mapping: when the `<sec>` value carries the spec sentinel
    // `DURATION_INFINITE_SEC` (= 0x7FFFFFFF), we map to
    // `QosDuration::INFINITE` (`{i32::MAX, u32::MAX}`). That is the
    // wire convention from DDSI-RTPS §9.3.2 and handles both
    // `<sec>DURATION_INFINITY</sec>` (with or without `<nanosec>`) and
    // `<sec>2147483647</sec><nanosec>2147483647</nanosec>` correctly.
    if sec == DURATION_INFINITE_SEC
        && (nsec == DURATION_INFINITE_NSEC || numeric_text(el, "nanosec").is_none())
    {
        return Ok(QosDuration::INFINITE);
    }

    // nanosec → fraction = nsec * 2^32 / 1e9, rounded to the nearest; a
    // nanosec of a second or more carries.
    Ok(QosDuration::from_sec_nanosec(sec, nsec))
}

fn parse_duration_text_sentinel(t: &str) -> Result<QosDuration, XmlError> {
    if t == "DURATION_INFINITY" {
        Ok(QosDuration::INFINITE)
    } else {
        Err(XmlError::ValueOutOfRange(format!(
            "duration inline-text must be `DURATION_INFINITY`, got `{t}`"
        )))
    }
}

// ============================================================================
// Per-Policy-Parser
// ============================================================================

/// Checks the children of a policy element against the ones its schema
/// defines. Each parser looked up the children it knew and skipped the rest,
/// so a misspelt `<max_blocking_tme>` or a vendor sub-element left the
/// default in place without a message; now the reading's
/// [`UnknownElements`](crate::options::UnknownElements) decides (error, or a
/// diagnostic and the element is left out).
fn only_children(ctx: &mut Ctx, el: &XmlElement, allowed: &[&str]) -> Result<(), XmlError> {
    no_text(el)?;
    check_children(ctx, el, allowed)
}

/// A structure element holds its fields as child elements (§7.2.3); text
/// next to them would be dropped.
fn no_text(el: &XmlElement) -> Result<(), XmlError> {
    if !el.text.trim().is_empty() {
        return Err(XmlError::InvalidXml(format!(
            "<{}> holds text `{}` where only child elements are defined",
            el.path,
            el.text.trim()
        )));
    }
    Ok(())
}

/// The child-element half of [`only_children`], for an element that may
/// also carry text (`Duration_t` in the sentinel form).
fn check_children(ctx: &mut Ctx, el: &XmlElement, allowed: &[&str]) -> Result<(), XmlError> {
    check_known_children(ctx, el, allowed)?;
    // §7.2.3: a structure field is a single element. Two occurrences carry
    // two values; keeping the first (as `child` would) drops the other
    // without a word.
    let mut seen: alloc::collections::BTreeSet<&str> = alloc::collections::BTreeSet::new();
    for c in el
        .children
        .iter()
        .filter(|c| allowed.contains(&c.name.as_str()))
    {
        if !seen.insert(c.name.as_str()) {
            return Err(XmlError::InvalidXml(format!(
                "<{}> carries <{}> more than once",
                el.path, c.name
            )));
        }
    }
    Ok(())
}

/// Reports a child that is not in `allowed`; repetition is the caller's
/// business (the items of a sequence repeat).
fn check_known_children(ctx: &mut Ctx, el: &XmlElement, allowed: &[&str]) -> Result<(), XmlError> {
    for c in el
        .children
        .iter()
        .filter(|c| !allowed.contains(&c.name.as_str()))
    {
        ctx.unknown_child(c)?;
    }
    Ok(())
}

/// A numeric field child: `None` when the child is absent or blank. The
/// schema's numeric simple types (`nonNegativeInteger_UNLIMITED`,
/// `nonNegativeInteger_Duration_SEC`, …) allow the empty string, and an
/// element that is present but empty has the value the DDS gives an omitted
/// field (§7.2.3).
fn numeric_text<'a>(el: &'a XmlElement, name: &str) -> Option<&'a str> {
    el.child(name)
        .map(|c| c.text.trim())
        .filter(|t| !t.is_empty())
}

/// The text of a `kind`-style child, or `None` when the child is absent
/// or empty. §7.2.3 makes structure fields (`minOccurs="0"`) optional and
/// the XSD provides the DDS default; an element that is present but
/// empty is read the same way.
fn parse_kind_text_opt(el: &XmlElement) -> Option<&str> {
    el.child("kind")
        .map(|k| k.text.trim())
        .filter(|s| !s.is_empty())
}

fn parse_durability(ctx: &mut Ctx, el: &XmlElement) -> Result<DurabilityQosPolicy, XmlError> {
    only_children(ctx, el, &["kind"])?;
    let mut policy = DurabilityQosPolicy::default();
    if let Some(kind_str) = parse_kind_text_opt(el) {
        policy.kind = match kind_str {
            // §7.1.4 Tab.7.1: "Legal values are the ones defined for
            // QoS Policies in the DCPS IDL"; §7.2.1.1 spells them out
            // as the IDL enumeration literals.
            "VOLATILE_DURABILITY_QOS" => DurabilityKind::Volatile,
            "TRANSIENT_LOCAL_DURABILITY_QOS" => DurabilityKind::TransientLocal,
            "TRANSIENT_DURABILITY_QOS" => DurabilityKind::Transient,
            "PERSISTENT_DURABILITY_QOS" => DurabilityKind::Persistent,
            other => return Err(XmlError::BadEnum(other.to_string())),
        };
    }
    Ok(policy)
}

/// A `long` that the schema types as `xs:positiveInteger` (`min` 1) or
/// `xs:nonNegativeInteger` (`min` 0).
fn at_least(t: &str, min: i32, what: &str) -> Result<i32, XmlError> {
    let v = parse_long(t)?;
    if v < min {
        return Err(XmlError::ValueOutOfRange(format!(
            "{what} `{t}`: the schema's type takes {min} and up"
        )));
    }
    Ok(v)
}

fn parse_history(ctx: &mut Ctx, el: &XmlElement) -> Result<HistoryQosPolicy, XmlError> {
    only_children(ctx, el, &["kind", "depth"])?;
    let kind = match parse_kind_text_opt(el) {
        // Absent or empty: the DDS default of §2.2.3.18.
        None => HistoryKind::KeepLast,
        Some("KEEP_LAST_HISTORY_QOS") => HistoryKind::KeepLast,
        Some("KEEP_ALL_HISTORY_QOS") => HistoryKind::KeepAll,
        Some(other) => return Err(XmlError::BadEnum(other.to_string())),
    };
    let depth = if let Some(t) = numeric_text(el, "depth") {
        at_least(t, 1, "history depth")?
    } else {
        // Spec-Default §2.2.3.18: 1.
        1
    };
    Ok(HistoryQosPolicy { kind, depth })
}

fn parse_reliability(
    ctx: &mut Ctx,
    el: &XmlElement,
    default_kind: ReliabilityKind,
) -> Result<ReliabilityQosPolicy, XmlError> {
    only_children(ctx, el, &["kind", "max_blocking_time"])?;
    let mut policy = ReliabilityQosPolicy {
        kind: default_kind,
        ..ReliabilityQosPolicy::default()
    };
    if let Some(kind_str) = parse_kind_text_opt(el) {
        policy.kind = match kind_str {
            "BEST_EFFORT_RELIABILITY_QOS" => ReliabilityKind::BestEffort,
            "RELIABLE_RELIABILITY_QOS" => ReliabilityKind::Reliable,
            other => return Err(XmlError::BadEnum(other.to_string())),
        };
    }
    if let Some(mbt) = el.child("max_blocking_time") {
        policy.max_blocking_time = parse_duration_element(ctx, mbt)?;
    }
    Ok(policy)
}

fn parse_deadline(ctx: &mut Ctx, el: &XmlElement) -> Result<DeadlineQosPolicy, XmlError> {
    only_children(ctx, el, &["period"])?;
    let mut policy = DeadlineQosPolicy::default();
    if let Some(period) = el.child("period") {
        policy.period = parse_duration_element(ctx, period)?;
    }
    Ok(policy)
}

fn parse_latency_budget(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<LatencyBudgetQosPolicy, XmlError> {
    only_children(ctx, el, &["duration"])?;
    let mut policy = LatencyBudgetQosPolicy::default();
    if let Some(dur) = el.child("duration") {
        policy.duration = parse_duration_element(ctx, dur)?;
    }
    Ok(policy)
}

fn parse_lifespan(ctx: &mut Ctx, el: &XmlElement) -> Result<LifespanQosPolicy, XmlError> {
    only_children(ctx, el, &["duration"])?;
    let mut policy = LifespanQosPolicy::default();
    if let Some(dur) = el.child("duration") {
        policy.duration = parse_duration_element(ctx, dur)?;
    }
    Ok(policy)
}

fn parse_time_based_filter(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<TimeBasedFilterQosPolicy, XmlError> {
    only_children(ctx, el, &["minimum_separation"])?;
    let mut policy = TimeBasedFilterQosPolicy::default();
    if let Some(sep) = el.child("minimum_separation") {
        policy.minimum_separation = parse_duration_element(ctx, sep)?;
    }
    Ok(policy)
}

fn parse_liveliness(ctx: &mut Ctx, el: &XmlElement) -> Result<LivelinessQosPolicy, XmlError> {
    only_children(ctx, el, &["kind", "lease_duration"])?;
    let kind = if let Some(k) = el.child("kind") {
        // §7.1.4 Tab.7.1 / §7.2.1.1: the DCPS IDL enumeration literals.
        match k.text.trim() {
            "AUTOMATIC_LIVELINESS_QOS" => LivelinessKind::Automatic,
            "MANUAL_BY_PARTICIPANT_LIVELINESS_QOS" => LivelinessKind::ManualByParticipant,
            "MANUAL_BY_TOPIC_LIVELINESS_QOS" => LivelinessKind::ManualByTopic,
            other => return Err(XmlError::BadEnum(other.to_string())),
        }
    } else {
        LivelinessKind::Automatic
    };
    let lease_duration = if let Some(ld) = el.child("lease_duration") {
        parse_duration_element(ctx, ld)?
    } else {
        QosDuration::INFINITE
    };
    Ok(LivelinessQosPolicy {
        kind,
        lease_duration,
    })
}

fn parse_destination_order(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<DestinationOrderQosPolicy, XmlError> {
    only_children(ctx, el, &["kind"])?;
    let mut policy = DestinationOrderQosPolicy::default();
    if let Some(kind_str) = parse_kind_text_opt(el) {
        policy.kind = match kind_str {
            // §7.1.4 Tab.7.1 / §7.2.1.1: the DCPS IDL enumeration literals.
            "BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS" => {
                DestinationOrderKind::ByReceptionTimestamp
            }
            "BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS" => DestinationOrderKind::BySourceTimestamp,
            other => return Err(XmlError::BadEnum(other.to_string())),
        };
    }
    Ok(policy)
}

fn parse_ownership(ctx: &mut Ctx, el: &XmlElement) -> Result<OwnershipQosPolicy, XmlError> {
    only_children(ctx, el, &["kind"])?;
    let mut policy = OwnershipQosPolicy::default();
    if let Some(kind_str) = parse_kind_text_opt(el) {
        policy.kind = match kind_str {
            // §7.1.4 Tab.7.1 / §7.2.1.1: the DCPS IDL enumeration literals.
            "SHARED_OWNERSHIP_QOS" => OwnershipKind::Shared,
            "EXCLUSIVE_OWNERSHIP_QOS" => OwnershipKind::Exclusive,
            other => return Err(XmlError::BadEnum(other.to_string())),
        };
    }
    Ok(policy)
}

fn parse_ownership_strength(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<OwnershipStrengthQosPolicy, XmlError> {
    only_children(ctx, el, &["value"])?;
    let mut policy = OwnershipStrengthQosPolicy::default();
    if let Some(t) = numeric_text(el, "value") {
        policy.value = at_least(t, 0, "ownership_strength value")?;
    }
    Ok(policy)
}

fn parse_transport_priority(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<TransportPriorityQosPolicy, XmlError> {
    only_children(ctx, el, &["value"])?;
    let mut policy = TransportPriorityQosPolicy::default();
    if let Some(t) = numeric_text(el, "value") {
        policy.value = at_least(t, 0, "transport_priority value")?;
    }
    Ok(policy)
}

fn parse_presentation(ctx: &mut Ctx, el: &XmlElement) -> Result<PresentationQosPolicy, XmlError> {
    only_children(
        ctx,
        el,
        &["access_scope", "coherent_access", "ordered_access"],
    )?;
    let access_scope = if let Some(s) = el.child("access_scope") {
        // §7.1.4 Tab.7.1 / §7.2.1.1: the DCPS IDL enumeration literals.
        match s.text.trim() {
            "INSTANCE_PRESENTATION_QOS" => PresentationAccessScope::Instance,
            "TOPIC_PRESENTATION_QOS" => PresentationAccessScope::Topic,
            "GROUP_PRESENTATION_QOS" => PresentationAccessScope::Group,
            other => return Err(XmlError::BadEnum(other.to_string())),
        }
    } else {
        PresentationAccessScope::Instance
    };
    let coherent_access = if let Some(c) = el.child("coherent_access") {
        parse_bool_strict(&c.text)?
    } else {
        false
    };
    let ordered_access = if let Some(o) = el.child("ordered_access") {
        parse_bool_strict(&o.text)?
    } else {
        false
    };
    Ok(PresentationQosPolicy {
        access_scope,
        coherent_access,
        ordered_access,
    })
}

fn parse_partition(ctx: &mut Ctx, el: &XmlElement) -> Result<PartitionQosPolicy, XmlError> {
    // DDS 1.4 IDL: `struct PartitionQosPolicy { StringSeq name; };`.
    // The single field `name` maps to one `<name>` element (§7.2.3), and
    // its sequence value maps to `<element>` children (§7.2.4.1:
    // "The complexType contains zero or more elements named element").
    only_children(ctx, el, &["name"])?;
    let mut names: Vec<String> = Vec::new();
    let name_elements: Vec<&XmlElement> = el.children_named("name").collect();
    if name_elements.len() > 1 {
        return Err(XmlError::InvalidXml(format!(
            "<partition> carries {} <name> elements; §7.2.4.1 represents the sequence as one \
             <name> holding the <element> children",
            name_elements.len()
        )));
    }
    if let Some(name_el) = name_elements.first() {
        // `<element>` is the item of a sequence (§7.2.4.1) and repeats.
        no_text(name_el)?;
        check_known_children(ctx, name_el, &["element"])?;
        for item in name_el.children_named("element") {
            names.push(item.text.clone());
        }
    }
    Ok(PartitionQosPolicy { names })
}

/// `resourceLimitsQosPolicy` of the schema: the three limits of DDS 1.4
/// and `initial_samples`/`initial_instances`, which the schema has and the
/// DDS 1.4 structure does not. Those two are returned beside the policy.
fn parse_resource_limits(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<(ResourceLimitsQosPolicy, Option<InitialResourceLimits>), XmlError> {
    only_children(
        ctx,
        el,
        &[
            "max_samples",
            "max_instances",
            "max_samples_per_instance",
            "initial_samples",
            "initial_instances",
        ],
    )?;
    // `positiveInteger_UNLIMITED` (§7.2.2.1): 1 or more, or the name.
    let limit = |name: &str| -> Result<i32, XmlError> {
        match numeric_text(el, name) {
            Some(t) => parse_positive_long_unlimited(t),
            None => Ok(LENGTH_UNLIMITED),
        }
    };
    let initial = |name: &str| -> Result<Option<i32>, XmlError> {
        numeric_text(el, name)
            .map(|t| at_least(t, 1, name))
            .transpose()
    };
    let policy = ResourceLimitsQosPolicy {
        max_samples: limit("max_samples")?,
        max_instances: limit("max_instances")?,
        max_samples_per_instance: limit("max_samples_per_instance")?,
    };
    let initial_samples = initial("initial_samples")?;
    let initial_instances = initial("initial_instances")?;
    let initial = (initial_samples.is_some() || initial_instances.is_some()).then_some(
        InitialResourceLimits {
            initial_samples,
            initial_instances,
        },
    );
    Ok((policy, initial))
}

fn parse_entity_factory(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<EntityFactoryQosPolicy, XmlError> {
    only_children(ctx, el, &["autoenable_created_entities"])?;
    let mut policy = EntityFactoryQosPolicy::default();
    if let Some(auto) = el.child("autoenable_created_entities") {
        policy.autoenable_created_entities = parse_bool_strict(&auto.text)?;
    }
    Ok(policy)
}

fn parse_writer_data_lifecycle(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<WriterDataLifecycleQosPolicy, XmlError> {
    only_children(ctx, el, &["autodispose_unregistered_instances"])?;
    let mut policy = WriterDataLifecycleQosPolicy::default();
    if let Some(auto) = el.child("autodispose_unregistered_instances") {
        policy.autodispose_unregistered_instances = parse_bool_strict(&auto.text)?;
    }
    Ok(policy)
}

fn parse_reader_data_lifecycle(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<ReaderDataLifecycleQosPolicy, XmlError> {
    only_children(
        ctx,
        el,
        &[
            "autopurge_nowriter_samples_delay",
            "autopurge_disposed_samples_delay",
        ],
    )?;
    let nowriter = if let Some(c) = el.child("autopurge_nowriter_samples_delay") {
        parse_duration_element(ctx, c)?
    } else {
        QosDuration::INFINITE
    };
    let disposed = if let Some(c) = el.child("autopurge_disposed_samples_delay") {
        parse_duration_element(ctx, c)?
    } else {
        QosDuration::INFINITE
    };
    Ok(ReaderDataLifecycleQosPolicy {
        autopurge_nowriter_samples_delay: nowriter,
        autopurge_disposed_samples_delay: disposed,
    })
}

fn parse_durability_service(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<DurabilityServiceQosPolicy, XmlError> {
    only_children(
        ctx,
        el,
        &[
            "service_cleanup_delay",
            "history_kind",
            "history_depth",
            "max_samples",
            "max_instances",
            "max_samples_per_instance",
        ],
    )?;
    let mut p = DurabilityServiceQosPolicy::default();
    if let Some(c) = el.child("service_cleanup_delay") {
        p.service_cleanup_delay = parse_duration_element(ctx, c)?;
    }
    if let Some(c) = el.child("history_kind") {
        // §7.1.4 Tab.7.1 / §7.2.1.1: the DCPS IDL enumeration literals.
        p.history_kind = match c.text.trim() {
            "KEEP_LAST_HISTORY_QOS" => HistoryKind::KeepLast,
            "KEEP_ALL_HISTORY_QOS" => HistoryKind::KeepAll,
            other => return Err(XmlError::BadEnum(other.to_string())),
        };
    }
    if let Some(t) = numeric_text(el, "history_depth") {
        p.history_depth = at_least(t, 1, "history_depth")?;
    }
    if let Some(t) = numeric_text(el, "max_samples") {
        p.max_samples = parse_positive_long_unlimited(t)?;
    }
    if let Some(t) = numeric_text(el, "max_instances") {
        p.max_instances = parse_positive_long_unlimited(t)?;
    }
    if let Some(t) = numeric_text(el, "max_samples_per_instance") {
        p.max_samples_per_instance = parse_positive_long_unlimited(t)?;
    }
    Ok(p)
}

// ----- Octet-Sequence Helpers (UserData / TopicData / GroupData) -----------

/// Parses the `sequence<octet>` value of a data policy.
///
/// Two sources define the element: the schema of the specification
/// (`dds-xml_qos_definitions_nonamespace.xsd`, `userDataQosPolicy`) has one
/// `<value>` of type `xs:base64Binary`, and §7.2.4.2 of the text tells the
/// element name apart: `<value>` is a comma-separated list of octets in
/// decimal or hexadecimal (`84, 104, 0x71`), `<valueB64>` the same bytes in
/// Base64. All three are read:
///
/// * `<valueB64>` — Base64.
/// * `<value>` — Base64 (the schema), or the list of the text. The list
///   is tried first when the content has the shape of one (digits,
///   `0x` hexadecimal, commas, whitespace) and every octet is in 0..=255;
///   anything else is Base64. A content valid as both, such as `0x41`
///   (one octet, or three bytes of Base64), is the list.
///
/// The `xs:choice` in the text allows at most one of `<value>` and
/// `<valueB64>`; both together are an error. Neither means the empty
/// sequence.
fn parse_octet_value(ctx: &mut Ctx, el: &XmlElement) -> Result<Vec<u8>, XmlError> {
    only_children(ctx, el, &["value", "valueB64"])?;
    let value = el.child("value");
    let value_b64 = el.child("valueB64");
    match (value, value_b64) {
        (Some(_), Some(_)) => Err(XmlError::InvalidXml(format!(
            "<{}> carries both <value> and <valueB64>; §7.2.4.2 defines a choice of one",
            el.name
        ))),
        (Some(v), None) => parse_octet_value_text(&v.text, &el.name),
        (None, Some(b)) => decode_b64_text(&b.text, &el.name, "valueB64"),
        (None, None) => Ok(Vec::new()),
    }
}

fn decode_b64_text(text: &str, policy: &str, element: &str) -> Result<Vec<u8>, XmlError> {
    let raw = text.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    base64_decode(raw)
        .ok_or_else(|| XmlError::ValueOutOfRange(format!("invalid base64 in <{policy}/{element}>")))
}

fn parse_octet_value_text(text: &str, policy: &str) -> Result<Vec<u8>, XmlError> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(Vec::new());
    }
    let list_shaped = t
        .chars()
        .all(|c| c.is_ascii_hexdigit() || matches!(c, 'x' | 'X' | ',') || c.is_whitespace());
    if list_shaped {
        match crate::types::parse_octet_sequence(t) {
            Ok(bytes) => return Ok(bytes),
            Err(list_err) => {
                // Not a list of octets; Base64 may still read it (`1234`).
                return base64_decode(t).ok_or(list_err);
            }
        }
    }
    base64_decode(t).ok_or_else(|| {
        XmlError::ValueOutOfRange(format!(
            "<{policy}/value> is neither a list of octets nor Base64"
        ))
    })
}

fn parse_user_data(ctx: &mut Ctx, el: &XmlElement) -> Result<UserDataQosPolicy, XmlError> {
    Ok(UserDataQosPolicy {
        value: parse_octet_value(ctx, el)?,
    })
}
fn parse_topic_data(ctx: &mut Ctx, el: &XmlElement) -> Result<TopicDataQosPolicy, XmlError> {
    Ok(TopicDataQosPolicy {
        value: parse_octet_value(ctx, el)?,
    })
}
fn parse_group_data(ctx: &mut Ctx, el: &XmlElement) -> Result<GroupDataQosPolicy, XmlError> {
    Ok(GroupDataQosPolicy {
        value: parse_octet_value(ctx, el)?,
    })
}

/// Pure-Rust Base64 decoder with `=` padding tolerance. Duplicated from
/// `crates/security-permissions/src/governance.rs::base64_decode_anchor`
/// to avoid the crate dependency (see the module doc).
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = cleaned.as_bytes();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let chunks = bytes.len() / 4;
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        let mut vals = [0u8; 4];
        let mut pad = 0usize;
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' {
                pad += 1;
                vals[i] = 0;
            } else if pad > 0 {
                return None;
            } else {
                vals[i] = match c {
                    b'A'..=b'Z' => c - b'A',
                    b'a'..=b'z' => c - b'a' + 26,
                    b'0'..=b'9' => c - b'0' + 52,
                    b'+' => 62,
                    b'/' => 63,
                    _ => return None,
                };
            }
        }
        // `xs:base64Binary`: `=` pads only the final group, and by at most
        // two characters.
        if pad > 2 || (pad > 0 && index + 1 != chunks) {
            return None;
        }
        let n = (u32::from(vals[0]) << 18)
            | (u32::from(vals[1]) << 12)
            | (u32::from(vals[2]) << 6)
            | u32::from(vals[3]);
        out.push(((n >> 16) & 0xFF) as u8);
        if pad < 2 {
            out.push(((n >> 8) & 0xFF) as u8);
        }
        if pad < 1 {
            out.push((n & 0xFF) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_bool_strict_accepts_only_lowercase() {
        // §7.1.4 Tab.7.1: "Yes: 1 or true. No: 0 or false. Values are
        // case sensitive." All four spellings are legal.
        assert!(parse_bool_strict("true").unwrap());
        assert!(parse_bool_strict("1").unwrap());
        assert!(!parse_bool_strict("false").unwrap());
        assert!(!parse_bool_strict("0").unwrap());
        // Case variants are not.
        assert!(parse_bool_strict("True").is_err());
        assert!(parse_bool_strict("TRUE").is_err());
        assert!(parse_bool_strict("False").is_err());
        assert!(parse_bool_strict("FALSE").is_err());
        assert!(parse_bool_strict("yes").is_err());
    }

    #[test]
    fn octet_value_is_base64_or_a_list() {
        // The schema's form: Base64 in <value>.
        assert_eq!(parse_octet_value_text("aGk=", "user_data").unwrap(), b"hi");
        assert_eq!(
            parse_octet_value_text("cmF3X2J5dGVz", "user_data").unwrap(),
            b"raw_bytes"
        );
        // The list of §7.2.4.2.1.
        assert_eq!(
            parse_octet_value_text("84, 104, 101, 32, 0x71, 0x75, 0x69, 0x63", "user_data")
                .unwrap(),
            b"The quic"
        );
        // Digits that are no list of octets are Base64 ("1234" = d7 6d f8).
        assert_eq!(
            parse_octet_value_text("1234", "user_data").unwrap(),
            [0xd7, 0x6d, 0xf8]
        );
        // Empty is the empty sequence; garbage is an error.
        assert!(
            parse_octet_value_text("  ", "user_data")
                .unwrap()
                .is_empty()
        );
        assert!(parse_octet_value_text("%%%", "user_data").is_err());
        assert!(parse_octet_value_text("1,,2", "user_data").is_err());
    }

    #[test]
    fn base64_decode_basic() {
        // "raw_bytes" -> base64 = "cmF3X2J5dGVz".
        let v = base64_decode("cmF3X2J5dGVz").expect("decode");
        assert_eq!(v, b"raw_bytes");
    }

    #[test]
    fn base64_decode_with_padding() {
        // "raw" -> "cmF3"
        // "raw_" -> "cmF3Xw=="
        let v = base64_decode("cmF3Xw==").expect("decode");
        assert_eq!(v, b"raw_");
    }

    #[test]
    fn base64_decode_invalid_returns_none() {
        assert!(base64_decode("!!!").is_none());
        // Length not multiple of 4.
        assert!(base64_decode("abc").is_none());
        // Padding belongs to the last group only, and is at most two `=`.
        assert!(base64_decode("cmF3Xw==cmF3").is_none());
        assert!(base64_decode("c===").is_none());
    }

    #[test]
    fn parse_minimal_library() {
        let xml = r#"<?xml version="1.0"?>
<dds>
  <qos_library name="L1">
    <qos_profile name="P1"/>
  </qos_library>
</dds>"#;
        let lib = parse_qos_library(xml).expect("parse");
        assert_eq!(lib.name, "L1");
        assert_eq!(lib.profiles.len(), 1);
        assert_eq!(lib.profiles[0].name, "P1");
        // Spec: no containers.
        let p = &lib.profiles[0];
        assert!(p.datawriter_qos.is_empty());
        assert!(p.datareader_qos.is_empty());
    }

    #[test]
    fn parse_reliability_and_history() {
        let xml = r#"<dds>
  <qos_library name="L">
    <qos_profile name="P">
      <datawriter_qos>
        <reliability>
          <kind>RELIABLE_RELIABILITY_QOS</kind>
          <max_blocking_time><sec>1</sec><nanosec>0</nanosec></max_blocking_time>
        </reliability>
        <history>
          <kind>KEEP_LAST_HISTORY_QOS</kind>
          <depth>10</depth>
        </history>
      </datawriter_qos>
    </qos_profile>
  </qos_library>
</dds>"#;
        let lib = parse_qos_library(xml).expect("parse");
        let dw = &lib.profiles[0].datawriter_qos.first().expect("dw").qos;
        assert_eq!(
            dw.reliability.unwrap().kind,
            zerodds_qos::ReliabilityKind::Reliable
        );
        assert_eq!(dw.history.unwrap().depth, 10);
    }

    /// §7.1.4 Tab.7.1: "Legal values are the ones defined for QoS
    /// Policies in the DCPS IDL." The short spellings that are not IDL
    /// enumeration literals are refused, one per policy kind.
    #[test]
    fn short_enum_spellings_rejected() {
        let cases: &[(&str, &str)] = &[
            ("durability", "VOLATILE"),
            ("reliability", "BEST_EFFORT"),
            ("history", "KEEP_ALL"),
            ("liveliness", "AUTOMATIC"),
            ("destination_order", "BY_SOURCE_TIMESTAMP"),
            ("ownership", "EXCLUSIVE"),
        ];
        for (policy, short) in cases {
            let xml = format!(
                r#"<dds><qos_library name="L"><qos_profile name="P">
                  <datawriter_qos><{policy}><kind>{short}</kind></{policy}></datawriter_qos>
                </qos_profile></qos_library></dds>"#
            );
            let err = parse_qos_libraries(&xml).expect_err("short spelling");
            assert!(
                matches!(err, XmlError::BadEnum(_)),
                "{policy}/{short}: {err:?}"
            );
        }
        // presentation's enum sits in <access_scope>, not <kind>.
        let presentation = r#"<dds><qos_library name="L"><qos_profile name="P">
          <publisher_qos><presentation><access_scope>TOPIC</access_scope></presentation></publisher_qos>
        </qos_profile></qos_library></dds>"#;
        assert!(matches!(
            parse_qos_libraries(presentation),
            Err(XmlError::BadEnum(_))
        ));
        // durability_service spells the same history kind by element name.
        let durability_service = r#"<dds><qos_library name="L"><qos_profile name="P">
          <datawriter_qos><durability_service><history_kind>KEEP_LAST</history_kind></durability_service></datawriter_qos>
        </qos_profile></qos_library></dds>"#;
        assert!(matches!(
            parse_qos_libraries(durability_service),
            Err(XmlError::BadEnum(_))
        ));
    }

    #[test]
    fn rejects_non_dds_root() {
        let xml = r#"<root/>"#;
        let err = parse_qos_libraries(xml).expect_err("non-dds root");
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    #[test]
    fn missing_library_name_rejected() {
        let xml = r#"<dds><qos_library/></dds>"#;
        let err = parse_qos_libraries(xml).expect_err("missing-name");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    #[test]
    fn missing_profile_name_rejected() {
        let xml = r#"<dds><qos_library name="L"><qos_profile/></qos_library></dds>"#;
        let err = parse_qos_libraries(xml).expect_err("missing-name");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    // ---- §7.3.2.4.4 Single-QoS-Profile-Shortcut --------------------

    #[test]
    fn single_qos_shortcut_datawriter_creates_implicit_profile() {
        // Spec §7.3.2.4.4: <datawriter_qos name="..."> directly under
        // <qos_library> is equivalent to <qos_profile><datawriter_qos/>
        // </qos_profile>.
        let xml = r#"<dds>
          <qos_library name="L">
            <datawriter_qos name="ShortcutDW">
              <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
            </datawriter_qos>
          </qos_library>
        </dds>"#;
        let libs = parse_qos_libraries(xml).expect("parse");
        let lib = &libs[0];
        let prof = lib
            .profiles
            .iter()
            .find(|p| p.name == "ShortcutDW")
            .expect("implicit profile");
        assert!(!prof.datawriter_qos.is_empty());
        assert!(prof.datareader_qos.is_empty());
    }

    #[test]
    fn single_qos_shortcut_topic_creates_implicit_profile() {
        let xml = r#"<dds>
          <qos_library name="L">
            <topic_qos name="ShortcutT"/>
          </qos_library>
        </dds>"#;
        let libs = parse_qos_libraries(xml).expect("parse");
        let prof = libs[0]
            .profiles
            .iter()
            .find(|p| p.name == "ShortcutT")
            .expect("topic shortcut");
        assert!(!prof.topic_qos.is_empty());
    }

    #[test]
    fn single_qos_shortcut_without_name_is_kept() {
        // The schema's `qosLibrary` does not require the `name` of a QoS
        // container directly in a library. The container is a profile
        // nothing can refer to, kept under the empty name — not discarded.
        let xml = r#"<dds>
          <qos_library name="L">
            <qos_profile name="Real"/>
            <datawriter_qos><reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability></datawriter_qos>
          </qos_library>
        </dds>"#;
        let libs = parse_qos_libraries(xml).expect("parse");
        assert_eq!(libs[0].profiles.len(), 2);
        assert_eq!(libs[0].profiles[1].name, "");
        assert!(
            libs[0].profiles[1].datawriter_qos[0]
                .qos
                .reliability
                .is_some()
        );
    }

    /// What the parser does not read is refused by name at every level:
    /// an unknown policy, a misspelt policy child, an unknown profile
    /// child. Each used to leave the default in place without a word.
    #[test]
    fn unread_elements_are_refused_at_every_level() {
        let wrap = |inner: &str| {
            format!(
                r#"<dds><qos_library name="L"><qos_profile name="P">{inner}</qos_profile></qos_library></dds>"#
            )
        };
        for (inner, needle) in [
            (
                "<datawriter_qos><data_representation><value>XCDR2_DATA_REPRESENTATION</value></data_representation></datawriter_qos>",
                "datawriter_qos/data_representation",
            ),
            (
                "<datawriter_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind><max_blocking_tme><sec>1</sec></max_blocking_tme></reliability></datawriter_qos>",
                "reliability/max_blocking_tme",
            ),
            (
                "<datawriter_qos><deadline><period><sec>1</sec><nanosecs>0</nanosecs></period></deadline></datawriter_qos>",
                "period/nanosecs",
            ),
            ("<property/>", "qos_profile[P]/property"),
        ] {
            let err = parse_qos_libraries(&wrap(inner)).unwrap_err().to_string();
            assert!(err.contains(needle), "{needle}: {err}");
        }
    }

    #[test]
    fn single_qos_shortcut_multiple_kinds_in_same_library() {
        // Multiple shortcut kinds can co-exist.
        let xml = r#"<dds>
          <qos_library name="L">
            <datawriter_qos name="DW"/>
            <datareader_qos name="DR"/>
            <publisher_qos name="P"/>
          </qos_library>
        </dds>"#;
        let libs = parse_qos_libraries(xml).expect("parse");
        assert_eq!(libs[0].profiles.len(), 3);
        let names: alloc::collections::BTreeSet<&str> =
            libs[0].profiles.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains("DW"));
        assert!(names.contains("DR"));
        assert!(names.contains("P"));
    }

    #[test]
    fn boolean_case_sensitive_rejected() {
        // §7.1.4: only `true`/`false`. `True` in entity_factory.
        let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
          <domain_participant_qos>
            <entity_factory><autoenable_created_entities>True</autoenable_created_entities></entity_factory>
          </domain_participant_qos>
        </qos_profile></qos_library></dds>"#;
        let err = parse_qos_libraries(xml).expect_err("strict-bool");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn duration_inline_infinity_sentinel() {
        let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
          <datawriter_qos>
            <deadline><period><sec>DURATION_INFINITY</sec></period></deadline>
          </datawriter_qos>
        </qos_profile></qos_library></dds>"#;
        let lib = parse_qos_library(xml).expect("parse");
        let dw = &lib.profiles[0].datawriter_qos.first().expect("dw").qos;
        let p = dw.deadline.unwrap().period;
        assert!(p.is_infinite());
    }

    /// The schema gives every QoS container `base_name`. In a profile it is
    /// read with the container (and resolved with the profile, see
    /// `qos_inheritance`); the shortcut form's `base_name` is the
    /// profile's (§7.3.2.4.4).
    #[test]
    fn base_name_on_a_container_is_read() {
        let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
          <datawriter_qos base_name="Other"/>
        </qos_profile></qos_library></dds>"#;
        let libs = parse_qos_libraries(xml).expect("container base_name");
        assert_eq!(
            libs[0].profiles[0].datawriter_qos[0]
                .qos
                .base_name
                .as_deref(),
            Some("Other")
        );

        let shortcut = r#"<dds><qos_library name="L">
          <qos_profile name="Base"/>
          <datawriter_qos name="Derived" base_name="Base"/>
        </qos_library></dds>"#;
        let libs = parse_qos_libraries(shortcut).expect("shortcut");
        let derived = libs[0]
            .profiles
            .iter()
            .find(|p| p.name == "Derived")
            .unwrap();
        assert_eq!(derived.base_name.as_deref(), Some("Base"));
        assert!(derived.datawriter_qos[0].qos.base_name.is_none());
    }

    /// §7.3.2.4.2 "defined before" is about document order, and a shortcut
    /// is a profile: one written before the profile that inherits it is a
    /// legal base, and one written after is not.
    #[test]
    fn shortcut_and_profile_keep_document_order() {
        use crate::qos_inheritance::resolve_profile;
        let before = r#"<dds><qos_library name="L">
          <datawriter_qos name="S"><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos>
          <qos_profile name="D" base_name="S"/>
        </qos_library></dds>"#;
        let libs = parse_qos_libraries(before).expect("parse");
        assert_eq!(libs[0].profiles[0].name, "S");
        assert_eq!(libs[0].profiles[1].name, "D");
        let r = resolve_profile(&libs, "L::D").expect("S is defined before D");
        assert!(r.datawriter_qos[0].qos.history.is_some());

        let after = r#"<dds><qos_library name="L">
          <qos_profile name="D" base_name="S"/>
          <datawriter_qos name="S"/>
        </qos_library></dds>"#;
        let libs = parse_qos_libraries(after).expect("parse");
        assert!(matches!(
            resolve_profile(&libs, "L::D"),
            Err(XmlError::InvalidXml(_))
        ));
    }

    /// A structure element holds child elements (§7.2.3). Text next to
    /// them, or text where the sequence form `<name><element>…` belongs,
    /// would be dropped; both are refused.
    #[test]
    fn text_where_fields_belong_is_refused() {
        let wrap = |inner: &str| {
            format!(
                r#"<dds><qos_library name="L"><qos_profile name="P">{inner}</qos_profile></qos_library></dds>"#
            )
        };
        for inner in [
            "<publisher_qos><partition><name>g1</name></partition></publisher_qos>",
            "<datawriter_qos><history>KEEP_ALL<kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos>",
            "<datawriter_qos><deadline>soon</deadline></datawriter_qos>",
            "<datawriter_qos><deadline><period>5<sec>1</sec></period></deadline></datawriter_qos>",
        ] {
            let err = parse_qos_libraries(&wrap(inner)).unwrap_err();
            assert!(matches!(err, XmlError::InvalidXml(_)), "{inner}: {err:?}");
        }
        // The sentinel text of a Duration_t is still its own form, and
        // layout whitespace around child elements is no text.
        let ok = wrap(
            "<datawriter_qos>
               <deadline>
                 <period><sec>DURATION_INFINITY</sec></period>
               </deadline>
             </datawriter_qos>",
        );
        let libs = parse_qos_libraries(&ok).expect("sentinel and layout");
        let dw = &libs[0].profiles[0].datawriter_qos[0].qos;
        assert!(dw.deadline.unwrap().period.is_infinite());
    }

    #[test]
    fn vendor_topic_filter_element_ignores_layout_whitespace() {
        let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
          <topic_filter>
            Sensor*
          </topic_filter>
        </qos_profile></qos_library></dds>"#;
        let lib = parse_qos_libraries_with(xml, &ReadOptions::accept_vendor_forms())
            .expect("parse")
            .value
            .remove(0);
        assert_eq!(lib.profiles[0].topic_filter.as_deref(), Some("Sensor*"));
    }

    #[test]
    fn dtd_declaration_accepted_in_qos_context() {
        // §7.1.1: a DTD is part of a well-formed [XML] document. The
        // declaration alone changes nothing; an external entity is not
        // fetched and errors only if referenced.
        let xml = r#"<?xml version="1.0"?>
<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<dds><qos_library name="L"/></dds>"#;
        let libs = parse_qos_libraries(xml).expect("dtd");
        assert_eq!(libs[0].name, "L");
    }
}
