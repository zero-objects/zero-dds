// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-zenoh-bridge` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`:
//! - `src/runtime.rs` (feature `zenoh-runtime`): `qos::ExtensibilityForRepr`
//!   with its three variants, mapped one to one from the route's
//!   extensibility and handed to the DCPS writer.
//! - `src/daemon/server.rs` (feature `daemon`): `TypeIdentifier`; only an
//!   `EquivalenceHashComplete` is looked up and waited for, every other kind
//!   is left unresolved. The lookup is `resolve::TypeRegistry::get_complete`
//!   for the root and for every hash `TypeRegistry::transitive_dependencies`
//!   returns under `resolve::DEFAULT_MAX_RESOLVE_NODES`; one missing object
//!   leaves the type unresolved.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.1.2.1.8 (FINAL, APPENDABLE, MUTABLE),
//! §7.3.4.2 Table 29 (`EK_COMPLETE = 0xF2` identifies a type by the hash of
//! its complete TypeObject), Annex B ("First 14 bytes of MD5 of the
//! serialized TypeObject using XCDR version 2 with Little Endian
//! encoding"), the COMPLETE octets and hash CycloneDDS 11.0.1 and RTI Connext
//! 7.7.0 give for `module to { @final struct Inner { long x; long y; }; }`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`), and the octets of
//! `@final struct to::Outer { to::Inner i; }` written out by hand from
//! Annex B and §7.4.2; their MD5 was computed outside this tree.
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::qos::ExtensibilityForRepr;
use zerodds_types::resolve::{DEFAULT_MAX_RESOLVE_NODES, TypeRegistry};
use zerodds_types::type_identifier::kinds::{
    EK_COMPLETE, EK_MINIMAL, TI_STRONGLY_CONNECTED_COMPONENT,
};
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, StronglyConnectedComponentId, TypeIdentifier};

/// CycloneDDS's COMPLETE `TypeObject` of `to::Inner`.
const INNER_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a496e6e65720000002c0000000200000010000000000000000100040002000000780000001000000001000000010004000200000079000000";
/// Its COMPLETE hash, as CycloneDDS and RTI Connext report it.
const INNER_HASH: [u8; 14] = [
    0xf0, 0x03, 0x88, 0xe0, 0xe0, 0xc6, 0x61, 0x9e, 0x41, 0x75, 0xc6, 0x8e, 0x8a, 0xf8,
];

/// `@final struct to::Outer { to::Inner i; }`, COMPLETE, by hand:
/// DHEADER 72, `EK_COMPLETE`, `TK_STRUCTURE`, `IS_FINAL`; the header (a
/// DHEADER of 18, `TK_NONE` base, two absent optionals, padding, the name
/// "to::Outer"); two octets of padding; the member sequence (DHEADER 40,
/// count 1) holding one member (DHEADER 32: id 0, flags `TRY_CONSTRUCT1`,
/// the identifier `EK_COMPLETE` + `INNER_HASH`, padding, the name "i", two
/// absent optionals).
const OUTER_COMPLETE: &str = "48000000f251010012000000000000000a000000746f3a3a4f75746572000000280000000100000020000000000000000100f2f00388e0e0c6619e4175c68e8af80000000200000069000000";
/// MD5 of those 76 octets, first 14.
const OUTER_HASH: [u8; 14] = [
    0x91, 0x23, 0x71, 0x8c, 0xb7, 0x2c, 0x3e, 0x15, 0xbd, 0xec, 0x18, 0x80, 0x34, 0xf7,
];

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn xtypes_7_3_1_2_1_8_the_three_extensibility_kinds_map_one_to_one() {
    // Exhaustive on purpose: a fourth kind fails to compile here, where the
    // bridge's mapping from its route extensibility would miss it.
    let name = |kind: ExtensibilityForRepr| match kind {
        ExtensibilityForRepr::Final => "FINAL",
        ExtensibilityForRepr::Appendable => "APPENDABLE",
        ExtensibilityForRepr::Mutable => "MUTABLE",
    };
    assert_eq!(name(ExtensibilityForRepr::Final), "FINAL");
    assert_eq!(name(ExtensibilityForRepr::Appendable), "APPENDABLE");
    assert_eq!(name(ExtensibilityForRepr::Mutable), "MUTABLE");
    assert_ne!(
        ExtensibilityForRepr::Final,
        ExtensibilityForRepr::Appendable
    );
    assert_ne!(
        ExtensibilityForRepr::Appendable,
        ExtensibilityForRepr::Mutable
    );
}

