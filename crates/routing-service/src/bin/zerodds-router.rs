// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-router` — standalone DDS routing daemon.
//!
//! ```text
//! zerodds-router run      --config <file> [--types <shapes.json>]
//! zerodds-router validate --config <file>
//! ```
//!
//! `--config` is JSON or XML (chosen by extension; `.xml` → XML, else JSON).
//! `--types` is an optional JSON array of type shapes required by routes that
//! use a content filter or field transformation.

// A CLI daemon: human-facing status and errors go to stdout/stderr by design.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;
use std::sync::mpsc;

use zerodds_routing_service::{Router, RouterConfig, TypeRegistry};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let cmd = args.get(1).map(String::as_str);
    match cmd {
        Some("run") => cmd_run(&args[2..]),
        Some("validate") => cmd_validate(&args[2..]),
        Some("-h" | "--help") | None => {
            print_usage();
            Ok(ExitCode::SUCCESS)
        }
        Some(other) => {
            eprintln!("unknown command '{other}'");
            print_usage();
            Ok(ExitCode::FAILURE)
        }
    }
}

fn print_usage() {
    eprintln!(
        "zerodds-router — DDS routing daemon\n\n\
         USAGE:\n  \
         zerodds-router run      --config <file> [--types <shapes.json>]\n  \
         zerodds-router validate --config <file>\n\n\
         --config  route config (.xml → XML, else JSON)\n  \
         --types   JSON array of type shapes (for filter/transform routes)"
    );
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn load_config(path: &str) -> Result<RouterConfig, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let cfg = if path.ends_with(".xml") {
        RouterConfig::from_xml(&text)
    } else {
        RouterConfig::from_json(&text)
    }
    .map_err(|e| format!("parse {path}: {e}"))?;
    Ok(cfg)
}

fn load_types(path: Option<&str>) -> Result<TypeRegistry, String> {
    match path {
        None => Ok(TypeRegistry::new()),
        Some(p) => {
            let text = std::fs::read_to_string(p).map_err(|e| format!("read {p}: {e}"))?;
            TypeRegistry::from_json(&text).map_err(|e| format!("parse {p}: {e}"))
        }
    }
}

fn cmd_validate(args: &[String]) -> Result<ExitCode, String> {
    let path = opt(args, "--config").ok_or("validate: --config is required")?;
    let cfg = load_config(path)?;
    println!("config '{}' OK — {} route(s):", cfg.name, cfg.routes.len());
    for r in &cfg.routes {
        println!(
            "  {}: domain {} '{}' → domain {} '{}'{}{}",
            r.name,
            r.input.domain,
            r.input.topic,
            r.output.domain,
            r.output.topic,
            if r.filter.is_some() { " [filter]" } else { "" },
            if r.transform.is_some() {
                " [transform]"
            } else {
                ""
            },
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_run(args: &[String]) -> Result<ExitCode, String> {
    let path = opt(args, "--config").ok_or("run: --config is required")?;
    let cfg = load_config(path)?;
    let types = load_types(opt(args, "--types"))?;

    let router = Router::start_with_types(&cfg, types).map_err(|e| format!("start router: {e}"))?;
    println!(
        "router '{}' started — {} route(s). send SIGINT/SIGTERM to stop.",
        router.name(),
        router.route_names().len()
    );

    // Graceful shutdown on Ctrl-C / SIGTERM — block on the channel (no poll).
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .map_err(|e| format!("install signal handler: {e}"))?;

    let _ = rx.recv();

    println!("stopping…");
    for name in router.route_names() {
        if let Some(m) = router.route_metrics(&name) {
            println!(
                "  {name}: forwarded={} dropped_loop={} dropped_filter={} lifecycle={} errors={}",
                m.forwarded, m.dropped_loop, m.dropped_filter, m.lifecycle, m.errors
            );
        }
    }
    drop(router);
    Ok(ExitCode::SUCCESS)
}
