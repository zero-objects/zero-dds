// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The annotation TypeObjects, octet for octet against the IDL of DDS-XTypes
//! 1.3 Annex B.
//!
//! # Why the reference is the IDL and not a capture
//!
//! No vendor has recorded the octets of an annotation type. The other
//! type objects in this crate have vendor goldens
//! (`typeobject_member_annotations.rs` holds Cyclone DDS's and RTI Connext's
//! for a member's `@min`/`@max`/`@unit`, `typeobject_vendor_dheader.rs`
//! holds Cyclone's for four structures), because a topic type reaches a
//! peer through discovery and a capture is the only answer to "what does
//! the peer send". An annotation type reaches a peer only when that peer
//! asks for it over TypeLookup (§7.6.3.3), and no such request has been
//! made on a host with a vendor installed. What is measured here is
//! therefore the serialization the specification declares: the members,
//! their order, their extensibility and the `DHEADER`s that follow from
//! it, read off the Annex B IDL in §7.3.4.5 and §7.4.3.5.3.
//!
//! That is a weaker claim than a vendor capture and it is stated as one.
//! What it does hold is that a mutation which drops a `DHEADER`, writes a
//! `member_id` Annex B does not declare, replaces a `NameHash` with a name
//! where Annex B has the hash, or puts the default value behind a length
//! of its own moves an octet and fails here.
//!
//! # The declarations
//!
//! ```idl
//! @extensibility(FINAL) @nested
//! struct CommonAnnotationParameter {
//!      AnnotationParameterFlag       member_flags;
//!      TypeIdentifier                member_type_id;
//! };
//!
//! @extensibility(APPENDABLE) @nested
//! struct CompleteAnnotationParameter {
//!      CommonAnnotationParameter   common;
//!      MemberName                  name;
//!      AnnotationParameterValue    default_value;
//! };
//! // Ordered by CompleteAnnotationParameter.name
//!
//! @extensibility(APPENDABLE) @nested
//! struct MinimalAnnotationParameter {
//!      CommonAnnotationParameter   common;
//!      NameHash                    name_hash;
//!      AnnotationParameterValue    default_value;
//! };
//! // Ordered by MinimalAnnotationParameter.name_hash
//!
//! @extensibility(APPENDABLE) @nested
//! struct CompleteAnnotationHeader {
//!      QualifiedTypeName           annotation_name;
//! };
//!
//! @extensibility(APPENDABLE) @nested
//! struct MinimalAnnotationHeader {
//!      // Empty. Available for future extension
//! };
//!
//! @extensibility(FINAL) @nested
//! struct CompleteAnnotationType {
//!      AnnotationTypeFlag               annotation_flag;
//!      CompleteAnnotationHeader         header;
//!      CompleteAnnotationParameterSeq member_seq;
//! };
//!
//! @extensibility(FINAL) @nested
//! struct MinimalAnnotationType {
//!      AnnotationTypeFlag               annotation_flag;
//!      MinimalAnnotationHeader          header;
//!      MinimalAnnotationParameterSeq    member_seq;
//! };
//! ```
//!
//! Three `DHEADER`s come out of that, per XTypes 1.3 §7.4.3.5.3: the two
//! `Parameter`s are `@APPENDABLE` (rule 29), each `Header` is
//! `@APPENDABLE` (rule 29), and each `…ParameterSeq` is a `SEQUENCE_TYPE`
//! under version 2 (rule 12). `CommonAnnotationParameter` and
//! `AnnotationParameterValue` are `@FINAL` and carry none, and the two
//! `…AnnotationType`s are `@FINAL` and carry none either.
//!
//! # The two orderings
//!
//! `level` and `loud` are declared in that order. The complete sequence is
//! ordered by name (`level` < `loud`), which is the declaration order;
//! the minimal sequence is ordered by name hash, and
//! `MD5("loud")[0..4] = 51 ed f3 fe` sorts before
//! `MD5("level")[0..4] = c9 e9 a8 48`. The two sequences are not the same
//! sequence, which is why both orderings are below and not one of them.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_types::from_ast::{Form, annotation_type_object};
use zerodds_types::type_object::complete::CompleteTypeObject;
use zerodds_types::type_object::{MinimalTypeObject, TypeObject};

/// Two parameters, one with a declared default.
const IDL: &str = "\
@annotation Volume {
    long level default 3;
    boolean loud;
};";

