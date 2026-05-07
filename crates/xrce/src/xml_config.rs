// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE-XML-File-Configuration (Spec §7.7.3 + §9.3).
//!
//! Ein XRCE-Client/Agent kann seine ganze Object-Hierarchie aus einem
//! XML-File laden. Das File-Format folgt der DDS-XRCE-Spec §9.3 Tab.15
//! ("File-based Configuration"):
//!
//! ```xml
//! <dds>
//!   <type>
//!     <module name="ShapesDemoTypes"><struct name="ShapeType">…</struct></module>
//!   </type>
//!   <participant object_id="0xCAFE" domain_id="0">
//!     <topic object_id="0x0102" name="Square" type_name="ShapeType"
//!            qos_profile="Lib::ShapeProfile"/>
//!     <publisher object_id="0x0103">
//!       <data_writer object_id="0x0105" topic_ref="0x0102"
//!                    qos_profile="Lib::ShapeProfile"/>
//!     </publisher>
//!     <subscriber object_id="0x0104">
//!       <data_reader object_id="0x0106" topic_ref="0x0102"/>
//!     </subscriber>
//!   </participant>
//! </dds>
//! ```
//!
//! ## Bridge zu DDS-XML-Loader
//!
//! * `<type><module>…</module></type>` wird ueber
//!   [`zerodds_xml::parse_xml_tree`] eingelesen und kann an
//!   [`zerodds_xml::xtypes_parser::parse_types_element`] weitergeleitet werden
//!   (XML→TypeObject-Bridge ueber `zerodds_xml::typeobject_bridge`).
//! * `qos_profile`-Attribute referenzieren Profile aus einer separat
//!   geladenen `<qos_library>` (DDS-XML-Modul in `zerodds-xml`). Resolution
//!   erfolgt via [`XrceConfig::resolve_qos_profile`] mit injiziertem
//!   QoS-Loader.
//!
//! ## Mapping zu CREATE-Submessages (Spec §8.4.6)
//!
//! [`XrceConfig::to_create_messages`] generiert die CREATE-Submessage-
//! Sequence in topologisch korrekter Reihenfolge:
//!
//! ```text
//!   1. Type           (OBJK_TYPE,         REPRESENTATION_AS_XML_STRING)
//!   2. Participant    (OBJK_PARTICIPANT,  REPRESENTATION_AS_XML_STRING)
//!   3. Topic          (OBJK_TOPIC,        REPRESENTATION_AS_XML_STRING)
//!   4. Publisher      (OBJK_PUBLISHER,    REPRESENTATION_AS_XML_STRING)
//!   5. Subscriber     (OBJK_SUBSCRIBER,   REPRESENTATION_AS_XML_STRING)
//!   6. DataWriter     (OBJK_DATAWRITER,   REPRESENTATION_AS_XML_STRING)
//!   7. DataReader     (OBJK_DATAREADER,   REPRESENTATION_AS_XML_STRING)
//! ```
//!
//! Out-of-scope :
//! * `<application>`-Container, `<domain>`-Library — DDS-XML-Side.
//! * Live-Reload / Hot-Swap — Stretch.
//! * DTD-/XSD-Validation der Schema selbst — `xrce-config.xsd` ist
//!   Doku-Skizze, nicht Validator-Input.

extern crate alloc;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use zerodds_xml::{DdsXmlDocument, XmlElement, XmlError, parse_xml_tree};

use crate::error::XrceError;
use crate::object_id::ObjectId;
use crate::object_kind::ObjectKind;
use crate::object_repr::ObjectVariant;
use crate::submessages::create::CreatePayload;

/// Maximale Schachtelungs-Tiefe der XRCE-Object-Hierarchie. Spec §9.3
/// erlaubt formal `<dds>/<participant>/<publisher>/<data_writer>` —
/// Tiefe 4. Wir setzen ein DoS-Cap von 8 fuer Robustheit.
pub const MAX_HIERARCHY_DEPTH: usize = 8;

/// Maximale Anzahl Type-Definitionen pro File (DoS-Cap).
pub const MAX_TYPES_PER_FILE: usize = 256;

/// Fehler beim Laden eines XRCE-XML-Configuration-Files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrceXmlError {
    /// Underlying DDS-XML-Loader-Fehler (Wohlgeformtheit, DoS-Cap).
    InvalidXml(String),
    /// Wurzel-Element ist nicht `<dds>` (Spec §9.3).
    UnexpectedRoot(String),
    /// Pflicht-Attribut fehlt (z.B. `object_id`, `domain_id`, `name`).
    MissingAttribute {
        /// Element-Name.
        element: String,
        /// Attribut-Name.
        attribute: String,
    },
    /// Attribut-Wert ist nicht parsebar (z.B. `object_id="0xZZZZ"`).
    InvalidAttribute {
        /// Element-Name.
        element: String,
        /// Attribut-Name.
        attribute: String,
        /// Beobachteter Wert.
        value: String,
    },
    /// Doppelte ObjectId im Scope eines Participants (Spec §7.2.1
    /// fordert Eindeutigkeit pro Client/Agent-Object-Store).
    DuplicateObjectId(ObjectId),
    /// `topic_ref` zeigt auf eine ObjectId, die nicht im selben
    /// Participant deklariert ist.
    UnresolvedTopicRef {
        /// DataWriter-/DataReader-ObjectId.
        endpoint: ObjectId,
        /// Topic-ObjectId, auf die verwiesen wurde.
        topic: ObjectId,
    },
    /// `type_name` referenziert einen Type, der nicht in der globalen
    /// `<type>`-Section deklariert ist.
    UnresolvedTypeName {
        /// Topic-ObjectId, an der der Type-Name haengt.
        topic: ObjectId,
        /// Type-Name (z.B. `ShapeType` oder `Module::ShapeType`).
        type_name: String,
    },
    /// `qos_profile`-String konnte nicht aufgeloest werden (kein
    /// QoS-Loader injiziert oder Profile fehlt).
    UnresolvedQosProfile(String),
    /// Schachtelungs-Tiefe ueberschreitet [`MAX_HIERARCHY_DEPTH`].
    HierarchyTooDeep(usize),
    /// Anzahl Types ueberschreitet [`MAX_TYPES_PER_FILE`].
    TooManyTypes(usize),
    /// Innerhalb der Type-Definition referenziert sich ein Type selbst
    /// (struct A enthaelt struct A) — direkt oder transitiv.
    CircularType(String),
    /// Domain-Id ist out-of-range (DDS-XRCE 1.0 §7.7.3.6: `long`, also
    /// 0..=`i32::MAX`).
    DomainIdOutOfRange(u64),
    /// Eine ObjectId-/ObjectKind-Kombination passt nicht (z.B. eine
    /// als `<topic>` deklarierte ObjectId hat im unteren Nibble nicht
    /// `OBJK_TOPIC = 0x02`). Spec §7.2.1 verlangt diese Konsistenz.
    ObjectKindMismatch {
        /// ObjectId.
        id: ObjectId,
        /// Erwarteter Kind.
        expected: u8,
        /// Beobachteter Kind im unteren Nibble.
        actual: u8,
    },
    /// XRCE-Wire-Encode-Fehler (Payload-Cap, etc.).
    Wire(XrceError),
}

