// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `DdsType` bridge: what the generator emits, and whether it
//! compiles and puts the right octets on the wire.
//!
//! The text assertions here run everywhere. Several tests that build a crate
//! around the generated module are `#[ignore]`d and shell out to cargo,
//! the same shape as `crates/corba-rust/tests/compile_check.rs`: they
//! need a populated registry and a writable target directory, and the
//! first of them compiles the whole DCPS graph and the generator as build
//! dependencies. The weekly job `integration-codegen-build` builds that
//! graph once (`tests/integration/warm_build_consumer.sh`) and then runs
//! them, each within its 25 s. The external-storage
//! regression is also a generated consumer build, but runs by default.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_build::Config;

/// Compile one `.idl` with the bridge on and hand back the module text.
fn generated(name: &str, idl_source: &str) -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    generated_in(dir.path(), name, idl_source)
}

fn generated_in(dir: &Path, name: &str, idl_source: &str) -> String {
    let idl = dir.join(format!("{name}.idl"));
    fs::write(&idl, idl_source).expect("write idl");
    Config::new()
        .out_dir(dir)
        .emit_cargo_directives(false)
        .dds_type_impls(true)
        .compile(&[&idl])
        .expect("compile");
    fs::read_to_string(dir.join(format!("{name}.rs"))).expect("the module")
}

/// DDS-XTypes 1.3 §7.2.2.4.4.4.8 and Table 21 put `@key` on a union's
/// discriminator: the build carries it, and the union is a keyed topic
/// type whose key holder is the discriminator. The same union without
/// `@key` stays keyless.
#[test]
fn xtypes_7_2_2_4_4_4_8_a_union_keyed_by_its_discriminator_is_a_keyed_topic() {
    let keyed = generated(
        "Command",
        "@final union Command switch (@key long) { case 0: long value; default: long other; };",
    );
    assert!(keyed.contains("impl zerodds_dcps::DdsType for Command"));
    assert!(keyed.contains("const HAS_KEY: bool = true"), "{keyed}");
    assert!(keyed.contains("pub fn key_holder_be(&self)"), "{keyed}");
    assert!(keyed.contains("pub fn encode_key_holder(&self"), "{keyed}");

    let keyless = generated(
        "Command",
        "@final union Command switch (long) { case 0: long value; default: long other; };",
    );
    assert!(keyless.contains("impl zerodds_dcps::DdsType for Command"));
    assert!(!keyless.contains("const HAS_KEY: bool = true"));
}

/// The option is off unless asked for, so a consumer that wants the
/// codec alone keeps a dependency graph with no DCPS in it.
#[test]
fn the_bridge_is_absent_until_the_option_is_set() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Robot.idl");
    fs::write(&idl, "struct Robot { uint32 id; };").expect("write idl");

    Config::new()
        .out_dir(dir.path())
        .emit_cargo_directives(false)
        .compile(&[&idl])
        .expect("compile");
    let without = fs::read_to_string(dir.path().join("Robot.rs")).expect("the module");
    assert!(
        !without.contains("zerodds_dcps"),
        "the default output must not name the DCPS crate: {without}"
    );

    let with = generated_in(dir.path(), "Robot", "struct Robot { uint32 id; };");
    assert!(
        with.contains("impl zerodds_dcps::DdsType for Robot"),
        "{with}"
    );
}

/// All four pairs, and the alignment cap each XCDR version fixes.
///
/// XTypes 1.3 §7.4.1 gives `MAXALIGN(VERSION2) = 4` and
/// `MAXALIGN(VERSION1) = 8`, and the generated codec emits no alignment
/// of its own — the `Writer` and the `Reader` do it, capped at what they
/// were constructed with. A pair built with the wrong cap writes a
/// `double` behind a `long` at the wrong offset and says nothing.
#[test]
fn every_codec_pair_is_overridden_with_its_own_alignment_cap() {
    let text = generated("Pose", "@final struct Pose { int32 id; double x; };");
    for method in [
        "fn encode(",
        "fn encode_be(",
        "fn decode(",
        "fn decode_be(",
        "fn encode_xcdr1(",
        "fn encode_xcdr1_be(",
        "fn decode_xcdr1(",
        "fn decode_xcdr1_be(",
    ] {
        assert!(text.contains(method), "`{method}` is missing:\n{text}");
    }
    assert_eq!(
        text.matches("zerodds_wire::Endianness::Big,\n            4,")
            .count(),
        2,
        "one big-endian XCDR2 writer and one reader, both capped at four:\n{text}"
    );
    assert_eq!(
        text.matches("zerodds_wire::Endianness::Big,\n            8,")
            .count(),
        2,
        "one big-endian XCDR1 writer and one reader, both capped at eight:\n{text}"
    );
}

/// A generated type continues a sample behind a prefix someone else wrote
/// at any offset: `encode_continuing` and `decode_continuing` are its own,
/// not the trait's, which refuse an offset that is not a multiple of the
/// alignment cap. DDS-RPC 1.0 §7.5.1.1 puts a reply behind a `ReplyHeader`
/// of 28 octets; under XCDR1 the trait's default refused every generated
/// reply type there.
#[test]
fn the_continuing_pair_is_overridden() {
    let text = generated("Reply", "@final struct Reply { double v; };");
    for method in ["fn encode_continuing(", "fn decode_continuing("] {
        assert!(text.contains(method), "`{method}` is missing:\n{text}");
    }
    assert_eq!(
        text.matches("zerodds_wire::Writer::continuing(").count(),
        2,
        "one continuing writer per version:\n{text}"
    );
}

#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn a_generated_type_continues_behind_a_reply_header() {
    run_generated(
        "continuing",
        "@final struct Reply { double v; };",
        r#"
        #[test]
        fn a_double_behind_28_octets_aligns_to_the_sample() {
            use zerodds_dcps::DdsType;
            let value = Reply { v: 2.5 };
            // XCDR1: the double aligns to 8 counted from the sample's start,
            // four octets of padding behind the 28.
            let mut v1 = vec![0xAAu8; 28];
            value.encode_continuing(&mut v1, true).unwrap();
            assert_eq!(v1.len(), 40);
            assert_eq!(&v1[28..32], &[0, 0, 0, 0]);
            assert_eq!(Reply::decode_continuing(&v1, 28, true).unwrap().v, 2.5);
            // XCDR2: the cap is 4, and 28 is a multiple of it.
            let mut v2 = vec![0xAAu8; 28];
            value.encode_continuing(&mut v2, false).unwrap();
            assert_eq!(v2.len(), 36);
            assert_eq!(Reply::decode_continuing(&v2, 28, false).unwrap().v, 2.5);
        }
        "#,
    );
}

/// The impl never claims the two versions coincide.
///
/// `XCDR1_MATCHES_XCDR2` exists so a hand-written type may let the
/// trait's XCDR1 default delegate to the XCDR2 codec. A generated type
/// has no use for it: it either carries its own XCDR1 pair or it carries
/// none, and asserting the equivalence would replace a refusal with
/// version-2 octets under a `CDR_LE` header.
#[test]
fn the_bridge_never_asserts_the_two_versions_are_one_stream() {
    let text = generated(
        "Mixed",
        "@appendable struct Mixed { int32 a; double b; sequence<int32> c; };",
    );
    assert!(!text.contains("XCDR1_MATCHES_XCDR2"), "{text}");
}

/// `@key` reaches `HAS_KEY`, the holder and §7.6.8 step 5's branch.
#[test]
fn dcps_2_2_2_3_6_a_keyed_type_carries_its_holder_and_the_step_five_branch() {
    let bounded = generated("Small", "@final struct Small { @key uint32 id; int32 v; };");
    assert!(bounded.contains("const HAS_KEY: bool = true;"), "{bounded}");
    assert!(
        bounded.contains("const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(16);"),
        "{bounded}"
    );
    assert!(
        bounded.contains("Small::key_holder_be(self)"),
        "the holder comes from the generated plan, not from a second walk: {bounded}"
    );

    // An unbounded `@key string` has no maximum, so step 5 takes the MD5
    // branch — which `KEY_HOLDER_MAX_SIZE = None` is how the trait says.
    let unbounded = generated("Named", "@final struct Named { @key string topic; };");
    assert!(
        unbounded.contains("const KEY_HOLDER_MAX_SIZE: Option<usize> = None;"),
        "{unbounded}"
    );
}

/// `@nested` reaches `IS_NESTED`, which is what `create_topic` refuses on.
#[test]
fn a_nested_type_says_so() {
    let text = generated(
        "Nest",
        "@nested struct Inner { int32 a; }; struct Outer { Inner part; };",
    );
    let inner = text
        .split("impl zerodds_dcps::DdsType for Inner")
        .nth(1)
        .expect("the inner impl");
    assert!(inner.contains("const IS_NESTED: bool = true;"), "{inner}");
    let outer = text
        .split("impl zerodds_dcps::DdsType for Outer")
        .nth(1)
        .expect("the outer impl");
    assert!(!outer.contains("IS_NESTED"), "{outer}");
}

/// The identifier and the object are the **same** form.
///
/// `DcpsRuntime::register_type_object` keys the registry by the hash it
/// computes from the object it is handed. A complete hash over a minimal
/// object is an entry no lookup of this endpoint's own identifier finds.
#[test]
fn the_identifier_and_the_object_are_both_complete() {
    let text = generated("Ident", "@final struct Ident { int32 a; string b; };");
    assert!(
        text.contains("zerodds_types::TypeIdentifier::EquivalenceHashComplete("),
        "{text}"
    );
    assert!(
        text.contains("fn type_object() -> Option<zerodds_types::type_object::TypeObject>"),
        "{text}"
    );
}

/// A type in a strongly connected component is identified by the
/// component and its position (XTypes 1.3 §7.3.4.9.3) and hands the whole
/// component to the runtime. It used to carry no `TYPE_IDENTIFIER` at all,
/// and matching fell back to the type name.
#[test]
fn a_recursive_type_is_identified_by_its_component_and_hands_it_over() {
    let text = generated(
        "Recursive",
        "module m { @appendable struct Node { long value; sequence<Node> children; }; };",
    );
    assert!(
        text.contains("zerodds_types::TypeIdentifier::StronglyConnectedComponent("),
        "{text}"
    );
    assert!(text.contains("scc_length: 1,"), "{text}");
    assert!(text.contains("scc_index: 1,"), "{text}");
    assert!(
        text.contains("fn type_component() -> Option<zerodds_types::resolve::TypeComponent>"),
        "{text}"
    );
    assert!(
        !text.contains("No `TYPE_IDENTIFIER`"),
        "the identifier is spelled, not left to the type name: {text}"
    );
}

/// The generated component registers and announces: the identifier is a
/// component, the component has the length the identifier states in both
/// forms, and a registry that files it can build the type's
/// TypeInformation.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn a_recursive_type_registers_and_announces_its_component() {
    run_generated(
        "recursiveannounce",
        "module m { struct B; @appendable struct A { long value; sequence<B> bs; };          @appendable struct B { long weight; sequence<A> items; }; };",
        r#"
        #[test]
        fn the_component_registers_and_the_type_information_names_it() {
            use zerodds_dcps::DdsType;
            let zerodds_types::TypeIdentifier::StronglyConnectedComponent(own) =
                <m::B as DdsType>::TYPE_IDENTIFIER.clone()
            else {
                panic!("a recursive type is identified by its component");
            };
            assert_eq!((own.scc_length, own.scc_index), (2, 2), "B sorts second");
            let component = <m::B as DdsType>::type_component().expect("the component");
            assert_eq!(component.complete.len(), 2);
            assert_eq!(component.minimal.len(), 2);
            assert_eq!(component.complete_hash, own.hash);
            let mut registry = zerodds_types::resolve::TypeRegistry::new();
            registry.insert_type_component(&component);
            let information = zerodds_types::TypeInformation::from_component(&own, &registry)
                .expect("the type information of the component");
            assert_eq!(
                information.complete.typeid_with_size.type_id,
                zerodds_types::TypeIdentifier::StronglyConnectedComponent(own)
            );
        }
        "#,
    );
}

/// A dotted path reaches the nested type's own impl. DDS 1.4 §B.2.1.
#[test]
fn field_value_walks_into_a_nested_member() {
    let text = generated(
        "Filter",
        "module common { module util { struct Location { double x; double y; double z; }; }; \
         struct Robot { ::common::util::Location location; string name; }; };",
    );
    assert!(
        text.contains(
            "(\"location\", Some(rest)) => zerodds_dcps::DdsType::field_value(&self.location, rest),"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "(\"name\", None) => Some(zerodds_dcps::FilterValue::String(self.name.clone())),"
        ),
        "{text}"
    );
}

/// DDS-XTypes 1.3 §7.6.7.2: an optional member that is absent is equal to
/// `null` in a filter. The generated `field_value` had no arm for an
/// optional member at all, so a filter on one denied every sample.
#[test]
fn xtypes_7_6_7_2_an_absent_optional_member_is_null_to_a_filter() {
    let text = generated(
        "OptionalFilter",
        "@final struct S { @optional long a; @optional string b; long c; \
         @optional @external short d; };",
    );
    assert!(
        text.contains("(\"a\", None) => Some(match &self.a {"),
        "{text}"
    );
    assert!(
        text.contains("(\"b\", None) => Some(match &self.b {"),
        "{text}"
    );
    assert!(
        text.contains("None => zerodds_dcps::FilterValue::Null,"),
        "{text}"
    );
    assert!(
        text.contains("let v = **v;"),
        "the boxed one is unboxed: {text}"
    );
}

#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_6_7_2_the_generated_filter_value_of_an_optional_member_is_null_or_the_value() {
    run_generated(
        "optionalfilter",
        "@final struct S { @optional long a; @optional string b; long c; };",
        r#"
        #[test]
        fn absent_is_null_and_present_is_the_value() {
            use zerodds_dcps::{DdsType, FilterValue};
            let absent = S { a: None, b: None, c: 1 };
            assert!(matches!(absent.field_value("a"), Some(FilterValue::Null)));
            assert!(matches!(absent.field_value("b"), Some(FilterValue::Null)));
            assert!(matches!(absent.field_value("c"), Some(FilterValue::Int(1))));
            assert!(absent.field_value("nosuchmember").is_none());
            let present = S { a: Some(5), b: Some("x".to_owned()), c: 1 };
            assert!(matches!(present.field_value("a"), Some(FilterValue::Int(5))));
            assert!(matches!(present.field_value("b"), Some(FilterValue::String(s)) if s == "x"));
        }
        "#,
    );
}

/// Everything the generated codec calls for, other than the byte order
/// and the alignment cap, is already the constructor's default.
///
/// In the specification dialect the emitted `Writer::with_max_align(order,
/// cap)` sets two things and leaves five — the PL_CDR1 header form, the
/// `wchar` width and framing, the `wstring` unit order and the short
/// length codes — to `zerodds_wire`'s own defaults, because the emitter
/// spells only the settings that differ from them. That they coincide with
/// the specification dialect is a fact about two crates neither of which
/// knows about the other, so it is pinned rather than assumed: the day one
/// of them moves, this fails instead of a `@mutable` XCDR1 sample going out
/// with the wrong parameter headers.
#[test]
fn the_writer_defaults_are_the_specification_dialect() {
    use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};

    let key = LayoutKey::data(Encoding::PlCdr1, Endianness::Little);
    let writer = zerodds_wire::Writer::with_max_align(zerodds_wire::Endianness::Little, 8);

    let framing = writer.pl_cdr1_framing();
    assert_eq!(
        framing.pad_declared_length,
        key.pl_cdr1_headers.pads_declared_length()
    );
    assert_eq!(
        framing.pad_declared_optional_length,
        key.pl_cdr1_headers.pads_declared_optional_length()
    );
    assert_eq!(
        framing.pad_after_value,
        key.pl_cdr1_headers.pads_after_value()
    );
    assert_eq!(
        framing.require_extended,
        key.pl_cdr1_headers.always_extended()
    );
    assert_eq!(
        framing.highest_short_id,
        key.short_pid_range.highest_short_id()
    );
    assert_eq!(
        framing.member_must_understand,
        key.must_understand.writes_flag()
    );
    assert_eq!(
        framing.pid_flags,
        zerodds_wire::PidFlagPolicy::SentinelMustUnderstand
    );
    assert_eq!(
        key.pid_flags,
        zerodds_ast_layout::key::PidFlagPolicy::SentinelMustUnderstand
    );

    assert_eq!(writer.wchar_width(), zerodds_wire::WcharWidth::Two);
    assert_eq!(key.wchar_width, zerodds_ast_layout::key::WcharWidth::Two);
    assert_eq!(writer.wchar_framing(), zerodds_wire::WcharFraming::Fixed);
    assert_eq!(
        key.wchar_framing,
        zerodds_ast_layout::key::WcharFraming::Fixed
    );
    assert_eq!(
        writer.wstring_unit_order(),
        zerodds_wire::WstringUnitOrder::Stream
    );
    assert_eq!(
        key.wstring_unit_order,
        zerodds_ast_layout::key::WstringUnitOrder::Stream
    );
    assert!(!writer.short_length_codes());
    assert!(!key.length_codes.short_code_where_it_fits());

    // And the cap itself, which is the one thing the emitter states.
    assert_eq!(
        writer.max_align(),
        usize::from(Encoding::PlCdr1.max_alignment())
    );
    assert_eq!(
        zerodds_wire::Writer::with_max_align(zerodds_wire::Endianness::Little, 4).max_align(),
        usize::from(Encoding::PlainCdr2.max_alignment())
    );
}

