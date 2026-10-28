// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TLS bracket for the AMQP endpoint.
//!
//! Spec dds-amqp-1.0 §10.1 — TLS 1.2 minimum, 1.3 RECOMMENDED;
//! Cert validation; AMQP negotiation **after** TLS handshake.
//!
//! Implemented via `rustls` (cargo feature `tls`). After the
//! TLS handshake the resulting `rustls::StreamOwned`
//! is passed directly to [`crate::handler::handle_connection`] —
//! which accepts any `Read + Write` stream.
//!
//! # Server-Side
//!
//! [`accept_server`] takes a connected `TcpStream`,
//! performs the TLS handshake against the configured
//! [`ServerTlsConfig`] and returns a `StreamOwned` that
//! the caller can pass to `handle_connection`.
//!
//! # Client-Side (Bridge-Profile)
//!
//! [`connect_client`] symmetrically for the outbound bridge path.
//!
//! # Cert-Validation
//!
//! The server optionally accepts client certs (mTLS) via
//! `require_client_cert = true`. The client validates the
//! server cert against the configured root CAs. Both via
//! `rustls`-Default-Crypto-Provider (ring).

use std::io::{self, BufReader};
use std::net::TcpStream;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::server::WebPkiClientVerifier;
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
};

/// Spec §10.1 — server-side TLS configuration.
#[derive(Debug, Clone)]
pub struct ServerTlsConfig {
    /// PEM bytes of the server cert (may contain a chain).
    pub cert_pem: Vec<u8>,
    /// PEM bytes of the server private key (PKCS#8 or PKCS#1).
    pub key_pem: Vec<u8>,
    /// Optional CA bundle PEM bytes; if set + `require_client_cert`,
    /// mTLS is enabled.
    pub client_ca_pem: Option<Vec<u8>>,
    /// Spec §10.1 — require client cert (mTLS).
    pub require_client_cert: bool,
}

/// Spec §10.1 — client-side TLS configuration (bridge profile).
#[derive(Debug, Clone)]
pub struct ClientTlsConfig {
    /// Trusted root CA bundle (PEM bytes).
    pub trust_anchors_pem: Vec<u8>,
    /// Optional: client cert + key for mTLS.
    pub client_auth: Option<(Vec<u8>, Vec<u8>)>,
}

/// TLS-Setup error.
#[derive(Debug)]
pub enum TlsError {
    /// rustls error.
    Rustls(rustls::Error),
    /// PEM-Parsing error.
    PemParse(String),
    /// IO/Handshake error.
    Io(io::Error),
    /// Hostname conversion failed.
    InvalidHostname(String),
    /// Configuration incomplete.
    BadConfig(&'static str),
}

impl core::fmt::Display for TlsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Rustls(e) => write!(f, "rustls: {e}"),
            Self::PemParse(s) => write!(f, "pem parse: {s}"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::InvalidHostname(s) => write!(f, "invalid hostname: {s}"),
            Self::BadConfig(s) => write!(f, "bad tls config: {s}"),
        }
    }
}

impl std::error::Error for TlsError {}

impl From<rustls::Error> for TlsError {
    fn from(e: rustls::Error) -> Self {
        Self::Rustls(e)
    }
}
impl From<io::Error> for TlsError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Server-side TLS stream.
pub type ServerTlsStream = StreamOwned<ServerConnection, TcpStream>;
/// Client-side TLS stream.
pub type ClientTlsStream = StreamOwned<ClientConnection, TcpStream>;

