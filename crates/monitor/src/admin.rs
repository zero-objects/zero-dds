// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The admin endpoints of a daemon (spec §6.4): `/catalog`, `/healthz`,
//! the Prometheus text at the daemon's metrics path and an index at `/`,
//! one HTTP/1.1 request per connection.
//!
//! The bridge daemons serve these four paths on their admin port. The
//! accept loop, the request line and the responses are here; what the
//! catalog holds each daemon renders itself.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::Registry;

/// An HTTP/1.1 response that ends the connection.
#[must_use]
pub fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// The response of `/healthz`: `200 OK` with `OK` while `healthy` is set,
/// `503 Service Unavailable` with `DOWN` otherwise.
#[must_use]
pub fn health_response(healthy: &AtomicBool) -> String {
    if healthy.load(Ordering::SeqCst) {
        http_response("200 OK", "text/plain; charset=utf-8", "OK\n")
    } else {
        http_response(
            "503 Service Unavailable",
            "text/plain; charset=utf-8",
            "DOWN\n",
        )
    }
}

/// The path a request asks for: the target of its request line without
/// the query, `/` when the line names none.
#[must_use]
pub fn request_path(request: &str) -> &str {
    request
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
}

/// The four paths of a daemon's admin port.
pub struct AdminEndpoints<F> {
    /// The daemon's name, the first line of `/`.
    pub service_name: String,
    /// The path of the Prometheus text (`metrics.path` of a daemon that
    /// has the key, `/metrics` otherwise).
    pub metrics_path: String,
    /// Renders the JSON of `/catalog`.
    pub catalog: F,
    /// The metrics the Prometheus path renders.
    pub registry: Arc<Registry>,
    /// `/healthz` answers `200 OK` while this is set.
    pub healthy: Arc<AtomicBool>,
}

impl<F: Fn() -> String> AdminEndpoints<F> {
    /// The response to a request for `path`; `404 Not Found` for a path
    /// that is none of the four.
    #[must_use]
    pub fn response(&self, path: &str) -> String {
        match path {
            "/catalog" => http_response("200 OK", "application/json", &(self.catalog)()),
            "/healthz" => health_response(&self.healthy),
            p if p == self.metrics_path => http_response(
                "200 OK",
                "text/plain; version=0.0.4; charset=utf-8",
                &self.registry.render_prometheus(),
            ),
            "/" => http_response(
                "200 OK",
                "text/plain; charset=utf-8",
                &format!(
                    "{}\nendpoints: /catalog /healthz {}\n",
                    self.service_name, self.metrics_path
                ),
            ),
            _ => http_response("404 Not Found", "text/plain; charset=utf-8", ""),
        }
    }

    /// Reads one request (its first 1024 bytes) from `stream` and writes
    /// the response. A stream that cannot be read gets no response.
    pub fn handle(&self, stream: &mut TcpStream) {
        let mut buf = [0u8; 1024];
        let Ok(n) = stream.read(&mut buf) else {
            return;
        };
        let request = String::from_utf8_lossy(&buf[..n]);
        let _ = stream.write_all(self.response(request_path(&request)).as_bytes());
    }
}

/// Serves `endpoints` on `addr` from the thread `thread_name` until `stop`
/// is set ([`serve_connections`]). Returns the thread and the address the
/// listener is bound to, which names the port when `addr` asked for port 0.
///
/// # Errors
/// The bind of `addr`, and the start of the thread.
pub fn serve_admin_endpoints<F>(
    addr: SocketAddr,
    thread_name: &str,
    endpoints: AdminEndpoints<F>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)>
where
    F: Fn() -> String + Send + 'static,
{
    serve_connections(addr, thread_name, stop, move |stream| {
        endpoints.handle(stream);
    })
}

/// Binds `addr` and hands each accepted connection to `handle` on the
/// thread `thread_name`, the connection blocking with read and write
/// timeouts of 2 s. The loop ends when `stop` is set: it looks at the
/// flag before each accept, every 50 ms while no connection waits. Returns
/// the thread and the bound address.
///
/// # Errors
/// The bind of `addr`, and the start of the thread.
pub fn serve_connections<H>(
    addr: SocketAddr,
    thread_name: &str,
    stop: Arc<AtomicBool>,
    mut handle: H,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)>
