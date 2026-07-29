// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket server + DDS pump for the `zerodds-ws-bridged` daemon.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §4 + §9.
//!
//! `eprintln!` logging in the daemon path: Spec §8.1 leaves
//! structured logging to the daemon. Until a workspace tracing stack
//! is wired in, `eprintln` is the sink; marked locally as a clippy-allow
//! on the affected functions.
//!
//! Sync, blocking I/O on `std::net`. One reader thread per connection
//! (TCP→WS frames→router) plus one writer thread
//! (router channel→WS frames→TCP). The DDS pump thread consumes
//! `mpsc::Receiver<UserSample>` from each registered reader and
//! dispatches via the `Router`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::codec::{decode, encode};
use crate::frame::{Frame, Opcode};
use crate::handshake::{build_server_response, parse_client_request, render_server_response};

use super::config::{DaemonConfig, TopicConfig};
use super::router::{Router, RouterMsg};
#[cfg(feature = "daemon")]
use super::runtime_common::{
    BridgeMetrics, CatalogSnapshot, SERVICE_NAME, install_signal_watcher, otlp_config_from_env,
    serve_admin_endpoints, spawn_otlp_flush_loop,
};
#[cfg(feature = "daemon")]
use super::security::{
    AclOp, AuthSubject, SecurityCtx, authenticate_ws, authorize, ctx_from_daemon_config,
    extract_authorization_header, serve_tls_handshake,
};
#[cfg(feature = "daemon")]
use rustls::{ServerConnection, StreamOwned};
#[cfg(feature = "daemon")]
use zerodds_monitor::Registry;
#[cfg(feature = "daemon")]
use zerodds_observability_otlp::OtlpExporter;

#[cfg(feature = "daemon")]
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
#[cfg(feature = "daemon")]
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

