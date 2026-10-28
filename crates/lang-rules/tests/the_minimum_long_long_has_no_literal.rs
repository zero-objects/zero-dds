// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! −2^63 is a value four of these columns have no literal for.
//!
//! # Why this is a test
//!
//! Because the generators wrote one anyway. Every one of C, C++, D and
//! Kotlin gives an integer literal no sign — the minus in
//! `-9223372036854775808` is an operator applied to the decimal
//! constant `9223372036854775808`, and that constant is one past the
//! largest signed 64-bit value in all four. Each answers differently
//! and none of them answers with the value:
//!
//! - **C.** C99 6.4.4.1 lists the types an unsuffixed decimal constant
//!   may take and `9223372036854775808` fits none of them, so the
//!   implementation may give it an extended *unsigned* type and the
//!   negation that follows is an implementation-defined conversion.
//!   `INT64_C(..)` does not help: C99 7.18.4.1 makes the macro paste a
//!   suffix onto the token it is handed, so the same constant is what
//!   the minus applies to. Measured on the generated header, gcc 16.1.0
//!   says `warning: integer constant is so large that it is unsigned`
//!   and Apple clang 21.0.0 says
//!   `warning: integer literal is too large to be represented in a
//!   signed integer type, interpreting as unsigned
//!   [-Wimplicitly-unsigned-literal]`.
//! - **C++.** [lex.icon]/3: a literal whose value fits none of the
//!   types its suffix admits is ill-formed. g++ 16.1.0 and clang 21.0.0
//!   print the same two sentences as above.
//! - **D.** ldc2 1.40.0 and gdc 14.2.0 both refuse
//!   `cast(long)-9223372036854775808L` outright —
//!   `Error: signed integer overflow`. Not a warning: the column did
//!   not compile at all.
//! - **Kotlin.** kotlinc 2.4.0 says `error: value out of range.` on
//!   `-9223372036854775808L`, in a `const val` initialiser and in an
//!   assignment alike.
//!
//! The other seventeen columns were measured too and carry the value as
//! it stands: Rust, Java, C#, F#, Swift, Nim, Ada, Ada83, OCaml, Go,
//! Zig, Julia, Python, Elixir, TypeScript and Node each accept the bare
//! form (Java by JLS 3.10.1 and C# by ECMA-334 6.4.5.3, which both name
//! this exact value), and Lua writes it as the hexadecimal pattern
//! `0x8000000000000000` because a Lua decimal that large is a *float*.
//! None of them is touched here.
//!
//! # What this pins
//!
//! The four spellings, and the value behind them. `WIDE_MIN` is checked
//! against the header's own `INT64_MIN` and against `long.min` at
//! *compile* time, so a form that compiles but means something else
//! fails here rather than on the wire.
//!
//! `print_stderr` is allowed on purpose: a skipped compiler check has to
//! be said, and standard error is the channel a passing test does not
//! otherwise use.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::common::Language;
use zerodds_lang_rules::generate;

/// A constant, a union label and a `@default`, all carrying −2^63.
///
/// Three different seams reach the literal — the materialiser writes
/// the constant, `emit::<lang>::wide_int` writes the label and
/// `default_<lang>` writes the member's default — and a fix in one of
/// them is not a fix in the others. The corpus has no case that carries
/// this value in any of the three, which is why it went unseen.
const IDL: &str = "\
const long long WIDE_MIN = -9223372036854775808;
union Choice switch (long long) {
    case -9223372036854775808: long low;
    case 9223372036854775807: double high;
    default: double other;
};
@final struct Sample { Choice picked; @default(-9223372036854775808) long long tail; };
";

/// The digits with no form in front of them, which is what every column
/// here used to write.
const BARE: &str = "-9223372036854775808";

fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

/// One column's whole generated text — declarations and codec.
fn source(language: Language) -> String {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    generate::source(&model(), language, key)
        .unwrap_or_else(|error| panic!("{language:?} refused the source: {error}"))
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn scratch(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("zd-longlong-min-{}-{tag}", std::process::id()))
}

