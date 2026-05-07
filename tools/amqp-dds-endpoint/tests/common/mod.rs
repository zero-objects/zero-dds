//! Geteilte Test-Helpers fuer Annex-C E2E-Tests.
//!
//! Pro Test-File spawnen wir einen synchronen Server in einem
//! Thread, der genau eine (oder N) Connection(s) bedient. Client
//! verbindet als `TcpStream` und faehrt die Annex-C-Tests aus.

#![allow(dead_code, clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use amqp_dds_endpoint::handler::{HandlerConfig, handle_connection};
use zerodds_amqp_endpoint::MetricsHub;

/// Aufgesetzte Test-Server-Instanz.
pub struct TestServer {
    pub port: u16,
    pub metrics: Arc<MetricsHub>,
    pub shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Spawn einen Server, der bis zu `max_connections` Verbindungen
    /// nacheinander bedient (single-threaded je Connection per
    /// thread::spawn).
    pub fn spawn(cfg: HandlerConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        listener.set_nonblocking(true).expect("set_nonblocking");

        let metrics = cfg.metrics.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_loop = shutdown.clone();
        let handle = thread::spawn(move || {
            let cfg = cfg;
            while !shutdown_loop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut sock, _)) => {
                        // Auf macOS/Linux erbt die accepted Socket die
                        // nonblocking-Flag vom Listener. Wir muessen
                        // sie explizit zurueck auf blocking setzen,
                        // damit `read_exact` funktioniert.
                        let _ = sock.set_nonblocking(false);
                        let _ = sock.set_read_timeout(Some(Duration::from_secs(5)));
                        let _ = sock.set_write_timeout(Some(Duration::from_secs(5)));
                        let cfg = cfg.clone();
                        thread::spawn(move || {
                            if let Err(e) = handle_connection(&mut sock, &cfg) {
                                eprintln!("[test-server] handle_connection: {e}");
                            }
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => break,
                }
            }
        });

        // Geben dem Server einen Moment zum Binden.
        thread::sleep(Duration::from_millis(50));

        Self {
            port,
            metrics,
            shutdown,
            handle: Some(handle),
        }
    }

    /// Shutdown + join.
    pub fn shutdown(mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            // Server-Loop pollt alle 20ms; geben wir ihm Zeit.
            thread::sleep(Duration::from_millis(50));
            let _ = h.join();
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            thread::sleep(Duration::from_millis(50));
            let _ = h.join();
        }
    }
}

/// Verbindet einen TcpStream zum Test-Server.
pub fn connect_client(port: u16) -> TcpStream {
    let s = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    s
}

/// Frischer HandlerConfig fuer Tests, ohne TLS, mit AllowAll.
pub fn test_handler_cfg() -> HandlerConfig {
    HandlerConfig::for_tests(Arc::new(MetricsHub::new()))
}

/// Frischer HandlerConfig mit TLS-aktiv-Marker (wirkt nur auf
/// SASL-PLAIN-Akzeptanz; echtes TLS ist out-of-scope dieses
/// Daemon-Builds).
pub fn test_handler_cfg_with_tls(tls_active: bool) -> HandlerConfig {
    let mut c = HandlerConfig::for_tests(Arc::new(MetricsHub::new()));
    c.tls_active = tls_active;
    c
}
