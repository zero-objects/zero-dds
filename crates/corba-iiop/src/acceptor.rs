// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IIOP server acceptor — TCP listener with a per-connection worker.
//!
//! Spec §15.7.1: the server listens on a TCP port and accepts
//! incoming GIOP connections. A worker thread is spawned per
//! connection that reads GIOP messages and dispatches them to the
//! `MessageHandler`.

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::connection::Connection;
use crate::error::IiopError;

/// Acceptor configuration.
#[derive(Debug, Clone)]
pub struct AcceptorConfig {
    /// Bind address.
    pub bind: SocketAddr,
    /// Read timeout for accepted connections.
    pub read_timeout: Option<Duration>,
    /// Write timeout for accepted connections.
    pub write_timeout: Option<Duration>,
    /// TCP nodelay. Default `true`.
    pub nodelay: bool,
    /// `accept` polling interval for the shutdown check.
    pub accept_poll_interval: Duration,
}

impl AcceptorConfig {
    /// Constructor with default timeouts.
    #[must_use]
    pub fn new(bind: SocketAddr) -> Self {
        Self {
            bind,
            read_timeout: Some(Duration::from_secs(60)),
            write_timeout: Some(Duration::from_secs(30)),
            nodelay: true,
            accept_poll_interval: Duration::from_millis(100),
        }
    }
}

/// IIOP acceptor — starts a listener thread that accepts incoming
/// connections and invokes a `MessageHandler`.
pub struct Acceptor {
    listen_addr: SocketAddr,
    shutdown_flag: Arc<AtomicBool>,
    listener_thread: Option<JoinHandle<()>>,
}

impl Acceptor {
    /// Starts the acceptor with the given connection handler.
    ///
    /// `handler` is invoked per accepted connection in its own
    /// thread. The handler is responsible for reading and replying
    /// to GIOP messages on the connection. When the handler returns,
    /// the connection is closed.
    ///
    /// # Errors
    /// `Io` on bind failure.
    pub fn start<F>(config: AcceptorConfig, handler: F) -> Result<Self, IiopError>
    where
        F: Fn(Connection) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind(config.bind)?;
        let listen_addr = listener.local_addr()?;
        listener.set_nonblocking(false)?;
        // Make the listener "short-timeout" so that shutdown checks
        // take effect promptly.
        listener.set_nonblocking(true)?;

        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shutdown_flag_inner = Arc::clone(&shutdown_flag);
        let handler = Arc::new(handler);

