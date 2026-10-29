// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The `zerodds-webdds-bridged` accept loop.
//!
//! Sync, blocking I/O — the workspace convention (`crates/monitor`,
//! `crates/coap-bridge`, `crates/websocket-bridge`). One thread per TCP
//! connection, looping `http1::read_request`/`write_response` for
//! keep-alive; the request's path decides REST (`/dds/rest1/...`), SOAP
//! (`/soap/...`) or WSDL (`/wsdl`) dispatch, all against the one shared
//! [`Gateway`]. `metrics.enabled` also starts the admin `/catalog`+
//! `/healthz`+`/metrics` endpoint on its own port
//! ([`runtime_common::serve_admin_endpoints`]) — the daemon family's
//! own convention, not a DDS-WEB 1.0 requirement.

// Daemon logging: `eprintln!` is the sink until a workspace tracing
// stack is wired in — the same allowance every other
// `zerodds-*-bridged` daemon's `runtime_common`/`server` module takes.
#![allow(clippy::print_stderr)]

use std::io::BufReader;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::gateway::Gateway;
use crate::http1::{self, Response};
use crate::metadata::MetadataStore;
use crate::rpc_dispatch::RpcDispatcher;
use crate::{rest_service, soap_service};

use super::config::DaemonConfig;
use super::runtime_common;
use super::security::{self, SecurityCtx, serve_tls_handshake};
use zerodds_bridge_security::rustls;

/// Admin endpoint address when `metrics.enabled` is set without
/// `metrics.listen`.
pub const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9095";

/// Top-level error of the daemon.
#[derive(Debug)]
pub enum ServerError {
    /// A setting that cannot be served.
    Config(String),
    /// TLS material that does not load.
    Tls(String),
    /// Bind error.
    Bind(String),
    /// DCPS init error.
    Dds(String),
    /// I/O error during operation.
    Io(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Config(m) => write!(f, "config error: {m}"),
            Self::Tls(m) => write!(f, "tls error: {m}"),
            Self::Bind(m) => write!(f, "bind error: {m}"),
            Self::Dds(m) => write!(f, "dds error: {m}"),
            Self::Io(m) => write!(f, "io error: {m}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// A config reload hook, invoked on SIGHUP.
pub type ConfigLoader = Box<dyn Fn() -> Result<DaemonConfig, String> + Send + Sync>;

/// Daemon handle. On drop, no implicit shutdown — call
/// [`DaemonHandle::shutdown`] explicitly (mirrors every other
/// `zerodds-*-bridged` daemon).
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    /// Bound HTTP address (may differ from the configured listen if
    /// port 0 was asked for).
    pub local_addr: String,
    /// Bound admin (`/metrics`) address, if `metrics.enabled`.
    pub admin_addr: Option<String>,
    /// SIGHUP sets this.
    pub reload_flag: Arc<AtomicBool>,
    /// `true` once the listener is up.
    pub healthy: Arc<AtomicBool>,
    /// The shared gateway — exported for tests that want to inspect or
    /// drive it directly without going through HTTP.
    pub gateway: Arc<Mutex<Gateway>>,
}

impl DaemonHandle {
    /// Whether SIGTERM/SIGINT (or [`Self::shutdown`]) asked the daemon
    /// to stop.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Initiates a graceful shutdown: stops accepting new connections
    /// and joins the accept thread. Open connections are not force-
    /// closed — they finish their current request/response and then
    /// see `stop` on their next keep-alive iteration.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.healthy.store(false, Ordering::SeqCst);
        // Unblock `accept()` by connecting to ourselves once.
        if let Ok(mut s) = TcpStream::connect(&self.local_addr) {
            use std::io::Write;
            let _ = s.write_all(b"GET / HTTP/1.1\r\nConnection: close\r\n\r\n");
        }
        if let Some(h) = self.accept_thread.take() {
            let _ = h.join();
        }
    }
}

/// Starts the daemon. Blocking-thread accept loop, returned already
/// running.
///
/// # Errors
/// See [`ServerError`].
pub fn start(cfg: DaemonConfig) -> Result<DaemonHandle, ServerError> {
    start_with_reload(cfg, None)
}

/// As [`start`], with an optional SIGHUP reload hook.
///
/// # Errors
/// See [`ServerError`].
pub fn start_with_reload(
    cfg: DaemonConfig,
    reload: Option<ConfigLoader>,
) -> Result<DaemonHandle, ServerError> {
    let type_sources: std::collections::BTreeMap<String, crate::gateway::TypeSource> = cfg
        .types
        .iter()
        .filter_map(|t| {
            let source = if let Some(idl) = &t.idl_file {
                crate::gateway::TypeSource::Idl(PathBuf::from(idl))
            } else if let Some(xml) = &t.xml_file {
                crate::gateway::TypeSource::Xml(PathBuf::from(xml))
            } else {
                return None;
            };
            Some((t.type_name.clone(), source))
        })
        .collect();
    let metadata = Arc::new(MetadataStore::new());
    let gateway = Arc::new(Mutex::new(Gateway::new(
        type_sources,
        Arc::clone(&metadata),
    )));

    let (sec_ctx, rotating) =
        security::ctx_from_daemon_config(&cfg).map_err(|e| ServerError::Tls(e.to_string()))?;
    let sec_ctx = Arc::new(RwLock::new(sec_ctx));
    let rotating = rotating.map(Arc::new);

    let wsdl_xml = Arc::new(build_wsdl(&cfg, &cfg.listen).map_err(ServerError::Config)?);
    let rpc_dispatchers = Arc::new(build_rpc_dispatchers(&cfg)?);

    let listener = TcpListener::bind(&cfg.listen).map_err(|e| ServerError::Bind(e.to_string()))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| ServerError::Bind(e.to_string()))?
        .to_string();

