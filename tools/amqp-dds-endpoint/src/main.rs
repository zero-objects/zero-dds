// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// Daemon binary — eprintln is the natural logging form.
#![allow(clippy::print_stderr, clippy::print_stdout)]
//! DDS-AMQP 1.0 Endpoint Daemon — Binary.
//!
//! Synchronous, std-only AMQP 1.0 server. Reads configuration from
//! XML file (spec §9.2) and drives a TCP listener loop
//! on the configured address.
//!
//! # CLI
//!
//! ```text
//! amqp-dds-endpoint [--config <path>] [--listen <addr>]
//! ```
//!
//! Default: `--listen 0.0.0.0:5672`, no config file.

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use amqp_dds_endpoint::auth::{CredentialStore, SaslPolicy};
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
            "--hash-password" => {
                i += 1;
                let user = args.get(i).ok_or("--hash-password needs a user name")?;
                return hash_password(user);
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
    if let Some(path) = config_path {
        let xml = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let parsed = zerodds_amqp_endpoint::config_xml::parse_config(&xml)?;
        if let Some(ep) = parsed.endpoints.into_iter().next() {
            if !ep.listen_uri.is_empty() {
                cfg.listen_addr = strip_amqp_scheme(&ep.listen_uri);
            }
            if ep.limits.max_frame_size > 0 {
                cfg.max_frame_size = ep.limits.max_frame_size;
            }
            if !ep.endpoint_name.is_empty() {
                cfg.container_id = ep.endpoint_name;
            }
            let client_certs = ep.tls.enabled && ep.tls.require_client_cert;
            if ep.tls.enabled {
                cfg.tls = Some(server_tls(&ep.tls)?);
            }
            cfg.sasl = SaslPolicy::from_config(&ep.sasl, ep.tls.enabled, client_certs)?;
        }
    }
    // A flag beats the file.
    if let Some(addr) = listen_addr {
        cfg.listen_addr = addr;
    }

    let metrics = Arc::new(MetricsHub::new());
    let shutdown = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(sig, Arc::clone(&shutdown))?;
    }

    run_server(cfg, metrics, shutdown)?;
    Ok(())
}

/// The TLS termination an `<tls>` block names (DDS-AMQP 1.0 §10.1).
fn server_tls(
    tls: &zerodds_amqp_endpoint::annex_a::TlsConfig,
) -> Result<Arc<rustls::ServerConfig>, Box<dyn std::error::Error>> {
    let read =
        |path: &str, what: &str| std::fs::read(path).map_err(|e| format!("tls {what} {path}: {e}"));
    let client_ca_pem = if tls.ca_path.is_empty() {
        None
    } else {
        Some(read(&tls.ca_path, "ca-path")?)
    };
    let cfg = amqp_dds_endpoint::tls::ServerTlsConfig {
        cert_pem: read(&tls.cert_path, "cert-path")?,
        key_pem: read(&tls.key_path, "key-path")?,
        client_ca_pem,
        require_client_cert: tls.require_client_cert,
    };
    Ok(amqp_dds_endpoint::tls::build_server_config(&cfg).map_err(|e| format!("tls: {e}"))?)
}

/// Reads a password from stdin and prints the credential-store line of
/// `user` (`src/auth.rs`).
fn hash_password(user: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut password = String::new();
    std::io::stdin().read_line(&mut password)?;
    let password = password.trim_end_matches(['\n', '\r']);
    if password.is_empty() {
        return Err("empty password on stdin".into());
    }
    let mut salt = [0u8; 16];
    zerodds_bridge_security::tls_provider()
        .secure_random
        .fill(&mut salt)
        .map_err(|_| "no random source for the salt")?;
    println!(
        "{}",
        CredentialStore::entry_line(
            user,
            password,
            &salt,
            amqp_dds_endpoint::auth::DEFAULT_ITERATIONS
        )
    );
    Ok(())
}

fn print_usage() {
    eprintln!("usage: amqp-dds-endpoint [--config <path>] [--listen <addr>]");
    eprintln!("       amqp-dds-endpoint --hash-password <user>   (password on stdin)");
    eprintln!();
    eprintln!("  --config <path>          Load configuration from XML file (Spec §9.2)");
    eprintln!("  --listen <addr>          Listen address (default 0.0.0.0:5672); beats the file");
    eprintln!("  --hash-password <user>   Print the credential-store line of <user>");
    eprintln!("  --help                   Show this message");
}

fn strip_amqp_scheme(uri: &str) -> String {
    uri.strip_prefix("amqp://")
        .or_else(|| uri.strip_prefix("amqps://"))
        .map_or_else(|| uri.to_string(), ToString::to_string)
}