/// The `MINIMAL` object, 52 octets, assembled from the declarations above.
///
/// Reading it: a `DHEADER` of 48 over the rest; `EK_MINIMAL` and
/// `TK_ANNOTATION`; `annotation_flag` zero; the empty
/// `MinimalAnnotationHeader` as a bare `DHEADER` of 0; the
/// `MinimalAnnotationParameterSeq` as a `DHEADER` over a count and two
/// parameters; and each parameter a `DHEADER` over
/// `member_flags`, the `TypeIdentifier` (`TK_BOOLEAN`, `TK_INT32` — one
/// octet each, a primitive kind carries no body), the `NameHash`, and the
/// `AnnotationParameterValue` as a kind octet and the value.
const MINIMAL: &str = "\
30000000f15000000000000024000000020000000900000000000151edf3fe0100\
0000000c000000000004c9e9a8480403000000";

/// The `COMPLETE` object, 75 octets, from the same declarations.
///
/// `CompleteAnnotationHeader` is a `DHEADER` over the qualified name — a
/// `string`, so a length and the octets with the NUL — and not the
/// `CompleteTypeDetail` of a structure, which would have carried
/// `ann_builtin` and `ann_custom` as well.
const COMPLETE: &str = "\
47000000f25000000b00000007000000566f6c756d6500002f00000002000000\
1400000000000400060000006c6576656c000400030000000f00000000000100\
050000006c6f7564000100";

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "an odd number of hex digits");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The model, with [`IDL`] lowered and resolved.
fn model() -> (
    zerodds_ast::Ast,
    zerodds_ast_core::Id<zerodds_ast::AnnotationDefNode>,
) {
    let parsed = zerodds_idl_parse::parse(IDL);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the IDL does not parse: {errors:?}");
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "annotation.idl").ast;
    let report = ast.resolve_all_by_name().expect("resolution");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);
    let id = ast
        .annotation_defs()
        .find(|(_, node)| node.name == "Volume")
        .map(|(id, _)| id)
        .expect("the declaration");
    (ast, id)
}

/// The encoder writes Annex B's octets, and nothing else.
#[test]
fn the_minimal_object_is_annex_bs_octet_for_octet() {
    let (ast, id) = model();
    let object = annotation_type_object(&ast, id, Form::Minimal).expect("minimal");
    let bytes = TypeObject::Minimal(match object {
        TypeObject::Minimal(m) => m,
        other => panic!("not a minimal object: {other:?}"),
    })
    .to_bytes_le()
    .expect("encode");
    assert_eq!(bytes, unhex(MINIMAL), "got {}", hex(&bytes));
}

/// The same for the complete form.
#[test]
fn the_complete_object_is_annex_bs_octet_for_octet() {
    let (ast, id) = model();
    let object = annotation_type_object(&ast, id, Form::Complete).expect("complete");
    let bytes = TypeObject::Complete(match object {
        TypeObject::Complete(c) => c,
        other => panic!("not a complete object: {other:?}"),
    })
    .to_bytes_le()
    .expect("encode");
    assert_eq!(bytes, unhex(COMPLETE), "got {}", hex(&bytes));
}

/// The octets above decode back into the two objects the declaration
/// produces — the same test in the other direction, and the one that would
/// notice a decoder reading a shape the encoder does not write.
#[test]
fn annex_bs_octets_decode_into_the_two_objects() {
    let (ast, id) = model();
    for (form, octets) in [
        (Form::Minimal, unhex(MINIMAL)),
        (Form::Complete, unhex(COMPLETE)),
    ] {
        let built = annotation_type_object(&ast, id, form).expect("built");
        let decoded = TypeObject::from_bytes_le(&octets).expect("decode");
        assert_eq!(decoded, built, "{form:?}: the octets do not decode back");
        assert_eq!(decoded.to_bytes_le().expect("re-encode"), octets);
    }
}

/// The two sequences are ordered by different keys, and each is in its
/// own key's order rather than the declaration's.
#[test]
fn each_sequence_is_ordered_by_its_own_key() {
    let (ast, id) = model();

    let minimal = annotation_type_object(&ast, id, Form::Minimal).expect("minimal");
    let TypeObject::Minimal(MinimalTypeObject::Annotation(annotation)) = &minimal else {
        panic!("not a minimal annotation: {minimal:?}");
    };
    let hashes: Vec<[u8; 4]> = annotation
        .member_seq
        .iter()
        .map(|p| p.name_hash.0)
        .collect();
    assert_eq!(
        hashes,
        vec![[0x51, 0xed, 0xf3, 0xfe], [0xc9, 0xe9, 0xa8, 0x48]],
        "increasing NameHash, so `loud` before `level`"
    );

    let complete = annotation_type_object(&ast, id, Form::Complete).expect("complete");
    let TypeObject::Complete(CompleteTypeObject::Annotation(annotation)) = &complete else {
        panic!("not a complete annotation: {complete:?}");
    };
    let names: Vec<&str> = annotation
        .member_seq
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(names, vec!["level", "loud"], "increasing MemberName");
}