impl fmt::Display for XrceXmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidXml(msg) => write!(f, "invalid XRCE-XML: {msg}"),
            Self::UnexpectedRoot(name) => {
                write!(f, "expected <dds> root, got <{name}>")
            }
            Self::MissingAttribute { element, attribute } => {
                write!(f, "<{element}> missing required attribute `{attribute}`")
            }
            Self::InvalidAttribute {
                element,
                attribute,
                value,
            } => write!(
                f,
                "<{element}> attribute `{attribute}` has invalid value `{value}`"
            ),
            Self::DuplicateObjectId(id) => {
                write!(f, "duplicate ObjectId 0x{:04X}", id.raw())
            }
            Self::UnresolvedTopicRef { endpoint, topic } => write!(
                f,
                "endpoint 0x{:04X} references undefined topic 0x{:04X}",
                endpoint.raw(),
                topic.raw()
            ),
            Self::UnresolvedTypeName { topic, type_name } => write!(
                f,
                "topic 0x{:04X} references undefined type `{type_name}`",
                topic.raw()
            ),
            Self::UnresolvedQosProfile(name) => {
                write!(f, "unresolved QoS-profile reference `{name}`")
            }
            Self::HierarchyTooDeep(depth) => {
                write!(f, "XRCE hierarchy nesting exceeds limit (depth={depth})")
            }
            Self::TooManyTypes(count) => {
                write!(f, "too many type definitions (count={count})")
            }
            Self::CircularType(name) => {
                write!(f, "circular type definition involving `{name}`")
            }
            Self::DomainIdOutOfRange(v) => {
                write!(f, "domain_id {v} out of range (must fit i32)")
            }
            Self::ObjectKindMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "ObjectId 0x{:04X}: expected kind 0x{expected:X}, got 0x{actual:X}",
                id.raw()
            ),
            Self::Wire(e) => write!(f, "xrce wire error: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for XrceXmlError {}

impl From<XmlError> for XrceXmlError {
    fn from(e: XmlError) -> Self {
        Self::InvalidXml(e.to_string())
    }
}

impl From<XrceError> for XrceXmlError {
    fn from(e: XrceError) -> Self {
        Self::Wire(e)
    }
}

/// Top-Level XRCE-File-Configuration (Spec §9.3 Tab.15).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XrceConfig {
    /// Type-Definitionen (`<type><module>…</module></type>`). Speicher
    /// ist der Original-XML-Substring pro Type — der DDS-XRCE-Wire-Pfad
    /// uebergibt das ungeparste XML als `RepresentationByXmlString`.
    pub types: Vec<TypeConfig>,
    /// Participants. Pro Participant eigener ObjectStore-Scope.
    pub participants: Vec<ParticipantConfig>,
}

/// Eine `<type>`-Section (Spec §7.7.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeConfig {
    /// ObjectId fuer den OBJK_TYPE-Wrapper. Wird beim Aufrufer
    /// deklariert oder mit `ObjectId::new(raw, ObjectKind::Type)`
    /// generiert. Optional, weil §9.3 ObjectId-Ableitung via
    /// MD5-Hash erlaubt — siehe `derive_object_id`.
    pub object_id: ObjectId,
    /// Type-Name (top-level struct/enum/union oder Module).
    pub name: String,
    /// Alle in dieser Section deklarierten Type-Namen (rekursiv ueber
    /// `<module>`/`<struct>`/`<enum>`/`<union>`/`<typedef>`/
    /// `<bitmask>`/`<bitset>`). Topics duerfen Namen aus dieser Liste
    /// referenzieren — entspricht der DDS-XML 1.0 §7.3.3-Sicht.
    pub declared_names: Vec<String>,
    /// Original-XML der Type-Section (Substring zwischen
    /// `<type>` und `</type>`). Wird im CREATE als
    /// `RepresentationByXmlString` weitergegeben.
    pub xml: String,
}

/// Ein `<participant>` (Spec §7.7.3.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantConfig {
    /// ObjectId, kind=OBJK_PARTICIPANT.
    pub object_id: ObjectId,
    /// `domain_id`-Attribut.
    pub domain_id: u32,
    /// Topic-Definitionen.
    pub topics: Vec<TopicConfig>,
    /// Publisher-Definitionen.
    pub publishers: Vec<PublisherConfig>,
    /// Subscriber-Definitionen.
    pub subscribers: Vec<SubscriberConfig>,
}

/// Ein `<topic>` (Spec §7.7.3.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicConfig {
    /// ObjectId, kind=OBJK_TOPIC.
    pub object_id: ObjectId,
    /// `name`-Attribut.
    pub name: String,
    /// `type_name`-Attribut. Muss in der globalen `<type>`-Section
    /// deklariert sein.
    pub type_name: String,
    /// Optional `qos_profile="Lib::Profile"`.
    pub qos_profile: Option<String>,
}

/// Ein `<publisher>` (Spec §7.7.3.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublisherConfig {
    /// ObjectId, kind=OBJK_PUBLISHER.
    pub object_id: ObjectId,
    /// DataWriter-Endpoints.
    pub data_writers: Vec<DataWriterConfig>,
}

/// Ein `<subscriber>` (Spec §7.7.3.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriberConfig {
    /// ObjectId, kind=OBJK_SUBSCRIBER.
    pub object_id: ObjectId,
    /// DataReader-Endpoints.
    pub data_readers: Vec<DataReaderConfig>,
}

/// Ein `<data_writer>` (Spec §7.7.3.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataWriterConfig {
    /// ObjectId, kind=OBJK_DATAWRITER.
    pub object_id: ObjectId,
    /// Referenz auf ein Topic im selben Participant.
    pub topic_ref: ObjectId,
    /// Optional QoS-Profile.
    pub qos_profile: Option<String>,
}

/// Ein `<data_reader>` (Spec §7.7.3.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataReaderConfig {
    /// ObjectId, kind=OBJK_DATAREADER.
    pub object_id: ObjectId,
    /// Referenz auf ein Topic im selben Participant.
    pub topic_ref: ObjectId,
    /// Optional QoS-Profile.
    pub qos_profile: Option<String>,
}

