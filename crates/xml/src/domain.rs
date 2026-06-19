// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 §7.3.4 Building Block "Domain Library".
//!
//! A `<domain_library>` carries 1+ `<domain>` entries; each carries
//! a numeric `domain_id`, plus 0+ `<register_type>` and 0+
//! `<topic>` entries. Topics carry a `register_type_ref` (a reference
//! to the `<register_type>` element within the same domain) and
//! optionally an inline `<topic_qos>` or a `qos_profile_ref` attribute.
//!
//! Spec source: OMG DDS-XML 1.0 §7.3.4 (Domain Library Building Block).
//!
//! # XML → Rust type mapping
//!
//! ```text
//! <domain_library name=…>          | DomainLibrary
//! <domain name=… domain_id=…>      | DomainEntry
//! <register_type name=… type_ref=…>| RegisterType
//! <topic name=… register_type_ref=…> | TopicEntry
//! <topic_qos>…                     | TopicEntry.topic_qos (EntityQos)
//! qos_profile_ref="lib::profile"   | TopicEntry.qos_profile_ref (String)
//! topic_filter (attr or child)     | TopicEntry.topic_filter
//! ```

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::parser::{XmlElement, parse_xml_tree};
use crate::qos::EntityQos;
use crate::qos_parser::parse_entity_qos_public;
use crate::types::parse_ulong;

/// Container for 1+ domain definitions (§7.3.4.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainLibrary {
    /// Name of the library (`<domain_library name="…">`).
    pub name: String,
    /// Domain definitions in document order.
    pub domains: Vec<DomainEntry>,
}

impl DomainLibrary {
    /// Looks up a domain by its name.
    #[must_use]
    pub fn domain(&self, name: &str) -> Option<&DomainEntry> {
        self.domains.iter().find(|d| d.name == name)
    }
}

/// A single `<domain>` definition (§7.3.4.4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainEntry {
    /// Name of the domain (attribute `name`).
    pub name: String,
    /// Numeric domain ID (attribute `domain_id`, 0..=232).
    pub domain_id: u32,
    /// Spec §7.3.4.4.2: optional `base_name` reference to a previously
    /// defined domain. The inheritance resolver combines the
    /// `register_types` and `topics` of the parent chain with the local
    /// definitions.
    pub base_name: Option<String>,
    /// Type registrations within this domain.
    pub register_types: Vec<RegisterType>,
    /// Topic definitions within this domain.
    pub topics: Vec<TopicEntry>,
}

impl DomainEntry {
    /// Returns the type registration with the given name.
    #[must_use]
    pub fn register_type(&self, name: &str) -> Option<&RegisterType> {
        self.register_types.iter().find(|r| r.name == name)
    }
    /// Returns the topic with the given name.
    #[must_use]
    pub fn topic(&self, name: &str) -> Option<&TopicEntry> {
        self.topics.iter().find(|t| t.name == name)
    }
}

/// `<register_type name="…" type_ref="…"/>` (§7.3.4.4.4).
///
/// `type_ref` refers to an XTypes type definition from the
/// `types` building block (Cluster H — separate C7.D task).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegisterType {
    /// Name under which the type is registered.
    pub name: String,
    /// Reference to a `<type>` definition (`module::TypeName`).
    pub type_ref: String,
}

/// `<topic name="…" register_type_ref="…">` (§7.3.4.4.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopicEntry {
    /// Topic name (attribute `name`).
    pub name: String,
    /// Name of the type registration in the same domain.
    pub register_type_ref: String,
    /// Inline `<topic_qos>` container (may co-exist with `qos_profile_ref`;
    /// on conflict the inline container wins).
    pub topic_qos: Option<EntityQos>,
    /// Optional `qos_profile_ref` attribute reference (`library::profile`).
    pub qos_profile_ref: Option<String>,
    /// Optional topic filter glob (Spec Annex C, Cyclone/FastDDS
    /// convention; `<topic_filter>` as a child or attribute).
    pub topic_filter: Option<String>,
}

