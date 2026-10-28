// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java language binding of DDS-RPC 1.0 §7.11.2, compiled and run.
//!
//! The counterpart of `cpp_rpc_runtime.rs`, and it measures the same
//! three things in the other column:
//!
//! 1. `runtime/java/src/main/java/org/omg/dds/rpc` compiles under
//!    `javac -Xlint:all -Werror`, together with the `ZdServices` class
//!    `crates/lang-rules/src/emit/interface_java.rs` generates for the
//!    same interface. A generated `<I>Requester` that extended something
//!    the runtime does not have would be found by `javac` at a user's
//!    site and by nothing before it.
//! 2. `crates/rpc/tests/java/RpcRuntimeRoundtrip.java` runs, and every
//!    type §7.11.2.2.1 and §7.11.2.3 enumerate is used in it.
//! 3. The two headers of §7.5.1.1.1 take, octet for octet, the layout
//!    `crates/rpc/src/wire_codec.rs` writes — which is also what the C++
//!    column writes, so the three agree by transitivity through one
//!    authority rather than pairwise.
//!
//! `org.omg.dds.core.ServiceEnvironment` comes from
//! `crates/java-omgdds`, which §7.11.2.2.3 makes `RPCRuntime` extend;
//! the compile reads that tree and writes nothing into it.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_rpc::common_types::{RemoteExceptionCode, ReplyHeader, RequestHeader, SampleIdentity};
use zerodds_rpc::wire_codec::encode_reply_frame;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The DDS Java PSM implementation, selected the way DDS-Java-PSM 1.0 §7.3.1 and
/// `ServiceEnvironment.createInstance` define: the system property
/// `org.omg.dds.serviceClassName` names the class. The programs make their
/// participants from it.
const SERVICE_CLASS: &str = "-Dorg.omg.dds.serviceClassName=org.zerodds.ServiceEnvironmentImpl";

/// Whether `javac` and `java` are both on this machine.
///
/// Absent, the two tests below print that they measured nothing and
/// pass, because `cargo test --workspace` runs in images without a JDK.
/// `ZERODDS_REQUIRE_JDK=1` turns the absence into a failure: the CI job
/// that installs a JDK sets it, so a JDK that went missing from its image
/// fails the job instead of turning it green over nothing.
fn toolchain() -> bool {
    let present = ["javac", "java"].iter().all(|tool| {
        Command::new(tool)
            .arg("-version")
            .output()
            .is_ok_and(|out| out.status.success())
    });
    assert!(
        present || std::env::var("ZERODDS_REQUIRE_JDK").as_deref() != Ok("1"),
        "ZERODDS_REQUIRE_JDK=1 and no `javac`/`java` on PATH"
    );
    present
}

/// The three files the generator writes for `source`: `ZdTypes`,
/// `ZdServices` and `ZdCodec`.
///
/// `generate::source` writes them as one text of three public top-level
/// classes, and Java allows one per file, so `generate::java_units` cuts
/// the text where the classes begin. The services reference the codec, so
/// all three are compiled.
fn generated_files(source: &str, origin: &str) -> (String, String, String) {
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), origin);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");

    // the Java programs of this test read the IDL-to-Java shape (public fields)
    let text = zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        || {
            zerodds_lang_rules::generate::source(
                &ast,
                zerodds_lang_cst::Language::Java,
                zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
            )
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let units = zerodds_lang_rules::generate::java_units(&text);
    let classes: Vec<&str> = units.iter().map(|unit| unit.class).collect();
    assert_eq!(classes, ["ZdTypes", "ZdServices", "ZdCodec"]);
    let [types, services_text, codec] = &units[..] else {
        panic!("three compilation units");
    };
    let codec_text = format!("import org.zerodds.wire.*;\n\n{}", codec.text.trim_end());
    (types.text.clone(), services_text.text.clone(), codec_text)
}

/// The generated files for the interface the Java program uses.
fn generated_services() -> (String, String, String) {
    generated_files(
        "
        module robot {
          interface Calculator {
            long add(in long a, in long b);
          };
        };
    ",
        "calculator.idl",
    )
}

