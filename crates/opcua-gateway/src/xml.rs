// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-OPCUA Gateway XML Configuration Loader (Spec §10).
//!
//! Spec-Quelle: OMG DDS-OPCUA 1.0 §10 (S. 109-127) — XML-Configuration-
//! Schema fuer Bridge-Defs (UAtoDDS / DDStoUA).
//!
//! Wir parsen ein leichtgewichtiges Subset, das fuer den
//! Conformance-Punkt der Spec ausreicht:
//!
//! * `<zerodds_opcua_gateway>` — Root.
//! * `<bridge name="...">` — Connection-Definition mit Domain-Id +
//!   beliebig vielen UA-Connections.
//! * `<ua_to_dds_connection>` und `<dds_to_ua_connection>` —
//!   beide tragen Topic-Name, Type-Name, optional Browse-Path und
//!   Node-Id (numeric oder string).
//!
//! Cross-Ref: `crates/xml/src/qos.rs` als Schwester-Loader fuer
//! DDS-XML 1.0 §7.3.2 (gleiche `roxmltree`-Backend-Wahl).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use roxmltree::{Document, Node};

/// XML-Loader-Fehler (Spec §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpcuaXmlError {
    /// XML-Parser-Fehler (mal-formed XML).
    Parse(String),
    /// Wurzel-Element ist nicht `<zerodds_opcua_gateway>`.
    UnexpectedRoot(String),
    /// Pflicht-Attribut fehlt.
    MissingAttribute {
        /// Element-Name.
        element: String,
        /// Attribut-Name.
        attr: String,
    },
    /// Pflicht-Element fehlt.
    MissingElement {
        /// Element-Name.
        element: String,
    },
    /// Connection-Direction `<ua_to_dds_connection>` /
    /// `<dds_to_ua_connection>` falsch.
    InvalidDirection(String),
    /// Numeric NodeId konnte nicht geparsed werden.
    InvalidNumericNodeId(String),
}

impl fmt::Display for OpcuaXmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(s) => write!(f, "OPCUA XML parse error: {s}"),
            Self::UnexpectedRoot(s) => {
                write!(f, "expected <zerodds_opcua_gateway> root, got <{s}>")
            }
            Self::MissingAttribute { element, attr } => {
                write!(f, "<{element}> missing attribute '{attr}'")
            }
            Self::MissingElement { element } => write!(f, "missing <{element}> element"),
            Self::InvalidDirection(s) => write!(f, "invalid connection direction <{s}>"),
            Self::InvalidNumericNodeId(s) => write!(f, "invalid numeric node id '{s}'"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OpcuaXmlError {}

/// Geparste Top-Level-Konfiguration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayConfig {
    /// Liste aller `<bridge>`-Elemente.
    pub bridges: Vec<BridgeDef>,
}

/// Spec §10 — eine `<bridge>`-Definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeDef {
    /// Bridge-Name (`<bridge name="...">`).
    pub name: String,
    /// DDS Domain-Id (default 0).
    pub domain_id: u32,
    /// Liste aller UA-Connections (beide Richtungen gemischt).
    pub connections: Vec<UaConnection>,
}

/// Eine `<ua_to_dds_connection>` oder `<dds_to_ua_connection>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UaConnection {
    /// Richtung der Bridge.
    pub direction: ConnectionDirection,
    /// DDS-Topic-Name.
    pub dds_topic: String,
    /// DDS-Type-Name.
    pub dds_type: String,
    /// Optionaler `<browse_path>` als 2-Segment-Pfad.
    pub browse_path: Vec<String>,
    /// Optionale `<node_id>`. Spec §8.2.2 — numeric oder string.
    pub node_id: Option<XmlNodeId>,
}

/// Bridge-Direction (Spec §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionDirection {
    /// `<ua_to_dds_connection>` — OPC UA Server -> DDS Topic.
    UaToDds,
    /// `<dds_to_ua_connection>` — DDS Topic -> OPC UA Server.
    DdsToUa,
}

/// Spec §8.2.2 NodeId-Variante im XML-Schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlNodeId {
    /// `<numeric_node_id>` (`u32`).
    Numeric(u32),
    /// `<string_node_id>`.
    StringId(String),
}

