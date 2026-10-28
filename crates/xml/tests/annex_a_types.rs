// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//! The XML Type Representation of DDS-XTypes 1.3 (§7.3.2, Annex A) read by
//! `zerodds-xml`: the values the Annex A schema restricts, the namespace of
//! the schema, and the documents the schema accepts and rejects, with
//! `xmllint` as the second opinion.
//!
//! The schema is `schemas/dds-xtypes_type_definition.xsd`, the transcription
//! of Annex A (namespace `http://www.omg.org/dds`). `xmllint` is found by
//! running it; without it the tests that need it print `[skip]`.

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_xml::{
    AutoId, Extensibility, ReadOptions, TypeDef, TypeRef, XmlError, is_identifier_name,
    parse_type_libraries, parse_type_libraries_with,
};

const XT: &str = "http://www.omg.org/dds";

fn types(body: &str) -> Result<Vec<zerodds_xml::TypeLibrary>, XmlError> {
    parse_type_libraries(&format!("<types>{body}</types>"))
}

fn first_struct(body: &str) -> zerodds_xml::StructType {
    let libs = types(body).expect("the document parses");
    match &libs[0].types[0] {
        TypeDef::Struct(s) => s.clone(),
        other => panic!("not a struct: {other:?}"),
    }
}

// ---------------------------------------------------------------------
// §7.3.2.2: type and nonBasicTypeName
// ---------------------------------------------------------------------

/// §7.3.2.2: `type="nonBasic"` and `nonBasicTypeName` name a type that is
/// not a basic one, on a member, a typedef, a union discriminator, an
/// annotation member and a map key.
#[test]
fn a_non_basic_type_is_named_by_nonbasictypename_on_every_element_that_has_a_type() {
    let st = first_struct(
        r#"<struct name="S">
             <member name="a" type="nonBasic" nonBasicTypeName="::M::Other"/>
             <member name="m" type="int32" mapKeyType="nonBasic"
                     mapKeyNonBasicTypeName="Key" mapMaxLength="4"/>
           </struct>"#,
    );
    assert_eq!(st.members[0].type_ref, TypeRef::Named("::M::Other".into()));
    assert_eq!(st.members[1].map_key, Some(TypeRef::Named("Key".into())));

    let libs = types(
        r#"<typedef name="T" type="nonBasic" nonBasicTypeName="Other"/>
           <union name="U"><discriminator type="nonBasic" nonBasicTypeName="Kind"/>
             <case><caseDiscriminator value="A"/>
               <member name="x" type="nonBasic" nonBasicTypeName="Other"/></case>
           </union>
           <annotation name="Ann"><member name="p" type="nonBasic" nonBasicTypeName="Kind"/></annotation>"#,
    )
    .expect("parses");
    let TypeDef::Typedef(t) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(t.type_ref, TypeRef::Named("Other".into()));
    let TypeDef::Union(u) = &libs[0].types[1] else {
        panic!()
    };
    assert_eq!(u.discriminator, TypeRef::Named("Kind".into()));
    assert_eq!(u.cases[0].member.type_ref, TypeRef::Named("Other".into()));
    let TypeDef::Annotation(a) = &libs[0].types[2] else {
        panic!()
    };
    assert_eq!(a.members[0].type_ref, TypeRef::Named("Kind".into()));
}

/// §7.3.2.2: "It is an error to include this attribute if the type
/// attribute does not indicate a non-basic type."
#[test]
fn nonbasictypename_beside_a_basic_type_is_an_error() {
    for body in [
        r#"<struct name="S"><member name="a" type="int32" nonBasicTypeName="Other"/></struct>"#,
        r#"<struct name="S"><member name="a" type="string" nonBasicTypeName="Other"/></struct>"#,
        r#"<typedef name="T" type="int32" nonBasicTypeName="Other"/>"#,
        r#"<union name="U"><discriminator type="int32" nonBasicTypeName="K"/>
             <case><caseDiscriminator value="1"/><member name="x" type="int32"/></case></union>"#,
        r#"<annotation name="A"><member name="p" type="int32" nonBasicTypeName="K"/></annotation>"#,
        r#"<struct name="S"><member name="m" type="int32" mapKeyType="int32"
                  mapKeyNonBasicTypeName="K" mapMaxLength="2"/></struct>"#,
    ] {
        let error = types(body).expect_err(body);
        assert!(
            format!("{error:?}").contains("nonBasic"),
            "{body}: {error:?}"
        );
    }
}

