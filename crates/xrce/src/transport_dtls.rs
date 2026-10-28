// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE DTLS wrapper for UDP (Spec §11.5).
//!
//! C6.2.D delivers only a trait skeleton + an identity pass-through
//! implementation (`DummyDtls`) here. Rationale:
//!
//! - The DTLS stack choice is deployment-specific (embedded with
//!   `embedded-tls`, Linux with `webrtc-dtls` or `tokio-dtls`).
//! - A full integration would pull in transitive crypto providers
//!   (ring, aws-lc-rs) and decouple the `crates/xrce` crate from the
//!   `no_std` path.
//! - A trait boundary now + plug-in later is the leaner
//!   architecture, analogous to the security stack separation in
//!   `crates/security-runtime/`.
//!
//! The DTLS layer sits below the XRCE codec on top of the UDP layer:
//!
//! ```text
//!   XRCE Message (Plaintext)
//!         |
//!         v
//!     DTLS-Layer  --[handshake]-> DTLS-Records
//!         |
//!         v
//!     UDP Datagram
//! ```
//!
//! Inverse on the receive path.
//!
//! The Conn/Listener abstraction is deliberately `Arc<dyn ...>`: the concrete
//! DTLS stack is chosen at runtime/deployment (embedded vs. Linux),
//! generics would dissolve the plug-in boundary.
// zerodds-lint: allow no_dyn_in_safe

extern crate alloc;
use alloc::vec::Vec;

/// Error class for DTLS operations.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DtlsError {
    /// The handshake phase failed.
    HandshakeFailed {
        /// Description.
        reason: &'static str,
    },
    /// Send operation failed.
    SendFailed {
        /// Description.
        reason: &'static str,
    },
    /// Recv operation failed.
    RecvFailed {
        /// Description.
        reason: &'static str,
    },
    /// The stream was closed.
    Closed,
    /// A credential file (certificate, key, trust anchors) could not be
    /// read or is not usable.
    Config {
        /// Description, naming the file.
        reason: alloc::string::String,
    },
}

