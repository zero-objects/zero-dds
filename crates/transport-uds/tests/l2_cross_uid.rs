// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-uds-transport-1.0 §5.1: the permission model of a shared
//! directory, across a real UID boundary.
//!
//! Two processes under two different non-root UIDs, each with a primary
//! group of its own and the shared group as a supplementary group, bind
//! their sockets in one directory — the topology of two containers with
//! different users on one mounted volume, without the container runtime
//! (that is §5.2). The test binary starts itself under those IDs
//! (`setgroups`, `setgid`, `setuid` before `exec`): an echo process that
//! binds first, creating the directory, and a ping process that sends to
//! it and waits for the echo. The directory lies in a parent of the shared
//! group with the set-group-ID bit, as a volume an operator prepared, so
//! the directory the echo process creates takes the shared group.
//!
//! What it shows: with `socket_mode` `0o660` and `base_dir_mode` `0o3770`
//! the socket files take the shared group from the directory, not the
//! binder's primary group, and the two UIDs exchange datagrams in both
//! directions; without the set-group-ID bit (`0o770`) a socket file keeps
//! the binder's primary group and the other UID's send is refused; without
//! `socket_mode` (umask `022`, socket file `0755`) the other UID's send is
//! refused and named as a permission error; a directory the first binder
//! creates with the default mode `0700` keeps the other UID from binding
//! at all; under `0o3770` the other UID cannot remove a socket file, under
//! `0o2770` (no sticky bit) it can; a set-group-ID bit the kernel drops
//! (the binder is not in the directory's group) fails the bind.
//!
//! Starting a process under another UID needs root. The CI containers and
//! the Linux test host run the tests as root; under another user these
//! tests fail with a message that says so.
#![cfg(target_os = "linux")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs
)]

use std::env;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use zerodds_rtps::wire_types::Locator;
use zerodds_transport::Transport;
use zerodds_transport_uds::{UdsConfig, UdsTransport, socket_path};

const ROLE: &str = "ZERODDS_UDS_XUID_ROLE";
const TARGET: &str = "ZERODDS_UDS_XUID_TARGET";
const VOLUME: &str = "ZERODDS_UDS_XUID_VOLUME";
const SOCKET_MODE: &str = "ZERODDS_UDS_XUID_SOCKET_MODE";
const DIR_MODE: &str = "ZERODDS_UDS_XUID_DIR_MODE";

/// The two users and their shared group; numbers no account on the hosts
/// uses. The primary group of each user has the number of the user.
const UID_ECHO: u32 = 61_001;
const UID_PING: u32 = 61_002;
const GID_SHARED: u32 = 61_042;
/// A group neither user is in.
const GID_FOREIGN: u32 = 61_099;

const ID_ECHO: [u8; 16] = [0x5a, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01];
const ID_PING: [u8; 16] = [0x5a, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02];

const PING: &[u8] = b"zerodds-uds-cross-uid-ping";

/// Exit codes of the ping process.
const BIND_FAILED: i32 = 2;
const SEND_FAILED: i32 = 3;
const NO_ECHO: i32 = 4;

/// Runs the role the parent gave this process, if any; `true` when this
/// process is a child.
fn run_child_role() -> bool {
    let Ok(role) = env::var(ROLE) else {
        return false;
    };
    if role == "unlink" {
        // Removes the socket file of another UID, as an attacker in the
        // shared group would to put its own socket or a symlink there.
        let target = PathBuf::from(env::var(TARGET).unwrap());
        match fs::remove_file(&target) {
            Ok(()) => println!("unlink: removed"),
            Err(e) => println!("unlink error: {:?}", e.kind()),
        }
        return true;
    }
    // A fixed umask, so that "no socket_mode" means a socket file of 0755.
    // SAFETY: umask has no preconditions; nothing else in this child
    // creates files concurrently.
    unsafe {
        libc::umask(0o022);
    }
    let volume = PathBuf::from(env::var(VOLUME).unwrap());
    let octal = |name: &str| {
        env::var(name)
            .ok()
            .map(|v| u32::from_str_radix(&v, 8).unwrap())
    };
    let config = UdsConfig {
        base_dir: volume.clone(),
        max_datagram: 4096,
        recv_timeout: Some(Duration::from_secs(5)),
        socket_mode: octal(SOCKET_MODE),
        base_dir_mode: octal(DIR_MODE).unwrap_or(zerodds_transport_uds::DEFAULT_BASE_DIR_MODE),
        ..UdsConfig::default()
    };
    let (own, peer) = if role == "echo" {
        (ID_ECHO, ID_PING)
    } else {
        (ID_PING, ID_ECHO)
    };
    let transport = match UdsTransport::bind(own, config) {
        Ok(t) => t,
        Err(e) => {
            println!("bind error: {:?}: {e}", e.kind());
            println!("volume exists: {}", volume.exists());
            std::process::exit(BIND_FAILED);
        }
    };
    let meta = fs::symlink_metadata(socket_path(&volume, own)).unwrap();
    // SAFETY: getegid has no preconditions and cannot fail.
    let egid = unsafe { libc::getegid() };
    println!(
        "bound uid={} gid={} mode={:o} egid={egid}",
        meta.uid(),
        meta.gid(),
        meta.mode() & 0o7777
    );
    if role == "echo" {
        let got = transport.recv().expect("a datagram to echo");
        assert_eq!(got.source, Locator::uds(peer), "the ping's locator");
        transport.send(&got.source, &got.data).expect("echo");
        println!("echoed");
    } else {
        if let Err(e) = transport.send(&Locator::uds(peer), PING) {
            println!("send error: {e}");
            std::process::exit(SEND_FAILED);
        }
        match transport.recv() {
            Ok(got) if &got.data[..] == PING && got.source == Locator::uds(peer) => {
                println!("echo received");
            }
            other => {
                println!("no echo: {other:?}");
                std::process::exit(NO_ECHO);
            }
        }
    }
    true
}

