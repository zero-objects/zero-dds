// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IIOP client connector with a connection pool.
//!
//! Spec §15.7.1 normative: "Multiple GIOP requests MAY be sent over
//! the same connection. The same TCP/IP connection MAY be used to
//! invoke multiple objects on the same target endpoint."
//!
//! We implement a pool that holds a reusable connection per
//! `(host, port)` key. When all existing connections are in use and
//! `max_connections_per_endpoint` has not yet been reached, a new
//! connection is opened.

use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::connection::Connection;
use crate::error::IiopError;

/// Pool key. Plain IIOP is pooled by endpoint `SocketAddr`; SSLIOP is
/// additionally pooled by SNI + client-config identity, because two IORs to the
/// same `host:ssl_port` can require different trust/SNI (a TLS connection must
/// not be reused for a foreign config).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum PoolKey {
    /// Plain-IIOP endpoint.
    Tcp(SocketAddr),
    /// SSLIOP endpoint (address + SNI + `Arc::as_ptr(ClientConfig)`).
    #[cfg(feature = "tls")]
    Tls {
        addr: SocketAddr,
        sni: String,
        cfg_id: usize,
    },
    /// UIOP endpoint (Unix-domain-socket path).
    #[cfg(unix)]
    Uds(std::path::PathBuf),
}

/// Connector configuration.
#[derive(Debug, Clone)]
pub struct ConnectorConfig {
    /// Connect timeout. `None` = OS default.
    pub connect_timeout: Option<Duration>,
    /// Read timeout for all created connections.
    pub read_timeout: Option<Duration>,
    /// Write timeout for all created connections.
    pub write_timeout: Option<Duration>,
    /// TCP nodelay. Default `true` (CORBA convention).
    pub nodelay: bool,
    /// Max. number of concurrent connections per
    /// `(host, port)` endpoint.
    pub max_connections_per_endpoint: usize,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Some(Duration::from_secs(10)),
            read_timeout: Some(Duration::from_secs(30)),
            write_timeout: Some(Duration::from_secs(30)),
            nodelay: true,
            max_connections_per_endpoint: 16,
        }
    }
}

/// A connection slot borrowed from the pool. On drop it is
/// automatically returned to the pool — provided the connection is
/// still alive (`return_alive(true)`); otherwise it is discarded.
pub struct PooledConnection {
    inner: Option<Connection>,
    pool: Arc<Mutex<PoolInner>>,
    key: PoolKey,
    /// If `false`, the connection is not returned to the pool on
    /// drop.
    return_to_pool: bool,
}

impl PooledConnection {
    /// Mutable access to the connection. Returns `None` if the
    /// connection has already gone back to the pool via `Drop`
    /// (cannot happen from outside, because `&mut self` is exclusive).
    #[must_use]
    pub fn connection(&mut self) -> Option<&mut Connection> {
        self.inner.as_mut()
    }

    /// Marks the connection as broken — it is not returned to the
    /// pool on drop.
    pub fn invalidate(&mut self) {
        self.return_to_pool = false;
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        let conn = self.inner.take();
        if let Ok(mut pool) = self.pool.lock() {
            // Release the checked-out connection (in_use--), whether it goes
            // back into the pool or is discarded — otherwise the counter leaks
            // and an endpoint is falsely reported as exhausted after
            // `max_connections_per_endpoint` acquires (even sequential ones).
            if let Some(n) = pool.in_use_count.get_mut(&self.key) {
                *n = n.saturating_sub(1);
            }
            if self.return_to_pool {
                if let Some(c) = conn {
                    pool.idle.entry(self.key.clone()).or_default().push(c);
                }
            }
        }
    }
}

#[derive(Default)]
struct PoolInner {
    idle: HashMap<PoolKey, Vec<Connection>>,
    in_use_count: HashMap<PoolKey, usize>,
}

/// IIOP client connector.
pub struct Connector {
    config: ConnectorConfig,
    pool: Arc<Mutex<PoolInner>>,
}

impl Connector {
    /// Constructor.
    #[must_use]
    pub fn new(config: ConnectorConfig) -> Self {
        Self {
            config,
            pool: Arc::new(Mutex::new(PoolInner::default())),
        }
    }