    let stop = Arc::new(AtomicBool::new(false));
    let reload_flag = Arc::new(AtomicBool::new(false));
    let healthy = Arc::new(AtomicBool::new(true));
    install_signal_watcher(Arc::clone(&stop), Arc::clone(&reload_flag))
        .map_err(|e| ServerError::Io(e.to_string()))?;

    // Admin endpoint (`/catalog`+`/healthz`+`/metrics`) — the daemon
    // family's own convention, not a DDS-WEB 1.0 requirement; see
    // `daemon::runtime_common`'s module doc.
    let registry = Arc::new(zerodds_monitor::Registry::new());
    let admin_addr = if cfg.metrics_enabled {
        let addr_str = if cfg.metrics_addr.is_empty() {
            DEFAULT_METRICS_LISTEN
        } else {
            &cfg.metrics_addr
        };
        let addr = addr_str
            .parse()
            .map_err(|_| ServerError::Config(format!("bad metrics.listen: {addr_str}")))?;
        let catalog = Arc::new(runtime_common::CatalogSnapshot::from_config(&cfg));
        runtime_common::serve_admin_endpoints(
            addr,
            catalog,
            Arc::clone(&registry),
            Arc::clone(&healthy),
            Arc::clone(&stop),
        )
        .map_err(|e| ServerError::Bind(e.to_string()))?;
        Some(addr_str.to_string())
    } else {
        None
    };

