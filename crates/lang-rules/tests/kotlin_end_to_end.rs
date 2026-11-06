// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Kotlin, through every layer of the rebuild.
//!
//! The counterpart of `zig_end_to_end.rs` and `java_end_to_end.rs`, for
//! the Kotlin column.
//!
//! Two gates and not one:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//! 2. **The generated Kotlin compiles**, under the real compiler
//!    (`kotlinc`), alongside `runtime/kotlin/ZdWire.kt`. A snapshot says
//!    the emitter produced the text it produced last time; it does not
//!    say the text is Kotlin, that every name in it resolves, or that a
//!    property's type exists.
//!
//! `print_stderr` is allowed here on purpose, as it is in the sibling
//! columns' harnesses: a skipped compiler check and a corpus case the
//! frontend refused both have to be said out loud on a channel a passing
//! test does not use.
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
use zerodds_lang_rules::adapter::kotlin::kotlin_file;
use zerodds_lang_rules::rule::kotlin;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Kotlin out.
fn generate(idl: &str) -> String {
    try_generate(idl, "test.idl").unwrap_or_else(|why| panic!("{why}"))
}

/// The same, naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &kotlin::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = kotlin_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::kotlin::file(&file))
}

/// The generated text with every comment line taken out.
fn code_only(generated: &str) -> String {
    generated
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<&str>>()
        .join("\n")
}

#[test]
fn xtypes_7_2_2_2_a_struct_of_primitives_becomes_a_kotlin_data_class() {
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("data class Pose("), "{generated}");
    assert!(generated.contains("var x: Int"), "{generated}");
    assert!(generated.contains("var y: Double"), "{generated}");
    assert!(generated.contains("var active: Boolean"), "{generated}");
}

#[test]
fn xtypes_7_2_2_2_every_idl_primitive_reaches_a_kotlin_type() {
    let generated = generate(
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
    );
    for (field, type_) in [
        ("a", "Boolean"),
        ("b", "Int"),
        ("c", "Char"),
        ("d", "Int"),
        ("e", "Byte"),
        ("f", "Int"),
        ("g", "Short"),
        ("h", "Int"),
        ("i", "Int"),
        ("j", "Long"),
        ("k", "Long"),
        ("l", "Long"),
        ("m", "Float"),
        ("n", "Double"),
        ("o", "Float128"),
    ] {
        let needle = format!("var {field}: {type_}");
        assert!(generated.contains(&needle), "{needle}\n{generated}");
    }
}

#[test]
fn xtypes_7_2_2_2_a_long_double_is_a_float128_because_kotlin_has_no_binary128() {
    let generated = generate("struct S { long double v; };");
    assert!(generated.contains("var v: Float128"), "{generated}");
}

#[test]
fn xtypes_7_2_2_4_4_4_7_an_optional_member_becomes_a_nullable_type_and_defaults_to_null() {
    let generated = generate("struct S { @optional long a; long b; };");
    assert!(generated.contains("var a: Int? = null"), "{generated}");
    assert!(!generated.contains("var b: Int? "), "{generated}");
}

#[test]
fn xtypes_7_2_2_4_4_4_7_an_external_member_needs_no_indirection() {
    // Every JVM object field is already a reference, so `@external`
    // changes nothing this materialiser writes.
    let generated = generate("struct Node { @optional @external Node next; long v; };");
    assert!(generated.contains("var next: Node? = null"), "{generated}");
}

#[test]
fn idl_7_2_3_a_member_named_after_a_hard_keyword_is_escaped_and_keeps_its_idl_name() {
    let generated = generate("struct S { long class; long fun; long when; };");
    assert!(generated.contains("var class_: Int"), "{generated}");
    assert!(generated.contains("var fun_: Int"), "{generated}");
    assert!(generated.contains("var when_: Int"), "{generated}");
    assert!(generated.contains("IDL: `class`."), "{generated}");
}

#[test]
fn idl_7_2_3_a_member_named_after_a_soft_keyword_keeps_the_idl_spelling() {
    let generated = generate("struct S { long data; long value; };");
    assert!(generated.contains("var data: Int"), "{generated}");
    assert!(generated.contains("var value: Int"), "{generated}");
}

