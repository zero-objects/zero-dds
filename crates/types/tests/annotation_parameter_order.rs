// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The order of a `CompleteAnnotationParameterSeq`, DDS-XTypes 1.3 §7.3.4.5
//! against Annex B.
//!
//! §7.3.4.5 (S. 127): "The elements in CompleteAnnotationMemberSeq shall be
//! ordered in increasing values of the member_index." Annex B (S. 293), the
//! comment behind `CompleteAnnotationParameterSeq`: "Ordered by
//! CompleteAnnotationParameter.name". The normative text is §7.3.4.5; the
//! Annex B comment is the setting `AnnotationParameterOrder::Name`
//! (`xtypes.annotation_parameter_order: name`).
//!
//! The setting is process-wide, so one test function reads and sets it in
//! turn and leaves it at the default.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_types::from_ast::{Form, annotation_identifier, annotation_type_object};
use zerodds_types::type_object::complete::{
    AnnotationParameterOrder, CompleteTypeObject, annotation_parameter_order,
    set_annotation_parameter_order,
};
use zerodds_types::type_object::{MinimalTypeObject, TypeObject};

/// Declared in the order zebra, alpha, mid: neither the name order nor its
/// reverse.
const IDL: &str = "\
@annotation Wide {
    long zebra default 1;
    boolean alpha;
    long mid default 2;
};";

fn declaration() -> (
    zerodds_ast::Ast,
    zerodds_ast_core::Id<zerodds_ast::AnnotationDefNode>,
) {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "wide.idl").ast;
    let report = ast.resolve_all_by_name().expect("resolution");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);
    let id = ast
        .annotation_defs()
        .find(|(_, node)| node.name == "Wide")
        .map(|(id, _)| id)
        .expect("the declaration");
    (ast, id)
}

fn complete_names(
    ast: &zerodds_ast::Ast,
    id: zerodds_ast_core::Id<zerodds_ast::AnnotationDefNode>,
) -> Vec<String> {
    let TypeObject::Complete(CompleteTypeObject::Annotation(annotation)) =
        annotation_type_object(ast, id, Form::Complete).expect("complete")
    else {
        panic!("not a complete annotation object");
    };
    annotation
        .member_seq
        .iter()
        .map(|p| p.name.clone())
        .collect()
}

#[test]
fn the_parameters_follow_member_index_and_the_annex_b_comment_is_a_setting() {
    let (ast, id) = declaration();

    // The default is §7.3.4.5: the declaration order.
    assert_eq!(
        annotation_parameter_order(),
        AnnotationParameterOrder::MemberIndex
    );
    assert_eq!(complete_names(&ast, id), ["zebra", "alpha", "mid"]);
    let by_index = annotation_identifier(&ast, id, Form::Complete).expect("identifier");

    // The Annex B comment, as the named setting.
    set_annotation_parameter_order(AnnotationParameterOrder::Name);
    let by_name_names = complete_names(&ast, id);
    let by_name = annotation_identifier(&ast, id, Form::Complete).expect("identifier");
    set_annotation_parameter_order(AnnotationParameterOrder::MemberIndex);

    assert_eq!(by_name_names, ["alpha", "mid", "zebra"]);
    assert_ne!(
        by_index, by_name,
        "the order is in the serialized object, so it is in the hash"
    );
    assert_eq!(complete_names(&ast, id), ["zebra", "alpha", "mid"]);
}

#[test]
fn the_minimal_form_is_ordered_by_the_hash_of_the_name_in_both_readings() {
    let (ast, id) = declaration();
    let TypeObject::Minimal(MinimalTypeObject::Annotation(annotation)) =
        annotation_type_object(&ast, id, Form::Minimal).expect("minimal")
    else {
        panic!("not a minimal annotation object");
    };
    let hashes: Vec<[u8; 4]> = annotation
        .member_seq
        .iter()
        .map(|p| p.name_hash.0)
        .collect();
    let mut sorted = hashes.clone();
    sorted.sort_unstable();
    assert_eq!(hashes, sorted, "increasing member_name hash (§7.3.4.5)");
}

#[test]
fn the_setting_has_a_spelling_that_reads_back() {
    for order in [
        AnnotationParameterOrder::MemberIndex,
        AnnotationParameterOrder::Name,
    ] {
        assert_eq!(AnnotationParameterOrder::parse(order.as_str()), Some(order));
    }
    assert_eq!(AnnotationParameterOrder::parse("by-name"), None);
}
