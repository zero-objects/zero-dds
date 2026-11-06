// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The whole interop corpus, as Java, through `javac`.
//!
//! The counterpart of the `every_corpus_case_that_renders_also_compiles`
//! that `swift_end_to_end.rs`, `go_end_to_end.rs` and eight more carry.
//! The Java column had none: the six
//! oldest columns lost theirs when `zerodds-idl-<lang>` was removed.
//!
//! A snapshot says the emitter produced the text it produced last time.
//! It does not say the text is Java, that every name in it resolves, or
//! that the nesting is legal — JLS 8.1 forbids a nested class sharing the
//! simple name of an enclosing one, an `int` literal wider than 32 bits
//! is an error rather than a wrap, and a `case` label that repeats is
//! rejected. None of those is visible to a substring assertion.
//!
//! The file is named `ZdTypes.java` because that is what the column
//! emits and what `javac` requires: one public top-level type per file,
//! named as the file is. `tools/wireprobe/wireprobe-zerodds-java`
//! compiles the same declarations under the same name.
//!
//! `runtime/java` is on the classpath, and only `org/zerodds/wire` of it,
//! for the two reasons that script gives. An IDL `long double` becomes
//! `org.zerodds.wire.Float128`, so the runtime is part of what the
//! declarations mean; and `org/omg/dds/rpc` under the same path is the
//! OMG RPC PSM rather than runtime — six of its files import a package
//! that lives behind another `pom.xml`, and a `find` over the whole
//! directory turns every case red on the harness's own account.
//!
//! `print_stderr` is allowed here on purpose: a skipped compiler check
//! and a frontend refusal both have to be *said*, and standard error is
//! the channel a passing test does not otherwise use.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};

use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::interface::ServiceMapping;
use zerodds_lang_rules::rule::java;
use zerodds_lang_rules::{ModelGraph, apply};

#[path = "support/corpus.rs"]
mod corpus_harness;

/// IDL in, one Java compilation unit out — naming the layer that refused
/// instead of panicking.
///
/// The declarations and the RPC binding of DDS-RPC 1.0 §7.11.2 after
/// them, which is the pair `zerodds_lang_rules::generate::declarations`
/// assembles for `Language::Java`, in a file with no package and no
/// imports: the declarations name nothing outside themselves.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    try_generate_with(idl, origin, ServiceMapping::Basic)
}

/// [`try_generate`] for the given service mapping.
fn try_generate_with(idl: &str, origin: &str, mapping: ServiceMapping) -> Result<String, String> {
    let parsed = zerodds_idl_parse::parse(idl);
    if !parsed.diagnostics().is_empty() {
        return Err(format!("parse: {:?}", parsed.diagnostics()));
    }
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), origin);
    if !lowered.diagnostics.is_empty() {
        return Err(format!("lower: {:?}", lowered.diagnostics));
    }
    let mut ast: Ast = lowered.ast;
    let report = ast
        .resolve_all_by_name()
        .map_err(|error| format!("resolve: {error:?}"))?;
    if !report.is_clean() {
        return Err(format!("resolve: broken references {:?}", report.broken));
    }
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &java::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let mut types = zerodds_lang_rules::adapter::java::java_items(&ast, &applied)
        .map_err(|error| format!("materialise: {error}"))?;
    types.extend(
        zerodds_lang_rules::emit::interface_java::java_interface_items_with(&ast, mapping)
            .map_err(|error| format!("materialise: {error}"))?,
    );
    Ok(zerodds_lang_render::java::file(
        &zerodds_lang_cst::java::File {
            doc: zerodds_lang_cst::common::Doc::none(),
            package: None,
            imports: Vec::new(),
            types,
        },
    ))
}
// -- the corpus ----------------------------------------------------------
/// How many corpus cases have to reach `javac` for this gate to mean
/// anything. 417 cases render today.
const MUST_RENDER: usize = 350;

