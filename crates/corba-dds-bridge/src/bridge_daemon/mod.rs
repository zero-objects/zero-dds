// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon `zerodds-corba-bridged`
//! (`docs/specs/zerodds-corba-bridge-1.0.md`): an IIOP server (and SSLIOP)
//! on one side, a DDS participant on the other.
//!
//! * `corba_to_dds`: a GIOP request to a mapping's object and operation
//!   becomes a sample on the request topic; the sample on the reply topic
//!   with the same `request_id` becomes the GIOP reply (§4.3). No reply
//!   within `timeout_ms` is `CORBA::TIMEOUT`.
//! * `dds_to_corba`: a sample on the notify topic becomes a oneway GIOP
//!   request to every target (§4.4).
//!
//! The DDS samples carry the GIOP bytes unchanged, in the byte order the
//! GIOP message used (XCDR1, whose alignment is the one GIOP 1.2 gives a
//! body, 8 from the body start):
//!
//! ```idl
//! @final struct <Operation>Request {
//!     unsigned long long request_id;   // the bridge's, unique per daemon
//!     // the operation's in and inout arguments, as GIOP encodes them
//! };
//! @final struct <Operation>Reply {
//!     unsigned long long request_id;   // of the request it answers
//!     long reply_status;               // 0 NO_EXCEPTION, 1 USER_EXCEPTION,
//!                                      // 2 SYSTEM_EXCEPTION (GIOP ReplyStatusType)
//!     unsigned long reserved;          // 0; keeps the rest 8-aligned
//!     // the return value and out/inout arguments, or the exception
//! };
//! ```
//!
//! Because the alignment only matches for GIOP 1.2, a mapped operation
//! reached over GIOP 1.0 or 1.1 is answered `CORBA::NO_IMPLEMENT`.

pub(crate) mod config;
mod correlator;
mod framing;
mod ior;
mod log;
mod metrics;
mod notify;
mod object_key;
mod pump;
mod server;
mod trace;
mod typegen;

pub(crate) use config::slug;
pub use config::{
    AclRule, DaemonConfig, Direction, ENV, KEYS, Mapping, Target, TopicRef, is_key, resolve_target,
};
pub use ior::{build_ior, ior_lines};
pub use object_key::object_key;
pub use server::Reload;
pub use trace::Tracer;
pub use typegen::Raised;

#[allow(unused_imports)]
use prelude::*;

mod prelude {
    pub(crate) use super::config::*;
    pub(crate) use super::correlator::*;
    pub(crate) use super::framing::*;
    pub(crate) use super::ior::*;
    pub(crate) use super::log::*;
    pub(crate) use super::metrics::*;
    pub(crate) use super::notify::*;
    pub(crate) use super::object_key::*;
    pub(crate) use super::pump::*;
    pub(crate) use super::server::*;
    pub(crate) use super::trace::*;

    pub(crate) use crate::daemon_runtime::BridgeMetrics;
    pub(crate) use sha2::{Digest, Sha256};
    pub(crate) use std::collections::BTreeMap;
    pub(crate) use std::io::{self, Read, Write};
    pub(crate) use std::net::{TcpListener, TcpStream};
    pub(crate) use std::string::{String, ToString};
    pub(crate) use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    pub(crate) use std::sync::{Arc, Condvar, Mutex, mpsc};
    pub(crate) use std::time::{Duration, Instant};
    pub(crate) use std::vec::Vec;
    pub(crate) use zerodds_config::ConfigError;
    pub(crate) use zerodds_config::read::{Section, first_origin};
    pub(crate) use zerodds_corba_giop::object_ops::{self, ObjectOpReply};
    pub(crate) use zerodds_corba_giop::{
        LocateReply, LocateRequest, LocateStatusType, Message, ReplyStatusType, Request,
        ResponseFlags, ServiceContextList, TargetAddress, Version, decode_message_ctx,
        encode_message,
    };
    pub(crate) use zerodds_corba_iiop::{Connection, IiopProfileBody, IiopVersion};
    pub(crate) use zerodds_corba_ior::{
        Ior, ProfileId, TaggedProfile, from_stringified, parse_corbaloc, to_stringified,
    };
    pub(crate) use zerodds_dcps::runtime::{
        DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
    };
    pub(crate) use zerodds_rtps::wire_types::EntityId;
    pub(crate) use zerodds_wire::Endianness;
}

/// Why the daemon stopped.
#[derive(Debug)]
pub struct DaemonError {
    /// Exit code (§2).
    pub code: u8,
    /// Message.
    pub msg: String,
}

