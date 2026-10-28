// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon: ties [`crate::dds_bridge`], [`crate::address_space_build`],
//! [`crate::ua_client`] and [`crate::pubsub_glue`] into one running process.
//!
//! Spec: `docs/specs/zerodds-opcua-bridge-1.0.md` §4 + §9.
//!
//! - An `opc.tcp` accept loop serves [`zerodds_opcua_server::server::Server`]
//!   (Gateway 1.0 §9's `opc.tcp` surface) — one thread per accepted
//!   connection, all sharing one `Arc<Mutex<Server>>`.
//! - One pump thread ticks, on a fixed interval:
//!   1. every `dds_to_ua_connection` topic's DDS reader → `apply_sample`
//!      into the AddressSpace;
//!   2. every `ua_to_dds_connection` leaf's current Value, diffed against
//!      its last-seen value, → a DDS write (a third-party OPC-UA client's
//!      Write reaches DDS this way — `Server` exposes no write hook to
//!      catch it synchronously, see §7);
//!   3. every configured [`crate::ua_client::ClientJob`] (Gateway 1.0 §8);
//!   4. every configured [`crate::pubsub_glue::PublisherJob`]/
//!      [`crate::pubsub_glue::SubscriberJob`] (Part 14).

// Observability events (a decode/convert/transport error, a skipped
// topic, a connection error) go to stderr for now, same as
// `crates/coap-bridge`'s and `crates/xrce-agent`'s daemon modules — a
// structured sink is future work, not a silent drop.
#![allow(clippy::print_stderr)]

