// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A PostgreSQL server of one test alone: its own data directory and its own
//! Unix socket directory under the temporary directory, no TCP listener. It
//! starts with the test and stops with it; a PostgreSQL service of the host
//! is not touched.
//!
//! The tests that use it run in the integration pillar's job
//! `integration-services-vendor` (`ci/integration_scenarios.py`), where
//! `tests/integration/provision.sh postgresql` installs Debian's server
//! package. A missing `initdb` or `postgres` fails the test. PostgreSQL
//! refuses to run as root, so as root the server runs as the `postgres`
//! user the package creates (`runuser`).

#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long a server may take to report that it accepts connections.
const STARTUP: Duration = Duration::from_secs(20);

/// The directory of the server programs: `ZERODDS_PG_BINDIR`, the
/// directory of `initdb` on the PATH, or the newest
/// `/usr/lib/postgresql/<version>/bin` (Debian keeps them off the PATH).
fn bindir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ZERODDS_PG_BINDIR") {
        return PathBuf::from(dir);
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(dir) = std::env::split_paths(&path).find(|d| d.join("initdb").is_file()) {
        return dir;
    }
    let mut versions: Vec<(u32, PathBuf)> = std::fs::read_dir("/usr/lib/postgresql")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let version = e.file_name().to_str()?.parse().ok()?;
            let bin = e.path().join("bin");
            bin.join("initdb").is_file().then_some((version, bin))
        })
        .collect();
    versions.sort();
    versions
        .pop()
        .map(|(_, bin)| bin)
        .expect("PostgreSQL's initdb and postgres (tests/integration/provision.sh postgresql)")
}

fn id(args: &[&str]) -> u32 {
    let out = Command::new("id").args(args).output().expect("id");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .expect("a numeric id")
}

/// A running server.
pub struct Server {
    child: Option<Child>,
    dir: PathBuf,
}

impl Server {
    /// Creates a cluster with `initdb` and starts `postgres` on it; waits
    /// for "ready to accept connections".
    ///
    /// # Panics
    /// When the programs are missing, `initdb` fails, or the server does not
    /// report readiness within 20 s; the panic carries the output.
    pub fn start() -> Self {
        let bin = bindir();
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("zerodds-pg-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("server directory");
        // As root, everything the server writes belongs to `postgres`.
        let as_root = id(&["-u"]) == 0;
        let run_as = |program: PathBuf| -> Command {
            if as_root {
                let mut c = Command::new("runuser");
                c.args(["-u", "postgres", "--"]).arg(program);
                c
            } else {
                Command::new(program)
            }
        };
        if as_root {
            let (uid, gid) = (id(&["-u", "postgres"]), id(&["-g", "postgres"]));
            std::os::unix::fs::chown(&dir, Some(uid), Some(gid)).expect("chown");
        }
        let data = dir.join("data");
        let out = run_as(bin.join("initdb"))
            .arg("-D")
            .arg(&data)
            .args([
                "-U",
                "zerodds",
                "--auth=trust",
                "--no-sync",
                "-E",
                "UTF8",
                "--locale=C",
            ])
            .output()
            .expect("spawn initdb");
        assert!(
            out.status.success(),
            "initdb failed:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let mut child = run_as(bin.join("postgres"))
            .arg("-D")
            .arg(&data)
            .arg("-k")
            .arg(&dir)
            .args(["-c", "listen_addresses=", "-c", "fsync=off", "-p", "5432"])
            // A group of its own: as root `runuser` starts the server as its
            // child, and the stop below ends the whole group.
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn postgres");
        let (tx, rx) = mpsc::channel::<String>();
        let stderr = child.stderr.take().expect("stderr");
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let server = Self {
            child: Some(child),
            dir,
        };
        let deadline = Instant::now() + STARTUP;
        let mut seen = Vec::new();
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(line) => {
                    let up = line.contains("ready to accept connections");
                    seen.push(line);
                    if up {
                        break;
                    }
                }
                Err(_) => panic!(
                    "PostgreSQL did not report readiness within {STARTUP:?}:\n{}",
                    seen.join("\n")
                ),
            }
        }
        std::thread::spawn(move || for _ in rx {});
        server
    }

    /// The libpq connection string of the server's database `postgres`.
    #[must_use]
    pub fn url(&self) -> String {
        format!(
            "host={} port=5432 user=zerodds dbname=postgres",
            self.dir.display()
        )
    }

    /// The server's directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Server {
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
