// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Inheritance resolver for QoS profiles per DDS-XML 1.0 §7.3.2.4.2.
//!
//! Override semantics: a child profile with `base_name="parent"` inherits all
//! policies from the parent; each explicitly set policy in the child overrides
//! the inherited one. Multi-level inheritance (grandparent chain) is realized
//! via the foundation `resolve_chain` helper (with cycle detection and a
//! depth cap).
//!
//! Lookup paths: a `base_name` may carry either a single
//! profile name (within the same library) or a 2-segment path
//! `library::profile` (cross-library). The `lookup_path` API in
//! this module uses the same 2-segment format to determine the
//! resolution start point.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::inheritance::{MAX_INHERITANCE_DEPTH, resolve_chain};
use crate::qos::{
    EntityQos, FilteredEntityQos, QosLibrary, QosProfile, merge_filtered_slots,
    select_qos_for_topic,
};

/// Resolution result: a flat merged profile snapshot.
///
/// The QoS containers hold one slot per `topic_filter` (§7.3.2.4.3),
/// merged over the `base_name` chain: `[]` means the profile names no
/// container of that kind, which on materialization into
/// `WriterQos`/`ReaderQos` maps to spec defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedQos {
    /// Full lookup path of the resolved profile (`library::profile`).
    pub lookup_path: String,
    /// Effective vendor `<topic_filter>` child element (after the
    /// inheritance override).
    pub topic_filter: Option<String>,
    /// Merged `<datawriter_qos>` slots (§7.3.2.4.3).
    pub datawriter_qos: Vec<FilteredEntityQos>,
    /// Merged `<datareader_qos>` slots (§7.3.2.4.3).
    pub datareader_qos: Vec<FilteredEntityQos>,
    /// Merged `<topic_qos>` slots (§7.3.2.4.3).
    pub topic_qos: Vec<FilteredEntityQos>,
    /// Merged `<publisher_qos>`.
    pub publisher_qos: Vec<FilteredEntityQos>,
    /// Merged `<subscriber_qos>`.
    pub subscriber_qos: Vec<FilteredEntityQos>,
    /// Merged `<domainparticipant_qos>`.
    pub domainparticipant_qos: Vec<FilteredEntityQos>,
}

impl ResolvedQos {
    /// §7.3.2.4.3 selection of the DataWriter container for
    /// `topic_name`.
    #[must_use]
    pub fn datawriter_qos_for_topic(&self, topic_name: &str) -> Option<&EntityQos> {
        select_qos_for_topic(&self.datawriter_qos, topic_name)
    }

    /// §7.3.2.4.3 selection of the DataReader container for
    /// `topic_name`.
    #[must_use]
    pub fn datareader_qos_for_topic(&self, topic_name: &str) -> Option<&EntityQos> {
        select_qos_for_topic(&self.datareader_qos, topic_name)
    }

    /// §7.3.2.4.3 selection of the Topic container for `topic_name`.
    #[must_use]
    pub fn topic_qos_for_topic(&self, topic_name: &str) -> Option<&EntityQos> {
        select_qos_for_topic(&self.topic_qos, topic_name)
    }
}

/// Resolves a profile incl. its inheritance chain and returns a
/// flat merged snapshot.
///
/// `lookup_path` is `"library::profile"` (Spec §7.3.2.4.2 convention).
/// If the profile has no `base_name`, the snapshot is
/// simply taken 1:1 from the profile.
///
/// # Errors
/// * [`XmlError::UnresolvedReference`] — `lookup_path` points to no
///   existing library or no existing profile.
/// * [`XmlError::CircularInheritance`] — the `base_name` chain contains a
///   cycle.
/// * [`XmlError::LimitExceeded`] — inheritance depth exceeded.
pub fn resolve_profile(
    libraries: &[QosLibrary],
    lookup_path: &str,
) -> Result<ResolvedQos, XmlError> {
    resolve_profile_depth(libraries, lookup_path, 0)
}

/// Which of the six slot lists of a resolved profile a container belongs to.
type Pick = fn(&ResolvedQos) -> &Vec<FilteredEntityQos>;

