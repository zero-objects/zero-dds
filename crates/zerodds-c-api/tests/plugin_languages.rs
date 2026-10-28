// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 clause 11: a plugin of the application, written in C, C++
//! or Java, placed into a participant.
//!
//! Each language has a program that implements the AccessControl, Logging and
//! DataTagging plugins in that language, hands them to the security
//! configuration (`zerodds_security_set_*_plugin`), creates a secured runtime
//! over fixtures minted with `openssl`, and checks that the runtime called each
//! plugin where the specification calls it: the participant creation (9.8.1),
//! the creation of a DataWriter and a DataReader (9.8.6) that the plugin
//! refuses for the topic `Forbidden`, the log of the refusal (9.6), the tags
//! of the endpoint (9.7), and the release of each plugin. The program prints
//! `OK` and exits 0.
//!
//! A host without the compiler of a language, or without `openssl`, prints
//! `[skip]` and the test does not count as run.

#![cfg(all(feature = "security", unix))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[path = "../../dcps/tests/common/pki.rs"]
mod pki;

use pki::{must_run, openssl_available};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_dir()
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .expect("workspace root")
}

/// Runs `command`, panicking with both streams when it fails.
fn run(mut command: Command, what: &str) -> String {
    let out = command.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        out.status.success(),
        "{what} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn available(tool: &str, probe: &str) -> bool {
    Command::new(tool)
        .arg(probe)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The directory with `libzerodds.so`, built with the `security` feature.
fn library_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        // Next to the target directory of this build (the lane of a test host
        // keeps it between calls), not inside the sources.
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map_or_else(|| workspace_root().join("target"), PathBuf::from)
            .join("plugin-languages");
        let mut cargo = Command::new(env!("CARGO"));
        cargo
            .args(["build", "-p", "zerodds-c-api", "--features", "security"])
            .arg("--target-dir")
            .arg(&target)
            .current_dir(workspace_root());
        run(cargo, "cargo build zerodds-c-api");
        target.join("debug")
    })
}

/// The files a secured runtime reads, minted for one domain.
struct Fixtures {
    domain: u32,
    args: Vec<String>,
    _lease: zerodds_test_domains::DomainLease,
}

impl Fixtures {
    /// The arguments of the programs: domain, the five paths.
    fn arguments(&self) -> Vec<String> {
        let mut args = vec![self.domain.to_string()];
        args.extend(self.args.iter().cloned());
        args
    }
}

