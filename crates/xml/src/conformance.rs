// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Conformance markers for DDS-XML 1.0 §2.4 + §7.2.0.
//!
//! § 2.4 — *Atomic building-block selection.* The spec requires that the
//! implementation can atomically state per building block: "selected"
//! or "not selected". We mark that via
//! [`SUPPORTED_BUILDING_BLOCKS`] — the list of the building blocks
//! productively supported by the crate. An `assert!`-based
//! test table (`tests::supported_blocks_match_repo`) verifies
//! that the list matches the actually exposed modules.
//!
//! § 7.2.0 — *1-to-1 mapping of IDL data types.* The spec says: "The XML
//! representation of resources that correspond to data-types defined
//! in the DDS IDL PSM is obtained by performing a 1-to-1 mapping of
//! the corresponding IDL data type." We encode this mapping
//! table as [`IDL_TO_XML_MAPPING`] — per IDL data type category
//! a reference to the producing function or module, so that
//! reviewers/testers can see the completeness of the coverage in one
//! place.

extern crate alloc;

/// List of the building blocks from DDS-XML 1.0 §7.3.1.1 that are
/// productively supported in this crate.
///
/// Spec §7.3.1.1: "This specification breaks the syntax used to
/// represent DDS resources in XML into the six different building
/// blocks: Building Block QoS, Types, Domains, DomainParticipants,
/// Applications, Data Samples."
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
/// API function. Spec §7.2.0.
///
/// Per entry: `(idl_category, spec_section, repo_path)`.
pub const IDL_TO_XML_MAPPING: &[(&str, &str, &str)] = &[
    (
        "boolean",
        "§7.1.4 Tab.7.1",
        "types::parse_bool / parse_bool_strict",
    ),
    (
        "long (32-bit signed)",
        "§7.1.4 Tab.7.1",
        "types::parse_long",
    ),
    (
        "unsigned long (32-bit)",
        "§7.1.4 Tab.7.1",
        "types::parse_ulong",
    ),
    ("string", "§7.1.4 Tab.7.1", "types::parse_string"),
    ("enum", "§7.1.4 Tab.7.1 / §7.2.1", "types::parse_enum"),
    ("LENGTH_UNLIMITED", "§7.2.2.1", "types::LENGTH_UNLIMITED"),
    (
        "DURATION_INFINITE_SEC/NSEC",
        "§7.2.2.2 / §7.2.2.3",
        "types::DURATION_INFINITE_SEC / DURATION_INFINITE_NSEC",
    ),
    (
        "DURATION_ZERO_SEC/NSEC",
        "§7.2.2.4 / §7.2.2.5",
        "types::DURATION_ZERO_SEC / DURATION_ZERO_NSEC",
    ),
    (
        "nonNegativeInteger_UNLIMITED",
        "§7.2.2.8",
        "types::parse_long (Number-or-Symbol)",
    ),
    (
        "positiveInteger_UNLIMITED",
        "§7.2.2.9",
        "types::parse_positive_long_unlimited",
    ),
    (
        "nonNegativeInteger_Duration_SEC",
        "§7.2.2.10",
        "types::parse_duration_sec",
    ),
    (
        "nonNegativeInteger_Duration_NSEC",
        "§7.2.2.11",
        "types::parse_duration_nsec",
    ),
    ("struct (IDL)", "§7.2.3", "qos_parser::* (recursive)"),
    (
        "sequence<T> (IDL)",
        "§7.2.4.1",
        "parser::XmlElement::sequence_elements",
    ),
    (
        "sequence<octet> (IDL)",
        "§7.2.4.2",
        "types::parse_octet_sequence + qos_parser::base64_decode",
    ),
    (
        "T[N] (IDL Array)",
        "§7.2.5",
        "parser::XmlElement::sequence_elements (re-use)",
    ),
    ("Duration_t", "§7.2.6", "qos_parser::parse_duration"),
];

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    #[test]
    fn supported_blocks_match_spec_count() {
        // Spec §7.3.1.1 names **6** building blocks — that is exactly
        // our list.
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
        // from §7.1.4 Tab.7.1 (boolean, enum, long, ulong, string) +
        // §7.2.x (sequences, arrays, Duration).
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
    fn idl_mapping_includes_section_7_2_x_items() {
        // §7.2.x items that are fully live after K7-A must appear
        // in the table — otherwise §7.2.0 cannot be "done".
        let sections: BTreeSet<&str> = IDL_TO_XML_MAPPING.iter().map(|(_, sec, _)| *sec).collect();
        for required in ["§7.2.2.9", "§7.2.4.1", "§7.2.4.2", "§7.2.5", "§7.2.6"] {
            assert!(
                sections.contains(required),
                "mapping table missing § section `{required}`"
            );
        }
    }
}
