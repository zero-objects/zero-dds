// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Mini HTTP server for the Prometheus `/metrics` endpoint (spec §6.3).
//!
//! Pure Rust without a hyper dep. Sufficient for Prometheus scrape endpoints
//! and local tools.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::Registry;

/// Server error.
#[derive(Debug)]
pub enum ServeError {
    /// Bind/accept/IO error.
    Io(std::io::Error),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for ServeError {}

impl From<std::io::Error> for ServeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Starts a blocking mini HTTP server that exposes `/metrics` on
/// `addr`. Returns a `JoinHandle` — shutdown via dropping the
/// `TcpListener` by having the server thread make a new `accept()`
/// and the listener drop wake it up.
///
/// For production workloads an upstream reverse proxy
/// (Nginx, Envoy) should handle rate limiting and TLS — this
/// server is intentionally minimal.
pub fn serve_prometheus(
    addr: SocketAddr,
    registry: Arc<Registry>,
) -> Result<JoinHandle<()>, ServeError> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(false).ok();

    let handle = thread::Builder::new()
        .name("zerodds-monitor-prom".into())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut s) => {
                        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                        handle_connection(&mut s, &registry);
                    }
                    Err(_) => break,
                }
            }
        })
        .map_err(ServeError::Io)?;

    Ok(handle)
}

fn handle_connection(stream: &mut std::net::TcpStream, registry: &Registry) {
    let mut buf = [0u8; 1024];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let request_line = match std::str::from_utf8(&buf[..n]) {
        Ok(s) => s.lines().next().unwrap_or(""),
        Err(_) => return,
    };

    let body = if request_line.contains(" /metrics ") || request_line.contains(" /metrics?") {
        registry.render_prometheus()
    } else {
        String::new()
    };

    let status = if body.is_empty() && !request_line.contains("/metrics") {
        "404 Not Found"
    } else {
        "200 OK"
    };

    let resp = format!(
        "HTTP/1.1 {}\r\n\
         Content-Type: text/plain; version=0.0.4; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status,
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes());
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::{Labels, Registry};
    use std::io::Read;
    use std::net::TcpStream;
    use std::sync::Arc;
    use std::time::Duration;

    fn pick_free_port() -> u16 {
        let s = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        s.local_addr().expect("addr").port()
    }

    #[test]
    fn server_serves_metrics_on_request() {
        let port = pick_free_port();
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");
        let reg = Arc::new(Registry::new());
        reg.set_help("dds_test_total", "Test counter");
        reg.counter("dds_test_total", Labels::new()).add(7);

        let _h = serve_prometheus(addr, Arc::clone(&reg)).expect("serve");
        // give server a moment to enter accept-loop
        std::thread::sleep(Duration::from_millis(50));

        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("write");
        let mut buf = String::new();
        s.read_to_string(&mut buf).ok();
        assert!(buf.contains("HTTP/1.1 200 OK"), "got: {buf}");
        assert!(buf.contains("dds_test_total 7"), "got: {buf}");
    }

    #[test]
    fn server_404s_unknown_path() {
        let port = pick_free_port();
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");
        let reg = Arc::new(Registry::new());
        let _h = serve_prometheus(addr, Arc::clone(&reg)).expect("serve");
        std::thread::sleep(Duration::from_millis(50));

        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /foo HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("write");
        let mut buf = String::new();
        s.read_to_string(&mut buf).ok();
        assert!(buf.contains("404 Not Found"), "got: {buf}");
    }
}
