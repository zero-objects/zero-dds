// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon `zerodds-amqp-bridged`
//! (`docs/specs/zerodds-amqp-bridge-daemon-1.0.md`): a DDS participant on
//! one side, a client of an AMQP 1.0 broker on the other.
//!
//! Per topic of the configuration:
//!
//! * `out` / `bidir`: a DDS reader, and a sender link to the topic's AMQP
//!   address. Every sample becomes one message (§4.3): the Pass-Through
//!   body of DDS-AMQP 1.0 §8.1.1, the properties of §8.2, the `dds:*`
//!   application properties of §8.3.
//! * `in` / `bidir`: a receiver link from the address, and a DDS writer.
//!   Every message is written as a sample and settled (§4.4, §4.5):
//!   `accepted` once written, `rejected` for a body that is not a DDS
//!   sample, `released` when the write fails.
//!
//! A sample the broker has no credit for waits in the link's queue, bounded
//! by the topic's HISTORY (§6). Unsettled deliveries survive a reconnect and
//! are sent again on the new connection (§9.3). A message this bridge sent
//! itself, coming back on a receiver of the same address, is dropped by its
//! `dds:bridge-id` (DDS-AMQP 1.0 §8.4); a DDS sample this bridge wrote,
//! coming back on its own reader, is dropped by its writer GUID.
//!
//! The modules follow the parts of the specification (§12.1):
//!
//! * `config`: the configuration of §3;
//! * `message`: the message form of a sample, §4.3 and §4.4;
//! * `link_state`: sessions, links and credit, §4.2 and §9.3;
//! * `disposition`: settling deliveries in both directions, §4.4 and §4.5;
//! * `dds_pump`: the loop over DDS samples and broker frames, §4.3 and §9.2;
//! * `acl`: the topic ACL, §7.3; `reload`: SIGHUP, §9.2;
//! * `cbs`: the `put-token` exchange of claims-based security, §7.2;
//! * `log`: the JSON log line, §8.1; `trace`: the OTLP spans, §8.3.
//!
//! The frames themselves, the transport and the SASL exchange are
//! [`crate::broker`]'s.

mod acl;
mod cbs;
mod config;
mod dds_pump;
mod disposition;
mod link_state;
mod log;
mod message;
mod reload;
mod trace;

use std::collections::{BTreeMap, VecDeque};
use std::string::{String, ToString};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig};
use zerodds_rtps::wire_types::EntityId;

use crate::broker::{self, AmqpError, ConnectParams, LinkSpec, SaslCredentials};
use crate::coexistence::CoexistenceConfig;
use crate::daemon_runtime::BridgeMetrics;

pub use config::{
    DRAIN_BUDGET, DaemonConfig, Direction, ENV, History, KEEP_ALL_QUEUE_CAP, KEYS, Reconnect,
    TenantConfig, TopicMapping, is_key, parse_broker_url,
};
pub use message::{
    CONTENT_TYPE_XCDR1, CONTENT_TYPE_XCDR2, InMessage, OutSample, decode_message, encode_message,
    split_encapsulation, wire_extensibility_of,
};
pub use reload::Reload;
pub use trace::Tracer;

use self::acl::{Live, allowed, subjects_of};
use self::dds_pump::{Event, pump};
use self::link_state::{
    InLink, LinkState, OutLink, Pending, begin_all, connection_lost, next_delay,
};
use self::log::{Field, LOG_LEVEL, emit, level_rank, log, set_participant};
use self::reload::{Watched, reload_all};

