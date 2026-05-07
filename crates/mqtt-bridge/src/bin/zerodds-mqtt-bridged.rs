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

use std::path::Path;
use std::process::ExitCode;

use zerodds_mqtt_bridge::daemon::cli::{self, CliArgs, HELP_TEXT, VERSION_TEXT};
use zerodds_mqtt_bridge::daemon::config::DaemonConfig;
use zerodds_mqtt_bridge::daemon::server::{self, ServerError};

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
        Err(ServerError::BrokerConnect(m)) => {
            eprintln!("[zerodds-mqtt-bridged] broker error: {m}");
            ExitCode::from(2)
        }
        Err(ServerError::Dds(m)) => {
            eprintln!("[zerodds-mqtt-bridged] dds error: {m}");
            ExitCode::from(3)
        }
        Err(ServerError::Tls(m)) => {
            eprintln!("[zerodds-mqtt-bridged] tls error: {m}");
            ExitCode::from(4)
        }
        Err(ServerError::Auth(m)) => {
            eprintln!("[zerodds-mqtt-bridged] auth error: {m}");
            ExitCode::from(5)
        }
        Err(ServerError::Io(m)) => {
            eprintln!("[zerodds-mqtt-bridged] io error: {m}");
            ExitCode::from(1)
        }
    }
}

fn run(args: CliArgs) -> Result<(), ServerError> {
    let mut cfg = if let Some(path) = &args.config {
        DaemonConfig::load_from_file(Path::new(path))
            .map_err(|e| ServerError::Io(format!("config: {e}")))?
    } else {
        DaemonConfig::default_for_dev()
    };
    if let Some(b) = args.broker {
        cfg.broker_url = b;
    }
    if let Some(c) = args.client_id {
        cfg.client_id = c;
    }
    if let Some(d) = args.domain {
        cfg.domain = d;
    }
    if let Some(u) = args.username {
        cfg.username = Some(u);
    }
    if let Some(p) = args.password {
        cfg.password = Some(p);
    }
    if let Some(lvl) = args.log_level {
        cfg.log_level = lvl;
    }

    let _handle = server::start(cfg)?;
    eprintln!("[zerodds-mqtt-bridged] running");

    loop {
        std::thread::park_timeout(std::time::Duration::from_secs(60));
    }
}
