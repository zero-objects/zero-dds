// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon against omniORB 4.3 (`zerodds-corba-bridge-1.0` §10,
//! §12.3). The omniORB programs in `tests/omniorb/` are built with omniidl
//! and g++; the bridge and a DDS backend run in this process.
//!
//! * CORBA → DDS: the omniORB client calls `add(2, 40)` on the bridge's
//!   IOR; the backend reads the request sample, answers `42` on the reply
//!   topic; the client prints `add 42`.
//! * `never` has no backend: the client gets `CORBA::TIMEOUT`.
//! * `never` on the `add` object is an operation the object does not map:
//!   `CORBA::BAD_OPERATION`.
//! * `echo` of 200 000 bytes: the reply leaves the bridge in fragments
//!   (fragment_size 16 384), the request as omniORB sends it.
//! * DDS → CORBA: three samples on the notify topic reach an omniORB
//!   servant as oneway `on_value` calls.
//! * SSLIOP with a client certificate the ACL names.
//! * NameService, both ways: the bridge rebinds its objects into omniORB's
//!   `omniNames` and unbinds them on stop; omniORB's `nameclt` binds, lists,
//!   resolves and unbinds against `zerodds-naming-svc`.
//!
//! Needs omniidl, g++, libomniORB4, libomnisslTP4, omniNames and nameclt
//! (Linux test host). A missing piece fails the test.
//!
//! ```bash
//! cargo test -p zerodds-corba-dds-bridge --test omniorb_e2e -- --ignored --test-threads=1
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

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/omniorb")
}

/// A fresh directory under the test target directory.
fn workdir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// omniidl + g++: `calc_client`, `listener_server` and `ssl_client` in `dir`.
fn build(dir: &Path) {
    common::build_step(
        Command::new("omniidl")
            .args(["-bcxx", "-C"])
            .arg(dir)
            .arg(common::calc_idl()),
    );
    for (prog, ssl) in [
        ("calc_client", false),
        ("listener_server", false),
        ("ssl_client", true),
    ] {
        let mut cmd = Command::new("g++");
        cmd.arg("-std=c++17")
            .arg("-I")
            .arg(dir)
            .arg(src().join(format!("{prog}.cc")))
            .arg(dir.join("CalcSK.cc"));
        if ssl {
            cmd.arg("-lomnisslTP4");
        }
        common::build_step(
            cmd.args(["-lomniORB4", "-lomnithread", "-o"])
                .arg(dir.join(prog)),
        );
    }
}