use std::net::{SocketAddr, TcpListener};
use std::string::{String, ToString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::vec::Vec;

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_monitor::{Counter, Labels, Registry};
use zerodds_observability_otlp::{OtlpConfig, OtlpExporter};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::xml::{ConnectionDirection, XmlNodeId, parse_gateway_config};
use zerodds_opcua_pubsub::transport::UdpTransport;
use zerodds_opcua_server::address_space::AddressSpace;
use zerodds_opcua_server::server::Server;
use zerodds_types::dynamic::DynamicType;

use super::config::DaemonConfig;
use super::security::{Acl, AclOp};
use crate::address_space_build::{self, TopicAddressSpace};
use crate::dds_bridge::DdsBridge;
use crate::dynamic_value::{self, FlatField};
use crate::pubsub_glue::{PublisherJob, SubscriberJob};
use crate::ua_client::{ClientJob, NodeTarget};

/// The namespace index every gateway-mapped node lives in. Vendor-defined
/// (`DomainNode::for_domain`'s own doc) — one fixed value keeps every node
/// this daemon creates in one namespace an operator's `Namespaces` array
/// entry can name once.
pub const NS: u16 = 2;

/// Poll interval for the pump loop.
const PUMP_INTERVAL: Duration = Duration::from_millis(100);

/// Daemon top-level error.
#[derive(Debug)]
pub enum ServerError {
    /// A setting that cannot be served (exit 1).
    Config(String),
    /// `opc.tcp` bind (exit 2).
    Bind(String),
    /// DDS init (exit 3).
    Dds(String),
    /// SecureChannel/certificate setup (exit 4).
    Security(String),
    /// IO.
    Io(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Config(m) => write!(f, "config: {m}"),
            Self::Bind(m) => write!(f, "bind: {m}"),
            Self::Dds(m) => write!(f, "dds: {m}"),
            Self::Security(m) => write!(f, "security: {m}"),
            Self::Io(m) => write!(f, "io: {m}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Reloads the configuration from the same layered sources the daemon
/// started with (SIGHUP).
pub type ConfigLoader = Box<dyn Fn() -> Result<DaemonConfig, String> + Send>;

/// Metrics this daemon exposes at `GET /metrics`.
#[derive(Clone)]
pub struct BridgeMetrics {
    /// Samples applied DDS → AddressSpace.
    pub dds_to_ua_samples_total: Arc<Counter>,
    /// Writes applied AddressSpace/UA-client → DDS.
    pub ua_to_dds_samples_total: Arc<Counter>,
    /// UADP NetworkMessages published.
    pub pubsub_published_total: Arc<Counter>,
    /// UADP DataSets received and written into DDS.
    pub pubsub_received_total: Arc<Counter>,
    /// Accepted `opc.tcp` connections.
    pub connections_total: Arc<Counter>,
    /// Decode/convert/transport errors, never silently dropped.
    pub errors_total: Arc<Counter>,
}

impl BridgeMetrics {
    fn register(registry: &Registry) -> Self {
        registry.set_help(
            "zerodds_opcua_dds_to_ua_samples_total",
            "Samples applied DDS to the AddressSpace",
        );
        registry.set_help(
            "zerodds_opcua_ua_to_dds_samples_total",
            "Writes applied AddressSpace/client to DDS",
        );
        registry.set_help(
            "zerodds_opcua_pubsub_published_total",
            "UADP NetworkMessages published",
        );
        registry.set_help(
            "zerodds_opcua_pubsub_received_total",
            "UADP DataSets received and written into DDS",
        );
        registry.set_help(
            "zerodds_opcua_connections_total",
            "Accepted opc.tcp connections",
        );
        registry.set_help(
            "zerodds_opcua_errors_total",
            "Decode/convert/transport errors",
        );
        Self {
            dds_to_ua_samples_total: registry
                .counter("zerodds_opcua_dds_to_ua_samples_total", Labels::new()),
            ua_to_dds_samples_total: registry
                .counter("zerodds_opcua_ua_to_dds_samples_total", Labels::new()),
            pubsub_published_total: registry
                .counter("zerodds_opcua_pubsub_published_total", Labels::new()),
            pubsub_received_total: registry
                .counter("zerodds_opcua_pubsub_received_total", Labels::new()),
            connections_total: registry.counter("zerodds_opcua_connections_total", Labels::new()),
            errors_total: registry.counter("zerodds_opcua_errors_total", Labels::new()),
        }
    }
}

/// Daemon handle. `shutdown` joins every worker.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    pump_thread: Option<JoinHandle<()>>,
    admin_thread: Option<JoinHandle<()>>,
    signal_thread: Option<JoinHandle<()>>,
    /// Ends [`install_signal_watcher`]'s thread from inside the process.
    signal_close: SignalClose,
    reload_thread: Option<JoinHandle<()>>,
    otlp_thread: Option<JoinHandle<()>>,
    otlp: Option<Arc<OtlpExporter>>,
    /// Bound `opc.tcp` address (may differ from the config bind when port 0).
    pub local_addr: SocketAddr,
    /// Bound metrics address, if `metrics.enabled`.
    pub admin_addr: Option<SocketAddr>,
    /// SIGHUP reload flag, exposed for tests.
    pub reload_flag: Arc<AtomicBool>,
    /// Metric set, exposed for tests.
    pub metrics: BridgeMetrics,
}

impl DaemonHandle {
    /// Whether SIGTERM/SIGINT (or [`Self::shutdown`]) asked the daemon to
    /// stop.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Stops taking connections/samples and joins every worker.
    ///
    /// Callable whether or not an actual SIGTERM/SIGINT was received:
    /// `install_signal_watcher`'s thread blocks on `signal_hook`'s
    /// self-pipe, and closing its `Handle` ends that wait for this daemon
    /// alone. Raising SIGTERM instead, as this did before, stopped every
    /// daemon in the process and any other SIGTERM handler of the host
    /// program (measured: 7 of 9 `daemon_e2e` tests failed when run in
    /// parallel, the first test to finish took the others' listeners down).
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        close_signal_watcher(&self.signal_close);
        // Wake the blocking accept() with a self-connect.
        if let Ok(stream) =
            std::net::TcpStream::connect_timeout(&self.local_addr, Duration::from_millis(200))
        {
            drop(stream);
        }
        for h in [
            self.accept_thread.take(),
            self.pump_thread.take(),
            self.admin_thread.take(),
            self.signal_thread.take(),
            self.reload_thread.take(),
            self.otlp_thread.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = h.join();
        }
        // A last flush after the otlp thread's own final one (joined
        // above) — covers a span [`accept_loop`] added between that
        // thread's last periodic flush and `stop` taking effect.
        if let Some(otlp) = &self.otlp {
            let _ = otlp.flush();
        }
    }
}

/// Everything the pump loop needs for one `dds_to_ua_connection` topic.
struct DdsToUaJob {
    dyn_type: DynamicType,
    type_support: Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>,
    reader: crate::dds_bridge::ReaderHandle,
    topic_space: TopicAddressSpace,
}

/// Everything the pump loop needs for one `ua_to_dds_connection` topic —
/// polled for a Value change against `last`.
struct UaToDdsJob {
    dds_topic: String,
    dyn_type: DynamicType,
    type_support: Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>,
    topic_space: TopicAddressSpace,
    last: std::collections::HashMap<String, zerodds_opcua_gateway::data_value::Variant>,
}

/// Starts the daemon. `reload` (if given) is called on SIGHUP to rebuild
/// [`DaemonConfig`] from the same sources the process started with; a
/// reload that fails to load changes nothing (logged, the running
/// configuration keeps serving). Changing `domain`, `opcua.server.bind`, or
/// any IDL/gateway-config-driven topic set needs a restart — SIGHUP only
/// re-reads `acl.*` and `metrics.*`, the same scope the other bridges'
/// SIGHUP covers.
///
/// # Errors
/// [`ServerError`] — see its variants.
pub fn start_with_reload(
    cfg: DaemonConfig,
    runtime_config: RuntimeConfig,
    reload: Option<ConfigLoader>,
) -> Result<DaemonHandle, ServerError> {
    let models =
        crate::idl::compile_all(&cfg.idl, &[]).map_err(|e| ServerError::Config(e.to_string()))?;

    let mut dds_bridge = DdsBridge::start(cfg.domain, "zerodds-opcua-bridged", runtime_config)
        .map_err(|e| ServerError::Dds(e.to_string()))?;

    let acl = Arc::new(Mutex::new(Acl::new(
        cfg.acl_default_deny,
        cfg.acl_rules.clone(),
    )));

    let mut address_space = AddressSpace::new();
    let mut dds_to_ua_jobs = Vec::new();
    let mut ua_to_dds_jobs = Vec::new();
    // Tracks which `opcua.server.historical.topics` entries were actually
    // applied to a `dds_to_ua_connection` — Spec §9.3.4.4 historizing has
    // no meaning for a `ua_to_dds_connection` leaf (the OPC UA client, not
    // DDS, is the value's source there), so an entry naming only such a
    // connection, or naming no connection at all, is unused config: a
    // load-time error, not a silent no-op.
    let mut historical_topics_applied: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    if let Some(gateway_config_path) = &cfg.gateway_config {
        let xml = std::fs::read_to_string(gateway_config_path).map_err(|e| {
            ServerError::Config(std::format!("{}: {e}", gateway_config_path.display()))
        })?;
        let gateway = parse_gateway_config(&xml).map_err(|e| ServerError::Config(e.to_string()))?;
        for bridge in &gateway.bridges {
            for connection in &bridge.connections {
                let writable = matches!(connection.direction, ConnectionDirection::UaToDds);
                let op = if writable { AclOp::Write } else { AclOp::Read };
                if !acl
                    .lock()
                    .map_err(|_| ServerError::Config("acl mutex poisoned".to_string()))?
                    .topic_exposed(&connection.dds_topic, op)
                {
                    eprintln!(
                        "[zerodds-opcua-bridged] topic `{}` not exposed by acl.rules for {op:?} — skipped",
                        connection.dds_topic
                    );
                    continue;
                }
                let dyn_type = crate::idl::find_type(&models, &connection.dds_type)
                    .map_err(|e| ServerError::Config(e.to_string()))?;
                let ddstype = crate::dynamic_type::to_dds_type(&dyn_type)
                    .map_err(|e| ServerError::Config(e.to_string()))?;
                let type_support = crate::idl::type_support_for(&models, &connection.dds_type)
                    .map_err(|e| ServerError::Config(e.to_string()))?;
                let node_id_override = connection
                    .node_id
                    .as_ref()
                    .map(|id| xml_node_id_to_ua(id, NS))
                    .transpose()
                    .map_err(|e| {
                        ServerError::Config(std::format!(
                            "topic `{}`: <node_id>: {e}",
                            connection.dds_topic
                        ))
                    })?;
                let historical_depth = if writable {
                    None
                } else {
                    let depth = cfg
                        .server
                        .historical
                        .topics
                        .iter()
                        .find(|t| t.dds_topic == connection.dds_topic)
                        .map(|t| t.depth);
                    if depth.is_some() {
                        historical_topics_applied.insert(connection.dds_topic.clone());
                    }
                    depth
                };
                let topic_space = address_space_build::materialize_instance_with_override(
                    &mut address_space,
                    NS,
                    bridge.domain_id,
                    &connection.dds_topic,
                    &ddstype,
                    writable,
                    node_id_override,
                    &connection.browse_path,
                    historical_depth,
                )
                .map_err(|e| {
                    ServerError::Config(std::format!("topic `{}`: {e}", connection.dds_topic))
                })?;
                if writable {
                    let keyed = crate::idl::is_keyed(&dyn_type);
                    dds_bridge
                        .writer(
                            &connection.dds_topic,
                            &connection.dds_type,
                            keyed,
                            false,
                            Some(Arc::clone(&type_support)),
                        )
                        .map_err(|e| ServerError::Dds(e.to_string()))?;
                    ua_to_dds_jobs.push(UaToDdsJob {
                        dds_topic: connection.dds_topic.clone(),
                        dyn_type,
                        type_support,
                        topic_space,
                        last: std::collections::HashMap::new(),
                    });
                } else {
                    let keyed = crate::idl::is_keyed(&dyn_type);
                    let reader = dds_bridge
                        .reader(
                            &connection.dds_topic,
                            &connection.dds_type,
                            keyed,
                            false,
                            Some(Arc::clone(&type_support)),
                        )
                        .map_err(|e| ServerError::Dds(e.to_string()))?;
                    dds_to_ua_jobs.push(DdsToUaJob {
                        dyn_type,
                        type_support,
                        reader,
                        topic_space,
                    });
                }
            }
        }
    }
    for topic in &cfg.server.historical.topics {
        if !historical_topics_applied.contains(&topic.dds_topic) {
            return Err(ServerError::Config(std::format!(
                "opcua.server.historical.topics: `{}` names no dds_to_ua_connection topic",
                topic.dds_topic
            )));
        }
    }

    #[cfg(feature = "crypto")]
    let server_security = build_server_security(&cfg)?;

    #[allow(unused_mut)]
    let mut server = Server::new(std::format!("opc.tcp://{}", cfg.server.bind), address_space);
    #[cfg(feature = "crypto")]
    if let Some(security) = server_security {
        server.set_security(security);
    }
    // `acl.users`/`acl.allow_anonymous` give this Server's UserStore the
    // real, distinguishable identities `acl.rules[].subject` and the
    // Write hook below actually need — without this, every session was
    // Anonymous (`Server::new`'s own default), and per-session ACL had
    // no session identity to differ on at all.
    let mut user_store = zerodds_opcua_server::identity::UserStore::anonymous_only();
    user_store.set_allow_anonymous(cfg.acl_allow_anonymous);
    for u in &cfg.acl_users {
        user_store.add_user(&u.policy_id, &u.username, &u.password);
    }
    server.set_user_store(user_store);
    // Part 4 §5.7.3.1: the Write hook runs inside `handle_service`, which
    // already knows the calling Session's identity — the per-session
    // enforcement `Acl::topic_exposed` (config-load time, kept as-is
    // above) could never do. Every `ua_to_dds_connection` leaf maps back
    // to its DDS topic once, here, rather than searching `ua_to_dds_jobs`
    // on every Write.
    let node_to_topic: std::collections::HashMap<String, String> = ua_to_dds_jobs
        .iter()
        .flat_map(|job| {
            job.topic_space
                .leaves
                .values()
                .map(move |id| (id.to_canonical_string(), job.dds_topic.clone()))
        })
        .collect();
    let write_acl = Arc::clone(&acl);
    server.set_write_hook(Box::new(move |wv, user| {
        let Some(topic) = node_to_topic.get(&wv.node_id.to_canonical_string()) else {
            // Not a DDS-mapped leaf (e.g. a server-internal node this
            // daemon never registered) — no ACL opinion; the normal
            // AccessLevel/WriteMask checks decide.
            return None;
        };
        let subject = super::security::client_subject(user);
        let allowed = write_acl
            .lock()
            .map(|a| a.allow_write(&subject, topic))
            .unwrap_or(false);
        if allowed {
            None
        } else {
            Some(zerodds_opcua_server::status::BAD_USER_ACCESS_DENIED)
        }
    }));
    let server = Arc::new(Mutex::new(server));

    let mut client_jobs = Vec::new();
    for foreign in &cfg.client.servers {
        let mut targets = Vec::new();
        for node in &foreign.nodes {
            if !acl
                .lock()
                .map_err(|_| ServerError::Config("acl mutex poisoned".to_string()))?
                .topic_exposed(&node.dds_topic, AclOp::Write)
            {
                eprintln!(
                    "[zerodds-opcua-bridged] client target topic `{}` not exposed by acl.rules for Write — skipped",
                    node.dds_topic
                );
                continue;
            }
            let node_id = parse_node_id(&node.node_id)
                .map_err(|e| ServerError::Config(std::format!("{}: {e}", node.node_id)))?;
            let dyn_type = crate::idl::find_type(&models, &node.dds_type)
                .map_err(|e| ServerError::Config(e.to_string()))?;
            let type_support = crate::idl::type_support_for(&models, &node.dds_type)
                .map_err(|e| ServerError::Config(e.to_string()))?;
            targets.push(NodeTarget {
                node_id,
                dds_topic: node.dds_topic.clone(),
                dds_type: node.dds_type.clone(),
                dyn_type,
                type_support,
                keyed: node.keyed,
                field_path: node.field_path.clone(),
            });
        }
        if targets.is_empty() {
            continue;
        }
        match ClientJob::connect(&mut dds_bridge, &foreign.endpoint_url, targets) {
            Ok(job) => client_jobs.push(job),
            Err(e) => eprintln!(
                "[zerodds-opcua-bridged] client `{}`: {e} — not connected",
                foreign.endpoint_url
            ),
        }
    }

    let mut publisher_jobs = Vec::new();
    let mut subscriber_jobs = Vec::new();
    for connection in &cfg.pubsub.connections {
        let multicast: SocketAddr = connection
            .multicast_addr
            .parse()
            .map_err(|e| ServerError::Config(std::format!("{}: {e}", connection.multicast_addr)))?;
        #[cfg(feature = "pubsub-security")]
        let pubsub_security = build_pubsub_security(&connection.security);
        let local: SocketAddr = connection
            .local_addr
            .parse()
            .map_err(|e| ServerError::Config(std::format!("{}: {e}", connection.local_addr)))?;
        for publisher in &connection.publishers {
            if !acl
                .lock()
                .map_err(|_| ServerError::Config("acl mutex poisoned".to_string()))?
                .topic_exposed(&publisher.dds_topic, AclOp::Read)
            {
                eprintln!(
                    "[zerodds-opcua-bridged] pubsub publisher topic `{}` not exposed by acl.rules for Read — skipped",
                    publisher.dds_topic
                );
                continue;
            }
            let dyn_type = crate::idl::find_type(&models, &publisher.dds_type)
                .map_err(|e| ServerError::Config(e.to_string()))?;
            let type_support = crate::idl::type_support_for(&models, &publisher.dds_type)
                .map_err(|e| ServerError::Config(e.to_string()))?;
            let transport = UdpTransport::bind(local, multicast)
                .map_err(|e| ServerError::Bind(e.to_string()))?;
            #[allow(unused_mut)]
            let mut job = PublisherJob::new(
                &mut dds_bridge,
                publisher.dds_topic.clone(),
                &publisher.dds_type,
                dyn_type,
                type_support,
                publisher.keyed,
                transport,
                connection.publisher_id,
                publisher.writer_group_id,
                publisher.dataset_writer_id,
                publisher.dataset_name.clone(),
                Duration::from_millis(u64::from(publisher.publish_interval_ms)),
            )
            .map_err(|e| ServerError::Dds(e.to_string()))?;
            #[cfg(feature = "pubsub-security")]
            if let Some((policy, sks)) = &pubsub_security {
                job.set_security(*policy, sks.clone());
            }
            publisher_jobs.push(job);
        }
        for subscriber in &connection.subscribers {
            if !acl
                .lock()
                .map_err(|_| ServerError::Config("acl mutex poisoned".to_string()))?
                .topic_exposed(&subscriber.dds_topic, AclOp::Write)
            {
                eprintln!(
                    "[zerodds-opcua-bridged] pubsub subscriber topic `{}` not exposed by acl.rules for Write — skipped",
                    subscriber.dds_topic
                );
                continue;
            }
            let dyn_type = crate::idl::find_type(&models, &subscriber.dds_type)
                .map_err(|e| ServerError::Config(e.to_string()))?;
            let type_support = crate::idl::type_support_for(&models, &subscriber.dds_type)
                .map_err(|e| ServerError::Config(e.to_string()))?;
            // The subscriber's own receive socket must bind `multicast`'s own
            // *port*, not `local`'s (`0.0.0.0:0` by default): UDP multicast
            // delivery is demultiplexed by destination (ip, port) at the
            // kernel, so a socket bound to an arbitrary ephemeral port never
            // receives a datagram sent to the group's real port — measured
            // to matter on Linux (silently "worked" on macOS, which is not a
            // portable guarantee to rely on). `local`'s own IP is kept, for
            // an operator who named a specific interface to receive on.
            // Never sends, so `destination` (`multicast`) is otherwise
            // inert — still needs a syntactically valid one, which it
            // already is.
            let subscriber_local = SocketAddr::new(local.ip(), multicast.port());
            let transport = UdpTransport::bind(subscriber_local, multicast)
                .map_err(|e| ServerError::Bind(e.to_string()))?;
            if let Ok(local_bound) = transport.local_addr() {
                if let std::net::IpAddr::V4(group) = multicast.ip() {
                    let _ = transport.join_multicast_v4(
                        group,
                        local_bound
                            .ip()
                            .to_string()
                            .parse()
                            .unwrap_or(std::net::Ipv4Addr::UNSPECIFIED),
                    );
                }
            }
            let _ = transport.set_read_timeout(Some(Duration::from_millis(50)));
            #[allow(unused_mut)]
            let mut job = SubscriberJob::new(
                &mut dds_bridge,
                subscriber.dds_topic.clone(),
                &subscriber.dds_type,
                dyn_type,
                type_support,
                subscriber.keyed,
                transport,
                subscriber.dataset_name.clone(),
                &subscriber.fields,
            )
            .map_err(|e| ServerError::Dds(e.to_string()))?;
            #[cfg(feature = "pubsub-security")]
            if let Some((policy, sks)) = &pubsub_security {
                job.set_security(*policy, sks.clone());
            }
            subscriber_jobs.push(job);
        }
    }

    let bind_addr: SocketAddr = cfg
        .server
        .bind
        .parse()
        .map_err(|e| ServerError::Bind(std::format!("{}: {e}", cfg.server.bind)))?;
    let listener = TcpListener::bind(bind_addr).map_err(|e| ServerError::Bind(e.to_string()))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| ServerError::Bind(e.to_string()))?;

    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);

    let stop = Arc::new(AtomicBool::new(false));
    let dds_bridge = Arc::new(dds_bridge);

    // `metrics.otlp_endpoint`: coarse-grained endpoint-lifecycle spans
    // (Spec-adjacent — `zerodds_foundation::tracing`'s own design line is
    // "endpoint lifecycle and discovery phases, not per-sample
    // latencies"), exported to an OTel collector. `None` when
    // unconfigured — this daemon's unchanged default.
    let otlp: Option<Arc<OtlpExporter>> = cfg
        .metrics_otlp_endpoint
        .as_deref()
        .map(|endpoint| Arc::new(OtlpExporter::new(otlp_config_from_endpoint(endpoint))));
    const OTLP_FLUSH_INTERVAL: Duration = Duration::from_secs(5);
    let otlp_thread = otlp.as_ref().map(|exp| {
        let exp = Arc::clone(exp);
        let stop = Arc::clone(&stop);
        thread::Builder::new()
            .name("zerodds-opcua-otlp".into())
            .spawn(move || {
                let mut next_flush = Instant::now() + OTLP_FLUSH_INTERVAL;
                // Polls `stop` at `PUMP_INTERVAL` granularity rather than
                // sleeping the whole `OTLP_FLUSH_INTERVAL` at once — a
                // shutdown must not wait out however much of the interval
                // remains before this thread's `join` in
                // `DaemonHandle::shutdown` returns.
                while !stop.load(Ordering::SeqCst) {
                    thread::sleep(PUMP_INTERVAL);
                    if Instant::now() >= next_flush {
                        if let Err(e) = exp.flush() {
                            eprintln!("[zerodds-opcua-bridged] otlp flush error: {e}");
                        }
                        next_flush = Instant::now() + OTLP_FLUSH_INTERVAL;
                    }
                }
                let _ = exp.flush();
            })
    });
    let otlp_thread = otlp_thread
        .transpose()
        .map_err(|e: std::io::Error| ServerError::Io(e.to_string()))?;

    let accept_thread = {
        let server = Arc::clone(&server);
        let stop = Arc::clone(&stop);
        let metrics = metrics.clone();
        let otlp = otlp.clone();
        thread::Builder::new()
            .name("zerodds-opcua-accept".into())
            .spawn(move || accept_loop(listener, server, stop, metrics, otlp))
            .map_err(|e| ServerError::Io(e.to_string()))?
    };

    let pump_thread = {
        let server = Arc::clone(&server);
        let stop = Arc::clone(&stop);
        let dds_bridge = Arc::clone(&dds_bridge);
        let metrics = metrics.clone();
        thread::Builder::new()
            .name("zerodds-opcua-pump".into())
            .spawn(move || {
                pump_loop(
                    server,
                    dds_bridge,
                    stop,
                    metrics,
                    dds_to_ua_jobs,
                    ua_to_dds_jobs,
                    client_jobs,
                    publisher_jobs,
                    subscriber_jobs,
                )
            })
            .map_err(|e| ServerError::Io(e.to_string()))?
    };

    let reload_flag = Arc::new(AtomicBool::new(false));
    let (signal_thread, signal_close) =
        install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag))
            .map_err(|e| ServerError::Io(e.to_string()))?;

    let reload_thread = match reload {
        Some(reload_fn) => {
            let stop = Arc::clone(&stop);
            let reload_flag = Arc::clone(&reload_flag);
            let acl = Arc::clone(&acl);
            let handle = thread::Builder::new()
                .name("zerodds-opcua-reload".into())
                .spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        if reload_flag.swap(false, Ordering::SeqCst) {
                            match reload_fn() {
                                Ok(new_cfg) => {
                                    if let Ok(mut guard) = acl.lock() {
                                        *guard = Acl::new(new_cfg.acl_default_deny, new_cfg.acl_rules);
                                    }
                                    eprintln!("[zerodds-opcua-bridged] configuration reloaded (acl only — see man page)");
                                }
                                Err(e) => eprintln!("[zerodds-opcua-bridged] reload failed, configuration unchanged: {e}"),
                            }
                        }
                        thread::sleep(Duration::from_millis(200));
                    }
                })
                .map_err(|e| ServerError::Io(e.to_string()))?;
            Some(handle)
        }
        None => None,
    };

    let mut admin_thread = None;
    let mut admin_addr = None;
    if cfg.metrics_enabled || !cfg.metrics_addr.is_empty() {
        let addr_str = if cfg.metrics_addr.is_empty() {
            "127.0.0.1:9095".to_string()
        } else {
            cfg.metrics_addr.clone()
        };
        match addr_str.parse::<SocketAddr>() {
            Ok(addr) => match serve_metrics(addr, Arc::clone(&registry), Arc::clone(&stop)) {
                Ok((handle, bound)) => {
                    admin_addr = Some(bound);
                    admin_thread = Some(handle);
                }
                Err(e) => eprintln!("[zerodds-opcua-bridged] metrics bind error: {e}"),
            },
            Err(e) => eprintln!("[zerodds-opcua-bridged] metrics addr parse error: {e}"),
        }
    }

    Ok(DaemonHandle {
        stop,
        accept_thread: Some(accept_thread),
        pump_thread: Some(pump_thread),
        admin_thread,
        signal_thread: Some(signal_thread),
        signal_close,
        reload_thread,
        otlp_thread,
        otlp,
        local_addr,
        admin_addr,
        reload_flag,
        metrics,
    })
}

