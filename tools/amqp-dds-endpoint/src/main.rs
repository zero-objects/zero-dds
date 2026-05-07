// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// Daemon-Binary — eprintln ist die natuerliche Logging-Form.
#![allow(clippy::print_stderr)]
//! DDS-AMQP 1.0 Endpoint Daemon — Binary.
//!
//! Synchronen, std-only AMQP-1.0-Server. Liest Konfiguration aus
//! XML-Datei (Spec §9.2) und treibt einen TCP-Listener-Loop
//! auf der konfigurierten Adresse.
//!
//! # CLI
//!
//! ```text
//! amqp-dds-endpoint [--config <path>] [--listen <addr>]
//! ```
//!
//! Default: `--listen 0.0.0.0:5672`, kein Config-File.

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use amqp_dds_endpoint::{ServerConfig, run_server};
use zerodds_amqp_endpoint::MetricsHub;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut listen_addr: Option<String> = None;
    let mut config_path: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" => {
                i += 1;
                listen_addr = args.get(i).cloned();
            }
            "--config" => {
                i += 1;
                config_path = args.get(i).cloned();
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => {
                return Err(format!("unknown argument: {other}").into());
            }
        }
        i += 1;
    }

    let mut cfg = ServerConfig::default_listen();
    if let Some(addr) = listen_addr {
        cfg.listen_addr = addr;
    }
    if let Some(path) = config_path {
        let xml = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let parsed = zerodds_amqp_endpoint::config_xml::parse_config(&xml)?;
        if let Some(ep) = parsed.endpoints.into_iter().next() {
            if !ep.listen_uri.is_empty() {
                cfg.listen_addr = strip_amqp_scheme(&ep.listen_uri);
            }
            cfg.tls_active = ep.tls.enabled;
            if ep.limits.max_frame_size > 0 {
                cfg.max_frame_size = ep.limits.max_frame_size;
            }
            if !ep.endpoint_name.is_empty() {
                cfg.container_id = ep.endpoint_name;
            }
        }
    }

    let metrics = Arc::new(MetricsHub::new());
    let shutdown = Arc::new(AtomicBool::new(false));
    install_signal_handler(&shutdown);

    run_server(cfg, metrics, shutdown)?;
    Ok(())
}

fn print_usage() {
    eprintln!("usage: amqp-dds-endpoint [--config <path>] [--listen <addr>]");
    eprintln!();
    eprintln!("  --config <path>   Load configuration from XML file (Spec §9.2)");
    eprintln!("  --listen <addr>   Listen address (default 0.0.0.0:5672)");
    eprintln!("  --help            Show this message");
}

fn strip_amqp_scheme(uri: &str) -> String {
    uri.strip_prefix("amqp://")
        .or_else(|| uri.strip_prefix("amqps://"))
        .map_or_else(|| uri.to_string(), ToString::to_string)
}

#[cfg(unix)]
fn install_signal_handler(shutdown: &Arc<AtomicBool>) {
    // Spec: graceful Shutdown auf SIGINT/SIGTERM. std hat keinen
    // signal-Handler; Caller darf das Atomic von aussen setzen.
    // In Produktion wuerde man `signal-hook` o.ae. nutzen.
    let _ = shutdown; // ungenutzt im no-signal-Fallback.
}

#[cfg(not(unix))]
fn install_signal_handler(shutdown: &Arc<AtomicBool>) {
    let _ = shutdown;
}
