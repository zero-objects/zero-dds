// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `zerodds-corba-bridged` — DDS↔CORBA bridge daemon (IIOP server).
//
// Spec: `docs/specs/zerodds-corba-bridge-1.0.md`. The command line, the
// admin endpoint and the observability hooks; the bridge itself is
// `zerodds_corba_dds_bridge::bridge_daemon`.

//! `zerodds-corba-bridged`: the command line of the DDS↔CORBA bridge.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_corba_dds_bridge::bridge_daemon::{self, DaemonConfig, DaemonError, is_key};
use zerodds_corba_dds_bridge::daemon_runtime::{
    BridgeMetrics, CatalogSnapshot, CatalogTopic, SERVICE_NAME, install_signal_watcher,
    otlp_config_from_env, serve_admin_endpoints, spawn_otlp_flush_loop,
};
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The base name of the daemon's file: `corba-bridged.yaml`, `corba-bridged.d/`.
const CONFIG_STEM: &str = "corba-bridged";

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

fn run() -> Result<(), DaemonError> {
    let args: Vec<String> = env::args().skip(1).collect();
    let (mut command_line, rest) =
        zerodds_config::CommandLine::extract(args).map_err(|e| daemon_error(1, e.to_string()))?;
    let mut sets: Vec<String> = Vec::new();
    let mut tls_cert: Option<String> = None;
    let mut tls_key: Option<String> = None;
    let mut dump_iors = false;
    let mut idle_timeout_ms: u64 = 0;

    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i].as_str();
        let mut value = || -> Result<String, DaemonError> {
            i += 1;
            rest.get(i)
                .cloned()
                .ok_or_else(|| daemon_error(1, format!("{flag} needs a value")))
        };
        // Spec §2: each option is `--set` of its file key, after any
        // explicit `--set`, so it wins over the files and names itself as
        // the origin.
        match flag {
            "--iiop-bind" => sets.push(format!("corba.iiop.bind={}", value()?)),
            "--ssliop-bind" => {
                sets.push(format!("corba.ssliop.bind={}", value()?));
                sets.push("corba.ssliop.enabled=true".to_string());
            }
            "--domain" => sets.push(format!("domain={}", value()?)),
            "--orb-id" => sets.push(format!("corba.orb_id={}", value()?)),
            "--tls-cert" => tls_cert = Some(value()?),
            "--tls-key" => tls_key = Some(value()?),
            "--topic" => sets.push(format!("+mappings={}", value()?)),
            "--log-level" => sets.push(format!("log_level={}", value()?)),
            "--metrics" => {
                sets.push(format!("metrics.listen={}", value()?));
                sets.push("metrics.enabled=true".to_string());
            }
            "--dump-iors" => dump_iors = true,
            "--idle-timeout-ms" => {
                let v = value()?;
                idle_timeout_ms = v.parse().map_err(|_| {
                    daemon_error(1, format!("--idle-timeout-ms {v}: expected milliseconds"))
                })?;
            }
            "--version" | "-V" => {
                println!("zerodds-corba-bridged {VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => {
                return Err(daemon_error(1, format!("unknown argument: {other}")));
            }
        }
        i += 1;
    }
    match (tls_cert, tls_key) {
        (Some(c), Some(k)) => {
            sets.push(format!("corba.ssliop.cert_file={c}"));
            sets.push(format!("corba.ssliop.key_file={k}"));
            sets.push("corba.ssliop.enabled=true".to_string());
        }
        (None, None) => {}
        (Some(_), None) => return Err(daemon_error(1, "--tls-cert needs --tls-key")),
        (None, Some(_)) => return Err(daemon_error(1, "--tls-key needs --tls-cert")),
    }
    for assignment in &sets {
        command_line
            .push_set(assignment)
            .map_err(|e| daemon_error(1, e.to_string()))?;
    }
    // Kept for SIGHUP: the reload reads the same layered sources again.
    let reload_command_line = command_line.clone();
    // A file that does not load (unreadable, syntax, include) is exit 2
    // under `--check-config` (deployment §2.1); at start it is a
    // configuration error, exit 1 — 2 is the bind error of spec §2.
    let checking = command_line.check();
    let configured = zerodds_dcps::cli::Configured::from_command_line(
        command_line,
        Vec::new(),
        CONFIG_STEM,
        is_key,
        bridge_daemon::ENV,
    )
    .map_err(|e| daemon_error(if checking { 2 } else { 1 }, format!("config: {e}")))?;
    let cfg = match DaemonConfig::from_config(configured.source.tree()) {
        Ok(cfg) => cfg,
        Err(e) if configured.checking() => {
            println!("error: {e}\nconfiguration invalid");
            return Err(daemon_error(1, "configuration invalid"));
        }
        Err(e) => return Err(daemon_error(1, format!("config: {e}"))),
    };
    if let Some(valid) = configured.check_config_valid() {
        return if valid {
            Ok(())
        } else {
            Err(daemon_error(1, "configuration invalid"))
        };
    }

    if dump_iors {
        for line in bridge_daemon::ior_lines(&cfg, &cfg.iiop_advertise_host)
            .map_err(|e| daemon_error(1, e))?
        {
            println!("{line}");
        }
        return Ok(());
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    // Metrics registry + standard counters (§8.2 Prometheus).
    let registry = Arc::new(Registry::new());
    let bridge_metrics = BridgeMetrics::register_shared(&registry);

    // Signal watcher (§9.2 graceful shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&shutdown), Arc::clone(&reload)) {
        eprintln!("{{\"event\":\"signal_watcher_init_failed\",\"err\":\"{e}\"}}");
    }

    // Admin endpoint (§5.2 catalog/healthz + §8.2 metrics).
    let healthy = Arc::new(AtomicBool::new(true));
    let _admin_h = if let Some(addr_s) = metrics_addr_for(&cfg) {
        match addr_s.parse::<std::net::SocketAddr>() {
            Ok(sa) => {
                let topics: Vec<CatalogTopic> = cfg
                    .mappings
                    .iter()
                    .map(|m| CatalogTopic {
                        dds_name: m.request_topic.dds_name.clone(),
                        amqp_address: m.repo_id.clone(),
                        direction: match m.direction {
                            bridge_daemon::Direction::CorbaToDds => "corba_to_dds".to_string(),
                            bridge_daemon::Direction::DdsToCorba => "dds_to_corba".to_string(),
                            bridge_daemon::Direction::Bidir => "bidir".to_string(),
                        },
                    })
                    .collect();
                let snap = Arc::new(CatalogSnapshot::new(topics));
                match serve_admin_endpoints(
                    sa,
                    snap,
                    Arc::clone(&registry),
                    Arc::clone(&healthy),
                    Arc::clone(&shutdown),
                ) {
                    Ok((h, bound)) => {
                        eprintln!("{{\"event\":\"admin_endpoint\",\"addr\":\"{bound}\"}}");
                        Some(h)
                    }
                    Err(e) => {
                        eprintln!("{{\"event\":\"admin_endpoint_failed\",\"err\":\"{e}\"}}");
                        None
                    }
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // OTLP exporter (§8.3).
    let _otlp_h = if let Some(otlp_cfg) = otlp_config_from_env(SERVICE_NAME) {
        let exp = Arc::new(OtlpExporter::new(otlp_cfg));
        spawn_otlp_flush_loop(exp, Arc::clone(&shutdown), Duration::from_secs(5)).ok()
    } else {
        None
    };

    if idle_timeout_ms > 0 {
        // Test hook: stop after the given time.
        let sd = Arc::clone(&shutdown);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(idle_timeout_ms));
            sd.store(true, Ordering::SeqCst);
        });
    }
    let reload_load = move || -> Result<DaemonConfig, String> {
        let configured = zerodds_dcps::cli::Configured::from_command_line(
            reload_command_line.clone(),
            Vec::new(),
            CONFIG_STEM,
            is_key,
            bridge_daemon::ENV,
        )
        .map_err(|e| e.to_string())?;
        DaemonConfig::from_config(configured.source.tree()).map_err(|e| e.to_string())
    };
    let res = bridge_daemon::run_with_reload(
        &cfg,
        configured.runtime.clone(),
        &shutdown,
        &bridge_metrics,
        Some(bridge_daemon::Reload {
            flag: Arc::clone(&reload),
            load: Box::new(reload_load),
        }),
    );
    healthy.store(false, Ordering::SeqCst);
    res
}

