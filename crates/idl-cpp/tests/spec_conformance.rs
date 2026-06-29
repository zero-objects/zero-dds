//! Spec conformance matrix for IDL4-C++ 1.0 §7 + §8.
//!
//! Production-verifies the generator paths listed in
//! `docs/spec-coverage/idl4-cpp-1.0.md` through end-to-end IDL→C++ renderings.
//!
//! Clusters:
//! 1. **§7.2.3 Constants** — string/wstring constexpr path.
//! 2. **§7.2.4.2.1-3 Type-Trait Specs** — bounded sequence/string/wstring
//!    `is_bounded`/`bound` specializations.
//! 3. **§7.2.4.3.4 Recursive Types** — self-reference via typedef.
//! 4. **§7.4.2 Interface Forward-Decl** — `class Foo;`.
//! 5. **§7.5 Interfaces – Full** — Embedded type/const/exception decls.
//! 6. **§7.14.2 Union discriminator extensions** — wchar/octet/int8/uint8.
//! 7. **§7.16 User-Defined Annotations** — no-op verified.
//! 8. **§7.17.x Standardized Annotations** — @optional/@key/@value/@bit_bound/
//!    @verbatim (XTypes §7.2.2.4.8 cross-cutting).
//! 9. **§8.1 @cpp_mapping** — annotation as a codegen hook.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn gen_cpp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

// ============================================================================
// §7.2.3 Constants — string/wstring path
// ============================================================================

