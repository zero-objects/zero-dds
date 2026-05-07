// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XML PSM — Spec §7.3.1.
//!
//! Liest eine `<dds_tsn>`-Konfig per Spec-XSD und materialisiert sie
//! in [`super::DdsTsnConfig`]. Schreibt im selben Format zurueck. Wir
//! folgen dem Spec-XSD-Schema vereinfacht: Element-Namen + Mult-
//! Constraints kommen aus Tab 7.1-7.14.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use roxmltree::{Document, Node};

use super::DdsTsnConfig;
use super::application::{
    Application, ApplicationLibrary, DataReaderQosRef, DataWriterQosRef, Domain, DomainLibrary,
    DomainParticipant, DomainParticipantLibrary, DomainParticipantQosRef, PublisherQosRef,
    QosLibrary, QosProfile, RegisteredType, SubscriberQosRef, TopicLibraryEntry, TopicQosRef,
};
use super::deployment::{
    Deployment, DeploymentConfiguration, DeploymentLibrary, IpV4, IpV6, MacAddr, Node as PimNode,
    NodeLibrary,
};

/// Fehler waehrend XML-Parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseXmlError {
    /// Underlying-Roxmltree-Fehler (Wohlgeformtheits-Verletzung).
    Wellformedness(String),
    /// Root-Element ist nicht `<dds_tsn>`.
    UnexpectedRoot(String),
    /// Pflicht-Attribut fehlt.
    MissingAttribute {
        /// Element, das das Attribut tragen sollte.
        element: String,
        /// Attribut-Name.
        attribute: String,
    },
    /// Wert kann nicht in den erwarteten Type geparst werden.
    InvalidValue {
        /// Element + Attribut/Text.
        element: String,
        /// Aktueller Wert.
        value: String,
        /// Erwarteter Type-Name.
        expected: &'static str,
    },
}

/// Parsiert ein `<dds_tsn>`-XML-Dokument.
///
/// # Errors
/// Siehe [`ParseXmlError`].
pub fn parse_dds_tsn_xml(src: &str) -> Result<DdsTsnConfig, ParseXmlError> {
    let doc = Document::parse(src).map_err(|e| ParseXmlError::Wellformedness(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "dds_tsn" {
        return Err(ParseXmlError::UnexpectedRoot(
            root.tag_name().name().to_string(),
        ));
    }
    let mut cfg = DdsTsnConfig::default();
    for c in root.children().filter(Node::is_element) {
        match c.tag_name().name() {
            "qos_library" => cfg.qos_libraries.push(parse_qos_library(c)?),
            "domain_library" => cfg.domain_libraries.push(parse_domain_library(c)?),
            "domain_participant_library" => cfg
                .domain_participant_libraries
                .push(parse_domain_participant_library(c)?),
            "application_library" => {
                cfg.application_libraries
                    .push(parse_application_library(c)?);
            }
            "node_library" => cfg.node_libraries.push(parse_node_library(c)?),
            "deployment_library" => cfg.deployment_libraries.push(parse_deployment_library(c)?),
            _ => {}
        }
    }
    Ok(cfg)
}

fn require_attr(n: Node<'_, '_>, attr: &str) -> Result<String, ParseXmlError> {
    n.attribute(attr)
        .map(str::to_string)
        .ok_or_else(|| ParseXmlError::MissingAttribute {
            element: n.tag_name().name().to_string(),
            attribute: attr.to_string(),
        })
}

fn parse_qos_library(n: Node<'_, '_>) -> Result<QosLibrary, ParseXmlError> {
    let name = require_attr(n, "name")?;
    let mut qos_profiles = Vec::new();
    for p in n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "qos_profile")
    {
        qos_profiles.push(parse_qos_profile(p)?);
    }
    Ok(QosLibrary { name, qos_profiles })
}

fn parse_qos_profile(n: Node<'_, '_>) -> Result<QosProfile, ParseXmlError> {
    let name = require_attr(n, "name")?;
    let mut p = QosProfile {
        name,
        ..QosProfile::default()
    };
    for c in n.children().filter(Node::is_element) {
        let inner_name = c.attribute("name").unwrap_or("").to_string();
        let raw_qos = c.text().unwrap_or("").trim().to_string();
        match c.tag_name().name() {
            "domain_participant_qos" => p.domain_participant_qos.push(DomainParticipantQosRef {
                name: inner_name,
                raw_qos,
            }),
            "publisher_qos" => p.publisher_qos.push(PublisherQosRef {
                name: inner_name,
                raw_qos,
            }),
            "subscriber_qos" => p.subscriber_qos.push(SubscriberQosRef {
                name: inner_name,
                raw_qos,
            }),
            "topic_qos" => p.topic_qos.push(TopicQosRef {
                name: inner_name,
                raw_qos,
            }),
            "datawriter_qos" => p.datawriter_qos.push(DataWriterQosRef {
                name: inner_name,
                raw_qos,
            }),
            "datareader_qos" => p.datareader_qos.push(DataReaderQosRef {
                name: inner_name,
                raw_qos,
            }),
            _ => {}
        }
    }
    Ok(p)
}

fn parse_domain_library(n: Node<'_, '_>) -> Result<DomainLibrary, ParseXmlError> {
    let name = require_attr(n, "name")?;
    let mut domains = Vec::new();
    for d in n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "domain")
    {
        let mut dom = Domain {
            name: require_attr(d, "name")?,
            ..Domain::default()
        };
        for child in d.children().filter(Node::is_element) {
            match child.tag_name().name() {
                "registered_type" => dom.registered_types.push(RegisteredType {
                    name: require_attr(child, "name")?,
                    type_ref: require_attr(child, "type_ref")?,
                }),
                "topic" => dom.topics.push(TopicLibraryEntry {
                    topic_name: require_attr(child, "topic_name")?,
                    type_name: require_attr(child, "type_name")?,
                }),
                _ => {}
            }
        }
        domains.push(dom);
    }
    Ok(DomainLibrary { name, domains })
}

