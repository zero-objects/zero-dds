// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Embedded XSD schema files of DDS-XML 1.0.
//!
//! For the QoS, Types, Domains, DomainParticipants and Applications
//! building blocks and the DDS system block set (§8.1) these are the
//! normative files of the specification
//! (`http://www.omg.org/spec/DDS-XML/20180501/`), byte for byte, with their
//! names (§7.3.2.3, §7.3.3.3, §7.3.4.3, §7.3.5.3, §7.3.6.3). They include
//! each other by that absolute URL; to validate offline map the prefix
//! onto the directory that holds them (an XML catalog with a `rewriteURI`
//! entry, which is what the tests do).
//!
//! The Data Samples files and `dds-xml_common.xsd` are not from the
//! specification: §7.3.7.3.1 says no generic XSD can describe samples, so
//! the specification ships none (the file headers say what they pin).
//!
//! Per building block there are two XSD files:
//! `dds-xml_<bb>_definitions_nonamespace.xsd` (chameleon, without
//! targetNamespace) and `dds-xml_<bb>_definitions.xsd` (with
//! `targetNamespace="http://www.omg.org/spec/DDS-XML"`).
//!
//! We embed the schemas via `include_str!`, so that the crate
//! is self-contained — the build needs no external
//! schema files.
//!
//! # Building-block list (Spec §7.3.1 + §8.1)
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
/// schemas). Spec §7.2.2.1 + §7.1.4.
pub const COMMON_XSD: &str = include_str!("../schemas/dds-xml_common.xsd");

/// QoS Building Block — chameleon XSD without targetNamespace.
pub const QOS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_qos_definitions_nonamespace.xsd");

/// QoS Building Block — XSD with targetNamespace + top-level
/// `<qos_library>`.
pub const QOS_NAMESPACED_XSD: &str = include_str!("../schemas/dds-xml_qos_definitions.xsd");

/// Types Building Block — chameleon.
pub const TYPES_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_type_definitions_nonamespace.xsd");

/// Types Building Block — Namespaced.
pub const TYPES_NAMESPACED_XSD: &str = include_str!("../schemas/dds-xml_type_definitions.xsd");

/// The namespace of the XML Type Representation of DDS-XTypes 1.3 (Annex A):
/// the `targetNamespace` of [`XTYPES_TYPES_XSD`].
pub const XTYPES_TYPES_NAMESPACE: &str = "http://www.omg.org/dds";

/// DDS-XTypes 1.3 Annex A, `dds-xtypes_type_definition.xsd`: the schema with
/// `targetNamespace="http://www.omg.org/dds"` and the root element `types`
/// of type `typeLibrary`. It includes [`XTYPES_TYPES_NONAMESPACE_XSD`] by the
/// absolute URL `https://www.omg.org/spec/DDS-XTypes/20190301/`.
pub const XTYPES_TYPES_XSD: &str = include_str!("../schemas/dds-xtypes_type_definition.xsd");

/// DDS-XTypes 1.3 Annex A, `dds-xtypes_type_definition_nonamespace.xsd`: the
/// type definitions, without a `targetNamespace` (chameleon).
pub const XTYPES_TYPES_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xtypes_type_definition_nonamespace.xsd");

/// Domains Building Block — chameleon.
pub const DOMAINS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_domain_definitions_nonamespace.xsd");

/// Domains Building Block — Namespaced.
pub const DOMAINS_NAMESPACED_XSD: &str = include_str!("../schemas/dds-xml_domain_definitions.xsd");

/// DomainParticipants Building Block — chameleon.
pub const DOMAIN_PARTICIPANTS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_domainparticipant_definitions_nonamespace.xsd");

/// DomainParticipants Building Block — Namespaced.
pub const DOMAIN_PARTICIPANTS_NAMESPACED_XSD: &str =
    include_str!("../schemas/dds-xml_domainparticipant_definitions.xsd");

/// Applications Building Block — chameleon.
pub const APPLICATIONS_NONAMESPACE_XSD: &str =
    include_str!("../schemas/dds-xml_application_definitions_nonamespace.xsd");

/// Applications Building Block — Namespaced.
pub const APPLICATIONS_NAMESPACED_XSD: &str =
    include_str!("../schemas/dds-xml_application_definitions.xsd");

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