/// Eine generierte CREATE-Submessage zusammen mit ihrer ObjectId und
/// ObjectKind, damit der Caller sie an die richtige Stelle in der
/// Submessage-Pipeline einfuegen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateMessage {
    /// ObjectId, fuer die das CREATE gilt.
    pub object_id: ObjectId,
    /// Kind (Topic/Publisher/…) zur Sortier-Validation.
    pub kind: ObjectKind,
    /// Fertig konstruiertes [`CreatePayload`] mit
    /// `RepresentationByXmlString`-Body. Der Aufrufer wandelt es ueber
    /// [`CreatePayload::into_submessage`] in eine [`crate::Submessage`].
    pub payload: CreatePayload,
}

/// Trait zum Auflosen von `qos_profile="Lib::Profile"`-Referenzen aus
/// einer separat geladenen DDS-XML-`<qos_library>` (DDS-XML-Modul in
/// `zerodds-xml`). Optional injiziert; wenn `None`, werden Referenzen
/// strukturell mitgefuehrt aber nicht resolved.
pub trait QosProfileResolver {
    /// Liefert den `<qos_profile>…</qos_profile>`-XML-Substring fuer
    /// einen Pfad `Library::Profile`. `None` bei nicht-existentem
    /// Profile (Caller dann [`XrceXmlError::UnresolvedQosProfile`]).
    fn resolve(&self, path: &str) -> Option<String>;
}

/// Convenience-Resolver, der direkt eine Map `Lib::Profile -> XML`
/// haelt. Pure-Logik, kein DDS-XML-Layer noetig.
#[derive(Debug, Default, Clone)]
pub struct InMemoryQosResolver {
    /// Lookup-Map.
    pub profiles: alloc::collections::BTreeMap<String, String>,
}

impl InMemoryQosResolver {
    /// Konstruiert einen leeren Resolver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Fuegt ein Profile hinzu.
    pub fn add<P: Into<String>, X: Into<String>>(&mut self, path: P, xml: X) {
        self.profiles.insert(path.into(), xml.into());
    }
}

impl QosProfileResolver for InMemoryQosResolver {
    fn resolve(&self, path: &str) -> Option<String> {
        self.profiles.get(path).cloned()
    }
}

/// Parst einen XML-String in eine [`XrceConfig`].
///
/// Validation:
/// * Wurzel muss `<dds>` sein.
/// * Jede ObjectId ist eindeutig (ueber alle Sections + Participants).
/// * Jede ObjectId hat im unteren Nibble den passenden ObjectKind.
/// * `topic_ref` zeigt auf ein Topic im selben Participant.
/// * `type_name` ist in der `<type>`-Section deklariert.
/// * Type-Definitionen sind zyklus-frei.
///
/// # Errors
/// [`XrceXmlError`] siehe einzelne Varianten.
pub fn load_xrce_config(xml: &str) -> Result<XrceConfig, XrceXmlError> {
    let doc: DdsXmlDocument = parse_xml_tree(xml)?;
    if doc.root.name != "dds" {
        return Err(XrceXmlError::UnexpectedRoot(doc.root.name.clone()));
    }
    XrceConfig::from_root(&doc.root)
}

/// Liest und parst eine Datei (nur mit `feature = "std"`).
///
/// # Errors
/// IO-Fehler werden als [`XrceXmlError::InvalidXml`] re-emittiert,
/// XML-Fehler ueber [`load_xrce_config`].
#[cfg(feature = "std")]
pub fn load_xrce_config_from_file(path: &std::path::Path) -> Result<XrceConfig, XrceXmlError> {
    let xml = std::fs::read_to_string(path).map_err(|e| {
        XrceXmlError::InvalidXml(format!("io error reading `{}`: {}", path.display(), e))
    })?;
    load_xrce_config(&xml)
}

impl XrceConfig {
    /// Baut die Config aus dem `<dds>`-Wurzel-Element auf.
    fn from_root(root: &XmlElement) -> Result<Self, XrceXmlError> {
        let mut cfg = Self::default();
        let mut seen_type_names: alloc::collections::BTreeSet<String> =
            alloc::collections::BTreeSet::new();
        for type_el in root.children_named("type") {
            if cfg.types.len() >= MAX_TYPES_PER_FILE {
                return Err(XrceXmlError::TooManyTypes(cfg.types.len() + 1));
            }
            let tc = TypeConfig::from_element(type_el, &mut seen_type_names)?;
            cfg.types.push(tc);
        }
        // Cycle-Check ueber alle Type-Definitionen.
        check_type_cycles(&cfg.types)?;

        let mut global_ids: alloc::collections::BTreeSet<ObjectId> =
            alloc::collections::BTreeSet::new();
        for tc in &cfg.types {
            if !global_ids.insert(tc.object_id) {
                return Err(XrceXmlError::DuplicateObjectId(tc.object_id));
            }
        }

        for p_el in root.children_named("participant") {
            let part = ParticipantConfig::from_element(p_el, 1, &cfg.types, &mut global_ids)?;
            cfg.participants.push(part);
        }
        Ok(cfg)
    }

    /// Generiert die CREATE-Submessage-Sequence. Reihenfolge:
    /// Type → Participant → Topic → Publisher → Subscriber →
    /// DataWriter → DataReader.
    ///
    /// `RepresentationByXmlString`-Body ist pro Object eine kompakte
    /// XML-Snippet (kein voller File-Inhalt).
    ///
    /// # Errors
    /// [`XrceXmlError::Wire`] bei Encode-Fehlern (Payload-Cap).
    pub fn to_create_messages(&self) -> Result<Vec<CreateMessage>, XrceXmlError> {
        let mut out = Vec::new();
        // 1. Types
        for tc in &self.types {
            out.push(create_message(tc.object_id, ObjectKind::Type, &tc.xml)?);
        }
        // 2. Participants
        for p in &self.participants {
            let xml = format!(
                "<participant><domain_id>{}</domain_id></participant>",
                p.domain_id
            );
            out.push(create_message(p.object_id, ObjectKind::Participant, &xml)?);
        }
        // 3. Topics (pro Participant in Reihenfolge)
        for p in &self.participants {
            for t in &p.topics {
                let qos = t
                    .qos_profile
                    .as_deref()
                    .map(|q| format!(" qos_profile=\"{q}\""))
                    .unwrap_or_default();
                let xml = format!(
                    "<topic name=\"{}\" type_name=\"{}\"{}/>",
                    escape_xml_attr(&t.name),
                    escape_xml_attr(&t.type_name),
                    qos
                );
                out.push(create_message(t.object_id, ObjectKind::Topic, &xml)?);
            }
        }
        // 4. Publishers
        for p in &self.participants {
            for pub_ in &p.publishers {
                out.push(create_message(
                    pub_.object_id,
                    ObjectKind::Publisher,
                    "<publisher/>",
                )?);
            }
        }
        // 5. Subscribers
        for p in &self.participants {
            for sub in &p.subscribers {
                out.push(create_message(
                    sub.object_id,
                    ObjectKind::Subscriber,
                    "<subscriber/>",
                )?);
            }
        }
        // 6. DataWriters
        for p in &self.participants {
            for pub_ in &p.publishers {
                for dw in &pub_.data_writers {
                    let qos = dw
                        .qos_profile
                        .as_deref()
                        .map(|q| format!(" qos_profile=\"{q}\""))
                        .unwrap_or_default();
                    let xml = format!(
                        "<data_writer topic_ref=\"0x{:04X}\"{}/>",
                        dw.topic_ref.raw(),
                        qos
                    );
                    out.push(create_message(dw.object_id, ObjectKind::DataWriter, &xml)?);
                }
            }
        }
        // 7. DataReaders
        for p in &self.participants {
            for sub in &p.subscribers {
                for dr in &sub.data_readers {
                    let qos = dr
                        .qos_profile
                        .as_deref()
                        .map(|q| format!(" qos_profile=\"{q}\""))
                        .unwrap_or_default();
                    let xml = format!(
                        "<data_reader topic_ref=\"0x{:04X}\"{}/>",
                        dr.topic_ref.raw(),
                        qos
                    );
                    out.push(create_message(dr.object_id, ObjectKind::DataReader, &xml)?);
                }
            }
        }
        Ok(out)
    }

