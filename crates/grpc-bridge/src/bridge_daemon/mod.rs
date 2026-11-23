// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon `zerodds-grpc-bridged`
//! (`docs/specs/zerodds-grpc-bridge-1.0.md`): an HTTP/2 gRPC server on one
//! side (TLS with ALPN `h2`, §4.1, §7.1), DDS participants on the other
//! (§4.2, §1 L6). The auth modes of §7.2 are checked per call on the
//! `authorization` metadata or the client certificate, the topic ACL of
//! §7.3 per call; SIGHUP reloads all three (§9.2).
//!
//! * `config`: the keys of §3 and their reader.
//! * `connection`: the listener and the HTTP/2 connection loop.
//! * `calls`: the calls of a connection.
//! * `dispatch`: the route of a call, the `Sample` conversions.
//! * `dds_pump`: the DDS side.
//! * `auth`: TLS, the auth modes, the ACL, the reload.
//! * `catalog`: the catalog, reflection services, the IDL endpoint.
//! * `metrics`: the Prometheus families of §8.2.
//! * `log`: the JSON log on stdout of §8.1.
//! * `trace`: the OTLP spans of §8.3.

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
    clippy::useless_conversion
)]

use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::bridge_security::{
    AclEntry, AclOp, AuthSubject, SecurityConfig, SecurityCtx, authenticate_grpc, authorize,
    build_ctx, extract_mtls_subject,
};
use crate::daemon_runtime::{
    GrpcTlsOptions, TlsMinVersion, build_grpc_tls_config_opts, select_tls_suites,
};

use crate::daemon_runtime::{
    BridgeMetrics, CatalogSnapshot, CatalogTopic, SERVICE_NAME, install_signal_watcher,
    otlp_config_from_env, serve_admin_endpoints_at,
};
use std::collections::HashMap;

use crate::compression::{self, Encoding};
use crate::messages::{
    CatalogResp, PublishAck, Sample, SubscribeReq, TopicEntry, decode_catalog_req,
};
use crate::reflection::{self, Reflection};
use crate::server::{CallEvent, GrpcCall, GrpcServer, SendError, ServerLimits};
use crate::service_gen::{self, ServiceCatalog, TopicService};
use crate::status::Status;
use crate::{TimeoutUnit, decode_timeout};
use zerodds_http2::{ErrorCode, FrameType, check_preface};
use zerodds_monitor::Registry;
use zerodds_observability_otlp::OtlpExporter;

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The name of the Windows service (Spec §11, zerodds-deployment-1.0 §4.3),
/// the one `packaging/windows/services/Install-Services.ps1` and the MSI
/// register; the bin runs under it through
/// `zerodds_dcps::cli::run_as_service`.
pub const WINDOWS_SERVICE: &str = "ZeroDDSGrpcBridge";

mod auth;
mod calls;
mod catalog;
mod config;
mod connection;
mod dds_pump;
mod dispatch;
mod log;
mod metrics;
#[cfg(test)]
mod test_support;
mod trace;

pub(crate) use auth::*;
pub(crate) use calls::*;
pub(crate) use catalog::*;
pub(crate) use config::*;
pub(crate) use connection::*;
pub(crate) use dds_pump::*;
pub(crate) use dispatch::*;
pub(crate) use log::*;
pub(crate) use metrics::*;
pub(crate) use trace::*;

/// Why the daemon stopped, with the exit code of spec §2 (1 configuration,
/// 2 bind, 3 DDS, 4 TLS).
#[derive(Debug)]
pub struct DaemonError {
    code: u8,
    msg: String,
}

impl DaemonError {
    pub(crate) fn new(code: u8, msg: impl Into<String>) -> Self {
        Self {
            code,
            msg: msg.into(),
        }
    }

    /// The exit code of spec §2.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        self.code
    }
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for DaemonError {}

impl From<std::io::Error> for DaemonError {
    fn from(e: std::io::Error) -> Self {
        Self::new(2, format!("io: {e}"))
    }
}