/// Top-level error of the daemon.
#[derive(Debug)]
pub enum ServerError {
    /// Bind error (exit code 2).
    Bind(String),
    /// DCPS init error (exit code 3).
    Dds(String),
    /// I/O error during operation.
    Io(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bind(m) => write!(f, "bind error: {m}"),
            Self::Dds(m) => write!(f, "dds error: {m}"),
            Self::Io(m) => write!(f, "io error: {m}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Daemon handle. On drop, shutdown is invoked.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    pump_threads: Vec<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    admin_thread: Option<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    otlp_thread: Option<JoinHandle<()>>,
    router: Arc<Mutex<Router>>,
    /// Bound address (may differ from the configured listen if port=0).
    pub local_addr: String,
    /// Bound admin address (Prometheus + catalog + healthz).
    #[cfg(feature = "daemon")]
    pub admin_addr: Option<String>,
    /// Lifecycle: SIGHUP sets this; the server loop can react.
    #[cfg(feature = "daemon")]
    pub reload_flag: Arc<AtomicBool>,
    /// Healthz flag — DCPS runtime up == true.
    #[cfg(feature = "daemon")]
    pub healthy: Arc<AtomicBool>,
    /// Metric set for the §8.2 wireup. Reader-side for tests.
    #[cfg(feature = "daemon")]
    pub metrics: Option<BridgeMetrics>,
    /// Bridge-internal DCPS runtime — exported for E2E tests that
    /// synthetically inject samples into the daemon reader channel
    /// (via `DcpsRuntime::test_inject_user_alive`) without going through
    /// the wire path.
    #[cfg(feature = "daemon")]
    pub runtime: Arc<DcpsRuntime>,
    /// Topic name → registered writer EntityId.
    #[cfg(feature = "daemon")]
    pub user_writers: std::collections::BTreeMap<String, EntityId>,
    /// Topic name → registered reader EntityId.
    #[cfg(feature = "daemon")]
    pub user_readers: std::collections::BTreeMap<String, EntityId>,
}

impl DaemonHandle {
    /// Initiates a graceful shutdown.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        #[cfg(feature = "daemon")]
        {
            self.healthy.store(false, Ordering::SeqCst);
        }
        // Unblock accept().
        if let Ok(addr) = self.local_addr.parse::<std::net::SocketAddr>() {
            // Self-connect to wake accept().
            let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
        }
        // Self-connect the admin server (so the accept loop unblocks).
        #[cfg(feature = "daemon")]
        if let Some(admin) = self.admin_addr.as_deref() {
            if let Ok(addr) = admin.parse::<std::net::SocketAddr>() {
                let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
            }
        }
        if let Ok(r) = self.router.lock() {
            r.broadcast_shutdown();
        }
        if let Some(j) = self.accept_thread.take() {
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
/// Starts the daemon with the given config. Does NOT block —
/// returns a `DaemonHandle` through which the caller (either
/// the binary or an E2E test) controls the lifecycle.
///
/// # Errors
/// `Bind` if the TCP listener cannot bind (Spec exit code 2).
/// `Dds` if the `DcpsRuntime` cannot start (Spec exit code 3).
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_lines)]
pub fn start(cfg: DaemonConfig) -> Result<DaemonHandle, ServerError> {
    eprintln!(
        "[zerodds-ws-bridged] starting on {} domain={} topics={}",
        cfg.listen,
        cfg.domain,
        cfg.topics.len()
    );

    // 0. Metrics registry + standard counter set (§8.2).
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);

    // 0b. Bridge security: security ctx + optional RotatingTlsConfig for
    //     SIGHUP hot-reload (§7.1 TLS / §7.2 auth / §7.3 ACL).
    let (security_ctx, rotating_tls) = ctx_from_daemon_config(&cfg)
        .map_err(|e| ServerError::Bind(alloc_format(format_args!("security: {e}"))))?;
    let security_ctx = Arc::new(security_ctx);
    let rotating_tls = rotating_tls.map(Arc::new);
    if rotating_tls.is_some() {
        eprintln!(
            "[zerodds-ws-bridged] TLS active (cert={}, mtls={})",
            cfg.tls_cert_file,
            !cfg.tls_client_ca_file.is_empty(),
        );
    }
    eprintln!(
        "[zerodds-ws-bridged] auth-mode={} acl-entries={}",
        cfg.auth_mode,
        cfg.topic_acl.len()
    );

    // 1. Bring up the DCPS runtime.
    let prefix = stable_prefix_for(&cfg.listen);
    let runtime = DcpsRuntime::start(cfg.domain, prefix, RuntimeConfig::default())
        .map_err(|e| ServerError::Dds(alloc_format(format_args!("{e:?}"))))?;
    let healthy = Arc::new(AtomicBool::new(true));

    // 2. Register a reader+writer per topic.
    let mut writers: std::collections::BTreeMap<String, EntityId> =
        std::collections::BTreeMap::new();
    let mut reader_eids: std::collections::BTreeMap<String, EntityId> =
        std::collections::BTreeMap::new();
    let mut readers: Vec<(String, std::sync::mpsc::Receiver<UserSample>)> = Vec::new();
    for topic in &cfg.topics {
        let (reader_eid, writer_eid) = register_topic_endpoints(&runtime, topic)?;
        if let Some((eid, rx)) = reader_eid {
            reader_eids.insert(topic.name.clone(), eid);
            readers.push((topic.name.clone(), rx));
        }
        if let Some(eid) = writer_eid {
            writers.insert(topic.name.clone(), eid);
        }
    }

    // 3. Router + TCP listener.
    let router = Arc::new(Mutex::new(Router::new()));
    let listener = TcpListener::bind(&cfg.listen)
        .map_err(|e| ServerError::Bind(alloc_format(format_args!("{e}"))))?;
    let local_addr = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| cfg.listen.clone());
    listener
        .set_nonblocking(false)
        .map_err(|e| ServerError::Io(alloc_format(format_args!("{e}"))))?;

    eprintln!("[zerodds-ws-bridged] bound on {local_addr}");

