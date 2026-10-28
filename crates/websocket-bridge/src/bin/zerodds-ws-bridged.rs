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
use std::process::ExitCode;

use zerodds_config::CommandLine;
use zerodds_dcps::cli::Configured;
use zerodds_websocket_bridge::daemon::cli::{self, CliArgs, HELP_TEXT, VERSION_TEXT};
use zerodds_websocket_bridge::daemon::config::{self, DaemonConfig};
use zerodds_websocket_bridge::daemon::server::{self, ServerError};

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if raw_args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-ws-bridged {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let (mut command_line, rest) = match CommandLine::extract(raw_args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let args = match cli::parse(&rest) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("{HELP_TEXT}\n{}", zerodds_dcps::cli::HELP);
            return ExitCode::from(1);
        }
    };
    if args.help {
        println!("{HELP_TEXT}\n{}", zerodds_dcps::cli::HELP);
        return ExitCode::SUCCESS;
    }
    if args.version {
        println!("{VERSION_TEXT}");
        return ExitCode::SUCCESS;
    }
    if let Err(e) = add_convenience_flags(&mut command_line, &args) {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    // Kept for SIGHUP: the reload reads the same layered sources again.
    let reload_command_line = command_line.clone();
    // A file that does not load (unreadable, syntax, include) is exit 2
    // under `--check-config` (deployment §2.1); at start it is a
    // configuration error, exit 1 — 2 is the bind error of spec §2.
    let checking = command_line.check();
    let configured = match Configured::from_command_line(
        command_line,
        Vec::new(),
        config::CONFIG_STEM,
        config::is_key,
        &[],
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[zerodds-ws-bridged] config error: {e}");
            return ExitCode::from(if checking { 2 } else { 1 });
        }
    };
    let cfg = match DaemonConfig::from_config(configured.source.tree()) {
        Ok(c) => c,
        Err(e) => {
            if configured.checking() {
                println!("error: {e}\nconfiguration invalid");
            } else {
                eprintln!("[zerodds-ws-bridged] config error: {e}");
            }
            return ExitCode::from(1);
        }
    };
    if let Some(code) = configured.check_config() {
        return code;
    }
    let load: server::ConfigLoader = Box::new(move || {
        let configured = Configured::from_command_line(
            reload_command_line.clone(),
            Vec::new(),
            config::CONFIG_STEM,
            config::is_key,
            &[],
        )
        .map_err(|e| e.to_string())?;
        DaemonConfig::from_config(configured.source.tree()).map_err(|e| e.to_string())
    });
    match run(cfg, configured.runtime, load) {
        Ok(()) => ExitCode::SUCCESS,
        Err(ServerError::Config(m)) => {
            eprintln!("[zerodds-ws-bridged] config error: {m}");
            ExitCode::from(1)
        }
        Err(ServerError::Tls(m)) => {
            eprintln!("[zerodds-ws-bridged] tls error: {m}");
            ExitCode::from(4)
        }
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

/// Spec §2: the convenience flags override file values. They are `--set`
/// entries of their keys, after any explicit `--set`; `--topic` appends.
fn add_convenience_flags(
    command_line: &mut CommandLine,
    args: &CliArgs,
) -> Result<(), zerodds_config::ConfigError> {
    for assignment in args.sets() {
        command_line.push_set(&assignment)?;
    }
    Ok(())
}

fn run(
    cfg: DaemonConfig,
    runtime: zerodds_dcps::runtime::RuntimeConfig,
    load: server::ConfigLoader,
) -> Result<(), ServerError> {
    let mut handle = server::start_with_reload(cfg, runtime, Some(load))?;
    eprintln!("[zerodds-ws-bridged] running on {}", handle.local_addr);
    // SIGTERM/SIGINT set the stop flag. The loop used to park forever on
    // the assumption that the runtime ends the process; it does not, and
    // the daemon was still running 3 s after SIGTERM (measured).
    zerodds_dcps::cli::notify_service_manager("READY=1");
    while !handle.stop_requested() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    handle.shutdown();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::add_convenience_flags;
    use zerodds_config::CommandLine;
    use zerodds_websocket_bridge::daemon::cli;
    use zerodds_websocket_bridge::daemon::config::DaemonConfig;

    /// The flag layer alone, as the daemon merges it over the files.
    fn with_flags(flags: &[&str]) -> DaemonConfig {
        let args = cli::parse(&flags.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
            .expect("valid flags");
        let mut command_line = CommandLine::default();
        add_convenience_flags(&mut command_line, &args).expect("valid sets");
        let mut tree = zerodds_config::Config::default();
        command_line.apply(&mut tree).expect("merges");
        DaemonConfig::from_config(Some(&tree)).expect("valid")
    }

    #[test]
    fn listen_override_replaces_default() {
        assert_eq!(
            with_flags(&["--listen", "0.0.0.0:9000"]).listen,
            "0.0.0.0:9000"
        );
    }

    #[test]
    fn domain_override_replaces_default() {
        assert_eq!(with_flags(&["--domain", "42"]).domain, 42);
    }

    #[test]
    fn log_level_override_replaces_default() {
        assert_eq!(with_flags(&["--log-level", "debug"]).log_level, "debug");
    }

    #[test]
    fn auth_token_sets_bearer_mode_and_token() {
        let c = with_flags(&["--auth-token", "secret-32-bytes"]);
        assert_eq!(c.auth_mode, "bearer");
        assert_eq!(c.auth_bearer_token.as_deref(), Some("secret-32-bytes"));
    }

    #[test]
    fn tls_cert_and_key_enable_tls() {
        let c = with_flags(&[
            "--tls-cert",
            "/etc/tls/cert.pem",
            "--tls-key",
            "/etc/tls/key.pem",
        ]);
        assert!(c.tls_enabled);
        assert_eq!(c.tls_cert_file, "/etc/tls/cert.pem");
        assert_eq!(c.tls_key_file, "/etc/tls/key.pem");
    }

    #[test]
    fn metrics_addr_enables_metrics() {
        let c = with_flags(&["--metrics", "0.0.0.0:9091"]);
        assert!(c.metrics_enabled);
        assert_eq!(c.metrics_addr, "0.0.0.0:9091");
    }

    #[test]
    fn topics_cli_appends_to_existing() {
        let c = with_flags(&["--topic", "Card", "--topic", "Thread"]);
        assert_eq!(c.topics.len(), 2);
        assert_eq!(c.topics[0].name, "Card");
        assert_eq!(c.topics[0].direction, "bidir");
        assert_eq!(c.topics[0].ws_path, "/topics/card");
        assert_eq!(c.topics[1].name, "Thread");
    }

    #[test]
    fn empty_args_leaves_config_unchanged() {
        let c = with_flags(&[]);
        let baseline = DaemonConfig::default_for_dev();
        assert_eq!(c.listen, baseline.listen);
        assert_eq!(c.domain, baseline.domain);
        assert!(!c.tls_enabled);
        assert!(!c.metrics_enabled);
        assert_eq!(c.auth_mode, "none");
        assert!(c.topics.is_empty());
    }
}
