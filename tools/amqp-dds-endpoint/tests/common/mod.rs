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
    /// Spawn a server that serves up to `max_connections` connections
    /// served sequentially (single-threaded per connection per
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
                        // On macOS/Linux the accepted socket inherits the
                        // nonblocking flag from the listener. We must
                        // set it explicitly back to blocking,
                        // so that `read_exact` works.
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

        // Give the server a moment to bind.
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
            // The server loop polls every 20ms; give it time.
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
