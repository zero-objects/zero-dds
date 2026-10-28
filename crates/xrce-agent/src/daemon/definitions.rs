// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The XRCE objects a configuration file defines (DDS-XRCE 1.0 §9.3).
//!
//! "The XRCE Agent shall parse the XML file and for each of the elements
//! defined in Table 15, it shall construct the corresponding XRCE Object
//! [...]. All the created XRCE Objects shall be made available to XRCE
//! clients such that they may refer to them using the representation format
//! REPRESENTATION_BY_REFERENCE." This module holds the library kinds that
//! have no `<types>`/`<qos_library>` counterpart in
//! [`crate::daemon::profiles::AgentProfiles`]: the Domains
//! (`<domain_library>`), DomainParticipants (`<domain_participant_library>`)
//! and Applications (`<application_library>`), each under its fully
//! qualified name, and the entities inside a DomainParticipant (topics,
//! publishers, subscribers, DataWriters, DataReaders) under their name.
//!
//! The `ObjectId` of every one of them comes from the MD5 of its
//! `REPRESENTATION_BY_REFERENCE` string ([`ObjectId::from_reference`]), so a
//! client that knows the string knows the id. A participant plan
//! ([`SystemDefinitions::plan_participant`]) lists the XRCE objects that
//! belong to a DomainParticipant, in the order they are created, each as the
//! `ObjectVariant` a client would send in a CREATE.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_xml::XmlElement;
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};

/// `<register_type name="…" type_ref="…"/>` (DDS-XML §7.3.4.4.4): the name a
/// type goes by in the participant, and the type it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterType {
    /// The name the topics of the participant use (`register_type_ref`).
    pub name: String,
    /// The type definition: a fully qualified type name.
    pub type_ref: String,
}

/// An XRCE Domain: `Library::Domain` (§7.7.3.4, Table 15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDef {
    /// `Library::Domain`.
    pub reference: String,
    /// `domain_id`, own or inherited through `base_name`.
    pub domain_id: Option<u32>,
    /// The type registrations of the domain; a participant that references
    /// the domain has them in its scope (§7.7.3.6.3).
    pub register_types: Vec<RegisterType>,
    /// The `<topic>` elements of the domain.
    pub topics: Vec<XmlElement>,
}

/// An XRCE DomainParticipant definition (§7.7.3.6, Table 15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantDef {
    /// `Library::Participant` for a `<domain_participant_library>`,
    /// `Library::Application::Participant` for a participant of an
    /// application (Table 16).
    pub reference: String,
    /// The library the element is in (a base_name without a library is
    /// looked up there).
    pub library: String,
    /// The application it belongs to, if it is one of an application.
    pub application: Option<String>,
    /// The `<domain_participant>` element.
    pub element: XmlElement,
}

/// An XRCE Application: `Library::Application` (§7.7.3.5, Table 15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationDef {
    /// `Library::Application`.
    pub reference: String,
    /// The references of its participants.
    pub participants: Vec<String>,
}

/// What is in the scope of a DomainParticipant: the type registrations and
/// the topics defined directly on it and in the Domain it references
/// (DDS-XRCE 1.0 §7.7.3.6.3, §7.7.3.7.1: "The Topic could be defined directly
/// on the XRCE DomainParticipant, or else in the XRCE Domain associated with
/// the DomainParticipant").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParticipantScope {
    /// Type registrations: the name a topic uses, and the type it is.
    pub register_types: Vec<RegisterType>,
    /// The `<topic>` elements.
    pub topics: Vec<XmlElement>,
}

impl ParticipantScope {
    /// The `<topic>` named `name`.
    #[must_use]
    pub fn topic(&self, name: &str) -> Option<&XmlElement> {
        self.topics
            .iter()
            .find(|t| t.attribute("name") == Some(name))
    }

    /// The type registration named `name`.
    #[must_use]
    pub fn register_type(&self, name: &str) -> Option<&RegisterType> {
        self.register_types.iter().find(|r| r.name == name)
    }
}

/// One XRCE object of a participant plan, as a CREATE would describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedObject {
    /// Its `ObjectId`, derived from its name (§9.3).
    pub oid: ObjectId,
    /// Its kind.
    pub kind: ObjectKind,
    /// The name (`REPRESENTATION_BY_REFERENCE` string) the id comes from.
    pub name: String,
    /// The representation to create it with.
    pub variant: ObjectVariant,
    /// The trailing field of its `ObjectVariant`: the container it is
    /// created under.
    pub extra: ObjectVariantExtra,
}