#[test]
fn idl_7_2_3_two_members_that_would_reach_one_kotlin_name_are_refused_by_name() {
    let error = try_generate("struct S { long class; long class_; };", "t.idl").unwrap_err();
    assert!(error.contains("materialise"), "{error}");
}

#[test]
fn a_sequence_becomes_an_array_and_a_string_stays_a_string() {
    let generated = generate("struct S { sequence<long> a; string b; };");
    assert!(generated.contains("var a: IntArray"), "{generated}");
    assert!(generated.contains("var b: String"), "{generated}");
}

#[test]
fn xtypes_7_2_2_4_3_a_sequence_of_an_aggregate_is_a_boxed_array() {
    let generated = generate("struct Item { long v; }; struct S { sequence<Item> items; };");
    assert!(generated.contains("var items: Array<Item>"), "{generated}");
}

#[test]
fn empty_sequences_do_not_construct_an_element() {
    for (idl, expected) in [
        (
            "struct S { sequence<sequence<long> > items; };",
            "var items: Array<IntArray> = arrayOf<IntArray>()",
        ),
        (
            "typedef long Distance; struct S { sequence<Distance> items; };",
            "var items: Array<Distance> = arrayOf<Distance>()",
        ),
        (
            "struct S { sequence<long double> items; };",
            "var items: Array<Float128> = arrayOf<Float128>()",
        ),
        (
            "struct S { sequence<map<long, long> > items; };",
            "var items: Array<Array<ZdEntry_long_long>> = arrayOf<Array<ZdEntry_long_long>>()",
        ),
        (
            "typedef long Triple[3]; struct S { sequence<Triple> items; };",
            "var items: Array<Triple> = arrayOf<Triple>()",
        ),
    ] {
        let generated = generate(idl);
        assert!(generated.contains(expected), "{expected}\n{generated}");
    }
}

#[test]
fn xtypes_7_2_2_2_a_fixed_holds_its_decimal_as_a_string() {
    let generated = generate("struct S { fixed<9,2> price; };");
    assert!(generated.contains("var price: String"), "{generated}");
}

#[test]
fn an_array_nests_outermost_dimension_first() {
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(
        generated.contains("var grid: Array<IntArray>"),
        "{generated}"
    );
}

#[test]
fn xtypes_7_2_2_4_3_a_map_becomes_an_array_of_a_generated_entry_class() {
    let generated = generate("struct S { map<long, string> t; };");
    assert!(
        generated.contains("class ZdEntry_long_string("),
        "{generated}"
    );
    assert!(
        generated.contains("var t: Array<ZdEntry_long_string>"),
        "{generated}"
    );
}

#[test]
fn a_typedef_becomes_a_type_alias() {
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(
        generated.contains("typealias Distance = Int"),
        "{generated}"
    );
    assert!(generated.contains("var d: Distance"), "{generated}");
}

#[test]
fn xtypes_7_2_2_4_1_1_an_enum_becomes_an_object_of_constants_with_every_value_written_out() {
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(generated.contains("object Mode {"), "{generated}");
    assert!(generated.contains("const val IDLE: Int = 0"), "{generated}");
    assert!(
        generated.contains("const val ACTIVE: Int = 4"),
        "{generated}"
    );
    assert!(
        generated.contains("const val FAULT: Int = 5"),
        "{generated}"
    );
    // Not the code, only the doc comment explaining the choice: this is
    // a substring check against the declarations themselves.
    assert!(!code_only(&generated).contains("enum class"), "{generated}");
}

#[test]
fn xtypes_7_2_2_4_1_1_an_enum_typed_member_is_a_plain_int() {
    let generated = generate("enum Mode { IDLE, ACTIVE }; struct S { Mode m; };");
    assert!(generated.contains("var m: Int"), "{generated}");
}

#[test]
fn a_union_becomes_a_data_class_with_a_discriminator_and_one_property_per_branch() {
    let generated =
        generate("enum M { A, B }; union U switch (M) { case A: long a; case B: double b; };");
    let code = code_only(&generated);
    assert!(code.contains("data class U("), "{code}");
    assert!(code.contains("var _d: Int"), "{code}");
    assert!(code.contains("var a: Int"), "{code}");
    assert!(code.contains("var b: Double"), "{code}");
}

