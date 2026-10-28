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
//! See `internal/dossiers/dtls-pure-rust-dossier.md` + ADR 0011.
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
use webrtc_dtls::cipher_suite::CipherSuiteId;
use webrtc_dtls::config::{ClientAuthType, Config, ExtendedMasterSecretType};
use webrtc_dtls::conn::DTLSConn;
use webrtc_dtls::crypto::Certificate;
use webrtc_dtls::listener::DTLSListener;
use webrtc_util::conn::conn_udp_listener::ListenConfig;
use webrtc_util::conn::{Conn, Listener};

use crate::codec;
use crate::dtls::{DtlsCipherSuite, DtlsCredentials};
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

/// The identity a DTLS peer authenticated with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DtlsPeerIdentity {
    /// The leaf certificate, DER.
    Certificate(Vec<u8>),
    /// The PSK identity (RFC 4279 §2).
    Psk(Vec<u8>),
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

    /// Who the peer proved to be in the handshake: its certificate (leaf
    /// DER) when it presented one, else the PSK identity it used. `None`
    /// for a session without either.
    pub async fn peer_identity(&self) -> Option<DtlsPeerIdentity> {
        let dtls = self.conn.as_any().downcast_ref::<DTLSConn>()?;
        let state = dtls.connection_state().await;
        if let Some(leaf) = state.peer_certificates.first() {
            return Some(DtlsPeerIdentity::Certificate(leaf.clone()));
        }
        (!state.identity_hint.is_empty())
            .then(|| DtlsPeerIdentity::Psk(state.identity_hint.clone()))
    }

    /// Gracefully closes the DTLS connection (close_notify).
    pub async fn close(&self) -> Result<(), DtlsError> {
        self.conn.close().await?;
        Ok(())
    }
}

/// Loads the server certificate chain and private key from the PEM files the
/// Bridge-Spec §7.1 `coap.dtls.cert_file` / `key_file` name.
///
/// The key has to be PKCS#8 (`-----BEGIN PRIVATE KEY-----`); a SEC1
/// (`EC PRIVATE KEY`) or PKCS#1 (`RSA PRIVATE KEY`) file is refused with a
/// message naming the conversion, instead of failing later inside the
/// handshake.
///
/// # Errors
/// [`DtlsError::Io`] if a file cannot be read, [`DtlsError::Dtls`] if the PEM
/// is malformed or the key is not PKCS#8.
pub fn load_certificate(cert_file: &str, key_file: &str) -> Result<Certificate, DtlsError> {
    let cert_pem = std::fs::read_to_string(cert_file)?;
    let key_pem = std::fs::read_to_string(key_file)?;
    let key_blocks = pem::parse_many(&key_pem)
        .map_err(|e| DtlsError::Dtls(format!("key_file {key_file}: {e}")))?;
    let key = key_blocks
        .iter()
        .find(|p| p.tag() == "PRIVATE KEY" || p.tag() == "PRIVATE_KEY")
        .ok_or_else(|| {
            DtlsError::Dtls(format!(
                "key_file {key_file}: no PKCS#8 'PRIVATE KEY' block \
                 (convert with `openssl pkcs8 -topk8 -nocrypt`)"
            ))
        })?;
    let certs = pem::parse_many(&cert_pem)
        .map_err(|e| DtlsError::Dtls(format!("cert_file {cert_file}: {e}")))?;
    if !certs.iter().any(|p| p.tag() == "CERTIFICATE") {
        return Err(DtlsError::Dtls(format!(
            "cert_file {cert_file}: no 'CERTIFICATE' block"
        )));
    }
    // webrtc-dtls' `Certificate::from_pem` wants one document whose first
    // block is the key under its own `PRIVATE_KEY` tag, so re-emit both files
    // in that shape rather than reaching into its private key type.
    let mut joined = pem::encode(&pem::Pem::new(
        "PRIVATE_KEY".to_owned(),
        key.contents().to_vec(),
    ));
    for block in certs.iter().filter(|p| p.tag() == "CERTIFICATE") {
        joined.push_str(&pem::encode(&pem::Pem::new(
            "CERTIFICATE".to_owned(),
            block.contents().to_vec(),
        )));
    }
    Certificate::from_pem(&joined)
        .map_err(|e| DtlsError::Dtls(format!("cert_file {cert_file} / key_file {key_file}: {e}")))
}

