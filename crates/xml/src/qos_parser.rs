// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Parser fuer DDS-XML 1.0 §7.3.2 QoS-Profile-Library.
//!
//! Konsumiert Roh-XML, baut den Foundation-Tree (siehe
//! [`crate::parser::parse_xml_tree`]) auf und mappt ihn auf das typed
//! Datenmodell aus [`crate::qos`].
//!
//! # Spec-konforme Boolean-Semantik
//!
//! Der Foundation-`parse_bool` aus [`crate::types`] akzeptiert bewusst
//! `TRUE`/`FALSE`-Schreibweisen aus Cyclone/FastDDS-Kompatibilitaet.
//! DDS-XML 1.0 §7.1.4 verlangt dagegen **strict** `true`/`false`. Wir
//! verwenden hier den lokalen [`parse_bool_strict`] Helper, der nur die
//! Spec-Werte zulaesst.
//!
//! # Single-QoS-Shortcut (§7.3.2.4.1)
//!
//! Ein `<qos_profile>` mit nur einem `<datawriter_qos>` (oder einem
//! anderen 6 Entity-Containern) wird wie eine voll-spezifizierte Profile
//! behandelt — der direkte Container-Tag ersetzt den Container-fuer-
//! mehrere-Policies-Stil. Andere Entity-Container in derselben Profile
//! werden zusaetzlich akzeptiert; das ist ein Superset des Spec-
//! Examples in §7.3.2.4.1.

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
use crate::parser::{XmlElement, parse_xml_tree};
use crate::qos::{EntityQos, QosLibrary, QosProfile};
use crate::types::{
    DURATION_INFINITE_NSEC, DURATION_INFINITE_SEC, LENGTH_UNLIMITED, parse_duration_nsec,
    parse_duration_sec, parse_long,
};

/// Parsed das oberste `<dds>`-Dokument und gibt **alle** enthaltenen
/// QoS-Libraries zurueck (Spec §7.3.2.3 erlaubt mehrere Libraries pro
/// Dokument).
///
/// # Errors
/// * [`XmlError::InvalidXml`] — XML nicht wohlgeformt oder keine
///   `<dds>`-Wurzel.
/// * Weitere Fehler aus `parse_qos_library_element` durchgereicht.
pub fn parse_qos_libraries(xml: &str) -> Result<Vec<QosLibrary>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    if doc.root.name != "dds" {
        return Err(XmlError::InvalidXml(format!(
            "expected <dds> root, got <{}>",
            doc.root.name
        )));
    }
    let mut libs = Vec::new();
    for lib_node in doc.root.children_named("qos_library") {
        libs.push(parse_qos_library_element(lib_node)?);
    }
    Ok(libs)
}

/// Convenience: parsed das Dokument und gibt die **erste** QoS-Library
/// zurueck. Liefert `MissingRequiredElement("qos_library")` wenn keine
/// vorhanden.
///
/// # Errors
/// Wie [`parse_qos_libraries`], plus `MissingRequiredElement` wenn das
/// Dokument keine Library enthaelt.
pub fn parse_qos_library(xml: &str) -> Result<QosLibrary, XmlError> {
    parse_qos_libraries(xml)?
        .into_iter()
        .next()
        .ok_or_else(|| XmlError::MissingRequiredElement("qos_library".into()))
}

/// Crate-internal `pub`-Wrapper fuer [`parse_qos_library_element`], damit
/// der Top-Level-Loader (`crate::zerodds_xml::parse_dds_xml`) eine
/// `<qos_library>`-Element-Sub-Tree direkt dekodieren kann ohne den
/// Parser zu duplizieren.
///
/// # Errors
/// Siehe `parse_qos_library_element`.
pub fn parse_qos_library_element_public(el: &XmlElement) -> Result<QosLibrary, XmlError> {
    parse_qos_library_element(el)
}

fn parse_qos_library_element(el: &XmlElement) -> Result<QosLibrary, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("qos_library@name".into()))?
        .to_string();
    let mut profiles = Vec::new();
    for prof_node in el.children_named("qos_profile") {
        profiles.push(parse_qos_profile_element(prof_node)?);
    }
    // Spec §7.3.2.4.4: "The definition of an individual QoS is a
    // shortcut for defining a QoS profile with a single QoS." Wir
    // erkennen `<datawriter_qos name="…">` etc. direkt unter
    // `<qos_library>` und wickeln sie in einen impliziten
    // qos_profile-Wrapper.
    for child in &el.children {
        if let Some(profile) = parse_single_qos_shortcut(child)? {
            profiles.push(profile);
        }
    }
    Ok(QosLibrary { name, profiles })
}

