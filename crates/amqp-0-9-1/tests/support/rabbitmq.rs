// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A RabbitMQ node of one test alone: its own AMQP port, its own Erlang
//! distribution port and port mapper, its own data directory (in
//! `/dev/shm`, else under the temporary directory; [`node_base`]). It
//! starts with the test and stops with it; a RabbitMQ service already
//! running on the host is not touched. The default user `guest`/`guest`
//! may connect over the loopback interface.
//!
//! Shared by the tests of `zerodds-amqp-0-9-1` and `zerodds-amqp-endpoint`
//! that need a broker; they run in the integration pillar's job
//! `integration-bridges-vendor-amqp` (`ci/integration_scenarios.py`), where
//! `tests/integration/provision.sh rabbitmq` installs RabbitMQ and pika.
//! A missing `rabbitmq-server` fails the test.

#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long a node may take to print its startup line.
const STARTUP: Duration = Duration::from_secs(20);

/// The Python with pika and qpid-proton: `python3`, or the interpreter
/// `ZERODDS_AMQP_PYTHON` names. In the interop image `python3` is the venv
/// of the cyclonedds wheel, and Debian's pika and qpid-proton belong to
/// `/usr/bin/python3`.
#[must_use]
pub fn python() -> std::ffi::OsString {
    std::env::var_os("ZERODDS_AMQP_PYTHON").unwrap_or_else(|| "python3".into())
}

/// A running RabbitMQ node.
pub struct RabbitNode {
    child: Option<Child>,
    /// The AMQP port (0-9-1 and 1.0 on the same listener, RabbitMQ 4).
    pub port: u16,
    epmd_port: u16,
    dir: PathBuf,
    node: String,
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free port")
        .port()
}

/// Where a node keeps its data: `/dev/shm` when it is a tmpfs with at
/// least 256 MiB free, else the temporary directory.
///
/// On the CI runner host a node with its data in the container's root file system did
/// not finish booting in 20 s, twice (pipeline 8967, job 229698): no line
/// after `WAL: ra_log_wal init` from 21:46:16.6 UTC to the deadline, and
/// from 21:46:36.6 to 21:46:54.2. The job in the other slot (229706) spent
/// 21:46:06.7 to 21:46:54.3 in `Updating crates.io index`, 47.6 s, where
/// the same step took 3.6 to 6.4 s in the three other jobs of its kind
/// (229524 to 229526). What held both is not measured; both write to
/// the CI runner host's disk, whose stalls are internal/release/OPEN-ITEMS.md 97. The
/// `system_memory_high_watermark` alarm the node printed is os_mon's check
/// of the host's free memory and does not stop a boot. In a tmpfs the node
/// writes nothing to that disk; there its boot took 2.8 to 4.1 s in four
/// starts on the CI runner host, two of them right after 2 GiB were written to the root
/// file system (pipeline 8980, job 229742).
fn node_base() -> PathBuf {
    let shm = Path::new("/dev/shm");
    if shm.is_dir() && available_mib(shm).is_some_and(|mib| mib >= 256) {
        return shm.to_path_buf();
    }
    std::env::temp_dir()
}

/// The space free in the file system of `dir`, from `df -Pk`.
fn available_mib(dir: &Path) -> Option<u64> {
    let out = Command::new("df").arg("-Pk").arg(dir).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let kib: u64 = text
        .lines()
        .nth(1)?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()?;
    Some(kib / 1024)
}