// -- the four spellings --------------------------------------------------

#[test]
fn c_writes_the_subtraction_and_never_the_bare_digits() {
    let text = source(Language::C);
    assert!(
        text.contains("(-9223372036854775807LL - 1)"),
        "the C column lost the subtraction:\n{text}"
    );
    assert!(
        !text.contains(BARE),
        "the C column still writes a constant it has no signed type for:\n{text}"
    );
    // The value one below is still the macro: only the minimum has no
    // `INT64_C` form, and widening every other label would be a second
    // change hiding behind this one.
    assert!(
        text.contains("INT64_C(9223372036854775807)"),
        "the other end of the range stopped being `INT64_C`:\n{text}"
    );
}

#[test]
fn cpp_writes_the_subtraction_and_never_the_bare_digits() {
    let text = source(Language::Cpp);
    assert!(
        text.contains("(-9223372036854775807LL - 1)"),
        "the C++ column lost the subtraction:\n{text}"
    );
    assert!(
        !text.contains(BARE),
        "the C++ column still writes an ill-formed literal:\n{text}"
    );
    assert!(
        text.contains("9223372036854775807LL"),
        "the other end of the range lost its suffix:\n{text}"
    );
}

#[test]
fn d_writes_long_min_and_never_the_suffixed_digits() {
    let text = source(Language::D);
    assert!(
        text.contains("cast(long)long.min"),
        "the D column lost `long.min`:\n{text}"
    );
    assert!(
        !text.contains("-9223372036854775808L"),
        "the D column still writes a literal both its compilers refuse:\n{text}"
    );
    assert!(
        text.contains("9223372036854775807L"),
        "the other end of the range lost its suffix:\n{text}"
    );
}

#[test]
fn kotlin_writes_long_min_value_and_never_the_suffixed_digits() {
    let text = source(Language::Kotlin);
    assert!(
        text.contains("Long.MIN_VALUE"),
        "the Kotlin column lost `Long.MIN_VALUE`:\n{text}"
    );
    assert!(
        !text.contains(BARE),
        "the Kotlin column still writes a literal kotlinc calls out of range:\n{text}"
    );
    assert!(
        text.contains("9223372036854775807L"),
        "the other end of the range lost its suffix:\n{text}"
    );
}

/// The columns that were measured to carry the value as it stands, and
/// so are not to be given a form of their own.
///
/// A list and not a filter: a column that stops writing the bare digits
/// fails here, because that is a change to a column this defect never
/// reached.
#[test]
fn the_columns_that_already_held_the_value_still_write_it_plainly() {
    for language in [
        Language::Rust,
        Language::Java,
        Language::CSharp,
        Language::FSharp,
        Language::Swift,
        Language::Nim,
        Language::Ada,
        Language::Ada83,
        Language::OCaml,
        Language::Go,
        Language::Zig,
        Language::Julia,
        Language::Python,
        Language::Elixir,
        Language::TypeScript,
        Language::Node,
    ] {
        let text = source(language);
        assert!(
            text.contains(BARE),
            "{language:?} stopped writing the value plainly, which nothing here asked for:\n{text}"
        );
    }
    // Lua is the one column that never wrote the digits: a Lua decimal
    // above 2^63 − 1 denotes a float (Lua 5.4 reference 3.1), so the
    // minimum goes out as the hexadecimal pattern the integer holds.
    let lua = source(Language::Lua);
    assert!(
        lua.contains("0x8000000000000000"),
        "the Lua column lost the hexadecimal pattern:\n{lua}"
    );
    assert!(
        !lua.contains(BARE),
        "the Lua column started writing a decimal that is a float:\n{lua}"
    );
}

// -- through the compilers -----------------------------------------------

/// The first C compiler on the path, if there is one.
fn cc() -> Option<String> {
    if let Ok(chosen) = std::env::var("CC") {
        return Some(chosen);
    }
    ["cc", "gcc", "clang"]
        .iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(|found| (*found).to_string())
}

