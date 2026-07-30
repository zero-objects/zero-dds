// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Broad-audit P0-5 — `@non_serialized` members are absent from BOTH the
//! Minimal AND the Complete TypeObject, and the survivors' member ids compact
//! (no gap) exactly as if the member were never declared (#2 (a): the changed
//! TypeIdentifier is the intended rc correction).
//!
//! Before the fix the Minimal path already dropped the member but the Complete
//! path (`build_complete_struct_type`) re-included it, so the two disagreed and
//! the emitted `TYPE_IDENTIFIER` still covered a member that never reaches the
//! wire.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_idl::ast::types::{Specification, StructDef};
use zerodds_idl::ast::{ConstrTypeDecl, Definition, StructDcl, TypeDecl};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::semantics::{
    build_complete_struct_type, build_type_registry, lower_struct_to_minimal,
};

/// `@final` so the members would take plain sequential ids 0,1,2 if `secret`
/// were counted — proving the compaction to `a=0, b=1`.
const SRC: &str = "\
@final
struct S {
    long a;
    @non_serialized long secret;
    long b;
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

#[test]
fn minimal_typeobject_omits_non_serialized_and_compacts_ids() {
    let spec = parse(SRC);
    let s = first_struct(&spec, "S");
    let minimal = lower_struct_to_minimal(&s).expect("minimal");

    // Exactly the two serialized members, in declaration order.
    assert_eq!(
        minimal.member_seq.len(),
        2,
        "Minimal TypeObject must carry only `a` and `b`, not `secret`"
    );
    let ids: Vec<u32> = minimal
        .member_seq
        .iter()
        .map(|m| m.common.member_id)
        .collect();
    // Compacted, gap-free: `a` = 0, `b` = 1 (NOT 0, 2).
    assert_eq!(
        ids,
        vec![0, 1],
        "sequential ids must compact over survivors"
    );
}

#[test]
fn complete_typeobject_omits_non_serialized_and_matches_minimal() {
    let spec = parse(SRC);
    let s = first_struct(&spec, "S");
    let lowered = build_type_registry(&spec).expect("registry");
    let complete = build_complete_struct_type(&s, &[], &lowered.names).expect("complete");

    let names: Vec<&str> = complete
        .member_seq
        .iter()
        .map(|m| m.detail.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["a", "b"],
        "Complete TypeObject must carry only `a` and `b`, not `secret`"
    );
    let ids: Vec<u32> = complete
        .member_seq
        .iter()
        .map(|m| m.common.member_id)
        .collect();
    assert_eq!(ids, vec![0, 1], "Complete ids must compact over survivors");

    // Minimal and Complete must agree on the member set (same count + ids), so
    // no member is on one TypeObject but not the other.
    let minimal = lower_struct_to_minimal(&s).expect("minimal");
    assert_eq!(
        minimal.member_seq.len(),
        complete.member_seq.len(),
        "Minimal and Complete must carry the identical member set"
    );
    let min_ids: Vec<u32> = minimal
        .member_seq
        .iter()
        .map(|m| m.common.member_id)
        .collect();
    assert_eq!(min_ids, ids, "Minimal and Complete member ids must match");
}
