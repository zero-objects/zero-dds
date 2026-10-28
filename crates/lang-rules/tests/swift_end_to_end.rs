// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Swift, through every layer of the rebuild.
//!
//! The counterpart of `go_end_to_end.rs`, which does the same for Go, and
//! of `c_end_to_end.rs`, which does it for C.
//!
//! Two gates and not one:
//!
//! 1. **Every corpus case renders**, or names the layer that refused. A
//!    case that stopped rendering would otherwise be found by whichever
//!    probe ran next, which is a build directory away from the construct
//!    that caused it.
//! 2. **The generated Swift type-checks**, under `swiftc -typecheck`. A
//!    snapshot says the emitter produced the text it produced last time;
//!    it does not say the text is Swift, that every name in it resolves,
//!    or that a property's type exists. `-typecheck` and not `-parse`:
//!    parsing accepts a file full of names that do not exist, and the
//!    whole point of a types column is that the names do.
//!    [`the_compiler_check_catches_a_deliberate_error`] runs a source
//!    that must fail, so a green result here cannot come from a compiler
//!    that accepts anything.
//!
//! `print_stderr` is allowed here on purpose. A skipped compiler check and
//! a frontend refusal both have to be *said*, and standard error is the
//! channel a passing test does not otherwise use — a silent skip is the
//! failure these tests exist to catch.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;
#[path = "support/resolved.rs"]
mod resolved;

use std::process::Command;
use zerodds_lang_rules::adapter::swift::swift_file;
use zerodds_lang_rules::rule::swift;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Swift out.
fn generate(idl: &str) -> String {
    try_generate(idl, "test.idl").unwrap_or_else(|why| panic!("{why}"))
}

/// The same, naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    // The same step the C, C++, Go and TypeScript columns take: a member
    // whose type is written as a scoped name arrives as an unresolved
    // reference, and a materialiser that asked `resolved()` without this
    // would report "the member type does not evaluate" for every
    // cross-module reference in the corpus.
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &swift::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = swift_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::swift::file(&file))
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_a_public_swift_struct() {
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("public struct Pose {"), "{generated}");
    assert!(generated.contains("public var x: Int32"), "{generated}");
    assert!(generated.contains("public var y: Double"), "{generated}");
    assert!(generated.contains("public var active: Bool"), "{generated}");
}

#[test]
fn a_struct_carries_a_written_public_memberwise_initialiser() {
    // Swift synthesises a memberwise initialiser and gives it `internal`
    // access even when every property is `public` (*The Swift Programming
    // Language*, "Access Control"), so a `public struct` with no written
    // `init` is a type another module can hold and cannot construct.
    let generated = generate("struct Pose { long x; double y; };");
    assert!(
        generated.contains("public init(x: Int32, y: Double) {"),
        "{generated}"
    );
    assert!(generated.contains("self.x = x"), "{generated}");
    assert!(generated.contains("self.y = y"), "{generated}");
}

#[test]
fn a_struct_with_no_members_still_gets_an_initialiser() {
    let generated = generate("struct Empty { };");
    assert!(generated.contains("public init() {}"), "{generated}");
}

