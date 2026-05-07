// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket-Server + DDS-Pump fuer den `zerodds-ws-bridged`-Daemon.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §4 + §9.
//!
//! `eprintln!`-Logging im Daemon-Pfad: Spec §8.1 ueberlaesst dem
//! Daemon strukturiertes Logging. Bis ein workspace-tracing-Stack
//! gewired ist, ist `eprintln` die Senke; lokal als clippy-allow
//! auf den betroffenen Funktionen markiert.
//!
//! Sync, blockierendes I/O auf `std::net`. Pro Connection ein
//! Reader-Thread (TCP→WS-Frames→Router) plus ein Writer-Thread
//! (Router-Channel→WS-Frames→TCP). Der DDS-Pump-Thread konsumiert
//! `mpsc::Receiver<UserSample>` aus jedem registrierten Reader und
//! dispatcht ueber den `Router`.

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

/// Top-Level-Fehler des Daemons.
#[derive(Debug)]
pub enum ServerError {
    /// Bind-Fehler (Exit-Code 2).
    Bind(String),
    /// DCPS-Init-Fehler (Exit-Code 3).
    Dds(String),
    /// IO-Fehler waehrend Operation.
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

/// Daemon-Handle. Beim Drop wird shutdown aufgerufen.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    pump_threads: Vec<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    admin_thread: Option<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    otlp_thread: Option<JoinHandle<()>>,
    router: Arc<Mutex<Router>>,
    /// Bound-Address (kann von Config-listen abweichen wenn Port=0).
    pub local_addr: String,
    /// Bound Admin-Address (Prometheus + Catalog + Healthz).
    #[cfg(feature = "daemon")]
    pub admin_addr: Option<String>,
    /// Lifecycle: SIGHUP setzt das hier; Server-Loop kann reagieren.
    #[cfg(feature = "daemon")]
    pub reload_flag: Arc<AtomicBool>,
    /// Healthz-Flag — DCPS-Runtime up == true.
    #[cfg(feature = "daemon")]
    pub healthy: Arc<AtomicBool>,
    /// Metric-Set fuer §8.2-Wireup. Reader-side fuer Tests.
    #[cfg(feature = "daemon")]
    pub metrics: Option<BridgeMetrics>,
}

