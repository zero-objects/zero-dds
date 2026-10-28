// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Generic well-formed XML loader for DDS-XML 1.0 §7.1.
//!
//! Provides a roxmltree-based parser that
//! adheres to the well-formedness rules from §7.1.1 (UTF-8,
//! whitespace-tolerant, comment-stripping, namespace-aware) and stores the
//! result in a generic [`DdsXmlDocument`] container.
//!
//! Building-block-specific decoders (QoS library, types, domains,
//! participants, applications, samples) build on this container
//! (`qos_parser`, `xtypes_parser`, `domain`, `participant`, `application`).

use crate::errors::XmlError;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Namespace of the `xsi:` attributes (`xsi:schemaLocation` and the like).
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// DDS-XML 1.0 spec namespace for building-block top-level elements.
///
/// Spec ref: §7.3.1 ("targetNamespace = <http://www.omg.org/spec/DDS-XML">).
pub const DDS_XML_NS: &str = "http://www.omg.org/spec/DDS-XML";

/// DoS cap for list elements (children per node).
pub const MAX_LIST_ELEMENTS: usize = 1024;

/// DoS cap for the total node count per document.
pub const MAX_TOTAL_ELEMENTS: usize = 64 * 1024;

/// DoS cap for the recursive tree depth.
///
/// Protects the recursive `build_element` call from stack overflow
/// on adversarial deeply-nested XML inputs (TS-1 finding 3,
/// `internal/test-harness/plan.md`). Realistic DDS-XML profiles go
/// 4-8 deep; even complex `<application>`/`<participant>`
/// nestings stay below 32.
pub const MAX_TREE_DEPTH: usize = 64;

/// Generic in-memory container for a DDS-XML document per §7.1.
///
/// An element is mapped as an [`XmlElement`] (tag, attributes, children, text).
/// Building-block decoders navigate over this tree and
/// produce typed structures (e.g. QoS profiles).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdsXmlDocument {
    /// Root element of the document (e.g. `<dds>`, `<qos_library>`,
    /// `<domain_participant_library>`).
    pub root: XmlElement,
}

impl DdsXmlDocument {
    /// Returns the local tag name of the root (without the namespace prefix).
    #[must_use]
    pub fn root_name(&self) -> &str {
        &self.root.name
    }

    /// Returns the building-block library elements named `element`,
    /// accepting **both** document roots the specification defines for
    /// them.
    ///
    /// Every building block's schema declares a top-level element of its
    /// own — §7.3.2.3 (`<qos_library>`), §7.3.3.3 (`<types>`), §7.3.4.3
    /// (`<domain_library>`), §7.3.5.3 (`<domain_participant_library>`)
    /// and §7.3.6.3 (`<application_library>`) each say the building
    /// block's `*_definitions.xsd` "defines the `<…>` root element and
    /// sets targetNamespace to `http://www.omg.org/spec/DDS-XML`". The
    /// composed `dds-xml_dds_system_definitions.xsd` adds `<dds>` as a
    /// root that carries all of them at once (§8.1). Both forms are
    /// legal documents, and the OMG's own example files use both:
    /// `dds-xml_qos_example.xml` opens with `<qos_library>`,
    /// `dds-xml_dds_system_example.xml` with `<dds>`.
    ///
    /// # Errors
    /// [`XmlError::InvalidXml`] if the root is neither `<dds>` nor
    /// `<element>`.
    pub fn library_elements<'a>(&'a self, element: &str) -> Result<Vec<&'a XmlElement>, XmlError> {
        if self.root.name == "dds" {
            // Not `children_named`: that one ties the name's lifetime to
            // the document's, and `element` outlives neither.
            return Ok(self
                .root
                .children
                .iter()
                .filter(|c| c.name == element)
                .collect());
        }
        if self.root.name == element {
            return Ok(alloc::vec![&self.root]);
        }
        Err(XmlError::InvalidXml(format!(
            "expected <dds> or <{element}> root, got <{}>",
            self.root.name
        )))
    }
}