#[test]
fn every_idl_primitive_reaches_a_swift_type() {
    // The mapping is total over `PrimitiveKind`; this shows it produces
    // the type each one should, rather than merely producing something.
    let generated = generate(
        "struct P {
           boolean a; octet b; char c; wchar d;
           int8 e; uint8 f; short g; unsigned short h;
           long i; unsigned long j; long long k; unsigned long long l;
           float m; double n;
         };",
    );
    for expected in [
        "public var a: Bool",
        "public var b: UInt8",
        "public var c: UInt8",
        "public var d: UInt16",
        "public var e: Int8",
        "public var f: UInt8",
        "public var g: Int16",
        "public var h: UInt16",
        "public var i: Int32",
        "public var j: UInt32",
        "public var k: Int64",
        "public var l: UInt64",
        "public var m: Float",
        "public var n: Double",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn no_generated_type_has_an_implementation_defined_width() {
    // `Int` and `UInt` are the platform's word size (*The Swift
    // Programming Language*, "The Basics"). A member declared with one
    // would be four octets on a 32-bit target and eight on a 64-bit one,
    // for a wire format that fixes every width.
    let generated = generate(
        "struct P { long a; unsigned long b; long long c; sequence<long> d; long e[2]; };",
    );
    for forbidden in [": Int\n", ": UInt\n", ": [Int]\n", ": [UInt]\n"] {
        assert!(
            !generated.contains(forbidden),
            "{forbidden:?} reached the output\n{generated}"
        );
    }
}

#[test]
fn a_long_double_is_a_string_and_never_a_float80_or_a_double() {
    // `Float80` exists only on x86. Measured, on arm64-apple-macosx:
    // `'Float80' is unavailable: Float80 is not available on target
    // platform.` `Double` is not the fallback either: 53 bits of
    // significand against binary128's 113 loses sixty bits at the
    // assignment, before any codec runs.
    let generated = generate("struct S { long double big; };");
    assert!(generated.contains("public var big: String"), "{generated}");
    assert!(
        !generated.contains("Float80"),
        "Float80 reached the output\n{generated}"
    );
}

#[test]
fn an_optional_member_becomes_an_optional_and_the_others_do_not() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7. A Swift value has no "unset" state
    // of its own, so a `long` that arrived as 0 and one that did not
    // arrive at all would be the same value; `nil` is the one state no
    // written value can take.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(generated.contains("public var b: UInt8?"), "{generated}");
    assert!(generated.contains("public var c: Int32"), "{generated}");
    assert!(!generated.contains("public var c: Int32?"), "{generated}");
}

#[test]
fn an_external_member_is_boxed_in_a_class_because_a_struct_cannot_recurse() {
    // Measured, on Swift 6.3: `struct N { var next: N? }` is rejected
    // with `value type 'N' cannot have a stored property that recursively
    // contains it`. `@external` says the member is stored out of line
    // (DDS-XTypes 1.3 Table 21), and a reference is Swift's only stored
    // form that is.
    let generated = generate("struct Node { long value; @optional @external Node next; };");
    assert!(
        generated.contains("public final class ZdExternal<Value> {"),
        "{generated}"
    );
    assert!(
        generated.contains("public var next: ZdExternal<Node>?"),
        "{generated}"
    );
}

#[test]
fn the_external_box_is_absent_when_nothing_reaches_it() {
    // An unused public class is a name in the output that nothing put
    // there.
    let generated = generate("struct S { long x; };");
    assert!(
        !generated.contains("ZdExternal"),
        "the box was emitted unused\n{generated}"
    );
}

#[test]
fn a_member_named_after_a_swift_keyword_takes_back_quotes_and_keeps_its_name() {
    // Back quotes do not rename. `` `default` `` *is* `default`, so the
    // IDL spelling is the Swift spelling — which is what @autoid(HASH)
    // hashes (DDS-XTypes 1.3 §7.3.1.2.1.1). The Go column has to carry an
    // `idl` struct tag on every field because `type` → `Type_` cannot be
    // read back; this column has nothing to record.
    // `class`, `protocol` and `guard` and not `default` or `switch`:
    // those are IDL keywords too, and the point here is the pair of
    // languages disagreeing about what a keyword is.
    let generated = generate("struct S { long class; long protocol; long guard; };");
    assert!(
        generated.contains("public var `class`: Int32"),
        "{generated}"
    );
    assert!(
        generated.contains("public var `protocol`: Int32"),
        "{generated}"
    );
    assert!(
        generated.contains("public var `guard`: Int32"),
        "{generated}"
    );
    assert!(generated.contains("self.`class` = `class`"), "{generated}");
}