/// Why the daemon stopped.
#[derive(Debug)]
pub struct DaemonError {
    /// Exit code (§2): 1 config, 2 TCP connect, 3 DDS participant, 4 TLS,
    /// 5 SASL, 6 open refused or incompatible.
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

/// Shuts a runtime down when dropped.
struct ShutdownOnDrop(Arc<DcpsRuntime>);

impl Drop for ShutdownOnDrop {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

/// DEADLINE and LIVELINESS of the DDS endpoints of topic `t` (§6):
/// `qos.deadline_ms` as the period, `qos.liveliness_lease_ms` as the lease
/// of AUTOMATIC liveliness; infinite without the key.
fn endpoint_qos(
    t: &TopicMapping,
) -> (
    zerodds_qos::DeadlineQosPolicy,
    zerodds_qos::LivelinessQosPolicy,
) {
    // The configuration refuses a period beyond i32 milliseconds.
    let duration =
        |ms: u32| zerodds_qos::Duration::from_millis(i32::try_from(ms).unwrap_or(i32::MAX));
    let deadline = t
        .deadline_ms
        .map_or_else(zerodds_qos::DeadlineQosPolicy::default, |ms| {
            zerodds_qos::DeadlineQosPolicy {
                period: duration(ms),
            }
        });
    let liveliness =
        t.liveliness_lease_ms
            .map_or_else(zerodds_qos::LivelinessQosPolicy::default, |ms| {
                zerodds_qos::LivelinessQosPolicy {
                    kind: zerodds_qos::LivelinessKind::Automatic,
                    lease_duration: duration(ms),
                }
            });
    (deadline, liveliness)
}

/// The type `t.type_file` describes, for the instance keys of a keyed
/// topic's reader and writer.
fn key_type_support(
    t: &TopicMapping,
) -> Result<Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>, String> {
    let bytes = std::fs::read(&t.type_file)
        .map_err(|e| format!("topics[{}].type_file '{}': {e}", t.dds_name, t.type_file))?;
    let object = zerodds_types::type_object::TypeObject::from_bytes_le(&bytes).map_err(|e| {
        format!(
            "topics[{}].type_file '{}': not a TypeObject: {e:?}",
            t.dds_name, t.type_file
        )
    })?;
    let zerodds_types::type_object::TypeObject::Complete(complete) = object else {
        return Err(format!(
            "topics[{}].type_file '{}': a MINIMAL TypeObject; the file must hold the COMPLETE one",
            t.dds_name, t.type_file
        ));
    };
    let type_name = if t.dds_type.is_empty() {
        t.dds_name.as_str()
    } else {
        t.dds_type.as_str()
    };
    let support = zerodds_dcps::serialized_type::SerializedTypeSupport::from_complete_type_objects(
        type_name,
        complete,
        Vec::new(),
    )
    .map_err(|e| format!("topics[{}].type_file '{}': {e}", t.dds_name, t.type_file))?;
    if !support.is_keyed() {
        return Err(format!(
            "topics[{}].type_file '{}': the type has no @key, the topic is keyed",
            t.dds_name, t.type_file
        ));
    }
    Ok(support)
}

/// Sets the `log_level` threshold of the JSON log (§8.1).
pub fn set_log_level(level: &str) {
    LOG_LEVEL.store(level_rank(level), Ordering::Relaxed);
}

/// One line of the JSON log (§8.1) on stdout: `text` fields, then
/// `numbers`; for the lines the program writes beside the daemon's.
pub fn log_event(event: &str, text: &[(&str, &str)], numbers: &[(&str, u64)]) {
    let fields: Vec<(&str, Field<'_>)> = text
        .iter()
        .map(|(k, v)| (*k, Field::Text(v)))
        .chain(numbers.iter().map(|(k, n)| (*k, Field::Number(*n))))
        .collect();
    emit(event, &fields);
}

fn err(code: u8, msg: impl Into<String>) -> DaemonError {
    DaemonError {
        code,
        msg: msg.into(),
    }
}

/// The catalog message (§5.2): JSON in a data section.
fn catalog_message(cfg: &DaemonConfig) -> Result<Vec<u8>, String> {
    let properties = MessageSection::Properties(AmqpExtValue::List(vec![
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Str(cfg.catalog_address()),
        AmqpExtValue::Str("catalog".into()),
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Symbol("application/json".into()),
    ]));
    let mut out = properties.encode().map_err(|e| format!("{e:?}"))?;
    out.extend_from_slice(
        &MessageSection::Data(cfg.catalog_json().into_bytes())
            .encode()
            .map_err(|e| format!("{e:?}"))?,
    );
    Ok(out)
}

/// What the admin endpoint serves on `/catalog`: the daemon's topics, then
/// those of each tenant (§1.1) with the tenant's name.
#[must_use]
pub fn admin_catalog(cfg: &DaemonConfig) -> crate::daemon_runtime::CatalogSnapshot {
    let entry = |t: &TopicMapping, tenant: Option<&str>| crate::daemon_runtime::CatalogTopic {
        dds_name: t.dds_name.clone(),
        amqp_address: t.amqp_address.clone(),
        direction: match t.direction {
            Direction::In => "in",
            Direction::Out => "out",
            Direction::Bidir => "bidir",
        }
        .to_string(),
        tenant: tenant.map(str::to_string),
    };
    let mut topics: Vec<_> = cfg.topics.iter().map(|t| entry(t, None)).collect();
    for tenant in &cfg.tenants {
        topics.extend(tenant.topics.iter().map(|t| entry(t, Some(&tenant.name))));
    }
    crate::daemon_runtime::CatalogSnapshot::new(topics)
}

fn bridge_uuid() -> String {
    let mut b = [0u8; 16];
    let _ = zerodds_bridge_security::tls_provider()
        .secure_random
        .fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0],
        b[1],
        b[2],
        b[3],
        b[4],
        b[5],
        b[6],
        b[7],
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15]
    )
}

fn tls_config(cfg: &DaemonConfig) -> Result<Option<Arc<rustls::ClientConfig>>, String> {
    if !cfg.tls_enabled {
        return Ok(None);
    }
    if cfg.tls_ca_file.is_empty() {
        return Err("amqp.tls.enabled needs amqp.tls.ca_file, the CA the broker certificate is checked against".into());
    }
    use std::path::PathBuf;
    let path = |s: &str| (!s.is_empty()).then(|| PathBuf::from(s));
    zerodds_bridge_security::connection::build_client_tls_connector(
        path(&cfg.tls_ca_file).as_deref(),
        path(&cfg.tls_cert_file).as_deref(),
        path(&cfg.tls_key_file).as_deref(),
    )
    .map(Some)
    .map_err(|e| format!("amqp.tls: {e}"))
}

/// Runs the daemon until `shutdown` is set, or, with `once`, until the
/// first connection ends.
///
/// # Errors
/// [`DaemonError`] with the exit code of §2.
pub fn run(
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    shutdown: &Arc<AtomicBool>,
    metrics: &BridgeMetrics,
    once: bool,
    connect_timeout: Duration,
) -> Result<(), DaemonError> {
    run_with_reload(
        cfg,
        runtime_config,
        shutdown,
        metrics,
        once,
        connect_timeout,
        None,
        Tracer::default(),
    )
}

