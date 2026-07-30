//! Broad-audit P0-3 — cross-backend member-id parity for
//! `@autoid(HASH)` / `@hashid` / `@id`.
//!
//! The member ids are resolved ONCE in the semantic layer
//! (`zerodds_idl::semantics::member_id`). This test proves — through the REAL
//! emit paths, not by calling the hash function for the comparison — that the
//! ids the C++ backend puts on the wire (EMHEADER / PL_CDR1 `case` labels) are
//! identical to the ids idl-rust emits (`encode_member`) and to the ids the
//! TypeObject builder assigns (`lower_struct_to_minimal`). Before the fix the
//! C++ backend used a positional counter that ignored `@autoid(HASH)` and
//! `@hashid`, so the three drifted.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::collections::BTreeSet;

use zerodds_idl::ast::types::{Specification, StructDef};
use zerodds_idl::ast::{ConstrTypeDecl, Definition, StructDcl, TypeDecl};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::semantics::lower_struct_to_minimal;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};
use zerodds_types::type_object::common::NameHash;

/// `@autoid(HASH)` container + a `@hashid`-overridden member + an explicit
/// `@id(42)` member. `@mutable` so every member carries a wire id (EMHEADER /
/// PL_CDR1 parameter id) in all three emitters.
const SRC: &str = "\
@mutable @autoid(HASH)
struct P0_3 {
    long alpha;
    @hashid(\"altname\") long beta;
    @id(42) long gamma;
};
";

fn parse(src: &str) -> Specification {
    zerodds_idl::parse(src, &ParserConfig::default()).expect("parse")
}

fn first_struct(spec: &Specification, name: &str) -> StructDef {
    for d in &spec.definitions {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) = d {
            if s.name.text == name {
                return s.clone();
            }
        }
    }
    panic!("struct {name} not found");
}

/// Member ids from the C++ decode `case 0x..u:` labels (both the PL_CDR1 and the
/// PL_CDR2 switch carry the identical id set).
fn cpp_member_ids(cpp: &str) -> BTreeSet<u32> {
    let mut ids = BTreeSet::new();
    for (i, _) in cpp.match_indices("case 0x") {
        let rest = &cpp[i + "case 0x".len()..];
        let hex: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
        if let Ok(v) = u32::from_str_radix(&hex, 16) {
            ids.insert(v);
        }
    }
    ids
}

/// Member ids from idl-rust's `@mutable` encode calls (`enc.encode_member(<id>,`
/// / `enc.encode_member_lc(<id>,`).
fn rust_member_ids(rust: &str) -> BTreeSet<u32> {
    let mut ids = BTreeSet::new();
    for anchor in ["encode_member(", "encode_member_lc("] {
        for (i, _) in rust.match_indices(anchor) {
            let rest = &rust[i + anchor.len()..];
            let dec: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(v) = dec.parse::<u32>() {
                ids.insert(v);
            }
        }
    }
    ids
}

/// Member ids the TypeObject builder assigns (the canonical reference).
fn type_object_member_ids(s: &StructDef) -> BTreeSet<u32> {
    lower_struct_to_minimal(s)
        .expect("lower to minimal")
        .member_seq
        .iter()
        .map(|m| m.common.member_id)
        .collect()
}

#[test]
fn cpp_rust_typeobject_agree_on_autoid_hashid_id() {
    let spec = parse(SRC);
    let s = first_struct(&spec, "P0_3");

    let expected: BTreeSet<u32> = [
        NameHash::member_id_from_name("alpha"), // @autoid(HASH) over member name
        NameHash::member_id_from_name("altname"), // @hashid("altname") over the hint
        42,                                     // @id(42) explicit
    ]
    .into_iter()
    .collect();
    assert_eq!(
        expected.len(),
        3,
        "hash collision would invalidate the test"
    );

    let to_ids = type_object_member_ids(&s);
    assert_eq!(to_ids, expected, "TypeObject member ids != expected");

    let cpp = generate_cpp_header(&spec, &CppGenOptions::default()).expect("cpp gen");
    let cpp_ids = cpp_member_ids(&cpp);
    assert_eq!(
        cpp_ids, expected,
        "cpp emitted member ids drift from @autoid/@hashid/@id resolution"
    );

    let rust = generate_rust_module(&spec, &RustGenOptions::default()).expect("rust gen");
    let rust_ids = rust_member_ids(&rust);
    assert_eq!(rust_ids, expected, "rust emitted member ids drift");

    // The three emit paths carry byte-for-byte identical u32 member ids.
    assert_eq!(cpp_ids, to_ids);
    assert_eq!(cpp_ids, rust_ids);
}