fn parse_domain_participant_library(
    n: Node<'_, '_>,
) -> Result<DomainParticipantLibrary, ParseXmlError> {
    let name = require_attr(n, "name")?;
    let mut domain_participants = Vec::new();
    for dp in n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "domain_participant")
    {
        domain_participants.push(DomainParticipant {
            name: require_attr(dp, "name")?,
            base_name: dp.attribute("base_name").map(str::to_string),
            domain_ref: dp.attribute("domain_ref").map(str::to_string),
        });
    }
    Ok(DomainParticipantLibrary {
        name,
        domain_participants,
    })
}

fn parse_application_library(n: Node<'_, '_>) -> Result<ApplicationLibrary, ParseXmlError> {
    let name = require_attr(n, "name")?;
    let mut applications = Vec::new();
    for a in n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "application")
    {
        let mut refs = Vec::new();
        for child in a
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "domain_participant_ref")
        {
            if let Some(t) = child.text() {
                refs.push(t.trim().to_string());
            }
        }
        applications.push(Application {
            name: require_attr(a, "name")?,
            domain_participant_refs: refs,
        });
    }
    Ok(ApplicationLibrary { name, applications })
}

fn parse_node_library(n: Node<'_, '_>) -> Result<NodeLibrary, ParseXmlError> {
    let name = require_attr(n, "name")?;
    let mut nodes = Vec::new();
    for nd in n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "node")
    {
        let mac_str = require_attr(nd, "mac_address")?;
        let mac = parse_mac(&mac_str).ok_or_else(|| ParseXmlError::InvalidValue {
            element: "node".into(),
            value: mac_str.clone(),
            expected: "MAC address (6 hex octets, separated by `:` or `-`)",
        })?;
        let ipv4 = nd
            .attribute("ipv4_address")
            .map(parse_ipv4)
            .transpose()?
            .flatten();
        let ipv6 = nd
            .attribute("ipv6_address")
            .map(parse_ipv6)
            .transpose()?
            .flatten();
        nodes.push(PimNode {
            name: require_attr(nd, "name")?,
            hostname: require_attr(nd, "hostname")?,
            mac_address: mac,
            ipv4_address: ipv4,
            ipv6_address: ipv6,
        });
    }
    Ok(NodeLibrary { name, nodes })
}

