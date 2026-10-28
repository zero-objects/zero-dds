// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A user-declared annotation, through the `TypeObject` and back.
//!
//! §7.3.4.5's `AppliedAnnotation` is the only place an `@annotation` a
//! user declared reaches a peer, and it says almost nothing: an
//! `annotation_typeid`, which is a hash, and one
//! `AppliedAnnotationParameter` per parameter given, each named by the
//! hash of its name. What is measured here is that both directions carry
//! that — `crate::from_ast` writes it out of the model's
//! `Facets::annotations`, and `dynamic::to_ast` reads it back into one.
//!
//! ## What is measured, and what is not
//!
//! **No vendor capture exists.** No peer has been asked for the octets of
//! an applied annotation: an annotation type reaches a peer only over
//! TypeLookup (§7.6.3.3), and no such request has been made on a host
//! with a vendor installed. The builtin member annotations beside this
//! one *were* captured — `typeobject_member_annotations.rs` holds Cyclone
//! DDS's and RTI Connext's — because a topic type reaches a peer through
//! discovery and a capture is the only answer to "what does the peer
//! send". What is checked below is therefore the form the specification
//! declares, read off Annex B octet by octet. That is a weaker claim
//! than a vendor capture and it is stated as one.
//!
//! ## The identifier, and why it is an `EquivalenceHash`
//!
//! §7.3.4.2's Table 29 gives `TK_ANNOTATION` a row — "Not plain type.
//! Has TypeObject. Uses Hash of the TypeObject representation of the
//! Annotation declaration" — and Annex B's `union TypeIdentifier switch
//! (octet)` has **no case for it**. This tree follows Annex B
//! (`docs/omg-xtypes-issues.md` §B9 writes the contradiction up), so the
//! hash goes in the `EK_COMPLETE` case, which is the only arm that can
//! carry fourteen hash octets. `EK_COMPLETE` and not `EK_MINIMAL`
//! because `ann_custom` is a member of `CompleteTypeDetail` and of
//! `CompleteMemberDetail` and of no minimal struct: §7.3.4.4 names
//! "user-defined annotations" among the things the minimal form leaves
//! out.
//!
//! ## What does not come back
//!
//! The written form of an application. §7.3.4.5 encodes `@Unit("m")` and
//! `@Unit(symbol = "m")` identically — both are one
//! `AppliedAnnotationParameter` under one `paramname_hash` — so the
//! rebuilt model writes the named form, which is the only one a reader
//! can tell. `the_short_form_comes_back_as_the_named_one` says so.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::string::ToString;

use zerodds_ast::{Ast, ConstValue, MemberNode, TypeNode};
use zerodds_ast_core::{Id, Ref};
use zerodds_types::dynamic::dynamic_type;
use zerodds_types::from_ast::{
    Form, annotation_identifier, annotation_type_object, complete_type_object,
    dependency_type_objects, minimal_type_object, register_all,
};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_object::common::{
    AnnotationParameterValue, AppliedAnnotation, NameHash, OptionalAppliedAnnotationSeq,
};
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};
use zerodds_types::{EquivalenceHash, TypeIdentifier, compute_hash};

/// The declaration under test. `symbol` is the single member, so
/// `@Unit("m")` and `@Unit(symbol = "m")` are both valid applications of
/// it — IDL 4.2 §7.4.15.4.2's short form is for an annotation with
/// exactly one member.
const IDL: &str = r#"
@annotation Unit {
    string symbol;
};

@final struct Sample {
    @Unit("m") double distance;
    @Unit(symbol = "s") long elapsed;
    long plain;
};
"#;