/// The first C++ compiler on the path, if there is one.
fn cxx() -> Option<String> {
    if let Ok(chosen) = std::env::var("CXX") {
        return Some(chosen);
    }
    ["c++", "g++", "clang++"]
        .iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(|found| (*found).to_string())
}

/// The first D compiler on the path, if there is one — the same three
/// `d_end_to_end.rs` looks for, in the same order.
fn dc() -> Option<&'static str> {
    ["ldc2", "gdc", "dmd"].into_iter().find(|compiler| {
        Command::new(compiler)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    })
}

/// The generated C, compiled under the flag set
/// `tools/wireprobe/wireprobe-zerodds-c` measures the column with.
///
/// The translation unit carries one check the compiler answers by
/// itself: a `typedef` whose array length is `-1` unless `WIDE_MIN`
/// equals `<stdint.h>`'s own `INT64_MIN` is a constraint violation
/// (C99 6.7.5.2), so a spelling that compiles but names a different
/// value fails here and not on the wire.
#[test]
fn the_generated_c_compiles_and_says_the_value_exactly() {
    let Some(cc) = cc() else {
        eprintln!("SKIPPED: no C compiler on the path, so the generated C was not compiled.");
        return;
    };
    let dir = scratch("c");
    std::fs::create_dir_all(&dir).expect("the scratch directory");
    std::fs::write(
        dir.join("zd_types.h"),
        format!("#include \"zerodds_wire.h\"\n{}", source(Language::C)),
    )
    .expect("the header");
    // The codec is `static`, so a translation unit that only includes
    // the header leaves every entry point unused and `-Werror` reports
    // the harness rather than the generated code. Calling them is what
    // a consumer does, and it puts the union's own comparison chain —
    // where the label lives — through the compiler.
    std::fs::write(
        dir.join("tu.c"),
        "#include \"zd_types.h\"\n\
         typedef char zd_wide_min_is_int64_min[(WIDE_MIN == INT64_MIN) ? 1 : -1];\n\
         int64_t zd_use_wide_min(void) { return WIDE_MIN; }\n\
         void zd_use_codec(zdw_writer *w, zdw_reader *r, Sample *s) {\n\
         \x20   zd_enc_Sample(w, s);\n\
         \x20   zd_dec_Sample(r, s, ZDW_TC_DISCARD);\n\
         }\n",
    )
    .expect("the translation unit");
    let output = Command::new(&cc)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Wconversion",
            "-Wsign-conversion",
            "-Werror",
            "-fsyntax-only",
        ])
        .arg(format!("-I{}", dir.display()))
        .arg(format!(
            "-I{}",
            repository_root().join("runtime/c/include").display()
        ))
        .arg(dir.join("tu.c"))
        .output()
        .expect("invoke the C compiler");
    let why = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output.status;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.success(), "the generated C did not compile:\n{why}");
}

/// The generated C++, under the flag set `cpp_corpus_compiles.rs` uses,
/// with the same value check spelled as a `static_assert`.
#[test]
fn the_generated_cpp_compiles_and_says_the_value_exactly() {
    let Some(cxx) = cxx() else {
        eprintln!("SKIPPED: no C++ compiler on the path, so the generated C++ was not compiled.");
        return;
    };
    let dir = scratch("cpp");
    std::fs::create_dir_all(&dir).expect("the scratch directory");
    std::fs::write(
        dir.join("zd_types.hpp"),
        format!(
            "#pragma once\n#include <cstdint>\n#include \"zerodds/wire.hpp\"\n{}",
            source(Language::Cpp)
        ),
    )
    .expect("the header");
    // As in the C gate: the codec is `static`, so the entry points are
    // called rather than left unused.
    std::fs::write(
        dir.join("tu.cpp"),
        "#include \"zd_types.hpp\"\n\
         static_assert(WIDE_MIN == INT64_MIN, \"the minimum moved\");\n\
         std::int64_t zd_use_wide_min() { return WIDE_MIN; }\n\
         void zd_use_codec(zerodds::wire::Writer& w, zerodds::wire::Reader& r, ::Sample& s) {\n\
         \x20   zd_enc_Sample(w, s);\n\
         \x20   zd_dec_Sample(r, s, zerodds::wire::TryConstruct::Discard);\n\
         }\n",
    )
    .expect("the translation unit");
    let output = Command::new(&cxx)
        .args(["-std=c++20", "-fsyntax-only", "-Wall", "-Wextra", "-Werror"])
        .arg(format!("-I{}", dir.display()))
        .arg(format!(
            "-I{}",
            repository_root().join("runtime/cpp/include").display()
        ))
        .arg(dir.join("tu.cpp"))
        .output()
        .expect("invoke the C++ compiler");
    let why = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output.status;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        status.success(),
        "the generated C++ did not compile:\n{why}"
    );
}

