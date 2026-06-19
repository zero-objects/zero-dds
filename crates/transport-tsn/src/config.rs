// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-TSN PIM Configuration + PSM (XML/JSON) Loader.
//!
//! Spec source: OMG DDS-TSN 1.0 §7.2.1-§7.2.2 (DDS Application +
//! Deployment Configuration Tables 7.1-7.14: QoS library,
//! DomainLibrary, ApplicationLibrary, DeploymentLibrary) + §7.3
//! (wire representation in XML/JSON).
//!
//! # Layer discipline
//!
//! The PIM model ([`TsnConfiguration`]) is the truth. XML
//! ([`parse_xml_config`]) and JSON ([`render_json_config`]) are
//! pure representation conversions — they do not define their
//! own fields. The YANG PSM is the caller layer (separate `yang` crate).
//!
//! Cross-ref:
//! * `crates/xml/src/qos.rs` — DDS-XML 1.0 §7.3.2 QoS profile loader
//!   (same `roxmltree` backend choice).
//! * `crates/opcua-gateway/src/xml.rs` — sister loader for
//!   DDS-OPCUA 1.0 §10.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use roxmltree::{Document, Node};

/// Configuration loader error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// XML parser error (malformed XML).
    Parse(String),
    /// Unexpected root element.
    UnexpectedRoot(String),
    /// Required attribute missing.
    MissingAttribute {
        /// Element name.
        element: String,
        /// Attribute name.
        attr: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(s) => write!(f, "TSN config parse error: {s}"),
            Self::UnexpectedRoot(s) => write!(f, "expected <dds_tsn> root, got <{s}>"),
            Self::MissingAttribute { element, attr } => {
                write!(f, "<{element}> missing attribute '{attr}'")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConfigError {}

/// Root container for the DDS-TSN configuration.
///
/// Spec §7.2.1 Tab 7.1 + §7.2.2 Tab 7.2 (DDS Application + Deployment
/// Configuration Tables).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TsnConfiguration {
    /// QoS library — Spec Tab 7.3 (TSN-specific QoS profiles).
    pub qos_library: TsnQosLibrary,
    /// Domain library — Spec Tab 7.4.
    pub domain_library: DomainLibrary,
    /// Deployment library — Spec Tab 7.5 (talker/listener bindings).
    pub deployment_library: DeploymentLibrary,
}

/// Spec Tab 7.3 — TSN QoS library.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TsnQosLibrary {
    /// Library name.
    pub name: String,
    /// List of the TSN QoS profiles.
    pub profiles: Vec<QosProfileEntry>,
}

/// A TSN QoS profile entry referencing VLAN/PCP/DSCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QosProfileEntry {
    /// Profile name.
    pub name: String,
    /// Spec Tab 7.21 — VLAN-ID (12-bit).
    pub vlan_id: Option<u16>,
    /// Spec Tab 7.21 — PCP (3-bit).
    pub pcp: Option<u8>,
    /// Spec Tab 7.21 — DEI.
    pub dei: Option<bool>,
    /// Spec RFC 2474 — DSCP (6-bit).
    pub dscp: Option<u8>,
}

/// Spec Tab 7.4 — domain library.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainLibrary {
    /// List of the domain definitions.
    pub domains: Vec<DomainEntry>,
}

/// A domain library row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEntry {
    /// Domain ID.
    pub domain_id: u32,
    /// Default QoS profile name.
    pub default_profile: String,
}

/// Spec Tab 7.5 — deployment library with talker/listener bindings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeploymentLibrary {
    /// List of the talker definitions (see Spec Tab 7.15).
    pub talkers: Vec<TalkerEntry>,
    /// List of the listener definitions (see Spec Tab 7.24).
    pub listeners: Vec<ListenerEntry>,
}

/// Talker entry (Spec Tab 7.15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TalkerEntry {
    /// Stream identifier (mac + unique_id).
    pub stream_id: String,
    /// QoS profile reference.
    pub qos_profile: String,
    /// Topic name.
    pub topic: String,
}

/// Listener entry (Spec Tab 7.24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenerEntry {
    /// Stream identifier (mac + unique_id).
    pub stream_id: String,
    /// Topic name.
    pub topic: String,
}