    // SIGHUP: re-reads the config and swaps in the new auth/ACL (and, via
    // `rotating`, the new TLS cert material) — matching every other
    // `zerodds-*-bridged` daemon's documented SIGHUP contract. A config
    // that fails to load or to validate changes nothing and is logged.
    if let Some(load) = reload {
        let reload_flag_watch = Arc::clone(&reload_flag);
        let stop_watch = Arc::clone(&stop);
        let sec_ctx_watch = Arc::clone(&sec_ctx);
        let rotating_watch = rotating.clone();
        let _ = thread::Builder::new()
            .name("zerodds-webdds-reload".into())
            .spawn(move || {
                while !stop_watch.load(Ordering::SeqCst) {
                    if reload_flag_watch.swap(false, Ordering::SeqCst) {
                        match load() {
                            Ok(new_cfg) => match security::ctx_from_daemon_config(&new_cfg) {
                                Ok((new_ctx, _new_rot)) => {
                                    if let Ok(mut w) = sec_ctx_watch.write() {
                                        *w = new_ctx;
                                    }
                                    // Cert/key/CA *paths* are fixed for the
                                    // process's lifetime (an `Arc<RotatingTlsConfig>`
                                    // is shared with every already-spawned
                                    // connection thread); a config that keeps
                                    // them and only renewed the file content
                                    // is what `reload()` re-reads. A config
                                    // that names *different* paths needs a
                                    // restart — logged, not applied silently.
                                    if let Some(rot) = rotating_watch.as_ref() {
                                        if let Err(e) = rot.reload() {
                                            eprintln!(
                                                "[zerodds-webdds-bridged] tls reload refused: {e}"
                                            );
                                        }
                                    }
                                }
                                Err(e) => eprintln!("[zerodds-webdds-bridged] reload refused: {e}"),
                            },
                            Err(e) => eprintln!("[zerodds-webdds-bridged] reload refused: {e}"),
                        }
                    }
                    thread::sleep(Duration::from_millis(200));
                }
            });
    }

    let max_body = cfg.max_body_bytes;
    let stop_for_loop = Arc::clone(&stop);
    let gateway_for_loop = Arc::clone(&gateway);
    let accept_thread = thread::Builder::new()
        .name("zerodds-webdds-accept".into())
        .spawn(move || {
            for stream in listener.incoming() {
                if stop_for_loop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(tcp) = stream else { continue };
                let gw = Arc::clone(&gateway_for_loop);
                let meta = Arc::clone(&metadata);
                let ctx = Arc::clone(&sec_ctx);
                let wsdl = Arc::clone(&wsdl_xml);
                let rpc = Arc::clone(&rpc_dispatchers);
                let rot = rotating.clone();
                thread::spawn(move || {
                    let web_stream = match rot.as_ref() {
                        Some(rot) => {
                            match serve_tls_handshake(rot.current(), tcp, Duration::from_secs(5)) {
                                Ok((tcp, conn, _mtls_subj)) => {
                                    WebStream::Tls(Box::new(rustls::StreamOwned::new(conn, tcp)))
                                }
                                Err(e) => {
                                    eprintln!("[zerodds-webdds-bridged] tls handshake error: {e}");
                                    return;
                                }
                            }
                        }
                        None => WebStream::Plain(tcp),
                    };
                    handle_connection(web_stream, &gw, &meta, &ctx, &wsdl, &rpc, max_body);
                });
            }
        })
        .map_err(|e| ServerError::Io(e.to_string()))?;

    Ok(DaemonHandle {
        stop,
        accept_thread: Some(accept_thread),
        local_addr,
        admin_addr,
        reload_flag,
        healthy,
        gateway,
    })
}

/// Either side of an accepted connection — plain when `tls.enabled` is
/// false, a server-side rustls stream when it is true. One `BufReader`
/// wraps it for reads; a write goes straight to the inner stream via
/// [`BufReader::get_mut`] (which never touches the read buffer), so the
/// two paths need no `Clone` — `StreamOwned`'s rustls session state
/// could not provide one anyway.
enum WebStream {
    /// `tls.enabled=false`.
    Plain(TcpStream),
    /// Server-side TLS stream on an accepted connection.
    Tls(Box<rustls::StreamOwned<rustls::ServerConnection, TcpStream>>),
}

impl WebStream {
    fn set_read_timeout(&mut self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_read_timeout(dur),
            Self::Tls(s) => s.sock.set_read_timeout(dur),
        }
    }
}

impl std::io::Read for WebStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(buf),
            Self::Tls(s) => s.read(buf),
        }
    }
}