/// The `CompleteTypeObject` of `Sample`, 228 octets, assembled by hand
/// from Annex B and cross-checked octet for octet against an
/// implementation of §7.3.4.5 written beside this file from the IDL alone.
///
/// Reading it, with the offsets the tests below use:
///
/// - **0** — a `DHEADER` of 224 over the rest: `@extensibility(APPENDABLE)`
///   on the `CompleteTypeObject` itself.
/// - **4, 5** — `EK_COMPLETE`, `TK_STRUCTURE`. **6–7** — `struct_flags`,
///   `IS_FINAL` at bit 0.
/// - **8** — the `CompleteStructHeader`'s own `DHEADER`, of 15: **12**
///   `base_type` as `TK_NONE`, **13** `@optional ann_builtin` absent,
///   **14** `@optional ann_custom` absent, **16–19** the qualified name's
///   length, **20–26** `Sample` and its NUL. The type applied nothing.
/// - **28** — the `CompleteStructMemberSeq`'s `DHEADER` (rule 12), **32**
///   the count of three.
/// - **36** — `distance`'s `CompleteStructMember` `DHEADER`, of 78 over a
///   `CommonStructMember` — **40** `member_id`, **44** `member_flags`,
///   **46** `member_type_id` as `TK_FLOAT64`, one octet, a primitive kind
///   carries no body — and a `CompleteMemberDetail`: **48** the name's
///   length, **52–60** `distance` and its NUL, **61** `@optional
///   ann_builtin` absent, **62** `@optional ann_custom` **present**.
/// - **64** — the `AppliedAnnotationSeq`'s `DHEADER`, of 50, and **68** a
///   count of one. **72** — the `AppliedAnnotation`'s own `DHEADER`, of
///   42: **76** the `annotation_typeid` as `EK_COMPLETE`, **77–90** the
///   fourteen hash octets of `Unit`'s own `CompleteTypeObject`, **91**
///   `@optional param_seq` present.
/// - **92** — the `AppliedAnnotationParameterSeq`'s `DHEADER`, of 22, and
///   **96** a count of one. **100** — the
///   `AppliedAnnotationParameter`'s `DHEADER`, of 14: **104–107** the
///   `NameHash` of `symbol`, **108** the `AnnotationParameterValue`
///   union's `TK_STRING8`, **112** the string's own length, **116–117**
///   `m` and its NUL.
/// - **120** — `elapsed`'s member, the same shape with `TK_INT32` and
///   `s`. **204** — `plain`'s, of 20: `@optional ann_custom` at **227**
///   **absent**, one octet.
const SAMPLE: &str = "\
e0000000f25101000f000000000000000700000053616d706c650000c4000000\
030000004e0000000000000001000a000900000064697374616e636500000100\
32000000010000002a000000f24a8db6765e6c5e7d8773c740dd8c0116000000\
010000000e00000097bff26820000000020000006d0000004e00000001000000\
0100040008000000656c6170736564000001000032000000010000002a000000\
f24a8db6765e6c5e7d8773c740dd8c0116000000010000000e00000097bff268\
20000000020000007300000014000000020000000100040006000000706c6169\
6e000000";

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "an odd number of hex digits");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "custom.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_of(ast: &Ast, name: &str) -> Id<TypeNode> {
    let segments: Vec<String> = name.split("::").map(ToString::to_string).collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("`{name}` is not in the model"))
}

fn annotation_of(ast: &Ast, name: &str) -> Id<zerodds_ast::AnnotationDefNode> {
    ast.annotation_defs()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("`@annotation {name}` is not in the model"))
}

fn member<'a>(ast: &'a Ast, type_: Id<TypeNode>, name: &str) -> &'a MemberNode {
    ast.members_of(type_)
        .iter()
        .map(|id| ast.member(*id).expect("a member the type listed"))
        .find(|member| member.name == name)
        .unwrap_or_else(|| panic!("no member `{name}`"))
}

/// `sample`'s `ann_custom` for one member.
fn custom(object: &CompleteTypeObject, member: &str) -> OptionalAppliedAnnotationSeq {
    let CompleteTypeObject::Struct(node) = object else {
        panic!("not a struct object");
    };
    node.member_seq
        .iter()
        .find(|entry| entry.detail.name == member)
        .unwrap_or_else(|| panic!("no member `{member}`"))
        .detail
        .ann_custom
        .clone()
}

/// The applications of an `ann_custom` that is present.
fn applied(sequence: &OptionalAppliedAnnotationSeq) -> Vec<AppliedAnnotation> {
    sequence
        .0
        .clone()
        .unwrap_or_else(|| panic!("`ann_custom` is absent"))
}

fn complete_hash_of(ast: &Ast, annotation: Id<zerodds_ast::AnnotationDefNode>) -> EquivalenceHash {
    match annotation_identifier(ast, annotation, Form::Complete).expect("identifier") {
        TypeIdentifier::EquivalenceHashComplete(hash) => hash,
        other => panic!("not a complete hash: {other:?}"),
    }
}

/// `sample`'s object through the wire form, so everything below is
/// checked against the octets and not against the in-memory struct.
fn through_the_wire(ast: &Ast) -> CompleteTypeObject {
    let object = complete_type_object(ast, type_of(ast, "Sample")).expect("the object builds");
    let bytes = TypeObject::Complete(object)
        .to_bytes_le()
        .expect("the object encodes");
    match TypeObject::from_bytes_le(&bytes).expect("the octets decode") {
        TypeObject::Complete(decoded) => decoded,
        other => panic!("a complete object came back as {other:?}"),
    }
}

/// `from_ast` writes an `AppliedAnnotation` for an applied
/// user-declared annotation, and its `annotation_typeid` is the
/// `EK_COMPLETE` hash of that declaration's own `TypeObject`.
///
/// Both halves are checked against `annotation_type_object` and
/// `compute_hash` rather than against a literal, so this says the
/// identifier is *that* declaration's hash and not merely some hash. The
/// literal is [`SAMPLE`], and `the_applied_annotation_octets_are_annex_b`
/// checks the encoding itself.
#[test]
fn an_applied_annotation_names_its_declaration_by_hash() {
    let ast = model(IDL);
    let object = through_the_wire(&ast);
    let unit = annotation_of(&ast, "Unit");
    let declaration = match annotation_type_object(&ast, unit, Form::Complete).expect("Unit") {
        TypeObject::Complete(complete) => complete,
        other => panic!("not a complete annotation object: {other:?}"),
    };
    let expected = compute_hash(&TypeObject::Complete(declaration)).expect("the hash over Unit");

    let distance = custom(&object, "distance");
    let applications = applied(&distance);
    assert_eq!(applications.len(), 1, "one application of `@Unit`");
    assert_eq!(
        applications[0].annotation_typeid,
        TypeIdentifier::EquivalenceHashComplete(expected)
    );
    // The same answer the public accessor gives, so the two are one
    // derivation and not two that happen to agree.
    assert_eq!(
        annotation_identifier(&ast, unit, Form::Complete).expect("identifier"),
        TypeIdentifier::EquivalenceHashComplete(expected)
    );
}