/// A directory the two UIDs can reach: the test binary copied out of the
/// build directory (which another UID cannot read), and a parent for the
/// shared directory, owned by root with the group `parent_gid` and the
/// mode `parent_mode` — a volume an operator prepared.
struct Stage {
    _tmp: tempfile::TempDir,
    exe: PathBuf,
    volume: PathBuf,
}

/// The stage of most tests: a parent of the shared group with the
/// set-group-ID bit, writable by that group.
fn stage() -> Stage {
    stage_with_parent(GID_SHARED, 0o2770)
}

fn stage_with_parent(parent_gid: u32, parent_mode: u32) -> Stage {
    // SAFETY: geteuid has no preconditions and cannot fail.
    let euid = unsafe { libc::geteuid() };
    assert_eq!(
        euid, 0,
        "these tests start processes under other UIDs, which needs root \
         (the CI containers and the Linux test host run them as root)"
    );
    let tmp = tempfile::tempdir().unwrap();
    fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let exe = tmp.path().join("l2-cross-uid");
    fs::copy(env::current_exe().unwrap(), &exe).unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
    let parent = tmp.path().join("volumes");
    fs::create_dir(&parent).unwrap();
    std::os::unix::fs::chown(&parent, Some(0), Some(parent_gid)).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(parent_mode)).unwrap();
    Stage {
        volume: parent.join("volume"),
        _tmp: tmp,
        exe,
    }
}

