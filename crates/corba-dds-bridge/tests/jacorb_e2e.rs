// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon against JacORB 3.9 (`zerodds-corba-bridge-1.0` §10,
//! §12.3). The Java programs in `tests/jacorb/` are compiled against the IDL
//! of `tests/omniorb/Calc.idl` with JacORB's own IDL compiler; the bridge and
//! a DDS backend run in this process.
//!
//! * CORBA → DDS and DDS → CORBA as in `omniorb_e2e.rs`.
//! * CSIv2: JacORB's SAS interceptor sends GSSUP credentials set through
//!   `GssUpContext`; the bridge accepts `alice`/`secret`, the ACL lets her
//!   through, a wrong password and a missing context do not get through.
//! * NameService, both ways: the bridge rebinds into JacORB's NameServer and
//!   unbinds on stop; a JacORB client resolves the bridge's object through
//!   `zerodds-naming-svc`.
//!
//! JacORB 3.9 runs on Java 8 (it replaces the JDK's CORBA classes through
//! `java.endorsed.dirs`). `JACORB_HOME` (default `/opt/jacorb`) and
//! `JACORB_JAVA_HOME` (default `/opt/jdk8`) name the installations; a
//! missing piece fails the test.
//!
//! ```bash
//! cargo test -p zerodds-corba-dds-bridge --test jacorb_e2e -- --ignored --test-threads=1
//! ```

#![cfg(feature = "std")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

mod common;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::{Bridge, CalcBackend, ChildGuard};

fn jacorb_home() -> PathBuf {
    std::env::var_os("JACORB_HOME").map_or_else(|| PathBuf::from("/opt/jacorb"), PathBuf::from)
}

fn java_home() -> PathBuf {
    std::env::var_os("JACORB_JAVA_HOME").map_or_else(|| PathBuf::from("/opt/jdk8"), PathBuf::from)
}

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/jacorb")
}

