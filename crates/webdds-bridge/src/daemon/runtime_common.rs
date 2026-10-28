// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The admin `/catalog`+`/healthz`+`/metrics` endpoint every
//! `zerodds-*-bridged` daemon serves on its own port
//! (`metrics_addr`/`metrics.listen`) — **this is the daemon family's own
//! convention, not part of OMG DDS-WEB 1.0**: the spec text (§5-§9,
//! Annex A) has no mention of a catalog, a health check, or `/metrics`
//! anywhere; this daemon's own config already promised it
//! (`DaemonConfig::metrics_enabled`'s doc: "toggles the Prometheus/
//! catalog/healthz endpoint") without it being built. Same shape as
//! `crates/websocket-bridge/src/daemon/runtime_common.rs`'s
//! `CatalogSnapshot`/`serve_admin_endpoints`, adjusted for what this
//! daemon actually has fixed at start-up to report: not a `topics:`
//! list (REST creates topics dynamically at run time, not from static
//! config) but the config-named `types:`/`interfaces:` files.

#![allow(clippy::print_stderr)]

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_monitor::Registry;

use super::config::DaemonConfig;

/// Service name for the catalog.
pub const SERVICE_NAME: &str = "zerodds-webdds-bridged";

/// Version string — Cargo crate version (single source).
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Catalog snapshot for the `/catalog` endpoint: what this daemon
/// instance was configured with at start-up.
#[derive(Clone)]
pub struct CatalogSnapshot {
    /// Service name.
    pub service: String,
    /// Version.
    pub version: String,
    /// `listen` — the REST/SOAP/WSDL address.
    pub listen: String,
    /// `domain` — the DDS domain a `domain_participant` resource is
    /// created on names its own; see `DaemonConfig::domain`'s doc for
    /// the one exception (the DDS-RPC client participant).
    pub domain: i32,
    /// `types[].name` — every type this gateway can resolve from a
    /// config-named IDL/XML file.
    pub type_names: Vec<String>,
    /// `interfaces` — IDL files whose `interface`s this gateway
    /// dispatches over SOAP.
    pub interface_files: Vec<String>,
}

impl CatalogSnapshot {
    /// Builds from the daemon config.
    #[must_use]
    pub fn from_config(cfg: &DaemonConfig) -> Self {
        Self {
            service: SERVICE_NAME.into(),
            version: SERVICE_VERSION.into(),
            listen: cfg.listen.clone(),
            domain: cfg.domain,
            type_names: cfg.types.iter().map(|t| t.type_name.clone()).collect(),
            interface_files: cfg.interfaces.clone(),
        }
    }

    /// Renders as JSON.
    #[must_use]
    pub fn render_json(&self) -> String {
        let mut out =
            String::with_capacity(256 + (self.type_names.len() + self.interface_files.len()) * 64);
        out.push_str("{\"service\":\"");
        push_json_str(&mut out, &self.service);
        out.push_str("\",\"version\":\"");
        push_json_str(&mut out, &self.version);
        out.push_str("\",\"listen\":\"");
        push_json_str(&mut out, &self.listen);
        out.push_str("\",\"domain\":");
        out.push_str(&self.domain.to_string());
        out.push_str(",\"types\":[");
        for (i, t) in self.type_names.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push('"');
            push_json_str(&mut out, t);
            out.push('"');
        }
        out.push_str("],\"interfaces\":[");
        for (i, f) in self.interface_files.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push('"');
            push_json_str(&mut out, f);
            out.push('"');
        }
        out.push_str("]}");
        out
    }
}

fn push_json_str(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

/// Mini HTTP worker serving `/catalog`, `/healthz` and `/metrics` on
/// the admin port. Returns the join handle and the bound address (the
/// caller asked for port `0`, this is where it actually landed).
pub fn serve_admin_endpoints(
    addr: SocketAddr,
    catalog: Arc<CatalogSnapshot>,
    registry: Arc<Registry>,
    healthy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    let listener = TcpListener::bind(addr)?;
    let bound = listener.local_addr()?;
    listener.set_nonblocking(true)?;

    let h = thread::Builder::new()
        .name("zerodds-webdds-admin".into())
        .spawn(move || {
            loop {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                match listener.accept() {
                    Ok((mut s, _peer)) => {
                        let _ = s.set_nonblocking(false);
                        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                        admin_handle(&mut s, &catalog, &registry, &healthy);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        })?;
    Ok((h, bound))
}

fn admin_handle(
    stream: &mut std::net::TcpStream,
    catalog: &CatalogSnapshot,
    registry: &Registry,
    healthy: &AtomicBool,
) {
    use std::io::{Read, Write};
    let mut buf = [0u8; 1024];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let first_line = req.lines().next().unwrap_or("");
    let path = first_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");

    let (status, ctype, body) = match path {
        "/catalog" => ("200 OK", "application/json", catalog.render_json()),
        "/healthz" => {
            if healthy.load(Ordering::SeqCst) {
                ("200 OK", "text/plain; charset=utf-8", "OK\n".to_string())
            } else {
                (
                    "503 Service Unavailable",
                    "text/plain; charset=utf-8",
                    "DOWN\n".to_string(),
                )
            }
        }
        "/metrics" => (
            "200 OK",
            "text/plain; version=0.0.4; charset=utf-8",
            registry.render_prometheus(),
        ),
        "/" => (
            "200 OK",
            "text/plain; charset=utf-8",
            format!("{SERVICE_NAME}\nendpoints: /catalog /healthz /metrics\n"),
        ),
        _ => ("404 Not Found", "text/plain; charset=utf-8", String::new()),
    };

    let resp = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {ctype}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn catalog_render_json_contains_config_fields() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.types.push(super::super::config::TypeEntry {
            type_name: "M::Sensor".into(),
            idl_file: Some("/etc/zerodds/m.idl".into()),
            xml_file: None,
        });
        cfg.interfaces.push("/etc/zerodds/calc.idl".into());
        let snap = CatalogSnapshot::from_config(&cfg);
        let j = snap.render_json();
        assert!(j.contains("\"service\":\"zerodds-webdds-bridged\""));
        assert!(j.contains("\"types\":[\"M::Sensor\"]"));
        assert!(j.contains("\"interfaces\":[\"/etc/zerodds/calc.idl\"]"));
    }

    #[test]
    fn admin_endpoint_serves_catalog_and_healthz_and_metrics() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.types.push(super::super::config::TypeEntry {
            type_name: "M::Sensor".into(),
            idl_file: Some("/etc/zerodds/m.idl".into()),
            xml_file: None,
        });
        let snap = Arc::new(CatalogSnapshot::from_config(&cfg));
        let reg = Arc::new(Registry::new());
        reg.counter("zerodds_webdds_test_total", zerodds_monitor::Labels::new())
            .add(7);

        let healthy = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));

        let (_h, bound) = serve_admin_endpoints(
            "127.0.0.1:0".parse().unwrap(),
            Arc::clone(&snap),
            Arc::clone(&reg),
            Arc::clone(&healthy),
            Arc::clone(&stop),
        )
        .expect("spawn admin");

        let mut s =
            TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect catalog");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /catalog HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("HTTP/1.1 200"));
        assert!(body.contains("\"M::Sensor\""), "got: {body}");

        let mut s =
            TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect health");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("HTTP/1.1 200"));
        assert!(body.contains("OK"));

        let mut s =
            TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect metrics");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("zerodds_webdds_test_total 7"), "got: {body}");

        healthy.store(false, Ordering::SeqCst);
        let mut s =
            TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect health2");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.contains("HTTP/1.1 503"));

        stop.store(true, Ordering::SeqCst);
    }
}
