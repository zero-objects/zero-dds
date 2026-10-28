// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What the daemon holds for a `BY_REFERENCE` representation and a
//! `qos_profile` reference.
//!
//! DDS-XRCE 1.0 §7.7.3: `REPRESENTATION_BY_REFERENCE` names a definition
//! the agent already holds. The eProsima Micro-XRCE-DDS Agent reads these
//! from a Fast DDS profiles file (`MicroXRCEAgent -r refs.xml`), and its
//! Client's `uxr_buffer_create_*_ref` calls name a `profile_name` there:
//!
//! ```xml
//! <profiles>
//!   <participant profile_name="default_xrce_participant">…</participant>
//!   <topic profile_name="shapetype_topic">
//!     <kind>WITH_KEY</kind><name>Square</name><dataType>ShapeType</dataType>
//!   </topic>
//!   <data_writer profile_name="shapetype_data_writer">…</data_writer>
//! </profiles>
//! ```
//!
//! `xml_profile_files:` are such files (a `<profiles>` root, or `<dds>`
//! around `<profiles>`, `<types>` and `<qos_library>`); a referenced
//! element is read by `crate::daemon::entity_repr` like the same element
//! sent inline. `<types>` define topic types for every client, as
//! `type_files:` do. `qos_library_files:` are DDS-XML 1.0 `<qos_library>`
//! documents; an attribute-form representation's
//! `qos_profile="Library::Profile"` names one of their profiles, resolved
//! with its `base_name` chain (DDS-XML §7.3.2.4.2).
//!
//! A file after the schema of DDS-XRCE 1.0 §9.3 (`dds-xml_system_example.xsd`)
//! also holds `<domain_library>`, `<domain_participant_library>` and
//! `<application_library>`. Their Domains, DomainParticipants and
//! Applications, and what is inside a DomainParticipant, are XRCE objects
//! under the `REPRESENTATION_BY_REFERENCE` strings of Table 15, with the
//! `ObjectId` derived from the MD5 of the string
//! ([`PredefinedObject`], [`crate::daemon::definitions`]).

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use zerodds_types::CompleteTypeObject;
use zerodds_xml::XmlElement;
use zerodds_xml::qos::QosLibrary;
use zerodds_xml::qos_inheritance::ResolvedQos;
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_kind::ObjectKind;

use crate::daemon::definitions::SystemDefinitions;

/// The entity elements a profile file defines by `profile_name`.
const ENTITY_ELEMENTS: &[&str] = &[
    "participant",
    "topic",
    "publisher",
    "subscriber",
    "data_writer",
    "data_reader",
];

/// Profiles, QoS libraries and types, loaded at start.
#[derive(Debug, Clone, Default)]
pub struct AgentProfiles {
    entities: BTreeMap<(String, String), XmlElement>,
    qos_libraries: Vec<QosLibrary>,
    types: BTreeMap<String, CompleteTypeObject>,
    referable: BTreeMap<String, CompleteTypeObject>,
    definitions: SystemDefinitions,
    unused: Vec<String>,
    vendor_forms: bool,
    vendor_notes: Vec<String>,
}

/// An XRCE object the configuration files define (DDS-XRCE 1.0 §9.3,
/// Table 15 and Table 16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredefinedObject {
    /// The `REPRESENTATION_BY_REFERENCE` string.
    pub reference: String,
    /// The kind of the XRCE object.
    pub kind: ObjectKind,
    /// The `ObjectId` derived from `reference`.
    pub oid: ObjectId,
    /// The reference of the DomainParticipant it is inside of, for a topic,
    /// publisher, subscriber, DataWriter or DataReader.
    pub participant: Option<String>,
}

impl AgentProfiles {
    /// Reads `xml_profile_files:` and `qos_library_files:`.
    ///
    /// # Errors
    /// A message naming the file: unreadable, not XML, a profile without
    /// `profile_name`, a `profile_name` defined twice for one element, a
    /// type library that does not compile, a QoS library that does not
    /// parse.
    pub fn load(
        profile_files: &[std::path::PathBuf],
        qos_library_files: &[std::path::PathBuf],
    ) -> Result<Self, String> {
        Self::load_with(profile_files, qos_library_files, false)
    }

