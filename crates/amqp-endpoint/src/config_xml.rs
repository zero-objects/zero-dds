// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XML configuration loader for Annex-A structures.
//!
//! Spec §9.2 — Implementations MAY accept configuration in XML
//! form. The namespace `http://www.zerodds.org/dds-amqp/v1.0`
//! identifies the top-level element `<dds-amqp>`.
//!
//! Requires the Cargo feature `std` (on by default), which pulls in
//! `roxmltree`.
//!
//! # Example input
//!
//! ```xml
//! <dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
//!   <endpoint name="edge-gateway-1">
//!     <listen-uri>amqp://0.0.0.0:5672</listen-uri>
//!     <bridge-id>3a7c-...</bridge-id>
//!     <bridge-hop-cap>8</bridge-hop-cap>
//!     <tls enabled="true" cert="cert.pem" key="key.pem" ca="ca.pem"
//!          require-client-cert="true"/>
//!     <sasl>
//!       <mechanism>EXTERNAL</mechanism>
//!       <mechanism>PLAIN</mechanism>
//!     </sasl>
//!     <topics>
//!       <topic-mapping>
//!         <amqp-address>Sensor</amqp-address>
//!         <dds-topic>SensorReading</dds-topic>
//!         <dds-type>org::iot::SensorReading</dds-type>
//!         <domain-id>0</domain-id>
//!         <partition>zone-a</partition>
//!         <body-mode>MODE_PASSTHROUGH</body-mode>
//!         <descriptor-form>DESC_TRUNCATED</descriptor-form>
//!         <rpc-aware>false</rpc-aware>
//!       </topic-mapping>
//!     </topics>
//!     <limits max-connections="1024" max-frame-size="1048576"
//!             idle-timeout-ms="60000"/>
//!   </endpoint>
//! </dds-amqp>
//! ```

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crate::annex_a::{
    AmqpBridgeConfig, AmqpEndpointConfig, BodyEncodingMode, DescriptorForm, DynamicTopicConfig,
    LinkDirection, ResourceLimits, SaslConfig, SaslMechanism, TimeMapping, TlsConfig, TopicMapping,
};
use crate::security::{DataProtectionKind, GovernanceDocument, GovernanceRule};

/// Spec §9.2 — XML namespace for `<dds-amqp>`.
pub const XML_NAMESPACE: &str = "http://www.zerodds.org/dds-amqp/v1.0";

/// XML loader error.
#[derive(Debug)]
pub enum XmlConfigError {
    /// XML parser error from `roxmltree`.
    Parse(String),
    /// Wrong top-level element.
    UnexpectedRoot {
        /// Expected root element name.
        expected: &'static str,
        /// Actual root element name.
        got: String,
    },
    /// Required element missing.
    MissingElement(String),
    /// Required attribute missing.
    MissingAttribute(String),
    /// Value could not be parsed.
    InvalidValue {
        /// Element/attribute name.
        field: String,
        /// Actual raw value.
        value: String,
    },
}

impl fmt::Display for XmlConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(s) => write!(f, "xml parse error: {s}"),
            Self::UnexpectedRoot { expected, got } => {
                write!(f, "expected <{expected}> root, got <{got}>")
            }
            Self::MissingElement(s) => write!(f, "missing element <{s}>"),
            Self::MissingAttribute(s) => write!(f, "missing attribute @{s}"),
            Self::InvalidValue { field, value } => {
                write!(f, "invalid value for {field}: '{value}'")
            }
        }
    }
}

impl std::error::Error for XmlConfigError {}

/// Spec §9.2 — top-level parse result.
#[derive(Debug, Clone, Default)]
pub struct DdsAmqpConfig {
    /// Endpoints (`<endpoint>`).
    pub endpoints: Vec<AmqpEndpointConfig>,
    /// Bridges (`<bridge>`).
    pub bridges: Vec<AmqpBridgeConfig>,
}

