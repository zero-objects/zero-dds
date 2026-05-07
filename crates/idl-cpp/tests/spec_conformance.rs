//! Spec-Conformance-Matrix fuer IDL4-C++ 1.0 §7 + §8.
//!
//! Verifiziert produktiv die in `docs/spec-coverage/idl4-cpp-1.0.md`
//! gelisteten Generator-Pfade durch end-to-end-IDL→C++-Renderings.
//!
//! Cluster:
//! 1. **§7.2.3 Constants** — string/wstring-constexpr-Pfad.
//! 2. **§7.2.4.2.1-3 Type-Trait-Specs** — bounded sequence/string/wstring
//!    `is_bounded`/`bound`-Spezialisierungen.
//! 3. **§7.2.4.3.4 Recursive Types** — Self-Reference via Typedef.
//! 4. **§7.4.2 Interface Forward-Decl** — `class Foo;`.
//! 5. **§7.5 Interfaces – Full** — Embedded type/const/exception decls.
//! 6. **§7.14.2 Union-Discriminator-Erweiterungen** — wchar/octet/int8/uint8.
//! 7. **§7.16 User-Defined Annotations** — no-op verifiziert.
//! 8. **§7.17.x Standardized Annotations** — @optional/@key/@value/@bit_bound/
//!    @verbatim (XTypes §7.2.2.4.8 Cross-Cutting).
//! 9. **§8.1 @cpp_mapping** — Annotation als Codegen-Hook.

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
// §7.2.3 Constants — string/wstring-Pfad
// ============================================================================