/// One directory per test: `cargo test` runs them on threads, and two
/// `javac` invocations writing one class directory is a race.
fn build_directory(test: &str) -> PathBuf {
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dds-rpc-java")
        .join(test);
    std::fs::create_dir_all(&at).expect("the build directory");
    at
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

/// Compile the runtime package, the generated services and the
/// round-trip program into one class directory.
fn compile(root: &Path, test: &str) -> PathBuf {
    let at = build_directory(test);
    let classes = at.join("classes");
    // no class of an earlier run (another source layout) may stay in the directory
    let _ = std::fs::remove_dir_all(&classes);
    std::fs::create_dir_all(&classes).expect("the class directory");
    let (types, services, codec) = generated_services();
    std::fs::write(at.join("ZdTypes.java"), types).expect("ZdTypes");
    std::fs::write(at.join("ZdServices.java"), services).expect("ZdServices");
    std::fs::write(at.join("ZdCodec.java"), codec).expect("ZdCodec");

    let sourcepath = format!(
        "{}:{}:{}",
        root.join("runtime/java/src/main/java").display(),
        root.join("crates/java-omgdds/java/src/main/java").display(),
        at.display(),
    );
    // The generated files first, without `-Werror`: the generated codec is
    // held to the corpus gate of `lang-rules`, the runtime to the step
    // below.
    run(
        Command::new("javac")
            .args(["-encoding", "UTF-8"])
            .arg("-d")
            .arg(&classes)
            .args(["-Xlint:all"])
            .arg("-sourcepath")
            .arg(&sourcepath)
            .arg(at.join("ZdTypes.java"))
            .arg(at.join("ZdServices.java"))
            .arg(at.join("ZdCodec.java")),
        "compiling the generated files",
    );

    let mut javac = Command::new("javac");
    // The sources are UTF-8; `javac` without `-encoding` reads the
    // platform encoding, which is US-ASCII in the CI container.
    javac
        .args(["-encoding", "UTF-8"])
        .arg("-d")
        .arg(&classes)
        .arg("-cp")
        .arg(&classes)
        // `-Xlint:all -Werror`: a runtime that only compiles with the
        // warnings off is one a consumer cannot put in their build.
        .args(["-Xlint:all,-unchecked,-options", "-Werror"])
        .arg("-sourcepath")
        .arg(&sourcepath)
        .arg(root.join("crates/rpc/tests/java/RpcRuntimeRoundtrip.java"))
        // Uses the in-process binding of the round-trip program.
        .arg(root.join("crates/rpc/tests/java/RpcInheritanceTest.java"))
        .arg(root.join("crates/rpc/tests/java/SpecSignatureCheck.java"));
    // The package itself and the binding over the DDS Java PSM next to it.
    for package in [
        "runtime/java/src/main/java/org/omg/dds/rpc",
        "runtime/java/src/main/java/org/zerodds/ddsrpc",
    ] {
        for entry in std::fs::read_dir(root.join(package)).expect("the rpc package") {
            let path = entry.expect("a package entry").path();
            if path.extension().is_some_and(|ext| ext == "java") {
                javac.arg(path);
            }
        }
    }
    run(&mut javac, "compiling the Java binding");
    classes
}

/// §7.11.2.2.1 and §7.11.2.3: the whole binding, compiled and run.
#[test]
fn the_java_rpc_runtime_and_the_generated_services_run_together() {
    if !toolchain() {
        println!("no JDK on this machine — the Java binding is not measured here");
        return;
    }
    let root = repository_root();
    let classes = compile(&root, "roundtrip");
    let output = run(
        Command::new("java")
            .arg(SERVICE_CLASS)
            .arg("-cp")
            .arg(&classes)
            .arg("RpcRuntimeRoundtrip"),
        "the round-trip program",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}

/// §7.11.2.2.1 through §7.11.2.3: the entities of the binding are
/// interfaces, `Server` and `ClientEndpoint` inherit and give what the
/// machine-readable Java files give them, `Sample.Iterator` is closeable,
/// and the runtime makes services and clients from the generated
/// constructors.
#[test]
fn the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients() {
    if !toolchain() {
        println!("no JDK on this machine — the Java entities are not measured here");
        return;
    }
    let root = repository_root();
    let classes = compile(&root, "inheritance");
    let output = run(
        Command::new("java")
            .arg(SERVICE_CLASS)
            .arg("-cp")
            .arg(&classes)
            .arg("RpcInheritanceTest"),
        "the inheritance program",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}

/// §7.11.2: "The machine readable files associated with this specification
/// represent the normative language bindings for Java." Every public
/// declaration of `omg-dds-rpc-java.zip` — `omg-dds-rpc-java.signatures`,
/// which `extract_spec_signatures.py` wrote from it — is checked by
/// reflection against the runtime: the kind and supertypes of each type, the
/// parameter and return types of each method after erasure, the
/// constructors and the enum constants.
#[test]
fn the_java_binding_has_every_signature_of_the_machine_readable_files() {
    if !toolchain() {
        println!("no JDK on this machine — the signatures are not checked here");
        return;
    }
    let root = repository_root();
    let classes = compile(&root, "signatures");
    let output = Command::new("java")
        .arg("-cp")
        .arg(&classes)
        .arg("SpecSignatureCheck")
        .arg(root.join("crates/rpc/tests/java/omg-dds-rpc-java.signatures"))
        .output()
        .expect("java");
    let text = String::from_utf8_lossy(&output.stdout);
    println!("{text}");
    assert!(
        output.status.success() && text.contains("every signature held"),
        "the check said:\n{text}{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// §7.5.1.1.1: the Java header codec and `wire_codec.rs` write the same
/// octets.
#[test]
fn the_two_headers_take_the_same_octets_in_both_columns() {
    if !toolchain() {
        println!("no JDK on this machine — the header octets are not compared here");
        return;
    }
    let root = repository_root();
    let classes = compile(&root, "header-octets");
    let output = run(
        Command::new("java")
            .arg("-cp")
            .arg(&classes)
            .arg("RpcRuntimeRoundtrip")
            .arg("--header-octets"),
        "the header-octet dump",
    );
    let dumped: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(dumped.len(), 2, "one line per header: {dumped:?}");

    let identity = SampleIdentity::new([0x42; 16], 7);
    let request = RequestHeader::new(identity, "calc-A")
        .to_cdr_le()
        .expect("encode request");
    let reply = encode_reply_frame(
        &ReplyHeader::new(identity, RemoteExceptionCode::InvalidArgument),
        &[],
    );

    assert_eq!(dumped[0], hex(&request), "the RequestHeader octets differ");
    assert_eq!(dumped[1], hex(&reply), "the ReplyHeader octets differ");
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|octet| format!("{octet:02x}"))
        .collect::<String>()
}

/// The three files the generator writes for an interface with a data type,
/// an exception, an attribute, an `out` parameter and a base: `ZdTypes`,
/// `ZdServices` and `ZdCodec`.
///
/// `generate::source` writes them as one text of three public top-level
/// classes, and Java allows one per file, so `generate::java_units` cuts
/// the text where the classes begin.
fn function_call_files() -> (String, String, String) {
    let source = "
        module robot {
          exception Overflow { long code; };
          struct Pose { long x; long y; };
          interface Base {
            long ping();
          };
          interface Calculator : Base {
            readonly attribute long total;
            attribute string label;
            long add(in long a, in long b) raises (Overflow);
            long divmod(in long a, in long b, out long remainder);
            Pose locate(in string room);
            void reset();
            double scale(in double value);
          };
        };
    ";
    generated_files(source, "function_call.idl")
}

/// §7.2.2.3, §7.9.1, §7.11.1.5 and §7.11.2.3: the generated function-call
/// client calls the generated service, over the DDS Java PSM, with the
/// generated codec, under classic CDR (the default) and under XCDR2.
#[test]
fn the_generated_function_call_client_calls_the_generated_service() {
    if !toolchain() {
        println!("no JDK on this machine — the function-call binding is not measured here");
        return;
    }
    let root = repository_root();
    let at = build_directory("function-call");
    let classes = at.join("classes");
    // no class of an earlier run (another source layout) may stay in the directory
    let _ = std::fs::remove_dir_all(&classes);
    std::fs::create_dir_all(&classes).expect("the class directory");
    let (types, services, codec) = function_call_files();
    std::fs::write(at.join("ZdTypes.java"), types).expect("ZdTypes");
    std::fs::write(at.join("ZdServices.java"), services).expect("ZdServices");
    std::fs::write(at.join("ZdCodec.java"), codec).expect("ZdCodec");

    let mut javac = Command::new("javac");
    javac
        .args(["-encoding", "UTF-8"])
        .arg("-d")
        .arg(&classes)
        // No `-Werror`: the generated codec is held to the corpus gate of
        // `lang-rules`, the runtime to the other test of this file.
        .args(["-Xlint:all"])
        .arg("-sourcepath")
        .arg(format!(
            "{}:{}:{}",
            root.join("runtime/java/src/main/java").display(),
            root.join("crates/java-omgdds/java/src/main/java").display(),
            at.display(),
        ))
        .arg(at.join("ZdTypes.java"))
        .arg(at.join("ZdServices.java"))
        .arg(at.join("ZdCodec.java"))
        .arg(root.join("crates/rpc/tests/java/FunctionCallRoundtrip.java"));
    for package in [
        "runtime/java/src/main/java/org/omg/dds/rpc",
        "runtime/java/src/main/java/org/zerodds/ddsrpc",
    ] {
        for entry in std::fs::read_dir(root.join(package)).expect("the rpc package") {
            let path = entry.expect("a package entry").path();
            if path.extension().is_some_and(|ext| ext == "java") {
                javac.arg(path);
            }
        }
    }
    run(&mut javac, "compiling the generated function-call binding");
    let output = run(
        Command::new("java")
            .arg(SERVICE_CLASS)
            .arg("-cp")
            .arg(&classes)
            .arg("FunctionCallRoundtrip"),
        "the function-call round trip",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}
