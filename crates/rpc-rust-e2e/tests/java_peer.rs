// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java binding of DDS-RPC 1.0 against the Rust binding, over real RTPS
//! between two processes: a client in Java calls a service in Rust, and a
//! client in Rust calls a service in Java (§7.5.1.2, §7.6.2). Both sides
//! announce the TypeInformation of their topic types and the discovery
//! extension.
//!
//! The Java PSM pairs the endpoints of the Enhanced mapping as the Rust one
//! does: over the `topic_aliases` of the peer (§7.4.3, §7.6.2.1) and by the
//! assignability of the announced types (§7.5.1.2.6, XTypes 1.3 §7.6.3.4.2),
//! not by the type name. A client of the base interface reaches a service of
//! the derived one in both directions; a changed parameter type (§7.7.2.5.3)
//! and a return type the old one is not assignable to (§7.7.2.5.5) are not
//! matched, by the Java side; a return type with a member appended to a
//! `@mutable` structure is matched and its reply read. The interfaces of the
//! evolution cells are those of `build.rs` (`EVOLUTION_*`), for the Java side
//! given here again.
//!
//! Needs a JDK: absent, the test fails when `ZERODDS_REQUIRE_JDK=1` and
//! otherwise says it measured nothing. Runs on Linux, or with
//! `ZD_RUN_GENERATED=1`.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zerodds_dcps::dds_type::DdsType;
use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::Replier;
use zerodds_rpc_rust_e2e::enhanced_bindings::enh::{
    Base2, Base2Client, Base2Error, Derived2, Derived2Client, Derived2Dispatcher, Derived2Error,
};
use zerodds_types::type_lookup::TypeLookupEncoding;

const INTERFACE: &str = "
    module enh {
      interface Base2 { long ident(); };
      interface Derived2 : Base2 { long more(); };
    };
";

/// `EVOLUTION_OLD` of `build.rs`: the version the Java client has.
const EVOLUTION_OLD: &str = "
module evo {
  exception Overflow { long limit; };
  @DDSService interface Svc {
    long add(in long a, in long b);
    long scale(in long k) raises (Overflow);
    @Mutable long mix(in long x, in long y);
    long legacy();
  };
};
";

/// `EVOLUTION_POSE_OLD` of `build.rs`: the version the Java client has.
const EVOLUTION_POSE_OLD: &str = "
module shapes {
  @mutable struct Pose { long x; };
  @DDSService interface Svc {
    long length(in Pose p);
    Pose origin();
  };
};
";

const SERVICE_CLASS: &str = "-Dorg.omg.dds.serviceClassName=org.zerodds.ServiceEnvironmentImpl";

/// How long a Java client of a negative cell waits for the service, in
/// seconds: the endpoints of the peer are discovered and its types fetched
/// well inside it.
const NO_MATCH_WAIT: &str = "8";

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