/// Spec §9.2 — parse XML into `DdsAmqpConfig`.
///
/// # Errors
/// `XmlConfigError` on parser errors or missing required fields.
pub fn parse_config(xml: &str) -> Result<DdsAmqpConfig, XmlConfigError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| XmlConfigError::Parse(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "dds-amqp" {
        return Err(XmlConfigError::UnexpectedRoot {
            expected: "dds-amqp",
            got: root.tag_name().name().to_string(),
        });
    }
    let mut cfg = DdsAmqpConfig::default();
    for child in root.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "endpoint" => cfg.endpoints.push(parse_endpoint(child)?),
            "bridge" => cfg.bridges.push(parse_bridge(child)?),
            _ => {} // unknown top-level elements are ignored.
        }
    }
    Ok(cfg)
}

fn parse_endpoint(node: roxmltree::Node) -> Result<AmqpEndpointConfig, XmlConfigError> {
    let mut cfg = AmqpEndpointConfig {
        endpoint_name: attr(&node, "name").unwrap_or_default(),
        bridge_hop_cap: 8,
        ..AmqpEndpointConfig::default()
    };
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "listen-uri" => cfg.listen_uri = text(child),
            "bridge-id" => cfg.bridge_id = text(child),
            "bridge-hop-cap" => cfg.bridge_hop_cap = parse_u8(child)?,
            "tls" => cfg.tls = parse_tls(child)?,
            "sasl" => cfg.sasl = parse_sasl(child)?,
            "topics" => cfg.topics = parse_topics(child)?,
            "dynamic" => cfg.dynamic = parse_dynamic(child)?,
            "limits" => cfg.limits = parse_limits(child)?,
            _ => {}
        }
    }
    Ok(cfg)
}

fn parse_bridge(node: roxmltree::Node) -> Result<AmqpBridgeConfig, XmlConfigError> {
    let mut cfg = AmqpBridgeConfig {
        bridge_name: attr(&node, "name").unwrap_or_default(),
        bridge_hop_cap: 8,
        ..AmqpBridgeConfig::default()
    };
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "upstream-uri" => cfg.upstream_uri = text(child),
            "bridge-id" => cfg.bridge_id = text(child),
            "bridge-hop-cap" => cfg.bridge_hop_cap = parse_u8(child)?,
            "tls" => cfg.tls = parse_tls(child)?,
            "sasl" => cfg.sasl = parse_sasl(child)?,
            "topics" => cfg.topics = parse_topics(child)?,
            _ => {}
        }
    }
    Ok(cfg)
}

fn parse_tls(node: roxmltree::Node) -> Result<TlsConfig, XmlConfigError> {
    Ok(TlsConfig {
        enabled: parse_bool_attr(&node, "enabled").unwrap_or(false),
        cert_path: attr(&node, "cert").unwrap_or_default(),
        key_path: attr(&node, "key").unwrap_or_default(),
        ca_path: attr(&node, "ca").unwrap_or_default(),
        require_client_cert: parse_bool_attr(&node, "require-client-cert").unwrap_or(false),
    })
}

fn parse_sasl(node: roxmltree::Node) -> Result<SaslConfig, XmlConfigError> {
    let mut sasl = SaslConfig::default();
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "mechanism" => {
                let raw = text(child);
                let m = SaslMechanism::parse(&raw).ok_or(XmlConfigError::InvalidValue {
                    field: "mechanism".into(),
                    value: raw,
                })?;
                sasl.enabled_mechanisms.push(m);
            }
            "credential-store-uri" => sasl.credential_store_uri = text(child),
            _ => {}
        }
    }
    Ok(sasl)
}

fn parse_topics(node: roxmltree::Node) -> Result<Vec<TopicMapping>, XmlConfigError> {
    let mut out = Vec::new();
    for child in node.children().filter(roxmltree::Node::is_element) {
        if child.tag_name().name() == "topic-mapping" {
            out.push(parse_topic_mapping(child)?);
        }
    }
    Ok(out)
}

