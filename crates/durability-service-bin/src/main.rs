// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-durability-svc` — the standalone ZeroDDS Durability-Service daemon
//! (ADR 0009). One process per domain, a pluggable cold-store adapter, and
//! optional auto-discovery of TRANSIENT/PERSISTENT topics.
//!
//! ```text
//! zerodds-durability-svc --domain 0 --store sqlite --path /var/lib/zerodds/dur.db --auto
//! zerodds-durability-svc --domain 0 --store file   --path /var/lib/zerodds/dur   --topic Sensors --topic Cmd
//! zerodds-durability-svc --domain 0 --store lakehouse --path /var/lib/zerodds/dur.duckdb --auto
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc;

use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::{Contract, DurabilityStore};
use zerodds_durability_store_file::FileStore;
#[cfg(feature = "lakehouse")]
use zerodds_durability_store_lakehouse::LakehouseStore;
use zerodds_durability_store_sqlite::SqliteStore;

const USAGE: &str = "\
zerodds-durability-svc — ZeroDDS Durability-Service daemon (ADR 0009)

USAGE:
  zerodds-durability-svc --domain N [--store sqlite|file|lakehouse] [--path PATH]
                         [--auto] [--topic NAME]...

OPTIONS:
  --domain N        DDS domain id (required). One daemon per domain.
  --store KIND      Cold-store adapter: sqlite (default) | file | lakehouse.
                    lakehouse (DuckDB) is opt-in: build with --features lakehouse.
  --path PATH       Store location. sqlite/lakehouse: a db file (omit = in-memory,
                    NOT durable across restart). file: a directory (required).
  --auto            Auto-serve every topic whose writers declare TRANSIENT or
                    PERSISTENT durability (no application-side code needed).
  --topic NAME      Explicitly serve a topic (repeatable). Combine with --auto
                    or use instead of it. At least one of --auto / --topic.
  -h, --help        This help.
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("zerodds-durability-svc: error: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Args {
    domain: i32,
    store: String,
    path: Option<String>,
    auto: bool,
    topics: Vec<String>,
}

fn parse_args() -> Result<Args, String> {
    let argv: Vec<String> = std::env::args().collect();
    let mut domain: Option<i32> = None;
    let mut store = "sqlite".to_string();
    let mut path: Option<String> = None;
    let mut auto = false;
    let mut topics: Vec<String> = Vec::new();

    let mut i = 1;
    while i < argv.len() {
        let a = &argv[i];
        let mut val = || {
            i += 1;
            argv.get(i)
                .cloned()
                .ok_or_else(|| format!("missing value for {a}"))
        };
        match a.as_str() {
            "--domain" => {
                domain = Some(val()?.parse().map_err(|_| "invalid --domain".to_string())?)
            }
            "--store" => store = val()?,
            "--path" => path = Some(val()?),
            "--auto" => auto = true,
            "--topic" => topics.push(val()?),
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}\n\n{USAGE}")),
        }
        i += 1;
    }

    let domain = domain.ok_or_else(|| format!("--domain is required\n\n{USAGE}"))?;
    if !auto && topics.is_empty() {
        return Err(format!(
            "at least one of --auto or --topic is required\n\n{USAGE}"
        ));
    }
    Ok(Args {
        domain,
        store,
        path,
        auto,
        topics,
    })
}

fn build_store(args: &Args, contract: Contract) -> Result<Arc<dyn DurabilityStore>, String> {
    let store: Arc<dyn DurabilityStore> = match args.store.as_str() {
        "sqlite" => match &args.path {
            Some(p) => Arc::new(SqliteStore::open(p, contract).map_err(|e| e.to_string())?),
            None => {
                eprintln!("warning: sqlite in-memory store — NOT durable across restart");
                Arc::new(SqliteStore::open_in_memory(contract).map_err(|e| e.to_string())?)
            }
        },
        #[cfg(feature = "lakehouse")]
        "lakehouse" => match &args.path {
            Some(p) => Arc::new(LakehouseStore::open(p, contract).map_err(|e| e.to_string())?),
            None => {
                eprintln!("warning: lakehouse in-memory store — NOT durable across restart");
                Arc::new(LakehouseStore::open_in_memory(contract).map_err(|e| e.to_string())?)
            }
        },
        #[cfg(not(feature = "lakehouse"))]
        "lakehouse" => {
            return Err(
                "the 'lakehouse' (DuckDB) backend is not compiled into this binary — \
                 rebuild with `--features lakehouse` (requires the system libduckdb)"
                    .to_string(),
            );
        }
        "file" => {
            let p = args
                .path
                .as_ref()
                .ok_or_else(|| "file store requires --path DIRECTORY".to_string())?;
            Arc::new(FileStore::open(p, contract).map_err(|e| e.to_string())?)
        }
        other => return Err(format!("unknown --store '{other}' (sqlite|file|lakehouse)")),
    };
    Ok(store)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let contract = Contract::keep_all();
    let store = build_store(&args, contract)?;

    let service =
        Arc::new(DurabilityService::start(args.domain, store).map_err(|e| e.to_string())?);
    println!(
        "zerodds-durability-svc: domain={} store={} path={}",
        args.domain,
        args.store,
        args.path.as_deref().unwrap_or("<memory>")
    );

    if args.auto {
        service
            .enable_auto_discovery(contract)
            .map_err(|e| e.to_string())?;
        println!("  auto-discovery: ON (TRANSIENT/PERSISTENT topics)");
    }
    for topic in &args.topics {
        service.serve(topic, contract).map_err(|e| e.to_string())?;
        println!("  serving topic: {topic}");
    }

    // Graceful shutdown on Ctrl-C / SIGTERM.
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .map_err(|e| format!("installing signal handler: {e}"))?;

    println!("running — send SIGINT/SIGTERM to stop");
    let _ = rx.recv();
    println!("shutting down…");
    service.shutdown();
    Ok(())
}