/// Spec §7.3.2.4.4 Single-QoS-Shortcut — wenn ein `<datawriter_qos>`
/// (oder `_reader`/`_topic`/`_publisher`/`_subscriber`/
/// `_participant`) direkt unter `<qos_library>` steht UND ein
/// `name`-Attribut traegt, ist das Aequivalent zu einem
/// `<qos_profile name="…">` mit genau einer QoS.
///
/// Liefert `None` wenn das Element kein Shortcut ist (z.B. ein
/// regulaeres `<qos_profile>`-Child oder ein qos-Element ohne `name`).
fn parse_single_qos_shortcut(el: &XmlElement) -> Result<Option<QosProfile>, XmlError> {
    let name = match el.attribute("name") {
        Some(n) => n.to_string(),
        None => return Ok(None),
    };
    let base_name = el.attribute("base_name").map(ToString::to_string);
    let mut profile = QosProfile {
        name,
        base_name,
        ..QosProfile::default()
    };
    let qos = parse_entity_qos(el)?;
    match el.name.as_str() {
        "datawriter_qos" => profile.datawriter_qos = Some(qos),
        "datareader_qos" => profile.datareader_qos = Some(qos),
        "topic_qos" => profile.topic_qos = Some(qos),
        "publisher_qos" => profile.publisher_qos = Some(qos),
        "subscriber_qos" => profile.subscriber_qos = Some(qos),
        "domainparticipant_qos" | "participant_qos" => {
            profile.domainparticipant_qos = Some(qos);
        }
        _ => return Ok(None),
    }
    Ok(Some(profile))
}

fn parse_qos_profile_element(el: &XmlElement) -> Result<QosProfile, XmlError> {
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
        match child.name.as_str() {
            "topic_filter" => {
                profile.topic_filter = Some(child.text.clone());
            }
            "datawriter_qos" => {
                profile.datawriter_qos = Some(parse_entity_qos(child)?);
            }
            "datareader_qos" => {
                profile.datareader_qos = Some(parse_entity_qos(child)?);
            }
            "topic_qos" => {
                profile.topic_qos = Some(parse_entity_qos(child)?);
            }
            "publisher_qos" => {
                profile.publisher_qos = Some(parse_entity_qos(child)?);
            }
            "subscriber_qos" => {
                profile.subscriber_qos = Some(parse_entity_qos(child)?);
            }
            "domainparticipant_qos" => {
                profile.domainparticipant_qos = Some(parse_entity_qos(child)?);
            }
            // Unbekannte Elements werden im "lax"-Modus ignoriert
            // (Spec §7.1.4 erlaubt "lax" als Lese-Strategie); strenge
            // Validation kann auf Aufrufer-Seite via dedizierter
            // Whitelist nachgezogen werden.
            _ => {}
        }
    }
    Ok(profile)
}

/// Crate-internal `pub`-Wrapper fuer [`parse_entity_qos`], damit andere
/// Building-Block-Module (Domain, Participant) inline-QoS-Container
/// dekodieren koennen ohne den Parser zu duplizieren.
///
/// # Errors
/// Siehe `parse_entity_qos`.
pub fn parse_entity_qos_public(el: &XmlElement) -> Result<EntityQos, XmlError> {
    parse_entity_qos(el)
}

