// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Embedded normative XSD-Schema-Files fuer DDS-XML 1.0 §7.1.2 + §8.1.2.
//!
//! Pro Building-Block liefert die Spec zwei XSD-Files:
//! `dds-xml_<bb>_definitions_nonamespace.xsd` (chameleon, ohne
//! targetNamespace) und `dds-xml_<bb>_definitions.xsd` (mit
//! `targetNamespace="http://www.omg.org/spec/DDS-XML"`).
//!
//! Wir embedden die Schemas via `include_str!`, sodass das Crate
//! self-contained ist — der Build benoetigt keine externen
//! Schema-Files.
//!
//! # Building-Block-Liste (Spec §7.3.1.1 + §8.1)
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

/// Common Datatypes (chameleon, von allen Building-Block-Schemas
/// includiert). Spec §7.2.2 + §7.1.4.
pub const COMMON_XSD: &str = include_str!("../schemas/dds-xml_common.xsd");

/// QoS Building Block — Chameleon-XSD ohne targetNamespace.
pub const QOS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_qos_definitions_nonamespace.xsd");

/// QoS Building Block — XSD mit targetNamespace + Top-Level
/// `<qos_library>`.
pub const QOS_NAMESPACED_XSD: &str = include_str!("../schemas/dds-xml_qos_definitions.xsd");

/// Types Building Block — Chameleon.
pub const TYPES_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_types_definitions_nonamespace.xsd");

/// Types Building Block — Namespaced.
pub const TYPES_NAMESPACED_XSD: &str = include_str!("../schemas/dds-xml_types_definitions.xsd");

/// Domains Building Block — Chameleon.
pub const DOMAINS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_domains_definitions_nonamespace.xsd");

/// Domains Building Block — Namespaced.
pub const DOMAINS_NAMESPACED_XSD: &str = include_str!("../schemas/dds-xml_domains_definitions.xsd");

/// DomainParticipants Building Block — Chameleon.
pub const DOMAIN_PARTICIPANTS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_domain_participants_definitions_nonamespace.xsd");

/// DomainParticipants Building Block — Namespaced.
pub const DOMAIN_PARTICIPANTS_NAMESPACED_XSD: &str =
    include_str!("../schemas/dds-xml_domain_participants_definitions.xsd");

/// Applications Building Block — Chameleon.
pub const APPLICATIONS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_applications_definitions_nonamespace.xsd");

/// Applications Building Block — Namespaced.
pub const APPLICATIONS_NAMESPACED_XSD: &str =
    include_str!("../schemas/dds-xml_applications_definitions.xsd");

/// Data Samples Building Block — Chameleon.
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

/// Liefert die Liste aller Building-Block-Namen, fuer die ein
/// XSD-File-Paerchen embedded ist.
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
        // Spec §7.3.1.2 — Chameleon-XSD darf kein
        // `targetNamespace="..."`-Attribut auf dem <xs:schema>-Tag
        // tragen. Wir parsen das XML und pruefen das root-Element.
        for (name, nons, _ns, _root) in ALL_SCHEMAS {
            let doc = roxmltree::Document::parse(nons)
                .unwrap_or_else(|e| panic!("Block `{name}` chameleon-XSD parse: {e}"));
            assert!(
                doc.root_element().attribute("targetNamespace").is_none(),
                "Block `{name}` chameleon-XSD <xs:schema> hat targetNamespace-Attribut"
            );
        }
    }

    #[test]
    fn namespaced_xsds_include_target_namespace() {
        // Spec §7.3.1.3 — namespaced-XSD setzt
        // targetNamespace="http://www.omg.org/spec/DDS-XML".
        for (name, _nons, ns, _root) in ALL_SCHEMAS {
            let doc = roxmltree::Document::parse(ns)
                .unwrap_or_else(|e| panic!("Block `{name}` namespaced-XSD parse: {e}"));
            assert_eq!(
                doc.root_element().attribute("targetNamespace"),
                Some("http://www.omg.org/spec/DDS-XML"),
                "Block `{name}` namespaced-XSD <xs:schema> ohne korrekten targetNamespace"
            );
        }
    }

    #[test]
    fn namespaced_xsds_define_top_level_element() {
        // Pro Block muss das namespaced-XSD ein <xs:element name="..."> mit
        // dem erwarteten Top-Level-Tag enthalten.
        for (name, _nons, ns, root) in ALL_SCHEMAS {
            let needle = alloc::format!("name=\"{root}\"");
            assert!(
                ns.contains(&needle),
                "Block `{name}` namespaced-XSD definiert kein xs:element {root}"
            );
        }
    }

    #[test]
    fn all_schemas_includes_six_building_blocks_plus_system() {
        // Spec §7.3.1.1: 6 Building-Blocks. §8.1: 1 zusaetzlicher
        // System-Block-Set.
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
            assert!(names.iter().any(|n| n == required), "fehlt: {required}");
        }
    }

    #[test]
    fn nonamespace_xsds_can_be_loaded_by_xsd_loader() {
        // Sanity: jedes embedded chameleon-XSD ist syntaktisch
        // wohlgeformtes XML — durchquert den existierenden xsd_loader-
        // Pfad ohne Fehler.
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