#[test]
fn xtypes_7_2_2_4_1_2_a_bitmask_becomes_no_declaration_of_its_own_and_one_const_val_per_bit() {
    // No `typealias`: the module documentation explains why one would
    // add nothing but a name `crate::emit::kotlin::Kotlin::cast` could
    // not tell apart from a real aggregate.
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(!generated.contains("typealias Flags"), "{generated}");
    assert!(
        generated.contains("const val READY: Int = 1 shl 0"),
        "{generated}"
    );
    assert!(
        generated.contains("const val MOVING: Int = 1 shl 4"),
        "{generated}"
    );
}

#[test]
fn xtypes_7_2_2_4_1_2_a_wide_bitmask_packs_its_bits_in_a_long() {
    let generated = generate("@bit_bound(64) bitmask Wide { @position(63) TOP };");
    assert!(
        generated.contains("const val TOP: Long = 1L shl 63"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_data_class_of_holder_typed_properties() {
    let generated = generate("bitset T { bitfield<3> a; bitfield<20> b; };");
    assert!(generated.contains("data class T("), "{generated}");
    assert!(generated.contains("var a: Int"), "{generated}");
    // Past sixteen bits the holder is a `Long`, the boundary
    // `adapter/kotlin.rs::unsigned_holder` draws and the one
    // `adapter/java.rs` draws: an `Int` holds the pattern but not its
    // sign, and the codec computes in the wider type.
    assert!(generated.contains("var b: Long"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_data_class_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("data class V("), "{generated}");
}

#[test]
fn a_constant_becomes_a_const_val_that_ascribes_its_type() {
    let generated = generate("const octet LIMIT = 8;");
    assert!(
        generated.contains("const val LIMIT: Int = 8"),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_data_class_of_its_members() {
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("data class Failure("), "{generated}");
}

#[test]
fn an_interface_becomes_a_plain_class_of_function_typed_properties() {
    let generated = generate(
        "struct Target { long x; }; interface Control { void go(in Target t); long status(); };",
    );
    assert!(generated.contains("class Control("), "{generated}");
    assert!(!generated.contains("data class Control("), "{generated}");
    assert!(
        generated.contains("val go: (Target) -> Unit"),
        "{generated}"
    );
    assert!(generated.contains("val status: () -> Int"), "{generated}");
}

#[test]
fn a_native_declaration_becomes_an_alias_to_any() {
    let generated = generate("native Handle;");
    assert!(generated.contains("typealias Handle = Any"), "{generated}");
}

#[test]
fn idl_7_5_2_a_module_flattens_onto_the_name_because_a_kotlin_file_has_one_package() {
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        generated.contains("data class common_util_Location("),
        "{generated}"
    );
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(generated.contains("// Generated by ZeroDDS"), "{generated}");
    assert!(!generated.contains("Sensitive"), "{generated}");
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    let generated = generate("struct First { long a; }; struct Second { long b; };");
    let first = generated.find("data class First").expect("First");
    let second = generated.find("data class Second").expect("Second");
    assert!(first < second, "the output reordered the declarations");
}

#[test]
fn xtypes_7_3_1_two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn xtypes_7_2_2_4_every_construct_the_catalogue_covers_produces_output() {
    let sources = [
        ("struct S { long a; };", "data class S("),
        ("enum E { A };", "object E {"),
        ("typedef long T;", "typealias T = Int"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "data class U(",
        ),
        // A bitmask's default `@bit_bound` is 32 (DDS-XTypes 1.3
        // §7.3.1.2.1.5), which `adapter/kotlin.rs::unsigned_holder` puts
        // in a `Long`.
        ("bitmask B { X };", "const val X: Long"),
        ("bitset T { bitfield<3> a; };", "data class T("),
        ("valuetype V { public long a; };", "data class V("),
        ("const long N = 1;", "const val N: Int = 1"),
        ("exception E2 { long code; };", "data class E2("),
        ("interface I { void ping(); };", "class I("),
        ("native N2;", "typealias N2 = Any"),
        ("module m { struct S { long a; }; };", "data class m_S("),
        ("struct S { map<long,long> t; };", "class ZdEntry_"),
    ];
    for (idl, expected) in sources {
        let generated = generate(idl);
        assert!(
            generated.contains(expected),
            "{idl:?} produced no {expected:?}:\n{generated}"
        );
    }
}

// -- the corpus ------------------------------------------------------------

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

/// How many corpus cases have to reach Kotlin for this gate to mean
/// anything — the same reasoning `zig_end_to_end.rs::MUST_RENDER` gives.
const MUST_RENDER: usize = 350;

/// The names this column itself refuses, each with the reason. Empty:
/// this column refuses no corpus case the frontend admits.
const REFUSED: &[(&str, &str)] = &[];

#[test]
fn xtypes_7_3_1_every_corpus_case_renders_or_names_its_failure() {
    let cases = corpus();
    assert!(
        cases.len() > 300,
        "the corpus is not where it was: {} cases",
        cases.len()
    );
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
            (Ok(_), None) => rendered += 1,
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
            "{} corpus case(s) the frontend refuses, so Kotlin never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Kotlin; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the compiler ------------------------------------------------------------

static SERIAL: AtomicU32 = AtomicU32::new(0);

/// A scratch directory of this process's own, removed when the check is
/// done.
fn scratch() -> std::path::PathBuf {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("zd-kotlin-check-{}-{serial}", std::process::id()))
}

/// The runtime source, read once per check: a generated types file
/// needing `Float128` has to compile beside it.
fn runtime_source() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/kotlin/ZdWire.kt");
    std::fs::read_to_string(&path).expect("runtime/kotlin/ZdWire.kt")
}

/// The runtime as `tests/integration/kotlin_runtime.sh` compiled it once
/// for a job, under the same `kotlinc -Werror`: its classes, when the
/// directory holds `complete` and the `ZdWire.kt` it compiled is the one
/// this tree has.
fn precompiled_runtime() -> Option<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("zd-kotlin-runtime");
    let compiled = std::fs::read_to_string(dir.join("ZdWire.kt")).ok()?;
    (dir.join("complete").is_file() && compiled == runtime_source()).then(|| dir.join("classes"))
}

/// One generated file through `kotlinc -Werror`, against the runtime it may
/// reference: the classes the job compiled once, or, without them, the
/// runtime source compiled beside the file (9.6 s against 5.3 s for one
/// small file on the Linux test host).
fn kotlin_checks(source: &str) -> Result<(), String> {
    let dir = scratch();
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let types = dir.join("ZdTypes.kt");
    let mut text = String::from("import zerodds.wire.*\n\n");
    text.push_str(source);
    std::fs::write(&types, text).map_err(|error| error.to_string())?;
    let mut command = kotlinc();
    command.arg("-Werror").arg(&types);
    if let Some(classes) = precompiled_runtime() {
        command.arg("-cp").arg(classes);
    } else {
        let runtime = dir.join("ZdWire.kt");
        std::fs::write(&runtime, runtime_source()).map_err(|error| error.to_string())?;
        command.arg(&runtime);
    }
    let out = dir.join("out");
    let output = command
        .arg("-d")
        .arg(&out)
        .output()
        .map_err(|error| error.to_string());
    let _ = std::fs::remove_dir_all(&dir);
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(String::from_utf8_lossy(&output.stderr).into_owned()),
        Err(why) => Err(why),
    }
}

/// Whether `kotlinc` is on the path at all. A look at `PATH` and not a
/// `kotlinc -version`: that starts a JVM, which under eight compilers side by
/// side took seconds of a test's 25 s and twice failed outright (pipeline
/// 8678, job 228157); a `kotlinc` that cannot start fails the compile that
/// follows, with its message.
fn kotlinc_available() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("kotlinc").is_file()))
}