#[test]
#[ignore = "needs omniidl, g++ and libomniORB4 (Linux test host)"]
fn omniorb_calls_reach_a_dds_backend_and_dds_samples_reach_an_omniorb_servant() {
    let dir = workdir("omniorb-e2e");
    build(&dir);

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

    // omniORB's call timeout: a lost reply fails the call instead of
    // hanging the test.
    let client = common::output(
        Command::new(dir.join("calc_client"))
            .arg(&ior_file)
            .args(["-ORBclientCallTimeOutPeriod", "15000"]),
    );

    // DDS → CORBA: an omniORB Listener, three samples.
    let mut server = Command::new(dir.join("listener_server"))
        .args([listener_port.to_string(), "3".into()])
        .stdout(Stdio::piped())
        .spawn()
        .expect("listener_server");
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

/// The bridge rebinds `Calc::add` into omniORB's `omniNames` at start;
/// `nameclt` lists it and resolves it to the bridge's IOR; after a clean
/// stop the name is gone.
#[test]
#[ignore = "needs omniNames and nameclt (Linux test host)"]
fn the_bridge_rebinds_into_omninames_and_unbinds_on_stop() {
    let dir = workdir("omniorb-omninames");
    let log = dir.join("omninames-log");
    std::fs::create_dir_all(&log).unwrap();
    let ns_port = common::free_port();
    let _ns = ChildGuard(
        Command::new("omniNames")
            .args(["-start", &ns_port.to_string(), "-logdir"])
            .arg(&log)
            .args(["-ORBendPoint", &format!("giop:tcp:127.0.0.1:{ns_port}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("omniNames"),
    );
    common::wait_for_listener(ns_port, "omniNames");

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
    let nameclt = |args: &[&str]| {
        common::output(
            Command::new("nameclt")
                .args([
                    "-ORBInitRef",
                    &format!("NameService=corbaloc::127.0.0.1:{ns_port}/NameService"),
                ])
                .args(args),
        )
    };
    let mut listed = String::new();
    for _ in 0..50 {
        listed = nameclt(&["list"]);
        if listed.contains("Calc::add") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let resolved = nameclt(&["resolve", "Calc::add"]);
    bridge.stop();
    let after = nameclt(&["list"]);

    let mut names: Vec<&str> = listed.lines().map(str::trim).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["Calc::add", "Calc::echo", "Calc::never"]);
    let bridge_ior = std::fs::read_to_string(&ior_file).unwrap();
    let add_ior = bridge_ior
        .lines()
        .find_map(|l| l.strip_prefix("IDL:zerodds_test/Calc:1.0 add "))
        .unwrap();
    assert_eq!(resolved.trim(), add_ior.trim());
    assert_eq!(after.trim(), "", "a clean stop unbinds every name");
}

/// omniORB's `nameclt` against `zerodds-naming-svc`: it narrows the root
/// context with a remote `_is_a` (the reference is a `corbaloc:` one without
/// a type id), then binds a new context (and prints its IOR), lists,
/// resolves and removes it. `nameclt` refuses `unbind` for a context and
/// asks for `remove_context`.
#[test]
#[ignore = "needs nameclt (Linux test host)"]
fn omniorb_nameclt_binds_lists_resolves_and_unbinds_on_zerodds_naming_svc() {
    let port = common::free_port();
    let _ns = common::start_naming_svc(port);
    let nameclt = |args: &[&str]| {
        common::output(
            Command::new("nameclt")
                .args([
                    "-ORBInitRef",
                    &format!("NameService=corbaloc::127.0.0.1:{port}/NameService"),
                ])
                .args(args),
        )
    };
    let bound = nameclt(&["bind_new_context", "ctx1"]);
    let listed = nameclt(&["list"]);
    let resolved = nameclt(&["resolve", "ctx1"]);
    let _ = nameclt(&["remove_context", "ctx1"]);
    let after = nameclt(&["list"]);

    assert!(bound.trim().starts_with("IOR:"), "{bound}");
    assert_eq!(listed.trim(), "ctx1/");
    assert_eq!(
        resolved.trim(),
        bound.trim(),
        "resolve returns the bound context"
    );
    assert_eq!(after.trim(), "");
}

/// SSLIOP against omniORB (§7.1, §7.3): the IOR's TAG_SSL_SEC_TRANS names
/// the SSL port, the bridge requires a client certificate, and the ACL
/// allows only certificate A (subject `mtls:<SHA-256 of its DER>`). `never`
/// has no backend, so a call the ACL lets through ends in `TIMEOUT`, one it
/// refuses in `NO_PERMISSION`; A can only reach `TIMEOUT` over SSL with its
/// certificate.
#[test]
#[ignore = "needs omniidl, g++, libomniORB4 and libomnisslTP4 (Linux test host)"]
fn omniorb_over_ssliop_passes_the_acl_with_the_allowed_certificate_only() {
    use rcgen::{CertificateParams, IsCa, KeyPair, SanType};
    use sha2::{Digest, Sha256};

    let dir = workdir("omniorb-ssl");
    build(&dir);

    // Each certificate needs its own subject: rcgen's default DN is one name
    // for all, and OpenSSL then takes a leaf for self-signed (UnknownCA).
    let named = |cn: &str| {
        let mut dn = rcgen::DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, cn);
        dn
    };
    let mut ca_p = CertificateParams::new(vec!["bridge test CA".to_string()]).unwrap();
    ca_p.distinguished_name = named("bridge test CA");
    ca_p.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = ca_p.self_signed(&ca_key).unwrap();
    let leaf = |name: &str| {
        let mut p = CertificateParams::new(vec![name.to_string()]).unwrap();
        p.distinguished_name = named(name);
        p.subject_alt_names
            .push(SanType::IpAddress(std::net::IpAddr::from([127, 0, 0, 1])));
        p.is_ca = IsCa::NoCa;
        let k = KeyPair::generate().unwrap();
        let c = p.signed_by(&k, &ca, &ca_key).unwrap();
        (c, k)
    };
    let (server, server_key) = leaf("localhost");
    let (alice, alice_key) = leaf("alice");
    let (bob, bob_key) = leaf("bob");
    let write = |name: &str, text: String| {
        let p = dir.join(name);
        std::fs::write(&p, text).unwrap();
        p
    };
    let ca_pem = write("ca.pem", ca.pem());
    let server_pem = write("server.pem", server.pem());
    let server_key_pem = write("server-key.pem", server_key.serialize_pem());
    // omniORB reads certificate and key from one file.
    let alice_pem = write("alice.pem", alice.pem() + &alice_key.serialize_pem());
    let bob_pem = write("bob.pem", bob.pem() + &bob_key.serialize_pem());
    let alice_subject: String = Sha256::digest(alice.der())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let lease = common::lease();
    let iiop_port = common::free_port();
    let ssl_port = common::free_port();
    let ior_file = dir.join("bridge.ior");
    let yaml = format!(
        "domain: {d}\n\
         corba:\n  iiop:\n    bind: \"127.0.0.1:{iiop_port}\"\n  ssliop:\n    enabled: true\n    bind: \"127.0.0.1:{ssl_port}\"\n    cert_file: \"{cert}\"\n    key_file: \"{key}\"\n    ca_file: \"{ca}\"\n    require_client_cert: true\n  ior:\n    ior_file: \"{ior}\"\n\
         mappings:\n\
         \x20 - repo_id: \"IDL:zerodds_test/Calc:1.0\"\n    operation: never\n    request_topic:\n      dds_name: Ssl_never_Request\n    reply_topic:\n      dds_name: Ssl_never_Reply\n    timeout_ms: 1000\n\
         acl:\n  default_deny: true\n  rules:\n    - subject: \"mtls:{alice_subject}\"\n      allow_invoke: [\"IDL:zerodds_test/*\"]\n",
        d = lease.id(),
        cert = server_pem.display(),
        key = server_key_pem.display(),
        ca = ca_pem.display(),
        ior = ior_file.display()
    );
    let bridge = Bridge::start(&yaml, &ior_file);
    let call = |key: Option<&Path>| {
        let mut cmd = Command::new(dir.join("ssl_client"));
        cmd.arg(&ior_file).arg(&ca_pem);
        if let Some(k) = key {
            cmd.arg(k);
        }
        let out = cmd.output().unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout).trim(),
            if out.stdout.is_empty() {
                format!(" (stderr {})", String::from_utf8_lossy(&out.stderr))
            } else {
                String::new()
            }
        )
    };
    let with_alice = call(Some(&alice_pem));
    let with_bob = call(Some(&bob_pem));
    bridge.stop();
    assert_eq!(
        with_alice, "never EXCEPTION TIMEOUT",
        "alice passes the ACL"
    );
    assert_eq!(with_bob, "never EXCEPTION NO_PERMISSION", "bob does not");
}