impl DaemonHandle {
    /// Initiiert graceful Shutdown.
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
        // Self-connect Admin-Server (so der accept-loop blockt).
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
/// Startet den Daemon mit der gegebenen Config. Blockiert NICHT —
/// gibt einen `DaemonHandle` zurueck ueber den der Caller (entweder
/// das Binary oder ein E2E-Test) das Lifecycle steuert.
///
/// # Errors
/// `Bind` wenn der TCP-Listener nicht binden kann (Spec Exit-Code 2).
/// `Dds` wenn der `DcpsRuntime` nicht starten kann (Spec Exit-Code 3).
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_lines)]
pub fn start(cfg: DaemonConfig) -> Result<DaemonHandle, ServerError> {
    eprintln!(
        "[zerodds-ws-bridged] starting on {} domain={} topics={}",
        cfg.listen,
        cfg.domain,
        cfg.topics.len()
    );

    // 0. Metrics-Registry + Standard-Counter-Set (§8.2).
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);

    // 0b. Bridge-Security: Security-Ctx + ggf. RotatingTlsConfig für
    //     SIGHUP-Hot-Reload (§7.1 TLS / §7.2 Auth / §7.3 ACL).
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

    // 1. DCPS-Runtime hochfahren.
    let prefix = stable_prefix_for(&cfg.listen);
    let runtime = DcpsRuntime::start(cfg.domain, prefix, RuntimeConfig::default())
        .map_err(|e| ServerError::Dds(alloc_format(format_args!("{e:?}"))))?;
    let healthy = Arc::new(AtomicBool::new(true));

    // 2. Pro Topic Reader+Writer registrieren.
    let mut writers: std::collections::BTreeMap<String, EntityId> =
        std::collections::BTreeMap::new();
    let mut readers: Vec<(String, std::sync::mpsc::Receiver<UserSample>)> = Vec::new();
    for topic in &cfg.topics {
        let (reader_eid, writer_eid) = register_topic_endpoints(&runtime, topic)?;
        if let Some((eid, rx)) = reader_eid {
            let _ = eid;
            readers.push((topic.name.clone(), rx));
        }
        if let Some(eid) = writer_eid {
            writers.insert(topic.name.clone(), eid);
        }
    }

    // 3. Router + TCP-Listener.
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

    // 4. Pump-Threads pro Reader.
    let mut pump_threads = Vec::new();
    for (topic_name, rx) in readers {
        let router_c = Arc::clone(&router);
        let stop_c = Arc::clone(&stop);
        let topic_name_c = topic_name.clone();
        let dds_out = Arc::clone(&metrics.dds_samples_out_total);
        let h = thread::spawn(move || {
            while !stop_c.load(Ordering::SeqCst) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(UserSample::Alive { payload, .. }) => {
                        dds_out.inc();
                        if let Ok(mut r) = router_c.lock() {
                            r.dispatch(&topic_name_c, payload);
                        }
                    }
                    Ok(UserSample::Lifecycle { .. }) => {
                        // Lifecycle-Events: wir koennten dispose-Frames
                        // pushen — fuer L1-L4-Wireup reicht der Alive-
                        // Pfad.
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        pump_threads.push(h);
    }

    // 5. Accept-Thread.
    let next_conn_id = Arc::new(AtomicU64::new(1));
    let stop_acc = Arc::clone(&stop);
    let router_acc = Arc::clone(&router);
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
                        // Wenn TLS konfiguriert ist: rustls-Handshake. Sonst Plain.
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

    // 6. Admin-Endpoint (§8.2 Prometheus + §5.2 Catalog/Healthz).
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

    // 7. Signal-Watcher (§9.2 Graceful Shutdown).
    if let Err(e) = install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag)) {
        eprintln!("[{SERVICE_NAME}] signal watcher init failed: {e}");
    }

    // 7b. Bridge-Security: SIGHUP-Hook für TLS-Cert-Hot-Reload. Polled das
    //     reload_flag und ruft RotatingTlsConfig::reload() auf.
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

    // 8. OTLP-Span-Exporter (§8.3) wenn ENV gesetzt.
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

    // 1. HTTP-Upgrade-Handshake einlesen.
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

    // §7.2 — Authentication. Bei Reject: HTTP 401 + close.
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

    // Auto-Subscribe wenn Pfad einem Topic entspricht (Spec §4.2).
    let mut auto_topic: Option<String> = None;
    for t in topics.iter() {
        if t.ws_path == req.path || super::config::default_ws_path(&t.name) == req.path {
            auto_topic = Some(t.name.clone());
            break;
        }
    }

    // §7.3 — Auto-Subscribe-Topic ACL-Check (Read).
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

    // 2. Connection an Router registrieren.
    let (tx, rx) = std::sync::mpsc::channel::<RouterMsg>();
    if let Ok(mut r) = router.lock() {
        r.register_connection(conn_id, tx);
        if let Some(topic) = &auto_topic {
            r.subscribe(conn_id, topic.clone());
        }
    }

    // 3. Writer-Thread (Router-Channel → WS-Frames). Der Stream ist
    //    Arc<Mutex<>>-shared zwischen Reader-Loop und Writer-Thread,
    //    weil TLS-Streams sich nicht via `try_clone` duplizieren lassen
    //    (rustls-Session-State). Plain-TCP würde via try_clone gehen,
    //    aber wir nehmen einheitlich den Mutex-Pfad.
    let stream = Arc::new(Mutex::new(stream));
    let stop_w = Arc::clone(&stop);
    let frames_out = Arc::clone(&metrics.frames_out_total);
    let bytes_out = Arc::clone(&metrics.bytes_out_total);
    let errors_out = Arc::clone(&metrics.errors_total);
    let stream_w = Arc::clone(&stream);
    // Per-Connection ACL-State: pro Topic wird der Read-Check gemacht
    // bevor wir zu router.dispatch reichen. Hier vor dem Send: Subject
    // + Acl haben wir per Closure-Move.
    let security_w = Arc::clone(&security);
    let subject_w = subject.clone();
    let writer_thread = thread::spawn(move || {
        while !stop_w.load(Ordering::SeqCst) {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(RouterMsg::Sample { topic, payload }) => {
                    // §7.3 — Read-ACL (post-subscribe gate): wenn die
                    // ACL für diesen Subject + Topic auf Deny steht,
                    // droppe das Sample (kein Disclose).
                    if !authorize(&security_w.acl, &subject_w, AclOp::Read, &topic) {
                        continue;
                    }
                    let json = render_notify_json(&topic, &payload);
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

    // 4. Reader-Loop (TCP/TLS → WS-Frames → Router/DDS-Writer).
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

/// Eine Connection ist entweder Plain-TCP oder TLS-gewrapped.
/// Read/Write-Operationen gehen durch denselben Trait, damit der WS-
/// Reader/Writer-Loop dieselbe Logik für beide Pfade hat.
#[cfg(feature = "daemon")]
enum WsStream {
    /// Plain `TcpStream` — `tls_enabled=false`.
    Plain(TcpStream),
    /// Server-Side TLS-Stream auf einer akzeptierten Connection.
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

/// RAII-Guard, der `connections_active` beim Drop dekrementiert.
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
    // Versuche JSON-Op zu parsen.
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
    // Fallback: wenn Connection an einen einzelnen Topic-Pfad gebunden
    // ist, behandle das ganze Frame als opaque Payload-Publish.
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

fn render_notify_json(topic: &str, payload: &[u8]) -> String {
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
    alloc_format(format_args!(
        "{{\"op\":\"notify\",\"topic\":\"{topic}\",\"data\":{payload_json}}}"
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
    bytes[0] ^= 0x42; // damit ein Prefix von 0x00 ausgeschlossen ist
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
        let s = render_notify_json("Trade", b"hello");
        assert!(s.contains("\"op\":\"notify\""));
        assert!(s.contains("\"topic\":\"Trade\""));
        assert!(s.contains("\"hello\""));
    }

    #[test]
    fn render_notify_json_with_object_payload() {
        let s = render_notify_json("X", b"{\"a\":1}");
        assert!(s.contains("\"data\":{\"a\":1}"));
    }

    #[test]
    fn render_notify_json_escapes_quotes() {
        let s = render_notify_json("X", b"a\"b");
        assert!(s.contains("\\\""));
    }
}
