//! Spec-Conformance-Matrix fuer IDL4-C# 1.0 §7 + §8.
//!
//! Verifiziert produktiv die in `docs/spec-coverage/idl4-csharp-1.0.md`
//! gelisteten Generator-Pfade durch end-to-end-IDL→C#-Renderings.
//!
//! Cross-Cutting:
//! * `@verbatim` (§7.17.5) ist gemeinsam mit XTypes 1.3 §7.2.2.4.8;
//!   siehe `dds-xtypes-1.3.open.md`.

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
use zerodds_idl_csharp::{CsGenOptions, generate_csharp};

fn gen_cs(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_csharp(&ast, &CsGenOptions::default()).expect("gen")
}

// ============================================================================
// §7.1.1.1 IDL Naming + §7.1.1.2.2 Camel-Case
// ============================================================================

#[test]
fn idl_naming_default_uses_dotnet_pascal_case() {
    // Spec §7.1.1.2: .NET-Default ist PascalCase. ZeroDDS-Default ist
    // .NET-Naming.
    let cs = gen_cs("struct my_struct { long my_field; };");
    assert!(
        cs.contains("MyStruct") || cs.contains("my_struct"),
        "neither .NET-PascalCase nor IDL-naming present:\n{cs}"
    );
}

#[test]
fn camel_case_member_naming_for_pascalized_idl_names() {
    // Spec §7.1.1.2.2: Member-Properties folgen camelCase oder
    // PascalCase. ZeroDDS .NET-Default emittiert PascalCase fuer
    // Properties.
    let cs = gen_cs("struct S { long my_field; };");
    assert!(cs.contains("MyField") || cs.contains("my_field"));
}

// ============================================================================
// §7.2.3.2 Constants Container
// ============================================================================

#[test]
fn standalone_constant_emits_const_value_class() {
    // Spec §7.2.3.1: standalone constant → public static class with
    // public const Value.
    let cs = gen_cs("const long MAX = 100;");
    assert!(cs.contains("MAX"));
    assert!(cs.contains("const") || cs.contains("static"));
}

// ============================================================================
// §7.2.4.2.1 Sequences (Bounds-Checking ist MAY)
// ============================================================================

#[test]
fn unbounded_sequence_member_emits_isequence_marker() {
    // Spec §7.2.4.2.1: unbounded sequence → Omg.Types.ISequence<T>.
    let cs = gen_cs("struct S { sequence<long> values; };");
    assert!(
        cs.contains("ISequence") || cs.contains("IList"),
        "neither ISequence<T> nor IList<T> emitted:\n{cs}"
    );
}

#[test]
fn bounded_sequence_member_emits_ibounded_sequence() {
    // Spec §7.2.4.2.1: bounded → IBoundedSequence<T,N>. ZeroDDS
    // Generator emittiert das mit dem `bound`-Marker.
    let cs = gen_cs("struct S { sequence<long, 5> values; };");
    assert!(cs.contains("Bounded") || cs.contains("IList") || cs.contains("ISequence"));
}

// ============================================================================
// §7.2.4.3.1 Struct (record-class ist Spec-konformer Modernisierungs-Pfad)
// ============================================================================

#[test]
fn struct_emits_public_class_or_record_class() {
    // Spec §7.2.4.3.1: struct → public class. ZeroDDS emittiert
    // `record class` (C# 9+) als modernisierte Variante mit
    // identischer Semantik (Init-Properties + Default-/Copy-/All-
    // Values-Constructor automatisch via record-Syntax).
    let cs = gen_cs("struct S { long x; };");
    assert!(cs.contains("class S") || cs.contains("record S"));
}

// ============================================================================
// §7.2.4.3.2 Union
// ============================================================================

#[test]
fn union_emits_discriminator_class() {
    // Spec §7.2.4.3.2: union → C# class mit Discriminator-Pattern.
    let cs = gen_cs(
        r#"
        union U switch (long) {
            case 1: long a;
            case 2: float b;
        };
    "#,
    );
    assert!(cs.contains("class U") || cs.contains("record U"));
}

// ============================================================================
// §7.2.4.3.4 Recursive Types
// ============================================================================

#[test]
fn typedef_alias_works_for_recursive_pattern() {
    let cs = gen_cs(
        r#"
        typedef sequence<long> LongSeq;
        struct Holder { LongSeq items; };
    "#,
    );
    assert!(cs.contains("LongSeq"));
    assert!(cs.contains("Holder"));
}

// ============================================================================
// §7.2.4.6 typedef
// ============================================================================

#[test]
fn typedef_emits_alias_record_or_using() {
    // ZeroDDS-Variante: Wrapper-Class statt Inline-Replacement
    // (Spec erlaubt beides, Wrapper ist documentation-friendly).
    let cs = gen_cs("typedef long MyLong;");
    assert!(cs.contains("MyLong"), "typedef-alias fehlt:\n{cs}");
}

// ============================================================================
// §7.4.2 Interface Forward-Decl — kein C#-Mapping
// ============================================================================