    /// Versucht alle `qos_profile`-Referenzen via Resolver aufzuloesen.
    /// Liefert die Liste der gefundenen `(path, xml)`-Paare; fehlende
    /// Profile werden als [`XrceXmlError::UnresolvedQosProfile`]
    /// gesammelt (erstes Encounter).
    ///
    /// # Errors
    /// [`XrceXmlError::UnresolvedQosProfile`] beim ersten fehlenden
    /// Profile.
    pub fn resolve_qos_profile<R: QosProfileResolver>(
        &self,
        path: &str,
        resolver: &R,
    ) -> Result<String, XrceXmlError> {
        resolver
            .resolve(path)
            .ok_or_else(|| XrceXmlError::UnresolvedQosProfile(path.to_string()))
    }

    /// Sammelt alle in der Hierarchie referenzierten QoS-Profile-Pfade.
    #[must_use]
    pub fn qos_profile_refs(&self) -> Vec<String> {
        let mut out: alloc::collections::BTreeSet<String> = alloc::collections::BTreeSet::new();
        for p in &self.participants {
            for t in &p.topics {
                if let Some(q) = &t.qos_profile {
                    out.insert(q.clone());
                }
            }
            for pub_ in &p.publishers {
                for dw in &pub_.data_writers {
                    if let Some(q) = &dw.qos_profile {
                        out.insert(q.clone());
                    }
                }
            }
            for sub in &p.subscribers {
                for dr in &sub.data_readers {
                    if let Some(q) = &dr.qos_profile {
                        out.insert(q.clone());
                    }
                }
            }
        }
        out.into_iter().collect()
    }
}

impl TypeConfig {
    fn from_element(
        el: &XmlElement,
        seen: &mut alloc::collections::BTreeSet<String>,
    ) -> Result<Self, XrceXmlError> {
        let object_id = parse_object_id_attr(el, "object_id", ObjectKind::Type)?;
        // Top-Level-Name: aus `name`-Attribut auf `<type>` selbst, oder
        // vom ersten Type-tragenden Kind-Element (Modul/Struct/...).
        let name = if let Some(n) = el.attribute("name") {
            n.to_string()
        } else if let Some(child) = el.children.first() {
            child
                .attribute("name")
                .ok_or_else(|| XrceXmlError::MissingAttribute {
                    element: "type".to_string(),
                    attribute: "name".to_string(),
                })?
                .to_string()
        } else {
            return Err(XrceXmlError::MissingAttribute {
                element: "type".to_string(),
                attribute: "name".to_string(),
            });
        };
        // Alle deklarierten Type-Namen (rekursiv durch <module>).
        let mut declared = Vec::new();
        collect_declared_types(el, &mut declared);
        if declared.is_empty() {
            declared.push(name.clone());
        }
        for d in &declared {
            if !seen.insert(d.clone()) {
                return Err(XrceXmlError::CircularType(d.clone()));
            }
        }
        let xml = serialize_element(el);
        Ok(Self {
            object_id,
            name,
            declared_names: declared,
            xml,
        })
    }

    /// Sammelt alle Type-Namen, die diese Definition referenziert
    /// (fuer Cycle-Detection auf der Type-Ebene).
    fn referenced_types(&self) -> Vec<String> {
        let Ok(doc) = parse_xml_tree(&self.xml) else {
            return Vec::new();
        };
        collect_member_types(&doc.root)
    }
}

/// Sammelt rekursiv alle Type-Namen unter einem Element (struct/enum/
/// union/typedef/bitmask/bitset, in `<module>`-Wraps geschachtelt).
///
/// zerodds-lint: recursion-depth 32
///
/// XML-Module-Hierarchie ist mit `MAX_HIERARCHY_DEPTH=32` durch den
/// Loader-Top-Level vorab gecappt; dieser Walk geht nicht tiefer.
fn collect_declared_types(el: &XmlElement, out: &mut Vec<String>) {
    for child in &el.children {
        match child.name.as_str() {
            "struct" | "enum" | "union" | "typedef" | "bitmask" | "bitset" => {
                if let Some(n) = child.attribute("name") {
                    out.push(n.to_string());
                }
                collect_declared_types(child, out);
            }
            "module" => {
                if let Some(n) = child.attribute("name") {
                    out.push(n.to_string());
                }
                collect_declared_types(child, out);
            }
            _ => collect_declared_types(child, out),
        }
    }
}

/// zerodds-lint: recursion-depth 32
///
/// Member-/Case-Walks innerhalb eines Type-Elements; tiefe Verschachtelung
/// ist durch `MAX_HIERARCHY_DEPTH` im Loader bereits gecappt.
fn collect_member_types(el: &XmlElement) -> Vec<String> {
    let mut out = Vec::new();
    for child in &el.children {
        if child.name == "member" || child.name == "case" {
            if let Some(t) = child.attribute("type") {
                out.push(t.to_string());
            }
        }
        out.extend(collect_member_types(child));
    }
    out
}