/// Spec §7.3 XML wire-format loader.
///
/// Accepts a lightweight subset of the spec XML schema:
/// `<dds_tsn>` as root, with `<qos_library>` containing `<qos_profile>`
/// entries, `<domain_library>` with `<domain>` entries,
/// `<deployment_library>` with `<talker>` / `<listener>` entries.
///
/// # Errors
/// Returns [`ConfigError`] on malformed XML.
pub fn parse_xml_config(src: &str) -> Result<TsnConfiguration, ConfigError> {
    let doc = Document::parse(src).map_err(|e| ConfigError::Parse(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "dds_tsn" {
        return Err(ConfigError::UnexpectedRoot(
            root.tag_name().name().to_string(),
        ));
    }
    let mut cfg = TsnConfiguration::default();
    for n in root.children().filter(Node::is_element) {
        match n.tag_name().name() {
            "qos_library" => cfg.qos_library = parse_qos_library(n)?,
            "domain_library" => cfg.domain_library = parse_domain_library(n)?,
            "deployment_library" => cfg.deployment_library = parse_deployment_library(n)?,
            _ => {}
        }
    }
    Ok(cfg)
}

fn parse_qos_library(node: Node<'_, '_>) -> Result<TsnQosLibrary, ConfigError> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let mut profiles = Vec::new();
    for p in node
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "qos_profile")
    {
        let prof_name = required_attr(p, "name")?;
        let mut entry = QosProfileEntry {
            name: prof_name,
            vlan_id: None,
            pcp: None,
            dei: None,
            dscp: None,
        };
        for f in p.children().filter(Node::is_element) {
            let txt = f.text().map(str::trim).unwrap_or("");
            match f.tag_name().name() {
                "vlan_id" => entry.vlan_id = txt.parse().ok(),
                "pcp" => entry.pcp = txt.parse().ok(),
                "dei" => entry.dei = Some(matches!(txt, "true" | "1")),
                "dscp" => entry.dscp = txt.parse().ok(),
                _ => {}
            }
        }
        profiles.push(entry);
    }
    Ok(TsnQosLibrary { name, profiles })
}

fn parse_domain_library(node: Node<'_, '_>) -> Result<DomainLibrary, ConfigError> {
    let mut domains = Vec::new();
    for d in node
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "domain")
    {
        let domain_id = required_attr(d, "id")?
            .parse::<u32>()
            .map_err(|_| ConfigError::Parse("domain.id must be u32".to_string()))?;
        let default_profile = d
            .children()
            .filter(Node::is_element)
            .find(|c| c.tag_name().name() == "default_profile")
            .and_then(|c| c.text().map(str::trim).map(str::to_string))
            .unwrap_or_default();
        domains.push(DomainEntry {
            domain_id,
            default_profile,
        });
    }
    Ok(DomainLibrary { domains })
}

fn parse_deployment_library(node: Node<'_, '_>) -> Result<DeploymentLibrary, ConfigError> {
    let mut talkers = Vec::new();
    let mut listeners = Vec::new();
    for c in node.children().filter(Node::is_element) {
        match c.tag_name().name() {
            "talker" => talkers.push(TalkerEntry {
                stream_id: required_attr(c, "stream_id")?,
                qos_profile: child_text(c, "qos_profile").unwrap_or_default(),
                topic: child_text(c, "topic").unwrap_or_default(),
            }),
            "listener" => listeners.push(ListenerEntry {
                stream_id: required_attr(c, "stream_id")?,
                topic: child_text(c, "topic").unwrap_or_default(),
            }),
            _ => {}
        }
    }
    Ok(DeploymentLibrary { talkers, listeners })
}

fn required_attr(node: Node<'_, '_>, name: &str) -> Result<String, ConfigError> {
    node.attribute(name)
        .map(ToString::to_string)
        .ok_or_else(|| ConfigError::MissingAttribute {
            element: node.tag_name().name().to_string(),
            attr: name.to_string(),
        })
}

fn child_text(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.children()
        .filter(Node::is_element)
        .find(|c| c.tag_name().name() == name)
        .and_then(|c| c.text().map(str::trim).map(str::to_string))
}