impl std::io::Write for WebStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(buf),
            Self::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Tls(s) => s.flush(),
        }
    }
}

fn handle_connection(
    mut stream: WebStream,
    gateway: &Arc<Mutex<Gateway>>,
    metadata: &Arc<MetadataStore>,
    sec_ctx: &Arc<RwLock<SecurityCtx>>,
    wsdl_xml: &Arc<String>,
    rpc_dispatchers: &Arc<Vec<RpcDispatcher>>,
    max_body: usize,
) {
    if stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .is_err()
    {
        return;
    }
    let mut reader = BufReader::new(stream);
    loop {
        let req = match http1::read_request(&mut reader, max_body) {
            Ok(r) => r,
            Err(http1::HttpError::ConnectionClosed) => break,
            Err(http1::HttpError::BodyTooLarge) => {
                let _ = http1::write_response(
                    reader.get_mut(),
                    &Response::empty(413, "Payload Too Large"),
                    false,
                );
                break;
            }
            Err(_) => {
                let _ = http1::write_response(
                    reader.get_mut(),
                    &Response::empty(400, "Bad Request"),
                    false,
                );
                break;
            }
        };
        let keep_alive = req.keep_alive();
        let resp = route_http(&req, gateway, metadata, sec_ctx, wsdl_xml, rpc_dispatchers);
        if http1::write_response(reader.get_mut(), &resp, keep_alive).is_err() {
            break;
        }
        if !keep_alive {
            break;
        }
    }
}

fn route_http(
    req: &http1::Request,
    gateway: &Arc<Mutex<Gateway>>,
    metadata: &Arc<MetadataStore>,
    sec_ctx: &Arc<RwLock<SecurityCtx>>,
    wsdl_xml: &Arc<String>,
    rpc_dispatchers: &Arc<Vec<RpcDispatcher>>,
) -> Response {
    let path = req.path();
    if path.starts_with("/wsdl") {
        return Response::new(200, "OK", "text/xml", wsdl_xml.as_bytes().to_vec());
    }

    let ctx = sec_ctx
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let subject = match security::authenticate_http(&ctx.auth, req.header("authorization"), None) {
        Ok(s) => s,
        Err(_) => return Response::empty(401, "Unauthorized"),
    };
    let op = security::op_of_method(&req.method);
    let resource = security::application_of_path(path).unwrap_or_else(|| "*".to_string());
    let permit = security::authorize(&ctx.acl, &subject, op, &resource);
    drop(ctx);

    if path.starts_with("/soap") {
        let body_text = String::from_utf8_lossy(&req.body).to_string();
        let xml = soap_service::dispatch_envelope(gateway, rpc_dispatchers, &body_text, permit);
        return Response::new(200, "OK", "application/soap+xml", xml.into_bytes());
    }
    rest_service::dispatch(gateway, metadata, req, permit)
}

/// Builds the SOAP WSDL document: the fixed DDS-WEB object-model
/// operations plus every `interface`'s operations from `cfg.interfaces`
/// (`crate::wsdl_bridge`).
fn build_wsdl(cfg: &DaemonConfig, listen: &str) -> Result<String, String> {
    let endpoint = format!("http://{listen}/soap");
    let mut generator = zerodds_soap::wsdl::WsdlGenerator::new(
        "ZeroDDSWebDDS",
        "urn:zerodds:webdds",
        &endpoint,
        zerodds_soap::wsdl::WsdlVersion::V11,
    );
    for op in crate::wsdl_bridge::builtin_operations() {
        generator = generator.operation(op);
    }
    for idl_path in &cfg.interfaces {
        let source = std::fs::read_to_string(idl_path).map_err(|e| format!("{idl_path}: {e}"))?;
        let ast = zerodds_wire_probe::encode::model_from_idl(&source, idl_path)
            .map_err(|e| format!("{idl_path}: {e}"))?;
        let by_service =
            crate::wsdl_bridge::operations_of(&ast).map_err(|e| format!("{idl_path}: {e:?}"))?;
        for (_, ops) in by_service {
            for op in ops {
                generator = generator.operation(op);
            }
        }
    }
    Ok(generator.render())
}

