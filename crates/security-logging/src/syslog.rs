// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Syslog RFC-5424 UDP backend.
//!
//! Writes security events as RFC-5424-formatted messages to
//! a syslog collector via UDP (port 514 default). Example wire:
//!
//! ```text
//! <14>1 2026-04-24T11:22:33Z zerodds - AUTH - - bad cert from peer
//! ```
//!
//! `<14>` = facility 1 (user) × 8 + severity 6 (info) — computed from the
//! `LogLevel`. The facility is hardwired to `LOCAL0` (16 × 8 = 128).
//!
//! # Not included
//!
//! * TCP transport (RFC 5425) — follows on demand.
//! * Structured-data blocks `[sd-id@...]` — we use `-` (nil).
//! * TLS — most syslog deployments run in a trusted segment.

use alloc::string::String;
use alloc::vec::Vec;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Mutex;

use zerodds_security::logging::{LogLevel, LoggingPlugin};

/// Facility code per RFC 5424 §6.2.1. We fix it to `LOCAL0` (16) —
/// conventional for application security logs.
const FACILITY_LOCAL0: u8 = 16;

fn severity_code(l: LogLevel) -> u8 {
    // RFC 5424 §6.2.1: the same numbers 0..7 as LogLevel.
    l as u8
}

fn priority(level: LogLevel) -> u8 {
    FACILITY_LOCAL0 * 8 + severity_code(level)
}

/// UDP-based syslog client.
pub struct SyslogLoggingPlugin {
    min_level: LogLevel,
    socket: Mutex<UdpSocket>,
    target: SocketAddr,
    app_name: String,
    hostname: String,
}

impl SyslogLoggingPlugin {
    /// Connects to a syslog collector.
    ///
    /// # Errors
    /// `io::Error` if the UDP socket cannot be bound
    /// (not the connect — UDP is connectionless; we only bind
    /// locally and then send via `send_to`).
    pub fn connect(
        target: SocketAddr,
        app_name: impl Into<String>,
        hostname: impl Into<String>,
        min_level: LogLevel,
    ) -> std::io::Result<Self> {
        // 0.0.0.0:0 = the kernel picks an ephemeral port.
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

/// Escapes for the MSG part: RFC-5424 accepts UTF-8, but
/// CR/LF must go so the line is not torn apart in the collector.
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
    // We leave TIMESTAMP as "-" (nil), because we do not want to build a
    // global time wrapper here (the caller/collector fills it in).
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
            // send_to fails silently if the collector is down —
            // security logs must not crash the app.
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
        // Bind a kernel-ephemeral port, the syslog plugin points there.
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
            "below min_level nothing must be sent"
        );
    }

    #[test]
    fn plugin_class_id_stable() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let plugin = SyslogLoggingPlugin::connect(addr, "x", "y", LogLevel::Debug).unwrap();
        assert_eq!(plugin.plugin_class_id(), "DDS:Logging:syslog");
    }
}