/// [`run`], reloading ACL and TLS material on `reload` (§9.2), the spans of
/// every TRANSFER round trip to `tracer` (§8.3).
///
/// With `tenants` (§1 L6) every tenant's participant and broker connection
/// runs in a thread of its own beside the daemon's; a tenant that stops
/// with an error stops the others, and the first error by the order of
/// the configuration (the daemon's own first) is the result.
///
/// # Errors
/// As [`run`].
#[allow(clippy::too_many_arguments)]
pub fn run_with_reload(
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    shutdown: &Arc<AtomicBool>,
    metrics: &BridgeMetrics,
    once: bool,
    connect_timeout: Duration,
    reload: Option<Reload>,
    tracer: Tracer,
) -> Result<(), DaemonError> {
    set_log_level(&cfg.log_level);
    // The threads below watch `shutdown`; they end with this call, on every
    // path out of it.
    let mut watchers = Watchers::default();
    // STOPPING=1 as soon as the stop is seen, wherever the daemon is then
    // (connecting, pumping or backing off); the drain follows.
    {
        let shutdown = Arc::clone(shutdown);
        watchers.spawn("amqp-stopping", move |done| {
            watch_stopping(&shutdown, done, zerodds_dcps::cli::notify_service_manager);
        })?;
    }
    // The participants: the daemon's own, then one per tenant.
    let mut parts: Vec<Participant> = vec![Participant::prepare(
        None,
        cfg.clone(),
        metrics,
        tracer.clone(),
    )?];
    for t in &cfg.tenants {
        parts.push(Participant::prepare(
            Some(t.name.clone()),
            cfg.of_tenant(t),
            &metrics.for_tenant(&t.name),
            tracer.clone(),
        )?);
    }
    if let Some(r) = reload {
        let lives: Vec<Watched> = parts
            .iter()
            .map(|p| (p.tenant.clone(), Arc::clone(&p.live), p.cfg.clone()))
            .collect();
        let shutdown = Arc::clone(shutdown);
        watchers.spawn("amqp-reload", move |done| {
            while !shutdown.load(Ordering::SeqCst) && !done.load(Ordering::SeqCst) {
                std::thread::park_timeout(Duration::from_millis(100));
                // A run that has ended has no participant to reload.
                if done.load(Ordering::SeqCst) {
                    break;
                }
                if r.flag.swap(false, Ordering::SeqCst) {
                    reload_all(&lives, &(r.load)());
                }
            }
        })?;
    }
    if parts.len() == 1 {
        let p = parts.remove(0);
        return p.run(runtime_config, shutdown, once, connect_timeout);
    }
    // One stop for all: the caller's, or the first participant that ends
    // with an error.
    let stop = Arc::new(AtomicBool::new(false));
    {
        let (stop, shutdown) = (Arc::clone(&stop), Arc::clone(shutdown));
        watchers.spawn("amqp-stop", move |done| {
            while !stop.load(Ordering::SeqCst) && !done.load(Ordering::SeqCst) {
                if shutdown.load(Ordering::SeqCst) {
                    stop.store(true, Ordering::SeqCst);
                }
                std::thread::park_timeout(Duration::from_millis(50));
            }
        })?;
    }
    let own = parts.remove(0);
    let mut joins = Vec::new();
    for p in parts {
        let (stop_t, rc) = (Arc::clone(&stop), runtime_config.clone());
        let name = p.tenant.clone().unwrap_or_default();
        let spawned = std::thread::Builder::new()
            .name(format!("amqp-tenant-{name}"))
            .spawn(move || {
                let result = p.run(rc, &stop_t, once, connect_timeout);
                if let Err(e) = &result {
                    log("tenant_stopped", &[("err", &e.msg)]);
                    stop_t.store(true, Ordering::SeqCst);
                }
                result
            });
        match spawned {
            Ok(join) => joins.push(join),
            Err(e) => {
                // The tenants already running end with this call.
                stop.store(true, Ordering::SeqCst);
                for j in joins {
                    let _ = j.join();
                }
                return Err(err(3, format!("tenant thread {name}: {e}")));
            }
        }
    }
    let first = own.run(runtime_config, &stop, once, connect_timeout);
    if first.is_err() {
        stop.store(true, Ordering::SeqCst);
    }
    let mut result = first;
    for j in joins {
        let r = j
            .join()
            .unwrap_or_else(|_| Err(err(3, "a tenant thread panicked")));
        if result.is_ok() {
            result = r;
        }
    }
    stop.store(true, Ordering::SeqCst);
    result
}

/// The threads of one [`run_with_reload`] call that watch the caller's
/// `shutdown`: STOPPING=1, SIGHUP, the common stop of the tenants. Dropped
/// on every path out of the call, it raises their `done` flag, wakes them
/// and joins them. None of them outlives the call: a caller that runs the
/// daemon again in the same process (a test, an embedding) does not
/// collect them, a reload does not reach the participants of an ended run,
/// and a `shutdown` set after the call sends no STOPPING=1.
#[derive(Default)]
struct Watchers {
    done: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl Watchers {
    /// Runs `body` in thread `name`. `body` gets the flag raised when the
    /// call ends and is woken then (`Thread::unpark`); it waits with
    /// `std::thread::park_timeout`.
    fn spawn(
        &mut self,
        name: &str,
        body: impl FnOnce(&AtomicBool) + Send + 'static,
    ) -> Result<(), DaemonError> {
        let done = Arc::clone(&self.done);
        let thread = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || body(&done))
            .map_err(|e| err(1, format!("{name} thread: {e}")))?;
        self.threads.push(thread);
        Ok(())
    }
}