/// Starts the test binary in `role` under `uid`, with the primary group
/// of the same number and `groups` as its supplementary groups.
/// `Command::uid` would drop every supplementary group (`setgroups(0)`)
/// before a `pre_exec` hook runs, so the hook sets all three IDs itself,
/// as root, in this order.
fn spawn_as(stage: &Stage, test: &str, role: &str, uid: u32, groups: &[u32]) -> Command {
    let mut command = Command::new(&stage.exe);
    let groups: Vec<libc::gid_t> = groups.to_vec();
    let hook = move || -> std::io::Result<()> {
        // SAFETY: `groups` holds `groups.len()` group IDs, of which the
        // kernel reads no more; `setgid` and `setuid` take no pointers.
        let failed = unsafe {
            libc::setgroups(groups.len(), groups.as_ptr()) != 0
                || libc::setgid(uid) != 0
                || libc::setuid(uid) != 0
        };
        if failed {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    };
    // SAFETY: the hook runs in the forked child before `exec` and calls
    // only `setgroups`, `setgid` and `setuid`, which are async-signal-safe
    // system calls; `groups` was allocated before the fork.
    unsafe {
        command.pre_exec(hook);
    }
    command
        .current_dir(stage.exe.parent().unwrap())
        .env(ROLE, role)
        .args(["--exact", test, "--nocapture", "--test-threads", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}

fn spawn(
    stage: &Stage,
    test: &str,
    role: &str,
    uid: u32,
    socket_mode: Option<u32>,
    dir_mode: Option<u32>,
) -> Child {
    spawn_in_groups(stage, test, role, uid, &[GID_SHARED], socket_mode, dir_mode)
}

fn spawn_in_groups(
    stage: &Stage,
    test: &str,
    role: &str,
    uid: u32,
    groups: &[u32],
    socket_mode: Option<u32>,
    dir_mode: Option<u32>,
) -> Child {
    let mut command = spawn_as(stage, test, role, uid, groups);
    command
        .env(VOLUME, &stage.volume)
        .env_remove(SOCKET_MODE)
        .env_remove(DIR_MODE);
    if let Some(mode) = socket_mode {
        command.env(SOCKET_MODE, format!("{mode:o}"));
    }
    if let Some(mode) = dir_mode {
        command.env(DIR_MODE, format!("{mode:o}"));
    }
    command.spawn().unwrap()
}

/// Waits until the socket at `path` exists and, with a `socket_mode`,
/// carries it (the bind sets the mode right after binding), failing with
/// the child's output if the child ends first or 5 s pass.
fn wait_for(path: &Path, child: &mut Child, socket_mode: Option<u32>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let ready = || {
        fs::symlink_metadata(path)
            .is_ok_and(|meta| socket_mode.is_none_or(|mode| meta.mode() & 0o777 == mode))
    };
    while !ready() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!(
                "the echo process ended with {status} before binding: {}",
                output(child)
            );
        }
        assert!(Instant::now() < deadline, "no socket at {path:?} after 5 s");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn output(child: &mut Child) -> String {
    let mut text = String::new();
    if let Some(mut out) = child.stdout.take() {
        out.read_to_string(&mut text).unwrap();
    }
    text
}

/// Waits for the child, at most 10 s.
fn finish(child: &mut Child) -> (ExitStatus, String) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return (status, output(child));
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            return (status, output(child));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn stop(child: &mut Child) -> String {
    child.kill().unwrap();
    child.wait().unwrap();
    output(child)
}

/// The line a bound child prints for its socket file.
fn bound(uid: u32, gid: u32, mode: u32) -> String {
    format!("bound uid={uid} gid={gid} mode={mode:o} egid={uid}")
}

#[test]
fn uds_5_1_two_uids_with_a_shared_group_exchange_datagrams_in_both_directions() {
    if run_child_role() {
        return;
    }
    let test = "uds_5_1_two_uids_with_a_shared_group_exchange_datagrams_in_both_directions";
    let stage = stage();
    let mut echo = spawn(&stage, test, "echo", UID_ECHO, Some(0o660), Some(0o3770));
    wait_for(&socket_path(&stage.volume, ID_ECHO), &mut echo, Some(0o660));
    let mut ping = spawn(&stage, test, "ping", UID_PING, Some(0o660), Some(0o3770));
    let (ping_status, ping_out) = finish(&mut ping);
    let (echo_status, echo_out) = finish(&mut echo);
    assert!(ping_status.success(), "ping: {ping_status}: {ping_out}");
    assert!(echo_status.success(), "echo: {echo_status}: {echo_out}");
    // The socket files carry the shared group, which neither process has
    // as its primary group: the set-group-ID bit of the directory gave it.
    assert!(
        echo_out.contains(&bound(UID_ECHO, GID_SHARED, 0o660)),
        "{echo_out}"
    );
    assert!(echo_out.contains("echoed"), "{echo_out}");
    assert!(
        ping_out.contains(&bound(UID_PING, GID_SHARED, 0o660)),
        "{ping_out}"
    );
    assert!(ping_out.contains("echo received"), "{ping_out}");
    let dir = fs::metadata(&stage.volume).unwrap();
    assert_eq!(
        (dir.uid(), dir.gid(), dir.mode() & 0o7777),
        (UID_ECHO, GID_SHARED, 0o3770),
        "the first binder created the shared directory with base_dir_mode"
    );
}

#[test]
fn uds_5_1_without_the_set_group_id_bit_a_socket_file_keeps_the_binders_group_and_is_refused() {
    if run_child_role() {
        return;
    }
    let test =
        "uds_5_1_without_the_set_group_id_bit_a_socket_file_keeps_the_binders_group_and_is_refused";
    let stage = stage();
    let mut echo = spawn(&stage, test, "echo", UID_ECHO, Some(0o660), Some(0o770));
    wait_for(&socket_path(&stage.volume, ID_ECHO), &mut echo, Some(0o660));
    let mut ping = spawn(&stage, test, "ping", UID_PING, Some(0o660), Some(0o770));
    let (ping_status, ping_out) = finish(&mut ping);
    let echo_out = stop(&mut echo);
    let dir = fs::metadata(&stage.volume).unwrap();
    assert_eq!(
        (dir.gid(), dir.mode() & 0o7777),
        (GID_SHARED, 0o770),
        "the directory is of the shared group, without the set-group-ID bit"
    );
    assert!(
        echo_out.contains(&bound(UID_ECHO, UID_ECHO, 0o660)),
        "the socket file took the binder's primary group: {echo_out}"
    );
    // The other UID may bind in the directory (group write), but the
    // echo's socket file grants its group nothing.
    assert!(
        ping_out.contains(&bound(UID_PING, UID_PING, 0o660)),
        "{ping_out}"
    );
    assert_eq!(ping_status.code(), Some(SEND_FAILED), "{ping_out}");
    assert!(
        ping_out.contains("uds: permission denied on destination"),
        "{ping_out}"
    );
}

#[test]
fn uds_5_1_under_3770_the_other_uid_cannot_remove_a_socket_file_it_can_remove_under_2770() {
    if run_child_role() {
        return;
    }
    let test =
        "uds_5_1_under_3770_the_other_uid_cannot_remove_a_socket_file_it_can_remove_under_2770";
    for (dir_mode, expected) in [
        (0o3770, "unlink error: PermissionDenied"),
        (0o2770, "unlink: removed"),
    ] {
        let stage = stage();
        let mut echo = spawn(&stage, test, "echo", UID_ECHO, Some(0o660), Some(dir_mode));
        let target = socket_path(&stage.volume, ID_ECHO);
        wait_for(&target, &mut echo, Some(0o660));
        let mut unlink = spawn_as(&stage, test, "unlink", UID_PING, &[GID_SHARED])
            .env(TARGET, &target)
            .spawn()
            .unwrap();
        let (status, out) = finish(&mut unlink);
        let still_there = fs::symlink_metadata(&target).is_ok();
        stop(&mut echo);
        assert!(status.success(), "{dir_mode:o}: {status}: {out}");
        assert!(out.contains(expected), "{dir_mode:o}: {out}");
        assert_eq!(
            still_there,
            dir_mode == 0o3770,
            "{dir_mode:o}: the socket file of uid {UID_ECHO} after the other uid's unlink"
        );
    }
}

#[test]
fn uds_5_1_a_set_group_id_bit_the_kernel_drops_fails_the_bind() {
    if run_child_role() {
        return;
    }
    let test = "uds_5_1_a_set_group_id_bit_the_kernel_drops_fails_the_bind";
    // The parent is of a group the echo process is not in; the directory
    // it creates there takes that group, and the kernel drops the
    // set-group-ID bit of a chmod by a process outside the group.
    let stage = stage_with_parent(GID_FOREIGN, 0o3777);
    let mut echo = spawn_in_groups(
        &stage,
        test,
        "echo",
        UID_ECHO,
        &[],
        Some(0o660),
        Some(0o3770),
    );
    let (status, out) = finish(&mut echo);
    assert_eq!(status.code(), Some(BIND_FAILED), "{out}");
    assert!(
        out.contains("bind error: PermissionDenied: uds base_dir_mode 3770: the kernel set 1770"),
        "{out}"
    );
    assert!(out.contains("removed it again"), "{out}");
    assert!(out.contains("volume exists: false"), "{out}");
}

#[test]
fn uds_5_1_without_socket_mode_the_other_uid_is_refused_with_permission_denied() {
    if run_child_role() {
        return;
    }
    let test = "uds_5_1_without_socket_mode_the_other_uid_is_refused_with_permission_denied";
    let stage = stage();
    let mut echo = spawn(&stage, test, "echo", UID_ECHO, None, Some(0o3770));
    wait_for(&socket_path(&stage.volume, ID_ECHO), &mut echo, None);
    let mut ping = spawn(&stage, test, "ping", UID_PING, None, Some(0o3770));
    let (ping_status, ping_out) = finish(&mut ping);
    let echo_out = stop(&mut echo);
    assert!(
        echo_out.contains("mode=755"),
        "the umask's mode: {echo_out}"
    );
    assert_eq!(ping_status.code(), Some(SEND_FAILED), "{ping_out}");
    assert!(
        ping_out.contains("uds: permission denied on destination"),
        "{ping_out}"
    );
}

#[test]
fn uds_5_1_a_base_dir_created_with_the_default_mode_shuts_the_other_uid_out() {
    if run_child_role() {
        return;
    }
    let test = "uds_5_1_a_base_dir_created_with_the_default_mode_shuts_the_other_uid_out";
    let stage = stage();
    let mut echo = spawn(&stage, test, "echo", UID_ECHO, Some(0o660), None);
    wait_for(&socket_path(&stage.volume, ID_ECHO), &mut echo, Some(0o660));
    let mut ping = spawn(&stage, test, "ping", UID_PING, Some(0o660), None);
    let (ping_status, ping_out) = finish(&mut ping);
    stop(&mut echo);
    assert_eq!(
        fs::metadata(&stage.volume).unwrap().mode() & 0o7777,
        0o700,
        "the default base_dir_mode"
    );
    assert_eq!(ping_status.code(), Some(BIND_FAILED), "{ping_out}");
    assert!(
        ping_out.contains("bind error: PermissionDenied"),
        "{ping_out}"
    );
}