    /// As [`Self::load`]; `vendor_forms` is the agent setting `vendor_forms:`
    /// (see [`Self::vendor_forms`]).
    ///
    /// # Errors
    /// As [`Self::load`].
    pub fn load_with(
        profile_files: &[std::path::PathBuf],
        qos_library_files: &[std::path::PathBuf],
        vendor_forms: bool,
    ) -> Result<Self, String> {
        let mut out = Self {
            vendor_forms,
            ..Self::default()
        };
        for path in profile_files {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            out.add_profiles(&text)
                .map_err(|e| format!("{}: {e}", path.display()))?;
        }
        for path in qos_library_files {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let parsed =
                zerodds_xml::qos_parser::parse_qos_libraries_with(&text, &out.read_options())
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            out.note_vendor_forms(&parsed.diagnostics);
            out.qos_libraries.extend(parsed.value);
        }
        // A file that defines objects the agent cannot make is refused at
        // start, not when the first client names them.
        out.predefined_objects()?;
        Ok(out)
    }

    /// Adds one profiles document.
    ///
    /// # Errors
    /// As [`Self::load`].
    pub fn add_profiles(&mut self, xml: &str) -> Result<(), String> {
        let doc = zerodds_xml::parse_xml_tree(xml).map_err(|e| e.to_string())?;
        match doc.root.name.as_str() {
            "profiles" => self.add_profiles_element(&doc.root),
            "dds" => {
                for child in &doc.root.children {
                    match child.name.as_str() {
                        "profiles" => self.add_profiles_element(child)?,
                        "types" => self.add_types(child)?,
                        "qos_library" => self.add_qos_library(child)?,
                        "domain_library" | "domain_participant_library" | "application_library" => {
                            self.definitions.add_document(child)?
                        }
                        other => self.unused.push(format!("<dds>/<{other}>")),
                    }
                }
                Ok(())
            }
            // DDS-XML gives each library building block both roots (§7.3.4.3,
            // §7.3.5.3, §7.3.6.3): a library can be the whole document.
            "types" => self.add_types(&doc.root),
            "qos_library" => self.add_qos_library(&doc.root),
            "domain_library" | "domain_participant_library" | "application_library" => {
                self.definitions.add_document(&doc.root)
            }
            other => Err(format!("expected <profiles> or <dds>, found <{other}>")),
        }
    }

    fn add_qos_library(&mut self, el: &XmlElement) -> Result<(), String> {
        let parsed =
            zerodds_xml::qos_parser::parse_qos_library_element_with(el, &self.read_options())
                .map_err(|e| e.to_string())?;
        self.note_vendor_forms(&parsed.diagnostics);
        let mut library = parsed.value;
        // DDS-XRCE 1.0 §7.7.3.2.2 writes the base of a profile
        // `base_name="MyQosLib:MyQosProfile"` with one colon (§7.7.3.2.1 says
        // `MyLibrary::MyProfile`): read as the qualified name.
        for profile in &mut library.profiles {
            if let Some(base) = profile.base_name.as_mut() {
                if base.contains(':') && !base.contains("::") {
                    *base = base.replacen(':', "::", 1);
                }
            }
        }
        // A library of the same name gets the profiles it lacks; a profile of
        // the same name and the same content is the same profile.
        match self
            .qos_libraries
            .iter_mut()
            .find(|l| l.name == library.name)
        {
            None => self.qos_libraries.push(library),
            Some(known) => {
                for profile in library.profiles {
                    match known.profiles.iter().find(|p| p.name == profile.name) {
                        Some(existing) if *existing == profile => {}
                        Some(_) => {
                            return Err(format!(
                                "qos profile `{}::{}` is defined twice",
                                known.name, profile.name
                            ));
                        }
                        None => known.profiles.push(profile),
                    }
                }
            }
        }
        Ok(())
    }