/// The octets [`SAMPLE`] describes, octet for octet.
///
/// This is the test that would notice a mutation the object model hides:
/// a `DHEADER` dropped from `AppliedAnnotation` or from
/// `AppliedAnnotationParameter`, a parameter value written behind a length
/// of its own rather than as the final union Annex B declares, a
/// `member_id` where `CommonAnnotationParameter` has none, or the
/// fourteen hash octets of the annotation's identifier replaced by
/// something another reader would resolve differently.
#[test]
fn the_applied_annotation_octets_are_annex_b() {
    let ast = model(IDL);
    let object = complete_type_object(&ast, type_of(&ast, "Sample")).expect("the object builds");
    let bytes = TypeObject::Complete(object)
        .to_bytes_le()
        .expect("the object encodes");
    let expected = unhex(SAMPLE);
    assert_eq!(bytes.len(), expected.len(), "{}", hex(&bytes));
    assert_eq!(bytes, expected, "got {}", hex(&bytes));
}

/// Each position of [`SAMPLE`], checked against Annex B on its own, so a
/// mutation is reported as the field that moved rather than only as a
/// whole-object mismatch.
#[test]
fn the_applied_annotation_is_where_annex_b_puts_it() {
    let octets = unhex(SAMPLE);
    let announced = |at: usize| {
        u32::from_le_bytes([octets[at], octets[at + 1], octets[at + 2], octets[at + 3]]) as usize
    };
    assert_eq!(announced(0), octets.len() - 4, "the object's own DHEADER");
    assert_eq!(&octets[4..6], &[0xF2, 0x51], "EK_COMPLETE, TK_STRUCTURE");
    assert_eq!(
        &octets[6..8],
        &[0x01, 0x00],
        "struct_flags with IS_FINAL at bit 0"
    );

    // The type applied nothing: `@optional ann_custom` is one absent octet
    // at 14, and the qualified name follows at 16.
    assert_eq!(announced(8), 15, "the CompleteStructHeader's DHEADER");
    assert_eq!(octets[12], 0x00, "base_type: TK_NONE");
    assert_eq!(octets[13], 0, "@optional ann_builtin absent");
    assert_eq!(octets[14], 0, "@optional ann_custom absent on the type");
    assert_eq!(
        u32::from_le_bytes([octets[16], octets[17], octets[18], octets[19]]),
        7
    );
    assert_eq!(&octets[20..27], b"Sample\0", "the qualified name");

    assert_eq!(
        announced(28),
        196,
        "the member sequence's DHEADER: three members"
    );
    assert_eq!(
        u32::from_le_bytes([octets[32], octets[33], octets[34], octets[35]]),
        3,
        "three members"
    );

    // `distance`, the annotated one.
    assert_eq!(announced(36), 78, "`distance`'s own DHEADER");
    assert_eq!(
        u32::from_le_bytes([octets[40], octets[41], octets[42], octets[43]]),
        0,
        "member_id 0: the declaration order, §7.3.1.2.1.1"
    );
    assert_eq!(octets[46], 0x0A, "TK_FLOAT64, and a primitive has no body");
    assert_eq!(&octets[52..61], b"distance\0");
    assert_eq!(octets[61], 0, "@optional ann_builtin absent");
    assert_eq!(
        octets[62], 1,
        "@optional ann_custom present: one application was written"
    );
    assert_eq!(
        announced(64),
        50,
        "the AppliedAnnotationSeq's DHEADER over a count and one element"
    );
    assert_eq!(
        u32::from_le_bytes([octets[68], octets[69], octets[70], octets[71]]),
        1,
        "one AppliedAnnotation"
    );
    assert_eq!(
        announced(72),
        42,
        "the AppliedAnnotation's own DHEADER (rule 29)"
    );
    assert_eq!(
        octets[76], 0xF2,
        "EK_COMPLETE: the hash form, since Annex B has no TK_ANNOTATION arm"
    );
    assert_eq!(octets[91], 1, "@optional param_seq present");

    assert_eq!(
        announced(92),
        22,
        "the AppliedAnnotationParameterSeq's DHEADER"
    );
    assert_eq!(
        u32::from_le_bytes([octets[96], octets[97], octets[98], octets[99]]),
        1,
        "one parameter"
    );
    assert_eq!(
        announced(100),
        14,
        "the AppliedAnnotationParameter's DHEADER over a NameHash and a string"
    );
    assert_eq!(
        &octets[104..108],
        NameHash::from_name("symbol").0,
        "the NameHash of the parameter name, and no name"
    );
    assert_eq!(
        octets[108], 0x20,
        "TK_STRING8: the value is the union Annex B declares"
    );
    assert_eq!(
        u32::from_le_bytes([octets[112], octets[113], octets[114], octets[115]]),
        2,
        "the string's own length, `string<ANNOTATION_STR_VALUE_MAX_LEN>`"
    );
    assert_eq!(&octets[116..118], b"m\0");

    // `plain`, which applies nothing: `ann_custom` absent is one octet,
    // and an empty sequence instead would move every octet after it.
    assert_eq!(announced(204), 20, "`plain`'s DHEADER");
    assert_eq!(&octets[220..226], b"plain\0");
    assert_eq!(octets[226], 0, "@optional ann_builtin absent");
    assert_eq!(
        octets[227], 0,
        "@optional ann_custom absent: no application, no sequence"
    );
}