fn check_type_cycles(types: &[TypeConfig]) -> Result<(), XrceXmlError> {
    use alloc::collections::BTreeMap;
    use alloc::collections::BTreeSet;
    let mut graph: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for t in types {
        graph.insert(t.name.as_str(), t.referenced_types());
    }
    // DFS pro Knoten.
    for start in types.iter().map(|t| t.name.as_str()) {
        let mut stack: Vec<&str> = vec![start];
        let mut on_path: BTreeSet<&str> = BTreeSet::new();
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        while let Some(node) = stack.last().copied() {
            if !visited.contains(node) {
                visited.insert(node);
                on_path.insert(node);
            }
            let mut pushed = false;
            if let Some(neigh) = graph.get(node) {
                for n in neigh {
                    let n_str = n.as_str();
                    if on_path.contains(n_str) {
                        return Err(XrceXmlError::CircularType(n.clone()));
                    }
                    if !visited.contains(n_str) && graph.contains_key(n_str) {
                        stack.push(n_str);
                        pushed = true;
                        break;
                    }
                }
            }
            if !pushed {
                on_path.remove(node);
                stack.pop();
            }
        }
    }
    Ok(())
}

impl ParticipantConfig {
    fn from_element(
        el: &XmlElement,
        depth: usize,
        types: &[TypeConfig],
        global_ids: &mut alloc::collections::BTreeSet<ObjectId>,
    ) -> Result<Self, XrceXmlError> {
        if depth > MAX_HIERARCHY_DEPTH {
            return Err(XrceXmlError::HierarchyTooDeep(depth));
        }
        let object_id = parse_object_id_attr(el, "object_id", ObjectKind::Participant)?;
        if !global_ids.insert(object_id) {
            return Err(XrceXmlError::DuplicateObjectId(object_id));
        }
        let domain_id = parse_domain_id_attr(el)?;

        let mut topics = Vec::new();
        let mut topic_ids: alloc::collections::BTreeSet<ObjectId> =
            alloc::collections::BTreeSet::new();
        for t_el in el.children_named("topic") {
            let t = TopicConfig::from_element(t_el, types)?;
            if !global_ids.insert(t.object_id) || !topic_ids.insert(t.object_id) {
                return Err(XrceXmlError::DuplicateObjectId(t.object_id));
            }
            topics.push(t);
        }

        let mut publishers = Vec::new();
        for p_el in el.children_named("publisher") {
            let p = PublisherConfig::from_element(p_el, depth + 1, &topic_ids, global_ids)?;
            publishers.push(p);
        }

        let mut subscribers = Vec::new();
        for s_el in el.children_named("subscriber") {
            let s = SubscriberConfig::from_element(s_el, depth + 1, &topic_ids, global_ids)?;
            subscribers.push(s);
        }

        Ok(Self {
            object_id,
            domain_id,
            topics,
            publishers,
            subscribers,
        })
    }
}

impl TopicConfig {
    fn from_element(el: &XmlElement, types: &[TypeConfig]) -> Result<Self, XrceXmlError> {
        let object_id = parse_object_id_attr(el, "object_id", ObjectKind::Topic)?;
        let name = required_attr(el, "name")?;
        let type_name = required_attr(el, "type_name")?;
        // type_name darf entweder gegen den Top-Level-Namen einer
        // <type>-Section matchen oder gegen einen darunter geschachtelten
        // Type (z.B. "ShapeType" innerhalb von <module>).
        let known = types
            .iter()
            .any(|t| t.name == type_name || t.declared_names.iter().any(|d| d == &type_name));
        if !known {
            return Err(XrceXmlError::UnresolvedTypeName {
                topic: object_id,
                type_name,
            });
        }
        let qos_profile = el.attribute("qos_profile").map(ToString::to_string);
        Ok(Self {
            object_id,
            name,
            type_name,
            qos_profile,
        })
    }
}

impl PublisherConfig {
    fn from_element(
        el: &XmlElement,
        depth: usize,
        topic_ids: &alloc::collections::BTreeSet<ObjectId>,
        global_ids: &mut alloc::collections::BTreeSet<ObjectId>,
    ) -> Result<Self, XrceXmlError> {
        if depth > MAX_HIERARCHY_DEPTH {
            return Err(XrceXmlError::HierarchyTooDeep(depth));
        }
        let object_id = parse_object_id_attr(el, "object_id", ObjectKind::Publisher)?;
        if !global_ids.insert(object_id) {
            return Err(XrceXmlError::DuplicateObjectId(object_id));
        }
        let mut data_writers = Vec::new();
        for dw_el in el.children_named("data_writer") {
            let dw = DataWriterConfig::from_element(dw_el, topic_ids)?;
            if !global_ids.insert(dw.object_id) {
                return Err(XrceXmlError::DuplicateObjectId(dw.object_id));
            }
            data_writers.push(dw);
        }
        Ok(Self {
            object_id,
            data_writers,
        })
    }
}

impl SubscriberConfig {
    fn from_element(
        el: &XmlElement,
        depth: usize,
        topic_ids: &alloc::collections::BTreeSet<ObjectId>,
        global_ids: &mut alloc::collections::BTreeSet<ObjectId>,
    ) -> Result<Self, XrceXmlError> {
        if depth > MAX_HIERARCHY_DEPTH {
            return Err(XrceXmlError::HierarchyTooDeep(depth));
        }
        let object_id = parse_object_id_attr(el, "object_id", ObjectKind::Subscriber)?;
        if !global_ids.insert(object_id) {
            return Err(XrceXmlError::DuplicateObjectId(object_id));
        }
        let mut data_readers = Vec::new();
        for dr_el in el.children_named("data_reader") {
            let dr = DataReaderConfig::from_element(dr_el, topic_ids)?;
            if !global_ids.insert(dr.object_id) {
                return Err(XrceXmlError::DuplicateObjectId(dr.object_id));
            }
            data_readers.push(dr);
        }
        Ok(Self {
            object_id,
            data_readers,
        })
    }
}

impl DataWriterConfig {
    fn from_element(
        el: &XmlElement,
        topic_ids: &alloc::collections::BTreeSet<ObjectId>,
    ) -> Result<Self, XrceXmlError> {
        let object_id = parse_object_id_attr(el, "object_id", ObjectKind::DataWriter)?;
        let topic_ref = parse_object_id_attr(el, "topic_ref", ObjectKind::Topic)?;
        if !topic_ids.contains(&topic_ref) {
            return Err(XrceXmlError::UnresolvedTopicRef {
                endpoint: object_id,
                topic: topic_ref,
            });
        }
        let qos_profile = el.attribute("qos_profile").map(ToString::to_string);
        Ok(Self {
            object_id,
            topic_ref,
            qos_profile,
        })
    }
}

impl DataReaderConfig {
    fn from_element(
        el: &XmlElement,
        topic_ids: &alloc::collections::BTreeSet<ObjectId>,
    ) -> Result<Self, XrceXmlError> {
        let object_id = parse_object_id_attr(el, "object_id", ObjectKind::DataReader)?;
        let topic_ref = parse_object_id_attr(el, "topic_ref", ObjectKind::Topic)?;
        if !topic_ids.contains(&topic_ref) {
            return Err(XrceXmlError::UnresolvedTopicRef {
                endpoint: object_id,
                topic: topic_ref,
            });
        }
        let qos_profile = el.attribute("qos_profile").map(ToString::to_string);
        Ok(Self {
            object_id,
            topic_ref,
            qos_profile,
        })
    }
}

