// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Top-level server for `zerodds-mqtt-bridged`.
//!
//! Spec: `zerodds-mqtt-bridge-1.0.md` §9 (lifecycle).
//!
//! Architecture:
//! 1. Start the DCPS runtime.
//! 2. Register a reader+writer per topic.
//! 3. Connect the MQTT-5 client to the broker.
//! 4. SUBSCRIBE to all MQTT topics for `direction=in|bidir`.
//! 5. Inbound loop thread: MQTT PUBLISH → DDS writer.
//! 6. Outbound pump thread: DDS sample → MQTT PUBLISH.

use std::collections::BTreeMap;
use std::string::String;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::vec::Vec;

#[cfg(feature = "daemon")]
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
#[cfg(feature = "daemon")]
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

use super::client::{InboundEvent, MqttClient};
use super::config::{DaemonConfig, TopicConfig, parse_broker_url};
#[cfg(feature = "daemon")]
use super::runtime_common::{
    BridgeMetrics, CatalogSnapshot, SERVICE_NAME, install_signal_watcher, otlp_config_from_env,
    serve_admin_endpoints, spawn_otlp_flush_loop,
};
#[cfg(feature = "daemon")]
use super::security::{AclOp, AuthSubject, authorize, ctx_from_daemon_config};
#[cfg(feature = "daemon")]
use zerodds_monitor::Registry;
#[cfg(feature = "daemon")]
use zerodds_observability_otlp::OtlpExporter;

/// Daemon top-level error (maps to Spec §2 exit codes).
#[derive(Debug)]
pub enum ServerError {
    /// Broker-connect error (exit 2).
    BrokerConnect(String),
    /// DCPS init error (exit 3).
    Dds(String),
    /// TLS error (exit 4).
    Tls(String),
    /// Auth error (exit 5).
    Auth(String),
    /// Generic IO error.
    Io(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BrokerConnect(m) => write!(f, "broker connect: {m}"),
            Self::Dds(m) => write!(f, "dds: {m}"),
            Self::Tls(m) => write!(f, "tls: {m}"),
            Self::Auth(m) => write!(f, "auth: {m}"),
            Self::Io(m) => write!(f, "io: {m}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Daemon-Handle. Drop initiiert Shutdown.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    inbound_thread: Option<JoinHandle<()>>,
    pump_threads: Vec<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    admin_thread: Option<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    otlp_thread: Option<JoinHandle<()>>,
    /// Bound admin address for `/metrics`, `/catalog`, `/healthz`.
    #[cfg(feature = "daemon")]
    pub admin_addr: Option<String>,
    /// SIGHUP-Reload-Flag.
    #[cfg(feature = "daemon")]
    pub reload_flag: Arc<AtomicBool>,
    /// Healthz-Flag.
    #[cfg(feature = "daemon")]
    pub healthy: Arc<AtomicBool>,
    /// Standard metric set for tests.
    #[cfg(feature = "daemon")]
    pub metrics: Option<BridgeMetrics>,
    /// Bridge-internal DCPS runtime — managed by the daemon itself,
    /// but exposed for E2E tests (and telemetry inspectors) so that
    /// one can publish against the same participant without
    /// cross-participant discovery.
    #[cfg(feature = "daemon")]
    pub runtime: Arc<DcpsRuntime>,
    /// Topic name (DDS) → registered writer EntityId. Test helper for
    /// L4 inbound inspection (which EID belongs to which topic).
    #[cfg(feature = "daemon")]
    pub user_writers: BTreeMap<String, EntityId>,
    /// Topic name (DDS) → registered reader EntityId. Test helper for
    /// L3 outbound: allows synthetic sample injection via
    /// `DcpsRuntime::test_inject_user_alive` without going through the
    /// wire path (CI-stable without multicast discovery).
    #[cfg(feature = "daemon")]
    pub user_readers: BTreeMap<String, EntityId>,
}

impl DaemonHandle {
    /// Sets the stop flag and joins the workers.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        #[cfg(feature = "daemon")]
        {
            self.healthy.store(false, Ordering::SeqCst);
            if let Some(admin) = self.admin_addr.as_deref() {
                if let Ok(addr) = admin.parse::<std::net::SocketAddr>() {
                    let _ = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200));
                }
            }
        }
        if let Some(j) = self.inbound_thread.take() {
            let _ = j.join();
        }
        for j in self.pump_threads.drain(..) {
            let _ = j.join();
        }
        #[cfg(feature = "daemon")]
        {
            if let Some(j) = self.admin_thread.take() {
                let _ = j.join();
            }
            if let Some(j) = self.otlp_thread.take() {
                let _ = j.join();
            }
        }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}
