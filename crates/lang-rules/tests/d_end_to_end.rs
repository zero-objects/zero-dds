// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to D, through every layer of the rebuild.
//!
//! The counterpart of `go_end_to_end.rs`, which does the same for Go,
//! and of `c_end_to_end.rs`, which does it for C.
//!
//! Two gates and not one:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//!    A case that stopped rendering would otherwise be found by whichever
//!    probe ran next, which is a build directory away from the construct
//!    that caused it.
//! 2. **The generated D compiles**, under `ldc2 -o-` — parse, semantic
//!    analysis and type check, with no object file written. A snapshot
//!    says the emitter produced the text it produced last time; it does
//!    not say the text is D, that every name in it resolves, or that a
//!    field's type exists. The compiler is skipped where none is on the
//!    path and the skip says so on standard error — a test that went
//!    green without running it would be the failure it is meant to
//!    catch.
//!
//! Two of the assertions below exist because of a specific way this
//! column could be wrong, and they name it:
//!
//! - [`an_idl_long_is_a_d_int_and_an_idl_long_long_is_a_d_long`]. Both
//!   languages have a type called `long` and they are different widths.
//! - [`an_array_reverses_its_dimensions_because_d_reads_them_right_to_left`].
//!   `int[3][2]` is two arrays of three in D and three arrays of two in
//!   C.
// `print_stderr` is `warn` in the workspace lint table and `-D warnings`
// promotes it to an error, so the skip messages below would fail the
// clippy gate. They are the point of two of these tests: a compiler that
// is not on the path has to say so on the channel a passing test does
// not use, and a test that went green in silence would be the failure it
// exists to catch.
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
use zerodds_lang_rules::adapter::d::d_file;
use zerodds_lang_rules::rule::d;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, D out.
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
    let applied = apply(&model, &d::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = d_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::d::file(&file))
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_file_declares_one_module_and_flattens_the_idl_modules_into_it() {
    // D's correspondence between a module and a file is one-to-one, so
    // the nested IDL modules of one file cannot each be a D module here.
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(generated.contains("module zdtypes;"), "{generated}");
    assert!(
        generated.contains("struct common_util_Location"),
        "{generated}"
    );
    assert!(
        generated.contains("IDL: `Location`."),
        "the IDL name was not recorded\n{generated}"
    );
}

#[test]
fn a_struct_of_primitives_becomes_a_d_struct_and_never_a_class() {
    // A D `class` is a reference type with a vtable pointer and a
    // monitor field on the GC heap; the wire form of an IDL aggregate is
    // a value.
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("struct Pose"), "{generated}");
    assert!(!generated.contains("class Pose"), "{generated}");
    assert!(generated.contains("@(\"idl:x\") int x;"), "{generated}");
    assert!(generated.contains("@(\"idl:y\") double y;"), "{generated}");
    assert!(
        generated.contains("@(\"idl:active\") bool active;"),
        "{generated}"
    );
}

#[test]
fn an_idl_long_is_a_d_int_and_an_idl_long_long_is_a_d_long() {
    // The single most likely defect in this column. IDL 4.2 §7.4.1.4.4.2
    // makes an IDL `long` 32 bits and an IDL `long long` 64; the D
    // language specification makes a D `int` 32 and a D `long` 64. A
    // mapping that followed the spelling would double the width of every
    // 32-bit member — a struct that compiles, encodes four extra octets
    // per member, and is read by no peer.
    let generated =
        generate("struct W { long a; unsigned long b; long long c; unsigned long long d; };");
    assert!(generated.contains("int a;"), "{generated}");
    assert!(generated.contains("uint b;"), "{generated}");
    assert!(generated.contains("long c;"), "{generated}");
    assert!(generated.contains("ulong d;"), "{generated}");
    // And the wrong way round, stated as a refusal rather than left to
    // the reader of the four lines above.
    assert!(
        !generated.contains("long a;"),
        "an IDL `long` became a D `long`\n{generated}"
    );
    assert!(
        !generated.contains("int c;"),
        "an IDL `long long` became a D `int`\n{generated}"
    );
}

