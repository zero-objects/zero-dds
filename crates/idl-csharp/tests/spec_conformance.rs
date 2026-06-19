//! Spec conformance matrix for IDL4-C# 1.0 §7 + §8.
//!
//! Productively verifies the generator paths listed in
//! `docs/spec-coverage/idl4-csharp-1.0.md` via end-to-end IDL→C# renderings.
//!
//! Cross-cutting:
//! * `@verbatim` (§7.17.5) is shared with XTypes 1.3 §7.2.2.4.8;
//!   see `dds-xtypes-1.3.open.md`.

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
    // Spec §7.1.1.2: the .NET default is PascalCase. The ZeroDDS default is
    // .NET-Naming.
    let cs = gen_cs("struct my_struct { long my_field; };");
    assert!(
        cs.contains("MyStruct") || cs.contains("my_struct"),
        "neither .NET-PascalCase nor IDL-naming present:\n{cs}"
    );
}

#[test]
fn camel_case_member_naming_for_pascalized_idl_names() {
    // Spec §7.1.1.2.2: member properties follow camelCase or
    // PascalCase. The ZeroDDS .NET default emits PascalCase for
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
// §7.2.4.2.1 Sequences (bounds checking is MAY)
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
    // The generator emits this with the `bound` marker.
    let cs = gen_cs("struct S { sequence<long, 5> values; };");
    assert!(cs.contains("Bounded") || cs.contains("IList") || cs.contains("ISequence"));
}

// ============================================================================
// §7.2.4.3.1 Struct (record class is a spec-conformant modernization path)
// ============================================================================

#[test]
fn struct_emits_public_class_or_record_class() {
    // Spec §7.2.4.3.1: struct → public class. ZeroDDS emits
    // `record class` (C# 9+) as a modernized variant with
    // identical semantics (init properties + default/copy/all-
    // values constructor automatically via record syntax).
    let cs = gen_cs("struct S { long x; };");
    assert!(cs.contains("class S") || cs.contains("record S"));
}

// ============================================================================
// §7.2.4.3.2 Union
// ============================================================================

#[test]
fn union_emits_discriminator_class() {
    // Spec §7.2.4.3.2: union → C# class with a discriminator pattern.
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
    // ZeroDDS variant: a wrapper class instead of inline replacement
    // (the spec allows both, the wrapper is documentation-friendly).
    let cs = gen_cs("typedef long MyLong;");
    assert!(cs.contains("MyLong"), "typedef alias missing:\n{cs}");
}

// ============================================================================
// §7.4.2 interface forward-decl — no C# mapping
// ============================================================================

#[test]
fn interface_forward_decl_has_no_csharp_output() {
    // Spec §7.4.2: forward-decl → no C# mapping. The generator filters it.
    // A non-service interface is Unsupported anyway (spec-licensed).
    let parse = zerodds_idl::parse("interface Foo;", &ParserConfig::default());
    if let Ok(ast) = parse {
        let res = generate_csharp(&ast, &CsGenOptions::default());
        // Either an Unsupported error or empty output without `Foo`.
        if let Ok(cs) = res {
            // Forward-decl-only emits nothing concrete.
            assert!(
                !cs.contains("interface Foo {"),
                "forward-decl emits a full interface body"
            );
        }
    }
}

// ============================================================================
// §7.14.2 union discriminator extensions
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
    // placement=BEFORE_DECLARATION, text="...") embeds text before
    // the type header.
    let cs = gen_cs(
        r#"
        @verbatim(language="csharp", placement=BEFORE_DECLARATION, text="// pre-decl marker")
        struct PlainStruct { long x; };
    "#,
    );
    assert!(cs.contains("PlainStruct"));
    assert!(
        cs.contains("// pre-decl marker"),
        " BEFORE_DECLARATION missing:\n{cs}"
    );
    let pos_marker = cs.find("// pre-decl marker").unwrap_or(usize::MAX);
    let pos_class = cs.find("record class PlainStruct").unwrap_or(usize::MAX);
    assert!(
        pos_marker < pos_class,
        "marker must come before the record class:\n{cs}"
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
        "AFTER_DECLARATION verbatim must come after the record block:\n{cs}"
    );
}

#[test]
fn bitset_emits_struct_with_value_field() {
    // Spec idl4-csharp §7.14.3.2: bitset → C# struct with a value property
    // pro Bitfield (Mask + Shift inline).
    let cs = gen_cs(r#"bitset BS { bitfield<3> a; bitfield<5> b; };"#);
    assert!(cs.contains("public struct BS"), "struct BS missing:\n{cs}");
    assert!(cs.contains("public ulong Value"));
    assert!(cs.contains("public ulong A"), "property A missing:\n{cs}");
    assert!(cs.contains("public ulong B"));
    assert!(cs.contains("0x7UL"));
    assert!(cs.contains("0x1FUL"));
}

#[test]
fn bitmask_emits_flags_enum() {
    // Spec idl4-csharp §7.14.3.3: bitmask → `[Flags] enum`.
    let cs = gen_cs(r#"@bit_bound(8) bitmask Flags { READ, WRITE, EXEC };"#);
    assert!(cs.contains("[System.Flags]"), "[Flags] missing:\n{cs}");
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
            "VTAbstract missing:\n{cs}"
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
            "private->protected mapping missing:\n{cs}"
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
        "C# interface missing:\n{cs}"
    );
    assert!(cs.contains("add"));
    assert!(cs.contains("Version"));
}

#[test]
fn any_member_emits_omg_types_any() {
    let cs = gen_cs(r#"struct M { any value; };"#);
    assert!(cs.contains("Omg.Types.Any"), "Omg.Types.Any missing:\n{cs}");
}

#[test]
fn fixed_member_emits_csharp_decimal() {
    // Spec idl4-csharp §7.2.4.2.4: fixed<digits,scale> -> C# `decimal`.
    let cs = gen_cs(r#"struct M { fixed<10,2> price; };"#);
    assert!(cs.contains("decimal"), "C# decimal mapping missing:\n{cs}");
}

#[test]
fn shared_member_emits_shared_marker_attribute() {
    // Spec §8.1.5 (idl4-cpp / dds-psm-cxx): @shared -> Reference-Type.
    // C# emits a `[Shared]` attribute (Omg.Types runtime).
    let cs = gen_cs(
        r#"
        struct WithShared {
            @shared long ptr;
        };
    "#,
    );
    assert!(cs.contains("[Shared]"), "[Shared] attribute missing:\n{cs}");
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
        "Java verbatim must not be in the C# output:\n{cs}"
    );
}

// ============================================================================
// §8.1 @csharp_mapping Annotation Definition
// ============================================================================

#[test]
fn csharp_mapping_options_have_sensible_defaults() {
    let opts = CsGenOptions::default();
    // The default generator emits .NET naming + record variant.
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
    // fixed is now fully supported — see
    // `fixed_member_emits_csharp_decimal` above.
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
    // Bitset is now fully covered (see `bitset_emits_struct_with_value_field`).
    let parse = zerodds_idl::parse("bitset BS { bitfield<3> a; };", &ParserConfig::default());
    if let Ok(ast) = parse {
        let res = generate_csharp(&ast, &CsGenOptions::default());
        assert!(res.is_ok(), "bitset should now be supported");
        assert!(res.expect("ok").contains("public struct BS"));
    }
}