        let cfg = config.clone();
        let listener_thread = thread::Builder::new()
            .name(alloc::format!("iiop-acceptor-{}", listen_addr.port()))
            .spawn(move || {
                while !shutdown_flag_inner.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _peer)) => {
                            let _ = stream.set_nonblocking(false);
                            let conn = match Connection::from_stream(stream) {
                                Ok(c) => c,
                                Err(_) => continue,
                            };
                            let _ = conn.set_read_timeout(cfg.read_timeout);
                            let _ = conn.set_write_timeout(cfg.write_timeout);
                            let _ = conn.set_nodelay(cfg.nodelay);
                            let h = Arc::clone(&handler);
                            thread::Builder::new()
                                .name("iiop-conn-worker".into())
                                .spawn(move || {
                                    h(conn);
                                })
                                .ok();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(cfg.accept_poll_interval);
                        }
                        Err(_) => {
                            // Catch transient OS errors.
                            thread::sleep(cfg.accept_poll_interval);
                        }
                    }
                }
            })
            .map_err(IiopError::Io)?;

        Ok(Self {
            listen_addr,
            shutdown_flag,
            listener_thread: Some(listener_thread),
        })
    }

    /// Like [`Self::start`], but accepts **UIOP** connections on a
    /// Unix-domain-socket `path`. A stale socket file is removed beforehand.
    /// `config.bind` is ignored (only timeouts/poll interval are used);
    /// `listen_addr()` returns a `127.0.0.1:0` placeholder for UDS.
    ///
    /// # Errors
    /// `Io` on bind failure.
    #[cfg(unix)]
    pub fn start_uds<F>(
        path: &std::path::Path,
        config: AcceptorConfig,
        handler: F,
    ) -> Result<Self, IiopError>
    where
        F: Fn(Connection) + Send + Sync + 'static,
    {
        use std::os::unix::net::UnixListener;
        // Remove a stale socket file from an earlier run (otherwise EADDRINUSE).
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;

        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shutdown_flag_inner = Arc::clone(&shutdown_flag);
        let handler = Arc::new(handler);
        let cfg = config.clone();
        let listener_thread = thread::Builder::new()
            .name("uiop-acceptor".into())
            .spawn(move || {
                while !shutdown_flag_inner.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _peer)) => {
                            let _ = stream.set_nonblocking(false);
                            let conn = match Connection::from_unix_stream(stream) {
                                Ok(c) => c,
                                Err(_) => continue,
                            };
                            let _ = conn.set_read_timeout(cfg.read_timeout);
                            let _ = conn.set_write_timeout(cfg.write_timeout);
                            let h = Arc::clone(&handler);
                            thread::Builder::new()
                                .name("uiop-conn-worker".into())
                                .spawn(move || {
                                    h(conn);
                                })
                                .ok();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(cfg.accept_poll_interval);
                        }
                        Err(_) => {
                            thread::sleep(cfg.accept_poll_interval);
                        }
                    }
                }
            })
            .map_err(IiopError::Io)?;

        let placeholder = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 0);
        Ok(Self {
            listen_addr: placeholder,
            shutdown_flag,
            listener_thread: Some(listener_thread),
        })
    }

    /// Like [`Self::start`], but accepts **SSLIOP** connections: each
    /// incoming TCP stream is wrapped via a rustls `ServerConfig` into a TLS
    /// [`Connection`] (handshake is lazy on the first GIOP read). The
    /// GIOP dispatch in `handler` is identical to the plain-IIOP path.
    ///
    /// # Errors
    /// `Io` on bind failure.
    #[cfg(feature = "tls")]
    pub fn start_tls<F>(
        config: AcceptorConfig,
        tls_config: Arc<rustls::ServerConfig>,
        handler: F,
    ) -> Result<Self, IiopError>
    where
        F: Fn(Connection) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind(config.bind)?;
        let listen_addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;

        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shutdown_flag_inner = Arc::clone(&shutdown_flag);
        let handler = Arc::new(handler);
        let cfg = config.clone();

        let listener_thread = thread::Builder::new()
            .name(alloc::format!("ssliop-acceptor-{}", listen_addr.port()))
            .spawn(move || {
                while !shutdown_flag_inner.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _peer)) => {
                            let _ = stream.set_nonblocking(false);
                            let conn = match crate::tls::accept_tls(stream, Arc::clone(&tls_config))
                            {
                                Ok(c) => c,
                                Err(_) => continue,
                            };
                            let _ = conn.set_read_timeout(cfg.read_timeout);
                            let _ = conn.set_write_timeout(cfg.write_timeout);
                            let _ = conn.set_nodelay(cfg.nodelay);
                            let h = Arc::clone(&handler);
                            thread::Builder::new()
                                .name("ssliop-conn-worker".into())
                                .spawn(move || h(conn))
                                .ok();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(cfg.accept_poll_interval);
                        }
                        Err(_) => thread::sleep(cfg.accept_poll_interval),
                    }
                }
            })
            .map_err(IiopError::Io)?;

        Ok(Self {
            listen_addr,
            shutdown_flag,
            listener_thread: Some(listener_thread),
        })
    }

    /// Returns the effective listen address (e.g. after `port = 0`
    /// auto-allocation).
    #[must_use]
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    /// Stops the acceptor (signals the listener thread to leave the
    /// loop). Existing connection workers keep running until their
    /// handlers return.
    pub fn shutdown(mut self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
        if let Some(t) = self.listener_thread.take() {
            // Best-effort join — we don't block if the thread has
            // already finished.
            let _ = t.join();
        }
    }
}

