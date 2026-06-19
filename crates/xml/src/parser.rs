// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Generic well-formed XML loader for DDS-XML 1.0 §7.1.
//!
//! Cluster-F foundation: provides a roxmltree-based parser that
//! adheres to the well-formedness rules from §7.1.1 (UTF-8,
//! whitespace-tolerant, comment-stripping, namespace-aware) and stores the
//! result in a generic [`DdsXmlDocument`] container.
//!
//! Building-block-specific decoders (QoS library, types, domains,
//! participants, applications, samples) build on this container
//! (Cluster G/H/I/J in `docs/spec-coverage/zerodds-xml-1.0.open.md`).

use crate::errors::XmlError;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// DDS-XML 1.0 spec namespace for building-block top-level elements.
///
/// Spec ref: §7.3.x ("targetNamespace = http://www.omg.org/spec/DDS-XML").
pub const DDS_XML_NS: &str = "http://www.omg.org/spec/DDS-XML";

/// DoS cap for list elements (children per node).
pub const MAX_LIST_ELEMENTS: usize = 1024;

/// DoS cap for the total node count per document.
pub const MAX_TOTAL_ELEMENTS: usize = 64 * 1024;

/// DoS cap for the recursive tree depth.
///
/// Protects the recursive `build_element` call from stack overflow
/// on adversarial deeply-nested XML inputs (TS-1 finding 3,
/// `docs/test-harness/plan.md`). Realistic DDS-XML profiles go
/// 4-8 deep; even complex `<application>`/`<participant>`
/// nestings stay below 32.
pub const MAX_TREE_DEPTH: usize = 64;

/// Generic in-memory container for a DDS-XML document per §7.1.
///
/// An element is mapped as an [`XmlElement`] (tag, attributes, children, text).
/// Building-block decoders navigate over this tree and
/// produce typed structures (e.g. QoS profiles in Cluster G).
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
}

/// A single XML element per §7.1.4 / §7.1.5 (element + attributes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XmlElement {
    /// Local tag name (without the namespace prefix).
    pub name: String,
    /// Optional namespace URI per §7.1.3.
    pub namespace: Option<String>,
    /// Attributes (name -> value, alphabetically sorted via BTreeMap for
    /// deterministic iteration).
    pub attributes: BTreeMap<String, String>,
    /// Child elements in document order.
    pub children: Vec<XmlElement>,
    /// Direct text content (contiguous text part immediately
    /// before the first child element). Empty if no text is present.
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
    /// Used by the IDL-PSM mapping (`qos_parser`, `sample`) to
    /// iterate generic sequences/arrays.
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
///   (Spec §7.3.x: `targetNamespace = http://www.omg.org/spec/DDS-XML`).
///
/// DoS caps per Cluster F: at most [`MAX_LIST_ELEMENTS`] children per
/// node, at most [`MAX_TOTAL_ELEMENTS`] nodes total.
///
/// # Errors
/// * [`XmlError::InvalidXml`] — XML not well-formed.
/// * [`XmlError::LimitExceeded`] — a DoS cap was hit.
pub fn parse_xml_tree(xml: &str) -> Result<DdsXmlDocument, XmlError> {
    // Pre-validation: deep nesting would cause a stack overflow in the
    // recursive `roxmltree` parser — we reject
    // inputs over `MAX_TREE_DEPTH` *before* the parser call.
    // TS-1 finding 3.
    precheck_depth(xml)?;
    let opts = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(xml, opts)
        .map_err(|e| XmlError::InvalidXml(e.to_string()))?;
    let mut counter: usize = 0;
    let root = build_element(doc.root_element(), &mut counter, 0)?;
    Ok(DdsXmlDocument { root })
}

/// Byte-level pre-check of the tag nesting depth.
///
/// Walks the bytes and counts:
/// * `<` without `/`, `!`, `?` directly after it: depth + 1
/// * `</`: depth - 1
/// * `<!` or `<?`: comment / PI / DTD — skipped
/// * `/>`: self-closing — depth + 1 - 1 (net 0)
///
/// Heuristic, but an **upper bound** on the real tag depth —
/// if this bound is already over `MAX_TREE_DEPTH`, the
/// downstream recursive parser is unsafe.
fn precheck_depth(xml: &str) -> Result<(), XmlError> {
    let bytes = xml.as_bytes();
    let mut depth: i64 = 0;
    let mut max_seen: i64 = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // Look-ahead after the first character.
        let next = bytes.get(i + 1).copied();
        match next {
            Some(b'/') => {
                depth = depth.saturating_sub(1);
                i += 2;
            }
            Some(b'!') | Some(b'?') => {
                // Skip to the next `>` (comment or PI).
                i += 2;
                while i < bytes.len() && bytes[i] != b'>' {
                    i += 1;
                }
            }
            _ => {
                // Opening tag — check if self-closing: scan to `>`
                // and see whether the byte before it is `/`.
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'>' {
                    i += 1;
                }
                let self_closing = i > start && bytes.get(i - 1) == Some(&b'/');
                if !self_closing {
                    depth += 1;
                    if depth > max_seen {
                        max_seen = depth;
                    }
                    if depth > MAX_TREE_DEPTH as i64 {
                        return Err(XmlError::LimitExceeded(format!(
                            "tag nesting exceeds {MAX_TREE_DEPTH} — refusing to parse to \
                             protect against stack overflow"
                        )));
                    }
                }
            }
        }
        // Skip `>` (at the end of the match arm, i is on `>`).
        if i < bytes.len() && bytes[i] == b'>' {
            i += 1;
        }
    }
    Ok(())
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
    let mut element = XmlElement {
        name: tag.name().to_string(),
        namespace: tag.namespace().map(ToString::to_string),
        attributes: BTreeMap::new(),
        children: Vec::new(),
        text: String::new(),
    };

    // §7.1.5 Tab.7.2 — take over the attributes.
    for attr in node.attributes() {
        element
            .attributes
            .insert(attr.name().to_string(), attr.value().to_string());
    }

    // §7.1.4 Tab.7.1 — first text content (before the first element child).
    // roxmltree delivers text as its own nodes between elements.
    if let Some(text) = node.text() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            element.text = trimmed.to_string();
        }
    }

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
        element
            .children
            .push(build_element(child_node, counter, depth + 1)?);
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
    fn whitespace_tolerant() {
        let xml = r#"
            <root>
                <child>  hello  </child>
            </root>
        "#;
        let doc = parse_xml_tree(xml).expect("parse");
        // text is trimmed
        assert_eq!(doc.root.children[0].text, "hello");
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
    fn dtd_rejected() {
        // §7.1.1 — XML 1.0 Fifth Edition; DTDs are allowed, but we
        // forbid them for security reasons (XXE avoidance).
        let xml = r#"<?xml version="1.0"?>
<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<root>&xxe;</root>"#;
        let err = parse_xml_tree(xml).expect_err("dtd");
        assert!(matches!(err, XmlError::InvalidXml(_)));
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
