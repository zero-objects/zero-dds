// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to the Java of IDL4 to Java 1.0, compiled with `javac` and run with
//! `java` against a check program written from the specification's text.

#![allow(dead_code, clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_lang_rules::emit::idl4_java::{JavaFile, Options, generate};

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

/// The Java files of `idl` under `options`.
pub fn java(idl: &str, options: &Options) -> Vec<JavaFile> {
    generate(&model(idl), options).unwrap_or_else(|error| panic!("{error}"))
}

/// The text of the generated file at `path`.
pub fn file<'a>(files: &'a [JavaFile], path: &str) -> &'a str {
    files
        .iter()
        .find(|file| file.path == path)
        .map(|file| file.text.as_str())
        .unwrap_or_else(|| {
            panic!(
                "no file {path}; generated: {:?}",
                files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>()
            )
        })
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(tool).is_file()))
}

/// The runtime's sources: `org.omg.type`, and `org.omg.CORBA`'s own
/// classes when the CORBA module they name is among the files compiled.
fn runtime_sources(corba: bool) -> Vec<PathBuf> {
    let java = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/java");
    let mut out = Vec::new();
    let mut packages = vec![java.join("src/main/java/org/omg/type")];
    if corba {
        packages.push(java.join("corba/org/omg/CORBA"));
    }
    for package in packages {
        let Ok(entries) = std::fs::read_dir(&package) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|ext| ext == "java") {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    out
}

/// Compile `files` with the `org.omg` runtime and the check program
/// `check` (a class `Check` in the unnamed package with a `main`), run it,
/// and fail on a compile error, a failed check or an exception.
///
/// `javac --release 8 -Xlint:all,-serial,-options -Werror`: the warnings
/// are errors, except `serial`, which flags a non-serializable field type
/// (`java.util.List<E>`) in a serializable class — the shape §7.2.4.3.1 and
/// §7.2.4.2.1.2 prescribe together — and `options`, which flags release 8
/// as old.
///
/// Without a `javac` on the PATH it says so and returns; the codegen job
/// of the Java column runs it with a JDK (ci/integration_scenarios.py).
pub fn compile_and_run(tag: &str, files: &[JavaFile], check: &str) {
    if !on_path("javac") || !on_path("java") {
        eprintln!("NOT RUN: javac or java is not on the PATH");
        return;
    }
    let dir = std::env::temp_dir().join(format!("zd-idl4java-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let src = dir.join("src");
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).expect("a class directory");
    let corba = files
        .iter()
        .any(|file| file.path.starts_with("org/omg/CORBA/"));
    let mut sources = runtime_sources(corba);
    for file in files {
        let path = src.join(&file.path);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a package directory");
        std::fs::write(&path, &file.text).expect("a source file");
        sources.push(path);
    }
    let check_path = src.join("Check.java");
    std::fs::write(&check_path, check).expect("the check program");
    sources.push(check_path);
    let output = Command::new("javac")
        .args([
            "--release",
            "8",
            "-Xlint:all,-serial,-options",
            "-Werror",
            "-encoding",
            "UTF-8",
            "-d",
        ])
        .arg(&classes)
        .args(&sources)
        .output()
        .expect("javac runs");
    if !output.status.success() {
        let listing: String = files
            .iter()
            .map(|f| format!("==== {}\n{}\n", f.path, f.text))
            .collect();
        panic!(
            "javac refused the generated code:\n{}\n{listing}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new("java")
        .arg("-cp")
        .arg(&classes)
        .arg("Check")
        .output()
        .expect("java runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("CHECKED"),
        "the check program failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Java helpers every check program starts with.
pub const CHECK_PRELUDE: &str = r#"
    static int failures = 0;

    static void check(boolean condition, String what) {
        if (!condition) {
            System.out.println("FAILED: " + what);
            failures++;
        }
    }

    static void throwsIt(Class<? extends Throwable> expected, Runnable action, String what) {
        try {
            action.run();
            System.out.println("FAILED (no exception): " + what);
            failures++;
        } catch (Throwable thrown) {
            if (!expected.isInstance(thrown)) {
                System.out.println("FAILED (" + thrown + "): " + what);
                failures++;
            }
        }
    }

    static java.lang.reflect.Method method(Class<?> type, String name, Class<?>... params) {
        try {
            return type.getMethod(name, params);
        } catch (NoSuchMethodException e) {
            System.out.println("FAILED (no method " + name + "): " + type.getName());
            failures++;
            return null;
        }
    }

    static void returns(Class<?> type, String name, Class<?> result, Class<?>... params) {
        java.lang.reflect.Method m = method(type, name, params);
        if (m != null) {
            check(m.getReturnType() == result,
                type.getName() + "." + name + " returns " + m.getReturnType() + ", not " + result);
        }
    }

    static void absent(Class<?> type, String name, Class<?>... params) {
        try {
            type.getMethod(name, params);
            System.out.println("FAILED (method present): " + type.getName() + "." + name);
            failures++;
        } catch (NoSuchMethodException e) {
            // absent, as asked
        }
    }

    static void done() {
        if (failures > 0) {
            System.out.println(failures + " FAILED");
            System.exit(1);
        }
        System.out.println("CHECKED");
    }
"#;

/// A check program: the prelude, then `body` as the statements of `main`.
pub fn check_program(body: &str) -> String {
    format!(
        "public class Check {{\n{CHECK_PRELUDE}\n    public static void main(String[] args) throws Exception {{\n{body}\n        done();\n    }}\n}}\n"
    )
}

/// The IDL of the CORBA module: the pseudo IDL the ORB implements
/// (`runtime/corba/CORBA_PIDL.idl`), which includes `orb.idl`.
pub fn corba_idl() -> String {
    let pidl = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/corba/CORBA_PIDL.idl"),
    )
    .expect("runtime/corba/CORBA_PIDL.idl");
    format!("{pidl}\n")
}

/// The Java of the CORBA module on the CORBA platform (Annex A.1): the
/// files the ORB's runtime holds besides `UserException`, `TypeCode`,
/// `Object` and `Any`.
pub fn corba_module() -> Vec<JavaFile> {
    let options = Options {
        platform: zerodds_lang_rules::emit::idl4_java::Platform::Corba,
        corba_module: true,
        ..Options::default()
    };
    java(&corba_idl(), &options)
}