/// The names the Java column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too — an exemption nothing checks is an exemption that outlives its
/// cause.
const REFUSED: &[(&str, &str)] = &[(
    "resolve-module-named-as-type",
    "a declaration named `common` inside a class of that name (JLS 8.1: \
     a nested class may not share the simple name of an enclosing one)",
)];

/// The cases whose generated Java `javac` rejects, each with the message
/// it answers. Empty, and a case that starts failing is a finding rather
/// than an entry to add without one.
const DOES_NOT_COMPILE: &[(&str, &str)] = &[];

// -- the compiler --------------------------------------------------------

/// Whether `javac` is usable at all.
fn javac_available() -> bool {
    // A look at PATH and not a start of the tool: a JVM, .NET or BEAM
    // start cost seconds of a test's 25 s with eight compilers side by side
    // (pipeline 8678); a tool that cannot start fails the compile that follows.
    on_path("javac")
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(tool).is_file()))
}

/// The compiled wire runtime, built once for the whole test binary, or
/// taken from the codegen job that compiled it before the tests
/// (`corpus_harness::precompiled_java_runtime`).
///
/// Once, because it is sixteen files and every case would otherwise pay
/// for them again; and once rather than per test, because two `javac`
/// invocations writing one output directory is a race whose message is
/// about the harness.
static RUNTIME: LazyLock<Result<PathBuf, String>> = LazyLock::new(build_runtime);

/// The directory the compiled runtime classes are in, or `javac`'s own
/// message.
fn runtime() -> Result<PathBuf, String> {
    RUNTIME.clone()
}