    fn add_profiles_element(&mut self, profiles: &XmlElement) -> Result<(), String> {
        for el in &profiles.children {
            if el.name == "types" {
                self.add_types(el)?;
                continue;
            }
            if !ENTITY_ELEMENTS.contains(&el.name.as_str()) {
                self.unused.push(format!("<profiles>/<{}>", el.name));
                continue;
            }
            let name = el
                .attribute("profile_name")
                .ok_or_else(|| format!("<{}> without profile_name", el.name))?;
            let key = (el.name.clone(), name.to_string());
            if self.entities.insert(key, el.clone()).is_some() {
                return Err(format!(
                    "<{} profile_name=\"{name}\"> is defined twice",
                    el.name
                ));
            }
        }
        Ok(())
    }

    fn add_types(&mut self, el: &XmlElement) -> Result<(), String> {
        let parsed = zerodds_xml::xtypes_parser::parse_types_element_with(el, &self.read_options())
            .map_err(|e| format!("<types>: {e}"))?;
        self.note_vendor_forms(&parsed.diagnostics);
        let library = parsed.value;
        // §7.7.3.3.2: a `<types>` may reference types the agent already
        // knows: those of the files read before it and the ones `refer_to`
        // names.
        let mut known = self.referable.clone();
        known.extend(self.types.iter().map(|(n, d)| (n.clone(), d.clone())));
        let compiled =
            zerodds_xml::typeobject_bridge::bridge_library_complete_with_known(&library, &known)
                .map_err(|e| format!("<types>: {e:?}"))?;
        for (name, def) in compiled {
            match self.types.get(&name) {
                // The same definition from another file or client is the
                // same type.
                Some(known) if *known == def => {}
                Some(_) => return Err(format!("type `{name}` is defined twice")),
                None => {
                    self.types.insert(name, def);
                }
            }
        }
        Ok(())
    }

    /// Types a `<types>` read from now on may reference without defining them
    /// (the `type_files:` of the agent). They are not part of [`Self::types`].
    pub(crate) fn refer_to(&mut self, types: &BTreeMap<String, CompleteTypeObject>) {
        self.referable
            .extend(types.iter().map(|(n, d)| (n.clone(), d.clone())));
    }

    /// Whether the agent reads the forms other DDS products write and DDS-XML
    /// 1.0 does not (`vendor_forms: accept`, for example the files of
    /// eProsima). Off by default: such a form is refused with an error that
    /// names it and the form of the specification.
    #[must_use]
    pub fn vendor_forms(&self) -> bool {
        self.vendor_forms
    }

    /// These profiles with the `vendor_forms:` setting.
    #[must_use]
    pub fn with_vendor_forms(mut self, accept: bool) -> Self {
        self.vendor_forms = accept;
        self
    }

    /// The reading options of the DDS-XML readers for the `vendor_forms:`
    /// setting.
    #[must_use]
    pub fn read_options(&self) -> zerodds_xml::options::ReadOptions {
        let options = zerodds_xml::options::ReadOptions::default();
        if self.vendor_forms {
            options.with_vendor_forms()
        } else {
            options
        }
    }

    /// The vendor forms the files contained and the agent read as given
    /// (one line each: the path and what the specification writes).
    #[must_use]
    pub fn vendor_notes(&self) -> &[String] {
        &self.vendor_notes
    }

    fn note_vendor_forms(&mut self, diagnostics: &[zerodds_xml::options::Diagnostic]) {
        self.vendor_notes
            .extend(diagnostics.iter().map(ToString::to_string));
    }

    /// The `<element profile_name="name">` a `BY_REFERENCE`
    /// representation names.
    #[must_use]
    pub fn entity(&self, element: &str, name: &str) -> Option<&XmlElement> {
        self.entities.get(&(element.to_string(), name.to_string()))
    }