/// `resolve_profile` with the nesting of container `base_name`s counted.
///
/// zerodds-lint: recursion-depth = [`MAX_INHERITANCE_DEPTH`]; a container
/// `base_name` resolves another profile one level deeper, and `depth` past
/// the limit is an error.
fn resolve_profile_depth(
    libraries: &[QosLibrary],
    lookup_path: &str,
    depth: usize,
) -> Result<ResolvedQos, XmlError> {
    if depth > MAX_INHERITANCE_DEPTH {
        return Err(XmlError::LimitExceeded(format!(
            "QoS container base_name nesting deeper than {MAX_INHERITANCE_DEPTH} at `{lookup_path}`"
        )));
    }
    // Split the path into (lib, profile).
    let (lib_name, prof_name) = split_path(lookup_path)?;

    // resolve_chain operates on the profile names within the
    // same library scope. But base_name may itself be a
    // 2-segment path — we flatten that by internally using keys of the
    // form "lib::profile".
    let chain = resolve_chain(&format!("{lib_name}::{prof_name}"), |canonical| {
        let (l, p) = split_path(canonical)?;
        let prof = locate(libraries, l, p)?;
        // Normalize base_name: the one-segment form stays in the same
        // library; the two-segment form overrides the library.
        Ok(prof.base_name.as_deref().map(|b| {
            if b.contains("::") {
                b.to_string()
            } else {
                format!("{l}::{b}")
            }
        }))
    })?;

    // §7.3.2.4.2: a profile "can only inherit from QoS profiles that have
    // been defined its definition" — i.e. earlier in the document. The
    // cycle check in `resolve_chain` has already run, so a legitimate
    // chain is linear here.
    check_definition_order(&chain, |canonical| {
        let (l, p) = split_path(canonical)?;
        position(libraries, l, p)
    })?;

    // chain is base-first: [grandparent, parent, child].
    // Each entry is a "lib::profile" key. Containers merge per
    // topic_filter: a derived container with the same filter overrides
    // its base's policies, a new filter is appended (§7.3.2.4.3 selects
    // the result by topic name).
    let mut topic_filter: Option<String> = None;
    let mut dw: Vec<FilteredEntityQos> = Vec::new();
    let mut dr: Vec<FilteredEntityQos> = Vec::new();
    let mut topic: Vec<FilteredEntityQos> = Vec::new();
    let mut pub_q: Vec<FilteredEntityQos> = Vec::new();
    let mut sub_q: Vec<FilteredEntityQos> = Vec::new();
    let mut dp: Vec<FilteredEntityQos> = Vec::new();

    for key in &chain {
        let (l, p) = split_path(key)?;
        let prof = locate(libraries, l, p)?;
        if let Some(t) = &prof.topic_filter {
            topic_filter = Some(t.clone());
        }
        // The vendor `<topic_filter>` child element is this profile's
        // default filter for containers that carry no attribute; a
        // container's own `base_name` is resolved here, against the
        // library of the profile that holds it.
        let prepare = |slots: &[FilteredEntityQos],
                       pick: Pick|
         -> Result<Vec<FilteredEntityQos>, XmlError> {
            slots
                .iter()
                .map(|f| {
                    let filter = f.topic_filter.clone().or_else(|| prof.topic_filter.clone());
                    let qos =
                        resolve_container_base(libraries, f, filter.as_deref(), l, pick, depth)?;
                    Ok(FilteredEntityQos {
                        topic_filter: filter,
                        name: f.name.clone(),
                        qos,
                    })
                })
                .collect()
        };
        dw = merge_filtered_slots(dw, &prepare(&prof.datawriter_qos, |r| &r.datawriter_qos)?);
        dr = merge_filtered_slots(dr, &prepare(&prof.datareader_qos, |r| &r.datareader_qos)?);
        topic = merge_filtered_slots(topic, &prepare(&prof.topic_qos, |r| &r.topic_qos)?);
        pub_q = merge_filtered_slots(pub_q, &prepare(&prof.publisher_qos, |r| &r.publisher_qos)?);
        sub_q = merge_filtered_slots(
            sub_q,
            &prepare(&prof.subscriber_qos, |r| &r.subscriber_qos)?,
        );
        dp = merge_filtered_slots(
            dp,
            &prepare(&prof.domainparticipant_qos, |r| &r.domainparticipant_qos)?,
        );
    }

    Ok(ResolvedQos {
        lookup_path: lookup_path.to_string(),
        topic_filter,
        datawriter_qos: dw,
        datareader_qos: dr,
        topic_qos: topic,
        publisher_qos: pub_q,
        subscriber_qos: sub_q,
        domainparticipant_qos: dp,
    })
}