/// Loads the trust anchors of `ca_file` into a rustls root store, so client
/// certificates can be validated (Bridge-Spec §7.1 `coap.dtls.ca_file`).
///
/// # Errors
/// [`DtlsError::Io`] if the file cannot be read, [`DtlsError::Dtls`] if it
/// holds no usable `CERTIFICATE` block.
pub fn load_client_cas(ca_file: &str) -> Result<webrtc_dtls::config::Config, DtlsError> {
    let pem_text = std::fs::read_to_string(ca_file)?;
    let blocks = pem::parse_many(&pem_text)
        .map_err(|e| DtlsError::Dtls(format!("ca_file {ca_file}: {e}")))?;
    let mut store = rustls::RootCertStore::empty();
    let mut added = 0usize;
    for block in blocks.iter().filter(|p| p.tag() == "CERTIFICATE") {
        store
            .add(rustls::pki_types::CertificateDer::from(
                block.contents().to_vec(),
            ))
            .map_err(|e| DtlsError::Dtls(format!("ca_file {ca_file}: {e}")))?;
        added += 1;
    }
    if added == 0 {
        return Err(DtlsError::Dtls(format!(
            "ca_file {ca_file}: no 'CERTIFICATE' block"
        )));
    }
    Ok(Config {
        client_cas: store,
        ..Default::default()
    })
}

/// Builds the webrtc-dtls [`Config`] for the configured credentials
/// (Bridge-Spec §7.1): PSK mode when identities are configured, certificate
/// mode otherwise, with the §7.1 cipher suites the DTLS 1.2 stack offers.
///
/// # Errors
/// [`DtlsError`] if a certificate/key/CA file cannot be loaded, or if the
/// configured cipher suites are all unsupported by the DTLS stack.
pub fn server_config(credentials: &DtlsCredentials) -> Result<Config, DtlsError> {
    let mut cfg = Config {
        extended_master_secret: ExtendedMasterSecretType::Require,
        cipher_suites: supported_cipher_suites(&credentials.effective_cipher_suites())?,
        ..Default::default()
    };
    if credentials.uses_psk() {
        // RFC 4279: the server answers the identity the client sent. The
        // callback closes over a snapshot of the store, so a later rotation
        // affects only listeners built afterwards.
        let store = credentials.psk.clone();
        cfg.psk = Some(Arc::new(move |identity: &[u8]| {
            store
                .secret_for(identity)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| {
                    webrtc_dtls::Error::Other(format!(
                        "unknown PSK identity {:?}",
                        String::from_utf8_lossy(identity)
                    ))
                })
        }));
        cfg.psk_identity_hint = credentials
            .psk
            .identity_hint()
            .map(|h| h.as_bytes().to_vec());
        return Ok(cfg);
    }
    match (&credentials.cert_file, &credentials.key_file) {
        (Some(cert), Some(key)) => cfg.certificates = vec![load_certificate(cert, key)?],
        // No cert_file/key_file configured: a self-signed certificate keeps
        // `coaps://` usable for a first run, which is what ADR 0011 shipped.
        _ => {
            cfg.certificates = vec![Certificate::generate_self_signed(vec![
                "localhost".to_owned(),
            ])?];
        }
    }
    if let Some(ca) = &credentials.ca_file {
        cfg.client_cas = load_client_cas(ca)?.client_cas;
        cfg.client_auth = ClientAuthType::RequireAndVerifyClientCert;
    }
    Ok(cfg)
}

/// Maps the Bridge-Spec §7.1 cipher suites onto the ones the DTLS 1.2 stack
/// implements, keeping the configured preference order.
///
/// # Errors
/// [`DtlsError::Dtls`] naming every suite that was configured but is not
/// implemented, when none of the configured suites survives — a handshake that
/// silently negotiated something the operator did not ask for would be worse.
fn supported_cipher_suites(wanted: &[DtlsCipherSuite]) -> Result<Vec<CipherSuiteId>, DtlsError> {
    let mut out = Vec::new();
    let mut missing = Vec::new();
    for suite in wanted {
        match suite {
            DtlsCipherSuite::PskWithAes128Ccm8 => {
                out.push(CipherSuiteId::Tls_Psk_With_Aes_128_Ccm_8)
            }
            DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8 => {
                out.push(CipherSuiteId::Tls_Ecdhe_Ecdsa_With_Aes_128_Ccm_8);
            }
            DtlsCipherSuite::EcdhePskWithAes128CbcSha256 => missing.push(suite.name()),
        }
    }
    if out.is_empty() {
        return Err(DtlsError::Dtls(format!(
            "none of the configured cipher suites is implemented by this DTLS 1.2 \
             stack (missing: {})",
            missing.join(", ")
        )));
    }
    Ok(out)
}

/// A DTLS-secured CoAP server.
///
/// The UDP listener and the DTLS configuration are separate: the socket
/// outlives a [`rotate`](Self::rotate), so a SIGHUP that re-reads the
/// certificate affects the next handshake while established sessions keep
/// running on the keys they already derived (Bridge-Spec §7.1: "Cert-Rotation
/// via SIGHUP (Daemon liest cert/key neu, Sessions halten)").
pub struct DtlsCoapServer {
    udp: Arc<dyn Listener + Send + Sync>,
    listener: std::sync::Mutex<Arc<dyn Listener + Send + Sync>>,
}