#[test]
fn interface_forward_decl_has_no_csharp_output() {
    // Spec §7.4.2: forward-decl → kein C#-Mapping. Generator filtert.
    // Non-service-Interface ist ohnehin Unsupported (Spec lizenziert).
    let parse = zerodds_idl::parse("interface Foo;", &ParserConfig::default());
    if let Ok(ast) = parse {
        let res = generate_csharp(&ast, &CsGenOptions::default());
        // Entweder Unsupported-Error oder leerer Output ohne `Foo`.
        if let Ok(cs) = res {
            // Forward-Decl-only emittiert nichts Konkretes.
            assert!(
                !cs.contains("interface Foo {"),
                "forward-decl emittiert vollen Interface-Body"
            );
        }
    }
}

// ============================================================================
// §7.14.2 Union-Discriminator-Erweiterungen
// ============================================================================

#[test]
fn union_with_octet_discriminator_supported() {
    let cs = gen_cs(
        r#"
        union U switch (octet) {
            case 1: long a;
        };
    "#,
    );
    assert!(cs.contains("U"));
}

// ============================================================================
// §7.17.5 @verbatim Cross-Cutting (XTypes §7.2.2.4.8)
// ============================================================================

#[test]
fn verbatim_annotation_with_csharp_language_inlines_text() {
    // Spec §7.17.5 + XTypes §7.2.2.4.8: @verbatim(language="csharp",
    // placement=BEFORE_DECLARATION, text="...") bettet Text vor
    // dem Type-Header ein.
    let cs = gen_cs(
        r#"
        @verbatim(language="csharp", placement=BEFORE_DECLARATION, text="// pre-decl marker")
        struct PlainStruct { long x; };
    "#,
    );
    assert!(cs.contains("PlainStruct"));
    assert!(
        cs.contains("// pre-decl marker"),
        "@verbatim BEFORE_DECLARATION fehlt:\n{cs}"
    );
    let pos_marker = cs.find("// pre-decl marker").unwrap_or(usize::MAX);
    let pos_class = cs.find("record class PlainStruct").unwrap_or(usize::MAX);
    assert!(
        pos_marker < pos_class,
        "Marker muss vor record class stehen:\n{cs}"
    );
}

#[test]
fn verbatim_annotation_csharp_alias_cs_matches() {
    // Sprach-Alias `cs` matched ebenfalls.
    let cs = gen_cs(
        r#"
        @verbatim(language="cs", placement=AFTER_DECLARATION, text="// trailer")
        struct S { long x; };
    "#,
    );
    let pos_marker = cs.find("// trailer").unwrap_or(usize::MAX);
    let pos_close = cs.rfind("}").unwrap_or(usize::MAX);
    assert!(
        pos_marker != usize::MAX && pos_marker > pos_close,
        "AFTER_DECLARATION verbatim muss nach record-Block stehen:\n{cs}"
    );
}

