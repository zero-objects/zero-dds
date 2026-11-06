// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-integration`. Safety classification: **TOOLING**.
//!
//! The integration pillar of the test suite
//! (`internal/release/rc8-test-suite.md`, "Pillars → Integration").
//!
//! A scenario runs ZeroDDS end to end, in processes of its own, and holds
//! what comes out against an oracle outside the code under test: a clause
//! of a specification whose expected value the scenario states before the
//! run, a foreign implementation on the other side (a broker, a client
//! library, a compiler), or the kernel's view of the sockets the processes
//! hold. Rules every scenario keeps (`rc8-test-suite.md`, "Rules"):
//!
//! * no `sleep` as synchronisation: every wait is on an event — a line a
//!   process prints, a DDS status, the end of a stream — with a deadline
//!   ([`Peer::wait_for`]);
//! * at most 25 s per test (the nextest profile `integration`);
//! * DDS domains from the leases of `zerodds-test-domains`;
//! * a foreign program that is missing fails the scenario ([`tools::require`]);
//!   nothing is skipped.
//!
//! Where each scenario runs (merge, weekly) is `ci/integration_scenarios.py`.

// Test scaffolding: a scenario whose step fails has no result, so the helpers
// end it with a panic that names the step, as `crates/endpoint-e2e` does.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

#[cfg(feature = "dcps")]
pub mod dds;
pub mod kernel;
pub mod tools;

pub use zerodds_test_domains::DomainLease;

/// A DDS domain no other process of this host holds, from the host's share
/// of the band (`zerodds-test-domains`: the CI jobs lease from the share
/// `ZERODDS_TEST_DOMAIN_BAND=ci`, other hosts from theirs, and the domains
/// foreign processes use on the LAN are never leased).
#[must_use]
pub fn lease_domain() -> DomainLease {
    zerodds_test_domains::lease_domain()
}

/// The environment variable that names the role a re-executed test binary
/// plays (see [`role`] and [`role_command`]).
pub const ROLE_VARIABLE: &str = "ZERODDS_INTEGRATION_ROLE";

/// The prefix of the lines a role prints for its scenario to read.
pub const EVENT: &str = "EVENT ";

/// The role this process plays, when a scenario started it with
/// [`role_command`]; `None` in the scenario itself.
#[must_use]
pub fn role() -> Option<String> {
    std::env::var(ROLE_VARIABLE).ok()
}

/// A command that runs the test function `test` of this test binary again,
/// as a process of its own that plays `role`.
///
/// The test function asks [`role`] first and, in a role, does the role's
/// part and returns. The arguments are libtest's and nextest's own
/// (`--exact <name> --nocapture`), so the child runs that one function.
///
/// # Panics
/// When the path of the running test binary cannot be read.
#[must_use]
pub fn role_command(test: &str, role: &str) -> Command {
    // zerodds-lint: allow no_panic_in_safe — a test binary without a path
    // cannot start its peers.
    let exe = std::env::current_exe().unwrap_or_else(|e| panic!("current_exe: {e}"));
    let mut command = Command::new(exe);
    command
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(ROLE_VARIABLE, role);
    command
}

/// Prints one event line for the scenario that started this role.
pub fn event(text: impl AsRef<str>) {
    println!("{EVENT}{}", text.as_ref());
}

/// Blocks until the scenario closes this role's standard input: the
/// scenario's signal that it has read what it needs and the role may end.
pub fn wait_for_release() {
    let mut sink = Vec::new();
    let _ = std::io::stdin().lock().read_to_end(&mut sink);
}

/// Blocks until the scenario sends the line `token` on this role's
/// standard input ([`Peer::send_line`]).
///
/// # Panics
/// When standard input ends first.
pub fn wait_for_line_on_stdin(token: &str) {
    let mut line = String::new();
    loop {
        line.clear();
        // zerodds-lint: allow no_panic_in_safe — a role whose scenario
        // went away has nothing left to do.
        let read = std::io::stdin()
            .lock()
            .read_line(&mut line)
            .unwrap_or_else(|e| panic!("stdin: {e}"));
        assert!(read > 0, "stdin ended before the line `{token}`");
        if line.trim_end() == token {
            return;
        }
    }
}

/// Where a line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// Standard output.
    Out,
    /// Standard error.
    Err,
}

/// One line a peer printed.
#[derive(Debug, Clone)]
pub struct Line {
    /// Standard output or standard error.
    pub stream: Stream,
    /// The text, without the line end.
    pub text: String,
}

enum Message {
    Line(Line),
    Closed,
}

/// A process of a scenario: a ZeroDDS role, a daemon, a broker or a
/// foreign client. Its output arrives as lines on a channel, which is what
/// [`Peer::wait_for`] waits on; its standard input stays open until
/// [`Peer::release`], so a role can block on it. Dropping a peer that still
/// runs kills it.
pub struct Peer {
    name: String,
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Message>,
    open: usize,
    seen: Vec<Line>,
}

