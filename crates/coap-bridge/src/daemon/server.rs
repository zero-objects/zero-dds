// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP server for `zerodds-coap-bridged`.
//!
//! Spec: `zerodds-coap-bridge-1.0.md` §4 + §9.
//!
//! UDP-only sync server. For each received CoAP datagram:
//! * `POST` / `PUT` with a matching `Uri-Path` → DDS write.
//! * `GET` with `Observe: 0` → observer registry; every new DDS
//!   sample is sent to the peer as a CoAP notify (NON).
//! * `GET` with `Observe: 1` → deregister.
//! * `DELETE` → DDS dispose marker (L1-L3 stub: only acks back).
//!
//! Block-wise (RFC 7959), DTLS (Spec §7.1) and OSCORE (§7.2) are
//! marked as L5 stubs.

use std::collections::BTreeMap;
use std::net::{SocketAddr, UdpSocket};
use std::string::{String, ToString};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
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

use crate::blockwise::{BlockReassembler, BlockValue};
use crate::codec::{decode, encode};
use crate::message::{CoapCode, CoapMessage, MessageType};
use crate::option::{CoapOption, OptionValue, numbers};

use super::config::{DaemonConfig, TopicConfig};
#[cfg(feature = "daemon")]
use super::runtime_common::{
    BridgeMetrics, CatalogSnapshot, SERVICE_NAME, install_signal_watcher, otlp_config_from_env,
    serve_admin_endpoints, spawn_otlp_flush_loop,
};
#[cfg(feature = "daemon")]
use super::security::{
    AclOp, COAP_OPTION_AUTH_TOKEN, SecurityCtx, authenticate_coap, authorize,
    ctx_from_daemon_config,
};
#[cfg(feature = "daemon")]
use zerodds_monitor::Registry;
#[cfg(feature = "daemon")]
use zerodds_observability_otlp::OtlpExporter;