impl Drop for Acceptor {
    fn drop(&mut self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
        if let Some(t) = self.listener_thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::connector::{Connector, ConnectorConfig};
    use std::sync::atomic::AtomicUsize;
    use zerodds_cdr::Endianness;
    use zerodds_corba_giop::{
        Message, Request, ResponseFlags, ServiceContextList, TargetAddress, Version,
    };

    #[test]
    fn round_trip_via_acceptor_and_connector() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_handler = Arc::clone(&counter);

        let acceptor = Acceptor::start(
            AcceptorConfig::new("127.0.0.1:0".parse().unwrap()),
            move |mut conn| {
                while let Ok(msg) = conn.read_message() {
                    counter_handler.fetch_add(1, Ordering::Relaxed);
                    // Echo reply with the request ID from the original request.
                    if let Message::Request(req) = msg {
                        let reply = Message::Reply(zerodds_corba_giop::Reply {
                            request_id: req.request_id,
                            reply_status: zerodds_corba_giop::ReplyStatusType::NoException,
                            service_context: ServiceContextList::default(),
                            body: req.body.clone(),
                        });
                        let _ = conn.write_message(Version::V1_2, Endianness::Big, false, &reply);
                    }
                }
            },
        )
        .unwrap();

        let addr = acceptor.listen_addr();
        let connector = Connector::new(ConnectorConfig::default());
        let mut pooled = connector
            .connect(&addr.ip().to_string(), addr.port())
            .unwrap();
        let conn = pooled.connection().unwrap();

        let request = Message::Request(Request {
            request_id: 42,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0xab]),
            operation: "ping".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        });
        conn.write_message(Version::V1_2, Endianness::Big, false, &request)
            .unwrap();
        let reply = conn.read_message().unwrap();
        match reply {
            Message::Reply(r) => {
                assert_eq!(r.request_id, 42);
                assert_eq!(r.body, alloc::vec![1, 2, 3, 4, 5, 6, 7, 8]);
            }
            other => panic!("expected Reply, got {other:?}"),
        }
        assert!(counter.load(Ordering::Relaxed) >= 1);

        acceptor.shutdown();
    }

    /// Full SSLIOP server loop: Acceptor::start_tls dispatches GIOP over TLS,
    /// the connect_tls client round-trips Request→Reply.
    #[cfg(feature = "tls")]
    #[test]
    fn ssliop_round_trip_via_acceptor_tls() {
        let ck = rcgen::generate_simple_self_signed(alloc::vec!["localhost".to_string()]).unwrap();
        let cert = ck.cert.pem();
        let key = ck.key_pair.serialize_pem();
        let server_cfg = crate::tls::load_server_config(cert.as_bytes(), key.as_bytes()).unwrap();
        let client_cfg = crate::tls::load_client_config_trusting(cert.as_bytes()).unwrap();

        let acceptor = Acceptor::start_tls(
            AcceptorConfig::new("127.0.0.1:0".parse().unwrap()),
            server_cfg,
            move |mut conn| {
                while let Ok(msg) = conn.read_message() {
                    if let Message::Request(req) = msg {
                        let reply = Message::Reply(zerodds_corba_giop::Reply {
                            request_id: req.request_id,
                            reply_status: zerodds_corba_giop::ReplyStatusType::NoException,
                            service_context: ServiceContextList::default(),
                            body: req.body.clone(),
                        });
                        let _ = conn.write_message(Version::V1_2, Endianness::Big, false, &reply);
                    }
                }
            },
        )
        .unwrap();

        let addr = acceptor.listen_addr();
        let mut conn =
            crate::tls::connect_tls("localhost", addr.port(), "localhost", client_cfg).unwrap();
        let request = Message::Request(Request {
            request_id: 7,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0xab]),
            operation: "ping".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![9, 8, 7, 6],
        });
        conn.write_message(Version::V1_2, Endianness::Big, false, &request)
            .unwrap();
        match conn.read_message().unwrap() {
            Message::Reply(r) => {
                assert_eq!(r.request_id, 7);
                assert_eq!(r.body, alloc::vec![9, 8, 7, 6]);
            }
            other => panic!("expected Reply, got {other:?}"),
        }
        acceptor.shutdown();
    }

    #[test]
    fn acceptor_picks_random_port_with_zero() {
        let acceptor = Acceptor::start(
            AcceptorConfig::new("127.0.0.1:0".parse().unwrap()),
            |_conn| {},
        )
        .unwrap();
        assert!(acceptor.listen_addr().port() != 0);
        acceptor.shutdown();
    }
}