impl core::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for DaemonError {}

pub(crate) fn err(code: u8, msg: impl Into<String>) -> DaemonError {
    DaemonError {
        code,
        msg: msg.into(),
    }
}

/// Runs the daemon until `shutdown` is set.
///
/// # Errors
/// [`DaemonError`] with the exit code of §2: 1 config, 2 bind, 3 DDS, 4 TLS,
/// 5 NameService.
pub fn run(
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    shutdown: &Arc<AtomicBool>,
    metrics: &BridgeMetrics,
) -> Result<(), DaemonError> {
    run_with_reload(
        cfg,
        runtime_config,
        shutdown,
        metrics,
        None,
        Tracer::default(),
    )
}

/// The DDS endpoints of a mapping's topic (§6): reliability, durability,
/// history (for the replay of `TRANSIENT_LOCAL`), lifespan, deadline,
/// liveliness lease and partition.
fn endpoint_qos(
    m: &Mapping,
) -> (
    zerodds_qos::DeadlineQosPolicy,
    zerodds_qos::LivelinessQosPolicy,
    zerodds_qos::LifespanQosPolicy,
    zerodds_qos::DurabilityServiceQosPolicy,
) {
    let duration =
        |ms: u64| zerodds_qos::Duration::from_millis(i32::try_from(ms).unwrap_or(i32::MAX));
    let deadline = m
        .deadline_ms
        .map_or_else(zerodds_qos::DeadlineQosPolicy::default, |ms| {
            zerodds_qos::DeadlineQosPolicy {
                period: duration(ms),
            }
        });
    let liveliness =
        m.liveliness_lease_ms
            .map_or_else(zerodds_qos::LivelinessQosPolicy::default, |ms| {
                zerodds_qos::LivelinessQosPolicy {
                    kind: zerodds_qos::LivelinessKind::Automatic,
                    lease_duration: duration(ms),
                }
            });
    let lifespan = m
        .lifespan_ms
        .map_or_else(zerodds_qos::LifespanQosPolicy::default, |ms| {
            zerodds_qos::LifespanQosPolicy {
                duration: duration(ms),
            }
        });
    let mut service = zerodds_qos::DurabilityServiceQosPolicy::default();
    match m.history_depth {
        Some(depth) => {
            service.history_kind = zerodds_qos::HistoryKind::KeepLast;
            service.history_depth = i32::try_from(depth).unwrap_or(i32::MAX);
        }
        None => service.history_kind = zerodds_qos::HistoryKind::KeepAll,
    }
    (deadline, liveliness, lifespan, service)
}

/// A participant with its mappings' endpoints and its dispatch state, the
/// receivers of its reply and notify topics by mapping.
type Built = (
    Arc<Dispatch>,
    Vec<(usize, mpsc::Receiver<UserSample>)>,
    Vec<(usize, mpsc::Receiver<UserSample>)>,
);

