//! Cluster-E-Tests fuer C5.4-b — Bitset/Bitmask, Multi-Inheritance,
//! `@value`-Enum, Annotation-Bridge, TopicType-Marker.
//!
//! Markerbasierte Snapshots — siehe `tests/fixtures.rs` fuer das
//! Pattern.

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
use zerodds_idl_java::{JavaFile, JavaGenError, JavaGenOptions, generate_java_files};

fn genfiles(src: &str) -> Vec<JavaFile> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_java_files(&ast, &JavaGenOptions::default()).expect("gen")
}

fn gen_err(src: &str) -> JavaGenError {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_java_files(&ast, &JavaGenOptions::default()).expect_err("expected gen error")
}

fn find<'a>(files: &'a [JavaFile], class: &str) -> &'a JavaFile {
    files
        .iter()
        .find(|f| f.class_name == class)
        .unwrap_or_else(|| {
            panic!(
                "class {class} not in {:?}",
                files.iter().map(|f| &f.class_name).collect::<Vec<_>>()
            )
        })
}

fn join(files: &[JavaFile]) -> String {
    files
        .iter()
        .map(|f| f.source.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

// ============================================================================
// Bitmask
// ============================================================================

#[test]
fn bitmask_default_bit_bound_32() {
    let files = genfiles("bitmask Flags { READ, WRITE, EXEC };");
    let f = find(&files, "Flags");
    assert!(f.source.contains("public final class Flags"));
    assert!(f.source.contains("BIT_BOUND = 32"));
    assert!(f.source.contains("READ(0)"));
    assert!(f.source.contains("WRITE(1)"));
    assert!(f.source.contains("EXEC(2);"));
    assert!(f.source.contains("java.util.EnumSet<Flag>"));
    assert!(f.source.contains("public boolean isSet(Flag f)"));
}

#[test]
fn bitmask_bit_bound_8_emits_constant() {
    let files = genfiles("@bit_bound(8) bitmask M { A };");
    let f = find(&files, "M");
    assert!(f.source.contains("BIT_BOUND = 8"));
}

#[test]
fn bitmask_bit_bound_16_emits_constant() {
    let files = genfiles("@bit_bound(16) bitmask M { A };");
    let f = find(&files, "M");
    assert!(f.source.contains("BIT_BOUND = 16"));
}

#[test]
fn bitmask_bit_bound_64_emits_constant() {
    let files = genfiles("@bit_bound(64) bitmask M { A };");
    let f = find(&files, "M");
    assert!(f.source.contains("BIT_BOUND = 64"));
}

#[test]
fn bitmask_position_overrides_implicit_cursor() {
    let files = genfiles("@bit_bound(16) bitmask M { @position(3) A, B };");
    let f = find(&files, "M");
    // explicit `@position(3)` → A(3); next implicit B(4).
    assert!(f.source.contains("A(3)"));
    assert!(f.source.contains("B(4)"));
}

#[test]
fn bitmask_inner_enum_has_position_accessor() {
    let files = genfiles("bitmask M { A };");
    let f = find(&files, "M");
    assert!(f.source.contains("public int position()"));
}

// ============================================================================
// Bitset
// ============================================================================

#[test]
fn bitset_simple_two_fields() {
    let files = genfiles("bitset MyBits { bitfield<3> a; bitfield<5> b; };");
    let f = find(&files, "MyBits");
    assert!(f.source.contains("public final class MyBits"));
    assert!(f.source.contains("private long bits;"));
    assert!(f.source.contains("public int getA()"));
    assert!(f.source.contains("public int getB()"));
    assert!(f.source.contains("public void setA(int value)"));
    assert!(f.source.contains("public void setB(int value)"));
}

#[test]
fn bitset_a_uses_mask_0x7_offset_0() {
    let files = genfiles("bitset MyBits { bitfield<3> a; };");
    let f = find(&files, "MyBits");
    assert!(f.source.contains("0x7L"));
    assert!(f.source.contains(">>> 0)"));
}

#[test]
fn bitset_b_uses_mask_0x1f_offset_3() {
    let files = genfiles("bitset MyBits { bitfield<3> a; bitfield<5> b; };");
    let f = find(&files, "MyBits");
    assert!(f.source.contains("0x1FL"));
    // setB shifts by 3
    assert!(f.source.contains("<< 3)"));
}

#[test]
fn bitset_cumulative_64bit_filled_is_ok() {
    let files = genfiles("bitset Full { bitfield<32> a; bitfield<32> b; };");
    let f = find(&files, "Full");
    assert!(f.source.contains("BIT_WIDTH = 64"));
}

#[test]
fn bitset_over_64_returns_error() {
    let err = gen_err("bitset Big { bitfield<40> a; bitfield<30> b; };");
    match err {
        JavaGenError::UnsupportedConstruct { construct, .. } => {
            assert!(construct.contains("64"), "msg: {construct}");
        }
        other => panic!("expected UnsupportedConstruct, got {other:?}"),
    }
}

#[test]
fn bitset_64bit_field_returns_long_typed_accessor() {
    let files = genfiles("bitset Wide { bitfield<64> a; };");
    let f = find(&files, "Wide");
    assert!(f.source.contains("public long getA()"));
    assert!(f.source.contains("public void setA(long value)"));
}

#[test]
fn bitset_anonymous_padding_is_skipped_but_offsets_advance() {
    let files = genfiles("bitset P { bitfield<2> a; bitfield<3>; bitfield<4> b; };");
    let f = find(&files, "P");
    // a@0, padding@2 (3 wide), b@5
    assert!(f.source.contains("padding: width=3 offset=2"));
    assert!(f.source.contains("offset=5"));
    assert!(f.source.contains("public int getA()"));
    assert!(f.source.contains("public int getB()"));
}

// ============================================================================
// `@value`-Enum
// ============================================================================

#[test]
fn enum_value_overrides_emit_explicit_int() {
    let files = genfiles("enum Color { @value(0) RED, @value(2) GREEN, @value(4) BLUE };");
    let f = find(&files, "Color");
    assert!(f.source.contains("RED(0),"));
    assert!(f.source.contains("GREEN(2),"));
    assert!(f.source.contains("BLUE(4);"));
}

#[test]
fn enum_value_partial_overrides_continue_from_override() {
    // After @value(10), the next implicit ordinal is 11.
    let files = genfiles("enum X { A, @value(10) B, C };");
    let f = find(&files, "X");
    assert!(f.source.contains("A(0),"));
    assert!(f.source.contains("B(10),"));
    assert!(f.source.contains("C(11);"));
}

#[test]
fn enum_value_non_ascending_is_allowed() {
    // @value-Spec gives us full freedom — non-monotonic is legal.
    let files = genfiles("enum X { @value(5) A, @value(2) B };");
    let f = find(&files, "X");
    assert!(f.source.contains("A(5),"));
    assert!(f.source.contains("B(2);"));
}

#[test]
fn enum_without_value_keeps_sequential_ordinals() {
    let files = genfiles("enum Color { RED, GREEN, BLUE };");
    let f = find(&files, "Color");
    assert!(f.source.contains("RED(0),"));
    assert!(f.source.contains("GREEN(1),"));
    assert!(f.source.contains("BLUE(2);"));
}

// ============================================================================
// Multi-Inheritance via Interface-Pattern
// ============================================================================

#[test]
fn base_emits_companion_interface_descendant_does_not() {
    let files = genfiles("struct A { long x; }; struct B : A { long y; };");
    // A is a base of B → companion `AInterface.java` IS emitted.
    let names: Vec<&str> = files.iter().map(|f| f.class_name.as_str()).collect();
    assert!(names.contains(&"A"));
    assert!(names.contains(&"B"));
    assert!(names.contains(&"AInterface"));
    // B has no descendant → no `BInterface`.
    assert!(!names.contains(&"BInterface"));
}

#[test]
fn two_level_chain_emits_grandparent_interface_clause() {
    // A : B, B : C → A extends B implements CInterface, ...
    let files =
        genfiles("struct C { long c; }; struct B : C { long b; }; struct A : B { long a; };");
    let a = find(&files, "A");
    assert!(a.source.contains("public class A extends B"));
    assert!(a.source.contains("CInterface"));
    // B is base of A → companion BInterface.
    assert!(files.iter().any(|f| f.class_name == "BInterface"));
    // C is base of B → companion CInterface.
    assert!(files.iter().any(|f| f.class_name == "CInterface"));
}

#[test]
fn three_level_chain_emits_two_grandparent_interfaces() {
    // A : B : C : D — A extends B implements CInterface, DInterface
    let files = genfiles(
        "struct D { long d; }; struct C : D { long c; }; \
         struct B : C { long b; }; struct A : B { long a; };",
    );
    let a = find(&files, "A");
    assert!(a.source.contains("public class A extends B"));
    assert!(a.source.contains("CInterface"));
    assert!(a.source.contains("DInterface"));
}

#[test]
fn companion_interface_lists_getter_signatures() {
    let files = genfiles("struct Parent { long x; string s; }; struct Child : Parent { long y; };");
    let pi = find(&files, "ParentInterface");
    assert!(pi.source.contains("public interface ParentInterface"));
    assert!(pi.source.contains("int getX();"));
    assert!(pi.source.contains("String getS();"));
}

// ============================================================================
// TopicType-Marker
// ============================================================================

#[test]
fn top_level_struct_implements_topic_type() {
    let files = genfiles("struct S { long x; };");
    let f = find(&files, "S");
    assert!(
        f.source
            .contains("implements org.omg.dds.topic.TopicType<S>")
    );
}

#[test]
fn nested_struct_does_not_implement_topic_type() {
    let files = genfiles("@nested struct Inner { long x; };");
    let f = find(&files, "Inner");
    assert!(!f.source.contains("TopicType"));
    // and carries the @Nested type-annotation.
    assert!(f.source.contains("@org.zerodds.types.Nested"));
}

#[test]
fn struct_with_base_inherits_topic_type_from_parent() {
    // TS-3-Finding 4 (gefixt 2026-05-01): bei Inheritance wird
    // `TopicType<T>` nur am Wurzel-Type emittiert. Java verbietet
    // `Child extends Base<Base> implements TopicType<Child>` weil
    // beide TopicType-Generic-Args kollidieren. Spec DDS-Java-PSM
    // betrachtet TopicType als Marker-Interface — Sub-Classes erben
    // den Marker via `extends Parent` und sind weiterhin
    // `instanceof TopicType<Parent>`.
    let files = genfiles("struct Parent { long x; }; struct Child : Parent { long y; };");
    let parent = find(&files, "Parent");
    let child = find(&files, "Child");
    assert!(
        parent.source.contains("TopicType<Parent>"),
        "Parent must implement TopicType (Wurzel der Kette)"
    );
    assert!(child.source.contains("extends Parent"));
    assert!(
        !child.source.contains("TopicType"),
        "Child must NOT re-implement TopicType — inherits via extends Parent"
    );
}

// ============================================================================
// Annotation-Bridge
// ============================================================================

#[test]
fn key_annotation_emits_java_annotation() {
    let files = genfiles("struct S { @key long id; long val; };");
    let f = find(&files, "S");
    assert!(f.source.contains("@org.zerodds.types.Key"));
}

#[test]
fn id_annotation_includes_value() {
    let files = genfiles("struct S { @id(7) long x; };");
    let f = find(&files, "S");
    assert!(f.source.contains("@org.zerodds.types.Id(7)"));
}

#[test]
fn must_understand_annotation_emits() {
    let files = genfiles("struct S { @must_understand long x; };");
    let f = find(&files, "S");
    assert!(f.source.contains("@org.zerodds.types.MustUnderstand"));
}

#[test]
fn external_annotation_emits() {
    let files = genfiles("struct S { @external long x; };");
    let f = find(&files, "S");
    assert!(f.source.contains("@org.zerodds.types.External"));
}

#[test]
fn optional_annotation_emits_marker_in_addition_to_optional_type() {
    let files = genfiles("struct S { @optional long x; };");
    let f = find(&files, "S");
    assert!(f.source.contains("@org.zerodds.types.Optional"));
    assert!(f.source.contains("java.util.Optional"));
}

#[test]
fn extensibility_final_emits_type_annotation() {
    let files = genfiles("@final struct S { long x; };");
    let f = find(&files, "S");
    assert!(f.source.contains(
        "@org.zerodds.types.Extensibility(\
        org.zerodds.types.Extensibility.Kind.FINAL)"
    ));
}

#[test]
fn extensibility_appendable_explicit_form() {
    let files = genfiles("@extensibility(APPENDABLE) struct S { long x; };");
    let f = find(&files, "S");
    assert!(f.source.contains("Extensibility.Kind.APPENDABLE"));
}

#[test]
fn extensibility_mutable_emits_type_annotation() {
    let files = genfiles("@mutable struct S { long x; };");
    let f = find(&files, "S");
    assert!(f.source.contains("Extensibility.Kind.MUTABLE"));
}

#[test]
fn nested_annotation_on_enum_is_emitted() {
    let files = genfiles("@nested enum E { A };");
    let f = find(&files, "E");
    assert!(f.source.contains("@org.zerodds.types.Nested"));
}

#[test]
fn key_id_and_optional_appear_in_deterministic_order() {
    let files = genfiles("struct S { @optional @id(3) @key long x; };");
    let f = find(&files, "S");
    let blob = &f.source;
    let key_pos = blob.find("@org.zerodds.types.Key").expect("Key");
    let id_pos = blob.find("@org.zerodds.types.Id(3)").expect("Id");
    let opt_pos = blob.find("@org.zerodds.types.Optional").expect("Optional");
    assert!(key_pos < id_pos, "Key before Id");
    assert!(id_pos < opt_pos, "Id before Optional");
}

#[test]
fn multiple_annotations_combined_test() {
    // Big mixed scenario hits the full bridge.
    let files = genfiles(
        "@mutable struct S { \
           @key @id(1) long topic_key; \
           @optional @must_understand @external long maybe; \
         };",
    );
    let blob = join(&files);
    assert!(blob.contains("Extensibility.Kind.MUTABLE"));
    assert!(blob.contains("@org.zerodds.types.Key"));
    assert!(blob.contains("@org.zerodds.types.Id(1)"));
    assert!(blob.contains("@org.zerodds.types.Optional"));
    assert!(blob.contains("@org.zerodds.types.MustUnderstand"));
    assert!(blob.contains("@org.zerodds.types.External"));
}
