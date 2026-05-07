// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Multi-Connection-Accept-Loop mit thread-per-connection.
//!
//! Spec dds-amqp-1.0 §2.1 Cl. 2 — Server-Side, akzeptiert
//! eingehende AMQP-1.0-Connections.

use std::io;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use zerodds_amqp_endpoint::MetricsHub;

use crate::handler::{HandlerConfig, handle_connection};

// Re-export fuer einheitlichen Public-Surface.
pub use crate::frame_io::AmqpProtocol;

/// Server-Konfiguration fuer `run_server`.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Listen-Address (z.B. `"0.0.0.0:5672"`).
    pub listen_addr: String,
    /// Container-Id, die der Server in Open-Frames meldet.
    pub container_id: String,
    /// Max Frame-Size (DoS-Cap).
    pub max_frame_size: u32,
    /// Ist TLS aktiv? (Beeinflusst SASL-PLAIN-Akzeptanz §10.2.1.)
    pub tls_active: bool,
    /// Read-Timeout pro Connection (None = unlimited; im Daemon
    /// typisch 60s = idle-timeout).
    pub read_timeout: Option<Duration>,
    /// Per-Connection Write-Timeout.
    pub write_timeout: Option<Duration>,
}

impl ServerConfig {
    /// Default-Konfiguration fuer `0.0.0.0:5672`.
    #[must_use]
    pub fn default_listen() -> Self {
        Self {
            listen_addr: "0.0.0.0:5672".to_string(),
            container_id: "zerodds-amqp-endpoint".to_string(),
            max_frame_size: 1_048_576,
            tls_active: false,
            read_timeout: Some(Duration::from_secs(60)),
            write_timeout: Some(Duration::from_secs(60)),
        }
    }
}

/// Server-Fehler.
#[derive(Debug)]
pub enum ServerError {
    /// Bind-Fehler.
    Bind(io::Error),
    /// Allgemeiner IO-Fehler.
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
/// Akzeptiert blocking auf `listen_addr`, spawnt pro
/// akzeptierter Connection einen Thread, der
/// `handle_connection` durchlaeuft.
///
/// `shutdown_signal` ist ein Atomic-Flag, das von aussen auf
/// `true` gesetzt werden kann — der Accept-Loop pollt es
/// periodisch (per `set_nonblocking` + sleep) und beendet
/// sauber.
///
/// `metrics` ist der prozess-globale Counter-Hub, der pro
/// Connection und pro Frame inkrementiert wird.
///
/// # Errors
/// `Bind` wenn der Listener nicht binden kann; `Io` bei
/// schweren Accept-Fehlern.
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
    if let Some(t) = cfg.read_timeout {
        stream.set_read_timeout(Some(t))?;
    }
    if let Some(t) = cfg.write_timeout {
        stream.set_write_timeout(Some(t))?;
    }
    let mut handler_cfg = HandlerConfig::for_tests(metrics.clone());
    handler_cfg.container_id = cfg.container_id.clone();
    handler_cfg.max_frame_size = cfg.max_frame_size;
    handler_cfg.tls_active = cfg.tls_active;
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
        assert!(!c.tls_active);
        assert!(c.read_timeout.is_some());
    }

    /// E2E: Server bindet auf 127.0.0.1:0 (zufaelliger Port),
    /// Klient connectet, schickt AMQP-Header + Close.
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
            tls_active: false,
            read_timeout: Some(Duration::from_secs(2)),
            write_timeout: Some(Duration::from_secs(2)),
        };

        // Listener wieder freigeben — der Test-Server bindet
        // ihn neu in einem Thread.
        drop(listener);

        let server_metrics = metrics.clone();
        let server_shutdown = shutdown.clone();
        let server_thread = thread::spawn(move || {
            let _ = run_server(cfg, server_metrics, server_shutdown);
        });

        // Bisschen warten bis der Server bindet.
        thread::sleep(Duration::from_millis(100));

        // Klient connectet.
        let mut client = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();

        // AMQP-Protocol-Header senden.
        client.write_all(&AmqpProtocol::Amqp.as_bytes()).unwrap();

        // Open-Performative + Close-Performative senden.
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

        // Server schickt: AMQP-Header + Open-Reply + Close-Reply.
        let mut buf = [0u8; 8];
        std::io::Read::read_exact(&mut client, &mut buf).unwrap();
        assert_eq!(&buf[0..4], b"AMQP");

        // Wir lesen nicht alle Server-Frames detailliert; uns
        // reicht: Server hat geantwortet + Stats sind hochgezaehlt.
        // Klient schliesst.
        drop(client);

        // Bisschen warten bis Server-Side fertig ist.
        thread::sleep(Duration::from_millis(200));

        assert_eq!(metrics.snapshot("connections.total"), Some(1));

        // Shutdown.
        shutdown.store(true, Ordering::Relaxed);
        server_thread.join().unwrap();
    }
}
