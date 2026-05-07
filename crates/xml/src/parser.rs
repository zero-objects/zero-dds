// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Generischer well-formed XML-Loader fuer DDS-XML 1.0 §7.1.
//!
//! Cluster-F-Foundation: liefert einen Roxmltree-basierten Parser, der
//! sich an die Wohlgeformtheits-Regeln aus §7.1.1 haelt (UTF-8,
//! Whitespace-tolerant, Comment-Stripping, Namespace-Aware) und das
//! Ergebnis in einem generischen [`DdsXmlDocument`]-Container ablegt.
//!
//! Building-Block-spezifische Decoder (QoS-Library, Types, Domains,
//! Participants, Applications, Samples) bauen auf diesem Container auf
//! (Cluster G/H/I/J in `docs/spec-coverage/zerodds-xml-1.0.open.md`).

use crate::errors::XmlError;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// DDS-XML 1.0 Spec-Namespace fuer Building-Block-Top-Level-Elemente.
///
/// Spec-Ref: §7.3.x ("targetNamespace = http://www.omg.org/spec/DDS-XML").
pub const DDS_XML_NS: &str = "http://www.omg.org/spec/DDS-XML";

/// DoS-Cap fuer Listen-Elemente (Children pro Knoten).
pub const MAX_LIST_ELEMENTS: usize = 1024;

/// DoS-Cap fuer Gesamt-Knoten-Anzahl pro Dokument.
pub const MAX_TOTAL_ELEMENTS: usize = 64 * 1024;

/// DoS-Cap fuer die rekursive Baum-Tiefe.
///
/// Schuetzt den rekursiven `build_element`-Aufruf vor Stack-Overflow
/// bei adversarial deeply-nested XML-Inputs (TS-1-Finding 3,
/// `docs/test-harness/plan.md`). Realistische DDS-XML-Profile gehen
/// 4-8 tief; selbst komplexe `<application>`-/`<participant>`-
/// Verschachtelungen bleiben unter 32.
pub const MAX_TREE_DEPTH: usize = 64;

/// Generischer In-Memory-Container fuer ein DDS-XML-Dokument nach §7.1.
///
/// Ein Element wird als [`XmlElement`] (Tag, Attribute, Children, Text)
/// abgebildet. Building-Block-Decoder navigieren ueber diesen Baum und
/// erzeugen typed Strukturen (z.B. QoS-Profile in Cluster G).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdsXmlDocument {
    /// Wurzel-Element des Dokuments (z.B. `<dds>`, `<qos_library>`,
    /// `<domain_participant_library>`).
    pub root: XmlElement,
}

impl DdsXmlDocument {
    /// Liefert den lokalen Tag-Namen der Wurzel (ohne Namespace-Prefix).
    #[must_use]
    pub fn root_name(&self) -> &str {
        &self.root.name
    }
}

/// Ein einzelnes XML-Element nach §7.1.4 / §7.1.5 (Element + Attribute).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XmlElement {
    /// Lokaler Tag-Name (ohne Namespace-Prefix).
    pub name: String,
    /// Optionaler Namespace-URI gemaess §7.1.3.
    pub namespace: Option<String>,
    /// Attribute (Name -> Wert, alphabetisch sortiert via BTreeMap fuer
    /// deterministische Iteration).
    pub attributes: BTreeMap<String, String>,
    /// Kind-Elemente in Dokument-Reihenfolge.
    pub children: Vec<XmlElement>,
    /// Direkter Text-Inhalt (zusammenhaengender Text-Teil unmittelbar
    /// vor dem ersten Kind-Element). Leer wenn kein Text vorhanden.
    pub text: String,
}

impl XmlElement {
    /// Liefert den ersten Kind mit dem gegebenen lokalen Namen, falls
    /// vorhanden.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&XmlElement> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Liefert alle Kinder mit dem gegebenen lokalen Namen.
    pub fn children_named<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a XmlElement> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// Liefert den Wert eines Attributs, falls vorhanden.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    /// Iteriert ueber `<element>`-Kinder gemaess Spec §7.2.4.1
    /// (IDL-Sequence-Mapping) und §7.2.5 (IDL-Array-Mapping = Sequence-
    /// Mapping mit gleichem Element-Tag).
    ///
    /// Spec OMG DDS-XML 1.0 §7.2.4.1: "The complexType contains zero
    /// or more elements named element. Nested inside each element is
    /// the XSD schema obtained from mapping the IDL type of the
    /// element itself."
    ///
    /// Spec §7.2.5: "The XML representation of IDL arrays is the same
    /// as it would be for IDL sequences of the same element type." —
    /// d.h. `<element>`-Tag wird auch fuer Arrays genutzt.
    ///
    /// Wird vom IDL-PSM-Mapping (`qos_parser`, `sample`) genutzt, um
    /// generische Sequenzen/Arrays zu iterieren.
    pub fn sequence_elements(&self) -> impl Iterator<Item = &XmlElement> + '_ {
        self.children_named("element")
    }
}

