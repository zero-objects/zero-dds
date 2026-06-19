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

    /// A TLS-secured XRCE stream over a `rustls::StreamOwned`. `C` is either a
    /// `ClientConnection` or a `ServerConnection`.
    pub struct RustlsTlsStream<C> {
        stream: StreamOwned<C, TcpStream>,
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
            })
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
            })
        }
    }
}

#[cfg(feature = "tls")]
pub use rustls_impl::{RustlsTlsClient, RustlsTlsServer, RustlsTlsStream};

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