    /// Gets a connection to the `(host, port)` endpoint — either from
    /// the pool or by a fresh connect.
    ///
    /// # Errors
    /// `Io` on connect failure, `PoolExhausted` when `max_connections_
    /// per_endpoint` has been reached.
    pub fn connect(&self, host: &str, port: u16) -> Result<PooledConnection, IiopError> {
        let endpoint = (host, port).to_socket_addrs()?.next().ok_or_else(|| {
            IiopError::Other(alloc::format!("no address resolved for {host}:{port}"))
        })?;
        let cfg = self.config.clone();
        self.acquire(PoolKey::Tcp(endpoint), || {
            let stream = if let Some(t) = cfg.connect_timeout {
                TcpStream::connect_timeout(&endpoint, t)?
            } else {
                TcpStream::connect(endpoint)?
            };
            Connection::from_stream(stream)
        })
    }

    /// Like [`Self::connect`], but over **SSLIOP/TLS** — pooled by
    /// (address, SNI, client-config identity). An established TLS connection
    /// is reused across multiple calls (no handshake per call).
    ///
    /// # Errors
    /// `Io`/TLS error on connect/handshake, `PoolExhausted` at the endpoint limit.
    #[cfg(feature = "tls")]
    pub fn connect_tls(
        &self,
        host: &str,
        ssl_port: u16,
        sni: &str,
        config: Arc<rustls::ClientConfig>,
    ) -> Result<PooledConnection, IiopError> {
        let addr = (host, ssl_port).to_socket_addrs()?.next().ok_or_else(|| {
            IiopError::Other(alloc::format!("no address resolved for {host}:{ssl_port}"))
        })?;
        let cfg_id = Arc::as_ptr(&config) as *const () as usize;
        let key = PoolKey::Tls {
            addr,
            sni: String::from(sni),
            cfg_id,
        };
        let host = String::from(host);
        let sni = String::from(sni);
        self.acquire(key, move || {
            crate::tls::connect_tls(&host, ssl_port, &sni, config)
        })
    }

    /// Like [`Self::connect`], but over **UIOP** (Unix-domain socket) — pooled
    /// by socket path. For same-host IPC without a TCP/IP stack.
    ///
    /// # Errors
    /// `Io` on connect failure, `PoolExhausted` at the endpoint limit.
    #[cfg(unix)]
    pub fn connect_uds(&self, path: &std::path::Path) -> Result<PooledConnection, IiopError> {
        let key = PoolKey::Uds(path.to_path_buf());
        let path = path.to_path_buf();
        self.acquire(key, move || {
            let stream = std::os::unix::net::UnixStream::connect(&path)?;
            Connection::from_unix_stream(stream)
        })
    }

    /// Number of idle UIOP connections for a socket path — diagnostics/test.
    #[cfg(unix)]
    #[must_use]
    pub fn idle_count_uds(&self, path: &std::path::Path) -> usize {
        self.pool
            .lock()
            .map(|p| {
                p.idle
                    .get(&PoolKey::Uds(path.to_path_buf()))
                    .map_or(0, Vec::len)
            })
            .unwrap_or(0)
    }

    /// Shared pool path: idle reuse, endpoint-limit check, otherwise
    /// `make()` for a fresh connection (transport-specific).
    fn acquire<F>(&self, key: PoolKey, make: F) -> Result<PooledConnection, IiopError>
    where
        F: FnOnce() -> Result<Connection, IiopError>,
    {
        // 1. Reuse an idle connection?
        {
            let mut pool = self
                .pool
                .lock()
                .map_err(|_| IiopError::Other("connector pool mutex poisoned".into()))?;
            if let Some(conn) = pool.idle.get_mut(&key).and_then(Vec::pop) {
                *pool.in_use_count.entry(key.clone()).or_insert(0) += 1;
                return Ok(PooledConnection {
                    inner: Some(conn),
                    pool: Arc::clone(&self.pool),
                    key,
                    return_to_pool: true,
                });
            }
            let in_use = pool.in_use_count.get(&key).copied().unwrap_or(0);
            if in_use >= self.config.max_connections_per_endpoint {
                return Err(IiopError::PoolExhausted);
            }
            *pool.in_use_count.entry(key.clone()).or_insert(0) += 1;
        }

        // 2. Fresh connection. On error, roll back the in_use counter.
        let conn = match make() {
            Ok(c) => c,
            Err(e) => {
                if let Ok(mut pool) = self.pool.lock() {
                    if let Some(n) = pool.in_use_count.get_mut(&key) {
                        *n = n.saturating_sub(1);
                    }
                }
                return Err(e);
            }
        };
        conn.set_read_timeout(self.config.read_timeout)?;
        conn.set_write_timeout(self.config.write_timeout)?;
        conn.set_nodelay(self.config.nodelay)?;
        Ok(PooledConnection {
            inner: Some(conn),
            pool: Arc::clone(&self.pool),
            key,
            return_to_pool: true,
        })
    }

