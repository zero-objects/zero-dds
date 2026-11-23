// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! An Apache Qpid Dispatch Router of one test alone: `qdrouterd` in
//! standalone mode with a configuration in a directory of its own, one
//! listener on a free port of 127.0.0.1 (SASL ANONYMOUS, no
//! authentication), the prefix `zd` as a balanced (anycast) address, and
//! the prefix `zd_refused` link-routed to a connection that does not
//! exist, so the router refuses a link to it.
//!
//! The router is the build under `QPID_DISPATCH_HOME`, else
//! `/opt/qpid-dispatch`, where `tests/integration/provision.sh dispatch`
//! puts it, with the CPython it embeds under `python/` there (1.19.0 does
//! not start on Python 3.13; `PYTHONHOME` names that one). A missing router
//! fails the test.

#![allow(dead_code)]

use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// How long the router may take to listen.
const STARTUP: Duration = Duration::from_secs(20);

/// A running router.
pub struct DispatchRouter {
    child: Option<Child>,
    /// The port of its listener.
    pub port: u16,
    dir: PathBuf,
}

fn home() -> PathBuf {
    std::env::var_os("QPID_DISPATCH_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/qpid-dispatch"))
}

fn qdrouterd() -> PathBuf {
    let program = home().join("sbin").join("qdrouterd");
    assert!(
        program.is_file(),
        "{} is missing (tests/integration/provision.sh dispatch)",
        program.display()
    );
    program
}

impl DispatchRouter {
    /// Writes the configuration and starts the router; returns once its
    /// listener accepts connections.
    ///
    /// # Panics
    /// When the router is missing or does not listen within 20 s; the
    /// panic carries its log.
    pub fn start() -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("zerodds-dispatch-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .expect("a free port")
            .port();
        let conf = format!(
            "router {{\n    mode: standalone\n    id: zd-router-{n}\n}}\n\
             listener {{\n    host: 127.0.0.1\n    port: {port}\n    role: normal\n    authenticatePeer: no\n    saslMechanisms: ANONYMOUS\n}}\n\
             address {{\n    prefix: zd\n    distribution: balanced\n}}\n\
             linkRoute {{\n    prefix: zd_refused\n    direction: in\n    connection: nowhere\n}}\n\
             linkRoute {{\n    prefix: zd_refused\n    direction: out\n    connection: nowhere\n}}\n\
             log {{\n    module: DEFAULT\n    enable: info+\n    outputFile: {}\n}}\n",
            dir.join("router.log").display()
        );
        std::fs::write(dir.join("qdrouterd.conf"), conf).unwrap();
        let mut router = Self {
            child: None,
            port,
            dir,
        };
        router.run();
        router
    }

    fn run(&mut self) {
        let mut command = Command::new(qdrouterd());
        let python = home().join("python");
        if python.is_dir() {
            command.env("PYTHONHOME", python);
        }
        let child = command
            .arg("-c")
            .arg(self.dir.join("qdrouterd.conf"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("qdrouterd");
        self.child = Some(child);
        let deadline = Instant::now() + STARTUP;
        while TcpStream::connect(("127.0.0.1", self.port)).is_err() {
            if let Some(Ok(Some(status))) = self.child.as_mut().map(Child::try_wait) {
                panic!("qdrouterd ended with {status}:\n{}", self.log());
            }
            assert!(
                Instant::now() < deadline,
                "qdrouterd did not listen within {STARTUP:?}:\n{}",
                self.log()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The router's log.
    pub fn log(&self) -> String {
        std::fs::read_to_string(self.dir.join("router.log")).unwrap_or_default()
    }

    /// Stops the router (every connection ends without a close) and starts
    /// it again on the same port.
    pub fn restart(&mut self) {
        self.kill();
        self.run();
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
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

impl Drop for DispatchRouter {
    fn drop(&mut self) {
        self.kill();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
