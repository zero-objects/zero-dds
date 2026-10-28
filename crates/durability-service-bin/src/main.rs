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
//!
//! Configuration: `durability-svc.yaml` (+ `durability-svc.d/`) in the
//! system configuration directory, `--config PATH` for another, `--set
//! PATH=VALUE` over the file, and this binary's own flags as `--set` of
//! their key — see [`cli::HELP_TEXT`] and `zerodds-durability-svc(5)`.
//! Per-topic `type`/`keyed`/`contract` overrides are config-only (no CLI
//! equivalent): `--topic NAME` resolves the type and contract from
//! discovery, exactly like `--auto` does — UNLESS the store already has
//! metadata for that topic from an earlier run, in which case it resumes
//! immediately (`DurabilityService::serve_if_stored`), with no writer/reader
//! to wait for.

#![allow(clippy::print_stdout, clippy::print_stderr)]

mod cli;
mod config;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc;

use config::DaemonConfig;
use zerodds_config::CommandLine;
use zerodds_dcps::cli::Configured;
use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::{Contract, DurabilityStore};
use zerodds_durability_store_file::FileStore;
#[cfg(feature = "lakehouse")]
use zerodds_durability_store_lakehouse::LakehouseStore;
#[cfg(feature = "postgres")]
use zerodds_durability_store_postgres::PostgresStore;
use zerodds_durability_store_sqlite::SqliteStore;

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if raw_args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-durability-svc {}", env!("CARGO_PKG_VERSION"));
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
            eprintln!("{}\n{}", cli::HELP_TEXT, zerodds_dcps::cli::HELP);
            return ExitCode::from(1);
        }
    };
    if args.help {
        println!("{}\n{}", cli::HELP_TEXT, zerodds_dcps::cli::HELP);
        return ExitCode::SUCCESS;
    }
    if args.version {
        println!("{}", cli::VERSION_TEXT);
        return ExitCode::SUCCESS;
    }
    // The convenience flags override file values; they are `--set` entries
    // of their keys, after any explicit `--set`.
    for assignment in args.sets() {
        if let Err(e) = command_line.push_set(&assignment) {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    }
    // A file that does not load (unreadable, syntax, include) is exit 2
    // under `--check-config`; at start it is a configuration error, exit 1.
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
            eprintln!("zerodds-durability-svc: error: configuration: {e}");
            return ExitCode::from(if checking { 2 } else { 1 });
        }
    };
    let cfg = match DaemonConfig::from_config(configured.source.tree()) {
        Ok(c) => c,
        Err(e) => {
            if configured.checking() {
                println!("error: {e}\nconfiguration invalid");
            } else {
                eprintln!("zerodds-durability-svc: error: {e}");
            }
            return ExitCode::from(1);
        }
    };
    if let Some(code) = configured.check_config() {
        return code;
    }
    match run(cfg, configured.runtime) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("zerodds-durability-svc: error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn build_store(cfg: &DaemonConfig, contract: Contract) -> Result<Arc<dyn DurabilityStore>, String> {
    let store: Arc<dyn DurabilityStore> = match cfg.store.as_str() {
        "sqlite" => match &cfg.path {
            Some(p) => Arc::new(SqliteStore::open(p, contract).map_err(|e| e.to_string())?),
            None => {
                eprintln!("warning: sqlite in-memory store — NOT durable across restart");
                Arc::new(SqliteStore::open_in_memory(contract).map_err(|e| e.to_string())?)
            }
        },
        #[cfg(feature = "lakehouse")]
        "lakehouse" => match &cfg.path {
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
        #[cfg(feature = "postgres")]
        "postgres" => {
            let url = cfg
                .path
                .as_ref()
                .ok_or_else(|| "postgres store requires path: CONNECTION_STRING".to_string())?;
            Arc::new(PostgresStore::connect(url, contract).map_err(|e| e.to_string())?)
        }
        #[cfg(not(feature = "postgres"))]
        "postgres" => {
            return Err("the 'postgres' backend is not compiled into this binary — \
                 rebuild with `--features postgres`"
                .to_string());
        }
        "file" => {
            let p = cfg
                .path
                .as_ref()
                .ok_or_else(|| "file store requires path: DIRECTORY".to_string())?;
            Arc::new(FileStore::open(p, contract).map_err(|e| e.to_string())?)
        }
        other => {
            return Err(format!(
                "unknown store '{other}' (sqlite|file|lakehouse|postgres)"
            ));
        }
    };
    Ok(store)
}

fn run(cfg: DaemonConfig, runtime: zerodds_dcps::runtime::RuntimeConfig) -> Result<(), String> {
    // `from_config` refuses a `None` domain, so `cfg` reaching here always
    // has one.
    let domain = cfg.domain.unwrap_or_default();
    let contract = Contract::keep_all();
    let store = build_store(&cfg, contract)?;

    let service = Arc::new(
        DurabilityService::start_with_config(domain, store, runtime).map_err(|e| e.to_string())?,
    );
    println!(
        "zerodds-durability-svc: domain={} store={} path={}",
        domain,
        cfg.store,
        cfg.path.as_deref().unwrap_or("<memory>")
    );

    if cfg.auto {
        service
            .enable_auto_discovery(contract)
            .map_err(|e| e.to_string())?;
        println!("  auto-discovery: ON (TRANSIENT/PERSISTENT topics)");
    }
    for topic in &cfg.topics {
        match (&topic.type_name, topic.contract) {
            (Some(type_name), explicit_contract) => {
                let keyed = topic.keyed.unwrap_or(false);
                let topic_contract = explicit_contract.unwrap_or(contract);
                service
                    .serve_typed(&topic.name, type_name, keyed, topic_contract)
                    .map_err(|e| e.to_string())?;
                println!(
                    "  serving topic: {} (type={type_name} keyed={keyed} contract=config)",
                    topic.name
                );
            }
            (None, Some(explicit_contract)) => {
                // No `type` override. If the store already has TopicMeta for
                // this topic (an earlier run resolved and persisted its type
                // + keyedness), resume serving it immediately — the pinned
                // contract wins over whatever was stored, config always
                // outranks a previously-resolved value. Otherwise fall back
                // to today's behaviour: type/keyedness come from discovery,
                // with the contract pinned regardless of what the discovered
                // writer announces. Either way, `serve_discovered` also runs
                // (a no-op re-serve when already served) so a later writer's
                // disagreement is still watched for and reported, never
                // silently applied.
                let served_now = service
                    .serve_if_stored(&topic.name, Some(explicit_contract))
                    .map_err(|e| e.to_string())?;
                service
                    .serve_discovered(&topic.name, explicit_contract)
                    .map_err(|e| e.to_string())?;
                println!(
                    "  serving topic: {} (type={} contract=config)",
                    topic.name,
                    if served_now { "stored" } else { "discovered" }
                );
            }
            (None, None) => {
                // Neither a type nor a contract override. Same precedence as
                // above, minus the pinned contract: a topic the store already
                // has metadata for resumes immediately under its stored type/
                // keyedness/contract; a genuinely new topic still resolves
                // all three from whichever writer is discovered first —
                // exactly like `--auto`, but for a topic named explicitly and
                // regardless of its writer's durability kind. Either way,
                // `serve_discovered` also runs (a no-op re-serve when already
                // served) so a later writer's disagreement is still watched
                // for and reported, never silently applied.
                let served_now = service
                    .serve_if_stored(&topic.name, None)
                    .map_err(|e| e.to_string())?;
                service
                    .serve_discovered(&topic.name, contract)
                    .map_err(|e| e.to_string())?;
                println!(
                    "  serving topic: {} (type={} contract={})",
                    topic.name,
                    if served_now { "stored" } else { "discovered" },
                    if served_now {
                        "stored"
                    } else {
                        "discovered/default"
                    }
                );
            }
        }
    }

    // Graceful shutdown on Ctrl-C / SIGTERM.
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .map_err(|e| format!("installing signal handler: {e}"))?;

    zerodds_dcps::cli::notify_service_manager("READY=1");
    println!("running — send SIGINT/SIGTERM to stop");
    let _ = rx.recv();
    zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    println!("shutting down…");
    service.shutdown();
    Ok(())
}