/// zerodds-lint: recursion-depth 64 (start bounded by AST depth)
/// Starts the daemon with the given config.
///
/// # Errors
/// `BrokerConnect` (Spec Exit-Code 2), `Dds` (3), `Tls` (4), `Auth` (5).
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_lines)]
pub fn start(cfg: DaemonConfig) -> Result<DaemonHandle, ServerError> {
    eprintln!(
        "[zerodds-mqtt-bridged] starting domain={} broker={} topics={}",
        cfg.domain,
        cfg.broker_url,
        cfg.topics.len()
    );

    // 0. Metrics registry + standard counters (§8.2 Prometheus).
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);
    let healthy = Arc::new(AtomicBool::new(true));
    let reload_flag = Arc::new(AtomicBool::new(false));

    // 0b. Bridge-Security: Security-Ctx + TLS-Client-Config (§7.1/§7.2/§7.3).
    let (security_ctx, tls_client_cfg) =
        ctx_from_daemon_config(&cfg).map_err(|e| ServerError::Tls(format!("security: {e}")))?;
    let security_ctx = Arc::new(security_ctx);
    eprintln!(
        "[zerodds-mqtt-bridged] auth-mode={} acl-entries={} broker-tls={}",
        cfg.auth_mode,
        cfg.topic_acl.len(),
        cfg.broker_tls_enabled,
    );

    // 1. Broker-URL parsen.
    let (host, port, tls) = parse_broker_url(&cfg.broker_url)
        .map_err(|e| ServerError::BrokerConnect(format!("{e}")))?;
    if tls && tls_client_cfg.is_none() {
        // mqtts:// without tls setup → strict reject (spec §7.1).
        return Err(ServerError::Tls(
            "mqtts:// scheme requires mqtt.tls.enabled=true and ca_file (Spec §7.1)".to_string(),
        ));
    }

    // 2. DCPS-Runtime.
    let prefix = stable_prefix_for(&cfg.client_id);
    let runtime = DcpsRuntime::start(cfg.domain, prefix, RuntimeConfig::default())
        .map_err(|e| ServerError::Dds(format!("{e:?}")))?;

    // 3. Pro Topic Reader+Writer.
    let mut writers: BTreeMap<String, EntityId> = BTreeMap::new();
    let mut reader_eids: BTreeMap<String, EntityId> = BTreeMap::new();
    let mut mqtt_to_dds: BTreeMap<String, String> = BTreeMap::new();
    let mut readers: Vec<(
        String,
        String,
        std::sync::mpsc::Receiver<UserSample>,
        u8,
        bool,
    )> = Vec::new();
    for topic in &cfg.topics {
        register_topic(
            &runtime,
            topic,
            &mut writers,
            &mut reader_eids,
            &mut mqtt_to_dds,
            &mut readers,
        )?;
    }

    // 4. Connect the MQTT client — with an optional TLS wrap (§7.1).
    metrics.connections_total.inc();
    let mut client = MqttClient::connect_secure(&host, port, &cfg, tls_client_cfg.clone())
        .map_err(|e| {
            metrics.errors_total.inc();
            map_client_err(e)
        })?;
    metrics.connections_active.set(1);

    // 5. SUBSCRIBE to all in/bidir topics — an ACL read check per topic
    //    against the bridge's own subject (Spec §7.3). Topics for which
    //    the bridge subject has no read permission are not
    //    subscribed (no disclose).
    let bridge_subject = AuthSubject::new(
        cfg.auth_bearer_subject
            .as_deref()
            .unwrap_or("zerodds-mqtt-bridge"),
    );
    let mut sub_filters: Vec<(String, u8)> = Vec::new();
    for topic in &cfg.topics {
        if matches!(topic.direction.as_str(), "in" | "bidir") {
            if !authorize(
                &security_ctx.acl,
                &bridge_subject,
                AclOp::Read,
                &topic.dds_name,
            ) {
                eprintln!(
                    "[zerodds-mqtt-bridged] acl-skip-subscribe topic={} subject={}",
                    topic.dds_name, bridge_subject.name
                );
                metrics.errors_total.inc();
                continue;
            }
            let qos = if topic.mqtt_qos == 0 && topic.reliability == "reliable" {
                1
            } else {
                topic.mqtt_qos
            };
            sub_filters.push((topic.mqtt_topic.clone(), qos));
        }
    }
    client.subscribe(&sub_filters).map_err(map_client_err)?;
    eprintln!(
        "[zerodds-mqtt-bridged] subscribed to {} mqtt topic(s)",
        sub_filters.len()
    );

    let stop = Arc::new(AtomicBool::new(false));

    // We share the client over a mutex between the inbound
    // (next_event) and outbound pump (publish). That is sync enough
    // because our frames are small and the TCP stream pushes atomic writes
    // per frame with `write_all`.
    let client = Arc::new(Mutex::new(client));
    let runtime_arc = Arc::clone(&runtime);

    // 6. Outbound-Pump pro Reader-Topic.
    let mut pump_threads = Vec::new();
    let cfg_topics = Arc::new(cfg.topics.clone());
    for (dds_topic_name, mqtt_topic, rx, mqtt_qos, retain) in readers {
        let stop_c = Arc::clone(&stop);
        let client_c = Arc::clone(&client);
        let dds_topic_c = dds_topic_name.clone();
        let mqtt_topic_c = mqtt_topic.clone();
        let frames_out = Arc::clone(&metrics.frames_out_total);
        let bytes_out = Arc::clone(&metrics.bytes_out_total);
        let dds_out = Arc::clone(&metrics.dds_samples_out_total);
        let errs = Arc::clone(&metrics.errors_total);
        let security_pump = Arc::clone(&security_ctx);
        let bridge_subject_pump = bridge_subject.clone();
        let h = thread::spawn(move || {
            while !stop_c.load(Ordering::SeqCst) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(UserSample::Alive { payload, .. }) => {
                        // Spec §7.3 — ACL read check per sample that we
                        // hand out to MQTT.
                        if !authorize(
                            &security_pump.acl,
                            &bridge_subject_pump,
                            AclOp::Read,
                            &dds_topic_c,
                        ) {
                            errs.inc();
                            continue;
                        }
                        let len = payload.len() as u64;
                        if let Ok(mut c) = client_c.lock() {
                            if let Err(e) = c.publish(&mqtt_topic_c, &payload, mqtt_qos, retain) {
                                errs.inc();
                                eprintln!(
                                    "[zerodds-mqtt-bridged] publish err on {dds_topic_c}: {e}"
                                );
                            } else {
                                frames_out.inc();
                                bytes_out.add(len);
                                dds_out.inc();
                            }
                        }
                    }
                    Ok(UserSample::Lifecycle { .. }) => {
                        // Lifecycle → could be sent as a zerodds_op=dispose user
                        // property. L1-L4: no lifecycle wire.
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        pump_threads.push(h);
    }

    // 7. Inbound-Loop (MQTT → DDS).
    let stop_inbound = Arc::clone(&stop);
    let client_in = Arc::clone(&client);
    let runtime_in = Arc::clone(&runtime_arc);
    // Export a snapshot of the writer map before the inbound loop moves it
    // into an Arc. Exposed in the DaemonHandle as `user_writers`
    // (E2E test helper).
    let writers_export = writers.clone();
    let writers_arc = Arc::new(writers);
    let mqtt_to_dds_arc = Arc::new(mqtt_to_dds);
    let cfg_topics_in = Arc::clone(&cfg_topics);
    let metrics_in = metrics.clone();
    let conns_active = Arc::clone(&metrics.connections_active);
    let security_inbound = Arc::clone(&security_ctx);
    let bridge_subject_inbound = bridge_subject.clone();
    let inbound_thread = thread::spawn(move || {
        while !stop_inbound.load(Ordering::SeqCst) {
            let event = {
                let mut c = match client_in.lock() {
                    Ok(c) => c,
                    Err(_) => break,
                };
                c.next_event()
            };
            // Mutex fairness: insert a short yield-sleep after each lock
            // release. Inbound + outbound pump share the
            // `client_in` mutex; without the yield the inbound loop
            // grabs the lock again immediately (especially on macOS, where the
            // OS mutex schedules unfairly), and the pump thread hangs
            // permanently on `client_c.lock()`. Verified via the
            // `dds_publish_pumps_to_mqtt_broker` E2E test.
            thread::sleep(Duration::from_millis(1));
            match event {
                Ok(Some(InboundEvent::Publish {
                    topic,
                    payload,
                    qos: _,
                })) => {
                    metrics_in.frames_in_total.inc();
                    metrics_in.bytes_in_total.add(payload.len() as u64);
                    let dds_topic =
                        match resolve_dds_for_mqtt(&topic, &mqtt_to_dds_arc, &cfg_topics_in) {
                            Some(d) => d,
                            None => continue,
                        };
                    // Spec §7.3 — ACL write check per sample that we
                    // feed into DDS.
                    if !authorize(
                        &security_inbound.acl,
                        &bridge_subject_inbound,
                        AclOp::Write,
                        &dds_topic,
                    ) {
                        metrics_in.errors_total.inc();
                        continue;
                    }
                    if let Some(eid) = writers_arc.get(&dds_topic) {
                        match runtime_in.write_user_sample(*eid, payload) {
                            Ok(()) => {
                                metrics_in.dds_samples_in_total.inc();
                            }
                            Err(e) => {
                                metrics_in.errors_total.inc();
                                eprintln!("[zerodds-mqtt-bridged] dds write err: {e:?}");
                            }
                        }
                    }
                }
                Ok(Some(InboundEvent::Disconnected(reason))) => {
                    metrics_in.errors_total.inc();
                    conns_active.set(0);
                    eprintln!("[zerodds-mqtt-bridged] broker disconnected: {reason}");
                    break;
                }
                Ok(None) => continue,
                Err(e) => {
                    metrics_in.errors_total.inc();
                    eprintln!("[zerodds-mqtt-bridged] inbound err: {e}");
                    break;
                }
            }
        }
        conns_active.set(0);
    });

    // 8. Admin-Endpoint (§5.2 Catalog/Healthz + §8.2 Metrics).
    let mut admin_thread: Option<JoinHandle<()>> = None;
    let mut admin_addr: Option<String> = None;
    if cfg.metrics_enabled || !cfg.metrics_addr.is_empty() {
        let bind_str = if cfg.metrics_addr.is_empty() {
            "127.0.0.1:9090".to_string()
        } else {
            cfg.metrics_addr.clone()
        };
        match bind_str.parse::<std::net::SocketAddr>() {
            Ok(sock) => {
                let snap = Arc::new(CatalogSnapshot::from_config(&cfg));
                match serve_admin_endpoints(
                    sock,
                    snap,
                    Arc::clone(&registry),
                    Arc::clone(&healthy),
                    Arc::clone(&stop),
                ) {
                    Ok((h, bound)) => {
                        eprintln!(
                            "[{SERVICE_NAME}] admin endpoint on {bound} (/metrics /catalog /healthz)"
                        );
                        admin_addr = Some(bound.to_string());
                        admin_thread = Some(h);
                    }
                    Err(e) => eprintln!("[{SERVICE_NAME}] admin bind error: {e}"),
                }
            }
            Err(e) => eprintln!("[{SERVICE_NAME}] admin addr parse error: {e}"),
        }
    }

    // 9. Signal-Watcher (§9.2).
    if let Err(e) = install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag)) {
        eprintln!("[{SERVICE_NAME}] signal watcher init failed: {e}");
    }

    // 10. OTLP-Exporter (§8.3).
    let otlp_thread = if let Some(otlp_cfg) = otlp_config_from_env(SERVICE_NAME) {
        let exp = Arc::new(OtlpExporter::new(otlp_cfg));
        match spawn_otlp_flush_loop(exp, Arc::clone(&stop), Duration::from_secs(5)) {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("[{SERVICE_NAME}] OTLP spawn failed: {e}");
                None
            }
        }
    } else {
        None
    };

    Ok(DaemonHandle {
        stop,
        inbound_thread: Some(inbound_thread),
        pump_threads,
        admin_thread,
        otlp_thread,
        admin_addr,
        reload_flag,
        healthy,
        metrics: Some(metrics),
        runtime: Arc::clone(&runtime),
        user_writers: writers_export,
        user_readers: reader_eids,
    })
}

