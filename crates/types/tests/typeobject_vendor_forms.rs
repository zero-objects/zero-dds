// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The forms of `TypeObjectForm` against the equivalence hashes the
//! implementations print for the OMG dds-xtypes suite's `struct_hashid_1`
//! and `struct_hashid_2` (`types/idl/extensibility.idl`).
//!
//! The values are the `--print-typeid` output of RTI Connext 7.7.0, CoreDX
//! 6.15.3, Cyclone DDS 11.0.1 and HDDS 1.4.8 in the official workflow of
//! the suite in the ZeroDDS fork (runs 42504601421 and 42504609519,
//! `junit_typeid_report_to2`). Connext and CoreDX print one minimal hash,
//! Cyclone and HDDS another (the trailing octet of
//! `docs/omg-xtypes-issues.md` §C2); all four print the same complete one.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_core::Id;
use zerodds_types::from_ast::{
    Form, TypeObjectForm, complete_type_object_in, minimal_type_object_in, type_identifier_in,
};
use zerodds_types::hash::{compute_complete_hash, compute_minimal_hash_with};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::TypeIdentifier;
use zerodds_types::type_information::TypeInformation;

const IDL: &str = r#"
module Test {
  @autoid
  @final
  struct struct_hashid_1 {
    int32 x1;
  };

  @autoid
  @final
  struct struct_hashid_2 {
    @hashid("x1")
    int32 x2;
  };
};
"#;

fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "extensibility.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_node(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    let segments: Vec<String> = name.split("::").map(str::to_string).collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("`{name}` is not in the model"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn minimal_hash(ast: &Ast, name: &str, form: TypeObjectForm) -> String {
    let object = minimal_type_object_in(ast, type_node(ast, name), form).expect("minimal");
    hex(
        &compute_minimal_hash_with(&object, form.minimal_member_trailing_octet)
            .expect("hash")
            .0,
    )
}

fn complete_hash(ast: &Ast, name: &str, form: TypeObjectForm) -> String {
    let object = complete_type_object_in(ast, type_node(ast, name), form).expect("complete");
    hex(&compute_complete_hash(&object).expect("hash").0)
}

const VENDORS: TypeObjectForm = TypeObjectForm {
    minimal_member_trailing_octet: false,
    minimal_type_flags_kept: true,
    hash_id_omitted: true,
};

const VENDORS_RTI_CAMP: TypeObjectForm = TypeObjectForm {
    minimal_member_trailing_octet: true,
    minimal_type_flags_kept: true,
    hash_id_omitted: true,
};

/// The minimal object of an `@autoid(HASH)` structure with the type flags
/// kept is Cyclone's and HDDS's, in the other camp Connext's and CoreDX's;
/// with Annex B's `TypeFlagMinimalMask` (the default) it is this tree's
/// own.
#[test]
fn xtypes_annex_b_the_kept_minimal_type_flags_give_the_vendors_hashes() {
    let ast = model();
    assert_eq!(
        minimal_hash(&ast, "Test::struct_hashid_1", VENDORS),
        "e4d670195d5577762f1aeac58789"
    );
    assert_eq!(
        minimal_hash(&ast, "Test::struct_hashid_1", VENDORS_RTI_CAMP),
        "cc05f6317a177441dedb7e570235"
    );
    assert_eq!(
        minimal_hash(&ast, "Test::struct_hashid_2", VENDORS),
        "eaff87eeb0600b9b39747fcb9853"
    );
    assert_eq!(
        minimal_hash(&ast, "Test::struct_hashid_2", VENDORS_RTI_CAMP),
        "ebbb5f92c83020e8aaa0f3ade8f0"
    );
    assert_eq!(
        minimal_hash(&ast, "Test::struct_hashid_1", TypeObjectForm::default()),
        "d1e3035ed00b4e1bddf9b9d95891"
    );
}