/// The hash inside `distance`'s `annotation_typeid` is the MD5 of
/// `Unit`'s own object, prefix 14 — §7.3.4.6's rule for every
/// non-plain type, applied to an annotation declaration by §7.3.4.2's
/// Table 29 row for `TK_ANNOTATION`.
#[test]
fn the_typeid_carries_the_hash_of_the_declarations_own_object() {
    let ast = model(IDL);
    let octets = unhex(SAMPLE);
    let in_the_wire = &octets[77..91];
    let expected = complete_hash_of(&ast, annotation_of(&ast, "Unit"));
    assert_eq!(in_the_wire, expected.0, "the fourteen octets on the wire");

    // And it is the MD5 of those octets, recomputed here from the
    // declaration's object — the whole of §7.3.4.6 for this position.
    let object = annotation_type_object(&ast, annotation_of(&ast, "Unit"), Form::Complete)
        .expect("Unit's object");
    let to_bytes = object.to_bytes_le().expect("Unit's object encodes");
    assert_eq!(
        &zerodds_types::hash::hash_bytes(&to_bytes).0,
        in_the_wire,
        "§7.3.4.6: the first fourteen octets of the MD5 over the object"
    );
}

/// `symbol`'s value is the one the application gave, in the parameter's
/// own kind — Annex B's union, not a length-prefixed string of its
/// own and not the parameter's declared default.
#[test]
fn the_parameter_value_is_the_value_that_was_applied() {
    let ast = model(IDL);
    let object = through_the_wire(&ast);
    for (name, symbol) in [("distance", "m"), ("elapsed", "s")] {
        let sequence = custom(&object, name);
        let applications = applied(&sequence);
        assert_eq!(applications[0].param_seq.len(), 1, "`{name}`");
        assert_eq!(
            applications[0].param_seq[0].value,
            AnnotationParameterValue::String8(symbol.to_string()),
            "`{name}`"
        );
        assert_eq!(
            applications[0].param_seq[0].paramname_hash,
            NameHash::from_name("symbol"),
            "named by the hash of its name"
        );
    }
}

/// §7.3.4.5 orders `AnnotationParameterSeq` by increasing `paramname_hash`,
/// and `Unit`'s single parameter cannot show it. This one can: two
/// parameters whose name hashes sort the other way from their declaration
/// order.
///
/// `level` hashes to `c9e9a848` and `loud` to `51edf3fe`, so `loud` comes
/// first whatever order they are written in — the two names
/// `typeobject_annotation_annexb.rs` uses, whose hashes are recorded there.
#[test]
fn the_parameters_are_ordered_by_name_hash() {
    let ast = model(
        r#"
@annotation Verbosity {
    long level default 1;
    boolean loud default TRUE;
};

@final struct Sample {
    @Verbosity(level = 3, loud = FALSE) long noise;
};
"#,
    );
    let object = through_the_wire(&ast);
    let noise = custom(&object, "noise");
    let parameters = &applied(&noise)[0].param_seq;
    let hashes: Vec<[u8; 4]> = parameters.iter().map(|p| p.paramname_hash.0).collect();
    assert_eq!(
        hashes,
        vec![[0x51, 0xed, 0xf3, 0xfe], [0xc9, 0xe9, 0xa8, 0x48]],
        "increasing paramname_hash: `loud` before `level`"
    );
    assert_eq!(
        parameters[0].value,
        AnnotationParameterValue::Boolean(false)
    );
    assert_eq!(parameters[1].value, AnnotationParameterValue::Int32(3));
}

/// §7.3.4.5 orders `AppliedAnnotationSeq` by increasing
/// `annotation_typeid`, so two annotations on one member come out in hash
/// order rather than in the order they were written.
///
/// The two declarations are named so that the written order and the
/// required order differ: `@Bb` is written first and its hash is the
/// larger of the two, so a writer that kept the written order would put
/// `Aa` last and fail the first assertion.
#[test]
fn the_applications_are_ordered_by_annotation_typeid() {
    let ast = model(
        r#"
@annotation Aa { long a default 0; };
@annotation Bb { long b default 0; };

@final struct Sample {
    @Bb @Aa(7) long noise;
};
"#,
    );
    let object = through_the_wire(&ast);
    let noise = custom(&object, "noise");
    let applications = applied(&noise);
    assert_eq!(applications.len(), 2, "both applications");

    let read = |applications: &[AppliedAnnotation]| -> Vec<EquivalenceHash> {
        applications
            .iter()
            .map(|application| match &application.annotation_typeid {
                TypeIdentifier::EquivalenceHashComplete(hash) => *hash,
                other => panic!("not a complete hash: {other:?}"),
            })
            .collect()
    };
    let hashes = read(&applications);
    let mut ascending = hashes.clone();
    ascending.sort();
    assert_eq!(hashes, ascending, "increasing annotation_typeid: §7.3.4.5");

    // And the order is not the written one, which is the whole point.
    let written = vec![
        complete_hash_of(&ast, annotation_of(&ast, "Bb")),
        complete_hash_of(&ast, annotation_of(&ast, "Aa")),
    ];
    assert_ne!(
        hashes, written,
        "the sequence is not what the source wrote, or this test would pass on a \
         writer that kept the written order"
    );
    // Both declarations are the ones applied, whichever order they came
    // out in.
    let mut both = written.clone();
    both.sort();
    assert_eq!(hashes, both, "`@Aa` and `@Bb`, and nothing else");
}

