// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to F#, through every layer of the rebuild.
//!
//! The counterpart of `zig_end_to_end.rs`, which does the same for Zig.
//! Two gates, for the same reason that file states them:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//! 2. **The generated F# compiles**, under the real compiler.
//!
//! ## What "compiles" means here
//!
//! `dotnet fsi --exec` is the check: it type-checks and runs the file
//! in one step, and a types-only fragment with no `[<EntryPoint>]` and
//! no side-effecting top-level code simply declares its module and
//! exits 0 — there is no F# analogue of Zig's "declarations are
//! analysed lazily" trap, because F#'s type checker walks every
//! declaration in the file it is given regardless of whether anything
//! calls it (measured, `fsi` from the .NET 9.0.106 SDK: a file holding
//! only `type X = { A: int }` with a field of a type that does not
//! exist reports the error on `--exec`, with nothing referencing `X`).
//!
//! `print_stderr` is allowed here on purpose, as it is in the sibling
//! columns' harnesses — the module documentation of `zig_end_to_end.rs`
//! says why.
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
use std::sync::atomic::{AtomicU32, Ordering};
use zerodds_lang_rules::adapter::fsharp::fsharp_file;
use zerodds_lang_rules::rule::fsharp;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, F# out.
fn generate(idl: &str) -> String {
    try_generate(idl, "test.idl").unwrap_or_else(|why| panic!("{why}"))
}

/// The same, naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &fsharp::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = fsharp_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::fsharp::file(&file))
}

/// The decode direction's `Types.fs`: the declarations, the decoder for
/// `type_name`, and the defaulters it calls — the same three pieces
/// `wireprobe-zerodds-fsharp-gen`'s `source` assembles for `--in`, built
/// independently here so this file's own compiler gate covers the read
/// direction too. The read direction is where every bug this column's
/// bring-up found actually lived: the write direction's own eighteen
/// cases in [`the_generated_fsharp_compiles`] passed on the first
/// attempt every one of those bugs was fixed under, because none of
/// them — the curried-call convention, `for`'s `int`-only counter, the
/// missing early `return`, the bitwise operators' third symbol, the
/// bitmask holder cast — is reachable from an encoder alone.
fn generate_decode(idl: &str, type_name: &str) -> Result<String, String> {
    use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
    use zerodds_lang_rules::default_fsharp::fsharp_defaulters_called_by;
    use zerodds_lang_rules::emit::decode::decoder_from;
    use zerodds_lang_rules::emit::encode::CodecOptions;
    use zerodds_lang_rules::emit::fsharp::Fsharp;

    let ast = resolved::ast(idl, "test.idl")?;
    let root = ast
        .types()
        .find(|(_, node)| node.name == type_name)
        .map(|(id, _)| id)
        .ok_or_else(|| format!("`{type_name}` is not a type in this model"))?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &fsharp::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let items = zerodds_lang_rules::adapter::fsharp::fsharp_items(&ast, &applied)
        .map_err(|error| format!("materialise: {error:?}"))?;
    let key = LayoutKey::in_dialect(
        Dialect::Specification,
        Encoding::PlainCdr2,
        Endianness::Little,
        Purpose::Data,
    );
    let decoders = decoder_from::<Fsharp>(&ast, root, key, CodecOptions::default())
        .map_err(|error| format!("decode: {error}"))?;
    let defaulters = fsharp_defaulters_called_by(&ast, &decoders)
        .map_err(|error| format!("default: {error}"))?;
    let mut file = zerodds_lang_cst::fsharp::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        module: zerodds_lang_cst::common::Ident::new(
            zerodds_lang_rules::emit::fsharp::module_name(),
        ),
        decls: Vec::new(),
    };
    file.decls.extend(items);
    file.decls.extend(defaulters);
    file.decls.extend(decoders);
    Ok(zerodds_lang_render::fsharp::file(&file))
}

/// The generated text with every comment line taken out.
fn code_only(generated: &str) -> String {
    generated
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<&str>>()
        .join("\n")
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_an_fsharp_class() {
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("type Pose() ="), "{generated}");
    assert!(
        generated.contains("member val x : int32 = 0 with get, set"),
        "{generated}"
    );
    assert!(
        generated.contains("member val y : float = 0.0 with get, set"),
        "{generated}"
    );
    assert!(
        generated.contains("member val active : bool = false with get, set"),
        "{generated}"
    );
}

