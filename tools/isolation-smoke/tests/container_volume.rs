// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! zerodds-uds-transport-1.0 §5.2: the pattern of §5.1 across a real
//! container boundary. Two containers mount one volume at the transport's
//! `base_dir`, each binds its socket file there under its own UID, and
//! datagrams go both ways: the `isolation-smoke` binary of this package runs
//! as `echo` in one container and as `ping` in the other.
//!
//! - Docker: a named volume, the containers under UIDs 61001 and 61002, each
//!   with its own primary group and the shared group 61042 as a
//!   supplementary group (`--user`, `--group-add`: Kubernetes' `runAsUser`
//!   and `supplementalGroups`).
//! - Podman: each container in a user namespace of its own (`--uidmap`): UID
//!   1000 in both containers is UID 101000 on the host for one and 201000
//!   for the other; the shared group 61042 is mapped to itself in both.
//!   The volume is a host directory: Podman gives a new named volume, on its
//!   first mount, the owner and group of the container's user, which
//!   removes the shared group (measured on the Linux test host with Podman
//!   5.4.2).
//!
//! The root of the volume belongs to root and the group 61042 with mode
//! `3770`, as an operator prepares it (§5.1). A thread of the test watches
//! the volume from the host and records the owner, group and mode each
//! socket file had when it last saw it; both roles keep their socket bound
//! 1.5 s after the exchange (`--hold-ms`) so that both files are there at
//! once, with the mode the role set. The mounts
//! carry `:z`, the label both containers may use on an SELinux host; without
//! SELinux the runtimes ignore it.
//!
//! The containers start from `debian:trixie-slim` with the binary this
//! package built mounted read-only, so it must be built for the image's
//! glibc (Debian trixie or older). The tests need root on the machine the
//! container daemon runs on (they set the owner, group and mode of the
//! volume's root and read the socket files on the volume from the host), a
//! Docker daemon and Podman. The GitLab runners have no admin rights;
//! runner 8 starts Docker containers in colima's VM, where the volume lies
//! as well, not on the job's machine; no runner has Podman. So they are
//! manual tests on the Linux test host (`ci/manual-tests.tsv`, decision 11
//! of `internal/release/rc8-test-suite.md`):
//! ```bash
//! cargo test -p zerodds-isolation-smoke --test container_volume -- --ignored
//! ```
//! `ZERODDS_CONTAINER_LABEL=key=value` gives every container and volume the
//! test creates that label, for a host that tracks them by label.

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::collections::BTreeMap;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

const IMAGE: &str = "docker.io/library/debian:trixie-slim";
/// Where both containers mount the volume; the transport's `base_dir`.
const MOUNT: &str = "/run/zerodds-uds";
const BINARY: &str = "/usr/local/bin/isolation-smoke";
const SHARED_GID: u32 = 61_042;
const COUNT: usize = 50;
const HOLD_MS: u64 = 1_500;
/// The ids of the two roles and the socket files they bind.
const ECHO_ID: &str = "80";
const PING_ID: &str = "81";
const ECHO_SOCKET: &str = "00000000000000000000000000000080.sock";
const PING_SOCKET: &str = "00000000000000000000000000000081.sock";

fn smoke_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_isolation-smoke"))
}

fn label() -> Option<String> {
    std::env::var("ZERODDS_CONTAINER_LABEL").ok()
}

