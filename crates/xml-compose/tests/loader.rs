// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the loader does with a document that is not a clean pair with an
//! IDL file: the constructs only DDS-XML can write, the two spellings the
//! corpus uses for one construct, and the things it refuses rather than
//! carries.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, TypeKind};
use zerodds_xml_compose::{
    ComposeOptions, KeyOnUnionDiscriminator, Severity, compose_all_sources, compose_source,
    compose_source_with,
};

fn type_named<'a>(ast: &'a Ast, name: &str) -> &'a zerodds_ast::TypeNode {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(_, node)| node)
        .unwrap_or_else(|| panic!("no type named `{name}`"))
}

/// A member whose `nonBasicTypeName` names nothing is an error, and the
/// type it is in is still in the model. Dropping the member quietly is
/// the failure this asserts against: a struct that reaches the wire one
/// field short reports as a decode error at the peer, which is the
/// hardest place to see it from.
#[test]
fn a_type_name_that_resolves_to_nothing_is_reported() {
    let composed = compose_source(
        "x.xml",
        r#"<dds><types><module name="M">
             <struct name="S" extensibility="final">
               <member name="a" type="nonBasic" nonBasicTypeName="Absent"/>
             </struct>
           </module></types></dds>"#,
    );
    assert!(composed.failed(), "an absent type name must be an error");
    assert!(
        composed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("resolve")),
        "{:#?}",
        composed.diagnostics
    );
}

const KEYED_DISCRIMINATOR: &str = r#"<dds><types><module name="M">
             <union name="U"><discriminator type="uint32" key="true"/>
               <case><caseDiscriminator value="1"/><member name="x" type="int16"/></case>
             </union>
           </module></types></dds>"#;

/// DDS-XTypes 1.3 Table 21 lists `@key` for "Structure Members, Union
/// discriminator" and §7.2.2.4.4.4.8 writes `union MyCommand switch (@key
/// CommandKind)`, so the document loads by default.
#[test]
fn a_key_on_a_union_discriminator_is_carried_by_default() {
    let composed = compose_source("x.xml", KEYED_DISCRIMINATOR);
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
}

/// `KeyOnUnionDiscriminator::Refused`, for a tool that must stay within
/// what the measured toolchains accept, refuses it by name and by clause,
/// as the IDL front end does under the same setting.
#[test]
fn a_key_on_a_union_discriminator_is_refused_under_the_refusing_setting_and_says_why() {
    let composed = compose_source_with(
        "x.xml",
        KEYED_DISCRIMINATOR,
        ComposeOptions {
            key_on_union_discriminator: KeyOnUnionDiscriminator::Refused,
            ..ComposeOptions::default()
        },
    );
    assert!(composed.failed());
    let found = composed
        .diagnostics
        .iter()
        .find(|d| d.spec_reference.contains("Table 21"))
        .unwrap_or_else(|| panic!("{:#?}", composed.diagnostics));
    assert_eq!(found.severity, Severity::Error);
    assert!(found.message.contains("union discriminator"), "{found}");
}

/// The corpus writes a bitmask's bits two ways: the OMG `dds-xtypes`
/// suite writes `<flag name="…" value="…"/>` and DDS-XML's own Types
/// building block writes `<bit_value name="…" position="…"/>`. Both name
/// the same construct and both have to reach the same model, or half the
/// corpus loads with an empty bitmask.
#[test]
fn both_bitmask_spellings_reach_the_same_bits() {
    let flags = compose_source(
        "flags.xml",
        r#"<dds><types><module name="M">
             <bitmask name="B" bitBound="16">
               <flag name="F1" value="0"/><flag name="F2" value="3"/>
             </bitmask>
           </module></types></dds>"#,
    );
    let values = compose_source(
        "values.xml",
        r#"<dds><types><module name="M">
             <bitmask name="B" bitBound="16">
               <bit_value name="F1" position="0"/><bit_value name="F2" position="3"/>
             </bitmask>
           </module></types></dds>"#,
    );
    assert!(!flags.failed(), "{:#?}", flags.diagnostics);
    assert!(!values.failed(), "{:#?}", values.diagnostics);

    for composed in [&flags, &values] {
        let (id, _) = composed
            .ast
            .types()
            .find(|(_, node)| node.name == "B")
            .expect("the bitmask");
        let bits = composed.ast.bits_of(id);
        assert_eq!(bits.len(), 2);
        assert_eq!(composed.ast.effective_bit_position(bits[1]), Some(3));
        assert_eq!(composed.ast.effective_bit_bound(id), 16);
    }
}