/// A single XML element per §7.1.4 / §7.1.5 (element + attributes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XmlElement {
    /// Local tag name (without the namespace prefix).
    pub name: String,
    /// Optional namespace URI per §7.1.3.
    pub namespace: Option<String>,
    /// Path of the element from the root, for messages: the names of the
    /// elements on the way, each followed by the value of its `name`
    /// attribute in brackets where it has one
    /// (`dds/qos_library[L]/qos_profile[P]/datawriter_qos`).
    pub path: String,
    /// Attributes (name -> value, alphabetically sorted via BTreeMap for
    /// deterministic iteration). An attribute in a namespace is keyed
    /// `{namespace}local`; those of `xsi:` (schema location hints) and
    /// `xml:` are left out, they are no content.
    pub attributes: BTreeMap<String, String>,
    /// Child elements in document order.
    pub children: Vec<XmlElement>,
    /// Concatenated direct text content of the element (Spec §7.1.4
    /// Tab.7.1 `string`: whitespace is part of the value). Comments and
    /// processing instructions between text runs do not truncate it.
    /// Empty if the element has no text.
    pub text: String,
}

impl XmlElement {
    /// Returns the first child with the given local name, if
    /// present.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&XmlElement> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Returns all children with the given local name.
    pub fn children_named<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a XmlElement> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// Returns the value of an attribute, if present.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    /// Iterates over `<element>` children per Spec §7.2.4.1
    /// (IDL sequence mapping) and §7.2.5 (IDL array mapping = sequence
    /// mapping with the same element tag).
    ///
    /// Spec OMG DDS-XML 1.0 §7.2.4.1: "The complexType contains zero
    /// or more elements named element. Nested inside each element is
    /// the XSD schema obtained from mapping the IDL type of the
    /// element itself."
    ///
    /// Spec §7.2.5: "The XML representation of IDL arrays is the same
    /// as it would be for IDL sequences of the same element type." —
    /// i.e. the `<element>` tag is also used for arrays.
    ///
    /// Used by `webdds-bridge` to iterate generic sequences/arrays;
    /// `qos_parser` reads `StringSeq` with `children_named("element")`.
    pub fn sequence_elements(&self) -> impl Iterator<Item = &XmlElement> + '_ {
        self.children_named("element")
    }
}

/// Parses a DDS-XML 1.0 document per §7.1.
///
/// Well-formedness guarantees:
/// * XML declaration optional (Spec §7.1.1).
/// * Whitespace-tolerant (roxmltree normalizes text).
/// * Comments are stripped (`<!-- ... -->` are not delivered by roxmltree
///   as element nodes).
/// * Namespace-aware: the top-level namespace is carried in the root
///   (Spec §7.3.1: `targetNamespace = http://www.omg.org/spec/DDS-XML`).
///
/// DoS caps: at most [`MAX_LIST_ELEMENTS`] children per
/// node, at most [`MAX_TOTAL_ELEMENTS`] nodes total.
///
/// # Errors
/// * [`XmlError::InvalidXml`] — XML not well-formed.
/// * [`XmlError::LimitExceeded`] — a DoS cap was hit.
pub fn parse_xml_tree(xml: &str) -> Result<DdsXmlDocument, XmlError> {
    // §7.1.1: "It shall use UTF-8 character encoding." roxmltree accepts
    // a `&str` and discards the declaration's encoding, so a different
    // one has to be refused here.
    check_declared_encoding(xml)?;
    check_entity_declarations(xml)?;
    // `roxmltree` descends once per open element, so nesting has to be
    // bounded before it runs, not after.
    precheck_depth(xml)?;
    // §7.1.1: a well-formed document per [XML] §2.1 may carry a DTD, so
    // one is accepted. roxmltree resolves internal entities with a loop
    // detector and does not fetch external ones, so this stays safe.
    let opts = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(xml, opts)
        .map_err(|e| XmlError::InvalidXml(e.to_string()))?;
    // Entity expansion can add nesting the scan above did not see; measure
    // the real element depth of the parsed tree, iteratively, before the
    // recursive build.
    check_tree_depth(doc.root_element())?;
    let mut counter: usize = 0;
    let root = build_element(doc.root_element(), "", &mut counter, 0)?;
    Ok(DdsXmlDocument { root })
}

/// Largest value an internal entity may declare, in bytes.
pub const MAX_ENTITY_VALUE_BYTES: usize = 256;

/// Largest number of `<!ENTITY …>` declarations in a document.
pub const MAX_ENTITY_DECLARATIONS: usize = 64;