#[test]
fn every_idl_primitive_reaches_a_d_type() {
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
        "bool a;",
        "ubyte b;",
        "ubyte c;",
        "ushort d;",
        "byte e;",
        "ubyte f;",
        "short g;",
        "ushort h;",
        "int i;",
        "uint j;",
        "long k;",
        "ulong l;",
        "float m;",
        "double n;",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn an_idl_character_becomes_an_integer_and_never_a_d_char() {
    // D's `char` is one octet *of a UTF-8 sequence* and its `wchar` one
    // UTF-16 code unit. An IDL `char` is an octet and an IDL `wchar` a
    // 16-bit code unit with no encoding attached: an arbitrary octet
    // assigned to a D `char` is accepted and then produces an invalid
    // UTF-8 sequence the moment anything in Phobos reads it.
    let generated = generate("struct C { char a; wchar b; };");
    assert!(generated.contains("ubyte a;"), "{generated}");
    assert!(generated.contains("ushort b;"), "{generated}");
    assert!(!generated.contains("char a;"), "{generated}");
    assert!(!generated.contains("wchar b;"), "{generated}");
}

#[test]
fn a_long_double_is_a_string_and_never_a_real() {
    // D's `real` is 80-bit x87 extended on x86 and a plain `double` — 53
    // bits of significand — on AArch64. An IDL `long double` is a
    // binary128 with 113. `real` would lose a different number of bits
    // on each target, which is the reason the C column refuses the
    // compiler's `long double` as well.
    let generated = generate("struct S { long double big; };");
    assert!(generated.contains("string big;"), "{generated}");
    assert!(
        !generated.contains("real"),
        "`real` reached the output\n{generated}"
    );
}

#[test]
fn no_generated_type_is_target_dependent_or_unimplemented() {
    // `real` is the target's largest float; `cent` and `ucent` are
    // reserved and implemented by no D compiler.
    let generated =
        generate("struct P { long a; long double b; sequence<long> c; long d[2]; string e; };");
    for forbidden in ["real ", "cent ", "ucent ", "creal", "ireal"] {
        assert!(
            !generated.contains(forbidden),
            "{forbidden:?} reached the output\n{generated}"
        );
    }
}

#[test]
fn a_sequence_becomes_a_dynamic_array_and_a_string_a_string() {
    let generated = generate("struct Bag { sequence<long> ids; string name; wstring label; };");
    assert!(generated.contains("int[] ids;"), "{generated}");
    assert!(generated.contains("string name;"), "{generated}");
    // A `wstring` is a D `string` holding UTF-8, not a D `wstring`:
    // D's `wstring` is `immutable(wchar)[]` — UTF-16 — and holding
    // UTF-16 would put the DDS-XTypes 1.3 §7.4.1.1.2 surrogate
    // arithmetic in this column instead of in the runtime.
    assert!(generated.contains("string label;"), "{generated}");
    assert!(!generated.contains("wstring label;"), "{generated}");
}

#[test]
fn an_array_reverses_its_dimensions_because_d_reads_them_right_to_left() {
    // The second most likely defect in this column. `long grid[2][3]` is
    // two rows of three (IDL 4.2 §7.4.1.4.4.4). In D `int[3][2]` is two
    // arrays of three — the suffixes read right to left — where C's
    // `int a[3][2]` is three arrays of two. Emitting `int[2][3]` here
    // would transpose every matrix in the corpus and compile.
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(generated.contains("int[3][2] grid;"), "{generated}");
    assert!(
        !generated.contains("int[2][3] grid;"),
        "the dimensions were written in the IDL order\n{generated}"
    );
}

#[test]
fn a_one_dimensional_array_keeps_its_length() {
    let generated = generate("struct S { long row[4]; };");
    assert!(generated.contains("int[4] row;"), "{generated}");
}

#[test]
fn a_sequence_of_arrays_and_an_array_of_sequences_stay_apart() {
    // `int[3][]` is a dynamic array of `int[3]`; `int[][2]` is two
    // dynamic arrays. Both read right to left.
    let generated = generate("typedef long Row[3]; struct S { sequence<Row> rows; };");
    assert!(generated.contains("alias Row = int[3];"), "{generated}");
    assert!(generated.contains("Row[] rows;"), "{generated}");
}

#[test]
fn a_map_becomes_a_slice_of_entries_and_never_an_associative_array() {
    // A D associative array has no defined iteration order, and a
    // `map<K,V>` goes on the wire as a sequence of pairs
    // (DDS-XTypes 1.3 §7.4.1.1.4). A `V[K]` would give a different octet
    // stream for one value depending on the table's insertion history.
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        generated.contains("ZdMapEntry_Long_String[] table;"),
        "{generated}"
    );
    assert!(
        generated.contains("struct ZdMapEntry_Long_String"),
        "{generated}"
    );
    assert!(
        !generated.contains("string[int]"),
        "an associative array reached the output\n{generated}"
    );
}