    /// The profile `Library::Profile` of the QoS libraries, with its
    /// `base_name` chain merged.
    ///
    /// # Errors
    /// The reference names no library or profile, or its chain loops.
    pub fn qos_profile(&self, reference: &str) -> Result<ResolvedQos, String> {
        zerodds_xml::qos_inheritance::resolve_profile(&self.qos_libraries, reference)
            .map_err(|e| format!("qos_profile `{reference}`: {e}"))
    }

    /// The QoS an entity QoS element states: `<datawriter_qos>` (also
    /// `<data_writer_qos>`), `<datareader_qos>`, `<publisher_qos>`,
    /// `<subscriber_qos>`, `<topic_qos>` or `<domainparticipant_qos>`, with the
    /// policies it lists over the profile its `base_name` names (DDS-XML
    /// §7.3.2.4.2; the XRCE clauses write `Library:Profile` with one colon in
    /// places, which is read as `Library::Profile`).
    ///
    /// # Errors
    /// The element is none of those, a policy does not parse, or the
    /// `base_name` names no profile.
    pub fn qos_element(&self, el: &XmlElement) -> Result<ResolvedQos, String> {
        use zerodds_xml::qos::{QosLibrary, QosProfile};
        let inline = zerodds_xml::qos_parser::parse_entity_qos_with(el, &self.read_options())
            .map_err(|e| format!("<{}>: {e}", el.name))?
            .value;
        let mut profile = QosProfile {
            name: "x".to_string(),
            base_name: el.attribute("base_name").map(|b| {
                if b.contains("::") {
                    b.to_string()
                } else {
                    b.replacen(':', "::", 1)
                }
            }),
            ..QosProfile::default()
        };
        // DDS-XML 1.0 §7.3.2.4.3: a profile holds its containers of a kind
        // as a list, each with the `topic_filter` it was written with.
        let slot = vec![zerodds_xml::qos::FilteredEntityQos {
            topic_filter: el.attribute("topic_filter").map(str::to_string),
            name: el.attribute("name").map(str::to_string),
            qos: inline,
        }];
        match el.name.as_str() {
            "datawriter_qos" | "data_writer_qos" => profile.datawriter_qos = slot,
            "datareader_qos" | "data_reader_qos" => profile.datareader_qos = slot,
            "publisher_qos" => profile.publisher_qos = slot,
            "subscriber_qos" => profile.subscriber_qos = slot,
            "topic_qos" => profile.topic_qos = slot,
            "domain_participant_qos" | "domainparticipant_qos" => {
                profile.domainparticipant_qos = slot;
            }
            other => return Err(format!("<{other}> is no entity QoS element")),
        }
        // The profile hangs in a library of its own, so that its base_name
        // (a profile of the same library, or `Library::Profile`) resolves
        // against the libraries of the files.
        let mut libraries = self.qos_libraries.clone();
        let inline_name = "\u{0}inline".to_string();
        if let Some(base) = profile.base_name.as_mut() {
            if !base.contains("::") {
                *base = format!("{inline_name}::{base}");
            }
        }
        libraries.push(QosLibrary {
            name: inline_name.clone(),
            profiles: vec![profile],
        });
        zerodds_xml::qos_inheritance::resolve_profile(&libraries, &format!("{inline_name}::x"))
            .map_err(|e| format!("<{}>: {e}", el.name))
    }

    /// The types the profile files define, by name.
    #[must_use]
    pub fn types(&self) -> &BTreeMap<String, CompleteTypeObject> {
        &self.types
    }

    /// The Domains, DomainParticipants and Applications of the files.
    #[must_use]
    pub fn definitions(&self) -> &SystemDefinitions {
        &self.definitions
    }

    /// The type definition a `type_ref` names: the type of that fully
    /// qualified name, else the one type whose name ends in `::type_ref`
    /// (a `<register_type type_ref="ShapeType">` of a type in a module).
    #[must_use]
    pub fn resolve_type_ref(&self, type_ref: &str) -> Option<String> {
        if self.types.contains_key(type_ref) {
            return Some(type_ref.to_string());
        }
        let suffix = format!("::{type_ref}");
        let mut hits = self.types.keys().filter(|n| n.ends_with(&suffix));
        match (hits.next(), hits.next()) {
            (Some(only), None) => Some(only.clone()),
            _ => None,
        }
    }

