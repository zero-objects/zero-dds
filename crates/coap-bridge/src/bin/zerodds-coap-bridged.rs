// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-coap-bridged` — DDS↔CoAP-Bridge-Daemon.
//!
//! Spec: `docs/specs/zerodds-coap-bridge-1.0.md`.

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

use std::process::ExitCode;

use zerodds_coap_bridge::daemon::cli::{self, HELP_TEXT, VERSION_TEXT};
use zerodds_coap_bridge::daemon::config::{self, DaemonConfig};
use zerodds_coap_bridge::daemon::server::{self, ServerError};
use zerodds_config::CommandLine;
use zerodds_dcps::cli::Configured;

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if raw_args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-coap-bridged {}", env!("CARGO_PKG_VERSION"));
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
    // Spec §2/§7.1: the convenience flags override file values; they are
    // `--set` entries of their keys, after any explicit `--set`.
    let sets = match args.sets() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    for assignment in sets {
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
        &[],
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[zerodds-coap-bridged] config error: {e}");
            return ExitCode::from(if checking { 2 } else { 1 });
        }
    };
    let cfg = match DaemonConfig::from_config(configured.source.tree()) {
        Ok(c) => c,
        Err(e) => {
            if configured.checking() {
                println!("error: {e}\nconfiguration invalid");
            } else {
                eprintln!("[zerodds-coap-bridged] config error: {e}");
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
            eprintln!("[zerodds-coap-bridged] config error: {m}");
            ExitCode::from(1)
        }
        Err(ServerError::Bind(m)) => {
            eprintln!("[zerodds-coap-bridged] bind error: {m}");
            ExitCode::from(2)
        }
        Err(ServerError::Dds(m)) => {
            eprintln!("[zerodds-coap-bridged] dds error: {m}");
            ExitCode::from(3)
        }
        Err(ServerError::Dtls(m)) => {
            eprintln!("[zerodds-coap-bridged] dtls error: {m}");
            ExitCode::from(4)
        }
        Err(ServerError::Oscore(m)) => {
            eprintln!("[zerodds-coap-bridged] oscore error: {m}");
            ExitCode::from(4)
        }
        Err(ServerError::Io(m)) => {
            eprintln!("[zerodds-coap-bridged] io error: {m}");
            ExitCode::from(1)
        }
    }
}

fn run(
    cfg: DaemonConfig,
    runtime: zerodds_dcps::runtime::RuntimeConfig,
    load: server::ConfigLoader,
) -> Result<(), ServerError> {
    let mut handle = server::start_with_reload(cfg, runtime, Some(load))?;
    eprintln!("[zerodds-coap-bridged] running");
    // SIGTERM/SIGINT set the stop flag; the loop used to park forever and
    // the daemon was still running 3 s after SIGTERM (measured).
    zerodds_dcps::cli::notify_service_manager("READY=1");
    while !handle.stop_requested() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    handle.shutdown();
    Ok(())
}