fn tempdir(prefix: &str) -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("{prefix}{now}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Mints a CA, an identity and the signed governance and permissions of a
/// domain of its own; `None` where `openssl` is missing.
fn fixtures() -> Option<Fixtures> {
    if !openssl_available() {
        println!("[skip] openssl not on PATH");
        return None;
    }
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let dir = tempdir("zerodds_plugin_lang_");
    let minted = {
        let _g = pki::OPENSSL_LOCK.lock().unwrap();
        pki::mint_ca_and_leaves(
            &dir,
            "/CN=ZeroDDS Plugin Language CA",
            "/CN=zerodds-plugin-{name}/emailAddress={name}@plugin.local",
            &["alice"],
        )
    };
    let certs = dir.join("certs");
    let ca_cert = certs.join("identity_ca.pem");
    let ca_key = certs.join("identity_ca_key.pem");
    std::fs::write(
        dir.join("governance.xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_governance">
  <domain_access_rules>
    <domain_rule>
      <domains><id>{domain}</id></domains>
      <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
      <enable_join_access_control>false</enable_join_access_control>
      <discovery_protection_kind>NONE</discovery_protection_kind>
      <liveliness_protection_kind>NONE</liveliness_protection_kind>
      <rtps_protection_kind>NONE</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>false</enable_discovery_protection>
          <enable_liveliness_protection>false</enable_liveliness_protection>
          <enable_read_access_control>false</enable_read_access_control>
          <enable_write_access_control>false</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("permissions.xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_permissions">
  <permissions>
    <grant>
      <subject_name>CN=zerodds-plugin-alice,emailAddress=alice@plugin.local</subject_name>
      <validity>
        <not_before>2025-01-01T00:00:00</not_before>
        <not_after>2099-01-01T00:00:00</not_after>
      </validity>
      <allow_rule>
        <domains><id>{domain}</id></domains>
        <publish><topics><topic>*</topic></topics></publish>
        <subscribe><topics><topic>*</topic></topics></subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
  </permissions>
</dds>
"#
        ),
    )
    .unwrap();
    for name in ["governance", "permissions"] {
        let mut sign = Command::new("openssl");
        sign.args(["smime", "-sign", "-in"])
            .arg(dir.join(format!("{name}.xml")))
            .args(["-text", "-out"])
            .arg(dir.join(format!("{name}.p7s")))
            .arg("-signer")
            .arg(&ca_cert)
            .arg("-inkey")
            .arg(&ca_key);
        must_run(&format!("cms-sign {name}"), sign);
    }
    let path = |p: &Path| p.to_string_lossy().into_owned();
    Some(Fixtures {
        domain,
        args: vec![
            path(&ca_cert),
            path(&minted[0].cert),
            path(&minted[0].key),
            path(&ca_cert),
            path(&dir.join("governance.p7s")),
            path(&dir.join("permissions.p7s")),
        ],
        _lease: lease,
    })
}

/// Runs a built program with the library on its path; the program prints
/// `OK` at the end.
fn run_program(mut command: Command, fixtures: &Fixtures, what: &str) {
    command
        .args(fixtures.arguments())
        .env("LD_LIBRARY_PATH", library_dir());
    let stdout = run(command, what);
    assert!(stdout.lines().any(|l| l == "OK"), "{what}: {stdout}");
}

#[test]
fn a_plugin_in_c_is_placed_into_a_participant() {
    if !available("cc", "--version") {
        println!("[skip] cc not on PATH");
        return;
    }
    let Some(fixtures) = fixtures() else {
        return;
    };
    let lib = library_dir();
    let work = tempdir("zerodds_plugin_c_");
    let program = work.join("plugin_test_c");
    let mut compile = Command::new("cc");
    compile
        .args(["-std=c99", "-O1", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(crate_dir().join("include"))
        .arg("-o")
        .arg(&program)
        .arg(crate_dir().join("tests/plugin_c/plugin_test.c"))
        .arg("-L")
        .arg(lib)
        .args(["-lzerodds", "-lpthread", "-ldl", "-lm"]);
    run(compile, "cc plugin_test.c");
    run_program(Command::new(&program), &fixtures, "plugin_test (C)");
}

#[test]
fn a_plugin_in_cpp_is_placed_into_a_participant() {
    if !available("c++", "--version") {
        println!("[skip] c++ not on PATH");
        return;
    }
    let Some(fixtures) = fixtures() else {
        return;
    };
    let lib = library_dir();
    let work = tempdir("zerodds_plugin_cpp_");
    let program = work.join("plugin_test_cpp");
    let mut compile = Command::new("c++");
    compile
        .args(["-std=c++11", "-O1", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(crate_dir().join("include"))
        .arg("-o")
        .arg(&program)
        .arg(crate_dir().join("tests/plugin_cpp/plugin_test.cpp"))
        .arg("-L")
        .arg(lib)
        .args(["-lzerodds", "-lpthread", "-ldl", "-lm"]);
    run(compile, "c++ plugin_test.cpp");
    run_program(Command::new(&program), &fixtures, "plugin_test (C++)");
}

/// The major version of the JDK that `javac` belongs to, if there is one.
fn javac_major() -> Option<u32> {
    let out = Command::new("javac").arg("-version").output().ok()?;
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    text.split_whitespace()
        .nth(1)?
        .split('.')
        .next()?
        .parse()
        .ok()
}

/// The plugin interfaces in Java reach the native library through the foreign
/// function interface of JDK 21 (a preview API there).
#[test]
fn a_plugin_in_java_is_placed_into_a_participant() {
    if !available("java", "-version") || javac_major().is_none_or(|v| v < 21) {
        println!("[skip] no JDK 21 (javac, java) on PATH");
        return;
    }
    let Some(fixtures) = fixtures() else {
        return;
    };
    let lib = library_dir();
    let work = tempdir("zerodds_plugin_java_");
    let classes = work.join("classes");
    std::fs::create_dir_all(&classes).unwrap();
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(crate_dir().join("java/org/zerodds/security")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "java") {
            sources.push(path);
        }
    }
    sources.push(crate_dir().join("tests/plugin_java/PluginTest.java"));
    let mut compile = Command::new("javac");
    compile
        .args(["--enable-preview", "--release", "21", "-d"])
        .arg(&classes)
        .args(&sources);
    run(compile, "javac");
    let library = lib.join(if cfg!(target_os = "macos") {
        "libzerodds.dylib"
    } else {
        "libzerodds.so"
    });
    let mut java = Command::new("java");
    java.args([
        "--enable-preview",
        "--enable-native-access=ALL-UNNAMED",
        "-cp",
    ])
    .arg(&classes)
    .arg("PluginTest")
    .arg(&library);
    run_program(java, &fixtures, "PluginTest (Java)");
}