    /// Every XRCE object the files define, with its `ObjectId` (DDS-XRCE 1.0
    /// §9.3, Table 15): the Types, QosProfiles, Applications and
    /// DomainParticipants under their fully qualified names, and the topics,
    /// publishers, subscribers, DataWriters and DataReaders inside each
    /// DomainParticipant under their names.
    ///
    /// # Errors
    /// A DomainParticipant that cannot be planned, or two top-level objects
    /// (Types, QosProfiles, Applications, DomainParticipants) that derive one
    /// `ObjectId`: a client could not tell them apart.
    pub fn predefined_objects(&self) -> Result<Vec<PredefinedObject>, String> {
        let mut out: Vec<PredefinedObject> = Vec::new();
        let mut top = |reference: &str, kind: ObjectKind| -> Result<(), String> {
            let oid = ObjectId::from_reference(reference, kind);
            if let Some(other) = out
                .iter()
                .find(|o| o.oid == oid && o.participant.is_none() && o.reference != reference)
            {
                return Err(format!(
                    "`{reference}` and `{}` derive the same ObjectId {:04x}",
                    other.reference,
                    oid.raw()
                ));
            }
            out.push(PredefinedObject {
                reference: reference.to_string(),
                kind,
                oid,
                participant: None,
            });
            Ok(())
        };
        for name in self.types.keys() {
            top(name, ObjectKind::Type)?;
        }
        for library in &self.qos_libraries {
            for profile in &library.profiles {
                top(
                    &format!("{}::{}", library.name, profile.name),
                    ObjectKind::QosProfile,
                )?;
            }
        }
        for app in self.definitions.applications() {
            top(&app.reference, ObjectKind::Application)?;
        }
        for def in self.definitions.participants() {
            top(&def.reference, ObjectKind::Participant)?;
        }
        for def in self.definitions.participants() {
            let participant = ObjectId::from_reference(&def.reference, ObjectKind::Participant);
            let plan = self
                .definitions
                .plan_participant(def, participant, &|t| self.resolve_type_ref(t))?;
            for nested in plan.nested {
                out.push(PredefinedObject {
                    reference: nested.name,
                    kind: nested.kind,
                    oid: nested.oid,
                    participant: Some(def.reference.clone()),
                });
            }
        }
        Ok(out)
    }