/// The Java peer `peer` (a file of `tests/java`) with the generated binding
/// of `idl`, compiled once per name for all the tests of this binary.
fn compile(name: &str, idl: &str, peer: &str) -> PathBuf {
    static COMPILED: Mutex<BTreeMap<String, PathBuf>> = Mutex::new(BTreeMap::new());
    // A panic of one compilation (a test that failed) leaves the map as it was.
    let mut compiled = COMPILED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(classes) = compiled.get(name) {
        return classes.clone();
    }
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), &format!("{name}.idl"));
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let text = zerodds_lang_rules::generate::source_with_mapping(
        &ast,
        zerodds_lang_cst::Language::Java,
        zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
        zerodds_lang_rules::emit::encode::CodecOptions::default(),
        zerodds_lang_rules::generate::ServiceMapping::Enhanced,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dds-rpc-java-peer")
        .join(name);
    let classes = at.join("classes");
    std::fs::create_dir_all(&classes).expect("the class directory");
    let mut files = Vec::new();
    for unit in zerodds_lang_rules::generate::java_units(&text) {
        let body = if unit.class == "ZdCodec" {
            format!("import org.zerodds.wire.*;\n\n{}", unit.text.trim_end())
        } else {
            unit.text.clone()
        };
        let file = at.join(format!("{}.java", unit.class));
        std::fs::write(&file, body).expect("a generated unit");
        files.push(file);
    }
    files.push(root().join("crates/rpc-rust-e2e/tests/java").join(peer));
    for package in [
        "runtime/java/src/main/java/org/omg/dds/rpc",
        "runtime/java/src/main/java/org/zerodds/ddsrpc",
    ] {
        for entry in std::fs::read_dir(root().join(package)).expect("the rpc package") {
            let path = entry.expect("an entry").path();
            if path.extension().is_some_and(|ext| ext == "java") {
                files.push(path);
            }
        }
    }
    let out = Command::new("javac")
        .args(["-encoding", "UTF-8", "-Xlint:all"])
        .arg("-d")
        .arg(&classes)
        .arg("-sourcepath")
        .arg(format!(
            "{}:{}:{}",
            root().join("runtime/java/src/main/java").display(),
            root()
                .join("crates/java-omgdds/java/src/main/java")
                .display(),
            at.display()
        ))
        .args(&files)
        .output()
        .expect("javac");
    assert!(
        out.status.success(),
        "javac failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    compiled.insert(name.to_owned(), classes.clone());
    classes
}

fn java(classes: &Path, main: &str) -> Command {
    java_with(classes, main, None)
}

/// [`java`], with `wire.type_lookup_encoding` set to `encoding` when given.
fn java_with(classes: &Path, main: &str, encoding: Option<&str>) -> Command {
    let mut command = Command::new("java");
    // The PSM matches over RTPS (another process) only when asked to.
    command.arg("-Dzerodds.rtps.enable=true");
    if let Some(encoding) = encoding {
        command.arg(format!("-Dzerodds.wire.type_lookup_encoding={encoding}"));
    }
    command.arg(SERVICE_CLASS).arg("-cp").arg(classes).arg(main);
    command
}

fn said(output: &Output) -> String {
    format!(
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A participant whose TypeLookup service writes `encoding` (`wire.type_lookup_encoding`).
fn participant_with(domain: i32, encoding: TypeLookupEncoding) -> DomainParticipant {
    let config = zerodds_dcps::runtime::RuntimeConfig {
        type_lookup_encoding: encoding,
        ..zerodds_dcps::runtime::RuntimeConfig::default()
    };
    DomainParticipantFactory::instance()
        .create_participant_with_config(domain, DomainParticipantQos::default(), config)
        .expect("a participant")
}

fn participant(domain: i32) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .expect("a participant")
}

struct Impl;
impl Base2 for Impl {
    fn ident(&self) -> Result<i32, Base2Error> {
        Ok(41)
    }
}
impl Derived2 for Impl {
    fn more(&self) -> Result<i32, Derived2Error> {
        Ok(42)
    }
}

struct Ticking {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Ticking {
    fn spawn<TIn: DdsType + Send + 'static, TOut: DdsType + Send + 'static>(
        replier: Replier<TIn, TOut>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                let _ = replier.tick();
                thread::sleep(Duration::from_millis(2));
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for Ticking {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// A Java service of `Derived2`, until the returned child's input closes.
fn java_service(classes: &Path, domain: i32) -> std::process::Child {
    java_service_with(classes, domain, None)
}

/// [`java_service`], with `wire.type_lookup_encoding` set to `encoding` when given.
fn java_service_with(classes: &Path, domain: i32, encoding: Option<&str>) -> std::process::Child {
    let mut child = java_with(classes, "JavaPeer", encoding)
        .args(["service", &domain.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("java");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout"));
    let mut first = String::new();
    lines.read_line(&mut first).expect("a line");
    assert!(first.contains("serving"), "the Java service said: {first}");
    child
}

/// A Java client of `Derived2` calls the Rust service of `Derived2`.
#[test]
fn a_java_client_calls_a_rust_service() {
    if !toolchain() || !runs_here() {
        println!("no JDK, or not a host that runs generated programs: not measured");
        return;
    }
    let classes = compile("enh", INTERFACE, "JavaPeer.java");
    let lease = zerodds_test_domains::lease_domain();
    let participant = participant(lease.id());
    let qos = RpcQos::default_enhanced();
    let service = Derived2Dispatcher::new(Impl)
        .serve(&participant, &qos)
        .expect("the Rust service");
    let _service = Ticking::spawn(service);
    let output = java(&classes, "JavaPeer")
        .args(["client", &lease.id().to_string()])
        .output()
        .expect("java");
    assert!(
        output.status.success() && said(&output).contains("more=42"),
        "the Java client said: {}",
        said(&output)
    );
}

/// A Rust client of `Derived2` calls the Java service of `Derived2`.
#[test]
fn a_rust_client_calls_a_java_service() {
    if !toolchain() || !runs_here() {
        println!("no JDK, or not a host that runs generated programs: not measured");
        return;
    }
    let classes = compile("enh", INTERFACE, "JavaPeer.java");
    let lease = zerodds_test_domains::lease_domain();
    let mut child = java_service(&classes, lease.id());
    let participant = participant(lease.id());
    let qos = RpcQos::default_enhanced();
    let client = Derived2Client::new(&participant, &qos).expect("the Rust client");
    let result = client.more();
    drop(child.stdin.take());
    let _ = child.wait();
    assert_eq!(result.expect("more"), 42);
}

/// §7.4.3, §7.5.1.2.6: a Java client of `Base2` calls the Rust service of
/// `Derived2`. The request and reply types of the two interfaces have other
/// names; the Java endpoints pair with the service's over the aliases it
/// announces and the assignability of the types they fetch from it.
#[test]
fn a_java_client_of_the_base_calls_a_rust_service_of_the_derived_interface() {
    if !toolchain() || !runs_here() {
        println!("no JDK, or not a host that runs generated programs: not measured");
        return;
    }
    let classes = compile("enh", INTERFACE, "JavaPeer.java");
    let lease = zerodds_test_domains::lease_domain();
    let participant = participant(lease.id());
    let qos = RpcQos::default_enhanced();
    let service = Derived2Dispatcher::new(Impl)
        .serve(&participant, &qos)
        .expect("the Rust service");
    let _service = Ticking::spawn(service);
    let output = java(&classes, "JavaPeer")
        .args(["client-base", &lease.id().to_string()])
        .output()
        .expect("java");
    assert!(
        output.status.success() && said(&output).contains("ident=41"),
        "the Java client said: {}",
        said(&output)
    );
}

/// The other direction: a Rust client of `Base2` calls the Java service of
/// `Derived2`, whose endpoints pair with the client's over their own
/// aliases and the types they fetch from the client.
#[test]
fn a_rust_client_of_the_base_calls_a_java_service_of_the_derived_interface() {
    if !toolchain() || !runs_here() {
        println!("no JDK, or not a host that runs generated programs: not measured");
        return;
    }
    let classes = compile("enh", INTERFACE, "JavaPeer.java");
    let lease = zerodds_test_domains::lease_domain();
    let mut child = java_service(&classes, lease.id());
    let participant = participant(lease.id());
    let qos = RpcQos::default_enhanced();
    let client = Base2Client::new(&participant, &qos).expect("the Rust client");
    let result = client.ident();
    drop(child.stdin.take());
    let _ = child.wait();
    assert_eq!(result.expect("ident"), 41);
}

/// The two encodings of the TypeLookup service (XTypes 1.4 Beta 1 §7.6.3.3.3, DDSI-RTPS 2.5
/// §10.4) on the way the Java endpoints fetch the types of the peer: the base and the derived
/// interface pair only after both sides fetched the other's objects. The Java side asks in CDR2_LE
/// (`xcdr2`, the default) and reads the CDR_LE replies of the Rust side (`xcdr1`), then asks in
/// CDR_LE and reads CDR2_LE replies; the Rust side reads what the Java side asks in either.
#[test]
fn a_java_client_of_the_base_pairs_over_both_type_lookup_encodings() {
    if !toolchain() || !runs_here() {
        println!("no JDK, or not a host that runs generated programs: not measured");
        return;
    }
    let classes = compile("enh", INTERFACE, "JavaPeer.java");
    for (java, rust) in [
        ("xcdr2", TypeLookupEncoding::Xcdr1),
        ("xcdr1", TypeLookupEncoding::Xcdr2),
    ] {
        let lease = zerodds_test_domains::lease_domain();
        let participant = participant_with(lease.id(), rust);
        let qos = RpcQos::default_enhanced();
        let service = Derived2Dispatcher::new(Impl)
            .serve(&participant, &qos)
            .expect("the Rust service");
        let _service = Ticking::spawn(service);
        let output = java_with(&classes, "JavaPeer", Some(java))
            .args(["client-base", &lease.id().to_string()])
            .output()
            .expect("java");
        assert!(
            output.status.success() && said(&output).contains("ident=41"),
            "Java {java}, Rust {rust:?}: the Java client said: {}",
            said(&output)
        );
    }
}

/// The other direction of [`a_java_client_of_the_base_pairs_over_both_type_lookup_encodings`]:
/// a Rust client of `Base2` and the Java service of `Derived2`.
#[test]
fn a_rust_client_of_the_base_pairs_with_a_java_service_over_both_type_lookup_encodings() {
    if !toolchain() || !runs_here() {
        println!("no JDK, or not a host that runs generated programs: not measured");
        return;
    }
    let classes = compile("enh", INTERFACE, "JavaPeer.java");
    for (java, rust) in [
        ("xcdr2", TypeLookupEncoding::Xcdr1),
        ("xcdr1", TypeLookupEncoding::Xcdr2),
    ] {
        let lease = zerodds_test_domains::lease_domain();
        let mut child = java_service_with(&classes, lease.id(), Some(java));
        let participant = participant_with(lease.id(), rust);
        let qos = RpcQos::default_enhanced();
        let client = Base2Client::new(&participant, &qos).expect("the Rust client");
        let result = client.ident();
        drop(child.stdin.take());
        let _ = child.wait();
        assert_eq!(result.expect("ident"), 41, "Java {java}, Rust {rust:?}");
    }
}

mod versions {
    use super::*;
    use zerodds_rpc_rust_e2e::evolution_changed::evo as changed;
    use zerodds_rpc_rust_e2e::evolution_old::evo as old;
    use zerodds_rpc_rust_e2e::evolution_return::evo as returned;

    struct OldService;

    impl old::Svc for OldService {
        fn add(&self, a: i32, b: i32) -> Result<i32, old::SvcError> {
            Ok(a + b)
        }
        fn scale(&self, k: i32) -> Result<i32, old::SvcError> {
            Ok(k)
        }
        fn mix(&self, x: i32, y: i32) -> Result<i32, old::SvcError> {
            Ok(x + y)
        }
        fn legacy(&self) -> Result<i32, old::SvcError> {
            Ok(7)
        }
    }

    struct ChangedService {
        calls: Arc<AtomicUsize>,
    }

    impl changed::Svc for ChangedService {
        fn add(&self, _a: String, b: i32) -> Result<i32, changed::SvcError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(b)
        }
        fn scale(&self, k: i32) -> Result<i32, changed::SvcError> {
            Ok(k)
        }
        fn mix(&self, x: i32, y: i32) -> Result<i32, changed::SvcError> {
            Ok(x + y)
        }
        fn legacy(&self) -> Result<i32, changed::SvcError> {
            Ok(7)
        }
    }

    struct ReturnedService;

    impl returned::Svc for ReturnedService {
        fn add(&self, a: i32, b: i32) -> Result<i32, returned::SvcError> {
            Ok(a + b)
        }
        fn scale(&self, k: i32) -> Result<i64, returned::SvcError> {
            Ok(i64::from(k))
        }
        fn mix(&self, x: i32, y: i32) -> Result<i32, returned::SvcError> {
            Ok(x + y)
        }
        fn legacy(&self) -> Result<i32, returned::SvcError> {
            Ok(7)
        }
    }

    fn java_client(domain: i32, wait: &str) -> Output {
        let classes = compile("evo", EVOLUTION_OLD, "JavaEvolutionPeer.java");
        java(&classes, "JavaEvolutionPeer")
            .args(["client", &domain.to_string(), wait])
            .output()
            .expect("java")
    }

    /// The control of the two cells below: the Java client and the Rust
    /// service of the same version are matched over this setup, and the
    /// call is answered.
    #[test]
    fn an_old_java_client_calls_an_old_rust_service() {
        if !toolchain() || !runs_here() {
            println!("no JDK, or not a host that runs generated programs: not measured");
            return;
        }
        let lease = zerodds_test_domains::lease_domain();
        let participant = participant(lease.id());
        let qos = RpcQos::default_enhanced();
        let service = old::SvcDispatcher::new(OldService)
            .serve(&participant, &qos)
            .expect("the old service");
        let _service = Ticking::spawn(service);
        let output = java_client(lease.id(), "20");
        assert!(
            output.status.success() && said(&output).contains("add=3"),
            "the Java client said: {}",
            said(&output)
        );
    }

    /// §7.7.2.5.3: the type of a parameter changed (`long` to `string`).
    /// The request type of the Java client is not assignable to the
    /// service's, and the Java client does not match the service; matched
    /// by type name it would send the request to a service that cannot
    /// read it.
    #[test]
    fn a_changed_parameter_type_does_not_match_the_java_client() {
        if !toolchain() || !runs_here() {
            println!("no JDK, or not a host that runs generated programs: not measured");
            return;
        }
        let lease = zerodds_test_domains::lease_domain();
        let participant = participant(lease.id());
        let qos = RpcQos::default_enhanced();
        let calls = Arc::new(AtomicUsize::new(0));
        let service = changed::SvcDispatcher::new(ChangedService {
            calls: Arc::clone(&calls),
        })
        .serve(&participant, &qos)
        .expect("the changed service");
        let _service = Ticking::spawn(service);
        let output = java_client(lease.id(), NO_MATCH_WAIT);
        assert!(
            said(&output).contains("no service matched"),
            "the Java client said: {}",
            said(&output)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0, "the service saw a request");
    }

    /// §7.7.2.5.5: the return type of `scale` changed from `long` to
    /// `long long`, which the old reply type is not assignable from: the
    /// reply reader of the Java client does not match the service's writer.
    #[test]
    fn a_return_type_the_old_one_is_not_assignable_to_does_not_match_the_java_client() {
        if !toolchain() || !runs_here() {
            println!("no JDK, or not a host that runs generated programs: not measured");
            return;
        }
        let lease = zerodds_test_domains::lease_domain();
        let participant = participant(lease.id());
        let qos = RpcQos::default_enhanced();
        let service = returned::SvcDispatcher::new(ReturnedService)
            .serve(&participant, &qos)
            .expect("the changed service");
        let _service = Ticking::spawn(service);
        let output = java_client(lease.id(), NO_MATCH_WAIT);
        assert!(
            said(&output).contains("no service matched"),
            "the Java client said: {}",
            said(&output)
        );
    }
}

mod appended_member {
    use super::*;
    use zerodds_rpc_rust_e2e::evolution_pose_new::shapes as new;

    struct NewService;

    impl new::Svc for NewService {
        fn length(&self, p: new::Pose) -> Result<i32, new::SvcError> {
            Ok(p.x + p.y)
        }
        fn origin(&self) -> Result<new::Pose, new::SvcError> {
            Ok(new::Pose { x: 3, y: 4 })
        }
    }

    /// §7.7.2.5.3 and §7.7.2.5.5: `Pose` of the service has a member the
    /// Java client's does not. The types are assignable (XTypes 1.3
    /// §7.2.4.4, `@mutable`), the Java client is matched, its `Pose`
    /// argument reaches the service with the new member at its default, and
    /// it reads the reply with the member it does not know skipped.
    #[test]
    fn a_member_appended_to_a_mutable_structure_matches_the_java_client() {
        if !toolchain() || !runs_here() {
            println!("no JDK, or not a host that runs generated programs: not measured");
            return;
        }
        let lease = zerodds_test_domains::lease_domain();
        let participant = participant(lease.id());
        let qos = RpcQos::default_enhanced();
        let service = new::SvcDispatcher::new(NewService)
            .serve(&participant, &qos)
            .expect("the new service");
        let _service = Ticking::spawn(service);
        let classes = compile("shapes", EVOLUTION_POSE_OLD, "JavaPosePeer.java");
        let output = java(&classes, "JavaPosePeer")
            .args(["client", &lease.id().to_string()])
            .output()
            .expect("java");
        let text = said(&output);
        assert!(
            output.status.success() && text.contains("origin.x=3") && text.contains("length=5"),
            "the Java client said: {text}"
        );
    }
}