#[test]
fn every_idl_primitive_reaches_an_fsharp_type() {
    let generated = generate(
        "struct P {
           boolean a; octet b; char c; wchar d;
           int8 e; uint8 f; short g; unsigned short h;
           long i; unsigned long j; long long k; unsigned long long l;
           float m; double n; long double o;
         };",
    );
    for expected in [
        "a : bool",
        "b : byte",
        "c : byte",
        "d : uint16",
        "e : sbyte",
        "f : byte",
        "g : int16",
        "h : uint16",
        "i : int32",
        "j : uint32",
        "k : int64",
        "l : uint64",
        "m : float32",
        "n : float",
        "o : string",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn a_long_double_holds_decimal_text_because_dotnet_has_no_binary128() {
    let generated = generate("struct S { long double big; };");
    assert!(generated.contains("big : string"), "{generated}");
}

#[test]
fn an_optional_member_becomes_an_option_and_defaults_to_none() {
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(
        generated.contains("b : byte option = None with get, set"),
        "{generated}"
    );
    assert!(generated.contains("c : int32 = 0"), "{generated}");
    assert!(!generated.contains("c : int32 option"), "{generated}");
}

#[test]
fn an_external_member_stays_inline_because_a_class_is_a_reference_already() {
    // `@external` says the member is stored outside the containing
    // object; an F# class already is a reference, so a self-recursive
    // type needs nothing extra. Found by the compiler and not by any
    // Rust assertion: `next: m_Node option` inside `m_Node` is exactly
    // as finite as a Java or C# field of its own class would be.
    let generated = generate(
        "module m {
           struct Node;
           @mutable struct Node { long value; @optional @external Node next; };
         };",
    );
    assert!(
        generated.contains("next : m_Node option = None with get, set"),
        "{generated}"
    );
}

#[test]
fn a_member_named_after_an_fsharp_keyword_is_escaped_and_keeps_its_idl_name() {
    let generated = generate("struct S { long type; long member; };");
    assert!(generated.contains("type_ : int32"), "{generated}");
    assert!(generated.contains("member_ : int32"), "{generated}");
    assert!(generated.contains("IDL: `type`."), "{generated}");
    assert!(generated.contains("IDL: `member`."), "{generated}");
}

#[test]
fn a_declaration_named_after_a_primitive_is_escaped() {
    let generated = generate("struct byte { long x; };");
    assert!(generated.contains("type byte_() ="), "{generated}");
}

#[test]
fn two_members_that_would_reach_one_fsharp_name_are_refused_by_name() {
    let err = try_generate("struct S { long type; long type_; };", "t.idl").unwrap_err();
    assert!(err.contains("materialise:"), "{err}");
}

#[test]
fn a_sequence_becomes_an_array_and_a_string_stays_a_string() {
    let generated = generate("struct S { sequence<long> ids; string name; };");
    assert!(generated.contains("ids : int32 [] ="), "{generated}");
    assert!(generated.contains("name : string ="), "{generated}");
}

#[test]
fn a_fixed_holds_its_decimal_as_text() {
    let generated = generate("struct S { fixed<9,2> price; };");
    assert!(generated.contains("price : string ="), "{generated}");
}

#[test]
fn an_array_nests_outermost_dimension_first() {
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(generated.contains("grid : int32 [] [] ="), "{generated}");
}

#[test]
fn a_map_becomes_an_array_of_entries_and_never_a_dictionary() {
    let generated = generate("struct S { map<long, string> t; };");
    assert!(
        generated.contains("type ZdMapEntry_long_string() ="),
        "{generated}"
    );
    assert!(generated.contains("Key : int32"), "{generated}");
    assert!(generated.contains("Value : string"), "{generated}");
    assert!(
        generated.contains("t : ZdMapEntry_long_string [] ="),
        "{generated}"
    );
    assert!(!generated.contains("Dictionary"), "{generated}");
}

#[test]
fn a_typedef_becomes_a_type_abbreviation() {
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(generated.contains("type Distance = int32"), "{generated}");
}

#[test]
fn an_enum_holds_its_cases_with_every_value_written_out() {
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(generated.contains("type Mode ="), "{generated}");
    assert!(generated.contains("| IDLE = 0"), "{generated}");
    assert!(generated.contains("| ACTIVE = 4"), "{generated}");
    assert!(generated.contains("| FAULT = 5"), "{generated}");
}