/// Parses a DDS-XML 1.0 document gemaess §7.1.
///
/// Wohlgeformtheits-Garantien:
/// * XML-Declaration optional (Spec §7.1.1).
/// * Whitespace-tolerant (roxmltree normalisiert Text).
/// * Kommentare werden gestrippt (`<!-- ... -->` werden von roxmltree
///   nicht als Element-Knoten geliefert).
/// * Namespace-Aware: Top-Level-Namespace wird im Root mitgefuehrt
///   (Spec §7.3.x: `targetNamespace = http://www.omg.org/spec/DDS-XML`).
///
/// DoS-Caps gemaess Cluster-F: maximal [`MAX_LIST_ELEMENTS`] Children pro
/// Knoten, maximal [`MAX_TOTAL_ELEMENTS`] Knoten insgesamt.
///
/// # Errors
/// * [`XmlError::InvalidXml`] — XML nicht wohlgeformt.
/// * [`XmlError::LimitExceeded`] — DoS-Cap getroffen.
pub fn parse_xml_tree(xml: &str) -> Result<DdsXmlDocument, XmlError> {
    // Pre-Validation: tiefe Verschachtelung wuerde im rekursiven
    // `roxmltree`-Parser zu Stack-Overflow fuehren — wir lehnen
    // Inputs ueber `MAX_TREE_DEPTH` *vor* dem Parser-Call ab.
    // TS-1-Finding 3.
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

/// Byte-level Vor-Pruefung der Tag-Verschachtelungstiefe.
///
/// Geht Bytes durch und zaehlt:
/// * `<` ohne `/`, `!`, `?` direkt danach: depth + 1
/// * `</`: depth - 1
/// * `<!` oder `<?`: Kommentar / PI / DTD — uebersprungen
/// * `/>`: self-closing — depth + 1 - 1 (netto 0)
///
/// Heuristisch, aber ein **Upper Bound** auf die echte Tag-Tiefe —
/// wenn dieser Bound bereits ueber `MAX_TREE_DEPTH` liegt, ist der
/// nachgelagerte rekursive Parser unsicher.
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
        // Look-ahead nach dem ersten Zeichen.
        let next = bytes.get(i + 1).copied();
        match next {
            Some(b'/') => {
                depth = depth.saturating_sub(1);
                i += 2;
            }
            Some(b'!') | Some(b'?') => {
                // Skip bis zum naechsten `>` (Kommentar oder PI).
                i += 2;
                while i < bytes.len() && bytes[i] != b'>' {
                    i += 1;
                }
            }
            _ => {
                // Opening-Tag — pruefe ob self-closing: skanne bis `>`
                // und schau, ob das Byte davor `/` ist.
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
        // `>` ueberspringen (am Ende des match arms steht i auf `>`).
        if i < bytes.len() && bytes[i] == b'>' {
            i += 1;
        }
    }
    Ok(())
}

/// Rekursiver Baum-Aufbau aus einem roxmltree-Knoten.
///
/// `depth` zaehlt die aktuelle Verschachtelungstiefe (Wurzel = 0) und
/// schuetzt vor Stack-Overflow ueber [`MAX_TREE_DEPTH`].
///
/// zerodds-lint: recursion-depth = xml-tree-depth (durch `MAX_TREE_DEPTH`
/// gecappt; durch `MAX_TOTAL_ELEMENTS` und `MAX_LIST_ELEMENTS`
/// zusaetzlich gegen Wide-/Tall-DoS abgesichert).
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

    // §7.1.5 Tab.7.2 — Attribute uebernehmen.
    for attr in node.attributes() {
        element
            .attributes
            .insert(attr.name().to_string(), attr.value().to_string());
    }

    // §7.1.4 Tab.7.1 — Erster Text-Inhalt (vor dem ersten Element-Kind).
    // roxmltree liefert Text als eigene Knoten zwischen Elementen.
    if let Some(text) = node.text() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            element.text = trimmed.to_string();
        }
    }

    // Children-Element-Knoten (Comments + Whitespace-Text werden
    // automatisch gefiltert).
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
        // text wird getrimmed
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
        // §7.1.1 — XML 1.0 Fifth Edition; DTDs sind erlaubt, aber wir
        // verbieten sie aus Security-Gruenden (XXE-Vermeidung).
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
        // Spec §7.2.4.1: IDL-Sequence wird als <complexType> mit
        // 0..n <element>-Children dargestellt.
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
        // Andere Tags wie <kind>, <depth> werden ignoriert — nur
        // <element>-Tag zaehlt als Sequenz-Eintrag.
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
        // Spec §7.2.5: IDL-Array-Mapping ist identisch zu IDL-Sequence-
        // Mapping. Der gleiche `sequence_elements`-Iterator muss also
        // auch fuer fixed-size IDL-Arrays funktionieren.
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