#[test]
fn a_member_named_self_gets_a_second_parameter_name_for_the_body() {
    // A parameter labelled `` `self` `` shadows the implicit `self`
    // inside the initialiser. Measured, on Swift 6.3: `value of type
    // 'Int32' has no member 'default'`, from an initialiser whose first
    // parameter was labelled `` `self` ``.
    let generated = generate("struct S { long self; long other; };");
    assert!(
        generated.contains("public init(`self` zdSelf: Int32, other: Int32)"),
        "{generated}"
    );
    assert!(generated.contains("self.`self` = zdSelf"), "{generated}");
}

#[test]
fn the_escape_cannot_make_two_idl_names_collide() {
    // The Go column refuses this pair: `type` escapes to `Type_` and a
    // member already spelled `type_` exports to the same name, so one of
    // the two would be unreachable. Back quotes do not rename, so the two
    // stay two names and the file is emitted.
    let generated = generate("struct S { long class; long class_; };");
    assert!(
        generated.contains("public var `class`: Int32"),
        "{generated}"
    );
    assert!(
        generated.contains("public var class_: Int32"),
        "{generated}"
    );
}

#[test]
fn a_declared_type_named_after_a_standard_library_type_is_renamed() {
    // Back quotes would not help: `` `Bool` `` is `Bool`, and a
    // declaration of it is found ahead of `Swift.Bool` by every
    // unqualified use in the module. Measured, on Swift 6.3: the file
    // compiles with no diagnostic at all, which is the silent
    // substitution this project is arranged against.
    //
    // `Bool` and not `String` or `Int32`: IDL compares identifiers
    // without regard to case (IDL 4.2 §7.2.3), so `String` and `Int32`
    // collide with the keywords `string` and `int32` and the frontend
    // refuses them before this column sees them. `boolean` is spelled
    // differently from `Bool`, so this one reaches here.
    let generated = generate("struct Bool { long x; }; struct Holder { boolean flag; };");
    assert!(generated.contains("public struct Bool_ {"), "{generated}");
    assert!(generated.contains("IDL: `Bool`."), "{generated}");
    assert!(generated.contains("public var flag: Bool"), "{generated}");
}

#[test]
fn a_sequence_becomes_an_array_and_a_string_a_string() {
    let generated = generate("struct Bag { sequence<long> ids; string name; };");
    assert!(generated.contains("public var ids: [Int32]"), "{generated}");
    assert!(generated.contains("public var name: String"), "{generated}");
}

#[test]
fn an_array_nests_outermost_dimension_first_and_records_its_bound() {
    // Swift has no fixed-size array type, so `[[Int32]]` is all the shape
    // there is and the declared lengths go in the comment above the
    // member. Reversing the nesting would silently transpose the data.
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(
        generated.contains("public var grid: [[Int32]]"),
        "{generated}"
    );
    assert!(
        generated.contains("A fixed-size array of 2 x 3 elements"),
        "{generated}"
    );
}

#[test]
fn a_large_array_is_still_an_array_and_never_a_tuple() {
    // The alternative encodes the length in the type — `(Int32, Int32,
    // Int32)` — and stops being usable: the standard library declares
    // `==` for tuples of arity two to six only, a tuple cannot be
    // subscripted by a value computed at run time, and it cannot be
    // iterated. `long a[1000]` under that scheme is a type of several
    // thousand characters and a codec of a thousand statements.
    let generated = generate("struct S { long a[1000]; };");
    assert!(generated.contains("public var a: [Int32]"), "{generated}");
    assert!(
        generated.contains("A fixed-size array of 1000 elements"),
        "{generated}"
    );
    assert!(
        !generated.contains("(Int32, Int32"),
        "a tuple reached the output\n{generated}"
    );
}