    /// Number of idle plain-IIOP connections in the pool for an endpoint — diagnostics.
    #[must_use]
    pub fn idle_count(&self, host: &str, port: u16) -> usize {
        let Some(endpoint) = (host, port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next())
        else {
            return 0;
        };
        self.pool
            .lock()
            .map(|p| p.idle.get(&PoolKey::Tcp(endpoint)).map_or(0, Vec::len))
            .unwrap_or(0)
    }

    /// Number of idle SSLIOP connections for (address, SNI, config) — diagnostics/test.
    #[cfg(feature = "tls")]
    #[must_use]
    pub fn idle_count_tls(
        &self,
        host: &str,
        ssl_port: u16,
        sni: &str,
        config: &Arc<rustls::ClientConfig>,
    ) -> usize {
        let Some(addr) = (host, ssl_port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next())
        else {
            return 0;
        };
        let key = PoolKey::Tls {
            addr,
            sni: String::from(sni),
            cfg_id: Arc::as_ptr(config) as *const () as usize,
        };
        self.pool
            .lock()
            .map(|p| p.idle.get(&key).map_or(0, Vec::len))
            .unwrap_or(0)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    /// Helper echo server: reads the GIOP header (12 bytes), reads the body,
    /// and echoes it back identically.
    fn echo_server(listener: TcpListener) {
        loop {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            thread::spawn(move || {
                use std::io::{Read, Write};
                let mut buf = [0u8; 4096];
                loop {
                    let Ok(n) = stream.read(&mut buf) else {
                        return;
                    };
                    if n == 0 {
                        return;
                    }
                    if stream.write_all(&buf[..n]).is_err() {
                        return;
                    }
                }
            });
        }
    }

    #[test]
    fn connect_reuses_pooled_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || echo_server(listener));

        let connector = Connector::new(ConnectorConfig::default());
        let host = addr.ip().to_string();
        let port = addr.port();

        // First connection - comes fresh.
        {
            let _c1 = connector.connect(&host, port).unwrap();
        } // Drop -> back into the pool.
        assert_eq!(connector.idle_count(&host, port), 1);