/// Spec §10.1 — `rustls::ServerConfig` from spec-compliant config
/// bauen.
///
/// Enables only TLS 1.2 + 1.3 (spec minimum standards). Default
/// crypto provider of rustls (ring).
///
/// # Errors
/// `TlsError::PemParse` on broken PEM, `TlsError::Rustls`
/// on cipher suite conflict.
pub fn build_server_config(cfg: &ServerTlsConfig) -> Result<Arc<ServerConfig>, TlsError> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let certs = parse_certificates(&cfg.cert_pem)?;
    if certs.is_empty() {
        return Err(TlsError::BadConfig(
            "server cert PEM contained no certificates",
        ));
    }
    let key = parse_private_key(&cfg.key_pem)?;

    let builder = ServerConfig::builder();
    let builder = if cfg.require_client_cert {
        let ca_pem = cfg.client_ca_pem.as_ref().ok_or(TlsError::BadConfig(
            "require_client_cert=true but client_ca_pem missing",
        ))?;
        let mut roots = RootCertStore::empty();
        for ca in parse_certificates(ca_pem)? {
            roots.add(ca)?;
        }
        let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
            .build()
            .map_err(|e| {
                TlsError::Rustls(rustls::Error::General(format!("client verifier: {e}")))
            })?;
        builder.with_client_cert_verifier(verifier)
    } else {
        builder.with_no_client_auth()
    };
    let server_config = builder
        .with_single_cert(certs, key)
        .map_err(TlsError::Rustls)?;
    Ok(Arc::new(server_config))
}

/// Spec §10.1 — build `rustls::ClientConfig`.
///
/// # Errors
/// `TlsError::PemParse` on broken PEM.
pub fn build_client_config(cfg: &ClientTlsConfig) -> Result<Arc<ClientConfig>, TlsError> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut roots = RootCertStore::empty();
    for ca in parse_certificates(&cfg.trust_anchors_pem)? {
        roots.add(ca)?;
    }
    if roots.is_empty() {
        return Err(TlsError::BadConfig(
            "trust_anchors_pem contained no certificates",
        ));
    }

    let builder = ClientConfig::builder().with_root_certificates(roots);
    let client_config = if let Some((cert_pem, key_pem)) = &cfg.client_auth {
        let certs = parse_certificates(cert_pem)?;
        let key = parse_private_key(key_pem)?;
        builder
            .with_client_auth_cert(certs, key)
            .map_err(TlsError::Rustls)?
    } else {
        builder.with_no_client_auth()
    };
    Ok(Arc::new(client_config))
}

/// Spec §10.1 — server-side TLS handshake on an accepted
/// `TcpStream`.
///
/// Returns a `StreamOwned` that is `Read + Write` and can be
/// passed to `handle_connection` directly after the handshake.
///
/// # Errors
/// `TlsError::Rustls` on handshake error.
pub fn accept_server(
    config: Arc<ServerConfig>,
    tcp: TcpStream,
) -> Result<ServerTlsStream, TlsError> {
    let conn = ServerConnection::new(config)?;
    let mut stream = StreamOwned::new(conn, tcp);
    // rustls handshakes lazily on the first read/write — we
    // force the handshake here.
    while stream.conn.is_handshaking() {
        match stream.conn.complete_io(&mut stream.sock) {
            Ok(_) => break,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(TlsError::Io(e)),
        }
    }
    Ok(stream)
}

/// Spec §10.1 — client-side TLS handshake.
///
/// `hostname` is the server hostname for the SNI/cert
/// Validation (e.g. `"broker.example"`).
///
/// # Errors
/// `TlsError::Rustls` / `InvalidHostname`.
pub fn connect_client(
    config: Arc<ClientConfig>,
    hostname: &str,
    tcp: TcpStream,
) -> Result<ClientTlsStream, TlsError> {
    let server_name = ServerName::try_from(hostname.to_string())
        .map_err(|_| TlsError::InvalidHostname(hostname.to_string()))?;
    let conn = ClientConnection::new(config, server_name)?;
    let mut stream = StreamOwned::new(conn, tcp);
    while stream.conn.is_handshaking() {
        match stream.conn.complete_io(&mut stream.sock) {
            Ok(_) => break,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(TlsError::Io(e)),
        }
    }
    Ok(stream)
}

// ============================================================
// PEM parsing helpers
// ============================================================

fn parse_certificates(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let mut reader = BufReader::new(pem);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::PemParse(format!("certs: {e}")))
}