fn text(out: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A container runtime's command line and what the test created with it;
/// on drop the containers and the volume are removed.
struct Runtime {
    program: &'static str,
    containers: Vec<String>,
    volume: Option<String>,
}

impl Runtime {
    fn new(program: &'static str) -> Self {
        let version = Command::new(program)
            .arg("version")
            .output()
            .unwrap_or_else(|e| panic!("`{program}` cannot be started: {e}"));
        assert!(
            version.status.success(),
            "`{program} version` failed: {}",
            text(&version)
        );
        Self {
            program,
            containers: Vec::new(),
            volume: None,
        }
    }

    fn cmd(&self) -> Command {
        Command::new(self.program)
    }

    /// A named volume and its directory on the host.
    fn volume(&mut self, name: &str) -> PathBuf {
        let mut create = self.cmd();
        create.args(["volume", "create"]);
        if let Some(l) = label() {
            create.args(["--label", &l]);
        }
        let out = create.arg(name).output().expect("volume create");
        assert!(out.status.success(), "volume create: {}", text(&out));
        self.volume = Some(name.to_string());
        let out = self
            .cmd()
            .args(["volume", "inspect", "--format", "{{.Mountpoint}}", name])
            .output()
            .expect("volume inspect");
        assert!(out.status.success(), "volume inspect: {}", text(&out));
        PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
    }

    /// `run` with the common arguments: no network (the transport needs
    /// none), the volume and the binary mounted, the label.
    fn run_args(&self, name: &str, volume: &str, identity: &[String]) -> Command {
        let mut c = self.cmd();
        c.args(["run", "--name", name, "--network", "none"]);
        if let Some(l) = label() {
            c.args(["--label", &l]);
        }
        c.args(identity);
        c.args(["-v", &format!("{volume}:{MOUNT}:z")]);
        c.args(["-v", &format!("{}:{BINARY}:ro,z", smoke_bin().display())]);
        c
    }

    /// Starts the echo role in the background.
    fn start_echo(&mut self, name: &str, volume: &str, identity: &[String], extra: &[&str]) {
        let mut c = self.run_args(name, volume, identity);
        c.arg("-d").arg(IMAGE).arg(BINARY).args([
            "--transport=uds",
            "--role=echo",
            &format!("--local-id={ECHO_ID}"),
            &format!("--base-dir={MOUNT}"),
            &format!("--count={COUNT}"),
            &format!("--hold-ms={HOLD_MS}"),
        ]);
        c.args(extra);
        self.containers.push(name.to_string());
        let out = c.output().expect("run echo");
        assert!(out.status.success(), "start echo: {}", text(&out));
    }

    /// Runs the ping role in the foreground; its output.
    fn ping(&mut self, name: &str, volume: &str, identity: &[String], extra: &[&str]) -> Output {
        let mut c = self.run_args(name, volume, identity);
        c.arg(IMAGE).arg(BINARY).args([
            "--transport=uds",
            "--role=ping",
            &format!("--local-id={PING_ID}"),
            &format!("--peer-id={ECHO_ID}"),
            &format!("--base-dir={MOUNT}"),
            &format!("--count={COUNT}"),
            &format!("--hold-ms={HOLD_MS}"),
        ]);
        c.args(extra);
        self.containers.push(name.to_string());
        c.output().expect("run ping")
    }

    /// Waits for a background container; its exit code and log.
    fn finish(&self, name: &str) -> (i32, String) {
        let out = self.cmd().args(["wait", name]).output().expect("wait");
        assert!(out.status.success(), "wait {name}: {}", text(&out));
        let code = String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<i32>()
            .unwrap_or_else(|_| panic!("wait {name}: {}", text(&out)));
        let log = self.cmd().args(["logs", name]).output().expect("logs");
        (code, text(&log))
    }

    /// A container run once with `identity` so that a later start does not
    /// wait for the image or its layers to be prepared for the mapping.
    fn warm(&self, identity: &[String]) {
        let mut c = self.cmd();
        c.args(["run", "--rm", "--network", "none"]);
        if let Some(l) = label() {
            c.args(["--label", &l]);
        }
        let out = c
            .args(identity)
            .args([IMAGE, "true"])
            .output()
            .expect("warm");
        assert!(out.status.success(), "warm-up run: {}", text(&out));
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for name in &self.containers {
            let _ = self.cmd().args(["rm", "-f", name]).output();
        }
        if let Some(volume) = &self.volume {
            let _ = self.cmd().args(["volume", "rm", "-f", volume]).output();
        }
    }
}

/// Gives the volume's root the owner root, the group 61042 and the mode
/// `3770`, as an operator prepares a shared directory (§5.1).
fn prepare(root: &Path) {
    std::os::unix::fs::chown(root, Some(0), Some(SHARED_GID))
        .unwrap_or_else(|e| panic!("chown {}: {e} (the test needs root)", root.display()));
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o3770)).unwrap();
}

/// Owner, group and mode of a socket file as the host sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seen {
    uid: u32,
    gid: u32,
    mode: u32,
    socket: bool,
}