/// The first of `names` that is an executable file in one of `dirs` or on
/// the PATH.
fn find(names: &[&str], dirs: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut all: Vec<PathBuf> = dirs.iter().map(PathBuf::from).collect();
    all.extend(std::env::split_paths(&path));
    for dir in all {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

impl RabbitNode {
    /// Starts a node and waits for its startup line.
    ///
    /// # Panics
    /// When `rabbitmq-server` is missing or does not report its startup
    /// within 20 s; the panic carries the node's output.
    pub fn start() -> Self {
        // The server script itself: Debian's /usr/sbin wrapper switches to
        // the `rabbitmq` user, who cannot write this test's directory.
        let server = find(&["rabbitmq-server"], &["/usr/lib/rabbitmq/bin"])
            .filter(|p| !p.starts_with("/usr/sbin"))
            .or_else(|| {
                let direct = PathBuf::from("/usr/lib/rabbitmq/bin/rabbitmq-server");
                direct.is_file().then_some(direct)
            })
            .expect("rabbitmq-server (tests/integration/provision.sh rabbitmq)");
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = node_base().join(format!("zerodds-rabbit-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("conf.d")).expect("node directory");
        std::fs::write(dir.join("enabled_plugins"), "[].\n").expect("enabled_plugins");
        let port = free_port();
        let epmd_port = free_port();
        let node = format!("zd{}x{n}@localhost", std::process::id());
        let mut command = Command::new(server);
        command
            .env("RABBITMQ_NODENAME", &node)
            .env("RABBITMQ_NODE_PORT", port.to_string())
            .env("RABBITMQ_DIST_PORT", free_port().to_string())
            .env("ERL_EPMD_PORT", epmd_port.to_string())
            // The node's port mapper takes `epmd -kill` while it still lists
            // the node (the stop below kills the node first, and the port
            // mapper may not have seen it go).
            .env("ERL_EPMD_RELAXED_COMMAND_CHECK", "1")
            .env("RABBITMQ_MNESIA_BASE", dir.join("mnesia"))
            .env("RABBITMQ_LOG_BASE", dir.join("log"))
            .env("RABBITMQ_LOGS", "-")
            .env("RABBITMQ_ENABLED_PLUGINS_FILE", dir.join("enabled_plugins"))
            .env("RABBITMQ_CONFIG_FILES", dir.join("conf.d"))
            .env("HOME", &dir)
            // A group of its own: `rabbitmq-server` runs the Erlang VM as a
            // child it waits on, and the stop below ends the whole group.
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn rabbitmq-server");
        let (tx, rx) = mpsc::channel::<String>();
        for stream in [
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        let node = Self {
            child: Some(child),
            port,
            epmd_port,
            dir,
            node,
        };
        let deadline = Instant::now() + STARTUP;
        let mut seen = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(line) => {
                    let up = line.contains("Server startup complete")
                        || line.contains("Starting broker... completed");
                    seen.push(line);
                    if up {
                        break;
                    }
                }
                Err(_) => panic!(
                    "RabbitMQ did not report its startup within {STARTUP:?}:\n{}",
                    seen.join("\n")
                ),
            }
        }
        // Keep draining the node's output so that it never blocks on a
        // full pipe.
        std::thread::spawn(move || for _ in rx {});
        node
    }

    /// `amqp://guest:guest@127.0.0.1:<port>`.
    #[must_use]
    pub fn url(&self) -> String {
        format!("amqp://guest:guest@127.0.0.1:{}", self.port)
    }

    /// `127.0.0.1:<port>`.
    #[must_use]
    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// The node's directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Runs `script` with Python's pika connected to this node as `c` and
    /// a channel as `ch`; returns its standard output. The interpreter is
    /// [`python`].
    ///
    /// # Panics
    /// When python3 or pika is missing or the script fails.
    #[must_use]
    pub fn pika(&self, script: &str) -> String {
        let out = Command::new(python())
            .arg("-c")
            .arg(format!(
                "import pika, sys\n\
                 c = pika.BlockingConnection(pika.ConnectionParameters('127.0.0.1', {port}, credentials=pika.PlainCredentials('guest','guest')))\n\
                 ch = c.channel()\n{script}\nc.close()\n",
                port = self.port
            ))
            .output()
            .expect("python3");
        assert!(
            out.status.success(),
            "pika failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

impl RabbitNode {
    /// Runs `rabbitmqctl -n <node> <args>` against this node (its port
    /// mapper, its cookie) and returns its standard output.
    ///
    /// # Panics
    /// When `rabbitmqctl` is missing or fails.
    #[must_use]
    pub fn ctl(&self, args: &[&str]) -> String {
        let ctl = find(&["rabbitmqctl"], &["/usr/lib/rabbitmq/bin"])
            .filter(|p| !p.starts_with("/usr/sbin"))
            .expect("rabbitmqctl beside rabbitmq-server");
        let out = Command::new(ctl)
            .arg("-n")
            .arg(&self.node)
            .args(args)
            .env("ERL_EPMD_PORT", self.epmd_port.to_string())
            .env("HOME", &self.dir)
            .output()
            .expect("rabbitmqctl");
        assert!(
            out.status.success(),
            "rabbitmqctl {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

impl Drop for RabbitNode {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
        }
        // The node's own port mapper, started with the node.
        if let Some(epmd) = find(&["epmd"], &["/usr/lib/erlang/bin"]) {
            let _ = Command::new(epmd)
                .arg("-kill")
                .env("ERL_EPMD_PORT", self.epmd_port.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