/// Bounds what an internal DTD subset can expand to.
///
/// §7.1.1 asks for a well-formed document, which may declare entities.
/// `roxmltree` expands a reference when it builds the text, with no limit
/// on the size of the result: one declared entity referenced many times, or
/// entities referencing entities, grows a small document into gigabytes.
/// So internal entities are read only when each value is short and refers
/// to no other entity; the expansion is then at most
/// [`MAX_ENTITY_VALUE_BYTES`] per reference in the document. External
/// entities are never fetched (`roxmltree` ignores their declaration).
fn check_entity_declarations(xml: &str) -> Result<(), XmlError> {
    const MARKER: &str = "<!ENTITY";
    let mut declarations = 0usize;
    let mut rest = xml;
    while let Some(pos) = rest.find(MARKER) {
        rest = &rest[pos + MARKER.len()..];
        declarations += 1;
        if declarations > MAX_ENTITY_DECLARATIONS {
            return Err(XmlError::LimitExceeded(format!(
                "more than {MAX_ENTITY_DECLARATIONS} entity declarations"
            )));
        }
        // `<!ENTITY [%] Name Value>`: the value is the quoted string after
        // the name. SYSTEM/PUBLIC declarations name an external resource
        // and carry no text.
        let r = rest.trim_start();
        let r = r.strip_prefix('%').map_or(r, str::trim_start);
        let r = r.trim_start_matches(|c: char| !c.is_whitespace());
        let r = r.trim_start();
        let Some(quote) = r.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
            continue;
        };
        let tail = &r[1..];
        let Some(close) = tail.find(quote) else {
            continue;
        };
        let value = &tail[..close];
        if value.len() > MAX_ENTITY_VALUE_BYTES {
            return Err(XmlError::LimitExceeded(format!(
                "an entity declares {} bytes, over {MAX_ENTITY_VALUE_BYTES}",
                value.len()
            )));
        }
        let mut refs = value.split('&').skip(1);
        if refs.any(|r| {
            let name = r.split(';').next().unwrap_or_default();
            !(name.starts_with('#') || matches!(name, "lt" | "gt" | "amp" | "apos" | "quot"))
        }) {
            return Err(XmlError::LimitExceeded(
                "an entity value refers to another entity".into(),
            ));
        }
    }
    Ok(())
}

/// Iteratively scans the source for the element nesting depth and refuses
/// a document that opens more than [`MAX_TREE_DEPTH`] elements at once.
///
/// This runs before `roxmltree` sees the text: its parser recurses once
/// per open element, so an unclosed run of start tags from the wire would
/// otherwise overflow the stack before any later check ran. The scan skips
/// what is not markup: comments, CDATA sections, processing instructions,
/// the DOCTYPE with its internal subset, and quoted attribute values (a `>`
/// inside a quote does not end a tag). A malformed document may be
/// miscounted; it is refused by the parser afterwards in any case.
fn precheck_depth(xml: &str) -> Result<(), XmlError> {
    zerodds_foundation::xml_depth::check_nesting(xml, MAX_TREE_DEPTH).map_err(|_| {
        XmlError::LimitExceeded(format!(
            "tag nesting exceeds {MAX_TREE_DEPTH} — refusing to parse to \
             protect against stack overflow"
        ))
    })
}

/// Iteratively checks that no element in the parsed tree nests deeper
/// than [`MAX_TREE_DEPTH`].
///
/// Unlike a byte scan, this cannot mistake `<` inside a comment, a CDATA
/// section or an attribute value for a tag (the old `precheck_depth` did,
/// and could reject a well-formed document).
fn check_tree_depth(root: roxmltree::Node<'_, '_>) -> Result<(), XmlError> {
    let mut stack: Vec<(roxmltree::Node<'_, '_>, usize)> = alloc::vec![(root, 0)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_TREE_DEPTH {
            return Err(XmlError::LimitExceeded(format!(
                "tag nesting exceeds {MAX_TREE_DEPTH} — refusing to parse to \
                 protect against stack overflow"
            )));
        }
        for child in node.children().filter(roxmltree::Node::is_element) {
            stack.push((child, depth + 1));
        }
    }
    Ok(())
}