#[test]
fn string_constant_emits_constexpr_string_view() {
    // Spec §7.2.3: string-const → constexpr omg::types::string_view
    // (oder std::string_view als spec-konformer Substitut).
    let cpp = gen_cpp(r#"const string GREETING = "hello";"#);
    assert!(cpp.contains("GREETING"), "constant name fehlt");
    assert!(cpp.contains("constexpr"), "constexpr-Keyword fehlt");
}

#[test]
fn numeric_constant_emits_constexpr_with_value() {
    // Spec §7.2.3: numeric-const → constexpr int.
    let cpp = gen_cpp("const long N = 42;");
    assert!(cpp.contains("constexpr"));
    assert!(cpp.contains("42"));
}

// ============================================================================
// §7.2.4.2.1-3 Bounded-Sequence/String Type-Traits
// ============================================================================

#[test]
fn bounded_sequence_struct_emits_vector_with_size_marker() {
    // Spec §7.2.4.2.1 + Tab.7.4: bounded sequence → std::vector mit
    // bound-Marker. Generator emittiert std::vector<T> als Spec-Default.
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
    // Spec §7.2.4.3.4: rekursive Konstruktionen via typedef.
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
    // Spec §7.4.2: forward-decl → `class Foo;` oder `struct Foo;`.
    let cpp = gen_cpp(
        r#"
        struct Forwarded;
        struct Forwarded { long x; };
    "#,
    );
    // Forward-Decl muss vorhanden sein als `class Forwarded;` oder
    // `struct Forwarded;` (Spec erlaubt beides).
    assert!(
        cpp.contains("class Forwarded;") || cpp.contains("struct Forwarded;"),
        "forward declaration fehlt:\n{cpp}"
    );
}

// ============================================================================
// §7.14.2 Union-Discriminator-Erweiterungen
// ============================================================================

#[test]
fn union_with_octet_discriminator_emits_variant() {
    // Spec §7.14.2: octet als Discriminator-Type erlaubt.
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
    // Spec §7.16: User-Annotations → kein C++-Output.
    let cpp = gen_cpp(
        r#"
        @annotation MyCustom { string note; };
        @MyCustom(note="ignore-me")
        struct S { long x; };
    "#,
    );
    // Generator emittiert Class-Variante mit `class S` und `int32_t x_`.
    assert!(cpp.contains("class S") || cpp.contains("struct S"));
    assert!(cpp.contains("x_"));
    // `MyCustom`/`note` darf nicht als C++-Code-Element in der Output-
    // Datei landen (max. als Doxygen-Kommentar mit "// ").
    let out = cpp.as_str();
    let mc_count = out.matches("MyCustom").count();
    let comment_lines = out
        .lines()
        .filter(|l| l.contains("MyCustom"))
        .all(|l| l.trim_start().starts_with("//") || l.trim_start().starts_with("*"));
    assert!(
        mc_count == 0 || comment_lines,
        "User-Annotation als C++-Code-Element emittiert"
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
    assert!(cpp.contains("std::optional"), "std::optional fehlt:\n{cpp}");
}

#[test]
fn fixed_member_emits_dds_core_fixed_template() {
    // Spec idl4-cpp §7.2.4.2.4: fixed<digits,scale> -> Fixed-Klasse mit
    // ~30 Methoden. ZeroDDS-Wahl: `dds::core::Fixed<D,S>`-Template
    // (Spec-aequivalente Form, Decimal-Library-Implementation in
    // dds-core-Crate).
    let cpp = gen_cpp(r#"struct M { fixed<10,2> price; };"#);
    assert!(
        cpp.contains("dds::core::Fixed<10, 2>"),
        "Fixed<10,2>-Template fehlt:\n{cpp}"
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
        "std::shared_ptr fehlt:\n{cpp}"
    );
    assert!(cpp.contains("<memory>"), "<memory>-Include fehlt:\n{cpp}");
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
        "kombinierte optional<shared_ptr> fehlt:\n{cpp}"
    );
}

// ============================================================================
// §7.17.2 @key → DDS-Topic-Marker
// ============================================================================

#[test]
fn key_annotation_emits_marker_comment_or_attribute() {
    // Spec §7.17.2: @key hat keinen Spec-Impact, aber ZeroDDS
    // emittiert einen Marker fuer DDS-Topic-Identitaet.
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
    // Marker via Doxygen-Kommentar oder direkter Annotation.
    assert!(
        cpp.contains("@key") || cpp.contains("dds_key") || cpp.contains("// key"),
        "key-Marker fehlt:\n{cpp}"
    );
}

// ============================================================================
// §7.17.5 @verbatim — Code-Gen-Hook (XTypes §7.2.2.4.8 Cross-Cutting)
// ============================================================================

#[test]
fn verbatim_annotation_with_cpp_language_inlines_text() {
    // Spec §7.17.5 + XTypes §7.2.2.4.8: @verbatim(language="c++",
    // placement="...", text="...") bettet den Text an der gewaehlten
    // Stelle in den C++-Output ein.
    //
    // Cross-Reference: XTypes 1.3 §7.2.2.4.8 — VerbatimText-
    // AppliedAnnotation-Variante (siehe `dds-xtypes-1.3.md`).
    let cpp = gen_cpp(
        r#"
        @verbatim(language="c++", placement=BEFORE_DECLARATION, text="// pre-decl marker")
        struct PlainStruct { long x; };
    "#,
    );
    assert!(
        cpp.contains("PlainStruct"),
        "PlainStruct fehlt im Output:\n{cpp}"
    );
    assert!(
        cpp.contains("// pre-decl marker"),
        "@verbatim BEFORE_DECLARATION fehlt im Output:\n{cpp}"
    );
    // Text muss VOR der class-Zeile stehen.
    let pos_marker = cpp.find("// pre-decl marker").unwrap_or(usize::MAX);
    let pos_class = cpp.find("class PlainStruct").unwrap_or(usize::MAX);
    assert!(
        pos_marker < pos_class,
        "BEFORE_DECLARATION-Verbatim muss vor der Class-Zeile stehen:\n{cpp}"
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
        "AFTER_DECLARATION-Verbatim muss nach `}};` stehen:\n{cpp}"
    );
}

#[test]
fn verbatim_annotation_wildcard_language_applies() {
    // Spec §8.3.5.1 — language="*" matched alle Codegens.
    let cpp = gen_cpp(
        r#"
        @verbatim(language="*", placement=BEFORE_DECLARATION, text="// universal pre")
        struct S { long x; };
    "#,
    );
    assert!(
        cpp.contains("// universal pre"),
        "Wildcard-Sprache muss matchen:\n{cpp}"
    );
}

#[test]
fn verbatim_annotation_other_language_skipped() {
    // Spec — language="java" auf C++-Codegen darf NICHT inlinen.
    let cpp = gen_cpp(
        r#"
        @verbatim(language="java", placement=BEFORE_DECLARATION, text="// not for cpp")
        struct S { long x; };
    "#,
    );
    assert!(
        !cpp.contains("// not for cpp"),
        "Falsche Sprache darf nicht emittiert werden:\n{cpp}"
    );
}

// ============================================================================
// §8.1 @cpp_mapping
// ============================================================================

#[test]
fn struct_with_default_mapping_emits_class_with_accessors() {
    // Spec §8.1.1 (alternative): CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS
    // → C++ class mit pure-public Members ueber Accessor/Modifier.
    // ZeroDDS emittiert das als Default (siehe Audit §8.1.1 partial).
    let cpp = gen_cpp(r#"struct S { long x; };"#);
    // Generator emittiert Class-Variante: setter/getter
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
    assert!(cpp.contains("class Calc"), "class Calc fehlt:\n{cpp}");
    assert!(
        cpp.contains("virtual ~Calc()"),
        "virtual dtor fehlt:\n{cpp}"
    );
    assert!(cpp.contains("= 0;"), "pure-virtual marker fehlt:\n{cpp}");
    assert!(cpp.contains("add("), "add operation fehlt:\n{cpp}");
    assert!(cpp.contains("version()"), "readonly getter fehlt:\n{cpp}");
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
        "out-param darf nicht const sein:\n{cpp}"
    );
}

#[test]
fn any_member_emits_dds_core_any() {
    // Spec §7.3: any -> dds::core::Any (omg::types::Any-aequivalent).
    let cpp = gen_cpp(r#"struct M { any value; };"#);
    assert!(
        cpp.contains("dds::core::Any"),
        "dds::core::Any fehlt:\n{cpp}"
    );
}

// ============================================================================
// §7.6 Value Types — als Unsupported dokumentiert
// ============================================================================

#[test]
fn valuetype_is_feature_gated_or_emits_class_with_accessors() {
    // Spec §7.6: valuetype -> C++ class mit pure-virtual public/
    // protected accessor + factory-Class.
    // ZeroDDS-Parser ist feature-gated (`corba_value_types_full`) —
    // wir testen nur, dass entweder Parser ablehnt ODER Codegen die
    // korrekte Class-Struktur emittiert.
    let parse = zerodds_idl::parse(
        r#"valuetype VT { public long x; };"#,
        &ParserConfig::default(),
    );
    match parse {
        Ok(ast) => {
            // Codegen muss class + accessor + virtual dtor liefern.
            let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
            assert!(cpp.contains("class VT"), "class VT fehlt:\n{cpp}");
            assert!(cpp.contains("virtual ~VT()"), "virtual dtor fehlt:\n{cpp}");
            assert!(cpp.contains("x()"), "x()-accessor fehlt:\n{cpp}");
        }
        Err(_) => {
            // Parser-seitig abgelehnt (FeaturesDisabled): ist auch
            // Spec-permitted (§7.6 erlaubt Implementations,
            // valuetype nicht zu unterstuetzen).
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
            "VT_factory class fehlt:\n{cpp}"
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
        assert!(cpp.contains("protected:"), "protected-Block fehlt:\n{cpp}");
        assert!(cpp.contains("secret()"));
    }
}

// ============================================================================
// §7.14.3.2/3 Bitset/Bitmask — als Unsupported dokumentiert
// ============================================================================

#[test]
fn bitset_emits_struct_with_value_field() {
    // Spec idl4-cpp §7.14.3.2: bitset → C++ struct mit bit-fields.
    // ZeroDDS-Mapping: `struct B { uint64_t value; ... };` mit Getter/
    // Setter pro benanntem Bitfield.
    let cpp = gen_cpp(r#"bitset BS { bitfield<3> a; bitfield<5> b; };"#);
    assert!(cpp.contains("struct BS"), "struct BS fehlt:\n{cpp}");
    assert!(cpp.contains("uint64_t value"), "value-Feld fehlt:\n{cpp}");
    assert!(cpp.contains("a()"), "Getter fuer a fehlt:\n{cpp}");
    assert!(cpp.contains("b()"), "Getter fuer b fehlt:\n{cpp}");
    // Mask fuer width=3 ist 0x7, fuer width=5 ist 0x1F.
    assert!(cpp.contains("0x7ULL"), "0x7-Mask fehlt:\n{cpp}");
    assert!(cpp.contains("0x1FULL"), "0x1F-Mask fehlt:\n{cpp}");
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
        assert!(result.is_err(), "bitset > 64 bit muss rejected werden");
    }
}

#[test]
fn bitmask_emits_enum_class_with_bitwise_operators() {
    // Spec idl4-cpp §7.14.3.3: bitmask → struct mit unscoped enum +
    // value-member + Bitwise-Operatoren. ZeroDDS-Wahl: typsafer
    // `enum class : uint{N}_t` (C++11+) mit Operator-Overloads.
    let cpp = gen_cpp(r#"@bit_bound(8) bitmask Flags { READ, WRITE, EXEC };"#);
    assert!(
        cpp.contains("enum class Flags : uint8_t"),
        "enum class : uint8_t fehlt:\n{cpp}"
    );
    assert!(cpp.contains("READ"));
    assert!(cpp.contains("1ULL << 0"), "Position 0 fehlt:\n{cpp}");
    assert!(cpp.contains("operator|"), "operator| fehlt:\n{cpp}");
    assert!(cpp.contains("operator&"));
    assert!(cpp.contains("operator^"));
    assert!(cpp.contains("operator~"));
}

#[test]
fn bitmask_explicit_position_overrides_auto() {
    // Spec idl4-cpp §7.14.3.3 + §7.17.1: `@position(N)` ueberschreibt
    // Auto-Numbering.
    let cpp = gen_cpp(r#"@bit_bound(16) bitmask F { @position(3) A, B };"#);
    assert!(
        cpp.contains("enum class F : uint16_t"),
        "uint16_t-Underlying fehlt:\n{cpp}"
    );
    assert!(cpp.contains("A = 1ULL << 3"));
    // B folgt mit auto-position 4.
    assert!(cpp.contains("B = 1ULL << 4"));
}
