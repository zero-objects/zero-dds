// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! An Apache Qpid Broker-J of one test alone: a work directory of its own
//! (in `/dev/shm` when it is a tmpfs, else under the temporary directory),
//! an initial configuration the test writes (`config` gets the AMQP port
//! and returns the JSON), one AMQP 1.0 port. It starts with the test and
//! stops with it.
//!
//! The broker is the release archive under `QPID_HOME`, else
//! `/opt/qpid-broker-j`, where `tests/integration/provision.sh brokerj`
//! puts it; it runs on the `java` of the PATH. A missing broker fails the
//! test.

#![allow(dead_code)]

#[path = "../../../amqp-0-9-1/tests/support/child_lines.rs"]
mod child_lines;

use std::net::TcpListener;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use child_lines::child_lines;

/// How long the broker's start may take.
const STARTUP: Duration = Duration::from_secs(90);

/// A running Broker-J.
pub struct BrokerJ {
    child: Option<Child>,
    /// The AMQP port.
    pub port: u16,
    dir: PathBuf,
    output: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free port")
        .port()
}

fn instance_base() -> PathBuf {
    let shm = Path::new("/dev/shm");
    if shm.is_dir() {
        return shm.to_path_buf();
    }
    std::env::temp_dir()
}

/// `bin/qpid-server` of the release archive.
fn qpid_server() -> PathBuf {
    let home = std::env::var_os("QPID_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/qpid-broker-j"));
    let program = home.join("bin").join("qpid-server");
    assert!(
        program.is_file(),
        "{} is missing (tests/integration/provision.sh brokerj)",
        program.display()
    );
    program
}

impl BrokerJ {
    /// Writes the initial configuration `config(port)` returns, starts the
    /// broker and waits until it reports that it is ready (`BRK-1004`).
    ///
    /// # Panics
    /// When the archive or `java` is missing, or when the broker is not
    /// ready within 90 s; the panic carries the broker's output.
    pub fn start(config: impl FnOnce(u16) -> String) -> Self {
        let program = qpid_server();
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = instance_base().join(format!("zerodds-brokerj-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("work directory");
        let port = free_port();
        let initial = dir.join("initial-config.json");
        std::fs::write(&initial, config(port)).expect("initial configuration");
        let mut child = Command::new(program)
            .arg("--initial-config-path")
            .arg(&initial)
            .env("QPID_WORK", &dir)
            .env("QPID_JAVA_MEM", "-Xmx256m -XX:MaxDirectMemorySize=256m")
            // A group of its own: the stop below ends the JVM the script
            // starts.
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("qpid-server");
        let rx = child_lines(&mut child);
        let broker = Self {
            child: Some(child),
            port,
            dir,
            output: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let deadline = Instant::now() + STARTUP;
        let mut seen = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                // BRK-1004: "Qpid Broker Ready".
                Ok(line) => {
                    let up = line.contains("BRK-1004");
                    seen.push(line);
                    if up {
                        break;
                    }
                }
                Err(_) => panic!(
                    "Broker-J did not report that it is ready within {STARTUP:?}:\n{}",
                    seen.join("\n")
                ),
            }
        }
        broker.output.lock().unwrap().extend(seen);
        let output = std::sync::Arc::clone(&broker.output);
        std::thread::spawn(move || {
            for line in rx {
                output.lock().unwrap().push(line);
            }
        });
        broker
    }

    /// What the broker wrote so far, for the message of a failed test.
    #[must_use]
    pub fn output(&self) -> String {
        self.output.lock().unwrap().join("\n")
    }

    /// `127.0.0.1:<port>`.
    #[must_use]
    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

impl Drop for BrokerJ {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