#[test]
fn an_optional_member_becomes_a_nullable_and_the_others_do_not() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7. `Nullable!(T)` and not `T*`: the
    // member keeps its own storage whatever `T` is, and the absent state
    // is `isNull` rather than a dereference that is undefined when it is
    // wrong. The argument is parenthesised because `Nullable!int[]`
    // parses as `(Nullable!int)[]`.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(generated.contains("Nullable!(ubyte) b;"), "{generated}");
    assert!(generated.contains("int c;"), "{generated}");
    assert!(!generated.contains("Nullable!(int) c;"), "{generated}");
    assert!(
        generated.contains("import std.typecons : Nullable;"),
        "the import was not written\n{generated}"
    );
    assert!(!generated.contains("ubyte* b;"), "{generated}");
}

#[test]
fn a_file_with_no_optional_member_imports_nothing() {
    // An import in a file with no optional member would make every
    // consumer link Phobos for a name it does not use.
    let generated = generate("struct S { long a; };");
    assert!(!generated.contains("import"), "{generated}");
}

#[test]
fn an_optional_sequence_parenthesises_its_template_argument() {
    // `Nullable!int[]` is `(Nullable!int)[]` — an array of optional
    // ints, which is not what `@optional sequence<long>` says.
    let generated = generate("struct S { @optional sequence<long> ids; };");
    assert!(generated.contains("Nullable!(int[]) ids;"), "{generated}");
}

#[test]
fn an_external_member_becomes_a_pointer_whatever_its_type_is() {
    // `@external` says the member is stored outside the containing
    // object (DDS-XTypes 1.3 Table 21). It changes no octet on the wire
    // and is a statement about where the binding keeps the value; in D a
    // pointer is the whole of it.
    //
    // Honoured on every `@external` member and not only where a cycle
    // forces it. Four of the six corpus cases that carry the annotation
    // — a primitive, a `sequence`, a `string`, an aggregate — would
    // compile perfectly well without the indirection, and a mapping that
    // produced a pointer only for the recursive one would put the same
    // IDL member in two shapes depending on what else the file declared.
    let generated = generate(
        "struct Inner { long a; };
         struct S {
           @external long a;
           @external sequence<long> ids;
           @external string text;
           @external Inner nested;
           long tail;
         };",
    );
    assert!(generated.contains("int* a;"), "{generated}");
    assert!(generated.contains("int[]* ids;"), "{generated}");
    assert!(generated.contains("string* text;"), "{generated}");
    assert!(generated.contains("Inner* nested;"), "{generated}");
    // The member with no annotation keeps its own storage.
    assert!(generated.contains("int tail;"), "{generated}");
    assert!(!generated.contains("int* tail;"), "{generated}");
}

#[test]
fn an_optional_external_member_is_one_pointer_and_not_a_nullable_pointer() {
    // Measured on `tests/interop-corpus/recursive-optional-external`,
    // which declares `@optional @external Node next` inside `Node`. Held
    // inline as `Nullable!(m_Node)` the struct contains itself and
    // LDC 1.40.0 refuses it — "unable to determine fields of `m_Node`
    // because of forward references", raised from `core.internal.traits`
    // instantiating `Nullable!(m_Node)`, because `Nullable!T` stores its
    // `T` by value. The optional alone does not break the cycle; the
    // pointer does.
    //
    // One pointer and not `Nullable!(Node*)`: a pointer already carries
    // a state no written value can take. Two would admit
    // present-but-`null`, which the wire form has no encoding for.
    let generated = generate(
        "module m {
           struct Node;
           @mutable struct Node {
             long value;
             @optional @external Node next;
           };
         };",
    );
    assert!(generated.contains("m_Node* next;"), "{generated}");
    assert!(
        !generated.contains("Nullable!(m_Node)"),
        "the member is held inline and the struct contains itself\n{generated}"
    );
    assert!(
        !generated.contains("Nullable!(m_Node*)"),
        "a second absence state the wire form cannot encode\n{generated}"
    );
    // Nothing else in the file is optional, so nothing needs the import.
    assert!(
        !generated.contains("import std.typecons"),
        "an import was written for a name the file does not use\n{generated}"
    );
}