/// zerodds-lint: recursion-depth = [`MAX_INHERITANCE_DEPTH`] (indirectly via
/// `resolve_profile_depth`, which counts and caps the nesting).
///
/// A container's policies with its own `base_name` applied: the named
/// profile's container of the same kind (the one with the same
/// `topic_filter`, else the one a topic-less selection takes) is the base,
/// the container's policies override it.
fn resolve_container_base(
    libraries: &[QosLibrary],
    slot: &FilteredEntityQos,
    filter: Option<&str>,
    library: &str,
    pick: Pick,
    depth: usize,
) -> Result<EntityQos, XmlError> {
    let Some(base) = slot.qos.base_name.as_deref() else {
        return Ok(slot.qos.clone());
    };
    let path = if base.contains("::") {
        base.to_string()
    } else {
        format!("{library}::{base}")
    };
    // The named profile has to exist.
    let (l, p) = split_path(&path)?;
    locate(libraries, l, p)?;
    // The base is resolved as a profile of its own, so its chain and its
    // own container bases count.
    let resolved = resolve_profile_depth(libraries, &path, depth + 1)?;
    let base_slots = pick(&resolved);
    let base_qos = base_slots
        .iter()
        .find(|b| b.topic_filter.as_deref() == filter)
        .or_else(|| default_slot(base_slots));
    let mut qos = match base_qos {
        Some(b) => b.qos.clone().merge(&slot.qos),
        None => slot.qos.clone(),
    };
    qos.base_name = None;
    Ok(qos)
}

/// The container a topic-less selection takes: the one without a filter,
/// else the first.
fn default_slot(slots: &[FilteredEntityQos]) -> Option<&FilteredEntityQos> {
    slots
        .iter()
        .find(|f| f.topic_filter.is_none())
        .or_else(|| slots.first())
}

/// Splits a lookup path `library::profile` into its two segments.
fn split_path(path: &str) -> Result<(&str, &str), XmlError> {
    match path.split_once("::") {
        Some((l, p)) if !l.is_empty() && !p.is_empty() => Ok((l, p)),
        _ => Err(XmlError::UnresolvedReference(format!(
            "expected `library::profile`, got `{path}`"
        ))),
    }
}

/// Document-order position of a profile: `(library index, profile index)`.
fn position(
    libraries: &[QosLibrary],
    lib_name: &str,
    prof_name: &str,
) -> Result<(usize, usize), XmlError> {
    let (li, lib) = libraries
        .iter()
        .enumerate()
        .find(|(_, l)| l.name == lib_name)
        .ok_or_else(|| XmlError::UnresolvedReference(format!("library `{lib_name}`")))?;
    let pi = lib
        .profiles
        .iter()
        .position(|p| p.name == prof_name)
        .ok_or_else(|| {
            XmlError::UnresolvedReference(format!("profile `{lib_name}::{prof_name}`"))
        })?;
    Ok((li, pi))
}

/// Checks that each link of a base-first chain is defined before the link
/// that inherits it.
fn check_definition_order<F>(chain: &[String], mut position: F) -> Result<(), XmlError>
where
    F: FnMut(&str) -> Result<(usize, usize), XmlError>,
{
    for pair in chain.windows(2) {
        let base = position(&pair[0])?;
        let derived = position(&pair[1])?;
        if base >= derived {
            return Err(XmlError::InvalidXml(format!(
                "`{}` inherits from `{}`, which is defined later (a base must be previously \
                 defined, §7.3.2.4.2)",
                pair[1], pair[0]
            )));
        }
    }
    Ok(())
}