/// Converts a DDS `INFO_TS` source timestamp (DDSI-RTPS §8.7.3: `u32`
/// seconds since the Unix epoch + `u32` nanosecond fraction) into an
/// OPC-UA `DateTime` (Part 6 §5.2.2.5: 100 ns ticks since 1601-01-01).
fn opcua_ticks_from_he_timestamp(ts: zerodds_rtps::header_extension::HeTimestamp) -> i64 {
    use zerodds_opcua_uacp::securechannel::DATETIME_TICKS_AT_UNIX_EPOCH;
    let unix_ticks = i64::from(ts.seconds)
        .saturating_mul(10_000_000)
        .saturating_add(i64::from(ts.fraction / 100));
    DATETIME_TICKS_AT_UNIX_EPOCH.saturating_add(unix_ticks)
}

/// This process's wall clock as an OPC-UA `DateTime` — used when a sample
/// carried no `INFO_TS` (`source_timestamp: None`), so distinct samples
/// still get distinct receipt times rather than all colliding on the same
/// value (which [`zerodds_opcua_gateway::historical::adapter::InMemoryHistoryCache`]
/// keys its retention on).
fn opcua_ticks_now() -> i64 {
    use zerodds_opcua_uacp::securechannel::DATETIME_TICKS_AT_UNIX_EPOCH;
    let Ok(since_epoch) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return DATETIME_TICKS_AT_UNIX_EPOCH;
    };
    let ticks = i64::try_from(since_epoch.as_nanos() / 100).unwrap_or(i64::MAX);
    DATETIME_TICKS_AT_UNIX_EPOCH.saturating_add(ticks)
}