/// Daemon top-level error.
#[derive(Debug)]
pub enum ServerError {
    /// UDP bind (exit 2).
    Bind(String),
    /// DDS init (exit 3).
    Dds(String),
    /// DTLS (exit 4).
    Dtls(String),
    /// IO.
    Io(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bind(m) => write!(f, "bind: {m}"),
            Self::Dds(m) => write!(f, "dds: {m}"),
            Self::Dtls(m) => write!(f, "dtls: {m}"),
            Self::Io(m) => write!(f, "io: {m}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Daemon handle. Drop initiates shutdown.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    pump_threads: Vec<JoinHandle<()>>,
    /// Bound address (may differ from the config bind when port=0).
    pub local_addr: String,
    #[cfg(feature = "daemon")]
    admin_thread: Option<JoinHandle<()>>,
    #[cfg(feature = "daemon")]
    otlp_thread: Option<JoinHandle<()>>,
    /// Bound admin address.
    #[cfg(feature = "daemon")]
    pub admin_addr: Option<String>,
    /// SIGHUP reload flag.
    #[cfg(feature = "daemon")]
    pub reload_flag: Arc<AtomicBool>,
    /// Healthz.
    #[cfg(feature = "daemon")]
    pub healthy: Arc<AtomicBool>,
    /// Metric set for tests.
    #[cfg(feature = "daemon")]
    pub metrics: Option<BridgeMetrics>,
}

impl DaemonHandle {
    /// Sets the stop flag and joins workers.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        #[cfg(feature = "daemon")]
        {
            self.healthy.store(false, Ordering::SeqCst);
            if let Some(admin) = self.admin_addr.as_deref() {
                if let Ok(addr) = admin.parse::<SocketAddr>() {
                    let _ = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200));
                }
            }
        }
        // Wake socket-recv loop with a self-sent dgram.
        if let Ok(addr) = self.local_addr.parse::<SocketAddr>() {
            if let Ok(sock) = UdpSocket::bind("127.0.0.1:0") {
                let _ = sock.send_to(&[0u8; 4], addr);
            }
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

/// Observer registry entry.
#[derive(Debug, Clone)]
struct Observer {
    addr: SocketAddr,
    token: Vec<u8>,
}

/// Topic state.
#[derive(Debug, Default)]
struct TopicState {
    observers: Vec<Observer>,
    next_seq: u32,
}

/// Block1 reassembly key (peer + token + path).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BlockKey {
    peer: SocketAddr,
    token: Vec<u8>,
    path: String,
}
/// zerodds-lint: recursion-depth 64 (start bounded by AST depth)
/// Starts the daemon with the given config.
///
/// # Errors
/// `Bind` (exit 2), `Dds` (exit 3), `Dtls` (exit 4).
#[cfg(feature = "daemon")]
#[allow(clippy::too_many_lines)]
pub fn start(cfg: DaemonConfig) -> Result<DaemonHandle, ServerError> {
    eprintln!(
        "[zerodds-coap-bridged] starting bind={} domain={} topics={}",
        cfg.bind,
        cfg.domain,
        cfg.topics.len()
    );

    // 0. Metrics registry + standard counters (§8.2 Prometheus).
    let registry = Arc::new(Registry::new());
    let metrics = BridgeMetrics::register(&registry);
    let healthy = Arc::new(AtomicBool::new(true));
    let reload_flag = Arc::new(AtomicBool::new(false));

    // 0b. Bridge security: auth + ACL (§7.2/§7.3; DTLS via separate ADR).
    let security_ctx =
        ctx_from_daemon_config(&cfg).map_err(|e| ServerError::Dtls(format!("security: {e}")))?;
    let security_ctx = Arc::new(security_ctx);
    eprintln!(
        "[zerodds-coap-bridged] auth-mode={} acl-entries={} (dtls=rejected per §7.1 next-phase)",
        cfg.auth_mode,
        cfg.topic_acl.len(),
    );

    // 1. Bind the UDP socket.
    let socket = UdpSocket::bind(&cfg.bind).map_err(|e| ServerError::Bind(format!("{e}")))?;
    let local_addr = socket
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| cfg.bind.clone());
    socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| ServerError::Io(format!("set timeout: {e}")))?;

    eprintln!("[zerodds-coap-bridged] bound on {local_addr}");

    if cfg.dtls_enabled {
        // FUTURE: DTLS handshake layer (RFC 7252 §9). L5 stub.
        eprintln!("[zerodds-coap-bridged] DTLS L5-stub: not implemented");
    }

    // 2. DCPS runtime.
    let prefix = stable_prefix_for(&local_addr);
    let runtime = DcpsRuntime::start(cfg.domain, prefix, RuntimeConfig::default())
        .map_err(|e| ServerError::Dds(format!("{e:?}")))?;

    // 3. Reader+writer per topic + path map.
    let mut writers: BTreeMap<String, EntityId> = BTreeMap::new();
    let mut path_to_dds: BTreeMap<String, String> = BTreeMap::new();
    let mut readers: Vec<(String, String, std::sync::mpsc::Receiver<UserSample>)> = Vec::new();
    for topic in &cfg.topics {
        register_topic(
            &runtime,
            topic,
            &mut writers,
            &mut path_to_dds,
            &mut readers,
        )?;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let topic_state: Arc<Mutex<BTreeMap<String, TopicState>>> =
        Arc::new(Mutex::new(BTreeMap::new()));
    let next_mid = Arc::new(AtomicU16::new(1));
    // Block1 (RFC 7959 §2.5) — reassembly per peer+token+path.
    let block1_state: Arc<Mutex<BTreeMap<BlockKey, BlockReassembler>>> =
        Arc::new(Mutex::new(BTreeMap::new()));

    // 4. Pump threads per reader topic.
    let mut pump_threads = Vec::new();
    for (dds_topic_name, coap_path, rx) in readers {
        let stop_c = Arc::clone(&stop);
        let socket_c = socket
            .try_clone()
            .map_err(|e| ServerError::Io(format!("{e}")))?;
        let state_c = Arc::clone(&topic_state);
        let mid_c = Arc::clone(&next_mid);
        let _topic_name = dds_topic_name.clone();
        let path = coap_path.clone();
        let dds_out = Arc::clone(&metrics.dds_samples_out_total);
        let h = thread::spawn(move || {
            while !stop_c.load(Ordering::SeqCst) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(UserSample::Alive { payload, .. }) => {
                        dds_out.inc();
                        push_notify(&socket_c, &state_c, &path, &payload, &mid_c);
                    }
                    Ok(UserSample::Lifecycle { .. }) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        pump_threads.push(h);
    }

    // 5. Accept thread (recv loop).
    let stop_acc = Arc::clone(&stop);
    let socket_acc = socket
        .try_clone()
        .map_err(|e| ServerError::Io(format!("{e}")))?;
    let writers_arc = Arc::new(writers);
    let path_to_dds_arc = Arc::new(path_to_dds);
    let runtime_arc = Arc::clone(&runtime);
    let topic_state_acc = Arc::clone(&topic_state);
    let cfg_arc = Arc::new(cfg.clone());
    let mid_acc = Arc::clone(&next_mid);
    let block1_acc = Arc::clone(&block1_state);
    let metrics_acc = metrics.clone();
    let security_acc = Arc::clone(&security_ctx);
    let accept_thread = thread::spawn(move || {
        let mut buf = [0u8; 65535];
        while !stop_acc.load(Ordering::SeqCst) {
            match socket_acc.recv_from(&mut buf) {
                Ok((n, peer)) => {
                    let bytes = &buf[..n];
                    metrics_acc.frames_in_total.inc();
                    metrics_acc.bytes_in_total.add(n as u64);
                    if n > cfg_arc.max_message_size {
                        // Spec §4.2 — `4.13 Request Entity Too Large`.
                        if let Ok(req) = decode(bytes) {
                            let resp = make_response(&req, CoapCode::new(4, 13), Vec::new());
                            send_msg(&socket_acc, &resp, peer);
                            metrics_acc.frames_out_total.inc();
                        }
                        metrics_acc.errors_total.inc();
                        continue;
                    }
                    if let Err(e) = handle_request(
                        bytes,
                        peer,
                        &socket_acc,
                        &writers_arc,
                        &path_to_dds_arc,
                        &runtime_arc,
                        &topic_state_acc,
                        &mid_acc,
                        &cfg_arc,
                        &block1_acc,
                        &metrics_acc,
                        &security_acc,
                    ) {
                        metrics_acc.errors_total.inc();
                        eprintln!("[zerodds-coap-bridged] handle err from {peer}: {e}");
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                Err(e) => {
                    if !stop_acc.load(Ordering::SeqCst) {
                        eprintln!("[zerodds-coap-bridged] recv err: {e}");
                    }
                    continue;
                }
            }
        }
    });

    // 6. Admin-Endpoint (§5.2 Catalog/Healthz + §8.2 Metrics).
    let mut admin_thread: Option<JoinHandle<()>> = None;
    let mut admin_addr: Option<String> = None;
    if cfg.metrics_enabled || !cfg.metrics_addr.is_empty() {
        let bind_str = if cfg.metrics_addr.is_empty() {
            "127.0.0.1:9090".to_string()
        } else {
            cfg.metrics_addr.clone()
        };
        match bind_str.parse::<SocketAddr>() {
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

    // 7. Signal watcher.
    if let Err(e) = install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag)) {
        eprintln!("[{SERVICE_NAME}] signal watcher init failed: {e}");
    }

    // 8. OTLP.
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
        accept_thread: Some(accept_thread),
        pump_threads,
        local_addr,
        admin_thread,
        otlp_thread,
        admin_addr,
        reload_flag,
        healthy,
        metrics: Some(metrics),
    })
}

#[cfg(feature = "daemon")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn handle_request(
    bytes: &[u8],
    peer: SocketAddr,
    socket: &UdpSocket,
    writers: &Arc<BTreeMap<String, EntityId>>,
    path_to_dds: &Arc<BTreeMap<String, String>>,
    runtime: &Arc<DcpsRuntime>,
    topic_state: &Arc<Mutex<BTreeMap<String, TopicState>>>,
    _mid: &Arc<AtomicU16>,
    _cfg: &Arc<DaemonConfig>,
    block1_state: &Arc<Mutex<BTreeMap<BlockKey, BlockReassembler>>>,
    metrics: &BridgeMetrics,
    security: &Arc<SecurityCtx>,
) -> Result<(), String> {
    let req = decode(bytes).map_err(|e| format!("decode: {e}"))?;
    let path = extract_uri_path(&req);

    // `/.well-known/core` → resource catalog (open, no auth).
    if path == ".well-known/core" {
        let body = render_well_known_core(path_to_dds);
        let mut resp = make_response(&req, CoapCode::CONTENT, body.into_bytes());
        resp.options.push(CoapOption::content_format(40)); // application/link-format
        send_msg(socket, &resp, peer);
        return Ok(());
    }

    // §7.2 — Authentication via CoAP-Option 65000 (Bearer-Token).
    let auth_token = extract_auth_token(&req);
    let subject = match authenticate_coap(&security.auth, auth_token.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            metrics.errors_total.inc();
            // 4.01 Unauthorized.
            let resp = make_response(&req, CoapCode::new(4, 1), b"unauthorized".to_vec());
            send_msg(socket, &resp, peer);
            metrics.frames_out_total.inc();
            return Err(format!("auth reject: {e}"));
        }
    };

    if req.code == CoapCode::POST || req.code == CoapCode::PUT {
        // POST/PUT → DDS write. Optional Block1 (RFC 7959 §2.5).
        let dds_topic = path_to_dds
            .get(&path)
            .ok_or_else(|| format!("unknown path: {path}"))?;
        // §7.3 — ACL write check.
        if !authorize(&security.acl, &subject, AclOp::Write, dds_topic) {
            metrics.errors_total.inc();
            // 4.03 Forbidden.
            let resp = make_response(&req, CoapCode::new(4, 3), b"forbidden".to_vec());
            send_msg(socket, &resp, peer);
            metrics.frames_out_total.inc();
            return Ok(());
        }
        let eid = writers.get(dds_topic).ok_or("no writer")?;

        // Block1 path: collect chunks until !more, then DDS write.
        if let Some(b1) = extract_block(&req, numbers::BLOCK1) {
            let key = BlockKey {
                peer,
                token: req.token.clone(),
                path: path.clone(),
            };
            let assembled = {
                let mut map = block1_state.lock().map_err(|_| "block1 mutex poisoned")?;
                let entry = map
                    .entry(key.clone())
                    .or_insert_with(|| BlockReassembler::new(b1.block_size().unwrap_or(64)));
                entry
                    .accept(b1, &req.payload)
                    .map_err(|e| format!("block1 reassemble: {e}"))?;
                if entry.is_complete() {
                    // SAFETY: just verified is_complete(); remove() returns Some.
                    #[allow(clippy::expect_used)]
                    let r = map.remove(&key).expect("just complete");
                    Some(r.into_payload())
                } else {
                    None
                }
            };
            if let Some(payload) = assembled {
                runtime
                    .write_user_sample(*eid, payload)
                    .map_err(|e| format!("dds-write: {e:?}"))?;
                metrics.dds_samples_in_total.inc();
                // 2.04 Changed with Block1 echo (more=false).
                let mut resp = make_response(&req, CoapCode::CHANGED, Vec::new());
                resp.options.push(CoapOption::block1(b1.num, false, b1.szx));
                send_msg(socket, &resp, peer);
                metrics.frames_out_total.inc();
            } else {
                // 2.31 Continue with Block1 echo (more=true).
                let mut resp = make_response(&req, CoapCode::new(2, 31), Vec::new());
                resp.options.push(CoapOption::block1(b1.num, true, b1.szx));
                send_msg(socket, &resp, peer);
                metrics.frames_out_total.inc();
            }
            return Ok(());
        }

        runtime
            .write_user_sample(*eid, req.payload.clone())
            .map_err(|e| format!("dds-write: {e:?}"))?;
        metrics.dds_samples_in_total.inc();
        let resp = make_response(&req, CoapCode::CHANGED, Vec::new());
        send_msg(socket, &resp, peer);
        metrics.frames_out_total.inc();
        return Ok(());
    }

    if req.code == CoapCode::GET {
        let observe = extract_observe(&req);
        let dds_topic = path_to_dds
            .get(&path)
            .ok_or_else(|| format!("unknown path: {path}"))?;
        // §7.3 — ACL read check for observe-register and plain GET.
        if !authorize(&security.acl, &subject, AclOp::Read, dds_topic) {
            metrics.errors_total.inc();
            let resp = make_response(&req, CoapCode::new(4, 3), b"forbidden".to_vec());
            send_msg(socket, &resp, peer);
            metrics.frames_out_total.inc();
            return Ok(());
        }
        match observe {
            Some(0) => {
                // Register.
                if let Ok(mut st) = topic_state.lock() {
                    let entry = st.entry(path.clone()).or_default();
                    entry.observers.push(Observer {
                        addr: peer,
                        token: req.token.clone(),
                    });
                }
                metrics.connections_total.inc();
                metrics.connections_active.inc();
                // Initial response 2.05 with observe seq 0 (Spec §4.3).
                let mut resp = make_response(&req, CoapCode::CONTENT, Vec::new());
                resp.options.push(CoapOption::observe(0));
                resp.options.push(CoapOption::content_format(65000));
                send_msg(socket, &resp, peer);
                metrics.frames_out_total.inc();
                let _ = dds_topic; // placeholder for catalog
                return Ok(());
            }
            Some(1) => {
                // Deregister.
                if let Ok(mut st) = topic_state.lock() {
                    if let Some(entry) = st.get_mut(&path) {
                        let before = entry.observers.len();
                        entry
                            .observers
                            .retain(|o| !(o.addr == peer && o.token == req.token));
                        let removed = before - entry.observers.len();
                        for _ in 0..removed {
                            metrics.connections_active.dec();
                        }
                    }
                }
                let resp = make_response(&req, CoapCode::CONTENT, Vec::new());
                send_msg(socket, &resp, peer);
                metrics.frames_out_total.inc();
                return Ok(());
            }
            _ => {
                // Plain GET — empty content (spec says 2.05 with the current sample;
                // we have no cache layer in L1-L4).
                let resp = make_response(&req, CoapCode::CONTENT, Vec::new());
                send_msg(socket, &resp, peer);
                metrics.frames_out_total.inc();
                return Ok(());
            }
        }
    }

    if req.code == CoapCode::DELETE {
        // DELETE → dispose marker (L1-L3 stub: only 2.02 back).
        let resp = make_response(&req, CoapCode::DELETED, Vec::new());
        send_msg(socket, &resp, peer);
        return Ok(());
    }

    // Unknown code — 4.00 Bad Request.
    let resp = make_response(&req, CoapCode::BAD_REQUEST, Vec::new());
    send_msg(socket, &resp, peer);
    Ok(())
}

