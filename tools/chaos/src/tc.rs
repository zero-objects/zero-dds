// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Linux `tc qdisc`-Wrapper.
//!
//! Spawns `tc` with the matching netem parameters. Operates with
//! root privileges (or `CAP_NET_ADMIN`) — rejects cleanly without them.
//!
//! On non-Linux platforms the functions are stubs that return an
//! `Unsupported` error — the tool remains, even on
//! compilable on macOS/Windows.

#[cfg(target_os = "linux")]
use std::process::Command;
use std::time::Duration;

/// Configuration knobs for `tc qdisc add ... netem ...`.
#[derive(Default, Clone, Debug)]
pub struct NetemConfig {
    /// Packet-Loss-Rate (0.0 - 1.0).
    pub loss: f64,
    /// Average delay.
    pub delay: Duration,
    /// Jitter (random in [-jitter, +jitter] around delay).
    pub jitter: Duration,
    /// Duplicate probability.
    pub duplicate: f64,
    /// Reorder probability (forces netem to move pkt(N) to pkt(N+1) position).
    pub reorder: f64,
    /// Correlation of the loss distribution in % (0=uncorrelated, 25=GE model).
    pub loss_correlation_pct: u32,
}

/// Errors from the tc wrapper.
#[derive(Debug)]
pub enum TcError {
    /// Platform does not support a tc backend.
    Unsupported,
    /// Caller does not have euid=0 / CAP_NET_ADMIN.
    NotRoot,
    /// `tc` binary not executable.
    Spawn(std::io::Error),
    /// `tc` started, but exited with an error code.
    NonZeroExit {
        /// stderr output from tc.
        stderr: String,
        /// Exit code (None if terminated by signal).
        code: Option<i32>,
    },
}

impl core::fmt::Display for TcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TcError::Unsupported => write!(f, "tc backend only supported on Linux"),
            TcError::NotRoot => write!(f, "tc requires root or CAP_NET_ADMIN"),
            TcError::Spawn(e) => write!(f, "could not start tc: {e}"),
            TcError::NonZeroExit { stderr, code } => {
                write!(f, "tc failed (exit {code:?}): {stderr}")
            }
        }
    }
}

impl std::error::Error for TcError {}

#[cfg(target_os = "linux")]
fn require_root() -> Result<(), TcError> {
    // SAFETY: libc::geteuid is signature-compatible; reading the effective UID
    // is read-only and thread-safe.
    let euid = unsafe { libc_stub::geteuid() };
    if euid != 0 {
        Err(TcError::NotRoot)
    } else {
        Ok(())
    }
}

/// Sets netem on the interface. Idempotent: an existing qdisc is
/// cleaned up beforehand.
#[cfg(target_os = "linux")]
pub fn apply(interface: &str, cfg: &NetemConfig) -> Result<(), TcError> {
    require_root()?;
    let _ = Command::new("tc")
        .args(["qdisc", "del", "dev", interface, "root"])
        .output(); // ignore — qdisc may not exist.

    let mut args: Vec<String> = vec![
        "qdisc".into(),
        "add".into(),
        "dev".into(),
        interface.into(),
        "root".into(),
        "netem".into(),
    ];
    if cfg.delay > Duration::ZERO {
        args.push("delay".into());
        args.push(format_ms(cfg.delay));
        if cfg.jitter > Duration::ZERO {
            args.push(format_ms(cfg.jitter));
        }
    }
    if cfg.loss > 0.0 {
        args.push("loss".into());
        args.push(format!("{:.2}%", cfg.loss * 100.0));
        if cfg.loss_correlation_pct > 0 {
            args.push(format!("{}%", cfg.loss_correlation_pct));
        }
    }
    if cfg.duplicate > 0.0 {
        args.push("duplicate".into());
        args.push(format!("{:.2}%", cfg.duplicate * 100.0));
    }
    if cfg.reorder > 0.0 {
        args.push("reorder".into());
        args.push(format!("{:.2}%", cfg.reorder * 100.0));
    }
    let out = Command::new("tc")
        .args(&args)
        .output()
        .map_err(TcError::Spawn)?;
    if !out.status.success() {
        return Err(TcError::NonZeroExit {
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code(),
        });
    }
    Ok(())
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn apply(_interface: &str, _cfg: &NetemConfig) -> Result<(), TcError> {
    Err(TcError::Unsupported)
}

/// Removes the root qdisc from the interface.
#[cfg(target_os = "linux")]
pub fn clear(interface: &str) -> Result<(), TcError> {
    require_root()?;
    let out = Command::new("tc")
        .args(["qdisc", "del", "dev", interface, "root"])
        .output()
        .map_err(TcError::Spawn)?;
    if !out.status.success() {
        return Err(TcError::NonZeroExit {
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code(),
        });
    }
    Ok(())
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn clear(_interface: &str) -> Result<(), TcError> {
    Err(TcError::Unsupported)
}

#[cfg(target_os = "linux")]
fn format_ms(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1_000.0;
    format!("{ms:.2}ms")
}

#[cfg(target_os = "linux")]
mod libc_stub {
    // Mini libc binding for geteuid: avoids a heavyweight
    // libc crate dep just for a permissions check.
    unsafe extern "C" {
        pub fn geteuid() -> u32;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
mod tests {
    use super::*;

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_returns_unsupported() {
        let cfg = NetemConfig::default();
        assert!(matches!(apply("eth0", &cfg), Err(TcError::Unsupported)));
        assert!(matches!(clear("eth0"), Err(TcError::Unsupported)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_root_returns_not_root() {
        // SAFETY: geteuid is read-only.
        let euid = unsafe { libc_stub::geteuid() };
        if euid == 0 {
            // root: test makes no assertion; skip.
            return;
        }
        let cfg = NetemConfig::default();
        let r = apply("eth0", &cfg);
        assert!(matches!(r, Err(TcError::NotRoot)), "got {r:?}");
    }
}