/// A member with no application writes `ann_custom` absent — one octet —
/// and a member that applies one writes the presence octet. A writer that
/// wrote an empty sequence instead would move every octet after it.
#[test]
fn an_unapplied_member_writes_ann_custom_absent() {
    let ast = model(IDL);
    let object = through_the_wire(&ast);
    assert!(
        custom(&object, "plain").0.is_none(),
        "`plain` applies nothing, so the sequence is absent rather than empty"
    );
    assert!(custom(&object, "distance").0.is_some());
}

/// A declaration with no parameters is applied with an empty
/// `param_seq`, and `AppliedAnnotation::encode_into` writes that absent.
/// The parameter values of such a declaration are all defaults, and
/// `CompleteAnnotationParameter` carries them on the declaration itself —
/// which is what `annotation_typeid` points at.
#[test]
fn a_parameterless_annotation_is_written_without_a_param_seq() {
    let ast = model(
        r#"
@annotation Marker {};

@final struct Sample {
    @Marker long noise;
};
"#,
    );
    let object = through_the_wire(&ast);
    let noise = custom(&object, "noise");
    let applications = applied(&noise);
    assert_eq!(applications.len(), 1);
    assert!(
        applications[0].param_seq.is_empty(),
        "a declaration with no members, so nothing was given"
    );
}

/// §7.3.4.4 leaves user-defined annotations out of the minimal form, so
/// the minimal object of a member annotated `@Unit` is the object of a
/// member that is not.
///
/// This is not only §7.3.4.4's sentence: a minimal object carrying an
/// applied annotation would hash differently from every peer's, because
/// no peer writes one.
#[test]
fn the_minimal_form_leaves_the_applied_annotation_out() {
    let ast = model(IDL);
    let with = minimal_type_object(&ast, type_of(&ast, "Sample")).expect("minimal");
    let plain = model(
        r#"
@final struct Sample {
    double distance;
    long elapsed;
    long plain;
};
"#,
    );
    let without = minimal_type_object(&plain, type_of(&plain, "Sample")).expect("minimal");
    assert_eq!(
        with, without,
        "§7.3.4.4: the minimal form carries no user-defined annotation"
    );
}

/// The round trip the whole thing exists for: a model with an
/// application, its `CompleteTypeObject`, the octets, and a second model
/// rebuilt out of nothing but what a peer was sent — where the
/// application is on the member again, with its declaration and its
/// value.
#[test]
fn an_applied_annotation_comes_back_into_the_rebuilt_model() {
    let ast = model(IDL);
    let object = through_the_wire(&ast);
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");

    let rebuilt = dynamic_type(&object, &registry).expect("it rebuilds");
    let rebuilt_ast = rebuilt.ast().clone();
    let sample = type_of(&rebuilt_ast, &rebuilt.qualified_name());

    for (name, symbol) in [("distance", "m"), ("elapsed", "s")] {
        let node = member(&rebuilt_ast, sample, name);
        assert_eq!(
            node.facets.annotations.len(),
            1,
            "`{name}`: the application came back"
        );
        let application = &node.facets.annotations[0];
        assert_eq!(
            application.name, "Unit",
            "`{name}`: named as it was written"
        );
        // The declaration came back too — looked up by the hash the wire
        // carried, and rebuilt from the `CompleteAnnotationType` the
        // registry holds for it.
        let declaration = match application.definition {
            Ref::Resolved(id) => rebuilt_ast
                .annotation_def(id)
                .expect("the declaration the typeid named"),
            Ref::Pending(_) | Ref::Broken(_) => {
                panic!("`{name}`: the declaration did not come back")
            }
        };
        assert_eq!(declaration.name, "Unit");
        assert_eq!(
            declaration.members.len(),
            1,
            "`@annotation Unit` declares one member"
        );
        assert_eq!(
            application.arguments.len(),
            1,
            "`{name}`: one argument, the one that was given"
        );
        assert_eq!(
            application.arguments[0].name.as_deref(),
            Some("symbol"),
            "`{name}`: a parameter is named by its hash, so the name comes back"
        );
        assert_eq!(
            rebuilt_ast.eval_const(application.arguments[0].value),
            Ok(ConstValue::String(symbol.to_string())),
            "`{name}`: and its value"
        );
    }

    assert!(
        member(&rebuilt_ast, sample, "plain")
            .facets
            .annotations
            .is_empty(),
        "`plain` had no application and gains none"
    );
}