/// Spec §7.3 JSON wire-format renderer.
///
/// We use a hand-rolled JSON format (without a `serde-json`
/// dependency) that outputs all fields of [`TsnConfiguration`] in a
/// structured way. The format is deterministic (key order)
/// and byte-wise consistent — the roundtrip
/// `parse_xml_config -> render_json_config -> external tooling` is
/// countable.
///
/// # Returns
/// JSON string (UTF-8) conforming to RFC 8259.
#[must_use]
pub fn render_json_config(cfg: &TsnConfiguration) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"qos_library\": ");
    render_qos_library(&cfg.qos_library, &mut out, 2);
    out.push_str(",\n  \"domain_library\": ");
    render_domain_library(&cfg.domain_library, &mut out, 2);
    out.push_str(",\n  \"deployment_library\": ");
    render_deployment_library(&cfg.deployment_library, &mut out, 2);
    out.push_str("\n}\n");
    out
}

fn render_qos_library(lib: &TsnQosLibrary, out: &mut String, indent: usize) {
    out.push_str("{\n");
    let inner = " ".repeat(indent + 2);
    out.push_str(&inner);
    out.push_str("\"name\": ");
    push_json_string(&lib.name, out);
    out.push_str(",\n");
    out.push_str(&inner);
    out.push_str("\"profiles\": [");
    for (i, p) in lib.profiles.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('\n');
        out.push_str(&" ".repeat(indent + 4));
        out.push('{');
        out.push_str("\"name\": ");
        push_json_string(&p.name, out);
        push_opt_field(out, "vlan_id", p.vlan_id.map(u32::from));
        push_opt_field(out, "pcp", p.pcp.map(u32::from));
        if let Some(d) = p.dei {
            out.push_str(", \"dei\": ");
            out.push_str(if d { "true" } else { "false" });
        }
        push_opt_field(out, "dscp", p.dscp.map(u32::from));
        out.push('}');
    }
    if !lib.profiles.is_empty() {
        out.push('\n');
        out.push_str(&inner);
    }
    out.push(']');
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push('}');
}

fn render_domain_library(lib: &DomainLibrary, out: &mut String, indent: usize) {
    out.push('[');
    for (i, d) in lib.domains.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('\n');
        out.push_str(&" ".repeat(indent + 2));
        out.push_str("{\"domain_id\": ");
        out.push_str(&alloc::format!("{}", d.domain_id));
        out.push_str(", \"default_profile\": ");
        push_json_string(&d.default_profile, out);
        out.push('}');
    }
    if !lib.domains.is_empty() {
        out.push('\n');
        out.push_str(&" ".repeat(indent));
    }
    out.push(']');
}

fn render_deployment_library(lib: &DeploymentLibrary, out: &mut String, indent: usize) {
    out.push_str("{\n");
    let inner = " ".repeat(indent + 2);
    out.push_str(&inner);
    out.push_str("\"talkers\": [");
    for (i, t) in lib.talkers.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('\n');
        out.push_str(&" ".repeat(indent + 4));
        out.push_str("{\"stream_id\": ");
        push_json_string(&t.stream_id, out);
        out.push_str(", \"qos_profile\": ");
        push_json_string(&t.qos_profile, out);
        out.push_str(", \"topic\": ");
        push_json_string(&t.topic, out);
        out.push('}');
    }
    if !lib.talkers.is_empty() {
        out.push('\n');
        out.push_str(&inner);
    }
    out.push_str("],\n");
    out.push_str(&inner);
    out.push_str("\"listeners\": [");
    for (i, l) in lib.listeners.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('\n');
        out.push_str(&" ".repeat(indent + 4));
        out.push_str("{\"stream_id\": ");
        push_json_string(&l.stream_id, out);
        out.push_str(", \"topic\": ");
        push_json_string(&l.topic, out);
        out.push('}');
    }
    if !lib.listeners.is_empty() {
        out.push('\n');
        out.push_str(&inner);
    }
    out.push(']');
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push('}');
}

fn push_opt_field(out: &mut String, key: &str, val: Option<u32>) {
    if let Some(v) = val {
        out.push_str(", \"");
        out.push_str(key);
        out.push_str("\": ");
        out.push_str(&alloc::format!("{v}"));
    }
}