fn parse_deployment_library(n: Node<'_, '_>) -> Result<DeploymentLibrary, ParseXmlError> {
    let name = require_attr(n, "name")?;
    let mut deployments = Vec::new();
    for d in n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "deployment")
    {
        let mut refs = Vec::new();
        let mut configuration = None;
        for child in d.children().filter(Node::is_element) {
            match child.tag_name().name() {
                "application_ref" => {
                    if let Some(t) = child.text() {
                        refs.push(t.trim().to_string());
                    }
                }
                "configuration" => {
                    let tsn_ref = child
                        .children()
                        .filter(Node::is_element)
                        .find(|c| c.tag_name().name() == "tsn")
                        .and_then(|c| c.attribute("name").map(str::to_string));
                    configuration = Some(DeploymentConfiguration {
                        tsn_configuration_ref: tsn_ref,
                    });
                }
                _ => {}
            }
        }
        deployments.push(Deployment {
            name: require_attr(d, "name")?,
            node_ref: require_attr(d, "node_ref")?,
            application_refs: refs,
            configuration,
        });
    }
    Ok(DeploymentLibrary { name, deployments })
}

// -------------------------------------------------------------------
// Hilfs-Parser fuer Adressen.
// -------------------------------------------------------------------

fn parse_mac(s: &str) -> Option<MacAddr> {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | ' '))
        .collect();
    if cleaned.len() != 12 || !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 6];
    for i in 0..6 {
        out[i] = u8::from_str_radix(&cleaned[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(MacAddr(out))
}

fn parse_ipv4(s: &str) -> Result<Option<IpV4>, ParseXmlError> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return Err(ParseXmlError::InvalidValue {
            element: "node".into(),
            value: s.to_string(),
            expected: "IPv4 address (a.b.c.d)",
        });
    }
    let mut out = [0u8; 4];
    for (i, p) in parts.iter().enumerate() {
        out[i] = p.parse::<u8>().map_err(|_| ParseXmlError::InvalidValue {
            element: "node".into(),
            value: s.to_string(),
            expected: "IPv4 octet 0..255",
        })?;
    }
    Ok(Some(IpV4(out)))
}