/// Whether a member's type stores `declared` inside the struct itself.
///
/// A type holds its target by value once `Nullable!(…)` is unwrapped and
/// every static-array suffix is dropped: `Node`, `Nullable!(Node)` and
/// `Node[3]` all put a whole `Node` in the enclosing object, and
/// `Node[]` and `Node*` do not — a dynamic array is a pointer and a
/// length, and a pointer is a pointer.
fn holds_by_value(type_text: &str, declared: &str) -> bool {
    let mut rest = type_text;
    while let Some(inner) = rest
        .strip_prefix("Nullable!(")
        .and_then(|inner| inner.strip_suffix(')'))
    {
        rest = inner;
    }
    // Drop `[N]` suffixes; stop at `[]`, which is the finite one.
    while rest.ends_with(']') {
        let Some(open) = rest.rfind('[') else { break };
        if rest[open + 1..rest.len() - 1].is_empty() {
            return false;
        }
        rest = &rest[..open];
    }
    rest == declared
}

/// The type text of one rendered field line, if it is a field at all.
///
/// A field is `[UDA ]Type name;` — `@("idl:x") Node* next;` or, for the
/// discriminator, `Mode _d;`. No type this renderer writes contains a
/// space, so the name is whatever follows the last one.
fn field_type(line: &str) -> Option<&str> {
    let line = line.trim().strip_suffix(';')?;
    let after_uda = line.rsplit_once("\") ").map_or(line, |(_, rest)| rest);
    after_uda.rsplit_once(' ').map(|(type_, _)| type_)
}

#[test]
fn no_generated_struct_holds_itself_by_value_anywhere_in_the_corpus() {
    // The shape LDC refuses. A struct that stores itself has no size,
    // and the error names a Phobos template rather than the member —
    // `unable to determine fields of m_Node because of forward
    // references`, from `core.internal.traits` instantiating
    // `Nullable!(m_Node)`. That is a long way from the IDL that caused
    // it, which is why the shape is looked for here rather than left to
    // the compile run on the Linux host.
    //
    // Scoped to the declaring struct's own body: `Nullable!(m_Inner)`
    // inside `m_S` is an optional member and perfectly finite, and a
    // check that only asked whether the file contained the text reported
    // `optional-nested-struct` and was wrong.
    let mut offenders = Vec::new();
    let mut examined = 0_usize;
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let Ok(text) = try_generate(&source, &path.display().to_string()) else {
            continue;
        };
        let mut declared: Option<&str> = None;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("struct ") {
                declared = Some(rest.trim());
                continue;
            }
            if line == "}" {
                declared = None;
                continue;
            }
            let (Some(declared), Some(type_)) = (declared, field_type(line)) else {
                continue;
            };
            examined += 1;
            if holds_by_value(type_, declared) {
                offenders.push(format!("{name}: {declared} stores itself as `{type_}`"));
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
    // A field scanner that matched nothing would report no offender and
    // say nothing at all. The corpus declares thousands of members; this
    // is the number that stops the check going quietly vacuous if the
    // rendered shape of a field ever changes under it.
    assert!(
        examined > 1000,
        "the field scanner found only {examined} members, so it is not reading the output"
    );
}

#[test]
fn a_typedef_becomes_an_alias() {
    // IDL 4.2 §7.4.1.4.3: an alias "does not define a new type", which
    // is exactly what `alias Distance = int;` says.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(generated.contains("alias Distance = int;"), "{generated}");
    assert!(generated.contains("Distance d;"), "{generated}");
}

#[test]
fn an_enum_is_one_declaration_with_its_base_and_every_value_written_out() {
    // A D `enum` is a named scope, so the enumerators need no module
    // prefix and no constants of their own — the difference from the C
    // and Go columns. The values are written out because D numbers an
    // unvalued member one above its predecessor and `@value` admits
    // gaps, so leaving one unvalued would renumber the wire.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(generated.contains("enum Mode : int"), "{generated}");
    assert!(generated.contains("IDLE = 0,"), "{generated}");
    assert!(generated.contains("ACTIVE = 4,"), "{generated}");
    assert!(generated.contains("FAULT = 5"), "{generated}");
}

#[test]
fn a_union_becomes_a_struct_with_a_discriminator_and_an_anonymous_union() {
    // The branches overlay, which is the shape the wire form has: only
    // the branch `_d` selects is written (DDS-XTypes 1.3 §7.4.1.1.5.2).
    // This is the C column's answer; the Go column puts the branches
    // side by side only because Go has no overlaid storage at all.
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(generated.contains("struct Payload"), "{generated}");
    assert!(generated.contains("Mode _d;"), "{generated}");
    assert!(generated.contains("union"), "{generated}");
    assert!(
        generated.contains("@(\"idl:idle\") int idle;"),
        "{generated}"
    );
    assert!(
        generated.contains("@(\"idl:active\") double active;"),
        "{generated}"
    );
}

/// An `@external` union branch is the same `T*` a structure's
/// `@external` member is: the branch declaration routes through the
/// same `member_field`, and the mirror in `idl-lower` is what lets it
/// see the annotation at all.
#[test]
fn an_external_union_branch_becomes_a_pointer() {
    let generated = generate(
        "struct Inner { long a; };
         union Payload switch (long) {
           case 1: @external Inner nested;
           default: long other;
         };",
    );
    assert!(
        generated.contains("@(\"idl:nested\") Inner* nested;"),
        "{generated}"
    );
    assert!(
        !generated.contains("@(\"idl:nested\") Inner nested;"),
        "the pointer was dropped on the branch\n{generated}"
    );
}

#[test]
fn a_bitmask_becomes_an_alias_and_one_manifest_constant_per_bit() {
    // An alias and not a D `enum`: a bitmask value is any combination of
    // its bits (DDS-XTypes 1.3 §7.4.1.1.3.2), and a D enum is a closed
    // set of named values — `READY | MOVING` would be a value of no
    // member.
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("alias Flags = ushort;"), "{generated}");
    assert!(
        generated.contains("enum Flags READY = 1 << 0;"),
        "{generated}"
    );
    assert!(
        generated.contains("enum Flags MOVING = 1 << 4;"),
        "{generated}"
    );
}