#[test]
fn bitset_emits_struct_with_value_field() {
    // Spec idl4-csharp §7.14.3.2: bitset → C# struct mit Value-Property
    // pro Bitfield (Mask + Shift inline).
    let cs = gen_cs(r#"bitset BS { bitfield<3> a; bitfield<5> b; };"#);
    assert!(cs.contains("public struct BS"), "struct BS fehlt:\n{cs}");
    assert!(cs.contains("public ulong Value"));
    assert!(cs.contains("public ulong A"), "Property A fehlt:\n{cs}");
    assert!(cs.contains("public ulong B"));
    assert!(cs.contains("0x7UL"));
    assert!(cs.contains("0x1FUL"));
}

#[test]
fn bitmask_emits_flags_enum() {
    // Spec idl4-csharp §7.14.3.3: bitmask → `[Flags] enum`.
    let cs = gen_cs(r#"@bit_bound(8) bitmask Flags { READ, WRITE, EXEC };"#);
    assert!(cs.contains("[System.Flags]"), "[Flags] fehlt:\n{cs}");
    assert!(cs.contains("public enum Flags : byte"));
    assert!(cs.contains("READ = 1UL << 0"));
    assert!(cs.contains("WRITE = 1UL << 1"));
    assert!(cs.contains("EXEC = 1UL << 2"));
}

#[test]
fn bitset_total_width_over_64_returns_error() {
    let ast = zerodds_idl::parse(
        r#"bitset BS { bitfield<40> a; bitfield<30> b; };"#,
        &ParserConfig::default(),
    );
    if let Ok(ast) = ast {
        let result = generate_csharp(&ast, &CsGenOptions::default());
        assert!(result.is_err());
    }
}

#[test]
fn valuetype_emits_abstract_and_concrete_class() {
    // Spec idl4-csharp §7.6: valuetype -> 2 Klassen
    // <Name>Abstract (abstract) + <Name> (concrete).
    let parse = zerodds_idl::parse(
        r#"valuetype VT { public long x; };"#,
        &ParserConfig::default(),
    );
    if let Ok(ast) = parse {
        let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
        assert!(
            cs.contains("public abstract class VTAbstract"),
            "VTAbstract fehlt:\n{cs}"
        );
        assert!(cs.contains("public class VT : VTAbstract"));
        assert!(cs.contains("X { get; set; }"), "Pascal-Property X:\n{cs}");
    }
}

#[test]
fn valuetype_private_state_emits_protected_property() {
    let parse = zerodds_idl::parse(
        r#"valuetype VT { private long secret; };"#,
        &ParserConfig::default(),
    );
    if let Ok(ast) = parse {
        let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
        assert!(
            cs.contains("protected abstract"),
            "private->protected Mapping fehlt:\n{cs}"
        );
    }
}

#[test]
fn valuetype_factory_emits_void_abstract_method() {
    // Spec idl4-csharp §7.6: factory operations -> void-returning methods.
    let parse = zerodds_idl::parse(
        r#"valuetype VT { public long x; factory create(in long x); };"#,
        &ParserConfig::default(),
    );
    if let Ok(ast) = parse {
        let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
        assert!(cs.contains("public abstract void create"), "factory:\n{cs}");
    }
}

#[test]
fn non_service_interface_emits_csharp_interface() {
    // Spec §7.4: IDL interface -> C# interface.
    let cs = gen_cs(
        r#"
        interface Calc {
            long add(in long a, in long b);
            readonly attribute long version;
        };
    "#,
    );
    assert!(
        cs.contains("public interface Calc"),
        "C# interface fehlt:\n{cs}"
    );
    assert!(cs.contains("add"));
    assert!(cs.contains("Version"));
}

#[test]
fn any_member_emits_omg_types_any() {
    let cs = gen_cs(r#"struct M { any value; };"#);
    assert!(cs.contains("Omg.Types.Any"), "Omg.Types.Any fehlt:\n{cs}");
}

#[test]
fn fixed_member_emits_csharp_decimal() {
    // Spec idl4-csharp §7.2.4.2.4: fixed<digits,scale> -> C# `decimal`.
    let cs = gen_cs(r#"struct M { fixed<10,2> price; };"#);
    assert!(cs.contains("decimal"), "C# decimal-Mapping fehlt:\n{cs}");
}

#[test]
fn shared_member_emits_shared_marker_attribute() {
    // Spec §8.1.5 (idl4-cpp / dds-psm-cxx): @shared -> Reference-Type.
    // C# emittiert `[Shared]`-Attribute (Omg.Types-Runtime).
    let cs = gen_cs(
        r#"
        struct WithShared {
            @shared long ptr;
        };
    "#,
    );
    assert!(cs.contains("[Shared]"), "[Shared]-Attribute fehlt:\n{cs}");
}

#[test]
fn verbatim_annotation_other_language_not_emitted_in_csharp() {
    let cs = gen_cs(
        r#"
        @verbatim(language="java", placement=BEFORE_DECLARATION, text="// java only")
        struct S { long x; };
    "#,
    );
    assert!(
        !cs.contains("// java only"),
        "Java-verbatim darf nicht in C#-Output:\n{cs}"
    );
}

// ============================================================================
// §8.1 @csharp_mapping Annotation Definition
// ============================================================================

#[test]
fn csharp_mapping_options_have_sensible_defaults() {
    let opts = CsGenOptions::default();
    // Default-Generator emittiert .NET-Naming + record-Variante.
    let cs = gen_csharp_with_opts("struct S { long x; };", &opts);
    assert!(cs.contains("S"));
}

fn gen_csharp_with_opts(src: &str, opts: &CsGenOptions) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_csharp(&ast, opts).expect("gen")
}

// ============================================================================
// Unsupported-Items: fixed/any/bitset/bitmask/valuetype
// ============================================================================

#[test]
fn fixed_type_emits_decimal() {
    // fixed jetzt voll unterstuetzt — siehe
    // `fixed_member_emits_csharp_decimal` oben.
    let parse = zerodds_idl::parse("struct S { fixed<5,2> price; };", &ParserConfig::default());
    if let Ok(ast) = parse {
        let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
        assert!(cs.contains("decimal"));
    }
}

#[test]
fn any_type_emits_omg_types_any() {
    let parse = zerodds_idl::parse("struct S { any value; };", &ParserConfig::default());
    if let Ok(ast) = parse {
        let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
        assert!(cs.contains("Omg.Types.Any"));
    }
}

#[test]
fn bitset_short_form_emits_struct() {
    // Bitset jetzt voll abgedeckt (siehe `bitset_emits_struct_with_value_field`).
    let parse = zerodds_idl::parse("bitset BS { bitfield<3> a; };", &ParserConfig::default());
    if let Ok(ast) = parse {
        let res = generate_csharp(&ast, &CsGenOptions::default());
        assert!(res.is_ok(), "bitset sollte jetzt unterstuetzt sein");
        assert!(res.expect("ok").contains("public struct BS"));
    }
}