fn push_json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&alloc::format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds_tsn>
  <qos_library name="industrial">
    <qos_profile name="cyclic">
      <vlan_id>10</vlan_id>
      <pcp>5</pcp>
      <dei>false</dei>
      <dscp>46</dscp>
    </qos_profile>
    <qos_profile name="best-effort">
      <vlan_id>20</vlan_id>
      <pcp>0</pcp>
    </qos_profile>
  </qos_library>
  <domain_library>
    <domain id="0">
      <default_profile>cyclic</default_profile>
    </domain>
    <domain id="7">
      <default_profile>best-effort</default_profile>
    </domain>
  </domain_library>
  <deployment_library>
    <talker stream_id="aa:bb:cc:dd:ee:ff/1">
      <qos_profile>cyclic</qos_profile>
      <topic>SensorData</topic>
    </talker>
    <listener stream_id="aa:bb:cc:dd:ee:ff/1">
      <topic>SensorData</topic>
    </listener>
  </deployment_library>
</dds_tsn>
"#;

    #[test]
    fn parses_full_qos_library_with_two_profiles() {
        let cfg = parse_xml_config(SAMPLE_XML).expect("parse");
        assert_eq!(cfg.qos_library.name, "industrial");
        assert_eq!(cfg.qos_library.profiles.len(), 2);
        let cyclic = &cfg.qos_library.profiles[0];
        assert_eq!(cyclic.name, "cyclic");
        assert_eq!(cyclic.vlan_id, Some(10));
        assert_eq!(cyclic.pcp, Some(5));
        assert_eq!(cyclic.dei, Some(false));
        assert_eq!(cyclic.dscp, Some(46));
    }

    #[test]
    fn parses_domain_library_with_two_domains() {
        let cfg = parse_xml_config(SAMPLE_XML).expect("parse");
        assert_eq!(cfg.domain_library.domains.len(), 2);
        assert_eq!(cfg.domain_library.domains[0].domain_id, 0);
        assert_eq!(cfg.domain_library.domains[0].default_profile, "cyclic");
        assert_eq!(cfg.domain_library.domains[1].domain_id, 7);
    }

    #[test]
    fn parses_deployment_library_talker_listener() {
        let cfg = parse_xml_config(SAMPLE_XML).expect("parse");
        let dl = &cfg.deployment_library;
        assert_eq!(dl.talkers.len(), 1);
        assert_eq!(dl.talkers[0].stream_id, "aa:bb:cc:dd:ee:ff/1");
        assert_eq!(dl.talkers[0].qos_profile, "cyclic");
        assert_eq!(dl.listeners.len(), 1);
        assert_eq!(dl.listeners[0].topic, "SensorData");
    }

    #[test]
    fn malformed_xml_yields_parse_error() {
        let err = parse_xml_config("<not xml").expect_err("error expected");
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn wrong_root_yields_unexpected_root_error() {
        let err = parse_xml_config("<other/>").expect_err("error expected");
        assert!(matches!(err, ConfigError::UnexpectedRoot(_)));
    }

    #[test]
    fn render_json_emits_qos_library_section() {
        let cfg = parse_xml_config(SAMPLE_XML).expect("parse");
        let json = render_json_config(&cfg);
        assert!(json.contains("\"qos_library\""));
        assert!(json.contains("\"name\": \"industrial\""));
        assert!(json.contains("\"name\": \"cyclic\""));
        assert!(json.contains("\"vlan_id\": 10"));
        assert!(json.contains("\"pcp\": 5"));
        assert!(json.contains("\"dei\": false"));
        assert!(json.contains("\"dscp\": 46"));
    }

    #[test]
    fn render_json_emits_domain_library_section() {
        let cfg = parse_xml_config(SAMPLE_XML).expect("parse");
        let json = render_json_config(&cfg);
        assert!(json.contains("\"domain_library\""));
        assert!(json.contains("\"domain_id\": 0"));
        assert!(json.contains("\"default_profile\": \"cyclic\""));
    }

    #[test]
    fn render_json_emits_deployment_library_section() {
        let cfg = parse_xml_config(SAMPLE_XML).expect("parse");
        let json = render_json_config(&cfg);
        assert!(json.contains("\"deployment_library\""));
        assert!(json.contains("\"talkers\""));
        assert!(json.contains("\"listeners\""));
        assert!(json.contains("\"stream_id\": \"aa:bb:cc:dd:ee:ff/1\""));
    }

    #[test]
    fn json_string_escapes_special_chars() {
        let mut s = String::new();
        push_json_string("a\"b\\c\nd\x01e", &mut s);
        // RFC 8259 §7: control chars (< 0x20) MUST be \u-escaped.
        assert_eq!(s, "\"a\\\"b\\\\c\\nd\\u0001e\"");
    }
}
