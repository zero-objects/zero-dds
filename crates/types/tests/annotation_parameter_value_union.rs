// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `AnnotationParameterValue`, the union of DDS-XTypes 1.3 Annex B
//! (PDF p. 286): its cases `TK_FLOAT128`, `TK_CHAR16`, `TK_STRING16` and
//! the `default` case `ExtendedAnnotationParameterValue`, and §7.2.2.6
//! (p. 70), which lets an annotation parameter be of any primitive type
//! and of a string type of `Char8` or `Char16` elements.
//!
//! The octets are written out by hand from the clause: the union is
//! `@extensibility(FINAL)`, so the discriminator octet is followed by the
//! value at its own alignment, capped at four by XCDR2's `MAXALIGN`
//! (§7.4.3.2.2); the `default` case is an `@extensibility(MUTABLE)`
//! structure and so starts with a `DHEADER`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_ast::Ast;
use zerodds_types::from_ast::{Form, annotation_type_object};
use zerodds_types::type_object::common::AnnotationParameterValue as V;
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "values.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

/// `TK_CHAR16` (0x11) and a `wchar`: two octets, aligned to two, so one
/// octet of padding after the discriminator (Table 31, §7.4.1.1.1).
#[test]
fn a_wchar_value_is_tk_char16_and_two_octets() {
    let value = V::Char16('a');
    assert_eq!(value.kind(), 0x11);
    let bytes = value.to_bytes().unwrap();
    assert_eq!(bytes, [0x11, 0x00, 0x61, 0x00]);
    assert_eq!(V::from_bytes(&bytes).unwrap(), value);
}

/// `TK_STRING16` (0x21) and a `wstring`: a `UInt32` length equal to twice
/// the number of characters, then the UTF-16 code units, no terminator
/// (§7.4.1.1.2).
#[test]
fn a_wstring_value_is_tk_string16_with_an_octet_length() {
    let value = V::String16("ab".into());
    assert_eq!(value.kind(), 0x21);
    let bytes = value.to_bytes().unwrap();
    assert_eq!(
        bytes,
        [0x21, 0, 0, 0, 0x04, 0, 0, 0, 0x61, 0x00, 0x62, 0x00]
    );
    assert_eq!(V::from_bytes(&bytes).unwrap(), value);
}

/// `TK_FLOAT128` (0x0B) and a `long double`: sixteen octets of IEEE 754
/// binary128. 1.5 is sign 0, exponent 0x3FFF, the top fraction bit set.
#[test]
fn a_long_double_value_is_tk_float128_and_sixteen_octets() {
    let bits: u128 = 0x3FFF_8000_0000_0000_0000_0000_0000_0000;
    let value = V::Float128Bits(bits);
    assert_eq!(value.kind(), 0x0B);
    let bytes = value.to_bytes().unwrap();
    let mut expected = vec![0x0B, 0, 0, 0];
    expected.extend_from_slice(&[0; 13]);
    expected.extend_from_slice(&[0x80, 0xFF, 0x3F]);
    assert_eq!(bytes, expected);
    assert_eq!(V::from_bytes(&bytes).unwrap(), value);
}

/// A discriminator without a case is the `default` branch of the union,
/// `ExtendedAnnotationParameterValue`: `@extensibility(MUTABLE)` and
/// empty, so a `DHEADER` of zero. It is read and kept, not refused.
#[test]
fn an_unknown_discriminator_is_the_extended_value() {
    let bytes = [0x7F, 0, 0, 0, 0, 0, 0, 0];
    let value = V::from_bytes(&bytes).unwrap();
    assert_eq!(
        value,
        V::Extended {
            kind: 0x7F,
            body: Vec::new()
        }
    );
    assert_eq!(value.to_bytes().unwrap(), bytes);

    // A later version's members inside the DHEADER are kept as read.
    let with_members = [0x7F, 0, 0, 0, 0x04, 0, 0, 0, 1, 2, 3, 4];
    let value = V::from_bytes(&with_members).unwrap();
    assert_eq!(value.to_bytes().unwrap(), with_members);
}

/// A discriminator that has a case cannot be written as the extended one.
#[test]
fn an_extended_value_with_the_discriminator_of_a_case_is_refused() {
    let value = V::Extended {
        kind: 0x11,
        body: Vec::new(),
    };
    assert!(value.to_bytes().is_err());
}

/// §7.2.2.6: an annotation declaration with a `long double`, a `wchar`
/// and a `wstring` parameter builds a `CompleteTypeObject`, and each
/// default is the value the declaration wrote.
#[test]
fn an_annotation_with_long_double_wchar_and_wstring_parameters_builds() {
    let ast = model(
        r#"
@annotation Wide {
    long double d default 1.5;
    wchar c default L'x';
    wstring w default L"hi";
};
"#,
    );
    let definition = ast
        .annotation_defs()
        .find(|(_, node)| node.name == "Wide")
        .map(|(id, _)| id)
        .expect("Wide");
    let object = annotation_type_object(&ast, definition, Form::Complete).expect("builds");
    let bytes = object.to_bytes_le().unwrap();
    let TypeObject::Complete(CompleteTypeObject::Annotation(annotation)) =
        TypeObject::from_bytes_le(&bytes).unwrap()
    else {
        panic!("not a complete annotation object");
    };
    let default_of = |name: &str| {
        annotation
            .member_seq
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no parameter {name}"))
            .default_value
            .clone()
    };
    assert_eq!(
        default_of("d"),
        V::Float128Bits(0x3FFF_8000_0000_0000_0000_0000_0000_0000)
    );
    assert_eq!(default_of("c"), V::Char16('x'));
    assert_eq!(default_of("w"), V::String16("hi".into()));
}

/// Table 9 gives a parameter without `default` the default of its type;
/// for these three kinds that is zero, `'\0'` and the empty string.
#[test]
fn the_type_default_of_the_three_new_kinds_is_zero_nul_and_empty() {
    let ast = model(
        r#"
@annotation Plain {
    long double d;
    wchar c;
    wstring w;
};
"#,
    );
    let definition = ast
        .annotation_defs()
        .find(|(_, node)| node.name == "Plain")
        .map(|(id, _)| id)
        .expect("Plain");
    let TypeObject::Complete(CompleteTypeObject::Annotation(annotation)) =
        annotation_type_object(&ast, definition, Form::Complete).expect("builds")
    else {
        panic!("not a complete annotation object");
    };
    let default_of = |name: &str| {
        annotation
            .member_seq
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .default_value
            .clone()
    };
    assert_eq!(default_of("d"), V::Float128Bits(0));
    assert_eq!(default_of("c"), V::Char16('\0'));
    assert_eq!(default_of("w"), V::String16(String::new()));
}