fn required_attr(el: &XmlElement, name: &str) -> Result<String, XrceXmlError> {
    el.attribute(name)
        .map(ToString::to_string)
        .ok_or_else(|| XrceXmlError::MissingAttribute {
            element: el.name.clone(),
            attribute: name.to_string(),
        })
}

fn parse_object_id_attr(
    el: &XmlElement,
    attr: &str,
    expected_kind: ObjectKind,
) -> Result<ObjectId, XrceXmlError> {
    let raw = required_attr(el, attr)?;
    let parsed = parse_u16(&raw).ok_or_else(|| XrceXmlError::InvalidAttribute {
        element: el.name.clone(),
        attribute: attr.to_string(),
        value: raw.clone(),
    })?;
    let id = ObjectId::from_raw(parsed);
    let actual = (parsed & 0x000F) as u8;
    if actual != expected_kind.to_u8() {
        return Err(XrceXmlError::ObjectKindMismatch {
            id,
            expected: expected_kind.to_u8(),
            actual,
        });
    }
    Ok(id)
}

fn parse_domain_id_attr(el: &XmlElement) -> Result<u32, XrceXmlError> {
    let raw = required_attr(el, "domain_id")?;
    let parsed: u64 = raw.parse().map_err(|_| XrceXmlError::InvalidAttribute {
        element: el.name.clone(),
        attribute: "domain_id".to_string(),
        value: raw.clone(),
    })?;
    if parsed > i32::MAX as u64 {
        return Err(XrceXmlError::DomainIdOutOfRange(parsed));
    }
    Ok(parsed as u32)
}

fn parse_u16(s: &str) -> Option<u16> {
    if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(rest, 16).ok()
    } else {
        s.parse().ok()
    }
}

fn create_message(
    object_id: ObjectId,
    kind: ObjectKind,
    xml: &str,
) -> Result<CreateMessage, XrceXmlError> {
    let variant = ObjectVariant::ByXmlString(xml.to_string());
    let representation = variant.encode(crate::encoding::Endianness::Little)?;
    let payload = CreatePayload {
        representation,
        reuse: false,
        replace: false,
    };
    Ok(CreateMessage {
        object_id,
        kind,
        payload,
    })
}

fn escape_xml_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Serialisiert ein [`XmlElement`] in eine kompakte XML-String-Form.
///
/// Diese Reserialisierung ist nicht byte-identisch mit dem Eingabe-XML
/// (Whitespace, Attribut-Reihenfolge), aber strukturell aequivalent —
/// genau das, was [`ObjectVariant::ByXmlString`] braucht.
fn serialize_element(el: &XmlElement) -> String {
    let mut out = String::new();
    write_element(&mut out, el);
    out
}