#[test]
fn a_bit_above_thirty_takes_a_ulong_literal_because_an_int_shift_would_not_compile() {
    // `1` is an `int` in D and a shift count outside 0..31 on an `int`
    // is rejected at compile time. At position 31 the `int` result would
    // already be the sign bit.
    let generated = generate("@bit_bound(64) bitmask Wide { @position(63) TOP };");
    assert!(generated.contains("alias Wide = ulong;"), "{generated}");
    assert!(
        generated.contains("enum Wide TOP = 1UL << 63;"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_struct_of_holder_typed_fields() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("struct B"), "{generated}");
    assert!(generated.contains("ubyte low;"), "{generated}");
    assert!(generated.contains("ushort high;"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_struct_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("struct V"), "{generated}");
    assert!(generated.contains("int counter;"), "{generated}");
    assert!(generated.contains("string secret;"), "{generated}");
}

#[test]
fn a_constant_becomes_a_manifest_constant_of_its_declared_type() {
    // `enum` and not `immutable`: a manifest constant has no storage and
    // no address, where an `immutable` at module scope is a variable
    // with both, placed by the linker in every binary that imports it.
    let generated = generate("const octet LIMIT = 8; const string TAG = \"abc\";");
    assert!(generated.contains("enum ubyte LIMIT = 8;"), "{generated}");
    assert!(
        generated.contains("enum string TAG = \"abc\";"),
        "{generated}"
    );
    assert!(!generated.contains("immutable"), "{generated}");
}

#[test]
fn an_exception_becomes_a_struct_of_its_members() {
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("struct Failure"), "{generated}");
    assert!(generated.contains("int code;"), "{generated}");
    assert!(generated.contains("string reason;"), "{generated}");
}

#[test]
fn an_interface_becomes_a_d_interface_of_signatures() {
    // A D interface declares signatures and nothing else, which is what
    // an IDL interface declares.
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void go(in Pose target);
           long status();
         };",
    );
    assert!(generated.contains("interface Control"), "{generated}");
    assert!(generated.contains("void go(Pose target);"), "{generated}");
    assert!(generated.contains("int status();"), "{generated}");
}

#[test]
fn a_native_declaration_becomes_an_opaque_pointer() {
    // `void*` and not `Object`: an IDL `native` names a type the binding
    // supplies, and D's `Object` would add a vtable and a GC root to
    // something whose contents the IDL does not describe.
    let generated = generate("native Handle;");
    assert!(generated.contains("alias Handle = void*;"), "{generated}");
}

