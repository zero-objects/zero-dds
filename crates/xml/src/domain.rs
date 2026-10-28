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
//! qos_profile_ref="lib::profile"   | TopicEntry.qos_profile_ref (String, vendor form)
//! topic_filter (attr or child)     | TopicEntry.topic_filter (vendor form)
//! ```

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::options::{Ctx, Parsed, ReadOptions};
use crate::parser::{XmlElement, parse_xml_tree};
use crate::qos::EntityQos;
use crate::qos_parser::set_inline_qos;
use crate::types::parse_ulong;

/// Container for 1+ domain definitions (§7.3.4.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainLibrary {
    /// Name of the library (`<domain_library name="…">`).
    pub name: String,
    /// Domain definitions in document order.
    pub domains: Vec<DomainEntry>,
    /// How many `<types>` blocks precede this library in the `<dds>`
    /// document it was read from; `None` for a library read on its own.
    /// A `register_type` refers to a type "previously defined"
    /// (§7.3.4.4.1), so only those blocks count.
    pub type_libraries_before: Option<usize>,
}

impl DomainLibrary {
    /// Looks up a domain by its name.
    #[must_use]
    pub fn domain(&self, name: &str) -> Option<&DomainEntry> {
        self.domains.iter().find(|d| d.name == name)
    }

    /// Returns the `domain_id` that `entry` ends up with once its
    /// `base_name` chain is followed (§7.3.4.4.2).
    ///
    /// The nearest definition that carries the attribute wins; `None`
    /// means no definition in the chain carries one.
    ///
    /// # Errors
    /// * [`XmlError::CircularInheritance`] — `base_name` cycle.
    /// * [`XmlError::LimitExceeded`] — chain longer than
    ///   [`crate::inheritance::MAX_INHERITANCE_DEPTH`].
    /// * [`XmlError::UnresolvedReference`] — a `base_name` names a domain
    ///   this library does not define.
    pub fn effective_domain_id(&self, entry: &DomainEntry) -> Result<Option<u32>, XmlError> {
        Ok(self.effective_domain(entry)?.domain_id)
    }

    /// Returns `entry` with everything it inherits from its `base_name`
    /// chain (§7.3.4.4.2) merged in: the `domain_id`, the registered
    /// types and the topics. A derived definition overrides an inherited
    /// item of the same name; the rest is appended.
    ///
    /// # Errors
    /// As [`Self::effective_domain_id`].
    pub fn effective_domain(&self, entry: &DomainEntry) -> Result<DomainEntry, XmlError> {
        let local = |name: &str| -> Option<&DomainEntry> {
            match name.rsplit_once("::") {
                // A qualified base_name may only point into this library:
                // §7.3.4.4.2 requires the base to be defined before.
                Some((lib, tail)) if lib == self.name => self.domain(tail),
                Some(_) => None,
                None => self.domain(name),
            }
        };
        let chain = crate::inheritance::resolve_chain(&entry.name, |name| {
            let d = local(name)
                .ok_or_else(|| XmlError::UnresolvedReference(format!("domain `{name}`")))?;
            Ok(d.base_name.clone())
        })?;
        // §7.3.4.4.2: "A Domain can only inherit from domains that have
        // been defined before." The cycle check already ran.
        for pair in chain.windows(2) {
            let base_name = bare_name(&pair[0]);
            let derived_name = bare_name(&pair[1]);
            let base = self
                .domains
                .iter()
                .position(|d| d.name == base_name)
                .ok_or_else(|| XmlError::UnresolvedReference(format!("domain `{base_name}`")))?;
            let derived = self
                .domains
                .iter()
                .position(|d| d.name == derived_name)
                .ok_or_else(|| XmlError::UnresolvedReference(format!("domain `{derived_name}`")))?;
            if base >= derived {
                return Err(XmlError::InvalidXml(format!(
                    "domain `{}` inherits from `{}`, which is defined later (§7.3.4.4.2)",
                    pair[1], pair[0]
                )));
            }
        }
        // base-first: a later (more derived) definition overrides.
        let mut register_types: Vec<RegisterType> = Vec::new();
        let mut topics: Vec<TopicEntry> = Vec::new();
        let mut domain_id = None;
        for name in &chain {
            if let Some(d) = local(name) {
                merge_by_name(&mut register_types, &d.register_types, |r| r.name.clone());
                merge_by_name(&mut topics, &d.topics, |t| t.name.clone());
                if d.domain_id.is_some() {
                    domain_id = d.domain_id;
                }
            }
        }
        Ok(DomainEntry {
            name: entry.name.clone(),
            domain_id,
            base_name: None,
            register_types,
            topics,
        })
    }
}