/// Parses all `<domain_library>` entries from a `<dds>` root element.
///
/// # Errors
/// * [`XmlError::InvalidXml`] — no `<dds>` root or XML not
///   well-formed.
/// * Further errors from the per-element parsers.
pub fn parse_domain_libraries(xml: &str) -> Result<Vec<DomainLibrary>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    if doc.root.name != "dds" {
        return Err(XmlError::InvalidXml(format!(
            "expected <dds> root, got <{}>",
            doc.root.name
        )));
    }
    let mut libs = Vec::new();
    for lib_node in doc.root.children_named("domain_library") {
        libs.push(parse_domain_library_element(lib_node)?);
    }
    Ok(libs)
}

pub(crate) fn parse_domain_library_element(el: &XmlElement) -> Result<DomainLibrary, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain_library@name".into()))?
        .to_string();
    let mut domains = Vec::new();
    for d in el.children_named("domain") {
        domains.push(parse_domain_element(d)?);
    }
    Ok(DomainLibrary { name, domains })
}

fn parse_domain_element(el: &XmlElement) -> Result<DomainEntry, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain@name".into()))?
        .to_string();
    let domain_id_str = el
        .attribute("domain_id")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain@domain_id".into()))?;
    let domain_id = parse_ulong(domain_id_str)?;
    if domain_id > 232 {
        // Spec §2.1.2.2.1.1: domain_id range 0..=232.
        return Err(XmlError::ValueOutOfRange(format!(
            "domain_id `{domain_id}` exceeds 232"
        )));
    }

    let base_name = el.attribute("base_name").map(ToString::to_string);
    let mut register_types = Vec::new();
    let mut topics = Vec::new();
    for child in &el.children {
        match child.name.as_str() {
            "register_type" => register_types.push(parse_register_type(child)?),
            "topic" => topics.push(parse_topic_entry(child)?),
            _ => {}
        }
    }

    Ok(DomainEntry {
        name,
        domain_id,
        base_name,
        register_types,
        topics,
    })
}

fn parse_register_type(el: &XmlElement) -> Result<RegisterType, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("register_type@name".into()))?
        .to_string();
    // type_ref is optional in the spec — some spellings simply use
    // `<register_type name="…"/>` and reference the type
    // implicitly via the `name`. We accept both; if type_ref is
    // missing we fall back to `name`.
    let type_ref = el
        .attribute("type_ref")
        .map_or_else(|| name.clone(), ToString::to_string);
    Ok(RegisterType { name, type_ref })
}

