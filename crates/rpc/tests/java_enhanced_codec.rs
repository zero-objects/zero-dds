// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The octets of the Enhanced Service Mapping (DDS-RPC 1.0 §7.5.1.2) that
//! the generated Java codec writes, against the octets of the clause.
//!
//! `crates/lang-rules/src/emit/interface_java/enhanced_calls.rs` writes the
//! codec; `tests/java/EnhancedCodecBytes.java` compares what it writes and
//! reads with the octets computed by hand. The generated files and the
//! program are compiled with `javac -Xlint:all` wherever there is a JDK; the
//! program runs on Linux, or where `ZD_RUN_GENERATED=1` asks for it.
//! `tests/java/EnhancedRuntimeChecks.java` runs the generated service and
//! client over a loopback binding of the three abstract entities.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_lang_rules::adapter::interface::{ServiceMapping, member_id};

const INTERFACE: &str = "
    module robot {
      exception TooFast { long limit; string why; };
      interface Base2 {
        long ident();
      };
      interface Calc : Base2 {
        long add(in long a, in long b) raises (TooFast);
        void ping();
        void note(in string text);
      };
      interface Fast {
        @Mutable long square(in long x);
      };
    };
    // Two versions of one interface (EvolutionChecks.java): operations added
    // and removed, the parameters of a @Mutable operation reordered, an
    // exception added.
    module ev_old {
      exception E1 { long code; };
      interface Svc {
        long add(in long a, in long b);
        long legacy();
        @Mutable void mix(in long x, in long y);
        long fail() raises (E1);
      };
    };
    module ev_new {
      exception E1 { long code; };
      exception E2 { long code; };
      interface Svc {
        long add(in long a, in long b);
        long extra();
        @Mutable void mix(in long y, in long x);
        long fail() raises (E1, E2);
      };
    };
";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn toolchain() -> bool {
    ["javac", "java"].iter().all(|tool| {
        Command::new(tool)
            .arg("-version")
            .output()
            .is_ok_and(|out| out.status.success())
    })
}