    let stop = Arc::new(AtomicBool::new(false));
    let reload_flag = Arc::new(AtomicBool::new(false));

    // 4. Pump threads per reader.
    let mut pump_threads = Vec::new();
    for (topic_name, rx) in readers {
        let router_c = Arc::clone(&router);
        let stop_c = Arc::clone(&stop);
        let topic_name_c = topic_name.clone();
        let dds_out = Arc::clone(&metrics.dds_samples_out_total);
        let h = thread::spawn(move || {
            while !stop_c.load(Ordering::SeqCst) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(UserSample::Alive {
                        payload,
                        big_endian,
                        ..
                    }) => {
                        dds_out.inc();
                        if let Ok(mut r) = router_c.lock() {
                            r.dispatch(&topic_name_c, payload.to_vec(), big_endian);
                        }
                    }
                    Ok(UserSample::Lifecycle { .. }) => {
                        // Lifecycle events: we could push dispose frames —
                        // for the L1-L4 wireup the alive path is enough.
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        pump_threads.push(h);
    }

    // 5. Accept thread.
    let next_conn_id = Arc::new(AtomicU64::new(1));
    let stop_acc = Arc::clone(&stop);
    let router_acc = Arc::clone(&router);
    // Snapshot of the writer map for the DaemonHandle export, before `writers`
    // is moved into the Arc (accepted by the dispatch path).
    let writers_export = writers.clone();
    let writers_arc = Arc::new(writers);
    let runtime_acc = Arc::clone(&runtime);
    let topics_arc = Arc::new(cfg.topics.clone());
    let metrics_acc = metrics.clone();
    let security_acc = Arc::clone(&security_ctx);
    let rotating_acc = rotating_tls.clone();

    let accept_thread = thread::spawn(move || {
        for incoming in listener.incoming() {
            if stop_acc.load(Ordering::SeqCst) {
                break;
            }
            match incoming {
                Ok(tcp) => {
                    let conn_id = next_conn_id.fetch_add(1, Ordering::SeqCst);
                    let router_h = Arc::clone(&router_acc);
                    let writers_h = Arc::clone(&writers_arc);
                    let runtime_h = Arc::clone(&runtime_acc);
                    let stop_h = Arc::clone(&stop_acc);
                    let topics_h = Arc::clone(&topics_arc);
                    let metrics_h = metrics_acc.clone();
                    let security_h = Arc::clone(&security_acc);
                    let rot_h = rotating_acc.clone();
                    thread::spawn(move || {
                        // If TLS is configured: rustls handshake. Otherwise plain.
                        let (stream, mtls_subj) = if let Some(rot) = rot_h.as_ref() {
                            let cfg = rot.current();
                            match serve_tls_handshake(cfg, tcp, Duration::from_secs(5)) {
                                Ok((tcp, conn, subj)) => {
                                    (WsStream::Tls(Box::new(StreamOwned::new(conn, tcp))), subj)
                                }
                                Err(e) => {
                                    metrics_h.errors_total.inc();
                                    eprintln!(
                                        "[zerodds-ws-bridged] tls handshake err conn={conn_id}: {e}"
                                    );
                                    return;
                                }
                            }
                        } else {
                            (WsStream::Plain(tcp), None)
                        };
                        let _ = serve_connection(
                            conn_id, stream, mtls_subj, router_h, writers_h, runtime_h, stop_h,
                            topics_h, metrics_h, security_h,
                        );
                    });
                }
                Err(e) => {
                    eprintln!("[zerodds-ws-bridged] accept error: {e}");
                    continue;
                }
            }
        }
    });

    // 6. Admin endpoint (§8.2 Prometheus + §5.2 catalog/healthz).
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
                    Err(e) => {
                        eprintln!("[{SERVICE_NAME}] admin bind error: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("[{SERVICE_NAME}] admin addr parse error: {e}");
            }
        }
    }

    // 7. Signal watcher (§9.2 graceful shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag)) {
        eprintln!("[{SERVICE_NAME}] signal watcher init failed: {e}");
    }

    // 7b. Bridge security: SIGHUP hook for TLS cert hot-reload. Polls the
    //     reload_flag and calls RotatingTlsConfig::reload().
    if let Some(rot) = rotating_tls.clone() {
        let stop_r = Arc::clone(&stop);
        let reload_r = Arc::clone(&reload_flag);
        thread::Builder::new()
            .name("zerodds-ws-tls-reload".into())
            .spawn(move || {
                while !stop_r.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(250));
                    if reload_r.swap(false, Ordering::SeqCst) {
                        match rot.reload() {
                            Ok(()) => eprintln!(
                                "[{SERVICE_NAME}] SIGHUP TLS-cert reloaded"
                            ),
                            Err(e) => eprintln!(
                                "[{SERVICE_NAME}] SIGHUP TLS-cert reload FAILED: {e} (keeping old cert)"
                            ),
                        }
                    }
                }
            })
            .ok();
    }

    // 8. OTLP span exporter (§8.3) if the env var is set.
    let otlp_thread = if let Some(otlp_cfg) = otlp_config_from_env(SERVICE_NAME) {
        let exporter = Arc::new(OtlpExporter::new(otlp_cfg));
        match spawn_otlp_flush_loop(exporter, Arc::clone(&stop), Duration::from_secs(5)) {
            Ok(h) => {
                eprintln!("[{SERVICE_NAME}] OTLP exporter active");
                Some(h)
            }
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
        accept_thread: Some(accept_thread),
        pump_threads,
        admin_thread,
        otlp_thread,
        router,
        local_addr,
        admin_addr,
        reload_flag,
        healthy,
        metrics: Some(metrics),
        runtime: Arc::clone(&runtime),
        user_writers: writers_export,
        user_readers: reader_eids,
    })
}