fn parse_topic_mapping(node: roxmltree::Node) -> Result<TopicMapping, XmlConfigError> {
    let mut t = TopicMapping::default();
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "amqp-address" => t.amqp_address = text(child),
            "dds-topic" => t.dds_topic = text(child),
            "dds-type" => t.dds_type_name = text(child),
            "domain-id" => t.dds_domain_id = parse_u32(child)?,
            "partition" => t.dds_partition.push(text(child)),
            "body-mode" => {
                let raw = text(child);
                t.mode = BodyEncodingMode::parse(&raw).ok_or(XmlConfigError::InvalidValue {
                    field: "body-mode".into(),
                    value: raw,
                })?;
            }
            "time-mapping" => {
                let raw = text(child);
                t.time_mapping = TimeMapping::parse(&raw).ok_or(XmlConfigError::InvalidValue {
                    field: "time-mapping".into(),
                    value: raw,
                })?;
            }
            "descriptor-form" => {
                let raw = text(child);
                t.descriptor_form =
                    DescriptorForm::parse(&raw).ok_or(XmlConfigError::InvalidValue {
                        field: "descriptor-form".into(),
                        value: raw,
                    })?;
            }
            "rpc-aware" => t.rpc_aware = parse_bool(child)?,
            "rpc-timeout-ms" => t.rpc_timeout_ms = parse_u32(child)?,
            "direction" => {
                let raw = text(child);
                t.direction = LinkDirection::parse(&raw).ok_or(XmlConfigError::InvalidValue {
                    field: "direction".into(),
                    value: raw,
                })?;
            }
            _ => {}
        }
    }
    if t.amqp_address.is_empty() {
        return Err(XmlConfigError::MissingElement("amqp-address".into()));
    }
    if t.dds_topic.is_empty() {
        return Err(XmlConfigError::MissingElement("dds-topic".into()));
    }
    Ok(t)
}

fn parse_dynamic(node: roxmltree::Node) -> Result<DynamicTopicConfig, XmlConfigError> {
    let mut d = DynamicTopicConfig::default();
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "permit" => d.permit_dynamic_topics = parse_bool(child)?,
            "default-type" => d.dynamic_topic_default_type = text(child),
            "default-mode" => {
                let raw = text(child);
                d.default_mode =
                    BodyEncodingMode::parse(&raw).ok_or(XmlConfigError::InvalidValue {
                        field: "default-mode".into(),
                        value: raw,
                    })?;
            }
            _ => {}
        }
    }
    Ok(d)
}

fn parse_limits(node: roxmltree::Node) -> Result<ResourceLimits, XmlConfigError> {
    let mut l = ResourceLimits::default();
    if let Some(s) = attr(&node, "max-connections") {
        l.max_connections = parse_u32_str(&s, "max-connections")?;
    }
    if let Some(s) = attr(&node, "max-sessions-per-connection") {
        l.max_sessions_per_connection = parse_u32_str(&s, "max-sessions-per-connection")?;
    }
    if let Some(s) = attr(&node, "max-links-per-session") {
        l.max_links_per_session = parse_u32_str(&s, "max-links-per-session")?;
    }
    if let Some(s) = attr(&node, "max-frame-size") {
        l.max_frame_size = parse_u32_str(&s, "max-frame-size")?;
    }
    if let Some(s) = attr(&node, "idle-timeout-ms") {
        l.idle_timeout_ms = parse_u64_str(&s, "idle-timeout-ms")?;
    }
    Ok(l)
}

// ============================================================
// Governance document XML (§10.4)
// ============================================================

/// Spec §10.4 — load a governance document from XML.
///
/// Expects a `<governance>` root element with `<rule>` children.
///
/// # Errors
/// `XmlConfigError`.
pub fn parse_governance(xml: &str) -> Result<GovernanceDocument, XmlConfigError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| XmlConfigError::Parse(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "governance" {
        return Err(XmlConfigError::UnexpectedRoot {
            expected: "governance",
            got: root.tag_name().name().to_string(),
        });
    }
    let mut g = GovernanceDocument::new();
    for child in root.children().filter(roxmltree::Node::is_element) {
        if child.tag_name().name() == "rule" {
            g.add_rule(parse_governance_rule(child)?);
        }
    }
    Ok(g)
}