impl core::fmt::Display for DtlsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::HandshakeFailed { reason } => write!(f, "dtls handshake failed: {reason}"),
            Self::SendFailed { reason } => write!(f, "dtls send failed: {reason}"),
            Self::RecvFailed { reason } => write!(f, "dtls recv failed: {reason}"),
            Self::Closed => write!(f, "dtls stream closed"),
            Self::Config { reason } => write!(f, "dtls credentials: {reason}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DtlsError {}

/// Trait for DTLS layer implementations. Plug-in point for a
/// real DTLS engine.
pub trait DtlsLayer {
    /// Performs the DTLS handshake. Must have succeeded before the
    /// first `send`/`recv`.
    ///
    /// # Errors
    /// `HandshakeFailed`.
    fn handshake(&mut self) -> Result<(), DtlsError>;

    /// Encrypts+sends `plaintext` over the DTLS layer.
    ///
    /// # Errors
    /// `SendFailed`, `Closed`.
    fn send(&mut self, plaintext: &[u8]) -> Result<(), DtlsError>;

    /// Receives+decrypts a DTLS record. Returns the plaintext.
    ///
    /// # Errors
    /// `RecvFailed`, `Closed`.
    fn recv(&mut self) -> Result<Vec<u8>, DtlsError>;

    /// Closes the stream (DTLS close-notify).
    ///
    /// # Errors
    /// `SendFailed`.
    fn close(&mut self) -> Result<(), DtlsError>;

    /// `true` when the handshake is complete.
    fn is_handshake_complete(&self) -> bool;
}

/// Identity pass-through DTLS — for tests and as a pre-production stub.
///
/// Encrypts nothing, passes plaintext through. With it the
/// DTLS handshake/send/recv code path can be exercised in integration
/// tests without a real crypto lib.
#[derive(Debug, Default)]
pub struct DummyDtls {
    handshake_done: bool,
    inbox: alloc::collections::VecDeque<Vec<u8>>,
    closed: bool,
}

impl DummyDtls {
    /// Fresh pass-through stub.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a plaintext block into the inbox so that the next
    /// `recv()` returns it. Test helper.
    pub fn inject(&mut self, plaintext: Vec<u8>) {
        self.inbox.push_back(plaintext);
    }

    /// Number of packets in the inbox.
    #[must_use]
    pub fn inbox_len(&self) -> usize {
        self.inbox.len()
    }
}

impl DtlsLayer for DummyDtls {
    fn handshake(&mut self) -> Result<(), DtlsError> {
        self.handshake_done = true;
        Ok(())
    }

    fn send(&mut self, plaintext: &[u8]) -> Result<(), DtlsError> {
        if self.closed {
            return Err(DtlsError::Closed);
        }
        if !self.handshake_done {
            return Err(DtlsError::SendFailed {
                reason: "handshake not complete",
            });
        }
        // Loopback: the sent plaintext lands in our own inbox.
        self.inbox.push_back(plaintext.to_vec());
        Ok(())
    }

    fn recv(&mut self) -> Result<Vec<u8>, DtlsError> {
        if self.closed && self.inbox.is_empty() {
            return Err(DtlsError::Closed);
        }
        if !self.handshake_done {
            return Err(DtlsError::RecvFailed {
                reason: "handshake not complete",
            });
        }
        self.inbox.pop_front().ok_or(DtlsError::RecvFailed {
            reason: "inbox empty",
        })
    }

    fn close(&mut self) -> Result<(), DtlsError> {
        self.closed = true;
        Ok(())
    }

    fn is_handshake_complete(&self) -> bool {
        self.handshake_done
    }
}

/// Productive DTLS 1.2 backend (webrtc-dtls) wired into the sync [`DtlsLayer`]
/// trait via an owned current-thread tokio runtime. Opt-in `dtls` feature; the
/// no_std wire-codec build is unaffected. Mirrors `crates/coap-bridge` §7.1.
#[cfg(feature = "dtls")]
mod webrtc_impl {
    use super::{DtlsError, DtlsLayer};
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use std::net::SocketAddr;
    use std::string::ToString;

    use tokio::net::UdpSocket;
    use tokio::runtime::Runtime;
    use webrtc_dtls::config::{Config, ExtendedMasterSecretType};
    use webrtc_dtls::conn::DTLSConn;
    use webrtc_dtls::crypto::Certificate;
    use webrtc_dtls::listener::listen;
    use webrtc_util::conn::{Conn, Listener};

    fn build_rt() -> Result<Runtime, DtlsError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| DtlsError::HandshakeFailed {
                reason: "tokio runtime build",
            })
    }

    /// A productive DTLS 1.2 session exposed through the sync [`DtlsLayer`].
    pub struct WebrtcDtls {
        rt: Arc<Runtime>,
        conn: Arc<dyn Conn + Send + Sync>,
        handshake_done: bool,
    }

    impl WebrtcDtls {
        /// Dials `server`, completing the DTLS handshake. `verify = false`
        /// accepts a self-signed server certificate (test/dev).
        ///
        /// # Errors
        /// [`DtlsError::HandshakeFailed`] on bind/connect/handshake failure.
        pub fn connect(
            server: SocketAddr,
            server_name: &str,
            verify: bool,
        ) -> Result<Self, DtlsError> {
            let rt = Arc::new(build_rt()?);
            let name = server_name.to_string();
            let conn = rt.block_on(async move {
                let bind_addr = if server.is_ipv4() {
                    "0.0.0.0:0"
                } else {
                    "[::]:0"
                };
                let udp = UdpSocket::bind(bind_addr)
                    .await
                    .map_err(|_| DtlsError::HandshakeFailed { reason: "udp bind" })?;
                udp.connect(server)
                    .await
                    .map_err(|_| DtlsError::HandshakeFailed {
                        reason: "udp connect",
                    })?;
                let udp_conn: Arc<dyn Conn + Send + Sync> = Arc::new(udp);
                let cfg = Config {
                    server_name: name,
                    insecure_skip_verify: !verify,
                    extended_master_secret: ExtendedMasterSecretType::Require,
                    ..Default::default()
                };
                let dtls = DTLSConn::new(udp_conn, cfg, true, None)
                    .await
                    .map_err(|_| DtlsError::HandshakeFailed {
                        reason: "dtls client handshake",
                    })?;
                Ok::<Arc<dyn Conn + Send + Sync>, DtlsError>(Arc::new(dtls))
            })?;
            Ok(Self {
                rt,
                conn,
                handshake_done: true,
            })
        }
    }

    impl DtlsLayer for WebrtcDtls {
        fn handshake(&mut self) -> Result<(), DtlsError> {
            // Completed eagerly in `connect`/`accept`.
            Ok(())
        }

        fn send(&mut self, plaintext: &[u8]) -> Result<(), DtlsError> {
            self.rt
                .block_on(self.conn.send(plaintext))
                .map(|_| ())
                .map_err(|_| DtlsError::SendFailed {
                    reason: "dtls send",
                })
        }

        fn recv(&mut self) -> Result<Vec<u8>, DtlsError> {
            let mut buf = std::vec![0u8; 65_535];
            let n =
                self.rt
                    .block_on(self.conn.recv(&mut buf))
                    .map_err(|_| DtlsError::RecvFailed {
                        reason: "dtls recv",
                    })?;
            buf.truncate(n);
            Ok(buf)
        }

        fn close(&mut self) -> Result<(), DtlsError> {
            self.rt
                .block_on(self.conn.close())
                .map_err(|_| DtlsError::SendFailed {
                    reason: "dtls close",
                })
        }

        fn is_handshake_complete(&self) -> bool {
            self.handshake_done
        }
    }

    /// A DTLS server: binds a listener with a fresh self-signed cert. `accept`
    /// returns a [`WebrtcDtls`] session sharing this server's runtime.
    pub struct WebrtcDtlsServer {
        rt: Arc<Runtime>,
        listener: Arc<dyn Listener + Send + Sync>,
        addr: SocketAddr,
    }

    impl WebrtcDtlsServer {
        /// Binds a DTLS listener on `addr` with a freshly generated self-signed
        /// certificate (RFC 7627 extended master secret required).
        ///
        /// # Errors
        /// [`DtlsError::HandshakeFailed`] on cert/listen failure.
        pub fn bind(addr: SocketAddr) -> Result<Self, DtlsError> {
            let rt = Arc::new(build_rt()?);
            let (listener, addr) = rt.block_on(async {
                let cert = Certificate::generate_self_signed(std::vec!["localhost".to_string()])
                    .map_err(|_| DtlsError::HandshakeFailed { reason: "cert gen" })?;
                let cfg = Config {
                    certificates: std::vec![cert],
                    extended_master_secret: ExtendedMasterSecretType::Require,
                    ..Default::default()
                };
                let l = listen(addr, cfg)
                    .await
                    .map_err(|_| DtlsError::HandshakeFailed {
                        reason: "dtls listen",
                    })?;
                let a = l.addr().await.map_err(|_| DtlsError::HandshakeFailed {
                    reason: "listener addr",
                })?;
                Ok::<_, DtlsError>((Arc::new(l) as Arc<dyn Listener + Send + Sync>, a))
            })?;
            Ok(Self { rt, listener, addr })
        }

        /// The bound local address (use with `127.0.0.1:0` for an ephemeral port).
        #[must_use]
        pub fn local_addr(&self) -> SocketAddr {
            self.addr
        }

        /// Accepts one inbound DTLS connection, completing the handshake.
        ///
        /// # Errors
        /// [`DtlsError::HandshakeFailed`] on accept/handshake failure.
        pub fn accept(&self) -> Result<WebrtcDtls, DtlsError> {
            let conn = self.rt.block_on(async {
                let (conn, _peer) =
                    self.listener
                        .accept()
                        .await
                        .map_err(|_| DtlsError::HandshakeFailed {
                            reason: "dtls accept",
                        })?;
                Ok::<_, DtlsError>(conn)
            })?;
            Ok(WebrtcDtls {
                rt: Arc::clone(&self.rt),
                conn,
                handshake_done: true,
            })
        }
    }

    /// The files an agent's DTLS listener authenticates with (DDS-XRCE 1.0
    /// §11.1 (5), §11.2): its certificate and key, so that a client can
    /// authenticate the agent, and optionally the trust anchors a client
    /// certificate must chain to.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DtlsServerCredentials {
        /// PEM file with the agent's certificate chain, leaf first.
        pub cert_file: std::string::String,
        /// PEM file with the agent's private key, PKCS#8.
        pub key_file: std::string::String,
        /// PEM file with the trust anchors; when set, a client must present
        /// a certificate that chains to one of them.
        pub client_ca_file: Option<std::string::String>,
    }

    fn config_err(reason: std::string::String) -> DtlsError {
        DtlsError::Config { reason }
    }

    /// Reads a certificate chain and its PKCS#8 private key from PEM files.
    ///
    /// # Errors
    /// [`DtlsError::Config`] naming the file: unreadable, malformed, no
    /// `CERTIFICATE` block, or a key that is not PKCS#8.
    pub fn load_certificate(cert_file: &str, key_file: &str) -> Result<Certificate, DtlsError> {
        use std::format;
        let cert_pem = std::fs::read_to_string(cert_file)
            .map_err(|e| config_err(format!("cert_file {cert_file}: {e}")))?;
        let key_pem = std::fs::read_to_string(key_file)
            .map_err(|e| config_err(format!("key_file {key_file}: {e}")))?;
        let key_blocks = pem::parse_many(&key_pem)
            .map_err(|e| config_err(format!("key_file {key_file}: {e}")))?;
        let key = key_blocks
            .iter()
            .find(|p| p.tag() == "PRIVATE KEY" || p.tag() == "PRIVATE_KEY")
            .ok_or_else(|| {
                config_err(format!(
                    "key_file {key_file}: no PKCS#8 `PRIVATE KEY` block \
                     (convert with `openssl pkcs8 -topk8 -nocrypt`)"
                ))
            })?;
        let certs = pem::parse_many(&cert_pem)
            .map_err(|e| config_err(format!("cert_file {cert_file}: {e}")))?;
        if !certs.iter().any(|p| p.tag() == "CERTIFICATE") {
            return Err(config_err(format!(
                "cert_file {cert_file}: no `CERTIFICATE` block"
            )));
        }
        // `Certificate::from_pem` takes one document whose first block is
        // the key under the tag `PRIVATE_KEY`, then the certificates.
        let mut joined = pem::encode(&pem::Pem::new(
            "PRIVATE_KEY".to_string(),
            key.contents().to_vec(),
        ));
        for block in certs.iter().filter(|p| p.tag() == "CERTIFICATE") {
            joined.push_str(&pem::encode(&pem::Pem::new(
                "CERTIFICATE".to_string(),
                block.contents().to_vec(),
            )));
        }
        Certificate::from_pem(&joined)
            .map_err(|e| config_err(format!("cert_file {cert_file} / key_file {key_file}: {e}")))
    }

    /// Reads the `CERTIFICATE` blocks of a PEM file into a root store.
    ///
    /// # Errors
    /// [`DtlsError::Config`] naming the file: unreadable, malformed, or
    /// without a usable certificate.
    pub fn load_trust_anchors(ca_file: &str) -> Result<rustls::RootCertStore, DtlsError> {
        use std::format;
        let text = std::fs::read_to_string(ca_file)
            .map_err(|e| config_err(format!("ca_file {ca_file}: {e}")))?;
        let blocks =
            pem::parse_many(&text).map_err(|e| config_err(format!("ca_file {ca_file}: {e}")))?;
        let mut store = rustls::RootCertStore::empty();
        let mut added = 0usize;
        for block in blocks.iter().filter(|p| p.tag() == "CERTIFICATE") {
            store
                .add(rustls::pki_types::CertificateDer::from(
                    block.contents().to_vec(),
                ))
                .map_err(|e| config_err(format!("ca_file {ca_file}: {e}")))?;
            added += 1;
        }
        if added == 0 {
            return Err(config_err(format!(
                "ca_file {ca_file}: no `CERTIFICATE` block"
            )));
        }
        Ok(store)
    }

    impl WebrtcDtls {
        /// Dials `server` and authenticates it: its certificate has to chain
        /// to the trust anchors of `ca_file` and name `server_name`
        /// (DDS-XRCE 1.0 §11.1 (5): "the means for the Client to authenticate
        /// the Agent"). With `client_certificate` (certificate file, key
        /// file) the client presents its own, for an agent that asks for one.
        ///
        /// # Errors
        /// [`DtlsError::Config`] for an unusable file,
        /// [`DtlsError::HandshakeFailed`] if the handshake fails, including
        /// a server certificate that does not verify.
        pub fn connect_verified(
            server: SocketAddr,
            server_name: &str,
            ca_file: &str,
            client_certificate: Option<(&str, &str)>,
        ) -> Result<Self, DtlsError> {
            let roots = load_trust_anchors(ca_file)?;
            let certificates = match client_certificate {
                Some((cert, key)) => std::vec![load_certificate(cert, key)?],
                None => std::vec::Vec::new(),
            };
            let rt = Arc::new(build_rt()?);
            let name = server_name.to_string();
            let conn = rt.block_on(async move {
                let bind_addr = if server.is_ipv4() {
                    "0.0.0.0:0"
                } else {
                    "[::]:0"
                };
                let udp = UdpSocket::bind(bind_addr)
                    .await
                    .map_err(|_| DtlsError::HandshakeFailed { reason: "udp bind" })?;
                udp.connect(server)
                    .await
                    .map_err(|_| DtlsError::HandshakeFailed {
                        reason: "udp connect",
                    })?;
                let udp_conn: Arc<dyn Conn + Send + Sync> = Arc::new(udp);
                let cfg = Config {
                    server_name: name,
                    insecure_skip_verify: false,
                    roots_cas: roots,
                    certificates,
                    extended_master_secret: ExtendedMasterSecretType::Require,
                    ..Default::default()
                };
                let dtls = DTLSConn::new(udp_conn, cfg, true, None)
                    .await
                    .map_err(|_| DtlsError::HandshakeFailed {
                        reason: "dtls client handshake",
                    })?;
                Ok::<Arc<dyn Conn + Send + Sync>, DtlsError>(Arc::new(dtls))
            })?;
            Ok(Self {
                rt,
                conn,
                handshake_done: true,
            })
        }
    }

    /// One accepted DTLS connection of a [`WebrtcDtlsListener`]. Unlike
    /// [`WebrtcDtls`] its methods take `&self`, so one thread can read while
    /// another writes.
    pub struct WebrtcDtlsConn {
        rt: Arc<Runtime>,
        conn: Arc<dyn Conn + Send + Sync>,
        peer: SocketAddr,
    }

    impl WebrtcDtlsConn {
        /// The peer's address.
        #[must_use]
        pub fn peer_addr(&self) -> SocketAddr {
            self.peer
        }

        /// Encrypts and sends `plaintext` as one DTLS record.
        ///
        /// # Errors
        /// [`DtlsError::SendFailed`].
        pub fn send(&self, plaintext: &[u8]) -> Result<(), DtlsError> {
            self.rt
                .block_on(self.conn.send(plaintext))
                .map(|_| ())
                .map_err(|_| DtlsError::SendFailed {
                    reason: "dtls send",
                })
        }

        /// Waits up to `timeout` for the next record; `Ok(None)` when none
        /// arrived.
        ///
        /// # Errors
        /// [`DtlsError::Closed`] when the connection is closed or failed.
        pub fn recv_timeout(
            &self,
            timeout: std::time::Duration,
        ) -> Result<Option<Vec<u8>>, DtlsError> {
            let mut buf = std::vec![0u8; 65_535];
            // `tokio::time::timeout` reads the time driver of the current
            // runtime when it is called, so it has to be called inside
            // `block_on`, not as its argument.
            match self
                .rt
                .block_on(async { tokio::time::timeout(timeout, self.conn.recv(&mut buf)).await })
            {
                Err(_elapsed) => Ok(None),
                Ok(Ok(0)) | Ok(Err(_)) => Err(DtlsError::Closed),
                Ok(Ok(n)) => {
                    buf.truncate(n);
                    Ok(Some(buf))
                }
            }
        }

        /// Closes the connection (close_notify).
        pub fn close(&self) {
            let _ = self.rt.block_on(self.conn.close());
        }
    }

    /// An agent's DTLS listener (DDS-XRCE 1.0 §11.2): binds a UDP port,
    /// authenticates itself with the certificate of its credentials, and
    /// hands out the connections whose handshake completed. It owns a
    /// multi-thread runtime, so the connections progress between calls.
    pub struct WebrtcDtlsListener {
        rt: Arc<Runtime>,
        accepted: std::sync::Mutex<std::sync::mpsc::Receiver<Result<WebrtcDtlsConn, DtlsError>>>,
        addr: SocketAddr,
    }

    impl WebrtcDtlsListener {
        /// Binds on `addr`.
        ///
        /// # Errors
        /// [`DtlsError::Config`] for an unusable credential file,
        /// [`DtlsError::HandshakeFailed`] if the port cannot be bound.
        pub fn bind(
            addr: SocketAddr,
            credentials: &DtlsServerCredentials,
        ) -> Result<Self, DtlsError> {
            let mut cfg = Config {
                certificates: std::vec![load_certificate(
                    &credentials.cert_file,
                    &credentials.key_file
                )?],
                extended_master_secret: ExtendedMasterSecretType::Require,
                ..Default::default()
            };
            if let Some(ca) = &credentials.client_ca_file {
                cfg.client_cas = load_trust_anchors(ca)?;
                cfg.client_auth = webrtc_dtls::config::ClientAuthType::RequireAndVerifyClientCert;
            }
            let rt = Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .map_err(|_| DtlsError::HandshakeFailed {
                        reason: "tokio runtime build",
                    })?,
            );
            let (listener, bound) = rt.block_on(async {
                let udp: Arc<dyn Listener + Send + Sync> = Arc::new(
                    webrtc_util::conn::conn_udp_listener::ListenConfig::default()
                        .listen(addr)
                        .await
                        .map_err(|_| DtlsError::HandshakeFailed {
                            reason: "udp listen",
                        })?,
                );
                let bound = udp.addr().await.map_err(|_| DtlsError::HandshakeFailed {
                    reason: "listener addr",
                })?;
                let dtls = webrtc_dtls::listener::DTLSListener::new(udp, cfg).map_err(|_| {
                    DtlsError::HandshakeFailed {
                        reason: "dtls listen",
                    }
                })?;
                Ok::<_, DtlsError>((Arc::new(dtls) as Arc<dyn Listener + Send + Sync>, bound))
            })?;
            let (tx, rx) = std::sync::mpsc::channel();
            let task_rt = Arc::clone(&rt);
            rt.spawn(async move {
                loop {
                    let item = match listener.accept().await {
                        Ok((conn, peer)) => Ok(WebrtcDtlsConn {
                            rt: Arc::clone(&task_rt),
                            conn,
                            peer,
                        }),
                        Err(_) => Err(DtlsError::HandshakeFailed {
                            reason: "dtls accept",
                        }),
                    };
                    let failed = item.is_err();
                    if tx.send(item).is_err() {
                        break;
                    }
                    if failed {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            });
            Ok(Self {
                rt,
                accepted: std::sync::Mutex::new(rx),
                addr: bound,
            })
        }

        /// The bound local address.
        #[must_use]
        pub fn local_addr(&self) -> SocketAddr {
            self.addr
        }

        /// Waits up to `timeout` for a connection whose handshake completed;
        /// `Ok(None)` when there is none.
        ///
        /// # Errors
        /// [`DtlsError::HandshakeFailed`] for a handshake that failed (the
        /// listener keeps accepting), [`DtlsError::Closed`] if the listener's
        /// accept task is gone.
        pub fn accept_timeout(
            &self,
            timeout: std::time::Duration,
        ) -> Result<Option<WebrtcDtlsConn>, DtlsError> {
            let rx = self.accepted.lock().map_err(|_| DtlsError::Closed)?;
            match rx.recv_timeout(timeout) {
                Ok(item) => item.map(Some),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(DtlsError::Closed),
            }
        }

        /// The runtime that drives the listener, kept alive with it.
        #[must_use]
        pub fn runtime(&self) -> &Arc<Runtime> {
            &self.rt
        }
    }
}