#[test]
fn a_map_becomes_an_array_of_entries_and_never_a_dictionary() {
    // A `map<K,V>` goes on the wire as a sequence of pairs
    // (DDS-XTypes 1.3 §7.4.1.1.4). Swift's `Dictionary` has no order and
    // seeds its hash per process, so one program over one value would
    // produce a different octet stream on each run; and `Dictionary`
    // requires `Key: Hashable`, which a generated struct holding a
    // `Double` is not naturally.
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        generated.contains("public var table: [ZdMapEntry_Long_String]"),
        "{generated}"
    );
    assert!(
        generated.contains("public struct ZdMapEntry_Long_String {"),
        "{generated}"
    );
    assert!(
        !generated.contains("[Int32: String]"),
        "a Dictionary reached the output\n{generated}"
    );
    assert!(
        !generated.contains("Dictionary"),
        "a Dictionary reached the output\n{generated}"
    );
}

#[test]
fn a_typedef_becomes_a_typealias_and_not_a_wrapper_struct() {
    // IDL 4.2 §7.4.1.4.3: an alias "does not define a new type". A
    // wrapper struct would be a second type needing a conversion the IDL
    // does not ask for.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(
        generated.contains("public typealias Distance = Int32"),
        "{generated}"
    );
    assert!(generated.contains("public var d: Distance"), "{generated}");
}

#[test]
fn an_enum_becomes_a_swift_enum_with_its_numbering_written_out() {
    // Swift numbers an unassigned case one above the previous one, and
    // `@value` admits gaps, so a case left to the language would renumber
    // the wire.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(
        generated.contains("public enum Mode: Int32 {"),
        "{generated}"
    );
    assert!(generated.contains("case IDLE = 0"), "{generated}");
    assert!(generated.contains("case ACTIVE = 4"), "{generated}");
    assert!(generated.contains("case FAULT = 5"), "{generated}");
}

#[test]
fn a_union_becomes_a_struct_with_a_discriminator_and_one_optional_per_branch() {
    // Swift's enum with associated values is the shape this looks like
    // and cannot be: an IDL discriminator may take a value that selects
    // no branch (IDL 4.2 §7.4.1.4.4), and DDS-XTypes 1.3 §7.4.1.1.5.2
    // then writes the discriminator and nothing else. A struct has that
    // state; an enum has no case for it.
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(generated.contains("public struct Payload {"), "{generated}");
    assert!(generated.contains("public var _d: Mode"), "{generated}");
    assert!(generated.contains("public var idle: Int32?"), "{generated}");
    assert!(
        generated.contains("public var active: Double?"),
        "{generated}"
    );
    assert!(
        !generated.contains("case idle(Int32)"),
        "an enum with associated values reached the output\n{generated}"
    );
}

/// An `@external` union branch keeps its box inside the branch's
/// optional: `ZdExternal<Inner>?`, the same shape and the same order a
/// structure member takes — the box says where the value is held, the
/// optional says whether `_d` selects the branch.
#[test]
fn an_external_union_branch_keeps_its_box_inside_the_branchs_optional() {
    let generated = generate(
        "struct Inner { long a; };
         union Payload switch (long) {
           case 1: @external Inner nested;
           default: long other;
         };",
    );
    assert!(
        generated.contains("public var nested: ZdExternal<Inner>?"),
        "{generated}"
    );
    assert!(
        !generated.contains("public var nested: Inner?"),
        "the box was dropped on the branch\n{generated}"
    );
}

