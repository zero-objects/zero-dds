// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-xrce-agentd` — the DDS-XRCE agent daemon.
//!
//! Crate README: `crates/xrce-agent/README.md`. Man pages:
//! `man/man1/zerodds-xrce-agentd.1`, `man/man5/zerodds-xrce-agentd.yaml.5`.

#![forbid(unsafe_code)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::ExitCode;

use zerodds_config::CommandLine;
use zerodds_dcps::cli::Configured;
use zerodds_xrce_agent::daemon::cli::{self, HELP_TEXT, VERSION_TEXT};
use zerodds_xrce_agent::daemon::config::{self, DaemonConfig};
use zerodds_xrce_agent::daemon::server::{self, ServerError};

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if raw_args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-xrce-agentd {}", env!("CARGO_PKG_VERSION"));
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
            eprintln!("[zerodds-xrce-agentd] config error: {e}");
            return ExitCode::from(if checking { 2 } else { 1 });
        }
    };
    let cfg = match DaemonConfig::from_config(configured.source.tree()) {
        Ok(c) => c,
        Err(e) => {
            if configured.checking() {
                println!("error: {e}\nconfiguration invalid");
            } else {
                eprintln!("[zerodds-xrce-agentd] config error: {e}");
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
            eprintln!("[zerodds-xrce-agentd] config error: {m}");
            ExitCode::from(1)
        }
        Err(ServerError::Bind(m)) => {
            eprintln!("[zerodds-xrce-agentd] bind error: {m}");
            ExitCode::from(2)
        }
        Err(ServerError::Dds(m)) => {
            eprintln!("[zerodds-xrce-agentd] dds error: {m}");
            ExitCode::from(3)
        }
    }
}

fn run(
    cfg: DaemonConfig,
    runtime: zerodds_dcps::runtime::RuntimeConfig,
    load: server::ConfigLoader,
) -> Result<(), ServerError> {
    let mut handle = server::start(cfg, runtime, Some(load))?;
    eprintln!("[zerodds-xrce-agentd] running");
    zerodds_dcps::cli::notify_service_manager("READY=1");
    let _signals = server::install_signal_watcher(handle.stop_flag(), handle.reload_flag());
    while !handle.stop_requested() {
        handle.reload_if_requested();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    handle.shutdown();
    Ok(())
}
