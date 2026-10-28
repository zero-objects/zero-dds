// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Multi-connection accept loop with thread-per-connection.
//!
//! Spec dds-amqp-1.0 §2.1 Cl. 2 — server side, accepts
//! incoming AMQP-1.0 connections.

use std::io;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use zerodds_amqp_endpoint::MetricsHub;

use crate::handler::{HandlerConfig, handle_connection};

// Re-export for a uniform public surface.
pub use crate::frame_io::AmqpProtocol;

/// Server configuration for `run_server`.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Listen address (e.g. `"0.0.0.0:5672"`).
    pub listen_addr: String,
    /// Container-Id that the server reports in Open frames.
    pub container_id: String,
    /// Max frame size (DoS cap).
    pub max_frame_size: u32,
    /// Read timeout per connection (None = unlimited; in the daemon
    /// typically 60s = idle timeout).
    pub read_timeout: Option<Duration>,
    /// Per-connection write timeout.
    pub write_timeout: Option<Duration>,
    /// The SASL mechanisms and the credential store (DDS-AMQP 1.0 §10.2).
    pub sasl: crate::auth::SaslPolicy,
    /// TLS termination (§10.1); `None` = plaintext. `tls_active` is set from
    /// it: a connection is TLS exactly when this wraps it.
    #[cfg(feature = "tls")]
    pub tls: Option<Arc<rustls::ServerConfig>>,
}

impl ServerConfig {
    /// Default configuration for `0.0.0.0:5672`.
    #[must_use]
    pub fn default_listen() -> Self {
        Self {
            listen_addr: "0.0.0.0:5672".to_string(),
            container_id: "zerodds-amqp-endpoint".to_string(),
            max_frame_size: 1_048_576,
            read_timeout: Some(Duration::from_secs(60)),
            write_timeout: Some(Duration::from_secs(60)),
            sasl: crate::auth::SaslPolicy::default(),
            #[cfg(feature = "tls")]
            tls: None,
        }
    }
}

/// Server error.
#[derive(Debug)]
pub enum ServerError {
    /// Bind error.
    Bind(io::Error),
    /// General IO error.
    Io(io::Error),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bind(e) => write!(f, "bind error: {e}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// Spec §2.1 Cl. 2 — TCP-Listener-Loop.
///
/// Accepts blocking on `listen_addr`, spawns per
/// accepted connection a thread that
/// runs `handle_connection`.
///
/// `shutdown_signal` is an atomic flag that is set externally to
/// can be set to `true` — the accept loop polls it
/// periodically (via `set_nonblocking` + sleep) and terminates
/// cleanly.
///
/// `metrics` is the process-global counter hub that per
/// connection and per frame.
///
/// # Errors
/// `Bind` if the listener cannot bind; `Io` on
/// severe accept errors.
pub fn run_server(
    cfg: ServerConfig,
    metrics: Arc<MetricsHub>,
    shutdown_signal: Arc<AtomicBool>,
) -> Result<(), ServerError> {
    let listener = bind_listener(&cfg.listen_addr).map_err(ServerError::Bind)?;
    listener.set_nonblocking(true).map_err(ServerError::Io)?;

    eprintln!(
        "amqp-dds-endpoint listening on {} (container_id={}, max_frame_size={})",
        cfg.listen_addr, cfg.container_id, cfg.max_frame_size
    );

    while !shutdown_signal.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, peer)) => {
                let cfg = cfg.clone();
                let metrics = metrics.clone();
                let _ = thread::Builder::new()
                    .name(format!("amqp-conn-{peer}"))
                    .spawn(move || {
                        if let Err(e) = serve_one(stream, &cfg, &metrics) {
                            eprintln!("connection from {peer} ended: {e}");
                        }
                    });
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                eprintln!("accept error: {e}");
                return Err(ServerError::Io(e));
            }
        }
    }
    eprintln!("amqp-dds-endpoint shutting down");
    Ok(())
}

fn bind_listener<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
    TcpListener::bind(addr)
}