fn parse_topic_entry(el: &XmlElement) -> Result<TopicEntry, XmlError> {
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("topic@name".into()))?
        .to_string();
    let register_type_ref = el
        .attribute("register_type_ref")
        .ok_or_else(|| XmlError::MissingRequiredElement("topic@register_type_ref".into()))?
        .to_string();
    let qos_profile_ref = el.attribute("qos_profile_ref").map(ToString::to_string);
    let mut topic_filter = el.attribute("topic_filter").map(ToString::to_string);
    let mut topic_qos: Option<EntityQos> = None;
    for child in &el.children {
        match child.name.as_str() {
            "topic_qos" => topic_qos = Some(parse_entity_qos_public(child)?),
            "topic_filter" => topic_filter = Some(child.text.clone()),
            _ => {}
        }
    }
    Ok(TopicEntry {
        name,
        register_type_ref,
        topic_qos,
        qos_profile_ref,
        topic_filter,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_domain_library() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="D" domain_id="42"/>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].name, "L");
        assert_eq!(libs[0].domains[0].name, "D");
        assert_eq!(libs[0].domains[0].domain_id, 42);
    }

    #[test]
    fn parse_domain_with_topic() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="D" domain_id="0">
              <register_type name="StateType" type_ref="MyTypes::State"/>
              <topic name="StateTopic" register_type_ref="StateType"/>
            </domain>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        let d = &libs[0].domains[0];
        assert_eq!(d.register_types[0].name, "StateType");
        assert_eq!(d.register_types[0].type_ref, "MyTypes::State");
        assert_eq!(d.topics[0].name, "StateTopic");
        assert_eq!(d.topics[0].register_type_ref, "StateType");
    }

    #[test]
    fn parse_topic_with_inline_qos() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="D" domain_id="1">
              <register_type name="T1"/>
              <topic name="Topic1" register_type_ref="T1">
                <topic_qos>
                  <reliability><kind>RELIABLE</kind></reliability>
                </topic_qos>
              </topic>
            </domain>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        let t = &libs[0].domains[0].topics[0];
        assert!(t.topic_qos.is_some());
        let q = t.topic_qos.as_ref().unwrap();
        assert!(q.reliability.is_some());
    }

    #[test]
    fn missing_domain_id_rejected() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="D"/>
          </domain_library>
        </dds>"#;
        let err = parse_domain_libraries(xml).expect_err("missing");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    #[test]
    fn domain_id_out_of_range() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="D" domain_id="500"/>
          </domain_library>
        </dds>"#;
        let err = parse_domain_libraries(xml).expect_err("oor");
        assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    }

    #[test]
    fn topic_missing_register_type_ref_rejected() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="D" domain_id="0">
              <topic name="T1"/>
            </domain>
          </domain_library>
        </dds>"#;
        let err = parse_domain_libraries(xml).expect_err("missing");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    // ---- §7.3.4.4.2 Domain Inheritance via base_name ----------------

    #[test]
    fn parse_domain_with_base_name() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="Base" domain_id="0">
              <register_type name="T"/>
            </domain>
            <domain name="Derived" domain_id="1" base_name="Base">
              <register_type name="T2"/>
            </domain>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        let lib = &libs[0];
        let base = lib.domains.iter().find(|d| d.name == "Base").expect("base");
        let derived = lib
            .domains
            .iter()
            .find(|d| d.name == "Derived")
            .expect("derived");
        assert_eq!(base.base_name, None);
        assert_eq!(derived.base_name.as_deref(), Some("Base"));
    }

    #[test]
    fn domain_inheritance_chain_resolves_via_resolve_chain() {
        // Spec §7.3.4.4.2: domain inheritance uses the same
        // resolve_chain mechanism as qos_profile.
        use crate::inheritance::resolve_chain;
        use alloc::collections::BTreeMap;

        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="A" domain_id="0"/>
            <domain name="B" domain_id="1" base_name="A"/>
            <domain name="C" domain_id="2" base_name="B"/>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        let lib = &libs[0];
        let mut by_name: BTreeMap<String, Option<String>> = BTreeMap::new();
        for d in &lib.domains {
            by_name.insert(d.name.clone(), d.base_name.clone());
        }
        let chain = resolve_chain("C", |n| {
            by_name
                .get(n)
                .cloned()
                .ok_or_else(|| XmlError::MissingRequiredElement(n.into()))
        })
        .expect("chain");
        assert_eq!(chain, alloc::vec!["A".to_string(), "B".into(), "C".into()]);
    }

    #[test]
    fn domain_inheritance_cycle_detected() {
        use crate::inheritance::resolve_chain;
        use alloc::collections::BTreeMap;

        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="A" domain_id="0" base_name="B"/>
            <domain name="B" domain_id="1" base_name="A"/>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        let lib = &libs[0];
        let mut by_name: BTreeMap<String, Option<String>> = BTreeMap::new();
        for d in &lib.domains {
            by_name.insert(d.name.clone(), d.base_name.clone());
        }
        let err = resolve_chain("A", |n| {
            by_name
                .get(n)
                .cloned()
                .ok_or_else(|| XmlError::MissingRequiredElement(n.into()))
        })
        .expect_err("cycle");
        assert!(matches!(err, XmlError::CircularInheritance(_)));
    }
}