/// The generated D through the front end, beside the runtime it
/// imports, with D's own `static assert` on the value.
#[test]
fn the_generated_d_compiles_and_says_the_value_exactly() {
    let Some(compiler) = dc() else {
        eprintln!(
            "SKIPPED: no D compiler on the path (`ldc2`, `gdc`, `dmd`), so the generated D was \
             not compiled."
        );
        return;
    };
    let dir = scratch("d");
    std::fs::create_dir_all(&dir).expect("the scratch directory");
    let runtime = repository_root().join("runtime/d/zdwire.d");
    std::fs::copy(&runtime, dir.join("zdwire.d")).expect("runtime/d/zdwire.d");
    // Compiling both modules does not import one into the other. As with
    // the C/C++ includes and Kotlin import in the adjacent checks, the
    // consumer supplies the runtime dependency used by the emitted codec.
    std::fs::write(
        dir.join("zdtypes.d"),
        format!(
            "{}\nimport zdwire;\nstatic assert(WIDE_MIN == long.min, \"the minimum moved\");\n",
            source(Language::D)
        ),
    )
    .expect("the module");
    let args: &[&str] = if compiler == "gdc" {
        &["-fsyntax-only"]
    } else {
        &["-o-"]
    };
    let output = Command::new(compiler)
        .args(args)
        .arg(dir.join("zdtypes.d"))
        .arg(dir.join("zdwire.d"))
        .current_dir(&dir)
        .output()
        .expect("invoke the D compiler");
    let mut why = String::from_utf8_lossy(&output.stderr).into_owned();
    why.push_str(&String::from_utf8_lossy(&output.stdout));
    let status = output.status;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.success(), "the generated D did not compile:\n{why}");
}

/// The generated Kotlin through `kotlinc -Werror`, beside the runtime,
/// the way `kotlin_end_to_end.rs` compiles the column.
#[test]
fn the_generated_kotlin_compiles() {
    let available = Command::new("kotlinc")
        .arg("-version")
        .output()
        .is_ok_and(|out| out.status.success());
    if !available {
        eprintln!(
            "SKIPPED: `kotlinc` is not on the path, so the generated Kotlin was not compiled."
        );
        return;
    }
    let dir = scratch("kotlin");
    std::fs::create_dir_all(&dir).expect("the scratch directory");
    std::fs::write(
        dir.join("ZdTypes.kt"),
        format!("import zerodds.wire.*\n\n{}", source(Language::Kotlin)),
    )
    .expect("the file");
    let runtime = repository_root().join("runtime/kotlin/ZdWire.kt");
    std::fs::copy(&runtime, dir.join("ZdWire.kt")).expect("runtime/kotlin/ZdWire.kt");
    let output = Command::new("kotlinc")
        .arg("-Werror")
        .arg(dir.join("ZdTypes.kt"))
        .arg(dir.join("ZdWire.kt"))
        .arg("-d")
        .arg(dir.join("out"))
        .output()
        .expect("invoke kotlinc");
    let why = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output.status;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        status.success(),
        "the generated Kotlin did not compile:\n{why}"
    );
}