/// The complete header is `CompleteAnnotationHeader` and carries the
/// qualified name and nothing else.
#[test]
fn the_complete_header_carries_the_name_alone() {
    let (ast, id) = model();
    let complete = annotation_type_object(&ast, id, Form::Complete).expect("complete");
    let TypeObject::Complete(CompleteTypeObject::Annotation(annotation)) = &complete else {
        panic!("not a complete annotation: {complete:?}");
    };
    assert_eq!(annotation.header.annotation_name, "Volume");
}

/// A `CommonAnnotationParameter` is two members and no `member_id`, so the
/// parameter's own octets begin with the flags. A `member_id` in front of
/// them shifts every parameter by four octets and both sequences by the
/// sum of them.
#[test]
fn a_parameter_begins_with_its_flags_and_has_no_member_id() {
    let minimal = unhex(MINIMAL);
    let TypeObject::Minimal(annotation) =
        TypeObject::from_bytes_le(&minimal).expect("decode minimal")
    else {
        panic!("not a minimal object");
    };
    let MinimalTypeObject::Annotation(annotation) = annotation else {
        panic!("not an annotation");
    };
    for parameter in &annotation.member_seq {
        assert_eq!(parameter.common.member_flags.0, 0);
    }
}

/// The default value goes out as Annex B's `AnnotationParameterValue`: a
/// kind octet and the value in the parameter's own kind. `level` declares
/// `default 3`; `loud` declares none, and takes its type's default per
/// §7.3.1.2.4 and Table 9 — `FALSE` for a `boolean`.
#[test]
fn the_default_value_is_the_annex_b_union_and_never_empty() {
    use zerodds_types::type_object::common::AnnotationParameterValue;
    use zerodds_types::type_object::common::NameHash;
    let (ast, id) = model();

    let minimal = annotation_type_object(&ast, id, Form::Minimal).expect("minimal");
    let TypeObject::Minimal(MinimalTypeObject::Annotation(a)) = &minimal else {
        panic!("not a minimal annotation: {minimal:?}");
    };
    let by_hash = |name: &str| {
        a.member_seq
            .iter()
            .find(|p| p.name_hash == NameHash::from_name(name))
            .unwrap_or_else(|| panic!("no parameter `{name}`"))
    };
    assert_eq!(
        by_hash("level").default_value,
        AnnotationParameterValue::Int32(3)
    );
    assert_eq!(
        by_hash("loud").default_value,
        AnnotationParameterValue::Boolean(false),
        "Table 9's default for a boolean, since the declaration gives none"
    );

    let complete = annotation_type_object(&ast, id, Form::Complete).expect("complete");
    let TypeObject::Complete(CompleteTypeObject::Annotation(a)) = &complete else {
        panic!("not a complete annotation: {complete:?}");
    };
    let by_name = |name: &str| {
        a.member_seq
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no parameter `{name}`"))
    };
    assert_eq!(
        by_name("level").default_value,
        AnnotationParameterValue::Int32(3)
    );
    assert_eq!(
        by_name("loud").default_value,
        AnnotationParameterValue::Boolean(false)
    );
}

/// A parameter the declaration gives no default for is not left off the
/// wire and not sent as an empty blob: `default_value` is not `@optional`
/// in Annex B, so it always carries a value.
#[test]
fn a_parameter_without_a_declared_default_still_carries_one() {
    let octets = unhex(MINIMAL);
    // `loud`'s parameter body ends with the kind octet `TK_BOOLEAN` and
    // the one octet of `FALSE`; there is no length in front of them.
    let position = octets
        .windows(2)
        .position(|w| w == [0x01, 0x00])
        .expect("`loud`'s TK_BOOLEAN and FALSE");
    assert_eq!(
        &octets[position - 4..position],
        [0x51, 0xed, 0xf3, 0xfe],
        "the NameHash sits directly in front of the value union"
    );
}