/// `Config::dialect` reaches the reader and the writer the generated
/// `DdsType` constructs, not only the plan: in the Fast DDS dialect both
/// get the bare PID flags and the true parameter lengths, and in the
/// specification dialect neither gets a setting at all.
#[test]
fn a_dialect_reaches_the_reader_and_the_writer_the_glue_constructs() {
    const RICH_M: &str =
        "@mutable struct RichM { @key unsigned long id; double value; sequence<octet> blob; };";
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("RichM.idl");
    fs::write(&idl, RICH_M).expect("write idl");
    Config::new()
        .out_dir(dir.path())
        .emit_cargo_directives(false)
        .dds_type_impls(true)
        .dialect(zerodds_ast_layout::Dialect::FastDds)
        .compile(&[&idl])
        .expect("compile");
    let fastdds = fs::read_to_string(dir.path().join("RichM.rs")).expect("the module");
    assert!(
        fastdds.contains(".with_pl_cdr1_framing(zerodds_wire::PlCdr1Framing {"),
        "{fastdds}"
    );
    assert!(
        fastdds.contains("pid_flags: zerodds_wire::PidFlagPolicy::Bare,"),
        "{fastdds}"
    );
    let specification = generated("RichM", RICH_M);
    assert!(!specification.contains(".with_"), "{specification}");
}

/// XTypes 1.3 §7.4.1.2.1 and §7.4.1.2.2: `Config::pl_cdr1_reading` reaches
/// the reader the glue constructs, and not the writer, which has no such
/// setting. The default emits nothing.
#[test]
fn a_tolerant_pl_cdr1_reading_reaches_the_reader_and_not_the_writer() {
    const RICH_M: &str =
        "@mutable struct RichM { @key unsigned long id; double value; sequence<octet> blob; };";
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("RichM.idl");
    fs::write(&idl, RICH_M).expect("write idl");
    Config::new()
        .out_dir(dir.path())
        .emit_cargo_directives(false)
        .dds_type_impls(true)
        .pl_cdr1_reading(zerodds_ast_layout::PlCdr1Reading::Tolerant)
        .compile(&[&idl])
        .expect("compile");
    let tolerant = fs::read_to_string(dir.path().join("RichM.rs")).expect("the module");
    let readers = tolerant
        .matches(".with_pl_cdr1_reading(zerodds_wire::PlCdr1Reading::Tolerant)")
        .count();
    assert!(readers >= 1, "{tolerant}");
    let strict = generated("RichM", RICH_M);
    assert!(!strict.contains("PlCdr1Reading"), "{strict}");
}

// ---------------------------------------------------------------------
// The two that compile and run the generated module.

/// #41 — the reporter's own IDL, and the `create_topic` call they could
/// not make.
///
/// The report is that a cross-module member type leaves a generated type
/// `create_topic` will not take. Both halves are here: the module chain
/// resolves, and the type satisfies `T: DdsType`.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn dcps_2_2_2_2_1_5_issue_41_a_cross_module_type_is_a_topic_type() {
    run_generated(
        "issue41",
        "module common { module util { struct Location { double x; double y; double z; }; }; \
         struct Robot { ::common::util::Location location; }; };",
        r#"
        #[test]
        fn create_topic_takes_the_generated_type() {
            // The type parameter is the whole point: this line is what
            // #41 could not write. The participant is not started, so
            // nothing here waits on a socket.
            fn topic_type<T: zerodds_dcps::DdsType>() -> &'static str { T::TYPE_NAME }
            assert_eq!(topic_type::<common::Robot>(), "common::Robot");
            assert_eq!(topic_type::<common::util::Location>(), "common::util::Location");
        }
        "#,
    );
}

/// A bitmask member does not take the whole file down with it.
///
/// `bitmask Perm { READ, WRITE, EXEC; }` is emitted as `pub type Perm = u32`,
/// and `dds_type_impls` wrote `impl zerodds_dcps::DdsType for Perm` beside
/// it — an impl for `u32`, where neither the trait nor the type is local.
/// `error[E0117]`, and an `E0308` for every member of the aggregate that
/// followed it. Any type carrying a bitmask generated a file that did not
/// compile.
///
/// The aggregate keeps its own impl; a bitmask has no members and no key, so
/// it is a member type and never a topic type.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn a_bitmask_member_leaves_the_file_compiling() {
    run_generated(
        "bitmaskmember",
        "@bit_bound(32) bitmask Perm { READ, WRITE, EXEC }; \
         @final struct Doc { Perm access; int32 v; };",
        r#"
        #[test]
        fn the_aggregate_is_a_topic_type_and_the_bitmask_is_a_u32() {
            use zerodds_dcps::DdsType;
            let d = Doc { access: 0b101, v: 7 };
            let mut bytes = Vec::new();
            // Qualified: the generated type also carries an inherent
            // `encode(&mut Writer)`, which wins unqualified method lookup.
            <Doc as DdsType>::encode(&d, &mut bytes).expect("encode");
            let back = <Doc as DdsType>::decode(&bytes).expect("decode");
            assert_eq!(back.access, 0b101);
            assert_eq!(back.v, 7);
            assert_eq!(<Doc as DdsType>::TYPE_NAME, "Doc");
            // The bitmask itself is the alias, so it takes u32's methods.
            let p: Perm = 0b011;
            assert_eq!(p.count_ones(), 2);
        }
        "#,
    );
}

/// A `@key` member that is itself a struct, through `rustc`.
///
/// The emitter wrote `self.nested.encode(holder)` into `key_holder_be` and
/// `key_hash`, where `holder` is a local `Writer` and `encode` wants a
/// `&mut Writer`. Every keyed type whose key is an aggregate generated a file
/// that did not compile — `error[E0308]`, twice.
///
/// Nothing caught it for two reasons, and both are why this case sits here
/// and not next to the string assertions: `crates/lang-rules` renders text and
/// never compiles it, and `rust_corpus_compiles`, which does compile, runs
/// `generate::declarations` — the type declarations alone, without the codec
/// and without the holder. The corpus had the shape all along
/// (`annotation-key-whole-struct`); no target ever built it.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn an_aggregate_key_member_compiles_and_hashes() {
    run_generated(
        "aggkey",
        "@final struct Inner { int32 a; int32 b; }; \
         @final struct Outer { @key Inner nested; int32 v; };",
        r#"
        #[test]
        fn the_holder_and_the_hash_are_reachable() {
            use zerodds_dcps::DdsType;
            let s = Outer { nested: Inner { a: 1, b: 2 }, v: 3 };
            // The two functions that did not compile.
            let holder = s.key_holder_be().expect("holder");
            assert!(!holder.is_empty(), "a keyed type has holder octets");
            let hash = s.key_hash().expect("hash");
            // Two equal keys hash equally, two different keys do not —
            // otherwise instances would collide or never match.
            let same = Outer { nested: Inner { a: 1, b: 2 }, v: 99 };
            let other = Outer { nested: Inner { a: 1, b: 3 }, v: 3 };
            assert_eq!(hash, same.key_hash().expect("hash"), "the key is the key, not the sample");
            assert_ne!(hash, other.key_hash().expect("hash"));
            assert!(<Outer as DdsType>::has_key());
        }
        "#,
    );
}

/// A `map` member, through the whole consumer path.
///
/// The one member kind the Rust column had no writer for until
/// `0b2e8df9f6b:docs/codec-runtime-gap-followup.md`'s last section. This case is the
/// end of that path rather than another byte assertion: `zerodds-build`
/// generates the file, a real crate includes it, `rustc` compiles it —
/// which is what catches the path of `MapEntry` and the `Default` impl's
/// container at once — and the `DdsType` round trip closes over a
/// sample.
///
/// `map<string, long>` and not `map<long, long>`: the two halves are
/// different types, so a writer that swapped them would not compile
/// here, and the string key is the half that carries a NUL and padding.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn a_map_member_reaches_the_generated_type_and_round_trips() {
    run_generated(
        "map",
        "@appendable struct Telemetry { @key int32 unit; map<string, int32> counters; int32 tail; };",
        r#"
        use zerodds_dcps::DdsType;
        use zerodds_wire::MapEntry;

        // XTypes 1.3 §7.5.1.3.6: the Rust column holds a map as the
        // sequence of `MapEntry` of §7.4.1.1.4 (`RustMapForm::Entries`).
        fn sample() -> Telemetry {
            let counters = vec![
                MapEntry::new("hits".to_string(), 3),
                MapEntry::new("misses".to_string(), -1),
            ];
            Telemetry { unit: 7, counters, tail: 9 }
        }

        #[test]
        fn the_map_survives_every_pair() {
            for (label, encode, decode) in [
                (
                    "XCDR2 le",
                    (|s: &Telemetry, b: &mut Vec<u8>| DdsType::encode(s, b)) as fn(&Telemetry, &mut Vec<u8>) -> _,
                    (|b: &[u8]| <Telemetry as DdsType>::decode(b)) as fn(&[u8]) -> _,
                ),
                (
                    "XCDR2 be",
                    (|s: &Telemetry, b: &mut Vec<u8>| DdsType::encode_be(s, b)) as fn(&Telemetry, &mut Vec<u8>) -> _,
                    (|b: &[u8]| <Telemetry as DdsType>::decode_be(b)) as fn(&[u8]) -> _,
                ),
                (
                    "XCDR1 le",
                    (|s: &Telemetry, b: &mut Vec<u8>| DdsType::encode_xcdr1(s, b)) as fn(&Telemetry, &mut Vec<u8>) -> _,
                    (|b: &[u8]| <Telemetry as DdsType>::decode_xcdr1(b)) as fn(&[u8]) -> _,
                ),
                (
                    "XCDR1 be",
                    (|s: &Telemetry, b: &mut Vec<u8>| DdsType::encode_xcdr1_be(s, b)) as fn(&Telemetry, &mut Vec<u8>) -> _,
                    (|b: &[u8]| <Telemetry as DdsType>::decode_xcdr1_be(b)) as fn(&[u8]) -> _,
                ),
            ] {
                let mut bytes = Vec::new();
                encode(&sample(), &mut bytes).unwrap_or_else(|e| panic!("{label}: {e:?}"));
                let back = decode(&bytes).unwrap_or_else(|e| panic!("{label}: {e:?}"));
                assert_eq!(back, sample(), "{label}");
            }
        }

        /// Table 9's default for a map is an empty one.
        #[test]
        fn the_default_map_is_empty() {
            assert!(Telemetry::default().counters.is_empty());
        }
        "#,
    );
}

/// A `@mutable` member that aligns to eight, after an EMHEADER that leaves
/// the stream on a four-octet boundary. The Rust column refused it — "a
/// member of 8 octets under length code 3, not 12" — when its harness
/// handed the generated codec a `Writer` capped at eight; XCDR2 caps at
/// four (XTypes 1.3 §7.4.1), and the generated `DdsType` builds its own
/// writer with that cap. The model is the one that was refused; the
/// string's length moves the `double` through every offset modulo eight.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn a_mutable_double_after_an_emheader_round_trips_at_every_offset() {
    run_generated(
        "mutable_double",
        "enum Colour { RED, GREEN, BLUE }; \
         union Choice switch (long) { case 1: long a; case 2: string b; default: double c; }; \
         @mutable struct Inner { @key long id; @optional long maybe; string s; sequence<long> seq; \
         long arr[3]; Colour col; Choice pick; char ch; octet o; boolean b; float f; double d; }; \
         @appendable struct Outer { Inner one; sequence<Inner> many; };",
        r#"
        use zerodds_dcps::DdsType;

        fn sample(n: usize) -> Outer {
            let one = || {
                let mut one = Inner::default();
                one.id = 4;
                one.maybe = Some(5);
                one.s = "x".repeat(n);
                one.seq = vec![1, 2];
                one.f = 2.5;
                one.d = 1.5;
                one
            };
            let mut outer = Outer::default();
            outer.one = one();
            outer.many = vec![one(), one()];
            outer
        }

        #[test]
        fn every_pair_round_trips() {
            for n in 0..8 {
                for (label, encode, decode) in [
                    (
                        "XCDR2 le",
                        (|s: &Outer, b: &mut Vec<u8>| DdsType::encode(s, b)) as fn(&Outer, &mut Vec<u8>) -> _,
                        (|b: &[u8]| <Outer as DdsType>::decode(b)) as fn(&[u8]) -> _,
                    ),
                    (
                        "XCDR2 be",
                        (|s: &Outer, b: &mut Vec<u8>| DdsType::encode_be(s, b)) as fn(&Outer, &mut Vec<u8>) -> _,
                        (|b: &[u8]| <Outer as DdsType>::decode_be(b)) as fn(&[u8]) -> _,
                    ),
                    (
                        "XCDR1 le",
                        (|s: &Outer, b: &mut Vec<u8>| DdsType::encode_xcdr1(s, b)) as fn(&Outer, &mut Vec<u8>) -> _,
                        (|b: &[u8]| <Outer as DdsType>::decode_xcdr1(b)) as fn(&[u8]) -> _,
                    ),
                    (
                        "XCDR1 be",
                        (|s: &Outer, b: &mut Vec<u8>| DdsType::encode_xcdr1_be(s, b)) as fn(&Outer, &mut Vec<u8>) -> _,
                        (|b: &[u8]| <Outer as DdsType>::decode_xcdr1_be(b)) as fn(&[u8]) -> _,
                    ),
                ] {
                    let mut bytes = Vec::new();
                    encode(&sample(n), &mut bytes).unwrap_or_else(|e| panic!("{label}, s of {n}: {e:?}"));
                    let back = decode(&bytes).unwrap_or_else(|e| panic!("{label}, s of {n}: {e:?}"));
                    assert_eq!(back, sample(n), "{label}, s of {n}");
                }
            }
        }
        "#,
    );
}

/// The four pairs, measured: the octets each puts out and the round trip
/// each closes.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn the_four_pairs_write_the_octets_the_encoding_calls_for() {
    run_generated(
        "octets",
        "@final struct Pose { int32 id; double x; }; \
         @final struct Keyed { @key uint32 id; int32 v; }; \
         enum Colour { RED, GREEN }; \
         @final union Shape switch (int32) { case 1: int32 a; default: double b; };",
        r#"
        use zerodds_dcps::DdsType;

        fn sample() -> Pose { Pose { id: 1, x: 2.0 } }

        /// Every call here names the **trait** method. The generated
        /// type also has an inherent `encode`/`decode` over
        /// `zerodds_wire`, and Rust resolves an inherent item before a
        /// trait one — which is exactly what the generated impl relies
        /// on to call its own codec, and exactly what would make this
        /// test measure the wrong function if it were written the short
        /// way.
        ///
        /// XCDR2 caps alignment at four (XTypes 1.3 §7.4.1), so the
        /// `double` sits at offset four and the payload is twelve
        /// octets. XCDR1 caps at eight and pads to offset eight, for
        /// sixteen. The vendor runs behind the four are recorded in
        /// `crates/ast-layout/tests/fixtures/vendor_bytes.md`.
        #[test]
        fn each_pair_writes_its_own_encoding() {
            let mut le = Vec::new();
            DdsType::encode(&sample(), &mut le).unwrap();
            assert_eq!(
                le,
                [0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x40],
                "XCDR2 little endian"
            );

            let mut be = Vec::new();
            DdsType::encode_be(&sample(), &mut be).unwrap();
            assert_eq!(
                be,
                [0, 0, 0, 0x01, 0x40, 0, 0, 0, 0, 0, 0, 0],
                "XCDR2 big endian"
            );

            let mut v1 = Vec::new();
            DdsType::encode_xcdr1(&sample(), &mut v1).unwrap();
            assert_eq!(
                v1,
                [0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x40],
                "XCDR1 little endian, MAXALIGN eight"
            );

            let mut v1be = Vec::new();
            DdsType::encode_xcdr1_be(&sample(), &mut v1be).unwrap();
            assert_eq!(
                v1be,
                [0, 0, 0, 0x01, 0, 0, 0, 0, 0x40, 0, 0, 0, 0, 0, 0, 0],
                "XCDR1 big endian, MAXALIGN eight"
            );
        }

        #[test]
        fn every_combination_round_trips() {
            let mut buf = Vec::new();
            DdsType::encode(&sample(), &mut buf).unwrap();
            assert_eq!(<Pose as DdsType>::decode(&buf).unwrap(), sample());

            buf.clear();
            DdsType::encode_be(&sample(), &mut buf).unwrap();
            assert_eq!(<Pose as DdsType>::decode_be(&buf).unwrap(), sample());

            buf.clear();
            DdsType::encode_xcdr1(&sample(), &mut buf).unwrap();
            assert_eq!(<Pose as DdsType>::decode_xcdr1(&buf).unwrap(), sample());

            buf.clear();
            DdsType::encode_xcdr1_be(&sample(), &mut buf).unwrap();
            assert_eq!(<Pose as DdsType>::decode_xcdr1_be(&buf).unwrap(), sample());
        }

        /// A little-endian sample read as big-endian must not come back
        /// as a sample: the byte-order axis is what the trait's
        /// defaults substituted silently, and an overridden pair is what closes
        /// it.
        #[test]
        fn the_two_orders_are_not_one_stream() {
            let mut le = Vec::new();
            DdsType::encode(&sample(), &mut le).unwrap();
            assert_ne!(
                <Pose as DdsType>::decode_be(&le).unwrap(),
                sample(),
                "reading LE octets as BE must not reproduce the sample"
            );
        }

        /// A union is a topic type, and so is carried by the bridge.
        ///
        /// It gets its codec from the same emitter a struct does, so it
        /// gets the same four pairs — including the nested-call renaming,
        /// which is what a union's branch goes through. An enumeration is
        /// not one (DDS-XTypes 1.3 §7.6.2); its codec is the inherent
        /// pair.
        #[test]
        fn a_union_carries_the_bridge() {
            let mut buf = Vec::new();
            DdsType::encode_xcdr1_be(&Shape::A(7), &mut buf).unwrap();
            assert_eq!(<Shape as DdsType>::decode_xcdr1_be(&buf).unwrap(), Shape::A(7));

            buf.clear();
            DdsType::encode_be(&Shape::B(2, 2.0), &mut buf).unwrap();
            assert_eq!(<Shape as DdsType>::decode_be(&buf).unwrap(), Shape::B(2, 2.0));
        }

        /// The KeyHash of a keyed type comes off the generated holder,
        /// which is what the runtime's instance path calls.
        #[test]
        fn the_key_holder_reaches_the_key_hash() {
            let a = Keyed { id: 0x1122_3344, v: 7 };
            let b = Keyed { id: 0x1122_3345, v: 7 };
            let hash = a.compute_key_hash().expect("keyed");
            assert_eq!(&hash[0..4], &[0x11, 0x22, 0x33, 0x44], "PLAIN_CDR2 big endian");
            assert_eq!(&hash[4..16], &[0u8; 12], "§7.6.8 step 5, zero-padded");
            assert_ne!(a.compute_key_hash(), b.compute_key_hash());
        }
        "#,
    );
}