/// Rejects an XML declaration that names an encoding other than UTF-8
/// (Spec §7.1.1). An absent declaration is fine: the input is already a
/// `&str`, so it is UTF-8.
fn check_declared_encoding(xml: &str) -> Result<(), XmlError> {
    let trimmed = xml.trim_start();
    if !trimmed.starts_with("<?xml") {
        return Ok(());
    }
    let Some(end) = trimmed.find("?>") else {
        return Ok(());
    };
    let decl = &trimmed[..end];
    let Some(pos) = decl.find("encoding") else {
        return Ok(());
    };
    let after = decl[pos + "encoding".len()..].trim_start();
    let Some(rest) = after.strip_prefix('=') else {
        return Ok(());
    };
    let rest = rest.trim_start();
    let quote = match rest.chars().next() {
        Some(q @ ('"' | '\'')) => q,
        _ => return Ok(()),
    };
    let rest = &rest[1..];
    let Some(close) = rest.find(quote) else {
        return Ok(());
    };
    let value = &rest[..close];
    if value.eq_ignore_ascii_case("utf-8") || value.eq_ignore_ascii_case("utf8") {
        return Ok(());
    }
    Err(XmlError::InvalidXml(format!(
        "document declares encoding `{value}`; §7.1.1 requires UTF-8"
    )))
}

