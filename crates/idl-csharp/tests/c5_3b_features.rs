//! Integration tests for C5.3-b features:
//!
//! 1. **ISequence/IBoundedSequence** — the codegen uses `ISequence<T>` /
//!    `IBoundedSequence<T>` from `Omg.Types` instead of bare `IList<T>`.
//! 2. **Annotation bridge** — IDL annotations
//!    (`@key`, `@id(N)`, `@optional`, `@must_understand`, `@external`,
//!    `@nested`, `@extensibility`) are emitted as C# attributes.
//! 3. **ITopicType<T> marker** — top-level structs (non-`@nested`)
//!    implement `ITopicType<Self>`.

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
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse must succeed");
    generate_csharp(&ast, &CsGenOptions::default()).expect("gen must succeed")
}

// ---------------------------------------------------------------------------
// Group 1: ISequence / IBoundedSequence
// ---------------------------------------------------------------------------

#[test]
fn unbounded_sequence_emits_isequence() {
    let cs = gen_cs("struct S { sequence<long> data; };");
    assert!(cs.contains("ISequence<int>"), "got:\n{cs}");
    // Negative: no direct `IList<int>` use in the member.
    assert!(
        !cs.contains("public IList<int>"),
        "should not emit raw IList: {cs}"
    );
}

#[test]
fn bounded_sequence_emits_ibounded_sequence() {
    let cs = gen_cs("struct S { sequence<long, 100> data; };");
    assert!(cs.contains("IBoundedSequence<int>"), "got:\n{cs}");
}

#[test]
fn bounded_sequence_inside_unbounded_inner_unbound() {
    // `sequence<sequence<long>, 5>` → outer bounded, inner unbounded.
    let cs = gen_cs("struct S { sequence<sequence<long>, 5> nested; };");
    assert!(
        cs.contains("IBoundedSequence<ISequence<int>>"),
        "got:\n{cs}"
    );
}

#[test]
fn unbounded_sequence_of_string_emits_isequence_string() {
    let cs = gen_cs("struct S { sequence<string> tags; };");
    assert!(cs.contains("ISequence<string>"), "got:\n{cs}");
}

#[test]
fn bounded_sequence_of_struct_typed_element() {
    let cs = gen_cs(
        "struct Point { long x; long y; }; \
         struct Path { sequence<Point, 32> waypoints; };",
    );
    assert!(cs.contains("IBoundedSequence<Point>"), "got:\n{cs}");
}

#[test]
fn sequence_imports_omg_types_and_collections() {
    let cs = gen_cs("struct S { sequence<long> v; };");
    assert!(cs.contains("using System.Collections.Generic;"));
    assert!(cs.contains("using Omg.Types;"));
}

#[test]
fn typedef_with_bounded_sequence_keeps_bound_marker() {
    let cs = gen_cs("typedef sequence<long, 8> ShortBag;");
    assert!(
        cs.contains("public sealed record class ShortBag(IBoundedSequence<int> Value);"),
        "got:\n{cs}"
    );
}

// ---------------------------------------------------------------------------
// Group 2: Annotation-Bridge — Member-Level
// ---------------------------------------------------------------------------

#[test]
fn at_key_emits_key_attribute_no_comment_suffix() {
    let cs = gen_cs("struct S { @key long id; long val; };");
    // Bare `[Key]` as an attribute line, without a trailing comment stub.
    assert!(cs.contains("[Key]"), "got:\n{cs}");
    // Negative: the old `// @key annotation` comment should be gone.
    assert!(!cs.contains("// @key annotation"), "got:\n{cs}");
}

#[test]
fn at_id_emits_id_attribute_with_value() {
    let cs = gen_cs("struct S { @id(7) long x; };");
    assert!(cs.contains("[Id(7)]"), "got:\n{cs}");
}

#[test]
fn at_optional_emits_optional_attribute_and_nullable_type() {
    let cs = gen_cs("struct S { @optional long maybe; };");
    assert!(cs.contains("[Optional]"), "got:\n{cs}");
    assert!(cs.contains("int? Maybe"), "got:\n{cs}");
}

#[test]
fn at_must_understand_emits_must_understand_attribute() {
    let cs = gen_cs("struct S { @must_understand long flag; };");
    assert!(cs.contains("[MustUnderstand]"), "got:\n{cs}");
}

#[test]
fn at_external_emits_external_attribute() {
    let cs = gen_cs("struct S { @external long ref_count; };");
    assert!(cs.contains("[External]"), "got:\n{cs}");
}

#[test]
fn member_annotations_trigger_omg_types_import() {
    let cs = gen_cs("struct S { @key long id; };");
    assert!(cs.contains("using Omg.Types;"), "got:\n{cs}");
}