/// The JVM options of every `kotlinc` here (`JAVA_OPTS`, which the
/// `kotlinc` script hands to the JVM; its own default is `-Xmx256M -Xss2m`).
/// The codegen jobs run eight compilers side by side on a host that shows
/// the JVM 32 CPUs, and a JVM sizes its compiler and GC threads by them;
/// two CPUs, the serial collector and the first compiler tier are what a
/// compile of a few files needs.
const KOTLINC_JAVA_OPTS: &str =
    "-Xmx512M -Xss2m -XX:ActiveProcessorCount=2 -XX:+UseSerialGC -XX:TieredStopAtLevel=1";

fn kotlinc() -> Command {
    let mut command = Command::new("kotlinc");
    command.env("JAVA_OPTS", KOTLINC_JAVA_OPTS);
    command
}

/// The cases of one share through one `kotlinc -Werror`, each in a file
/// and a package of its own (`zd_case_<n>`; the generated text declares no
/// package, `adapter::kotlin::kotlin_file`), against the runtime: one JVM
/// for the share instead of one per case. When the batch fails, every case
/// is compiled alone, so that a failure names its case.
fn kotlin_checks_together(cases: &[(String, String)]) -> Vec<(String, String)> {
    if cases.is_empty() {
        return Vec::new();
    }
    let dir = scratch();
    let together = compile_together(&dir, cases);
    let _ = std::fs::remove_dir_all(&dir);
    if together.is_ok() {
        return Vec::new();
    }
    cases
        .iter()
        .filter_map(|(name, source)| kotlin_checks(source).err().map(|why| (name.clone(), why)))
        .collect()
}

