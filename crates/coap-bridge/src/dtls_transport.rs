// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP over DTLS 1.2 (`coaps://`, RFC 7252 §9) on OpenSSL, through
//! `zerodds-dtls-openssl` (ADR 0014): pre-shared keys (§9.1.3.1), raw
//! public keys checked against RFC 6920 names (§9.1.3.2, RFC 7250) and
//! X.509 certificates (§9.1.3.3). One CoAP message per DTLS record.
//! Blocking: a session belongs to one thread, other threads hand it
//! messages through a channel and its [`Waker`].

use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use zerodds_dtls_openssl as dtls;
pub use zerodds_dtls_openssl::{KeyName, PeerIdentity as DtlsPeerIdentity, Waker};

use crate::codec;
use crate::dtls::DtlsCredentials;
use crate::message::CoapMessage;

/// The `coaps` default port (RFC 7252 §6.2).
pub const COAPS_PORT: u16 = 5684;

/// The names of the certificate a server generates when it has no
/// credential of its own (a first run).
pub const SELF_SIGNED_NAMES: [&str; 1] = ["localhost"];

/// DTLS transport errors.
#[derive(Debug)]
pub enum DtlsError {
    /// The socket, or a handshake that failed or timed out.
    Io(io::Error),
    /// The credentials do not make a DTLS context.
    Config(String),
    /// A record that is no CoAP message.
    Codec(codec::CodecError),
    /// The peer closed the session.
    Closed,
}

impl fmt::Display for DtlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "dtls: {e}"),
            Self::Config(e) => write!(f, "dtls configuration: {e}"),
            Self::Codec(e) => write!(f, "coap codec: {e}"),
            Self::Closed => write!(f, "dtls session closed"),
        }
    }
}

impl std::error::Error for DtlsError {}

impl From<io::Error> for DtlsError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::ConnectionAborted {
            Self::Closed
        } else {
            Self::Io(e)
        }
    }
}

impl From<dtls::ConfigError> for DtlsError {
    fn from(e: dtls::ConfigError) -> Self {
        Self::Config(e.to_string())
    }
}

/// A DTLS-secured CoAP session, server- or client-side.
pub struct DtlsCoapSession {
    inner: dtls::Session,
}

impl DtlsCoapSession {
    /// The peer's address.
    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.inner.peer_addr()
    }

    /// Who the peer proved to be: its pre-shared key identity, its
    /// certificate or its raw public key.
    #[must_use]
    pub fn peer_identity(&self) -> Option<DtlsPeerIdentity> {
        self.inner.peer_identity().cloned()
    }

    /// The cipher suite of the session, OpenSSL's name.
    #[must_use]
    pub fn cipher(&self) -> Option<&'static str> {
        self.inner.cipher()
    }

    /// The handle that interrupts a waiting [`Self::recv_message`].
    #[must_use]
    pub fn waker(&self) -> Waker {
        self.inner.waker()
    }

    /// Encodes `msg` and sends it as one record.
    ///
    /// # Errors
    /// [`DtlsError`].
    pub fn send_message(&mut self, msg: &CoapMessage) -> Result<(), DtlsError> {
        let bytes = codec::encode(msg).map_err(DtlsError::Codec)?;
        self.inner.send(&bytes)?;
        Ok(())
    }

    /// The next message, `None` when none came within `timeout` or the
    /// waker was used.
    ///
    /// # Errors
    /// [`DtlsError::Closed`] when the peer closed the session,
    /// [`DtlsError::Codec`] for a record that is no CoAP message.
    pub fn recv_message(&mut self, timeout: Duration) -> Result<Option<CoapMessage>, DtlsError> {
        match self.inner.recv(timeout)? {
            Some(bytes) => codec::decode(&bytes).map(Some).map_err(DtlsError::Codec),
            None => Ok(None),
        }
    }

    /// Sends close_notify.
    pub fn close(&mut self) {
        self.inner.close();
    }
}

/// The RFC 6920 names of `names`.
///
/// # Errors
/// [`DtlsError::Config`] naming the first that is none.
pub fn parse_key_names(names: &[String]) -> Result<Vec<KeyName>, DtlsError> {
    names
        .iter()
        .map(|n| KeyName::parse(n).map_err(|e| DtlsError::Config(format!("`{n}`: {e}"))))
        .collect()
}

/// The server side of the Bridge-Spec §7.1 credentials: the pre-shared
/// keys, the certificate or the raw public key, the client CA and the
/// trusted client keys, the cipher suites; without any credential a
/// self-signed certificate for `localhost`.
///
/// # Errors
/// A trusted key that is no RFC 6920 name.
pub fn server_config(credentials: &DtlsCredentials) -> Result<dtls::ServerConfig, DtlsError> {
    let certificate = match (&credentials.cert_file, &credentials.key_file) {
        (Some(cert), Some(key)) => Some(dtls::CertificateFiles {
            cert_file: PathBuf::from(cert),
            key_file: PathBuf::from(key),
        }),
        _ => None,
    };
    let own = certificate.is_some() || credentials.rpk_key_file.is_some();
    Ok(dtls::ServerConfig {
        psk: credentials
            .psk
            .entries()
            .iter()
            .map(|e| (e.identity.clone(), e.secret.clone()))
            .collect(),
        psk_identity_hint: credentials.psk.identity_hint().map(str::to_string),
        self_signed: if own || !credentials.psk.is_empty() {
            Vec::new()
        } else {
            SELF_SIGNED_NAMES.map(String::from).to_vec()
        },
        certificate,
        raw_public_key: credentials.rpk_key_file.as_ref().map(PathBuf::from),
        client_ca_file: credentials.ca_file.as_ref().map(PathBuf::from),
        trusted_client_keys: parse_key_names(&credentials.rpk_trusted)?,
        cipher_suites: credentials
            .cipher_suites
            .iter()
            .map(|s| s.name().to_string())
            .collect(),
    })
}

