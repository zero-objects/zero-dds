// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A JacORB server for the cross-ORB tests: its IDL through JacORB's IDL
//! compiler, its Java sources through javac, started with JacORB as the
//! ORB. JacORB 3.9 under `JACORB_HOME` (default `/opt/jacorb`) and a
//! Java 8 JDK under `JACORB_JAVA_HOME` (default `/opt/jdk8`), as
//! `crates/corba-dds-bridge/tests/jacorb_e2e.rs` uses them;
//! `tests/integration/provision.sh jacorb` installs both there.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub fn jacorb_home() -> PathBuf {
    std::env::var_os("JACORB_HOME").map_or_else(|| PathBuf::from("/opt/jacorb"), PathBuf::from)
}

pub fn java_home() -> PathBuf {
    std::env::var_os("JACORB_JAVA_HOME").map_or_else(|| PathBuf::from("/opt/jdk8"), PathBuf::from)
}

/// Runs a build step; a failure names the step and its output.
fn build_step(cmd: &mut Command) {
    let out = cmd.output().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    assert!(
        out.status.success(),
        "{cmd:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Every `.java` file under `dir`.
fn java_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            java_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
}

/// The JacORB server process, stopped with the test.
pub struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// What a test hands over: the IDL and Java sources of the server, its
/// main class, its system properties, and how it publishes its IOR.
pub struct Spec<'a> {
    /// A directory of its own under the test target directory.
    pub name: &'a str,
    pub idl: Vec<PathBuf>,
    pub java: Vec<PathBuf>,
    pub main: &'a str,
    pub props: &'a [&'a str],
    /// The IOR goes to the file the server is given as its one argument
    /// (`true`), or to stdout as `<prefix>IOR:…` (`false`).
    pub ior_to_file: bool,
    pub stdout_prefix: &'a str,
}

/// Builds and starts the server; returns it and its IOR.
pub fn start(spec: &Spec<'_>) -> (Server, String) {
    let home = jacorb_home();
    let lib = home.join("lib");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(spec.name);
    let _ = std::fs::remove_dir_all(&dir);
    let (generated, classes) = (dir.join("gen"), dir.join("classes"));
    std::fs::create_dir_all(&classes).unwrap();
    build_step(
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
            .args(&spec.idl),
    );
    let mut sources = Vec::new();
    java_sources(&generated, &mut sources);
    sources.extend(spec.java.iter().cloned());
    build_step(
        Command::new(java_home().join("bin/javac"))
            .arg("-nowarn")
            .arg("-encoding")
            .arg("UTF-8")
            .arg("-endorseddirs")
            .arg(&lib)
            .arg("-cp")
            .arg(format!("{}/*", lib.display()))
            .arg("-d")
            .arg(&classes)
            .args(&sources),
    );
    let ior_file = dir.join("server.ior");
    let mut cmd = Command::new(java_home().join("bin/java"));
    cmd.arg(format!("-Djava.endorsed.dirs={}", lib.display()))
        .arg(format!("-Djacorb.home={}", home.display()))
        .arg("-Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB")
        .arg("-Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton")
        .arg("-Dorg.jacorb.ipAddr=127.0.0.1")
        .arg("-DOAIAddr=127.0.0.1")
        .args(spec.props.iter().map(|p| format!("-D{p}")))
        .arg("-cp")
        .arg(format!("{}:{}/*", classes.display(), lib.display()))
        .arg(spec.main)
        .stderr(Stdio::null());
    if spec.ior_to_file {
        cmd.arg(&ior_file).stdout(Stdio::null());
    } else {
        cmd.stdout(std::fs::File::create(&ior_file).unwrap());
    }
    let server = Server(cmd.spawn().expect("the JacORB server"));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(text) = std::fs::read_to_string(&ior_file)
            && let Some(ior) = text.lines().find_map(|l| {
                l.strip_prefix(spec.stdout_prefix)
                    .filter(|rest| rest.starts_with("IOR:"))
            })
        {
            return (server, ior.trim().to_string());
        }
        assert!(Instant::now() < deadline, "the JacORB server wrote no IOR");
        std::thread::sleep(Duration::from_millis(100));
    }
}
