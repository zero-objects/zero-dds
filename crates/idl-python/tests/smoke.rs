// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Smoke tests for the Python codegen — covers struct with primitives,
//! enum, sequence, string, nested module.

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
    // SX2: unannotated struct defaults to @appendable (§7.3.3.1).
    assert!(
        py.contains("@idl_struct(typename=\"Greeting\", extensibility=\"appendable\")"),
        "{py}"
    );
    assert!(py.contains("@dataclass"), "{py}");
    assert!(py.contains("class Greeting:"), "{py}");
    assert!(py.contains("    id: Int32"), "{py}");
    // `string<128>` carries a bound, so it emits the bound-enforcing
    // `BoundedString[128]` brand (an over-bound write raises at runtime).
    assert!(py.contains("    text: BoundedString[128]"), "{py}");
}

#[test]
fn explicit_final_struct_omits_extensibility_kwarg() {
    // An EXPLICIT @final emits NO DHEADER, so the kwarg is omitted (runtime
    // default). Post-SX2 an UNANNOTATED struct instead defaults to @appendable.
    let py = emit("@final struct S { long a; };");
    assert!(py.contains("@idl_struct(typename=\"S\")"), "{py}");
    assert!(!py.contains("extensibility="), "{py}");
}

#[test]
fn appendable_struct_emits_extensibility_kwarg() {
    // `@appendable` drives the top-level + nested DHEADER (XTypes 1.3
    // §7.4.3.5.3 rule(30)); the runtime needs the `extensibility=` kwarg to
    // frame it. Cross-PSM XCDR2 convergence (Bug XW) depends on this.
    let py = emit("@appendable struct S { long a; };");
    assert!(
        py.contains("@idl_struct(typename=\"S\", extensibility=\"appendable\")"),
        "{py}"
    );
}

#[test]
fn mutable_struct_emits_extensibility_kwarg() {
    let py = emit("@mutable struct S { long a; };");
    // A @mutable struct also carries its per-member id list (PL_CDR2 EMHEADER
    // ids, XTypes 1.3 §7.4.3.4.2). With no explicit @id the SEQUENTIAL default
    // assigns id 0 to the first member.
    assert!(
        py.contains("@idl_struct(typename=\"S\", extensibility=\"mutable\", member_ids=[0])"),
        "{py}"
    );
}

#[test]
fn extensibility_annotation_form_emits_kwarg() {
    let py = emit("@extensibility(APPENDABLE) struct S { long a; };");
    assert!(
        py.contains("@idl_struct(typename=\"S\", extensibility=\"appendable\")"),
        "{py}"
    );
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
    // The typename keeps the full IDL scope with ::.
    assert!(py.contains("typename=\"M::Inner\""), "{py}");
}

#[test]
fn python_keyword_in_field_name_gets_trailing_underscore() {
    let py = emit("struct K { boolean class; };");
    // `boolean` maps to the runtime `Bool` brand (importable from zerodds.idl);
    // the reserved field name `class` gets the trailing underscore.
    assert!(py.contains("class_: Bool"), "{py}");
}

