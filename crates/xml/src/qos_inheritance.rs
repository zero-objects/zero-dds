// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Inheritance-Resolver fuer QoS-Profile gemaess DDS-XML 1.0 §7.3.2.4.2.
//!
//! Override-Semantik: Ein Kind-Profile mit `base_name="parent"` erbt alle
//! Policies vom Parent; jede explizit gesetzte Policy im Kind ueberschreibt
//! die geerbte. Multi-Level-Vererbung (Grossparent-Kette) wird ueber den
//! Foundation-`resolve_chain`-Helper realisiert (mit Zyklus-Erkennung und
//! Tiefen-Cap).
//!
//! Lookup-Pfade: ein `base_name` darf entweder einen einzelnen
//! Profile-Namen (innerhalb derselben Library) oder einen 2-Segment-Pfad
//! `library::profile` (cross-Library) tragen. Die `lookup_path`-API in
//! diesem Modul nutzt das gleiche 2-Segment-Format zum Bestimmen des
//! Aufloesungs-Startpunkts.

use alloc::format;
use alloc::string::{String, ToString};

use crate::errors::XmlError;
use crate::inheritance::resolve_chain;
use crate::qos::{EntityQos, QosLibrary, QosProfile};

/// Aufloesungs-Resultat: ein flach gemergter Profile-Snapshot.
///
/// Alle 6 Entity-QoS-Container werden erschoepfend gemergt: `None` heisst
/// "auch nach Auflosung weder im Kind noch im Parent gesetzt", was bei
/// der Materialisierung in `WriterQos`/`ReaderQos` zu Spec-Defaults
/// abgebildet wird.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedQos {
    /// Voller Lookup-Pfad des aufgeloesten Profile (`library::profile`).
    pub lookup_path: String,
    /// Effektiver Topic-Filter (nach Inheritance-Override).
    pub topic_filter: Option<String>,
    /// Gemergtes `<datawriter_qos>`.
    pub datawriter_qos: Option<EntityQos>,
    /// Gemergtes `<datareader_qos>`.
    pub datareader_qos: Option<EntityQos>,
    /// Gemergtes `<topic_qos>`.
    pub topic_qos: Option<EntityQos>,
    /// Gemergtes `<publisher_qos>`.
    pub publisher_qos: Option<EntityQos>,
    /// Gemergtes `<subscriber_qos>`.
    pub subscriber_qos: Option<EntityQos>,
    /// Gemergtes `<domainparticipant_qos>`.
    pub domainparticipant_qos: Option<EntityQos>,
}