/// Parses `metrics.otlp_endpoint` (`host:port` or `http(s)://host:port`)
/// into an [`OtlpConfig`] for this daemon's service name.
fn otlp_config_from_endpoint(raw: &str) -> OtlpConfig {
    let trimmed = raw
        .strip_prefix("http://")
        .or_else(|| raw.strip_prefix("https://"))
        .unwrap_or(raw)
        .trim_end_matches('/');
    let (host, port) = match trimmed.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(4318)),
        None => (trimmed.to_string(), 4318),
    };
    OtlpConfig {
        host,
        port,
        service_name: "zerodds-opcua-bridged".to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        timeout: Duration::from_secs(5),
    }
}

/// Builds a root, already-completed [`zerodds_foundation::tracing::Span`]
/// for one coarse-grained lifecycle event (Spec-adjacent — see
/// `zerodds_foundation::tracing`'s own "endpoint lifecycle... not
/// per-sample" design line) and adds it to `otlp`, if configured.
fn emit_lifecycle_span(
    otlp: &Option<Arc<OtlpExporter>>,
    name: &'static str,
    attributes: Vec<zerodds_foundation::observability::Attribute>,
) {
    let Some(otlp) = otlp else { return };
    use zerodds_foundation::tracing::{Span, SpanContext, SpanKind, SpanStatus};
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    otlp.add_span(Span {
        context: SpanContext::new_root(new_trace_id(), new_span_id()),
        name: name.to_string(),
        kind: SpanKind::Server,
        start_unix_ns: now_ns,
        end_unix_ns: now_ns,
        status: SpanStatus::Ok,
        status_description: None,
        attributes,
    });
}

