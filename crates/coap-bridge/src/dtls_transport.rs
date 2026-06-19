// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! **Experimental** DTLS 1.2 transport for `coaps://` (RFC 7252 §9 / RFC 6347).
//!
//! Feature `dtls`. Built on [`webrtc-dtls`](https://crates.io/crates/webrtc-dtls)
//! — a pure-Rust DTLS 1.2 stack (MIT/Apache, used in the `webrtc-rs` project).
//! The transport opens a UDP socket, runs the DTLS handshake, and then carries
//! CoAP messages (encoded via [`crate::codec`]) as DTLS application data.
//!
//! # Maturity
//!
//! **This is an opt-in, experimental security profile.** webrtc-dtls is not
//! independently audited for general (non-WebRTC) use, and DTLS 1.2 (not 1.3)
//! is the negotiated version. It is **not** wired into the published no_std
//! codec core and is **not** a production-hardened default. For constrained
//! object security without a DTLS handshake, prefer OSCORE ([`crate::oscore`]).
//! See `docs/dossiers/dtls-pure-rust-dossier.md` + ADR 0011.
//!
//! # Example
//!
//! ```no_run
//! # async fn ex() -> Result<(), zerodds_coap_bridge::dtls_transport::DtlsError> {
//! use zerodds_coap_bridge::dtls_transport::{DtlsCoapServer, DtlsCoapClient};
//! use zerodds_coap_bridge::message::{CoapMessage, CoapCode, MessageType};
//!
//! // Server: bind, accept one DTLS session, echo a 2.05 Content.
//! let server = DtlsCoapServer::bind("127.0.0.1:5684".parse().unwrap()).await?;
//! let session = server.accept().await?;
//! let req = session.recv_message().await?;
//! let mut resp = CoapMessage::new(MessageType::Acknowledgement, CoapCode::CONTENT, req.message_id);
//! resp.token = req.token.clone();
//! resp.payload = b"hi".to_vec();
//! session.send_message(&resp).await?;
//! # Ok(()) }
//! ```

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;
use webrtc_dtls::config::{Config, ExtendedMasterSecretType};
use webrtc_dtls::conn::DTLSConn;
use webrtc_dtls::crypto::Certificate;
use webrtc_dtls::listener::listen;
use webrtc_util::conn::{Conn, Listener};

use crate::codec;
use crate::message::CoapMessage;

/// Default `coaps://` UDP port (RFC 7252 §12.2).
pub const COAPS_PORT: u16 = 5684;

/// Maximum datagram the transport will read in one `recv` (DTLS record + CoAP).
const MAX_DATAGRAM: usize = 1500;

/// Errors from the DTLS-CoAP transport.
#[derive(Debug)]
pub enum DtlsError {
    /// Underlying I/O / socket error.
    Io(std::io::Error),
    /// DTLS handshake or record error from webrtc-dtls.
    Dtls(String),
    /// CoAP codec error encoding/decoding the application data.
    Codec(codec::CodecError),
    /// Peer closed the DTLS connection.
    Closed,
}

impl fmt::Display for DtlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DtlsError::Io(e) => write!(f, "io: {e}"),
            DtlsError::Dtls(e) => write!(f, "dtls: {e}"),
            DtlsError::Codec(e) => write!(f, "coap codec: {e:?}"),
            DtlsError::Closed => write!(f, "dtls connection closed"),
        }
    }
}

impl std::error::Error for DtlsError {}

impl From<std::io::Error> for DtlsError {
    fn from(e: std::io::Error) -> Self {
        DtlsError::Io(e)
    }
}

impl From<webrtc_dtls::Error> for DtlsError {
    fn from(e: webrtc_dtls::Error) -> Self {
        DtlsError::Dtls(e.to_string())
    }
}

impl From<webrtc_util::Error> for DtlsError {
    fn from(e: webrtc_util::Error) -> Self {
        DtlsError::Dtls(e.to_string())
    }
}

/// A DTLS-secured CoAP session — wraps the established DTLS connection
/// ([`webrtc_util::conn::Conn`]) for both the server-accepted and the
/// client-dialed side.
pub struct DtlsCoapSession {
    conn: Arc<dyn Conn + Send + Sync>,
    peer: SocketAddr,
}

impl DtlsCoapSession {
    /// The remote peer address.
    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    /// Encodes `msg` and writes it as one DTLS application-data record.
    pub async fn send_message(&self, msg: &CoapMessage) -> Result<(), DtlsError> {
        let bytes = codec::encode(msg).map_err(DtlsError::Codec)?;
        self.conn.send(&bytes).await?;
        Ok(())
    }

    /// Reads the next DTLS application-data record and decodes it as a CoAP
    /// message. Returns [`DtlsError::Closed`] when the peer closes.
    pub async fn recv_message(&self) -> Result<CoapMessage, DtlsError> {
        let mut buf = vec![0u8; MAX_DATAGRAM];
        let n = self.conn.recv(&mut buf).await?;
        if n == 0 {
            return Err(DtlsError::Closed);
        }
        codec::decode(&buf[..n]).map_err(DtlsError::Codec)
    }

    /// Gracefully closes the DTLS connection (close_notify).
    pub async fn close(&self) -> Result<(), DtlsError> {
        self.conn.close().await?;
        Ok(())
    }
}

/// A DTLS-secured CoAP server. Generates a self-signed certificate on bind.
pub struct DtlsCoapServer {
    listener: Arc<dyn Listener + Send + Sync>,
}

impl DtlsCoapServer {
    /// Binds a DTLS listener on `addr` with a freshly generated self-signed
    /// certificate (CN/SAN `localhost`). Require extended master secret
    /// (RFC 7627) to harden against triple-handshake.
    pub async fn bind(addr: SocketAddr) -> Result<Self, DtlsError> {
        let cert = Certificate::generate_self_signed(vec!["localhost".to_owned()])?;
        let cfg = Config {
            certificates: vec![cert],
            extended_master_secret: ExtendedMasterSecretType::Require,
            ..Default::default()
        };
        let listener = listen(addr, cfg).await?;
        Ok(Self {
            listener: Arc::new(listener),
        })
    }

    /// Accepts one inbound DTLS connection, completing the handshake.
    pub async fn accept(&self) -> Result<DtlsCoapSession, DtlsError> {
        let (conn, peer) = self.listener.accept().await?;
        Ok(DtlsCoapSession { conn, peer })
    }

    /// The bound local address.
    pub async fn local_addr(&self) -> Result<SocketAddr, DtlsError> {
        Ok(self.listener.addr().await?)
    }
}

/// A DTLS-secured CoAP client.
pub struct DtlsCoapClient;

impl DtlsCoapClient {
    /// Dials `server`, completing the DTLS handshake, and returns a session.
    ///
    /// `verify` controls peer-certificate verification: `false` accepts the
    /// server's self-signed certificate (test/dev — the **experimental**
    /// default), `true` requires a chain validating against the system roots.
    pub async fn connect(
        server: SocketAddr,
        server_name: &str,
        verify: bool,
    ) -> Result<DtlsCoapSession, DtlsError> {
        let udp = UdpSocket::bind(if server.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        })
        .await?;
        udp.connect(server).await?;
        let conn: Arc<dyn Conn + Send + Sync> = Arc::new(udp);
        let cfg = Config {
            server_name: server_name.to_owned(),
            insecure_skip_verify: !verify,
            extended_master_secret: ExtendedMasterSecretType::Require,
            ..Default::default()
        };
        let dtls = DTLSConn::new(conn, cfg, true, None).await?;
        Ok(DtlsCoapSession {
            conn: Arc::new(dtls),
            peer: server,
        })
    }
}