#[cfg(feature = "dtls")]
pub use webrtc_impl::{
    DtlsServerCredentials, WebrtcDtls, WebrtcDtlsConn, WebrtcDtlsListener, WebrtcDtlsServer,
    load_certificate, load_trust_anchors,
};

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn dummy_dtls_handshake_then_send_then_recv() {
        let mut d = DummyDtls::new();
        assert!(!d.is_handshake_complete());
        d.handshake().unwrap();
        assert!(d.is_handshake_complete());
        d.send(&[1, 2, 3, 4]).unwrap();
        let pt = d.recv().unwrap();
        assert_eq!(pt, alloc::vec![1, 2, 3, 4]);
    }

    #[test]
    fn dummy_dtls_send_before_handshake_fails() {
        let mut d = DummyDtls::new();
        let res = d.send(&[1, 2, 3]);
        assert!(matches!(
            res,
            Err(DtlsError::SendFailed {
                reason: "handshake not complete"
            })
        ));
    }

    #[test]
    fn dummy_dtls_recv_before_handshake_fails() {
        let mut d = DummyDtls::new();
        let res = d.recv();
        assert!(matches!(
            res,
            Err(DtlsError::RecvFailed {
                reason: "handshake not complete"
            })
        ));
    }

    #[test]
    fn dummy_dtls_close_returns_closed_on_subsequent_send() {
        let mut d = DummyDtls::new();
        d.handshake().unwrap();
        d.close().unwrap();
        let res = d.send(&[1]);
        assert!(matches!(res, Err(DtlsError::Closed)));
    }

    #[test]
    fn dummy_dtls_close_drains_inbox_then_returns_closed() {
        let mut d = DummyDtls::new();
        d.handshake().unwrap();
        d.send(&[1]).unwrap();
        d.close().unwrap();
        // Inbox drain still allowed.
        let pt = d.recv().unwrap();
        assert_eq!(pt, alloc::vec![1]);
        // danach Closed.
        let res = d.recv();
        assert!(matches!(res, Err(DtlsError::Closed)));
    }

    #[test]
    fn dummy_dtls_inject_makes_recv_yield_payload() {
        let mut d = DummyDtls::new();
        d.handshake().unwrap();
        d.inject(alloc::vec![9, 8, 7]);
        assert_eq!(d.inbox_len(), 1);
        assert_eq!(d.recv().unwrap(), alloc::vec![9, 8, 7]);
        assert_eq!(d.inbox_len(), 0);
    }

    #[test]
    fn dtls_error_display_formats_handshake() {
        let e = DtlsError::HandshakeFailed { reason: "bad cert" };
        let s = alloc::format!("{e}");
        assert!(s.contains("bad cert"));
    }

    #[test]
    fn dtls_error_display_formats_closed() {
        let s = alloc::format!("{}", DtlsError::Closed);
        assert!(s.contains("closed"));
    }

    #[test]
    fn dummy_dtls_default_is_constructible() {
        let _ = DummyDtls::default();
    }
}
