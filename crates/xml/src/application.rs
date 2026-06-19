// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.6 Building Block "Application Library".
//!
//! An application binds one or more domain participants
//! (`<domain_participant ref="…"/>`) into a logical deployment unit.
//! In the spec example an application typically references a
//! single participant; multiple are allowed (Spec §7.3.6.4.2).
//!
//! # XML → Rust type mapping
//!
//! ```text
//! <application_library name=…>     | ApplicationLibrary
//! <application name=…>             | ApplicationEntry
//! <domain_participant ref=…/>      | ApplicationEntry.domain_participants[i]
//! ```

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::parser::{XmlElement, parse_xml_tree};

/// Container for 1+ application definitions (§7.3.6.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplicationLibrary {
    /// Library name.
    pub name: String,
    /// Application definitions.
    pub applications: Vec<ApplicationEntry>,
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
    /// References to domain participants (`library::participant`).
    pub domain_participants: Vec<String>,
}

/// Parses all `<application_library>` entries from a `<dds>` root
/// element.
///
/// # Errors
/// As [`crate::parse_xml_tree`] plus spec validation.
pub fn parse_application_libraries(xml: &str) -> Result<Vec<ApplicationLibrary>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    if doc.root.name != "dds" {
        return Err(XmlError::InvalidXml(format!(
            "expected <dds> root, got <{}>",
            doc.root.name
        )));
    }
    let mut libs = Vec::new();
    for lib_node in doc.root.children_named("application_library") {
        libs.push(parse_app_library_element(lib_node)?);
    }
    Ok(libs)
}

pub(crate) fn parse_app_library_element(el: &XmlElement) -> Result<ApplicationLibrary, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("application_library@name".into()))?
        .to_string();
    let mut applications = Vec::new();
    for app_node in el.children_named("application") {
        applications.push(parse_app_element(app_node)?);
    }
    Ok(ApplicationLibrary { name, applications })
}

fn parse_app_element(el: &XmlElement) -> Result<ApplicationEntry, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("application@name".into()))?
        .to_string();
    let mut dps = Vec::new();
    for child in el.children_named("domain_participant") {
        let r = child
            .attribute("ref")
            .ok_or_else(|| {
                XmlError::MissingRequiredElement("application/domain_participant@ref".into())
            })?
            .to_string();
        dps.push(r);
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
              <domain_participant ref="dpl::P"/>
            </application>
          </application_library>
        </dds>"#;
        let libs = parse_application_libraries(xml).expect("parse");
        assert_eq!(libs[0].name, "al");
        assert_eq!(libs[0].applications[0].name, "App");
        assert_eq!(libs[0].applications[0].domain_participants[0], "dpl::P");
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

    #[test]
    fn missing_dp_ref_rejected() {
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
}