/// zerodds-lint: recursion-depth 32
///
/// XML-Reserialisierung-Walk; Tiefe ist durch `MAX_HIERARCHY_DEPTH` beim
/// Loader bereits beschraenkt.
fn write_element(out: &mut String, el: &XmlElement) {
    out.push('<');
    out.push_str(&el.name);
    for (k, v) in &el.attributes {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&escape_xml_attr(v));
        out.push('"');
    }
    if el.children.is_empty() && el.text.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    if !el.text.is_empty() {
        for c in el.text.chars() {
            match c {
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '&' => out.push_str("&amp;"),
                _ => out.push(c),
            }
        }
    }
    for child in &el.children {
        write_element(out, child);
    }
    out.push_str("</");
    out.push_str(&el.name);
    out.push('>');
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn cfg_basic() -> &'static str {
        r#"<dds>
            <type object_id="0x000A">
              <module name="ShapesDemoTypes">
                <struct name="ShapeType">
                  <member name="color" type="string"/>
                </struct>
              </module>
            </type>
            <participant object_id="0xCAF1" domain_id="0">
              <topic object_id="0x0102" name="Square" type_name="ShapeType" qos_profile="Lib::ShapeProfile"/>
              <publisher object_id="0x0103">
                <data_writer object_id="0x0105" topic_ref="0x0102" qos_profile="Lib::ShapeProfile"/>
              </publisher>
              <subscriber object_id="0x0104">
                <data_reader object_id="0x0106" topic_ref="0x0102"/>
              </subscriber>
            </participant>
          </dds>"#
    }

    // ===== 5x XML-Roundtrips / Strukturelle Equivalenz =====

    #[test]
    fn roundtrip_basic_hierarchy_parses() {
        let cfg = load_xrce_config(cfg_basic()).expect("parse");
        assert_eq!(cfg.types.len(), 1);
        assert_eq!(cfg.types[0].name, "ShapesDemoTypes");
        assert_eq!(cfg.participants.len(), 1);
        let p = &cfg.participants[0];
        assert_eq!(p.domain_id, 0);
        assert_eq!(p.topics.len(), 1);
        assert_eq!(p.publishers.len(), 1);
        assert_eq!(p.subscribers.len(), 1);
        assert_eq!(p.publishers[0].data_writers.len(), 1);
        assert_eq!(p.subscribers[0].data_readers.len(), 1);
    }

    #[test]
    fn roundtrip_object_ids_preserved() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        assert_eq!(cfg.participants[0].object_id, ObjectId::from_raw(0xCAF1));
        assert_eq!(
            cfg.participants[0].topics[0].object_id,
            ObjectId::from_raw(0x0102)
        );
    }

    #[test]
    fn roundtrip_qos_profile_carries_string() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        assert_eq!(
            cfg.participants[0].topics[0].qos_profile.as_deref(),
            Some("Lib::ShapeProfile")
        );
        assert_eq!(
            cfg.participants[0].publishers[0].data_writers[0]
                .qos_profile
                .as_deref(),
            Some("Lib::ShapeProfile")
        );
        assert!(
            cfg.participants[0].subscribers[0].data_readers[0]
                .qos_profile
                .is_none()
        );
    }

    #[test]
    fn roundtrip_topic_ref_preserved() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        assert_eq!(
            cfg.participants[0].publishers[0].data_writers[0].topic_ref,
            ObjectId::from_raw(0x0102)
        );
        assert_eq!(
            cfg.participants[0].subscribers[0].data_readers[0].topic_ref,
            ObjectId::from_raw(0x0102)
        );
    }

    #[test]
    fn roundtrip_multiple_participants() {
        let xml = r#"<dds>
              <type object_id="0x000A" name="T1"/>
              <participant object_id="0xCAF1" domain_id="0"/>
              <participant object_id="0xBEE1" domain_id="42"/>
            </dds>"#;
        let cfg = load_xrce_config(xml).unwrap();
        assert_eq!(cfg.participants.len(), 2);
        assert_eq!(cfg.participants[1].domain_id, 42);
    }

    // ===== 5x Validation-Errors =====

    #[test]
    fn err_unexpected_root() {
        let res = load_xrce_config("<not_dds/>");
        assert!(matches!(res, Err(XrceXmlError::UnexpectedRoot(_))));
    }

    #[test]
    fn err_duplicate_object_id_two_topics() {
        // Beide Topics teilen ObjectId 0x0102 — Duplikat innerhalb des
        // Topic-Scopes des Participants.
        let xml = r#"<dds>
              <type object_id="0x000A" name="T1"/>
              <participant object_id="0xCAF1" domain_id="0">
                <topic object_id="0x0102" name="A" type_name="T1"/>
                <topic object_id="0x0102" name="B" type_name="T1"/>
              </participant>
            </dds>"#;
        let res = load_xrce_config(xml);
        assert!(matches!(res, Err(XrceXmlError::DuplicateObjectId(_))));
    }

    #[test]
    fn err_unresolved_topic_ref() {
        let xml = r#"<dds>
              <type object_id="0x000A" name="T1"/>
              <participant object_id="0xCAF1" domain_id="0">
                <topic object_id="0x0102" name="A" type_name="T1"/>
                <publisher object_id="0x0103">
                  <data_writer object_id="0x0105" topic_ref="0x0FF2"/>
                </publisher>
              </participant>
            </dds>"#;
        let res = load_xrce_config(xml);
        assert!(matches!(res, Err(XrceXmlError::UnresolvedTopicRef { .. })));
    }

    #[test]
    fn err_unresolved_type_name() {
        let xml = r#"<dds>
              <type object_id="0x000A" name="T1"/>
              <participant object_id="0xCAF1" domain_id="0">
                <topic object_id="0x0102" name="A" type_name="NotDeclared"/>
              </participant>
            </dds>"#;
        let res = load_xrce_config(xml);
        assert!(matches!(res, Err(XrceXmlError::UnresolvedTypeName { .. })));
    }

    #[test]
    fn err_object_kind_mismatch() {
        // ObjectId 0x0103 endet auf 3 = OBJK_PUBLISHER, wird aber als
        // Topic deklariert (erwartet ObjectKind::Topic = 0x02).
        let xml = r#"<dds>
              <type object_id="0x000A" name="T1"/>
              <participant object_id="0xCAF1" domain_id="0">
                <topic object_id="0x0103" name="A" type_name="T1"/>
              </participant>
            </dds>"#;
        let res = load_xrce_config(xml);
        assert!(matches!(res, Err(XrceXmlError::ObjectKindMismatch { .. })));
    }

    #[test]
    fn err_circular_type_self_reference() {
        // struct A enthaelt member type=A → Cycle
        let xml = r#"<dds>
              <type object_id="0x000A" name="A">
                <struct name="A">
                  <member name="self_ref" type="A"/>
                </struct>
              </type>
              <participant object_id="0xCAF1" domain_id="0"/>
            </dds>"#;
        let res = load_xrce_config(xml);
        assert!(matches!(res, Err(XrceXmlError::CircularType(_))));
    }

    // ===== 5x to_create_messages =====

    #[test]
    fn create_messages_has_correct_count() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let msgs = cfg.to_create_messages().unwrap();
        // 1 Type + 1 Participant + 1 Topic + 1 Publisher + 1 Subscriber +
        // 1 DataWriter + 1 DataReader = 7
        assert_eq!(msgs.len(), 7);
    }

    #[test]
    fn create_messages_topological_order() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let msgs = cfg.to_create_messages().unwrap();
        let kinds: Vec<ObjectKind> = msgs.iter().map(|m| m.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ObjectKind::Type,
                ObjectKind::Participant,
                ObjectKind::Topic,
                ObjectKind::Publisher,
                ObjectKind::Subscriber,
                ObjectKind::DataWriter,
                ObjectKind::DataReader,
            ]
        );
    }

    #[test]
    fn create_messages_carry_xml_representation() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let msgs = cfg.to_create_messages().unwrap();
        let topic_msg = msgs
            .iter()
            .find(|m| m.kind == ObjectKind::Topic)
            .expect("topic");
        let body = &topic_msg.payload.representation;
        // Discriminator-Byte muss BY_XML_STRING = 0x02 sein.
        assert_eq!(body[0], crate::object_repr::repr_disc::AS_XML_STRING);
    }

    #[test]
    fn create_messages_default_flags_have_no_reuse_replace() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let msgs = cfg.to_create_messages().unwrap();
        for m in &msgs {
            assert!(!m.payload.reuse);
            assert!(!m.payload.replace);
        }
    }

    #[test]
    fn create_messages_for_empty_participant_only_participant_msg() {
        let xml = r#"<dds>
              <type object_id="0x000A" name="T1"/>
              <participant object_id="0xCAF1" domain_id="0"/>
            </dds>"#;
        let cfg = load_xrce_config(xml).unwrap();
        let msgs = cfg.to_create_messages().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].kind, ObjectKind::Type);
        assert_eq!(msgs[1].kind, ObjectKind::Participant);
    }

    // ===== 5x QoS-Profile-Resolution =====

    #[test]
    fn qos_resolver_finds_profile() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let mut r = InMemoryQosResolver::new();
        r.add("Lib::ShapeProfile", "<qos_profile name=\"ShapeProfile\"/>");
        let xml = cfg.resolve_qos_profile("Lib::ShapeProfile", &r).unwrap();
        assert!(xml.contains("ShapeProfile"));
    }

    #[test]
    fn qos_resolver_unresolved_returns_error() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let r = InMemoryQosResolver::new();
        let res = cfg.resolve_qos_profile("Lib::Missing", &r);
        assert!(matches!(res, Err(XrceXmlError::UnresolvedQosProfile(_))));
    }

    #[test]
    fn qos_profile_refs_collects_all() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let refs = cfg.qos_profile_refs();
        assert_eq!(refs, vec!["Lib::ShapeProfile".to_string()]);
    }

    #[test]
    fn qos_profile_refs_dedup() {
        let xml = r#"<dds>
              <type object_id="0x000A" name="T1"/>
              <participant object_id="0xCAF1" domain_id="0">
                <topic object_id="0x0102" name="A" type_name="T1" qos_profile="L::P"/>
                <publisher object_id="0x0103">
                  <data_writer object_id="0x0105" topic_ref="0x0102" qos_profile="L::P"/>
                </publisher>
              </participant>
            </dds>"#;
        let cfg = load_xrce_config(xml).unwrap();
        let refs = cfg.qos_profile_refs();
        assert_eq!(refs, vec!["L::P".to_string()]);
    }

    #[test]
    fn qos_resolver_via_phase7_dds_xml_loader_shape() {
        // Bridge-Test: DDS-XML-Loader liefert ein parseable
        // <qos_library>-XML; daraus extrahieren wir den Profile-XML-Block
        // und stecken ihn in unseren Resolver. Hier simulieren wir das
        // mit `zerodds_xml::parse_xml_tree`.
        let lib_xml = r#"<dds><qos_library name="Lib"><qos_profile name="ShapeProfile"><datawriter_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></datawriter_qos></qos_profile></qos_library></dds>"#;
        let doc = parse_xml_tree(lib_xml).unwrap();
        let lib = doc.root.child("qos_library").unwrap();
        let profile = lib.child("qos_profile").unwrap();
        let mut r = InMemoryQosResolver::new();
        r.add("Lib::ShapeProfile", serialize_element(profile));

        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let xml = cfg.resolve_qos_profile("Lib::ShapeProfile", &r).unwrap();
        assert!(xml.contains("RELIABLE_RELIABILITY_QOS"));
    }

    // ===== 5x Type-Reuse via xml-Crate-Bridge =====

    #[test]
    fn type_reuse_xml_substring_parseable() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        // Der gespeicherte XML-Substring muss vom zerodds-xml-Loader wieder
        // parseable sein (Bridge zur Type-Library).
        let doc = parse_xml_tree(&cfg.types[0].xml).unwrap();
        assert_eq!(doc.root.name, "type");
    }

    #[test]
    fn type_reuse_carries_module_struct() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let doc = parse_xml_tree(&cfg.types[0].xml).unwrap();
        let module = doc.root.child("module").expect("module");
        assert_eq!(module.attribute("name"), Some("ShapesDemoTypes"));
        assert!(module.child("struct").is_some());
    }

    #[test]
    fn type_reuse_member_type_extraction() {
        let cfg = load_xrce_config(cfg_basic()).unwrap();
        let refs = cfg.types[0].referenced_types();
        // member type=string steckt im Modul-Tree
        assert!(refs.iter().any(|t| t == "string"));
    }

    #[test]
    fn type_reuse_two_types_no_cycle() {
        let xml = r#"<dds>
              <type object_id="0x000A" name="A">
                <struct name="A"><member name="x" type="long"/></struct>
              </type>
              <type object_id="0x001A" name="B">
                <struct name="B"><member name="a" type="A"/></struct>
              </type>
              <participant object_id="0xCAF1" domain_id="0"/>
            </dds>"#;
        let cfg = load_xrce_config(xml).unwrap();
        assert_eq!(cfg.types.len(), 2);
    }

    #[test]
    fn type_reuse_indirect_cycle_detected() {
        let xml = r#"<dds>
              <type object_id="0x000A" name="A">
                <struct name="A"><member name="b" type="B"/></struct>
              </type>
              <type object_id="0x001A" name="B">
                <struct name="B"><member name="a" type="A"/></struct>
              </type>
              <participant object_id="0xCAF1" domain_id="0"/>
            </dds>"#;
        let res = load_xrce_config(xml);
        assert!(matches!(res, Err(XrceXmlError::CircularType(_))));
    }

    // ===== 5x Edge-Cases =====

    #[test]
    fn edge_empty_dds_root_is_valid() {
        let cfg = load_xrce_config("<dds/>").unwrap();
        assert_eq!(cfg.types.len(), 0);
        assert_eq!(cfg.participants.len(), 0);
    }

    #[test]
    fn edge_missing_root_invalid_xml() {
        let res = load_xrce_config("");
        assert!(matches!(res, Err(XrceXmlError::InvalidXml(_))));
    }

    #[test]
    fn edge_invalid_domain_id_string() {
        let xml = r#"<dds>
              <participant object_id="0xCAF1" domain_id="not_a_number"/>
            </dds>"#;
        let res = load_xrce_config(xml);
        assert!(matches!(res, Err(XrceXmlError::InvalidAttribute { .. })));
    }

    #[test]
    fn edge_domain_id_overflow() {
        let xml = format!(
            "<dds><participant object_id=\"0xCAF1\" domain_id=\"{}\"/></dds>",
            (i32::MAX as u64) + 1
        );
        let res = load_xrce_config(&xml);
        assert!(matches!(res, Err(XrceXmlError::DomainIdOutOfRange(_))));
    }

    #[test]
    fn edge_too_many_types_capped() {
        let mut xml = String::from("<dds>");
        for i in 0..(MAX_TYPES_PER_FILE + 1) {
            xml.push_str(&format!(
                "<type object_id=\"0x{:04X}\" name=\"T{}\"/>",
                ((i as u16 + 1) << 4) | ObjectKind::Type.to_u8() as u16,
                i
            ));
        }
        xml.push_str("</dds>");
        let res = load_xrce_config(&xml);
        assert!(matches!(res, Err(XrceXmlError::TooManyTypes(_))));
    }

    #[test]
    fn edge_topic_missing_required_attr() {
        let xml = r#"<dds>
              <type object_id="0x000A" name="T1"/>
              <participant object_id="0xCAF1" domain_id="0">
                <topic object_id="0x0102" name="A"/>
              </participant>
            </dds>"#;
        let res = load_xrce_config(xml);
        assert!(matches!(res, Err(XrceXmlError::MissingAttribute { .. })));
    }

    #[test]
    fn edge_invalid_object_id_format() {
        let xml = r#"<dds>
              <participant object_id="0xZZZZ" domain_id="0"/>
            </dds>"#;
        let res = load_xrce_config(xml);
        assert!(matches!(res, Err(XrceXmlError::InvalidAttribute { .. })));
    }

    #[test]
    fn edge_decimal_object_id_supported() {
        let xml = r#"<dds>
              <type object_id="10" name="T1"/>
              <participant object_id="51953" domain_id="0"/>
            </dds>"#;
        let cfg = load_xrce_config(xml).unwrap();
        assert_eq!(cfg.participants[0].object_id, ObjectId::from_raw(51953));
    }

    // ===== Fmt + From-Konvertierung =====

    #[test]
    fn display_xrce_xml_error_messages() {
        let e = XrceXmlError::DuplicateObjectId(ObjectId::from_raw(0x1234));
        assert!(format!("{e}").contains("1234"));
        let e = XrceXmlError::HierarchyTooDeep(MAX_HIERARCHY_DEPTH + 1);
        assert!(format!("{e}").contains("limit"));
        let e = XrceXmlError::TooManyTypes(MAX_TYPES_PER_FILE + 1);
        assert!(format!("{e}").contains("count"));
    }

    #[test]
    fn from_xrce_error_wraps_wire() {
        let e: XrceXmlError = XrceError::ValueOutOfRange { message: "x" }.into();
        assert!(matches!(e, XrceXmlError::Wire(_)));
    }
}
