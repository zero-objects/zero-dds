// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IIOP Server-Acceptor — TCP-Listener mit Per-Connection-Worker.
//!
//! Spec §15.7.1: Server lauscht auf einem TCP-Port und akzeptiert
//! eingehende GIOP-Connections. Pro Connection wird ein
//! Worker-Thread gestartet, der GIOP-Messages liest und an den
//! `MessageHandler` dispatcht.

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::connection::Connection;
use crate::error::IiopError;

/// Konfiguration des Acceptors.
#[derive(Debug, Clone)]
pub struct AcceptorConfig {
    /// Bind-Address.
    pub bind: SocketAddr,
    /// Read-Timeout fuer akzeptierte Connections.
    pub read_timeout: Option<Duration>,
    /// Write-Timeout fuer akzeptierte Connections.
    pub write_timeout: Option<Duration>,
    /// TCP-Nodelay. Default `true`.
    pub nodelay: bool,
    /// `accept`-Polling-Intervall fuer Shutdown-Check.
    pub accept_poll_interval: Duration,
}

impl AcceptorConfig {
    /// Konstruktor mit Default-Timeouts.
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

/// IIOP-Acceptor — startet einen Listener-Thread, der eingehende
/// Connections akzeptiert und einen `MessageHandler` aufruft.
pub struct Acceptor {
    listen_addr: SocketAddr,
    shutdown_flag: Arc<AtomicBool>,
    listener_thread: Option<JoinHandle<()>>,
}

impl Acceptor {
    /// Startet den Acceptor mit dem gegebenen Connection-Handler.
    ///
    /// `handler` wird pro akzeptierter Connection in einem eigenen
    /// Thread aufgerufen. Der Handler ist verantwortlich fuer das
    /// Lesen + Beantworten von GIOP-Messages auf der Connection.
    /// Wenn der Handler returned, wird die Connection geschlossen.
    ///
    /// # Errors
    /// `Io` bei Bind-Failure.
    pub fn start<F>(config: AcceptorConfig, handler: F) -> Result<Self, IiopError>
    where
        F: Fn(Connection) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind(config.bind)?;
        let listen_addr = listener.local_addr()?;
        listener.set_nonblocking(false)?;
        // Wir machen den listener "kurz-time-out", damit Shutdown-
        // Checks zeitnah greifen.
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
                            // Transient OS-Errors abfangen.
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

    /// Liefert die effektive Listen-Address (z.B. nach `port = 0`-
    /// Auto-Allokation).
    #[must_use]
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    /// Stoppt den Acceptor (signalisiert dem Listener-Thread, dass
    /// er die Schleife verlassen soll). Bestehende Connection-Worker
    /// laufen weiter, bis ihre Handler returnen.
    pub fn shutdown(mut self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
        if let Some(t) = self.listener_thread.take() {
            // Best-Effort-Join — wir blockieren nicht, falls der
            // Thread bereits beendet ist.
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
                    // Echo-Reply mit dem Request-ID aus dem Original-Request.
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
