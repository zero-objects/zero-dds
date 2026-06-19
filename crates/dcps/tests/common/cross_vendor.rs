//! Cross-vendor live-interop helpers for C5.5.
//!
//! Provides subprocess spawners for FastDDS and Cyclone tools that are
//! executed over `ssh llvm@llvm` on the lab host. The structs hold the
//! local child handle plus a `Drop` impl that kills the remote process
//! via `pkill` — otherwise `fastdds shape` / `ddsperf` processes would
//! linger on the VM.
//!
//! # Type strategy
//!
//! `RemoteSubprocess` is a generic wrapper: the `pkill_pattern`
//! identifies the remote program, the local `Child` holds the
//! SSH session. Without an SSH key, the lab password is piped in via
//! `sshpass` — acceptable for a lab setup, to be replaced by
//! key-based auth in production.
//!
//! # `#[ignore]` strategy
//!
//! All live tests are marked with `#[ignore]`. Locally:
//!
//! ```bash
//! cargo test -p zerodds-discovery --features live-interop \
//!     --test fastdds_live_pub -- --ignored --nocapture
//! ```
//!
//! If you cannot reach the lab, read `live_host_available()` — as soon
//! as the env var `LLVM_HOST_AVAILABLE=1` is not set AND `sshpass`
//! or the host connection is missing, the helpers skip with a clear
//! error message.

#![allow(dead_code, clippy::expect_used, clippy::print_stderr)]

use core::time::Duration;
use std::process::{Child, Command, Stdio};

/// SSH user for the lab host (see memory `reference_bench_hosts`).
pub const SSH_USER: &str = "llvm";
/// SSH password for the lab host. Lab-only — production needs key auth.
pub const SSH_PASS: &str = "llvm";
/// SSH hostname; resolved via `~/.ssh/config`.
pub const SSH_HOST: &str = "llvm";
/// Default timeout for live-subprocess bring-up.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Reliability variant for FastDDS tools. `fastdds shape` accepts
/// exactly these two variants (see `fastdds shape --help`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastReliability {
    BestEffort,
    Reliable,
}

/// Durability variant for FastDDS tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastDurability {
    Volatile,
    TransientLocal,
}

/// Complete QoS set for FastDDS tools.
#[derive(Debug, Clone, Copy)]
pub struct FastQos {
    pub reliability: FastReliability,
    pub durability: FastDurability,
}

impl FastQos {
    /// Default for the `fastdds shape` default profile.
    pub const DEFAULT: Self = Self {
        reliability: FastReliability::Reliable,
        durability: FastDurability::Volatile,
    };

    /// Maps to CLI flags for `fastdds shape`. `-r reliable|besteffort`,
    /// `-d volatile|transient_local`.
    pub fn cli_flags(&self) -> [&'static str; 4] {
        let r = match self.reliability {
            FastReliability::BestEffort => "besteffort",
            FastReliability::Reliable => "reliable",
        };
        let d = match self.durability {
            FastDurability::Volatile => "volatile",
            FastDurability::TransientLocal => "transient_local",
        };
        ["-r", r, "-d", d]
    }
}