impl Peer {
    /// Starts `command` as the peer `name`.
    ///
    /// # Panics
    /// When the program cannot be started: a scenario whose peer does not
    /// run has no result.
    #[must_use]
    pub fn spawn(name: &str, mut command: Command) -> Self {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // zerodds-lint: allow no_panic_in_safe — see the doc comment.
        let mut child = command
            .spawn()
            .unwrap_or_else(|e| panic!("{name}: cannot start {command:?}: {e}"));
        let (tx, rx) = mpsc::channel();
        let mut open = 0;
        if let Some(out) = child.stdout.take() {
            forward(out, Stream::Out, tx.clone());
            open += 1;
        }
        if let Some(err) = child.stderr.take() {
            forward(err, Stream::Err, tx);
            open += 1;
        }
        let stdin = child.stdin.take();
        Self {
            name: name.to_owned(),
            child,
            stdin,
            lines: rx,
            open,
            seen: Vec::new(),
        }
    }

    /// The process id.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Every line the peer printed so far.
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.seen
    }

    /// Waits until the peer prints a line for which `matches` holds, and
    /// returns it. `what` names the line in the failure message.
    ///
    /// # Panics
    /// When `timeout` passes first, or the peer closes both streams (it
    /// exited) first; the message carries everything the peer printed.
    pub fn wait_for(
        &mut self,
        what: &str,
        timeout: Duration,
        matches: impl Fn(&Line) -> bool,
    ) -> Line {
        let deadline = Instant::now() + timeout;
        loop {
            if self.open == 0 {
                let status = self.child.wait().ok();
                // zerodds-lint: allow no_panic_in_safe — the scenario fails here.
                panic!(
                    "{}: ended ({status:?}) before {what}\n{}",
                    self.name,
                    self.transcript()
                );
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(Message::Line(line)) => {
                    self.seen.push(line.clone());
                    if matches(&line) {
                        return line;
                    }
                }
                Ok(Message::Closed) => self.open -= 1,
                Err(RecvTimeoutError::Timeout) => {
                    // zerodds-lint: allow no_panic_in_safe — the scenario fails here.
                    panic!(
                        "{}: no {what} within {timeout:?}\n{}",
                        self.name,
                        self.transcript()
                    )
                }
                Err(RecvTimeoutError::Disconnected) => self.open = 0,
            }
        }
    }

    /// Waits for the event line `EVENT <name>…` of a role and returns what
    /// follows the name.
    pub fn wait_for_event(&mut self, name: &str, timeout: Duration) -> String {
        let prefix = format!("{EVENT}{name}");
        let line = self.wait_for(&format!("event `{name}`"), timeout, |line| {
            line.stream == Stream::Out
                && (line.text == prefix || line.text.starts_with(&format!("{prefix} ")))
        });
        line.text[prefix.len()..].trim_start().to_owned()
    }

    /// Writes `text` and a line end to the peer's standard input.
    ///
    /// # Panics
    /// When the peer's standard input is closed.
    pub fn send_line(&mut self, text: &str) {
        use std::io::Write as _;
        // zerodds-lint: allow no_panic_in_safe — the scenario fails here.
        let stdin = self
            .stdin
            .as_mut()
            .unwrap_or_else(|| panic!("{}: stdin is closed", self.name));
        // zerodds-lint: allow no_panic_in_safe — the scenario fails here.
        writeln!(stdin, "{text}")
            .and_then(|()| stdin.flush())
            .unwrap_or_else(|e| panic!("{}: write to stdin: {e}", self.name));
    }

    /// Closes the peer's standard input, the role's signal to finish, and
    /// leaves the peer running so the scenario can read what it prints after.
    pub fn close_stdin(&mut self) {
        drop(self.stdin.take());
    }

    /// Closes the peer's standard input and waits until it has closed both
    /// output streams, then reaps it.
    ///
    /// # Panics
    /// When the streams stay open past `timeout`.
    pub fn release(mut self, timeout: Duration) -> ExitStatus {
        drop(self.stdin.take());
        self.drain(timeout)
    }

    /// Kills the peer (`Child::kill`, SIGKILL on Unix) and reaps it: how a
    /// scenario ends a broker or a daemon, and how it crashes one.
    pub fn stop(mut self) -> ExitStatus {
        drop(self.stdin.take());
        let _ = self.child.kill();
        // zerodds-lint: allow no_panic_in_safe — wait after kill only fails
        // when the child was reaped already, which `Peer` never does twice.
        self.child
            .wait()
            .unwrap_or_else(|e| panic!("{}: wait: {e}", self.name))
    }

    /// [`Peer::stop`], and every line the peer printed: how a scenario ends
    /// a daemon whose output belongs in its failure message.
    ///
    /// # Panics
    /// When the streams stay open past `timeout` after the kill.
    pub fn stop_lines(mut self, timeout: Duration) -> (ExitStatus, Vec<Line>) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let status = self.drain(timeout);
        (status, std::mem::take(&mut self.seen))
    }

    /// Waits until the peer exits on its own (both streams closed) and
    /// returns its status.
    ///
    /// # Panics
    /// When the streams stay open past `timeout`.
    pub fn wait_exit(mut self, timeout: Duration) -> ExitStatus {
        self.drain(timeout)
    }

    /// [`Peer::wait_exit`], and every line the peer printed.
    ///
    /// # Panics
    /// When the streams stay open past `timeout`.
    pub fn wait_exit_lines(mut self, timeout: Duration) -> (ExitStatus, Vec<Line>) {
        let status = self.drain(timeout);
        (status, std::mem::take(&mut self.seen))
    }

    fn drain(&mut self, timeout: Duration) -> ExitStatus {
        let deadline = Instant::now() + timeout;
        while self.open > 0 {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(Message::Line(line)) => self.seen.push(line),
                Ok(Message::Closed) => self.open -= 1,
                Err(RecvTimeoutError::Timeout) => {
                    // zerodds-lint: allow no_panic_in_safe — the scenario fails here.
                    panic!(
                        "{}: did not end within {timeout:?}\n{}",
                        self.name,
                        self.transcript()
                    )
                }
                Err(RecvTimeoutError::Disconnected) => self.open = 0,
            }
        }
        // zerodds-lint: allow no_panic_in_safe — both streams are closed, so
        // the process has exited; wait reaps it.
        self.child
            .wait()
            .unwrap_or_else(|e| panic!("{}: wait: {e}", self.name))
    }

    /// Everything the peer printed, for a failure message.
    #[must_use]
    pub fn transcript(&self) -> String {
        let mut out = String::new();
        for line in &self.seen {
            let tag = match line.stream {
                Stream::Out => "out",
                Stream::Err => "err",
            };
            out.push_str(&format!("  [{} {tag}] {}\n", self.name, line.text));
        }
        out
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        if std::thread::panicking() {
            // The scenario failed: what every peer printed belongs in the
            // failure, not only the one whose wait ran out.
            eprintln!("{}", self.transcript());
        }
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn forward(stream: impl Read + Send + 'static, kind: Stream, tx: Sender<Message>) {
    std::thread::spawn(move || {
        for mut text in BufReader::new(stream).lines().map_while(Result::ok) {
            // libtest writes `test <name> ... ` without a line end before the
            // test's own output, so a role's first event shares its line.
            if let Some(at) = text.find(EVENT).filter(|&at| at > 0) {
                text = text.split_off(at);
            }
            if tx.send(Message::Line(Line { stream: kind, text })).is_err() {
                return;
            }
        }
        let _ = tx.send(Message::Closed);
    });
}

/// The runtime settings a daemon of a scenario starts with, through the
/// environment every ZeroDDS program reads (`RuntimeConfig`): the 20 ms tick
/// and 100 ms SPDP period of the scenario's own participants
/// (`dds::quick_config` where the feature `dcps` is on), so discovery between
/// them is as quick in both directions.
pub const DAEMON_ENV: [(&str, &str); 2] = [
    ("ZERODDS_TICK_PERIOD_MS", "20"),
    ("ZERODDS_SPDP_PERIOD_MS", "100"),
];

/// A command for the daemon `variable` names (see [`tools::built`]), with
/// [`DAEMON_ENV`].
#[must_use]
pub fn daemon_command(variable: &str) -> Command {
    let mut command = Command::new(tools::built(variable));
    command.envs(DAEMON_ENV);
    command
}

/// Parses the `key=value` words of an event line (`EVENT received n=200
/// id=3`) into a map.
#[must_use]
pub fn fields(text: &str) -> BTreeMap<String, String> {
    text.split_whitespace()
        .filter_map(|word| word.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// A TCP port on 127.0.0.1 that was free a moment ago, for a broker or a
/// daemon the scenario starts and tells where to listen.
///
/// # Panics
/// When no port can be bound.
#[must_use]
pub fn free_port() -> u16 {
    // zerodds-lint: allow no_panic_in_safe — a host without a free port
    // runs no scenario.
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap_or_else(|e| panic!("free port: {e}"))
}

/// Lower-case hex of `bytes`.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A directory of this test process alone, removed when the value drops.
pub struct Scratch {
    path: std::path::PathBuf,
}

impl Scratch {
    /// Creates `$TMPDIR/zerodds-integration-<label>-<pid>`, empty.
    ///
    /// # Panics
    /// When the directory cannot be created.
    #[must_use]
    pub fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "zerodds-integration-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        // zerodds-lint: allow no_panic_in_safe — a scenario without its
        // directory has no result.
        std::fs::create_dir_all(&path).unwrap_or_else(|e| panic!("mkdir {}: {e}", path.display()));
        Self { path }
    }

    /// The directory.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
