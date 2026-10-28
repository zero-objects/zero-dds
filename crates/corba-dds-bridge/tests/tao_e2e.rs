// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon against TAO 2.5 (`zerodds-corba-bridge-1.0` §10,
//! §12.3). The C++ programs in `tests/tao/` are built with `tao_idl` and g++
//! against the IDL of `tests/omniorb/Calc.idl`; the bridge and a DDS backend
//! run in this process.
//!
//! * CORBA → DDS and DDS → CORBA as in `omniorb_e2e.rs`; TAO's servant is
//!   reachable through its `IORTable`.
//! * NameService: a TAO client resolves the bridge's object through a
//!   `corbaname:` URL against `zerodds-naming-svc`.
//!
//! The TAO on the measurement host is the one OpenDDS was built with; it has
//! no CosNaming service and no SSLIOP library, so TAO is measured over IIOP
//! and as a naming client only. `TAO_PREFIX` (default `/opt/opendds`) holds
//! `bin/tao_idl`, `include/` and `lib/`; a missing piece fails the test.
//!
//! ```bash
//! cargo test -p zerodds-corba-dds-bridge --test tao_e2e -- --ignored --test-threads=1
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

use common::{Bridge, CalcBackend};

fn prefix() -> PathBuf {
    std::env::var_os("TAO_PREFIX").map_or_else(|| PathBuf::from("/opt/opendds"), PathBuf::from)
}

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tao")
}

/// A fresh directory under the test target directory.
fn workdir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `tao_idl` + g++: `calc_client`, `listener_server` and `naming_client` in
/// `dir`.
fn build(dir: &Path) {
    let lib = prefix().join("lib");
    common::build_step(
        Command::new(prefix().join("bin/tao_idl"))
            .env("LD_LIBRARY_PATH", &lib)
            .current_dir(dir)
            // No AnyTypeCode: the programs use neither Any nor TypeCode.
            .args(["-Sa", "-St"])
            .arg(common::calc_idl()),
    );
    for (prog, skeleton, libs) in [
        ("calc_client", false, &["-lTAO", "-lACE"][..]),
        ("naming_client", false, &["-lTAO", "-lACE"][..]),
        (
            "listener_server",
            true,
            &[
                "-lTAO_IORTable",
                "-lTAO_PortableServer",
                "-lTAO_AnyTypeCode",
                "-lTAO",
                "-lACE",
            ][..],
        ),
    ] {
        let mut cmd = Command::new("g++");
        cmd.args(["-std=c++17", "-w", "-I"])
            .arg(prefix().join("include"))
            .arg("-I")
            .arg(dir)
            .arg(src().join(format!("{prog}.cpp")))
            .arg(dir.join("CalcC.cpp"));
        if skeleton {
            cmd.arg(dir.join("CalcS.cpp"));
        }
        common::build_step(
            cmd.arg("-L")
                .arg(&lib)
                .arg(format!("-Wl,-rpath,{}", lib.display()))
                .args(libs)
                .arg("-o")
                .arg(dir.join(prog)),
        );
    }
}

#[test]
#[ignore = "needs tao_idl, g++ and the TAO libraries (Linux test host)"]
fn tao_calls_reach_a_dds_backend_and_dds_samples_reach_a_tao_servant() {
    let dir = workdir("tao-e2e");
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

    let client = common::output(Command::new(dir.join("calc_client")).arg(&ior_file));

    let mut server = Command::new(dir.join("listener_server"))
        .args([listener_port.to_string(), "3".into()])
        .stdout(Stdio::piped())
        .spawn()
        .expect("listener_server");
    let mut lines = BufReader::new(server.stdout.take().unwrap()).lines();
    assert_eq!(lines.next().unwrap().unwrap(), "ready");
    backend.notify(&[7, 8, 9]);
    let mut received = Vec::new();
    for line in lines.map_while(Result::ok) {
        received.push(line);
        if received.len() == 3 {
            break;
        }
    }
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

/// A TAO client resolves the bridge's object through `zerodds-naming-svc`
/// with a `corbaname:` URL, which TAO's core library resolves itself.
#[test]
#[ignore = "needs tao_idl, g++ and the TAO libraries (Linux test host)"]
fn tao_resolves_the_bridge_through_zerodds_naming_svc() {
    let dir = workdir("tao-zd-naming");
    build(&dir);

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
        Command::new(dir.join("naming_client"))
            .arg(format!("corbaname::127.0.0.1:{ns_port}#Calc::add")),
    );
    bridge.stop();
    drop(backend);

    assert_eq!(resolved.trim(), "add 42");
}
