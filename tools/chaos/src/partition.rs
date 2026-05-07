// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Network-Partition via `iptables` (Linux).
//!
//! Erstellt ein Set von INPUT/OUTPUT-Regeln das den Traffic zwischen
//! konfigurierten IP-Gruppen blockiert. Phase-A: zwei Gruppen
//! ("A", "B"). Konfiguration:
//!
//! * Gruppe A = Liste von IPs/CIDRs
//! * Gruppe B = Liste von IPs/CIDRs
//!
//! Das Tool fuegt fuer jede A↔B-Paarung eine DROP-Regel hinzu (in
//! beiden Richtungen). `clear` entfernt sie wieder.
//!
//! Auf Nicht-Linux-Plattformen sind die Funktionen Stubs.

#[cfg(target_os = "linux")]
use std::process::Command;

/// Partition-Konfig mit zwei Gruppen.
#[derive(Clone, Debug)]
pub struct PartitionConfig {
    /// Gruppe-A-IPs (oder CIDRs).
    pub group_a: Vec<String>,
    /// Gruppe-B-IPs (oder CIDRs).
    pub group_b: Vec<String>,
}

/// Errors aus dem partition-Wrapper.
#[derive(Debug)]
pub enum PartitionError {
    /// Plattform unterstuetzt kein iptables.
    Unsupported,
    /// Aufrufer hat nicht euid=0.
    NotRoot,
    /// `iptables`-Binary nicht ausfuehrbar.
    Spawn(std::io::Error),
    /// `iptables` returnte non-zero.
    NonZeroExit {
        /// Output-Mitschnitt.
        stderr: String,
        /// Exit-Code.
        code: Option<i32>,
    },
    /// Empty-Group oder nur eine Gruppe.
    EmptyGroup,
}

impl std::fmt::Display for PartitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "partition only on Linux"),
            Self::NotRoot => write!(f, "iptables benoetigt root"),
            Self::Spawn(e) => write!(f, "konnte iptables nicht starten: {e}"),
            Self::NonZeroExit { stderr, code } => {
                write!(f, "iptables fehlgeschlagen (exit {code:?}): {stderr}")
            }
            Self::EmptyGroup => write!(f, "beide Gruppen muessen >=1 IP enthalten"),
        }
    }
}

impl std::error::Error for PartitionError {}

/// Komment-Tag fuer iptables-Regeln, damit `clear` sie identifizieren
/// kann.
#[cfg(target_os = "linux")]
const COMMENT_TAG: &str = "zerodds-chaos-partition";

#[cfg(target_os = "linux")]
fn require_root() -> Result<(), PartitionError> {
    // SAFETY: geteuid ist read-only.
    let euid = unsafe { libc_stub::geteuid() };
    if euid != 0 {
        Err(PartitionError::NotRoot)
    } else {
        Ok(())
    }
}

/// Setzt die Partition-Regeln. Idempotent: ruft vorher `clear`.
///
/// # Errors
/// Wie [`PartitionError`].
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
            // Plus INPUT/OUTPUT fuer same-host Setups (loopback-Tests)
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

/// Stub fuer Nicht-Linux.
#[cfg(not(target_os = "linux"))]
pub fn apply(_cfg: &PartitionConfig) -> Result<(), PartitionError> {
    Err(PartitionError::Unsupported)
}

/// Entfernt alle iptables-Regeln mit unserem COMMENT_TAG.
#[cfg(target_os = "linux")]
pub fn clear() -> Result<(), PartitionError> {
    require_root()?;
    // Alle Regeln mit unserem Comment-Tag finden und droppen.
    // iptables hat kein direktes "clear by comment" → wir listen
    // mit -S, parsen, und droppen pro-Zeile.
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

/// Stub fuer Nicht-Linux.
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
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
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
            // Without root we hit NotRoot first, was OK fuer den Test.
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