/// Strips an optional `library::` qualifier from a reference.
fn bare_name(name: &str) -> &str {
    name.rsplit_once("::").map_or(name, |(_, tail)| tail)
}

/// Merges `override_` into `acc` by name: an item with an existing key
/// replaces it, a new one is appended.
fn merge_by_name<T, K, F>(acc: &mut Vec<T>, override_: &[T], key: F)
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

/// A single `<domain>` definition (§7.3.4.4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainEntry {
    /// Name of the domain (attribute `name`).
    pub name: String,
    /// Numeric domain ID (attribute `domain_id`), an `unsigned long` per
    /// §7.1.5 Tab.7.2.
    ///
    /// Optional. §7.3.4.4.2 defines domain inheritance through
    /// `base_name` and writes `<domain name="MyDomain"
    /// base_name="BaseDomain">` without a `domain_id` of its own; the
    /// schema marks the attribute `use="optional"` accordingly. Use
    /// [`DomainLibrary::effective_domain_id`] to read the value a domain
    /// ends up with after its base chain.
    pub domain_id: Option<u32>,
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

/// `<register_type name="…" type_ref="…"/>` (§7.3.4.4.1).
///
/// `type_ref` refers to an XTypes type definition from the
/// `types` building block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegisterType {
    /// Name under which the type is registered.
    pub name: String,
    /// Reference to a `<type>` definition (`module::TypeName`).
    pub type_ref: String,
    /// The `format` attribute of the schema (`registerType`): how the
    /// registered type is represented. Carried, not interpreted.
    pub format: Option<String>,
    /// The `<registered_name>` child of the schema: the name the type is
    /// registered under with the DDS implementation (`register_type`'s
    /// `type_name`), where it differs from `name`.
    pub registered_name: Option<String>,
}

/// `<topic name="…" register_type_ref="…">` (§7.3.4.4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopicEntry {
    /// Topic name (attribute `name`).
    pub name: String,
    /// Name of the type registration in the same domain.
    pub register_type_ref: String,
    /// The `<registered_name>` child the schema (`topic`) allows next to
    /// the QoS. Carried, not interpreted.
    pub registered_name: Option<String>,
    /// Inline `<topic_qos>` container (may co-exist with `qos_profile_ref`;
    /// on conflict the inline container wins).
    pub topic_qos: Option<EntityQos>,
    /// Optional `qos_profile_ref` attribute reference (`library::profile`).
    pub qos_profile_ref: Option<String>,
    /// Optional topic filter glob (`<topic_filter>` as a child or an
    /// attribute). DDS-XML 1.0 defines `topic_filter` only on the QoS
    /// containers (§7.3.2.4.3); on a `<topic>` it is an extension.
    pub topic_filter: Option<String>,
}

/// Parses all `<domain_library>` entries from a `<dds>` or a bare
/// `<domain_library>` document (§7.3.4.3 gives this building block both
/// roots — see [`crate::parser::DdsXmlDocument::library_elements`]).
///
/// # Errors
/// * [`XmlError::InvalidXml`] — XML not well-formed, or the root is
///   neither `<dds>` nor `<domain_library>`.
/// * Further errors from the per-element parsers.
pub fn parse_domain_libraries(xml: &str) -> Result<Vec<DomainLibrary>, XmlError> {
    Ok(parse_domain_libraries_with(xml, &ReadOptions::default())?.value)
}

/// As [`parse_domain_libraries`], with reading options.
///
/// # Errors
/// As [`parse_domain_libraries`].
pub fn parse_domain_libraries_with(
    xml: &str,
    options: &ReadOptions,
) -> Result<Parsed<Vec<DomainLibrary>>, XmlError> {
    let doc = parse_xml_tree(xml)?;
    let mut ctx = Ctx::new(*options);
    let mut libs = Vec::new();
    for lib_node in doc.library_elements("domain_library")? {
        libs.push(parse_domain_library_element(&mut ctx, lib_node)?);
    }
    Ok(ctx.finish(libs))
}

