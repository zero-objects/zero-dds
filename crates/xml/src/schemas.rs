// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Embedded normative XSD schema files for DDS-XML 1.0 §7.1.2 + §8.1.2.
//!
//! Per building block the spec provides two XSD files:
//! `dds-xml_<bb>_definitions_nonamespace.xsd` (chameleon, without
//! targetNamespace) and `dds-xml_<bb>_definitions.xsd` (with
//! `targetNamespace="http://www.omg.org/spec/DDS-XML"`).
//!
//! We embed the schemas via `include_str!`, so that the crate
//! is self-contained — the build needs no external
//! schema files.
//!
//! # Building-block list (Spec §7.3.1.1 + §8.1)
//!
//! | Block              | Top-Level Element             |
//! |--------------------|-------------------------------|
//! | QoS                | `<qos_library>`               |
//! | Types              | `<types>`                     |
//! | Domains            | `<domain_library>`            |
//! | DomainParticipants | `<domain_participant_library>`|
//! | Applications       | `<application_library>`       |
//! | Data Samples       | `<data>`                      |
//! | DDS System         | `<dds>`                       |

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// Common datatypes (chameleon, included by all building-block
/// schemas). Spec §7.2.2 + §7.1.4.
pub const COMMON_XSD: &str = include_str!("../schemas/dds-xml_common.xsd");

/// QoS Building Block — chameleon XSD without targetNamespace.
pub const QOS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_qos_definitions_nonamespace.xsd");

/// QoS Building Block — XSD with targetNamespace + top-level
/// `<qos_library>`.
pub const QOS_NAMESPACED_XSD: &str = include_str!("../schemas/dds-xml_qos_definitions.xsd");

/// Types Building Block — chameleon.
pub const TYPES_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_types_definitions_nonamespace.xsd");

/// Types Building Block — Namespaced.
pub const TYPES_NAMESPACED_XSD: &str = include_str!("../schemas/dds-xml_types_definitions.xsd");

/// Domains Building Block — chameleon.
pub const DOMAINS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_domains_definitions_nonamespace.xsd");

/// Domains Building Block — Namespaced.
pub const DOMAINS_NAMESPACED_XSD: &str = include_str!("../schemas/dds-xml_domains_definitions.xsd");

/// DomainParticipants Building Block — chameleon.
pub const DOMAIN_PARTICIPANTS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_domain_participants_definitions_nonamespace.xsd");

/// DomainParticipants Building Block — Namespaced.
pub const DOMAIN_PARTICIPANTS_NAMESPACED_XSD: &str =
    include_str!("../schemas/dds-xml_domain_participants_definitions.xsd");

/// Applications Building Block — chameleon.
pub const APPLICATIONS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_applications_definitions_nonamespace.xsd");

/// Applications Building Block — Namespaced.
pub const APPLICATIONS_NAMESPACED_XSD: &str =
    include_str!("../schemas/dds-xml_applications_definitions.xsd");

/// Data Samples Building Block — chameleon.
pub const DATA_SAMPLES_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_data_samples_definitions_nonamespace.xsd");

/// Data Samples Building Block — Namespaced.
pub const DATA_SAMPLES_NAMESPACED_XSD: &str =
    include_str!("../schemas/dds-xml_data_samples_definitions.xsd");

/// DDS System Block Set — Chameleon.
pub const DDS_SYSTEM_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_dds_system_definitions_nonamespace.xsd");

/// DDS System Block Set — Namespaced (Top-Level `<dds>`).
pub const DDS_SYSTEM_NAMESPACED_XSD: &str =
    include_str!("../schemas/dds-xml_dds_system_definitions.xsd");