/// Parst einen XML-Source-String in eine [`GatewayConfig`].
///
/// Spec §10 / §10.2.
///
/// # Errors
/// Liefert [`OpcuaXmlError`] bei mal-formed XML oder fehlenden
/// Pflicht-Feldern.
pub fn parse_gateway_config(src: &str) -> Result<GatewayConfig, OpcuaXmlError> {
    let doc = Document::parse(src).map_err(|e| OpcuaXmlError::Parse(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "zerodds_opcua_gateway" {
        return Err(OpcuaXmlError::UnexpectedRoot(
            root.tag_name().name().to_string(),
        ));
    }
    let mut bridges = Vec::new();
    for n in root.children().filter(Node::is_element) {
        if n.tag_name().name() == "bridge" {
            bridges.push(parse_bridge(n)?);
        }
    }
    Ok(GatewayConfig { bridges })
}

fn parse_bridge(node: Node<'_, '_>) -> Result<BridgeDef, OpcuaXmlError> {
    let name = required_attr(node, "name")?;
    let domain_id = optional_text_child(node, "domain_id")
        .map(|s| s.parse::<u32>().unwrap_or(0))
        .unwrap_or(0);
    let mut connections = Vec::new();
    for c in node.children().filter(Node::is_element) {
        match c.tag_name().name() {
            "ua_to_dds_connection" => {
                connections.push(parse_connection(c, ConnectionDirection::UaToDds)?);
            }
            "dds_to_ua_connection" => {
                connections.push(parse_connection(c, ConnectionDirection::DdsToUa)?);
            }
            "domain_id" => {} // already extracted
            other => {
                if !other.is_empty() {
                    return Err(OpcuaXmlError::InvalidDirection(other.to_string()));
                }
            }
        }
    }
    Ok(BridgeDef {
        name,
        domain_id,
        connections,
    })
}

fn parse_connection(
    node: Node<'_, '_>,
    direction: ConnectionDirection,
) -> Result<UaConnection, OpcuaXmlError> {
    let dds_topic = required_text_child(node, "dds_topic")?;
    let dds_type = required_text_child(node, "dds_type")?;
    let browse_path = optional_text_child(node, "browse_path")
        .map(|s| s.split('/').map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    let node_id = parse_node_id(node)?;
    Ok(UaConnection {
        direction,
        dds_topic,
        dds_type,
        browse_path,
        node_id,
    })
}

fn parse_node_id(node: Node<'_, '_>) -> Result<Option<XmlNodeId>, OpcuaXmlError> {
    if let Some(numeric) = optional_text_child(node, "numeric_node_id") {
        let parsed = numeric
            .trim()
            .parse::<u32>()
            .map_err(|_| OpcuaXmlError::InvalidNumericNodeId(numeric.to_string()))?;
        return Ok(Some(XmlNodeId::Numeric(parsed)));
    }
    if let Some(s) = optional_text_child(node, "string_node_id") {
        return Ok(Some(XmlNodeId::StringId(s.trim().to_string())));
    }
    Ok(None)
}

fn required_attr(node: Node<'_, '_>, name: &str) -> Result<String, OpcuaXmlError> {
    node.attribute(name)
        .map(ToString::to_string)
        .ok_or_else(|| OpcuaXmlError::MissingAttribute {
            element: node.tag_name().name().to_string(),
            attr: name.to_string(),
        })
}

fn required_text_child(node: Node<'_, '_>, name: &str) -> Result<String, OpcuaXmlError> {
    optional_text_child(node, name).ok_or_else(|| OpcuaXmlError::MissingElement {
        element: name.to_string(),
    })
}

fn optional_text_child(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.children()
        .filter(Node::is_element)
        .find(|n| n.tag_name().name() == name)
        .and_then(|n| n.text().map(str::trim).map(str::to_string))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<zerodds_opcua_gateway>
  <bridge name="primary">
    <domain_id>0</domain_id>
    <ua_to_dds_connection>
      <dds_topic>Tracking</dds_topic>
      <dds_type>robotics::TrackingResult</dds_type>
      <browse_path>Objects/Sensors/Camera1</browse_path>
      <numeric_node_id>2247</numeric_node_id>
    </ua_to_dds_connection>
    <dds_to_ua_connection>
      <dds_topic>Commands</dds_topic>
      <dds_type>robotics::Command</dds_type>
      <string_node_id>ns=2;s=Cmd</string_node_id>
    </dds_to_ua_connection>
  </bridge>
  <bridge name="auxiliary">
    <ua_to_dds_connection>
      <dds_topic>Heartbeat</dds_topic>
      <dds_type>robotics::Heartbeat</dds_type>
    </ua_to_dds_connection>
  </bridge>
</zerodds_opcua_gateway>
"#;

    #[test]
    fn parses_full_two_bridge_sample() {
        let cfg = parse_gateway_config(SAMPLE_XML).expect("parse");
        assert_eq!(cfg.bridges.len(), 2);
        let p = &cfg.bridges[0];
        assert_eq!(p.name, "primary");
        assert_eq!(p.domain_id, 0);
        assert_eq!(p.connections.len(), 2);
    }

    #[test]
    fn ua_to_dds_connection_yields_expected_fields() {
        let cfg = parse_gateway_config(SAMPLE_XML).expect("parse");
        let c = &cfg.bridges[0].connections[0];
        assert_eq!(c.direction, ConnectionDirection::UaToDds);
        assert_eq!(c.dds_topic, "Tracking");
        assert_eq!(c.dds_type, "robotics::TrackingResult");
        assert_eq!(
            c.browse_path,
            alloc::vec![
                String::from("Objects"),
                String::from("Sensors"),
                String::from("Camera1")
            ]
        );
        assert_eq!(c.node_id, Some(XmlNodeId::Numeric(2247)));
    }

    #[test]
    fn dds_to_ua_connection_uses_string_node_id() {
        let cfg = parse_gateway_config(SAMPLE_XML).expect("parse");
        let c = &cfg.bridges[0].connections[1];
        assert_eq!(c.direction, ConnectionDirection::DdsToUa);
        assert_eq!(
            c.node_id,
            Some(XmlNodeId::StringId("ns=2;s=Cmd".to_string()))
        );
    }

    #[test]
    fn auxiliary_bridge_omits_optional_node_id_and_browse_path() {
        let cfg = parse_gateway_config(SAMPLE_XML).expect("parse");
        let b = &cfg.bridges[1];
        let c = &b.connections[0];
        assert!(c.browse_path.is_empty());
        assert!(c.node_id.is_none());
    }

    #[test]
    fn malformed_xml_yields_parse_error() {
        let err = parse_gateway_config("<not xml").expect_err("error expected");
        assert!(matches!(err, OpcuaXmlError::Parse(_)));
    }

    #[test]
    fn wrong_root_element_yields_unexpected_root_error() {
        let err = parse_gateway_config("<wrong/>").expect_err("error expected");
        assert!(matches!(err, OpcuaXmlError::UnexpectedRoot(_)));
    }

    #[test]
    fn missing_dds_topic_yields_missing_element_error() {
        let xml = r#"<zerodds_opcua_gateway>
            <bridge name="x">
              <ua_to_dds_connection>
                <dds_type>T</dds_type>
              </ua_to_dds_connection>
            </bridge>
          </zerodds_opcua_gateway>"#;
        let err = parse_gateway_config(xml).expect_err("error expected");
        assert!(matches!(err, OpcuaXmlError::MissingElement { .. }));
    }

    #[test]
    fn invalid_numeric_node_id_yields_specific_error() {
        let xml = r#"<zerodds_opcua_gateway>
            <bridge name="x">
              <ua_to_dds_connection>
                <dds_topic>T</dds_topic>
                <dds_type>U</dds_type>
                <numeric_node_id>not-a-number</numeric_node_id>
              </ua_to_dds_connection>
            </bridge>
          </zerodds_opcua_gateway>"#;
        let err = parse_gateway_config(xml).expect_err("error expected");
        assert!(matches!(err, OpcuaXmlError::InvalidNumericNodeId(_)));
    }

    #[test]
    fn bridge_without_name_yields_missing_attribute_error() {
        let xml = r#"<zerodds_opcua_gateway><bridge/></zerodds_opcua_gateway>"#;
        let err = parse_gateway_config(xml).expect_err("error expected");
        assert!(matches!(err, OpcuaXmlError::MissingAttribute { .. }));
    }
}