/// Spec §5.1 — fallback DDS topic name when an incoming MQTT
/// topic matches exactly one configured entry. Wildcards
/// are not expanded here; only exact match (and all other
/// topics are dropped — spec-conformant with "topics not in the config
/// are ignored").
fn resolve_dds_for_mqtt(
    mqtt_topic: &str,
    direct: &BTreeMap<String, String>,
    cfg_topics: &[TopicConfig],
) -> Option<String> {
    if let Some(d) = direct.get(mqtt_topic) {
        return Some(d.clone());
    }
    // Optional: wildcard filters can be configured per topic entry
    // — we check here for simple suffix-`#` matches.
    for t in cfg_topics {
        if let Some(prefix) = t.mqtt_topic.strip_suffix("/#") {
            if mqtt_topic.starts_with(prefix) {
                return Some(t.dds_name.clone());
            }
        }
    }
    None
}

#[cfg(feature = "daemon")]
fn register_topic(
    rt: &Arc<DcpsRuntime>,
    topic: &TopicConfig,
    writers: &mut BTreeMap<String, EntityId>,
    reader_eids: &mut BTreeMap<String, EntityId>,
    mqtt_to_dds: &mut BTreeMap<String, String>,
    readers: &mut Vec<(
        String,
        String,
        std::sync::mpsc::Receiver<UserSample>,
        u8,
        bool,
    )>,
) -> Result<(), ServerError> {
    use zerodds_qos::{
        DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
    };

    let durability = match topic.durability.as_str() {
        "transient_local" => DurabilityKind::TransientLocal,
        "transient" => DurabilityKind::Transient,
        "persistent" => DurabilityKind::Persistent,
        _ => DurabilityKind::Volatile,
    };
    let reliable = !matches!(topic.reliability.as_str(), "best_effort");
    // Spec §5.1: the daemon expects an entry with direction=`out|bidir|in`.
    let want_writer = matches!(topic.direction.as_str(), "in" | "bidir");
    let want_reader = matches!(topic.direction.as_str(), "out" | "bidir");

    if want_reader {
        let (eid, rx) = rt
            .register_user_reader(UserReaderConfig {
                topic_name: topic.dds_name.clone(),
                type_name: topic.dds_type.clone(),
                reliable,
                durability,
                deadline: DeadlineQosPolicy::default(),
                latency_budget: Default::default(),
                destination_order: Default::default(),
                liveliness: LivelinessQosPolicy::default(),
                ownership: OwnershipKind::Shared,
                presentation: Default::default(),
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_identifier: zerodds_types::TypeIdentifier::None,
                type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                data_representation_offer: None,
            })
            .map_err(|e| ServerError::Dds(format!("reader: {e:?}")))?;
        // QoS-Auto-Derive (Spec §6).
        let mqtt_qos = if topic.mqtt_qos > 0 {
            topic.mqtt_qos
        } else if reliable {
            1
        } else {
            0
        };
        let retain = topic.retain || matches!(durability, DurabilityKind::TransientLocal);
        reader_eids.insert(topic.dds_name.clone(), eid);
        readers.push((
            topic.dds_name.clone(),
            topic.mqtt_topic.clone(),
            rx,
            mqtt_qos,
            retain,
        ));
    }

    if want_writer {
        let eid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: topic.dds_name.clone(),
                type_name: topic.dds_type.clone(),
                reliable,
                durability,
                deadline: DeadlineQosPolicy::default(),
                latency_budget: Default::default(),
                destination_order: Default::default(),
                lifespan: LifespanQosPolicy::default(),
                liveliness: LivelinessQosPolicy::default(),
                ownership: OwnershipKind::Shared,
                ownership_strength: 0,
                presentation: Default::default(),
                partition: Vec::new(),
                user_data: Vec::new(),
                topic_data: Vec::new(),
                group_data: Vec::new(),
                type_identifier: zerodds_types::TypeIdentifier::None,
                data_representation_offer: None,
            })
            .map_err(|e| ServerError::Dds(format!("writer: {e:?}")))?;
        writers.insert(topic.dds_name.clone(), eid);
        mqtt_to_dds.insert(topic.mqtt_topic.clone(), topic.dds_name.clone());
    }
    Ok(())
}