/// Builds one [`RpcDispatcher`] per `cfg.interfaces` IDL file, each on
/// its own DDS-RPC client `DomainParticipant` on `cfg.domain` — the
/// config key the man page has documented since round 1 as "reserved
/// for a future default"; this is that default's first user.
///
/// Empty (no participant created at all) where `cfg.interfaces` is
/// empty — a daemon with no business interfaces configured opens no
/// DDS-RPC client participant.
fn build_rpc_dispatchers(cfg: &DaemonConfig) -> Result<Vec<RpcDispatcher>, ServerError> {
    if cfg.interfaces.is_empty() {
        return Ok(Vec::new());
    }
    let factory = zerodds_dcps::DomainParticipantFactory::instance();
    let mut out = Vec::with_capacity(cfg.interfaces.len());
    for idl_path in &cfg.interfaces {
        let source = std::fs::read_to_string(idl_path)
            .map_err(|e| ServerError::Config(format!("{idl_path}: {e}")))?;
        let ast = zerodds_wire_probe::encode::model_from_idl(&source, idl_path)
            .map_err(|e| ServerError::Config(format!("{idl_path}: {e}")))?;
        let participant = factory
            .create_participant(cfg.domain, zerodds_dcps::DomainParticipantQos::default())
            .map_err(|e| {
                ServerError::Dds(format!(
                    "rpc client participant for {idl_path} on domain {}: {e:?}",
                    cfg.domain
                ))
            })?;
        let dispatcher = RpcDispatcher::new(Arc::new(ast), participant)
            .map_err(|e| ServerError::Config(format!("{idl_path}: {e}")))?;
        out.push(dispatcher);
    }
    Ok(out)
}

/// Installs a signal watcher for `SIGTERM`/`SIGINT`/`SIGHUP`.
#[cfg(unix)]
fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP])?;
    thread::Builder::new()
        .name("zerodds-webdds-signals".into())
        .spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGTERM | SIGINT => {
                        shutdown_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                    SIGHUP => reload_flag.store(true, Ordering::SeqCst),
                    _ => {}
                }
            }
        })
}

#[cfg(not(unix))]
fn install_signal_watcher(
    _shutdown_flag: Arc<AtomicBool>,
    _reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    // Windows: signal_hook::iterator is POSIX-only. Spawn a dummy thread,
    // shutdown runs over the normal socket-close paths.
    thread::Builder::new()
        .name("zerodds-webdds-signals".into())
        .spawn(|| {})
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn free_addr() -> String {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().expect("addr").to_string()
    }

    #[test]
    fn daemon_serves_rest_and_wsdl_and_shuts_down() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.listen = free_addr();
        let mut handle = start(cfg).expect("start");

        std::thread::sleep(Duration::from_millis(50));

        let mut s = TcpStream::connect(&handle.local_addr).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(
            b"POST /dds/rest1/applications HTTP/1.1\r\nAccept: application/dds-web+xml\r\nContent-Length: 41\r\n\r\n<application><name>app1</name></application>",
        )
        .expect("write");
        let mut buf = [0u8; 4096];
        let n = s.read(&mut buf).expect("read");
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");

        let mut s2 = TcpStream::connect(&handle.local_addr).expect("connect2");
        s2.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s2.write_all(b"GET /wsdl HTTP/1.1\r\nConnection: close\r\n\r\n")
            .expect("write2");
        let mut out = Vec::new();
        s2.read_to_end(&mut out).ok();
        let resp2 = String::from_utf8_lossy(&out);
        assert!(resp2.contains("<definitions"), "{resp2}");
        assert!(resp2.contains("WriteSample"), "{resp2}");

        handle.shutdown();
        assert!(handle.stop_requested());
    }
}
