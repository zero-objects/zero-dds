// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Smoke-Tests fuer den Python-Codegen — deckt struct mit primitives,
//! enum, sequence, string, nested module ab.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_python::{IdlPythonError, PythonGenOptions, generate_python_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_python_module(&ast, &PythonGenOptions::default()).expect("gen")
}

#[test]
fn struct_with_primitives_emits_idl_struct_and_dataclass() {
    let py = emit("struct Greeting { long id; string<128> text; };");
    assert!(py.contains("@idl_struct(typename=\"Greeting\")"), "{py}");
    assert!(py.contains("@dataclass"), "{py}");
    assert!(py.contains("class Greeting:"), "{py}");
    assert!(py.contains("    id: Int32"), "{py}");
    assert!(py.contains("    text: String"), "{py}");
}

#[test]
fn struct_with_unbounded_string_uses_string_brand() {
    let py = emit("struct M { string note; };");
    assert!(py.contains("note: String"));
}

#[test]
fn struct_with_wstring_uses_wstring_brand() {
    let py = emit("struct M { wstring note; };");
    assert!(py.contains("note: WString"));
}

#[test]
fn sequence_maps_to_typing_list() {
    let py = emit("struct Bag { sequence<long> ids; };");
    assert!(py.contains("from typing import List"), "{py}");
    assert!(py.contains("ids: List[Int32]"), "{py}");
}

#[test]
fn nested_sequence_emits_nested_list() {
    let py = emit("struct G { sequence<sequence<long>> grid; };");
    assert!(py.contains("grid: List[List[Int32]]"), "{py}");
}

#[test]
fn enum_emits_intenum_subclass() {
    let py = emit("enum Color { RED, GREEN, BLUE };");
    assert!(py.contains("from enum import IntEnum"), "{py}");
    assert!(py.contains("class Color(IntEnum):"), "{py}");
    assert!(py.contains("    RED = 0"), "{py}");
    assert!(py.contains("    GREEN = 1"), "{py}");
    assert!(py.contains("    BLUE = 2"), "{py}");
}

#[test]
fn module_nesting_yields_underscored_class_name_and_qualified_typename() {
    let py = emit("module M { struct Inner { long x; }; };");
    // Python-PSM-Konvention: flacher Klassenname M_Inner.
    assert!(py.contains("class M_Inner:"), "{py}");
    // Typename behaelt den vollen IDL-Scope mit ::.
    assert!(py.contains("typename=\"M::Inner\""), "{py}");
}

#[test]
fn python_keyword_in_field_name_gets_trailing_underscore() {
    let py = emit("struct K { boolean class; };");
    assert!(py.contains("class_: bool"), "{py}");
}

#[test]
fn boolean_maps_to_python_bool() {
    let py = emit("struct B { boolean active; };");
    assert!(py.contains("active: bool"), "{py}");
}

#[test]
fn floating_types_map_to_brands() {
    let py = emit("struct F { float f; double d; };");
    assert!(py.contains("f: Float32"));
    assert!(py.contains("d: Float64"));
}

#[test]
fn header_comment_is_emitted_as_python_comments() {
    let ast = zerodds_idl::parse("struct S { long x; };", &ParserConfig::default()).expect("parse");
    let opts = PythonGenOptions {
        header_comment: Some("Generated for ZeroDDS Pilot.\nDo not hand-edit.".into()),
    };
    let py = generate_python_module(&ast, &opts).expect("gen");
    assert!(py.contains("# Generated for ZeroDDS Pilot."));
    assert!(py.contains("# Do not hand-edit."));
}

// =============================================================================
// Phase 2 — typedef, exception, bitmask, bitset, union, struct inheritance
// =============================================================================

#[test]
fn typedef_emits_type_alias() {
    let py = emit("typedef long Temperature;");
    assert!(py.contains("from typing import"), "{py}");
    assert!(py.contains("TypeAlias"), "{py}");
    assert!(py.contains("Temperature: TypeAlias = Int32"), "{py}");
}

#[test]
fn typedef_sequence_yields_list_alias() {
    let py = emit("typedef sequence<long> Histogram;");
    assert!(py.contains("Histogram: TypeAlias = List[Int32]"), "{py}");
}

#[test]
fn exception_emits_dataclass_subclassing_exception() {
    let py = emit("exception NotFound { long code; string detail; };");
    assert!(py.contains("@idl_struct(typename=\"NotFound\")"), "{py}");
    assert!(py.contains("class NotFound(Exception):"), "{py}");
    assert!(py.contains("    code: Int32"), "{py}");
    assert!(py.contains("    detail: String"), "{py}");
}

#[test]
fn bitmask_emits_intflag_with_shifted_bits() {
    let py = emit("bitmask Permissions { READ, WRITE, EXEC };");
    assert!(py.contains("from enum import"), "{py}");
    assert!(py.contains("IntFlag"), "{py}");
    assert!(py.contains("class Permissions(IntFlag):"), "{py}");
    assert!(py.contains("    READ = 1 << 0"), "{py}");
    assert!(py.contains("    WRITE = 1 << 1"), "{py}");
    assert!(py.contains("    EXEC = 1 << 2"), "{py}");
}