#[test]
fn a_bitmask_becomes_an_option_set() {
    // `OptionSet` is the standard library's own answer to a set of bit
    // flags: it gives the type set algebra over the bound the IDL
    // declared and keeps the flags inside the type, where the Go column
    // has to emit free constants that could be applied to the wrong
    // bitmask.
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(
        generated.contains("public struct Flags: OptionSet {"),
        "{generated}"
    );
    assert!(
        generated.contains("public let rawValue: UInt16"),
        "{generated}"
    );
    assert!(
        generated.contains("public init(rawValue: UInt16) {"),
        "{generated}"
    );
    assert!(
        generated.contains("public static let READY = Flags(rawValue: 1 << 0)"),
        "{generated}"
    );
    assert!(
        generated.contains("public static let MOVING = Flags(rawValue: 1 << 4)"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_struct_of_holder_typed_properties() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("public struct B {"), "{generated}");
    assert!(generated.contains("public var low: UInt8"), "{generated}");
    assert!(generated.contains("public var high: UInt16"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_struct_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("public struct V {"), "{generated}");
    assert!(
        generated.contains("public var counter: Int32"),
        "{generated}"
    );
    assert!(
        generated.contains("public var secret: String"),
        "{generated}"
    );
}

#[test]
fn a_constant_becomes_a_let_of_its_declared_type() {
    // The type is written out rather than left to inference: an integer
    // literal with no annotation is `Int`, whose width is the platform's.
    let generated = generate("const octet LIMIT = 8;");
    assert!(
        generated.contains("public let LIMIT: UInt8 = 8"),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_struct_of_its_members() {
    // `: Error`, so an operation whose `raises` names it can literally
    // `throw` one — `Swift.Error` has no required members, so this is
    // the whole difference from an ordinary struct.
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(
        generated.contains("public struct Failure: Error {"),
        "{generated}"
    );
    assert!(generated.contains("public var code: Int32"), "{generated}");
    assert!(
        generated.contains("public var reason: String"),
        "{generated}"
    );
}

#[test]
fn an_interface_becomes_a_protocol_of_requirements() {
    // A Swift protocol is a set of method requirements with names, which
    // is exactly what an IDL interface declares.
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void go(in Pose target);
           long status();
         };",
    );
    assert!(
        generated.contains("public protocol Control {"),
        "{generated}"
    );
    assert!(generated.contains("func go(target: Pose)"), "{generated}");
    assert!(generated.contains("func status() -> Int32"), "{generated}");
}

#[test]
fn a_native_declaration_becomes_an_opaque_type_alias() {
    let generated = generate("native Handle;");
    assert!(
        generated.contains("public typealias Handle = Any"),
        "{generated}"
    );
}

#[test]
fn a_module_flattens_onto_the_name_and_keeps_the_idl_spelling_of_each_segment() {
    // Swift's unit of namespacing is the module, which is a build-system
    // target and not something a source file declares. The in-file
    // equivalent is a nested type, and it is rejected: unqualified lookup
    // inside a nested type finds the enclosing type's members before the
    // module's, so a nested `Foo.Baz` silently captures a member meant
    // for the top-level `Baz` — measured, on Swift 6.3, with no
    // diagnostic.
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        generated.contains("public struct common_util_Location {"),
        "{generated}"
    );
    assert!(
        generated.contains("IDL: `Location`."),
        "the IDL name was not recorded\n{generated}"
    );
    assert!(
        !generated.contains("public enum common {"),
        "a namespace enum reached the output\n{generated}"
    );
}

#[test]
fn two_modules_that_flatten_onto_one_name_are_refused_by_name() {
    // `a_b::C` and `a::b_C` both give `a_b_C`. Swift would report
    // `invalid redeclaration of 'a_b_C'` and name neither IDL
    // declaration.
    let error = try_generate(
        "module a_b { struct C { long x; }; };
         module a { struct b_C { long y; }; };",
        "collide.idl",
    )
    .expect_err("the collision was not reported");
    assert!(error.contains("a_b_C"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no Swift equivalent, and the
    // coverage table says so. What is left is the file comment.
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(generated.contains("Generated by ZeroDDS"), "{generated}");
    assert!(!generated.contains("public struct"), "{generated}");
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    let generated = generate("struct First { long a; }; struct Second { long b; };");
    let first = generated.find("public struct First").expect("First");
    let second = generated.find("public struct Second").expect("Second");
    assert!(first < second, "the output reordered the declarations");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_table_claims_for_swift_produces_output() {
    // The table says a rule covers it; this says the rule reaches text.
    let sources = [
        ("struct S { long a; };", "public struct S"),
        ("enum E { A };", "public enum E: Int32"),
        ("typedef long T;", "public typealias T = Int32"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "public struct U",
        ),
        ("bitmask B { X };", "public struct B: OptionSet"),
        ("bitset T { bitfield<3> a; };", "public struct T"),
        ("valuetype V { public long a; };", "public struct V"),
        ("const long N = 1;", "public let N: Int32 = 1"),
        ("exception E2 { long code; };", "public struct E2"),
        ("interface I { void go(); };", "public protocol I"),
        ("native N2;", "public typealias N2 = Any"),
    ];
    for (idl, expected) in sources {
        let generated = generate(idl);
        assert!(
            generated.contains(expected),
            "{idl:?} produced no {expected:?}:\n{generated}"
        );
    }
}

// -- the corpus ----------------------------------------------------------

/// Every corpus case, by name.
fn corpus() -> Vec<(String, std::path::PathBuf)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root).expect("the corpus directory") {
        let entry = entry.expect("a corpus entry");
        let idl = entry.path().join("type.idl");
        if idl.is_file() {
            out.push((entry.file_name().to_string_lossy().into_owned(), idl));
        }
    }
    out.sort();
    out
}

/// How many corpus cases have to reach Swift for this gate to mean
/// anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the Rust column's own corpus gate reports the same
/// ones — so those are counted and printed rather than failed on here.
/// What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it.
const MUST_RENDER: usize = 350;

/// The names the Swift column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too — an exemption nothing checks is an exemption that outlives its
/// cause.
const REFUSED: &[(&str, &str)] = &[];

#[test]
fn every_corpus_case_renders_or_names_its_failure() {
    let cases = corpus();
    assert!(
        cases.len() > 300,
        "the corpus is not where it was: {} cases",
        cases.len()
    );
    let out = std::env::var("ZD_SWIFT_OUT").ok();
    let mut failures = Vec::new();
    let mut frontend = Vec::new();
    let mut unexpectedly_fine = Vec::new();
    let mut rendered = 0_usize;
    for (name, path) in cases {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let expected = REFUSED.iter().find(|(case, _)| *case == name);
        match (try_generate(&source, &path.display().to_string()), expected) {
            (Ok(text), None) => {
                rendered += 1;
                if let Some(dir) = &out {
                    let case = std::path::Path::new(dir).join(&name);
                    std::fs::create_dir_all(&case).expect("the output directory");
                    std::fs::write(case.join("ZdTypes.swift"), text).expect("the output file");
                }
            }
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
            }
            (Err(why), None) => {
                // The stage that refused is what decides whose failure it
                // is. `parse` and `lower` are the frontend's and are the
                // same for every column; `apply` and `materialise` are
                // this one's.
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else {
                    failures.push(format!("{name}: {why}"));
                }
            }
            (Err(_), Some(_)) => {}
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(unexpectedly_fine.is_empty(), "{unexpectedly_fine:#?}");
    if !frontend.is_empty() {
        // Named on standard error rather than swallowed. They are not
        // this column's failures and they are not nothing either.
        eprintln!(
            "{} corpus case(s) the frontend refuses, so Swift never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Swift; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the compiler --------------------------------------------------------

/// One generated file through `swiftc -typecheck`.
///
/// `-typecheck` and not `-parse`: parsing accepts a file full of names
/// that do not exist, and the whole point of a types column is that they
/// do. Type checking resolves every name, checks every property's type,
/// and applies the conformance rules an `OptionSet` and a raw-valued
/// `enum` are subject to.
fn swift_typechecks(source: &str, tag: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "zd-swift-check-{}-{}-{}",
        std::process::id(),
        tag,
        source.len()
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("ZdTypes.swift");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new("swiftc")
        .arg("-typecheck")
        .arg(&file)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Whether `swiftc` is on the path at all.
fn swiftc_available() -> bool {
    Command::new("swiftc")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn the_compiler_check_catches_a_deliberate_error() {
    // Without this, a green result from the two tests below could come
    // from a compiler that accepts anything — or from a harness that
    // never ran one.
    if !swiftc_available() {
        eprintln!("SKIPPED: `swiftc` is not on the path.");
        return;
    }
    let bad = "public struct S { public var x: NoSuchTypeExists }\n";
    let why = swift_typechecks(bad, "deliberate").expect_err("swiftc accepted a bad source");
    assert!(why.contains("NoSuchTypeExists"), "{why}");
    // And a good one passes, so the check is not simply always red.
    swift_typechecks("public struct S { public var x: Int32 }\n", "control")
        .expect("swiftc rejected a good source");
}

/// Every shape above, type-checked.
///
/// One `swiftc` invocation per source and not one for all of them: two
/// generated files declare types of the same name — every source here
/// declares its own `S` — and a single invocation would report a
/// redeclaration that no consumer would ever see.
#[test]
fn the_generated_swift_compiles() {
    if !swiftc_available() {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the compiler is exactly the failure this test
        // exists to catch.
        eprintln!(
            "SKIPPED: `swiftc` is not on the path, so the generated Swift was not \
             type-checked. Set it up or run this on a host that has it."
        );
        return;
    }
    let sources = [
        "struct Pose { long x; double y; boolean active; };",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
        "struct S { @optional octet b; long c; string name; sequence<long> ids; \
         long grid[2][3]; map<long, string> table; };",
        "struct T { long class; long self; long protocol; long init; long guard; };",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
        "valuetype V { public long counter; private string secret; };",
        "const octet LIMIT = 8; const string TAG = \"abc\";",
        "exception Failure { long code; string reason; };",
        "struct Target { long x; }; interface Control { void go(in Target t); long status(); };",
        "exception Overrun { long limit; };\n\
         interface Base { readonly attribute long counter; };\n\
         interface Derived : Base {\n\
         \x20   attribute long memory;\n\
         \x20   long f(in long x) raises (Overrun);\n\
         };",
        "native Handle;",
        "module common { module util { struct Location { long x; }; }; };",
        "typedef long Distance; struct Dist { Distance d; };",
        "struct Node { sequence<Node> children; };",
        "struct Ext { long value; @optional @external Ext next; };",
        "struct Bool { long x; }; struct Holder { boolean flag; };",
    ];
    for (index, idl) in sources.iter().enumerate() {
        let generated = generate(idl);
        if let Err(why) = swift_typechecks(&generated, &format!("shape{index}")) {
            panic!("{idl}\n\n{generated}\n\nswiftc said:\n{why}");
        }
    }
}

/// The whole corpus through `swiftc -typecheck`, one invocation per case.
///
/// `#[ignore]`, because it is one invocation of a compiler per corpus
/// case and each costs about half a second. It is what proves the column
/// rather than samples it, so it is run with `--include-ignored` on a
/// host that has Swift and its result is what
/// `docs/language-columns-status.md` records.
#[test]
#[ignore = "one `swiftc -typecheck` per corpus case; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    assert!(
        swiftc_available(),
        "the explicitly selected swift gate requires its compiler"
    );
    let mut failures = Vec::new();
    let mut checked = 0_usize;
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let text = try_generate(&source, &path.display().to_string())
            .unwrap_or_else(|why| panic!("accepted corpus case {name} failed generation: {why}"));
        checked += 1;
        if let Err(why) = swift_typechecks(&text, &name) {
            failures.push(format!("{name}: {why}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} did not type-check:\n{failures:#?}",
        failures.len()
    );
    eprintln!("{checked} corpus case(s) checked by Swift");
    assert!(
        checked >= MUST_RENDER,
        "only {checked} corpus cases reached the compiler; the gate says nothing \
         below {MUST_RENDER}"
    );
}
