//! Shared test helpers for Annex-C E2E tests.
//!
//! Per test file we spawn a synchronous server in a
//! thread that serves exactly one (or N) connection(s). The client
//! connects as a `TcpStream` and runs the Annex-C tests.

#![allow(dead_code, clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use amqp_dds_endpoint::handler::{HandlerConfig, handle_connection};
use zerodds_amqp_endpoint::MetricsHub;

/// Set up test server instance.
pub struct TestServer {
    pub port: u16,
    pub metrics: Arc<MetricsHub>,
    pub shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// A server on a port of its own: every accepted connection is handled
    /// in a thread of its own.
    pub fn spawn(cfg: HandlerConfig) -> Self {
        Self::on(TcpListener::bind("127.0.0.1:0").expect("bind"), cfg)
    }

    /// A server on `port` (a port the test reserved and released).
    pub fn spawn_on(port: u16, cfg: HandlerConfig) -> Self {
        Self::on(TcpListener::bind(("127.0.0.1", port)).expect("bind"), cfg)
    }

    fn on(listener: TcpListener, cfg: HandlerConfig) -> Self {
        // The listener is bound and listening before this returns: a
        // client's connect queues until the accept below takes it.
        let port = listener.local_addr().expect("addr").port();
        let metrics = cfg.metrics.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_loop = shutdown.clone();
        let handle = thread::spawn(move || {
            for incoming in listener.incoming() {
                // `shutdown` wakes this accept with a connect of its own.
                if shutdown_loop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut sock) = incoming else { break };
                let _ = sock.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = sock.set_write_timeout(Some(Duration::from_secs(5)));
                let cfg = cfg.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_connection(&mut sock, &cfg) {
                        eprintln!("[test-server] handle_connection: {e}");
                    }
                });
            }
        });
        Self {
            port,
            metrics,
            shutdown,
            handle: Some(handle),
        }
    }

    /// Shutdown + join.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = TcpStream::connect(("127.0.0.1", self.port));
            let _ = h.join();
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Connects a TcpStream to the test server.
pub fn connect_client(port: u16) -> TcpStream {
    let s = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    s
}

/// Fresh HandlerConfig for tests, without TLS, with AllowAll.
pub fn test_handler_cfg() -> HandlerConfig {
    HandlerConfig::for_tests(Arc::new(MetricsHub::new()))
}

/// Fresh HandlerConfig with TLS-active marker (only affects
/// SASL-PLAIN acceptance; real TLS is out of scope for this
/// Daemon-Builds).
pub fn test_handler_cfg_with_tls(tls_active: bool) -> HandlerConfig {
    let mut c = HandlerConfig::for_tests(Arc::new(MetricsHub::new()));
    c.tls_active = tls_active;
    c
}
