// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE TLS wrapper for TCP (Spec §11.3 + §11.4).
//!
//! C6.2.D delivers only a **skeleton** here, no live TLS connection.
//! Rationale: the workspace does have `rustls-pki-types` and
//! `rustls-webpki` (in `crates/security-pki/`, `crates/security-permissions/`),
//! but no `rustls` server/client engine as a dependency. A full
//! integration would pull in transitive crypto providers (ring, aws-lc-rs)
//! and is deployment-specific (embedded vs. Linux). We therefore
//! define a **trait boundary** that can later be filled with rustls or an
//! embedded-TLS lib (e.g. `embedded-tls`).
//!
//! The real wire format (TLS records + length-prefix framing) is
//! identical to §11.3 — TLS terminates on the stream side, the XRCE
//! codec sees only plain bytes.
//!
//! The API form follows `transport_tcp.rs`, so that a later plug-in
//! step needs minimal call-site drift.

use crate::error::XrceError;
use crate::submessages::Message;

/// Trait for TLS stream implementations. Fully implemented in
/// a separate plug-in crate (e.g. `xrce-tls-rustls`).
pub trait XrceTlsStream {
    /// Sends a `Message` over the TLS stream.
    ///
    /// # Errors
    /// `XrceError` if the TLS send or encode fails.
    fn send_message(&mut self, msg: &Message) -> Result<(), XrceError>;

    /// Receives a `Message`.
    ///
    /// # Errors
    /// `XrceError` if the TLS recv or decode fails.
    fn recv_message(&mut self) -> Result<Message, XrceError>;

    /// Closes the stream (close-notify).
    ///
    /// # Errors
    /// `XrceError` if the close-notify cannot be negotiated.
    fn close(&mut self) -> Result<(), XrceError>;
}

/// Skeleton for a TLS client. The real implementation is enabled via the
/// `tls` Cargo feature and uses an external TLS lib.
#[derive(Debug, Default)]
pub struct XrceTlsClient {
    /// Server name for SNI / cert validation.
    pub server_name: alloc::string::String,
}

#[cfg(feature = "alloc")]
extern crate alloc;

impl XrceTlsClient {
    /// Constructor. Takes only the SNI server name. The real connection
    /// happens in `connect`.
    #[must_use]
    pub fn new(server_name: alloc::string::String) -> Self {
        Self { server_name }
    }

    /// Connects (stub). Always returns the `Err(NotImplemented)` equivalent
    /// as `ValueOutOfRange`, until a TLS engine is plugged in.
    ///
    /// # Errors
    /// `XrceError::ValueOutOfRange` with a description. C6.2.D delivers only
    /// the skeleton.
    pub fn connect(&self) -> Result<(), XrceError> {
        Err(XrceError::ValueOutOfRange {
            message: "tls connect: no engine plugged (C6.2.D skeleton)",
        })
    }
}

/// Skeleton for a TLS server.
#[derive(Debug, Default)]
pub struct XrceTlsServer;

impl XrceTlsServer {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Binds a TLS listener (stub).
    ///
    /// # Errors
    /// `XrceError::ValueOutOfRange`, until a TLS engine is plugged in.
    pub fn bind(&self) -> Result<(), XrceError> {
        Err(XrceError::ValueOutOfRange {
            message: "tls bind: no engine plugged (C6.2.D skeleton)",
        })
    }
}