/// Pro Building-Block: `(name, nonamespace_xsd, namespaced_xsd,
/// top_level_element)`.
pub const ALL_SCHEMAS: &[(&str, &str, &str, &str)] = &[
    (
        "QoS",
        QOS_NONAMESPACE_XSD,
        QOS_NAMESPACED_XSD,
        "qos_library",
    ),
    (
        "Types",
        TYPES_NONAMESPACE_XSD,
        TYPES_NAMESPACED_XSD,
        "types",
    ),
    (
        "Domains",
        DOMAINS_NONAMESPACE_XSD,
        DOMAINS_NAMESPACED_XSD,
        "domain_library",
    ),
    (
        "DomainParticipants",
        DOMAIN_PARTICIPANTS_NONAMESPACE_XSD,
        DOMAIN_PARTICIPANTS_NAMESPACED_XSD,
        "domain_participant_library",
    ),
    (
        "Applications",
        APPLICATIONS_NONAMESPACE_XSD,
        APPLICATIONS_NAMESPACED_XSD,
        "application_library",
    ),
    (
        "DataSamples",
        DATA_SAMPLES_NONAMESPACE_XSD,
        DATA_SAMPLES_NAMESPACED_XSD,
        "data",
    ),
    (
        "DDSSystem",
        DDS_SYSTEM_NONAMESPACE_XSD,
        DDS_SYSTEM_NAMESPACED_XSD,
        "dds",
    ),
];

/// Returns the list of all building-block names for which an
/// XSD file pair is embedded.
#[must_use]
pub fn embedded_block_names() -> Vec<String> {
    ALL_SCHEMAS
        .iter()
        .map(|(name, _, _, _)| String::from(*name))
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn common_xsd_starts_with_xml_declaration() {
        assert!(COMMON_XSD.starts_with("<?xml"));
        assert!(COMMON_XSD.contains("xs:schema"));
    }

    #[test]
    fn nonamespace_xsds_omit_target_namespace() {
        // Spec §7.3.1.2 — a chameleon XSD may not carry a
        // `targetNamespace="..."` attribute on the <xs:schema> tag.
        // We parse the XML and check the root element.
        for (name, nons, _ns, _root) in ALL_SCHEMAS {
            let doc = roxmltree::Document::parse(nons)
                .unwrap_or_else(|e| panic!("block `{name}` chameleon XSD parse: {e}"));
            assert!(
                doc.root_element().attribute("targetNamespace").is_none(),
                "block `{name}` chameleon XSD <xs:schema> has a targetNamespace attribute"
            );
        }
    }

    #[test]
    fn namespaced_xsds_include_target_namespace() {
        // Spec §7.3.1.3 — the namespaced XSD sets
        // targetNamespace="http://www.omg.org/spec/DDS-XML".
        for (name, _nons, ns, _root) in ALL_SCHEMAS {
            let doc = roxmltree::Document::parse(ns)
                .unwrap_or_else(|e| panic!("block `{name}` namespaced XSD parse: {e}"));
            assert_eq!(
                doc.root_element().attribute("targetNamespace"),
                Some("http://www.omg.org/spec/DDS-XML"),
                "block `{name}` namespaced XSD <xs:schema> without the correct targetNamespace"
            );
        }
    }

    #[test]
    fn namespaced_xsds_define_top_level_element() {
        // Per block the namespaced XSD must contain an <xs:element name="..."> with
        // the expected top-level tag.
        for (name, _nons, ns, root) in ALL_SCHEMAS {
            let needle = alloc::format!("name=\"{root}\"");
            assert!(
                ns.contains(&needle),
                "block `{name}` namespaced XSD defines no xs:element {root}"
            );
        }
    }

    #[test]
    fn all_schemas_includes_six_building_blocks_plus_system() {
        // Spec §7.3.1.1: 6 building blocks. §8.1: 1 additional
        // system block set.
        assert_eq!(ALL_SCHEMAS.len(), 7);
        let names = embedded_block_names();
        for required in [
            "QoS",
            "Types",
            "Domains",
            "DomainParticipants",
            "Applications",
            "DataSamples",
            "DDSSystem",
        ] {
            assert!(names.iter().any(|n| n == required), "missing: {required}");
        }
    }

    #[test]
    fn nonamespace_xsds_can_be_loaded_by_xsd_loader() {
        // Sanity: each embedded chameleon XSD is syntactically
        // well-formed XML — passes through the existing xsd_loader
        // path without error.
        for (name, nons, _ns, _root) in ALL_SCHEMAS {
            let _ = roxmltree::Document::parse(nons)
                .unwrap_or_else(|e| panic!("Block `{name}` chameleon-XSD parsefehler: {e}"));
        }
    }

    #[test]
    fn namespaced_xsds_can_be_loaded_by_xsd_loader() {
        for (name, _nons, ns, _root) in ALL_SCHEMAS {
            let _ = roxmltree::Document::parse(ns)
                .unwrap_or_else(|e| panic!("Block `{name}` namespaced-XSD parsefehler: {e}"));
        }
    }
}
