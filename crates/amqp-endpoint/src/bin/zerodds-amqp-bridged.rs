// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `zerodds-amqp-bridged` — DDS↔AMQP-1.0 bridge daemon (client mode).
//
// Spec: `docs/specs/zerodds-amqp-bridge-daemon-1.0.md`. The command line,
// the admin endpoint and the observability hooks; the bridge itself is
// `zerodds_amqp_endpoint::bridge_daemon`.

//! `zerodds-amqp-bridged`: the command line of the DDS↔AMQP 1.0 bridge.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_amqp_endpoint::bridge_daemon::{self, DaemonConfig, DaemonError, ENV, is_key};
use zerodds_amqp_endpoint::daemon_runtime::{
    BridgeMetrics, SERVICE_NAME, install_signal_watcher, otlp_config_from_env,
    serve_admin_endpoints, spawn_otlp_flush_loop,
};
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The base name of the daemon's file: `amqp-bridged.yaml`, `amqp-bridged.d/`.
const CONFIG_STEM: &str = "amqp-bridged";

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
    let mut once = false;
    let mut connect_timeout_ms: u64 = 5000;
    let mut tls = false;

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
            "--broker" => sets.push(format!("amqp.broker_url={}", value()?)),
            "--container-id" => sets.push(format!("amqp.container_id={}", value()?)),
            "--domain" => sets.push(format!("domain={}", value()?)),
            "--sasl-mechanism" => sets.push(format!("amqp.sasl.mechanism={}", value()?)),
            "--user" => sets.push(format!("amqp.sasl.username={}", value()?)),
            "--password" => sets.push(format!("amqp.sasl.password={}", value()?)),
            "--tls-ca" => {
                sets.push(format!("amqp.tls.ca_file={}", value()?));
                tls = true;
            }
            "--tls-cert" => {
                sets.push(format!("amqp.tls.cert_file={}", value()?));
                tls = true;
            }
            "--tls-key" => sets.push(format!("amqp.tls.key_file={}", value()?)),
            "--topic" => sets.push(format!("+topics={}", value()?)),
            "--log-level" => sets.push(format!("log_level={}", value()?)),
            "--metrics" => {
                sets.push(format!("metrics.listen={}", value()?));
                sets.push("metrics.enabled=true".to_string());
            }
            "--version" | "-V" => {
                println!("zerodds-amqp-bridged {VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            "--once" => once = true,
            "--connect-timeout-ms" => {
                let v = value()?;
                connect_timeout_ms = v.parse().map_err(|_| {
                    daemon_error(
                        1,
                        format!("--connect-timeout-ms {v}: expected milliseconds"),
                    )
                })?;
            }
            other => {
                return Err(daemon_error(1, format!("unknown argument: {other}")));
            }
        }
        i += 1;
    }
    if tls {
        sets.push("amqp.tls.enabled=true".to_string());
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
        ENV,
    )
    .map_err(|e| daemon_error(if checking { 2 } else { 1 }, format!("config: {e}")))?;
    let cfg = match DaemonConfig::from_config(configured.source.tree()) {
        Ok(cfg) => cfg,
        Err(e) if configured.checking() => {
            println!("error: {e}\nconfiguration invalid");
            return Err(daemon_error(1, "configuration invalid".to_string()));
        }
        Err(e) => return Err(daemon_error(1, format!("config: {e}"))),
    };
    if let Some(valid) = configured.check_config_valid() {
        return if valid {
            Ok(())
        } else {
            Err(daemon_error(1, "configuration invalid".to_string()))
        };
    }

    if cfg.broker_url.is_empty() {
        return Err(daemon_error(
            1,
            "broker URL required (--broker or config.amqp.broker_url)".to_string(),
        ));
    }

    // The JSON log on stdout (spec §8.1), from the first line on.
    bridge_daemon::set_log_level(&cfg.log_level);
    bridge_daemon::log_event(
        "startup",
        &[
            ("broker", &cfg.broker_url),
            ("container_id", &cfg.container_id),
        ],
        &[
            ("domain", u64::from(cfg.domain.unsigned_abs())),
            ("topics", cfg.topics.len() as u64),
            ("tenants", cfg.tenants.len() as u64),
        ],
    );

    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    // Metrics registry + standard counters (§8.2 Prometheus).
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register_shared(&registry);

    // Signal watcher (§9.2 graceful shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&shutdown), Arc::clone(&reload)) {
        bridge_daemon::log_event(
            "signal_watcher_init_failed",
            &[("err", &e.to_string())],
            &[],
        );
    }

    // Admin endpoint (§5.2 catalog/healthz + §8.2 metrics).
    let admin_handle = if let Some(addr_s) = metrics_addr_for(&cfg) {
        match addr_s.parse::<std::net::SocketAddr>() {
            Ok(sa) => {
                let snap = Arc::new(bridge_daemon::admin_catalog(&cfg));
                let healthy = Arc::new(AtomicBool::new(true));
                let stop_admin = Arc::clone(&shutdown);
                match serve_admin_endpoints(
                    sa,
                    snap,
                    Arc::clone(&registry),
                    Arc::clone(&healthy),
                    stop_admin,
                ) {
                    Ok((h, bound)) => {
                        bridge_daemon::log_event(
                            "admin_endpoint",
                            &[("addr", &bound.to_string())],
                            &[],
                        );
                        Some((h, healthy))
                    }
                    Err(e) => {
                        bridge_daemon::log_event(
                            "admin_endpoint_bind_failed",
                            &[("err", &e.to_string())],
                            &[],
                        );
                        None
                    }
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // OTLP exporter (§8.3): the spans of every TRANSFER round trip, sent
    // every 5 s and once more at the end.
    let exporter = otlp_config_from_env(SERVICE_NAME).map(|c| Arc::new(OtlpExporter::new(c)));
    let otlp_stop = Arc::new(AtomicBool::new(false));
    let otlp_handle = exporter.as_ref().and_then(|exp| {
        spawn_otlp_flush_loop(
            Arc::clone(exp),
            Arc::clone(&otlp_stop),
            Duration::from_secs(5),
        )
        .ok()
    });
    let tracer = bridge_daemon::Tracer::new(exporter);

    let reload_load = move || -> Result<DaemonConfig, String> {
        let configured = zerodds_dcps::cli::Configured::from_command_line(
            reload_command_line.clone(),
            Vec::new(),
            CONFIG_STEM,
            is_key,
            ENV,
        )
        .map_err(|e| e.to_string())?;
        DaemonConfig::from_config(configured.source.tree()).map_err(|e| e.to_string())
    };
    let res = bridge_daemon::run_with_reload(
        &cfg,
        configured.runtime.clone(),
        &shutdown,
        &metrics,
        once,
        Duration::from_millis(connect_timeout_ms),
        Some(bridge_daemon::Reload {
            flag: Arc::clone(&reload),
            load: Box::new(reload_load),
        }),
        tracer,
    );
    // `run_with_reload` has told STOPPING=1 for a stop during the run before
    // it returned; a stop after it (an error, an ended `--once`) is told
    // here before the process ends (a repeat is harmless).
    if shutdown.load(std::sync::atomic::Ordering::SeqCst) {
        zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    }

    // The last spans go out before the process ends.
    otlp_stop.store(true, Ordering::SeqCst);
    if let Some(h) = otlp_handle {
        let _ = h.join();
    }

    // /healthz answers 503 once the bridge stopped.
    if let Some((_h, healthy)) = admin_handle {
        healthy.store(false, Ordering::SeqCst);
    }
    res
}

/// Admin-endpoint bind address. Precedence: CLI `--metrics` beats the
/// config file `metrics.listen:` (Spec Â§3 / Â§8.2); `metrics.enabled: false`
/// keeps the endpoint off. An enabled block without `listen:` falls back
/// to the documented default.
/// Documented fallback when `metrics.enabled: true` names no `listen:`.
const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9094";

fn metrics_addr_for(cfg: &DaemonConfig) -> Option<String> {
    if !cfg.metrics_enabled {
        return None;
    }
    if cfg.metrics_listen.is_empty() {
        return Some(DEFAULT_METRICS_LISTEN.to_string());
    }
    Some(cfg.metrics_listen.clone())
}

fn print_usage() {
    eprintln!("zerodds-amqp-bridged {VERSION}");
    eprintln!();
    eprintln!("USAGE: zerodds-amqp-bridged [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --config <FILE>          base file (default: amqp-bridged.yaml in the");
    eprintln!("                           system configuration directory) with FILE.d/");
    eprintln!("  --set <PATH=VALUE>       one setting above every file, repeatable");
    eprintln!("  --check-config           validate, print every value with its origin, exit");
    eprintln!("  --broker <URL>           AMQP broker URL (amqp://, amqps://)");
    eprintln!("  --container-id <ID>      AMQP container-id");
    eprintln!("  --domain <ID>            DDS Domain ID");
    eprintln!("  --sasl-mechanism <NAME>  PLAIN|SCRAM-SHA-256|SCRAM-SHA-512|EXTERNAL|ANONYMOUS|");
    eprintln!("                           XOAUTH2");
    eprintln!("  --user <USER>            SASL username");
    eprintln!("  --password <PASS>        SASL password");
    eprintln!("  --tls-ca <FILE>          TLS CA certificate");
    eprintln!("  --tls-cert <FILE>        TLS client cert");
    eprintln!("  --tls-key <FILE>         TLS client key");
    eprintln!("  --topic <DDS=ADDR>       Inline topic mapping override");
    eprintln!("  --log-level <LEVEL>      trace|debug|info|warn|error");
    eprintln!("  --metrics <ADDR>         Prometheus listen addr");
    eprintln!();
    eprintln!("  Each option above is `--set` of its file key (`--broker X` is");
    eprintln!("  `--set amqp.broker_url=X`); precedence: defaults < environment");
    eprintln!("  ($AMQP_PASSWORD for amqp.sasl.password, $AMQP_CBS_SECRET for");
    eprintln!("  amqp.cbs.secret) < files < flags.");
    eprintln!();
    eprintln!("  Test hooks. Real options, listed because the program accepts");
    eprintln!("  them and an option nobody can find is one nobody maintains:");
    eprintln!("  --once                   Serve one connection, then exit");
    eprintln!("  --connect-timeout-ms <N> Give up connecting after N ms");
    eprintln!();
    eprintln!("  --version                Print version");
    eprintln!("  --help                   Show this help");
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
            "/../../man/man5/zerodds-amqp-bridged.yaml.5"
        ))
        .unwrap_or_else(|e| panic!("man5 page: {e}"));
        let named = format!("Default\n.IR \"{DEFAULT_METRICS_LISTEN}\"\nwhen");
        assert!(
            page.contains(&named),
            "man5 does not name {DEFAULT_METRICS_LISTEN}"
        );
    }
}