/// `LENGTH_UNLIMITED` is written `-1` and `0`, and the suite's own
/// `sequences.xml` comments it "unlimited (0 or -1)". Both have to reach
/// the unbounded sequence, not a sequence bounded at zero — which would
/// be a collection that can hold nothing. `-1` is the clause's spelling;
/// `0` is the suite's, and is read as unbounded only when the caller says
/// so (`ComposeOptions::zero_bound_is_unbounded`).
#[test]
fn a_length_unlimited_bound_is_an_unbounded_collection() {
    for written in ["-1", "0"] {
        let composed = compose_source_with(
            "x.xml",
            &format!(
                r#"<dds><types><module name="M">
                     <struct name="S" extensibility="final">
                       <member name="a" type="int32" sequenceMaxLength="{written}"/>
                     </struct>
                   </module></types></dds>"#
            ),
            ComposeOptions {
                zero_bound_is_unbounded: true,
                ..ComposeOptions::default()
            },
        );
        assert!(!composed.failed(), "{:#?}", composed.diagnostics);
        let node = type_named(&composed.ast, "S");
        let TypeKind::Structure { members, .. } = &node.kind else {
            panic!("not a struct")
        };
        let member = composed.ast.member(members[0]).expect("the member");
        let element = composed
            .ast
            .type_(member.type_.resolved().expect("resolved"))
            .expect("the sequence");
        match &element.kind {
            TypeKind::Sequence { bound, .. } => assert_eq!(
                *bound,
                zerodds_ast::Bound::Unbounded,
                "`{written}` is LENGTH_UNLIMITED"
            ),
            other => panic!("expected a sequence, got {}", other.as_str()),
        }
    }
}

/// Two documents that both open `module Test` produce **one** module
/// node, which is what the OMG suite's ten files need: every one of them
/// declares its types in `Test`, and two separate models would emit that
/// module twice.
#[test]
fn two_documents_reopen_one_module() {
    let composed = compose_all_sources(
        &[
            (
                String::from("a.xml"),
                String::from(
                    r#"<dds><types><module name="Test">
                         <struct name="First" extensibility="final">
                           <member name="a" type="int32"/>
                         </struct>
                       </module></types></dds>"#,
                ),
            ),
            (
                String::from("b.xml"),
                String::from(
                    r#"<dds><types><module name="Test">
                         <struct name="Second" extensibility="final">
                           <member name="b" type="nonBasic" nonBasicTypeName="First"/>
                         </struct>
                       </module></types></dds>"#,
                ),
            ),
        ],
        ComposeOptions::default(),
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let modules: Vec<&str> = composed
        .ast
        .modules()
        .map(|(_, node)| node.name.as_str())
        .filter(|name| *name == "Test")
        .collect();
    assert_eq!(modules.len(), 1, "one module node, not two");
    // The second document's member resolves against the first
    // document's type.
    let b = type_named(&composed.ast, "Second");
    let TypeKind::Structure { members, .. } = &b.kind else {
        panic!("not a struct")
    };
    let member = composed.ast.member(members[0]).expect("the member");
    assert!(
        member.type_.is_resolved(),
        "a name written in one document resolves to a type declared in another"
    );
}

/// A declaration this loader does not know is an error naming the
/// element, not a silent omission.
#[test]
fn a_declaration_with_no_model_node_is_named_rather_than_dropped() {
    let composed = compose_source(
        "x.xml",
        r#"<dds><types><module name="M">
             <widget name="Bits"/>
           </module></types></dds>"#,
    );
    assert!(composed.failed());
    assert!(
        composed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("<widget>")),
        "{:#?}",
        composed.diagnostics
    );
}

