// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Linux `tc qdisc`-Wrapper.
//!
//! Spawnt `tc` mit den passenden netem-Parametern. Operiert mit
//! Root-Privilegien (oder `CAP_NET_ADMIN`) — lehnt ohne sauber ab.
//!
//! Auf Nicht-Linux-Plattformen sind die Funktionen Stubs, die einen
//! `Unsupported`-Fehler zurueckgeben — das Tool bleibt auch auf
//! macOS/Windows compileierbar.

#[cfg(target_os = "linux")]
use std::process::Command;
use std::time::Duration;

/// Konfigurations-Knobs fuer `tc qdisc add ... netem ...`.
#[derive(Default, Clone, Debug)]
pub struct NetemConfig {
    /// Packet-Loss-Rate (0.0 - 1.0).
    pub loss: f64,
    /// Mittlerer Delay.
    pub delay: Duration,
    /// Jitter (zufaellig in [-jitter, +jitter] um delay).
    pub jitter: Duration,
    /// Duplicate-Wahrscheinlichkeit.
    pub duplicate: f64,
    /// Reorder-Wahrscheinlichkeit (zwingt netem in pkt(N) auf pkt(N+1)-Position).
    pub reorder: f64,
    /// Korrelation der Loss-Verteilung in % (0=unkorreliert, 25=GE-Modell).
    pub loss_correlation_pct: u32,
}

/// Errors aus dem tc-Wrapper.
#[derive(Debug)]
pub enum TcError {
    /// Plattform unterstuetzt kein tc-Backend.
    Unsupported,
    /// Aufrufer hat nicht euid=0 / CAP_NET_ADMIN.
    NotRoot,
    /// `tc`-Binary nicht ausfuehrbar.
    Spawn(std::io::Error),
    /// `tc` gestartet, aber mit Fehlercode beendet.
    NonZeroExit {
        /// stderr-Output von tc.
        stderr: String,
        /// Exit-Code (None wenn ueber Signal beendet).
        code: Option<i32>,
    },
}

impl core::fmt::Display for TcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TcError::Unsupported => write!(f, "tc-Backend nur auf Linux unterstuetzt"),
            TcError::NotRoot => write!(f, "tc benoetigt root oder CAP_NET_ADMIN"),
            TcError::Spawn(e) => write!(f, "konnte tc nicht starten: {e}"),
            TcError::NonZeroExit { stderr, code } => {
                write!(f, "tc fehlgeschlagen (exit {code:?}): {stderr}")
            }
        }
    }
}

impl std::error::Error for TcError {}

#[cfg(target_os = "linux")]
fn require_root() -> Result<(), TcError> {
    // SAFETY: libc::geteuid ist signaturkompatibel; effective UID lesen
    // ist read-only und thread-safe.
    let euid = unsafe { libc_stub::geteuid() };
    if euid != 0 {
        Err(TcError::NotRoot)
    } else {
        Ok(())
    }
}

/// Setzt netem auf das Interface. Idempotent: vorhandene qdisc wird
/// vorher abgeraeumt.
#[cfg(target_os = "linux")]
pub fn apply(interface: &str, cfg: &NetemConfig) -> Result<(), TcError> {
    require_root()?;
    let _ = Command::new("tc")
        .args(["qdisc", "del", "dev", interface, "root"])
        .output(); // ignoriere — qdisc existiert evtl. nicht.

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

/// Stub fuer Nicht-Linux-Plattformen.
#[cfg(not(target_os = "linux"))]
pub fn apply(_interface: &str, _cfg: &NetemConfig) -> Result<(), TcError> {
    Err(TcError::Unsupported)
}

/// Entfernt die Root-qdisc vom Interface.
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

/// Stub fuer Nicht-Linux-Plattformen.
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
    // Mini-libc-Binding fuer geteuid: vermeidet eine schwergewichtige
    // libc-Crate-Dep nur fuer einen Permissions-Check.
    unsafe extern "C" {
        pub fn geteuid() -> u32;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
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
        // SAFETY: geteuid ist read-only.
        let euid = unsafe { libc_stub::geteuid() };
        if euid == 0 {
            // root: Test trifft keine Aussage; skip.
            return;
        }
        let cfg = NetemConfig::default();
        let r = apply("eth0", &cfg);
        assert!(matches!(r, Err(TcError::NotRoot)), "got {r:?}");
    }
}