/// What a DomainParticipant definition amounts to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantPlan {
    /// The DDS domain.
    pub domain_id: i32,
    /// The nested objects, in the order they are created: topics, then
    /// publishers and subscribers, then DataWriters and DataReaders.
    pub nested: Vec<PlannedObject>,
}

/// Domains, DomainParticipants and Applications of the configuration files.
#[derive(Debug, Clone, Default)]
pub struct SystemDefinitions {
    domains: BTreeMap<String, DomainDef>,
    participants: BTreeMap<String, ParticipantDef>,
    applications: BTreeMap<String, ApplicationDef>,
}

fn name_of<'a>(el: &'a XmlElement, what: &str) -> Result<&'a str, String> {
    el.attribute("name")
        .filter(|n| !n.is_empty())
        .ok_or_else(|| format!("<{what}> without a name"))
}

impl SystemDefinitions {
    /// Reads the libraries of one document: a `<dds>` around libraries, or
    /// a library as the root. `<types>`, `<qos_library>` and `<profiles>`
    /// are the business of [`crate::daemon::profiles::AgentProfiles`].
    ///
    /// # Errors
    /// A name defined twice, an element without a `name`, a `base_name`
    /// that names nothing defined before it, a `domain_id` that is not a
    /// number.
    pub fn add_document(&mut self, root: &XmlElement) -> Result<(), String> {
        if root.name == "dds" {
            for child in &root.children {
                self.add_library(child)?;
            }
            Ok(())
        } else {
            self.add_library(root)
        }
    }

    fn add_library(&mut self, el: &XmlElement) -> Result<(), String> {
        match el.name.as_str() {
            "domain_library" => self.add_domain_library(el),
            "domain_participant_library" => self.add_participant_library(el),
            "application_library" => self.add_application_library(el),
            _ => Ok(()),
        }
    }

    /// A `<domain_library>`.
    ///
    /// # Errors
    /// As [`Self::add_document`].
    pub fn add_domain_library(&mut self, lib: &XmlElement) -> Result<(), String> {
        let library = name_of(lib, "domain_library")?;
        for el in lib.children_named("domain") {
            let name = name_of(el, "domain")?;
            let reference = format!("{library}::{name}");
            let mut domain = DomainDef {
                reference: reference.clone(),
                domain_id: None,
                register_types: Vec::new(),
                topics: Vec::new(),
            };
            if let Some(base) = el.attribute("base_name") {
                let base = self.domain_in(library, base).ok_or_else(|| {
                    format!(
                        "<domain name=\"{name}\" base_name=\"{base}\">: no such domain before it"
                    )
                })?;
                domain.domain_id = base.domain_id;
                domain.register_types = base.register_types.clone();
                domain.topics = base.topics.clone();
            }
            if let Some(raw) = el.attribute("domain_id") {
                domain.domain_id = Some(raw.trim().parse().map_err(|_| {
                    format!("<domain name=\"{name}\">: domain_id `{raw}` is not a number")
                })?);
            }
            merge_children(el, &mut domain.register_types, &mut domain.topics)?;
            match self.domains.get(&reference) {
                Some(known) if *known == domain => {}
                Some(_) => return Err(format!("domain `{reference}` is defined twice")),
                None => {
                    self.domains.insert(reference, domain);
                }
            }
        }
        Ok(())
    }

    fn domain_in(&self, library: &str, name: &str) -> Option<&DomainDef> {
        if name.contains("::") {
            self.domains.get(name)
        } else {
            self.domains.get(&format!("{library}::{name}"))
        }
    }

    /// A `<domain_participant_library>`.
    ///
    /// # Errors
    /// As [`Self::add_document`].
    pub fn add_participant_library(&mut self, lib: &XmlElement) -> Result<(), String> {
        let library = name_of(lib, "domain_participant_library")?;
        for el in lib.children_named("domain_participant") {
            let name = name_of(el, "domain_participant")?;
            let def = ParticipantDef {
                reference: format!("{library}::{name}"),
                library: library.to_string(),
                application: None,
                element: el.clone(),
            };
            self.insert_participant(def)?;
        }
        Ok(())
    }

