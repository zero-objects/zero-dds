// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Top-Level Building-Block-Loader fuer DDS-XML 1.0.
//!
//! Aggregiert die vier Library-Typen (QoS, Domain, Domain-Participant,
//! Application) aus einem einzelnen `<dds>`-Root-Element zu einem
//! [`DdsXml`]-Snapshot. Bietet Cross-Library-Resolve-Helper, die einen
//! Participant inkl. seiner Inheritance-Kette und der referenzierten
//! Domain-/Topic-/QoS-Items aufloesen.
//!
//! Spec-Quellen: OMG DDS-XML 1.0 §7.3.2 - §7.3.6 zusammen.

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

/// Aggregierter Top-Level-Snapshot eines `<dds>`-Dokuments.
///
/// Alle vier Library-Typen aus DDS-XML 1.0 §7.3.2-7.3.6 sind hier in
/// ihrer geparsten Form versammelt. Cross-Library-Verweise werden
/// **lazily** beim Aufruf der `resolve_*`-Methoden aufgeloest — der
/// Konstruktor macht nur Wohlgeformtheits- und Schema-Pruefung.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DdsXml {
    /// Alle `<qos_library>`-Eintraege.
    pub qos_libraries: Vec<QosLibrary>,
    /// Alle `<domain_library>`-Eintraege.
    pub domain_libraries: Vec<DomainLibrary>,
    /// Alle `<domain_participant_library>`-Eintraege.
    pub participant_libraries: Vec<DomainParticipantLibrary>,
    /// Alle `<application_library>`-Eintraege.
    pub application_libraries: Vec<ApplicationLibrary>,
    /// Alle `<types>`-Top-Level-Bloecke (Spec §7.3.3).
    pub type_libraries: Vec<TypeLibrary>,
}

/// Aufgeloester Snapshot eines Domain-Participants nach Anwendung von:
/// 1. Multi-Level `base_name`-Inheritance (Participant-Kette).
/// 2. Domain-Lookup ueber `domain_ref`.
/// 3. Topic-Lookup ueber `topic_ref` der Children-Writer/Reader.
/// 4. Optional: QoS-Profile-Materialisierung wenn `qos_profile_ref`
///    gesetzt war oder Inline-QoS via Inheritance gemergt wurde.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedParticipant {
    /// Voller Lookup-Pfad (`library::name`).
    pub lookup_path: String,
    /// Effektiver Name.
    pub name: String,
    /// Aufgeloeste numerische Domain-ID.
    pub domain_id: u32,
    /// Voller Domain-Snapshot inkl. Topics + Type-Registrierungen.
    pub domain: DomainEntry,
    /// Inheritance-Kette der Participant-Definition (base-first).
    pub inheritance_chain: Vec<String>,
    /// Effektives Participant-QoS nach Merge der Kette.
    pub qos: Option<EntityQos>,
    /// Aufgeloeste Topic-Verweise.
    pub topics: Vec<ResolvedTopic>,
    /// Aufgeloeste Publishers.
    pub publishers: Vec<ResolvedPublisher>,
    /// Aufgeloeste Subscribers.
    pub subscribers: Vec<ResolvedSubscriber>,
}

/// Aufgeloester Topic-Snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedTopic {
    /// Topic-Name.
    pub name: String,
    /// Type-Name (aus `register_type_ref` der Domain).
    pub type_name: String,
    /// Effektives Topic-QoS (Inline aus Topic, oder via `qos_profile_ref`,
    /// oder `None` wenn nichts gesetzt).
    pub qos: Option<EntityQos>,
    /// Topic-Filter-Glob.
    pub topic_filter: Option<String>,
}

/// Aufgeloester Publisher-Snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedPublisher {
    /// Publisher-Name.
    pub name: String,
    /// Effektives Publisher-QoS (oder vom Participant geerbt).
    pub qos: Option<EntityQos>,
    /// Aufgeloeste DataWriter.
    pub data_writers: Vec<ResolvedDataWriter>,
}

/// Aufgeloester Subscriber-Snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedSubscriber {
    /// Subscriber-Name.
    pub name: String,
    /// Effektives Subscriber-QoS (oder vom Participant geerbt).
    pub qos: Option<EntityQos>,
    /// Aufgeloeste DataReader.
    pub data_readers: Vec<ResolvedDataReader>,
}

/// Aufgeloester DataWriter-Snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedDataWriter {
    /// Writer-Name.
    pub name: String,
    /// Aufgeloeste Topic-Referenz.
    pub topic: ResolvedTopic,
    /// Effektives Writer-QoS (Inline ueber Inheritance, oder Publisher-QoS,
    /// oder via `qos_profile_ref`).
    pub qos: Option<EntityQos>,
}

/// Aufgeloester DataReader-Snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedDataReader {
    /// Reader-Name.
    pub name: String,
    /// Aufgeloeste Topic-Referenz.
    pub topic: ResolvedTopic,
    /// Effektives Reader-QoS.
    pub qos: Option<EntityQos>,
}