#[test]
fn boolean_maps_to_python_bool() {
    let py = emit("struct B { boolean active; };");
    // Emit the `Bool` brand, not the builtin `bool` — `from zerodds.idl import
    // bool` does not exist, whereas `Bool` is a real 1-byte XCDR2 boolean kind.
    assert!(py.contains("active: Bool"), "{py}");
    assert!(
        py.contains("from zerodds.idl import idl_struct, Bool"),
        "{py}"
    );
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
    // Holder width = smallest unsigned int fitting the SUM of bitfield widths
    // (8+2 = 10 bits → uint16); the `Bitset[N]` brand selects it (XTypes 1.3
    // §7.4.13). A plain `Int64` alias would put 8 bytes on the wire.
    assert!(py.contains("SensorFlags: TypeAlias = Bitset[10]"), "{py}");
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
fn union_with_scoped_enum_labels_resolve() {
    // Scoped labels (enum-member references) resolve to the discriminant the
    // generated IntEnum assigns (Bug I — previously Unsupported).
    let py = emit(
        "enum Tag { A, B };
         union U switch (Tag) {
            case A: long a;
            case B: long b;
        };",
    );
    assert!(py.contains("0: (\"a\""), "A must resolve to 0:\n{py}");
    assert!(py.contains("1: (\"b\""), "B must resolve to 1:\n{py}");
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
    // The body needs at least `pass` so the Python syntax stays valid.
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
fn fixed_type_emits_fixed_brand() {
    // fixed<P,S> now generates the `Fixed[P,S]` runtime brand (CORBA-BCD codec),
    // not a hard error. The decimal value is a `str` on the Python side.
    let src = "struct M { fixed<5,2> price; };";
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let py = generate_python_module(&ast, &PythonGenOptions::default()).expect("gen");
    assert!(
        py.contains("Fixed[5, 2]"),
        "Fixed[5, 2] annotation missing:\n{py}"
    );
    assert!(py.contains("Fixed"), "Fixed import missing");
}

#[test]
fn any_type_still_unsupported() {
    // `any` (CORBA TypeCode + dynamic value) still has no Python wire codec.
    let src = "struct M { any value; };";
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let err = generate_python_module(&ast, &PythonGenOptions::default()).unwrap_err();
    assert!(matches!(err, IdlPythonError::Unsupported(_)));
}

/// Bug I: a union with an enum discriminator must emit, with `case ENUM_LIT:`
/// labels resolved to the discriminant the generated IntEnum assigns.
#[test]
fn union_with_enum_discriminator_resolves_case_labels() {
    let py = emit(
        "module conf { \
           enum Kind { K_A, K_B, K_C }; \
           union EnumUnion switch (Kind) { \
             case K_A: long a; \
             case K_B: short b; \
             default: octet other; \
           }; \
         };",
    );
    assert!(py.contains("idl_union("), "{py}");
    // Discriminator reference resolves to the flattened class name (the enum is
    // inside `module conf`, so it is emitted as `conf_Kind`) — Bug Q-cluster.
    assert!(py.contains("discriminator=conf_Kind"), "{py}");
    // K_A → 0, K_B → 1 (matching emit_enum).
    assert!(py.contains("0: (\"a\""), "K_A must resolve to 0:\n{py}");
    assert!(py.contains("1: (\"b\""), "K_B must resolve to 1:\n{py}");
    assert!(py.contains("default=(\"other\""), "{py}");
}

/// Bug K: map<K,V> emits a `Dict[K, V]` annotation + the typing/brand imports.
#[test]
fn map_emits_dict_annotation() {
    let py = emit("struct M { map<string, long> scores; };");
    assert!(py.contains("from typing import Dict"), "{py}");
    assert!(py.contains("scores: Dict[String, Int32]"), "{py}");
}

// =============================================================================
// Bug R5 (#64) — @optional members must be wrapped in the Optional brand.
// =============================================================================

#[test]
fn optional_member_wrapped_in_optional_brand() {
    let py = emit(
        "module conf { struct Optionals { \
            long required; @optional long maybe; @optional string note; \
        }; };",
    );
    // Required member stays plain.
    assert!(py.contains("required: Int32"), "{py}");
    // @optional members are wrapped in the runtime Optional brand.
    assert!(py.contains("maybe: Optional[Int32]"), "{py}");
    assert!(py.contains("note: Optional[String]"), "{py}");
    // The Optional brand is imported from zerodds.idl.
    assert!(
        py.contains("from zerodds.idl import") && py.contains("Optional"),
        "Optional must be imported:\n{py}"
    );
}

// =============================================================================
// Bug M (#56) — const declarations emit module-level Python constants.
// =============================================================================

#[test]
fn const_int_emits_module_constant() {
    let py = emit("const long MAX_ITEMS = 10;");
    assert!(py.contains("MAX_ITEMS = 10"), "{py}");
}

#[test]
fn const_in_module_is_flattened() {
    let py = emit("module conf { const long N = 4; };");
    assert!(py.contains("conf_N = 4"), "{py}");
}

#[test]
fn const_bool_string_float_emit_python_literals() {
    let py = emit(
        "const boolean ENABLED = TRUE; \
         const double RATE = 2.5; \
         const string NAME = \"abc\";",
    );
    assert!(py.contains("ENABLED = True"), "bool:\n{py}");
    assert!(py.contains("RATE = 2.5"), "float:\n{py}");
    assert!(py.contains("NAME = \"abc\""), "string:\n{py}");
}

#[test]
fn const_arithmetic_is_folded() {
    let py = emit("const long A = 3; const long B = A * 2 + 1;");
    assert!(py.contains("A = 3"), "{py}");
    assert!(py.contains("B = 7"), "B must fold to 7:\n{py}");
}

// =============================================================================
// Bug Q-cluster (#60) — module flattening of references, fixed arrays, char/
// wstring brands.
// =============================================================================

#[test]
fn intra_module_reference_uses_flattened_name() {
    // A member referencing another type in the SAME module must emit the
    // flattened class name (conf_Inner), not the bare IDL name.
    let py = emit(
        "module conf { \
            struct Inner { long x; }; \
            struct Outer { Inner nested; }; \
        };",
    );
    assert!(py.contains("class conf_Inner:"), "{py}");
    assert!(
        py.contains("nested: conf_Inner"),
        "reference must be flattened to conf_Inner:\n{py}"
    );
}

#[test]
fn union_discriminator_and_member_use_flattened_names() {
    let py = emit(
        "module combo { \
            enum Mode { MODE_IDLE, MODE_ACTIVE }; \
            union Reading switch (Mode) { \
                case MODE_IDLE: long idleTicks; \
                default: double activeRate; \
            }; \
            struct Telemetry { Mode mode; Reading reading; }; \
        };",
    );
    // Discriminator reference inside the union is flattened.
    assert!(
        py.contains("discriminator=combo_Mode"),
        "discriminator:\n{py}"
    );
    // Struct members referencing the enum + union are flattened.
    assert!(py.contains("mode: combo_Mode"), "enum member:\n{py}");
    assert!(py.contains("reading: combo_Reading"), "union member:\n{py}");
}

#[test]
fn typedef_member_reference_is_flattened() {
    let py = emit(
        "module combo { \
            typedef double CurrentInAmpsType; \
            struct S { CurrentInAmpsType battery; }; \
        };",
    );
    assert!(py.contains("battery: combo_CurrentInAmpsType"), "{py}");
}

#[test]
fn sequence_of_local_struct_is_flattened() {
    let py = emit(
        "module combo { \
            struct Sample { long seq; }; \
            struct S { sequence<Sample> history; }; \
        };",
    );
    assert!(py.contains("history: List[combo_Sample]"), "{py}");
}

#[test]
fn fixed_array_member_uses_array_brand_not_list() {
    let py = emit("struct A { long window[4]; };");
    assert!(
        py.contains("window: Array[Int32, 4]"),
        "fixed array must use the Array brand:\n{py}"
    );
    assert!(
        py.contains("from zerodds.idl import") && py.contains("Array"),
        "Array brand must be imported:\n{py}"
    );
}

#[test]
fn multidim_array_nests_array_brand() {
    let py = emit("struct A { long grid[4][4]; double cube[2][2][2]; };");
    assert!(
        py.contains("grid: Array[Array[Int32, 4], 4]"),
        "grid:\n{py}"
    );
    assert!(
        py.contains("cube: Array[Array[Array[Float64, 2], 2], 2]"),
        "cube:\n{py}"
    );
}

#[test]
fn char_and_wstring_brands_are_emitted() {
    let py = emit("struct C { char c; wstring w; };");
    assert!(py.contains("c: Char"), "{py}");
    assert!(py.contains("w: WString"), "{py}");
}

// =============================================================================
// Bound enforcement — bounded sequence/string/wstring/map emit the
// bound-checking runtime brand instead of the unbounded annotation, so an
// over-bound write raises at runtime rather than silently corrupting.
// =============================================================================

#[test]
fn bounded_sequence_emits_sequence_brand_with_bound() {
    let py = emit("struct S { sequence<long, 3> nums; };");
    assert!(py.contains("nums: Sequence[Int32, 3]"), "{py}");
    assert!(
        py.contains("from zerodds.idl import") && py.contains("Sequence"),
        "Sequence brand must be imported:\n{py}"
    );
}

#[test]
fn unbounded_sequence_stays_typing_list() {
    let py = emit("struct S { sequence<long> nums; };");
    assert!(py.contains("nums: List[Int32]"), "{py}");
}

#[test]
fn bounded_string_emits_bounded_string_brand() {
    let py = emit("struct S { string<5> name; };");
    assert!(py.contains("name: BoundedString[5]"), "{py}");
    assert!(
        py.contains("from zerodds.idl import") && py.contains("BoundedString"),
        "BoundedString brand must be imported:\n{py}"
    );
}

#[test]
fn bounded_wstring_emits_bounded_wstring_brand() {
    let py = emit("struct S { wstring<3> wname; };");
    assert!(py.contains("wname: BoundedWString[3]"), "{py}");
    assert!(
        py.contains("from zerodds.idl import") && py.contains("BoundedWString"),
        "BoundedWString brand must be imported:\n{py}"
    );
}

#[test]
fn bounded_map_emits_map_brand_with_bound() {
    let py = emit("struct S { map<string, long, 2> counters; };");
    assert!(py.contains("counters: Map[String, Int32, 2]"), "{py}");
    assert!(
        py.contains("from zerodds.idl import") && py.contains("Map"),
        "Map brand must be imported:\n{py}"
    );
}

#[test]
fn unbounded_map_stays_typing_dict() {
    let py = emit("struct S { map<string, long> counters; };");
    assert!(py.contains("counters: Dict[String, Int32]"), "{py}");
}

#[test]
fn bounded_sequence_by_named_const_folds_bound() {
    // `sequence<T, CONST>` must fold the const to the integer bound.
    let py = emit("const long N = 4; struct S { sequence<long, N> nums; };");
    assert!(py.contains("nums: Sequence[Int32, 4]"), "{py}");
}
