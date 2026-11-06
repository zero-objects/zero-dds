// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The programs of the cross-ORB tests: the foreign ORBs' servers and
//! clients that `competitors/build_peers.sh` builds into the directory
//! `ZERODDS_CORBA_PEERS` names, and this package's own `interop_server`,
//! `interop_client`, `ssliop_server` and `ssliop_client`. A program that is
//! missing fails the test.

#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long a server may take to print its IORs: a JVM with JacORB starts
/// in 1 to 3 s on the Linux test host.
const STARTUP: Duration = Duration::from_secs(20);
/// How long a client may take for its calls.
const CALLS: Duration = Duration::from_secs(20);

/// The directory of the foreign programs.
pub fn peers() -> PathBuf {
    let dir = std::env::var_os("ZERODDS_CORBA_PEERS").map(PathBuf::from).unwrap_or_else(|| {
        panic!("ZERODDS_CORBA_PEERS is not set: competitors/build_peers.sh builds the foreign programs")
    });
    assert!(
        dir.is_dir(),
        "ZERODDS_CORBA_PEERS={} is no directory",
        dir.display()
    );
    dir
}

/// A foreign program of `peers()`, which must exist.
pub fn peer(name: &str) -> PathBuf {
    let path = peers().join(name);
    assert!(
        path.is_file(),
        "{} is missing: competitors/build_peers.sh builds it",
        path.display()
    );
    path
}

/// A server process, stopped with the test, and what it printed.
pub struct Server {
    child: Child,
    lines: mpsc::Receiver<String>,
    seen: Vec<String>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    /// Starts `command` with its stdout piped; stderr goes to the test's.
    pub fn start(mut command: Command) -> Self {
        let mut child = command
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("{command:?}: {e}"));
        let stdout = child.stdout.take().expect("piped stdout");
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            lines,
            seen: Vec::new(),
        }
    }

    /// What follows `<prefix>` in the first line the server prints with it
    /// (omniNames puts its log's own prefix and a time ahead).
    pub fn value(&mut self, prefix: &str) -> String {
        let deadline = Instant::now() + STARTUP;
        loop {
            if let Some(found) = self
                .seen
                .iter()
                .find_map(|l| l.find(prefix).map(|at| &l[at + prefix.len()..]))
            {
                return found.trim().to_string();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => self.seen.push(line),
                Err(_) => panic!(
                    "the server printed no `{prefix}` within {STARTUP:?}; it printed:\n{}",
                    self.seen.join("\n")
                ),
            }
        }
    }

    /// The IOR of a line `<prefix>IOR:…`.
    pub fn ior(&mut self, prefix: &str) -> String {
        let value = self.value(prefix);
        assert!(value.starts_with("IOR:"), "`{prefix}{value}` is no IOR");
        value
    }

    /// Waits for a line equal to `line`.
    pub fn wait_for_line(&mut self, line: &str) {
        let deadline = Instant::now() + CALLS;
        loop {
            if self.seen.iter().any(|l| l == line) {
                return;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(l) => self.seen.push(l),
                Err(_) => panic!(
                    "the server printed no `{line}` within {CALLS:?}; it printed:\n{}",
                    self.seen.join("\n")
                ),
            }
        }
    }
}

/// Runs a client to its end; panics with its output unless it exits 0.
pub fn run_client(mut command: Command) -> String {
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("{command:?}: {e}"));
    let id = child.id();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let output = match rx.recv_timeout(CALLS) {
        Ok(output) => output.unwrap_or_else(|e| panic!("{command:?}: {e}")),
        Err(_) => {
            let _ = Command::new("kill").arg("-9").arg(id.to_string()).status();
            panic!("{command:?} did not end within {CALLS:?}");
        }
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "{command:?} failed ({}):\n{text}",
        output.status
    );
    text
}

/// This package's interop server on loopback: Echo, Bench and the
/// CosNaming root context.
pub fn zerodds_server() -> Server {
    let mut command = Command::new(env!("CARGO_BIN_EXE_interop_server"));
    command.args(["127.0.0.1", "0"]);
    Server::start(command)
}

/// This package's interop client: `mode` is `echo`, `bench` or `naming`.
pub fn zerodds_client(mode: &str, ior: &str) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_interop_client"));
    command.args([mode, ior]);
    run_client(command)
}

/// A directory of the test's own under the test target directory.
pub fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

/// A TCP port nothing listens on now, for a daemon that takes its port as
/// an argument.
pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .expect("a free port")
}