/// Adapter-Trait fuer das Anbinden eines aufgeloesten Participants an ein
/// echtes DCPS-`DomainParticipantFactory`. Dieses Crate implementiert
/// **bewusst nur das Trait-Skelett** — eine konkrete Wire-Up-Implementation
/// lebt in einem separaten Crate (z.B. `zerodds-dcps-xml-bridge`), um die
/// Schicht-Disziplin (`zerodds-xml` haengt **nicht** von `zerodds-dcps` ab) zu
/// wahren.
///
/// Spec-Bezug: DDS-XML 1.0 §7.3.5 (Domain Participant Library) liefert
/// die Konfiguration; die Anbindung an die DDS 1.4 §2.2.2 DCPS-Factory
/// ist Aufgabe des hoeher liegenden Adapters.
pub trait ParticipantFactoryAdapter {
    /// Wende einen aufgeloesten Participant-Snapshot auf einen DCPS-
    /// Factory an. Ein Adapter MUSS:
    /// 1. Das `DomainParticipant`-Objekt mit `domain_id` erzeugen,
    /// 2. Topics und ihre Type-Registrierungen anlegen,
    /// 3. Publishers/Subscribers mit den effektiven QoS instanziieren,
    /// 4. DataWriters/DataReaders an die Topics binden.
    ///
    /// # Errors
    /// Implementation-defined.
    fn apply(&self, participant: &ResolvedParticipant) -> Result<(), XmlError>;
}

/// Convenience-Funktion: leitet einen aufgeloesten Participant an einen
/// Adapter durch. Die Implementation ist Trivial-Forwarding und existiert
/// nur, damit die Top-Level-API ergonomisch ist.
///
/// # Errors
/// Wie [`ParticipantFactoryAdapter::apply`].
pub fn apply_to_factory(
    participant: &ResolvedParticipant,
    factory: &dyn ParticipantFactoryAdapter,
) -> Result<(), XmlError> {
    factory.apply(participant)
}

/// Parsed ein vollstaendiges `<dds>`-Dokument und liefert den aggregierten
/// Building-Block-Snapshot.
///
/// Akzeptiert Dokumente, die *jede beliebige Untermenge* der vier Library-
/// Typen enthalten — auch ein leeres `<dds/>` ist ein valides Dokument.
///
/// # Errors
/// * [`XmlError::InvalidXml`] — keine `<dds>`-Wurzel oder XML nicht
///   wohlgeformt.
/// * Weitere Fehler aus den Per-Library-Decoder-Pfaden.
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
    /// Lookup eines Participants ueber `library::participant`-Pfad.
    ///
    /// # Errors
    /// [`XmlError::UnresolvedReference`] wenn Library oder Participant
    /// fehlt.
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

    /// Lookup einer Domain ueber `library::name`.
    ///
    /// # Errors
    /// [`XmlError::UnresolvedReference`] wenn Library oder Domain fehlt.
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

    /// Loest einen Participant inklusive seiner Inheritance-Kette,
    /// referenzierter Domain, Topics und QoS-Profile auf.
    ///
    /// # Errors
    /// * [`XmlError::UnresolvedReference`] — Verweis nicht auffindbar.
    /// * [`XmlError::CircularInheritance`] — `base_name`-Zyklus.
    /// * [`XmlError::LimitExceeded`] — Inheritance-Tiefe > 32.
    pub fn resolve_participant(&self, path: &str) -> Result<ResolvedParticipant, XmlError> {
        let r = parse_library_ref(path)?;
        if !r.is_qualified() {
            return Err(XmlError::UnresolvedReference(format!(
                "participant ref `{path}` must be qualified `library::name`"
            )));
        }
        let canonical = format!("{}::{}", r.library, r.name);

        // Inheritance-Kette aufloesen.
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

        // Felder durch Merge der Kette aufloesen (base-first).
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

        // Topics: nur die explizit referenzierten (per `<topic ref="…"/>`)
        // werden in den ResolvedParticipant uebernommen. Wenn keine
        // `topics_ref` vorhanden sind, werden ALLE Topics der Domain
        // als implizit verfuegbar betrachtet (Spec §7.3.5.4.2 erlaubt
        // beide Lesarten — Annex C zeigt explizite Selektion; Cyclone
        // und FastDDS treten implizit alle Topics zur Verfuegung).
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
        // type-name ueber register_type_ref aufloesen.
        let rt = domain
            .register_type(&topic.register_type_ref)
            .ok_or_else(|| {
                XmlError::UnresolvedReference(format!(
                    "register_type `{}`",
                    topic.register_type_ref
                ))
            })?;
        // QoS: Inline gewinnt; sonst qos_profile_ref aufloesen.
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

    /// Loest einen Type-Namen (`Module::Sub::Type` oder bare `Type`) ueber
    /// alle [`Self::type_libraries`] auf.
    ///
    /// Bei mehreren passenden Eintraegen wird der erste in
    /// Dokument-Reihenfolge geliefert.
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

    /// Loest eine Application auf zu einer Liste von ResolvedParticipants
    /// (1+ Eintraege pro `<application>`).
    ///
    /// # Errors
    /// Wie [`Self::resolve_participant`].
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

/// zerodds-lint: recursion-depth = anzahl `::`-Segmente im Lookup-Pfad +
/// Modul-Schachtelungstiefe (durch `MAX_TOTAL_ELEMENTS`-DoS-Cap der
/// XML-Foundation effektiv beschraenkt; realistisch ≤ 16).
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
