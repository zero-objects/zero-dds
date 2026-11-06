// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-xtypes-test-main` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`:
//! - `src/model.rs`: `from_ast::complete_type_object` for the run's type and
//!   every other type of the file, wrapped in `TypeObject::Complete`, and
//!   `from_ast::type_identifier(.., Form::Complete)`, the identity the run
//!   announces (`carrier.rs` hands both out, or `TypeIdentifier::None` and no
//!   object under `--disable-type-info`).
//! - `src/main.rs` (`--print-typeid`, compared between products for the same
//!   type file): `from_ast::minimal_type_object_with` and
//!   `hash::compute_minimal_hash_with` with the camp flag, and
//!   `hash::compute_complete_hash`.
//! - `src/app.rs`: `qos::TypeConsistencyKind` and
//!   `qos::TypeConsistencyEnforcement::default()`, the policy default a `d`
//!   flag selects.
//! - `src/cli.rs`: `assignability::KeyMemberBounds` (default and `parse`)
//!   and `assignability::StrongAssignability::parse` for
//!   `--key-member-bounds` and `--strong-assignability`.
//!
//! The model is composed here with `zerodds-idl-compose`, as the run
//! composes an IDL type file.
//!
//! Oracle: DDS-XTypes 1.3 Annex B ("First 14 bytes of MD5 of the serialized
//! TypeObject") with the MINIMAL hashes CycloneDDS 11.0.1 and Fast DDS 3.6.1
//! agree on and the COMPLETE hashes CycloneDDS and RTI Connext 7.7.0 report
//! for the corpus below (`crates/types/tests/typeobject_vendor_dheader.rs`);
//! §7.6.3.4.1 (the policy defaults); the spellings this crate's
//! `--help` and error messages promise ("enforced or ignored", "equivalence
//! or names_ignored").
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::assignability::{KeyMemberBounds, StrongAssignability};
use zerodds_types::from_ast::{
    Form, complete_type_object, minimal_type_object_with, type_identifier,
};
use zerodds_types::hash::{compute_complete_hash, compute_minimal_hash_with};
use zerodds_types::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, TypeIdentifier};

const CORPUS: &str = "\
module to {
  @final
  struct Plain {
    long  a;
    short b;
  };
  @appendable
  struct Appendable {
    long  a;
    short b;
  };
  @mutable
  struct Mutable {
    @id(10) long  a;
    @id(20) short b;
  };
  @final
  struct Inner {
    long  x;
    long  y;
  };
};";

/// Name, MINIMAL hash (Cyclone and Fast DDS), COMPLETE hash (Cyclone and RTI).
const HASHES: [(&str, &str, &str); 4] = [
    (
        "Plain",
        "deb922f040f7c228b25728be51f6",
        "7c0c6fab7d066a0c56b8e93c0ea7",
    ),
    (
        "Appendable",
        "3c9161318d323a5141b44ee3e2b6",
        "27cb33d5ad3f7388a5627a1b48fd",
    ),
    (
        "Mutable",
        "81e76ce14253653ea25be4145831",
        "447863b1f0043ce0e42323a74866",
    ),
    (
        "Inner",
        "0c380bda28dab0250db24ae23c16",
        "f00388e0e0c6619e4175c68e8af8",
    ),
];

fn hash(hex: &str) -> EquivalenceHash {
    let mut out = [0u8; 14];
    for (i, octet) in out.iter_mut().enumerate() {
        *octet = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    EquivalenceHash(out)
}

#[test]
fn annex_b_the_announced_identifier_and_the_printed_hashes_are_the_vendors() {
    let composed = zerodds_idl_compose::compose_source(
        "corpus.idl",
        CORPUS,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed(), "the corpus composes without an error");
    let ast = &composed.ast;
    for (name, minimal, complete) in HASHES {
        let id = ast
            .types()
            .find(|(_, node)| node.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no type `{name}`"));

        // `model.rs`: the object and the identity a run announces.
        let object = complete_type_object(ast, id).unwrap();
        assert_eq!(
            type_identifier(ast, id, Form::Complete).unwrap(),
            TypeIdentifier::EquivalenceHashComplete(hash(complete)),
            "{name}: announced identity"
        );
        // `main.rs --print-typeid`, the default camp.
        assert_eq!(
            compute_complete_hash(&object).unwrap(),
            hash(complete),
            "{name}: COMPLETE"
        );
        let minimal_object = minimal_type_object_with(ast, id, false).unwrap();
        assert_eq!(
            compute_minimal_hash_with(&minimal_object, false).unwrap(),
            hash(minimal),
            "{name}: MINIMAL"
        );
        // The object a run registers: Annex B's appendable union, a DHEADER
        // over the rest.
        let registered = TypeObject::Complete(object).to_bytes_le().unwrap();
        let dheader =
            u32::from_le_bytes([registered[0], registered[1], registered[2], registered[3]]);
        assert_eq!(dheader as usize, registered.len() - 4, "{name}");
    }
}

#[test]
fn xtypes_7_6_3_4_1_the_policy_default_a_d_flag_selects() {
    let d = TypeConsistencyEnforcement::default();
    assert_eq!(d.kind, TypeConsistencyKind::AllowTypeCoercion);
    assert!(d.ignore_sequence_bounds);
    assert!(d.ignore_string_bounds);
    assert!(!d.ignore_member_names);
    assert!(!d.prevent_type_widening);
    assert!(!d.force_type_validation);
    assert_ne!(
        TypeConsistencyKind::AllowTypeCoercion,
        TypeConsistencyKind::DisallowTypeCoercion
    );
}

#[test]
fn the_command_line_spellings_of_the_two_assignability_axes() {
    assert_eq!(KeyMemberBounds::default(), KeyMemberBounds::Enforced);
    assert_eq!(
        KeyMemberBounds::parse("enforced"),
        Some(KeyMemberBounds::Enforced)
    );
    assert_eq!(
        KeyMemberBounds::parse("ignored"),
        Some(KeyMemberBounds::Ignored)
    );
    assert_eq!(KeyMemberBounds::parse("lenient"), None);
    assert_eq!(
        StrongAssignability::parse("equivalence"),
        Some(StrongAssignability::Equivalence)
    );
    assert_eq!(
        StrongAssignability::parse("names_ignored"),
        Some(StrongAssignability::NamesIgnored)
    );
    assert_eq!(StrongAssignability::parse("names-ignored"), None);
}