#[cfg(feature = "daemon")]
fn extract_auth_token(msg: &CoapMessage) -> Option<Vec<u8>> {
    for opt in &msg.options {
        if opt.number == COAP_OPTION_AUTH_TOKEN {
            return match &opt.value {
                OptionValue::Opaque(b) => Some(b.clone()),
                OptionValue::String(s) => Some(s.as_bytes().to_vec()),
                _ => None,
            };
        }
    }
    None
}

fn extract_uri_path(msg: &CoapMessage) -> String {
    // Spec §3.2 Uri-Path options are carried as strings; on the wire,
    // though, they may arrive as opaque (the decoder does not normalize).
    // We accept both variants.
    let mut buf = String::new();
    let mut first = true;
    for opt in &msg.options {
        if opt.number == numbers::URI_PATH {
            let segment = match &opt.value {
                OptionValue::String(s) => s.clone(),
                OptionValue::Opaque(b) => String::from_utf8_lossy(b).into_owned(),
                OptionValue::Empty => String::new(),
                OptionValue::Uint(v) => format!("{v}"),
            };
            if !first {
                buf.push('/');
            }
            buf.push_str(&segment);
            first = false;
        }
    }
    buf
}

/// Extracts the Block1 or Block2 option from a CoapMessage.
/// Spec RFC 7959 §2.1.
fn extract_block(msg: &CoapMessage, option_number: u16) -> Option<BlockValue> {
    for opt in &msg.options {
        if opt.number != option_number {
            continue;
        }
        let bytes: Vec<u8> = match &opt.value {
            OptionValue::Opaque(b) => b.clone(),
            OptionValue::Uint(v) => {
                let raw = *v;
                if raw < 0x100 {
                    vec![raw as u8]
                } else if raw < 0x1_0000 {
                    vec![((raw >> 8) & 0xff) as u8, (raw & 0xff) as u8]
                } else {
                    vec![
                        ((raw >> 16) & 0xff) as u8,
                        ((raw >> 8) & 0xff) as u8,
                        (raw & 0xff) as u8,
                    ]
                }
            }
            OptionValue::Empty => vec![0u8],
            _ => continue,
        };
        if let Ok(v) = BlockValue::decode(&bytes) {
            return Some(v);
        }
    }
    None
}