/// Checks whether the live host is reachable. Tests without a lab can
/// skip early, before SSH timeouts drain the test.
pub fn live_host_available() -> bool {
    if std::env::var("LLVM_HOST_AVAILABLE").is_ok() {
        return true;
    }
    // Fallback: is `sshpass` available locally? Without `sshpass`, all
    // subprocess spawns would fail immediately.
    Command::new("sshpass")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Generic SSH subprocess wrapper. `pkill_pattern` is the
/// `pkill -f` argument with which `Drop` kills the remote process.
pub struct RemoteSubprocess {
    child: Child,
    pkill_pattern: String,
    label: String,
}

impl RemoteSubprocess {
    /// Spawns `cmd` over `sshpass+ssh` on `llvm`. `pkill_pattern`
    /// matches the remote process (regex-free, used as a substring
    /// with `pkill -f`).
    pub fn spawn(label: &str, cmd: &str, pkill_pattern: &str) -> std::io::Result<Self> {
        let child = Command::new("sshpass")
            .arg("-p")
            .arg(SSH_PASS)
            .arg("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("ConnectTimeout=5")
            .arg(format!("{SSH_USER}@{SSH_HOST}"))
            .arg(cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self {
            child,
            pkill_pattern: pkill_pattern.to_string(),
            label: label.to_string(),
        })
    }

    /// Synonym to make the caller code more readable.
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Drop for RemoteSubprocess {
    fn drop(&mut self) {
        // Best-effort: kill the remote process, then the local SSH child.
        let cmd = format!("pkill -f '{}' || true", self.pkill_pattern);
        let _ = Command::new("sshpass")
            .arg("-p")
            .arg(SSH_PASS)
            .arg("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg(format!("{SSH_USER}@{SSH_HOST}"))
            .arg(&cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts `fastdds shape publisher` on the Linux bench host.
///
/// We set `--domain` explicitly; the topic is one of the three
/// shape topics (`Square`, `Triangle`, `Circle`) per FastDDS convention.
/// Color `RED` as the default, because that is the first available one
/// for the `fastdds shape` default profile.
pub fn start_fastdds_pub(
    topic: &str,
    domain: u32,
    qos: &FastQos,
) -> std::io::Result<RemoteSubprocess> {
    let flags = qos.cli_flags();
    let cmd = format!(
        "timeout 60 fastdds shape publisher --domain {} {} {} {} {} --topic {} --color RED \
         > /tmp/fastdds_pub_{}.log 2>&1",
        domain, flags[0], flags[1], flags[2], flags[3], topic, topic
    );
    RemoteSubprocess::spawn(
        &format!("fastdds-pub:{topic}"),
        &cmd,
        &format!("fastdds shape publisher --domain {domain}"),
    )
}

/// Starts `fastdds shape subscriber` on the Linux bench host.
pub fn start_fastdds_sub(
    topic: &str,
    domain: u32,
    qos: &FastQos,
) -> std::io::Result<RemoteSubprocess> {
    let flags = qos.cli_flags();
    let cmd = format!(
        "timeout 60 fastdds shape subscriber --domain {} {} {} {} {} --topic {} \
         > /tmp/fastdds_sub_{}.log 2>&1",
        domain, flags[0], flags[1], flags[2], flags[3], topic, topic
    );
    RemoteSubprocess::spawn(
        &format!("fastdds-sub:{topic}"),
        &cmd,
        &format!("fastdds shape subscriber --domain {domain}"),
    )
}

/// Starts `fastdds discovery` in server mode. The discovery-server spec
/// lives in FastDDS' server-client discovery protocol (see FastDDS 2.9
/// user manual §16). We set the GUID prefix to `44.53.00.5f.45.50...`
/// (the default prefix documented by eProsima as a reference).
pub fn start_fastdds_discovery_server(domain: u32, port: u16) -> std::io::Result<RemoteSubprocess> {
    let cmd = format!(
        "timeout 60 fastdds discovery -i 0 -d {domain} -p {port} \
         > /tmp/fastdds_disc_{domain}.log 2>&1"
    );
    RemoteSubprocess::spawn(
        &format!("fastdds-disc:{domain}"),
        &cmd,
        &format!("fastdds discovery -i 0 -d {domain}"),
    )
}

/// Cyclone pub via `ddsperf` — needed for gap-filler tests
/// (cyclone_typelookup_sub_side). `-D` is the duration in
/// seconds, `-i` is the domain ID (see memory
/// `reference_ddsperf_flags` — beware the flag trap).
pub fn start_cyclone_ddsperf_pub(
    domain: u32,
    duration_secs: u32,
) -> std::io::Result<RemoteSubprocess> {
    let cmd = format!(
        "CYCLONEDDS_URI=file:///tmp/cyc.xml timeout {} ddsperf -i {} -D {} pub 1Hz \
         > /tmp/cyc_pub.log 2>&1",
        duration_secs + 5,
        domain,
        duration_secs,
    );
    RemoteSubprocess::spawn(
        &format!("cyc-pub:{domain}"),
        &cmd,
        &format!("ddsperf -i {domain}"),
    )
}

/// Cyclone sub via `ddsperf`.
pub fn start_cyclone_ddsperf_sub(
    domain: u32,
    duration_secs: u32,
) -> std::io::Result<RemoteSubprocess> {
    let cmd = format!(
        "CYCLONEDDS_URI=file:///tmp/cyc.xml timeout {} ddsperf -i {} -D {} sub \
         > /tmp/cyc_sub.log 2>&1",
        duration_secs + 5,
        domain,
        duration_secs,
    );
    RemoteSubprocess::spawn(
        &format!("cyc-sub:{domain}"),
        &cmd,
        &format!("ddsperf -i {domain}"),
    )
}

/// Detects the local LAN IP — on macOS via `ipconfig`, on Linux via
/// `hostname -I`. Needed for SPDP beacons (Cyclone/FastDDS
/// send unicast replies to the locator announced in the beacon).
pub fn detect_local_interface() -> Option<std::net::Ipv4Addr> {
    if cfg!(target_os = "macos") {
        if let Ok(out) = Command::new("ipconfig").args(["getifaddr", "en0"]).output() {
            if let Ok(s) = core::str::from_utf8(&out.stdout) {
                if let Ok(ip) = s.trim().parse::<std::net::Ipv4Addr>() {
                    return Some(ip);
                }
            }
        }
    } else if let Ok(out) = Command::new("hostname").arg("-I").output() {
        if let Ok(s) = core::str::from_utf8(&out.stdout) {
            for tok in s.split_whitespace() {
                if let Ok(ip) = tok.parse::<std::net::Ipv4Addr>() {
                    if !ip.is_loopback() && !ip.is_link_local() {
                        return Some(ip);
                    }
                }
            }
        }
    }
    None
}
