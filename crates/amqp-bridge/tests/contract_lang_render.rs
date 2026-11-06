// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-amqp-bridge` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-render` (`src/codegen/emit.rs`):
//! `cpp::decl` for the carrier struct and the free helper functions,
//! `java::type_decl` for the carrier class and the codec class, and
//! `typescript::decl` for the carrier interface and the helper functions of
//! the DDS-AMQP 1.0 §8.1 codecs it generates. It relies on the carrier's
//! fields being public members in each language and on a keyword-named
//! field arriving under the name `escape_keyword` gave it.
//!
//! Oracle: ISO C++17 [class]/9 (a class-specifier ends in `;`; a struct's
//! members are public), the Java Language Specification §8.1.1 and §8.3.1
//! (`public final class`; a `public` field), and the TypeScript handbook's
//! interface declaration (`export interface Name { field: type; }`, an
//! optional property marked `?`).
#![allow(clippy::unwrap_used)]

use zerodds_lang_cst::common::{Doc, Ident, Path};
use zerodds_lang_cst::{cpp, java, typescript};

#[test]
fn a_cpp_carrier_is_a_struct_with_public_members_ended_by_a_semicolon() {
    let text = zerodds_lang_render::cpp::decl(&cpp::Decl::Class(cpp::Class {
        doc: Doc::none(),
        is_struct: true,
        name: Ident::new("Reading"),
        bases: Vec::new(),
        sections: vec![cpp::Section {
            access: cpp::Access::Public,
            members: vec![cpp::ClassMember::Field(cpp::Field {
                doc: Doc::none(),
                type_: cpp::Type::named(Path::new(&["int32_t"])),
                name: Ident::new("value"),
                dims: Vec::new(),
                default: None,
                is_static: false,
                is_constexpr: false,
                bits: None,
            })],
        }],
    }));
    assert!(text.contains("struct Reading"), "{text}");
    assert!(text.contains("int32_t value;"), "{text}");
    assert!(text.trim_end().ends_with("};"), "{text}");
}

#[test]
fn a_java_codec_class_is_public_final_and_a_carrier_field_is_public() {
    let text = zerodds_lang_render::java::type_decl(&java::TypeDecl::Class(java::Class {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public, java::Modifier::Final],
        name: Ident::new("ReadingAmqpCodec"),
        generics: Vec::new(),
        extends: None,
        implements: Vec::new(),
        members: vec![java::Member::Field(java::Field {
            doc: Doc::none(),
            annotations: Vec::new(),
            modifiers: vec![java::Modifier::Public],
            type_: java::Type::named(Path::single("int")),
            name: Ident::new(&java::escape_keyword("class")),
            value: None,
        })],
    }));
    assert!(
        text.contains("public final class ReadingAmqpCodec"),
        "{text}"
    );
    assert!(text.contains("public int "), "{text}");
    assert!(
        !text.contains("public int class;"),
        "a keyword is escaped: {text}"
    );
}

#[test]
fn a_typescript_carrier_is_an_exported_interface_with_an_optional_property() {
    let text = zerodds_lang_render::typescript::decl(&typescript::Decl::Interface(
        typescript::Interface {
            doc: Doc::none(),
            exported: true,
            name: Ident::new("Reading"),
            generics: Vec::new(),
            extends: Vec::new(),
            members: vec![typescript::Signature::Property(typescript::Property {
                doc: Doc::none(),
                readonly: false,
                name: typescript::PropertyName::ident("label"),
                optional: true,
                type_: typescript::Type::named(Path::single("string")),
            })],
        },
    ));
    assert!(text.contains("export interface Reading"), "{text}");
    assert!(text.contains("label?: string"), "{text}");
}