impl Drop for Watchers {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        for t in &self.threads {
            t.thread().unpark();
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// STOPPING=1 through `notify` once `shutdown` is seen (§9.2), looked at
/// every 100 ms; nothing when `done` comes first. A stop seen together with
/// `done` is still told.
fn watch_stopping(shutdown: &AtomicBool, done: &AtomicBool, notify: impl Fn(&str)) {
    loop {
        if shutdown.load(Ordering::SeqCst) {
            notify("STOPPING=1");
            return;
        }
        if done.load(Ordering::SeqCst) {
            return;
        }
        std::thread::park_timeout(Duration::from_millis(100));
    }
}

/// One participant of the daemon (§1 L6) with its broker connection,
/// checked before anything runs.
struct Participant {
    /// `None` for the daemon's own.
    tenant: Option<String>,
    cfg: DaemonConfig,
    metrics: BridgeMetrics,
    live: Arc<Live>,
    sasl: SaslCredentials,
    tls: Option<Arc<rustls::ClientConfig>>,
    /// The subjects the ACL knows the participant by (§7.3).
    subjects: Vec<String>,
    host: String,
    port: u16,
    tracer: Tracer,
}

impl Participant {
    /// The checks of §2 and §7 on `cfg`, the TLS material, the metrics of
    /// its topics.
    fn prepare(
        tenant: Option<String>,
        cfg: DaemonConfig,
        metrics: &BridgeMetrics,
        tracer: Tracer,
    ) -> Result<Self, DaemonError> {
        let at = |m: String| match &tenant {
            Some(t) => format!("tenants[{t}]: {m}"),
            None => m,
        };
        let (host, port, url_tls) = parse_broker_url(&cfg.broker_url).map_err(|e| err(1, at(e)))?;
        if url_tls != cfg.tls_enabled {
            return Err(err(
                1,
                at(format!(
                    "amqp.broker_url {} and amqp.tls.enabled={} disagree; amqps:// is TLS",
                    cfg.broker_url, cfg.tls_enabled
                )),
            ));
        }
        let sasl = cfg.sasl().map_err(|e| err(1, at(e)))?;
        let mechanism = sasl.mechanism();
        let topic_names: Vec<String> = cfg.topics.iter().map(|t| t.dds_name.clone()).collect();
        let metrics = metrics.with_topics(&topic_names, mechanism);
        if matches!(sasl, SaslCredentials::Plain { .. }) && !cfg.tls_enabled && !is_loopback(&host)
        {
            return Err(err(
                1,
                at("SASL PLAIN sends the password in the clear; use amqps:// (spec §7.2)".into()),
            ));
        }
        // With CBS over TLS the links are authorized by their tokens
        // (Azure Service Bus: SASL ANONYMOUS, then put-token).
        let cbs_over_tls = cfg.cbs_enabled && cfg.tls_enabled;
        if matches!(sasl, SaslCredentials::Anonymous) && !is_loopback(&host) && !cbs_over_tls {
            return Err(err(
                1,
                at(
                    "SASL ANONYMOUS is allowed only towards a loopback broker, or over TLS with amqp.cbs.enabled (spec §7.2)"
                        .into(),
                ),
            ));
        }
        if cfg.cbs_enabled && (cfg.cbs_key_name.is_empty() || cfg.cbs_secret.is_empty()) {
            return Err(err(
                1,
                at(
                    "amqp.cbs.enabled needs amqp.cbs.key_name and amqp.cbs.secret (spec §7.2)"
                        .into(),
                ),
            ));
        }
        let tls = tls_config(&cfg).map_err(|e| err(1, at(e)))?;
        if tls.is_some() {
            // §9.1 step 2, before the DDS participant of step 3.
            set_participant(tenant.as_deref(), Some(&cfg.container_id));
            log(
                "tls_loaded",
                &[
                    ("ca_file", &cfg.tls_ca_file),
                    ("cert_file", &cfg.tls_cert_file),
                ],
            );
        }
        let subjects = subjects_of(&cfg, &sasl).map_err(|e| err(1, at(e)))?;
        let live = Arc::new(Live::new(&cfg, tls.clone(), subjects.clone()));
        Ok(Self {
            tenant,
            cfg,
            metrics,
            live,
            sasl,
            tls,
            subjects,
            host,
            port,
            tracer,
        })
    }