/// Records, for each file that appears in a directory, its owner, group
/// and mode as the host last saw them, until stopped. A file is read on
/// every poll: between the bind and the `chmod` of `socket_mode` a socket
/// file has the mode the umask gives, and only the last sighting, while the
/// role holds its socket (`--hold-ms`), shows the mode it set.
struct Watcher {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<BTreeMap<String, Seen>>,
}

impl Watcher {
    fn start(dir: PathBuf) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let mut seen = BTreeMap::new();
            while !flag.load(Ordering::Acquire) {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if let Ok(meta) = std::fs::symlink_metadata(entry.path()) {
                            seen.insert(
                                name,
                                Seen {
                                    uid: meta.uid(),
                                    gid: meta.gid(),
                                    mode: meta.permissions().mode() & 0o7777,
                                    socket: meta.file_type().is_socket(),
                                },
                            );
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            seen
        });
        Self { stop, handle }
    }

    fn finish(self) -> BTreeMap<String, Seen> {
        self.stop.store(true, Ordering::Release);
        self.handle.join().expect("watcher")
    }
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

fn unique(what: &str) -> String {
    format!("zd-uds-5-2-{what}-{}", std::process::id())
}

/// Asserts that both roles exchanged `COUNT` datagrams each way: the ping
/// got every echo, unchanged and from the echo's locator, and the echo
/// answered the ping's locator.
fn assert_exchanged(ping: &Output, echo: &(i32, String)) {
    let ping_log = text(ping);
    assert!(ping.status.success(), "ping: {ping_log}");
    assert!(
        ping_log.contains(&format!(
            "ping: {COUNT} echoes received from Uds:{ECHO_ID:0>32}"
        )),
        "ping: {ping_log}"
    );
    assert_eq!(echo.0, 0, "echo: {}", echo.1);
    assert!(
        echo.1.contains(&format!(
            "echo: {COUNT} datagrams echoed to Uds:{PING_ID:0>32}"
        )),
        "echo: {}",
        echo.1
    );
}

/// §5.2 with Docker: the echo under UID 61001 (primary group 61001), the ping
/// under UID 61002 (primary group 61002), both with the supplementary group
/// 61042, on a named volume whose root is `root:61042` with `3770`; both bind
/// with `socket_mode` `0660`. Fifty datagrams go from the ping to the echo
/// and back. On the host both socket files are on the volume, sockets with
/// mode `0660`, owned by 61001 and 61002, in the group 61042 that neither
/// container has as its primary group (the set-group-ID bit of the root).
#[test]
#[ignore = "needs root, Docker and the debian:trixie-slim image; a manual test on the Linux test host (ci/manual-tests.tsv)"]
fn uds_5_2_two_containers_under_different_uids_exchange_datagrams_over_a_volume_in_both_directions()
{
    let mut rt = Runtime::new("docker");
    let volume = unique("docker-vol");
    let root = rt.volume(&volume);
    prepare(&root);
    let echo = unique("docker-echo");
    let echo_identity = args(&["--user", "61001:61001", "--group-add", "61042"]);
    let ping_identity = args(&["--user", "61002:61002", "--group-add", "61042"]);
    let watcher = Watcher::start(root.clone());
    rt.start_echo(&echo, &volume, &echo_identity, &["--socket-mode=660"]);
    let ping = rt.ping(
        &unique("docker-ping"),
        &volume,
        &ping_identity,
        &["--socket-mode=660"],
    );
    let echo_result = rt.finish(&echo);
    let seen = watcher.finish();
    assert_exchanged(&ping, &echo_result);
    let expect = |uid| Seen {
        uid,
        gid: SHARED_GID,
        mode: 0o660,
        socket: true,
    };
    assert_eq!(seen.get(ECHO_SOCKET), Some(&expect(61_001)), "{seen:?}");
    assert_eq!(seen.get(PING_SOCKET), Some(&expect(61_002)), "{seen:?}");
}

