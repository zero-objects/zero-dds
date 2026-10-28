// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `DdsType` bridge: what the generator emits, and whether it
//! compiles and puts the right octets on the wire.
//!
//! The text assertions here run everywhere. Several tests that build a crate
//! around the generated module are `#[ignore]`d and shell out to cargo,
//! the same shape as `crates/corba-rust/tests/compile_check.rs`: they
//! need a populated registry and a writable target directory, and they
//! take the better part of a minute even with the workspace's own target
//! directory warm. Run them with `--include-ignored`. The external-storage
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

#[test]
fn a_keyed_union_is_explicitly_refused_by_the_default_frontend() {
    let dir = tempfile::tempdir().expect("tempdir");
    let idl = dir.path().join("Command.idl");
    fs::write(
        &idl,
        "@final union Command switch (@key long) { case 0: long value; default: long other; };",
    )
    .expect("write idl");
    let error = Config::new()
        .out_dir(dir.path())
        .emit_cargo_directives(false)
        .dds_type_impls(true)
        .compile(&[&idl])
        .expect_err("the default frontend must report its keyed-discriminator boundary")
        .to_string();
    assert!(
        error.contains("`@key` on a union discriminator is not carried"),
        "{error}"
    );
    assert!(
        error.contains("this setting reports it rather than computing a KeyHash no peer computes"),
        "{error}"
    );

    // Independent negative control: the same union without @key remains a
    // legitimate keyless topic and must not be refused merely for being a union.
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
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
fn a_keyed_type_carries_its_holder_and_the_step_five_branch() {
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
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
fn an_absent_optional_member_is_null_to_a_filter() {
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
fn the_generated_filter_value_of_an_optional_member_is_null_or_the_value() {
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
fn issue_41_a_cross_module_type_is_a_topic_type() {
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
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
/// `docs/codec-runtime-gap-followup.md`'s last section. This case is the
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
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
        /// as a sample: the byte-order axis is what
        /// `docs/OPEN-ITEMS.md` records the trait's defaults as
        /// substituting silently, and an overridden pair is what closes
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
zerodds-dcps = {{ path = "{root}/crates/dcps" }}
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
             include!(concat!(env!(\"OUT_DIR\"), \"/idl.rs\"));\n\n\
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
        // The workspace's own target directory, so the DCPS graph is
        // compiled once for the whole tree and not once per case.
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("cargo invocation");
    assert!(
        output.status.success(),
        "the generated crate does not pass (exit {:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|path| path.join("Cargo.lock").exists())
        .map_or(manifest.clone(), Path::to_path_buf)
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
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
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
/// framing and refused the sentinel; the specification-dialect module still
/// refuses it, and should.
#[test]
#[ignore = "builds a crate around the generated module; run with --include-ignored"]
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
fn an_enumeration_and_a_bitset_are_no_topic_types() {
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
