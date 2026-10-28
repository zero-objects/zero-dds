// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The rules of Annex A of DDS-XTypes 1.3 that need more than reading an
//! attribute, for the XML loader: `nonBasicTypeName`, `identifierName`,
//! `annotationTypeDecl/@baseType` and `moduleDecl/@autoid`.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use zerodds_ast::{AnyId, MemberIdOrigin, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_xml_compose::{Composed, compose_source};

fn load(body: &str) -> Composed {
    compose_source(
        "t.xml",
        &format!("<dds><types><module name=\"T\">{body}</module></types></dds>"),
    )
}

fn type_in(composed: &Composed, path: &[&str]) -> Id<TypeNode> {
    composed
        .ast
        .find_by_scoped_name(&ScopedName::absolute(
            path.iter().map(|s| (*s).to_owned()).collect(),
        ))
        .ok()
        .and_then(AnyId::as_type)
        .unwrap_or_else(|| panic!("{path:?} is declared"))
}

fn messages(composed: &Composed) -> String {
    composed
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

/// §7.3.2.2: `type="nonBasic"` and `nonBasicTypeName` name a non-basic
/// type; "it is an error to include this attribute if the type attribute
/// does not indicate a non-basic type".
#[test]
fn nonbasictypename_names_a_type_only_beside_nonbasic() {
    let composed = load(
        "<struct name=\"Other\"><member name=\"x\" type=\"int32\"/></struct>\
         <struct name=\"S\"><member name=\"o\" type=\"nonBasic\" nonBasicTypeName=\"Other\"/>\
         </struct>\
         <typedef name=\"T2\" type=\"nonBasic\" nonBasicTypeName=\"Other\"/>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);

    for body in [
        "<struct name=\"S\"><member name=\"a\" type=\"int32\" nonBasicTypeName=\"Other\"/></struct>",
        "<struct name=\"S\"><member name=\"a\" type=\"string\" nonBasicTypeName=\"Other\"/></struct>",
        "<typedef name=\"X\" type=\"int32\" nonBasicTypeName=\"Other\"/>",
    ] {
        let composed = load(&format!(
            "<struct name=\"Other\"><member name=\"x\" type=\"int32\"/></struct>{body}"
        ));
        assert!(composed.failed(), "{body}");
        assert!(
            messages(&composed).contains("nonBasicTypeName"),
            "{body}: {}",
            messages(&composed)
        );
    }
}

/// Annex A (S. 243): a name that does not match `identifierName` is an
/// error that names the attribute and the element.
#[test]
fn a_name_that_is_no_identifier_name_is_an_error() {
    for body in [
        "<struct name=\"1S\"><member name=\"a\" type=\"int32\"/></struct>",
        "<struct name=\"S\"><member name=\"a-b\" type=\"int32\"/></struct>",
        "<enum name=\"E\"><enumerator name=\"A B\"/></enum>",
        "<bitmask name=\"B\"><flag name=\"x.y\" position=\"0\"/></bitmask>",
        "<typedef name=\"_T\" type=\"int32\"/>",
        "<struct name=\"S\" baseType=\"a b\"><member name=\"a\" type=\"int32\"/></struct>",
    ] {
        let composed = load(body);
        assert!(composed.failed(), "{body}");
        assert!(
            messages(&composed).contains("identifierName"),
            "{body}: {}",
            messages(&composed)
        );
    }
    let composed = load("<struct name=\"S_1\"><member name=\"a_2\" type=\"int32\"/></struct>");
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
}

/// Annex A (S. 266): `annotationTypeDecl/@baseType`. The members of the base
/// are the first members of the annotation that names it.
#[test]
fn an_annotation_with_a_base_type_has_the_members_of_its_base_first() {
    let composed = load(
        "<annotation name=\"Base\"><member name=\"level\" type=\"int32\"/></annotation>\
         <annotation name=\"Wide\" baseType=\"Base\"><member name=\"tag\" type=\"string\"/>\
         </annotation>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let members: Vec<String> = composed
        .ast
        .annotation_defs()
        .find(|(_, node)| node.name == "Wide")
        .map(|(_, node)| {
            node.members
                .iter()
                .map(|m| composed.ast.annotation_member(*m).unwrap().name.clone())
                .collect()
        })
        .expect("the annotation");
    assert_eq!(members, ["level", "tag"]);
}

/// A base no annotation of the document names is an error.
#[test]
fn an_annotation_base_that_is_not_declared_is_an_error() {
    let composed = load(
        "<annotation name=\"Wide\" baseType=\"Nowhere\"><member name=\"tag\" type=\"string\"/>\
         </annotation>",
    );
    assert!(composed.failed());
    assert!(
        messages(&composed).contains("baseType"),
        "{}",
        messages(&composed)
    );
}

/// Annex A (S. 275): `moduleDecl/@autoid` governs the aggregated types of
/// the module that carry none of their own (§7.3.1.2.1.1).
#[test]
fn a_module_autoid_governs_the_types_declared_in_it() {
    let hashed = |composed: &Composed, name: &str| {
        let id = type_in(composed, &["T", name]);
        let member = composed.ast.members_of(id)[0];
        composed
            .ast
            .member(member)
            .unwrap()
            .facets
            .xtypes
            .as_ref()
            .and_then(|x| x.member_id.clone())
            .expect("an id was assigned")
    };
    let composed = compose_source(
        "t.xml",
        "<dds><types>\
         <module name=\"T\" autoid=\"hash\">\
           <struct name=\"Inherits\"><member name=\"a\" type=\"int32\"/></struct>\
           <struct name=\"Own\" autoid=\"sequencial\"><member name=\"a\" type=\"int32\"/></struct>\
         </module>\
         <module name=\"Plain\"><struct name=\"S\"><member name=\"a\" type=\"int32\"/></struct></module>\
         </types></dds>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    assert_eq!(
        hashed(&composed, "Inherits").origin,
        MemberIdOrigin::AutoHash
    );
    assert_eq!(
        hashed(&composed, "Own").origin,
        MemberIdOrigin::AutoSequential,
        "a type's own autoid wins over its module's"
    );
    let plain = type_in(&composed, &["Plain", "S"]);
    let member = composed.ast.members_of(plain)[0];
    let origin = composed
        .ast
        .member(member)
        .unwrap()
        .facets
        .xtypes
        .as_ref()
        .and_then(|x| x.member_id.clone())
        .expect("an id")
        .origin;
    assert_eq!(origin, MemberIdOrigin::PreviousPlusOne);

    // A bad value is an error.
    let composed = compose_source(
        "t.xml",
        "<dds><types><module name=\"T\" autoid=\"random\"/></types></dds>",
    );
    assert!(composed.failed());
}
