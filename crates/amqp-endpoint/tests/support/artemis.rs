// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! An Apache ActiveMQ Artemis broker of one test alone: an instance
//! created by `artemis create` in a directory of its own (in `/dev/shm`
//! when it is a tmpfs, else under the temporary directory), with one
//! acceptor on a free port for all protocols and the anycast queues the
//! test names. It starts with the test and stops with it. The user
//! `guest`/`guest` may connect; anonymous connections are admitted.
//!
//! The broker is the release archive under `ARTEMIS_HOME`, else
//! `/opt/artemis`, where `tests/integration/provision.sh artemis` puts it;
//! it runs on the `java` of the PATH. A missing broker fails the test.

#![allow(dead_code)]

#[path = "../../../amqp-0-9-1/tests/support/child_lines.rs"]
mod child_lines;

use std::net::TcpListener;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use child_lines::child_lines;

/// How long `artemis create` and the broker's start may take each.
const STARTUP: Duration = Duration::from_secs(60);

/// A running Artemis broker.
pub struct ArtemisBroker {
    child: Option<Child>,
    /// The port of its acceptor (AMQP among the protocols).
    pub port: u16,
    dir: PathBuf,
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free port")
        .port()
}

/// Where an instance lives: `/dev/shm` when it is a directory, else the
/// temporary directory.
fn instance_base() -> PathBuf {
    let shm = Path::new("/dev/shm");
    if shm.is_dir() {
        return shm.to_path_buf();
    }
    std::env::temp_dir()
}

/// The `artemis` program of the release archive.
fn artemis_program() -> PathBuf {
    let home = std::env::var_os("ARTEMIS_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/artemis"));
    let program = home.join("bin").join("artemis");
    assert!(
        program.is_file(),
        "{} is missing (tests/integration/provision.sh artemis)",
        program.display()
    );
    program
}

impl ArtemisBroker {
    /// Creates an instance with the anycast `queues`, starts it and waits
    /// until the broker reports that it is active.
    ///
    /// # Panics
    /// When the archive or `java` is missing, or when the instance is not
    /// created or the broker is not active within 60 s; the panic carries
    /// the broker's output.
    pub fn start(queues: &[&str]) -> Self {
        Self::start_configured(queues, |_| {})
    }

    /// [`Self::start`], with `configure` run on the instance directory
    /// after `artemis create` and before the broker starts: a test that
    /// needs another acceptor or login module edits `etc/` there.
    ///
    /// # Panics
    /// As [`Self::start`].
    pub fn start_configured(queues: &[&str], configure: impl FnOnce(&Path)) -> Self {
        let program = artemis_program();
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = instance_base().join(format!("zerodds-artemis-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let port = free_port();
        let created = Command::new(&program)
            .arg("create")
            .arg(&dir)
            .args([
                "--silent",
                "--force",
                "--user",
                "guest",
                "--password",
                "guest",
                "--allow-anonymous",
                "--no-web",
                "--no-autotune",
                "--nio",
                "--no-amqp-acceptor",
                "--no-hornetq-acceptor",
                "--no-mqtt-acceptor",
                "--no-stomp-acceptor",
                "--host",
                "127.0.0.1",
                "--default-port",
                &port.to_string(),
                "--queues",
                &queues.join(","),
            ])
            .stdin(Stdio::null())
            .output()
            .expect("artemis create (java on the PATH)");
        assert!(
            created.status.success(),
            "artemis create failed:\n{}\n{}",
            String::from_utf8_lossy(&created.stdout),
            String::from_utf8_lossy(&created.stderr)
        );
        configure(&dir);
        let mut broker = Self {
            child: None,
            port,
            dir,
        };
        broker.run();
        broker
    }

    /// Runs the instance and waits until the broker reports that it is
    /// active.
    fn run(&mut self) {
        let mut child = Command::new(self.dir.join("bin").join("artemis"))
            .arg("run")
            // A group of its own: the stop below ends the JVM the script
            // starts.
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("artemis run");
        let rx = child_lines(&mut child);
        self.child = Some(child);
        let deadline = Instant::now() + STARTUP;
        let mut seen = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                // AMQ221007: "Server is now active" (older releases: "live").
                Ok(line) => {
                    let up = line.contains("AMQ221007");
                    seen.push(line);
                    if up {
                        break;
                    }
                }
                Err(_) => panic!(
                    "Artemis did not report that it is active within {STARTUP:?}:\n{}",
                    seen.join("\n")
                ),
            }
        }
        // Keep draining the broker's output so that it never blocks on a
        // full pipe.
        std::thread::spawn(move || for _ in rx {});
    }

    /// Stops the broker (every connection ends without a close) and runs
    /// the same instance again on the same port.
    pub fn restart(&mut self) {
        self.kill();
        self.run();
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// `127.0.0.1:<port>`.
    #[must_use]
    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

impl Drop for ArtemisBroker {
    fn drop(&mut self) {
        self.kill();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