    /// An `<application_library>`.
    ///
    /// # Errors
    /// As [`Self::add_document`].
    pub fn add_application_library(&mut self, lib: &XmlElement) -> Result<(), String> {
        let library = name_of(lib, "application_library")?;
        for app in lib.children_named("application") {
            let name = name_of(app, "application")?;
            let reference = format!("{library}::{name}");
            let mut participants = Vec::new();
            for el in app.children_named("domain_participant") {
                let participant = name_of(el, "domain_participant")?;
                let def = ParticipantDef {
                    reference: format!("{reference}::{participant}"),
                    library: library.to_string(),
                    application: Some(reference.clone()),
                    element: el.clone(),
                };
                participants.push(def.reference.clone());
                self.insert_participant(def)?;
            }
            let def = ApplicationDef {
                reference: reference.clone(),
                participants,
            };
            match self.applications.get(&reference) {
                Some(known) if *known == def => {}
                Some(_) => return Err(format!("application `{reference}` is defined twice")),
                None => {
                    self.applications.insert(reference, def);
                }
            }
        }
        Ok(())
    }

    fn insert_participant(&mut self, def: ParticipantDef) -> Result<(), String> {
        match self.participants.get(&def.reference) {
            // The same definition again (another client defines the same
            // library) is the same participant.
            Some(known) if *known == def => Ok(()),
            Some(_) => Err(format!(
                "domain participant `{}` is defined twice",
                def.reference
            )),
            None => {
                self.participants.insert(def.reference.clone(), def);
                Ok(())
            }
        }
    }

    /// The Domain `Library::Domain`.
    #[must_use]
    pub fn domain(&self, reference: &str) -> Option<&DomainDef> {
        self.domains.get(reference)
    }

    /// What the Domain `Library::Domain` puts in the scope of a participant
    /// that references it: its type registrations and topics (DDS-XRCE 1.0
    /// §7.7.3.6.3: "Any XRCE Topic and Type definitions contained in the
    /// referenced domain are considered defined within the scope of the XRCE
    /// DomainParticipant").
    #[must_use]
    pub fn domain_scope(&self, reference: &str) -> Option<ParticipantScope> {
        self.domains.get(reference).map(|d| ParticipantScope {
            register_types: d.register_types.clone(),
            topics: d.topics.clone(),
        })
    }

    /// The DomainParticipant `Library::Participant` (or
    /// `Library::Application::Participant`).
    #[must_use]
    pub fn participant(&self, reference: &str) -> Option<&ParticipantDef> {
        self.participants.get(reference)
    }

    /// The Application `Library::Application`.
    #[must_use]
    pub fn application(&self, reference: &str) -> Option<&ApplicationDef> {
        self.applications.get(reference)
    }

    /// Every participant definition.
    pub fn participants(&self) -> impl Iterator<Item = &ParticipantDef> {
        self.participants.values()
    }

    /// Every application definition.
    pub fn applications(&self) -> impl Iterator<Item = &ApplicationDef> {
        self.applications.values()
    }

    /// The `<domain_participant>` of `def` with its `base_name` chain
    /// merged in (DDS-XML §7.3.5.4.2, §7.3.6.4.2): the base's attributes
    /// and children first, the participant's own over them; entities of the
    /// same name are replaced.
    pub(crate) fn effective_element(
        &self,
        def: &ParticipantDef,
        depth: usize,
    ) -> Result<XmlElement, String> {
        if depth > 16 {
            return Err(format!(
                "participant `{}`: base_name chain too long",
                def.reference
            ));
        }
        let mut merged = XmlElement {
            name: def.element.name.clone(),
            ..XmlElement::default()
        };
        if let Some(base) = def.element.attribute("base_name") {
            let key = if base.contains("::") {
                base.to_string()
            } else {
                format!("{}::{base}", def.library)
            };
            let base_def = self.participants.get(&key).ok_or_else(|| {
                format!(
                    "participant `{}`: base_name `{base}` names no participant",
                    def.reference
                )
            })?;
            let base_el = self.effective_element(base_def, depth + 1)?;
            merged.attributes = base_el.attributes;
            merged.children = base_el.children;
        }
        for (k, v) in &def.element.attributes {
            merged.attributes.insert(k.clone(), v.clone());
        }
        for child in &def.element.children {
            let key = child.attribute("name");
            match merged
                .children
                .iter()
                .position(|c| c.name == child.name && key.is_some() && c.attribute("name") == key)
            {
                Some(at) => merged.children[at] = child.clone(),
                None => merged.children.push(child.clone()),
            }
        }
        Ok(merged)
    }

