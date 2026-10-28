// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeLookup service between the Rust runtime and the pure-Java RTPS
//! participant of `crates/java-omgdds`, over real RTPS between two
//! processes, both with the default encoding: CDR2_LE with XCDR2 (XTypes
//! 1.4 Beta 1 §7.6.3.3.3).
//!
//! - Java asks the Rust runtime for an object it registered (`getTypes`) and
//!   gets it back octet for octet.
//! - The Rust runtime asks the Java participant for an object the Java side
//!   holds and files it in its registry.
//!
//! The Java peer is `tests/java/TypeLookupPeer.java`, compiled with `javac`
//! from the sources of `runtime/java` and `crates/java-omgdds`. Needs a JDK:
//! absent, the test fails when `ZERODDS_REQUIRE_JDK=1` and otherwise says it
//! measured nothing. Runs on Linux, or with `ZD_RUN_GENERATED=1`.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::type_lookup::TypeLookupEncoding;
use zerodds_types::{MinimalTypeObject, PrimitiveKind, TypeIdentifier, TypeObject};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

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

fn runs_here() -> bool {
    cfg!(target_os = "linux") || std::env::var("ZD_RUN_GENERATED").is_ok_and(|v| v == "1")
}

/// The Java peer and what it needs, compiled from source.
fn compile() -> PathBuf {
    let classes = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("java-typelookup-peer");
    std::fs::create_dir_all(&classes).expect("the class directory");
    let out = Command::new("javac")
        .args(["-encoding", "UTF-8", "-nowarn"])
        .arg("-d")
        .arg(&classes)
        .arg("-sourcepath")
        .arg(format!(
            "{}:{}",
            root().join("runtime/java/src/main/java").display(),
            root()
                .join("crates/java-omgdds/java/src/main/java")
                .display(),
        ))
        .arg(root().join("crates/dcps/tests/java/TypeLookupPeer.java"))
        .output()
        .expect("javac");
    assert!(
        out.status.success(),
        "javac failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    classes
}

fn java(classes: &Path) -> Command {
    let mut command = Command::new("java");
    command
        .arg("-Xmx256m")
        .arg("-cp")
        .arg(classes)
        .arg("TypeLookupPeer")
        // The default of both sides: no setting reaches the Java peer.
        .env_remove("ZERODDS_WIRE_TYPE_LOOKUP_ENCODING");
    command
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

fn object(name: &str) -> (TypeObject, zerodds_types::EquivalenceHash, Vec<u8>) {
    let minimal = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type(name)
            .extensibility(Extensibility::Appendable)
            .member(
                "id",
                TypeIdentifier::Primitive(PrimitiveKind::UInt32),
                |m| m,
            )
            .member(
                "range",
                TypeIdentifier::Primitive(PrimitiveKind::Int64),
                |m| m,
            )
            .build_minimal(),
    );
    let hash = zerodds_types::compute_minimal_hash(&minimal).expect("a hash");
    let mut w = zerodds_types::framing::xcdr2_writer();
    TypeIdentifier::EquivalenceHashMinimal(hash)
        .encode_into(&mut w)
        .unwrap();
    (TypeObject::Minimal(minimal), hash, w.finish())
}

fn runtime(domain: i32, prefix: u8) -> std::sync::Arc<DcpsRuntime> {
    let config = RuntimeConfig::default();
    assert_eq!(config.type_lookup_encoding, TypeLookupEncoding::Xcdr2);
    DcpsRuntime::start(domain, GuidPrefix::from_bytes([prefix; 12]), config).expect("runtime")
}

#[test]
fn java_asks_the_rust_runtime_for_a_type_in_xcdr2() {
    if !toolchain() || !runs_here() {
        println!("no JDK or not Linux: nothing measured");
        return;
    }
    let classes = compile();
    let lease = zerodds_test_domains::lease_domain();
    let rt = runtime(lease.id(), 0x4a);
    let (object, _, identifier) = object("::tl::JavaAsks");
    rt.register_type_object(object.clone()).expect("register");
    let out = java(&classes)
        .args([
            "ask",
            &lease.id().to_string(),
            &hex(&rt.guid_prefix.0),
            &hex(&identifier),
        ])
        .output()
        .expect("java");
    rt.shutdown();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "the Java peer failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("ENCODING=XCDR2"), "{stdout}");
    let line = stdout
        .lines()
        .find_map(|line| line.strip_prefix("OBJECT="))
        .unwrap_or_else(|| panic!("no object in {stdout}"));
    assert_eq!(
        TypeObject::from_bytes_le(&unhex(line)).expect("an object"),
        object,
        "the object Java got is not the one the Rust runtime holds"
    );
}

#[test]
fn the_rust_runtime_asks_java_for_a_type_in_xcdr2() {
    if !toolchain() || !runs_here() {
        println!("no JDK or not Linux: nothing measured");
        return;
    }
    let classes = compile();
    let lease = zerodds_test_domains::lease_domain();
    let (object, hash, identifier) = object("::tl::RustAsks");
    let mut child = java(&classes)
        .args([
            "serve",
            &lease.id().to_string(),
            &hex(&identifier),
            &hex(&object.to_bytes_le().unwrap()),
            "25",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("java");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
    let mut prefix = None;
    let mut encoding = None;
    for line in lines.by_ref().take(4) {
        let line = line.expect("a line");
        if let Some(value) = line.strip_prefix("ENCODING=") {
            encoding = Some(value.to_string());
        }
        if let Some(value) = line.strip_prefix("PREFIX=") {
            prefix = Some(value.to_string());
            break;
        }
    }
    let prefix = prefix.expect("the Java peer's prefix");
    assert_eq!(encoding.as_deref(), Some("XCDR2"));
    let mut bytes = [0u8; 12];
    bytes.copy_from_slice(&unhex(&prefix));
    let java_prefix = GuidPrefix::from_bytes(bytes);

    let rt = runtime(lease.id(), 0x4b);
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut asked = false;
    let mut filed = false;
    while Instant::now() < deadline && !filed {
        if !asked {
            asked = rt
                .send_type_lookup_request(
                    java_prefix,
                    &[TypeIdentifier::EquivalenceHashMinimal(hash)],
                )
                .expect("the request")
                .is_some();
        }
        filed = rt
            .type_lookup_server
            .lock()
            .map(|server| server.registry.get_minimal(&hash).is_some())
            .unwrap_or(false);
        std::thread::sleep(Duration::from_millis(100));
    }
    let fetched = rt
        .type_lookup_server
        .lock()
        .ok()
        .and_then(|server| server.registry.get_minimal(&hash).cloned());
    rt.shutdown();
    let _ = child.kill();
    let status = child.wait();
    assert!(
        asked,
        "the Java participant was not discovered within 15 s ({status:?})"
    );
    assert_eq!(
        fetched.map(TypeObject::Minimal),
        Some(object),
        "the Rust runtime did not file the object the Java peer holds"
    );
}