/// Write a crate around the generated module and run its tests.
fn run_generated(name: &str, idl_source: &str, tests: &str) {
    run_generated_with(name, idl_source, "", tests);
}

/// [`run_generated`] with `config_calls` spliced into the consumer's
/// `build.rs` after `Config::new()` — `.dialect(...)`, for one — and
/// `zerodds-ast-layout` among its build dependencies to name them with.
fn run_generated_with(name: &str, idl_source: &str, config_calls: &str, tests: &str) {
    run_generated_items(name, idl_source, config_calls, "", tests);
}

/// [`run_generated_with`] with `items` at the crate root beside the
/// generated module — the Rust types a `rust_type` call names. Answers the
/// consumer crate's directory.
fn run_generated_items(
    name: &str,
    idl_source: &str,
    config_calls: &str,
    items: &str,
    tests: &str,
) -> PathBuf {
    let (dir, output) = cargo_test_generated(name, idl_source, config_calls, items, tests);
    assert!(
        output.status.success(),
        "the generated crate does not pass (exit {:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    dir
}

/// Write the consumer crate and run `cargo test` in it, whatever the
/// outcome.
fn cargo_test_generated(
    name: &str,
    idl_source: &str,
    config_calls: &str,
    items: &str,
    tests: &str,
) -> (PathBuf, std::process::Output) {
    let root = workspace_root();
    let dir = std::env::temp_dir().join(format!("zerodds_build_dds_type_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).expect("mkdir");
    fs::write(dir.join("idl.idl"), idl_source).expect("idl");

    fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "zerodds_build_dds_type_{name}"
version = "0.0.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
zerodds-codec = {{ path = "{root}/crates/codec" }}
zerodds-dcps = {{ path = "{root}/crates/dcps" }}
zerodds-float128 = {{ path = "{root}/crates/float128" }}
zerodds-types = {{ path = "{root}/crates/types" }}
zerodds-wire = {{ path = "{root}/crates/wire" }}

[build-dependencies]
zerodds-build = {{ path = "{root}/crates/zerodds-build" }}
zerodds-ast-layout = {{ path = "{root}/crates/ast-layout" }}

[workspace]
"#,
            root = root.display()
        ),
    )
    .expect("manifest");

    fs::write(
        dir.join("build.rs"),
        format!(
            "fn main() -> Result<(), zerodds_build::Error> {{\n    \
             zerodds_build::Config::new(){config_calls}.dds_type_impls(true).compile(&[\"idl.idl\"])\n}}\n"
        ),
    )
    .expect("build.rs");

    // The consumer locks against the workspace's own Cargo.lock: its
    // dependency closure is a subset of the workspace's and every version
    // is the workspace's, so `cargo test --offline` never re-resolves from
    // the index. A freshly generated lock raced the runner's registry
    // cache once and made `if-addrs` unresolvable offline (pipeline 5306),
    // where the same job had passed with the cache warm an hour earlier.
    fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock"))
        .expect("the workspace lock travels with the consumer");

    // The keyed fixture is appended to every case rather than made a
    // second crate: one `cargo test` is the expensive part here.
    fs::write(
        dir.join("src/lib.rs"),
        format!(
            "#![allow(dead_code, clippy::all)]\n\
             include!(concat!(env!(\"OUT_DIR\"), \"/idl.rs\"));\n\n{items}\n\
             #[cfg(test)]\n#[allow(unused_imports)]\nmod generated_tests {{\n\
             use super::*;\n{tests}\n}}\n"
        ),
    )
    .expect("lib.rs");

    let output = Command::new(env!("CARGO"))
        .arg("test")
        .arg("--manifest-path")
        .arg(dir.join("Cargo.toml"))
        .arg("--offline")
        // The target directory of the run that started this test (a CI
        // job's shared one), else the workspace's own, so the DCPS graph is
        // compiled once for the whole tree and not once per case.
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from),
        )
        .output()
        .expect("cargo invocation");
    (dir, output)
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|path| path.join("Cargo.lock").exists())
        .map_or(manifest.clone(), Path::to_path_buf)
}

/// An evolved `@appendable` type nested in a `@final` one, both ways.
///
/// `v2::Part` has a third member, `v1::Part` does not. The generated
/// decoder stopped after its own last member, so `v1::Evo` read `tail`
/// out of `v2::Part`'s `c` and its padding: no error, a value the
/// writer never sent. Table 19: members a newer
/// appendable writer appended are skipped by the DHEADER's extent, and a
/// member an older writer lacks takes its default.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_2_3_a_nested_appendable_type_reads_a_newer_and_an_older_writer() {
    run_generated(
        "appendablenested",
        // One simple name in two modules (IDL 4.2 §7.5.2): the generated
        // file has to compile with both, `type_objects` constants included.
        "module v2 { @appendable struct Part { long a; double b; short c; }; \
         @final struct Evo { Part piece; long tail; }; }; \
         module v1 { @appendable struct Part { long a; double b; }; \
         @final struct Evo { Part piece; long tail; }; };",
        r#"
        #[test]
        fn the_member_after_the_nested_type_is_the_one_the_writer_sent() {
            use zerodds_dcps::DdsType;
            let newer = v2::Evo { piece: v2::Part { a: 7, b: 1.5, c: -3 }, tail: 42 };
            let mut bytes = Vec::new();
            <v2::Evo as DdsType>::encode(&newer, &mut bytes).expect("encode");
            let older = <v1::Evo as DdsType>::decode(&bytes).expect("decode");
            assert_eq!(older.piece.a, 7);
            assert_eq!(older.piece.b, 1.5);
            assert_eq!(older.tail, 42);

            let mut bytes = Vec::new();
            <v1::Evo as DdsType>::encode(&older, &mut bytes).expect("encode");
            let back = <v2::Evo as DdsType>::decode(&bytes).expect("decode");
            assert_eq!(back.piece.c, 0, "Table 9: the default of a short");
            assert_eq!(back.tail, 42);
        }
        "#,
    );
}

/// XTypes 1.3 §7.6.8: `decode_key_members` reads the `@key` members of a
/// generated type and steps over the rest. The KeyHash it gives is the
/// whole sample's, in XCDR2 and XCDR1 and both byte orders, and a member it
/// stepped over is at its default: octets, a string, a double, an array and
/// a sequence of structures before, between and after the keys, a key that
/// is a structure with a key of its own, and an `@optional` member.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_6_8_decode_key_members_gives_the_key_of_the_whole_sample() {
    run_generated(
        "keymembers",
        "@final struct Inner { string s; @key long a; }; \
         @appendable struct Track { sequence<octet> blob; string name; @key long id; \
         @key Inner inner_key; double d; long arr[3]; sequence<Inner> parts; @optional long note; };",
        r#"
        #[test]
        fn the_key_members_are_the_sample_s_and_the_rest_is_default() {
            use zerodds_dcps::DdsType;
            let track = Track {
                blob: vec![1, 2, 3, 4, 5],
                name: "track".to_string(),
                id: 7,
                inner_key: Inner { s: "x".to_string(), a: 9 },
                d: 2.5,
                arr: [1, 2, 3],
                parts: vec![Inner { s: "p".to_string(), a: 1 }],
                note: Some(4),
            };
            for (xcdr1, big_endian) in [(false, false), (false, true), (true, false), (true, true)] {
                let mut bytes = Vec::new();
                match (xcdr1, big_endian) {
                    (false, false) => <Track as DdsType>::encode(&track, &mut bytes),
                    (false, true) => <Track as DdsType>::encode_be(&track, &mut bytes),
                    (true, false) => <Track as DdsType>::encode_xcdr1(&track, &mut bytes),
                    (true, true) => <Track as DdsType>::encode_xcdr1_be(&track, &mut bytes),
                }
                .expect("encode");
                let keys = <Track as DdsType>::decode_key_members(&bytes, xcdr1, big_endian)
                    .expect("a generated type has the read")
                    .expect("the read succeeds");
                assert_eq!(keys.id, 7);
                assert_eq!(keys.inner_key.a, 9);
                assert_eq!(
                    <Track as DdsType>::key_hash(&keys),
                    <Track as DdsType>::key_hash(&track),
                    "xcdr1 {xcdr1}, big endian {big_endian}"
                );
                assert!(keys.blob.is_empty());
                assert!(keys.name.is_empty());
                assert_eq!(keys.d, 0.0);
                assert_eq!(keys.arr, [0, 0, 0]);
                assert!(keys.parts.is_empty());
                assert_eq!(keys.note, Some(4), "an @optional member is read, not stepped over");
            }
        }
        "#,
    );
}

/// The four encodings a key-only read is checked in, for the tests below.
const KEY_READ_CHECK: &str = r#"
    fn modes() -> [(bool, bool); 4] {
        [(false, false), (false, true), (true, false), (true, true)]
    }
    fn write<T: zerodds_dcps::DdsType>(value: &T, xcdr1: bool, be: bool) -> Vec<u8> {
        let mut out = Vec::new();
        match (xcdr1, be) {
            (false, false) => T::encode(value, &mut out),
            (false, true) => T::encode_be(value, &mut out),
            (true, false) => T::encode_xcdr1(value, &mut out),
            (true, true) => T::encode_xcdr1_be(value, &mut out),
        }
        .expect("encode");
        out
    }
    fn keys<T: zerodds_dcps::DdsType>(bytes: &[u8], xcdr1: bool, be: bool) -> T {
        T::decode_key_members(bytes, xcdr1, be)
            .expect("a generated type has the read")
            .expect("the read succeeds")
    }
"#;

/// Names the generated code uses itself (zero-dds-snippets
/// `zerodds-build-issues`): members named like the codec's locals (`r`,
/// `value`, `zd_origin`, `w`, `pid`, `end`), a member `gen` (reserved since
/// the 2024 edition), keywords with no raw form (`crate`, `Self`, `super`),
/// modules `std` and `core`, and types named like what the codec writes
/// without a path (`Option`, `Vec`, `Reader`). The crate compiles, and the
/// key-only read steps over the members.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn generated_rust_compiles_for_the_names_the_codec_uses_itself() {
    run_generated(
        "ownnames",
        "module std { @final struct S { @key long id; }; }; \
         module core { @final struct T { @key long id; }; }; \
         module crate { @final struct Self { @key long id; long super; }; }; \
         @final struct Option { @key long id; }; @final struct Vec { @key long id; }; \
         @final struct Reader { @key long id; }; \
         @appendable struct Locals { long r; long value; long zd_origin; long w; long pid; \
         long end; long gen; @key long id; };",
        &format!(
            r#"{KEY_READ_CHECK}
        #[test]
        fn the_members_read_back_and_the_key_only_read_steps_over_them() {{
            let r = Locals {{ r: 1, value: 2, zd_origin: 3, w: 4, pid: 5, end: 6, r#gen: 7, id: 8 }};
            for (xcdr1, be) in modes() {{
                let bytes = write(&r, xcdr1, be);
                let back: Locals = keys(&bytes, xcdr1, be);
                assert_eq!(back.id, 8);
                assert_eq!(back.r, 0, "a member the key-only read steps over is at its default");
            }}
            let _ = (super::std::S::default(), super::core::T::default(), crate_::Self_::default());
            let _ = (Option_::default(), Vec_::default(), Reader_::default());
        }}
        "#
        ),
    );
}

/// The key-only read drops a non-key member of an array type held in a
/// `Vec`, a `MapEntry` or a `Box` (zero-dds-snippets `zerodds-build-issues`):
/// `let _: <the member's type> = …`, so that rustc has the array's length.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn a_dropped_array_member_compiles_inside_a_sequence_a_map_and_a_box() {
    run_generated(
        "droppedarrays",
        "typedef char C4[4]; \
         @appendable struct S { sequence<C4> seqs; map<long, C4> m; @external char c[4]; \
         long double ld[2]; @key long id; };",
        &format!(
            r#"{KEY_READ_CHECK}
        #[test]
        fn the_key_only_read_gives_the_key() {{
            let mut s = S::default();
            s.seqs = vec![['a', 'b', 'c', 'd']];
            s.c = Box::new(['w', 'x', 'y', 'z']);
            s.id = 42;
            for (xcdr1, be) in modes() {{
                let back: S = keys(&write(&s, xcdr1, be), xcdr1, be);
                assert_eq!(back.id, 42);
                assert!(back.seqs.is_empty());
            }}
        }}
        "#
        ),
    );
}

/// A numeric `@default` takes the member's type: a `float` constant
/// (`@default(CF)`), an integer on a `fixed<5,2>` (`"1.00"`), an integer and
/// a decimal on a `long double` (zero-dds-snippets `zerodds-build-issues`).
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn a_numeric_default_takes_the_members_type() {
    run_generated(
        "numericdefaults",
        "const float CF = 2.5; \
         @final struct D { @key long id; @default(CF) float f; @default(1) fixed<5,2> fx; \
         @default(1) long double one; @default(1.5) long double half; };",
        r#"
        #[test]
        fn the_defaults_are_the_declared_values() {
            let d = D::default();
            assert_eq!(d.f, 2.5_f32);
            assert_eq!(d.fx, "1.00");
            assert_eq!(d.one, zerodds_float128::Float128::from_f64(1.0));
            assert_eq!(d.half, zerodds_float128::Float128::from_f64(1.5));
        }
        "#,
    );
}

/// GitHub #61: with `strip_enum_prefix(true)` every place that names an
/// enumerator in Rust takes the cut name: the declaration, a `@default`, the
/// first literal as a default, a union's case label and the codec. A
/// content filter still reads the IDL name.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn github_61_every_place_that_names_an_enumerator_takes_the_cut_name() {
    run_generated_with(
        "enumprefix",
        "enum MyEnum { MY_ENUM_A, MY_ENUM_B, MY_ENUM_1 }; \
         union U switch (MyEnum) { case MY_ENUM_A: long a; case MY_ENUM_B: short b; }; \
         @final struct S { @key long id; @default(MY_ENUM_B) MyEnum chosen; MyEnum first; U un; };",
        ".strip_enum_prefix(true)",
        &format!(
            r#"{KEY_READ_CHECK}
        #[test]
        fn the_variants_are_cut_and_the_values_hold() {{
            use zerodds_dcps::DdsType;
            let s = S::default();
            assert_eq!(s.chosen, MyEnum::B);
            assert_eq!(s.first, MyEnum::A);
            let _kept = MyEnum::MY_ENUM_1;
            let mut written = S::default();
            written.id = 3;
            written.chosen = MyEnum::MY_ENUM_1;
            for (xcdr1, be) in modes() {{
                let bytes = write(&written, xcdr1, be);
                let back: S = keys(&bytes, xcdr1, be);
                assert_eq!(back.id, 3);
            }}
            let filtered = format!("{{:?}}", DdsType::field_value(&written, "chosen"));
            assert!(filtered.contains("MY_ENUM_1"), "the filter reads the IDL name: {{filtered}}");
        }}
        "#
        ),
    );
}

