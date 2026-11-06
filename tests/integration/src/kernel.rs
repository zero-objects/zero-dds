// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The kernel's view of the sockets and mappings of a process (Linux
//! `procfs`, proc(5)).
//!
//! A scenario that configures a transport asks the kernel, not ZeroDDS,
//! whether the processes use it: a TCP connection between the writer's and
//! the reader's process, the Unix domain sockets both hold, a shared memory
//! segment both have mapped. ZeroDDS falling back to UDP without saying so
//! is the failure this catches.

use std::collections::BTreeSet;

/// The inodes of the sockets process `pid` holds (`/proc/<pid>/fd/*` that
/// read `socket:[<inode>]`).
#[must_use]
pub fn socket_inodes(pid: u32) -> BTreeSet<u64> {
    let mut inodes = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return inodes;
    };
    for entry in entries.flatten() {
        if let Ok(target) = std::fs::read_link(entry.path()) {
            let target = target.to_string_lossy();
            if let Some(inode) = target
                .strip_prefix("socket:[")
                .and_then(|rest| rest.strip_suffix(']'))
                .and_then(|n| n.parse().ok())
            {
                inodes.insert(inode);
            }
        }
    }
    inodes
}

/// One row of `/proc/net/{tcp,tcp6,udp,udp6}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InetSocket {
    /// Local port.
    pub local_port: u16,
    /// Remote port (0 for an unconnected socket).
    pub remote_port: u16,
    /// The `st` column: for TCP `0x01` is ESTABLISHED, `0x0A` LISTEN.
    pub state: u8,
    /// The socket's inode.
    pub inode: u64,
}

/// TCP state ESTABLISHED in the `st` column (include/net/tcp_states.h).
pub const TCP_ESTABLISHED: u8 = 0x01;

fn port_of(address: &str) -> Option<u16> {
    let (_, port) = address.rsplit_once(':')?;
    u16::from_str_radix(port, 16).ok()
}

fn inet_table(files: &[&str]) -> Vec<InetSocket> {
    let mut rows = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for line in text.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 10 {
                continue;
            }
            let (Some(local_port), Some(remote_port), Ok(state), Ok(inode)) = (
                port_of(cols[1]),
                port_of(cols[2]),
                u8::from_str_radix(cols[3], 16),
                cols[9].parse::<u64>(),
            ) else {
                continue;
            };
            rows.push(InetSocket {
                local_port,
                remote_port,
                state,
                inode,
            });
        }
    }
    rows
}

/// The TCP sockets (IPv4 and IPv6) process `pid` holds.
#[must_use]
pub fn tcp_sockets(pid: u32) -> Vec<InetSocket> {
    let mine = socket_inodes(pid);
    inet_table(&["/proc/net/tcp", "/proc/net/tcp6"])
        .into_iter()
        .filter(|s| mine.contains(&s.inode))
        .collect()
}

/// The UDP sockets (IPv4 and IPv6) process `pid` holds.
#[must_use]
pub fn udp_sockets(pid: u32) -> Vec<InetSocket> {
    let mine = socket_inodes(pid);
    inet_table(&["/proc/net/udp", "/proc/net/udp6"])
        .into_iter()
        .filter(|s| mine.contains(&s.inode))
        .collect()
}

/// The ESTABLISHED TCP connections between process `a` and process `b`,
/// as `(port of a, port of b)`: a socket of `a` whose remote port is the
/// local port of a socket of `b` that points back.
#[must_use]
pub fn tcp_connections_between(a: u32, b: u32) -> Vec<(u16, u16)> {
    let of_b = tcp_sockets(b);
    tcp_sockets(a)
        .into_iter()
        .filter(|s| s.state == TCP_ESTABLISHED)
        .filter(|s| {
            of_b.iter().any(|t| {
                t.state == TCP_ESTABLISHED
                    && t.local_port == s.remote_port
                    && t.remote_port == s.local_port
            })
        })
        .map(|s| (s.local_port, s.remote_port))
        .collect()
}

/// The paths of the Unix domain sockets process `pid` holds
/// (`/proc/net/unix`, bound sockets only).
#[must_use]
pub fn unix_socket_paths(pid: u32) -> BTreeSet<String> {
    let mine = socket_inodes(pid);
    let mut paths = BTreeSet::new();
    let Ok(text) = std::fs::read_to_string("/proc/net/unix") else {
        return paths;
    };
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 8 {
            continue;
        }
        if let Ok(inode) = cols[6].parse::<u64>()
            && mine.contains(&inode)
        {
            paths.insert(cols[7].to_owned());
        }
    }
    paths
}

/// The files under `/dev/shm` process `pid` has mapped
/// (`/proc/<pid>/maps`).
#[must_use]
pub fn shm_mappings(pid: u32) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/maps")) else {
        return paths;
    };
    for line in text.lines() {
        if let Some(at) = line.find("/dev/shm/") {
            let path = line[at..].trim_end_matches(" (deleted)").to_owned();
            paths.insert(path);
        }
    }
    paths
}