fn parse_entity_qos(el: &XmlElement) -> Result<EntityQos, XmlError> {
    let mut q = EntityQos::default();
    for child in &el.children {
        match child.name.as_str() {
            "durability" => q.durability = Some(parse_durability(child)?),
            "durability_service" => q.durability_service = Some(parse_durability_service(child)?),
            "presentation" => q.presentation = Some(parse_presentation(child)?),
            "deadline" => q.deadline = Some(parse_deadline(child)?),
            "latency_budget" => q.latency_budget = Some(parse_latency_budget(child)?),
            "ownership" => q.ownership = Some(parse_ownership(child)?),
            "ownership_strength" => q.ownership_strength = Some(parse_ownership_strength(child)?),
            "liveliness" => q.liveliness = Some(parse_liveliness(child)?),
            "time_based_filter" => q.time_based_filter = Some(parse_time_based_filter(child)?),
            "partition" => q.partition = Some(parse_partition(child)?),
            "reliability" => q.reliability = Some(parse_reliability(child)?),
            "transport_priority" => q.transport_priority = Some(parse_transport_priority(child)?),
            "lifespan" => q.lifespan = Some(parse_lifespan(child)?),
            "destination_order" => q.destination_order = Some(parse_destination_order(child)?),
            "history" => q.history = Some(parse_history(child)?),
            "resource_limits" => q.resource_limits = Some(parse_resource_limits(child)?),
            "entity_factory" => q.entity_factory = Some(parse_entity_factory(child)?),
            "writer_data_lifecycle" => {
                q.writer_data_lifecycle = Some(parse_writer_data_lifecycle(child)?);
            }
            "reader_data_lifecycle" => {
                q.reader_data_lifecycle = Some(parse_reader_data_lifecycle(child)?);
            }
            "user_data" => q.user_data = Some(parse_user_data(child)?),
            "topic_data" => q.topic_data = Some(parse_topic_data(child)?),
            "group_data" => q.group_data = Some(parse_group_data(child)?),
            // Unbekannte Children: lax (siehe `parse_qos_profile_element`).
            _ => {}
        }
    }
    Ok(q)
}

// ============================================================================
// Spec-strikter Boolean-Parser
// ============================================================================

/// DDS-XML 1.0 §7.1.4 verlangt strict `true`/`false` (case-sensitive).
///
/// # Errors
/// [`XmlError::ValueOutOfRange`] bei jeder anderen Schreibweise (auch
/// `True`, `1`, `yes`).
pub fn parse_bool_strict(s: &str) -> Result<bool, XmlError> {
    let t = s.trim();
    match t {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(XmlError::ValueOutOfRange(format!(
            "DDS-XML boolean must be `true` or `false` (case-sensitive), got `{t}`"
        ))),
    }
}

// ============================================================================
// Duration-Helper: XML-`<sec>/<nanosec>` ↔ `zerodds_qos::Duration`
// ============================================================================

/// Konvertiert eine XML-`Duration_t`-Repraesentation
/// (`<sec>` + optional `<nanosec>`, oder Inline-Text `DURATION_INFINITY`)
/// zu einem `zerodds_qos::Duration` (i32 seconds + u32 fraction).
///
/// Akzeptierte XML-Formate:
/// 1. `<duration><sec>1</sec><nanosec>0</nanosec></duration>` — voll.
/// 2. `<duration><sec>DURATION_INFINITY</sec></duration>` — Sentinel
///    (mappt auf [`QosDuration::INFINITE`]).
/// 3. `<duration>DURATION_INFINITY</duration>` — Inline-Text-Sentinel.
fn parse_duration_element(el: &XmlElement) -> Result<QosDuration, XmlError> {
    // Inline-Sentinel-Text (Form 3).
    let trimmed = el.text.trim();
    if !trimmed.is_empty() && el.children.is_empty() {
        return parse_duration_text_sentinel(trimmed);
    }

    let sec_el = el
        .child("sec")
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("{}/sec", el.name)))?;
    let sec_str = sec_el.text.trim();
    let sec = parse_duration_sec(sec_str)?;

    // nanosec optional; default 0.
    let nsec = if let Some(n_el) = el.child("nanosec") {
        parse_duration_nsec(n_el.text.trim())?
    } else {
        0
    };

    // Spec-Sentinel-Mapping: wenn der `<sec>`-Wert die Spec-Sentinel
    // `DURATION_INFINITE_SEC` (= 0x7FFFFFFF) traegt, mappen wir auf
    // `QosDuration::INFINITE` (`{i32::MAX, u32::MAX}`). Das ist die
    // Wire-Konvention aus DDSI-RTPS §9.3.2 und behandelt sowohl
    // `<sec>DURATION_INFINITY</sec>` (mit oder ohne `<nanosec>`) als
    // auch `<sec>2147483647</sec><nanosec>2147483647</nanosec>` korrekt.
    if sec == DURATION_INFINITE_SEC
        && (nsec == DURATION_INFINITE_NSEC || el.child("nanosec").is_none())
    {
        return Ok(QosDuration::INFINITE);
    }

    // Regulaere Conversion: nanosec → fraction = nsec * 2^32 / 1e9.
    // Wir berechnen in u64 um Overflow zu vermeiden.
    let fraction = (u64::from(nsec) * (1u64 << 32) / 1_000_000_000) as u32;
    Ok(QosDuration {
        seconds: sec,
        fraction,
    })
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