fn parse_private_key(pem: &[u8]) -> Result<PrivateKeyDer<'static>, TlsError> {
    let mut reader = BufReader::new(pem);
    let key = rustls_pemfile::private_key(&mut reader)
        .map_err(|e| TlsError::PemParse(format!("private key: {e}")))?
        .ok_or(TlsError::BadConfig("no private key found in PEM"))?;
    Ok(key)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    /// Produces a self-signed cert + key for tests.
    fn self_signed() -> (Vec<u8>, Vec<u8>) {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
            .expect("rcgen self-signed");
        let cert_pem = cert.cert.pem().into_bytes();
        let key_pem = cert.key_pair.serialize_pem().into_bytes();
        (cert_pem, key_pem)
    }

    #[test]
    fn build_server_config_from_self_signed() {
        let (cert_pem, key_pem) = self_signed();
        let cfg = ServerTlsConfig {
            cert_pem,
            key_pem,
            client_ca_pem: None,
            require_client_cert: false,
        };
        let r = build_server_config(&cfg);
        assert!(r.is_ok(), "build_server_config: {r:?}");
    }

    #[test]
    fn build_server_config_rejects_empty_pem() {
        let cfg = ServerTlsConfig {
            cert_pem: b"not a cert".to_vec(),
            key_pem: b"not a key".to_vec(),
            client_ca_pem: None,
            require_client_cert: false,
        };
        assert!(build_server_config(&cfg).is_err());
    }

    #[test]
    fn build_server_config_require_client_cert_needs_ca() {
        let (cert_pem, key_pem) = self_signed();
        let cfg = ServerTlsConfig {
            cert_pem,
            key_pem,
            client_ca_pem: None,
            require_client_cert: true,
        };
        let err = build_server_config(&cfg).unwrap_err();
        assert!(matches!(err, TlsError::BadConfig(_)));
    }

    #[test]
    fn build_client_config_from_root_ca() {
        let (cert_pem, _key_pem) = self_signed();
        // We use the self-signed cert as the root CA for the
        // Client trust store (common test pattern).
        let cfg = ClientTlsConfig {
            trust_anchors_pem: cert_pem,
            client_auth: None,
        };
        let r = build_client_config(&cfg);
        assert!(r.is_ok(), "{r:?}");
    }

    #[test]
    fn build_client_config_rejects_empty_trust_anchors() {
        let cfg = ClientTlsConfig {
            trust_anchors_pem: b"# empty\n".to_vec(),
            client_auth: None,
        };
        let err = build_client_config(&cfg).unwrap_err();
        assert!(matches!(err, TlsError::BadConfig(_)));
    }

    /// E2E: server + client perform a real TLS handshake against
    /// self-signed Cert.
    #[test]
    fn tls_handshake_round_trip_with_self_signed_cert() {
        let (cert_pem, key_pem) = self_signed();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_cert_pem = cert_pem.clone();
        let server_key_pem = key_pem.clone();
        let server = thread::spawn(move || -> Result<Vec<u8>, TlsError> {
            let (sock, _) = listener.accept()?;
            sock.set_read_timeout(Some(Duration::from_secs(5)))?;
            sock.set_write_timeout(Some(Duration::from_secs(5)))?;
            let cfg = ServerTlsConfig {
                cert_pem: server_cert_pem,
                key_pem: server_key_pem,
                client_ca_pem: None,
                require_client_cert: false,
            };
            let server_config = build_server_config(&cfg)?;
            let mut tls = accept_server(server_config, sock)?;
            // The server reads 5 bytes + writes "pong".
            let mut buf = [0u8; 5];
            tls.read_exact(&mut buf)?;
            tls.write_all(b"pong")?;
            tls.flush()?;
            Ok(buf.to_vec())
        });

        thread::sleep(Duration::from_millis(50));

        let tcp = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        tcp.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
        // Client trusts the self-signed server cert.
        let client_cfg = ClientTlsConfig {
            trust_anchors_pem: cert_pem,
            client_auth: None,
        };
        let client_config = build_client_config(&client_cfg).unwrap();
        let mut tls = connect_client(client_config, "localhost", tcp).unwrap();
        tls.write_all(b"hello").unwrap();
        tls.flush().unwrap();
        let mut buf = [0u8; 4];
        tls.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"pong");

        let server_received = server.join().unwrap().unwrap();
        assert_eq!(&server_received, b"hello");
    }
}