#[cfg(feature = "daemon")]
type ReaderEndpoint = (EntityId, std::sync::mpsc::Receiver<UserSample>);
#[cfg(feature = "daemon")]
type TopicEndpoints = (Option<ReaderEndpoint>, Option<EntityId>);

#[cfg(feature = "daemon")]
fn register_topic_endpoints(
    rt: &Arc<DcpsRuntime>,
    topic: &TopicConfig,
) -> Result<TopicEndpoints, ServerError> {
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
    let want_reader =
        matches!(topic.direction.as_str(), "in" | "bidir") || topic.direction.is_empty();
    let want_writer =
        matches!(topic.direction.as_str(), "out" | "bidir") || topic.direction.is_empty();

    let reader = if want_reader {
        let (eid, rx) = rt
            .register_user_reader(UserReaderConfig {
                topic_name: topic.name.clone(),
                type_name: if topic.type_name.is_empty() {
                    topic.name.clone()
                } else {
                    topic.type_name.clone()
                },
                reliable,
                durability,
                deadline: DeadlineQosPolicy::default(),
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
            .map_err(|e| ServerError::Dds(alloc_format(format_args!("reader: {e:?}"))))?;
        Some((eid, rx))
    } else {
        None
    };

    let writer = if want_writer {
        let eid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: topic.name.clone(),
                type_name: if topic.type_name.is_empty() {
                    topic.name.clone()
                } else {
                    topic.type_name.clone()
                },
                reliable,
                durability,
                deadline: DeadlineQosPolicy::default(),
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
            .map_err(|e| ServerError::Dds(alloc_format(format_args!("writer: {e:?}"))))?;
        Some(eid)
    } else {
        None
    };

    Ok((reader, writer))
}

#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn serve_connection(
    conn_id: u64,
    mut stream: WsStream,
    mtls_subject: Option<AuthSubject>,
    router: Arc<Mutex<Router>>,
    writers: Arc<std::collections::BTreeMap<String, EntityId>>,
    runtime: Arc<DcpsRuntime>,
    stop: Arc<AtomicBool>,
    topics: Arc<Vec<TopicConfig>>,
    metrics: BridgeMetrics,
    security: Arc<SecurityCtx>,
) -> Result<(), ServerError> {
    metrics.connections_total.inc();
    metrics.connections_active.inc();
    let conn_guard = ConnectionLifetime {
        active: Arc::clone(&metrics.connections_active),
    };
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();

    // 1. Read in the HTTP upgrade handshake.
    let mut buf = [0u8; 4096];
    let mut accumulated = Vec::new();
    let req_str = loop {
        match stream.read(&mut buf) {
            Ok(0) => return Err(ServerError::Io("eof during handshake".to_string())),
            Ok(n) => {
                accumulated.extend_from_slice(&buf[..n]);
                if accumulated.windows(4).any(|w| w == b"\r\n\r\n") {
                    let s = String::from_utf8_lossy(&accumulated).to_string();
                    break s;
                }
                if accumulated.len() > 64 * 1024 {
                    return Err(ServerError::Io("handshake too large".to_string()));
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(ServerError::Io(e.to_string())),
        }
    };

    let req = match parse_client_request(&req_str) {
        Ok(r) => r,
        Err(e) => {
            let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
            return Err(ServerError::Io(alloc_format(format_args!(
                "handshake parse: {e:?}"
            ))));
        }
    };

    // §7.2 — authentication. On reject: HTTP 401 + close.
    let auth_header = extract_authorization_header(&req_str);
    let auth_headers: Vec<(String, String)> = if let Some(v) = auth_header {
        vec![("authorization".to_string(), v)]
    } else {
        Vec::new()
    };
    let subject = match authenticate_ws(&security.auth, &auth_headers, mtls_subject.clone()) {
        Ok(s) => s,
        Err(e) => {
            metrics.errors_total.inc();
            let body = b"unauthorized";
            let resp = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nWWW-Authenticate: Bearer realm=\"zerodds-ws\"\r\nConnection: close\r\n\r\nunauthorized",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            eprintln!("[zerodds-ws-bridged] auth reject conn={conn_id} reason={e}");
            return Err(ServerError::Io(alloc_format(format_args!(
                "auth reject: {e}"
            ))));
        }
    };

    // Auto-subscribe if the path matches a topic (Spec §4.2).
    let mut auto_topic: Option<String> = None;
    for t in topics.iter() {
        if t.ws_path == req.path || super::config::default_ws_path(&t.name) == req.path {
            auto_topic = Some(t.name.clone());
            break;
        }
    }

    // §7.3 — auto-subscribe topic ACL check (read).
    if let Some(topic) = &auto_topic {
        if !authorize(&security.acl, &subject, AclOp::Read, topic) {
            metrics.errors_total.inc();
            let body = format!("forbidden: read on {topic}");
            let resp = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            eprintln!(
                "[zerodds-ws-bridged] acl reject conn={conn_id} subject={} topic={topic}",
                subject.name
            );
            return Err(ServerError::Io(alloc_format(format_args!(
                "acl reject: {topic}"
            ))));
        }
    }

    let resp = build_server_response(&req);
    let resp_bytes = render_server_response(&resp);
    stream
        .write_all(resp_bytes.as_bytes())
        .map_err(|e| ServerError::Io(e.to_string()))?;

    // 2. Register the connection with the router.
    let (tx, rx) = std::sync::mpsc::channel::<RouterMsg>();
    if let Ok(mut r) = router.lock() {
        r.register_connection(conn_id, tx);
        if let Some(topic) = &auto_topic {
            r.subscribe(conn_id, topic.clone());
        }
    }

    // 3. Writer thread (router channel → WS frames). The stream is
    //    Arc<Mutex<>>-shared between the reader loop and the writer thread,
    //    because TLS streams cannot be duplicated via `try_clone`
    //    (rustls session state). Plain TCP would work via try_clone,
    //    but we use the mutex path uniformly.
    let stream = Arc::new(Mutex::new(stream));
    let stop_w = Arc::clone(&stop);
    let frames_out = Arc::clone(&metrics.frames_out_total);
    let bytes_out = Arc::clone(&metrics.bytes_out_total);
    let errors_out = Arc::clone(&metrics.errors_total);
    let stream_w = Arc::clone(&stream);
    // Per-connection ACL state: the read check is done per topic
    // before we hand off to router.dispatch. Here before the send: we
    // have the subject + ACL via closure move.
    let security_w = Arc::clone(&security);
    let subject_w = subject.clone();
    let writer_thread = thread::spawn(move || {
        while !stop_w.load(Ordering::SeqCst) {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(RouterMsg::Sample {
                    topic,
                    payload,
                    big_endian,
                }) => {
                    // §7.3 — read ACL (post-subscribe gate): if the
                    // ACL for this subject + topic is deny,
                    // drop the sample (no disclosure).
                    if !authorize(&security_w.acl, &subject_w, AclOp::Read, &topic) {
                        continue;
                    }
                    let json = render_notify_json(&topic, &payload, big_endian);
                    let frame = Frame::text(json);
                    if let Ok(bytes) = encode(&frame) {
                        bytes_out.add(bytes.len() as u64);
                        let mut guard = match stream_w.lock() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        if guard.write_all(&bytes).is_err() {
                            errors_out.inc();
                            break;
                        }
                        frames_out.inc();
                    } else {
                        errors_out.inc();
                    }
                }
                Ok(RouterMsg::Shutdown) => {
                    let close = Frame::close(1001, "going away");
                    if let Ok(b) = encode(&close) {
                        let mut guard = match stream_w.lock() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        let _ = guard.write_all(&b);
                    }
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    // 4. Reader loop (TCP/TLS → WS frames → router/DDS writer).
    let mut frame_buf: Vec<u8> = Vec::new();
    'reader: loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let read_result = {
            let mut guard = match stream.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            guard.read(&mut buf)
        };
        // Mutex fairness: the stream lock is shared between the reader loop
        // and the writer thread. Without a yield the reader grabs the lock
        // again immediately after each lock release (post-read_timeout);
        // the writer thread (notify frames) does not get through under
        // mutex starvation. A 1 ms sleep gives the OS scheduler a chance
        // to assign the lock to the waiting writer.
        thread::sleep(Duration::from_millis(1));
        match read_result {
            Ok(0) => break,
            Ok(n) => {
                frame_buf.extend_from_slice(&buf[..n]);
                while let Ok((frame, used)) = decode(&frame_buf) {
                    frame_buf.drain(..used);
                    match frame.opcode {
                        Opcode::Text | Opcode::Binary => {
                            let payload = frame.payload;
                            metrics.frames_in_total.inc();
                            metrics.bytes_in_total.add(payload.len() as u64);
                            let result = handle_inbound_frame(
                                &payload,
                                conn_id,
                                &router,
                                &writers,
                                &runtime,
                                auto_topic.as_deref(),
                                &metrics,
                                &security,
                                &subject,
                                &stream,
                            );
                            if let Err(e) = result {
                                metrics.errors_total.inc();
                                eprintln!("[zerodds-ws-bridged] inbound err conn={conn_id}: {e}");
                            }
                        }
                        Opcode::Ping => {
                            let pong = Frame::pong(frame.payload);
                            if let Ok(b) = encode(&pong) {
                                let mut guard = match stream.lock() {
                                    Ok(g) => g,
                                    Err(p) => p.into_inner(),
                                };
                                let _ = guard.write_all(&b);
                            }
                        }
                        Opcode::Pong => {}
                        Opcode::Close => break 'reader,
                        _ => {}
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => break,
        }
    }

    // 5. Cleanup.
    if let Ok(mut r) = router.lock() {
        r.deregister_connection(conn_id);
    }
    let _ = writer_thread.join();
    drop(conn_guard);
    Ok(())
}

/// A connection is either plain TCP or TLS-wrapped.
/// Read/write operations go through the same trait so the WS
/// reader/writer loop has the same logic for both paths.
#[cfg(feature = "daemon")]
enum WsStream {
    /// Plain `TcpStream` — `tls_enabled=false`.
    Plain(TcpStream),
    /// Server-side TLS stream on an accepted connection.
    Tls(Box<StreamOwned<ServerConnection, TcpStream>>),
}

#[cfg(feature = "daemon")]
impl WsStream {
    fn set_read_timeout(&mut self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_read_timeout(dur),
            Self::Tls(s) => s.sock.set_read_timeout(dur),
        }
    }
}

#[cfg(feature = "daemon")]
impl Read for WsStream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(b),
            Self::Tls(s) => s.read(b),
        }
    }
}

#[cfg(feature = "daemon")]
impl Write for WsStream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(b),
            Self::Tls(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Tls(s) => s.flush(),
        }
    }
}

/// RAII guard that decrements `connections_active` on drop.
#[cfg(feature = "daemon")]
struct ConnectionLifetime {
    active: Arc<zerodds_monitor::Gauge>,
}

#[cfg(feature = "daemon")]
impl Drop for ConnectionLifetime {
    fn drop(&mut self) {
        self.active.dec();
    }
}

#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments)]
fn handle_inbound_frame(
    payload: &[u8],
    conn_id: u64,
    router: &Arc<Mutex<Router>>,
    writers: &Arc<std::collections::BTreeMap<String, EntityId>>,
    runtime: &Arc<DcpsRuntime>,
    auto_topic: Option<&str>,
    metrics: &BridgeMetrics,
    security: &Arc<SecurityCtx>,
    subject: &AuthSubject,
    stream: &Arc<Mutex<WsStream>>,
) -> Result<(), String> {
    use crate::dds_bridge::{BridgeOp, parse_op};
    // Try to parse a JSON op.
    let text =
        core::str::from_utf8(payload).map_err(|e| alloc_format(format_args!("utf8: {e}")))?;
    if let Ok(op) = parse_op(text) {
        match op {
            BridgeOp::Subscribe { topic, .. } => {
                if !authorize(&security.acl, subject, AclOp::Read, &topic) {
                    metrics.errors_total.inc();
                    let err = format!(
                        "{{\"op\":\"error\",\"code\":403,\"topic\":\"{topic}\",\"reason\":\"acl-deny-read\"}}"
                    );
                    send_text_frame(stream, &err);
                    eprintln!(
                        "[zerodds-ws-bridged] acl-deny conn={conn_id} subject={} read {topic}",
                        subject.name
                    );
                    return Ok(());
                }
                if let Ok(mut r) = router.lock() {
                    r.subscribe(conn_id, topic);
                }
                return Ok(());
            }
            BridgeOp::Unsubscribe { topic, .. } => {
                if let Ok(mut r) = router.lock() {
                    r.unsubscribe(conn_id, &topic);
                }
                return Ok(());
            }
            BridgeOp::Publish { topic, data } => {
                if !authorize(&security.acl, subject, AclOp::Write, &topic) {
                    metrics.errors_total.inc();
                    let err = format!(
                        "{{\"op\":\"error\",\"code\":403,\"topic\":\"{topic}\",\"reason\":\"acl-deny-write\"}}"
                    );
                    send_text_frame(stream, &err);
                    eprintln!(
                        "[zerodds-ws-bridged] acl-deny conn={conn_id} subject={} write {topic}",
                        subject.name
                    );
                    return Ok(());
                }
                if let Some(eid) = writers.get(&topic) {
                    runtime
                        .write_user_sample(*eid, data.into_bytes())
                        .map_err(|e| alloc_format(format_args!("dds-write: {e:?}")))?;
                    metrics.dds_samples_in_total.inc();
                }
                return Ok(());
            }
        }
    }
    // Fallback: if the connection is bound to a single topic path,
    // treat the whole frame as an opaque payload publish.
    if let Some(topic) = auto_topic {
        if !authorize(&security.acl, subject, AclOp::Write, topic) {
            metrics.errors_total.inc();
            let err = format!(
                "{{\"op\":\"error\",\"code\":403,\"topic\":\"{topic}\",\"reason\":\"acl-deny-write\"}}"
            );
            send_text_frame(stream, &err);
            return Ok(());
        }
        if let Some(eid) = writers.get(topic) {
            runtime
                .write_user_sample(*eid, payload.to_vec())
                .map_err(|e| alloc_format(format_args!("dds-write: {e:?}")))?;
            metrics.dds_samples_in_total.inc();
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(feature = "daemon")]
fn send_text_frame(stream: &Arc<Mutex<WsStream>>, text: &str) {
    let frame = Frame::text(text.to_string());
    if let Ok(b) = encode(&frame) {
        let mut g = match stream.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let _ = g.write_all(&b);
    }
}

fn render_notify_json(topic: &str, payload: &[u8], big_endian: bool) -> String {
    let payload_text = match core::str::from_utf8(payload) {
        Ok(s) => s.to_string(),
        Err(_) => format_bytes_array(payload),
    };
    let payload_json = if payload_text.starts_with('{') || payload_text.starts_with('[') {
        payload_text
    } else {
        let mut buf = String::from("\"");
        for c in payload_text.chars() {
            match c {
                '"' => buf.push_str("\\\""),
                '\\' => buf.push_str("\\\\"),
                '\n' => buf.push_str("\\n"),
                '\r' => buf.push_str("\\r"),
                '\t' => buf.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    buf.push_str(&alloc_format(format_args!("\\u{:04x}", c as u32)));
                }
                c => buf.push(c),
            }
        }
        buf.push('"');
        buf
    };
    // `"be":true` only for a big-endian payload; the LE default omits it so
    // the common-case frame bytes are unchanged (older clients ignore it).
    let be_field = if big_endian { ",\"be\":true" } else { "" };
    alloc_format(format_args!(
        "{{\"op\":\"notify\",\"topic\":\"{topic}\",\"data\":{payload_json}{be_field}}}"
    ))
}

fn format_bytes_array(b: &[u8]) -> String {
    let mut out = String::from("[");
    for (i, byte) in b.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&alloc_format(format_args!("{byte}")));
    }
    out.push(']');
    out
}

#[cfg(feature = "daemon")]
fn stable_prefix_for(addr: &str) -> GuidPrefix {
    let mut bytes = [0u8; 12];
    let src = addr.as_bytes();
    for (i, b) in src.iter().take(12).enumerate() {
        bytes[i] = *b;
    }
    bytes[0] ^= 0x42; // so that a prefix of 0x00 is ruled out
    GuidPrefix::from_bytes(bytes)
}

fn alloc_format(args: core::fmt::Arguments<'_>) -> String {
    use core::fmt::Write as _;
    let mut s = String::new();
    let _ = s.write_fmt(args);
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn render_notify_json_with_text_payload() {
        let s = render_notify_json("Trade", b"hello", false);
        assert!(s.contains("\"op\":\"notify\""));
        assert!(s.contains("\"topic\":\"Trade\""));
        assert!(s.contains("\"hello\""));
        // LE default omits the byte-order field — wire unchanged.
        assert!(!s.contains("\"be\""));
    }

    #[test]
    fn render_notify_json_with_object_payload() {
        let s = render_notify_json("X", b"{\"a\":1}", false);
        assert!(s.contains("\"data\":{\"a\":1}"));
    }

    #[test]
    fn render_notify_json_escapes_quotes() {
        let s = render_notify_json("X", b"a\"b", false);
        assert!(s.contains("\\\""));
    }

    #[test]
    fn render_notify_json_big_endian_sets_be_field() {
        let s = render_notify_json("Trade", b"hello", true);
        // A big-endian payload carries "be":true so the browser dispatches
        // the big-endian decoder.
        assert!(s.contains("\"be\":true"));
        assert!(s.contains("\"topic\":\"Trade\""));
    }
}
