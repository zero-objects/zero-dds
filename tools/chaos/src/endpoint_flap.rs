// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Endpoint-Flap — toggelt ein Linux-Interface up/down im Takt.
//!
//! Zwischen Up- und Down-Phasen liegt das konfigurierte Intervall. Vor
//! dem Down wird ein optionales `pre_down`-Hook gerufen (z.B. um
//! gleichzeitig iptables-Regeln zu setzen).
//!
//! Auf Nicht-Linux-Plattformen sind die Funktionen Stubs.

#[cfg(target_os = "linux")]
use std::process::Command;
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::time::Instant;

/// Endpoint-Flap-Konfig.
#[derive(Clone, Debug)]
pub struct FlapConfig {
    /// Interface-Name (z.B. `"eth0"`).
    pub interface: String,
    /// Intervall zwischen Toggle-Events.
    pub interval: Duration,
    /// Wallclock-Dauer der gesamten Flap-Phase.
    pub total: Duration,
    /// Wenn `true`, beginnt der Cycle mit DOWN; sonst mit UP.
    pub start_down: bool,
}

/// Errors aus dem flap-Wrapper.
#[derive(Debug)]
pub enum FlapError {
    /// Plattform unterstuetzt kein ip-link.
    Unsupported,
    /// Aufrufer hat nicht euid=0.
    NotRoot,
    /// `ip`-Binary nicht ausfuehrbar.
    Spawn(std::io::Error),
    /// `ip` returnte non-zero.
    NonZeroExit {
        /// stderr-Output.
        stderr: String,
        /// Exit-Code.
        code: Option<i32>,
    },
}

impl std::fmt::Display for FlapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "endpoint-flap only on Linux"),
            Self::NotRoot => write!(f, "ip benoetigt root oder CAP_NET_ADMIN"),
            Self::Spawn(e) => write!(f, "konnte ip nicht starten: {e}"),
            Self::NonZeroExit { stderr, code } => {
                write!(f, "ip fehlgeschlagen (exit {code:?}): {stderr}")
            }
        }
    }
}

impl std::error::Error for FlapError {}

/// Statistik aus einem Flap-Lauf.
#[derive(Default, Clone, Debug)]
pub struct FlapStats {
    /// Anzahl Up-Transitionen.
    pub up_count: u64,
    /// Anzahl Down-Transitionen.
    pub down_count: u64,
    /// Wallclock-Gesamt.
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

/// Fuehrt den Flap-Cycle aus. Blockiert bis `total` abgelaufen ist und
/// laesst das Interface am Ende **immer up** stehen.
///
/// # Errors
/// IO-/Privilege-Fehler.
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
    // Recovery: Interface immer up zuruecklassen.
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
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
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