fn locate<'a>(
    libraries: &'a [QosLibrary],
    lib_name: &str,
    prof_name: &str,
) -> Result<&'a QosProfile, XmlError> {
    let lib = libraries
        .iter()
        .find(|l| l.name == lib_name)
        .ok_or_else(|| XmlError::UnresolvedReference(format!("library `{lib_name}`")))?;
    lib.profile(prof_name)
        .ok_or_else(|| XmlError::UnresolvedReference(format!("profile `{lib_name}::{prof_name}`")))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;
    use zerodds_qos::{DurabilityKind, HistoryKind};

    /// Read with the vendor forms accepted: some tests here are about the
    /// `<topic_filter>` child element, which the default reading refuses.
    fn parse(xml: &str) -> Vec<QosLibrary> {
        crate::qos_parser::parse_qos_libraries_with(
            xml,
            &crate::options::ReadOptions::accept_vendor_forms(),
        )
        .expect("parse")
        .value
    }

    #[test]
    fn split_path_ok() {
        assert_eq!(split_path("L::P").unwrap(), ("L", "P"));
    }

    #[test]
    fn split_path_invalid() {
        assert!(matches!(
            split_path("just_one"),
            Err(XmlError::UnresolvedReference(_))
        ));
        assert!(matches!(
            split_path("::P"),
            Err(XmlError::UnresolvedReference(_))
        ));
        assert!(matches!(
            split_path("L::"),
            Err(XmlError::UnresolvedReference(_))
        ));
    }

    #[test]
    fn child_inherits_parent_reliability() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="Base">
            <datawriter_qos>
              <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
              <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>5</depth></history>
            </datawriter_qos>
          </qos_profile>
          <qos_profile name="Derived" base_name="Base">
            <datawriter_qos>
              <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            </datawriter_qos>
          </qos_profile>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "L::Derived").expect("resolve");
        let dw = &r.datawriter_qos.first().expect("dw").qos;
        // Reliability inherited from Base.
        assert_eq!(
            dw.reliability.unwrap().kind,
            zerodds_qos::ReliabilityKind::Reliable
        );
        // History overridden.
        assert_eq!(dw.history.unwrap().kind, HistoryKind::KeepAll);
    }

    #[test]
    fn three_level_inheritance_propagates() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="A">
            <datawriter_qos>
              <durability><kind>VOLATILE_DURABILITY_QOS</kind></durability>
              <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
            </datawriter_qos>
          </qos_profile>
          <qos_profile name="B" base_name="A">
            <datawriter_qos>
              <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
            </datawriter_qos>
          </qos_profile>
          <qos_profile name="C" base_name="B">
            <datawriter_qos>
              <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
            </datawriter_qos>
          </qos_profile>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "L::C").expect("resolve");
        let dw = &r.datawriter_qos.first().expect("dw").qos;
        // Durability: B-Override.
        assert_eq!(dw.durability.unwrap().kind, DurabilityKind::TransientLocal);
        // Reliability: C-Override.
        assert_eq!(
            dw.reliability.unwrap().kind,
            zerodds_qos::ReliabilityKind::Reliable
        );
    }

    #[test]
    fn cycle_detected() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="A" base_name="B"/>
          <qos_profile name="B" base_name="A"/>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let err = resolve_profile(&libs, "L::A").expect_err("cycle");
        assert!(matches!(err, XmlError::CircularInheritance(_)));
    }

    /// §7.3.2.4.2: the base must be defined before the profile that
    /// inherits it.
    #[test]
    fn forward_base_name_rejected() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="Child" base_name="Base"/>
          <qos_profile name="Base"/>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let err = resolve_profile(&libs, "L::Child").expect_err("forward");
        assert!(matches!(err, XmlError::InvalidXml(_)), "got {err:?}");
    }

    #[test]
    fn unresolved_base_name_errors() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="A" base_name="DoesNotExist"/>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let err = resolve_profile(&libs, "L::A").expect_err("missing-base");
        assert!(matches!(err, XmlError::UnresolvedReference(_)));
    }

    #[test]
    fn missing_profile_in_library_errors() {
        let libs = vec![QosLibrary {
            name: "L".into(),
            profiles: vec![],
        }];
        let err = resolve_profile(&libs, "L::Missing").expect_err("missing");
        assert!(matches!(err, XmlError::UnresolvedReference(_)));
    }

    #[test]
    fn missing_library_errors() {
        let libs = vec![QosLibrary {
            name: "L".into(),
            profiles: vec![QosProfile {
                name: "P".into(),
                ..Default::default()
            }],
        }];
        let err = resolve_profile(&libs, "Other::P").expect_err("missing-lib");
        assert!(matches!(err, XmlError::UnresolvedReference(_)));
    }

    #[test]
    fn deep_inheritance_cap_enforced() {
        // 40 generations -> deeper than MAX_INHERITANCE_DEPTH=32.
        let mut xml = String::from(r#"<dds><qos_library name="L">"#);
        for i in 0..40 {
            if i == 0 {
                xml.push_str(&format!(r#"<qos_profile name="P{i}"/>"#));
            } else {
                let prev = i - 1;
                xml.push_str(&format!(
                    r#"<qos_profile name="P{i}" base_name="P{prev}"/>"#
                ));
            }
        }
        xml.push_str("</qos_library></dds>");
        let libs = parse(&xml);
        let err = resolve_profile(&libs, "L::P39").expect_err("depth");
        assert!(matches!(err, XmlError::LimitExceeded(_)));
    }

    #[test]
    fn cross_library_base_name_two_segment() {
        let xml = r#"<dds>
          <qos_library name="LibBase">
            <qos_profile name="P">
              <datawriter_qos>
                <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
              </datawriter_qos>
            </qos_profile>
          </qos_library>
          <qos_library name="LibDerived">
            <qos_profile name="C" base_name="LibBase::P">
              <datawriter_qos>
                <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
              </datawriter_qos>
            </qos_profile>
          </qos_library>
        </dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "LibDerived::C").expect("resolve");
        let dw = &r.datawriter_qos.first().expect("dw").qos;
        assert_eq!(
            dw.reliability.unwrap().kind,
            zerodds_qos::ReliabilityKind::Reliable
        );
        assert_eq!(dw.history.unwrap().kind, HistoryKind::KeepAll);
    }

    #[test]
    fn topic_filter_inherited_and_overridden() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="A">
            <topic_filter>foo_*</topic_filter>
          </qos_profile>
          <qos_profile name="B" base_name="A"/>
          <qos_profile name="C" base_name="A">
            <topic_filter>bar_*</topic_filter>
          </qos_profile>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let rb = resolve_profile(&libs, "L::B").expect("B");
        assert_eq!(rb.topic_filter.as_deref(), Some("foo_*"));
        let rc = resolve_profile(&libs, "L::C").expect("C");
        assert_eq!(rc.topic_filter.as_deref(), Some("bar_*"));
    }

    /// §7.3.2.4.3: a profile may hold several DataWriter containers
    /// selected by their `topic_filter` attribute; explicit filters are
    /// evaluated in order and take precedence over a container without
    /// a filter (which is the assumed "*").
    #[test]
    fn topic_filter_selects_between_repeated_containers() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="P">
            <datawriter_qos topic_filter="A*">
              <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            </datawriter_qos>
            <datawriter_qos topic_filter="B*">
              <resource_limits><max_samples>128</max_samples></resource_limits>
            </datawriter_qos>
            <datawriter_qos>
              <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
            </datawriter_qos>
          </qos_profile>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "L::P").expect("resolve");
        let a = r.datawriter_qos_for_topic("A-topic").expect("A*");
        assert_eq!(a.history.unwrap().kind, HistoryKind::KeepAll);
        let b = r.datawriter_qos_for_topic("B-topic").expect("B*");
        assert_eq!(b.resource_limits.unwrap().max_samples, 128);
        // No explicit filter matches: the filterless container, "*".
        let other = r.datawriter_qos_for_topic("C").expect("fallback");
        assert_eq!(
            other.reliability.unwrap().kind,
            zerodds_qos::ReliabilityKind::BestEffort
        );
    }

    /// §7.3.2.4.3: two filtered containers and no filterless one — a
    /// topic matching neither has no QoS from this profile.
    #[test]
    fn no_matching_filter_has_no_qos() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="P">
            <datareader_qos topic_filter="A*">
              <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            </datareader_qos>
            <datareader_qos topic_filter="B*">
              <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>1</depth></history>
            </datareader_qos>
          </qos_profile>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "L::P").expect("resolve");
        assert!(r.datareader_qos_for_topic("A1").is_some());
        assert!(r.datareader_qos_for_topic("C").is_none());
    }

    /// Inheritance merges containers per filter: a derived container
    /// with the same `topic_filter` overrides its base's policies; a
    /// new filter is appended.
    #[test]
    fn filtered_containers_merge_over_the_base_chain() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="Base">
            <datawriter_qos topic_filter="A*">
              <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
              <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>5</depth></history>
            </datawriter_qos>
          </qos_profile>
          <qos_profile name="Derived" base_name="Base">
            <datawriter_qos topic_filter="A*">
              <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            </datawriter_qos>
            <datawriter_qos topic_filter="B*">
              <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
            </datawriter_qos>
          </qos_profile>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "L::Derived").expect("resolve");
        let a = r.datawriter_qos_for_topic("A1").expect("A*");
        assert_eq!(a.history.unwrap().kind, HistoryKind::KeepAll);
        assert_eq!(
            a.reliability.unwrap().kind,
            zerodds_qos::ReliabilityKind::Reliable
        );
        let b = r.datawriter_qos_for_topic("B1").expect("B*");
        assert_eq!(
            b.reliability.unwrap().kind,
            zerodds_qos::ReliabilityKind::BestEffort
        );
    }

    /// The vendor `<topic_filter>` child element is the profile's
    /// default filter for containers without an attribute.
    #[test]
    fn profile_level_topic_filter_defaults_for_attributeless_containers() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="P">
            <topic_filter>foo_*</topic_filter>
            <datawriter_qos>
              <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            </datawriter_qos>
          </qos_profile>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "L::P").expect("resolve");
        assert!(r.datawriter_qos_for_topic("foo_bar").is_some());
        assert!(r.datawriter_qos_for_topic("bar").is_none());
    }
}