fn parse_governance_rule(node: roxmltree::Node) -> Result<GovernanceRule, XmlConfigError> {
    let mut topic_pattern = String::new();
    let mut enable_discovery = true;
    let mut enable_liveliness = true;
    let mut data_protection_kind = DataProtectionKind::None;
    for child in node.children().filter(roxmltree::Node::is_element) {
        match child.tag_name().name() {
            "topic-pattern" => topic_pattern = text(child),
            "enable-discovery" => enable_discovery = parse_bool(child)?,
            "enable-liveliness" => enable_liveliness = parse_bool(child)?,
            "data-protection" => {
                let raw = text(child);
                data_protection_kind = match raw.as_str() {
                    "None" | "NONE" | "none" => DataProtectionKind::None,
                    "SignOnly" | "SIGN_ONLY" | "sign-only" => DataProtectionKind::SignOnly,
                    "SignAndEncrypt" | "SIGN_AND_ENCRYPT" | "sign-and-encrypt" => {
                        DataProtectionKind::SignAndEncrypt
                    }
                    _ => {
                        return Err(XmlConfigError::InvalidValue {
                            field: "data-protection".into(),
                            value: raw,
                        });
                    }
                };
            }
            _ => {}
        }
    }
    if topic_pattern.is_empty() {
        return Err(XmlConfigError::MissingElement("topic-pattern".into()));
    }
    Ok(GovernanceRule {
        topic_pattern,
        enable_discovery,
        enable_liveliness,
        data_protection_kind,
    })
}

// ============================================================
// Helpers
// ============================================================

fn attr(node: &roxmltree::Node, name: &str) -> Option<String> {
    node.attribute(name).map(ToString::to_string)
}

fn text(node: roxmltree::Node) -> String {
    node.text().unwrap_or("").trim().to_string()
}

fn parse_bool(node: roxmltree::Node) -> Result<bool, XmlConfigError> {
    let raw = text(node);
    match raw.as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(XmlConfigError::InvalidValue {
            field: node.tag_name().name().to_string(),
            value: raw,
        }),
    }
}