/// The written form of an application does not survive: `@Unit("m")` and
/// `@Unit(symbol = "m")` are one encoding under one `paramname_hash`, so
/// the short form comes back as the named one.
///
/// Stated as a fact about the specification rather than as a defect here:
/// §7.3.4.5 has one `AppliedAnnotationParameter` per given parameter and
/// names it by the hash of the parameter's name, which both forms write
/// the same.
#[test]
fn the_short_form_comes_back_as_the_named_one() {
    let ast = model(IDL);
    let object = through_the_wire(&ast);
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");
    let rebuilt = dynamic_type(&object, &registry).expect("it rebuilds");
    let rebuilt_ast = rebuilt.ast().clone();
    let sample = type_of(&rebuilt_ast, &rebuilt.qualified_name());
    // `distance` was written `@Unit("m")` — the short form.
    assert_eq!(
        member(&rebuilt_ast, sample, "distance").facets.annotations[0].arguments[0]
            .name
            .as_deref(),
        Some("symbol"),
        "the named form, which is the only one a parameter hash identifies"
    );
}

/// One declaration applied twice is one node in the rebuilt model. Two
/// `AnnotationDefNode`s of that name would be equal in every field and
/// not the same declaration, which is what
/// `Ast::find_by_scoped_name` reports as ambiguous.
#[test]
fn one_declaration_applied_twice_is_one_node() {
    let ast = model(IDL);
    let object = through_the_wire(&ast);
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");
    let rebuilt = dynamic_type(&object, &registry).expect("it rebuilds");
    let rebuilt_ast = rebuilt.ast().clone();
    let sample = type_of(&rebuilt_ast, &rebuilt.qualified_name());
    let declaration_of =
        |name: &str| match member(&rebuilt_ast, sample, name).facets.annotations[0].definition {
            Ref::Resolved(id) => id,
            Ref::Pending(_) | Ref::Broken(_) => panic!("`{name}`: no declaration came back"),
        };
    assert_eq!(
        declaration_of("distance"),
        declaration_of("elapsed"),
        "`@Unit` on two members is one declaration"
    );
    assert_eq!(
        rebuilt_ast
            .annotation_defs()
            .filter(|(id, _)| *id == declaration_of("distance"))
            .count(),
        1,
        "and it is declared once"
    );
}

/// An application on the **type**, not on a member: `@Unit("m")` in front
/// of `@final struct Sample` is one `AppliedAnnotation` in the
/// `CompleteTypeDetail`, and it comes back on the type node.
#[test]
fn an_annotation_applied_to_the_type_itself_reaches_the_object() {
    let ast = model(
        r#"
@annotation Unit { string symbol; };

@Unit("m") @final struct Sample {
    long distance;
};
"#,
    );
    let object = through_the_wire(&ast);
    let CompleteTypeObject::Struct(node) = &object else {
        panic!("not a struct object");
    };
    let applications = node
        .header
        .detail
        .ann_custom
        .0
        .as_ref()
        .expect("the type carries `ann_custom`");
    assert_eq!(applications.len(), 1);
    assert_eq!(
        applications[0].param_seq[0].value,
        AnnotationParameterValue::String8("m".to_string())
    );

    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");
    let rebuilt = dynamic_type(&object, &registry).expect("it rebuilds");
    let rebuilt_ast = rebuilt.ast().clone();
    let sample = type_of(&rebuilt_ast, &rebuilt.qualified_name());
    let node = rebuilt_ast.type_(sample).expect("the rebuilt struct");
    assert_eq!(
        node.facets.annotations.len(),
        1,
        "the application came back on the type"
    );
    assert_eq!(node.facets.annotations[0].name, "Unit");
    assert_eq!(
        member(&rebuilt_ast, sample, "distance")
            .facets
            .annotations
            .len(),
        0,
        "and it is the type's, not the member's"
    );
}

/// Every other position that carries a `CompleteMemberDetail`: a union
/// branch, an enumeration literal, a bitmask flag, a bitfield. Each is
/// written from its own node's facets and each is read back onto it.
#[test]
fn every_position_with_a_complete_member_detail_is_read() {
    // The declaration forms are the ones this project's IDL front end
    // accepts: `case 1: @Mark(1) long a;` rather than `@Mark(1) case 1:`
    // (IDL 4.2 §7.4.15 rule 218 puts the annotation before the
    // declaration, and a `case` label is not one), an enumerator's
    // annotation beside its `@value`, a bitmask bit's beside its
    // `@position`, and a bitset field's before `bitfield<N>`.
    let ast = model(
        r#"
@annotation Mark { long n default 0; };

@final union Choice switch (long) {
    case 1: @Mark(1) long a;
    default: long b;
};

@final enum Colour {
    @Mark(2) @value(3) RED,
    @value(4) GREEN
};

@bit_bound(8) bitmask Flags {
    @Mark(3) @position(0) A,
    @position(1) B
};

bitset Words {
    @Mark(4) bitfield<4> lo;
    bitfield<4> hi;
};
"#,
    );
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");

    // Each kind in turn: the object carries the applied annotation with
    // the value it was given, and the rebuilt model has it back on the
    // node it was written from.
    for (name, annotated, given) in [
        ("Choice", "a", 1_i32),
        ("Colour", "RED", 2),
        ("Flags", "A", 3),
        ("Words", "lo", 4),
    ] {
        let object = complete_type_object(&ast, type_of(&ast, name))
            .unwrap_or_else(|error| panic!("`{name}`: {error}"));
        let bytes = TypeObject::Complete(object)
            .to_bytes_le()
            .expect("the object encodes");
        let TypeObject::Complete(decoded) =
            TypeObject::from_bytes_le(&bytes).expect("the octets decode")
        else {
            panic!("`{name}`: a complete object came back in the minimal form");
        };
        assert_eq!(
            annotated_detail(&decoded, annotated),
            Some(AnnotationParameterValue::Int32(given)),
            "`{name}`: `@{annotated}` carries the value it was given"
        );

        let rebuilt = dynamic_type(&decoded, &registry).expect("it rebuilds");
        let rebuilt_ast = rebuilt.ast().clone();
        let id = type_of(&rebuilt_ast, &rebuilt.qualified_name());
        let applied_to = applied_names(&rebuilt_ast, id, annotated);
        assert_eq!(
            applied_to, 1,
            "`{name}`: the application came back on `{annotated}`"
        );
    }
}