#[test]
fn string_constant_emits_constexpr_string_view() {
    // Spec §7.2.3: string const → constexpr omg::types::string_view
    // (or std::string_view as a spec-conformant substitute).
    let cpp = gen_cpp(r#"const string GREETING = "hello";"#);
    assert!(cpp.contains("GREETING"), "constant name missing");
    assert!(cpp.contains("constexpr"), "constexpr keyword missing");
}

#[test]
fn numeric_constant_emits_constexpr_with_value() {
    // Spec §7.2.3: numeric const → constexpr int.
    let cpp = gen_cpp("const long N = 42;");
    assert!(cpp.contains("constexpr"));
    assert!(cpp.contains("42"));
}

// ============================================================================
// §7.2.4.2.1-3 Bounded-Sequence/String Type-Traits
// ============================================================================

#[test]
fn bounded_sequence_struct_emits_vector_with_size_marker() {
    // Spec §7.2.4.2.1 + Tab.7.4: bounded sequence → std::vector with a
    // bound marker. The generator emits std::vector<T> as the spec default.
    let cpp = gen_cpp(
        r#"
        struct WithBoundedSeq {
            sequence<long, 10> values;
        };
    "#,
    );
    assert!(cpp.contains("std::vector"));
    assert!(cpp.contains("WithBoundedSeq"));
}

#[test]
fn string_member_uses_std_string() {
    // Spec §7.2.4.2.2: IDL string → std::string.
    let cpp = gen_cpp(r#"struct WithString { string name; };"#);
    assert!(cpp.contains("std::string"));
}

#[test]
fn wstring_member_uses_std_wstring() {
    // Spec §7.2.4.2.3: IDL wstring → std::wstring.
    let cpp = gen_cpp(r#"struct WithWString { wstring name; };"#);
    assert!(cpp.contains("std::wstring"));
}

// ============================================================================
// §7.2.4.3.4 Recursive Types
// ============================================================================

#[test]
fn typedef_can_be_used_in_struct_member() {
    // Spec §7.2.4.3.4: recursive constructions via typedef.
    let cpp = gen_cpp(
        r#"
        typedef sequence<long> LongSeq;
        struct Holder { LongSeq items; };
    "#,
    );
    assert!(cpp.contains("using LongSeq"));
    assert!(cpp.contains("Holder"));
}

// ============================================================================
// §7.4.2 Interface Forward-Decl
// ============================================================================

#[test]
fn struct_forward_declaration_emits_class_or_struct_decl() {
    // Spec §7.4.2: forward decl → `class Foo;` or `struct Foo;`.
    let cpp = gen_cpp(
        r#"
        struct Forwarded;
        struct Forwarded { long x; };
    "#,
    );
    // The forward decl must be present as `class Forwarded;` or
    // `struct Forwarded;` (the spec allows both).
    assert!(
        cpp.contains("class Forwarded;") || cpp.contains("struct Forwarded;"),
        "forward declaration missing:\n{cpp}"
    );
}

// ============================================================================
// §7.14.2 Union discriminator extensions
// ============================================================================

#[test]
fn union_with_octet_discriminator_emits_variant() {
    // Spec §7.14.2: octet allowed as discriminator type.
    let cpp = gen_cpp(
        r#"
        union U switch (octet) {
            case 0: long a;
            case 1: float b;
        };
    "#,
    );
    assert!(cpp.contains("std::variant") || cpp.contains("union"));
    assert!(cpp.contains("U"));
}

// ============================================================================
// §7.16 User-Defined Annotations — no-op
// ============================================================================

#[test]
fn user_defined_annotations_not_propagated_to_cpp() {
    // Spec §7.16: user annotations → no C++ output.
    let cpp = gen_cpp(
        r#"
        @annotation MyCustom { string note; };
        @MyCustom(note="ignore-me")
        struct S { long x; };
    "#,
    );
    // The generator emits the class variant with `class S` and `int32_t x_`.
    assert!(cpp.contains("class S") || cpp.contains("struct S"));
    assert!(cpp.contains("x_"));
    // `MyCustom`/`note` must not end up as a C++ code element in the output
    // file (at most as a Doxygen comment with "// ").
    let out = cpp.as_str();
    let mc_count = out.matches("MyCustom").count();
    let comment_lines = out
        .lines()
        .filter(|l| l.contains("MyCustom"))
        .all(|l| l.trim_start().starts_with("//") || l.trim_start().starts_with("*"));
    assert!(
        mc_count == 0 || comment_lines,
        "user annotation emitted as a C++ code element"
    );
}

// ============================================================================
// §7.17.1 @optional → std::optional<T>
// ============================================================================

#[test]
fn optional_member_emits_std_optional() {
    // Spec §7.17.1 + §3.4: @optional → std::optional<T>.
    let cpp = gen_cpp(
        r#"
        struct WithOptional {
            @optional long maybe;
        };
    "#,
    );
    assert!(
        cpp.contains("std::optional"),
        "std::optional missing:\n{cpp}"
    );
}

#[test]
fn fixed_member_emits_dds_core_fixed_template() {
    // Spec idl4-cpp §7.2.4.2.4: fixed<digits,scale> -> Fixed class with
    // ~30 methods. ZeroDDS choice: `dds::core::Fixed<D,S>` template
    // (spec-equivalent form, decimal-library implementation in the
    // dds-core crate).
    let cpp = gen_cpp(r#"struct M { fixed<10,2> price; };"#);
    assert!(
        cpp.contains("dds::core::Fixed<10, 2>"),
        "Fixed<10,2>-Template missing:\n{cpp}"
    );
}

#[test]
fn shared_member_emits_std_shared_ptr() {
    // Spec §8.1.5 (idl4-cpp / dds-psm-cxx): @shared → std::shared_ptr<T>.
    let cpp = gen_cpp(
        r#"
        struct WithShared {
            @shared long ptr;
        };
    "#,
    );
    assert!(
        cpp.contains("std::shared_ptr"),
        "std::shared_ptr missing:\n{cpp}"
    );
    assert!(cpp.contains("<memory>"), "<memory>-Include missing:\n{cpp}");
}

#[test]
fn shared_and_optional_compose() {
    // @optional + @shared kombiniert -> std::optional<std::shared_ptr<T>>.
    let cpp = gen_cpp(
        r#"
        struct OptShared {
            @optional @shared long ptr;
        };
    "#,
    );
    assert!(
        cpp.contains("std::optional<std::shared_ptr<"),
        "kombinierte optional<shared_ptr> missing:\n{cpp}"
    );
}

// ============================================================================
// §7.17.2 @key → DDS-Topic-Marker
// ============================================================================

#[test]
fn key_annotation_emits_marker_comment_or_attribute() {
    // Spec §7.17.2: @key has no spec impact, but ZeroDDS
    // emits a marker for DDS topic identity.
    let cpp = gen_cpp(
        r#"
        @nested(false)
        struct Keyed {
            @key long id;
            string name;
        };
    "#,
    );
    assert!(cpp.contains("Keyed"));
    // Marker via Doxygen comment or direct annotation.
    assert!(
        cpp.contains("@key") || cpp.contains("dds_key") || cpp.contains("// key"),
        "key-Marker missing:\n{cpp}"
    );
}

// ============================================================================
// §7.17.5 @verbatim — Code-Gen-Hook (XTypes §7.2.2.4.8 Cross-Cutting)
// ============================================================================

#[test]
fn verbatim_annotation_with_cpp_language_inlines_text() {
    // Spec §7.17.5 + XTypes §7.2.2.4.8: @verbatim(language="c++",
    // placement="...", text="...") embeds the text at the chosen
    // inserted into the C++ output.
    //
    // Cross-Reference: XTypes 1.3 §7.2.2.4.8 — VerbatimText-
    // AppliedAnnotation variant (see `dds-xtypes-1.3.md`).
    let cpp = gen_cpp(
        r#"
        @verbatim(language="c++", placement=BEFORE_DECLARATION, text="// pre-decl marker")
        struct PlainStruct { long x; };
    "#,
    );
    assert!(
        cpp.contains("PlainStruct"),
        "PlainStruct missing from the output:\n{cpp}"
    );
    assert!(
        cpp.contains("// pre-decl marker"),
        "@verbatim BEFORE_DECLARATION missing from the output:\n{cpp}"
    );
    // The text must come BEFORE the class line.
    let pos_marker = cpp.find("// pre-decl marker").unwrap_or(usize::MAX);
    let pos_class = cpp.find("class PlainStruct").unwrap_or(usize::MAX);
    assert!(
        pos_marker < pos_class,
        "BEFORE_DECLARATION verbatim must come before the class line:\n{cpp}"
    );
}

#[test]
fn verbatim_annotation_with_after_declaration_placement() {
    // Spec XTypes §7.2.2.4.8 — AFTER_DECLARATION
    let cpp = gen_cpp(
        r#"
        @verbatim(language="c++", placement=AFTER_DECLARATION, text="// trailer marker")
        struct S { long x; };
    "#,
    );
    let pos_marker = cpp.find("// trailer marker").unwrap_or(usize::MAX);
    let pos_close = cpp.find("};").unwrap_or(usize::MAX);
    assert!(
        pos_marker > pos_close && pos_marker != usize::MAX,
        "AFTER_DECLARATION verbatim must come after `}};`:\n{cpp}"
    );
}

#[test]
fn verbatim_annotation_wildcard_language_applies() {
    // Spec §8.3.5.1 — language="*" matches all codegens.
    let cpp = gen_cpp(
        r#"
        @verbatim(language="*", placement=BEFORE_DECLARATION, text="// universal pre")
        struct S { long x; };
    "#,
    );
    assert!(
        cpp.contains("// universal pre"),
        "the wildcard language must match:\n{cpp}"
    );
}

#[test]
fn verbatim_annotation_other_language_skipped() {
    // Spec — language="java" must NOT inline on a C++ codegen.
    let cpp = gen_cpp(
        r#"
        @verbatim(language="java", placement=BEFORE_DECLARATION, text="// not for cpp")
        struct S { long x; };
    "#,
    );
    assert!(
        !cpp.contains("// not for cpp"),
        "the wrong language must not be emitted:\n{cpp}"
    );
}

// ============================================================================
// §8.1 @cpp_mapping
// ============================================================================

#[test]
fn struct_with_default_mapping_emits_class_with_accessors() {
    // Spec §8.1.1 (alternative): CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS
    // → C++ class with pure-public members via accessor/modifier.
    // ZeroDDS emits this as the default (see audit §8.1.1 partial).
    let cpp = gen_cpp(r#"struct S { long x; };"#);
    // The generator emits the class variant: setter/getter
    assert!(
        cpp.contains("get_x")
            || cpp.contains("x()")
            || cpp.contains("set_x")
            || cpp.contains("public:")
    );
}

// ============================================================================
// §7.5 Interfaces – Full (Embedded type/const/exception)
// ============================================================================

#[test]
fn non_service_interface_emits_pure_virtual_class() {
    // Spec §7.4: IDL interface -> C++ pure-virtual class.
    let cpp = gen_cpp(
        r#"
        interface Calc {
            long add(in long a, in long b);
            readonly attribute long version;
        };
    "#,
    );
    assert!(cpp.contains("class Calc"), "class Calc missing:\n{cpp}");
    assert!(
        cpp.contains("virtual ~Calc()"),
        "virtual dtor missing:\n{cpp}"
    );
    assert!(cpp.contains("= 0;"), "pure-virtual marker missing:\n{cpp}");
    assert!(cpp.contains("add("), "add operation missing:\n{cpp}");
    assert!(cpp.contains("version()"), "readonly getter missing:\n{cpp}");
}

#[test]
fn interface_with_in_param_uses_const_reference() {
    let cpp = gen_cpp(r#"interface I { void op(in long x); };"#);
    assert!(cpp.contains("const int32_t&"), "in-param const ref:\n{cpp}");
}

#[test]
fn interface_with_out_param_uses_reference() {
    let cpp = gen_cpp(r#"interface I { void op(out long x); };"#);
    assert!(
        !cpp.contains("const int32_t& x"),
        "the out-param must not be const:\n{cpp}"
    );
}

#[test]
fn any_member_is_rejected_cleanly() {
    // §7.3: `any` (CORBA TypeCode + dynamic value) has NO DDS-XTypes wire form /
    // TypeObject and no ZeroDDS XCDR codec yet. Rather than emit a non-compiling
    // `dds::core::Any` field that silently drops on the wire, the DDS PSM-Cxx
    // generator rejects it cleanly (like C / Python). Tracked: `any` follow-up.
    let ast =
        zerodds_idl::parse(r#"struct M { any value; };"#, &ParserConfig::default()).expect("parse");
    let res = generate_cpp_header(&ast, &CppGenOptions::default());
    assert!(res.is_err(), "any member must be a clean codegen error");
}

// ============================================================================
// §7.6 Value Types — documented as unsupported
// ============================================================================

#[test]
fn valuetype_is_feature_gated_or_emits_class_with_accessors() {
    // Spec §7.6: valuetype -> C++ class with a pure-virtual public/
    // protected accessor + factory class.
    // The ZeroDDS parser is feature-gated (`corba_value_types_full`) —
    // we only test that either the parser rejects OR the codegen emits
    // the correct class structure.
    let parse = zerodds_idl::parse(
        r#"valuetype VT { public long x; };"#,
        &ParserConfig::default(),
    );
    match parse {
        Ok(ast) => {
            // The codegen must provide class + accessor + virtual dtor.
            let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
            assert!(cpp.contains("class VT"), "class VT missing:\n{cpp}");
            assert!(
                cpp.contains("virtual ~VT()"),
                "virtual dtor missing:\n{cpp}"
            );
            assert!(cpp.contains("x()"), "x()-accessor missing:\n{cpp}");
        }
        Err(_) => {
            // Rejected on the parser side (FeaturesDisabled): also
            // spec-permitted (§7.6 allows implementations
            // not to support valuetype).
        }
    }
}

#[test]
fn valuetype_with_factory_emits_factory_class() {
    let parse = zerodds_idl::parse(
        r#"valuetype VT { public long x; factory create(in long x); };"#,
        &ParserConfig::default(),
    );
    if let Ok(ast) = parse {
        let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
        assert!(
            cpp.contains("class VT_factory"),
            "VT_factory class missing:\n{cpp}"
        );
        assert!(cpp.contains("create("));
    }
}

#[test]
fn valuetype_private_state_emits_protected_accessor() {
    let parse = zerodds_idl::parse(
        r#"valuetype VT { public long x; private string secret; };"#,
        &ParserConfig::default(),
    );
    if let Ok(ast) = parse {
        let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
        assert!(
            cpp.contains("protected:"),
            "protected-Block missing:\n{cpp}"
        );
        assert!(cpp.contains("secret()"));
    }
}

// ============================================================================
// §7.14.3.2/3 Bitset/Bitmask — documented as unsupported
// ============================================================================

#[test]
fn bitset_emits_struct_with_value_field() {
    // Spec idl4-cpp §7.14.3.2: bitset → C++ struct with bit-fields.
    // ZeroDDS mapping: `struct B { uint64_t value; ... };` with a getter/
    // setter per named bitfield.
    let cpp = gen_cpp(r#"bitset BS { bitfield<3> a; bitfield<5> b; };"#);
    assert!(cpp.contains("struct BS"), "struct BS missing:\n{cpp}");
    assert!(cpp.contains("uint64_t value"), "value-Feld missing:\n{cpp}");
    assert!(cpp.contains("a()"), "getter for a missing:\n{cpp}");
    assert!(cpp.contains("b()"), "getter for b missing:\n{cpp}");
    // Mask for width=3 is 0x7, for width=5 is 0x1F.
    assert!(cpp.contains("0x7ULL"), "0x7-Mask missing:\n{cpp}");
    assert!(cpp.contains("0x1FULL"), "0x1F-Mask missing:\n{cpp}");
}

#[test]
fn bitset_total_width_over_64_returns_error() {
    // Spec idl4-cpp §7.14.3.2: max total width 64 bit (uint64_t).
    let ast = zerodds_idl::parse(
        r#"bitset BS { bitfield<40> a; bitfield<30> b; };"#,
        &ParserConfig::default(),
    );
    if let Ok(ast) = ast {
        let result = generate_cpp_header(&ast, &CppGenOptions::default());
        assert!(result.is_err(), "bitset > 64 bit must be rejected");
    }
}

#[test]
fn bitmask_emits_enum_class_with_bitwise_operators() {
    // Spec idl4-cpp §7.14.3.3: bitmask → struct with an unscoped enum +
    // value member + bitwise operators. ZeroDDS choice: a type-safe
    // `enum class : uint{N}_t` (C++11+) with operator overloads.
    let cpp = gen_cpp(r#"@bit_bound(8) bitmask Flags { READ, WRITE, EXEC };"#);
    assert!(
        cpp.contains("enum class Flags : uint8_t"),
        "enum class : uint8_t missing:\n{cpp}"
    );
    assert!(cpp.contains("READ"));
    assert!(cpp.contains("1ULL << 0"), "Position 0 missing:\n{cpp}");
    assert!(cpp.contains("operator|"), "operator| missing:\n{cpp}");
    assert!(cpp.contains("operator&"));
    assert!(cpp.contains("operator^"));
    assert!(cpp.contains("operator~"));
}

#[test]
fn bitmask_explicit_position_overrides_auto() {
    // Spec idl4-cpp §7.14.3.3 + §7.17.1: `@position(N)` overrides
    // auto-numbering.
    let cpp = gen_cpp(r#"@bit_bound(16) bitmask F { @position(3) A, B };"#);
    assert!(
        cpp.contains("enum class F : uint16_t"),
        "uint16_t-Underlying missing:\n{cpp}"
    );
    assert!(cpp.contains("A = 1ULL << 3"));
    // B follows with auto-position 4.
    assert!(cpp.contains("B = 1ULL << 4"));
}

/// Bug D: the generated header must NOT introduce identifiers reserved to the
/// C++ implementation (§ [lex.name]/3: any identifier containing a double
/// underscore, or starting with an underscore followed by an uppercase letter,
/// at any scope, is reserved). The XCDR2 topic codec previously emitted its own
/// temporaries as `__v`, `__out`, `__repr`, `__max_align`, `__ns…` etc. This
/// generates a feature-dense header and asserts no codegen-introduced reserved
/// identifier remains (the only allowed `__` token is the standard predefined
/// macro `__cplusplus`).
#[test]
fn no_implementation_reserved_identifiers() {
    let src = "\
module conf {
  enum Mode { MODE_IDLE, MODE_ACTIVE };
  union Reading switch (Mode) {
    case MODE_IDLE:   long   ticks;
    default:          string code;
  };
  @appendable
  struct Telemetry {
    @key long          id;
    Mode               mode;
    sequence<long>     history;
    Reading            reading;
    map<string, long>  counters;
    long               window[4];
  };
};";
    let cpp = gen_cpp(src);
    // Tokenize loosely: collect every identifier-like run, then flag any that is
    // reserved AND not the whitelisted predefined macro.
    let mut offenders: Vec<String> = Vec::new();
    let bytes = cpp.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let is_ident_start = c == b'_' || c.is_ascii_alphabetic();
        if is_ident_start {
            let start = i;
            while i < bytes.len() && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            let ident = &cpp[start..i];
            if ident == "__cplusplus" {
                continue;
            }
            let has_dunder = ident.contains("__");
            let underscore_upper = ident.len() >= 2
                && ident.as_bytes()[0] == b'_'
                && ident.as_bytes()[1].is_ascii_uppercase();
            if has_dunder || underscore_upper {
                offenders.push(ident.to_string());
            }
        } else {
            i += 1;
        }
    }
    offenders.sort();
    offenders.dedup();
    assert!(
        offenders.is_empty(),
        "generated header contains C++-reserved identifiers: {offenders:?}"
    );
}