fn run(command: &mut Command, what: &str) -> std::process::Output {
    let output = command.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        output.status.success(),
        "{what} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// The three compilation units the generator writes for [`INTERFACE`].
fn generated_units() -> Vec<(String, String)> {
    let parsed = zerodds_idl_parse::parse(INTERFACE);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "calc.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    // the Java programs of this test read the IDL-to-Java shape (public fields)
    let text = zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        || {
            zerodds_lang_rules::generate::source_with_mapping(
                &ast,
                zerodds_lang_cst::Language::Java,
                zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
                zerodds_lang_rules::emit::encode::CodecOptions::default(),
                ServiceMapping::Enhanced,
            )
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    zerodds_lang_rules::generate::java_units(&text)
        .into_iter()
        .map(|unit| {
            let body = if unit.class == "ZdCodec" {
                format!("import org.zerodds.wire.*;\n\n{}", unit.text.trim_end())
            } else {
                unit.text.clone()
            };
            (unit.class.to_string(), body)
        })
        .collect()
}

/// One directory per test: `cargo test` runs them on threads, and two
/// `javac` invocations writing one class directory is a race.
fn compile(root: &Path, test: &str) -> PathBuf {
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dds-rpc-java")
        .join(test);
    let classes = at.join("classes");
    // no class of an earlier run (another source layout) may stay in the directory
    let _ = std::fs::remove_dir_all(&classes);
    std::fs::create_dir_all(&classes).expect("the class directory");
    let mut files = Vec::new();
    for (class, body) in generated_units() {
        let file = at.join(format!("{class}.java"));
        std::fs::write(&file, body).expect("a generated unit");
        files.push(file);
    }
    files.push(root.join("crates/rpc/tests/java/EnhancedCodecBytes.java"));
    files.push(root.join("crates/rpc/tests/java/EnhancedRuntimeChecks.java"));
    files.push(root.join("crates/rpc/tests/java/EvolutionChecks.java"));
    for package in [
        "runtime/java/src/main/java/org/omg/dds/rpc",
        "runtime/java/src/main/java/org/zerodds/ddsrpc",
    ] {
        for entry in std::fs::read_dir(root.join(package)).expect("the rpc package") {
            let path = entry.expect("a package entry").path();
            if path.extension().is_some_and(|ext| ext == "java") {
                files.push(path);
            }
        }
    }
    run(
        Command::new("javac")
            .args(["-encoding", "UTF-8", "-Xlint:all"])
            .arg("-d")
            .arg(&classes)
            .arg("-sourcepath")
            .arg(format!(
                "{}:{}:{}",
                root.join("runtime/java/src/main/java").display(),
                root.join("crates/java-omgdds/java/src/main/java").display(),
                at.display()
            ))
            .args(&files),
        "compiling the generated codec and the program",
    );
    classes
}

/// §7.5.1.2: the generated codec writes and reads the octets of the clause.
#[test]
fn the_generated_java_codec_writes_the_octets_of_the_clause() {
    if !toolchain() {
        println!("no JDK on this machine — the Java codec is not measured here");
        return;
    }
    let root = repository_root();
    let classes = compile(&root, "enhanced-codec");
    if !(cfg!(target_os = "linux") || std::env::var("ZD_RUN_GENERATED").is_ok_and(|v| v == "1")) {
        println!(
            "compiled, not run: this host does not run generated programs (set ZD_RUN_GENERATED=1)"
        );
        return;
    }
    let output = run(
        Command::new("java")
            .arg("-cp")
            .arg(&classes)
            .arg("EnhancedCodecBytes")
            .arg(member_id("add").to_string())
            .arg(member_id("result").to_string())
            .arg(member_id("remoteEx").to_string())
            .arg(member_id("x").to_string()),
        "the Enhanced codec program",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}

/// §7.7.2, §7.5.1.2.1.1, §7.5.2 and §7.4.1 at run time: the generated service
/// answers an unknown operation `REMOTE_EX_UNKNOWN_OPERATION` and an
/// undeclared failure `REMOTE_EX_UNKNOWN_EXCEPTION`, the codec holds a choice
/// to exactly one member, the client raises an exception member it does not
/// know as `REMOTE_EX_UNKNOWN_EXCEPTION`, and the generated `Support` names the
/// interface.
#[test]
fn the_generated_java_service_and_client_answer_the_error_cases_of_the_clauses() {
    if !toolchain() {
        println!("no JDK on this machine — the Java Enhanced run time is not measured here");
        return;
    }
    let root = repository_root();
    let classes = compile(&root, "enhanced-runtime");
    if !(cfg!(target_os = "linux") || std::env::var("ZD_RUN_GENERATED").is_ok_and(|v| v == "1")) {
        println!(
            "compiled, not run: this host does not run generated programs (set ZD_RUN_GENERATED=1)"
        );
        return;
    }
    let output = run(
        Command::new("java")
            .arg("-Dorg.omg.dds.serviceClassName=org.zerodds.ServiceEnvironmentImpl")
            .arg("-cp")
            .arg(&classes)
            .arg("EnhancedRuntimeChecks")
            .arg(member_id("add").to_string()),
        "the Enhanced run-time program",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}

/// §7.7.2: two generated versions of one interface run against each other
/// over shared topic names: an operation one version lacks is answered
/// `REMOTE_EX_UNKNOWN_OPERATION`, the parameters of a `@Mutable` operation
/// follow their names when they are reordered, and an exception the client
/// does not know is `REMOTE_EX_UNKNOWN_EXCEPTION`.
#[test]
fn two_generated_java_versions_of_an_interface_run_against_each_other() {
    if !toolchain() {
        println!("no JDK on this machine — the Java evolution is not measured here");
        return;
    }
    let root = repository_root();
    let classes = compile(&root, "enhanced-evolution");
    if !(cfg!(target_os = "linux") || std::env::var("ZD_RUN_GENERATED").is_ok_and(|v| v == "1")) {
        println!(
            "compiled, not run: this host does not run generated programs (set ZD_RUN_GENERATED=1)"
        );
        return;
    }
    let output = run(
        Command::new("java")
            .arg("-Dorg.omg.dds.serviceClassName=org.zerodds.ServiceEnvironmentImpl")
            .arg("-cp")
            .arg(&classes)
            .arg("EvolutionChecks"),
        "the evolution program",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}