/// A process-wide counter mixed into [`new_span_id`] — span/trace IDs only
/// need practical uniqueness for a collector to tell spans apart, not
/// cryptographic unpredictability, so no CSPRNG dependency is pulled in
/// for it.
static SPAN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn new_trace_id() -> zerodds_foundation::tracing::TraceId {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1);
    let mut bytes = nanos.to_be_bytes();
    if bytes.iter().all(|b| *b == 0) {
        bytes[15] = 1;
    }
    zerodds_foundation::tracing::TraceId(bytes)
}

fn new_span_id() -> zerodds_foundation::tracing::SpanId {
    let n = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = n ^ (u64::from(std::process::id()).rotate_left(32));
    let mut bytes = mixed.to_be_bytes();
    if bytes.iter().all(|b| *b == 0) {
        bytes[7] = 1;
    }
    zerodds_foundation::tracing::SpanId(bytes)
}

/// Builds the `(SecurityPolicy, SecurityKeyService)` pair
/// [`PublisherJob::set_security`]/[`SubscriberJob::set_security`] need
/// from `opcua.pubsub.connections[].security`, or `None` for cleartext
/// UADP (no `security:` key at all — this daemon's unchanged default).
#[cfg(feature = "pubsub-security")]
fn build_pubsub_security(
    cfg: &Option<super::config::PubSubSecurityConfig>,
) -> Option<(
    zerodds_opcua_pubsub::security::SecurityPolicy,
    zerodds_opcua_pubsub::security::SecurityKeyService,
)> {
    use super::config::PubSubSecurityPolicy;
    use zerodds_opcua_pubsub::security::{SecurityKey, SecurityKeyService, SecurityPolicy};

    let cfg = cfg.as_ref()?;
    let policy = match cfg.policy {
        PubSubSecurityPolicy::Aes128Ctr => SecurityPolicy::Aes128Ctr,
        PubSubSecurityPolicy::Aes256Ctr => SecurityPolicy::Aes256Ctr,
        PubSubSecurityPolicy::Aes256Gcm => SecurityPolicy::Aes256Gcm,
    };
    let key = SecurityKey {
        token_id: cfg.token_id,
        signing_key: cfg.signing_key.clone(),
        encrypting_key: cfg.encrypting_key.clone(),
        key_nonce: cfg.key_nonce.clone(),
    };
    Some((
        policy,
        SecurityKeyService::new(policy, cfg.security_group_id.clone(), key),
    ))
}