    /// Runs the participant until `shutdown` is set, or, with `once`,
    /// until its first connection ends.
    #[allow(clippy::too_many_lines)]
    fn run(
        self,
        runtime_config: RuntimeConfig,
        shutdown: &Arc<AtomicBool>,
        once: bool,
        connect_timeout: Duration,
    ) -> Result<(), DaemonError> {
        let Self {
            tenant,
            cfg,
            metrics,
            live,
            sasl,
            tls,
            subjects,
            host,
            port,
            tracer,
        } = self;
        // The log lines and spans of this thread are this participant's
        // (§1.1, §8.1, §8.3).
        set_participant(tenant.as_deref(), Some(&cfg.container_id));
        trace::set_tracer(tracer);
        let cfg = &cfg;
        let metrics = &metrics;
        let prefix = zerodds_dcps::participant::bridge_guid_prefix(&cfg.container_id);
        let runtime = DcpsRuntime::start(cfg.domain, prefix, runtime_config)
            .map_err(|e| err(3, format!("DDS participant: {e:?}")))?;
        // The participant ends with this function, on every path out of it: a
        // runtime's own threads hold it, so dropping the last handle stops
        // nothing, and a caller that runs the endpoint again in the same
        // process (a test, an embedding) met the previous participant — same
        // GUID prefix, same writers — still on the bus.
        let _participant = ShutdownOnDrop(Arc::clone(&runtime));
        // §9.1 step 3.
        emit(
            "participant_started",
            &[
                (
                    "domain",
                    Field::Number(u64::from(cfg.domain.unsigned_abs())),
                ),
                (
                    "guid_prefix",
                    Field::Text(&crate::properties::hex_lower(&prefix.to_bytes())),
                ),
            ],
        );

        let (tx, rx) = mpsc::channel::<Event>();
        let mut writers: BTreeMap<usize, EntityId> = BTreeMap::new();
        let mut readers: BTreeMap<usize, EntityId> = BTreeMap::new();
        for (i, t) in cfg.topics.iter().enumerate() {
            if t.direction.sends() {
                if !allowed(cfg, &subjects, &t.dds_name, false) {
                    log("acl_denied", &[("dds_topic", &t.dds_name), ("op", "read")]);
                } else {
                    let (deadline, liveliness) = endpoint_qos(t);
                    let (eid, samples) = runtime
                        .register_user_reader_kind(
                            UserReaderConfig {
                                topic_name: t.dds_name.clone(),
                                type_name: t.dds_type.clone(),
                                reliable: t.reliable,
                                durability: t.durability,
                                deadline,
                                liveliness,
                                partition: t.partition.clone(),
                                ..Default::default()
                            },
                            t.keyed,
                        )
                        .map_err(|e| err(3, format!("DDS reader {}: {e:?}", t.dds_name)))?;
                    // A writer that leaves PID_KEY_HASH out (Cyclone): the
                    // reader computes the KeyHash from the type (§3).
                    if t.keyed && !t.type_file.is_empty() {
                        let support = key_type_support(t).map_err(|e| err(1, e))?;
                        runtime
                            .set_user_reader_key_support(eid, support)
                            .map_err(|e| {
                                err(1, format!("topics[{}].type_file: {e:?}", t.dds_name))
                            })?;
                    }
                    readers.insert(i, eid);
                    let tx = tx.clone();
                    std::thread::Builder::new()
                        .name(format!("amqp-dds-{i}"))
                        .spawn(move || {
                            while let Ok(s) = samples.recv() {
                                if tx.send(Event::Dds(i, Box::new(s))).is_err() {
                                    break;
                                }
                            }
                        })
                        .map_err(|e| err(3, format!("reader thread: {e}")))?;
                }
            }
            if t.direction.receives() {
                if !allowed(cfg, &subjects, &t.dds_name, true) {
                    log("acl_denied", &[("dds_topic", &t.dds_name), ("op", "write")]);
                } else {
                    let (deadline, liveliness) = endpoint_qos(t);
                    let eid = runtime
                        .register_user_writer_kind(
                            UserWriterConfig {
                                topic_name: t.dds_name.clone(),
                                type_name: t.dds_type.clone(),
                                reliable: t.reliable,
                                durability: t.durability,
                                deadline,
                                liveliness,
                                partition: t.partition.clone(),
                                ..Default::default()
                            },
                            t.keyed,
                        )
                        .map_err(|e| err(3, format!("DDS writer {}: {e:?}", t.dds_name)))?;
                    // HISTORY of the writer (§6); `usize::MAX` is KEEP_ALL.
                    let depth = match t.history {
                        History::KeepLast(d) => d,
                        History::KeepAll => usize::MAX,
                    };
                    runtime
                        .set_user_writer_history_depth(eid, depth)
                        .map_err(|e| err(3, format!("DDS writer {}: {e:?}", t.dds_name)))?;
                    if t.keyed && !t.type_file.is_empty() {
                        let support = key_type_support(t).map_err(|e| err(1, e))?;
                        runtime
                            .set_user_writer_key_support(eid, support)
                            .map_err(|e| {
                                err(1, format!("topics[{}].type_file: {e:?}", t.dds_name))
                            })?;
                    }
                    writers.insert(i, eid);
                }
            }
        }

        // §9.1 step 4: the DDS endpoints stand before the broker is
        // contacted (step 5).
        emit(
            "endpoints_registered",
            &[
                ("readers", Field::Number(readers.len() as u64)),
                ("writers", Field::Number(writers.len() as u64)),
            ],
        );
        let coexistence = CoexistenceConfig::new(bridge_uuid());
        let initial_retry = Duration::from_millis(cfg.reconnect.initial_delay_ms);
        let mut outs: Vec<OutLink> = Vec::new();
        let mut ins: Vec<InLink> = Vec::new();
        let mut handle = 0u32;
        for (i, t) in cfg.topics.iter().enumerate() {
            if t.direction.sends() && allowed(cfg, &subjects, &t.dds_name, false) {
                let mut l = OutLink::for_topic(cfg, i, handle as u16, &live);
                l.reader = readers.get(&i).copied();
                outs.push(l);
                handle += 1;
            }
            if let Some(eid) = writers.get(&i) {
                ins.push(InLink::for_topic(cfg, i, handle as u16, *eid));
                handle += 1;
            }
        }
        if cfg.catalog_enabled {
            outs.push(OutLink {
                topic: None,
                dds_topic: String::new(),
                channel: handle as u16,
                state: LinkState::Down,
                spec: LinkSpec {
                    name: format!("{}/catalog/sender", cfg.container_id),
                    handle,
                    sender: true,
                    address: cfg.catalog_address(),
                    snd_settle_mode: 1,
                    rcv_settle_mode: 0,
                    durable: 0,
                    properties: vec![],
                    desired_capabilities: vec![],
                    coordinator: None,
                },
                settled: true,
                remote_handle: None,
                credit: 0,
                delivery_count: crate::broker::INITIAL_DELIVERY_COUNT,
                drain: false,
                echo: false,
                stopping: false,
                queue: VecDeque::new(),
                unsettled: BTreeMap::new(),
                cap: 1,
                next_tag: 0,
                retry_at: None,
                retry_delay: initial_retry,
                reader: None,
                deadline_missed: 0,
                deadline_note: None,
                writer_partitions: BTreeMap::new(),
                key_hash_unknown: std::collections::BTreeSet::new(),
                acl: Arc::clone(&live),
            });
        }
        let catalog = catalog_message(cfg).map_err(|e| err(1, format!("catalog: {e}")))?;
        let links = outs.len() + ins.len();
        // The CBS session follows the links' sessions (§7.2): one token per
        // address, before the links are attached.
        let mut cbs = cfg.cbs_enabled.then(|| {
            let audience_host = if cfg.hostname.is_empty() {
                host.as_str()
            } else {
                cfg.hostname.as_str()
            };
            let mut audiences: Vec<String> = outs
                .iter()
                .map(|l| l.spec.address.clone())
                .chain(ins.iter().map(|l| l.spec.address.clone()))
                .map(|a| cbs::audience(audience_host, &a))
                .collect();
            audiences.sort();
            audiences.dedup();
            cbs::Cbs::new(
                &cfg.container_id,
                links as u16,
                links as u32,
                &cfg.cbs_key_name,
                &cfg.cbs_secret,
                Duration::from_secs(u64::from(cfg.cbs_ttl_s)),
                audiences,
                connect_timeout,
            )
        });
        let sessions = links + usize::from(cbs.is_some());
        if sessions > usize::from(cfg.channel_max) + 1 {
            return Err(err(
                1,
                format!(
                    "{links} links and the CBS session need {sessions} sessions; amqp.channel_max {} allows {}",
                    cfg.channel_max,
                    usize::from(cfg.channel_max) + 1
                ),
            ));
        }

        let mut params = ConnectParams {
            addr: format!("{host}:{port}"),
            host: host.clone(),
            tls,
            sasl,
            container_id: cfg.container_id.clone(),
            hostname: Some(if cfg.hostname.is_empty() {
                host.clone()
            } else {
                cfg.hostname.clone()
            }),
            max_frame_size: cfg.max_frame_size,
            channel_max: cfg.channel_max,
            idle_time_out_ms: cfg.idle_time_out_ms,
            properties: vec![
                ("zerodds_version".into(), "1.0".into()),
                ("zerodds_role".into(), "bridge".into()),
            ],
            capabilities: vec!["AMQP_DDS_BRIDGE".into()],
            timeout: connect_timeout,
        };

        let mut delay = initial_retry;
        let mut connected_once = false;
        let mut generation = 0u64;
        let own_prefix = prefix.to_bytes();
        loop {
            if shutdown.load(Ordering::SeqCst) {
                // Stopped while no connection stood (§9.2).
                left_at_stop(&outs, &rx, metrics);
                return Ok(());
            }
            generation += 1;
            metrics.connections_total.inc();
            // The TLS material of the last reload (§9.2).
            if let Ok(t) = live.tls.lock() {
                params.tls = t.clone();
            }
            let (mut writer, mut reader, remote) = match broker::connect(&params) {
                Ok(c) => {
                    if let Some(sm) = &metrics.spec {
                        sm.connections_established.inc();
                    }
                    // Ready once the broker connection stands (also after a
                    // reconnect; a repeated READY=1 changes nothing).
                    zerodds_dcps::cli::notify_service_manager("READY=1");
                    c
                }
                Err(e) => {
                    metrics.errors_total.inc();
                    if let Some(sm) = &metrics.spec {
                        sm.connections_failed.inc();
                        if matches!(e.stage, broker::ConnectStage::Sasl) {
                            sm.sasl_failures.inc();
                        }
                    }
                    log(
                        "connect_error",
                        &[("broker", &params.addr), ("err", &e.to_string())],
                    );
                    // §2: 2 TCP, 4 TLS, 5 SASL, 6 open refused. A refusal before
                    // the first connection stood is a fault of the configuration
                    // or of the broker's, which a retry does not cure; a TCP
                    // failure and a time-out are retried after the back-off
                    // (§9.3).
                    if once || (!connected_once && is_refusal(&e)) {
                        return Err(err(
                            connect_exit_code(e.stage),
                            format!("connect {}: {e}", params.addr),
                        ));
                    }
                    sleep_unless(shutdown, delay);
                    delay = next_delay(delay, &cfg.reconnect);
                    continue;
                }
            };
            delay = initial_retry;
            connected_once = true;
            metrics.connections_active.set(1);
            log(
                "connected",
                &[
                    ("broker", &params.addr),
                    ("container", &remote.container_id),
                ],
            );
            {
                let tx = tx.clone();
                let gen_ = generation;
                std::thread::Builder::new()
                    .name("amqp-read".into())
                    .spawn(move || {
                        loop {
                            match reader.next_event() {
                                Ok(ev) => {
                                    let read_ns = trace::unix_ns();
                                    if tx.send(Event::Amqp(gen_, ev, read_ns)).is_err() {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(Event::Lost(gen_, e.to_string()));
                                    break;
                                }
                            }
                        }
                    })
                    .map_err(|e| err(2, format!("reader thread: {e}")))?;
            }
            // The catalog goes out once per connection (§5.2).
            if let Some(c) = outs.iter_mut().find(|l| l.topic.is_none()) {
                c.queue.clear();
                c.queue.push_back(Pending {
                    message: catalog.clone(),
                    attempts: 0,
                    taken_ns: 0,
                });
            }
            if sessions > usize::from(remote.channel_max) + 1 {
                writer.close(Some(&AmqpError {
                    condition: "amqp:resource-limit-exceeded".into(),
                    description: Some("channel-max".into()),
                }));
                // §2: an open whose properties do not fit the bridge is 6.
                return Err(err(
                    6,
                    format!(
                        "{sessions} sessions needed; the broker allows {}",
                        usize::from(remote.channel_max) + 1
                    ),
                ));
            }
            // With CBS the links are begun once their tokens are accepted.
            let begun = match cbs.as_mut() {
                Some(c) => c.start(&mut writer),
                None => begin_all(&mut writer, &mut outs, &mut ins),
            };
            let result = begun.and_then(|()| {
                pump(
                    cfg,
                    &live,
                    &runtime,
                    &mut writer,
                    &rx,
                    generation,
                    &remote,
                    &mut outs,
                    &mut ins,
                    &coexistence,
                    &own_prefix,
                    metrics,
                    shutdown,
                    cbs.as_mut(),
                )
            });
            metrics.connections_active.set(0);
            if let (Some(sm), Err(_)) = (&metrics.spec, &result) {
                sm.connections_lost.inc();
                sm.links_sender.set(0);
                sm.links_receiver.set(0);
            }
            match result {
                Ok(()) => {
                    // Shutdown: detach, end, close (§9.2). A link whose
                    // attach is out is detached too, also when the broker's
                    // attach is not read yet (Part 2 §2.6.3: the detach
                    // follows the endpoint's own attach).
                    let attached = |s: LinkState| matches!(s, LinkState::Up | LinkState::Attaching);
                    for l in &outs {
                        if attached(l.state) {
                            let _ = writer.detach(l.channel, l.spec.handle, None);
                        }
                    }
                    for l in &ins {
                        if attached(l.state) {
                            let _ = writer.detach(l.channel, l.spec.handle, None);
                        }
                    }
                    if let Some(c) = &cbs {
                        c.detach(&mut writer);
                    }
                    writer.close(None);
                    log("closed", &[]);
                    return Ok(());
                }
                Err(reason) => {
                    metrics.errors_total.inc();
                    log("connection_lost", &[("err", &reason)]);
                    writer.close(Some(&AmqpError {
                        condition: "amqp:internal-error".into(),
                        description: Some(reason.clone()),
                    }));
                    // Unsettled deliveries go out again first (§9.3).
                    connection_lost(&mut outs, &mut ins);
                    if once {
                        return Err(err(2, reason));
                    }
                    sleep_unless(shutdown, delay);
                    delay = next_delay(delay, &cfg.reconnect);
                }
            }
        }
    }
}

/// The exit code of a connect that failed in `stage` (§2).
fn connect_exit_code(stage: broker::ConnectStage) -> u8 {
    match stage {
        broker::ConnectStage::Tcp => 2,
        broker::ConnectStage::Tls => 4,
        broker::ConnectStage::Sasl => 5,
        broker::ConnectStage::Open => 6,
    }
}

/// Whether the broker refused the connection (§2): a failure after the TCP
/// connect stood, in the TLS handshake, the SASL exchange or the open, other
/// than a time-out. A broker that resets or ends the connection in these
/// stages has refused it too: RabbitMQ 4.0.5 resets the connection after the
/// open of an unknown virtual host (measured, no close frame). A TCP failure
/// is never a refusal: the broker may not be up yet.
fn is_refusal(e: &broker::ConnectError) -> bool {
    use std::io::ErrorKind as K;
    e.stage != broker::ConnectStage::Tcp
        && !matches!(e.error.kind(), K::TimedOut | K::WouldBlock | K::Interrupted)
}

/// What a stop without a broker connection leaves behind (§9.2): the
/// messages queued or unsettled on the topics' links and the samples the
/// DDS readers delivered that no pump took, counted in `errors_total` and
/// logged `shutdown_with_queued`.
fn left_at_stop(outs: &[OutLink], rx: &mpsc::Receiver<Event>, metrics: &BridgeMetrics) {
    let queued: usize = outs
        .iter()
        .filter(|l| l.topic.is_some())
        .map(|l| l.queue.len() + l.unsettled.len())
        .sum();
    let not_taken = rx
        .try_iter()
        .filter(|e| matches!(e, Event::Dds(..)))
        .count();
    if queued + not_taken > 0 {
        metrics.errors_total.inc();
        emit(
            "shutdown_with_queued",
            &[
                ("count", Field::Number(queued as u64)),
                ("not_taken", Field::Number(not_taken as u64)),
            ],
        );
    }
}

fn is_loopback(host: &str) -> bool {
    host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|a| a.is_loopback())
}

fn sleep_unless(shutdown: &AtomicBool, d: Duration) {
    let end = Instant::now() + d;
    while !shutdown.load(Ordering::SeqCst) {
        let now = Instant::now();
        if now >= end {
            break;
        }
        std::thread::sleep((end - now).min(Duration::from_millis(100)));
    }
}

/// Helpers the tests of the modules share.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod test_support {
    use super::*;