fn parse_kind_text(el: &XmlElement) -> Result<&str, XmlError> {
    let kind = el
        .child("kind")
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("{}/kind", el.name)))?;
    Ok(kind.text.trim())
}

fn parse_durability(el: &XmlElement) -> Result<DurabilityQosPolicy, XmlError> {
    let kind_str = parse_kind_text(el)?;
    let kind = match kind_str {
        // DDS-XML 1.0 §7.3.2 erlaubt sowohl die kurze IDL-Form als auch
        // die lange `_DURABILITY_QOS`-Suffix-Form aus DCPS-Spec.
        "VOLATILE" | "VOLATILE_DURABILITY_QOS" => DurabilityKind::Volatile,
        "TRANSIENT_LOCAL" | "TRANSIENT_LOCAL_DURABILITY_QOS" => DurabilityKind::TransientLocal,
        "TRANSIENT" | "TRANSIENT_DURABILITY_QOS" => DurabilityKind::Transient,
        "PERSISTENT" | "PERSISTENT_DURABILITY_QOS" => DurabilityKind::Persistent,
        other => return Err(XmlError::BadEnum(other.to_string())),
    };
    Ok(DurabilityQosPolicy { kind })
}

fn parse_history(el: &XmlElement) -> Result<HistoryQosPolicy, XmlError> {
    let kind_str = parse_kind_text(el).unwrap_or("");
    let kind = match kind_str {
        "" | "KEEP_LAST" | "KEEP_LAST_HISTORY_QOS" => HistoryKind::KeepLast,
        "KEEP_ALL" | "KEEP_ALL_HISTORY_QOS" => HistoryKind::KeepAll,
        other => return Err(XmlError::BadEnum(other.to_string())),
    };
    let depth = if let Some(d) = el.child("depth") {
        parse_long(d.text.trim())?
    } else {
        // Spec-Default §2.2.3.17.3: 1.
        1
    };
    Ok(HistoryQosPolicy { kind, depth })
}

fn parse_reliability(el: &XmlElement) -> Result<ReliabilityQosPolicy, XmlError> {
    let kind_str = parse_kind_text(el)?;
    let kind = match kind_str {
        "BEST_EFFORT" | "BEST_EFFORT_RELIABILITY_QOS" => ReliabilityKind::BestEffort,
        "RELIABLE" | "RELIABLE_RELIABILITY_QOS" => ReliabilityKind::Reliable,
        other => return Err(XmlError::BadEnum(other.to_string())),
    };
    let max_blocking_time = if let Some(mbt) = el.child("max_blocking_time") {
        parse_duration_element(mbt)?
    } else {
        QosDuration::from_millis(100)
    };
    Ok(ReliabilityQosPolicy {
        kind,
        max_blocking_time,
    })
}

fn parse_deadline(el: &XmlElement) -> Result<DeadlineQosPolicy, XmlError> {
    let period = el
        .child("period")
        .ok_or_else(|| XmlError::MissingRequiredElement("deadline/period".into()))?;
    Ok(DeadlineQosPolicy {
        period: parse_duration_element(period)?,
    })
}

fn parse_latency_budget(el: &XmlElement) -> Result<LatencyBudgetQosPolicy, XmlError> {
    let dur = el
        .child("duration")
        .ok_or_else(|| XmlError::MissingRequiredElement("latency_budget/duration".into()))?;
    Ok(LatencyBudgetQosPolicy {
        duration: parse_duration_element(dur)?,
    })
}

fn parse_lifespan(el: &XmlElement) -> Result<LifespanQosPolicy, XmlError> {
    let dur = el
        .child("duration")
        .ok_or_else(|| XmlError::MissingRequiredElement("lifespan/duration".into()))?;
    Ok(LifespanQosPolicy {
        duration: parse_duration_element(dur)?,
    })
}