/// GitHub #61: the Rust naming scheme changes the Rust names and nothing
/// else. One IDL file is compiled twice into one crate, with the IDL names
/// and with `rust_naming(RustNaming::Rust)`; the second module is held to
/// `deny(nonstandard_style)`. For the same values the two give the same
/// octets in every encoding, the same type name, TypeIdentifier and
/// TypeObject, and each reads what the other wrote.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn github_61_both_naming_schemes_give_the_same_type_and_the_same_octets() {
    run_generated_items(
        "rustnaming",
        "module sensor_data { \
           const long maxSamples = 8; \
           enum reading_kind { READING_KIND_raw, READING_KIND_filtered }; \
           bitmask status_flags { flagReady, flag_error }; \
           bitset packed_bits { bitfield<3> lowPart; bitfield<5> high_part; }; \
           typedef sequence<long> sample_list; \
           union reading_value switch (reading_kind) { \
             case READING_KIND_raw: long rawValue; \
             case READING_KIND_filtered: double filtered_value; }; \
           @appendable struct base_reading { @key long sensorId; @optional string unitName; }; \
           @appendable struct HTTPReading : base_reading { \
             @default(READING_KIND_filtered) reading_kind kind; sample_list Samples; \
             reading_value lastValue; status_flags flags; packed_bits bits; \
             long type; long r; double grid[2]; }; \
           @mutable struct sparse_reading { @key long sensorId; @optional double lastSeen; \
             sequence<HTTPReading> allReadings; }; \
         }; \
         module Outer_Scope { \
           @final struct top_level { @key sensor_data::reading_kind kindKey; @key long instanceNo; \
             sensor_data::HTTPReading inner_reading; }; \
         };",
        ".dds_type_impls(true).compile(&[\"idl.idl\"])?;\n    \
         let renamed = std::path::PathBuf::from(std::env::var(\"OUT_DIR\").expect(\"OUT_DIR\"))\
         .join(\"renamed\");\n    \
         std::fs::create_dir_all(&renamed).expect(\"mkdir\");\n    \
         zerodds_build::Config::new().out_dir(&renamed)\
         .rust_naming(zerodds_build::RustNaming::Rust)",
        "#[deny(nonstandard_style)]\npub mod by_rust {\n    \
         include!(concat!(env!(\"OUT_DIR\"), \"/renamed/idl.rs\"));\n}\n",
        &format!(
            r#"{KEY_READ_CHECK}
        fn by_idl_reading() -> sensor_data::HTTPReading {{
            let mut a = sensor_data::HTTPReading::default();
            a.parent.sensorId = 7;
            a.parent.unitName = Some("K".to_string());
            a.kind = sensor_data::reading_kind::READING_KIND_raw;
            a.Samples = vec![1, 2, 3];
            a.lastValue = sensor_data::reading_value::Filtered_value(2.5);
            a.flags = sensor_data::flagReady | sensor_data::flag_error;
            a.bits.lowPart = 5;
            a.bits.high_part = 17;
            a.r#type = 9;
            a.r = 4;
            a.grid = [1.0, 2.0];
            a
        }}
        fn by_rust_reading() -> by_rust::sensor_data::HttpReading {{
            let mut b = by_rust::sensor_data::HttpReading::default();
            b.parent.sensor_id = 7;
            b.parent.unit_name = Some("K".to_string());
            b.kind = by_rust::sensor_data::ReadingKind::ReadingKindRaw;
            b.samples = vec![1, 2, 3];
            b.last_value = by_rust::sensor_data::ReadingValue::FilteredValue(2.5);
            b.flags = by_rust::sensor_data::FLAG_READY | by_rust::sensor_data::FLAG_ERROR;
            b.bits.low_part = 5;
            b.bits.high_part = 17;
            b.r#type = 9;
            b.r = 4;
            b.grid = [1.0, 2.0];
            b
        }}
        fn same_type<A: zerodds_dcps::DdsType, B: zerodds_dcps::DdsType>() {{
            assert_eq!(A::TYPE_NAME, B::TYPE_NAME);
            assert_eq!(A::type_identifier(), B::type_identifier(), "{{}}", A::TYPE_NAME);
            assert_eq!(A::type_object(), B::type_object(), "{{}}", A::TYPE_NAME);
            assert_eq!(
                A::type_object_dependencies(),
                B::type_object_dependencies(),
                "{{}}",
                A::TYPE_NAME
            );
            assert_eq!(A::HAS_KEY, B::HAS_KEY);
            assert_eq!(A::MAX_SERIALIZED_SIZE, B::MAX_SERIALIZED_SIZE);
        }}

        #[test]
        fn the_type_is_the_same_under_both_schemes() {{
            assert_eq!(sensor_data::maxSamples, by_rust::sensor_data::MAX_SAMPLES);
            assert_eq!(sensor_data::flag_error, by_rust::sensor_data::FLAG_ERROR);
            same_type::<sensor_data::base_reading, by_rust::sensor_data::BaseReading>();
            same_type::<sensor_data::HTTPReading, by_rust::sensor_data::HttpReading>();
            same_type::<sensor_data::sparse_reading, by_rust::sensor_data::SparseReading>();
            same_type::<sensor_data::reading_value, by_rust::sensor_data::ReadingValue>();
            same_type::<Outer_Scope::top_level, by_rust::outer_scope::TopLevel>();
            assert_eq!(
                <sensor_data::HTTPReading as zerodds_dcps::DdsType>::TYPE_NAME,
                "sensor_data::HTTPReading"
            );
        }}

        #[test]
        fn the_octets_are_the_same_under_both_schemes() {{
            use zerodds_dcps::DdsType;
            let a = by_idl_reading();
            let b = by_rust_reading();
            assert_eq!(b.kind, by_rust::sensor_data::ReadingKind::ReadingKindRaw);
            assert_eq!(
                by_rust::sensor_data::HttpReading::default().kind,
                by_rust::sensor_data::ReadingKind::ReadingKindFiltered,
                "the @default names the enumerator by its Rust name"
            );
            let mut sparse_a = sensor_data::sparse_reading::default();
            sparse_a.sensorId = 11;
            sparse_a.lastSeen = Some(0.5);
            sparse_a.allReadings = vec![by_idl_reading(), by_idl_reading()];
            let mut sparse_b = by_rust::sensor_data::SparseReading::default();
            sparse_b.sensor_id = 11;
            sparse_b.last_seen = Some(0.5);
            sparse_b.all_readings = vec![by_rust_reading(), by_rust_reading()];
            let mut top_a = Outer_Scope::top_level::default();
            top_a.kindKey = sensor_data::reading_kind::READING_KIND_filtered;
            top_a.instanceNo = 3;
            top_a.inner_reading = by_idl_reading();
            let mut top_b = by_rust::outer_scope::TopLevel::default();
            top_b.kind_key = by_rust::sensor_data::ReadingKind::ReadingKindFiltered;
            top_b.instance_no = 3;
            top_b.inner_reading = by_rust_reading();
            for (xcdr1, be) in modes() {{
                let reading = write(&a, xcdr1, be);
                assert_eq!(reading, write(&b, xcdr1, be), "HTTPReading {{xcdr1}} {{be}}");
                let sparse = write(&sparse_a, xcdr1, be);
                assert_eq!(sparse, write(&sparse_b, xcdr1, be), "sparse_reading {{xcdr1}} {{be}}");
                let top = write(&top_a, xcdr1, be);
                assert_eq!(top, write(&top_b, xcdr1, be), "top_level {{xcdr1}} {{be}}");
                // The key-only read of one scheme takes the other's octets.
                let key: by_rust::outer_scope::TopLevel = keys(&top, xcdr1, be);
                assert_eq!(key.instance_no, 3);
                assert_eq!(key.kind_key, by_rust::sensor_data::ReadingKind::ReadingKindFiltered);
                let key: by_rust::sensor_data::HttpReading = keys(&reading, xcdr1, be);
                assert_eq!(key.parent.sensor_id, 7);
            }}
            // Each reads what the other wrote.
            let mut bytes = Vec::new();
            DdsType::encode(&a, &mut bytes).expect("encode");
            let back =
                <by_rust::sensor_data::HttpReading as DdsType>::decode(&bytes).expect("decode");
            assert_eq!(back, b);
            let mut bytes = Vec::new();
            DdsType::encode(&sparse_b, &mut bytes).expect("encode");
            let back = <sensor_data::sparse_reading as DdsType>::decode(&bytes).expect("decode");
            assert_eq!(back, sparse_a);
            // A content filter names the IDL member under either scheme.
            assert_eq!(
                format!("{{:?}}", DdsType::field_value(&top_a, "instanceNo")),
                format!("{{:?}}", DdsType::field_value(&top_b, "instanceNo"))
            );
            assert!(DdsType::field_value(&top_b, "instance_no").is_none());
            assert_eq!(
                format!("{{:?}}", DdsType::field_value(&top_b, "kindKey")),
                format!("{{:?}}", DdsType::field_value(&top_a, "kindKey"))
            );
        }}
        "#
        ),
    );
}

/// A `fixed<P, S>` member, through the whole consumer path.
///
/// The last member kind the Rust column had no writer for: `zerodds-build`
/// stopped with `NoWriter { member: "price", kind: "fixed" }` while
/// `crates/lang-rules/src/emit/encode.rs` had handled it for the other
/// twenty columns all along. `idl-conformance/_interop/rust-features`
/// (`feat::Money`) is the target that could not be built.
///
/// The oracle is CORBA 3.3 part 2 §9.3.2.8 and there is no other: no DDS
/// vendor generates code for `fixed`. All sixteen of its rows in
/// `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv` read
/// `ungenerated` (Cyclone, Fast DDS, RTI) or `uncompiled` (OpenDDS). What
/// holds the shape here is the clause, the twenty other columns that write
/// it the same way through `zerodds-wire-codec`, and the round trip below.
///
/// `fixed<9, 2>` in a keyed structure: the decimal belongs to the sample,
/// while only `account` belongs to the key. This also checks that adding
/// fixed support does not accidentally include a non-key field in the hash.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn a_fixed_member_reaches_the_generated_type_and_round_trips() {
    run_generated(
        "fixedmember",
        "@appendable struct Money { @key int32 account; fixed<9,2> price; int32 tail; };",
        r#"
        #[test]
        fn the_decimal_survives_the_round_trip_without_changing_the_account_key() {
            use zerodds_dcps::DdsType;
            let m = Money { account: 7, price: "1234567.89".to_string(), tail: 3 };
            let mut bytes = Vec::new();
            <Money as DdsType>::encode(&m, &mut bytes).expect("encode");
            let back = <Money as DdsType>::decode(&bytes).expect("decode");
            // Exact, not approximate: that is what `fixed` is for.
            assert_eq!(back.price, "1234567.89");
            assert_eq!(back.account, 7);
            assert_eq!(back.tail, 3);

            // The declaration's own spelling comes back, whatever the
            // sample was written as. `fixed<9,2>` has two fraction digits
            // and one zero before the point.
            let short = Money { account: 1, price: "1".to_string(), tail: 0 };
            let mut b = Vec::new();
            <Money as DdsType>::encode(&short, &mut b).expect("encode");
            assert_eq!(<Money as DdsType>::decode(&b).expect("decode").price, "1.00");

            // A value the declaration cannot carry is refused, not
            // truncated: a shortened decimal is the wrong number with
            // nothing to notice it by.
            let over = Money { account: 1, price: "12345678.90".to_string(), tail: 0 };
            let mut b = Vec::new();
            assert!(
                <Money as DdsType>::encode(&over, &mut b).is_err(),
                "eight integer digits do not fit fixed<9,2>"
            );

            // Only account reaches the key holder. Changing the unkeyed
            // decimal must not change the instance's hash.
            assert!(<Money as DdsType>::has_key());
            let holder = m.key_holder_be().expect("holder");
            assert!(!holder.is_empty());
            let other_price = Money { account: 7, price: "0.01".to_string(), tail: 3 };
            assert_eq!(
                m.key_hash().expect("hash"),
                other_price.key_hash().expect("hash"),
                "the key is `account`, so the price does not enter the hash"
            );
        }
        "#,
    );
}

#[test]
fn external_storage_is_finite_and_round_trips_without_changing_wire_values() {
    run_generated(
        "externalstorage",
        "module m { struct Node; @mutable struct Node { long value; @optional @external Node next; }; };\n\
         @final struct Inner { long value; };\n\
         @final struct Stored { @external long number; @external string<5> text; @external sequence<long,3> values; @external Inner nested; };\n\
         @final struct Plain { long number; string<5> text; sequence<long,3> values; Inner nested; };\n\
         @appendable struct Earlier { long id; };\n\
         @appendable struct Later { long id; @external @default(7) long counter; @external string tail; };\n\
         @final struct Loose { string text; };\n\
         @final struct Bounded { @external @try_construct(USE_DEFAULT) string<5> text; };\n\
         @final union Choice switch(long) { case 0: @external long number; default: @external string text; };",
        r#"
        #[test]
        fn recursive_optional_external_has_finite_owning_storage() {
            use zerodds_dcps::DdsType;
            let node = m::Node { value: 7, next: Some(Box::new(m::Node { value: 9, next: None })) };
            for (encode, decode) in [
                (<m::Node as DdsType>::encode as fn(&m::Node, &mut Vec<u8>) -> _, <m::Node as DdsType>::decode as fn(&[u8]) -> _),
                (<m::Node as DdsType>::encode_be, <m::Node as DdsType>::decode_be),
                (<m::Node as DdsType>::encode_xcdr1, <m::Node as DdsType>::decode_xcdr1),
                (<m::Node as DdsType>::encode_xcdr1_be, <m::Node as DdsType>::decode_xcdr1_be),
            ] {
                let mut bytes = Vec::new();
                encode(&node, &mut bytes).expect("encode recursive value");
                let back = decode(&bytes).expect("decode recursive value");
                assert_eq!(back.value, 7);
                let next = back.next.expect("present child");
                assert_eq!(next.value, 9);
                assert!(next.next.is_none());
            }
        }

        #[test]
        fn external_primitive_string_collection_and_aggregate_keep_the_plain_bytes() {
            use zerodds_dcps::DdsType;
            let stored = Stored { number: Box::new(42), text: Box::new("abc".to_owned()), values: Box::new(vec![1, 2]), nested: Box::new(Inner { value: 17 }) };
            let plain = Plain { number: 42, text: "abc".to_owned(), values: vec![1, 2], nested: Inner { value: 17 } };
            let mut bytes = Vec::new();
            let mut reference = Vec::new();
            <Stored as DdsType>::encode(&stored, &mut bytes).expect("external encode");
            <Plain as DdsType>::encode(&plain, &mut reference).expect("plain encode");
            assert_eq!(bytes, reference);
            let back = <Stored as DdsType>::decode(&bytes).expect("external decode");
            assert_eq!(*back.number, 42);
            assert_eq!(*back.text, "abc");
            assert_eq!(*back.values, vec![1, 2]);
            assert_eq!(back.nested.value, 17);
            assert!(matches!(stored.field_value("number"), Some(zerodds_dcps::FilterValue::Int(42))));
            assert!(matches!(stored.field_value("text"), Some(zerodds_dcps::FilterValue::String(s)) if s == "abc"));
            assert!(matches!(stored.field_value("nested.value"), Some(zerodds_dcps::FilterValue::Int(17))));
        }

        #[test]
        fn external_defaults_narrowing_and_union_payloads_are_boxed_consistently() {
            use zerodds_dcps::DdsType;
            let mut bytes = Vec::new();
            <Earlier as DdsType>::encode(&Earlier { id: 1 }, &mut bytes).expect("old writer");
            let later = <Later as DdsType>::decode(&bytes).expect("new reader defaults");
            assert_eq!(*later.counter, 7);
            assert_eq!(*later.tail, "");
            bytes.clear();
            <Loose as DdsType>::encode(&Loose { text: "too long".into() }, &mut bytes).expect("loose writer");
            assert_eq!(*<Bounded as DdsType>::decode(&bytes).expect("value default before box").text, "");
            for choice in [Choice::Number(Box::new(19)), Choice::Text(1, Box::new("selected".into()))] {
                bytes.clear();
                <Choice as DdsType>::encode(&choice, &mut bytes).expect("external union write");
                assert_eq!(<Choice as DdsType>::decode(&bytes).expect("external union read"), choice);
            }
        }
        "#,
    );
}

/// An inherited type registers its base beside itself: the generated
/// `type_object_dependencies` carries one object per hashed type the
/// type's own object reaches, and a type that reaches none emits no
/// override. Measured before this existed on the OMG `dds-xtypes`
/// suite: 55 of 625 cells communicated across extensibility kinds
/// because `Shape5*`'s base was never registered and the endpoint's
/// own closure resolved to nothing.
#[test]
fn an_inherited_type_carries_its_closure_beside_its_object() {
    let module = generated(
        "closure",
        "module cl {\n\
         enum Color { RED, GREEN };\n\
         struct Inner { Color c; string<8> s; };\n\
         struct Base { long a; };\n\
         struct Derived : Base { float angle; Inner nested; };\n\
         @final struct Plain { long x; };\n\
         };\n",
    );
    let derived_impl = module
        .split("impl zerodds_dcps::DdsType for")
        .find(|block| block.trim_start().starts_with("cl::Derived "))
        .expect("Derived has a DdsType impl");
    let dependencies = derived_impl
        .split("fn type_object_dependencies()")
        .nth(1)
        .expect("Derived carries type_object_dependencies");
    // One blob per hashed type in the closure: Base, Inner, Color.
    let blobs = dependencies
        .split("fn ")
        .next()
        .unwrap_or("")
        .matches("&[0x")
        .count();
    assert_eq!(
        blobs, 3,
        "Derived's closure is Base, Inner and Color:\n{dependencies}"
    );

    let plain_impl = module
        .split("impl zerodds_dcps::DdsType for")
        .find(|block| block.trim_start().starts_with("cl::Plain "))
        .expect("Plain has a DdsType impl");
    assert!(
        !plain_impl.contains("fn type_object_dependencies()"),
        "a type that reaches no hashed type keeps the trait default"
    );
}

/// The octets Fast DDS writes for a `@mutable` sample in XCDR1, read by a
/// module built in the Fast DDS dialect, and written back identically.
///
/// The oracle is Fast DDS itself: `RichMPubSubType::serialize` from
/// fastddsgen's output for this IDL, with Fast CDR 2.3.5 and
/// `XCDR_DATA_REPRESENTATION`, gave
/// `00 03 00 00 | 00 00 04 00 01 00 00 00 | 01 00 08 00 <3.141592653589793>
/// | 02 00 08 00 04 00 00 00 00 01 02 03 | 02 3f 00 00` — PL_CDR_LE, bare
/// parameter ids, true lengths and a bare sentinel. A module built with
/// `Config::dialect(FastDds)` used to decode it with the specification's
/// framing and refused the sentinel. The specification-dialect module reads
/// it as well now: the sentinel's must-understand bit is all that differs,
/// and DDS-XTypes 1.3 §7.4.1.2.1 has every reader take either setting.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn a_fastdds_dialect_module_reads_and_writes_what_fast_dds_writes() {
    run_generated_with(
        "fastdds_mutable",
        "@mutable struct RichM { @key unsigned long id; double value; sequence<octet> blob; };",
        ".dialect(zerodds_ast_layout::Dialect::FastDds)",
        r#"
        use zerodds_dcps::DdsType;

        const FAST_DDS: [u8; 36] = [
            0x00, 0x00, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x08, 0x00, 0x18, 0x2d, 0x44, 0x54, 0xfb, 0x21, 0x09, 0x40,
            0x02, 0x00, 0x08, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03,
            0x02, 0x3f, 0x00, 0x00,
        ];

        #[test]
        fn reads_and_writes_fast_dds_octets() {
            let sample = <RichM as DdsType>::decode_xcdr1(&FAST_DDS).expect("decode");
            assert_eq!(sample.id, 1);
            assert_eq!(sample.value, core::f64::consts::PI);
            assert_eq!(sample.blob, vec![0, 1, 2, 3]);
            let mut out = Vec::new();
            <RichM as DdsType>::encode_xcdr1(&sample, &mut out).expect("encode");
            assert_eq!(out, FAST_DDS.to_vec());
        }
        "#,
    );
}

