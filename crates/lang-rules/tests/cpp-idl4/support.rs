// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to the C++ of IDL4 to C++ 1.0, compiled with the system's C++
//! compiler against `runtime/cpp/include/omg` and run with a check program
//! written from the specification's text.

#![allow(dead_code, clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_lang_rules::emit::idl4_cpp::{CppFile, Options, generate};

/// The model of `idl`, front-end diagnostics refused. Composed as
/// `zerodds-idlc` composes a file: preprocessed with the built-in
/// `orb.idl`, lowered and resolved.
pub fn model(idl: &str) -> zerodds_ast::Ast {
    let loader = zerodds_idl_parse::MemoryLoader::new();
    let composed = zerodds_idl_compose::compose_source_with(
        "test.idl",
        idl,
        &loader,
        zerodds_idl_compose::ComposeOptions::default(),
    );
    let errors: Vec<_> = composed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the source does not compose: {errors:?}");
    composed.ast
}

/// The header of `idl` under `options`.
pub fn cpp(idl: &str, options: &Options) -> Vec<CppFile> {
    generate(&model(idl), options).unwrap_or_else(|error| panic!("{error}"))
}

/// The text of the one generated header.
pub fn header(files: &[CppFile]) -> &str {
    &files.first().expect("a header").text
}

/// The IDL of the CORBA module: the pseudo IDL the ORB implements
/// (`runtime/corba/CORBA_PIDL.idl`), which includes `orb.idl`.
pub fn corba_idl() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/corba/CORBA_PIDL.idl");
    std::fs::read_to_string(&path).expect("runtime/corba/CORBA_PIDL.idl")
}

fn include_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/cpp/include")
}

/// The C++ compiler: `$CXX`, else `c++`.
fn compiler() -> Option<String> {
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    Command::new(&cxx)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| cxx)
}

/// Compile `files` and the check program `check` (a translation unit with
/// a `main`) under each standard of `standards` (`c++11`, `c++17`,
/// `c++03`), run it, and fail on a compile error, a failed check or a
/// crash.
///
/// `-Wall -Wextra -Wpedantic -Werror`: a warning in generated code is a
/// question nobody answered. Without a C++ compiler it says so and
/// returns; the codegen job of the C++ column runs it with one
/// (ci/integration_scenarios.py).
pub fn compile_and_run(tag: &str, files: &[CppFile], check: &str, standards: &[&str]) {
    let Some(cxx) = compiler() else {
        eprintln!("NOT RUN: no C++ compiler");
        return;
    };
    let dir = std::env::temp_dir().join(format!("zd-cpp-idl4-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a source directory");
    for file in files {
        std::fs::write(dir.join(&file.path), &file.text).expect("a header");
    }
    let main = dir.join("check.cpp");
    std::fs::write(&main, check).expect("the check program");
    for standard in standards {
        let exe = dir.join(format!("check-{standard}"));
        let output = Command::new(&cxx)
            .arg(format!("-std={standard}"))
            .args(["-Wall", "-Wextra", "-Wpedantic", "-Werror", "-O0"])
            // C++03 has no 64-bit integer type; Tables C.1 and C.3 ask for
            // one "of the appropriate size in the target platform", which
            // is `long long` taken as the extension it is there.
            .args(if *standard == "c++03" {
                &["-Wno-long-long"][..]
            } else {
                &[][..]
            })
            .arg("-I")
            .arg(include_dir())
            .arg("-I")
            .arg(&dir)
            .arg(&main)
            .arg("-o")
            .arg(&exe)
            .output()
            .expect("the C++ compiler runs");
        if !output.status.success() {
            let listing: String = files
                .iter()
                .map(|f| format!("==== {}\n{}\n", f.path, f.text))
                .collect();
            panic!(
                "{cxx} -std={standard} refused the generated code:\n{}{}\n{listing}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let output = Command::new(&exe).output().expect("the check program runs");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("CHECKED"),
            "the check program failed under -std={standard}:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The helpers every check program starts with, after the generated
/// header.
pub const CHECK_PRELUDE: &str = r#"
#include <iostream>
#include <stdexcept>
#include <string>

static int failures = 0;

static void ok(bool condition, const char* what) {
    if (!condition) {
        std::cout << "FAILED: " << what << "\n";
        ++failures;
    }
}

#if __cplusplus >= 201103L
#include <type_traits>
// Variadic: the second type may have a comma of its own
// (`std::map<K, V>`), which the preprocessor takes for an argument's end.
#define SAME(A, ...) ok(std::is_same<A, __VA_ARGS__>::value, "the type is " #__VA_ARGS__)

template <typename E, typename F>
static void throws(F f, const char* what) {
    try {
        f();
        std::cout << "FAILED (no exception): " << what << "\n";
        ++failures;
    } catch (const E&) {
    } catch (...) {
        std::cout << "FAILED (another exception): " << what << "\n";
        ++failures;
    }
}
#endif

static int done() {
    if (failures > 0) {
        std::cout << failures << " FAILED\n";
        return 1;
    }
    std::cout << "CHECKED\n";
    return 0;
}
"#;

/// A check program: the header, the prelude, `declarations`, then `body`
/// as the statements of `main`.
pub fn check_program(header: &str, declarations: &str, body: &str) -> String {
    format!(
        "#include \"{header}\"\n{CHECK_PRELUDE}\n{declarations}\n\nint main() {{\n{body}\n    return done();\n}}\n"
    )
}