fn build_runtime() -> Result<PathBuf, String> {
    let classes = std::env::temp_dir().join(format!("zd-java-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&classes).map_err(|error| error.to_string())?;
    let wire =
        corpus_harness::repository_root().join("runtime/java/src/main/java/org/zerodds/wire");
    let mut sources: Vec<PathBuf> = std::fs::read_dir(&wire)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
        .collect();
    sources.sort();
    if let Some(classes) = corpus_harness::precompiled_java_runtime(&sources) {
        return Ok(classes);
    }
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8"])
        .arg("-d")
        .arg(&classes)
        .args(&sources)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(classes)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// The compilation units of one generated text, by class: one public
/// top-level class per file (JLS §7.6), `ZdTypes`, and `ZdServices` where
/// the case declares an interface.
fn units_of(source: &str) -> Vec<(String, String)> {
    let units = zerodds_lang_rules::generate::java_units(source);
    if units.is_empty() {
        return vec![("ZdTypes".to_string(), source.to_string())];
    }
    units
        .into_iter()
        .map(|unit| (unit.class.to_string(), unit.text))
        .collect()
}

/// One generated compilation unit through `javac`.
///
/// One invocation for this case alone: every generated file declares a
/// public `ZdTypes` in the unnamed package, so the cases of a share go
/// through [`java_compiles_together`] in packages of their own, and this
/// is the check that names a case when that fails.
fn java_compiles(runtime: &Path, source: &str, tag: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("zd-java-corpus-{}-{tag}", std::process::id()));
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    for (class, text) in units_of(source) {
        let file = dir.join(format!("{class}.java"));
        std::fs::write(&file, text).map_err(|error| error.to_string())?;
        files.push(file);
    }
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8"])
        .arg("-cp")
        .arg(runtime)
        .arg("-d")
        .arg(&classes)
        .args(&files)
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

/// A counter, so two batches of one test process do not share a directory.
static SERIAL: AtomicU32 = AtomicU32::new(0);

/// The cases of one share through one `javac`, each case's units in a
/// directory and a package of their own (`zd_case_<n>`; the generated text
/// declares no package, and every case declares a public `ZdTypes`): one
/// JVM for the share instead of one per case. A `javac` per case was most
/// of the 6.5 s a share of nine cases took on the CI runner host (pipeline 8967, job
/// 229627). When the batch fails, every case is compiled alone
/// ([`java_compiles`]), so that a failure names its case in the compiler's
/// words. The cases that did not compile, with `javac`'s message.
fn java_compiles_together(runtime: &Path, cases: &[(String, String)]) -> Vec<(String, String)> {
    if cases.is_empty() {
        return Vec::new();
    }
    let dir = std::env::temp_dir().join(format!(
        "zd-java-corpus-together-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let started = std::time::Instant::now();
    let together = javac_together(runtime, &dir, cases);
    let _ = std::fs::remove_dir_all(&dir);
    // Said on the channel a passing test does not use: how long the one
    // `javac` took, and whether the cases go on alone.
    eprintln!(
        "JAVA_TOGETHER cases={} ok={} seconds={:.1}",
        cases.len(),
        together.is_ok(),
        started.elapsed().as_secs_f64()
    );
    if together.is_ok() {
        return Vec::new();
    }
    cases
        .iter()
        .filter_map(|(name, source)| {
            java_compiles(runtime, source, name)
                .err()
                .map(|why| (name.clone(), why))
        })
        .collect()
}

/// One `javac` over `cases`, written into `dir`, each case under
/// `zd_case_<n>/` with `package zd_case_<n>;` ahead of its text.
fn javac_together(runtime: &Path, dir: &Path, cases: &[(String, String)]) -> Result<(), String> {
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    for (index, (_, source)) in cases.iter().enumerate() {
        let package = format!("zd_case_{index}");
        let case = dir.join(&package);
        std::fs::create_dir_all(&case).map_err(|error| error.to_string())?;
        for (class, text) in units_of(source) {
            let file = case.join(format!("{class}.java"));
            std::fs::write(&file, format!("package {package};\n\n{text}"))
                .map_err(|error| error.to_string())?;
            files.push(file);
        }
    }
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8"])
        .arg("-cp")
        .arg(runtime)
        .arg("-d")
        .arg(&classes)
        .args(&files)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

#[test]
fn the_compiler_check_catches_a_deliberate_error() {
    // Without this, a green result below could come from a compiler that
    // accepts anything — or from a harness that never ran one.
    if !javac_available() {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    let runtime = runtime().expect("runtime/java compiles");
    let bad = "public final class ZdTypes { NoSuchTypeExists x; }\n";
    let why = java_compiles(&runtime, bad, "deliberate").expect_err("javac accepted a bad unit");
    assert!(why.contains("NoSuchTypeExists"), "{why}");
    // And a good one passes, so the check is not simply always red — and
    // it names the runtime the classpath is here for, so a gate that
    // lost it goes red at this test rather than over five corpus cases.
    java_compiles(
        &runtime,
        "public final class ZdTypes { int x; org.zerodds.wire.Float128 wide; }\n",
        "control",
    )
    .expect("javac rejected a good unit");
    // The batch: two cases that both declare `ZdTypes` compile together,
    // and a bad one among them is named alone.
    let good = "public final class ZdTypes { int x; org.zerodds.wire.Float128 wide; }\n";
    assert!(
        java_compiles_together(
            &runtime,
            &[
                ("first".to_string(), good.to_string()),
                ("second".to_string(), good.to_string())
            ]
        )
        .is_empty(),
        "two good cases did not compile together"
    );
    let failures = java_compiles_together(
        &runtime,
        &[
            ("good".to_string(), good.to_string()),
            ("bad".to_string(), bad.to_string()),
        ],
    );
    assert_eq!(
        failures
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["bad"],
        "{failures:#?}"
    );
    assert!(failures[0].1.contains("NoSuchTypeExists"), "{failures:#?}");
}

/// The whole corpus through `javac`: every case renders or is refused as
/// listed, and the cases of `share` (of `shares`) compile, in one `javac`
/// ([`java_compiles_together`]).
fn every_corpus_case_that_renders_also_compiles(share: usize, shares: usize) {
    if !javac_available() {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    let runtime = runtime().expect("runtime/java compiles");
    let mut failures = Vec::new();
    let mut unexpectedly_fine = Vec::new();
    let mut frontend = Vec::new();
    let mut selected = Vec::new();
    let mut rendered = 0_usize;
    for (position, (name, path)) in corpus_harness::corpus().into_iter().enumerate() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let refused = REFUSED.iter().find(|(case, _)| *case == name);
        let text = match (try_generate(&source, &path.display().to_string()), refused) {
            (Ok(text), None) => text,
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
                continue;
            }
            (Err(why), None) => {
                // The stage that refused is what decides whose failure it
                // is. `parse` and `lower` are the frontend's and are the
                // same for every column.
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else {
                    failures.push(format!("{name}: {why}"));
                }
                continue;
            }
            (Err(_), Some(_)) => continue,
        };
        rendered += 1;
        if position % shares == share {
            selected.push((name, text));
        }
    }
    let checked = selected.len();
    let rejected = java_compiles_together(&runtime, &selected);
    for (name, _) in &selected {
        let known = DOES_NOT_COMPILE
            .iter()
            .find(|(case, _)| *case == name.as_str());
        let why = rejected.iter().find(|(case, _)| case == name);
        match (why, known) {
            (None, None) | (Some(_), Some(_)) => {}
            (None, Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: compiles now, but is listed as {why}"));
            }
            (Some((_, why)), None) => failures.push(format!("{name}: {why}")),
        }
    }
    if !frontend.is_empty() {
        eprintln!(
            "{} corpus case(s) the frontend refuses, so Java never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(unexpectedly_fine.is_empty(), "{unexpectedly_fine:#?}");
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} corpus cases reached `javac`; the gate says nothing \
         below {MUST_RENDER}"
    );
}

/// One RPC corpus case as the generator writes it for `javac`: the
/// declarations, the services and the codec, each its own file, compiled
/// with the RPC runtime and the DDS Java PSM it names — the way
/// `crates/rpc/tests/java_rpc_runtime.rs` compiles the function-call
/// binding.
fn rpc_case_compiles(
    source: &str,
    origin: &str,
    mapping: ServiceMapping,
    tag: &str,
) -> Result<(), String> {
    let parsed = zerodds_idl_parse::parse(source);
    if !parsed.diagnostics().is_empty() {
        return Err(format!("parse: {:?}", parsed.diagnostics()));
    }
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), origin);
    if !lowered.diagnostics.is_empty() {
        return Err(format!("lower: {:?}", lowered.diagnostics));
    }
    let mut ast: Ast = lowered.ast;
    let report = ast
        .resolve_all_by_name()
        .map_err(|error| format!("resolve: {error:?}"))?;
    if !report.is_clean() {
        return Err(format!("resolve: broken references {:?}", report.broken));
    }
    let text = zerodds_lang_rules::generate::source_with_mapping(
        &ast,
        zerodds_lang_cst::Language::Java,
        zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
        zerodds_lang_rules::emit::encode::CodecOptions::default(),
        mapping,
    )
    .map_err(|error| format!("generate: {error}"))?;

    let root = corpus_harness::repository_root();
    let dir = std::env::temp_dir().join(format!("zd-java-rpc-{}-{tag}", std::process::id()));
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    for unit in zerodds_lang_rules::generate::java_units(&text) {
        let body = if unit.class == "ZdCodec" {
            format!("import org.zerodds.wire.*;\n\n{}", unit.text)
        } else {
            unit.text.clone()
        };
        let file = dir.join(format!("{}.java", unit.class));
        std::fs::write(&file, body).map_err(|error| error.to_string())?;
        files.push(file);
    }
    for package in [
        "runtime/java/src/main/java/org/omg/dds/rpc",
        "runtime/java/src/main/java/org/zerodds/ddsrpc",
    ] {
        for entry in std::fs::read_dir(root.join(package)).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "java") {
                files.push(path);
            }
        }
    }
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8", "-Xlint:all"])
        .arg("-d")
        .arg(&classes)
        .arg("-sourcepath")
        .arg(format!(
            "{}:{}:{}",
            root.join("runtime/java/src/main/java").display(),
            root.join("crates/java-omgdds/java/src/main/java").display(),
            dir.display()
        ))
        .args(&files)
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

/// The RPC corpus — interfaces, under both service mappings — through
/// `javac`. The data corpus above has no `interface` in it, so nothing
/// there compiles the RPC binding.
fn every_rpc_corpus_case_compiles(share: usize, shares: usize) {
    if !javac_available() {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    let cases = corpus_harness::rpc_corpus();
    assert!(
        cases.len() >= 4,
        "the RPC corpus has Basic and Enhanced cases: {}",
        cases.len()
    );
    let mut failures = Vec::new();
    for (position, (name, path, mapping)) in cases.into_iter().enumerate() {
        if position % shares != share {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if let Err(why) = rpc_case_compiles(&source, &path.display().to_string(), mapping, &name) {
            failures.push(format!("{name}: {why}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// IDL4 to Java 1.0 §7.2.4.3.4 and the column: every corpus case that renders compiles under `javac`.
///
/// The corpus in 16 shares of 26 or 27 cases, each share one `javac`
/// ([`java_compiles_together`]) within the 25 s a test has
/// (internal/release/rc8-test-suite.md, "Crate tier"); the shares together
/// are every case. `#[ignore]`: the crate tier has no toolchain for it; the
/// weekly `integration-codegen-java` job runs them with the toolchain, four
/// at a time.
mod idl4java_7_2_4_3_4_every_corpus_case_that_renders_also_compiles {
    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_01() {
        super::every_corpus_case_that_renders_also_compiles(0, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_02() {
        super::every_corpus_case_that_renders_also_compiles(1, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_03() {
        super::every_corpus_case_that_renders_also_compiles(2, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_04() {
        super::every_corpus_case_that_renders_also_compiles(3, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_05() {
        super::every_corpus_case_that_renders_also_compiles(4, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_06() {
        super::every_corpus_case_that_renders_also_compiles(5, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_07() {
        super::every_corpus_case_that_renders_also_compiles(6, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_08() {
        super::every_corpus_case_that_renders_also_compiles(7, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_09() {
        super::every_corpus_case_that_renders_also_compiles(8, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_10() {
        super::every_corpus_case_that_renders_also_compiles(9, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_11() {
        super::every_corpus_case_that_renders_also_compiles(10, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_12() {
        super::every_corpus_case_that_renders_also_compiles(11, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_13() {
        super::every_corpus_case_that_renders_also_compiles(12, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_14() {
        super::every_corpus_case_that_renders_also_compiles(13, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_15() {
        super::every_corpus_case_that_renders_also_compiles(14, 16);
    }

    #[test]
    #[ignore = "a sixteenth of the corpus through one javac; runs in integration-codegen-java"]
    fn share_16() {
        super::every_corpus_case_that_renders_also_compiles(15, 16);
    }
}

/// The RPC corpus, both service mappings, through `javac`; 17 s for all of
/// it on the Linux test host.
///
/// In shares, each within the 25 s a test has (internal/release/rc8-test-suite.md,
/// "Crate tier"); the language's weekly job and its merge job run them.
mod every_rpc_corpus_case_compiles_shares {
    #[test]
    fn share_01() {
        super::every_rpc_corpus_case_compiles(0, 4);
    }

    #[test]
    fn share_02() {
        super::every_rpc_corpus_case_compiles(1, 4);
    }

    #[test]
    fn share_03() {
        super::every_rpc_corpus_case_compiles(2, 4);
    }

    #[test]
    fn share_04() {
        super::every_rpc_corpus_case_compiles(3, 4);
    }
}