fn parse_time_based_filter(el: &XmlElement) -> Result<TimeBasedFilterQosPolicy, XmlError> {
    let sep = el.child("minimum_separation").ok_or_else(|| {
        XmlError::MissingRequiredElement("time_based_filter/minimum_separation".into())
    })?;
    Ok(TimeBasedFilterQosPolicy {
        minimum_separation: parse_duration_element(sep)?,
    })
}

fn parse_liveliness(el: &XmlElement) -> Result<LivelinessQosPolicy, XmlError> {
    let kind = if let Some(k) = el.child("kind") {
        match k.text.trim() {
            "AUTOMATIC" | "AUTOMATIC_LIVELINESS_QOS" => LivelinessKind::Automatic,
            "MANUAL_BY_PARTICIPANT" | "MANUAL_BY_PARTICIPANT_LIVELINESS_QOS" => {
                LivelinessKind::ManualByParticipant
            }
            "MANUAL_BY_TOPIC" | "MANUAL_BY_TOPIC_LIVELINESS_QOS" => LivelinessKind::ManualByTopic,
            other => return Err(XmlError::BadEnum(other.to_string())),
        }
    } else {
        LivelinessKind::Automatic
    };
    let lease_duration = if let Some(ld) = el.child("lease_duration") {
        parse_duration_element(ld)?
    } else {
        QosDuration::INFINITE
    };
    Ok(LivelinessQosPolicy {
        kind,
        lease_duration,
    })
}

fn parse_destination_order(el: &XmlElement) -> Result<DestinationOrderQosPolicy, XmlError> {
    let kind_str = parse_kind_text(el)?;
    let kind = match kind_str {
        "BY_RECEPTION_TIMESTAMP" | "BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS" => {
            DestinationOrderKind::ByReceptionTimestamp
        }
        "BY_SOURCE_TIMESTAMP" | "BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS" => {
            DestinationOrderKind::BySourceTimestamp
        }
        other => return Err(XmlError::BadEnum(other.to_string())),
    };
    Ok(DestinationOrderQosPolicy { kind })
}

fn parse_ownership(el: &XmlElement) -> Result<OwnershipQosPolicy, XmlError> {
    let kind_str = parse_kind_text(el)?;
    let kind = match kind_str {
        "SHARED" | "SHARED_OWNERSHIP_QOS" => OwnershipKind::Shared,
        "EXCLUSIVE" | "EXCLUSIVE_OWNERSHIP_QOS" => OwnershipKind::Exclusive,
        other => return Err(XmlError::BadEnum(other.to_string())),
    };
    Ok(OwnershipQosPolicy { kind })
}

fn parse_ownership_strength(el: &XmlElement) -> Result<OwnershipStrengthQosPolicy, XmlError> {
    let val = el
        .child("value")
        .ok_or_else(|| XmlError::MissingRequiredElement("ownership_strength/value".into()))?;
    Ok(OwnershipStrengthQosPolicy {
        value: parse_long(val.text.trim())?,
    })
}

fn parse_transport_priority(el: &XmlElement) -> Result<TransportPriorityQosPolicy, XmlError> {
    let val = el
        .child("value")
        .ok_or_else(|| XmlError::MissingRequiredElement("transport_priority/value".into()))?;
    Ok(TransportPriorityQosPolicy {
        value: parse_long(val.text.trim())?,
    })
}