#[test]
fn a_member_named_after_a_d_keyword_is_escaped_and_keeps_its_idl_name() {
    // D has no raw-identifier syntax — nothing like Rust's `r#type` or
    // C#'s `@class` — so the trailing underscore is the whole mechanism,
    // and the IDL spelling goes in the UDA because
    // DDS-XTypes 1.3 §7.3.1.2.1.1 computes a member's ID from it.
    let generated = generate("struct S { long version; long scope; long align; };");
    assert!(
        generated.contains("@(\"idl:version\") int version_;"),
        "{generated}"
    );
    assert!(
        generated.contains("@(\"idl:scope\") int scope_;"),
        "{generated}"
    );
    assert!(
        generated.contains("@(\"idl:align\") int align_;"),
        "{generated}"
    );
}

#[test]
fn a_member_named_after_a_d_type_keyword_is_escaped_too() {
    // `int`, `real`, `ubyte` and `immutable` are D keywords and none of
    // the four is an IDL keyword, so all four can be written in a source
    // and all four have to move. `string` and `union` are D keywords too
    // and are not tested here: IDL 4.2 §7.4.1.4.4 makes both IDL
    // keywords as well, so neither can appear as a member name in a
    // source at all. Their escape is covered where it can be reached, in
    // `zerodds_lang_cst::d`'s own tests.
    let generated = generate("struct S { long int; long real; long ubyte; long immutable; };");
    assert!(generated.contains("int int_;"), "{generated}");
    assert!(generated.contains("int real_;"), "{generated}");
    assert!(generated.contains("int ubyte_;"), "{generated}");
    assert!(generated.contains("int immutable_;"), "{generated}");
}

#[test]
fn a_member_named_after_the_generated_import_is_escaped() {
    // `import std.typecons : Nullable;` puts both names at module scope.
    let generated = generate("struct S { long Nullable; long std; };");
    assert!(generated.contains("int Nullable_;"), "{generated}");
    assert!(generated.contains("int std_;"), "{generated}");
}