/// The first parameter value of the applied annotation `node` carries in
/// a `CompleteTypeObject`, where `node` is the branch, literal, flag or
/// field of that name.
fn annotated_detail(object: &CompleteTypeObject, node: &str) -> Option<AnnotationParameterValue> {
    let detail = match object {
        CompleteTypeObject::Union(object) => object
            .member_seq
            .iter()
            .find(|member| member.detail.name == node)?
            .detail
            .ann_custom
            .clone(),
        CompleteTypeObject::Enumerated(object) => object
            .literal_seq
            .iter()
            .find(|literal| literal.detail.name == node)?
            .detail
            .ann_custom
            .clone(),
        CompleteTypeObject::Bitmask(object) => object
            .flag_seq
            .iter()
            .find(|flag| flag.detail.name == node)?
            .detail
            .ann_custom
            .clone(),
        CompleteTypeObject::Bitset(object) => object
            .field_seq
            .iter()
            .find(|field| field.detail.name == node)?
            .detail
            .ann_custom
            .clone(),
        other => panic!("no applied annotation on {other:?}"),
    };
    detail
        .0?
        .first()
        .map(|application| application.param_seq[0].value.clone())
}

/// How many applied annotations the rebuilt model puts on the branch,
/// literal, flag or field of `name` inside `type_`.
fn applied_names(ast: &Ast, type_: Id<TypeNode>, name: &str) -> usize {
    let node = ast.type_(type_).expect("the rebuilt type");
    match &node.kind {
        zerodds_ast::TypeKind::Union { cases, .. } => cases
            .iter()
            .filter_map(|id| ast.case(*id).ok())
            .filter_map(|case| ast.member(case.member).ok())
            .filter(|member| member.name == name)
            .map(|member| member.facets.annotations.len())
            .sum(),
        zerodds_ast::TypeKind::Enumeration { enumerators } => enumerators
            .iter()
            .filter_map(|id| ast.enumerator(*id).ok())
            .filter(|literal| literal.name == name)
            .map(|literal| literal.facets.annotations.len())
            .sum(),
        zerodds_ast::TypeKind::Bitmask { bits } => bits
            .iter()
            .filter_map(|id| ast.bit_value(*id).ok())
            .filter(|bit| bit.name == name)
            .map(|bit| bit.facets.annotations.len())
            .sum(),
        zerodds_ast::TypeKind::Bitset { fields, .. } => fields
            .iter()
            .filter_map(|id| ast.bitfield(*id).ok())
            .filter(|field| field.name.as_deref() == Some(name))
            .map(|field| field.facets.annotations.len())
            .sum(),
        other => panic!("`{name}`: unexpected kind {other:?}"),
    }
}

/// An application on a collection element is read back, not refused.
///
/// §7.3.4.5 gives `CompleteCollectionElement` an `ann_custom`, and
/// §7.2.2.6 attaches annotations to "an element/member/literal of an
/// aggregated/collection/enumerated type". The model keeps them on the
/// collection's node (`XTypesFacet::element_annotations`), so a
/// `CompleteSequenceType` a peer sends with an annotated element
/// rebuilds with the application on it. (Refusing it was the answer
/// while the model had no field for it.)
///
/// A `CompleteSequenceType` with an annotated element is assembled here
/// as a peer would send it.
#[test]
fn an_annotation_on_a_collection_element_is_read() {
    use zerodds_types::type_identifier::PrimitiveKind;
    use zerodds_types::type_object::common::CompleteTypeDetail;
    use zerodds_types::type_object::complete::{CompleteCollectionElement, CompleteSequenceType};
    use zerodds_types::type_object::flags::CollectionTypeFlag;
    use zerodds_types::type_object::minimal::CommonCollectionElement;
    let ast = model(
        r#"
@annotation Mark { long n default 0; };

@final struct Sample {
    long value;
};
"#,
    );
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");

    let mark = complete_hash_of(&ast, annotation_of(&ast, "Mark"));
    let object = CompleteTypeObject::Sequence(CompleteSequenceType {
        collection_flag: CollectionTypeFlag(0),
        bound: 0,
        detail: Some(CompleteTypeDetail {
            type_name: "Sample::Q".to_string(),
            ann_builtin: Default::default(),
            ann_custom: OptionalAppliedAnnotationSeq(None),
        }),
        element: CompleteCollectionElement {
            common: CommonCollectionElement {
                element_flags: Default::default(),
                type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
            },
            ann_builtin: Default::default(),
            ann_custom: OptionalAppliedAnnotationSeq(Some(vec![AppliedAnnotation {
                annotation_typeid: TypeIdentifier::EquivalenceHashComplete(mark),
                param_seq: Vec::new(),
            }])),
        },
    });
    let rebuilt = dynamic_type(&object, &registry).expect("it rebuilds");
    let rebuilt_ast = rebuilt.ast().clone();
    let id = type_of(&rebuilt_ast, &rebuilt.qualified_name());
    let on_element = rebuilt_ast.element_annotations(id);
    assert_eq!(on_element.len(), 1, "the application is on the element");
    assert_eq!(on_element[0].name, "Mark");
    assert!(
        rebuilt_ast
            .type_(id)
            .expect("the collection")
            .facets
            .annotations
            .is_empty(),
        "and it is the element's, not the collection's"
    );
}