where
    H: FnMut(&mut TcpStream) + Send + 'static,
{
    let listener = TcpListener::bind(addr)?;
    let bound = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    let h = thread::Builder::new()
        .name(thread_name.into())
        .spawn(move || {
            loop {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                match listener.accept() {
                    Ok((mut s, _peer)) => {
                        let _ = s.set_nonblocking(false);
                        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                        handle(&mut s);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(100)),
                }
            }
        })?;
    Ok((h, bound))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::Labels;

    fn endpoints(metrics_path: &str) -> AdminEndpoints<impl Fn() -> String> {
        let registry = Arc::new(Registry::new());
        registry
            .counter("dds_admin_test_total", Labels::new())
            .add(3);
        AdminEndpoints {
            service_name: "zerodds-test-bridged".into(),
            metrics_path: metrics_path.into(),
            catalog: || "{\"service\":\"zerodds-test-bridged\"}".to_string(),
            registry,
            healthy: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Spec §6.4: an HTTP/1.1 response with the content type, the length
    /// of the body in bytes and `Connection: close`.
    #[test]
    fn monitor_6_4_http_response_carries_length_and_close() {
        assert_eq!(
            http_response("200 OK", "text/plain; charset=utf-8", "OK\nä"),
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\n\
             Content-Length: 5\r\nConnection: close\r\n\r\nOK\nä"
        );
    }

    /// Spec §6.4: the path is the request target without the query; a
    /// request line without a target asks for `/`.
    #[test]
    fn monitor_6_4_request_path_is_the_target_without_query() {
        assert_eq!(
            request_path("GET /metrics?x=1 HTTP/1.1\r\nHost: a\r\n"),
            "/metrics"
        );
        assert_eq!(request_path("GET /catalog HTTP/1.1"), "/catalog");
        assert_eq!(request_path(""), "/");
        assert_eq!(request_path("GET"), "/");
    }

    /// Spec §6.4: the catalog, the health, the metrics at the configured
    /// path, the index naming the four, and 404 for any other path.
    #[test]
    fn monitor_6_4_the_four_paths_and_404() {
        let e = endpoints("/prom");
        let catalog = e.response("/catalog");
        assert!(catalog.starts_with("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n"));
        assert!(catalog.ends_with("\r\n\r\n{\"service\":\"zerodds-test-bridged\"}"));
        assert!(e.response("/healthz").ends_with("\r\n\r\nOK\n"));
        e.healthy.store(false, Ordering::SeqCst);
        let down = e.response("/healthz");
        assert!(down.starts_with("HTTP/1.1 503 Service Unavailable\r\n"));
        assert!(down.ends_with("\r\n\r\nDOWN\n"));
        let metrics = e.response("/prom");
        assert!(metrics.contains("Content-Type: text/plain; version=0.0.4; charset=utf-8\r\n"));
        assert!(metrics.contains("dds_admin_test_total 3"));
        assert!(
            e.response("/metrics")
                .starts_with("HTTP/1.1 404 Not Found\r\n")
        );
        assert!(
            e.response("/")
                .ends_with("\r\n\r\nzerodds-test-bridged\nendpoints: /catalog /healthz /prom\n")
        );
    }

    /// Spec §6.4: the endpoints answer on the bound port, and the thread
    /// ends after the stop flag is set.
    #[test]
    fn monitor_6_4_serve_answers_and_stops() {
        let stop = Arc::new(AtomicBool::new(false));
        let (handle, bound) = serve_admin_endpoints(
            "127.0.0.1:0".parse().unwrap(),
            "zerodds-test-admin",
            endpoints("/metrics"),
            Arc::clone(&stop),
        )
        .expect("serve");
        let mut s = TcpStream::connect_timeout(&bound, Duration::from_secs(2)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n")
            .unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).ok();
        assert!(body.starts_with("HTTP/1.1 200 OK\r\n"), "{body}");
        assert!(body.contains("dds_admin_test_total 3"), "{body}");
        stop.store(true, Ordering::SeqCst);
        handle.join().expect("the admin thread ends");
    }
}