/// The participant of `cfg` (the daemon's, or a tenant's from
/// [`DaemonConfig::of_tenant`]) and its endpoints.
pub(crate) fn build_dispatch(
    cfg: &DaemonConfig,
    tenant: Option<&str>,
    runtime_config: RuntimeConfig,
    metrics: &BridgeMetrics,
    tracer: &Tracer,
    naming_servant: Option<Arc<zerodds_corba_cosnaming::servant::NamingServant>>,
) -> Result<Built, DaemonError> {
    let mut models: Vec<Option<Arc<zerodds_ast::Ast>>> = vec![None; cfg.mappings.len()];
    for unit in typegen::units(&cfg.mappings).map_err(|e| err(1, e))? {
        for index in &unit.mappings {
            if let Some(slot) = models.get_mut(*index) {
                *slot = Some(unit.generated.ast.clone());
            }
        }
    }
    let name = tenant.map_or_else(
        || format!("{}@{}", cfg.orb_id, cfg.iiop_bind),
        |t| format!("{}@{}/{t}", cfg.orb_id, cfg.iiop_bind),
    );
    let prefix = zerodds_dcps::participant::bridge_guid_prefix(&name);
    let runtime = DcpsRuntime::start(cfg.domain, prefix, runtime_config)
        .map_err(|e| err(3, format!("DDS participant: {e:?}")))?;
    let mut by_key = BTreeMap::new();
    let mut reply_rx = Vec::new();
    let mut notify_rx = Vec::new();
    for (i, m) in cfg.mappings.iter().enumerate() {
        let (deadline, liveliness, lifespan, durability_service) = endpoint_qos(m);
        let writer_cfg = |t: &TopicRef| UserWriterConfig {
            topic_name: t.dds_name.clone(),
            type_name: t.dds_type.clone(),
            reliable: m.reliable,
            durability: m.durability,
            durability_service,
            deadline,
            liveliness,
            lifespan,
            partition: m.partition.clone(),
            ..Default::default()
        };
        let reader_cfg = |t: &TopicRef| UserReaderConfig {
            topic_name: t.dds_name.clone(),
            type_name: t.dds_type.clone(),
            reliable: m.reliable,
            durability: m.durability,
            deadline,
            liveliness,
            partition: m.partition.clone(),
            ..Default::default()
        };
        if m.receives_giop() {
            let w = runtime
                .register_user_writer_kind(writer_cfg(&m.request_topic), false)
                .map_err(|e| err(3, format!("DDS writer {}: {e:?}", m.request_topic.dds_name)))?;
            let (_r, rx) = runtime
                .register_user_reader_kind(reader_cfg(&m.reply_topic), false)
                .map_err(|e| err(3, format!("DDS reader {}: {e:?}", m.reply_topic.dds_name)))?;
            by_key.insert(object_key(m), (i, w));
            reply_rx.push((i, rx));
        }
        if m.sends_giop() {
            let (_r, rx) = runtime
                .register_user_reader_kind(reader_cfg(&m.notify_topic), false)
                .map_err(|e| err(3, format!("DDS reader {}: {e:?}", m.notify_topic.dds_name)))?;
            notify_rx.push((i, rx));
        }
    }
    let csiv2_store = if cfg.csiv2_enabled {
        let text = std::fs::read_to_string(&cfg.csiv2_password_file)
            .map_err(|e| err(1, format!("corba.csiv2.gssup.password_file: {e}")))?;
        let store = zerodds_corba_csiv2::GssupUserStore::parse(&text)
            .map_err(|e| err(1, format!("corba.csiv2.gssup.password_file: {e}")))?;
        Some(Arc::new(store))
    } else {
        None
    };
    let label = tenant.map_or_else(String::new, |t| format!("{t}/"));
    let dispatch = Arc::new(Dispatch {
        cfg: cfg.clone(),
        runtime,
        by_key,
        in_flight: Arc::new(InFlight {
            pending: Mutex::new(BTreeMap::new()),
            changed: Condvar::new(),
        }),
        next_id: AtomicU64::new(1),
        metrics: metrics.clone(),
        spec: metrics
            .registry
            .as_ref()
            .map(|r| SpecMetrics::register(r, cfg, &label)),
        reloadable: std::sync::RwLock::new(Reloadable {
            acl_default_deny: cfg.acl_default_deny,
            acl_rules: cfg.acl_rules.clone(),
            tls: None,
        }),
        conns: Mutex::new(BTreeMap::new()),
        next_conn: AtomicU64::new(1),
        draining: AtomicBool::new(false),
        csiv2_store,
        naming_servant,
        tracer: tracer.clone(),
        tenant: tenant.map(str::to_string),
        replay: cfg
            .mappings
            .iter()
            .map(|_| Mutex::new(ReplayStore::default()))
            .collect(),
        clocks: cfg
            .mappings
            .iter()
            .map(|_| {
                Mutex::new(SampleClock {
                    last: None,
                    reported: false,
                })
            })
            .collect(),
        started: Instant::now(),
        has_deadlines: cfg.mappings.iter().any(|m| m.deadline_ms.is_some()),
        models,
    });
    Ok((dispatch, reply_rx, notify_rx))
}

/// The pumps and the timeout thread of a participant.
fn start_threads(built: Built, timers_stop: &Arc<AtomicBool>) {
    let (d, reply_rx, notify_rx) = built;
    for (i, rx) in reply_rx {
        let d = Arc::clone(&d);
        std::thread::spawn(move || pump_replies(&d, i, &rx));
    }
    for (i, rx) in notify_rx {
        let d = Arc::clone(&d);
        std::thread::spawn(move || pump_notify(&d, i, &rx));
    }
    let sd = Arc::clone(timers_stop);
    std::thread::spawn(move || timeout_loop(&d, &sd));
}