#[test]
fn xtypes_7_3_4_2_only_ek_complete_names_a_complete_type_object_by_its_hash() {
    let hash = EquivalenceHash(INNER_HASH);
    let complete = TypeIdentifier::EquivalenceHashComplete(hash);
    assert_eq!(EK_COMPLETE, 0xF2);
    assert_eq!(complete.discriminator(), 0xF2);
    let mut wire = vec![0xF2];
    wire.extend_from_slice(&INNER_HASH);
    assert_eq!(complete.to_bytes_le().unwrap(), wire);
    assert_eq!(TypeIdentifier::from_bytes_le(&wire).unwrap(), complete);

    // The kinds the bridge does not ask a peer for.
    let minimal = TypeIdentifier::EquivalenceHashMinimal(hash);
    assert_eq!(minimal.discriminator(), EK_MINIMAL);
    assert_eq!(EK_MINIMAL, 0xF1);
    let scc = TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
        equivalence_kind: EK_COMPLETE,
        hash,
        scc_length: 2,
        scc_index: 1,
    });
    assert_eq!(scc.discriminator(), TI_STRONGLY_CONNECTED_COMPONENT);
    assert_eq!(TI_STRONGLY_CONNECTED_COMPONENT, 0xB0);
    for other in [minimal, scc, TypeIdentifier::None] {
        assert!(!matches!(other, TypeIdentifier::EquivalenceHashComplete(_)));
    }
}

#[test]
fn annex_b_the_registry_files_a_complete_object_under_its_md5_hash() {
    let mut registry = TypeRegistry::new();
    let object = TypeObject::from_bytes_le(&unhex(INNER_COMPLETE)).unwrap();
    let hash = registry.insert_object(object.clone()).unwrap();
    assert_eq!(
        hash,
        EquivalenceHash(INNER_HASH),
        "CycloneDDS's and RTI's hash"
    );
    let TypeObject::Complete(complete) = object else {
        panic!("the octets open with EK_COMPLETE");
    };
    assert_eq!(registry.get_complete(&hash), Some(&complete));
    // Not filed under another hash.
    assert_eq!(registry.get_complete(&EquivalenceHash::ZERO), None);
}

#[test]
fn xtypes_7_3_4_6_4_the_transitive_dependencies_are_the_hashes_a_type_names() {
    let mut registry = TypeRegistry::new();
    let outer = registry
        .insert_object(TypeObject::from_bytes_le(&unhex(OUTER_COMPLETE)).unwrap())
        .unwrap();
    assert_eq!(outer, EquivalenceHash(OUTER_HASH));

    // The root is not among its dependencies; the member type is, by its
    // complete hash, whether or not the registry holds it yet.
    let before = registry.transitive_dependencies(&outer, DEFAULT_MAX_RESOLVE_NODES);
    assert_eq!(before, [EquivalenceHash(INNER_HASH)]);
    // Not held yet: the bridge leaves the type unresolved.
    assert!(registry.get_complete(&before[0]).is_none());

    let inner = registry
        .insert_object(TypeObject::from_bytes_le(&unhex(INNER_COMPLETE)).unwrap())
        .unwrap();
    let after = registry.transitive_dependencies(&outer, DEFAULT_MAX_RESOLVE_NODES);
    assert_eq!(after, [inner]);
    assert!(registry.get_complete(&after[0]).is_some());
    // A leaf has none.
    assert!(
        registry
            .transitive_dependencies(&inner, DEFAULT_MAX_RESOLVE_NODES)
            .is_empty()
    );
    // An unknown hash has none either; the bridge then fails on the root.
    assert!(
        registry
            .transitive_dependencies(&EquivalenceHash::ZERO, DEFAULT_MAX_RESOLVE_NODES)
            .is_empty()
    );
}