/// Per building block: `(name, nonamespace_xsd, namespaced_xsd,
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
        // Spec §7.3.1 — the `_nonamespace` file "specifies neither a
        // targetNamespace nor a root element", so it carries no
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
        // Spec §7.3.1 — the namespaced XSD sets
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
        // Per block the namespaced file declares the top-level element
        // (§7.3.1), and the chameleon file declares none.
        for (name, nons, ns, root) in ALL_SCHEMAS {
            let top_level = |text: &str| -> Vec<String> {
                let doc = roxmltree::Document::parse(text).expect("XSD parse");
                doc.root_element()
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "element")
                    .filter_map(|n| n.attribute("name").map(alloc::string::ToString::to_string))
                    .collect()
            };
            assert!(
                top_level(ns).iter().any(|n| n == root),
                "block `{name}`: the namespaced XSD declares no xs:element {root}"
            );
            assert!(
                top_level(nons).is_empty(),
                "block `{name}`: the chameleon XSD declares a root element"
            );
        }
    }

    #[test]
    fn all_schemas_includes_six_building_blocks_plus_system() {
        // Spec §7.3.1: 6 building blocks. §8.1: 1 additional
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
                .unwrap_or_else(|e| panic!("block `{name}` chameleon XSD parse error: {e}"));
        }
    }

    #[test]
    fn namespaced_xsds_can_be_loaded_by_xsd_loader() {
        for (name, _nons, ns, _root) in ALL_SCHEMAS {
            let _ = roxmltree::Document::parse(ns)
                .unwrap_or_else(|e| panic!("block `{name}` namespaced XSD parse error: {e}"));
        }
    }

    /// Collects the `xs:include/@schemaLocation` values of a chameleon
    /// XSD that name another building block. `dds-xml_common.xsd` is the
    /// shared chameleon of §7.1.3, not a building block, so it does not
    /// count as a dependency.
    fn building_block_includes(xsd: &str) -> Vec<String> {
        const XSD_NS: &str = "http://www.w3.org/2001/XMLSchema";
        let doc = roxmltree::Document::parse(xsd).expect("XSD parse");
        let mut out: Vec<String> = doc
            .descendants()
            .filter(|n| n.has_tag_name((XSD_NS, "include")))
            .filter_map(|n| n.attribute("schemaLocation"))
            // The schemas of the specification include by absolute URL;
            // the file name is what names the block.
            .map(|loc| loc.rsplit('/').next().unwrap_or(loc))
            .filter(|loc| *loc != "dds-xml_common.xsd")
            .map(String::from)
            .collect();
        out.sort();
        out
    }

    /// DDS-XML 1.0 §7.3.2.2 / §7.3.3.2 / §7.3.7.2 — "This building block
    /// has no dependencies on other building blocks." A building block is
    /// "a consistent set of XML schemas" (§4), so the dependency is
    /// exactly what its chameleon XSD pulls in with `xs:include`. QoS,
    /// Types and Data Samples must include no other block's schema; the
    /// three blocks that do declare dependencies are pinned here too, so
    /// that a new include cannot slip into an independent block unnoticed.
    #[test]
    fn building_block_xsd_dependencies_match_spec() {
        let expected: &[(&str, &[&str])] = &[
            // §7.3.2.2 — no dependencies.
            ("QoS", &[]),
            // §7.3.3.2 — no dependencies.
            ("Types", &[]),
            // §7.3.7.2 — no dependencies.
            ("DataSamples", &[]),
            // §7.3.4.2 — Domains builds on Types and QoS.
            (
                "Domains",
                &[
                    "dds-xml_qos_definitions_nonamespace.xsd",
                    "dds-xml_type_definitions_nonamespace.xsd",
                ],
            ),
            // §7.3.5.2 — DomainParticipants builds on Domains.
            (
                "DomainParticipants",
                &["dds-xml_domain_definitions_nonamespace.xsd"],
            ),
            // §7.3.6.2 — Applications builds on DomainParticipants.
            (
                "Applications",
                &["dds-xml_domainparticipant_definitions_nonamespace.xsd"],
            ),
        ];
        for (block, want) in expected {
            let (_, nons, _, _) = ALL_SCHEMAS
                .iter()
                .find(|(name, _, _, _)| name == block)
                .unwrap_or_else(|| panic!("no embedded XSD for block `{block}`"));
            let got = building_block_includes(nons);
            assert_eq!(
                got, *want,
                "building block `{block}` declares the wrong XSD dependencies"
            );
        }
    }
    /// XTypes 1.3 Annex A: the namespaced file declares `types` of type
    /// `typeLibrary` in `http://www.omg.org/dds` and includes the chameleon
    /// file, which declares no root element.
    #[test]
    fn annex_a_schema_has_root_element_types_in_the_xtypes_namespace() {
        const XSD_NS: &str = "http://www.w3.org/2001/XMLSchema";
        let doc = roxmltree::Document::parse(XTYPES_TYPES_XSD).expect("XSD parse");
        let root = doc.root_element();
        assert_eq!(
            root.attribute("targetNamespace"),
            Some(XTYPES_TYPES_NAMESPACE)
        );
        let types: Vec<_> = root
            .children()
            .filter(|n| n.has_tag_name((XSD_NS, "element")))
            .collect();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].attribute("name"), Some("types"));
        assert_eq!(types[0].attribute("type"), Some("typeLibrary"));
        assert_eq!(
            building_block_includes(XTYPES_TYPES_XSD),
            ["dds-xtypes_type_definition_nonamespace.xsd"]
        );
        let chameleon = roxmltree::Document::parse(XTYPES_TYPES_NONAMESPACE_XSD).expect("parse");
        assert!(
            chameleon
                .root_element()
                .attribute("targetNamespace")
                .is_none()
        );
        assert!(
            !chameleon
                .root_element()
                .children()
                .any(|n| n.has_tag_name((XSD_NS, "element")))
        );
    }
}