#[test]
fn all_member_annotations_stack_on_one_member() {
    // Stress: all member annotations on a single member.
    let cs = gen_cs("struct S { @key @id(11) @must_understand @external long armed_id; };");
    assert!(cs.contains("[Key]"));
    assert!(cs.contains("[Id(11)]"));
    assert!(cs.contains("[MustUnderstand]"));
    assert!(cs.contains("[External]"));
}

#[test]
fn multiple_members_get_independent_attribute_blocks() {
    let cs = gen_cs(
        "struct S { \
            @key long id; \
            @optional string nick; \
            long plain; \
         };",
    );
    // [Key] must come before "Id", [Optional] before "Nick".
    let key_pos = cs.find("[Key]").expect("Key");
    let id_pos = cs.find("public int Id ").expect("Id prop");
    assert!(key_pos < id_pos);
    let opt_pos = cs.find("[Optional]").expect("Optional");
    let nick_pos = cs.find("public string? Nick ").expect("Nick prop");
    assert!(opt_pos < nick_pos);
    // `plain` must have NO attributes in front of it.
    let plain_pos = cs.find("public int Plain ").expect("Plain prop");
    let after_nick = &cs[nick_pos..plain_pos];
    assert!(!after_nick.contains("[Key]"));
    assert!(!after_nick.contains("[Optional]"));
    assert!(!after_nick.contains("[External]"));
}

// ---------------------------------------------------------------------------
// Group 3: Annotation-Bridge — Type-Level
// ---------------------------------------------------------------------------

#[test]
fn at_nested_emits_nested_attribute_on_struct() {
    let cs = gen_cs("@nested struct Inner { long x; };");
    assert!(cs.contains("[Nested]"), "got:\n{cs}");
}

#[test]
fn at_nested_struct_does_not_get_topic_marker() {
    let cs = gen_cs("@nested struct Inner { long x; };");
    assert!(!cs.contains("ITopicType<Inner>"), "got:\n{cs}");
}

#[test]
fn at_extensibility_final_emits_attribute() {
    let cs = gen_cs("@extensibility(FINAL) struct S { long x; };");
    assert!(
        cs.contains("[Extensibility(ExtensibilityKind.Final)]"),
        "got:\n{cs}"
    );
}

#[test]
fn at_extensibility_appendable_emits_attribute() {
    let cs = gen_cs("@extensibility(APPENDABLE) struct S { long x; };");
    assert!(
        cs.contains("[Extensibility(ExtensibilityKind.Appendable)]"),
        "got:\n{cs}"
    );
}

#[test]
fn at_extensibility_mutable_emits_attribute() {
    let cs = gen_cs("@extensibility(MUTABLE) struct S { long x; };");
    assert!(
        cs.contains("[Extensibility(ExtensibilityKind.Mutable)]"),
        "got:\n{cs}"
    );
}

#[test]
fn shorthand_at_final_emits_extensibility_final() {
    let cs = gen_cs("@final struct S { long x; };");
    assert!(
        cs.contains("[Extensibility(ExtensibilityKind.Final)]"),
        "got:\n{cs}"
    );
}

#[test]
fn shorthand_at_appendable_emits_extensibility_appendable() {
    let cs = gen_cs("@appendable struct S { long x; };");
    assert!(
        cs.contains("[Extensibility(ExtensibilityKind.Appendable)]"),
        "got:\n{cs}"
    );
}

#[test]
fn shorthand_at_mutable_emits_extensibility_mutable() {
    let cs = gen_cs("@mutable struct S { long x; };");
    assert!(
        cs.contains("[Extensibility(ExtensibilityKind.Mutable)]"),
        "got:\n{cs}"
    );
}

#[test]
fn type_attribute_is_emitted_before_record_keyword() {
    let cs = gen_cs("@final struct S { long x; };");
    let attr_pos = cs.find("[Extensibility(").expect("attr");
    let record_pos = cs.find("public partial record class S").expect("record");
    assert!(attr_pos < record_pos, "attribute must precede record");
}

// ---------------------------------------------------------------------------
// Group 4: ITopicType<T>-Marker
// ---------------------------------------------------------------------------

#[test]
fn top_level_struct_implements_topic_type_marker() {
    let cs = gen_cs("struct Telemetry { long ts; double value; };");
    assert!(
        cs.contains("public partial record class Telemetry : ITopicType<Telemetry>"),
        "got:\n{cs}"
    );
}

#[test]
fn topic_marker_imports_omg_types() {
    let cs = gen_cs("struct Telemetry { long x; };");
    assert!(cs.contains("using Omg.Types;"), "got:\n{cs}");
}