fn extract_observe(msg: &CoapMessage) -> Option<u32> {
    use crate::option::uint_from_bytes;
    for opt in &msg.options {
        if opt.number == numbers::OBSERVE {
            let v = match &opt.value {
                OptionValue::Uint(v) => Some(u32::try_from(*v).unwrap_or(u32::MAX)),
                OptionValue::Opaque(b) => {
                    uint_from_bytes(b).map(|v| u32::try_from(v).unwrap_or(u32::MAX))
                }
                OptionValue::Empty => Some(0),
                _ => None,
            };
            if v.is_some() {
                return v;
            }
        }
    }
    None
}

fn render_well_known_core(path_to_dds: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut first = true;
    for path in path_to_dds.keys() {
        if !first {
            out.push(',');
        }
        out.push('<');
        out.push('/');
        out.push_str(path);
        out.push_str(">;rt=\"dds.topic\";ct=65000");
        first = false;
    }
    out
}

fn make_response(req: &CoapMessage, code: CoapCode, payload: Vec<u8>) -> CoapMessage {
    // ACK for a CON request, otherwise NON.
    let mtype = if matches!(req.message_type, MessageType::Confirmable) {
        MessageType::Acknowledgement
    } else {
        MessageType::NonConfirmable
    };
    CoapMessage {
        version: 1,
        message_type: mtype,
        code,
        message_id: req.message_id,
        token: req.token.clone(),
        options: Vec::new(),
        payload,
    }
}