fn print_usage() {
    eprintln!("zerodds-corba-bridged {VERSION}");
    eprintln!();
    eprintln!("USAGE: zerodds-corba-bridged [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --config <FILE>             base file (default: corba-bridged.yaml in the");
    eprintln!("                              system configuration directory) with FILE.d/");
    eprintln!("  --set <PATH=VALUE>          one setting above every file, repeatable");
    eprintln!("  --check-config              validate, print every value with its origin, exit");
    eprintln!("  --iiop-bind <ADDR>          IIOP listen (default 0.0.0.0:6833)");
    eprintln!("  --ssliop-bind <ADDR>        SSLIOP listen (default 0.0.0.0:6834)");
    eprintln!("  --domain <ID>               DDS Domain ID");
    eprintln!("  --orb-id <ID>               ORB-Identity");
    eprintln!("  --tls-cert <FILE>           SSLIOP server cert");
    eprintln!("  --tls-key <FILE>            SSLIOP server key");
    eprintln!("  --topic <REPO_ID=TOPIC>     Inline mapping override");
    eprintln!("  --dump-iors                 Print stringified IORs and exit");
    eprintln!("  --log-level <LEVEL>         trace|debug|info|warn|error");
    eprintln!("  --metrics <ADDR>            Prometheus listen addr");
    eprintln!();
    eprintln!("  Test hooks. Real options, listed because the program accepts");
    eprintln!("  them and an option nobody can find is one nobody maintains:");
    eprintln!("  --idle-timeout-ms <N>       Stop after N ms");
    eprintln!();
    eprintln!("  --version                   Print version");
    eprintln!("  --help                      Show this help");
}

