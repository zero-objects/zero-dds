// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Syslog-RFC-5424-UDP-Backend.
//!
//! Schreibt Security-Events als RFC-5424-formatierte Nachrichten an
//! einen Syslog-Collector via UDP (Port 514 default). Beispiel-Wire:
//!
//! ```text
//! <14>1 2026-04-24T11:22:33Z zerodds - AUTH - - bad cert from peer
//! ```
//!
//! `<14>` = Facility 1 (user) × 8 + Severity 6 (info) — wird aus dem
//! `LogLevel` berechnet. Facility ist hart auf `LOCAL0` (16 × 8 = 128).
//!
//! # Nicht enthalten
//!
//! * TCP-Transport (RFC 5425) — folgt bei Bedarf.
//! * Structured-Data-Blocks `[sd-id@...]` — wir nutzen `-` (nil).
//! * TLS — die meisten Syslog-Deployments laufen im vertrauten Segment.

use alloc::string::String;
use alloc::vec::Vec;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Mutex;

use zerodds_security::logging::{LogLevel, LoggingPlugin};

/// Facility-Code nach RFC 5424 §6.2.1. Wir fixen auf `LOCAL0` (16) —
/// konventionell fuer Application-Security-Logs.
const FACILITY_LOCAL0: u8 = 16;

fn severity_code(l: LogLevel) -> u8 {
    // RFC 5424 §6.2.1: gleiche Zahlen 0..7 wie LogLevel.
    l as u8
}

fn priority(level: LogLevel) -> u8 {
    FACILITY_LOCAL0 * 8 + severity_code(level)
}

/// UDP-basierter Syslog-Client.
pub struct SyslogLoggingPlugin {
    min_level: LogLevel,
    socket: Mutex<UdpSocket>,
    target: SocketAddr,
    app_name: String,
    hostname: String,
}

impl SyslogLoggingPlugin {
    /// Verbindet zu einem Syslog-Collector.
    ///
    /// # Errors
    /// `io::Error` wenn das UDP-Socket nicht gebunden werden kann
    /// (nicht das Connect — UDP ist connectionless; wir binden nur
    /// lokal und senden dann per `send_to`).
    pub fn connect(
        target: SocketAddr,
        app_name: impl Into<String>,
        hostname: impl Into<String>,
        min_level: LogLevel,
    ) -> std::io::Result<Self> {
        // 0.0.0.0:0 = Kernel waehlt einen ephemeralen Port.
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        Ok(Self {
            min_level,
            socket: Mutex::new(socket),
            target,
            app_name: app_name.into(),
            hostname: hostname.into(),
        })
    }
}

fn hex16(bytes: [u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for b in bytes {
        s.push_str(&alloc::format!("{b:02x}"));
    }
    s
}

/// Escapes fuer den MSG-Teil: RFC-5424 akzeptiert UTF-8, aber
/// CR/LF muessen raus damit die Zeile im Collector nicht zerrissen wird.
fn escape_msg(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
        .collect()
}

fn build_line(
    level: LogLevel,
    participant: [u8; 16],
    category: &str,
    message: &str,
    app_name: &str,
    hostname: &str,
) -> Vec<u8> {
    // <PRI>VERSION TIMESTAMP HOSTNAME APP-NAME PROCID MSGID STRUCTURED-DATA MSG
    // Wir lassen TIMESTAMP als "-" (nil), weil wir hier keinen
    // globalen Time-Wrapper bauen wollen (Caller/Collector ergaenzt).
    let pri = priority(level);
    let line = alloc::format!(
        "<{pri}>1 - {host} {app} - {cat} - participant={pid} {msg}",
        host = if hostname.is_empty() { "-" } else { hostname },
        app = if app_name.is_empty() { "-" } else { app_name },
        cat = if category.is_empty() { "-" } else { category },
        pid = hex16(participant),
        msg = escape_msg(message),
    );
    line.into_bytes()
}

impl LoggingPlugin for SyslogLoggingPlugin {
    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        if level > self.min_level {
            return;
        }
        let line = build_line(
            level,
            participant,
            category,
            message,
            &self.app_name,
            &self.hostname,
        );
        if let Ok(socket) = self.socket.lock() {
            // send_to failt still wenn der Collector down ist —
            // Security-Logs sollen App nicht crashen lassen.
            let _ = socket.send_to(&line, self.target);
        }
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:syslog"
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::{SocketAddr, UdpSocket};
    use std::time::Duration;

    #[test]
    fn priority_combines_facility_and_severity() {
        // Facility LOCAL0 = 16. Severity Critical = 2. PRI = 16*8+2 = 130.
        assert_eq!(priority(LogLevel::Critical), 130);
        // Severity Emergency = 0 → PRI = 128.
        assert_eq!(priority(LogLevel::Emergency), 128);
    }

    #[test]
    fn build_line_rfc5424_shape() {
        let line = build_line(
            LogLevel::Warning,
            [0xAB; 16],
            "auth.fail",
            "bad cert",
            "zerodds",
            "host1",
        );
        let s = std::str::from_utf8(&line).unwrap();
        assert!(s.starts_with("<132>1 "), "PRI(132) + version(1), got {s}");
        assert!(s.contains("host1"));
        assert!(s.contains("zerodds"));
        assert!(s.contains("auth.fail"));
        assert!(s.contains("participant=abababab"));
        assert!(s.ends_with("bad cert"));
    }

    #[test]
    fn build_line_escapes_newlines() {
        let line = build_line(
            LogLevel::Error,
            [0u8; 16],
            "cat",
            "multi\nline\rmsg",
            "app",
            "host",
        );
        let s = std::str::from_utf8(&line).unwrap();
        assert!(!s.contains('\n'));
        assert!(!s.contains('\r'));
    }

    #[test]
    fn roundtrip_udp_receives_formatted_line() {
        // Kernel-ephemeralen Port binden, Syslog-Plugin zeigt dorthin.
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let addr: SocketAddr = receiver.local_addr().unwrap();

        let plugin =
            SyslogLoggingPlugin::connect(addr, "zerodds", "test-host", LogLevel::Debug).unwrap();
        plugin.log(
            LogLevel::Critical,
            [0x42; 16],
            "audit.event",
            "something happened",
        );

        let mut buf = [0u8; 1024];
        let (n, _) = receiver.recv_from(&mut buf).expect("udp recv timeout");
        let got = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(got.contains("audit.event"));
        assert!(got.contains("participant=42424242"));
        assert!(got.contains("something happened"));
    }

    #[test]
    fn below_threshold_events_are_dropped_silently() {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let addr: SocketAddr = receiver.local_addr().unwrap();

        let plugin = SyslogLoggingPlugin::connect(addr, "app", "host", LogLevel::Error).unwrap();
        plugin.log(LogLevel::Informational, [0u8; 16], "cat", "ignored");

        let mut buf = [0u8; 1024];
        assert!(
            receiver.recv_from(&mut buf).is_err(),
            "unter min_level muss nix gesendet werden"
        );
    }

    #[test]
    fn plugin_class_id_stable() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let plugin = SyslogLoggingPlugin::connect(addr, "x", "y", LogLevel::Debug).unwrap();
        assert_eq!(plugin.plugin_class_id(), "DDS:Logging:syslog");
    }
}