/// The caller's default extensibility fills silence and does not
/// overrule the document — the same distinction
/// `zerodds_idl_compose::ComposeOptions` draws for IDL.
#[test]
fn the_default_extensibility_only_reaches_types_the_document_left_open() {
    use zerodds_ast::Extensibility;
    let source = r#"<dds><types><module name="M">
        <struct name="Said" extensibility="final"><member name="a" type="int32"/></struct>
        <struct name="Silent"><member name="a" type="int32"/></struct>
      </module></types></dds>"#;
    let composed = zerodds_xml_compose::compose_source_with(
        "x.xml",
        source,
        ComposeOptions {
            default_extensibility: Some(Extensibility::Mutable),
            ..ComposeOptions::default()
        },
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let of = |name: &str| {
        let (id, _) = composed
            .ast
            .types()
            .find(|(_, node)| node.name == name)
            .expect("the type");
        composed.ast.effective_extensibility(id)
    };
    assert_eq!(of("Said"), Extensibility::Final, "the document said final");
    assert_eq!(of("Silent"), Extensibility::Mutable, "the option filled it");

    // Without the option, silence stays silence and the specification's
    // own default applies.
    let bare = compose_source("x.xml", source);
    let (id, _) = bare
        .ast
        .types()
        .find(|(_, node)| node.name == "Silent")
        .expect("the type");
    assert_eq!(
        bare.ast.effective_extensibility(id),
        Extensibility::Appendable,
        "XTypes 1.3 §7.3.1.2.1.8"
    );
}

/// A document that is not well-formed is an error naming the document,
/// not an empty model that silently declares nothing.
#[test]
fn a_malformed_document_is_an_error_and_not_an_empty_model() {
    let composed = compose_source("x.xml", "<dds><types><module name=\"M\">");
    assert!(composed.failed(), "{:#?}", composed.diagnostics);
}

/// A union whose discriminator is a **bitmask**. DDS-XTypes 1.3 §7.5.2.6
/// admits a bitmask wherever it admits an enumeration for a
/// discriminator, and the OMG `dds-xtypes` suite declares two such
/// unions; OMG IDL 4.2 §7.4.1.4.4.3 rule (50) admits no bitmask in
/// `<switch_type_spec>` at all, so this construct is reachable only
/// through XML and `tests/idl_parity.rs` has no pair for it.
///
/// Until this loader existed, `Ast`'s case-label invariant reported
/// every such union as `IncompatibleCaseLabel` and the model refused
/// its cases outright — the union reached the wire with no branches.
#[test]
fn a_bitmask_discriminated_union_keeps_its_cases() {
    let composed = compose_source(
        "x.xml",
        r#"<dds><types><module name="M">
             <bitmask name="B_16" bitBound="16">
               <flag name="F1" value="0"/><flag name="F2" value="1"/>
             </bitmask>
             <union name="union_bitmask16" extensibility="appendable">
               <discriminator type="nonBasic" nonBasicTypeName="B_16"/>
               <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
               <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
             </union>
           </module></types></dds>"#,
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let (id, _) = composed
        .ast
        .types()
        .find(|(_, node)| node.name == "union_bitmask16")
        .expect("the union");
    assert_eq!(
        composed.ast.cases_of(id).len(),
        2,
        "both cases are in the model"
    );
    let TypeKind::Union { discriminator, .. } = &composed.ast.type_(id).expect("the union").kind
    else {
        panic!("not a union")
    };
    let disc = composed
        .ast
        .type_(discriminator.resolved().expect("resolved"))
        .expect("the discriminator");
    assert!(matches!(disc.kind, TypeKind::Bitmask { .. }));

    // And it has a `TypeIdentifier`, which is what a peer compares.
    let identifier = zerodds_types::from_ast::type_identifier(
        &composed.ast,
        id,
        zerodds_types::from_ast::Form::Complete,
    );
    assert!(identifier.is_ok(), "{identifier:?}");
}

/// DDS-XTypes 1.3 §7.3.2.4 with IDL 4.2 §7.4.1.4.4.3.1: a bound is a
/// positive integer or "-1"; zero is refused unless the caller reads the
/// suite's spelling.
#[test]
fn a_bound_of_zero_is_refused_by_default() {
    let source = r#"<dds><types><module name="M">
         <struct name="S"><member name="a" type="int32" sequenceMaxLength="0"/>
         <member name="text" type="string" stringMaxLength="0"/></struct>
       </module></types></dds>"#;
    let composed = compose_source("x.xml", source);
    assert!(composed.failed());
    assert_eq!(
        composed
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("bound of zero"))
            .count(),
        2,
        "{:#?}",
        composed.diagnostics
    );
    let lenient = compose_source_with(
        "x.xml",
        source,
        ComposeOptions {
            zero_bound_is_unbounded: true,
            ..ComposeOptions::default()
        },
    );
    assert!(!lenient.failed(), "{:#?}", lenient.diagnostics);
}