    /// The DDS domain of a participant definition: its own `domain_id`, or
    /// the one of the `domain_ref` Domain (DDS-XML §7.3.5.4.2).
    ///
    /// # Errors
    /// The participant names no domain, the `domain_ref` names no Domain,
    /// the id is out of range.
    pub fn domain_id_of(&self, def: &ParticipantDef) -> Result<i32, String> {
        let el = self.effective_element(def, 0)?;
        if let Some(raw) = el.attribute("domain_id") {
            return parse_domain_id(&def.reference, raw);
        }
        if let Some(reference) = el.attribute("domain_ref") {
            let domain = self.domains.get(reference).ok_or_else(|| {
                format!(
                    "participant `{}`: domain_ref `{reference}` names no Domain",
                    def.reference
                )
            })?;
            return domain
                .domain_id
                .ok_or_else(|| format!("Domain `{reference}` has no domain_id"))
                .and_then(|id| parse_domain_id(&def.reference, &id.to_string()));
        }
        Err(format!(
            "participant `{}` names neither a domain_id nor a domain_ref",
            def.reference
        ))
    }

    /// The type registrations and topics in the scope of `def`: those of its
    /// Domain (`domain_ref`) first, then its own, which replace a registration
    /// or topic of the same name.
    ///
    /// # Errors
    /// The `base_name` chain of the participant cannot be followed, or an
    /// element has no name.
    pub fn scope_of(&self, def: &ParticipantDef) -> Result<ParticipantScope, String> {
        let el = self.effective_element(def, 0)?;
        let mut scope = ParticipantScope::default();
        if let Some(reference) = el.attribute("domain_ref") {
            if let Some(domain) = self.domains.get(reference) {
                scope.register_types = domain.register_types.clone();
                scope.topics = domain.topics.clone();
            }
        }
        merge_children(&el, &mut scope.register_types, &mut scope.topics)?;
        Ok(scope)
    }