fn parse_bool_attr(node: &roxmltree::Node, name: &str) -> Option<bool> {
    let raw = node.attribute(name)?;
    match raw {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

fn parse_u8(node: roxmltree::Node) -> Result<u8, XmlConfigError> {
    let raw = text(node);
    raw.parse::<u8>().map_err(|_| XmlConfigError::InvalidValue {
        field: node.tag_name().name().to_string(),
        value: raw,
    })
}

fn parse_u32(node: roxmltree::Node) -> Result<u32, XmlConfigError> {
    let raw = text(node);
    raw.parse::<u32>()
        .map_err(|_| XmlConfigError::InvalidValue {
            field: node.tag_name().name().to_string(),
            value: raw,
        })
}

fn parse_u32_str(s: &str, field: &str) -> Result<u32, XmlConfigError> {
    s.parse::<u32>().map_err(|_| XmlConfigError::InvalidValue {
        field: field.to_string(),
        value: s.to_string(),
    })
}

fn parse_u64_str(s: &str, field: &str) -> Result<u64, XmlConfigError> {
    s.parse::<u64>().map_err(|_| XmlConfigError::InvalidValue {
        field: field.to_string(),
        value: s.to_string(),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_endpoint() {
        let xml = r#"<dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
          <endpoint name="ep1">
            <listen-uri>amqp://0.0.0.0:5672</listen-uri>
          </endpoint>
        </dds-amqp>"#;
        let cfg = parse_config(xml).unwrap();
        assert_eq!(cfg.endpoints.len(), 1);
        let ep = &cfg.endpoints[0];
        assert_eq!(ep.endpoint_name, "ep1");
        assert_eq!(ep.listen_uri, "amqp://0.0.0.0:5672");
        assert_eq!(ep.bridge_hop_cap, 8); // default.
    }

    #[test]
    fn parse_endpoint_with_topics_tls_sasl_limits() {
        let xml = r#"<dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
          <endpoint name="full">
            <listen-uri>amqp://0.0.0.0:5672</listen-uri>
            <bridge-id>3a7c-uuid</bridge-id>
            <bridge-hop-cap>10</bridge-hop-cap>
            <tls enabled="true" cert="c.pem" key="k.pem" ca="ca.pem"
                 require-client-cert="true"/>
            <sasl>
              <mechanism>EXTERNAL</mechanism>
              <mechanism>PLAIN</mechanism>
            </sasl>
            <topics>
              <topic-mapping>
                <amqp-address>Sensor</amqp-address>
                <dds-topic>SensorReading</dds-topic>
                <dds-type>Pose</dds-type>
                <domain-id>7</domain-id>
                <partition>zone-a</partition>
                <partition>zone-b</partition>
                <body-mode>MODE_JSON</body-mode>
                <descriptor-form>DESC_FULL</descriptor-form>
                <rpc-aware>true</rpc-aware>
                <rpc-timeout-ms>15000</rpc-timeout-ms>
              </topic-mapping>
            </topics>
            <limits max-connections="2048" max-frame-size="65536"
                    idle-timeout-ms="120000"/>
          </endpoint>
        </dds-amqp>"#;
        let cfg = parse_config(xml).unwrap();
        let ep = &cfg.endpoints[0];
        assert_eq!(ep.bridge_id, "3a7c-uuid");
        assert_eq!(ep.bridge_hop_cap, 10);
        assert!(ep.tls.enabled);
        assert_eq!(ep.tls.cert_path, "c.pem");
        assert!(ep.tls.require_client_cert);
        assert_eq!(ep.sasl.enabled_mechanisms.len(), 2);
        assert_eq!(ep.sasl.enabled_mechanisms[0], SaslMechanism::SaslExternal);
        assert_eq!(ep.topics.len(), 1);
        let t = &ep.topics[0];
        assert_eq!(t.amqp_address, "Sensor");
        assert_eq!(t.dds_topic, "SensorReading");
        assert_eq!(t.dds_type_name, "Pose");
        assert_eq!(t.dds_domain_id, 7);
        assert_eq!(t.dds_partition.len(), 2);
        assert_eq!(t.mode, BodyEncodingMode::ModeJson);
        assert_eq!(t.descriptor_form, DescriptorForm::DescFull);
        assert!(t.rpc_aware);
        assert_eq!(t.rpc_timeout_ms, 15_000);
        assert_eq!(ep.limits.max_connections, 2048);
        assert_eq!(ep.limits.max_frame_size, 65_536);
        assert_eq!(ep.limits.idle_timeout_ms, 120_000);
    }

    #[test]
    fn parse_bridge_with_upstream() {
        let xml = r#"<dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
          <bridge name="cloud-bridge">
            <upstream-uri>amqp://broker.example:5671</upstream-uri>
            <bridge-id>cloud-bridge-uuid</bridge-id>
            <topics>
              <topic-mapping>
                <amqp-address>Cmd</amqp-address>
                <dds-topic>Cmd</dds-topic>
                <dds-type>CmdType</dds-type>
              </topic-mapping>
            </topics>
          </bridge>
        </dds-amqp>"#;
        let cfg = parse_config(xml).unwrap();
        assert_eq!(cfg.bridges.len(), 1);
        let br = &cfg.bridges[0];
        assert_eq!(br.bridge_name, "cloud-bridge");
        assert_eq!(br.upstream_uri, "amqp://broker.example:5671");
        assert_eq!(br.bridge_id, "cloud-bridge-uuid");
        assert_eq!(br.topics.len(), 1);
    }

    #[test]
    fn unknown_root_yields_error() {
        let xml = r#"<not-dds-amqp/>"#;
        let err = parse_config(xml).unwrap_err();
        assert!(matches!(err, XmlConfigError::UnexpectedRoot { .. }));
    }

    #[test]
    fn missing_amqp_address_yields_error() {
        let xml = r#"<dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
          <endpoint name="ep">
            <topics>
              <topic-mapping>
                <dds-topic>T</dds-topic>
              </topic-mapping>
            </topics>
          </endpoint>
        </dds-amqp>"#;
        let err = parse_config(xml).unwrap_err();
        assert!(
            matches!(&err, XmlConfigError::MissingElement(s) if s == "amqp-address"),
            "got {err:?}"
        );
    }

    #[test]
    fn invalid_body_mode_yields_error() {
        let xml = r#"<dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
          <endpoint name="ep">
            <topics>
              <topic-mapping>
                <amqp-address>X</amqp-address>
                <dds-topic>T</dds-topic>
                <body-mode>NOT_A_MODE</body-mode>
              </topic-mapping>
            </topics>
          </endpoint>
        </dds-amqp>"#;
        let err = parse_config(xml).unwrap_err();
        assert!(matches!(err, XmlConfigError::InvalidValue { .. }));
    }

    #[test]
    fn malformed_xml_yields_parse_error() {
        let xml = "<dds-amqp><not-closed>";
        let err = parse_config(xml).unwrap_err();
        assert!(matches!(err, XmlConfigError::Parse(_)));
    }

    #[test]
    fn governance_document_loads_rules() {
        let xml = r#"<governance xmlns="http://www.zerodds.org/dds-amqp/v1.0">
          <rule>
            <topic-pattern>Sensor*</topic-pattern>
            <enable-discovery>true</enable-discovery>
            <enable-liveliness>true</enable-liveliness>
            <data-protection>SignOnly</data-protection>
          </rule>
          <rule>
            <topic-pattern>Cmd</topic-pattern>
            <data-protection>SignAndEncrypt</data-protection>
          </rule>
        </governance>"#;
        let g = parse_governance(xml).unwrap();
        let r = g.resolve("SensorTemp").unwrap();
        assert_eq!(r.data_protection_kind, DataProtectionKind::SignOnly);
        let r2 = g.resolve("Cmd").unwrap();
        assert_eq!(r2.data_protection_kind, DataProtectionKind::SignAndEncrypt);
    }

    #[test]
    fn governance_missing_topic_pattern_errors() {
        let xml = r#"<governance>
          <rule>
            <data-protection>None</data-protection>
          </rule>
        </governance>"#;
        let err = parse_governance(xml).unwrap_err();
        assert!(matches!(&err, XmlConfigError::MissingElement(s) if s == "topic-pattern"));
    }

    #[test]
    fn governance_invalid_data_protection_errors() {
        let xml = r#"<governance>
          <rule>
            <topic-pattern>X</topic-pattern>
            <data-protection>Bogus</data-protection>
          </rule>
        </governance>"#;
        let err = parse_governance(xml).unwrap_err();
        assert!(matches!(err, XmlConfigError::InvalidValue { .. }));
    }

    #[test]
    fn unknown_elements_are_ignored() {
        // Forward compat: unknown elements (e.g. from future spec
        // revisions) are not treated as errors.
        let xml = r#"<dds-amqp xmlns="http://www.zerodds.org/dds-amqp/v1.0">
          <endpoint name="ep">
            <listen-uri>amqp://x:1</listen-uri>
            <future-feature>some-config</future-feature>
          </endpoint>
        </dds-amqp>"#;
        let cfg = parse_config(xml).unwrap();
        assert_eq!(cfg.endpoints[0].listen_uri, "amqp://x:1");
    }

    #[test]
    fn xml_namespace_constant_matches_spec() {
        assert_eq!(XML_NAMESPACE, "http://www.zerodds.org/dds-amqp/v1.0");
    }
}