/// Documented fallback when `metrics.enabled: true` names no `listen:`.
const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9096";

/// Admin-endpoint bind address. Precedence: CLI `--metrics` beats the
/// config file `metrics.listen:` (Spec Â§3 / Â§8.2); `metrics.enabled: false`
/// keeps the endpoint off. An enabled block without `listen:` falls back
/// to the documented default.
fn metrics_addr_for(cfg: &DaemonConfig) -> Option<String> {
    if !cfg.metrics_enabled {
        return None;
    }
    if cfg.metrics_listen.is_empty() {
        return Some(DEFAULT_METRICS_LISTEN.to_string());
    }
    Some(cfg.metrics_listen.clone())
}

#[cfg(test)]
#[allow(clippy::panic)]
mod metrics_default_tests {
    use super::DEFAULT_METRICS_LISTEN;

    /// Each bridge daemon has its own admin port, so that several with
    /// `metrics.enabled: true` and no `metrics.listen` run on one host;
    /// man5 names it as the default.
    #[test]
    fn the_metrics_default_is_the_one_man5_names() {
        let page = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../man/man5/zerodds-corba-bridged.yaml.5"
        ))
        .unwrap_or_else(|e| panic!("man5 page: {e}"));
        let named = format!("Default\n.IR \"{DEFAULT_METRICS_LISTEN}\"\nwhen");
        assert!(
            page.contains(&named),
            "man5 does not name {DEFAULT_METRICS_LISTEN}"
        );
    }
}
