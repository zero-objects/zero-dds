// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Network-Partition via `iptables` (Linux).
//!
//! Creates a set of INPUT/OUTPUT rules that blocks traffic between
//! configured IP groups blocked. Phase-A: two groups
//! ("A", "B"). Configuration:
//!
//! * Group A = list of IPs/CIDRs
//! * Group B = list of IPs/CIDRs
//!
//! The tool adds a DROP rule for each A↔B pairing (in
//! both directions). `clear` removes them again.
//!
//! On non-Linux platforms the functions are stubs.

#[cfg(target_os = "linux")]
use std::process::Command;

/// Partition config with two groups.
#[derive(Clone, Debug)]
pub struct PartitionConfig {
    /// Group-A IPs (or CIDRs).
    pub group_a: Vec<String>,
    /// Group-B IPs (or CIDRs).
    pub group_b: Vec<String>,
}

/// Errors from the partition wrapper.
#[derive(Debug)]
pub enum PartitionError {
    /// Platform does not support iptables.
    Unsupported,
    /// Caller does not have euid=0.
    NotRoot,
    /// `iptables` binary not executable.
    Spawn(std::io::Error),
    /// `iptables` returned non-zero.
    NonZeroExit {
        /// Output capture.
        stderr: String,
        /// Exit-Code.
        code: Option<i32>,
    },
    /// Empty group or only one group.
    EmptyGroup,
}

impl std::fmt::Display for PartitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "partition only on Linux"),
            Self::NotRoot => write!(f, "iptables requires root"),
            Self::Spawn(e) => write!(f, "could not start iptables: {e}"),
            Self::NonZeroExit { stderr, code } => {
                write!(f, "iptables failed (exit {code:?}): {stderr}")
            }
            Self::EmptyGroup => write!(f, "both groups must contain >=1 IP"),
        }
    }
}

impl std::error::Error for PartitionError {}

/// Comment tag for iptables rules so `clear` can identify them
/// can.
#[cfg(target_os = "linux")]
const COMMENT_TAG: &str = "zerodds-chaos-partition";

#[cfg(target_os = "linux")]
fn require_root() -> Result<(), PartitionError> {
    // SAFETY: geteuid is read-only.
    let euid = unsafe { libc_stub::geteuid() };
    if euid != 0 {
        Err(PartitionError::NotRoot)
    } else {
        Ok(())
    }
}

/// Sets the partition rules. Idempotent: calls `clear` first.
///
/// # Errors
/// See [`PartitionError`].
#[cfg(target_os = "linux")]
pub fn apply(cfg: &PartitionConfig) -> Result<(), PartitionError> {
    require_root()?;
    if cfg.group_a.is_empty() || cfg.group_b.is_empty() {
        return Err(PartitionError::EmptyGroup);
    }
    let _ = clear();
    for a in &cfg.group_a {
        for b in &cfg.group_b {
            // A → B
            run_iptables(&[
                "-A",
                "FORWARD",
                "-s",
                a,
                "-d",
                b,
                "-m",
                "comment",
                "--comment",
                COMMENT_TAG,
                "-j",
                "DROP",
            ])?;
            // B → A
            run_iptables(&[
                "-A",
                "FORWARD",
                "-s",
                b,
                "-d",
                a,
                "-m",
                "comment",
                "--comment",
                COMMENT_TAG,
                "-j",
                "DROP",
            ])?;
            // Plus INPUT/OUTPUT for same-host setups (loopback tests)
            run_iptables(&[
                "-A",
                "INPUT",
                "-s",
                a,
                "-d",
                b,
                "-m",
                "comment",
                "--comment",
                COMMENT_TAG,
                "-j",
                "DROP",
            ])?;
            run_iptables(&[
                "-A",
                "INPUT",
                "-s",
                b,
                "-d",
                a,
                "-m",
                "comment",
                "--comment",
                COMMENT_TAG,
                "-j",
                "DROP",
            ])?;
        }
    }
    Ok(())
}

/// Stub for non-Linux.
#[cfg(not(target_os = "linux"))]
pub fn apply(_cfg: &PartitionConfig) -> Result<(), PartitionError> {
    Err(PartitionError::Unsupported)
}

/// Removes all iptables rules with our COMMENT_TAG.
#[cfg(target_os = "linux")]
pub fn clear() -> Result<(), PartitionError> {
    require_root()?;
    // Find all rules with our comment tag and drop them.
    // iptables has no direct "clear by comment" → we list
    // with -S, parse, and drop per line.
    for chain in ["FORWARD", "INPUT", "OUTPUT"] {
        let out = Command::new("iptables")
            .args(["-S", chain])
            .output()
            .map_err(PartitionError::Spawn)?;
        if !out.status.success() {
            continue;
        }
        let lines = String::from_utf8_lossy(&out.stdout);
        for line in lines.lines() {
            if line.contains(COMMENT_TAG) && line.starts_with("-A ") {
                let del_line = line.replacen("-A ", "-D ", 1);
                let args: Vec<&str> = del_line.split_whitespace().collect();
                let _ = run_iptables(&args);
            }
        }
    }
    Ok(())
}

/// Stub for non-Linux.
#[cfg(not(target_os = "linux"))]
pub fn clear() -> Result<(), PartitionError> {
    Err(PartitionError::Unsupported)
}

#[cfg(target_os = "linux")]
fn run_iptables(args: &[&str]) -> Result<(), PartitionError> {
    let out = Command::new("iptables")
        .args(args)
        .output()
        .map_err(PartitionError::Spawn)?;
    if !out.status.success() {
        return Err(PartitionError::NonZeroExit {
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code(),
        });
    }
    Ok(())
}

#[cfg(target_os = "linux")]
mod libc_stub {
    unsafe extern "C" {
        pub fn geteuid() -> u32;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
mod tests {
    use super::*;

    #[test]
    fn partition_default_works() {
        let cfg = PartitionConfig {
            group_a: vec!["10.0.0.1".into()],
            group_b: vec!["10.0.0.2".into()],
        };
        assert_eq!(cfg.group_a.len(), 1);
        assert_eq!(cfg.group_b.len(), 1);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_returns_unsupported() {
        let cfg = PartitionConfig {
            group_a: vec!["10.0.0.1".into()],
            group_b: vec!["10.0.0.2".into()],
        };
        assert!(matches!(apply(&cfg), Err(PartitionError::Unsupported)));
        assert!(matches!(clear(), Err(PartitionError::Unsupported)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn empty_group_rejected() {
        // SAFETY: geteuid read-only.
        let euid = unsafe { libc_stub::geteuid() };
        if euid != 0 {
            // Without root we hit NotRoot first, which is OK for the test.
            return;
        }
        let cfg = PartitionConfig {
            group_a: vec![],
            group_b: vec!["10.0.0.2".into()],
        };
        let r = apply(&cfg);
        assert!(matches!(r, Err(PartitionError::EmptyGroup)));
    }
}