/// A fresh directory under the test target directory.
fn workdir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// JacORB's IDL compiler and javac: every class of `tests/jacorb/` under
/// `<dir>/classes`.
fn build(dir: &Path) -> PathBuf {
    let lib = jacorb_home().join("lib");
    let generated = dir.join("gen");
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).unwrap();
    common::build_step(
        Command::new(java_home().join("bin/java"))
            .arg("-cp")
            .arg(format!(
                "{}:{}",
                lib.join("idl.jar").display(),
                lib.join("antlr-2.7.2.jar").display()
            ))
            .arg("org.jacorb.idl.parser")
            .arg("-d")
            .arg(&generated)
            .arg(common::calc_idl()),
    );
    let mut sources: Vec<PathBuf> = std::fs::read_dir(generated.join("zerodds_test"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    for f in [
        "CalcClient",
        "ListenerServer",
        "GssupClient",
        "NamingClient",
    ] {
        sources.push(src().join(format!("{f}.java")));
    }
    common::build_step(
        Command::new(java_home().join("bin/javac"))
            .arg("-nowarn")
            .arg("-endorseddirs")
            .arg(&lib)
            .arg("-cp")
            .arg(format!("{}/*", lib.display()))
            .arg("-d")
            .arg(&classes)
            .args(&sources),
    );
    classes
}

/// `java <class> <args>` the way JacORB's `jaco` starts it, plus `props` as
/// system properties.
fn jaco(classes: &Path, class: &str, props: &[(&str, &str)]) -> Command {
    let home = jacorb_home();
    let mut cmd = Command::new(java_home().join("bin/java"));
    cmd.arg(format!(
        "-Djava.endorsed.dirs={}",
        home.join("lib").display()
    ))
    .arg(format!("-Djacorb.home={}", home.display()))
    .arg("-Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB")
    .arg("-Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton")
    // A lost reply fails the call instead of hanging the test.
    .arg("-Djacorb.connection.client.pending_reply_timeout=15000");
    for (k, v) in props {
        cmd.arg(format!("-D{k}={v}"));
    }
    cmd.arg("-cp").arg(classes).arg(class);
    cmd
}

#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (Linux test host)"]
fn jacorb_calls_reach_a_dds_backend_and_dds_samples_reach_a_jacorb_servant() {
    let dir = workdir("jacorb-e2e");
    let classes = build(&dir);

    let lease = common::lease();
    let iiop_port = common::free_port();
    let listener_port = common::free_port();
    let ior_file = dir.join("bridge.ior");
    let yaml = format!(
        "domain: {d}\n\
         corba:\n  iiop:\n    bind: \"127.0.0.1:{iiop_port}\"\n    fragment_size: 16384\n  ior:\n    ior_file: \"{ior}\"\n{m}",
        d = lease.id(),
        ior = ior_file.display(),
        m = common::calc_mappings(Some(listener_port)),
    );
    let bridge = Bridge::start(&yaml, &ior_file);
    let backend = CalcBackend::start(lease.id());
    backend.settle();

    let client = common::output(jaco(&classes, "CalcClient", &[]).arg(&ior_file));

    let mut server = jaco(&classes, "ListenerServer", &[])
        .args([listener_port.to_string(), "3".into()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("ListenerServer");
    let mut lines = BufReader::new(server.stdout.take().unwrap()).lines();
    assert_eq!(lines.next().unwrap().unwrap(), "ready");
    backend.notify(&[7, 8, 9]);
    let received: Vec<String> = lines.map_while(Result::ok).collect();
    let _ = server.wait();

    bridge.stop();
    drop(backend);

    let lines: Vec<&str> = client.lines().collect();
    assert_eq!(
        lines,
        vec![
            "add 42",
            "never EXCEPTION TIMEOUT",
            "wrong EXCEPTION BAD_OPERATION",
            "echo 200000 same"
        ],
        "{client}"
    );
    assert_eq!(received, vec!["on_value 7", "on_value 8", "on_value 9"]);
}

/// CSIv2 GSSUP from JacORB's SAS interceptor (CORBA 3.3 Part 2 §26.2). The
/// bridge's IOR carries `TAG_CSI_SEC_MECH_LIST`; the ACL allows only
/// `gssup:alice`. `never` has no backend, so a call that gets through ends
/// in `TIMEOUT`.
#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (Linux test host)"]
fn jacorb_gssup_credentials_pass_csiv2_and_the_acl() {
    let dir = workdir("jacorb-gssup");
    let classes = build(&dir);

    let pw_file = dir.join("gssup.passwd");
    std::fs::write(
        &pw_file,
        zerodds_corba_csiv2::gssup_store::format_entry("alice", "secret", b"jacorb-salt", 1000),
    )
    .unwrap();
    let lease = common::lease();
    let iiop_port = common::free_port();
    let ior_file = dir.join("bridge.ior");
    let yaml = format!(
        "domain: {d}\n\
         corba:\n  iiop:\n    bind: \"127.0.0.1:{iiop_port}\"\n  ior:\n    ior_file: \"{ior}\"\n  csiv2:\n    enabled: true\n    gssup:\n      password_file: \"{pw}\"\n\
         acl:\n  default_deny: true\n  rules:\n    - subject: \"gssup:alice\"\n      allow_invoke: [\"*\"]\n{m}",
        d = lease.id(),
        ior = ior_file.display(),
        pw = pw_file.display(),
        m = common::calc_mappings(None),
    );
    let bridge = Bridge::start(&yaml, &ior_file);
    let sas = [
        (
            "jacorb.security.sas.contextClass",
            "org.jacorb.security.sas.GssUpContext",
        ),
        (
            "org.omg.PortableInterceptor.ORBInitializerClass.SAS",
            "org.jacorb.security.sas.SASInitializer",
        ),
        (
            "org.omg.PortableInterceptor.ORBInitializerClass.GSSUPProvider",
            "org.jacorb.security.sas.GSSUPProviderInitializer",
        ),
    ];
    let gssup = |user: &str, password: &str| {
        common::output(
            jaco(&classes, "GssupClient", &sas)
                .arg(&ior_file)
                .args([user, password]),
        )
        .trim()
        .to_string()
    };
    let alice = gssup("alice", "secret");
    let wrong = gssup("alice", "wrong");
    let no_context = common::output(jaco(&classes, "CalcClient", &[]).arg(&ior_file));
    bridge.stop();

    assert_eq!(alice, "never EXCEPTION TIMEOUT", "alice passes");
    assert_eq!(
        wrong, "never EXCEPTION NO_PERMISSION",
        "a wrong password does not"
    );
    assert_eq!(
        no_context.lines().nth(1),
        Some("never EXCEPTION NO_PERMISSION"),
        "no SAS context: the ACL refuses\n{no_context}"
    );
}

/// JacORB's NameServer as the bridge's rebind target: a JacORB client
/// resolves `Calc::add` through it and calls it; after a clean stop the name
/// is gone.
#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (Linux test host)"]
fn the_bridge_rebinds_into_jacorbs_name_server_and_unbinds_on_stop() {
    let dir = workdir("jacorb-ns");
    let classes = build(&dir);

    let ns_port = common::free_port();
    let _ns = ChildGuard(
        jaco(
            &classes,
            "org.jacorb.naming.NameServer",
            &[
                ("OAPort", &ns_port.to_string()),
                ("OAIAddr", "127.0.0.1"),
                (
                    "jacorb.naming.ior_filename",
                    &dir.join("ns.ior").display().to_string(),
                ),
            ],
        )
        // The NameServer keeps its naming database (`_nsdb_root`) in its
        // working directory.
        .current_dir(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("JacORB NameServer"),
    );
    common::wait_for_listener(ns_port, "the JacORB NameServer");

    let lease = common::lease();
    let iiop_port = common::free_port();
    let ior_file = dir.join("bridge.ior");
    let yaml = format!(
        "domain: {d}\n\
         corba:\n  iiop:\n    bind: \"127.0.0.1:{iiop_port}\"\n  ior:\n    ior_file: \"{ior}\"\n    naming_service: \"corbaloc::127.0.0.1:{ns_port}/NameService\"\n    publish_to_naming: true\n    naming_retry_ms: 8000\n{m}",
        d = lease.id(),
        ior = ior_file.display(),
        m = common::calc_mappings(None),
    );
    let bridge = Bridge::start(&yaml, &ior_file);
    let backend = CalcBackend::start(lease.id());
    backend.settle();
    let url = format!("corbaname::127.0.0.1:{ns_port}#Calc::add");
    let resolved = common::output(jaco(&classes, "NamingClient", &[]).arg(&url));
    bridge.stop();
    let after = common::output(jaco(&classes, "NamingClient", &[]).arg(&url));
    drop(backend);

    assert_eq!(resolved.trim(), "add 42");
    assert!(
        after.trim().starts_with("add EXCEPTION"),
        "a clean stop unbinds the name: {after}"
    );
}

/// A JacORB client resolves the bridge's object through `zerodds-naming-svc`:
/// JacORB narrows the root context with a remote `_is_a`, then resolves.
#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (Linux test host)"]
fn jacorb_resolves_the_bridge_through_zerodds_naming_svc() {
    let dir = workdir("jacorb-zd-naming");
    let classes = build(&dir);

    let ns_port = common::free_port();
    let _ns = common::start_naming_svc(ns_port);
    let lease = common::lease();
    let iiop_port = common::free_port();
    let ior_file = dir.join("bridge.ior");
    let yaml = format!(
        "domain: {d}\n\
         corba:\n  iiop:\n    bind: \"127.0.0.1:{iiop_port}\"\n  ior:\n    ior_file: \"{ior}\"\n    naming_service: \"corbaloc::127.0.0.1:{ns_port}/NameService\"\n    publish_to_naming: true\n    naming_retry_ms: 8000\n{m}",
        d = lease.id(),
        ior = ior_file.display(),
        m = common::calc_mappings(None),
    );
    let bridge = Bridge::start(&yaml, &ior_file);
    let backend = CalcBackend::start(lease.id());
    backend.settle();
    let resolved = common::output(
        jaco(&classes, "NamingClient", &[])
            .arg(format!("corbaname::127.0.0.1:{ns_port}#Calc::add")),
    );
    bridge.stop();
    drop(backend);

    assert_eq!(resolved.trim(), "add 42");
}