pub(crate) fn parse_domain_library_element(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<DomainLibrary, XmlError> {
    ctx.check_attributes(el, &["name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain_library@name".into()))?
        .to_string();
    let mut domains = Vec::new();
    for d in &el.children {
        match d.name.as_str() {
            "domain" => domains.push(parse_domain_element(ctx, d)?),
            _ => ctx.unknown_child(d)?,
        }
    }
    Ok(DomainLibrary {
        name,
        domains,
        type_libraries_before: None,
    })
}

fn parse_domain_element(ctx: &mut Ctx, el: &XmlElement) -> Result<DomainEntry, XmlError> {
    // `base_name` is §7.3.4.4.2's; the schema (`domain`) does not declare it.
    ctx.check_attributes(el, &["name", "domain_id", "base_name"])?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("domain@name".into()))?
        .to_string();
    // §7.3.4.4.2: a domain that inherits carries no `domain_id` of its
    // own. Absent means "take it from the base chain", not zero.
    // §7.1.5 Tab.7.2 types `unsigned long`; no narrower bound is defined
    // by DDS-XML, so the value is whatever fits the field.
    let domain_id = match el.attribute("domain_id") {
        Some(raw) => Some(parse_ulong(raw)?),
        None => None,
    };

    let base_name = el.attribute("base_name").map(ToString::to_string);
    let mut register_types = Vec::new();
    let mut topics = Vec::new();
    for child in &el.children {
        match child.name.as_str() {
            "register_type" => register_types.push(parse_register_type(ctx, child)?),
            "topic" => topics.push(parse_topic_entry(ctx, child)?),
            // §7.3.5.4.1: each entity is nested under its parent. An
            // unknown or misplaced child is reported, not skipped.
            _ => ctx.unknown_child(child)?,
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

/// The `qos_profile_ref` attribute of an entity, which no schema of the
/// specification declares: the entity names its QoS profile with
/// `base_name` on its inline QoS container (§7.3.5.4.4).
pub(crate) fn qos_profile_ref(
    ctx: &mut Ctx,
    el: &XmlElement,
    container: &str,
) -> Result<Option<String>, XmlError> {
    let Some(value) = el.attribute("qos_profile_ref") else {
        return Ok(None);
    };
    ctx.vendor_form(
        &alloc::format!("{}@qos_profile_ref", el.path),
        &alloc::format!("the qos_profile_ref attribute of <{}>", el.name),
        &alloc::format!("<{container} base_name=\"library::profile\"> (§7.3.5.4.4)"),
    )?;
    Ok(Some(value.to_string()))
}

/// The text of a `<registered_name>` child (`elementName` in the schema),
/// at most once.
fn registered_name(el: &XmlElement) -> Result<Option<String>, XmlError> {
    let mut found = el.children_named("registered_name");
    let first = found.next();
    if found.next().is_some() {
        return Err(XmlError::InvalidXml(format!(
            "<{}> carries <registered_name> more than once",
            el.path
        )));
    }
    Ok(first.map(|c| c.text.trim().to_string()))
}

pub(crate) fn parse_register_type(
    ctx: &mut Ctx,
    el: &XmlElement,
) -> Result<RegisterType, XmlError> {
    ctx.check_attributes(el, &["name", "type_ref", "format"])?;
    for c in el.children.iter().filter(|c| c.name != "registered_name") {
        ctx.unknown_child(c)?;
    }
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("register_type@name".into()))?
        .to_string();
    // §7.3.4.4.1: "Register types shall provide a reference to data types
    // that have been previously defined using the type_ref XML
    // attribute." It is required. (The schema declares it `optional`; the
    // text says "shall", and a registration without it names no type.)
    let type_ref = el
        .attribute("type_ref")
        .ok_or_else(|| XmlError::MissingRequiredElement("register_type@type_ref".into()))?
        .to_string();
    Ok(RegisterType {
        name,
        type_ref,
        format: el.attribute("format").map(ToString::to_string),
        registered_name: registered_name(el)?,
    })
}

pub(crate) fn parse_topic_entry(ctx: &mut Ctx, el: &XmlElement) -> Result<TopicEntry, XmlError> {
    // `qos_profile_ref` and `topic_filter` are ZeroDDS attributes; the
    // schema (`topic`) has `name` and `register_type_ref`.
    ctx.check_attributes(
        el,
        &[
            "name",
            "register_type_ref",
            "qos_profile_ref",
            "topic_filter",
        ],
    )?;
    let name = el
        .attribute("name")
        .ok_or_else(|| XmlError::MissingRequiredElement("topic@name".into()))?
        .to_string();
    let register_type_ref = el
        .attribute("register_type_ref")
        .ok_or_else(|| XmlError::MissingRequiredElement("topic@register_type_ref".into()))?
        .to_string();
    let qos_profile_ref = qos_profile_ref(ctx, el, "topic_qos")?;
    let mut topic_filter = el.attribute("topic_filter").map(ToString::to_string);
    if topic_filter.is_some() {
        ctx.vendor_form(
            &alloc::format!("{}@topic_filter", el.path),
            "the topic_filter attribute of <topic>",
            "the topic_filter attribute of a QoS container (§7.3.2.4.3)",
        )?;
    }
    let mut topic_qos: Option<EntityQos> = None;
    for child in &el.children {
        match child.name.as_str() {
            "topic_qos" => set_inline_qos(ctx, &mut topic_qos, "topic", child)?,
            "topic_filter" => {
                ctx.vendor_form(
                    &child.path,
                    "<topic_filter> as a child of <topic>",
                    "the topic_filter attribute of a QoS container (§7.3.2.4.3)",
                )?;
                topic_filter = Some(child.text.trim().to_string());
            }
            "registered_name" => {}
            _ => ctx.unknown_child(child)?,
        }
    }
    Ok(TopicEntry {
        name,
        register_type_ref,
        registered_name: registered_name(el)?,
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
        assert_eq!(libs[0].domains[0].domain_id, Some(42));
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
              <register_type name="T1" type_ref="T1"/>
              <topic name="Topic1" register_type_ref="T1">
                <topic_qos>
                  <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
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
    fn missing_domain_id_is_none_not_zero() {
        // §7.3.4.4.2 writes an inheriting domain without a domain_id of
        // its own, and the schema marks the attribute optional. A domain
        // that names none must read as `None` — reporting 0 would name a
        // real domain nobody wrote.
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="D"/>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        assert_eq!(libs[0].domains[0].domain_id, None);
    }

    #[test]
    fn domain_id_inherited_through_base_name() {
        // The §7.3.4.4.2 example: `<domain name="MyDomain"
        // base_name="BaseDomain">` with no domain_id of its own.
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="BaseDomain" domain_id="7"/>
            <domain name="MyDomain" base_name="BaseDomain"/>
            <domain name="Own" base_name="BaseDomain" domain_id="9"/>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        let lib = &libs[0];
        assert_eq!(
            lib.effective_domain_id(lib.domain("MyDomain").expect("MyDomain"))
                .expect("chain"),
            Some(7)
        );
        assert_eq!(
            lib.effective_domain_id(lib.domain("Own").expect("Own"))
                .expect("chain"),
            Some(9)
        );
        assert_eq!(
            lib.effective_domain_id(lib.domain("BaseDomain").expect("BaseDomain"))
                .expect("chain"),
            Some(7)
        );
    }

    /// §7.3.4.4.2: "A Domain can only inherit from domains that have been
    /// defined before."
    #[test]
    fn domain_forward_base_name_rejected() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="Derived" base_name="Base"/>
            <domain name="Base" domain_id="1"/>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        let lib = &libs[0];
        let err = lib
            .effective_domain(lib.domain("Derived").expect("Derived"))
            .expect_err("forward");
        assert!(matches!(err, XmlError::InvalidXml(_)), "got {err:?}");
    }

    #[test]
    fn effective_domain_id_reports_cycle() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="A" base_name="B"/>
            <domain name="B" base_name="A"/>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("parse");
        let lib = &libs[0];
        let err = lib
            .effective_domain_id(lib.domain("A").expect("A"))
            .expect_err("cycle");
        assert!(matches!(err, XmlError::CircularInheritance(_)));
    }

    #[test]
    fn domain_id_accepts_the_unsigned_long_range() {
        // §7.1.5 Tab.7.2: `unsigned long`. DDS-XML defines no narrower
        // bound, so 500 is a legal id.
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="D" domain_id="500"/>
          </domain_library>
        </dds>"#;
        let libs = parse_domain_libraries(xml).expect("500");
        assert_eq!(libs[0].domains[0].domain_id, Some(500));

        let too_big = r#"<dds>
          <domain_library name="L">
            <domain name="D" domain_id="4294967296"/>
          </domain_library>
        </dds>"#;
        assert!(matches!(
            parse_domain_libraries(too_big),
            Err(XmlError::ValueOutOfRange(_))
        ));
    }

    /// §7.3.4.4.1: "Register types shall provide a reference to data types
    /// that have been previously defined using the type_ref XML
    /// attribute." Without it the registration name is not a substitute.
    #[test]
    fn register_type_missing_type_ref_rejected() {
        let xml = r#"<dds>
          <domain_library name="L">
            <domain name="D" domain_id="0">
              <register_type name="T"/>
            </domain>
          </domain_library>
        </dds>"#;
        let err = parse_domain_libraries(xml).expect_err("missing type_ref");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
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
              <register_type name="T" type_ref="T"/>
            </domain>
            <domain name="Derived" domain_id="1" base_name="Base">
              <register_type name="T2" type_ref="T2"/>
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