#[test]
fn two_members_that_would_reach_one_d_name_are_refused_by_name() {
    // Not the case-only pair — IDL 4.2 §7.2.3.1 already forbids `value`
    // beside `Value` in one scope, and the frontend reports it. What is
    // left is the pair the keyword escape makes: `version` escapes to
    // `version_`, and a member already spelled `version_` reaches the
    // same name. D would reject the duplicate, naming an identifier and
    // not the two IDL members behind it.
    let error = try_generate("struct S { long version; long version_; };", "collide.idl")
        .expect_err("the collision was not reported");
    assert!(error.contains("version_"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn the_discriminator_is_named_where_no_idl_member_can_reach_it() {
    // IDL 4.2 §7.4.1.4.4.1 gives the discriminator no member name of its
    // own, so `_d` is this column's — as it is in every column but Go's,
    // which cannot export a name beginning with an underscore. The
    // anonymous union puts its fields directly on the enclosing struct,
    // so a branch and `_d` share one scope and are checked against each
    // other; IDL 4.2 §7.2.3 requires an identifier to begin with an
    // ASCII letter, so a branch cannot be spelled `_d` in the first
    // place and the two never meet.
    let generated = generate("enum M { A }; union U switch (M) { case A: long d; };");
    assert!(generated.contains("M _d;"), "{generated}");
    assert!(generated.contains("@(\"idl:d\") int d;"), "{generated}");
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no D equivalent. What is left is
    // the module declaration and the file comment.
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(generated.contains("module zdtypes;"), "{generated}");
    assert!(!generated.contains("struct "), "{generated}");
}

#[test]
fn a_member_whose_type_is_any_is_refused_by_name() {
    // Building Block Any is one of IDL 4.2's fifteen and what it holds
    // is decided at run time. D's `Variant` would compile and there
    // would be no wire form to generate a codec against.
    let error =
        try_generate("struct S { any payload; };", "any.idl").expect_err("`any` was not refused");
    assert!(error.contains("any"), "{error}");
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    // D makes every module-scope declaration visible throughout the
    // module regardless of where it appears (D language specification,
    // "Modules", "Declaration Order"), so unlike the C column there is
    // no dependency walk and the output tracks the source. A struct
    // naming a type declared below it still compiles.
    let generated = generate("struct First { Second s; }; struct Second { long b; };");
    let first = generated.find("struct First").expect("First");
    let second = generated.find("struct Second").expect("Second");
    assert!(first < second, "the output reordered the declarations");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_catalogue_covers_produces_output() {
    // The catalogue says a rule covers it; this says the rule reaches
    // text.
    let sources = [
        ("struct S { long a; };", "struct S"),
        ("enum E { A };", "enum E : int"),
        ("typedef long T;", "alias T = int;"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "struct U",
        ),
        ("bitmask B { X };", "alias B = uint;"),
        ("bitset T { bitfield<3> a; };", "struct T"),
        ("valuetype V { public long a; };", "struct V"),
        ("const long N = 1;", "enum int N = 1;"),
        ("exception E2 { long code; };", "struct E2"),
        ("interface I { void go(); };", "interface I"),
        ("native N2;", "alias N2 = void*;"),
        (
            "struct M2 { map<long, long> t; };",
            "struct ZdMapEntry_Long_Long",
        ),
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

/// How many corpus cases have to reach D for this gate to mean anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the Rust column's own corpus gate reports the same
/// ones — so those are counted and printed rather than failed on here.
/// What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it. The corpus grows;
/// the floor is the Go column's, so a case count that fell to it would
/// fail both gates at once rather than leaving one of them green.
const MUST_RENDER: usize = 350;

/// The names the D column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too — an exemption nothing checks is an exemption that outlives its
/// cause.
const REFUSED: &[(&str, &str)] = &[(
    "recursive-array",
    "a struct holding itself by value through an array, which no D compiler \
     accepts and IDL 4.2 §7.4.1.4.4.4.4 does not admit",
)];

#[test]
fn every_corpus_case_renders_or_names_its_failure() {
    let cases = corpus();
    assert!(
        cases.len() > 300,
        "the corpus is not where it was: {} cases",
        cases.len()
    );
    let out = std::env::var("ZD_D_OUT").ok();
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
                    std::fs::write(case.join("zdtypes.d"), text).expect("the output file");
                }
            }
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
            }
            (Err(why), None) => {
                // The stage that refused is what decides whose failure
                // it is. `parse` and `lower` are the frontend's and are
                // the same for every column; `apply` and `materialise`
                // are this one's.
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
            "{} corpus case(s) the frontend refuses, so D never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached D; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the compiler --------------------------------------------------------

/// The D compilers this test will use, in order of preference.
///
/// `ldc2` first: it is the LLVM-based reference this column targets, and
/// `-o-` is its "analyse and write nothing" switch. `gdc` and `dmd` are
/// tried after it so that a host with either still checks the syntax.
const COMPILERS: &[&str] = &["ldc2", "gdc", "dmd"];

/// The arguments each compiler takes to type-check without emitting.
///
/// - `ldc2 -o- file.d` — "do not write object file"; the front end still
///   runs parse, semantic analysis and type check.
/// - `dmd -o- file.d` — the same switch, same meaning.
/// - `gdc -fsyntax-only file.d` — GCC's own spelling of it.
fn check_args(compiler: &str) -> &'static [&'static str] {
    match compiler {
        "gdc" => &["-fsyntax-only"],
        _ => &["-o-"],
    }
}

/// The first D compiler on the path, if there is one.
fn d_compiler() -> Option<&'static str> {
    COMPILERS.iter().copied().find(|compiler| {
        Command::new(compiler)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    })
}

/// One generated file through the D front end.
fn d_compiles(compiler: &str, source: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "zd-d-compile-{}-{}",
        std::process::id(),
        source.len()
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    // Named after the module it declares. D's correspondence between a
    // module and a file is one-to-one, and a file whose name disagrees
    // with its module declaration is one no importer can find.
    let file = dir.join("zdtypes.d");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new(compiler)
        .args(check_args(compiler))
        .arg(&file)
        .current_dir(&dir)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        let mut why = String::from_utf8_lossy(&output.stderr).into_owned();
        why.push_str(&String::from_utf8_lossy(&output.stdout));
        Err(why)
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// The sources whose generated D is the sample this column stands on.
///
/// One list, used by the compile test and by `ZD_D_SAMPLES`, so the
/// files written out for a host with a compiler are the same ones the
/// test would have checked here.
const SAMPLES: &[(&str, &str)] = &[
    ("pose", "struct Pose { long x; double y; boolean active; };"),
    (
        "primitives",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
    ),
    (
        "collections",
        "struct S { @optional octet b; long c; string name; wstring label; \
         sequence<long> ids; long grid[2][3]; map<long, string> table; \
         @optional sequence<long> maybe; };",
    ),
    (
        "keywords",
        // Every name here is a D keyword or a module-scope name a
        // generated file relies on, and none of them is an IDL keyword —
        // `string` and `union` are both, so neither can be written in an
        // IDL source at all.
        "struct T { long version; long scope; long align; long immutable; long real; \
         long Nullable; long std; long init; };",
    ),
    (
        "union",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; \
         default: string other; };",
    ),
    (
        "bitmask",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };
         @bit_bound(64) bitmask Wide { @position(63) TOP };",
    ),
    (
        "bitset",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
    ),
    (
        "valuetype",
        "valuetype V { public long counter; private string secret; };",
    ),
    (
        "constants",
        "const octet LIMIT = 8; const string TAG = \"abc\"; const double RATE = 1.5;",
    ),
    (
        "exception",
        "exception Failure { long code; string reason; };",
    ),
    (
        "interface",
        "struct Target { long x; }; interface Control { void go(in Target t); long status(); };",
    ),
    (
        "interface_attributes_and_raises",
        "exception Overrun { long limit; };
         interface Base { readonly attribute long counter; };
         interface Derived : Base {
           attribute long memory;
           long f(in long x) raises (Overrun);
         };",
    ),
    ("native", "native Handle;"),
    (
        "modules",
        "module common { module util { struct Location { long x; }; }; };",
    ),
    (
        "typedefs",
        "typedef long Distance; typedef long Row[3]; struct Dist { Distance d; Row r; };",
    ),
    ("recursive", "struct Node { sequence<Node> children; };"),
    (
        // The shape LDC 1.40.0 refused when the member was held inline:
        // `Nullable!T` stores its `T` by value, so `Nullable!(m_Node)`
        // inside `m_Node` is a struct containing itself.
        "recursive-external",
        "module m {
           struct Node;
           @mutable struct Node { long value; @optional @external Node next; };
         };",
    ),
    (
        "external",
        "struct Inner { long a; short b; };
         @final struct Sample {
           @external long a;
           @external sequence<long> ids;
           @external string text;
           @external Inner nested;
           long tail;
         };",
    ),
    (
        "forward",
        "struct First { Second s; }; struct Second { long b; };",
    ),
];

/// Every shape above, compiled.
///
/// One invocation per sample rather than one file, because two samples
/// may declare the same name and D allows a name once per module.
#[test]
fn the_generated_d_compiles() {
    // Generated first and unconditionally. A sample source that stopped
    // parsing would otherwise be invisible on a host with no D compiler:
    // the skip below would return before anything ran the generator over
    // it, and the sample the Linux host is asked to compile would be a
    // file this test never produced. Found that way — `long string;` is
    // not an IDL member declaration, and it sat in the list until the
    // sample writer was asked for it.
    let generated: Vec<(&str, &str, String)> = SAMPLES
        .iter()
        .map(|(name, idl)| (*name, *idl, generate(idl)))
        .collect();

    // Written out whether or not a compiler is here, so that a host that
    // has one can check exactly what this test would have checked.
    if let Ok(dir) = std::env::var("ZD_D_SAMPLES") {
        for (name, idl, text) in &generated {
            let case = std::path::Path::new(&dir).join(name);
            std::fs::create_dir_all(&case).expect("the sample directory");
            std::fs::write(case.join("zdtypes.d"), text).expect("the sample file");
            std::fs::write(case.join("type.idl"), idl).expect("the sample source");
        }
    }
    let Some(compiler) = d_compiler() else {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the compiler is exactly the failure this test
        // exists to catch.
        eprintln!(
            "SKIPPED: no D compiler on the path — `ldc2`, `gdc` and `dmd` were all looked \
             for — so the generated D was not compiled. Set ZD_D_SAMPLES to write the \
             sample files out and run this on a host that has one."
        );
        return;
    };
    for (name, idl, text) in &generated {
        if let Err(why) = d_compiles(compiler, text) {
            panic!("{name}: {idl}\n\n{text}\n\n{compiler} said:\n{why}");
        }
    }
}

/// The whole corpus through the D front end, one module per case.
///
/// `#[ignore]`, because it is one invocation of a compiler per corpus
/// case and takes minutes. It is what proves the column rather than
/// samples it, so it is run with `--include-ignored` on a host that has
/// a D compiler.
#[test]
#[ignore = "one D front-end invocation per corpus case; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    let compiler = d_compiler().expect("the explicitly selected compiler must be installed");
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
        if let Err(why) = d_compiles(compiler, &text) {
            failures.push(format!("{name}: {why}"));
        }
    }
    eprintln!("{checked} corpus case(s) reached d_end_to_end");
    assert!(
        failures.is_empty(),
        "{} of the corpus did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(
        checked >= MUST_RENDER,
        "too few corpus cases reached the selected gate: {checked}"
    );
}