/// An annotation the file does not declare is ignored, which is IDL 4.2
/// §7.4.15.4.2's own instruction — "in case an unknown annotation is
/// encountered, it should be ignored by the IDL-processing tools" — and
/// which is why a vendor directive does not stop a `TypeObject` from
/// being built.
#[test]
fn an_annotation_the_file_did_not_declare_is_ignored() {
    let ast = model(
        r#"
@final struct Sample {
    @RTI_topic_kind(NO_KEY) long noise;
    long plain;
};
"#,
    );
    let object = complete_type_object(&ast, type_of(&ast, "Sample"))
        .expect("an undeclared annotation does not stop the object");
    assert!(
        custom(&object, "noise").0.is_none(),
        "§7.4.15.4.2: `ann_custom` holds no application whose declaration \
         is not in the file — there is no `TypeObject` whose hash could \
         name it"
    );
}

/// The applied annotation refers to the declaration's `TypeObject`, so the
/// declaration is part of the type's dependency closure: an endpoint that
/// served the annotated type but not the annotation would announce a
/// dependency it cannot answer for (§7.6.3.3.4).
#[test]
fn the_annotation_declaration_is_in_the_dependency_closure() {
    let ast = model(
        r#"
@annotation Unit { string symbol; };

@final struct Sample {
    @Unit("m") double distance;
};
"#,
    );
    let objects = dependency_type_objects(&ast, type_of(&ast, "Sample")).expect("closure");
    let names: Vec<String> = objects
        .iter()
        .map(|object| match object {
            CompleteTypeObject::Annotation(annotation) => annotation.header.annotation_name.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(names, ["Unit"], "`Unit` is the closure of `Sample`");

    // Which is what makes the annotated type's own identity resolvable:
    // `register_all` filed the declaration under the hash the application
    // names.
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");
    let hash = complete_hash_of(&ast, annotation_of(&ast, "Unit"));
    assert!(
        registry.get_complete(&hash).is_some(),
        "`register_all` filed `Unit`'s object under its own EquivalenceHash"
    );
    assert!(
        registry.get_minimal(&hash).is_none(),
        "and under the complete hash alone: the minimal hash of the same \
         object is a different value, and §7.3.4.5's `annotation_typeid` \
         is the complete one"
    );
}

/// An annotation parameter of an enumerated type puts that enumeration in
/// the closure too: `CommonAnnotationParameter::member_type_id` is a
/// hash, and a hash the endpoint cannot serve is a dependency it cannot
/// answer for.
#[test]
fn an_enumerated_annotation_parameter_is_in_the_closure() {
    let ast = model(
        r#"
@annotation Mode {
    enum Kind { READ, WRITE };
    Kind kind default READ;
};

@final struct Sample {
    @Mode long noise;
};
"#,
    );
    let objects = dependency_type_objects(&ast, type_of(&ast, "Sample")).expect("closure");
    let mut names: Vec<String> = objects
        .iter()
        .map(|object| match object {
            CompleteTypeObject::Enumerated(enumeration) => {
                enumeration.header.detail.type_name.clone()
            }
            CompleteTypeObject::Annotation(annotation) => annotation.header.annotation_name.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["Kind", "Mode"],
        "the declaration, and the enumeration its parameter names"
    );
}

/// §7.2.2.6: "Annotations can also be attached to the related_type of an
/// alias type." An application on a `typedef` is one `AppliedAnnotation` in
/// the `CompleteTypeDetail` of the alias header, with the value it was given.
#[test]
fn an_annotation_on_a_typedef_reaches_the_alias_object() {
    let ast = model(
        r#"
@annotation Unit { string symbol; };

@Unit("m") typedef long Distance;

@final struct Sample {
    Distance d;
};
"#,
    );
    let object = complete_type_object(&ast, type_of(&ast, "Distance")).expect("the alias object");
    let CompleteTypeObject::Alias(alias) = object else {
        panic!("not an alias object");
    };
    let applications = alias
        .header
        .detail
        .ann_custom
        .0
        .as_ref()
        .expect("the alias carries `ann_custom`");
    assert_eq!(applications.len(), 1);
    assert_eq!(
        applications[0].param_seq[0].value,
        AnnotationParameterValue::String8("m".to_string())
    );
}
