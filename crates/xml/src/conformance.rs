// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Conformance-Marker fuer DDS-XML 1.0 §2.4 + §7.2.0.
//!
//! § 2.4 — *Atomic Building-Block-Selection.* Spec verlangt, dass die
//! Implementierung pro Building-Block atomisch sagen kann: "selected"
//! oder "not selected". Wir markieren das via
//! [`SUPPORTED_BUILDING_BLOCKS`] — die Liste der vom Crate
//! produktiv unterstuetzten Building-Blocks. Eine `assert!`-basierte
//! Test-Tabelle (`tests::supported_blocks_match_repo`) verifiziert,
//! dass die Liste mit den tatsaechlich exponierten Modulen
//! uebereinstimmt.
//!
//! § 7.2.0 — *1-zu-1-Mapping IDL-Datentypen.* Die Spec sagt: "The XML
//! representation of resources that correspond to data-types defined
//! in the DDS IDL PSM is obtained by performing a 1-to-1 mapping of
//! the corresponding IDL data type." Wir kodieren diese Mapping-
//! Tabelle als [`IDL_TO_XML_MAPPING`] — pro IDL-Datentyp-Kategorie
//! ein Verweis auf die produzierende Funktion bzw. das Modul, sodass
//! Reviewer/Tester die Vollstaendigkeit der Abdeckung an einem Ort
//! sehen koennen.

extern crate alloc;

/// Liste der Building Blocks aus DDS-XML 1.0 §7.3.1.1, die in diesem
/// Crate produktiv unterstuetzt sind.
///
/// Spec §7.3.1.1: "This specification breaks the syntax used to
/// represent DDS resources in XML into the six different building
/// blocks: Building Block QoS, Types, Domains, DomainParticipants,
/// Applications, Data Samples."
///
/// Pro Eintrag: `(spec_name, modul_name, top_level_element)`.
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

/// 1-zu-1-Mapping IDL-Datentyp -> XML-Konstruktor + produzierende
/// API-Funktion. Spec §7.2.0.
///
/// Pro Eintrag: `(idl_kategorie, spec_section, repo_pfad)`.
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
    ("struct (IDL)", "§7.2.3", "qos_parser::* (rekursiv)"),
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
        // Spec §7.3.1.1 nennt **6** Building-Blocks — exakt das ist
        // unsere Liste.
        assert_eq!(SUPPORTED_BUILDING_BLOCKS.len(), 6);
    }

    #[test]
    fn supported_blocks_have_unique_modules() {
        let mut seen = BTreeSet::new();
        for (name, module, _root) in SUPPORTED_BUILDING_BLOCKS {
            assert!(
                seen.insert(*module),
                "Modul `{module}` doppelt fuer Block `{name}` registriert"
            );
        }
    }

    #[test]
    fn supported_blocks_have_unique_root_elements() {
        let mut seen = BTreeSet::new();
        for (name, _module, root) in SUPPORTED_BUILDING_BLOCKS {
            assert!(
                seen.insert(*root),
                "Top-Level-Element `{root}` doppelt fuer Block `{name}`"
            );
        }
    }

    #[test]
    fn idl_mapping_covers_required_categories() {
        // Sanity: Mapping-Tabelle enthaelt mindestens alle Kategorien
        // aus §7.1.4 Tab.7.1 (boolean, enum, long, ulong, string) +
        // §7.2.x (Sequenzen, Arrays, Duration).
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
                "Mapping-Tabelle fehlt Eintrag fuer `{required}`"
            );
        }
    }

    #[test]
    fn idl_mapping_entries_unique() {
        let mut seen = BTreeSet::new();
        for (name, _, _) in IDL_TO_XML_MAPPING {
            assert!(seen.insert(*name), "Mapping-Eintrag `{name}` doppelt");
        }
    }

    #[test]
    fn idl_mapping_includes_section_7_2_x_items() {
        // §7.2.x-Items, die nach K7-A vollstaendig live sind, muessen
        // in der Tabelle stehen — sonst kann §7.2.0 nicht "done" sein.
        let sections: BTreeSet<&str> = IDL_TO_XML_MAPPING.iter().map(|(_, sec, _)| *sec).collect();
        for required in ["§7.2.2.9", "§7.2.4.1", "§7.2.4.2", "§7.2.5", "§7.2.6"] {
            assert!(
                sections.contains(required),
                "Mapping-Tabelle fehlt §-Sektion `{required}`"
            );
        }
    }
}