/// The daemon: the command line of spec §2, the configuration of §3, the
/// startup of §9.1, the listener until SIGTERM or SIGINT, the shutdown of
/// §9.2.
///
/// # Errors
/// What stopped the daemon, with its exit code ([`DaemonError::exit_code`]).
pub fn run() -> Result<(), DaemonError> {
    let args: Vec<String> = env::args().skip(1).collect();
    let (mut command_line, rest) = zerodds_config::CommandLine::extract(args)
        .map_err(|e| DaemonError::new(1, e.to_string()))?;
    let mut sets: Vec<String> = Vec::new();
    let mut tls_cert: Option<String> = None;
    let mut tls_key: Option<String> = None;
    let mut once_shot: bool = false;
    let mut idle_timeout_ms: u64 = 0;

    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i].as_str();
        let mut value = || -> Result<String, DaemonError> {
            i += 1;
            rest.get(i)
                .cloned()
                .ok_or_else(|| DaemonError::new(1, format!("{flag} needs a value")))
        };
        // Spec §2: each option is `--set` of its file key, after any
        // explicit `--set`, so it wins over the files and names itself as
        // the origin — `--domain 0` over a file's `domain: 7` included.
        match flag {
            "--bind" => sets.push(format!("grpc.bind={}", value()?)),
            "--domain" => sets.push(format!("domain={}", value()?)),
            "--tls-cert" => tls_cert = Some(value()?),
            "--tls-key" => tls_key = Some(value()?),
            "--tls-client-ca" => sets.push(format!("grpc.tls.client_ca_file={}", value()?)),
            "--reflection" => sets.push("grpc.reflection.enabled=true".to_string()),
            "--topic" => sets.push(format!("+topics={}", value()?)),
            "--log-level" => sets.push(format!("log_level={}", value()?)),
            "--metrics" => {
                sets.push(format!("metrics.listen={}", value()?));
                sets.push("metrics.enabled=true".to_string());
            }
            "--version" | "-V" => {
                println!("zerodds-grpc-bridged {VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            // Test-only: serve exactly one connection then exit.
            "--once" => once_shot = true,
            "--idle-timeout-ms" => {
                let v = value()?;
                idle_timeout_ms = v.parse().map_err(|_| {
                    DaemonError::new(1, format!("--idle-timeout-ms {v}: expected milliseconds"))
                })?;
            }
            other => {
                return Err(DaemonError::new(1, format!("unknown argument: {other}")));
            }
        }
        i += 1;
    }
    match (tls_cert, tls_key) {
        (Some(c), Some(k)) => {
            sets.push(format!("grpc.tls.cert_file={c}"));
            sets.push(format!("grpc.tls.key_file={k}"));
            sets.push("grpc.tls.enabled=true".to_string());
        }
        (None, None) => {}
        (Some(_), None) => return Err(DaemonError::new(1, "--tls-cert needs --tls-key")),
        (None, Some(_)) => return Err(DaemonError::new(1, "--tls-key needs --tls-cert")),
    }
    for assignment in &sets {
        command_line
            .push_set(assignment)
            .map_err(|e| DaemonError::new(1, e.to_string()))?;
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
        &[],
    )
    .map_err(|e| DaemonError::new(if checking { 2 } else { 1 }, format!("config: {e}")))?;
    let cfg = match DaemonConfig::from_config(configured.source.tree()) {
        Ok(cfg) => cfg,
        Err(e) if configured.checking() => {
            println!("error: {e}\nconfiguration invalid");
            return Err(DaemonError::new(1, "configuration invalid"));
        }
        Err(e) => return Err(DaemonError::new(1, format!("config: {e}"))),
    };
    // §2: what the start refuses as a configuration error (exit 1) is
    // refused under `--check-config` too; the TLS material (exit 4) is
    // loaded at the start only.
    if configured.checking()
        && let Err((_, msg)) = security_settings(&cfg).and_then(|_| transport_settings(&cfg))
    {
        println!("error: {msg}\nconfiguration invalid");
        return Err(DaemonError::new(1, "configuration invalid"));
    }
    // §8.3: an `OTEL_EXPORTER_OTLP_ENDPOINT` the exporter cannot serve
    // (`https`, a path, a port that is no number) stops the start with
    // exit 1, and `--check-config` refuses it.
    let otlp = match otlp_config_from_env(SERVICE_NAME) {
        Ok(otlp) => otlp,
        Err(msg) if configured.checking() => {
            println!("error: {msg}\nconfiguration invalid");
            return Err(DaemonError::new(1, "configuration invalid"));
        }
        Err(msg) => return Err(DaemonError::new(1, msg)),
    };
    if let Some(valid) = configured.check_config_valid() {
        return if valid {
            Ok(())
        } else {
            Err(DaemonError::new(1, "configuration invalid"))
        };
    }
    let runtime_config = configured.runtime;
    // Spec §2 `--log-level`, §3 `log_level`: the threshold of the lines.
    set_log_level(&cfg.log_level);
    // §7: a setting that asks for TLS or authentication and cannot be
    // served stops the start. It used to be read and then ignored, so a
    // daemon configured for `auth.mode: jwt` answered every call unchecked.
    transport_settings(&cfg).map_err(|(code, msg)| DaemonError::new(code, msg))?;
    let live = Arc::new(zerodds_bridge_security::LiveSecurity::new(
        security_from(&cfg).map_err(|(code, msg)| DaemonError::new(code, msg))?,
    ));

    event(Level::Info, "startup")
        .text("bind", &cfg.bind)
        .int("domain", i64::from(cfg.domain))
        .num("topics", cfg.topics.len() as u64)
        .flag("reflection", cfg.reflection_enabled)
        .emit();

    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    // Metrics registry + standard counters (§8.2 Prometheus).
    let registry = Arc::new(Registry::new());
    let bridge_metrics = DaemonMetrics {
        base: BridgeMetrics::register(&registry),
        spec: SpecMetrics::register(&registry, &cfg),
    };

    // Signal watcher (§9.2 graceful shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&shutdown), Arc::clone(&reload)) {
        event(Level::Error, "signal_watcher_init_failed")
            .text("err", &e.to_string())
            .emit();
    }
    // §9.2 SIGHUP: TLS material, auth and ACL from the files as they are now.
    {
        let (flag, stop, live, started) = (
            Arc::clone(&reload),
            Arc::clone(&shutdown),
            Arc::clone(&live),
            cfg.clone(),
        );
        std::thread::spawn(move || {
            let load = || -> Result<DaemonConfig, String> {
                let configured = zerodds_dcps::cli::Configured::from_command_line(
                    reload_command_line.clone(),
                    Vec::new(),
                    CONFIG_STEM,
                    is_key,
                    &[],
                )
                .map_err(|e| e.to_string())?;
                DaemonConfig::from_config(configured.source.tree()).map_err(|e| e.to_string())
            };
            while !stop.load(Ordering::SeqCst) {
                if flag.swap(false, Ordering::SeqCst) {
                    reload_security(&live, &started, load());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }

    // Admin endpoint (§5.2 catalog/healthz + §8.2 metrics).
    let healthy = Arc::new(AtomicBool::new(true));
    let _admin_h = if let Some(sa) = metrics_addr_for(&cfg) {
        // §1 L6: a tenant's topics as `<tenant>/<name>`.
        let topics: Vec<CatalogTopic> = cfg
            .topics
            .iter()
            .map(|t| (None, t))
            .chain(cfg.tenants.iter().flat_map(|tenant| {
                tenant
                    .topics
                    .iter()
                    .map(|t| (Some(tenant.name.as_str()), t))
            }))
            .map(|(tenant, t)| CatalogTopic {
                dds_name: tenant_label(tenant, &t.dds_name),
                grpc_service: tenant_label(tenant, &t.full_service()),
                direction: t.direction.clone(),
            })
            .collect();
        let snap = Arc::new(CatalogSnapshot::new(topics));
        match serve_admin_endpoints_at(
            sa,
            cfg.metrics_path.clone(),
            snap,
            Arc::clone(&registry),
            Arc::clone(&healthy),
            Arc::clone(&shutdown),
        ) {
            Ok((h, bound)) => {
                event(Level::Info, "admin_endpoint")
                    .text("addr", &bound.to_string())
                    .text("metrics_path", &cfg.metrics_path)
                    .emit();
                Some(h)
            }
            Err(e) => {
                event(Level::Error, "admin_endpoint_failed")
                    .text("err", &e.to_string())
                    .emit();
                None
            }
        }
    } else {
        None
    };

    // §8.3: with `OTEL_EXPORTER_OTLP_ENDPOINT` the spans of the calls go to
    // that collector, every 5 s and at the end.
    let tracer = Tracer::new(otlp.map(|c| Arc::new(OtlpExporter::new(c))));
    let otlp_thread = match tracer.spawn_export_loop(Arc::clone(&shutdown), EXPORT_INTERVAL) {
        Ok(handle) => handle,
        Err(e) => {
            // The spans then go out at the end of the shutdown only.
            event(Level::Error, "otlp_export_thread_failed")
                .text("err", &e.to_string())
                .emit();
            None
        }
    };

    // L2 — DDS side (bridge spec §4.2): a DCPS participant with a writer
    // per topic bridged into DDS and a reader per topic bridged out of it
    // (`direction`), so `Publish` writes to DDS and `Subscribe` drains it;
    // §1 L6: one more for each tenant, on its domain with its topics.
    let mut participants: Vec<Participant> = Vec::with_capacity(1 + cfg.tenants.len());
    let default_and_tenants = std::iter::once((None, cfg.domain, Vec::new(), cfg.topics.clone()))
        .chain(cfg.tenants.iter().map(|t| {
            (
                Some(t.name.clone()),
                t.domain,
                t.subjects.clone(),
                t.topics.clone(),
            )
        }));
    for (tenant, domain, subjects, topics) in default_and_tenants {
        match BridgeDds::start(domain, &topics, runtime_config.clone()) {
            Ok(d) => {
                event(Level::Info, "dds_started")
                    .int("domain", i64::from(domain))
                    .num("topics", topics.len() as u64)
                    .text_opt("tenant", tenant.as_deref())
                    .emit();
                participants.push(Participant {
                    tenant,
                    subjects,
                    topics,
                    dds: Some(d),
                });
            }
            Err(e) => {
                event(Level::Error, "dds_start_failed")
                    .text("err", &e.to_string())
                    .emit();
                return Err(DaemonError::new(3, format!("dds: {e}")));
            }
        }
    }

    // Spec §6 TRANSIENT_LOCAL: the retained samples follow their topics
    // while the daemon runs.
    let res = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !shutdown.load(Ordering::SeqCst) {
                for p in &participants {
                    if let Some(dds) = p.dds() {
                        dds.refresh_retained();
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let served = serve(
            &cfg,
            shutdown.clone(),
            once_shot,
            idle_timeout_ms,
            &bridge_metrics,
            &participants,
            &live,
            &tracer,
        );
        // The listener is done (or never bound): the refresh ends too.
        shutdown.store(true, Ordering::SeqCst);
        served
    });
    healthy.store(false, Ordering::SeqCst);
    let stopped_at = res?;
    // §9.2: the samples the drained calls wrote reach their reliable
    // readers within what is left of `grpc.drain_secs`, then the DDS
    // entities are deleted (SEDP dispose) and the participants leave
    // (SPDP), then exit 0.
    let until = stopped_at + Duration::from_secs(cfg.drain_secs);
    for p in &participants {
        if let Some(dds) = p.dds() {
            let acknowledged = dds.await_writes(until);
            event(
                if acknowledged {
                    Level::Info
                } else {
                    Level::Warn
                },
                "dds_writes_drained",
            )
            .flag("acknowledged", acknowledged)
            .text_opt("tenant", p.tenant.as_deref())
            .emit();
        }
    }
    for p in participants {
        if let Some(dds) = p.dds {
            dds.close();
        }
    }
    // §8.3: the spans of the last calls reach the collector before the
    // exit: the export thread has ended (the stop flag is set since the
    // listener is done) and has no export in flight, then the last export;
    // both within EXPORT_STOP_BUDGET, so a collector that does not answer
    // does not hold the exit past the service manager's stop timeout.
    tracer.finish_by(otlp_thread, Instant::now() + EXPORT_STOP_BUDGET);
    Ok(())
}

/// A DDS participant of the daemon and the topics it serves: the one of
/// `domain`, or a tenant's (Spec §1 L6).
pub(crate) struct Participant {
    /// The tenant's name; `None` for the participant of `domain`.
    pub(crate) tenant: Option<String>,
    /// The client certificates bound to the tenant (subject DN or
    /// `mtls:` fingerprint).
    pub(crate) subjects: Vec<String>,
    pub(crate) topics: Vec<TopicMapping>,
    pub(crate) dds: Option<BridgeDds>,
}

impl Participant {
    /// A name as the tenant sees it in labels and ACL entries:
    /// `<tenant>/<name>`, or the name itself for the participant of
    /// `domain` (Spec §1 L6, §8.2).
    pub(crate) fn label(&self, name: &str) -> String {
        tenant_label(self.tenant.as_deref(), name)
    }

    pub(crate) fn dds(&self) -> Option<&BridgeDds> {
        self.dds.as_ref()
    }
}

/// `<tenant>/<name>`, or `name` without a tenant.
pub(crate) fn tenant_label(tenant: Option<&str>, name: &str) -> String {
    match tenant {
        Some(t) => format!("{t}/{name}"),
        None => name.to_string(),
    }
}

/// Spec §1 L6: the participant a connection is bound to for its life —
/// the tenant whose `subjects` name the client certificate (its subject
/// DN or its `mtls:` fingerprint), else the participant of `domain`
/// (the first).
pub(crate) fn participant_of<'a>(
    participants: &'a [Participant],
    subject: Option<&AuthSubject>,
) -> &'a Participant {
    subject
        .and_then(|subject| {
            participants.iter().skip(1).find(|p| {
                p.subjects
                    .iter()
                    .any(|s| *s == subject.name || subject.fingerprint.as_deref() == Some(s))
            })
        })
        .unwrap_or(&participants[0])
}

pub(crate) fn print_usage() {
    eprintln!("zerodds-grpc-bridged {VERSION}");
    eprintln!();
    eprintln!("USAGE: zerodds-grpc-bridged [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --config <FILE>          base file (default: grpc-bridged.yaml in the");
    eprintln!("                           system configuration directory) with FILE.d/");
    eprintln!("  --set <PATH=VALUE>       one setting above every file, repeatable");
    eprintln!("  --check-config           validate, print every value with its origin, exit");
    eprintln!("  --bind <ADDR>            HTTP/2 bind addr (default 0.0.0.0:50051)");
    eprintln!("  --domain <ID>            DDS Domain ID (default 0)");
    eprintln!("  --tls-cert <FILE>        TLS cert PEM");
    eprintln!("  --tls-key <FILE>         TLS key PEM");
    eprintln!("  --tls-client-ca <FILE>   Client CA PEM for mTLS");
    eprintln!("  --reflection             Enable gRPC reflection service");
    eprintln!("  --topic <DDS:SVC>        Inline topic mapping override");
    eprintln!("  --log-level <LEVEL>      trace|debug|info|warn|error");
    eprintln!("  --metrics <ADDR>         Prometheus listen addr");
    eprintln!();
    eprintln!("  Test hooks. Real options, listed because the program accepts");
    eprintln!("  them and an option nobody can find is one nobody maintains:");
    eprintln!("  --once                   Serve one request, then exit");
    eprintln!("  --idle-timeout-ms <N>    Exit after N ms without a request");
    eprintln!();
    eprintln!("  --version                Print version");
    eprintln!("  --help                   Show this help");
}

/// Spec §11: the service files of the packages start the daemon as §2 and
/// §9 describe it.
#[cfg(test)]
mod packaging_tests {
    use super::{DEFAULT_DRAIN_SECS, WINDOWS_SERVICE};

    fn repo_file(rel: &str) -> String {
        let path = format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    /// The number after `key` up to the first character that is no digit.
    fn number_after(text: &str, key: &str) -> u64 {
        let at = text.find(key).unwrap_or_else(|| panic!("{key} is missing")) + key.len();
        text[at..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or_else(|e| panic!("{key}: {e}"))
    }

    /// Spec §11: the Windows service the bin runs as is the one the
    /// PowerShell installer and the MSI register for
    /// `zerodds-grpc-bridged.exe`, and the one the spec names.
    #[test]
    fn grpcbridge_11_the_windows_service_is_the_one_the_installers_register() {
        let ps1 = repo_file("packaging/windows/services/Install-Services.ps1");
        let line = ps1
            .lines()
            .find(|l| l.contains("'zerodds-grpc-bridged.exe'"))
            .expect("Install-Services.ps1 registers the gRPC daemon");
        assert!(
            line.contains(&format!("Name = '{WINDOWS_SERVICE}'")),
            "{line}"
        );
        let wxs = repo_file("packaging/windows/msi/zerodds.wxs");
        let component = &wxs[wxs
            .find("zerodds-grpc-bridged.exe")
            .expect("the MSI ships the gRPC daemon")..];
        let component = &component[..component.find("</Component>").unwrap()];
        assert!(
            component.contains(&format!(
                "<ServiceInstall Id=\"GrpcBridgeService\" Name=\"{WINDOWS_SERVICE}\""
            )),
            "{component}"
        );
        assert!(component.contains(&format!(
            "<ServiceControl Id=\"GrpcBridgeServiceCtl\" Name=\"{WINDOWS_SERVICE}\" Stop=\"both\""
        )));
        let spec = repo_file("docs/specs/zerodds-grpc-bridge-1.0.md");
        assert!(spec.contains(&format!("- Windows service: `{WINDOWS_SERVICE}`")));
    }

    /// Spec §9.2, §11: systemd, launchd and Docker stop the daemon with
    /// SIGTERM and wait longer than the default drain of `grpc.drain_secs`
    /// before they kill it; systemd waits for `READY=1` (`Type=notify`)
    /// and sends SIGHUP for a reload.
    #[test]
    fn grpcbridge_11_the_service_managers_wait_longer_than_the_drain() {
        let unit = repo_file("packaging/linux/systemd/zerodds-grpc-bridged.service");
        assert!(unit.contains("\nType=notify\n"));
        assert!(unit.contains("\nKillSignal=SIGTERM\n"));
        assert!(unit.contains("\nExecReload=/bin/kill -HUP $MAINPID\n"));
        assert!(number_after(&unit, "\nTimeoutStopSec=") > DEFAULT_DRAIN_SECS);
        let plist = repo_file("packaging/macos/launchd/org.zerodds.grpc-bridged.plist");
        assert!(number_after(&plist, "<key>ExitTimeOut</key><integer>") > DEFAULT_DRAIN_SECS);
        let dockerfile = repo_file("packaging/docker/grpc-bridged/Dockerfile");
        assert!(dockerfile.contains("\nSTOPSIGNAL SIGTERM\n"));
        assert!(number_after(&dockerfile, "`docker stop -t ") > DEFAULT_DRAIN_SECS);
        assert!(number_after(&dockerfile, "stop_grace_period: ") > DEFAULT_DRAIN_SECS);
    }

    /// Spec §2, §11: each service file starts the binary of its platform
    /// with the configuration file of its platform, and the image carries
    /// the example configuration at that path.
    #[test]
    fn grpcbridge_11_the_service_files_start_the_binary_with_its_configuration() {
        let unit = repo_file("packaging/linux/systemd/zerodds-grpc-bridged.service");
        assert!(unit.contains(
            "\nExecStart=/usr/bin/zerodds-grpc-bridged --config /etc/zerodds/grpc-bridged.yaml\n"
        ));
        let plist = repo_file("packaging/macos/launchd/org.zerodds.grpc-bridged.plist");
        let args: Vec<&str> = plist
            .split("<string>")
            .skip(1)
            .map(|s| &s[..s.find("</string>").unwrap()])
            .collect();
        assert_eq!(
            args[1..4],
            [
                "/usr/local/bin/zerodds-grpc-bridged",
                "--config",
                "/usr/local/etc/zerodds/grpc-bridged.yaml"
            ]
        );
        let dockerfile = repo_file("packaging/docker/grpc-bridged/Dockerfile");
        assert!(dockerfile.contains(
            "\nCOPY packaging/linux/configs/grpc-bridged.yaml.example /etc/zerodds/grpc-bridged.yaml\n"
        ));
        assert!(dockerfile.contains(
            "\nENTRYPOINT [\"/usr/bin/tini\", \"--\", \"/usr/local/bin/zerodds-grpc-bridged\"]\n"
        ));
        assert!(dockerfile.contains("\nCMD [\"--config\", \"/etc/zerodds/grpc-bridged.yaml\"]\n"));
        let ps1 = repo_file("packaging/windows/services/Install-Services.ps1");
        assert!(ps1.contains("$binPath = \"`\"$exe`\" --config `\"$cfg`\"\""));
    }
}