    /// The configuration `yaml` through the daemon's reader.
    pub(crate) fn load(yaml: &str) -> Result<DaemonConfig, String> {
        let mut c = zerodds_config::Config::default();
        c.push(
            zerodds_config::parse_str(yaml, "<string>").map_err(|e| e.to_string())?,
            "<string>".into(),
        )
        .map_err(|e| e.to_string())?;
        DaemonConfig::from_config(Some(&c)).map_err(|e| e.to_string())
    }

    /// Two topics for the link tests: 0 `Out` (DDS → AMQP, KEEP_LAST 3,
    /// unsettled), 1 `In` (AMQP → DDS, credit 8); back-off 100 ms to 1 s.
    pub(crate) fn two_topics() -> DaemonConfig {
        load(
            "amqp:\n  container_id: cid\n  reconnect:\n    initial_delay_ms: 100\n    max_delay_ms: 1000\n    factor: 2.0\ntopics:\n  - dds_name: Out\n    amqp_address: /queues/out\n    direction: out\n    qos:\n      history:\n        kind: keep_last\n        depth: 3\n  - dds_name: In\n    amqp_address: /queues/in\n    direction: in\n    receiver:\n      credit: 8\n",
        )
        .unwrap()
    }

    /// The metrics with the families of spec §8.2 for `cfg`'s topics.
    pub(crate) fn metrics(cfg: &DaemonConfig) -> BridgeMetrics {
        let registry = Arc::new(zerodds_monitor::Registry::new());
        let names: Vec<String> = cfg.topics.iter().map(|t| t.dds_name.clone()).collect();
        BridgeMetrics::register_shared(&registry).with_topics(&names, "ANONYMOUS")
    }