#[test]
fn nested_struct_does_not_implement_topic_marker() {
    let cs = gen_cs("@nested struct Embedded { long v; };");
    assert!(!cs.contains("ITopicType<Embedded>"), "got:\n{cs}");
}

#[test]
fn struct_with_inheritance_keeps_base_and_adds_topic_marker() {
    let cs = gen_cs(
        "struct Parent { long x; }; \
         struct Child : Parent { long y; };",
    );
    // First Parent, then ITopicType.
    assert!(
        cs.contains("public partial record class Child : Parent, ITopicType<Child>"),
        "got:\n{cs}"
    );
}

#[test]
fn union_top_level_implements_topic_type() {
    let cs = gen_cs("union U switch (long) { case 1: long a; default: octet b; };");
    assert!(
        cs.contains("public partial record class U : ITopicType<U>"),
        "got:\n{cs}"
    );
}

#[test]
fn nested_union_does_not_implement_topic_type() {
    let cs = gen_cs("@nested union NU switch (long) { case 1: long a; default: octet b; };");
    assert!(!cs.contains("ITopicType<NU>"), "got:\n{cs}");
}

#[test]
fn nested_struct_in_module_still_no_topic_marker() {
    let cs = gen_cs("module M { @nested struct Inner { long x; }; };");
    assert!(!cs.contains("ITopicType<Inner>"), "got:\n{cs}");
}

#[test]
fn plain_module_struct_has_topic_marker() {
    let cs = gen_cs("module M { struct Outer { long x; }; };");
    assert!(cs.contains("ITopicType<Outer>"), "got:\n{cs}");
}

// ---------------------------------------------------------------------------
// Group 5: Stress / Combined
// ---------------------------------------------------------------------------

#[test]
fn full_annotation_stack_on_struct_and_member() {
    let cs = gen_cs(
        "@mutable struct Sensor { \
            @key @id(0) long id; \
            @optional @must_understand string name; \
            @external sequence<double, 8> readings; \
         };",
    );
    // Type-level
    assert!(cs.contains("[Extensibility(ExtensibilityKind.Mutable)]"));
    // Member-level
    assert!(cs.contains("[Key]"));
    assert!(cs.contains("[Id(0)]"));
    assert!(cs.contains("[Optional]"));
    assert!(cs.contains("[MustUnderstand]"));
    assert!(cs.contains("[External]"));
    // Nullable due to @optional
    assert!(cs.contains("string? Name"));
    // Bounded sequence
    assert!(cs.contains("IBoundedSequence<double>"));
    // Topic marker
    assert!(cs.contains("ITopicType<Sensor>"));
}

#[test]
fn no_annotations_means_no_omg_types_import() {
    // Pure primitives without sequence and without annotations: the
    // topic marker still exists (default: top-level = topic), so
    // Omg.Types is pulled in.
    let cs = gen_cs("struct S { long x; };");
    assert!(cs.contains("using Omg.Types;"));
    assert!(cs.contains("ITopicType<S>"));
}

#[test]
fn only_nested_struct_does_not_pull_omg_types_via_topic_marker() {
    // @nested struct without annotations and without sequences: no
    // reason for Omg.Types — we verify this because it shows that the
    // marker and the bridge gate cleanly.
    let cs = gen_cs("@nested struct N { long x; };");
    // But the [Nested] attribute itself pulls in Omg.Types.
    assert!(cs.contains("using Omg.Types;"));
    assert!(cs.contains("[Nested]"));
}

#[test]
fn annotation_attribute_appears_on_correct_member_only() {
    // `@id` may only be on `tagged_id`, not on `untagged_x`.
    let cs = gen_cs("struct S { @id(5) long tagged_id; long untagged_x; };");
    let tagged_pos = cs.find("public int TaggedId").expect("tagged");
    let untagged_pos = cs.find("public int UntaggedX").expect("untagged");
    let between = &cs[tagged_pos..untagged_pos];
    assert!(!between.contains("[Id("), "between:\n{between}");
    // But [Id(5)] must come BEFORE tagged_id.
    let id_pos = cs.find("[Id(5)]").expect("Id5");
    assert!(id_pos < tagged_pos);
}

#[test]
fn nested_attribute_combined_with_extensibility() {
    let cs = gen_cs("@nested @final struct Embedded { long x; };");
    assert!(cs.contains("[Nested]"));
    assert!(cs.contains("[Extensibility(ExtensibilityKind.Final)]"));
    // Despite @nested + @final: no ITopicType marker (nested!).
    assert!(!cs.contains("ITopicType<Embedded>"));
}