/// The empty `MinimalAnnotationHeader` is a `DHEADER` of zero — the
/// appendable aggregate of §7.4.3.4.1 with a body of nothing. Dropping it
/// moves every octet after it.
#[test]
fn the_minimal_header_is_a_dheader_of_zero() {
    let octets = unhex(MINIMAL);
    // `annotation_flag` (two octets, aligned to two), then the header.
    let flag = 6;
    assert_eq!(&octets[flag..flag + 2], &[0x00, 0x00], "annotation_flag");
    assert_eq!(
        &octets[flag + 2..flag + 6],
        &[0x00, 0x00, 0x00, 0x00],
        "MinimalAnnotationHeader is an empty DHEADER"
    );
}

/// Each parameter's own `DHEADER` covers its body and the sequence's
/// `DHEADER` covers the count and both parameters — the two lengths rule
/// (12) and rule (29) of §7.4.3.5.3 put there. A member sequence written
/// without the outer header reads as a length of zero and the whole object
/// as parameterless.
#[test]
fn every_appendable_and_sequence_carries_its_own_length() {
    fn announced(octets: &[u8], at: usize) -> usize {
        u32::from_le_bytes([octets[at], octets[at + 1], octets[at + 2], octets[at + 3]]) as usize
    }
    let minimal = unhex(MINIMAL);
    // MINIMAL: 4 outer, 2 discriminators, 2 flag — so the empty
    // `MinimalAnnotationHeader`'s DHEADER is at 8, the sequence's at 12,
    // the count at 16, and each parameter's at 20 and 36 after the
    // three-octet alignment gap `loud`'s nine-octet body leaves.
    assert_eq!(announced(&minimal, 0), minimal.len() - 4, "the object");
    assert_eq!(announced(&minimal, 8), 0, "the empty minimal header");
    assert_eq!(
        announced(&minimal, 12),
        minimal.len() - 16,
        "the parameter sequence: the count plus both parameters"
    );
    assert_eq!(announced(&minimal, 20), 9, "`loud`'s parameter body");
    assert_eq!(announced(&minimal, 36), 12, "`level`'s parameter body");

    let complete = unhex(COMPLETE);
    // COMPLETE: the `CompleteAnnotationHeader`'s DHEADER is at 8 and
    // announces the name's length and its seven octets; the sequence's is
    // at 24 after the one-octet gap, and the parameters' at 32 and 56.
    assert_eq!(announced(&complete, 0), complete.len() - 4, "the object");
    assert_eq!(
        announced(&complete, 8),
        11,
        "the header: the qualified name"
    );
    assert_eq!(
        announced(&complete, 24),
        complete.len() - 28,
        "the parameter sequence"
    );
    assert_eq!(announced(&complete, 32), 20, "`level`'s parameter body");
    assert_eq!(announced(&complete, 56), 15, "`loud`'s parameter body");

    // The bodies themselves, so a `member_id` in front of the flags or a
    // length in front of the value union is visible as octets and not only
    // as a moved length.
    assert_eq!(
        &minimal[24..33],
        // flags, TK_BOOLEAN, NameHash("loud"), TK_BOOLEAN, FALSE
        [0x00, 0x00, 0x01, 0x51, 0xed, 0xf3, 0xfe, 0x01, 0x00]
    );
    assert_eq!(
        &minimal[40..52],
        // flags, TK_INT32, NameHash("level"), TK_INT32, 3 — the `i32`
        // lands on the body's own four-octet boundary, so no pad
        [
            0x00, 0x00, 0x04, 0xc9, 0xe9, 0xa8, 0x48, 0x04, 0x03, 0x00, 0x00, 0x00
        ]
    );
    assert_eq!(
        &complete[36..56],
        // flags, TK_INT32, the string length and "level\0", TK_INT32,
        // then the two-octet gap before the `i32` at 52
        [
            0x00, 0x00, 0x04, 0x00, 0x06, 0x00, 0x00, 0x00, 0x6c, 0x65, 0x76, 0x65, 0x6c, 0x00,
            0x04, 0x00, 0x03, 0x00, 0x00, 0x00,
        ]
    );
    assert_eq!(
        &complete[60..75],
        // flags, TK_BOOLEAN, the pad that puts the string length at 64,
        // "loud", TK_BOOLEAN, FALSE
        [
            0x00, 0x00, 0x01, 0x00, 0x05, 0x00, 0x00, 0x00, 0x6c, 0x6f, 0x75, 0x64, 0x00, 0x01,
            0x00,
        ]
    );
}
