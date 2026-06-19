//! TS-3 — Codegen compile tests for Java.
//!
//! Generates `.java` files from IDL and invokes `javac` on them **against the
//! real ZeroDDS Java runtime** — never against hand-written stubs. The runtime
//! is the actual code the generated classes ship against:
//!   - `crates/java-omgdds/java/src/main/java` — the `omgdds.jar` runtime
//!     (`org.zerodds.cdr.*` XCDR2 codec, `org.omg.dds.*` PSM).
//!   - `crates/idl-java/runtime` — the codegen runtime (`org.zerodds.types.*`
//!     annotations, `org.zerodds.rpc.*` Holder/Requester/Replier/Future).
//!
//! This proves that generated code actually compiles against the runtime it
//! depends on — including the RPC path (Holder / Requester / Replier), which a
//! stub-based check would mask.
//!
//! **Prerequisite:** `javac` on `PATH`. Tests are skipped when not available.

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

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_java::{JavaGenOptions, generate_java_files};

fn javac_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Recursively collects every `.java` file under `dir`.
fn collect_java(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_java(&path, out);
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
}

/// Lazily `javac`-compiles the **real** ZeroDDS Java runtime (java-omgdds main
/// sources + the idl-java annotation/rpc runtime) into a classes directory and
/// returns it. `None` if `javac` is unavailable. The compiled classes are the
/// classpath every generated-code compile check runs against — no stubs.
///
/// Compiling this set is itself a test: a broken runtime source (e.g. a
/// malformed `runtime/rpc/Holder.java`) fails here.
fn real_runtime_classes() -> Option<&'static Path> {
    static CELL: OnceLock<Option<(tempfile::TempDir, PathBuf)>> = OnceLock::new();
    CELL.get_or_init(|| {
        if !javac_available() {
            eprintln!("WARNING: skipping Java compile-checks, no javac in PATH");
            return None;
        }
        let tmp = tempfile::tempdir().ok()?;
        let out = tmp.path().join("classes");
        std::fs::create_dir_all(&out).ok()?;

        let mut srcs = Vec::new();
        collect_java(
            &manifest().join("../java-omgdds/java/src/main/java"),
            &mut srcs,
        );
        collect_java(&manifest().join("runtime"), &mut srcs);
        assert!(
            !srcs.is_empty(),
            "no real runtime sources found — check crate layout"
        );

        let output = Command::new("javac")
            .arg("-nowarn")
            .arg("-d")
            .arg(&out)
            .args(&srcs)
            .output()
            .ok()?;
        if !output.status.success() {
            panic!(
                "real Java runtime failed to compile (java-omgdds + idl-java/runtime):\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Some((tmp, out))
    })
    .as_ref()
    .map(|(_, p)| p.as_path())
}

/// Generates Java from `src` and compiles it with `javac` against the real
/// runtime classpath (no stubs). `Ok(())` on success, otherwise stderr.
fn check_compiles(src: &str) -> Result<(), String> {
    let Some(classes) = real_runtime_classes() else {
        return Ok(()); // no javac — skip (warned once)
    };

    let ast =
        zerodds_idl::parse(src, &ParserConfig::default()).map_err(|e| format!("parse: {e:?}"))?;
    let files =
        generate_java_files(&ast, &JavaGenOptions::default()).map_err(|e| format!("gen: {e:?}"))?;
    if files.is_empty() {
        return Ok(());
    }

    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut paths = Vec::new();
    for f in &files {
        let dir = if f.package_path.is_empty() {
            tmp.path().to_path_buf()
        } else {
            tmp.path().join(f.package_path.replace('.', "/"))
        };
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.java", f.class_name));
        std::fs::write(&path, &f.source).map_err(|e| e.to_string())?;
        paths.push(path);
    }

    let output = Command::new("javac")
        .arg("-nowarn")
        .arg("-classpath")
        .arg(classes)
        .arg("-d")
        .arg(tmp.path().join("out"))
        .args(&paths)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("javac FAILED:\n--- stderr ---\n{stderr}"))
    }
}

#[test]
fn real_runtime_compiles() {
    // The shipped runtime (java-omgdds + idl-java/runtime) must itself compile.
    if !javac_available() {
        eprintln!("skipping — no javac");
        return;
    }
    assert!(
        real_runtime_classes().is_some(),
        "real Java runtime must compile"
    );
}

#[test]
fn compiles_simple_struct() {
    check_compiles("struct Point { long x; long y; };").expect("simple struct must compile");
}

#[test]
fn compiles_struct_with_string_sequence() {
    check_compiles("struct Bag { string name; sequence<long> ids; };")
        .expect("string+seq must compile");
}

#[test]
fn compiles_module_nesting() {
    check_compiles("module Outer { module Inner { struct S { long x; }; }; };")
        .expect("nested modules must compile");
}

#[test]
fn compiles_enum() {
    check_compiles("enum Color { RED, GREEN, BLUE };").expect("enum must compile");
}

#[test]
fn compiles_inheritance() {
    check_compiles("struct Base { long base_field; }; struct Child : Base { long child_field; };")
        .expect("inheritance must compile");
}

#[test]
fn compiles_keyed_struct() {
    check_compiles("struct Sensor { @key long id; double value; };")
        .expect("keyed struct must compile");
}

#[test]
fn compiles_optional_member() {
    check_compiles("struct S { @optional long maybe; };").expect("optional must compile");
}

#[test]
fn compiles_union() {
    check_compiles(
        "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
    )
    .expect("union must compile");
}

#[test]
fn compiles_typedef() {
    check_compiles("typedef long Counter;").expect("typedef must compile");
}

#[test]
fn compiles_exception() {
    check_compiles("exception NotFound { string what_; };").expect("exception must compile");
}

#[test]
fn compiles_unsigned_workaround() {
    check_compiles("struct U { unsigned short us; unsigned long ul; unsigned long long ull; };")
        .expect("unsigned must compile");
}

#[test]
fn compiles_array_member() {
    check_compiles("struct M { long matrix[3][4]; };").expect("array must compile");
}

#[test]
fn compiles_service_against_real_rpc_runtime() {
    // The RPC path: generated service interface + async + Requester + Replier
    // reference org.zerodds.rpc.{Holder, Requester, Replier, Future, Service}.
    // Compiling against the real runtime/rpc/*.java verifies they actually fit
    // (Holder<E>.value, Requester<Req,Rep> arity, @Service target, …) — the gap
    // that the previous stub-based check could not see.
    check_compiles(
        "@service interface Calc { \
             long add(in long a, in long b); \
             void scale(inout long acc, in long factor); \
             void note(out long handle); \
         };",
    )
    .expect("service codegen must compile against the real RPC runtime");
}

#[test]
fn compiles_service_with_struct_params_against_real_runtime() {
    // Service whose request/reply carry a struct → exercises the data-type
    // codegen (org.zerodds.cdr.* TypeSupport) AND the RPC runtime together.
    check_compiles(
        "struct Vec3 { double x; double y; double z; }; \
         @service interface Geometry { \
             Vec3 normalize(in Vec3 v); \
             void store(inout Vec3 acc); \
         };",
    )
    .expect("service with struct params must compile against the real runtime");
}