#[test]
fn bitset_emits_alias_plus_bits_helper() {
    let py = emit(
        "bitset SensorFlags {
            bitfield<8> sensor_id;
            bitfield<2> quality;
        };",
    );
    assert!(py.contains("SensorFlags: TypeAlias = Int64"), "{py}");
    assert!(py.contains("class SensorFlags_Bits:"), "{py}");
    assert!(py.contains("SENSOR_ID_SHIFT = 0"), "{py}");
    assert!(py.contains("SENSOR_ID_WIDTH = 8"), "{py}");
    assert!(py.contains("SENSOR_ID_MASK  = 0xff"), "{py}");
    assert!(py.contains("QUALITY_SHIFT = 8"), "{py}");
    assert!(py.contains("QUALITY_WIDTH = 2"), "{py}");
    assert!(py.contains("QUALITY_MASK  = 0x3"), "{py}");
}

#[test]
fn union_with_integer_switch_emits_idl_union_factory() {
    let py = emit(
        "union Payload switch (long) {
            case 0: long as_int;
            case 1: string as_str;
        };",
    );
    assert!(py.contains("Payload = idl_union("), "{py}");
    assert!(py.contains("typename=\"Payload\""), "{py}");
    assert!(py.contains("discriminator=Int32"), "{py}");
    assert!(py.contains("0: (\"as_int\", Int32)"), "{py}");
    assert!(py.contains("1: (\"as_str\", String)"), "{py}");
    assert!(py.contains("default=None"), "{py}");
}

#[test]
fn union_with_default_case_emits_default_arg() {
    let py = emit(
        "union Payload switch (long) {
            case 0: long known;
            default: string fallback;
        };",
    );
    assert!(py.contains("default=(\"fallback\", String)"), "{py}");
}

#[test]
fn union_with_negative_label_supported() {
    let py = emit(
        "union Sign switch (long) {
            case -1: long neg;
            case 1: long pos;
        };",
    );
    assert!(py.contains("-1: (\"neg\", Int32)"), "{py}");
    assert!(py.contains("1: (\"pos\", Int32)"), "{py}");
}

#[test]
fn union_with_hex_label_supported() {
    let py = emit(
        "union Tag switch (long) {
            case 0xff: long high;
            case 0x00: long low;
        };",
    );
    assert!(py.contains("255: (\"high\", Int32)"), "{py}");
    assert!(py.contains("0: (\"low\", Int32)"), "{py}");
}

#[test]
fn union_with_scoped_label_returns_unsupported() {
    // Scoped-Labels (z.B. enum-member-references) sind in einer Folge-
    // Iteration geplant — heute Unsupported.
    let src = "
        enum Tag { A, B };
        union U switch (Tag) {
            case A: long a;
            case B: long b;
        };
    ";
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let err = generate_python_module(&ast, &PythonGenOptions::default()).unwrap_err();
    assert!(matches!(err, IdlPythonError::Unsupported(_)));
}

#[test]
fn struct_inheritance_emits_subclass() {
    let py = emit(
        "struct Base { long id; };
         struct Derived : Base { long extra; };",
    );
    assert!(py.contains("class Base:"), "{py}");
    assert!(py.contains("class Derived(Base):"), "{py}");
    assert!(py.contains("    extra: Int32"), "{py}");
}

#[test]
fn empty_struct_with_base_still_has_pass() {
    let py = emit(
        "struct Base { long id; };
         struct Marker : Base {};",
    );
    assert!(py.contains("class Marker(Base):"), "{py}");
    // Body braucht mindestens `pass` damit Python-Syntax valid bleibt.
    assert!(py.contains("    pass"), "{py}");
}

#[test]
fn module_nesting_works_for_phase_2_constructs() {
    let py = emit(
        "module ns {
            typedef long Temperature;
            exception NotFound { long code; };
            bitmask Flags { A, B };
        };",
    );
    assert!(py.contains("ns_Temperature: TypeAlias = Int32"), "{py}");
    assert!(py.contains("class ns_NotFound(Exception):"), "{py}");
    assert!(py.contains("class ns_Flags(IntFlag):"), "{py}");
}

#[test]
fn interface_and_valuetype_still_unsupported() {
    let src = "interface Foo { void bar(); };";
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let err = generate_python_module(&ast, &PythonGenOptions::default()).unwrap_err();
    assert!(matches!(err, IdlPythonError::Unsupported(_)));
}

#[test]
fn fixed_type_still_unsupported() {
    let src = "struct M { fixed<5,2> price; };";
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let err = generate_python_module(&ast, &PythonGenOptions::default()).unwrap_err();
    assert!(matches!(err, IdlPythonError::Unsupported(_)));
}