/// The complete object without `hash_id` is the one all four print;
/// with it (the default) it is this tree's own. A structure without
/// `@hashid` is the same either way.
#[test]
fn xtypes_annex_b_the_omitted_hash_id_gives_the_vendors_complete_hash() {
    let ast = model();
    assert_eq!(
        complete_hash(&ast, "Test::struct_hashid_2", VENDORS),
        "82ca22bf6abcb911fb19260400d1"
    );
    assert_eq!(
        complete_hash(&ast, "Test::struct_hashid_2", TypeObjectForm::default()),
        "3c80e6780d9117f99d65a5120641"
    );
    for form in [VENDORS, TypeObjectForm::default()] {
        assert_eq!(
            complete_hash(&ast, "Test::struct_hashid_1", form),
            "ba591a8bfd5a0cf2bdeee042a9fc"
        );
    }
    // The identifier is the hash of the object it is formed with.
    let id = type_identifier_in(
        &ast,
        type_node(&ast, "Test::struct_hashid_2"),
        Form::Complete,
        VENDORS,
    )
    .expect("identifier");
    let TypeIdentifier::EquivalenceHashComplete(hash) = id else {
        panic!("a complete hash identifier, got {id:?}");
    };
    assert_eq!(hex(&hash.0), "82ca22bf6abcb911fb19260400d1");
}

/// The registry's projection of the complete object, which the runtime
/// announces and serves, is the minimal object `from_ast` forms: the same
/// hash in both camps, the announced `TypeInformation` included.
#[test]
fn xtypes_7_3_4_9_the_registry_projects_and_announces_in_the_chosen_form() {
    let ast = model();
    for (form, expected) in [
        (VENDORS, "e4d670195d5577762f1aeac58789"),
        (VENDORS_RTI_CAMP, "cc05f6317a177441dedb7e570235"),
        (TypeObjectForm::default(), "d1e3035ed00b4e1bddf9b9d95891"),
    ] {
        let complete =
            complete_type_object_in(&ast, type_node(&ast, "Test::struct_hashid_1"), form)
                .expect("complete");
        let mut registry = TypeRegistry::new();
        registry.set_minimal_member_trailing_octet(form.minimal_member_trailing_octet);
        registry.set_minimal_type_flags_kept(form.minimal_type_flags_kept);
        let projected = complete.to_minimal(&registry).expect("projects");
        let hash = compute_minimal_hash_with(&projected, form.minimal_member_trailing_octet)
            .expect("hash");
        assert_eq!(hex(&hash.0), expected, "{form:?}");
        let information = TypeInformation::from_registry(&complete, &registry).expect("announced");
        assert_eq!(
            information.minimal.typeid_with_size.type_id,
            TypeIdentifier::EquivalenceHashMinimal(hash),
            "{form:?}"
        );
    }
}

/// A minimal object in a getTypes reply is written in the camp its
/// identifier was hashed in: the reply of a participant in the RTI/CoreDX
/// camp carries the bytes whose hash is the identifier it announced.
#[test]
fn xtypes_7_6_3_3_4_a_gettypes_reply_writes_a_minimal_object_in_its_camp() {
    use zerodds_types::hash::minimal_hash_input_with;
    use zerodds_types::type_lookup::{
        GetTypesReply, GetTypesResult, ReplyHeader, TypeIdentifierTypeObjectPair,
        TypeLookupEncoding, TypeLookupReplyMessage, TypeLookupReturn,
    };
    use zerodds_types::type_object::TypeObject;

    let ast = model();
    let object = minimal_type_object_in(
        &ast,
        type_node(&ast, "Test::struct_hashid_1"),
        VENDORS_RTI_CAMP,
    )
    .expect("minimal");
    let hash = compute_minimal_hash_with(&object, true).expect("hash");
    let reply = TypeLookupReplyMessage {
        header: ReplyHeader {
            writer_guid: [7; 16],
            sequence_number: 1,
            remote_ex: 0,
        },
        ret: TypeLookupReturn::GetTypes(GetTypesResult::Ok(GetTypesReply {
            types: vec![TypeIdentifierTypeObjectPair {
                type_identifier: TypeIdentifier::EquivalenceHashMinimal(hash),
                type_object: TypeObject::Minimal(object.clone()),
            }],
            complete_to_minimal: Vec::new(),
        })),
    };
    let rti = minimal_hash_input_with(&object, true).expect("RTI camp bytes");
    let cyclone = minimal_hash_input_with(&object, false).expect("Cyclone camp bytes");
    assert_ne!(rti, cyclone);
    let contains = |payload: &[u8], bytes: &[u8]| payload.windows(bytes.len()).any(|w| w == bytes);
    let with_camp = reply
        .to_payload_with(TypeLookupEncoding::Xcdr2, true)
        .expect("encodes");
    assert!(
        contains(&with_camp, &rti),
        "the RTI camp's object is in the reply"
    );
    let without = reply
        .to_payload(TypeLookupEncoding::Xcdr2)
        .expect("encodes");
    assert!(
        contains(&without, &cyclone),
        "the default camp's object is in the reply"
    );
}