/// Converts a Spec §10 XML `<node_id>` (numeric or string, no namespace of
/// its own — the XML schema carries none) into this daemon's own fixed
/// namespace [`NS`].
///
/// # Errors
/// The string form's [`NodeId::string`] length bound (4096 bytes).
fn xml_node_id_to_ua(id: &XmlNodeId, ns: u16) -> Result<NodeId, String> {
    match id {
        XmlNodeId::Numeric(n) => Ok(NodeId::numeric(ns, *n)),
        XmlNodeId::StringId(s) => NodeId::string(ns, s.clone()).map_err(|e| e.to_string()),
    }
}

/// Parses the canonical `ns=N;i=NNN` / `ns=N;s=NAME` form
/// [`NodeId::to_canonical_string`] writes — the inverse it does not itself
/// provide. `g=`/`b=` (Guid/Opaque) are refused by name rather than
/// risking an under-tested byte-order guess: the Spec §10 XML schema this
/// daemon also reads (`zerodds_opcua_gateway::xml::XmlNodeId`) draws the
/// same line, supporting only Numeric/String.
///
/// # Errors
/// A string that is not `ns=<u16>;i=<u32>` or `ns=<u16>;s=<name>`.
fn parse_node_id(canonical: &str) -> Result<NodeId, String> {
    let (ns_part, rest) = canonical
        .split_once(';')
        .ok_or_else(|| std::format!("`{canonical}`: expected `ns=N;i=NNN` or `ns=N;s=NAME`"))?;
    let ns_str = ns_part
        .strip_prefix("ns=")
        .ok_or_else(|| std::format!("`{canonical}`: expected a leading `ns=`"))?;
    let namespace: u16 = ns_str
        .parse()
        .map_err(|_| std::format!("`{canonical}`: `{ns_str}` is not a u16 namespace index"))?;
    if let Some(numeric) = rest.strip_prefix("i=") {
        let id: u32 = numeric
            .parse()
            .map_err(|_| std::format!("`{canonical}`: `{numeric}` is not a u32 identifier"))?;
        return Ok(NodeId::numeric(namespace, id));
    }
    if let Some(name) = rest.strip_prefix("s=") {
        return NodeId::string(namespace, name).map_err(|e| std::format!("`{canonical}`: {e}"));
    }
    Err(std::format!(
        "`{canonical}`: only `i=`/`s=` identifiers are supported by this daemon's config parser (not `g=`/`b=`)"
    ))
}

/// What ends the signal watcher's wait from inside the process.
#[cfg(unix)]
type SignalClose = signal_hook::iterator::Handle;
/// What ends the signal watcher's wait from inside the process.
#[cfg(not(unix))]
type SignalClose = ();

#[cfg(unix)]
fn close_signal_watcher(close: &SignalClose) {
    close.close();
}

#[cfg(not(unix))]
fn close_signal_watcher(_close: &SignalClose) {}

#[cfg(unix)]
fn install_signal_watcher(
    stop: Arc<AtomicBool>,
    reload: Arc<AtomicBool>,
) -> std::io::Result<(JoinHandle<()>, SignalClose)> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;
    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP])?;
    let close = signals.handle();
    let thread = thread::Builder::new()
        .name("zerodds-opcua-signals".into())
        .spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGTERM | SIGINT => {
                        stop.store(true, Ordering::SeqCst);
                        break;
                    }
                    SIGHUP => reload.store(true, Ordering::SeqCst),
                    _ => {}
                }
            }
        })?;
    Ok((thread, close))
}