fn send_msg(socket: &UdpSocket, msg: &CoapMessage, peer: SocketAddr) {
    if let Ok(bytes) = encode(msg) {
        let _ = socket.send_to(&bytes, peer);
    }
}

fn push_notify(
    socket: &UdpSocket,
    topic_state: &Arc<Mutex<BTreeMap<String, TopicState>>>,
    path: &str,
    payload: &[u8],
    mid: &Arc<AtomicU16>,
) {
    let observers = match topic_state.lock() {
        Ok(mut st) => {
            let entry = st.entry(path.to_string()).or_default();
            entry.next_seq = entry.next_seq.wrapping_add(1).max(1);
            let seq = entry.next_seq;
            entry
                .observers
                .iter()
                .map(|o| (o.clone(), seq))
                .collect::<Vec<_>>()
        }
        Err(_) => return,
    };
    for (obs, seq) in observers {
        let new_mid = mid.fetch_add(1, Ordering::SeqCst);
        let msg = CoapMessage {
            version: 1,
            message_type: MessageType::NonConfirmable,
            code: CoapCode::CONTENT,
            message_id: new_mid,
            token: obs.token.clone(),
            options: vec![CoapOption::observe(seq), CoapOption::content_format(65000)],
            payload: payload.to_vec(),
        };
        send_msg(socket, &msg, obs.addr);
    }
}