fn serve_one(
    mut stream: TcpStream,
    cfg: &ServerConfig,
    metrics: &Arc<MetricsHub>,
) -> Result<(), Box<dyn std::error::Error>> {
    // The listener is nonblocking for the shutdown poll; on macOS and the
    // BSDs an accepted socket inherits that, and the first read of the
    // blocking handler failed with WouldBlock (Linux does not inherit it).
    stream.set_nonblocking(false)?;
    if let Some(t) = cfg.read_timeout {
        stream.set_read_timeout(Some(t))?;
    }
    if let Some(t) = cfg.write_timeout {
        stream.set_write_timeout(Some(t))?;
    }
    let mut handler_cfg = HandlerConfig::for_tests(metrics.clone());
    handler_cfg.container_id = cfg.container_id.clone();
    handler_cfg.max_frame_size = cfg.max_frame_size;
    handler_cfg.sasl = cfg.sasl.clone();
    #[cfg(feature = "tls")]
    if let Some(tls) = &cfg.tls {
        let mut tls_stream =
            crate::tls::accept_server(Arc::clone(tls), stream).map_err(|e| format!("TLS: {e}"))?;
        handler_cfg.tls_active = true;
        handler_cfg.client_cert = tls_stream
            .conn
            .peer_certificates()
            .and_then(|c| c.first())
            .map(|c| c.as_ref().to_vec());
        handle_connection(&mut tls_stream, &handler_cfg)?;
        return Ok(());
    }
    // Plaintext: PLAIN is not offered (§10.2.1).
    handler_cfg.tls_active = false;
    handle_connection(&mut stream, &handler_cfg)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpStream;

    #[test]
    fn server_config_default_has_sensible_values() {
        let c = ServerConfig::default_listen();
        assert!(c.listen_addr.ends_with(":5672"));
        assert!(c.max_frame_size >= 65_536);
        assert!(c.sasl.allows_no_sasl());
        assert!(c.read_timeout.is_some());
    }

    /// E2E: server binds on 127.0.0.1:0 (random port),
    /// client connects, sends AMQP header + Close.
    #[test]
    fn server_accepts_connection_and_handles_open_close() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let metrics = Arc::new(MetricsHub::new());
        let shutdown = Arc::new(AtomicBool::new(false));

        let cfg = ServerConfig {
            listen_addr: format!("127.0.0.1:{port}"),
            container_id: "test-server".into(),
            max_frame_size: 65_536,
            read_timeout: Some(Duration::from_secs(2)),
            write_timeout: Some(Duration::from_secs(2)),
            sasl: crate::auth::SaslPolicy::default(),
            #[cfg(feature = "tls")]
            tls: None,
        };

        // Release the listener again — the test server binds
        // it anew in a thread.
        drop(listener);

        let server_metrics = metrics.clone();
        let server_shutdown = shutdown.clone();
        let server_thread = thread::spawn(move || {
            let _ = run_server(cfg, server_metrics, server_shutdown);
        });

        // Wait for the server to bind, by trying to reach it.
        //
        // This used to be `sleep(100ms)` and a single connect, which
        // assumes the thread above got scheduled, bound the port and
        // reached its accept loop inside that window. On a loaded
        // machine it does not, and the test fails for a reason that has
        // nothing to do with the server. Retrying until a deadline
        // waits for the event rather than for a duration — the test
        // still fails if the server never comes up, which is what it is
        // there to catch.
        let addr = format!("127.0.0.1:{port}");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut client = loop {
            match TcpStream::connect(&addr) {
                Ok(stream) => break stream,
                Err(error) => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "the server never accepted a connection on {addr}: {error}"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
            }
        };
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();

        // Send AMQP protocol header.
        client.write_all(&AmqpProtocol::Amqp.as_bytes()).unwrap();

        // Send Open + Close performatives.
        let open = zerodds_amqp_bridge::performatives::open("client").unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + open.len() as u32,
            doff: 2,
            frame_type: zerodds_amqp_bridge::frame::FrameType::Amqp,
            channel: 0,
        };
        client
            .write_all(&zerodds_amqp_bridge::frame::encode_frame_header(h))
            .unwrap();
        client.write_all(&open).unwrap();

        let close = zerodds_amqp_bridge::performatives::close().unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + close.len() as u32,
            doff: 2,
            frame_type: zerodds_amqp_bridge::frame::FrameType::Amqp,
            channel: 0,
        };
        client
            .write_all(&zerodds_amqp_bridge::frame::encode_frame_header(h))
            .unwrap();
        client.write_all(&close).unwrap();

        // Server sends: AMQP header + Open reply + Close reply.
        let mut buf = [0u8; 8];
        std::io::Read::read_exact(&mut client, &mut buf).unwrap();
        assert_eq!(&buf[0..4], b"AMQP");

        // We do not read all server frames in detail; we
        // is enough: server has responded + stats have counted up.
        // Client closes.
        drop(client);

        // Wait a little until the server side is ready.
        thread::sleep(Duration::from_millis(200));

        assert_eq!(metrics.snapshot("connections.total"), Some(1));

        // Shutdown.
        shutdown.store(true, Ordering::Relaxed);
        server_thread.join().unwrap();
    }
}
