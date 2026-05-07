// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IIOP Client-Connector mit Connection-Pool.
//!
//! Spec §15.7.1 normativ: "Multiple GIOP requests MAY be sent over
//! the same connection. The same TCP/IP connection MAY be used to
//! invoke multiple objects on the same target endpoint."
//!
//! Wir implementieren einen Pool, der pro `(host, port)`-Schluessel
//! eine wiederverwendbare Connection haelt. Wenn alle existierenden
//! Connections belegt sind und `max_connections_per_endpoint` noch
//! nicht erreicht ist, wird eine neue Connection geoeffnet.

use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::connection::Connection;
use crate::error::IiopError;

/// Konfiguration des Connectors.
#[derive(Debug, Clone)]
pub struct ConnectorConfig {
    /// Connect-Timeout. `None` = OS-Default.
    pub connect_timeout: Option<Duration>,
    /// Read-Timeout fuer alle erstellten Connections.
    pub read_timeout: Option<Duration>,
    /// Write-Timeout fuer alle erstellten Connections.
    pub write_timeout: Option<Duration>,
    /// TCP-Nodelay. Default `true` (CORBA-Konvention).
    pub nodelay: bool,
    /// Max. Anzahl gleichzeitiger Connections pro
    /// `(host, port)`-Endpoint.
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

/// Ein vom Pool ausgeliehener Connection-Slot. Wird beim Drop
/// automatisch wieder in den Pool zurueckgegeben — sofern die
/// Connection noch alive ist (`return_alive(true)`); andernfalls
/// verworfen.
pub struct PooledConnection {
    inner: Option<Connection>,
    pool: Arc<Mutex<PoolInner>>,
    endpoint: SocketAddr,
    /// Wenn `false`, wird die Connection beim Drop nicht in den Pool
    /// zurueckgegeben.
    return_to_pool: bool,
}

impl PooledConnection {
    /// Mutable-Zugriff auf die Connection. Liefert `None`, wenn
    /// die Connection bereits per `Drop` zurueck in den Pool ging
    /// (kann nicht von aussen passieren, weil `&mut self` exklusiv).
    #[must_use]
    pub fn connection(&mut self) -> Option<&mut Connection> {
        self.inner.as_mut()
    }

    /// Markiert die Connection als kaputt — sie wird beim Drop
    /// nicht in den Pool zurueckgegeben.
    pub fn invalidate(&mut self) {
        self.return_to_pool = false;
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if !self.return_to_pool {
            return;
        }
        if let Some(c) = self.inner.take() {
            if let Ok(mut pool) = self.pool.lock() {
                pool.idle.entry(self.endpoint).or_default().push(c);
            }
        }
    }
}

#[derive(Default)]
struct PoolInner {
    idle: HashMap<SocketAddr, Vec<Connection>>,
    in_use_count: HashMap<SocketAddr, usize>,
}

/// IIOP Client-Connector.
pub struct Connector {
    config: ConnectorConfig,
    pool: Arc<Mutex<PoolInner>>,
}

impl Connector {
    /// Konstruktor.
    #[must_use]
    pub fn new(config: ConnectorConfig) -> Self {
        Self {
            config,
            pool: Arc::new(Mutex::new(PoolInner::default())),
        }
    }

    /// Holt eine Connection zum `(host, port)`-Endpoint — entweder
    /// aus dem Pool oder durch Neu-Connect.
    ///
    /// # Errors
    /// `Io` bei Connect-Failure, `PoolExhausted` wenn `max_connections_
    /// per_endpoint` erreicht ist.
    pub fn connect(&self, host: &str, port: u16) -> Result<PooledConnection, IiopError> {
        let endpoint = (host, port).to_socket_addrs()?.next().ok_or_else(|| {
            IiopError::Other(alloc::format!("no address resolved for {host}:{port}"))
        })?;

        // 1. Versuche, eine Idle-Connection zu reusen.
        {
            let mut pool = self
                .pool
                .lock()
                .map_err(|_| IiopError::Other("connector pool mutex poisoned".into()))?;
            if let Some(slots) = pool.idle.get_mut(&endpoint) {
                if let Some(conn) = slots.pop() {
                    *pool.in_use_count.entry(endpoint).or_insert(0) += 1;
                    return Ok(PooledConnection {
                        inner: Some(conn),
                        pool: Arc::clone(&self.pool),
                        endpoint,
                        return_to_pool: true,
                    });
                }
            }
            let in_use = pool.in_use_count.get(&endpoint).copied().unwrap_or(0);
            if in_use >= self.config.max_connections_per_endpoint {
                return Err(IiopError::PoolExhausted);
            }
            *pool.in_use_count.entry(endpoint).or_insert(0) += 1;
        }

        // 2. Sonst: neue TCP-Connection.
        let stream = if let Some(t) = self.config.connect_timeout {
            TcpStream::connect_timeout(&endpoint, t)?
        } else {
            TcpStream::connect(endpoint)?
        };
        let conn = Connection::from_stream(stream)?;
        conn.set_read_timeout(self.config.read_timeout)?;
        conn.set_write_timeout(self.config.write_timeout)?;
        conn.set_nodelay(self.config.nodelay)?;

        Ok(PooledConnection {
            inner: Some(conn),
            pool: Arc::clone(&self.pool),
            endpoint,
            return_to_pool: true,
        })
    }

    /// Anzahl Idle-Connections im Pool fuer ein Endpoint — Diagnose.
    #[must_use]
    pub fn idle_count(&self, host: &str, port: u16) -> usize {
        let Ok(addrs) = (host, port).to_socket_addrs() else {
            return 0;
        };
        let endpoint = addrs.into_iter().next();
        let Some(endpoint) = endpoint else {
            return 0;
        };
        self.pool
            .lock()
            .map(|p| p.idle.get(&endpoint).map_or(0, Vec::len))
            .unwrap_or(0)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    /// Hilfs-Echo-Server: liest GIOP-Header (12 Bytes), liest body,
    /// echot identisch zurueck.
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

        // Erste Connection - kommt frisch.
        {
            let _c1 = connector.connect(&host, port).unwrap();
        } // Drop -> zurueck in den Pool.
        assert_eq!(connector.idle_count(&host, port), 1);

        // Zweite Connection -> reuse aus Pool.
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
}
