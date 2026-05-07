//! Cross-Vendor-Live-Interop-Helfer fuer C5.5.
//!
//! Stellt Subprocess-Spawner fuer FastDDS- und Cyclone-Tools bereit,
//! die ueber `ssh llvm@llvm` auf dem Lab-Host ausgefuehrt werden. Die
//! Strukturen halten den lokalen Child-Handle plus eine Drop-Impl,
//! die das Remote-Process via `pkill` killt — sonst bleiben
//! `fastdds shape` / `ddsperf`-Prozesse auf der VM stehen.
//!
//! # Typ-Strategie
//!
//! `RemoteSubprocess` ist ein generischer Wrapper: das `pkill_pattern`
//! identifiziert das Remote-Programm, der lokale `Child` haelt die
//! SSH-Session. Ohne SSH-Schluessel wird via `sshpass` mit Lab-Passwort
//! gepiped — fuer ein Lab-Setup akzeptabel, in Produktion durch
//! Schluessel-Auth zu ersetzen.
//!
//! # `#[ignore]`-Strategie
//!
//! Alle Live-Tests sind per `#[ignore]` markiert. Lokal:
//!
//! ```bash
//! cargo test -p zerodds-discovery --features live-interop \
//!     --test fastdds_live_pub -- --ignored --nocapture
//! ```
//!
//! Wer das Lab nicht erreicht, lese `live_host_available()` — sobald
//! die Env-Var `LLVM_HOST_AVAILABLE=1` nicht gesetzt ist UND `sshpass`
//! oder die Host-Verbindung fehlt, ueberspringen die Helfer mit klarer
//! Fehler-Message.

#![allow(dead_code, clippy::expect_used, clippy::print_stderr)]

use core::time::Duration;
use std::process::{Child, Command, Stdio};

/// SSH-User fuer Lab-Host (siehe Memory `reference_bench_hosts`).
pub const SSH_USER: &str = "llvm";
/// SSH-Passwort fuer Lab-Host. Lab-only — Produktion braucht Key-Auth.
pub const SSH_PASS: &str = "llvm";
/// SSH-Hostname; wird via `~/.ssh/config` aufgeloest.
pub const SSH_HOST: &str = "llvm";
/// Default-Timeout fuer Live-Subprocess-Bring-up.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Reliability-Variante fuer FastDDS-Tools. `fastdds shape` akzeptiert
/// genau diese zwei Varianten (s. `fastdds shape --help`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastReliability {
    BestEffort,
    Reliable,
}

/// Durability-Variante fuer FastDDS-Tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastDurability {
    Volatile,
    TransientLocal,
}

/// Komplettes QoS-Set fuer FastDDS-Tools.
#[derive(Debug, Clone, Copy)]
pub struct FastQos {
    pub reliability: FastReliability,
    pub durability: FastDurability,
}

impl FastQos {
    /// Standard fuer `fastdds shape`-Default-Profile.
    pub const DEFAULT: Self = Self {
        reliability: FastReliability::Reliable,
        durability: FastDurability::Volatile,
    };

    /// Mappt auf CLI-Flags fuer `fastdds shape`. `-r reliable|besteffort`,
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

/// Pruefung ob Live-Host erreichbar ist. Tests ohne Lab koennen so
/// frueh skip-en, bevor SSH-Timeouts den Test ausbluten lassen.
pub fn live_host_available() -> bool {
    if std::env::var("LLVM_HOST_AVAILABLE").is_ok() {
        return true;
    }
    // Fallback: `sshpass` lokal vorhanden? Ohne `sshpass` wuerden alle
    // Subprocess-Spawns sofort scheitern.
    Command::new("sshpass")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Generischer SSH-Subprocess-Wrapper. `pkill_pattern` ist das
/// `pkill -f`-Argument, mit dem `Drop` den Remote-Prozess killt.
pub struct RemoteSubprocess {
    child: Child,
    pkill_pattern: String,
    label: String,
}

impl RemoteSubprocess {
    /// Spawnt `cmd` ueber `sshpass+ssh` auf `llvm`. `pkill_pattern`
    /// matcht den Remote-Prozess (regex-frei, wird mit `pkill -f`
    /// als Substring genutzt).
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

    /// Synonym damit der Aufrufer-Code lesbarer wird.
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Drop for RemoteSubprocess {
    fn drop(&mut self) {
        // Best-effort: Remote-Prozess killen, dann lokales SSH-Child.
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

/// Startet `fastdds shape publisher` auf llvm.
///
/// Wir setzen `--domain` explizit; Topic ist eine der drei
/// Shape-Topics (`Square`, `Triangle`, `Circle`) per FastDDS-Konvention.
/// Color `RED` als Default, weil das fuer das Default-Profile von
/// `fastdds shape` der erste verfuegbare ist.
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

/// Startet `fastdds shape subscriber` auf llvm.
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

/// Startet `fastdds discovery` Server-Modus. Discovery-Server-Spec liegt
/// in FastDDS' Server-Client-Discovery-Protokoll (siehe FastDDS 2.9
/// User-Manual §16). Wir setzen GUID-Prefix auf `44.53.00.5f.45.50...`
/// (das von eProsima als Reference dokumentierte Default-Prefix).
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

/// Cyclone-Pub via `ddsperf` — wird fuer Lueckenfueller-Tests
/// (cyclone_typelookup_sub_side) gebraucht. `-D` ist Duration in
/// Sekunden, `-i` ist Domain-ID (siehe Memory
/// `reference_ddsperf_flags` — Achtung Flag-Falle).
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

/// Cyclone-Sub via `ddsperf`.
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

/// Detect lokale LAN-IP — auf macOS via `ipconfig`, auf Linux via
/// `hostname -I`. Wird fuer SPDP-Beacons gebraucht (Cyclone/FastDDS
/// schicken Unicast-Antworten an den im Beacon angekuendigten
/// Locator).
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