    /// The sending link of topic `topic` on session `channel`, attached
    /// (remote handle 40 + channel), without credit.
    pub(super) fn out_link(cfg: &DaemonConfig, topic: usize, channel: u16) -> OutLink {
        let acl = Arc::new(Live::new(cfg, None, Vec::new()));
        let mut l = OutLink::for_topic(cfg, topic, channel, &acl);
        l.state = LinkState::Up;
        l.remote_handle = Some(40 + u32::from(channel));
        l
    }

    /// The receiving link of topic `topic` on session `channel`, attached
    /// (remote handle 40 + channel), its credit granted.
    pub(super) fn in_link(cfg: &DaemonConfig, topic: usize, channel: u16, second: bool) -> InLink {
        let mut l = InLink::for_topic(cfg, topic, channel, EntityId::user_writer_no_key([0, 0, 1]));
        l.state = LinkState::Up;
        l.remote_handle = Some(40 + u32::from(channel));
        l.outstanding = cfg.topics[topic].credit;
        l.second = second;
        l
    }

    /// A message for a queue.
    pub(super) fn pending(byte: u8) -> Pending {
        Pending {
            message: vec![byte],
            attempts: 0,
            taken_ns: 0,
        }
    }

    /// A keyed topic `T` on `/queues/t`, both directions.
    pub(crate) fn topic() -> TopicMapping {
        load("topics:\n  - dds_name: T\n    amqp_address: /queues/t\n    keyed: true\n")
            .unwrap()
            .topics
            .remove(0)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// §1.1: the admin endpoint's `/catalog` lists the daemon's topics
    /// without a tenant, then each tenant's topics with its name.
    #[test]
    fn amqpbridge_1_l6_the_admin_catalog_lists_the_tenants_topics() {
        let cfg = test_support::load(
            "amqp:\n  container_id: own\ntopics:\n  - dds_name: A\n    amqp_address: /queues/a\n    direction: out\ntenants:\n  - name: plant-b\n    domain: 9\n    amqp:\n      container_id: cb\n    topics:\n      - dds_name: B\n        amqp_address: /queues/b\n        direction: in\n      - dds_name: A\n        amqp_address: /queues/ba\n",
        )
        .unwrap();
        let snap = admin_catalog(&cfg);
        let rows: Vec<(&str, &str, &str, Option<&str>)> = snap
            .topics
            .iter()
            .map(|t| {
                (
                    t.dds_name.as_str(),
                    t.amqp_address.as_str(),
                    t.direction.as_str(),
                    t.tenant.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("A", "/queues/a", "out", None),
                ("B", "/queues/b", "in", Some("plant-b")),
                ("A", "/queues/ba", "bidir", Some("plant-b")),
            ]
        );
        let json = snap.render_json();
        assert!(
            json.contains(
                r#"{"dds_name":"B","amqp_address":"/queues/b","direction":"in","tenant":"plant-b"}"#
            ),
            "{json}"
        );
        assert!(
            json.contains(r#"{"dds_name":"A","amqp_address":"/queues/a","direction":"out"}"#),
            "{json}"
        );
    }

    /// §7.2 (CBS): `amqp.cbs` is read with its four keys; enabled without
    /// key name or key is a configuration error (exit 1); ANONYMOUS towards
    /// a remote broker is allowed over TLS with CBS only.
    #[test]
    fn amqpbridge_10_cbs_keys_and_the_anonymous_rule() {
        let metrics = test_support::metrics(&test_support::two_topics());
        let cbs = |extra: &str, url: &str, tls: &str| {
            test_support::load(&format!(
                "amqp:\n  broker_url: \"{url}\"\n  sasl:\n    mechanism: ANONYMOUS\n{tls}  cbs:\n    enabled: true\n{extra}"
            ))
        };
        let c = cbs(
            "    key_name: kn\n    secret: k\n    ttl_s: 120\n",
            "amqp://127.0.0.1",
            "",
        )
        .unwrap();
        assert!(c.cbs_enabled);
        assert_eq!(
            (c.cbs_key_name.as_str(), c.cbs_secret.as_str(), c.cbs_ttl_s),
            ("kn", "k", 120)
        );
        assert!(
            cbs("    ttl_s: 5\n", "amqp://127.0.0.1", "").is_err(),
            "below 10 s"
        );
        let without_key = cbs("    key_name: kn\n", "amqp://127.0.0.1", "").unwrap();
        let e = Participant::prepare(None, without_key, &metrics, Tracer::default())
            .err()
            .expect("no key");
        assert_eq!(e.code, 1);
        assert!(e.msg.contains("amqp.cbs.secret"), "{}", e.msg);
        let plain_remote = cbs(
            "    key_name: kn\n    secret: k\n",
            "amqp://broker.example",
            "",
        )
        .unwrap();
        let e = Participant::prepare(None, plain_remote, &metrics, Tracer::default())
            .err()
            .expect("ANONYMOUS without TLS towards a remote broker");
        assert!(e.msg.contains("ANONYMOUS"), "{}", e.msg);
        let mut tls_remote = cbs(
            "    key_name: kn\n    secret: k\n",
            "amqps://broker.example",
            "  tls:\n    enabled: true\n",
        )
        .unwrap();
        // No CA file: the ANONYMOUS rule passes, the TLS material is next.
        tls_remote.tls_ca_file = String::new();
        let e = Participant::prepare(None, tls_remote, &metrics, Tracer::default())
            .err()
            .expect("no CA file");
        assert!(e.msg.contains("amqp.tls.ca_file"), "{}", e.msg);
    }

    /// §2: a TLS, SASL or open failure ends the daemon with 4, 5, 6, a reset
    /// or end of stream in these stages included; a time-out there, or a TCP
    /// failure, is retried.
    #[test]
    fn amqpbridge_2_a_refusal_is_told_from_a_broken_connection() {
        use broker::{ConnectError, ConnectStage as S};
        use std::io::{Error, ErrorKind as K};
        let e = |stage, kind| ConnectError {
            stage,
            error: Error::new(kind, "x"),
        };
        assert!(is_refusal(&e(S::Tls, K::InvalidData)));
        assert!(is_refusal(&e(S::Sasl, K::PermissionDenied)));
        assert!(is_refusal(&e(S::Open, K::InvalidData)));
        assert!(!is_refusal(&e(S::Tcp, K::ConnectionRefused)));
        assert!(!is_refusal(&e(S::Tcp, K::InvalidData)));
        assert!(
            is_refusal(&e(S::Open, K::ConnectionReset)),
            "RabbitMQ 4.0.5"
        );
        assert!(is_refusal(&e(S::Tls, K::UnexpectedEof)));
        assert!(!is_refusal(&e(S::Sasl, K::TimedOut)));
        assert!(!is_refusal(&e(S::Open, K::WouldBlock)));
        assert_eq!(
            [S::Tcp, S::Tls, S::Sasl, S::Open].map(connect_exit_code),
            [2, 4, 5, 6]
        );
    }

    /// The STOPPING=1 watcher of a call whose watchers were dropped with
    /// `shutdown` as given; returns what it told.
    fn told_by_stopping_watcher(stop_before_the_end: bool) -> Vec<String> {
        let told = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut watchers = Watchers::default();
        {
            let (told, shutdown) = (Arc::clone(&told), Arc::clone(&shutdown));
            watchers
                .spawn("test-stopping", move |done| {
                    watch_stopping(&shutdown, done, |s| told.lock().unwrap().push(s.into()));
                })
                .unwrap();
        }
        if stop_before_the_end {
            shutdown.store(true, Ordering::SeqCst);
        }
        drop(watchers);
        if !stop_before_the_end {
            // A stop after the call: the watcher looked every 100 ms.
            shutdown.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(300));
        }
        told.lock().unwrap().clone()
    }

    /// §9.2: the STOPPING=1 watcher ends with `run_with_reload`; a stop
    /// after the call (one that ended with an error or `--once`) is not
    /// told.
    #[test]
    fn amqpbridge_9_2_a_stop_after_the_run_sends_no_stopping() {
        assert!(told_by_stopping_watcher(false).is_empty());
    }

    /// §9.2: a stop that comes before the call ends is told once before the
    /// call returns, also when the watcher had not looked at it yet.
    #[test]
    fn amqpbridge_9_2_a_stop_before_the_end_of_the_run_sends_stopping_once() {
        assert_eq!(told_by_stopping_watcher(true), ["STOPPING=1"]);
    }
}