/// DDS-XTypes 1.3 §7.6.2: "The only types that may be associated with a
/// DDS Topic are the Aggregated Types." An enumeration and a bitset are
/// not, so no `DdsType` impl is generated for them; the structure and the
/// union beside them keep theirs.
#[test]
fn xtypes_7_6_2_an_enumeration_and_a_bitset_are_no_topic_types() {
    let text = generated(
        "Members",
        "enum Colour { RED, GREEN }; \
         bitset Flags { bitfield<3> low; bitfield<5> high; }; \
         @final struct Holder { Colour c; Flags f; }; \
         @final union Choice switch (long) { case 1: long a; default: double b; };",
    );
    assert!(
        !text.contains("impl zerodds_dcps::DdsType for Colour"),
        "an enumeration is no topic type: {text}"
    );
    assert!(
        !text.contains("impl zerodds_dcps::DdsType for Flags"),
        "a bitset is no topic type: {text}"
    );
    assert!(
        text.contains("impl zerodds_dcps::DdsType for Holder"),
        "{text}"
    );
    assert!(
        text.contains("impl zerodds_dcps::DdsType for Choice"),
        "{text}"
    );
}

// -- GitHub #33: a Rust type for an IDL typedef, member or union branch ---

/// One unit, twice: `app` with `rust_type` mappings, `plain` without. The
/// plain module's octets are the oracle the mapped one is held to — the
/// IDL encoding, which the vendor runs hold the plain codec to.
const ISSUE_33_IDL: &str = "\
module app {
    typedef string<12> Dtg;
    typedef uint32 ItemId;
    @final struct Track {
        @key uint32 id;
        Dtg stamp;
        sequence<Dtg, 4> history;
        Dtg window[2];
        @optional Dtg until;
        string source;
        ItemId item;
        map<ItemId, Dtg> due;
        @external Dtg note;
        long count;
    };
    @final union Mark switch (long) { case 1: Dtg at; case 2: long n; };
    @appendable struct Older { long a; };
    @appendable struct Newer { long a; ItemId b; };
};
module plain {
    typedef string<12> Dtg;
    typedef uint32 ItemId;
    @final struct Track {
        @key uint32 id;
        Dtg stamp;
        sequence<Dtg, 4> history;
        Dtg window[2];
        @optional Dtg until;
        string source;
        ItemId item;
        map<ItemId, Dtg> due;
        @external Dtg note;
        long count;
    };
    @final union Mark switch (long) { case 1: Dtg at; case 2: long n; };
    @appendable struct Older { long a; };
    @appendable struct Newer { long a; ItemId b; };
};
";

/// The mappings of [`ISSUE_33_IDL`]: a typedef onto a checked newtype, a
/// key member and a typedef onto `NonZeroU32`, a member onto a foreign
/// type through `DisplayFromStr`, a union branch onto `NonZeroI32`.
const ISSUE_33_CONFIG: &str = "\
        .rust_type(\"app::Dtg\", \"crate::Dtg\")
        .rust_type(\"app::Track.id\", \"core::num::NonZeroU32\")
        .rust_type(\"app::ItemId\", \"core::num::NonZeroU32\")
        .rust_type_with(\"app::Track.source\", \"std::net::Ipv4Addr\", \"zerodds_wire::DisplayFromStr\")
        .rust_type(\"app::Mark.n\", \"core::num::NonZeroI32\")";

/// The checked newtype the reporter of #33 asked for: a date-time group,
/// `DDHHMMZMONYY`, that cannot hold an invalid one. Written with
/// `IdlOut::display`, read from the `&str` the sample lends.
const ISSUE_33_DTG: &str = r#"
/// A date-time group, `DDHHMMZMONYY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Dtg {
    day: u8,
    hour: u8,
    minute: u8,
    month: u8,
    year: u8,
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

impl Dtg {
    pub fn new(day: u8, hour: u8, minute: u8, month: u8, year: u8) -> Self {
        Self { day, hour, minute, month, year }
    }

    fn parse(text: &str) -> Option<Self> {
        let b = text.as_bytes();
        if b.len() != 12 || b[6] != b'Z' {
            return None;
        }
        let two = |i: usize| -> Option<u8> {
            let (t, u) = (b[i], b[i + 1]);
            (t.is_ascii_digit() && u.is_ascii_digit()).then(|| (t - b'0') * 10 + (u - b'0'))
        };
        let month = MONTHS.iter().position(|m| m.as_bytes() == &b[7..10])? as u8 + 1;
        let dtg = Self { day: two(0)?, hour: two(2)?, minute: two(4)?, month, year: two(10)? };
        ((1..=31).contains(&dtg.day) && dtg.hour < 24 && dtg.minute < 60).then_some(dtg)
    }
}

impl core::fmt::Display for Dtg {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02}{:02}{:02}Z{}{:02}",
            self.day,
            self.hour,
            self.minute,
            MONTHS[usize::from(self.month - 1)],
            self.year
        )
    }
}

impl zerodds_wire::WireRepr for Dtg {
    type Idl = str;
    fn to_wire(
        &self,
        out: zerodds_wire::IdlOut<'_, str>,
    ) -> Result<zerodds_wire::Written, zerodds_wire::WireError> {
        out.display(self)
    }
    fn from_wire(wire: &str) -> Result<Self, zerodds_wire::WireError> {
        Self::parse(wire)
            .ok_or_else(|| zerodds_wire::WireError::invalid_value("a date-time group DDHHMMZMONYY"))
    }
}
"#;

/// GitHub #33: a typedef, a key member, a member through an adapter and a
/// union branch held as Rust types — in a member, a sequence, an array, a
/// map, `@optional` and `@external` — write the octets of their IDL types in all four
/// encodings, and read back into the Rust types. A value the Rust type
/// refuses rejects the sample with its reason; the key hash is the IDL
/// value's (XTypes 1.3 §7.6.8); a member an older `@appendable` writer
/// left out takes its IDL default, and a Rust type that refuses that
/// default rejects the sample (Table 19).
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_4_issue_33_a_rust_type_writes_and_reads_the_octets_of_its_idl_type() {
    run_generated_items(
        "issue33",
        ISSUE_33_IDL,
        ISSUE_33_CONFIG,
        ISSUE_33_DTG,
        r#"
        use core::num::{NonZeroI32, NonZeroU32};
        use zerodds_dcps::DdsType;

        type Pair<A, P> = (
            fn(&A, &mut Vec<u8>) -> Result<(), zerodds_dcps::EncodeError>,
            fn(&P, &mut Vec<u8>) -> Result<(), zerodds_dcps::EncodeError>,
            fn(&[u8]) -> Result<A, zerodds_dcps::DecodeError>,
        );

        fn dtg(day: u8) -> Dtg { Dtg::new(day, 14, 30, 11, 26) }
        fn text(day: u8) -> String { format!("{day:02}1430ZNOV26") }
        fn nz(v: u32) -> NonZeroU32 { NonZeroU32::new(v).expect("non-zero") }

        fn mapped() -> app::Track {
            app::Track {
                id: nz(7),
                stamp: dtg(5),
                history: vec![dtg(1), dtg(2)],
                window: [dtg(3), dtg(4)],
                until: Some(dtg(6)),
                source: std::net::Ipv4Addr::new(10, 0, 0, 1),
                item: nz(42),
                due: vec![zerodds_wire::MapEntry { key: nz(9), value: dtg(8) }],
                note: Box::new(dtg(10)),
                count: -3,
            }
        }

        fn plain() -> plain::Track {
            plain::Track {
                id: 7,
                stamp: text(5),
                history: vec![text(1), text(2)],
                window: [text(3), text(4)],
                until: Some(text(6)),
                source: "10.0.0.1".to_string(),
                item: 42,
                due: vec![zerodds_wire::MapEntry { key: 9, value: text(8) }],
                note: Box::new(text(10)),
                count: -3,
            }
        }

        fn four<A: DdsType, P: DdsType>() -> [Pair<A, P>; 4] {
            [
                (<A as DdsType>::encode, <P as DdsType>::encode, <A as DdsType>::decode),
                (<A as DdsType>::encode_be, <P as DdsType>::encode_be, <A as DdsType>::decode_be),
                (<A as DdsType>::encode_xcdr1, <P as DdsType>::encode_xcdr1, <A as DdsType>::decode_xcdr1),
                (<A as DdsType>::encode_xcdr1_be, <P as DdsType>::encode_xcdr1_be, <A as DdsType>::decode_xcdr1_be),
            ]
        }

        #[test]
        fn the_octets_are_the_idl_types_and_read_back_into_the_rust_types() {
            for (encode, encode_plain, decode) in four::<app::Track, plain::Track>() {
                let (mut a, mut p) = (Vec::new(), Vec::new());
                encode(&mapped(), &mut a).expect("mapped");
                encode_plain(&plain(), &mut p).expect("plain");
                assert_eq!(a, p, "the IDL encoding is the oracle");
                assert_eq!(decode(&a).expect("decode"), mapped());
            }
            let marks = [
                (app::Mark::At(dtg(9)), plain::Mark::At(text(9))),
                (app::Mark::N(NonZeroI32::new(-5).expect("non-zero")), plain::Mark::N(-5)),
            ];
            for (mark, plain_mark) in marks {
                for (encode, encode_plain, decode) in four::<app::Mark, plain::Mark>() {
                    let (mut a, mut p) = (Vec::new(), Vec::new());
                    encode(&mark, &mut a).expect("mapped");
                    encode_plain(&plain_mark, &mut p).expect("plain");
                    assert_eq!(a, p);
                    assert_eq!(decode(&a).expect("decode"), mark);
                }
            }
        }

        #[test]
        fn a_value_the_rust_type_refuses_rejects_the_sample_with_its_reason() {
            for (field, reason) in [("stamp", "date-time group"), ("id", "other than zero"), ("source", "FromStr")] {
                let mut bad = plain();
                match field {
                    "stamp" => bad.stamp = "991430ZNOV26".to_string(),
                    "id" => bad.id = 0,
                    _ => bad.source = "10.0.0".to_string(),
                }
                let mut bytes = Vec::new();
                <plain::Track as DdsType>::encode(&bad, &mut bytes).expect("plain");
                let mut r = zerodds_wire::Reader::with_max_align(&bytes, zerodds_wire::Endianness::Little, 4);
                let refused = app::Track::decode(&mut r).expect_err("refused");
                assert!(refused.to_string().contains(reason), "{field}: {refused}");
                assert!(<app::Track as DdsType>::decode(&bytes).is_err(), "{field}");
            }
        }

        #[test]
        fn the_key_hash_is_the_idl_values() {
            let mapped_hash = mapped().key_hash().expect("hash");
            assert_eq!(mapped_hash, plain().key_hash().expect("hash"));
            // §7.6.8: PLAIN_CDR2 big endian, zero-padded to sixteen.
            assert_eq!(mapped_hash, [0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            // A key-only sample spells the other members out with their
            // defaults; `item` is a NonZeroU32, which Table 9's zero is not.
            let holder = mapped().key_holder_be().expect("holder");
            let mut r = zerodds_wire::Reader::with_max_align(&holder, zerodds_wire::Endianness::Big, 4);
            let refused = app::Track::key_from_holder_be(&mut r).expect_err("no default for item");
            assert!(refused.to_string().contains("did not carry"), "{refused}");
        }

        #[test]
        fn a_member_an_older_writer_left_out_takes_its_idl_default_or_rejects_the_sample() {
            let mut bytes = Vec::new();
            <app::Older as DdsType>::encode(&app::Older { a: 1 }, &mut bytes).expect("older");
            assert_eq!(<plain::Newer as DdsType>::decode(&bytes).expect("plain").b, 0);
            let mut r = zerodds_wire::Reader::with_max_align(&bytes, zerodds_wire::Endianness::Little, 4);
            let refused = app::Newer::decode(&mut r).expect_err("NonZeroU32 has no zero");
            assert!(refused.to_string().contains("did not carry"), "{refused}");
        }

        #[test]
        fn a_filter_compares_the_idl_value() {
            let track = mapped();
            assert!(matches!(track.field_value("stamp"), Some(zerodds_dcps::FilterValue::String(s)) if s == text(5)));
            assert!(matches!(track.field_value("id"), Some(zerodds_dcps::FilterValue::Int(7))));
            assert!(matches!(track.field_value("source"), Some(zerodds_dcps::FilterValue::String(s)) if s == "10.0.0.1"));
            assert!(matches!(track.field_value("until"), Some(zerodds_dcps::FilterValue::String(s)) if s == text(6)));
            assert!(matches!(track.field_value("note"), Some(zerodds_dcps::FilterValue::String(s)) if s == text(10)));
        }
        "#,
    );
}

/// GitHub #33: a Rust type whose `WireRepr::Idl` is not its member's IDL
/// type does not compile, and the error names the mapping — the build
/// error the design asks for, stated in the generated file
/// (`zerodds_wire::repr::idl_type_is`).
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn issue_33_a_rust_type_of_another_idl_type_fails_the_build() {
    let (_, output) = cargo_test_generated(
        "issue33mismatch",
        "module app { typedef uint32 ItemId; @final struct S { ItemId id; }; };",
        ".rust_type(\"app::ItemId\", \"crate::Dtg\")",
        ISSUE_33_DTG,
        "",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a `str` type on a uint32 compiled"
    );
    assert!(
        stderr.contains("idl_type_is::<crate::Dtg, u32>"),
        "the error names the mapping:\n{stderr}"
    );
}

/// GitHub #33: a C peer generated from the same IDL reads what the Rust
/// type wrote. The Rust side holds `NonZeroU32` and the checked
/// date-time group; `runtime/c` and the C column's own codec decode the
/// octets into `uint32_t` and `char *` and print them.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_4_issue_33_a_c_peer_reads_what_the_rust_type_wrote() {
    const IDL: &str = "module app { typedef string<12> Dtg; \
        @final struct Report { @key uint32 id; Dtg stamp; sequence<Dtg, 4> history; long count; }; };";
    let dir = run_generated_items(
        "issue33c",
        IDL,
        ".rust_type(\"app::Dtg\", \"crate::Dtg\").rust_type(\"app::Report.id\", \"core::num::NonZeroU32\")",
        ISSUE_33_DTG,
        r#"
        #[test]
        fn write_the_sample() {
            use zerodds_dcps::DdsType;
            let report = app::Report {
                id: core::num::NonZeroU32::new(77).expect("non-zero"),
                stamp: Dtg::new(5, 14, 30, 11, 26),
                history: vec![Dtg::new(1, 0, 5, 1, 27), Dtg::new(31, 23, 59, 12, 25)],
                count: -9,
            };
            let mut bytes = Vec::new();
            <app::Report as DdsType>::encode(&report, &mut bytes).expect("encode");
            std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/sample.bin"), bytes).expect("write");
        }
        "#,
    );
    let sample = fs::read(dir.join("sample.bin")).expect("the sample the Rust type wrote");
    let out = run_c_peer(
        &dir,
        IDL,
        &sample,
        "    app_Report back;\n\
         \x20   zdw_reader r;\n\
         \x20   uint8_t heap[512];\n\
         \x20   uint32_t i;\n\
         \x20   memset(&back, 0, sizeof back);\n\
         \x20   zdw_reader_init(&r, sample, sizeof sample, ZDW_LITTLE, 4);\n\
         \x20   zdw_reader_set_heap(&r, heap, sizeof heap);\n\
         \x20   zd_dec_app_Report(&r, &back, ZDW_TC_DISCARD);\n\
         \x20   printf(\"status %d id %u stamp %s count %d\\n\", (int)zdw_reader_status(&r),\n\
         \x20          (unsigned)back.id, back.stamp, (int)back.count);\n\
         \x20   for (i = 0; i < back.history._length; i++) { printf(\"history %s\\n\", back.history._buffer[i]); }\n",
    );
    assert_eq!(
        out,
        "status 0 id 77 stamp 051430ZNOV26 count -9\nhistory 010005ZJAN27\nhistory 312359ZDEC25\n",
        "the C peer's reading"
    );
}

/// XTypes 1.3 §7.6.8 with DDS 1.4 §2.2.2.5.1.13: an instance whose key
/// member is a struct comes back from its key holder — the key members
/// from the holder, every other member at its default. `key_members_with`
/// names the members of the nested struct too; the generated reader looked
/// each of them up as a member of the outer type, found none and refused
/// every holder.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_6_8_a_struct_typed_key_member_is_read_back_from_the_key_holder() {
    run_generated(
        "structkeyholder",
        "module k { @appendable struct Inner { long x; string<8> tag; }; \
         @appendable struct Outer { @key k::Inner who; long n; }; \
         @final struct Plain { long a; short b; }; \
         @final struct Mixed { @key long id; @key k::Plain p; double v; }; };",
        r#"
        #[test]
        fn the_key_members_come_back_and_the_others_take_their_default() {
            use zerodds_dcps::DdsType;
            use zerodds_dcps::dds_type::PlainCdr2BeKeyHolder;
            let outer = k::Outer { who: k::Inner { x: 3, tag: "t".into() }, n: 9 };
            let mut holder = PlainCdr2BeKeyHolder::new();
            outer.encode_key_holder_be(&mut holder);
            let back = k::Outer::decode_key_holder_be(holder.as_bytes()).expect("Outer from its holder");
            assert_eq!(back, k::Outer { who: k::Inner { x: 3, tag: "t".into() }, n: 0 });
            assert_eq!(back.key_hash(), outer.key_hash());

            let mixed = k::Mixed { id: -4, p: k::Plain { a: 7, b: 8 }, v: 2.5 };
            let mut holder = PlainCdr2BeKeyHolder::new();
            mixed.encode_key_holder_be(&mut holder);
            // PLAIN_CDR2 big endian: id, then the nested struct's members.
            assert_eq!(holder.as_bytes(), [0xff, 0xff, 0xff, 0xfc, 0, 0, 0, 7, 0, 8]);
            let back = k::Mixed::decode_key_holder_be(holder.as_bytes()).expect("Mixed from its holder");
            assert_eq!(back, k::Mixed { id: -4, p: k::Plain { a: 7, b: 8 }, v: 0.0 });
        }
        "#,
    );
}

/// XTypes 1.3 §7.6.8 steps 1, 3 and 4, applied to a nested key member and
/// to a union: each aggregate inside the holder is its own holder,
/// `@final` — no DHEADER, no EMHEADER — and holds only its key members
/// when it declares any; the holder of a union keyed by its discriminator
/// is the discriminator (§7.2.2.4.4.4.8's `Command`, 1.4 Beta 1
/// §7.2.3.2.8's `Command_KeyHolder`). The octets are the PLAIN_CDR2 big
/// endian of those holders, worked by hand from the clause.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_6_8_a_nested_key_member_is_its_own_final_key_holder() {
    run_generated(
        "nestedholder",
        "module k { @appendable struct Inner { long x; string<8> tag; }; \
         @appendable struct Outer { @key k::Inner who; long n; }; \
         @mutable struct MInner { @key long id; long y; }; \
         @final struct MOuter { @key k::MInner m; long z; }; \
         enum CommandKind { START, STOP, GO_LEFT, GO_RIGHT }; \
         @appendable union Command switch (@key k::CommandKind) { case START: float delay; \
         case STOP: float distance; case GO_LEFT: case GO_RIGHT: float angle; }; \
         @appendable union Loose switch (short) { case 1: long a; case 2: k::MInner b; }; \
         @final struct Ordered { @key octet tag; @key k::Command c; @key k::Loose l; long v; }; };",
        r#"
        #[test]
        fn each_nested_holder_is_final_and_holds_its_keys() {
            use zerodds_dcps::DdsType;
            use zerodds_dcps::dds_type::PlainCdr2BeKeyHolder;
            fn holder<T: DdsType>(value: &T) -> Vec<u8> {
                let mut holder = PlainCdr2BeKeyHolder::new();
                value.encode_key_holder_be(&mut holder);
                holder.as_bytes().to_vec()
            }
            // Inner declares no key: all of it, without its DHEADER.
            let outer = k::Outer { who: k::Inner { x: 3, tag: "t".into() }, n: 9 };
            assert_eq!(holder(&outer), [0, 0, 0, 3, 0, 0, 0, 2, b't', 0]);
            let back = k::Outer::decode_key_holder_be(&holder(&outer)).expect("Outer");
            assert_eq!(back, k::Outer { who: k::Inner { x: 3, tag: "t".into() }, n: 0 });
            // MInner declares `id`: `id` alone, without EMHEADER.
            let m = k::MOuter { m: k::MInner { id: 5, y: 6 }, z: 7 };
            assert_eq!(holder(&m), [0, 0, 0, 5]);
            let back = k::MOuter::decode_key_holder_be(&holder(&m)).expect("MOuter");
            assert_eq!(back, k::MOuter { m: k::MInner { id: 5, y: 0 }, z: 0 });
            // A union keyed by its discriminator: GO_LEFT is 2.
            let command = k::Command::Angle(k::CommandKind::GO_LEFT, 0.5);
            assert!(k::Command::HAS_KEY);
            assert_eq!(holder(&command), [0, 0, 0, 2]);
            assert_eq!(
                DdsType::key_hash(&command),
                Some([0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
            );
            let back = k::Command::decode_key_holder_be(&holder(&command)).expect("Command");
            assert_eq!(back, k::Command::Angle(k::CommandKind::GO_LEFT, 0.0));
            // Inside a struct's holder: the keyed union's discriminator
            // alone, and the unkeyed union whole and `@final`, its branch
            // as its own holder — the octet, padding to four, 2, the short
            // 2, padding, MInner's `id`.
            let ordered = k::Ordered {
                tag: 7,
                c: k::Command::Angle(k::CommandKind::GO_LEFT, 0.5),
                l: k::Loose::B(k::MInner { id: 5, y: 6 }),
                v: 1,
            };
            assert_eq!(
                holder(&ordered),
                [7, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, 0, 0, 0, 0, 5]
            );
            let back = k::Ordered::decode_key_holder_be(&holder(&ordered)).expect("Ordered");
            assert_eq!(back.tag, 7);
            assert_eq!(back.c, k::Command::Angle(k::CommandKind::GO_LEFT, 0.0));
            assert_eq!(back.l, k::Loose::B(k::MInner { id: 5, y: 0 }));
            assert_eq!(back.v, 0);
        }
        "#,
    );
}

/// The IDL of the derive tests: one type per construct `derive(DdsType)`
/// takes, written by hand, as a `.idl` file would declare it.
const DERIVE_IDL: &str = "\
module DDS {
    @final struct Duration_t { long sec; unsigned long nanosec; };
    @final struct Time_t { long sec; unsigned long nanosec; };
};
module eq {
    enum Color { RED, GREEN, BLUE };
    @bit_bound(8) enum Level { LOW, @value(5) MID, HIGH };
    @final struct Prims { boolean b; octet o; char c; int8 i1; short s; unsigned short us;
        long l; unsigned long ul; long long ll; unsigned long long ull; float f; double d; };
    @appendable struct Inner { long x; string<8> tag; };
    @final struct Collections { sequence<long> longs; sequence<long, 3> few;
        sequence<eq::Inner> inners; long grid[2][3]; eq::Inner pair[2]; string<16> label;
        string text; sequence<string> names; };
    @appendable struct Optionals { @optional long a; @optional string b;
        @optional eq::Inner c; long d; };
    @mutable struct Mut { @key long id; @id(10) string name; @optional double score;
        eq::Color color; eq::Level level; };
    @final struct Keyed { @key long k1; @key string<8> k2; double v; };
    @appendable struct NestedKey { @key eq::Inner who; long n; };
    @mutable @autoid(HASH) struct Hashed { long a; @hashid string b; @hashid(\"other\") long c; };
    @final struct Times { DDS::Duration_t d; DDS::Time_t t; };
    @appendable struct Track { @key unsigned long id; string at; string<64> label;
        @optional string note; };
    struct Node;
    @appendable struct Node { long v; sequence<eq::Node> kids; };
    @final struct Manual { long a; string b; };
    @final union Pick switch (short) { case 1: long a; case 2: case 3: string b; default: double c; };
    @appendable union Choice switch (eq::Color) { case RED: long r; case GREEN: eq::Inner g; };
    @mutable union Flex switch (long) { case 0: long x; case 1: @id(9) string y; };
    @final struct Holder { eq::Pick p; sequence<eq::Pick> ps; eq::Choice c; eq::Flex f; };
    @appendable struct UnionKey { @key eq::Pick k; long v; };
    @appendable union Command switch (@key eq::Color) { case RED: long r;
        case GREEN: case BLUE: eq::Inner gb; };
    @mutable struct UnionKeys { @key octet tag; @key eq::Command c; @key eq::Choice h; long v; };
    @appendable struct Pair_long { long first; long second; };
    @appendable struct Pair_eq_Inner { eq::Inner first; eq::Inner second; };
    @final struct Grid_3 { long cells[3]; };
    @final struct Borrowed { string text; sequence<octet> data; string note; sequence<octet> blob;
        @key string name; };
    @final struct Vec3F32 { float x; float y; float z; };
    @final enum MessagePayloadDiscriminator { @value(0) MESSAGE_PAYLOAD_DISCRIMINATOR_POSITION,
        MESSAGE_PAYLOAD_DISCRIMINATOR_VELOCITY, MESSAGE_PAYLOAD_DISCRIMINATOR_HEARTBEAT,
        MESSAGE_PAYLOAD_DISCRIMINATOR_ACKNOWLEDGEMENT };
    @final union MessagePayload switch (eq::MessagePayloadDiscriminator) {
        case MESSAGE_PAYLOAD_DISCRIMINATOR_POSITION: eq::Vec3F32 position;
        case MESSAGE_PAYLOAD_DISCRIMINATOR_VELOCITY: eq::Vec3F32 velocity; };
};
";

/// The Rust side of [`DERIVE_IDL`]: each type defined in Rust with
/// `derive(DdsType)`, and `Manual` with its codec written by hand.
const DERIVE_ITEMS: &str = r#"
/// The date-time group as `derive(WireRepr)` writes it: `TryFrom<&str>`
/// and `Display`.
#[derive(zerodds_codec::WireRepr, Debug, Clone, Copy, PartialEq)]
#[wire(idl = "string")]
pub struct Stamp(pub Dtg);

impl core::fmt::Display for Stamp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.0, f)
    }
}

