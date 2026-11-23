// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-mqtt-bridged` — DDS↔MQTT-5-Bridge-Daemon.
//!
//! Spec: `docs/specs/zerodds-mqtt-bridge-1.0.md`.

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

// Spec §8.1: the daemon's log is JSON on stdout (`daemon::log`); an error
// that ends the start is written to stderr as well, for a reader of the
// terminal. `--help`, `--version` and `--check-config` print text. The
// Windows service has no stdout: its log goes to
// %PROGRAMDATA%\ZeroDDS\logs\mqtt-bridged.log (§11).
use std::process::ExitCode;

use zerodds_config::CommandLine;
use zerodds_dcps::cli::Configured;
use zerodds_mqtt_bridge::daemon::cli::{self, HELP_TEXT, VERSION_TEXT};
use zerodds_mqtt_bridge::daemon::config::{self, DaemonConfig};
use zerodds_mqtt_bridge::daemon::log::{self, Level};
use zerodds_mqtt_bridge::daemon::server::{self, ServerError};

/// The name of the Windows service (Spec §11, zerodds-deployment-1.0 §4.3).
const WINDOWS_SERVICE: &str = "ZeroDDSMQTTBridge";

/// On Windows, started by the service control manager, the daemon runs as
/// the service `ZeroDDSMQTTBridge`; elsewhere and from a console, as it is.
fn main() -> ExitCode {
    zerodds_dcps::cli::run_as_service(WINDOWS_SERVICE, daemon_main)
}

fn daemon_main() -> ExitCode {
    // Spec §11: run by the service control manager, the process has no
    // stdout; the log goes to the file of zerodds-deployment-1.0 §4.1. A
    // file that cannot be opened ends the start: the service stops with
    // exit code 1.
    if zerodds_dcps::cli::running_as_service() {
        let path = log::service_log_path(std::env::var_os("PROGRAMDATA").as_deref());
        if let Err(e) = log::to_file(&path) {
            return start_failed("io", &format!("the log file {}: {e}", path.display()), 1);
        }
    }
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if raw_args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-mqtt-bridged {}", env!("CARGO_PKG_VERSION"));
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
    // Spec §2: the convenience flags override file values; they are
    // `--set` entries of their keys, after any explicit `--set`.
    for assignment in args.sets() {
        if let Err(e) = command_line.push_set(&assignment) {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
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
        config::ENV,
    ) {
        Ok(c) => c,
        Err(e) => {
            if checking {
                eprintln!("[zerodds-mqtt-bridged] config error: {e}");
                return ExitCode::from(2);
            }
            return start_failed("config", &e.to_string(), 1);
        }
    };
    let cfg = match DaemonConfig::from_config(configured.source.tree()) {
        Ok(c) => c,
        Err(e) => {
            if configured.checking() {
                println!("error: {e}\nconfiguration invalid");
                return ExitCode::from(1);
            }
            return start_failed("config", &e.to_string(), 1);
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
            config::ENV,
        )
        .map_err(|e| e.to_string())?;
        DaemonConfig::from_config(configured.source.tree()).map_err(|e| e.to_string())
    });
    // Spec §8.1: the threshold from the first line on.
    log::set_level(&cfg.log_level);
    match run(cfg, configured.runtime, load) {
        Ok(()) => ExitCode::SUCCESS,
        // SIGTERM/SIGINT while the start connected: a normal shutdown.
        Err(ServerError::Stopped) => {
            log::event(Level::Info, "stopped").emit();
            ExitCode::SUCCESS
        }
        // Spec §2: the exit code names the kind of error.
        Err(ServerError::Config(m)) => start_failed("config", &m, 1),
        Err(ServerError::BrokerConnect(m)) => start_failed("broker", &m, 2),
        Err(ServerError::Dds(m)) => start_failed("dds", &m, 3),
        Err(ServerError::Tls(m)) => start_failed("tls", &m, 4),
        Err(ServerError::Auth(m)) => start_failed("auth", &m, 5),
        Err(ServerError::Io(m)) => start_failed("io", &m, 1),
    }
}

/// An error that ends the start (Spec §2, §8.1): the log line
/// `start_failed` on stdout, the same as text on stderr, and the exit code.
fn start_failed(kind: &str, reason: &str, code: u8) -> ExitCode {
    log::event(Level::Error, "start_failed")
        .field("kind", kind)
        .field("reason", reason)
        .emit();
    eprintln!("[zerodds-mqtt-bridged] {kind} error: {reason}");
    ExitCode::from(code)
}

fn run(
    cfg: DaemonConfig,
    runtime: zerodds_dcps::runtime::RuntimeConfig,
    load: server::ConfigLoader,
) -> Result<(), ServerError> {
    let mut handle = server::start_with_reload(cfg, runtime, Some(load))?;
    log::event(Level::Info, "running")
        .field("admin", handle.admin_addr.as_deref().unwrap_or(""))
        .emit();
    // SIGTERM/SIGINT set the stop flag; the loop used to park forever, so
    // the process outlived the signal (measured: still running 3 s after
    // SIGTERM) until a SIGKILL ended it without DISCONNECT.
    zerodds_dcps::cli::notify_service_manager("READY=1");
    while !handle.stop_requested() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    log::event(Level::Info, "shutdown").emit();
    handle.shutdown();
    log::event(Level::Info, "stopped").emit();
    Ok(())
}
