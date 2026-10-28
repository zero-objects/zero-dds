// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Conformance markers for DDS-XML 1.0 clause 2 and §7.2.
//!
//! Clause 2 (Conformance Criteria) has no conformance points of its own;
//! it says a reference to the specification "shall result in a selection of
//! building blocks where all selected building blocks shall be supported
//! entirely". [`SUPPORTED_BUILDING_BLOCKS`] lists the building blocks of
//! §7.3.1 this crate reads, with the module and the top-level element of
//! each. Unit tests check the list for duplicates and against the count
//! the specification names.
//!
//! §7.2 says the XML representation of resources that correspond to data
//! types of the DDS IDL PSM is obtained by a 1-to-1 mapping of the IDL type.
//! [`IDL_TO_XML_MAPPING`] names, per IDL type category, the subsection of
//! §7.1.4 or §7.2 that defines it and the function that reads it.

extern crate alloc;

/// The building blocks of DDS-XML 1.0 §7.3.1 this crate reads.
///
/// Spec §7.3.1: "This specification breaks the syntax used to
/// represent DDS resources in XML into the six different building
/// building blocks": QoS, Types, Domains, DomainParticipants,
/// Applications, Data Samples.
///
/// Per entry: `(spec_name, module_name, top_level_element)`.
pub const SUPPORTED_BUILDING_BLOCKS: &[(&str, &str, &str)] = &[
    ("QoS", "qos", "qos_library"),
    ("Types", "xtypes_def", "types"),
    ("Domains", "domain", "domain_library"),
    (
        "DomainParticipants",
        "participant",
        "domain_participant_library",
    ),
    ("Applications", "application", "application_library"),
    ("DataSamples", "sample", "data"),
];

/// 1-to-1 mapping IDL data type -> XML constructor + producing
/// API function. Spec §7.2.
///
/// Per entry: `(idl_category, spec_section, repo_path)`.
pub const IDL_TO_XML_MAPPING: &[(&str, &str, &str)] = &[
    (
        "boolean",
        "§7.1.4 Table 7.1",
        "types::parse_bool / qos_parser::parse_bool_strict",
    ),
    (
        "long (32-bit signed)",
        "§7.1.4 Table 7.1",
        "types::parse_long",
    ),
    (
        "unsigned long (32-bit)",
        "§7.1.4 Table 7.1",
        "types::parse_ulong",
    ),
    ("string", "§7.1.4 Table 7.1", "types::parse_string"),
    ("enum", "§7.1.4 Table 7.1 / §7.2.1", "types::parse_enum"),
    ("LENGTH_UNLIMITED", "§7.2.2.1", "types::LENGTH_UNLIMITED"),
    (
        "DURATION_INFINITE_SEC/NSEC",
        "§7.2.2.1",
        "types::DURATION_INFINITE_SEC / DURATION_INFINITE_NSEC",
    ),
    (
        "DURATION_ZERO_SEC/NSEC",
        "§7.2.2.1",
        "types::DURATION_ZERO_SEC / DURATION_ZERO_NSEC",
    ),
    (
        "TIME_INVALID_SEC/NSEC",
        "§7.2.2.1",
        "types::TIME_INVALID_SEC / TIME_INVALID_NSEC",
    ),
    (
        "nonNegativeInteger_UNLIMITED",
        "§7.2.2.1",
        "types::parse_long (number or LENGTH_UNLIMITED)",
    ),
    (
        "positiveInteger_UNLIMITED",
        "§7.2.2.1",
        "types::parse_positive_long_unlimited",
    ),
    (
        "nonNegativeInteger_Duration_SEC",
        "§7.2.2.1",
        "types::parse_duration_sec",
    ),
    (
        "nonNegativeInteger_Duration_NSEC",
        "§7.2.2.1",
        "types::parse_duration_nsec",
    ),
    (
        "struct (IDL)",
        "§7.2.3",
        "qos_parser::* (one reader per policy)",
    ),
    (
        "sequence<T> (IDL)",
        "§7.2.4.1",
        "parser::XmlElement::sequence_elements",
    ),
    (
        "sequence<octet> (IDL)",
        "§7.2.4.2",
        "types::parse_octet_sequence; qos_parser reads <value> and <valueB64>",
    ),
    (
        "T[N] (IDL Array)",
        "§7.2.5",
        "parser::XmlElement::sequence_elements (same as a sequence)",
    ),
    (
        "Duration_t",
        "§7.2.6",
        "qos_parser (duration elements; sec / nanosec)",
    ),
];

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    #[test]
    fn supported_blocks_match_spec_count() {
        // Spec §7.3.1 names six building blocks; the list has one entry
        // for each.
        assert_eq!(SUPPORTED_BUILDING_BLOCKS.len(), 6);
    }

    #[test]
    fn supported_blocks_have_unique_modules() {
        let mut seen = BTreeSet::new();
        for (name, module, _root) in SUPPORTED_BUILDING_BLOCKS {
            assert!(
                seen.insert(*module),
                "module `{module}` registered twice for block `{name}`"
            );
        }
    }

    #[test]
    fn supported_blocks_have_unique_root_elements() {
        let mut seen = BTreeSet::new();
        for (name, _module, root) in SUPPORTED_BUILDING_BLOCKS {
            assert!(
                seen.insert(*root),
                "top-level element `{root}` duplicated for block `{name}`"
            );
        }
    }

    #[test]
    fn idl_mapping_covers_required_categories() {
        // Sanity: the mapping table contains at least all categories
        // from §7.1.4 Table 7.1 (boolean, enum, long, ulong, string) and
        // Duration_t (§7.2.6).
        let names: BTreeSet<&str> = IDL_TO_XML_MAPPING
            .iter()
            .map(|(name, _, _)| *name)
            .collect();
        for required in [
            "boolean",
            "long (32-bit signed)",
            "unsigned long (32-bit)",
            "string",
            "enum",
            "Duration_t",
        ] {
            assert!(
                names.contains(required),
                "mapping table missing entry for `{required}`"
            );
        }
    }

    #[test]
    fn idl_mapping_entries_unique() {
        let mut seen = BTreeSet::new();
        for (name, _, _) in IDL_TO_XML_MAPPING {
            assert!(seen.insert(*name), "mapping entry `{name}` duplicated");
        }
    }

    #[test]
    fn idl_mapping_names_the_sections_of_7_2() {
        // The subsections of §7.2 that define a mapping appear in the table.
        let sections: BTreeSet<&str> = IDL_TO_XML_MAPPING.iter().map(|(_, sec, _)| *sec).collect();
        for required in [
            "§7.2.2.1",
            "§7.2.3",
            "§7.2.4.1",
            "§7.2.4.2",
            "§7.2.5",
            "§7.2.6",
        ] {
            assert!(
                sections.contains(required),
                "mapping table missing § section `{required}`"
            );
        }
    }
}