#[cfg(not(unix))]
fn install_signal_watcher(
    _stop: Arc<AtomicBool>,
    _reload: Arc<AtomicBool>,
) -> std::io::Result<(JoinHandle<()>, SignalClose)> {
    let thread = thread::Builder::new()
        .name("zerodds-opcua-signals-noop".into())
        .spawn(|| {})?;
    Ok((thread, ()))
}

/// A minimal `GET /metrics` HTTP server — not
/// [`zerodds_monitor::server::serve_prometheus`], whose accept loop has no
/// way to stop: it blocks on `TcpListener::incoming()` with no stop-flag
/// check and no externally reachable listener handle to drop (its own doc
/// comment's "shutdown via dropping the TcpListener" describes an API this
/// function does not actually expose — measured directly: an
/// `opcua-bridged` process built against it never rejoined `admin_thread`
/// after SIGTERM). This one uses a non-blocking accept polled against
/// `stop`, the same primitive [`accept_loop`]'s own wake-on-shutdown uses
/// for the `opc.tcp` listener, so `DaemonHandle::shutdown` actually
/// terminates.
fn serve_metrics(
    addr: SocketAddr,
    registry: Arc<Registry>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    let bound = listener.local_addr()?;
    let handle = thread::Builder::new()
        .name("zerodds-opcua-metrics".into())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => handle_metrics_connection(&mut stream, &registry),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50))
                    }
                    Err(_) => break,
                }
            }
        })?;
    Ok((handle, bound))
}

fn handle_metrics_connection(stream: &mut std::net::TcpStream, registry: &Registry) {
    use std::io::{Read, Write};
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0u8; 1024];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let request_line = std::str::from_utf8(&buf[..n])
        .ok()
        .and_then(|s| s.lines().next())
        .unwrap_or("");
    let body = if request_line.starts_with("GET /metrics") {
        registry.render_prometheus()
    } else {
        String::new()
    };
    let status = if body.is_empty() {
        "404 Not Found"
    } else {
        "200 OK"
    };
    let response = std::format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn accept_loop(
    listener: TcpListener,
    server: Arc<Mutex<Server>>,
    stop: Arc<AtomicBool>,
    metrics: BridgeMetrics,
    otlp: Option<Arc<OtlpExporter>>,
) {
    let _ = listener.set_nonblocking(false);
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                if stop.load(Ordering::SeqCst) {
                    break; // the shutdown self-connect — do not serve it.
                }
                metrics.connections_total.inc();
                emit_lifecycle_span(
                    &otlp,
                    "zerodds-opcua-bridged.opc_tcp.accept",
                    Vec::from([zerodds_foundation::observability::Attribute {
                        key: "peer.addr",
                        value: peer.to_string(),
                    }]),
                );
                let server = Arc::clone(&server);
                let _ = thread::Builder::new()
                    .name("zerodds-opcua-conn".into())
                    .spawn(move || serve_connection_locked(&server, stream));
            }
            Err(e) => {
                eprintln!("[zerodds-opcua-bridged] accept error: {e}");
                break;
            }
        }
    }
}