/// A DTLS-secured CoAP server.
pub struct DtlsCoapServer {
    inner: dtls::Server,
}

impl DtlsCoapServer {
    /// Binds `addr` with a self-signed certificate for `localhost`.
    ///
    /// # Errors
    /// [`DtlsError`].
    pub fn bind(addr: SocketAddr) -> Result<Self, DtlsError> {
        Self::bind_with(addr, &DtlsCredentials::default())
    }

    /// Binds `addr` with the Bridge-Spec §7.1 credentials.
    ///
    /// # Errors
    /// [`DtlsError`].
    pub fn bind_with(addr: SocketAddr, credentials: &DtlsCredentials) -> Result<Self, DtlsError> {
        Ok(Self {
            inner: dtls::Server::bind(addr, &server_config(credentials)?)?,
        })
    }

    /// Takes new credentials for the next handshakes; established sessions
    /// keep theirs (Bridge-Spec §7.1: "Sessions halten").
    ///
    /// # Errors
    /// [`DtlsError`]; the previous credentials stay.
    pub fn rotate(&self, credentials: &DtlsCredentials) -> Result<(), DtlsError> {
        self.inner.rotate(&server_config(credentials)?)?;
        Ok(())
    }

    /// How many handshakes failed since the bind (Bridge-Spec §8.2,
    /// `dtls_handshakes_total{result="failed"}`).
    #[must_use]
    pub fn failed_handshakes(&self) -> u64 {
        self.inner.failed_handshakes()
    }

    /// The next session whose handshake completed, waiting up to `timeout`.
    #[must_use]
    pub fn accept(&self, timeout: Duration) -> Option<DtlsCoapSession> {
        self.inner
            .accept(timeout)
            .map(|inner| DtlsCoapSession { inner })
    }

    /// The bound address.
    ///
    /// # Errors
    /// As `UdpSocket::local_addr`.
    pub fn local_addr(&self) -> Result<SocketAddr, DtlsError> {
        Ok(self.inner.local_addr()?)
    }
}

/// A DTLS-secured CoAP client.
pub struct DtlsCoapClient;

impl DtlsCoapClient {
    /// Opens a session with `config`, the handshake within `timeout`.
    ///
    /// # Errors
    /// [`DtlsError`].
    pub fn connect(
        server: SocketAddr,
        config: &dtls::ClientConfig,
        timeout: Duration,
    ) -> Result<DtlsCoapSession, DtlsError> {
        Ok(DtlsCoapSession {
            inner: dtls::connect(server, config, timeout)?,
        })
    }

    /// A session with a pre-shared key (RFC 7252 §9.1.3.1).
    ///
    /// # Errors
    /// [`DtlsError`].
    pub fn connect_psk(
        server: SocketAddr,
        identity: &str,
        secret: Vec<u8>,
        timeout: Duration,
    ) -> Result<DtlsCoapSession, DtlsError> {
        let config = dtls::ClientConfig {
            psk: Some((identity.to_string(), secret)),
            ..dtls::ClientConfig::default()
        };
        Self::connect(server, &config, timeout)
    }

    /// A session whose server certificate is checked against `ca_file` and
    /// `server_name` (§9.1.3.3), presenting `client_cert` (certificate and
    /// key file) when given.
    ///
    /// # Errors
    /// [`DtlsError`], among others for a certificate the anchors do not
    /// vouch for.
    pub fn connect_verified(
        server: SocketAddr,
        server_name: &str,
        ca_file: &str,
        client_cert: Option<(&str, &str)>,
        timeout: Duration,
    ) -> Result<DtlsCoapSession, DtlsError> {
        let config = dtls::ClientConfig {
            ca_file: Some(PathBuf::from(ca_file)),
            server_name: Some(server_name.to_string()),
            certificate: client_cert.map(|(c, k)| dtls::CertificateFiles {
                cert_file: PathBuf::from(c),
                key_file: PathBuf::from(k),
            }),
            ..dtls::ClientConfig::default()
        };
        Self::connect(server, &config, timeout)
    }

    /// A session with raw public keys (§9.1.3.2): `key_file` the client's
    /// private key, `trusted` the names of the server keys it accepts.
    ///
    /// # Errors
    /// [`DtlsError`], among others for a server key not in `trusted`.
    pub fn connect_rpk(
        server: SocketAddr,
        key_file: &str,
        trusted: Vec<KeyName>,
        timeout: Duration,
    ) -> Result<DtlsCoapSession, DtlsError> {
        let config = dtls::ClientConfig {
            raw_public_key: Some(PathBuf::from(key_file)),
            trusted_server_keys: trusted,
            ..dtls::ClientConfig::default()
        };
        Self::connect(server, &config, timeout)
    }
}