fn map_client_err(e: super::client::ClientError) -> ServerError {
    match e {
        super::client::ClientError::ConnAck { reason } if reason == 0x86 || reason == 0x87 => {
            ServerError::Auth(format!("connack reason 0x{reason:02x}"))
        }
        super::client::ClientError::ConnAck { reason } => {
            ServerError::BrokerConnect(format!("connack reason 0x{reason:02x}"))
        }
        super::client::ClientError::Io(m) => ServerError::BrokerConnect(m),
        super::client::ClientError::Codec(m) => ServerError::BrokerConnect(format!("codec: {m}")),
    }
}

#[cfg(feature = "daemon")]
fn stable_prefix_for(seed: &str) -> GuidPrefix {
    let mut bytes = [0u8; 12];
    let src = seed.as_bytes();
    for (i, b) in src.iter().take(12).enumerate() {
        bytes[i] = *b;
    }
    bytes[0] ^= 0x37;
    GuidPrefix::from_bytes(bytes)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn resolve_exact_match() {
        let mut direct = BTreeMap::new();
        direct.insert("chat/message".to_string(), "Chat::Message".to_string());
        let r = resolve_dds_for_mqtt("chat/message", &direct, &[]);
        assert_eq!(r, Some("Chat::Message".to_string()));
    }

    #[test]
    fn resolve_no_match_returns_none() {
        let direct = BTreeMap::new();
        assert!(resolve_dds_for_mqtt("foo", &direct, &[]).is_none());
    }

    #[test]
    fn resolve_wildcard_suffix() {
        let direct = BTreeMap::new();
        let topics = vec![TopicConfig {
            dds_name: "Sensor".to_string(),
            mqtt_topic: "sensors/#".to_string(),
            ..Default::default()
        }];
        let r = resolve_dds_for_mqtt("sensors/temp/lab1", &direct, &topics);
        assert_eq!(r, Some("Sensor".to_string()));
    }

    #[test]
    fn map_connack_reject_to_auth_error() {
        let e = map_client_err(super::super::client::ClientError::ConnAck { reason: 0x87 });
        assert!(matches!(e, ServerError::Auth(_)));
    }

    #[test]
    fn map_connack_other_to_broker_connect() {
        let e = map_client_err(super::super::client::ClientError::ConnAck { reason: 0x80 });
        assert!(matches!(e, ServerError::BrokerConnect(_)));
    }
}