impl TryFrom<&str> for Stamp {
    type Error = &'static str;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Dtg::parse(text).map(Stamp).ok_or("a date-time group DDHHMMZMONYY")
    }
}

pub mod derived {
    use core::num::NonZeroU32;
    use zerodds_codec::DdsType;

    #[derive(DdsType, Debug, Clone, Copy, PartialEq)]
    #[dds(name = "eq::Color")]
    pub enum Color {
        #[dds(name = "RED")] Red,
        #[dds(name = "GREEN")] Green,
        #[dds(name = "BLUE")] Blue,
    }

    // GitHub #33: a `@final` enumeration as a discriminator, two of whose
    // values select no member; once as unit variants, once as `no_member`.
    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Vec3F32", extensibility = "final")]
    pub struct Vec3F32 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
    }

    #[derive(DdsType, Debug, Clone, Copy, PartialEq)]
    #[dds(name = "eq::MessagePayloadDiscriminator", extensibility = "final")]
    pub enum MessagePayloadDiscriminator {
        #[dds(name = "MESSAGE_PAYLOAD_DISCRIMINATOR_POSITION", value = 0)] Position,
        #[dds(name = "MESSAGE_PAYLOAD_DISCRIMINATOR_VELOCITY")] Velocity,
        #[dds(name = "MESSAGE_PAYLOAD_DISCRIMINATOR_HEARTBEAT")] Heartbeat,
        #[dds(name = "MESSAGE_PAYLOAD_DISCRIMINATOR_ACKNOWLEDGEMENT")] Acknowledgement,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::MessagePayload", extensibility = "final",
        discriminator = "MessagePayloadDiscriminator")]
    pub enum MessagePayload {
        #[dds(case = MessagePayloadDiscriminator::Position, name = "position")]
        Position(Vec3F32),
        #[dds(case = MessagePayloadDiscriminator::Velocity, name = "velocity")]
        Velocity(Vec3F32),
        #[dds(case = MessagePayloadDiscriminator::Heartbeat)]
        Heartbeat,
        #[dds(case = MessagePayloadDiscriminator::Acknowledgement)]
        Acknowledgement,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::MessagePayload", extensibility = "final",
        discriminator = "MessagePayloadDiscriminator")]
    pub enum MessagePayloadSignal {
        #[dds(case = MessagePayloadDiscriminator::Position, name = "position")]
        Position(Vec3F32),
        #[dds(case = MessagePayloadDiscriminator::Velocity, name = "velocity")]
        Velocity(Vec3F32),
        #[dds(no_member)]
        Signal(MessagePayloadDiscriminator),
    }

    #[derive(DdsType, Debug, Clone, Copy, PartialEq)]
    #[dds(name = "eq::Level", bit_bound = 8)]
    pub enum Level {
        #[dds(name = "LOW")] Low,
        #[dds(name = "MID")] Mid = 5,
        #[dds(name = "HIGH")] High,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Prims", extensibility = "final")]
    pub struct Prims {
        pub b: bool, pub o: u8, pub c: char, pub i1: i8, pub s: i16, pub us: u16, pub l: i32,
        pub ul: u32, pub ll: i64, pub ull: u64, pub f: f32, pub d: f64,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Inner")]
    pub struct Inner {
        pub x: i32,
        #[dds(bound = 8)]
        pub tag: String,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Collections", extensibility = "final")]
    pub struct Collections {
        pub longs: Vec<i32>,
        #[dds(bound = 3)]
        pub few: Vec<i32>,
        pub inners: Vec<Inner>,
        pub grid: [[i32; 3]; 2],
        pub pair: [Inner; 2],
        #[dds(bound = 16)]
        pub label: String,
        pub text: String,
        pub names: Vec<String>,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Optionals", extensibility = "appendable")]
    pub struct Optionals {
        pub a: Option<i32>,
        pub b: Option<String>,
        #[dds(optional)]
        pub c: Option<Inner>,
        pub d: i32,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Mut", extensibility = "mutable")]
    pub struct Mut {
        #[dds(key)]
        pub id: i32,
        #[dds(id = 10)]
        pub name: String,
        pub score: Option<f64>,
        pub color: Color,
        pub level: Level,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Keyed", extensibility = "final")]
    pub struct Keyed {
        #[dds(key)]
        pub k1: i32,
        #[dds(key, bound = 8)]
        pub k2: String,
        pub v: f64,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::NestedKey")]
    pub struct NestedKey {
        #[dds(key)]
        pub who: Inner,
        pub n: i32,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Hashed", extensibility = "mutable", autoid = "hash")]
    pub struct Hashed {
        pub a: i32,
        #[dds(hashid)]
        pub b: String,
        #[dds(hashid = "other")]
        pub c: i32,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Times", extensibility = "final")]
    pub struct Times {
        pub d: core::time::Duration,
        pub t: std::time::SystemTime,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Track", extensibility = "appendable")]
    pub struct Track {
        #[dds(key)]
        pub id: NonZeroU32,
        pub at: super::Stamp,
        #[dds(bound = 64)]
        pub label: String,
        pub note: Option<String>,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Node")]
    pub struct Node {
        pub v: i32,
        pub kids: Vec<Node>,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Pick", extensibility = "final", discriminator = "i16")]
    pub enum Pick {
        #[dds(case = 1, name = "a")]
        A(i32),
        #[dds(case = [2, 3], name = "b")]
        B(i16, String),
        #[dds(default, name = "c")]
        C(i16, f64),
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Choice", discriminator = "Color")]
    pub enum Choice {
        #[dds(case = Color::Red, name = "r")]
        R(i32),
        #[dds(case = Color::Green, name = "g")]
        G(Inner),
        #[dds(no_member)]
        Other(Color),
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Flex", extensibility = "mutable")]
    pub enum Flex {
        #[dds(name = "x")]
        X(i32),
        #[dds(name = "y", id = 9)]
        Y(String),
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Holder", extensibility = "final")]
    pub struct Holder {
        pub p: Pick,
        pub ps: Vec<Pick>,
        pub c: Choice,
        pub f: Flex,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::UnionKey")]
    pub struct UnionKey {
        #[dds(key)]
        pub k: Pick,
        pub v: i32,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Command", discriminator = "Color", key)]
    pub enum Command {
        #[dds(case = Color::Red, name = "r")]
        R(i32),
        #[dds(case = [Color::Green, Color::Blue], name = "gb")]
        Gb(Color, Inner),
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::UnionKeys", extensibility = "mutable")]
    pub struct UnionKeys {
        #[dds(key)]
        pub tag: u8,
        #[dds(key)]
        pub c: Command,
        #[dds(key)]
        pub h: Choice,
        pub v: i32,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Pair_{T}")]
    pub struct Pair<T> {
        pub first: T,
        pub second: T,
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Grid_{N}", extensibility = "final")]
    pub struct Grid<const N: usize> {
        pub cells: [i32; N],
    }

    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Borrowed", extensibility = "final")]
    pub struct Borrowed<'a> {
        pub text: &'a str,
        pub data: &'a [u8],
        pub note: std::borrow::Cow<'a, str>,
        pub blob: std::borrow::Cow<'a, [u8]>,
        #[dds(key)]
        pub name: &'a str,
    }

    /// The description from the derive, the codec by hand.
    #[derive(DdsType, Debug, Clone, PartialEq)]
    #[dds(name = "eq::Manual", extensibility = "final", codec = "manual")]
    pub struct Manual {
        pub a: i32,
        pub b: String,
    }

    impl zerodds_codec::DdsSerialize for Manual {
        fn serialize(
            &self,
            out: zerodds_codec::DdsSerializer<'_>,
        ) -> Result<zerodds_codec::Serialized, zerodds_codec::WireError> {
            out.serialize_struct::<Self>()?.member(&self.a)?.member(&self.b)?.end()
        }
    }

    impl<'de> zerodds_codec::DdsDeserialize<'de> for Manual {
        fn deserialize(
            input: zerodds_codec::DdsDeserializer<'_, 'de>,
        ) -> Result<zerodds_codec::Decoded<Self>, zerodds_codec::WireError> {
            let input = input.deserialize_struct::<Self>()?;
            let (a, input) = input.member()?;
            let (b, input) = input.member()?;
            Ok(input.end()?.with(Self { a, b }))
        }
    }
}
"#;

/// GitHub #33, the derive route: for every construct `derive(DdsType)`
/// takes — struct members of every kind, unions from data-carrying enums
/// (several labels, `default:`, no member, an enumeration discriminator,
/// `@mutable` framing, as a member, in a sequence, as a key), each
/// instantiation of a generic type, and borrowed fields — the type
/// defined in Rust and the type `zerodds-build` generates from the
/// equivalent IDL have the same `TypeIdentifier`, the same `TypeObject` and
/// dependencies (XTypes 1.3 §7.3.4), write every sample to the same octets
/// under XCDR2 and XCDR1 in both byte orders (§7.4), read each other's
/// octets, and have the same key holder and KeyHash (§7.6.8). A borrowed
/// field reads lent out of the sample. The oracle is the generated codec of
/// the IDL route.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_3_4_7_4_issue_33_a_derived_type_and_its_idl_are_one_type() {
    let items = format!("{ISSUE_33_DTG}\n{DERIVE_ITEMS}");
    run_generated_items(
        "issue33derive",
        DERIVE_IDL,
        ".rust_type(\"eq::Track.at\", \"crate::Dtg\").rust_type(\"eq::Track.id\", \"core::num::NonZeroU32\")",
        &items,
        r##"
        use core::num::NonZeroU32;
        use std::time::{Duration, UNIX_EPOCH};
        use zerodds_dcps::DdsType;
        use zerodds_dcps::dds_type::PlainCdr2BeKeyHolder;

        type Codec<T> = fn(&T, &mut Vec<u8>) -> Result<(), zerodds_dcps::EncodeError>;
        type Read<T> = fn(&[u8]) -> Result<T, zerodds_dcps::DecodeError>;

        fn encodings<T: DdsType>() -> [(&'static str, Codec<T>, Read<T>); 4] {
            [
                ("XCDR2 LE", T::encode, T::decode),
                ("XCDR2 BE", T::encode_be, T::decode_be),
                ("XCDR1 LE", T::encode_xcdr1, T::decode_xcdr1),
                ("XCDR1 BE", T::encode_xcdr1_be, T::decode_xcdr1_be),
            ]
        }

        fn one_type<D: DdsType + core::fmt::Debug, G: DdsType>(derived: &D, generated: &G) {
            // `type_name()`, not `TYPE_NAME`: a generic type's name is made
            // from its arguments at run time.
            let name = G::type_name();
            assert_eq!(D::type_name(), name);
            assert_eq!(D::type_identifier(), G::type_identifier(), "TypeIdentifier of {name}");
            assert!(
                !matches!(G::type_identifier(), zerodds_types::TypeIdentifier::None),
                "{name} has an identifier to compare"
            );
            assert_eq!(D::type_object(), G::type_object(), "TypeObject of {name}");
            assert!(G::type_object().is_some(), "{name} has an object to compare");
            // The closure as a set: its order is the order the IDL declares
            // the types in, which no peer reads.
            let closure = |objects: Vec<zerodds_types::type_object::TypeObject>| {
                let mut octets: Vec<Vec<u8>> = objects
                    .into_iter()
                    .map(|object| object.to_bytes_le().expect("octets"))
                    .collect();
                octets.sort();
                octets
            };
            assert_eq!(
                closure(D::type_object_dependencies()),
                closure(G::type_object_dependencies()),
                "dependencies of {name}"
            );
            assert_eq!(D::type_component(), G::type_component(), "component of {name}");
            assert_eq!(D::extensibility(), G::extensibility(), "extensibility of {name}");
            assert_eq!(D::has_key(), G::has_key(), "key of {name}");
            assert_eq!(D::key_holder_max_size(), G::key_holder_max_size(), "holder of {name}");
            for ((label, d_encode, d_decode), (_, g_encode, g_decode)) in
                encodings::<D>().into_iter().zip(encodings::<G>())
            {
                let mut theirs = Vec::new();
                let generated_result = g_encode(generated, &mut theirs);
                let mut ours = Vec::new();
                let derived_result = d_encode(derived, &mut ours);
                match (derived_result, generated_result) {
                    (Ok(()), Ok(())) => {}
                    (d, g) => panic!("{label} of {name}: derived {d:?}, generated {g:?}"),
                }
                assert_eq!(ours, theirs, "{label} octets of {name}");
                let back = d_decode(&theirs)
                    .unwrap_or_else(|e| panic!("{label}: the derived codec reads {name}: {e}"));
                let mut again = Vec::new();
                d_encode(&back, &mut again).expect("encode what was read");
                assert_eq!(again, theirs, "{label}: {name} read and written again");
                g_decode(&ours)
                    .unwrap_or_else(|e| panic!("{label}: the generated codec reads {name}: {e}"));
            }
            assert_eq!(derived.key_hash(), generated.key_hash(), "KeyHash of {name}");
            if G::has_key() {
                let mut ours = PlainCdr2BeKeyHolder::new();
                derived.encode_key_holder_be(&mut ours);
                let mut theirs = PlainCdr2BeKeyHolder::new();
                generated.encode_key_holder_be(&mut theirs);
                assert_eq!(ours.as_bytes(), theirs.as_bytes(), "key holder of {name}");
                // The members the holder does not carry take their IDL
                // default; a Rust type that refuses its default refuses the
                // holder, on both routes.
                let ours_back = D::decode_key_holder_be(ours.as_bytes());
                let theirs_back = G::decode_key_holder_be(theirs.as_bytes());
                assert_eq!(
                    ours_back.is_ok(),
                    theirs_back.is_ok(),
                    "{name} from its key holder: derived {:?}, generated {:?}",
                    ours_back.as_ref().err(),
                    theirs_back.as_ref().err()
                );
                if let Ok(back) = ours_back {
                    assert_eq!(back.key_hash(), derived.key_hash(), "{name} from its key holder");
                }
            }
        }

        fn inner(x: i32, tag: &str) -> (derived::Inner, eq::Inner) {
            (
                derived::Inner { x, tag: tag.into() },
                eq::Inner { x, tag: tag.into() },
            )
        }

        #[test]
        fn primitives() {
            one_type(
                &derived::Prims { b: true, o: 0xab, c: 'Z', i1: -7, s: -300, us: 60_000,
                    l: -70_000, ul: 4_000_000_000, ll: -5_000_000_000,
                    ull: 18_000_000_000_000_000_000, f: 1.5, d: -2.25 },
                &eq::Prims { b: true, o: 0xab, c: 'Z', i1: -7, s: -300, us: 60_000,
                    l: -70_000, ul: 4_000_000_000, ll: -5_000_000_000,
                    ull: 18_000_000_000_000_000_000, f: 1.5, d: -2.25 },
            );
        }

        #[test]
        fn collections_strings_and_arrays() {
            let (d1, g1) = inner(1, "a");
            let (d2, g2) = inner(2, "bb");
            let (d3, g3) = inner(3, "c");
            let (d4, g4) = inner(4, "dd");
            one_type(
                &derived::Collections { longs: vec![1, 2, 3, 4], few: vec![9, 8],
                    inners: vec![d1, d2], grid: [[1, 2, 3], [4, 5, 6]], pair: [d3, d4],
                    label: "label".into(), text: "text".into(),
                    names: vec!["x".into(), "yy".into()] },
                &eq::Collections { longs: vec![1, 2, 3, 4], few: vec![9, 8],
                    inners: vec![g1, g2], grid: [[1, 2, 3], [4, 5, 6]], pair: [g3, g4],
                    label: "label".into(), text: "text".into(),
                    names: vec!["x".into(), "yy".into()] },
            );
        }

        #[test]
        fn optional_members_present_and_absent() {
            let (d, g) = inner(7, "t");
            one_type(
                &derived::Optionals { a: Some(5), b: Some("bee".into()), c: Some(d), d: 9 },
                &eq::Optionals { a: Some(5), b: Some("bee".into()), c: Some(g), d: 9 },
            );
            one_type(
                &derived::Optionals { a: None, b: None, c: None, d: 1 },
                &eq::Optionals { a: None, b: None, c: None, d: 1 },
            );
        }

        #[test]
        fn mutable_members_ids_and_enums() {
            one_type(
                &derived::Mut { id: 42, name: "nm".into(), score: Some(0.5),
                    color: derived::Color::Green, level: derived::Level::Mid },
                &eq::Mut { id: 42, name: "nm".into(), score: Some(0.5),
                    color: eq::Color::GREEN, level: eq::Level::MID },
            );
            one_type(
                &derived::Mut { id: -1, name: String::new(), score: None,
                    color: derived::Color::Blue, level: derived::Level::High },
                &eq::Mut { id: -1, name: String::new(), score: None,
                    color: eq::Color::BLUE, level: eq::Level::HIGH },
            );
        }

        #[test]
        fn keys() {
            one_type(
                &derived::Keyed { k1: 3, k2: "key".into(), v: 2.5 },
                &eq::Keyed { k1: 3, k2: "key".into(), v: 2.5 },
            );
            let (d, g) = inner(8, "w");
            one_type(
                &derived::NestedKey { who: d, n: 1 },
                &eq::NestedKey { who: g, n: 1 },
            );
        }

        #[test]
        fn hashed_member_ids() {
            one_type(
                &derived::Hashed { a: 1, b: "h".into(), c: 2 },
                &eq::Hashed { a: 1, b: "h".into(), c: 2 },
            );
        }

        #[test]
        fn duration_and_system_time_are_duration_t_and_time_t() {
            one_type(
                &derived::Times { d: Duration::new(5, 250),
                    t: UNIX_EPOCH + Duration::new(1_700_000_000, 999) },
                &eq::Times { d: DDS::Duration_t { sec: 5, nanosec: 250 },
                    t: DDS::Time_t { sec: 1_700_000_000, nanosec: 999 } },
            );
            one_type(
                &derived::Times { d: Duration::MAX, t: UNIX_EPOCH },
                &eq::Times { d: DDS::Duration_t { sec: 0x7fff_ffff, nanosec: 0x7fff_ffff },
                    t: DDS::Time_t { sec: 0, nanosec: 0 } },
            );
        }

        #[test]
        fn wire_repr_members_and_optional_text() {
            let id = NonZeroU32::new(77).expect("non-zero");
            let at = Dtg::new(5, 14, 30, 11, 26);
            one_type(
                &derived::Track { id, at: Stamp(at), label: "convoy".into(),
                    note: Some("n".into()) },
                &eq::Track { id, at, label: "convoy".into(), note: Some("n".into()) },
            );
            one_type(
                &derived::Track { id, at: Stamp(at), label: String::new(), note: None },
                &eq::Track { id, at, label: String::new(), note: None },
            );
        }

        #[test]
        fn a_type_that_reaches_itself() {
            let leaf = |v| (derived::Node { v, kids: vec![] }, eq::Node { v, kids: vec![] });
            let ((d2, g2), (d4, g4)) = (leaf(2), leaf(4));
            one_type(
                &derived::Node { v: 1, kids: vec![d2, derived::Node { v: 3, kids: vec![d4] }] },
                &eq::Node { v: 1, kids: vec![g2, eq::Node { v: 3, kids: vec![g4] }] },
            );
        }

        #[test]
        fn a_hand_written_codec_over_a_derived_description() {
            one_type(
                &derived::Manual { a: 1, b: "m".into() },
                &eq::Manual { a: 1, b: "m".into() },
            );
        }

        #[test]
        fn unions_with_labels_a_default_no_member_and_mutable_framing() {
            one_type(&derived::Pick::A(5), &eq::Pick::A(5));
            one_type(&derived::Pick::B(3, "bee".into()), &eq::Pick::B(3, "bee".into()));
            one_type(&derived::Pick::C(77, 2.5), &eq::Pick::C(77, 2.5));
            one_type(&derived::Choice::R(7), &eq::Choice::R(7));
            let (d, g) = inner(1, "u");
            one_type(&derived::Choice::G(d), &eq::Choice::G(g));
            one_type(
                &derived::Choice::Other(derived::Color::Blue),
                &eq::Choice::_NoMember(eq::Color::BLUE),
            );
            one_type(&derived::Flex::X(4), &eq::Flex::X(4));
            one_type(&derived::Flex::Y("why".into()), &eq::Flex::Y("why".into()));
        }

        /// GitHub #33: a union switched by a `@final` enumeration, two of
        /// whose values select no member — as unit variants with `case`,
        /// and as one `no_member` variant — is the type of its IDL.
        #[test]
        fn issue_33_a_union_whose_discriminator_values_select_no_member() {
            let v = |x: f32| (
                derived::Vec3F32 { x, y: 2.0, z: -3.5 },
                eq::Vec3F32 { x, y: 2.0, z: -3.5 },
            );
            let (dv, gv) = v(1.0);
            one_type(&derived::MessagePayload::Position(dv.clone()), &eq::MessagePayload::Position(gv.clone()));
            one_type(&derived::MessagePayloadSignal::Position(dv), &eq::MessagePayload::Position(gv));
            let (dv, gv) = v(-0.25);
            one_type(&derived::MessagePayload::Velocity(dv), &eq::MessagePayload::Velocity(gv));
            use eq::MessagePayloadDiscriminator as G;
            use derived::MessagePayloadDiscriminator as D;
            one_type(
                &derived::MessagePayload::Heartbeat,
                &eq::MessagePayload::_NoMember(G::MESSAGE_PAYLOAD_DISCRIMINATOR_HEARTBEAT),
            );
            one_type(
                &derived::MessagePayload::Acknowledgement,
                &eq::MessagePayload::_NoMember(G::MESSAGE_PAYLOAD_DISCRIMINATOR_ACKNOWLEDGEMENT),
            );
            one_type(
                &derived::MessagePayloadSignal::Signal(D::Heartbeat),
                &eq::MessagePayload::_NoMember(G::MESSAGE_PAYLOAD_DISCRIMINATOR_HEARTBEAT),
            );
            one_type(
                &derived::MessagePayloadSignal::Signal(D::Acknowledgement),
                &eq::MessagePayload::_NoMember(G::MESSAGE_PAYLOAD_DISCRIMINATOR_ACKNOWLEDGEMENT),
            );
        }

        #[test]
        fn unions_as_members_in_sequences_and_as_a_key() {
            one_type(
                &derived::Holder {
                    p: derived::Pick::A(1),
                    ps: vec![derived::Pick::B(2, "x".into()), derived::Pick::C(-5, 0.5)],
                    c: derived::Choice::R(3),
                    f: derived::Flex::Y("f".into()),
                },
                &eq::Holder {
                    p: eq::Pick::A(1),
                    ps: vec![eq::Pick::B(2, "x".into()), eq::Pick::C(-5, 0.5)],
                    c: eq::Choice::R(3),
                    f: eq::Flex::Y("f".into()),
                },
            );
            one_type(
                &derived::UnionKey { k: derived::Pick::B(3, "k".into()), v: 9 },
                &eq::UnionKey { k: eq::Pick::B(3, "k".into()), v: 9 },
            );
        }

        /// DDS-XTypes 1.3 §7.2.2.4.4.4.8: `switch (@key D)` makes the
        /// discriminator the union's key; as a topic type its holder is the
        /// discriminator, and as a struct's key member only the
        /// discriminator is part of that key (§7.6.8 step 3), where an
        /// unkeyed union is whole and `@final`.
        #[test]
        fn a_union_keyed_by_its_discriminator() {
            let (d, g) = inner(4, "k");
            one_type(
                &derived::Command::Gb(derived::Color::Blue, d.clone()),
                &eq::Command::Gb(eq::Color::BLUE, g.clone()),
            );
            assert!(<derived::Command as DdsType>::has_key());
            one_type(&derived::Command::R(-2), &eq::Command::R(-2));
            one_type(
                &derived::UnionKeys {
                    tag: 9,
                    c: derived::Command::Gb(derived::Color::Green, d.clone()),
                    h: derived::Choice::G(d),
                    v: 3,
                },
                &eq::UnionKeys {
                    tag: 9,
                    c: eq::Command::Gb(eq::Color::GREEN, g.clone()),
                    h: eq::Choice::G(g),
                    v: 3,
                },
            );
            // The holder of `Command` is its discriminator: BLUE is 2.
            let mut holder = PlainCdr2BeKeyHolder::new();
            derived::Command::Gb(derived::Color::Blue, inner(0, "").0)
                .encode_key_holder_be(&mut holder);
            assert_eq!(holder.as_bytes(), [0, 0, 0, 2]);
        }

        /// DDS-XTypes 1.4 Beta 1 §7.6.7.1: a filter names a union's branch
        /// by its member's name and its discriminator `#discriminator`;
        /// "if the discriminator does not select the named member, the
        /// result of the filter evaluation shall be false". An enumerated
        /// discriminator compares with its enumerator's name (DDS 1.4
        /// Annex B.2). Both routes answer every expression as the clause
        /// does.
        #[test]
        fn a_filter_reads_union_branches_and_the_discriminator() {
            fn passes<T: DdsType>(expression: &str, sample: &T) -> bool {
                let base = zerodds_dcps::ReadCondition::new(0xFFFF, 0xFFFF, 0xFFFF, |_, _, _| true);
                zerodds_dcps::QueryCondition::new(base, expression, Vec::new())
                    .expect("expression")
                    .evaluate(&zerodds_dcps::DdsTypeRow::new(sample))
                    .unwrap_or(false)
            }
            let derived = derived::Holder {
                p: derived::Pick::B(3, "bee".into()),
                ps: Vec::new(),
                c: derived::Choice::R(7),
                f: derived::Flex::Y("why".into()),
            };
            let generated = eq::Holder {
                p: eq::Pick::B(3, "bee".into()),
                ps: Vec::new(),
                c: eq::Choice::R(7),
                f: eq::Flex::Y("why".into()),
            };
            for (expression, expected) in [
                ("p.#discriminator = 3", true),
                ("p.b = 'bee'", true),
                ("p.a = 1", false),
                ("p.a <> 1", false),
                ("NOT p.c = 0.5", false),
                ("c.#discriminator = 'RED' AND c.r = 7", true),
                ("c.g.x = 1", false),
                ("f.#discriminator = 1 AND f.y = 'why'", true),
                ("f.x = 0 OR f.y = 'why'", false),
            ] {
                assert_eq!(passes(expression, &derived), expected, "derived: {expression}");
                assert_eq!(passes(expression, &generated), expected, "generated: {expression}");
            }
            // A union as the topic type: its branches at the root.
            assert!(passes("#discriminator = 3 AND b = 'bee'", &derived::Pick::B(3, "bee".into())));
            assert!(passes("#discriminator = 3 AND b = 'bee'", &eq::Pick::B(3, "bee".into())));
            assert!(!passes("a = 1", &eq::Pick::B(3, "bee".into())));
            assert!(passes("#discriminator = 'BLUE'", &eq::Choice::_NoMember(eq::Color::BLUE)));
            assert!(passes("#discriminator = 'BLUE'", &derived::Choice::Other(derived::Color::Blue)));
            assert!(!passes("g.x = 1", &eq::Choice::_NoMember(eq::Color::BLUE)));
        }

        #[test]
        fn each_instantiation_of_a_generic_type_is_its_own_idl_type() {
            one_type(
                &derived::Pair::<i32> { first: 1, second: 2 },
                &eq::Pair_long { first: 1, second: 2 },
            );
            let ((d1, g1), (d2, g2)) = (inner(1, "a"), inner(2, "b"));
            one_type(
                &derived::Pair::<derived::Inner> { first: d1, second: d2 },
                &eq::Pair_eq_Inner { first: g1, second: g2 },
            );
            one_type(&derived::Grid::<3> { cells: [1, 2, 3] }, &eq::Grid_3 { cells: [1, 2, 3] });
            assert_eq!(<derived::Pair<i32> as DdsType>::type_name(), "eq::Pair_long");
            assert_eq!(<derived::Grid<3> as DdsType>::type_name(), "eq::Grid_3");
        }

        #[test]
        fn borrowed_fields_write_the_idl_octets_and_lend_from_the_sample() {
            use std::borrow::Cow;
            use zerodds_codec::Encapsulation;
            let data = [1u8, 2, 3];
            let ours_value = derived::Borrowed {
                text: "lent",
                data: &data,
                note: Cow::Borrowed("cow"),
                blob: Cow::Borrowed(&data[1..]),
                name: "n",
            };
            let theirs_value = eq::Borrowed {
                text: "lent".into(),
                data: vec![1, 2, 3],
                note: "cow".into(),
                blob: vec![2, 3],
                name: "n".into(),
            };
            let model = zerodds_codec::model::model::<derived::Borrowed<'_>>().expect("model");
            assert_eq!(model.type_identifier, <eq::Borrowed as DdsType>::type_identifier());
            assert_eq!(model.type_object, <eq::Borrowed as DdsType>::type_object());
            let encodings: [(Encapsulation, Codec<eq::Borrowed>); 4] = [
                (Encapsulation::Xcdr2Le, <eq::Borrowed as DdsType>::encode),
                (Encapsulation::Xcdr2Be, <eq::Borrowed as DdsType>::encode_be),
                (Encapsulation::Xcdr1Le, <eq::Borrowed as DdsType>::encode_xcdr1),
                (Encapsulation::Xcdr1Be, <eq::Borrowed as DdsType>::encode_xcdr1_be),
            ];
            for (encapsulation, encode) in encodings {
                let mut theirs = Vec::new();
                encode(&theirs_value, &mut theirs).expect("the generated codec writes");
                let mut ours = Vec::new();
                zerodds_codec::encode(&ours_value, encapsulation, &mut ours).expect("derived");
                assert_eq!(ours, theirs, "{encapsulation:?} octets");
                let back: derived::Borrowed<'_> =
                    zerodds_codec::decode(&theirs, encapsulation).expect("read");
                assert_eq!(back, ours_value, "{encapsulation:?} read back");
                let range = theirs.as_ptr_range();
                assert!(range.contains(&back.text.as_ptr()), "the text is lent from the sample");
                assert!(range.contains(&back.data.as_ptr()), "the octets are lent from the sample");
                assert!(matches!(back.note, Cow::Borrowed(_)) && matches!(back.blob, Cow::Borrowed(_)));
            }
            let mut holder = PlainCdr2BeKeyHolder::new();
            theirs_value.encode_key_holder_be(&mut holder);
            assert_eq!(zerodds_codec::key_holder(&ours_value).expect("holder"), holder.as_bytes());
            assert_eq!(
                zerodds_codec::key_hash(&ours_value).expect("hash"),
                <eq::Borrowed as DdsType>::key_hash(&theirs_value)
            );
        }
        "##,
    );
}

/// GitHub #33, the derive route: a C peer generated from the IDL a derived
/// type exports (`Report::idl()`) reads what the derived type wrote.
/// `runtime/c` and the C column's own codec decode the octets and print
/// them.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_4_issue_33_a_c_peer_reads_what_a_derived_type_wrote() {
    let items = format!(
        "{ISSUE_33_DTG}\n{}",
        r#"
#[derive(zerodds_codec::WireRepr, Debug, Clone, Copy, PartialEq)]
#[wire(idl = "string")]
pub struct Stamp(pub Dtg);

impl core::fmt::Display for Stamp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.0, f)
    }
}

impl TryFrom<&str> for Stamp {
    type Error = &'static str;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Dtg::parse(text).map(Stamp).ok_or("a date-time group DDHHMMZMONYY")
    }
}

#[derive(zerodds_codec::DdsType, Debug, Clone, PartialEq)]
#[dds(name = "app::Report", extensibility = "final")]
pub struct Report {
    #[dds(key)]
    pub id: core::num::NonZeroU32,
    #[dds(bound = 12)]
    pub stamp: Stamp,
    #[dds(bound = 4)]
    pub history: Vec<Stamp>,
    pub count: i32,
}
"#
    );
    let dir = run_generated_items(
        "issue33derivec",
        "module unused { @final struct U { long a; }; };",
        "",
        &items,
        r#"
        #[test]
        fn write_the_sample_and_the_idl() {
            use zerodds_dcps::DdsType;
            let report = Report {
                id: core::num::NonZeroU32::new(77).expect("non-zero"),
                stamp: Stamp(Dtg::new(5, 14, 30, 11, 26)),
                history: vec![Stamp(Dtg::new(1, 0, 5, 1, 27)), Stamp(Dtg::new(31, 23, 59, 12, 25))],
                count: -9,
            };
            let mut bytes = Vec::new();
            <Report as DdsType>::encode(&report, &mut bytes).expect("encode");
            std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/sample.bin"), bytes).expect("write");
            std::fs::write(
                concat!(env!("CARGO_MANIFEST_DIR"), "/exported.idl"),
                Report::idl().expect("the IDL of Report"),
            )
            .expect("write");
        }
        "#,
    );
    let sample = fs::read(dir.join("sample.bin")).expect("the sample the derived type wrote");
    let idl = fs::read_to_string(dir.join("exported.idl")).expect("the exported IDL");
    assert_eq!(
        idl.trim(),
        "module app { @final struct Report { @key unsigned long id; string<12> stamp; \
         sequence<string, 4> history; long count; }; };",
        "the IDL the derived type exports"
    );
    let out = run_c_peer(
        &dir,
        &idl,
        &sample,
        "    app_Report back;\n\
         \x20   zdw_reader r;\n\
         \x20   uint8_t heap[512];\n\
         \x20   uint32_t i;\n\
         \x20   memset(&back, 0, sizeof back);\n\
         \x20   zdw_reader_init(&r, sample, sizeof sample, ZDW_LITTLE, 4);\n\
         \x20   zdw_reader_set_heap(&r, heap, sizeof heap);\n\
         \x20   zd_dec_app_Report(&r, &back, ZDW_TC_DISCARD);\n\
         \x20   printf(\"status %d id %u stamp %s count %d\\n\", (int)zdw_reader_status(&r),\n\
         \x20          (unsigned)back.id, back.stamp, (int)back.count);\n\
         \x20   for (i = 0; i < back.history._length; i++) { printf(\"history %s\\n\", back.history._buffer[i]); }\n",
    );
    assert_eq!(
        out,
        "status 0 id 77 stamp 051430ZNOV26 count -9\nhistory 010005ZJAN27\nhistory 312359ZDEC25\n",
        "the C peer's reading"
    );
}

/// GitHub #33, the derive route: a C peer generated from the IDL a derived
/// type exports reads a union from a data-carrying enum and an
/// instantiation of a generic type, as `runtime/c` and the C column decode
/// them (`_d` and `_u` of the union, IDL 4.2 §7.4.1.4.4.4).
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn xtypes_7_4_issue_33_a_c_peer_reads_a_derived_union_and_generic_type() {
    let items = r#"
#[derive(zerodds_codec::DdsType, Debug, Clone, PartialEq)]
#[dds(name = "app::Shape", extensibility = "final", discriminator = "i16")]
pub enum Shape {
    #[dds(case = 1, name = "radius")]
    Circle(i32),
    #[dds(case = 2, name = "label")]
    Tag(String),
}

#[derive(zerodds_codec::DdsType, Debug, Clone, PartialEq)]
#[dds(name = "app::Pair_{T}", extensibility = "final")]
pub struct Pair<T> {
    pub first: T,
    pub second: T,
}

#[derive(zerodds_codec::DdsType, Debug, Clone, PartialEq)]
#[dds(name = "app::Scene", extensibility = "final")]
pub struct Scene {
    pub shape: Shape,
    pub other: Shape,
    pub pair: Pair<i32>,
}
"#;
    let dir = run_generated_items(
        "issue33derivecunion",
        "module unused { @final struct U { long a; }; };",
        "",
        items,
        r#"
        #[test]
        fn write_the_sample_and_the_idl() {
            use zerodds_dcps::DdsType;
            let scene = Scene {
                shape: Shape::Circle(9),
                other: Shape::Tag("tag".into()),
                pair: Pair { first: -4, second: 12 },
            };
            let mut bytes = Vec::new();
            <Scene as DdsType>::encode(&scene, &mut bytes).expect("encode");
            std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/sample.bin"), bytes).expect("write");
            std::fs::write(
                concat!(env!("CARGO_MANIFEST_DIR"), "/exported.idl"),
                Scene::idl().expect("the IDL of Scene"),
            )
            .expect("write");
        }
        "#,
    );
    let sample = fs::read(dir.join("sample.bin")).expect("the sample the derived type wrote");
    let idl = fs::read_to_string(dir.join("exported.idl")).expect("the exported IDL");
    assert!(
        idl.contains("module app { @final union Shape switch (short) { case 1: long radius; case 2: string label; }; };")
            && idl.contains("module app { @final struct Pair_long { long first; long second; }; };"),
        "the IDL the derived types export:\n{idl}"
    );
    let out = run_c_peer(
        &dir,
        &idl,
        &sample,
        "    app_Scene back;\n\
         \x20   zdw_reader r;\n\
         \x20   uint8_t heap[256];\n\
         \x20   memset(&back, 0, sizeof back);\n\
         \x20   zdw_reader_init(&r, sample, sizeof sample, ZDW_LITTLE, 4);\n\
         \x20   zdw_reader_set_heap(&r, heap, sizeof heap);\n\
         \x20   zd_dec_app_Scene(&r, &back, ZDW_TC_DISCARD);\n\
         \x20   printf(\"status %d shape %d %d other %d %s pair %d %d\\n\", (int)zdw_reader_status(&r),\n\
         \x20          (int)back.shape._d, (int)back.shape._u.radius, (int)back.other._d,\n\
         \x20          back.other._u.label, (int)back.pair.first, (int)back.pair.second);\n",
    );
    assert_eq!(
        out, "status 0 shape 1 9 other 2 tag pair -4 12\n",
        "the C peer's reading"
    );
}

/// GitHub #33, the typestate of a hand-written union codec: the branch is
/// a type-level index and the value has that branch's type; a value of
/// another type does not compile.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn issue_33_a_hand_written_union_codec_of_another_branch_type_does_not_compile() {
    let items = r#"
#[derive(zerodds_codec::DdsType, Debug)]
#[dds(name = "app::Either", extensibility = "final", codec = "manual")]
pub enum Either {
    #[dds(case = 1)]
    Number(i32),
    #[dds(case = 2)]
    Text(String),
}

impl zerodds_codec::DdsSerialize for Either {
    fn serialize(
        &self,
        out: zerodds_codec::DdsSerializer<'_>,
    ) -> Result<zerodds_codec::Serialized, zerodds_codec::WireError> {
        let union = out.serialize_union::<Self>()?;
        match self {
            Self::Number(_) => union.branch::<zerodds_codec::B0>(&1, &String::from("not a long")),
            Self::Text(text) => union.branch::<zerodds_codec::B1>(&2, text),
        }
    }
}

impl<'de> zerodds_codec::DdsDeserialize<'de> for Either {
    fn deserialize(
        input: zerodds_codec::DdsDeserializer<'_, 'de>,
    ) -> Result<zerodds_codec::Decoded<Self>, zerodds_codec::WireError> {
        let input = input.deserialize_union::<Self>()?;
        let (_, text, finish) = input.branch::<zerodds_codec::B1>()?;
        Ok(finish.with(Self::Text(text)))
    }
}
"#;
    let errors = compile_errors("issue33unionbranch", items);
    assert!(
        errors.contains("mismatched types") && errors.contains("expected `&i32`"),
        "the branch's type is the compiler's error:\n{errors}"
    );
}

/// A hand-written codec over `app::Pose { unsigned long id; string label; }`
/// whose `serialize` body is `body`.
fn pose_with(body: &str) -> String {
    format!(
        r#"
pub struct Pose {{
    pub id: u32,
    pub label: String,
}}

impl zerodds_codec::DdsTypeInfo for Pose {{
    type Members = (u32, (String, ()));
    type Extensibility = zerodds_codec::Final;
    const LAYOUT: zerodds_codec::codec::TypeDesc =
        zerodds_codec::codec::TypeDesc::structure::<Self::Members, zerodds_codec::Final>(
            "app::Pose",
            &[zerodds_codec::codec::MemberDesc::new("id"), zerodds_codec::codec::MemberDesc::new("label")],
        );
}}

impl zerodds_codec::DdsSerialize for Pose {{
    fn serialize(
        &self,
        out: zerodds_codec::DdsSerializer<'_>,
    ) -> Result<zerodds_codec::Serialized, zerodds_codec::WireError> {{
        {body}
    }}
}}

pub fn written() -> Vec<u8> {{
    let mut out = Vec::new();
    zerodds_codec::encode(&Pose {{ id: 1, label: String::new() }}, zerodds_codec::Encapsulation::Xcdr2Le, &mut out)
        .expect("encode");
    out
}}
"#
    )
}

/// The compiler's answer for a crate whose items are `items`.
fn compile_errors(name: &str, items: &str) -> String {
    let (_, output) = cargo_test_generated(
        name,
        "module unused { @final struct U { long a; }; };",
        "",
        items,
        "",
    );
    assert!(
        !output.status.success(),
        "{name} compiled:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// GitHub #33, the codec API's typestate (rc8-rust-newtype.md, "Codec
/// API"): a hand-written `serialize` that leaves a member out, or hands
/// over one more than the description has, does not compile — `end`
/// exists only on the empty member list, `member` only on a non-empty one.
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn issue_33_a_hand_written_codec_missing_or_adding_a_member_does_not_compile() {
    let missing = compile_errors(
        "issue33missing",
        &pose_with("out.serialize_struct::<Self>()?.member(&self.id)?.end()"),
    );
    assert!(
        missing.contains("no method named `end` found"),
        "the missing member is the compiler's error:\n{missing}"
    );
    let extra = compile_errors(
        "issue33extra",
        &pose_with(
            "out.serialize_struct::<Self>()?.member(&self.id)?.member(&self.label)?.member(&self.id)?.end()",
        ),
    );
    assert!(
        extra.contains("no method named `member` found"),
        "the extra member is the compiler's error:\n{extra}"
    );
}

/// GitHub #33, the typestate's other half: a member of another type than
/// the description's does not compile, and neither does a description
/// that names another number of members than its member list
/// (`DdsTypeInfo::CHECK`, evaluated where the type is serialized).
#[test]
#[ignore = "builds a crate around the generated module; runs in integration-codegen-build"]
fn issue_33_a_hand_written_codec_of_another_type_or_description_does_not_compile() {
    let mistyped = compile_errors(
        "issue33mistyped",
        &pose_with("out.serialize_struct::<Self>()?.member(&self.label)?.member(&self.id)?.end()"),
    );
    assert!(
        mistyped.contains("mismatched types"),
        "the mistyped member is the compiler's error:\n{mistyped}"
    );
    let described = compile_errors(
        "issue33described",
        &pose_with("out.serialize_struct::<Self>()?.member(&self.id)?.member(&self.label)?.end()")
            .replace(
                "zerodds_codec::codec::MemberDesc::new(\"label\")]",
                "zerodds_codec::codec::MemberDesc::new(\"label\"), zerodds_codec::codec::MemberDesc::new(\"extra\")]",
            ),
    );
    assert!(
        described.contains("names another number of members"),
        "the description's mismatch is the compiler's error:\n{described}"
    );
}

/// Generate the C column for `idl` (XCDR2 little endian, the encoding
/// `DdsType::encode` writes for a `@final` type), compile it with
/// `runtime/c` and a `main` that holds `sample` and runs `body`, and
/// answer what it printed.
fn run_c_peer(dir: &Path, idl: &str, sample: &[u8], body: &str) -> String {
    let unit = dir.join("peer.idl");
    fs::write(&unit, idl).expect("idl");
    let composed = zerodds_idl_compose::compose_with(
        &unit.to_string_lossy(),
        &zerodds_idl_compose::PathLoader::new(Vec::new()),
        zerodds_idl_compose::ComposeOptions::default(),
    )
    .expect("compose");
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    let generated = zerodds_lang_rules::generate::source(
        &composed.ast,
        zerodds_lang_cst::Language::C,
        zerodds_ast_layout::LayoutKey::data(
            zerodds_ast_layout::Encoding::PlainCdr2,
            zerodds_ast_layout::Endianness::Little,
        ),
    )
    .unwrap_or_else(|why| panic!("generate C: {why:?}"));
    fs::write(
        dir.join("gen.h"),
        format!(
            "#ifndef ZD_GEN_H\n#define ZD_GEN_H\n#include \"zerodds_wire.h\"\n\
             #define ZD_ENDIANNESS ZDW_LITTLE\n{generated}\n#endif\n"
        ),
    )
    .expect("header");
    let octets: Vec<String> = sample.iter().map(|b| format!("0x{b:02x}")).collect();
    fs::write(
        dir.join("main.c"),
        format!(
            "#include <stdio.h>\n#include <string.h>\n#include \"gen.h\"\n\
             static const uint8_t sample[] = {{ {} }};\n\
             int main(void) {{\n{body}    return 0;\n}}\n",
            octets.join(", ")
        ),
    )
    .expect("main");
    let runtime = workspace_root().join("runtime/c");
    let binary = dir.join("peer");
    let compiler = std::env::var("CC").unwrap_or_else(|_| String::from("cc"));
    let mut command = Command::new(&compiler);
    command
        .args(["-std=c99", "-Wall", "-Wno-unused-function"])
        .arg(format!("-I{}", dir.display()))
        .arg(format!("-I{}", runtime.join("include").display()))
        .arg("-o")
        .arg(&binary)
        .arg(dir.join("main.c"));
    for source in [
        "writer.c",
        "reader.c",
        "float128.c",
        "float128_decimal.c",
        "md5.c",
    ] {
        command.arg(runtime.join("src").join(source));
    }
    let compiled = command.output().expect("a C compiler (cc, or CC)");
    assert!(
        compiled.status.success(),
        "the C peer does not compile:\n{}\n{}",
        String::from_utf8_lossy(&compiled.stderr),
        fs::read_to_string(dir.join("gen.h")).unwrap_or_default()
    );
    let ran = Command::new(&binary).output().expect("the C peer starts");
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    String::from_utf8_lossy(&ran.stdout).into_owned()
}