#[cfg(feature = "daemon")]
fn register_topic(
    rt: &Arc<DcpsRuntime>,
    topic: &TopicConfig,
    writers: &mut BTreeMap<String, EntityId>,
    path_to_dds: &mut BTreeMap<String, String>,
    readers: &mut Vec<(String, String, std::sync::mpsc::Receiver<UserSample>)>,
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
    let want_writer = matches!(topic.direction.as_str(), "in" | "bidir");
    let want_reader = matches!(topic.direction.as_str(), "out" | "bidir");

    if want_reader {
        let (_eid, rx) = rt
            .register_user_reader(UserReaderConfig {
                topic_name: topic.dds_name.clone(),
                type_name: topic.dds_type.clone(),
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
            .map_err(|e| ServerError::Dds(format!("reader: {e:?}")))?;
        readers.push((topic.dds_name.clone(), topic.coap_uri_path.clone(), rx));
    }

    if want_writer {
        let eid = rt
            .register_user_writer(UserWriterConfig {
                topic_name: topic.dds_name.clone(),
                type_name: topic.dds_type.clone(),
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
            .map_err(|e| ServerError::Dds(format!("writer: {e:?}")))?;
        writers.insert(topic.dds_name.clone(), eid);
    }
    path_to_dds.insert(topic.coap_uri_path.clone(), topic.dds_name.clone());
    Ok(())
}

#[cfg(feature = "daemon")]
fn stable_prefix_for(seed: &str) -> GuidPrefix {
    let mut bytes = [0u8; 12];
    let src = seed.as_bytes();
    for (i, b) in src.iter().take(12).enumerate() {
        bytes[i] = *b;
    }
    bytes[0] ^= 0x29;
    GuidPrefix::from_bytes(bytes)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn req_with_path(path: &[&str]) -> CoapMessage {
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, 1);
        m.token = b"tok".to_vec();
        for seg in path {
            m.options.push(CoapOption::uri_path(*seg));
        }
        m
    }

    #[test]
    fn extract_uri_path_joins_segments() {
        let msg = req_with_path(&["chat", "message"]);
        assert_eq!(extract_uri_path(&msg), "chat/message");
    }

    #[test]
    fn extract_uri_path_empty_when_no_options() {
        let msg = CoapMessage::new(MessageType::NonConfirmable, CoapCode::GET, 1);
        assert_eq!(extract_uri_path(&msg), "");
    }

    #[test]
    fn make_response_uses_ack_for_con() {
        let req = req_with_path(&["x"]);
        let resp = make_response(&req, CoapCode::CHANGED, Vec::new());
        assert!(matches!(resp.message_type, MessageType::Acknowledgement));
        assert_eq!(resp.message_id, req.message_id);
        assert_eq!(resp.token, req.token);
    }

    #[test]
    fn make_response_uses_non_for_non_request() {
        let mut req = req_with_path(&["x"]);
        req.message_type = MessageType::NonConfirmable;
        let resp = make_response(&req, CoapCode::CONTENT, Vec::new());
        assert!(matches!(resp.message_type, MessageType::NonConfirmable));
    }

    #[test]
    fn render_well_known_core_format() {
        let mut map = BTreeMap::new();
        map.insert("chat/message".to_string(), "Chat::Message".to_string());
        let s = render_well_known_core(&map);
        assert!(s.contains("</chat/message>"));
        assert!(s.contains("rt=\"dds.topic\""));
        assert!(s.contains("ct=65000"));
    }

    #[test]
    fn extract_observe_register() {
        let mut msg = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
        msg.options.push(CoapOption::observe(0));
        assert_eq!(extract_observe(&msg), Some(0));
    }
}