// ---------------------------------------------------------------------
// Annex A: identifierName, fileName, memberId, bit bounds
// ---------------------------------------------------------------------

/// Annex A (S. 243): `identifierName` is `([a-zA-Z]|::)([a-zA-Z_0-9]|::)*`.
#[test]
fn identifier_name_is_the_pattern_of_annex_a() {
    for good in ["a", "A1", "my_name", "::M::T", "M::T", "a::", "::", "Z9_"] {
        assert!(is_identifier_name(good), "`{good}` matches the pattern");
    }
    for bad in [
        "", "1a", "_a", "a-b", "a b", "a.b", ":a", "a:b", "a:::b ", "é",
    ] {
        assert!(
            !is_identifier_name(bad),
            "`{bad}` does not match the pattern"
        );
    }
}

/// A `name`, a `baseType` and a `nonBasicTypeName` that are no
/// `identifierName` are errors that name the attribute.
#[test]
fn a_name_that_is_no_identifier_name_is_refused() {
    for body in [
        r#"<struct name="1S"/>"#,
        r#"<struct name="S" baseType="a-b"/>"#,
        r#"<struct name="S"><member name="a b" type="int32"/></struct>"#,
        r#"<struct name="S"><member name="a" type="nonBasic" nonBasicTypeName="x y"/></struct>"#,
        r#"<enum name="E"><enumerator name="1"/></enum>"#,
        r#"<bitmask name="B"><flag name="a.b" position="0"/></bitmask>"#,
        r#"<module name="m-1"/>"#,
        r#"<typedef name="_T" type="int32"/>"#,
        r#"<const name="C-1" type="int32" value="1"/>"#,
        r#"<annotation name="1A"/>"#,
        r#"<struct name="S"><annotate name="a b"/></struct>"#,
        r#"<forward_dcl name="F F" kind="struct"/>"#,
    ] {
        let error = types(body).expect_err(body);
        let text = format!("{error:?}");
        assert!(
            text.contains("identifierName"),
            "{body}: the error names the pattern: {text}"
        );
    }
    types(r#"<struct name="::M::S"><member name="a_1" type="int32"/></struct>"#)
        .expect("names that match the pattern are read");
}

/// Annex A (S. 244): `fileName` is `xs:string` with no restriction.
#[test]
fn a_file_name_is_any_string() {
    let libs = types(r#"<include file="dir with space/ä-é &amp; more.xml"/>"#).expect("parses");
    let TypeDef::Include(include) = &libs[0].types[0] else {
        panic!("not an include");
    };
    assert_eq!(include.file, "dir with space/ä-é & more.xml");
}

/// Annex A (S. 245): `memberId` is an `xs:int` from 0 to 0x0FFFFFFF.
#[test]
fn member_id_is_between_zero_and_0x0fffffff() {
    for ok in ["0", "7", "268435455"] {
        let st = first_struct(&format!(
            r#"<struct name="S"><member name="a" type="int32" id="{ok}"/></struct>"#
        ));
        assert_eq!(st.members[0].id, Some(ok.parse().unwrap()));
    }
    for bad in ["268435456", "4294967295", "-1"] {
        assert!(
            types(&format!(
                r#"<struct name="S"><member name="a" type="int32" id="{bad}"/></struct>"#
            ))
            .is_err(),
            "id={bad}"
        );
    }
}

/// Annex A (S. 271-272): `enumBitBound` 1..32, `bitmaskBitBound` 1..64,
/// `flagIndex` 0..63 and a flag's `position` is required.
#[test]
fn bit_bounds_and_flag_positions_are_within_their_ranges() {
    for (bound, ok) in [("1", true), ("32", true), ("0", false), ("33", false)] {
        let result = types(&format!(
            r#"<enum name="E" bitBound="{bound}"><enumerator name="A"/></enum>"#
        ));
        assert_eq!(result.is_ok(), ok, "enum bitBound={bound}: {result:?}");
    }
    for (bound, ok) in [("1", true), ("64", true), ("0", false), ("65", false)] {
        let result = types(&format!(
            r#"<bitmask name="B" bitBound="{bound}"><flag name="F" position="0"/></bitmask>"#
        ));
        assert_eq!(result.is_ok(), ok, "bitmask bitBound={bound}: {result:?}");
    }
    for (position, ok) in [("0", true), ("63", true), ("64", false)] {
        let result = types(&format!(
            r#"<bitmask name="B" bitBound="64"><flag name="F" position="{position}"/></bitmask>"#
        ));
        assert_eq!(result.is_ok(), ok, "flag position={position}: {result:?}");
    }
    assert!(
        types(r#"<bitmask name="B"><flag name="F"/></bitmask>"#).is_err(),
        "`position` is use=\"required\""
    );
}

// ---------------------------------------------------------------------
// Annex A: structs, members, unions, modules, the library
// ---------------------------------------------------------------------

/// `structDecl` (S. 268): annotate, verbatim, then members and constants,
/// `baseType`; `structMemberDecl` (S. 267): `id`, `optional`,
/// `mustUnderstand`, `nonSerialized`, `key`; `structOrUnionTypeDecl`:
/// `nested`, `extensibility`, `autoid`.
#[test]
fn a_struct_declaration_keeps_everything_annex_a_lets_it_say() {
    let st = first_struct(
        r#"<struct name="S" baseType="B" nested="true" extensibility="mutable" autoid="sequencial">
             <annotate name="Doc"><member name="text" value="x"/></annotate>
             <verbatim language="c++" placement="end-declaration"><text>// t</text></verbatim>
             <const name="N" type="int32" value="4"/>
             <member name="a" type="int32" id="5" optional="true" mustUnderstand="true"
                     nonSerialized="true" key="false"/>
             <member name="b" type="string" stringMaxLength="N" external="true"
                     tryConstruct="discard"/>
           </struct>"#,
    );
    assert_eq!(st.base_type.as_deref(), Some("B"));
    assert_eq!(st.nested, Some(true));
    assert_eq!(st.extensibility, Some(Extensibility::Mutable));
    assert_eq!(st.auto_id, Some(AutoId::Sequential));
    assert_eq!(st.annotations[0].name, "Doc");
    assert_eq!(st.verbatims[0].placement, "end-declaration");
    assert_eq!(st.constants[0].name, "N");
    let a = &st.members[0];
    assert_eq!(a.id, Some(5));
    assert!(a.optional && a.must_understand && !a.key);
    assert_eq!(a.non_serialized, Some(true));
    assert_eq!(st.members[1].string_max_length, Some(4));
    assert_eq!(st.members[1].external, Some(true));
}

/// `moduleDecl` (S. 275): `name` and `autoid`; `moduleElements` (S. 274):
/// include, forward_dcl, const, module, struct, union, annotation,
/// typedef, enum and bitmask in any order; `typeLibrary` is the same
/// group at the top.
#[test]
fn a_module_holds_every_module_element_in_any_order() {
    let libs = types(
        r#"<module name="M" autoid="hash">
             <include file="a.xml"/>
             <enum name="E"><enumerator name="A"/></enum>
             <forward_dcl name="S" kind="struct"/>
             <const name="C" type="int32" value="1"/>
             <struct name="S"><member name="a" type="int32"/></struct>
             <bitmask name="B"><flag name="F" position="1"/></bitmask>
             <typedef name="T" type="int32"/>
             <annotation name="Ann"/>
             <union name="U"><discriminator type="int32"/>
               <case><caseDiscriminator value="1"/><member name="x" type="int32"/></case></union>
             <module name="Inner"><struct name="X"/></module>
           </module>"#,
    )
    .expect("parses");
    let TypeDef::Module(m) = &libs[0].types[0] else {
        panic!("not a module");
    };
    assert_eq!(m.auto_id, Some(AutoId::Hash));
    let kinds: Vec<&str> = m
        .types
        .iter()
        .map(|t| match t {
            TypeDef::Include(_) => "include",
            TypeDef::Enum(_) => "enum",
            TypeDef::ForwardDcl(_) => "forward_dcl",
            TypeDef::Const(_) => "const",
            TypeDef::Struct(_) => "struct",
            TypeDef::Bitmask(_) => "bitmask",
            TypeDef::Typedef(_) => "typedef",
            TypeDef::Annotation(_) => "annotation",
            TypeDef::Union(_) => "union",
            TypeDef::Module(_) => "module",
            TypeDef::Bitset(_) => "bitset",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "include",
            "enum",
            "forward_dcl",
            "const",
            "struct",
            "bitmask",
            "typedef",
            "annotation",
            "union",
            "module"
        ]
    );
}

/// `annotationTypeDecl` (S. 266): `member` (a `simpleMemberDecl`), `name`
/// and `baseType`; `annotationDecl`/`annotationMemberValueDecl`: the
/// application with its member values.
#[test]
fn an_annotation_type_and_its_application_are_read() {
    let libs = types(
        r#"<annotation name="Base"><member name="level" type="int32"/></annotation>
           <annotation name="Wide" baseType="Base">
             <member name="tag" type="string"/>
             <member name="kind" type="nonBasic" nonBasicTypeName="Kind"/>
           </annotation>
           <struct name="S"><annotate name="Wide">
             <member name="tag" value="'x'"/><member name="flagOnly"/></annotate></struct>"#,
    )
    .expect("parses");
    let TypeDef::Annotation(wide) = &libs[0].types[1] else {
        panic!()
    };
    assert_eq!(wide.base_type.as_deref(), Some("Base"));
    assert_eq!(wide.members.len(), 2);
    assert_eq!(wide.members[1].type_ref, TypeRef::Named("Kind".into()));
    let TypeDef::Struct(s) = &libs[0].types[2] else {
        panic!()
    };
    assert_eq!(s.annotations[0].members[0].value.as_deref(), Some("'x'"));
    assert_eq!(s.annotations[0].members[1].value, None);
}

// ---------------------------------------------------------------------
// The namespace of Annex A
// ---------------------------------------------------------------------

/// Annex A declares `targetNamespace="http://www.omg.org/dds"` for the
/// element `types`; a document in that namespace is read, and the schema of
/// that namespace spells the middle extensibility `appendable` where DDS-XML
/// 1.0 spells it `append`. Each element is read by the schema of its own
/// namespace.
#[test]
fn a_document_in_the_xtypes_namespace_is_read_by_the_annex_a_schema() {
    let annex_a = format!(
        r#"<types xmlns="{XT}"><struct name="S" extensibility="appendable">
             <member name="a" type="int32"/></struct></types>"#
    );
    let libs = parse_type_libraries_with(&annex_a, &ReadOptions::default())
        .expect("appendable is Annex A's spelling in its own namespace")
        .value;
    let TypeDef::Struct(s) = &libs[0].types[0] else {
        panic!()
    };
    assert_eq!(s.extensibility, Some(Extensibility::Appendable));

    let wrong = annex_a.replace("appendable", "append");
    let error = parse_type_libraries(&wrong).expect_err("append is DDS-XML's spelling");
    assert!(format!("{error:?}").contains("appendable"), "{error:?}");

    // Without the namespace, or in the DDS-XML one, the DDS-XML schema reads.
    for namespace in ["", r#" xmlns="http://www.omg.org/spec/DDS-XML""#] {
        let dds_xml =
            format!(r#"<types{namespace}><struct name="S" extensibility="append"/></types>"#);
        parse_type_libraries(&dds_xml).expect("append in the DDS-XML schema");
        let other = dds_xml.replace("append", "appendable");
        assert!(
            parse_type_libraries(&other).is_err(),
            "appendable is a vendor form by default there"
        );
        assert!(
            parse_type_libraries_with(&other, &ReadOptions::accept_vendor_forms()).is_ok(),
            "and is read when vendor forms are accepted"
        );
    }
}

// ---------------------------------------------------------------------
// The schema as the second opinion
// ---------------------------------------------------------------------

fn xmllint() -> Option<String> {
    let prog = std::env::var("ZERODDS_XMLLINT").unwrap_or_else(|_| "xmllint".into());
    match Command::new(&prog).arg("--version").output() {
        Ok(out) if out.status.success() => Some(prog),
        _ => None,
    }
}

macro_rules! xmllint_or_skip {
    () => {
        match xmllint() {
            Some(p) => p,
            None => {
                eprintln!("[skip] xmllint not runnable (set ZERODDS_XMLLINT to override)");
                return;
            }
        }
    };
}

fn schemas() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas")
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join("zerodds-xml-annex-a");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// `Ok(())` when `xmllint` validates `xml` against Annex A's schema, else
/// its message. The schema includes its chameleon file by the absolute URL
/// of the OMG; a catalog maps that prefix onto the in-tree directory.
fn lint(prog: &str, name: &str, xml: &str) -> Result<(), String> {
    let dir = scratch();
    let catalog = dir.join("xtypes-catalog.xml");
    std::fs::write(
        &catalog,
        format!(
            r#"<?xml version="1.0"?>
<catalog xmlns="urn:oasis:names:tc:entity:xmlns:xml:catalog">
  <rewriteURI uriStartString="https://www.omg.org/spec/DDS-XTypes/20190301/" rewritePrefix="{d}/"/>
  <rewriteSystem systemIdStartString="https://www.omg.org/spec/DDS-XTypes/20190301/" rewritePrefix="{d}/"/>
</catalog>
"#,
            d = schemas().display()
        ),
    )
    .expect("catalog");
    let doc = dir.join(name);
    std::fs::write(&doc, xml).expect("document");
    let out = Command::new(prog)
        .env("XML_CATALOG_FILES", &catalog)
        .arg("--nonet")
        .arg("--noout")
        .arg("--schema")
        .arg(schemas().join("dds-xtypes_type_definition.xsd"))
        .arg(&doc)
        .output()
        .expect("xmllint runs");
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn assert_document_invalid(result: &Result<(), String>, what: &str) {
    match result {
        Ok(()) => panic!("{what}: the schema accepted it"),
        Err(message) => assert!(
            !message.contains("Schemas parser error") && !message.contains("failed to load"),
            "{what}: xmllint failed on the schema and not on the document:\n{message}"
        ),
    }
}

/// The oracle says no before anything trusts its yes.
#[test]
fn annex_a_schema_compiles_and_rejects_what_it_should() {
    let prog = xmllint_or_skip!();
    lint(
        &prog,
        "oracle_good.xml",
        &format!(r#"<types xmlns="{XT}"><struct name="S"><member name="a" type="int32"/></struct></types>"#),
    )
    .expect("a valid document validates");
    assert_document_invalid(
        &lint(
            &prog,
            "oracle_bad.xml",
            &format!(
                r#"<types xmlns="{XT}"><struct name="S"><member type="int32"/></struct></types>"#
            ),
        ),
        "a member without a name",
    );
}

/// Documents Annex A's schema accepts, read by `zerodds-xml`.
#[test]
fn documents_annex_a_accepts_are_read() {
    let prog = xmllint_or_skip!();
    let documents = [
        r#"<module name="M" autoid="hash">
             <include file="x.xml"/>
             <struct name="S" extensibility="appendable" nested="false" autoid="sequencial">
               <annotate name="Doc"><member name="text" value="x"/></annotate>
               <verbatim language="c++" placement="before-declaration"><text>//</text></verbatim>
               <member name="a" type="int32" id="1" key="true" optional="false"
                       mustUnderstand="true" nonSerialized="false" external="false"
                       tryConstruct="trim" stringMaxLength="8" sequenceMaxLength="4"
                       arrayDimensions="2,3"/>
               <member name="m" type="string" mapKeyType="int32" mapMaxLength="8"/>
             </struct>
             <union name="U" extensibility="final">
               <discriminator type="int32" key="false"/>
               <case><caseDiscriminator value="1"/><caseDiscriminator value="2"/>
                 <member name="x" type="int32"/></case>
               <case><caseDiscriminator value="default"/>
                 <member name="y" type="float64"/></case>
             </union>
             <enum name="E" bitBound="8">
               <enumerator name="A" value="1" defaultLiteral="true"/>
               <enumerator name="B"/>
             </enum>
             <bitmask name="B" bitBound="16"><flag name="F0" position="0"/></bitmask>
             <typedef name="T" type="nonBasic" nonBasicTypeName="M::S" sequenceMaxLength="-1"/>
             <const name="C" type="int32" value="42"/>
             <forward_dcl name="Fwd" kind="struct"/>
             <annotation name="Ann"><member name="p" type="int32"/></annotation>
             <module name="Inner"><struct name="X"><member name="a" type="uint8"/></struct></module>
           </module>"#,
        r#"<struct name="Top"><member name="a" type="int64"/></struct>
           <enum name="E"><enumerator name="A"/></enum>"#,
    ];
    for (n, body) in documents.iter().enumerate() {
        let xml = format!(r#"<types xmlns="{XT}">{body}</types>"#);
        lint(&prog, &format!("accepted{n}.xml"), &xml)
            .unwrap_or_else(|message| panic!("document {n}: the schema rejects it:\n{message}"));
        parse_type_libraries(&xml)
            .unwrap_or_else(|error| panic!("document {n}: zerodds-xml rejects it: {error:?}"));
    }
}

/// Documents Annex A's schema rejects are rejected by `zerodds-xml` too.
#[test]
fn documents_annex_a_rejects_are_rejected() {
    let prog = xmllint_or_skip!();
    let documents = [
        ("identifierName", r#"<struct name="1S"/>"#),
        (
            "member id above 0x0FFFFFFF",
            r#"<struct name="S"><member name="a" type="int32" id="268435456"/></struct>"#,
        ),
        (
            "enum bitBound 33",
            r#"<enum name="E" bitBound="33"><enumerator name="A"/></enum>"#,
        ),
        (
            "bitmask bitBound 65",
            r#"<bitmask name="B" bitBound="65"><flag name="F" position="0"/></bitmask>"#,
        ),
        (
            "flag position 64",
            r#"<bitmask name="B"><flag name="F" position="64"/></bitmask>"#,
        ),
        (
            "flag without position",
            r#"<bitmask name="B"><flag name="F"/></bitmask>"#,
        ),
        (
            "append instead of appendable",
            r#"<struct name="S" extensibility="append"/>"#,
        ),
        (
            "unknown tryConstruct",
            r#"<struct name="S"><member name="a" type="int32" tryConstruct="ignore"/></struct>"#,
        ),
        (
            "unknown type kind",
            r#"<struct name="S"><member name="a" type="integer"/></struct>"#,
        ),
        (
            "boolean that is not",
            r#"<struct name="S"><member name="a" type="int32" key="yes"/></struct>"#,
        ),
        (
            "enumerator without name",
            r#"<enum name="E"><enumerator value="1"/></enum>"#,
        ),
        (
            "struct member without type",
            r#"<struct name="S"><member name="a"/></struct>"#,
        ),
        (
            "an element the schema has not",
            r#"<struct name="S"><bogus/></struct>"#,
        ),
    ];
    for (what, body) in documents {
        let xml = format!(r#"<types xmlns="{XT}">{body}</types>"#);
        assert_document_invalid(&lint(&prog, "rejected.xml", &xml), what);
        assert!(
            parse_type_libraries(&xml).is_err(),
            "{what}: the schema rejects it and zerodds-xml reads it"
        );
    }
}
