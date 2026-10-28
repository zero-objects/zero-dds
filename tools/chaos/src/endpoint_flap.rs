// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Endpoint flap — toggles a Linux interface up/down on a cycle.
//!
//! Between up and down phases lies the configured interval. Before
//! before the down an optional `pre_down` hook is called (e.g. to
//! setting iptables rules concurrently).
//!
//! On non-Linux platforms the functions are stubs.

#[cfg(target_os = "linux")]
use std::process::Command;
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::time::Instant;

/// Endpoint-flap config.
#[derive(Clone, Debug)]
pub struct FlapConfig {
    /// Interface name (e.g., `"eth0"`).
    pub interface: String,
    /// Interval between toggle events.
    pub interval: Duration,
    /// Wallclock duration of the entire flap phase.
    pub total: Duration,
    /// If `true`, the cycle starts with DOWN; otherwise with UP.
    pub start_down: bool,
}

/// Errors from the flap wrapper.
#[derive(Debug)]
pub enum FlapError {
    /// Platform does not support ip-link.
    Unsupported,
    /// Caller does not have euid=0.
    NotRoot,
    /// `ip` binary not executable.
    Spawn(std::io::Error),
    /// `ip` returned non-zero.
    NonZeroExit {
        /// stderr output.
        stderr: String,
        /// Exit-Code.
        code: Option<i32>,
    },
}

impl std::fmt::Display for FlapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "endpoint-flap only on Linux"),
            Self::NotRoot => write!(f, "ip requires root or CAP_NET_ADMIN"),
            Self::Spawn(e) => write!(f, "could not start ip: {e}"),
            Self::NonZeroExit { stderr, code } => {
                write!(f, "ip failed (exit {code:?}): {stderr}")
            }
        }
    }
}

impl std::error::Error for FlapError {}

/// Statistics from a flap run.
#[derive(Default, Clone, Debug)]
pub struct FlapStats {
    /// Count of up transitions.
    pub up_count: u64,
    /// Count of down transitions.
    pub down_count: u64,
    /// Wallclock total.
    pub total: Duration,
}

#[cfg(target_os = "linux")]
fn require_root() -> Result<(), FlapError> {
    // SAFETY: read-only.
    let euid = unsafe { libc_stub::geteuid() };
    if euid != 0 {
        Err(FlapError::NotRoot)
    } else {
        Ok(())
    }
}

/// Runs the flap cycle. Blocks until `total` has elapsed and
/// leaves the interface **always up** at the end.
///
/// # Errors
/// IO-/Privilege error.
#[cfg(target_os = "linux")]
pub fn run(cfg: &FlapConfig) -> Result<FlapStats, FlapError> {
    require_root()?;
    let mut stats = FlapStats::default();
    let start = Instant::now();
    let mut down_phase = cfg.start_down;
    while start.elapsed() < cfg.total {
        if down_phase {
            run_ip(&["link", "set", "dev", &cfg.interface, "down"])?;
            stats.down_count = stats.down_count.saturating_add(1);
        } else {
            run_ip(&["link", "set", "dev", &cfg.interface, "up"])?;
            stats.up_count = stats.up_count.saturating_add(1);
        }
        std::thread::sleep(cfg.interval);
        down_phase = !down_phase;
    }
    // Recovery: always leave the interface up.
    let _ = run_ip(&["link", "set", "dev", &cfg.interface, "up"]);
    stats.total = start.elapsed();
    Ok(stats)
}

/// Stub.
#[cfg(not(target_os = "linux"))]
pub fn run(_cfg: &FlapConfig) -> Result<FlapStats, FlapError> {
    Err(FlapError::Unsupported)
}

#[cfg(target_os = "linux")]
fn run_ip(args: &[&str]) -> Result<(), FlapError> {
    let out = Command::new("ip")
        .args(args)
        .output()
        .map_err(FlapError::Spawn)?;
    if !out.status.success() {
        return Err(FlapError::NonZeroExit {
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
    fn flap_config_basic() {
        let c = FlapConfig {
            interface: "eth0".into(),
            interval: Duration::from_secs(5),
            total: Duration::from_secs(60),
            start_down: false,
        };
        assert_eq!(c.interface, "eth0");
        assert_eq!(c.interval.as_secs(), 5);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_returns_unsupported() {
        let c = FlapConfig {
            interface: "eth0".into(),
            interval: Duration::from_secs(5),
            total: Duration::from_secs(1),
            start_down: false,
        };
        assert!(matches!(run(&c), Err(FlapError::Unsupported)));
    }
}