fn parse_ipv6(s: &str) -> Result<Option<IpV6>, ParseXmlError> {
    // Vereinfacht: nur volle 8-Gruppen-Form (kein `::`-Compression).
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 8 {
        return Err(ParseXmlError::InvalidValue {
            element: "node".into(),
            value: s.to_string(),
            expected: "IPv6 address (a:b:c:d:e:f:g:h, no :: compression)",
        });
    }
    let mut out = [0u8; 16];
    for (i, p) in parts.iter().enumerate() {
        let v = u16::from_str_radix(p, 16).map_err(|_| ParseXmlError::InvalidValue {
            element: "node".into(),
            value: s.to_string(),
            expected: "IPv6 group 0..ffff",
        })?;
        out[i * 2] = (v >> 8) as u8;
        out[i * 2 + 1] = (v & 0xff) as u8;
    }
    Ok(Some(IpV6(out)))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<dds_tsn>
  <qos_library name="lib1">
    <qos_profile name="default">
      <domain_participant_qos name="dp1"></domain_participant_qos>
      <datawriter_qos name="dw1"></datawriter_qos>
    </qos_profile>
  </qos_library>
  <domain_library name="dlib">
    <domain name="DomainA">
      <registered_type name="Shape" type_ref="ShapeType"/>
      <topic topic_name="Square" type_name="Shape"/>
    </domain>
  </domain_library>
  <domain_participant_library name="dplib">
    <domain_participant name="DP1" domain_ref="dlib::DomainA"/>
  </domain_participant_library>
  <application_library name="alib">
    <application name="App1">
      <domain_participant_ref>dplib::DP1</domain_participant_ref>
    </application>
  </application_library>
  <node_library name="nlib">
    <node name="Edge1" hostname="edge1.local" mac_address="aa:bb:cc:dd:ee:ff" ipv4_address="10.0.0.1"/>
  </node_library>
  <deployment_library name="deploy">
    <deployment name="main" node_ref="nlib::Edge1">
      <application_ref>alib::App1</application_ref>
      <configuration>
        <tsn name="PerimeterTSN"/>
      </configuration>
    </deployment>
  </deployment_library>
</dds_tsn>"#;

    #[test]
    fn parses_full_sample_into_six_libraries() {
        let cfg = parse_dds_tsn_xml(SAMPLE).expect("parse ok");
        assert_eq!(cfg.qos_libraries.len(), 1);
        assert_eq!(cfg.domain_libraries.len(), 1);
        assert_eq!(cfg.domain_participant_libraries.len(), 1);
        assert_eq!(cfg.application_libraries.len(), 1);
        assert_eq!(cfg.node_libraries.len(), 1);
        assert_eq!(cfg.deployment_libraries.len(), 1);
    }

    #[test]
    fn qos_profile_carries_named_qos_categories() {
        let cfg = parse_dds_tsn_xml(SAMPLE).unwrap();
        let p = &cfg.qos_libraries[0].qos_profiles[0];
        assert_eq!(p.name, "default");
        assert_eq!(p.domain_participant_qos[0].name, "dp1");
        assert_eq!(p.datawriter_qos[0].name, "dw1");
    }

    #[test]
    fn domain_carries_registered_types_and_topics() {
        let cfg = parse_dds_tsn_xml(SAMPLE).unwrap();
        let d = &cfg.domain_libraries[0].domains[0];
        assert_eq!(d.registered_types[0].type_ref, "ShapeType");
        assert_eq!(d.topics[0].topic_name, "Square");
    }

    #[test]
    fn node_with_mac_and_ipv4_parses() {
        let cfg = parse_dds_tsn_xml(SAMPLE).unwrap();
        let n = &cfg.node_libraries[0].nodes[0];
        assert_eq!(n.mac_address.0, [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        assert_eq!(n.ipv4_address.unwrap().0, [10, 0, 0, 1]);
        assert!(n.ipv6_address.is_none());
    }

    #[test]
    fn deployment_with_tsn_configuration_ref() {
        let cfg = parse_dds_tsn_xml(SAMPLE).unwrap();
        let d = &cfg.deployment_libraries[0].deployments[0];
        assert_eq!(d.application_refs, alloc::vec!["alib::App1"]);
        assert_eq!(
            d.configuration.as_ref().unwrap().tsn_configuration_ref,
            Some("PerimeterTSN".into())
        );
    }

    #[test]
    fn missing_root_yields_unexpected_root_error() {
        let bad = r#"<?xml version="1.0"?><foo/>"#;
        let err = parse_dds_tsn_xml(bad).unwrap_err();
        assert!(matches!(err, ParseXmlError::UnexpectedRoot(s) if s == "foo"));
    }

    #[test]
    fn missing_required_attribute_is_diagnostic() {
        let bad = r#"<?xml version="1.0"?>
<dds_tsn>
  <qos_library>
    <qos_profile name="default"/>
  </qos_library>
</dds_tsn>"#;
        let err = parse_dds_tsn_xml(bad).unwrap_err();
        assert!(matches!(
            err,
            ParseXmlError::MissingAttribute { element, .. } if element == "qos_library"
        ));
    }

    #[test]
    fn invalid_mac_is_diagnostic() {
        let bad = r#"<?xml version="1.0"?>
<dds_tsn>
  <node_library name="nlib">
    <node name="Edge1" hostname="edge1.local" mac_address="not-a-mac"/>
  </node_library>
</dds_tsn>"#;
        let err = parse_dds_tsn_xml(bad).unwrap_err();
        assert!(matches!(err, ParseXmlError::InvalidValue { .. }));
    }

    #[test]
    fn parse_mac_accepts_dash_separator() {
        assert_eq!(
            parse_mac("aa-bb-cc-dd-ee-ff").unwrap().0,
            [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]
        );
    }

    #[test]
    fn parse_mac_rejects_short_input() {
        assert!(parse_mac("aa:bb:cc").is_none());
    }
}