fn parse_presentation(el: &XmlElement) -> Result<PresentationQosPolicy, XmlError> {
    let access_scope = if let Some(s) = el.child("access_scope") {
        match s.text.trim() {
            "INSTANCE" | "INSTANCE_PRESENTATION_QOS" => PresentationAccessScope::Instance,
            "TOPIC" | "TOPIC_PRESENTATION_QOS" => PresentationAccessScope::Topic,
            "GROUP" | "GROUP_PRESENTATION_QOS" => PresentationAccessScope::Group,
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

fn parse_partition(el: &XmlElement) -> Result<PartitionQosPolicy, XmlError> {
    // §7.3.2: <partition><name>g1</name><name>g2</name></partition>.
    // Ein optionales `<name_list>` mit Komma-separierten Eintraegen wird
    // zusaetzlich akzeptiert (Cyclone-/FastDDS-Konvention).
    let mut names: Vec<String> = Vec::new();
    for child in &el.children {
        match child.name.as_str() {
            "name" => names.push(child.text.clone()),
            "name_list" => {
                for tok in child.text.split(',') {
                    let t = tok.trim();
                    if !t.is_empty() {
                        names.push(t.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(PartitionQosPolicy { names })
}

fn parse_resource_limits(el: &XmlElement) -> Result<ResourceLimitsQosPolicy, XmlError> {
    let max_samples = if let Some(c) = el.child("max_samples") {
        parse_long(c.text.trim())?
    } else {
        LENGTH_UNLIMITED
    };
    let max_instances = if let Some(c) = el.child("max_instances") {
        parse_long(c.text.trim())?
    } else {
        LENGTH_UNLIMITED
    };
    let max_samples_per_instance = if let Some(c) = el.child("max_samples_per_instance") {
        parse_long(c.text.trim())?
    } else {
        LENGTH_UNLIMITED
    };
    Ok(ResourceLimitsQosPolicy {
        max_samples,
        max_instances,
        max_samples_per_instance,
    })
}

fn parse_entity_factory(el: &XmlElement) -> Result<EntityFactoryQosPolicy, XmlError> {
    let auto = el.child("autoenable_created_entities").ok_or_else(|| {
        XmlError::MissingRequiredElement("entity_factory/autoenable_created_entities".into())
    })?;
    Ok(EntityFactoryQosPolicy {
        autoenable_created_entities: parse_bool_strict(&auto.text)?,
    })
}

fn parse_writer_data_lifecycle(el: &XmlElement) -> Result<WriterDataLifecycleQosPolicy, XmlError> {
    let auto = el
        .child("autodispose_unregistered_instances")
        .ok_or_else(|| {
            XmlError::MissingRequiredElement(
                "writer_data_lifecycle/autodispose_unregistered_instances".into(),
            )
        })?;
    Ok(WriterDataLifecycleQosPolicy {
        autodispose_unregistered_instances: parse_bool_strict(&auto.text)?,
    })
}

fn parse_reader_data_lifecycle(el: &XmlElement) -> Result<ReaderDataLifecycleQosPolicy, XmlError> {
    let nowriter = if let Some(c) = el.child("autopurge_nowriter_samples_delay") {
        parse_duration_element(c)?
    } else {
        QosDuration::INFINITE
    };
    let disposed = if let Some(c) = el.child("autopurge_disposed_samples_delay") {
        parse_duration_element(c)?
    } else {
        QosDuration::INFINITE
    };
    Ok(ReaderDataLifecycleQosPolicy {
        autopurge_nowriter_samples_delay: nowriter,
        autopurge_disposed_samples_delay: disposed,
    })
}

fn parse_durability_service(el: &XmlElement) -> Result<DurabilityServiceQosPolicy, XmlError> {
    let mut p = DurabilityServiceQosPolicy::default();
    if let Some(c) = el.child("service_cleanup_delay") {
        p.service_cleanup_delay = parse_duration_element(c)?;
    }
    if let Some(c) = el.child("history_kind") {
        p.history_kind = match c.text.trim() {
            "KEEP_LAST" | "KEEP_LAST_HISTORY_QOS" => HistoryKind::KeepLast,
            "KEEP_ALL" | "KEEP_ALL_HISTORY_QOS" => HistoryKind::KeepAll,
            other => return Err(XmlError::BadEnum(other.to_string())),
        };
    }
    if let Some(c) = el.child("history_depth") {
        p.history_depth = parse_long(c.text.trim())?;
    }
    if let Some(c) = el.child("max_samples") {
        p.max_samples = parse_long(c.text.trim())?;
    }
    if let Some(c) = el.child("max_instances") {
        p.max_instances = parse_long(c.text.trim())?;
    }
    if let Some(c) = el.child("max_samples_per_instance") {
        p.max_samples_per_instance = parse_long(c.text.trim())?;
    }
    Ok(p)
}

// ----- Octet-Sequence Helpers (UserData / TopicData / GroupData) -----------

/// Parsed `<value>…</value>` als Octet-Sequence (Base64). Spec §7.2.4
/// erlaubt Base64 als kanonische Form fuer `sequence<octet>`.
fn parse_octet_value(el: &XmlElement) -> Result<Vec<u8>, XmlError> {
    let v = el
        .child("value")
        .ok_or_else(|| XmlError::MissingRequiredElement(format!("{}/value", el.name)))?;
    let raw = v.text.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    base64_decode(raw)
        .ok_or_else(|| XmlError::ValueOutOfRange(format!("invalid base64 in <{}>", el.name)))
}

fn parse_user_data(el: &XmlElement) -> Result<UserDataQosPolicy, XmlError> {
    Ok(UserDataQosPolicy {
        value: parse_octet_value(el)?,
    })
}
fn parse_topic_data(el: &XmlElement) -> Result<TopicDataQosPolicy, XmlError> {
    Ok(TopicDataQosPolicy {
        value: parse_octet_value(el)?,
    })
}
fn parse_group_data(el: &XmlElement) -> Result<GroupDataQosPolicy, XmlError> {
    Ok(GroupDataQosPolicy {
        value: parse_octet_value(el)?,
    })
}

/// Pure-Rust Base64-Decoder mit `=`-Padding-Toleranz. Dupliziert aus
/// `crates/security-permissions/src/governance.rs::base64_decode_anchor`
/// um die Crate-Dep zu vermeiden (siehe Modul-Doc).
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = cleaned.as_bytes();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
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
        assert!(parse_bool_strict("true").unwrap());
        assert!(!parse_bool_strict("false").unwrap());
        assert!(parse_bool_strict("True").is_err());
        assert!(parse_bool_strict("TRUE").is_err());
        assert!(parse_bool_strict("1").is_err());
        assert!(parse_bool_strict("yes").is_err());
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
        // Spec: alle Container None.
        let p = &lib.profiles[0];
        assert!(p.datawriter_qos.is_none());
        assert!(p.datareader_qos.is_none());
    }

    #[test]
    fn parse_reliability_and_history() {
        let xml = r#"<dds>
  <qos_library name="L">
    <qos_profile name="P">
      <datawriter_qos>
        <reliability>
          <kind>RELIABLE</kind>
          <max_blocking_time><sec>1</sec><nanosec>0</nanosec></max_blocking_time>
        </reliability>
        <history>
          <kind>KEEP_LAST</kind>
          <depth>10</depth>
        </history>
      </datawriter_qos>
    </qos_profile>
  </qos_library>
</dds>"#;
        let lib = parse_qos_library(xml).expect("parse");
        let dw = lib.profiles[0].datawriter_qos.as_ref().expect("dw");
        assert_eq!(
            dw.reliability.unwrap().kind,
            zerodds_qos::ReliabilityKind::Reliable
        );
        assert_eq!(dw.history.unwrap().depth, 10);
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
        // Spec §7.3.2.4.4: <datawriter_qos name="..."> direkt unter
        // <qos_library> ist Aequivalent zu <qos_profile><datawriter_qos/>
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
        assert!(prof.datawriter_qos.is_some());
        assert!(prof.datareader_qos.is_none());
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
        assert!(prof.topic_qos.is_some());
    }

    #[test]
    fn single_qos_shortcut_without_name_is_ignored() {
        // Spec verlangt name-Attribut. Ohne `name` waere das kein
        // Shortcut → wird verworfen (kein Crash).
        let xml = r#"<dds>
          <qos_library name="L">
            <qos_profile name="Real"/>
            <datawriter_qos><reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability></datawriter_qos>
          </qos_library>
        </dds>"#;
        let libs = parse_qos_libraries(xml).expect("parse");
        // Nur das echte qos_profile zaehlt — kein impliziter Wrapper.
        assert_eq!(libs[0].profiles.len(), 1);
        assert_eq!(libs[0].profiles[0].name, "Real");
    }

    #[test]
    fn single_qos_shortcut_multiple_kinds_in_same_library() {
        // Mehrere Shortcut-Kinds koennen ko-existieren.
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
          <domainparticipant_qos>
            <entity_factory><autoenable_created_entities>True</autoenable_created_entities></entity_factory>
          </domainparticipant_qos>
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
        let dw = lib.profiles[0].datawriter_qos.as_ref().expect("dw");
        let p = dw.deadline.unwrap().period;
        assert!(p.is_infinite());
    }

    #[test]
    fn dtd_rejected_in_qos_context() {
        let xml = r#"<?xml version="1.0"?>
<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<dds><qos_library name="L"/></dds>"#;
        let err = parse_qos_libraries(xml).expect_err("dtd");
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }
}