/// Loest ein Profile inkl. Vererbungs-Kette auf und liefert einen
/// flach gemergten Snapshot.
///
/// `lookup_path` ist `"library::profile"` (Spec §7.3.2.4.2-Konvention).
/// Falls das Profile keinen `base_name` hat, wird der Snapshot
/// einfach 1:1 aus dem Profile uebernommen.
///
/// # Errors
/// * [`XmlError::UnresolvedReference`] — `lookup_path` zeigt auf keine
///   existierende Library oder kein existierendes Profile.
/// * [`XmlError::CircularInheritance`] — `base_name`-Kette enthaelt einen
///   Zyklus.
/// * [`XmlError::LimitExceeded`] — Inheritance-Tiefe ueberschritten.
pub fn resolve_profile(
    libraries: &[QosLibrary],
    lookup_path: &str,
) -> Result<ResolvedQos, XmlError> {
    // Pfad in (lib, profile) zerlegen.
    let (lib_name, prof_name) = split_path(lookup_path)?;

    // resolve_chain operiert auf den Profile-Namen innerhalb des
    // gleichen Library-Scope. base_name darf aber selbst ein
    // 2-Segment-Pfad sein — wir flatten das, indem wir intern Keys der
    // Form "lib::profile" benutzen.
    let chain = resolve_chain(&format!("{lib_name}::{prof_name}"), |canonical| {
        let (l, p) = split_path(canonical)?;
        let prof = locate(libraries, l, p)?;
        // base_name normalisieren: Ein-Segment-Form bleibt in derselben
        // Library; Zwei-Segment-Form ueberschreibt die Library.
        Ok(prof.base_name.as_deref().map(|b| {
            if b.contains("::") {
                b.to_string()
            } else {
                format!("{l}::{b}")
            }
        }))
    })?;

    // chain ist base-first: [grandparent, parent, child].
    // Jeder Eintrag ist ein "lib::profile"-Key.
    let mut topic_filter: Option<String> = None;
    let mut dw: Option<EntityQos> = None;
    let mut dr: Option<EntityQos> = None;
    let mut topic: Option<EntityQos> = None;
    let mut pub_q: Option<EntityQos> = None;
    let mut sub_q: Option<EntityQos> = None;
    let mut dp: Option<EntityQos> = None;

    for key in &chain {
        let (l, p) = split_path(key)?;
        let prof = locate(libraries, l, p)?;
        if let Some(t) = &prof.topic_filter {
            topic_filter = Some(t.clone());
        }
        dw = merge_entity(dw, prof.datawriter_qos.as_ref());
        dr = merge_entity(dr, prof.datareader_qos.as_ref());
        topic = merge_entity(topic, prof.topic_qos.as_ref());
        pub_q = merge_entity(pub_q, prof.publisher_qos.as_ref());
        sub_q = merge_entity(sub_q, prof.subscriber_qos.as_ref());
        dp = merge_entity(dp, prof.domainparticipant_qos.as_ref());
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

/// Zerlegt einen Lookup-Pfad `library::profile` in seine zwei Segmente.
fn split_path(path: &str) -> Result<(&str, &str), XmlError> {
    match path.split_once("::") {
        Some((l, p)) if !l.is_empty() && !p.is_empty() => Ok((l, p)),
        _ => Err(XmlError::UnresolvedReference(format!(
            "expected `library::profile`, got `{path}`"
        ))),
    }
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

/// Mergt `child` (override) auf `acc` (Parent-Akku). Nimmt die Override-
/// Semantik aus [`EntityQos::merge`].
fn merge_entity(acc: Option<EntityQos>, child: Option<&EntityQos>) -> Option<EntityQos> {
    match (acc, child) {
        (None, None) => None,
        (Some(a), None) => Some(a),
        (None, Some(c)) => Some(c.clone()),
        (Some(a), Some(c)) => Some(a.merge(c)),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::qos_parser::parse_qos_libraries;
    use alloc::vec;
    use zerodds_qos::{DurabilityKind, HistoryKind};

    fn parse(xml: &str) -> Vec<QosLibrary> {
        parse_qos_libraries(xml).expect("parse")
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
              <reliability><kind>RELIABLE</kind></reliability>
              <history><kind>KEEP_LAST</kind><depth>5</depth></history>
            </datawriter_qos>
          </qos_profile>
          <qos_profile name="Derived" base_name="Base">
            <datawriter_qos>
              <history><kind>KEEP_ALL</kind></history>
            </datawriter_qos>
          </qos_profile>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "L::Derived").expect("resolve");
        let dw = r.datawriter_qos.as_ref().expect("dw");
        // Reliability geerbt von Base.
        assert_eq!(
            dw.reliability.unwrap().kind,
            zerodds_qos::ReliabilityKind::Reliable
        );
        // History ueberschrieben.
        assert_eq!(dw.history.unwrap().kind, HistoryKind::KeepAll);
    }

    #[test]
    fn three_level_inheritance_propagates() {
        let xml = r#"<dds><qos_library name="L">
          <qos_profile name="A">
            <datawriter_qos>
              <durability><kind>VOLATILE</kind></durability>
              <reliability><kind>BEST_EFFORT</kind></reliability>
            </datawriter_qos>
          </qos_profile>
          <qos_profile name="B" base_name="A">
            <datawriter_qos>
              <durability><kind>TRANSIENT_LOCAL</kind></durability>
            </datawriter_qos>
          </qos_profile>
          <qos_profile name="C" base_name="B">
            <datawriter_qos>
              <reliability><kind>RELIABLE</kind></reliability>
            </datawriter_qos>
          </qos_profile>
        </qos_library></dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "L::C").expect("resolve");
        let dw = r.datawriter_qos.as_ref().expect("dw");
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
                <reliability><kind>RELIABLE</kind></reliability>
              </datawriter_qos>
            </qos_profile>
          </qos_library>
          <qos_library name="LibDerived">
            <qos_profile name="C" base_name="LibBase::P">
              <datawriter_qos>
                <history><kind>KEEP_ALL</kind></history>
              </datawriter_qos>
            </qos_profile>
          </qos_library>
        </dds>"#;
        let libs = parse(xml);
        let r = resolve_profile(&libs, "LibDerived::C").expect("resolve");
        let dw = r.datawriter_qos.as_ref().expect("dw");
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
}