    /// The XRCE objects a DomainParticipant definition holds, as CREATEs
    /// under `participant`: its topics (those of its Domain first, §7.7.3.6.3),
    /// publishers, subscribers, DataWriters and DataReaders, each with the
    /// `ObjectId` of its name (§9.3). `resolve_type` maps the `type_ref` of a
    /// type registration to the name of the type definition it is; what it
    /// does not know is kept as written.
    ///
    /// # Errors
    /// A domain that cannot be resolved, a topic without a registered type
    /// or an endpoint of a topic the participant does not hold, two objects
    /// of one kind with one name, two objects that derive one `ObjectId`.
    pub fn plan_participant(
        &self,
        def: &ParticipantDef,
        participant: ObjectId,
        resolve_type: &dyn Fn(&str) -> Option<String>,
    ) -> Result<ParticipantPlan, String> {
        let el = self.effective_element(def, 0)?;
        let domain_id = self.domain_id_of(def)?;
        let ParticipantScope {
            register_types,
            topics,
        } = self.scope_of(def)?;

        let under = |parent: ObjectId| ObjectVariantExtra {
            domain_id: None,
            parent: Some(parent),
        };
        let mut nested: Vec<PlannedObject> = Vec::new();
        let push = |nested: &mut Vec<PlannedObject>,
                    kind: ObjectKind,
                    name: &str,
                    xml: String,
                    parent: ObjectId|
         -> Result<ObjectId, String> {
            let oid = ObjectId::from_reference(name, kind);
            if let Some(other) = nested.iter().find(|o| o.oid == oid) {
                return Err(format!(
                    "participant `{}`: `{name}` ({kind:?}) and `{}` ({:?}) derive the ObjectId \
                     {:04x}",
                    def.reference,
                    other.name,
                    other.kind,
                    oid.raw()
                ));
            }
            nested.push(PlannedObject {
                oid,
                kind,
                name: name.to_string(),
                variant: ObjectVariant::ByXmlString(xml),
                extra: under(parent),
            });
            Ok(oid)
        };

        let mut topic_ids: BTreeMap<String, ObjectId> = BTreeMap::new();
        for topic in &topics {
            let name = name_of(topic, "topic")?;
            let mut rewritten = topic.clone();
            if rewritten.attribute("type_name").is_none() {
                let registered = topic.attribute("register_type_ref").ok_or_else(|| {
                    format!(
                        "participant `{}`: topic `{name}` names no register_type_ref",
                        def.reference
                    )
                })?;
                let registration = register_types
                    .iter()
                    .find(|r| r.name == registered)
                    .ok_or_else(|| {
                        format!(
                            "participant `{}`: topic `{name}`: register_type_ref `{registered}` \
                             names no <register_type>",
                            def.reference
                        )
                    })?;
                rewritten
                    .attributes
                    .insert("type_name".into(), registration.name.clone());
                let definition = resolve_type(&registration.type_ref)
                    .unwrap_or_else(|| registration.type_ref.clone());
                rewritten.attributes.insert("type_ref".into(), definition);
            }
            let oid = push(
                &mut nested,
                ObjectKind::Topic,
                name,
                to_xml(&rewritten),
                participant,
            )?;
            topic_ids.insert(name.to_string(), oid);
        }

        let mut endpoints: Vec<(ObjectKind, String, XmlElement, ObjectId)> = Vec::new();
        for (container_tag, endpoint_tag, container_kind, endpoint_kind) in [
            (
                "publisher",
                "data_writer",
                ObjectKind::Publisher,
                ObjectKind::DataWriter,
            ),
            (
                "subscriber",
                "data_reader",
                ObjectKind::Subscriber,
                ObjectKind::DataReader,
            ),
        ] {
            for container in el.children_named(container_tag) {
                let name = name_of(container, container_tag)?;
                let mut shell = XmlElement {
                    name: container.name.clone(),
                    attributes: container.attributes.clone(),
                    ..XmlElement::default()
                };
                shell.children = container
                    .children
                    .iter()
                    .filter(|c| c.name != endpoint_tag)
                    .cloned()
                    .collect();
                let container_oid = push(
                    &mut nested,
                    container_kind,
                    name,
                    to_xml(&shell),
                    participant,
                )?;
                for endpoint in container.children_named(endpoint_tag) {
                    let endpoint_name = name_of(endpoint, endpoint_tag)?.to_string();
                    let topic_name = endpoint.attribute("topic_ref").ok_or_else(|| {
                        format!(
                            "participant `{}`: {endpoint_tag} `{endpoint_name}` names no topic_ref",
                            def.reference
                        )
                    })?;
                    let topic = topic_ids.get(topic_name).ok_or_else(|| {
                        format!(
                            "participant `{}`: {endpoint_tag} `{endpoint_name}`: topic_ref \
                             `{topic_name}` names no topic of the participant",
                            def.reference
                        )
                    })?;
                    let mut rewritten = endpoint.clone();
                    rewritten
                        .attributes
                        .insert("topic_ref".into(), format!("0x{:04x}", topic.raw()));
                    endpoints.push((endpoint_kind, endpoint_name, rewritten, container_oid));
                }
            }
        }
        for (kind, name, element, container) in endpoints {
            push(&mut nested, kind, &name, to_xml(&element), container)?;
        }
        Ok(ParticipantPlan { domain_id, nested })
    }
}

fn parse_domain_id(participant: &str, raw: &str) -> Result<i32, String> {
    let id: i32 = raw
        .trim()
        .parse()
        .map_err(|_| format!("participant `{participant}`: domain_id `{raw}` is not a number"))?;
    if !(0..=232).contains(&id) {
        return Err(format!(
            "participant `{participant}`: domain_id {id} is outside 0 to 232 (DDS 1.4 §2.2.2.2.1)"
        ));
    }
    Ok(id)
}