/// [`run`], reloading ACL and SSLIOP material on `reload` (§9.2) and
/// sending the spans of the exchanges to `tracer` (§8.3).
///
/// # Errors
/// As [`run`].
#[allow(clippy::too_many_lines)]
pub fn run_with_reload(
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    shutdown: &Arc<AtomicBool>,
    metrics: &BridgeMetrics,
    reload: Option<Reload>,
    tracer: Tracer,
) -> Result<(), DaemonError> {
    LOG_LEVEL.store(level_rank(&cfg.log_level), Ordering::Relaxed);

    // corba.naming.serve: a CosNaming::NamingContext (object key
    // "NameService") hosted on this same IIOP/SSLIOP listener, sharing
    // its transport and the servant code with zerodds-naming-svc's
    // NamingServant -- same persistence option (corba.naming.persist_file),
    // loaded here and saved after every mutating operation and on a clean
    // stop.
    let naming_graph = if cfg.naming_serve {
        let root = if cfg.naming_persist_file.is_empty() {
            None
        } else {
            let path = std::path::Path::new(&cfg.naming_persist_file);
            if path.exists() {
                Some(
                    zerodds_corba_cosnaming::persist::load(path)
                        .map_err(|e| err(1, format!("corba.naming.persist_file: {e}")))?,
                )
            } else {
                None
            }
        };
        Some(Arc::new(root.map_or_else(
            zerodds_corba_cosnaming::graph::NamingGraph::new,
            zerodds_corba_cosnaming::graph::NamingGraph::from_root,
        )))
    } else {
        None
    };
    let naming_servant = naming_graph.as_ref().map(|graph| {
        let (host, port) = host_port(&cfg.iiop_bind, 6833);
        // This daemon reads GIOP 1.0 to 1.2: the hosted naming
        // references name no higher IIOP version (CORBA 3.3 Part 2
        // §9.7.2), also for an IPv6 host.
        let mut servant = zerodds_corba_cosnaming::servant::NamingServant::new(
            Arc::clone(graph),
            advertised(host, &cfg.iiop_advertise_host),
            port,
        )
        .with_highest_version(zerodds_corba_iiop::IiopVersion::V1_2);
        if !cfg.naming_persist_file.is_empty() {
            servant = servant.with_persistence(zerodds_corba_cosnaming::servant::Persistence {
                path: std::path::PathBuf::from(&cfg.naming_persist_file),
            });
        }
        Arc::new(servant)
    });

    // §1 L6: the participant of `domain` and one per tenant.
    let default = build_dispatch(
        cfg,
        None,
        runtime_config.clone(),
        metrics,
        &tracer,
        naming_servant,
    )?;
    let mut tenant_built = Vec::new();
    for t in &cfg.tenants {
        let built = build_dispatch(
            &cfg.of_tenant(t),
            Some(&t.name),
            runtime_config.clone(),
            metrics,
            &tracer,
            None,
        )?;
        tenant_built.push((t.subjects.clone(), built));
    }
    let tenants = Arc::new(Tenants {
        default: Arc::clone(&default.0),
        bound: tenant_built
            .iter()
            .map(|(s, b)| (s.clone(), Arc::clone(&b.0)))
            .collect(),
    });

    // Listeners: IIOP, and SSLIOP when enabled — bound before an IOR is
    // written or published (§9.1 steps 5 and 6), so that a client that
    // reads one finds the port accepting.
    let iiop = TcpListener::bind(&cfg.iiop_bind)
        .map_err(|e| err(2, format!("bind {}: {e}", cfg.iiop_bind)))?;
    log(
        "listening",
        &[("addr", &cfg.iiop_bind), ("transport", "iiop")],
    );
    let ssl_listener = if cfg.ssliop_enabled {
        let server = ssliop_server_config(cfg).map_err(|e| err(4, e))?;
        for d in tenants.all() {
            if let Ok(mut r) = d.reloadable.write() {
                r.tls = Some(Arc::clone(&server));
            }
        }
        let l = TcpListener::bind(&cfg.ssliop_bind)
            .map_err(|e| err(2, format!("bind {}: {e}", cfg.ssliop_bind)))?;
        log(
            "listening",
            &[("addr", &cfg.ssliop_bind), ("transport", "ssliop")],
        );
        Some(l)
    } else {
        None
    };

    // IORs (§4.1).
    let lines =
        ior_lines(cfg, &cfg.iiop_advertise_host).map_err(|e| err(1, format!("IOR: {e}")))?;
    // §5.2: the generated IDL of the mappings that name theirs.
    for (path, text) in typegen::files(cfg).map_err(|e| err(1, e))? {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| err(1, format!("corba.idl_dir {}: {e}", dir.display())))?;
        }
        std::fs::write(&path, text)
            .map_err(|e| err(1, format!("corba.idl_dir {}: {e}", path.display())))?;
        log("idl_written", &[("file", &path.display().to_string())]);
    }
    if !cfg.ior_file.is_empty() {
        std::fs::write(&cfg.ior_file, lines.join("\n") + "\n")
            .map_err(|e| err(1, format!("corba.ior.ior_file {}: {e}", cfg.ior_file)))?;
    }
    for l in &lines {
        log("ior", &[("ior", l)]);
    }

    // corba.ior.naming_service: rebind every receiving mapping's servant,
    // retrying with backoff for corba.ior.naming_retry_ms before aborting
    // the start. A NameService that never comes up costs the daemon its own
    // start: a bridge whose servants no client can look up is not a working
    // bridge.
    let bound_names = if cfg.publish_to_naming {
        match crate::naming_client::rebind_all(cfg, &cfg.iiop_advertise_host) {
            Ok(names) => {
                log(
                    "naming_bound",
                    &[
                        ("count", &names.len().to_string()),
                        ("service", &cfg.naming_service),
                    ],
                );
                names
            }
            // §2: exit 5, NameService bind error.
            Err(e) => return Err(err(5, format!("corba.ior.naming_service: {e}"))),
        }
    } else {
        Vec::new()
    };

    // The pumps and timeouts keep running through the drain; they stop
    // after it.
    let timers_stop = Arc::new(AtomicBool::new(false));
    start_threads(default, &timers_stop);
    for (_, built) in tenant_built {
        start_threads(built, &timers_stop);
    }
    {
        let t = Arc::clone(&tenants);
        let sd = Arc::clone(shutdown);
        std::thread::spawn(move || accept_loop(&iiop, &t, &sd, false));
    }
    if let Some(l) = ssl_listener {
        let t = Arc::clone(&tenants);
        let sd = Arc::clone(shutdown);
        std::thread::spawn(move || accept_loop(&l, &t, &sd, true));
    }
    zerodds_dcps::cli::notify_service_manager("READY=1");
    log("running", &[("tenants", &cfg.tenants.len().to_string())]);
    while !shutdown.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(r) = &reload
            && r.flag.swap(false, Ordering::SeqCst)
        {
            apply_reload(&tenants.all(), cfg, &*r.load);
        }
    }
    zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    if !bound_names.is_empty() {
        crate::naming_client::unbind_all(cfg, &bound_names);
        log(
            "naming_unbound",
            &[("count", &bound_names.len().to_string())],
        );
    }
    let all = tenants.all();
    let drains: Vec<_> = all
        .iter()
        .map(|d| {
            let d = Arc::clone(d);
            std::thread::spawn(move || drain(&d, DRAIN_LIMIT))
        })
        .collect();
    for h in drains {
        let _ = h.join();
    }
    timers_stop.store(true, Ordering::SeqCst);
    for d in &all {
        d.in_flight.changed.notify_all();
    }
    if !cfg.naming_persist_file.is_empty()
        && let Some(graph) = &naming_graph
    {
        let _ = zerodds_corba_cosnaming::persist::save(
            &graph.root(),
            std::path::Path::new(&cfg.naming_persist_file),
        );
    }
    log("shutdown", &[]);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::config::tests::load;

    /// §6: the keys of `qos` are the policies of the DDS endpoints.
    #[test]
    fn corbabridge_6_the_qos_of_a_mapping_are_its_endpoint_policies() {
        let c = load(
            "mappings:\n  - repo_id: X\n    operation: op\n    qos:\n      durability: transient_local\n      history:\n        depth: 3\n      lifespan_ms: 1500\n      deadline_ms: 250\n      liveliness_lease_ms: 4000\n",
        )
        .unwrap();
        let (deadline, liveliness, lifespan, service) = endpoint_qos(&c.mappings[0]);
        assert_eq!(deadline.period, zerodds_qos::Duration::from_millis(250));
        assert_eq!(
            liveliness.lease_duration,
            zerodds_qos::Duration::from_millis(4000)
        );
        assert_eq!(lifespan.duration, zerodds_qos::Duration::from_millis(1500));
        assert_eq!(service.history_kind, zerodds_qos::HistoryKind::KeepLast);
        assert_eq!(service.history_depth, 3);
        let plain = load("mappings:\n  - repo_id: X\n    operation: op\n").unwrap();
        let (deadline, _, lifespan, _) = endpoint_qos(&plain.mappings[0]);
        assert_eq!(deadline, zerodds_qos::DeadlineQosPolicy::default());
        assert_eq!(lifespan, zerodds_qos::LifespanQosPolicy::default());
    }
}
