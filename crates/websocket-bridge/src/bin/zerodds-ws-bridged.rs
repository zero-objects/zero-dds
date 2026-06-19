// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-ws-bridged` — DDS↔WebSocket bridge daemon.
//!
//! Spec: `docs/specs/zerodds-ws-bridge-1.0.md`.

#![forbid(unsafe_code)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::approx_constant,
    clippy::unreachable,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    clippy::useless_conversion,
    missing_docs
)]

// Daemon logging: eprintln is the structured JSON log sink per
// Spec §8.1; a full-fledged tracing stack is not part of the workspace.
use std::path::Path;
use std::process::ExitCode;

use zerodds_websocket_bridge::daemon::cli::{self, CliArgs, HELP_TEXT, VERSION_TEXT};
use zerodds_websocket_bridge::daemon::config::{self, DaemonConfig, TopicConfig};
use zerodds_websocket_bridge::daemon::server::{self, ServerError};

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let args = match cli::parse(&raw_args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("{HELP_TEXT}");
            return ExitCode::from(1);
        }
    };
    if args.help {
        println!("{HELP_TEXT}");
        return ExitCode::SUCCESS;
    }
    if args.version {
        println!("{VERSION_TEXT}");
        return ExitCode::SUCCESS;
    }
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(ServerError::Bind(m)) => {
            eprintln!("[zerodds-ws-bridged] bind error: {m}");
            ExitCode::from(2)
        }
        Err(ServerError::Dds(m)) => {
            eprintln!("[zerodds-ws-bridged] dds error: {m}");
            ExitCode::from(3)
        }
        Err(ServerError::Io(m)) => {
            eprintln!("[zerodds-ws-bridged] io error: {m}");
            ExitCode::from(1)
        }
    }
}

fn run(args: CliArgs) -> Result<(), ServerError> {
    // Load config — the file takes precedence, CLI args override.
    let mut cfg = if let Some(path) = &args.config {
        DaemonConfig::load_from_file(Path::new(path))
            .map_err(|e| ServerError::Io(format!("config: {e}")))?
    } else {
        DaemonConfig::default_for_dev()
    };
    apply_cli_overrides(&mut cfg, args);

    let handle = server::start(cfg)?;
    eprintln!("[zerodds-ws-bridged] running on {}", handle.local_addr);

    // Park the thread until SIGINT/SIGTERM. We use std::thread::park() —
    // on SIGTERM the process is terminated by the runtime, and the drop
    // handler on `DaemonHandle` sets the stop flag.
    loop {
        std::thread::park_timeout(std::time::Duration::from_secs(60));
    }
    // Unreachable; dropping `handle` performs a graceful shutdown.
    #[allow(unreachable_code)]
    {
        drop(handle);
        Ok(())
    }
}

/// Applies CLI overrides to the loaded config. Crate-public for tests.
///
/// Spec §2: the CLI overrides file values. `--topic` is additive (may occur
/// multiple times), all other flags are replace operations.
fn apply_cli_overrides(cfg: &mut DaemonConfig, args: CliArgs) {
    if let Some(l) = args.listen {
        cfg.listen = l;
    }
    if let Some(d) = args.domain {
        cfg.domain = d;
    }
    if let Some(lvl) = args.log_level {
        cfg.log_level = lvl;
    }
    if let Some(token) = args.auth_token {
        cfg.auth_mode = "bearer".to_string();
        cfg.auth_bearer_token = Some(token);
    }
    if let Some(cert) = args.tls_cert {
        cfg.tls_cert_file = cert;
        cfg.tls_enabled = true;
    }
    if let Some(key) = args.tls_key {
        cfg.tls_key_file = key;
        cfg.tls_enabled = true;
    }
    if let Some(addr) = args.metrics {
        cfg.metrics_addr = addr;
        cfg.metrics_enabled = true;
    }
    for name in args.topics {
        let topic = TopicConfig {
            name: name.clone(),
            type_name: name.clone(),
            direction: "bidir".to_string(),
            ws_path: config::default_ws_path(&name),
            ..Default::default()
        };
        cfg.topics.push(topic);
    }
}

#[cfg(test)]
mod tests {
    use super::apply_cli_overrides;
    use zerodds_websocket_bridge::daemon::cli::CliArgs;
    use zerodds_websocket_bridge::daemon::config::DaemonConfig;

    fn cfg() -> DaemonConfig {
        DaemonConfig::default_for_dev()
    }

    fn args() -> CliArgs {
        CliArgs::default()
    }

    #[test]
    fn listen_override_replaces_default() {
        let mut c = cfg();
        let mut a = args();
        a.listen = Some("0.0.0.0:9000".into());
        apply_cli_overrides(&mut c, a);
        assert_eq!(c.listen, "0.0.0.0:9000");
    }

    #[test]
    fn domain_override_replaces_default() {
        let mut c = cfg();
        let mut a = args();
        a.domain = Some(42);
        apply_cli_overrides(&mut c, a);
        assert_eq!(c.domain, 42);
    }

    #[test]
    fn log_level_override_replaces_default() {
        let mut c = cfg();
        let mut a = args();
        a.log_level = Some("debug".into());
        apply_cli_overrides(&mut c, a);
        assert_eq!(c.log_level, "debug");
    }

    #[test]
    fn auth_token_sets_bearer_mode_and_token() {
        let mut c = cfg();
        let mut a = args();
        a.auth_token = Some("secret-32-bytes".into());
        apply_cli_overrides(&mut c, a);
        assert_eq!(c.auth_mode, "bearer");
        assert_eq!(c.auth_bearer_token.as_deref(), Some("secret-32-bytes"));
    }

    #[test]
    fn tls_cert_and_key_enable_tls() {
        let mut c = cfg();
        let mut a = args();
        a.tls_cert = Some("/etc/tls/cert.pem".into());
        a.tls_key = Some("/etc/tls/key.pem".into());
        apply_cli_overrides(&mut c, a);
        assert!(c.tls_enabled);
        assert_eq!(c.tls_cert_file, "/etc/tls/cert.pem");
        assert_eq!(c.tls_key_file, "/etc/tls/key.pem");
    }

    #[test]
    fn metrics_addr_enables_metrics() {
        let mut c = cfg();
        let mut a = args();
        a.metrics = Some("0.0.0.0:9091".into());
        apply_cli_overrides(&mut c, a);
        assert!(c.metrics_enabled);
        assert_eq!(c.metrics_addr, "0.0.0.0:9091");
    }

    #[test]
    fn topics_cli_appends_to_existing() {
        let mut c = cfg();
        let mut a = args();
        a.topics = vec!["Card".into(), "Thread".into()];
        apply_cli_overrides(&mut c, a);
        assert_eq!(c.topics.len(), 2);
        assert_eq!(c.topics[0].name, "Card");
        assert_eq!(c.topics[0].direction, "bidir");
        assert_eq!(c.topics[0].ws_path, "/topics/card");
        assert_eq!(c.topics[1].name, "Thread");
    }

    #[test]
    fn empty_args_leaves_config_unchanged() {
        let mut c = cfg();
        let baseline = c.clone();
        apply_cli_overrides(&mut c, args());
        assert_eq!(c.listen, baseline.listen);
        assert_eq!(c.domain, baseline.domain);
        assert!(!c.tls_enabled);
        assert!(!c.metrics_enabled);
        assert_eq!(c.auth_mode, "none");
        assert!(c.topics.is_empty());
    }
}
