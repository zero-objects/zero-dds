// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `zerodds-naming-svc` — standalone OMG CosNaming 1.3 server.
//
// Spec: this crate's README. The command line and the layered config
// load; the server itself is `zerodds_corba_cosnaming::naming_daemon`.

//! `zerodds-naming-svc`: the command line of the standalone NameService.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use zerodds_config::{CommandLine, Config, env_config, loader_for, unknown_keys};
use zerodds_corba_cosnaming::naming_daemon::{self, DaemonConfig, DaemonError, ENV, is_key};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The base name of the daemon's file: `naming-svc.yaml`, `naming-svc.d/`.
const CONFIG_STEM: &str = "naming-svc";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.code)
        }
    }
}

fn daemon_error(code: u8, msg: impl Into<String>) -> DaemonError {
    DaemonError {
        code,
        msg: msg.into(),
    }
}

/// Loads the layered configuration (env, files, `--set`), the same
/// order `zerodds_dcps::cli::Configured` uses for every other daemon
/// (env below files below flags) — see `naming_daemon`'s module doc for
/// why this program builds it directly on `zerodds-config` instead of
/// depending on `zerodds-dcps` for it.
fn load_config(command_line: &CommandLine) -> Result<Config, zerodds_config::ConfigError> {
    for (path, _) in command_line.sets() {
        let bare = match path.rsplit_once('.') {
            Some((head, last)) => format!("{head}.{}", last.trim_start_matches('+')),
            None => path.trim_start_matches('+').to_string(),
        };
        if !is_key(&bare) {
            return Err(zerodds_config::ConfigError::Usage(format!(
                "--set {path}: not a known setting (`--check-config` lists them)"
            )));
        }
    }
    let env_layer = env_config(ENV, |name| env::var(name).ok(), |_| false)?;
    let mut files = loader_for(command_line, CONFIG_STEM).load_onto(env_layer)?;
    command_line.apply(&mut files)?;
    Ok(files)
}

fn run() -> Result<(), DaemonError> {
    let args: Vec<String> = env::args().skip(1).collect();
    let (mut command_line, rest) =
        CommandLine::extract(args).map_err(|e| daemon_error(1, e.to_string()))?;

    let mut sets: Vec<String> = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i].as_str();
        let mut value = || -> Result<String, DaemonError> {
            i += 1;
            rest.get(i)
                .cloned()
                .ok_or_else(|| daemon_error(1, format!("{flag} needs a value")))
        };
        match flag {
            "--bind" => sets.push(format!("naming.bind={}", value()?)),
            "--advertise-host" => sets.push(format!("naming.advertise_host={}", value()?)),
            "--persist-file" => sets.push(format!("naming.persist_file={}", value()?)),
            "--ssliop-bind" => {
                sets.push(format!("naming.ssliop.bind={}", value()?));
                sets.push("naming.ssliop.enabled=true".to_string());
            }
            "--tls-cert" => sets.push(format!("naming.ssliop.cert_file={}", value()?)),
            "--tls-key" => sets.push(format!("naming.ssliop.key_file={}", value()?)),
            "--log-level" => sets.push(format!("log_level={}", value()?)),
            "--version" | "-V" => {
                println!("zerodds-naming-svc {VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => return Err(daemon_error(1, format!("unknown argument: {other}"))),
        }
        i += 1;
    }
    for assignment in &sets {
        command_line
            .push_set(assignment)
            .map_err(|e| daemon_error(1, e.to_string()))?;
    }

    let checking = command_line.check();
    let config = load_config(&command_line)
        .map_err(|e| daemon_error(if checking { 2 } else { 1 }, format!("config: {e}")))?;
    let cfg = match DaemonConfig::from_config(config.root()) {
        Ok(cfg) => cfg,
        Err(e) if checking => {
            println!("error: {e}\nconfiguration invalid");
            return Err(daemon_error(1, "configuration invalid"));
        }
        Err(e) => return Err(daemon_error(1, format!("config: {e}"))),
    };
    if checking {
        for (path, origin) in unknown_keys(&config, is_key) {
            eprintln!("warning: {origin}: unknown setting {path} is not read");
        }
        println!("configuration valid");
        return Ok(());
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    #[cfg(unix)]
    {
        let sd = Arc::clone(&shutdown);
        let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, sd);
        let sd = Arc::clone(&shutdown);
        let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, sd);
    }

    naming_daemon::run(&cfg, &shutdown)
}

fn print_usage() {
    eprintln!("zerodds-naming-svc {VERSION}");
    eprintln!();
    eprintln!("USAGE: zerodds-naming-svc [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --config <FILE>             base file (default: naming-svc.yaml in the");
    eprintln!("                              system configuration directory) with FILE.d/");
    eprintln!("  --set <PATH=VALUE>          one setting above every file, repeatable");
    eprintln!("  --check-config              validate, print every value with its origin, exit");
    eprintln!("  --bind <ADDR>               IIOP listen (default 0.0.0.0:2809)");
    eprintln!("  --advertise-host <HOST>     host named in every IOR when --bind is a wildcard");
    eprintln!("  --persist-file <FILE>       where the naming graph is saved (default: none)");
    eprintln!("  --ssliop-bind <ADDR>        SSLIOP listen (default 0.0.0.0:2810)");
    eprintln!("  --tls-cert <FILE>           SSLIOP server cert");
    eprintln!("  --tls-key <FILE>            SSLIOP server key");
    eprintln!("  --log-level <LEVEL>         trace|debug|info|warn|error");
    eprintln!();
    eprintln!("  --version                   Print version");
    eprintln!("  --help                      Show this help");
}
