// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.6 Building Block "Application Library".
//!
//! An application binds one or more domain participants into a logical
//! deployment unit. The participants are written out in full inside the
//! `<application>`; one that reuses a library definition says so with
//! `base_name` (§7.3.6.4.2).
//!
//! # XML → Rust type mapping
//!
//! ```text
//! <application_library name=…>     | ApplicationLibrary
//! <application name=…>             | ApplicationEntry
//! <domain_participant name=…>      | ApplicationEntry.domain_participants[i]
//! ```

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::options::{Ctx, Parsed, ReadOptions};
use crate::parser::{XmlElement, parse_xml_tree};
use crate::participant::{DomainParticipantEntry, parse_dp_element};

/// Container for 1+ application definitions (§7.3.6.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplicationLibrary {
    /// Library name.
    pub name: String,
    /// Application definitions.
    pub applications: Vec<ApplicationEntry>,
    /// How many `<domain_participant_library>` elements precede this
    /// library in the `<dds>` document it was read from; `None` for a
    /// library read on its own. A participant of an application "can only
    /// inherit from DomainParticipants in DomainParticipant Libraries that
    /// have been defined before its own definition" (§7.3.6.4.2).
    pub participant_libraries_before: Option<usize>,
}

impl ApplicationLibrary {
    /// Looks up an application by its name.
    #[must_use]
    pub fn application(&self, name: &str) -> Option<&ApplicationEntry> {
        self.applications.iter().find(|a| a.name == name)
    }
}

/// A single `<application>` entry (§7.3.6.4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplicationEntry {
    /// Application name.
    pub name: String,
    /// The application's domain participants, defined inline.
    ///
    /// `dds-xml_application_definitions_nonamespace.xsd` includes the
    /// DomainParticipant building block's schema and types these
    /// children as `domainParticipant` — a full definition, not a
    /// reference. A participant reuses a library definition through the
    /// `base_name` attribute (§7.3.6.4.2), which is
    /// [`DomainParticipantEntry::base_name`], and not through a `ref`
    /// attribute, which the schema does not define.
    pub domain_participants: Vec<DomainParticipantEntry>,
}

/// Parses all `<application_library>` entries from a `<dds>` or a bare
/// `<application_library>` document (§7.3.6.3 gives this building block
/// both roots — see
/// [`crate::parser::DdsXmlDocument::library_elements`]).
///
/// # Errors
/// As [`crate::parse_xml_tree`] plus spec validation.
pub fn parse_application_libraries(xml: &str) -> Result<Vec<ApplicationLibrary>, XmlError> {
    Ok(parse_application_libraries_with(xml, &ReadOptions::default())?.value)
}

/// As [`parse_application_libraries`], with reading options.
///
/// # Errors
/// As [`parse_application_libraries`].
pub fn parse_application_libraries_with(
    xml: &str,
    options: &ReadOptions,
) -> Result<Parsed<Vec<ApplicationLibrary>>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    let mut ctx = Ctx::new(*options);
    let mut libs = Vec::new();
    for lib_node in doc.library_elements("application_library")? {
        libs.push(parse_app_library_element(&mut ctx, lib_node)?);
    }
    Ok(ctx.finish(libs))
}

pub(crate) fn parse_app_library_element(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<ApplicationLibrary, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("application_library@name".into()))?
        .to_string();
    let mut applications = Vec::new();
    for app_node in &el.children {
        match app_node.name.as_str() {
            "application" => applications.push(parse_app_element(ctx, app_node)?),
            _ => ctx.unknown_child(app_node)?,
        }
    }
    Ok(ApplicationLibrary {
        name,
        applications,
        participant_libraries_before: None,
    })
}

fn parse_app_element(ctx: &mut Ctx, el: &XmlElement) -> Result<ApplicationEntry, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("application@name".into()))?
        .to_string();
    let mut dps = Vec::new();
    for child in &el.children {
        match child.name.as_str() {
            "domain_participant" => dps.push(parse_dp_element(ctx, child)?),
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(ApplicationEntry {
        name,
        domain_participants: dps,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_application() {
        let xml = r#"<dds>
          <application_library name="al">
            <application name="App">
              <domain_participant name="P" domain_ref="dl::D"/>
            </application>
          </application_library>
        </dds>"#;
        let libs = parse_application_libraries(xml).expect("parse");
        assert_eq!(libs[0].name, "al");
        assert_eq!(libs[0].applications[0].name, "App");
        let dp = &libs[0].applications[0].domain_participants[0];
        assert_eq!(dp.name, "P");
        assert_eq!(dp.domain_ref.as_deref(), Some("dl::D"));
    }

    /// §7.3.6.4.2's own example: the participant is inline and reuses a
    /// library definition through `base_name`, with no `domain_ref` of
    /// its own.
    #[test]
    fn application_participant_may_inherit_instead_of_naming_a_domain() {
        let xml = r#"<application_library name="al">
            <application name="SimpleShapesDemoApplication">
              <domain_participant name="MyParticipant"
                  base_name="ShapesParticipantLibrary::MyParticipant"/>
            </application>
          </application_library>"#;
        let libs = parse_application_libraries(xml).expect("parse");
        let dp = &libs[0].applications[0].domain_participants[0];
        assert_eq!(dp.name, "MyParticipant");
        assert_eq!(dp.domain_ref, None);
        assert_eq!(
            dp.base_name.as_deref(),
            Some("ShapesParticipantLibrary::MyParticipant")
        );
    }

    /// §7.3.6.4.1.1's own example: a full participant definition, with
    /// publishers and writers, nested in the application.
    #[test]
    fn application_participant_carries_its_whole_definition() {
        let xml = r#"<application_library name="ShapesRelatedApplications">
            <application name="SimpleShapesDemoApplication">
              <domain_participant name="MyParticipant"
                  domain_ref="ShapesDomainLibrary::ShapesDomain">
                <publisher name="MyPublisher">
                  <data_writer name="MySquareWriter" topic_ref="Square"/>
                </publisher>
              </domain_participant>
            </application>
          </application_library>"#;
        let libs = parse_application_libraries(xml).expect("parse");
        let dp = &libs[0].applications[0].domain_participants[0];
        assert_eq!(dp.publishers[0].name, "MyPublisher");
        assert_eq!(dp.publishers[0].data_writers[0].name, "MySquareWriter");
        assert_eq!(dp.publishers[0].data_writers[0].topic_ref, "Square");
    }

    #[test]
    fn missing_app_name_rejected() {
        let xml = r#"<dds>
          <application_library name="al">
            <application/>
          </application_library>
        </dds>"#;
        let err = parse_application_libraries(xml).expect_err("missing");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    /// `name` is the one `use="required"` attribute of the
    /// `domainParticipant` complex type, so it is the one whose absence
    /// is an error here.
    #[test]
    fn missing_dp_name_rejected() {
        let xml = r#"<dds>
          <application_library name="al">
            <application name="A">
              <domain_participant/>
            </application>
          </application_library>
        </dds>"#;
        let err = parse_application_libraries(xml).expect_err("missing");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    /// The root the building block's own schema defines (§7.3.6.3), which
    /// is what four of the OMG's five example documents use.
    #[test]
    fn bare_application_library_root_is_a_document() {
        let xml = r#"<application_library name="MyApplicationLibrary">
            <application name="AnApplication">
              <domain_participant name="HelloParticipant" domain_id="32"/>
            </application>
          </application_library>"#;
        let libs = parse_application_libraries(xml).expect("parse");
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].name, "MyApplicationLibrary");
        assert_eq!(
            libs[0].applications[0].domain_participants[0].domain_id,
            Some(32)
        );
    }
}