/// One `kotlinc -Werror` over `cases`, written into `dir`.
fn compile_together(dir: &std::path::Path, cases: &[(String, String)]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let mut command = kotlinc();
    command.arg("-Werror");
    for (index, (_, source)) in cases.iter().enumerate() {
        let file = dir.join(format!("ZdTypes{index}.kt"));
        let text = format!("package zd_case_{index}\n\nimport zerodds.wire.*\n\n{source}");
        std::fs::write(&file, text).map_err(|error| error.to_string())?;
        command.arg(file);
    }
    if let Some(classes) = precompiled_runtime() {
        command.arg("-cp").arg(classes);
    } else {
        let runtime = dir.join("ZdWire.kt");
        std::fs::write(&runtime, runtime_source()).map_err(|error| error.to_string())?;
        command.arg(&runtime);
    }
    let output = command
        .arg("-d")
        .arg(dir.join("out"))
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// Every shape above, through the compiler. One file per source: two
/// sources here declare a `Pose` and a `T`, and Kotlin allows one
/// top-level declaration per name in a file.
fn the_generated_kotlin_compiles(share: usize, shares: usize) {
    if !kotlinc_available() {
        eprintln!(
            "SKIPPED: `kotlinc` is not on the path, so the generated Kotlin was not compiled. \
             Set it up or run this on a host that has it."
        );
        return;
    }
    let sources = [
        "struct Pose { long x; double y; boolean active; };",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
        "struct S { @optional octet b; long c; string name; wstring wide; \
         sequence<long> ids; long grid[2][3]; map<long, string> table; fixed<9,2> price; };",
        "struct T { long class; long fun; long when; long value; long data; };",
        "typedef long ValueT; typedef long WhenT;",
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
        "native Handle;",
        "module common { module util { struct Location { long x; }; }; };",
        "typedef long Distance; struct Dist { Distance d; };",
        "struct Node { sequence<Node> children; };",
        "@annotation Sensitive { string reason; };",
        include_str!("../../../tests/interop-corpus/empty-struct/type.idl"),
        include_str!("../../../tests/interop-corpus/empty-struct-as-base/type.idl"),
        include_str!("../../../tests/interop-corpus/empty-struct-in-sequence/type.idl"),
        include_str!("../../../tests/interop-corpus/resolve-anonymous-nested-sequence/type.idl"),
        include_str!("../../../tests/interop-corpus/resolve-typedef-chain-three/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-double-and-longdouble/type.idl"),
        include_str!(
            "../../../tests/interop-corpus/sequence-double-and-longdouble-unannotated/type.idl"
        ),
        include_str!("../../../tests/interop-corpus/sequence-map/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-map-unannotated/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-nested/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-nested-struct/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-nested-struct-unannotated/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-nested-unannotated/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-of-array/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-of-array-unannotated/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-of-typedef/type.idl"),
        include_str!("../../../tests/interop-corpus/sequence-of-typedef-unannotated/type.idl"),
    ];
    // One `kotlinc` for the share's sources (`kotlin_checks_together`): one
    // JVM per source ran each of the four shares past its 25 s with eight
    // tests side by side (pipeline 8688, job 228189).
    let selected: Vec<(String, String)> = sources
        .iter()
        .enumerate()
        .filter(|(position, _)| position % shares == share)
        .map(|(_, idl)| ((*idl).to_owned(), generate(idl)))
        .collect();
    let failures = kotlin_checks_together(&selected);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The whole corpus through `kotlinc`, one invocation per case: every
/// case renders, and the cases of `share` (of `shares`) compile. `kotlinc`
/// starts a JVM per case, seconds each.
fn every_corpus_case_that_renders_also_compiles(share: usize, shares: usize) {
    assert!(
        kotlinc_available(),
        "the requested Kotlin compiler gate needs `kotlinc`"
    );
    let cases = corpus();
    let total = cases.len();
    let mut failures = Vec::new();
    let mut backend_refused = Vec::new();
    let mut frontend_refused = 0_usize;
    let mut checked = 0_usize;
    let mut compiled = 0_usize;
    let mut selected = Vec::new();
    for (position, (name, path)) in cases.into_iter().enumerate() {
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
        if position % shares != share {
            continue;
        }
        compiled += 1;
        selected.push((name, text));
    }
    for (name, why) in kotlin_checks_together(&selected) {
        eprintln!("COMPILER_FAILED {name}: {why}");
        failures.push(format!("{name}: {why}"));
    }
    eprintln!(
        "CORPUS_SUMMARY total={total} frontend_refused={frontend_refused} backend_refused={} reached={checked} compiled={compiled} passed={} failed={}",
        backend_refused.len(),
        compiled - failures.len(),
        failures.len(),
    );
    assert_eq!(total, frontend_refused + backend_refused.len() + checked);
    assert!(
        checked >= MUST_RENDER,
        "only {checked} cases reached Kotlin"
    );
    assert!(backend_refused.is_empty(), "{backend_refused:#?}");
    assert!(
        failures.is_empty(),
        "{} of the corpus did not compile:\n{failures:#?}",
        failures.len()
    );
}

/// zerodds-xcdr2-kotlin-1.0 §7.4.3: the generated Kotlin of every source above
/// compiles under `kotlinc`; 25 s for all of them on the Linux test host.
///
/// In shares, each within the 25 s a test has (internal/release/rc8-test-suite.md,
/// "Crate tier"); the language's weekly job and its merge job run them.
mod xtypes_7_4_3_the_generated_kotlin_compiles {
    #[test]
    fn share_01() {
        super::the_generated_kotlin_compiles(0, 4);
    }

    #[test]
    fn share_02() {
        super::the_generated_kotlin_compiles(1, 4);
    }

    #[test]
    fn share_03() {
        super::the_generated_kotlin_compiles(2, 4);
    }

    #[test]
    fn share_04() {
        super::the_generated_kotlin_compiles(3, 4);
    }
}

/// Every corpus case that renders compiles under `kotlinc`, in 64 shares,
/// each one `kotlinc` over its cases. `#[ignore]`: the crate tier has no
/// `kotlinc`; the weekly `integration-codegen-kotlin` job runs these with
/// it, eight shares at a time, each within the 25 s a test has
/// (internal/release/rc8-test-suite.md, "Crate tier").
mod every_corpus_case_that_renders_also_compiles_shares {
    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_01() {
        super::every_corpus_case_that_renders_also_compiles(0, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_02() {
        super::every_corpus_case_that_renders_also_compiles(1, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_03() {
        super::every_corpus_case_that_renders_also_compiles(2, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_04() {
        super::every_corpus_case_that_renders_also_compiles(3, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_05() {
        super::every_corpus_case_that_renders_also_compiles(4, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_06() {
        super::every_corpus_case_that_renders_also_compiles(5, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_07() {
        super::every_corpus_case_that_renders_also_compiles(6, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_08() {
        super::every_corpus_case_that_renders_also_compiles(7, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_09() {
        super::every_corpus_case_that_renders_also_compiles(8, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_10() {
        super::every_corpus_case_that_renders_also_compiles(9, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_11() {
        super::every_corpus_case_that_renders_also_compiles(10, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_12() {
        super::every_corpus_case_that_renders_also_compiles(11, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_13() {
        super::every_corpus_case_that_renders_also_compiles(12, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_14() {
        super::every_corpus_case_that_renders_also_compiles(13, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_15() {
        super::every_corpus_case_that_renders_also_compiles(14, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_16() {
        super::every_corpus_case_that_renders_also_compiles(15, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_17() {
        super::every_corpus_case_that_renders_also_compiles(16, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_18() {
        super::every_corpus_case_that_renders_also_compiles(17, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_19() {
        super::every_corpus_case_that_renders_also_compiles(18, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_20() {
        super::every_corpus_case_that_renders_also_compiles(19, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_21() {
        super::every_corpus_case_that_renders_also_compiles(20, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_22() {
        super::every_corpus_case_that_renders_also_compiles(21, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_23() {
        super::every_corpus_case_that_renders_also_compiles(22, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_24() {
        super::every_corpus_case_that_renders_also_compiles(23, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_25() {
        super::every_corpus_case_that_renders_also_compiles(24, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_26() {
        super::every_corpus_case_that_renders_also_compiles(25, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_27() {
        super::every_corpus_case_that_renders_also_compiles(26, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_28() {
        super::every_corpus_case_that_renders_also_compiles(27, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_29() {
        super::every_corpus_case_that_renders_also_compiles(28, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_30() {
        super::every_corpus_case_that_renders_also_compiles(29, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_31() {
        super::every_corpus_case_that_renders_also_compiles(30, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_32() {
        super::every_corpus_case_that_renders_also_compiles(31, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_33() {
        super::every_corpus_case_that_renders_also_compiles(32, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_34() {
        super::every_corpus_case_that_renders_also_compiles(33, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_35() {
        super::every_corpus_case_that_renders_also_compiles(34, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_36() {
        super::every_corpus_case_that_renders_also_compiles(35, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_37() {
        super::every_corpus_case_that_renders_also_compiles(36, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_38() {
        super::every_corpus_case_that_renders_also_compiles(37, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_39() {
        super::every_corpus_case_that_renders_also_compiles(38, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_40() {
        super::every_corpus_case_that_renders_also_compiles(39, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_41() {
        super::every_corpus_case_that_renders_also_compiles(40, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_42() {
        super::every_corpus_case_that_renders_also_compiles(41, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_43() {
        super::every_corpus_case_that_renders_also_compiles(42, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_44() {
        super::every_corpus_case_that_renders_also_compiles(43, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_45() {
        super::every_corpus_case_that_renders_also_compiles(44, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_46() {
        super::every_corpus_case_that_renders_also_compiles(45, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_47() {
        super::every_corpus_case_that_renders_also_compiles(46, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_48() {
        super::every_corpus_case_that_renders_also_compiles(47, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_49() {
        super::every_corpus_case_that_renders_also_compiles(48, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_50() {
        super::every_corpus_case_that_renders_also_compiles(49, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_51() {
        super::every_corpus_case_that_renders_also_compiles(50, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_52() {
        super::every_corpus_case_that_renders_also_compiles(51, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_53() {
        super::every_corpus_case_that_renders_also_compiles(52, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_54() {
        super::every_corpus_case_that_renders_also_compiles(53, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_55() {
        super::every_corpus_case_that_renders_also_compiles(54, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_56() {
        super::every_corpus_case_that_renders_also_compiles(55, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_57() {
        super::every_corpus_case_that_renders_also_compiles(56, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_58() {
        super::every_corpus_case_that_renders_also_compiles(57, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_59() {
        super::every_corpus_case_that_renders_also_compiles(58, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_60() {
        super::every_corpus_case_that_renders_also_compiles(59, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_61() {
        super::every_corpus_case_that_renders_also_compiles(60, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_62() {
        super::every_corpus_case_that_renders_also_compiles(61, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_63() {
        super::every_corpus_case_that_renders_also_compiles(62, 64);
    }

    #[test]
    #[ignore = "a share of the corpus through `kotlinc`; runs in integration-codegen-kotlin"]
    fn share_64() {
        super::every_corpus_case_that_renders_also_compiles(63, 64);
    }
}