#[test]
fn a_union_becomes_a_class_with_a_discriminator_and_one_field_per_branch() {
    let generated = generate(
        "enum M { A, B };
         union U switch (M) { case A: long a; case B: double b; };",
    );
    assert!(generated.contains("type U() ="), "{generated}");
    assert!(generated.contains("_d : M ="), "{generated}");
    assert!(generated.contains("a : int32 ="), "{generated}");
    assert!(generated.contains("b : float ="), "{generated}");
    assert!(
        !code_only(&generated).contains("| A of"),
        "a discriminated union leaked into the output\n{generated}"
    );
}

#[test]
fn an_external_union_branch_stays_inline_too() {
    // `Sel` and not `M`: IDL compares identifiers without regard to
    // case (§7.2.3), so an `enum M` inside `module m` would be the
    // scope's own name redeclared in its immediate scope, which IDL 4.2
    // §7.5.2 does not admit. The union's branches are what this
    // measures, and the discriminator's spelling is free.
    let generated = generate(
        "module m {
           struct Node { long value; };
           enum Sel { A, B };
           @mutable union U switch (Sel) { case A: @external Node child; case B: long leaf; };
         };",
    );
    assert!(generated.contains("child : m_Node ="), "{generated}");
}

#[test]
fn a_bitmask_becomes_a_type_abbreviation_and_one_constant_per_bit() {
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("type Flags = uint16"), "{generated}");
    // `uint16(1 <<< 0)` and not a bare `1 <<< 0`: F# gives `1` no width
    // until something asks, and `<<<`'s own result takes its left
    // operand's — `crates/lang-rules/src/adapter/fsharp.rs`'s
    // `typed_int` carries the measurement.
    assert!(
        generated.contains("let READY : Flags = uint16(1 <<< 0)"),
        "{generated}"
    );
    assert!(
        generated.contains("let MOVING : Flags = uint16(1 <<< 4)"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_class_of_holder_typed_fields() {
    let generated = generate("bitset T { bitfield<3> a; bitfield<12> b; };");
    assert!(generated.contains("type T() ="), "{generated}");
    // `0uy` and `0us` and not `byte(0)`/`uint16(0)`: a bare literal is
    // `int` before a conversion function sees it, so the suffix is what
    // gives the constant its width — `typed_int`'s documentation carries
    // the measurement.
    assert!(generated.contains("a : byte = 0uy"), "{generated}");
    assert!(generated.contains("b : uint16 = 0us"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_class_of_its_state() {
    let generated = generate("valuetype V { public long a; };");
    assert!(generated.contains("type V() ="), "{generated}");
    assert!(generated.contains("a : int32 = 0"), "{generated}");
}

#[test]
fn a_constant_becomes_a_let_that_ascribes_its_type() {
    let generated = generate("const long N = 1;");
    assert!(generated.contains("let N : int32 = 1"), "{generated}");
}

#[test]
fn an_exception_becomes_a_class_of_its_members() {
    let generated = generate("exception E2 { long code; };");
    assert!(generated.contains("type E2() ="), "{generated}");
    assert!(generated.contains("code : int32 = 0"), "{generated}");
}

#[test]
fn an_interface_becomes_a_class_of_function_valued_fields() {
    let generated = generate("interface I { void ping(); };");
    assert!(generated.contains("type I() ="), "{generated}");
    assert!(generated.contains("ping : unit -> unit"), "{generated}");
}

/// OMG IDL 4.2 §7.4.3: a derived interface inherits the operations of
/// its bases, and this column's class has no base to inherit them
/// through — it is a flat class of function-valued fields. Walking
/// `operations_of`, which answers only what the interface declares,
/// left `Derived` carrying `f` alone: `g` was in the source, in the
/// model and in no generated file, with no diagnostic to say so.
#[test]
fn a_derived_interface_carries_the_operations_of_its_base() {
    let out = generate("interface Base { void g(); }; interface Derived : Base { void f(); };");
    let derived = out
        .split("type Derived()")
        .nth(1)
        .expect("the derived class");
    assert!(derived.contains("member val g :"), "{out}");
    assert!(derived.contains("member val f :"), "{out}");
}

/// An attribute's implied get_/set_ pair reaches the derived class too
/// — round-2 feedback: F# had a working flattened interface before it
/// was silently missing attributes and `raises`. `readonly` writes only
/// the getter field.
#[test]
fn an_attribute_becomes_a_get_set_pair_of_fields_and_raises_is_documented() {
    let out = generate(
        "exception Overrun { long limit; };
         interface Base { readonly attribute long counter; };
         interface Derived : Base {
           attribute long memory;
           long f(in long x) raises (Overrun);
         };",
    );
    let derived = out
        .split("type Derived()")
        .nth(1)
        .expect("the derived class");
    assert!(derived.contains("member val get_counter :"), "{out}");
    assert!(!derived.contains("member val set_counter :"), "{out}");
    assert!(derived.contains("member val get_memory :"), "{out}");
    assert!(derived.contains("member val set_memory :"), "{out}");
    assert!(derived.contains("member val f :"), "{out}");
    assert!(
        out.contains("Raises, as the IDL declares it: Overrun"),
        "the raises clause is not documented: {out}"
    );
}

/// A diamond contributes the shared base's operation once. Twice would
/// be two fields of one name, which `no_spelling_collision` refuses
/// before F# ever sees the file.
#[test]
fn a_diamond_carries_the_shared_bases_operation_once() {
    let out = generate(
        "interface Top { void t(); };
         interface Left : Top { void l(); };
         interface Right : Top { void r(); };
         interface Bottom : Left, Right { void b(); };",
    );
    let bottom = out.split("type Bottom()").nth(1).expect("the bottom class");
    assert_eq!(bottom.matches("member val t :").count(), 1, "{out}");
    for name in ["l", "r", "b"] {
        assert!(bottom.contains(&format!("member val {name} :")), "{out}");
    }
}

#[test]
fn a_native_declaration_becomes_obj() {
    let generated = generate("native N2;");
    assert!(generated.contains("type N2 = obj"), "{generated}");
}

#[test]
fn a_module_flattens_onto_the_name_because_module_rec_does_not_nest_idl_modules() {
    let generated = generate("module m { struct S { long a; }; };");
    assert!(generated.contains("type m_S() ="), "{generated}");
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(!code_only(&generated).contains("Sensitive"), "{generated}");
}

#[test]
fn the_module_line_always_carries_rec() {
    let generated = generate("struct S { long a; };");
    assert!(generated.starts_with("// Generated"), "{generated}");
    assert!(generated.contains("module rec ZdTypes"), "{generated}");
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    // Not `struct A { long a; };`: IDL 4.2 §7.2.3 treats identifiers as
    // colliding when they differ only in case, and a struct's own name
    // collides with a member spelled the same way but lower-case —
    // measured, `zerodds_idl_lower` refuses `struct A { long a; };`
    // with `DuplicateName`.
    let generated = generate("struct First { long x; }; struct Second { long y; };");
    let at_first = generated.find("type First()").expect("First in the output");
    let at_second = generated
        .find("type Second()")
        .expect("Second in the output");
    assert!(at_first < at_second, "{generated}");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct S { long a; sequence<string> b; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_catalogue_covers_produces_output() {
    let sources = [
        ("struct S { long a; };", "type S()"),
        ("enum E { A };", "type E ="),
        ("typedef long T;", "type T = int32"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "type U()",
        ),
        ("bitmask B { X };", "type B = uint32"),
        ("bitset T { bitfield<3> a; };", "type T()"),
        ("valuetype V { public long a; };", "type V()"),
        ("const long N = 1;", "let N : int32 = 1"),
        ("exception E2 { long code; };", "type E2()"),
        ("interface I { void ping(); };", "type I()"),
        ("native N2;", "type N2 = obj"),
        ("module m { struct S { long a; }; };", "type m_S()"),
        ("struct S { map<long,long> t; };", "type ZdMapEntry_"),
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

/// How many corpus cases have to reach F# for this gate to mean
/// anything. The corpus is 378 cases; `zig_end_to_end.rs`'s own
/// `MUST_RENDER` is 350 and this column's own materialiser refuses
/// nothing more than Zig's does, so the same floor applies.
const MUST_RENDER: usize = 350;

/// The names this column itself refuses, each with the reason. Empty:
/// this column refuses nothing in the corpus by name.
const REFUSED: &[(&str, &str)] = &[];

#[test]
fn every_corpus_case_renders_or_names_its_failure() {
    let cases = corpus();
    assert!(
        cases.len() > 300,
        "the corpus is not where it was: {} cases",
        cases.len()
    );
    let out = std::env::var("ZD_FSHARP_OUT").ok();
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
                    std::fs::write(case.join("Types.fs"), text).expect("the output file");
                }
            }
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
            }
            (Err(why), None) => {
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
        eprintln!(
            "{} corpus case(s) the frontend refuses, so F# never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached F#; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the compiler --------------------------------------------------------

/// A counter, so two checks running at once do not share a directory.
static SERIAL: AtomicU32 = AtomicU32::new(0);

fn scratch() -> std::path::PathBuf {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("zd-fsharp-check-{}-{serial}", std::process::id()))
}

/// One generated file through the real compiler: `dotnet fsi --exec`,
/// with `ZdWire.fs` `#load`ed beside it so a fragment that names a
/// runtime type — none of the tests above do, but the corpus-wide
/// check might reach one that would — resolves.
fn fsharp_checks(source: &str) -> Result<(), String> {
    let dir = scratch();
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let runtime =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/fsharp/ZdWire.fs");
    std::fs::copy(&runtime, dir.join("ZdWire.fs")).map_err(|error| error.to_string())?;
    // `Types.fs`, a real source file `#load`ed from a script, not the
    // script's own inline content: measured (`dotnet fsi` from the
    // .NET 9.0.106 SDK), a top-level `module Name` with no `=` is
    // refused as `error FS0010` when it is the *script's own* content
    // — `dotnet fsi --exec` on a bare `module ZdTypes\ntype Pose = …`
    // fails at the first declaration under it — and compiles cleanly
    // the moment the same text is a separate file the script
    // `#load`s instead. This is exactly the shape
    // `tools/wireprobe/wireprobe-zerodds-fsharp` already uses:
    // `Types.fs` loaded, `Main.fsx` the script.
    std::fs::write(dir.join("Types.fs"), source).map_err(|error| error.to_string())?;
    let main = dir.join("Main.fsx");
    std::fs::write(&main, "#load \"ZdWire.fs\"\n#load \"Types.fs\"\n")
        .map_err(|error| error.to_string())?;
    let output = Command::new("dotnet")
        .arg("fsi")
        .arg("--exec")
        .arg(&main)
        .current_dir(&dir)
        .output()
        .map_err(|error| error.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "dotnet fsi --exec: {}",
        String::from_utf8_lossy(&output.stderr)
    ))
}

/// Whether `dotnet fsi` is usable at all.
fn fsharp_available() -> bool {
    Command::new("dotnet")
        .arg("fsi")
        .arg("--help")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
#[ignore = "one `dotnet fsi` start per case: 65s measured for a single test, over 2min for a set; run with --include-ignored"]
fn the_fsharp_check_reports_a_type_error_with_nothing_referencing_it() {
    // The check this gate depends on is not vacuous: unlike Zig's own
    // `build-obj`, `dotnet fsi --exec` type-checks a declaration that
    // nothing in the file calls.
    assert!(
        fsharp_available(),
        "the selected F# gate requires dotnet fsi"
    );
    let broken = "module rec ZdTypes\ntype X() =\n    member val A : NoSuchType = Unchecked.defaultof<NoSuchType> with get, set\n";
    let refused = fsharp_checks(broken).expect_err("an unresolved type name passed the check");
    assert!(
        refused.contains("error FS"),
        "the check refused for the wrong reason: {refused}"
    );
}

/// Every shape above, through the compiler.
#[test]
#[ignore = "one `dotnet fsi` start per case: 65s measured for a single test, over 2min for a set; run with --include-ignored"]
fn the_generated_fsharp_compiles() {
    assert!(
        fsharp_available(),
        "the requested F# gate needs `dotnet fsi`"
    );
    let sources = [
        "struct Pose { long x; double y; boolean active; };",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
        "struct S { @optional octet b; long c; string name; wstring wide; \
         sequence<long> ids; long grid[2][3]; map<long, string> table; fixed<9,2> price; };",
        "struct T { long type; long member; long open; };",
        "typedef long ByteAlias; typedef long TypeAlias;",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
        "@bit_bound(64) bitmask Wide { @position(63) TOP };",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
        "valuetype V { public long counter; private string secret; };",
        "const octet LIMIT = 8; const string TAG = \"abc\"; const long double BIG = 1.5; \
         const float NARROW = .5; const fixed PRICE = 1.25d; const boolean ON = TRUE;",
        "exception Failure { long code; string reason; };",
        "struct Target { long x; }; interface Control { void ping(in Target t); long status(); };",
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
        "@annotation Sensitive { string reason; };",
    ];
    for idl in sources {
        let generated = generate(idl);
        if let Err(why) = fsharp_checks(&generated) {
            panic!("{idl}\n\n{generated}\n\n{why}");
        }
    }
}

/// Every shape above, through the compiler, in the **read** direction —
/// decoders and defaulters, not encoders. [`generate_decode`]'s own
/// documentation says why this is a separate gate from
/// [`the_generated_fsharp_compiles`] and not redundant with it.
///
/// Split into three functions of a handful of cases each — not one with
/// all eleven — so the default test harness runs them as concurrent
/// threads instead of one `dotnet fsi` invocation after another in a
/// single process. Measured: decode-direction fragments (decoders plus
/// their defaulters) type-check markedly slower per case than the
/// encode-direction ones in [`the_generated_fsharp_compiles`], and
/// eleven of them run serially in one process outran this host's
/// tolerance for a single long-lived child even though no individual
/// `dotnet fsi` call hangs — the per-file budget in the project's own
/// conventions ("Einzeltest ≤25s, Lauf ≤5min") is the same rule.
fn run_decode_cases(sources: &[(&str, &str)]) {
    assert!(
        fsharp_available(),
        "the requested F# gate needs `dotnet fsi`"
    );
    for (idl, type_name) in sources {
        let generated = generate_decode(idl, type_name).unwrap_or_else(|why| panic!("{why}"));
        if let Err(why) = fsharp_checks(&generated) {
            panic!("{idl}\n\n{generated}\n\n{why}");
        }
    }
}

#[test]
#[ignore = "one `dotnet fsi` start per case: 65s measured for a single test, over 2min for a set; run with --include-ignored"]
fn the_generated_fsharp_decodes_primitives_and_collections() {
    run_decode_cases(&[
        ("struct Pose { long x; double y; boolean active; };", "Pose"),
        (
            "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
             unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
             float m; double n; long double o; };",
            "P",
        ),
        (
            "struct S { @optional octet b; long c; string name; wstring wide; \
             sequence<long> ids; long grid[2][3]; map<long, string> table; fixed<9,2> price; };",
            "S",
        ),
        (
            "struct Inner { long v; }; struct Outer { sequence<Inner> items; \
             map<long, Inner> table; };",
            "Outer",
        ),
    ]);
}

#[test]
#[ignore = "one `dotnet fsi` start per case: 65s measured for a single test, over 2min for a set; run with --include-ignored"]
fn the_generated_fsharp_decodes_enums_and_masks() {
    run_decode_cases(&[
        (
            "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
             union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; \
             default: string reason; };",
            "Payload",
        ),
        (
            "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
            "Flags",
        ),
        ("@bit_bound(64) bitmask Wide { @position(63) TOP };", "Wide"),
        (
            "bitset Bits { bitfield<3> low; bitfield<12> high; };",
            "Bits",
        ),
    ]);
}

#[test]
#[ignore = "one `dotnet fsi` start per case: 65s measured for a single test, over 2min for a set; run with --include-ignored"]
fn the_generated_fsharp_decodes_classes_and_recursion() {
    run_decode_cases(&[
        (
            "valuetype V { public long counter; private string secret; };",
            "V",
        ),
        // No exception here, and not by omission: `decoder_from` takes an
        // `Id<TypeNode>`, and `idl-lower` gives an exception a node of its
        // own (`AnyId::Exception`), so `ast.types()` never answers with one.
        // The declaration side does carry it —
        // `the_generated_fsharp_compiles` renders the same source, and every
        // other column asserts the same shape (zig_end_to_end.rs::
        // an_exception_becomes_a_struct_of_its_members). Whether an
        // exception needs a codec of its own, or reaches the wire only
        // inside the reply type DDS-RPC 1.0 §7.5.1.1.5 builds for it, is a
        // question about the RPC mapping and not about this column.
        ("struct Node { sequence<Node> children; };", "Node"),
    ]);
}

/// The whole corpus through the compiler, one file per case.
#[test]
#[ignore = "hundreds of `dotnet fsi` invocations; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    assert!(
        fsharp_available(),
        "the requested F# compiler gate needs `dotnet fsi`"
    );
    let cases = corpus();
    let total = cases.len();
    let mut failures = Vec::new();
    let mut backend_refused = Vec::new();
    let mut frontend_refused = 0_usize;
    let mut checked = 0_usize;
    for (name, path) in cases {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            frontend_refused += 1;
            continue;
        }
        let text = match try_generate(&source, &path.display().to_string()) {
            Ok(text) => text,
            Err(why) => {
                eprintln!("BACKEND_REFUSED {name}: {why}");
                backend_refused.push(format!("{name}: {why}"));
                continue;
            }
        };
        checked += 1;
        if let Err(why) = fsharp_checks(&text) {
            eprintln!("COMPILER_FAILED {name}: {why}");
            failures.push(format!("{name}: {why}"));
        }
    }
    eprintln!(
        "CORPUS_SUMMARY total={total} frontend_refused={frontend_refused} backend_refused={} reached={checked} passed={} failed={}",
        backend_refused.len(),
        checked - failures.len(),
        failures.len(),
    );
    assert_eq!(total, frontend_refused + backend_refused.len() + checked);
    assert!(checked >= MUST_RENDER, "only {checked} cases reached F#");
    assert!(backend_refused.is_empty(), "{backend_refused:#?}");
    assert!(
        failures.is_empty(),
        "{} of the corpus did not compile:\n{failures:#?}",
        failures.len()
    );
}

/// The PL_CDR1 PID-flag axis, measured in F#.
///
/// `runtime/fsharp/ZdWire.fs` wrote `PID_LIST_END | 0x4000` as a
/// literal and never set `PID_IMPL_EXTENSION`, so the F# column put
/// Cyclone's `7f02` on the wire under every dialect — 76 recorded
/// cells against `fastdds` and `opendds` under XCDR1 differed from the
/// peer's bytes for that reason alone.
///
/// The three answers below are the recorded vendor framings for a
/// one-member `@mutable` sample, `@id(1) long a;` holding 7, little
/// endian, and they are what
/// `crates/wire-probe/tests/pid_flag_positions.rs` derives from
/// `tests/interop-results/sources/vendor-corpus.tsv` on the Rust side:
///
/// ```text
///     Cyclone, RTI   member 0100   sentinel 7f02
///     Fast DDS       member 0100   sentinel 3f02
///     OpenDDS        member 01c0   sentinel 3f02
/// ```
///
/// Both directions of the counter-check are in that table. Fast DDS and
/// RTI agree at the member and differ at the sentinel, so the
/// sentinel's answer is not a function of the member's; Fast DDS and
/// OpenDDS differ at the member and agree at the sentinel, so the
/// member's is not a function of the sentinel's. A framing that carried
/// one bool could not hold both.
///
/// # Mutation-checked
///
/// `WriteSentinel` asking `pidFlagsAt framing.PidFlags MemberId`
/// instead of `Sentinel` — the position mutation, not the value one —
/// turns this red: `bare` then writes `3f02` where the policy is
/// `sentinel-must-understand` and `7f02` where it is not. Measured, and
/// `dotnet fsi --exec` exits 1.
#[test]
#[ignore = "one `dotnet fsi` start per case: 65s measured for a single test, over 2min for a set; run with --include-ignored"]
fn the_pid_flag_policy_answers_at_each_position() {
    assert!(
        fsharp_available(),
        "the selected F# gate requires dotnet fsi"
    );
    // The assertions are in the F# so that what is checked is what the
    // runtime wrote, and not a Rust re-derivation of it: `failwithf`
    // leaves `dotnet fsi --exec` with a non-zero status, which is what
    // `fsharp_checks` reads.
    let source = "\
module ZdPidFlagCheck

/// `@mutable struct { @id(1) long a; }` holding 7, written under
/// `policy`, as hexadecimal.
let private written (policy: ZdWire.PidFlagPolicy) : string =
    let framing = { ZdWire.specificationFraming with PidFlags = policy }
    let w = ZdWire.Writer(ZdWire.LE, ZdWire.XCDR1, framing, false)
    let header = w.WritePid(1, ZdWire.Member, false)
    w.WriteI32(7)
    w.FinishPid(header)
    w.WriteSentinel()
    w.Finish() |> Array.map (sprintf \"%02x\") |> String.concat \"\"

let private expect (policy: ZdWire.PidFlagPolicy) (want: string) =
    let got = written policy
    if got <> want then
        failwithf \"%s: expected %s, wrote %s\" (ZdWire.pidFlagPolicyName policy) want got

expect ZdWire.SentinelMustUnderstand \"0100040007000000027f0000\"
expect ZdWire.Bare \"0100040007000000023f0000\"
expect ZdWire.ImplExtensionOnMembers \"01c0040007000000023f0000\"
";
    fsharp_checks(source).expect("the three PID flag policies wrote their recorded framings");
}

/// The XCDR2 EMHEADER length code, measured in F#.
///
/// `runtime/fsharp/ZdWire.fs` declared `ShortCodeWhereItFits` on its
/// `PlCdr1Framing` and read it nowhere, so `FinishEmheader` kept every
/// `LC4` header it opened and patched its NEXTINT. Sixteen recorded
/// XCDR2 cells against `opendds` and `fastdds` differed from the peer's
/// bytes for that reason alone — `struct-mutable-length-codes` under
/// `opendds-xcdr2-le` was 176 octets against the peer's 168, eight more
/// for the two NEXTINTs that should have been dropped.
///
/// XTypes 1.3 §7.4.3.4.2, Table 34: `LC0` through `LC3` *are* the
/// lengths 1, 2, 4 and 8 and carry no NEXTINT; `LC4` is followed by an
/// explicit NEXTINT; `LC5` through `LC7` reuse the member's own leading
/// word, which the clause says "shall be reused also as part of the
/// serialized member". Several codes are therefore legal for the same
/// member and the peer's choice is the one to reproduce:
/// `LengthCodePolicy::short_code_where_it_fits` is true for Fast DDS and
/// OpenDDS and false for Cyclone, RTI and the specification dialect.
///
/// The plan answers this wherever the member's *type* fixes the size;
/// the writer answers the rest, because `string s = "abc"` is eight
/// octets only because of the sample. That second half is what the
/// eight rows below pin.
///
/// # Mutation-checked
///
/// Forcing `LC4` where `LC3` applies — `FinishEmheader` patching the
/// NEXTINT unconditionally instead of calling `ShortenEmheader`, which
/// is the state this test was written against — turns it red on the
/// first `shortCodes = true` row: `01000030 0102030405060708` becomes
/// `01000040 08000000 0102030405060708`. Measured, and `dotnet fsi
/// --exec` exits 1.
///
/// The id-and-flag row is the second half of the counter-check: raising
/// the code with `PatchU32(word, code <<< 28)` instead of masking only
/// bits 28-30 writes `00000020` where `ffffffaf` belongs, discarding
/// both the member id and `M_FLAG`.
#[test]
#[ignore = "one `dotnet fsi` start per case: 65s measured for a single test, over 2min for a set; run with --include-ignored"]
fn a_length_code_shortens_only_where_the_member_fits_one() {
    assert!(
        fsharp_available(),
        "the selected F# gate requires dotnet fsi"
    );
    // The assertions are in the F# so that what is checked is what the
    // runtime wrote, and not a Rust re-derivation of it.
    let source = "\
module ZdLengthCodeCheck

/// One `@mutable` member of `octets` octets, opened as `LC4` and closed
/// under `shortCodes`, as hexadecimal. The body is written octet by
/// octet so its length is exactly `octets` and nothing aligns inside it.
let private written
    (endian: ZdWire.Endian)
    (shortCodes: bool)
    (memberId: int)
    (mustUnderstand: bool)
    (octets: int)
    : string =
    let w = ZdWire.Writer(endian, ZdWire.XCDR2, ZdWire.specificationFraming, shortCodes)
    let h = w.WriteEmheader(memberId, ZdWire.LC4, mustUnderstand)
    for i in 1..octets do
        w.WriteU8(byte i)
    w.FinishEmheader(h)
    w.Finish() |> Array.map (sprintf \"%02x\") |> String.concat \"\"

let private expect (what: string) (got: string) (want: string) =
    if got <> want then
        failwithf \"%s: expected %s, wrote %s\" what want got

// LC0 to LC3 where the member fits one, LC4 where it does not.
expect \"one octet\" (written ZdWire.LE true 1 false 1) \"0100000001\"
expect \"two octets\" (written ZdWire.LE true 1 false 2) \"010000100102\"
expect \"four octets\" (written ZdWire.LE true 1 false 4) \"0100002001020304\"
expect \"eight octets\" (written ZdWire.LE true 1 false 8) \"010000300102030405060708\"
expect \"three octets\" (written ZdWire.LE true 1 false 3) \"0100004003000000010203\"

// The same member under a policy that does not shorten — Cyclone, RTI
// and the specification dialect. Eight octets is LC4 with a NEXTINT.
expect \"eight octets, unshortened\" (written ZdWire.LE false 1 false 8) \"01000040080000000102030405060708\"

// Raising the code keeps the other 29 bits: id 0x0FFFFFFF and M_FLAG.
expect \"id and must-understand survive\" (written ZdWire.LE true 0x0FFFFFFF true 4) \"ffffffaf01020304\"

// Big endian reads the header word back the way it wrote it.
expect \"eight octets, big endian\" (written ZdWire.BE true 1 false 8) \"300000010102030405060708\"
";
    fsharp_checks(source).expect("the length code shortened exactly where the member fits one");
}