/// Productive TLS 1.2/1.3 backend (rustls) for the §3.1.10 transport profile.
/// Opt-in `tls` feature; rustls is sync so no async runtime is needed. Frames
/// XRCE [`Message`]s with a `u16` little-endian length prefix over the
/// encrypted stream, identical to `transport_tcp.rs`.
#[cfg(feature = "tls")]
mod rustls_impl {
    use super::{Message, XrceError, XrceTlsStream};
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};

    use rustls::{ClientConfig, ClientConnection, ServerConfig, ServerConnection, StreamOwned};
    use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};

    fn err(message: &'static str) -> XrceError {
        XrceError::ValueOutOfRange { message }
    }

    /// The server side of a TLS connection.
    pub type RustlsServerStream = RustlsTlsStream<ServerConnection>;

    /// The client side of a TLS connection.
    pub type RustlsClientStream = RustlsTlsStream<ClientConnection>;

    /// A credential file (certificate chain, private key, trust anchors)
    /// that cannot be read or is not usable.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TlsConfigError {
        /// What is wrong, naming the file.
        pub reason: std::string::String,
    }

    impl core::fmt::Display for TlsConfigError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "tls credentials: {}", self.reason)
        }
    }

    impl std::error::Error for TlsConfigError {}

    fn config_err(reason: std::string::String) -> TlsConfigError {
        TlsConfigError { reason }
    }

    /// The certificate chain of a PEM file, leaf first.
    fn load_chain(cert_file: &str) -> Result<Vec<CertificateDer<'static>>, TlsConfigError> {
        use rustls_pki_types::pem::PemObject;
        let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert_file)
            .map_err(|e| config_err(std::format!("cert_file {cert_file}: {e}")))?
            .collect::<Result<_, _>>()
            .map_err(|e| config_err(std::format!("cert_file {cert_file}: {e}")))?;
        if chain.is_empty() {
            return Err(config_err(std::format!(
                "cert_file {cert_file}: no `CERTIFICATE` block"
            )));
        }
        Ok(chain)
    }

    /// The private key of a PEM file (PKCS#8, PKCS#1 or SEC1).
    fn load_key(key_file: &str) -> Result<PrivateKeyDer<'static>, TlsConfigError> {
        use rustls_pki_types::pem::PemObject;
        PrivateKeyDer::from_pem_file(key_file)
            .map_err(|e| config_err(std::format!("key_file {key_file}: {e}")))
    }

    /// The trust anchors of a PEM file.
    fn load_roots(ca_file: &str) -> Result<rustls::RootCertStore, TlsConfigError> {
        let mut roots = rustls::RootCertStore::empty();
        for cert in load_chain(ca_file)
            .map_err(|e| config_err(e.reason.replacen("cert_file", "ca_file", 1)))?
        {
            roots
                .add(cert)
                .map_err(|e| config_err(std::format!("ca_file {ca_file}: {e}")))?;
        }
        Ok(roots)
    }

    /// A TLS-secured XRCE stream over a `rustls::StreamOwned`. `C` is either a
    /// `ClientConnection` or a `ServerConnection`.
    pub struct RustlsTlsStream<C> {
        stream: StreamOwned<C, TcpStream>,
        /// Bytes read from the stream that do not yet make a whole frame
        /// ([`Self::poll_message`]).
        pending: Vec<u8>,
    }

    impl<C> RustlsTlsStream<C>
    where
        StreamOwned<C, TcpStream>: Read + Write,
    {
        fn write_message(&mut self, msg: &Message) -> Result<(), XrceError> {
            let bytes = msg.encode()?;
            let len = u16::try_from(bytes.len()).map_err(|_| err("tls message too large"))?;
            self.stream
                .write_all(&len.to_le_bytes())
                .map_err(|_| err("tls write length prefix"))?;
            self.stream
                .write_all(&bytes)
                .map_err(|_| err("tls write body"))?;
            self.stream.flush().map_err(|_| err("tls flush"))?;
            Ok(())
        }

        /// Limits how long a read waits, for [`Self::poll_message`].
        ///
        /// # Errors
        /// [`XrceError::ValueOutOfRange`] on a socket error.
        pub fn set_read_timeout(
            &self,
            timeout: Option<std::time::Duration>,
        ) -> Result<(), XrceError> {
            self.stream
                .sock
                .set_read_timeout(timeout)
                .map_err(|_| err("tls set_read_timeout"))
        }

        /// Reads the next message if one arrives within the read timeout set
        /// with [`Self::set_read_timeout`]; `Ok(None)` when none did. Bytes
        /// of a frame that is not complete yet are kept for the next call, so
        /// a timeout in the middle of a frame loses nothing.
        ///
        /// # Errors
        /// [`XrceError`] when the connection is closed or fails, or the
        /// frame is not an XRCE message.
        pub fn poll_message(&mut self) -> Result<Option<Message>, XrceError> {
            loop {
                if self.pending.len() >= 2 {
                    let len = usize::from(u16::from_le_bytes([self.pending[0], self.pending[1]]));
                    if self.pending.len() >= 2 + len {
                        let frame: Vec<u8> = self.pending.drain(..2 + len).skip(2).collect();
                        return Message::decode(&frame).map(Some);
                    }
                }
                let mut chunk = [0u8; 4096];
                match self.stream.read(&mut chunk) {
                    Ok(0) => return Err(err("tls connection closed")),
                    Ok(n) => self.pending.extend_from_slice(&chunk[..n]),
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) =>
                    {
                        return Ok(None);
                    }
                    Err(_) => return Err(err("tls read")),
                }
            }
        }

        fn read_message(&mut self) -> Result<Message, XrceError> {
            let mut prefix = [0u8; 2];
            self.stream
                .read_exact(&mut prefix)
                .map_err(|_| err("tls read length prefix"))?;
            let len = u16::from_le_bytes(prefix) as usize;
            let mut body = std::vec![0u8; len];
            self.stream
                .read_exact(&mut body)
                .map_err(|_| err("tls read body"))?;
            Message::decode(&body)
        }
    }

    impl XrceTlsStream for RustlsTlsStream<ClientConnection> {
        fn send_message(&mut self, msg: &Message) -> Result<(), XrceError> {
            self.write_message(msg)
        }
        fn recv_message(&mut self) -> Result<Message, XrceError> {
            self.read_message()
        }
        fn close(&mut self) -> Result<(), XrceError> {
            self.stream.conn.send_close_notify();
            self.stream.flush().map_err(|_| err("tls close flush"))
        }
    }

    impl XrceTlsStream for RustlsTlsStream<ServerConnection> {
        fn send_message(&mut self, msg: &Message) -> Result<(), XrceError> {
            self.write_message(msg)
        }
        fn recv_message(&mut self) -> Result<Message, XrceError> {
            self.read_message()
        }
        fn close(&mut self) -> Result<(), XrceError> {
            self.stream.conn.send_close_notify();
            self.stream.flush().map_err(|_| err("tls close flush"))
        }
    }

    /// A TLS server: binds a TCP listener and serves a freshly generated
    /// self-signed certificate (CN/SAN `localhost`).
    pub struct RustlsTlsServer {
        listener: TcpListener,
        cfg: Arc<ServerConfig>,
        cert_der: Vec<u8>,
    }

    impl RustlsTlsServer {
        /// Binds on `addr` (use `127.0.0.1:0` for an ephemeral port).
        ///
        /// # Errors
        /// [`XrceError::ValueOutOfRange`] on cert/config/bind failure.
        pub fn bind(addr: SocketAddr) -> Result<Self, XrceError> {
            let ck = rcgen::generate_simple_self_signed(std::vec!["localhost".to_string()])
                .map_err(|_| err("tls self-signed cert gen"))?;
            let cert_der = ck.cert.der().to_vec();
            let key_der = ck.key_pair.serialize_der();
            let certs = std::vec![CertificateDer::from(cert_der.clone())];
            let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der));
            let cfg = ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|_| err("tls server protocol versions"))?
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|_| err("tls server cert"))?;
            let listener = TcpListener::bind(addr).map_err(|_| err("tls tcp bind"))?;
            Ok(Self {
                listener,
                cfg: Arc::new(cfg),
                cert_der,
            })
        }

        /// The server's self-signed certificate (DER) — the client must trust
        /// this to validate the handshake.
        #[must_use]
        pub fn cert_der(&self) -> &[u8] {
            &self.cert_der
        }

        /// The bound local address.
        ///
        /// # Errors
        /// [`XrceError::ValueOutOfRange`] on socket error.
        pub fn local_addr(&self) -> Result<SocketAddr, XrceError> {
            self.listener
                .local_addr()
                .map_err(|_| err("tls local_addr"))
        }

        /// Accepts one inbound TCP connection and wraps it in a TLS stream. The
        /// handshake completes lazily on the first message exchange.
        ///
        /// # Errors
        /// [`XrceError::ValueOutOfRange`] on accept/TLS failure.
        pub fn accept(&self) -> Result<RustlsTlsStream<ServerConnection>, XrceError> {
            let (tcp, _peer) = self.listener.accept().map_err(|_| err("tls accept"))?;
            let conn = ServerConnection::new(Arc::clone(&self.cfg))
                .map_err(|_| err("tls server connection"))?;
            Ok(RustlsTlsStream {
                stream: StreamOwned::new(conn, tcp),
                pending: Vec::new(),
            })
        }

        /// Binds on `addr` with the certificate chain and private key of PEM
        /// files, so a client can authenticate the agent (DDS-XRCE 1.0
        /// §11.1 (5), §11.3). With `client_ca_file`, a client must present a
        /// certificate that chains to one of its trust anchors.
        ///
        /// # Errors
        /// [`TlsConfigError`] for an unusable file or a certificate that does
        /// not match the key, or a port that cannot be bound.
        pub fn bind_with_certificate(
            addr: SocketAddr,
            cert_file: &str,
            key_file: &str,
            client_ca_file: Option<&str>,
        ) -> Result<Self, TlsConfigError> {
            let chain = load_chain(cert_file)?;
            let key = load_key(key_file)?;
            let cert_der = chain[0].as_ref().to_vec();
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let builder = ServerConfig::builder_with_provider(Arc::clone(&provider))
                .with_safe_default_protocol_versions()
                .map_err(|e| config_err(std::format!("protocol versions: {e}")))?;
            let builder = match client_ca_file {
                Some(ca) => {
                    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
                        Arc::new(load_roots(ca)?),
                        provider,
                    )
                    .build()
                    .map_err(|e| config_err(std::format!("ca_file {ca}: {e}")))?;
                    builder.with_client_cert_verifier(verifier)
                }
                None => builder.with_no_client_auth(),
            };
            let cfg = builder.with_single_cert(chain, key).map_err(|e| {
                config_err(std::format!(
                    "cert_file {cert_file} / key_file {key_file}: {e}"
                ))
            })?;
            let listener = TcpListener::bind(addr)
                .map_err(|e| config_err(std::format!("bind {addr}: {e}")))?;
            Ok(Self {
                listener,
                cfg: Arc::new(cfg),
                cert_der,
            })
        }

        /// Makes [`Self::accept`] non-blocking (it then fails with
        /// `ValueOutOfRange` when no connection waits; use
        /// [`Self::try_accept`]).
        ///
        /// # Errors
        /// [`XrceError::ValueOutOfRange`] on a socket error.
        pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), XrceError> {
            self.listener
                .set_nonblocking(nonblocking)
                .map_err(|_| err("tls set_nonblocking"))
        }

        /// Accepts a waiting connection, `Ok(None)` if none waits; the
        /// listener has to be non-blocking ([`Self::set_nonblocking`]). The
        /// handshake completes on the first read or write of the stream, with
        /// its read timeout ([`RustlsTlsStream::set_read_timeout`]).
        ///
        /// # Errors
        /// [`XrceError::ValueOutOfRange`] on accept or TLS failure.
        pub fn try_accept(
            &self,
        ) -> Result<Option<(RustlsTlsStream<ServerConnection>, SocketAddr)>, XrceError> {
            let (tcp, peer) = match self.listener.accept() {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                Err(_) => return Err(err("tls accept")),
            };
            tcp.set_nonblocking(false)
                .map_err(|_| err("tls accepted socket"))?;
            let conn = ServerConnection::new(Arc::clone(&self.cfg))
                .map_err(|_| err("tls server connection"))?;
            Ok(Some((
                RustlsTlsStream {
                    stream: StreamOwned::new(conn, tcp),
                    pending: Vec::new(),
                },
                peer,
            )))
        }
    }

    /// A TLS client.
    pub struct RustlsTlsClient;

    impl RustlsTlsClient {
        /// Dials `server`, trusting `trust_cert_der` (e.g. the server's
        /// self-signed cert) for handshake validation. The handshake completes
        /// lazily on the first message.
        ///
        /// # Errors
        /// [`XrceError::ValueOutOfRange`] on config/connect failure.
        pub fn connect(
            server: SocketAddr,
            server_name: &str,
            trust_cert_der: &[u8],
        ) -> Result<RustlsTlsStream<ClientConnection>, XrceError> {
            let mut roots = rustls::RootCertStore::empty();
            roots
                .add(CertificateDer::from(trust_cert_der.to_vec()))
                .map_err(|_| err("tls add trust anchor"))?;
            let cfg = ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|_| err("tls client protocol versions"))?
            .with_root_certificates(roots)
            .with_no_client_auth();
            let name = ServerName::try_from(server_name.to_string())
                .map_err(|_| err("tls invalid server name"))?;
            let conn = ClientConnection::new(Arc::new(cfg), name)
                .map_err(|_| err("tls client connection"))?;
            let tcp = TcpStream::connect(server).map_err(|_| err("tls tcp connect"))?;
            Ok(RustlsTlsStream {
                stream: StreamOwned::new(conn, tcp),
                pending: Vec::new(),
            })
        }

        /// Dials `server` and authenticates it: its certificate has to chain
        /// to the trust anchors of `ca_file` and name `server_name`. With
        /// `client_certificate` (certificate file, key file) the client
        /// presents its own, for an agent that asks for one. The handshake
        /// completes lazily on the first message.
        ///
        /// # Errors
        /// [`TlsConfigError`] for an unusable file or a failed connect.
        pub fn connect_verified(
            server: SocketAddr,
            server_name: &str,
            ca_file: &str,
            client_certificate: Option<(&str, &str)>,
        ) -> Result<RustlsTlsStream<ClientConnection>, TlsConfigError> {
            let builder = ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|e| config_err(std::format!("protocol versions: {e}")))?
            .with_root_certificates(load_roots(ca_file)?);
            let cfg = match client_certificate {
                Some((cert, key)) => builder
                    .with_client_auth_cert(load_chain(cert)?, load_key(key)?)
                    .map_err(|e| config_err(std::format!("client certificate: {e}")))?,
                None => builder.with_no_client_auth(),
            };
            let name = ServerName::try_from(server_name.to_string())
                .map_err(|_| config_err(std::format!("invalid server name {server_name}")))?;
            let conn = ClientConnection::new(Arc::new(cfg), name)
                .map_err(|e| config_err(std::format!("client connection: {e}")))?;
            let tcp = TcpStream::connect(server)
                .map_err(|e| config_err(std::format!("connect {server}: {e}")))?;
            Ok(RustlsTlsStream {
                stream: StreamOwned::new(conn, tcp),
                pending: Vec::new(),
            })
        }
    }
}

#[cfg(feature = "tls")]
pub use rustls_impl::{
    RustlsClientStream, RustlsServerStream, RustlsTlsClient, RustlsTlsServer, RustlsTlsStream,
    TlsConfigError,
};

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn tls_client_new_stores_server_name() {
        let c = XrceTlsClient::new("agent.example.invalid".into());
        assert_eq!(c.server_name, "agent.example.invalid");
    }

    #[test]
    fn tls_client_connect_returns_skeleton_error() {
        let c = XrceTlsClient::new("x".into());
        let res = c.connect();
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn tls_server_bind_returns_skeleton_error() {
        let s = XrceTlsServer::new();
        let res = s.bind();
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn tls_server_default_constructible() {
        let _ = XrceTlsServer;
    }
}