/// §5.2 and §5.1 with Docker: the same two containers, the echo bound
/// without `socket_mode`. Its socket file keeps the mode of the container's
/// umask (no group write bit), and the ping's first send is refused with
/// "uds: permission denied on destination" (exit code 3).
#[test]
#[ignore = "needs root, Docker and the debian:trixie-slim image; a manual test on the Linux test host (ci/manual-tests.tsv)"]
fn uds_5_2_without_socket_mode_the_container_of_the_other_uid_is_refused() {
    let mut rt = Runtime::new("docker");
    let volume = unique("docker-refused-vol");
    let root = rt.volume(&volume);
    prepare(&root);
    let echo = unique("docker-refused-echo");
    let watcher = Watcher::start(root.clone());
    rt.start_echo(
        &echo,
        &volume,
        &args(&["--user", "61001:61001", "--group-add", "61042"]),
        &[],
    );
    let ping = rt.ping(
        &unique("docker-refused-ping"),
        &volume,
        &args(&["--user", "61002:61002", "--group-add", "61042"]),
        &["--socket-mode=660"],
    );
    let seen = watcher.finish();
    let ping_log = text(&ping);
    assert_eq!(ping.status.code(), Some(3), "ping: {ping_log}");
    assert!(
        ping_log.contains(
            "ping send[0] failed: transport I/O error: uds: permission denied on destination"
        ),
        "ping: {ping_log}"
    );
    let socket = seen
        .get(ECHO_SOCKET)
        .copied()
        .unwrap_or_else(|| panic!("{seen:?}"));
    assert!(socket.socket, "{socket:?}");
    assert_eq!(socket.uid, 61_001, "{socket:?}");
    assert_eq!(
        socket.mode & 0o022,
        0,
        "group or other write bit: {socket:?}"
    );
}

/// The `--uidmap` and `--gidmap` of a container whose IDs 0 to 65535 lie at
/// `base` on the host, except the shared group 61042, which is 61042 there.
fn mapped(base: u32) -> Vec<String> {
    let shared = SHARED_GID;
    vec![
        "--uidmap".into(),
        format!("0:{base}:65536"),
        "--gidmap".into(),
        format!("0:{base}:{shared}"),
        "--gidmap".into(),
        format!("{shared}:{shared}:1"),
        "--gidmap".into(),
        format!(
            "{}:{}:{}",
            shared + 1,
            base + shared + 1,
            65_536 - (shared + 1)
        ),
        "--user".into(),
        "1000:1000".into(),
        "--group-add".into(),
        shared.to_string(),
    ]
}

/// §5.2 with Podman and a user namespace per container: in both containers
/// the role runs as UID 1000, which is UID 101000 on the host for the echo
/// and UID 201000 for the ping; the shared group 61042 is 61042 in both.
/// Fifty datagrams go both ways over the volume, a host directory
/// `root:61042` with `3770`. On the host the two socket files carry the
/// mapped owners 101000 and 201000, the group 61042 and mode `0660`.
#[test]
#[ignore = "needs root, Podman and the debian:trixie-slim image; a manual test on the Linux test host (ci/manual-tests.tsv)"]
fn uds_5_2_two_containers_with_different_uid_mappings_exchange_datagrams_in_both_directions() {
    let mut rt = Runtime::new("podman");
    let dir = tempfile::Builder::new()
        .prefix("zd-uds-5-2-podman-")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap();
    prepare(dir.path());
    let volume = dir.path().display().to_string();
    let echo_identity = mapped(100_000);
    let ping_identity = mapped(200_000);
    rt.warm(&echo_identity);
    rt.warm(&ping_identity);
    let echo = unique("podman-echo");
    let watcher = Watcher::start(dir.path().to_path_buf());
    rt.start_echo(&echo, &volume, &echo_identity, &["--socket-mode=660"]);
    let ping = rt.ping(
        &unique("podman-ping"),
        &volume,
        &ping_identity,
        &["--socket-mode=660"],
    );
    let echo_result = rt.finish(&echo);
    let seen = watcher.finish();
    assert_exchanged(&ping, &echo_result);
    let expect = |uid| Seen {
        uid,
        gid: SHARED_GID,
        mode: 0o660,
        socket: true,
    };
    assert_eq!(seen.get(ECHO_SOCKET), Some(&expect(101_000)), "{seen:?}");
    assert_eq!(seen.get(PING_SOCKET), Some(&expect(201_000)), "{seen:?}");
}