        // Second connection -> reuse from pool.
        let _c2 = connector.connect(&host, port).unwrap();
        assert_eq!(connector.idle_count(&host, port), 0);
    }

    #[test]
    fn invalidated_connection_is_not_returned_to_pool() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || echo_server(listener));

        let connector = Connector::new(ConnectorConfig::default());
        let host = addr.ip().to_string();
        let port = addr.port();
        {
            let mut c = connector.connect(&host, port).unwrap();
            c.invalidate();
        }
        assert_eq!(connector.idle_count(&host, port), 0);
    }

    #[test]
    fn in_use_count_is_released_on_drop() {
        // Regression: in_use_count must be decremented on drop, otherwise
        // an endpoint is falsely reported as exhausted after `max` acquires
        // (even when the connections were long since returned/discarded).
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || echo_server(listener));
        let connector = Connector::new(ConnectorConfig {
            max_connections_per_endpoint: 1,
            ..ConnectorConfig::default()
        });
        let host = addr.ip().to_string();
        let port = addr.port();
        // Acquire + invalidate + drop the first connection → slot free.
        {
            let mut c = connector.connect(&host, port).unwrap();
            c.invalidate();
        }
        // Without the decrement fix: PoolExhausted. With the fix: fresh connection ok.
        let _c2 = connector
            .connect(&host, port)
            .expect("slot must be free after drop");
    }

    #[test]
    fn max_connections_per_endpoint_is_enforced() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || echo_server(listener));

        let connector = Connector::new(ConnectorConfig {
            max_connections_per_endpoint: 1,
            ..ConnectorConfig::default()
        });
        let host = addr.ip().to_string();
        let port = addr.port();
        let _c1 = connector
            .connect(&host, port)
            .map_err(|e| panic!("first connect: {e}"))
            .ok();
        match connector.connect(&host, port) {
            Ok(_) => panic!("expected PoolExhausted"),
            Err(IiopError::PoolExhausted) => {}
            Err(other) => panic!("expected PoolExhausted, got {other}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn connect_uds_reuses_pooled_connection() {
        use std::os::unix::net::UnixListener;
        let path = std::env::temp_dir().join(alloc::format!(
            "zerodds-conn-uds-{}-{:p}.sock",
            std::process::id(),
            &() as *const ()
        ));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        // Keep accepted connections OPEN (realistic server behavior):
        // an immediate drop closes the peer and makes set_read_timeout on the
        // half-closed UDS socket return EINVAL (macOS).
        thread::spawn(move || {
            let mut held = alloc::vec::Vec::new();
            while let Ok((s, _)) = listener.accept() {
                held.push(s);
            }
        });

        let connector = Connector::new(ConnectorConfig::default());
        {
            let _c = connector.connect_uds(&path).unwrap();
        }
        assert_eq!(connector.idle_count_uds(&path), 1);
        let _c2 = connector.connect_uds(&path).unwrap();
        assert_eq!(connector.idle_count_uds(&path), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(feature = "tls")]
    mod tls {
        use super::*;

        fn self_signed() -> (alloc::string::String, alloc::string::String) {
            let ck =
                rcgen::generate_simple_self_signed(alloc::vec!["localhost".to_string()]).unwrap();
            (ck.cert.pem(), ck.key_pair.serialize_pem())
        }

        /// SSLIOP connections are pooled by (address, SNI, config): after a
        /// drop exactly one is idle, and the next connect_tls reuses it.
        #[test]
        fn connect_tls_reuses_pooled_connection() {
            // A plain TCP listener is enough — connect_tls builds the StreamOwned
            // without an immediate handshake (lazy); pool reuse does not depend on GIOP I/O.
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            thread::spawn(move || while listener.accept().is_ok() {});
            let (cert, _key) = self_signed();
            let cfg = crate::tls::load_client_config_trusting(cert.as_bytes()).unwrap();
            let connector = Connector::new(ConnectorConfig::default());
            let host = addr.ip().to_string();
            let port = addr.port();

            {
                let _c = connector
                    .connect_tls(&host, port, "localhost", Arc::clone(&cfg))
                    .unwrap();
            }
            assert_eq!(connector.idle_count_tls(&host, port, "localhost", &cfg), 1);
            let _c2 = connector
                .connect_tls(&host, port, "localhost", Arc::clone(&cfg))
                .unwrap();
            assert_eq!(connector.idle_count_tls(&host, port, "localhost", &cfg), 0);
        }

        /// Two different client configs to the same endpoint do NOT share a
        /// pool slot (cfg_id separation) — otherwise a connection with foreign
        /// trust/SNI would be reused.
        #[test]
        fn distinct_config_does_not_reuse() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            thread::spawn(move || while listener.accept().is_ok() {});
            let (cert, _key) = self_signed();
            let cfg_a = crate::tls::load_client_config_trusting(cert.as_bytes()).unwrap();
            let cfg_b = crate::tls::load_client_config_trusting(cert.as_bytes()).unwrap();
            let connector = Connector::new(ConnectorConfig::default());
            let host = addr.ip().to_string();
            let port = addr.port();

            {
                let _c = connector
                    .connect_tls(&host, port, "localhost", Arc::clone(&cfg_a))
                    .unwrap();
            }
            // cfg_a has 1 idle, cfg_b has 0 (separate key).
            assert_eq!(
                connector.idle_count_tls(&host, port, "localhost", &cfg_a),
                1
            );
            assert_eq!(
                connector.idle_count_tls(&host, port, "localhost", &cfg_b),
                0
            );
        }
    }
}