/// The `<register_type>` and `<topic>` children of `el` over what is there:
/// a registration or topic of the same name is replaced.
fn merge_children(
    el: &XmlElement,
    register_types: &mut Vec<RegisterType>,
    topics: &mut Vec<XmlElement>,
) -> Result<(), String> {
    for child in &el.children {
        match child.name.as_str() {
            "register_type" => {
                let name = name_of(child, "register_type")?;
                let entry = RegisterType {
                    name: name.to_string(),
                    type_ref: child.attribute("type_ref").unwrap_or(name).to_string(),
                };
                match register_types.iter().position(|r| r.name == entry.name) {
                    Some(at) => register_types[at] = entry,
                    None => register_types.push(entry),
                }
            }
            "topic" => {
                let name = name_of(child, "topic")?;
                match topics
                    .iter()
                    .position(|t| t.attribute("name") == Some(name))
                {
                    Some(at) => topics[at] = child.clone(),
                    None => topics.push(child.clone()),
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The XML text of `el`.
#[must_use]
pub fn to_xml(el: &XmlElement) -> String {
    fn escape(text: &str, out: &mut String) {
        for c in text.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                c => out.push(c),
            }
        }
    }
    /// zerodds-lint: recursion-depth = xml-tree-depth (the trees come from
    /// `zerodds_xml::parser`, which refuses a nesting over `MAX_TREE_DEPTH`).
    fn write(el: &XmlElement, out: &mut String) {
        out.push('<');
        out.push_str(&el.name);
        for (k, v) in &el.attributes {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            escape(v, out);
            out.push('"');
        }
        if el.children.is_empty() && el.text.is_empty() {
            out.push_str("/>");
            return;
        }
        out.push('>');
        escape(&el.text, out);
        for child in &el.children {
            write(child, out);
        }
        out.push_str("</");
        out.push_str(&el.name);
        out.push('>');
    }
    let mut out = String::new();
    write(el, &mut out);
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// The application of DDS-XRCE 1.0 §9.3.1.
    const EXAMPLE: &str = r#"<dds>
      <application_library name="MyApplications">
        <application name="ShapesDemoApp">
          <domain_participant name="MyParticipant" domain_ref="ShapesDomainLibrary::ShapesDomain">
            <register_type name="ShapeType" type_ref="ShapeType" />
            <topic name="Square" register_type_ref="ShapeType" />
            <topic name="Circle" register_type_ref="ShapeType" />
            <topic name="Triangle" register_type_ref="ShapeType" />
            <publisher name="MyPublisher">
              <data_writer name="MySquareWriter" topic_ref="Square">
                <datawriter_qos base_name="MyQosLibrary::MyQosProfile"/>
              </data_writer>
              <data_writer name="MyCircleWriter" topic_ref="Circle" />
            </publisher>
            <subscriber name="MySubscriber">
              <data_reader name="MyTriangleRdr" topic_ref="Triangle">
                <datareader_qos base_name="MyQosLibrary::MyQosProfile"/>
              </data_reader>
            </subscriber>
          </domain_participant>
        </application>
      </application_library>
      <domain_library name="ShapesDomainLibrary">
        <domain name="ShapesDomain" domain_id="7"/>
      </domain_library>
    </dds>"#;

    fn example() -> SystemDefinitions {
        let doc = zerodds_xml::parse_xml_tree(EXAMPLE).unwrap();
        let mut defs = SystemDefinitions::default();
        defs.add_document(&doc.root).unwrap();
        defs
    }

    /// §9.3, Table 15 and Table 16: the application, its participant and the
    /// entities in it are objects under the references the table gives, with
    /// the ObjectIds of Table 16 (the participant is named after its
    /// application: "MyApplications::ShapesDemoApp::MyParticipant").
    #[test]
    fn the_example_file_defines_the_objects_of_table_16() {
        let defs = example();
        assert!(defs.application("MyApplications::ShapesDemoApp").is_some());
        let participant = defs
            .participant("MyApplications::ShapesDemoApp::MyParticipant")
            .expect("participant");
        assert_eq!(defs.domain_id_of(participant).unwrap(), 7);
        let oid = ObjectId::from_reference(&participant.reference, ObjectKind::Participant);
        assert_eq!(oid.to_bytes(), [0x56, 0xc1]);
        let plan = defs
            .plan_participant(participant, oid, &|name| {
                (name == "ShapeType").then(|| "ShapesDemoTypes::ShapeType".to_string())
            })
            .unwrap();
        assert_eq!(plan.domain_id, 7);
        let ids: Vec<(String, [u8; 2])> = plan
            .nested
            .iter()
            .map(|o| (o.name.clone(), o.oid.to_bytes()))
            .collect();
        assert_eq!(
            ids,
            vec![
                ("Square".to_string(), [0xce, 0xb2]),
                ("Circle".to_string(), [0x30, 0x92]),
                ("Triangle".to_string(), [0x5e, 0x52]),
                ("MyPublisher".to_string(), [0x13, 0xe3]),
                ("MySubscriber".to_string(), [0xae, 0x04]),
                ("MySquareWriter".to_string(), [0x1c, 0xc5]),
                ("MyCircleWriter".to_string(), [0xcf, 0x85]),
                // The example file names it "MyTriangleRdr"; Table 16 says
                // "MyTriangleReader".
                ("MyTriangleRdr".to_string(), {
                    ObjectId::from_reference("MyTriangleRdr", ObjectKind::DataReader).to_bytes()
                }),
            ]
        );
        // Writers hang under their publisher, topics and publishers under
        // the participant, and a writer names its topic by id.
        let writer = plan
            .nested
            .iter()
            .find(|o| o.name == "MySquareWriter")
            .unwrap();
        assert_eq!(writer.extra.parent, Some(plan.nested[3].oid));
        let ObjectVariant::ByXmlString(xml) = &writer.variant else {
            panic!("not XML");
        };
        assert!(xml.contains("topic_ref=\"0xceb2\""), "{xml}");
        let topic = &plan.nested[0];
        assert_eq!(topic.extra.parent, Some(oid));
        let ObjectVariant::ByXmlString(xml) = &topic.variant else {
            panic!("not XML");
        };
        assert!(xml.contains("type_name=\"ShapeType\""), "{xml}");
        assert!(
            xml.contains("type_ref=\"ShapesDemoTypes::ShapeType\""),
            "{xml}"
        );
    }

    #[test]
    fn a_participant_library_and_inheritance() {
        let doc = zerodds_xml::parse_xml_tree(
            r#"<dds><domain_library name="D"><domain name="Base" domain_id="3">
                 <register_type name="T" type_ref="Mod::T"/><topic name="A" register_type_ref="T"/>
               </domain><domain name="Derived" base_name="Base">
                 <topic name="B" register_type_ref="T"/></domain></domain_library>
               <domain_participant_library name="P">
                 <domain_participant name="One" domain_ref="D::Derived">
                   <publisher name="Pub"><data_writer name="W" topic_ref="B"/></publisher>
                 </domain_participant>
                 <domain_participant name="Two" base_name="One">
                   <subscriber name="Sub"><data_reader name="R" topic_ref="A"/></subscriber>
                 </domain_participant>
               </domain_participant_library></dds>"#,
        )
        .unwrap();
        let mut defs = SystemDefinitions::default();
        defs.add_document(&doc.root).unwrap();
        assert_eq!(defs.domain("D::Derived").unwrap().domain_id, Some(3));
        let two = defs.participant("P::Two").unwrap();
        // The domain, the topics of the domain and the publisher come from
        // the domains and from the base participant.
        assert_eq!(defs.domain_id_of(two).unwrap(), 3);
        let plan = defs
            .plan_participant(
                two,
                ObjectId::from_reference("P::Two", ObjectKind::Participant),
                &|n| Some(n.to_string()),
            )
            .unwrap();
        let names: Vec<&str> = plan.nested.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B", "Pub", "Sub", "W", "R"]);
    }

    #[test]
    fn what_cannot_be_planned_is_refused() {
        let mut defs = SystemDefinitions::default();
        for (xml, expected) in [
            // A writer of a topic the participant does not hold.
            (
                r#"<domain_participant_library name="L"><domain_participant name="P" domain_id="0">
                   <publisher name="Pub"><data_writer name="W" topic_ref="Missing"/></publisher>
                   </domain_participant></domain_participant_library>"#,
                "names no topic of the participant",
            ),
            // A topic whose type registration is missing.
            (
                r#"<domain_participant_library name="M"><domain_participant name="P" domain_id="0">
                   <topic name="T" register_type_ref="Nope"/></domain_participant>
                   </domain_participant_library>"#,
                "names no <register_type>",
            ),
            // No domain at all.
            (
                r#"<domain_participant_library name="N"><domain_participant name="P"/>
                   </domain_participant_library>"#,
                "neither a domain_id nor a domain_ref",
            ),
        ] {
            let doc = zerodds_xml::parse_xml_tree(xml).unwrap();
            defs.add_document(&doc.root).unwrap();
            let reference = format!("{}::P", doc.root.attribute("name").unwrap());
            let def = defs.participant(&reference).unwrap().clone();
            let err = defs
                .plan_participant(
                    &def,
                    ObjectId::from_reference(&reference, ObjectKind::Participant),
                    &|n| Some(n.to_string()),
                )
                .unwrap_err();
            assert!(err.contains(expected), "{err}");
        }
        let twice = zerodds_xml::parse_xml_tree(
            r#"<domain_participant_library name="L"><domain_participant name="P" domain_id="0"/>
               </domain_participant_library>"#,
        )
        .unwrap();
        assert!(
            defs.add_document(&twice.root)
                .unwrap_err()
                .contains("defined twice")
        );
    }
}