/// As [`zerodds_opcua_server::server`]'s own `serve_connection` (there is no
/// direct equivalent exposed for a `Server` several connections share), but
/// locking `server` per message rather than for the connection's whole
/// lifetime — required here since every accepted connection shares one
/// `Server` (Part 4 §5.6.2 allows several concurrent SecureChannels; a
/// Subscription can be transferred between them, so they cannot each own a
/// private `Server`).
fn serve_connection_locked(server: &Arc<Mutex<Server>>, mut stream: std::net::TcpStream) {
    use std::io::{Read, Write};
    loop {
        let mut header = [0u8; 8];
        if stream.read_exact(&mut header).is_err() {
            return;
        }
        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if !(8..=16 * 1024 * 1024).contains(&size) {
            return;
        }
        let mut rest = vec![0u8; size - 8];
        if stream.read_exact(&mut rest).is_err() {
            return;
        }
        let mut message = header.to_vec();
        message.extend_from_slice(&rest);
        let response = {
            let Ok(mut guard) = server.lock() else { return };
            guard.process(&message)
        };
        match response {
            Ok(Some(bytes)) => {
                if stream.write_all(&bytes).is_err() {
                    return;
                }
            }
            Ok(None) => return, // CloseSecureChannel — Part 6 §6.7.2.5.
            Err(e) => {
                eprintln!("[zerodds-opcua-bridged] connection error: {e}");
                return;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pump_loop(
    server: Arc<Mutex<Server>>,
    dds_bridge: Arc<DdsBridge>,
    stop: Arc<AtomicBool>,
    metrics: BridgeMetrics,
    dds_to_ua_jobs: Vec<DdsToUaJob>,
    mut ua_to_dds_jobs: Vec<UaToDdsJob>,
    mut client_jobs: Vec<ClientJob>,
    mut publisher_jobs: Vec<PublisherJob>,
    mut subscriber_jobs: Vec<SubscriberJob>,
) {
    let mut replaced_reported: Vec<u64> = vec![0; dds_to_ua_jobs.len()];
    while !stop.load(Ordering::SeqCst) {
        for (i, job) in dds_to_ua_jobs.iter().enumerate() {
            let replaced = job.reader.replaced();
            if replaced > replaced_reported[i] {
                eprintln!(
                    "[zerodds-opcua-bridged] reader {:?}: {} sample(s) gave way to newer ones \
                     at the {}-sample buffer bound before they were mapped ({replaced} so far)",
                    job.reader.entity_id(),
                    replaced - replaced_reported[i],
                    crate::dds_bridge::READER_BUFFER_CAP
                );
                replaced_reported[i] = replaced;
            }
            for sample in job.reader.poll() {
                match job.type_support.read_body(
                    &sample.body,
                    sample.representation,
                    sample.big_endian,
                ) {
                    Ok(decoded) => match dynamic_value::flatten(&job.dyn_type, decoded.data()) {
                        Ok((fields, skipped)) => {
                            for skip in &skipped {
                                eprintln!("[zerodds-opcua-bridged] {skip}");
                            }
                            let source_timestamp = sample
                                .source_timestamp
                                .map(opcua_ticks_from_he_timestamp)
                                .unwrap_or_else(opcua_ticks_now);
                            if let Ok(mut guard) = server.lock() {
                                let unmatched = address_space_build::apply_sample(
                                    guard.address_space_mut(),
                                    &job.topic_space,
                                    &fields,
                                    source_timestamp,
                                );
                                for path in unmatched {
                                    eprintln!(
                                        "[zerodds-opcua-bridged] sample field `{path}` matches no AddressSpace leaf"
                                    );
                                }
                            }
                            metrics.dds_to_ua_samples_total.inc();
                        }
                        Err(e) => {
                            eprintln!("[zerodds-opcua-bridged] {e}");
                            metrics.errors_total.inc();
                        }
                    },
                    Err(e) => {
                        eprintln!("[zerodds-opcua-bridged] decode: {e}");
                        metrics.errors_total.inc();
                    }
                }
            }
        }

        for job in &mut ua_to_dds_jobs {
            let changed: Vec<FlatField> = {
                let Ok(mut guard) = server.lock() else {
                    continue;
                };
                job.topic_space
                    .leaves
                    .iter()
                    .filter_map(|(path, node_id)| {
                        let value = guard.address_space_mut().value(node_id)?.value.clone()?;
                        if job.last.get(path) == Some(&value) {
                            return None;
                        }
                        job.last.insert(path.clone(), value.clone());
                        Some(FlatField {
                            path: path.clone(),
                            value,
                        })
                    })
                    .collect()
            };
            if changed.is_empty() {
                continue;
            }
            let default = match dynamic_value::default_data(&job.dyn_type) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("[zerodds-opcua-bridged] {e}");
                    metrics.errors_total.inc();
                    continue;
                }
            };
            match dynamic_value::unflatten(&job.dyn_type, &changed, &default) {
                Ok(data) => match job.type_support.from_dynamic_data(data) {
                    Ok(sample) => match job.type_support.write_body(&sample, 2, false) {
                        Ok(body) => match dds_bridge.write(&job.dds_topic, &body) {
                            Ok(()) => metrics.ua_to_dds_samples_total.inc(),
                            Err(e) => {
                                eprintln!("[zerodds-opcua-bridged] {e}");
                                metrics.errors_total.inc();
                            }
                        },
                        Err(e) => {
                            eprintln!("[zerodds-opcua-bridged] {e}");
                            metrics.errors_total.inc();
                        }
                    },
                    Err(e) => {
                        eprintln!("[zerodds-opcua-bridged] {e}");
                        metrics.errors_total.inc();
                    }
                },
                Err(e) => {
                    eprintln!("[zerodds-opcua-bridged] {e}");
                    metrics.errors_total.inc();
                }
            }
        }

        for job in &mut client_jobs {
            match job.tick(&dds_bridge) {
                Ok(n) => {
                    if n > 0 {
                        metrics.ua_to_dds_samples_total.add(n as u64);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[zerodds-opcua-bridged] client `{}`: {e}",
                        job.endpoint_url()
                    );
                    metrics.errors_total.inc();
                }
            }
        }

        let now = Instant::now();
        for job in &mut publisher_jobs {
            let dds_topic = job.dds_topic().to_string();
            match job.tick(now, &mut |topic, skip| {
                eprintln!("[zerodds-opcua-bridged] pubsub publisher `{topic}`: {skip}")
            }) {
                Ok((_, published)) => {
                    if published {
                        metrics.pubsub_published_total.inc();
                    }
                }
                Err(e) => {
                    eprintln!("[zerodds-opcua-bridged] pubsub publisher `{dds_topic}`: {e}");
                    metrics.errors_total.inc();
                }
            }
        }
        for job in &mut subscriber_jobs {
            match job.tick(&dds_bridge) {
                Ok(n) => {
                    if n > 0 {
                        metrics.pubsub_received_total.add(n as u64);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[zerodds-opcua-bridged] pubsub subscriber `{}`: {e}",
                        job.dds_topic()
                    );
                    metrics.errors_total.inc();
                }
            }
        }

        thread::sleep(PUMP_INTERVAL);
    }
}

#[cfg(feature = "crypto")]
fn build_server_security(
    cfg: &DaemonConfig,
) -> Result<Option<zerodds_opcua_server::server::ServerSecurity>, ServerError> {
    use zerodds_opcua_uacp::crypto::SecurityPolicy;

    let secured: Vec<&str> = cfg
        .server
        .security_policies
        .iter()
        .map(String::as_str)
        .filter(|p| *p != "none")
        .collect();
    let Some(policy_name) = secured.first() else {
        return Ok(None);
    };
    let policy = match *policy_name {
        "basic256sha256" => SecurityPolicy::Basic256Sha256,
        "aes128_sha256_rsaoaep" => SecurityPolicy::Aes128Sha256RsaOaep,
        "aes256_sha256_rsapss" => SecurityPolicy::Aes256Sha256RsaPss,
        other => {
            return Err(ServerError::Security(std::format!(
                "unknown SecurityPolicy `{other}`"
            )));
        }
    };
    let (Some(cert_path), Some(key_path)) =
        (&cfg.server.certificate_der, &cfg.server.private_key_pem)
    else {
        return Err(ServerError::Security(
            "a secured security_policy needs certificate_der and private_key_pem".to_string(),
        ));
    };
    let (Some(client_cert_path), Some(client_key_path)) = (
        &cfg.server.trusted_client_certificate_der,
        &cfg.server.trusted_client_public_key_pem,
    ) else {
        return Err(ServerError::Security(
            "a secured security_policy needs trusted_client_certificate_der and trusted_client_public_key_pem".to_string(),
        ));
    };
    let certificate =
        super::security::load_der(cert_path).map_err(|e| ServerError::Security(e.to_string()))?;
    let private_key = super::security::load_private_key_pem(key_path)
        .map_err(|e| ServerError::Security(e.to_string()))?;
    let client_certificate = super::security::load_der(client_cert_path)
        .map_err(|e| ServerError::Security(e.to_string()))?;
    let client_public_key = super::security::load_public_key_pem(client_key_path)
        .map_err(|e| ServerError::Security(e.to_string()))?;
    Ok(Some(zerodds_opcua_server::server::ServerSecurity {
        policy,
        mode: zerodds_opcua_uacp::crypto::SecuredMode::SignAndEncrypt,
        private_key,
        certificate,
        client_certificate,
        client_public_key,
        // `rand::rngs::OsRng` (rand 0.8) implements `rand_core` 0.6's
        // `RngCore + CryptoRng`, which is exactly what `rsa` 0.9's
        // `CryptoRngCore` (re-exported as `zerodds_opcua_uacp::crypto::
        // CryptoRngCore`) blanket-implements over — the same OS CSPRNG
        // `crates/opcua-server`'s own secured-handshake test uses.
        rng: Box::new(rand::rngs::OsRng),
    }))
}