/// Recursive tree build from a roxmltree node.
///
/// `depth` counts the current nesting depth (root = 0) and
/// protects against stack overflow via [`MAX_TREE_DEPTH`].
///
/// zerodds-lint: recursion-depth = xml-tree-depth (capped by `MAX_TREE_DEPTH`;
/// additionally secured against wide/tall DoS by `MAX_TOTAL_ELEMENTS` and
/// `MAX_LIST_ELEMENTS`).
fn build_element(
    node: roxmltree::Node<'_, '_>,
    parent_path: &str,
    counter: &mut usize,
    depth: usize,
) -> Result<XmlElement, XmlError> {
    if depth > MAX_TREE_DEPTH {
        return Err(XmlError::LimitExceeded(format!(
            "tree depth exceeds {MAX_TREE_DEPTH} — refusing to build to protect against \
             stack overflow"
        )));
    }
    *counter += 1;
    if *counter > MAX_TOTAL_ELEMENTS {
        return Err(XmlError::LimitExceeded(format!(
            "document exceeds {MAX_TOTAL_ELEMENTS} elements"
        )));
    }

    let tag = node.tag_name();
    let name = tag.name().to_string();
    let path = match (parent_path, node.attribute("name")) {
        ("", Some(n)) => format!("{name}[{n}]"),
        ("", None) => name.clone(),
        (parent, Some(n)) => format!("{parent}/{name}[{n}]"),
        (parent, None) => format!("{parent}/{name}"),
    };
    let mut element = XmlElement {
        name,
        namespace: tag.namespace().map(ToString::to_string),
        path,
        attributes: BTreeMap::new(),
        children: Vec::new(),
        text: String::new(),
    };

    // §7.1.5 Tab.7.2 — take over the attributes.
    for attr in node.attributes() {
        match attr.namespace() {
            None => {
                element
                    .attributes
                    .insert(attr.name().to_string(), attr.value().to_string());
            }
            Some(XSI_NS | roxmltree::NS_XML_URI) => {}
            Some(ns) => {
                element
                    .attributes
                    .insert(format!("{{{ns}}}{}", attr.name()), attr.value().to_string());
            }
        }
    }

    // §7.1.4 Tab.7.1 — the value of a `string` element is its text with
    // the XML escapes removed; whitespace belongs to it. roxmltree
    // delivers each text run as its own node, so all of them are joined:
    // a comment between two runs must not truncate the value.
    let mut text = String::new();
    for child in node.children() {
        if child.is_text() {
            if let Some(run) = child.text() {
                text.push_str(run);
            }
        }
    }
    element.text = text;

    // Child element nodes (comments + whitespace text are
    // filtered out automatically).
    let mut child_count: usize = 0;
    for child_node in node.children().filter(roxmltree::Node::is_element) {
        child_count += 1;
        if child_count > MAX_LIST_ELEMENTS {
            return Err(XmlError::LimitExceeded(format!(
                "<{}> has more than {MAX_LIST_ELEMENTS} children",
                element.name
            )));
        }
        element.children.push(build_element(
            child_node,
            &element.path,
            counter,
            depth + 1,
        )?);
    }

    Ok(element)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_document() {
        let xml = r#"<root/>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root_name(), "root");
        assert!(doc.root.children.is_empty());
    }

    #[test]
    fn parse_with_xml_declaration() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><root/>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root_name(), "root");
    }

    #[test]
    fn non_utf8_declared_encoding_rejected() {
        // §7.1.1: UTF-8 is required.
        let xml = r#"<?xml version="1.0" encoding="ISO-8859-1"?><root/>"#;
        let err = parse_xml_tree(xml).expect_err("encoding");
        assert!(matches!(err, XmlError::InvalidXml(_)));
        // UTF-8 (in any case) is accepted.
        let ok = r#"<?xml version="1.0" encoding="utf-8"?><root/>"#;
        assert!(parse_xml_tree(ok).is_ok());
    }

    #[test]
    fn parse_namespace_aware() {
        let xml = r#"<dds xmlns="http://www.omg.org/spec/DDS-XML"/>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root.namespace.as_deref(), Some(DDS_XML_NS));
    }

    #[test]
    fn comments_stripped() {
        let xml = r#"<root>
            <!-- this is a comment -->
            <child>value</child>
            <!-- another -->
        </root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root.children.len(), 1);
        assert_eq!(doc.root.children[0].name, "child");
        assert_eq!(doc.root.children[0].text, "value");
    }

    #[test]
    fn element_text_preserves_whitespace() {
        // §7.1.4 Tab.7.1 `string`: whitespace is not stripped from a value.
        let xml = r#"<root><child>  hello  </child></root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root.children[0].text, "  hello  ");
    }

    #[test]
    fn element_text_joins_runs_across_comments() {
        // A comment between two text runs must not truncate the value.
        let xml = r#"<root><name>A<!-- c -->B</name></root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root.children[0].text, "AB");
        // A comment before the text also leaves the value intact.
        let xml = r#"<root><name><!-- c -->A</name></root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root.children[0].text, "A");
    }

    #[test]
    fn attributes_preserved() {
        let xml = r#"<profile name="P1" base_name="P0"/>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root.attribute("name"), Some("P1"));
        assert_eq!(doc.root.attribute("base_name"), Some("P0"));
        assert_eq!(doc.root.attribute("missing"), None);
    }

    #[test]
    fn invalid_xml_rejected() {
        let xml = "<root><unclosed></root>";
        let err = parse_xml_tree(xml).expect_err("invalid");
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    #[test]
    fn dtd_and_internal_entity_accepted() {
        // §7.1.1: a well-formed document per [XML] §2.1 may carry a DTD.
        let xml = r#"<?xml version="1.0"?>
<!DOCTYPE root [<!ENTITY greet "hello">]>
<root>&greet;</root>"#;
        let doc = parse_xml_tree(xml).expect("dtd");
        assert_eq!(doc.root.text, "hello");
    }

    /// The DTD of a well-formed document is accepted, but not what makes a
    /// small document expand without bound: entities that refer to
    /// entities, an entity value over the cap, or too many declarations.
    #[test]
    fn entity_expansion_is_bounded() {
        let nested = r#"<!DOCTYPE r [<!ENTITY a "x"><!ENTITY b "&a;&a;&a;">]><r>&b;</r>"#;
        assert!(matches!(
            parse_xml_tree(nested),
            Err(XmlError::LimitExceeded(_))
        ));
        let long_value = "y".repeat(MAX_ENTITY_VALUE_BYTES + 1);
        let long = format!(r#"<!DOCTYPE r [<!ENTITY a "{long_value}">]><r>&a;</r>"#);
        assert!(matches!(
            parse_xml_tree(&long),
            Err(XmlError::LimitExceeded(_))
        ));
        let at_cap = "y".repeat(MAX_ENTITY_VALUE_BYTES);
        let ok = format!(r#"<!DOCTYPE r [<!ENTITY a "{at_cap}">]><r>&a;</r>"#);
        assert_eq!(
            parse_xml_tree(&ok).expect("at the cap").root.text.len(),
            MAX_ENTITY_VALUE_BYTES
        );
        let decls: String = (0..=MAX_ENTITY_DECLARATIONS)
            .map(|i| format!(r#"<!ENTITY e{i} "v">"#))
            .collect();
        let many = format!("<!DOCTYPE r [{decls}]><r/>");
        assert!(matches!(
            parse_xml_tree(&many),
            Err(XmlError::LimitExceeded(_))
        ));
        // Character references and the predefined entities in a value
        // are no references to declared entities.
        let predefined = r#"<!DOCTYPE r [<!ENTITY a "&lt;&#65;&amp;">]><r>&a;</r>"#;
        assert_eq!(
            parse_xml_tree(predefined).expect("predefined").root.text,
            "<A&"
        );
    }

    #[test]
    fn external_entity_reference_rejected() {
        // A reference to an external entity is not resolved (no fetch)
        // and is reported, so XXE has no effect.
        let xml = r#"<?xml version="1.0"?>
<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<root>&xxe;</root>"#;
        let err = parse_xml_tree(xml).expect_err("external entity");
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    #[test]
    fn deep_nesting_in_cdata_does_not_fool_the_depth_check() {
        // A CDATA section full of `<` must not be counted as tag nesting.
        let xml = "<root><![CDATA[<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<]]></root>";
        assert!(parse_xml_tree(xml).is_ok());
    }

    #[test]
    fn child_helper() {
        let xml = r#"<root><a/><b/><a/></root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root.child("a").map(|c| c.name.as_str()), Some("a"));
        assert_eq!(doc.root.children_named("a").count(), 2);
        assert_eq!(doc.root.children_named("missing").count(), 0);
    }

    #[test]
    fn list_dos_cap() {
        // Build XML with MAX_LIST_ELEMENTS+1 children.
        let mut xml = String::from("<root>");
        for _ in 0..(MAX_LIST_ELEMENTS + 1) {
            xml.push_str("<c/>");
        }
        xml.push_str("</root>");
        let err = parse_xml_tree(&xml).expect_err("dos");
        assert!(matches!(err, XmlError::LimitExceeded(_)));
    }

    #[test]
    fn nested_structure() {
        let xml = r#"<root>
            <profile name="P1">
                <history>
                    <kind>KEEP_LAST_HISTORY_QOS</kind>
                    <depth>10</depth>
                </history>
            </profile>
        </root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        let profile = doc.root.child("profile").expect("profile");
        assert_eq!(profile.attribute("name"), Some("P1"));
        let history = profile.child("history").expect("history");
        assert_eq!(
            history.child("kind").map(|c| c.text.as_str()),
            Some("KEEP_LAST_HISTORY_QOS")
        );
        assert_eq!(history.child("depth").map(|c| c.text.as_str()), Some("10"));
    }

    // ---- §7.2.4.1 + §7.2.5 sequence_elements ------------------------

    #[test]
    fn sequence_elements_iterates_element_tag_children() {
        // Spec §7.2.4.1: an IDL sequence is represented as a <complexType> with
        // 0..n <element> children.
        let xml = r#"<root>
            <ports>
                <element>7400</element>
                <element>7401</element>
                <element>7402</element>
            </ports>
        </root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        let ports = doc.root.child("ports").expect("ports");
        let texts: Vec<&str> = ports.sequence_elements().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, vec!["7400", "7401", "7402"]);
    }

    #[test]
    fn sequence_elements_skips_non_element_tagged_children() {
        // Other tags such as <kind>, <depth> are ignored — only the
        // <element> tag counts as a sequence entry.
        let xml = r#"<root>
            <history>
                <kind>KEEP_LAST_HISTORY_QOS</kind>
                <depth>10</depth>
                <element>not-a-real-history-field</element>
            </history>
        </root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        let hist = doc.root.child("history").expect("hist");
        let texts: Vec<&str> = hist.sequence_elements().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, vec!["not-a-real-history-field"]);
    }

    #[test]
    fn sequence_elements_empty_for_zero_children() {
        let xml = r#"<root><list></list></root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        let list = doc.root.child("list").expect("list");
        assert_eq!(list.sequence_elements().count(), 0);
    }

    #[test]
    fn array_uses_same_element_tag_as_sequence() {
        // Spec §7.2.5: IDL array mapping is identical to IDL sequence
        // mapping. So the same `sequence_elements` iterator must
        // also work for fixed-size IDL arrays.
        let xml = r#"<root>
            <coords_3d>
                <element>1.0</element>
                <element>2.0</element>
                <element>3.0</element>
            </coords_3d>
        </root>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        let arr = doc.root.child("coords_3d").expect("array");
        let texts: Vec<&str> = arr.sequence_elements().map(|e| e.text.as_str()).collect();
        assert_eq!(texts.len(), 3, "IDL-Array[3] = 3 <element>-Children");
        assert_eq!(texts, vec!["1.0", "2.0", "3.0"]);
    }
}