    /// Elements of the files the daemon does not use (`<transport_descriptors>`,
    /// `<requester>`, …), for the start-up log.
    #[must_use]
    pub fn unused(&self) -> &[String] {
        &self.unused
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// `agent.refs` as the eProsima Micro-XRCE-DDS Agent 3.0.1 build ships
    /// it (participant, writer, reader, topic, requester, replier).
    const EPROSIMA_REFS: &str = r#"<profiles>
    <participant profile_name="default_xrce_participant">
        <rtps><name>default_xrce_participant</name></rtps>
    </participant>
    <data_writer profile_name="shapetype_data_writer">
        <topic><kind>WITH_KEY</kind><name>Square</name><dataType>ShapeType</dataType></topic>
    </data_writer>
    <data_reader profile_name="shapetype_data_reader">
        <topic><kind>WITH_KEY</kind><name>Square</name><dataType>ShapeType</dataType></topic>
    </data_reader>
    <topic profile_name="shapetype_topic">
        <kind>WITH_KEY</kind><name>Square</name><dataType>ShapeType</dataType>
    </topic>
    <requester profile_name="shapetype_requester" service_name="shapetype_service"
               request_type="request_type" reply_type="reply_type"></requester>
    <replier profile_name="shapetype_replier" service_name="shapetype_service"
             request_type="request_type" reply_type="reply_type"></replier>
</profiles>"#;

    #[test]
    fn the_eprosima_refs_file_indexes_every_entity_profile() {
        let mut p = AgentProfiles::default();
        p.add_profiles(EPROSIMA_REFS).unwrap();
        assert!(
            p.entity("participant", "default_xrce_participant")
                .is_some()
        );
        assert!(p.entity("topic", "shapetype_topic").is_some());
        assert!(p.entity("data_writer", "shapetype_data_writer").is_some());
        assert!(p.entity("data_reader", "shapetype_data_reader").is_some());
        assert!(p.entity("topic", "shapetype_data_writer").is_none());
        assert_eq!(
            p.unused(),
            &[
                "<profiles>/<requester>".to_string(),
                "<profiles>/<replier>".to_string()
            ]
        );
    }

    #[test]
    fn a_profile_name_defined_twice_is_refused() {
        let mut p = AgentProfiles::default();
        let twice = "<profiles><topic profile_name=\"t\"><name>A</name></topic>\
                     <topic profile_name=\"t\"><name>B</name></topic></profiles>";
        assert!(p.add_profiles(twice).is_err());
    }

    #[test]
    fn types_and_qos_libraries_in_a_dds_document_are_read() {
        let mut p = AgentProfiles::default();
        p.add_profiles(
            "<dds><types><struct name=\"ShapeType\">\
             <member name=\"color\" type=\"string\" key=\"true\"/>\
             <member name=\"x\" type=\"int32\"/></struct></types>\
             <qos_library name=\"Lib\"><qos_profile name=\"Base\"><datawriter_qos>\
             <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>\
             </datawriter_qos></qos_profile>\
             <qos_profile name=\"Derived\" base_name=\"Base\"><datawriter_qos>\
             <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>\
             </datawriter_qos></qos_profile></qos_library></dds>",
        )
        .unwrap();
        assert!(p.types().contains_key("ShapeType"));
        let resolved = p.qos_profile("Lib::Derived").unwrap();
        let dw = &resolved.datawriter_qos.first().expect("dw").qos;
        assert!(dw.reliability.is_some(), "inherited from Base");
        assert!(dw.durability.is_some());
        assert!(p.qos_profile("Lib::Missing").is_err());
    }

    /// The configuration file of DDS-XRCE 1.0 §9.3.1 (the domain it names
    /// but does not define is added), loaded through `xml_profile_files:`:
    /// every row of Table 16 is an XRCE object with the ObjectId the table
    /// gives. The table spells the application
    /// "MyApplications::SimpleShapesDemoApplication" and the reader
    /// "MyTriangleReader", the file "ShapesDemoApp" and "MyTriangleRdr";
    /// the rows are checked against the strings of the file.
    #[test]
    fn the_example_file_of_the_spec_defines_the_objects_of_table_16() {
        let mut p = AgentProfiles::default();
        p.add_profiles(
            r#"<dds>
              <types><module name="ShapesDemoTypes"><struct name="ShapeType">
                <member name="color" key="true" type="string" stringMaxLength="128"/>
                <member name="x" type="int32"/><member name="y" type="int32"/>
                <member name="shapesize" type="int32"/></struct></module></types>
              <qos_library name="MyQosLibrary"><qos_profile name="MyQosProfile">
                <datawriter_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
                </datawriter_qos></qos_profile></qos_library>
              <domain_library name="ShapesDomainLibrary">
                <domain name="ShapesDomain" domain_id="0"/></domain_library>
              <application_library name="MyApplications">
                <application name="ShapesDemoApp">
                  <domain_participant name="MyParticipant"
                      domain_ref="ShapesDomainLibrary::ShapesDomain">
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
                      <data_reader name="MyTriangleRdr" topic_ref="Triangle"/>
                    </subscriber>
                  </domain_participant>
                </application>
              </application_library></dds>"#,
        )
        .unwrap();
        let objects = p.predefined_objects().unwrap();
        let id_of = |reference: &str, kind: ObjectKind| {
            objects
                .iter()
                .find(|o| o.reference == reference && o.kind == kind)
                .unwrap_or_else(|| panic!("no {kind:?} `{reference}`"))
                .oid
                .to_bytes()
        };
        assert_eq!(
            id_of("ShapesDemoTypes::ShapeType", ObjectKind::Type),
            [0x59, 0x5a]
        );
        assert_eq!(
            id_of("MyQosLibrary::MyQosProfile", ObjectKind::QosProfile),
            [0x3a, 0x3b]
        );
        assert_eq!(
            id_of(
                "MyApplications::ShapesDemoApp::MyParticipant",
                ObjectKind::Participant
            ),
            [0x56, 0xc1]
        );
        assert_eq!(id_of("Square", ObjectKind::Topic), [0xce, 0xb2]);
        assert_eq!(id_of("Circle", ObjectKind::Topic), [0x30, 0x92]);
        assert_eq!(id_of("Triangle", ObjectKind::Topic), [0x5e, 0x52]);
        assert_eq!(id_of("MyPublisher", ObjectKind::Publisher), [0x13, 0xe3]);
        assert_eq!(id_of("MySubscriber", ObjectKind::Subscriber), [0xae, 0x04]);
        assert_eq!(
            id_of("MySquareWriter", ObjectKind::DataWriter),
            [0x1c, 0xc5]
        );
        assert_eq!(
            id_of("MyCircleWriter", ObjectKind::DataWriter),
            [0xcf, 0x85]
        );
        assert_eq!(
            id_of("MyApplications::ShapesDemoApp", ObjectKind::Application),
            ObjectId::from_reference("MyApplications::ShapesDemoApp", ObjectKind::Application)
                .to_bytes()
        );
        assert_eq!(
            id_of("MyTriangleRdr", ObjectKind::DataReader),
            ObjectId::from_reference("MyTriangleRdr", ObjectKind::DataReader).to_bytes()
        );
        assert_eq!(
            p.resolve_type_ref("ShapeType").as_deref(),
            Some("ShapesDemoTypes::ShapeType")
        );
        assert_eq!(
            p.definitions()
                .domain("ShapesDomainLibrary::ShapesDomain")
                .unwrap()
                .domain_id,
            Some(0)
        );
        // The nested objects know their participant; the others do not.
        let reader = objects
            .iter()
            .find(|o| o.reference == "MyTriangleRdr")
            .unwrap();
        assert_eq!(
            reader.participant.as_deref(),
            Some("MyApplications::ShapesDemoApp::MyParticipant")
        );
        assert!(
            objects
                .iter()
                .find(|o| o.kind == ObjectKind::Type)
                .unwrap()
                .participant
                .is_none()
        );
    }

    /// A library is a document of its own (DDS-XML gives each building block
    /// both roots), and two top-level objects that derive one ObjectId are
    /// refused when the file is read.
    #[test]
    fn a_library_can_be_the_root_and_a_clash_of_ids_is_refused() {
        let mut p = AgentProfiles::default();
        p.add_profiles(
            r#"<domain_participant_library name="L">
                 <domain_participant name="P" domain_id="1"/></domain_participant_library>"#,
        )
        .unwrap();
        assert!(p.definitions().participant("L::P").is_some());
        let found = p.predefined_objects().unwrap();
        assert_eq!(found.len(), 1);
        // Find another name with the same ObjectId and define it as a second
        // participant: a client could not tell them apart.
        let target = ObjectId::from_reference("L::P", ObjectKind::Participant);
        let other = (0u32..)
            .map(|n| format!("Q{n}"))
            .find(|name| {
                ObjectId::from_reference(&format!("L::{name}"), ObjectKind::Participant) == target
            })
            .unwrap();
        p.add_profiles(&format!(
            "<domain_participant_library name=\"L\"><domain_participant name=\"{other}\" \
             domain_id=\"1\"/></domain_participant_library>"
        ))
        .unwrap();
        let err = p.predefined_objects().unwrap_err();
        assert!(err.contains("derive the same ObjectId"), "{err}");
    }
}