impl DtlsCoapServer {
    /// Binds a DTLS listener on `addr` with a freshly generated self-signed
    /// certificate (CN/SAN `localhost`). Requires extended master secret
    /// (RFC 7627) to harden against triple-handshake.
    pub async fn bind(addr: SocketAddr) -> Result<Self, DtlsError> {
        Self::bind_with(addr, &DtlsCredentials::default()).await
    }

    /// Binds a DTLS listener on `addr` with the Bridge-Spec §7.1 credentials:
    /// PSK identities, or a certificate/key pair and an optional client CA.
    pub async fn bind_with(
        addr: SocketAddr,
        credentials: &DtlsCredentials,
    ) -> Result<Self, DtlsError> {
        let cfg = server_config(credentials)?;
        let udp: Arc<dyn Listener + Send + Sync> = Arc::new(
            ListenConfig::default()
                .listen(addr)
                .await
                .map_err(|e| DtlsError::Dtls(e.to_string()))?,
        );
        let listener: Arc<dyn Listener + Send + Sync> =
            Arc::new(DTLSListener::new(Arc::clone(&udp), cfg)?);
        Ok(Self {
            udp,
            listener: std::sync::Mutex::new(listener),
        })
    }

    /// Re-reads the credentials and applies them to the next handshake, on the
    /// same UDP socket. Established sessions are untouched.
    ///
    /// # Errors
    /// [`DtlsError`] if the new credentials cannot be loaded; the previous
    /// configuration then stays in force.
    pub fn rotate(&self, credentials: &DtlsCredentials) -> Result<(), DtlsError> {
        let cfg = server_config(credentials)?;
        let fresh: Arc<dyn Listener + Send + Sync> =
            Arc::new(DTLSListener::new(Arc::clone(&self.udp), cfg)?);
        match self.listener.lock() {
            Ok(mut slot) => {
                *slot = fresh;
                Ok(())
            }
            Err(_) => Err(DtlsError::Dtls(
                "dtls listener lock poisoned during rotation".to_owned(),
            )),
        }
    }

    /// Accepts one inbound DTLS connection, completing the handshake.
    pub async fn accept(&self) -> Result<DtlsCoapSession, DtlsError> {
        let listener = {
            let slot = self
                .listener
                .lock()
                .map_err(|_| DtlsError::Dtls("dtls listener lock poisoned".to_owned()))?;
            Arc::clone(&slot)
        };
        let (conn, peer) = listener.accept().await?;
        Ok(DtlsCoapSession { conn, peer })
    }

    /// The bound local address.
    pub async fn local_addr(&self) -> Result<SocketAddr, DtlsError> {
        Ok(self.udp.addr().await?)
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
        let cfg = Config {
            server_name: server_name.to_owned(),
            insecure_skip_verify: !verify,
            extended_master_secret: ExtendedMasterSecretType::Require,
            cipher_suites: supported_cipher_suites(&[DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8])?,
            ..Default::default()
        };
        Self::dial(server, cfg).await
    }

    /// [`Self::connect`] presenting a client certificate (PEM, PKCS#8 key)
    /// for a server that requires one (`coap.dtls.ca_file`).
    pub async fn connect_with_certificate(
        server: SocketAddr,
        server_name: &str,
        cert_file: &str,
        key_file: &str,
    ) -> Result<DtlsCoapSession, DtlsError> {
        let cfg = Config {
            server_name: server_name.to_owned(),
            insecure_skip_verify: true,
            certificates: vec![load_certificate(cert_file, key_file)?],
            extended_master_secret: ExtendedMasterSecretType::Require,
            cipher_suites: supported_cipher_suites(&[DtlsCipherSuite::EcdheEcdsaWithAes128Ccm8])?,
            ..Default::default()
        };
        Self::dial(server, cfg).await
    }

    /// Dials `server` in PSK mode (Bridge-Spec §7.1
    /// `TLS_PSK_WITH_AES_128_CCM_8`), proving possession of `secret` under
    /// `identity`.
    pub async fn connect_psk(
        server: SocketAddr,
        identity: &str,
        secret: Vec<u8>,
    ) -> Result<DtlsCoapSession, DtlsError> {
        let cfg = Config {
            psk: Some(Arc::new(move |_hint: &[u8]| Ok(secret.clone()))),
            psk_identity_hint: Some(identity.as_bytes().to_vec()),
            extended_master_secret: ExtendedMasterSecretType::Require,
            cipher_suites: supported_cipher_suites(&[DtlsCipherSuite::PskWithAes128Ccm8])?,
            ..Default::default()
        };
        Self::dial(server, cfg).await
    }

    async fn dial(server: SocketAddr, cfg: Config) -> Result<DtlsCoapSession, DtlsError> {
        let udp = UdpSocket::bind(if server.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        })
        .await?;
        udp.connect(server).await?;
        let conn: Arc<dyn Conn + Send + Sync> = Arc::new(udp);
        let dtls = DTLSConn::new(conn, cfg, true, None).await?;
        Ok(DtlsCoapSession {
            conn: Arc::new(dtls),
            peer: server,
        })
    }
}
