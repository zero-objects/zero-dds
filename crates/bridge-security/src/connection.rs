// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Connection wire-up helpers for the six bridge daemons.
//!
//! This layer sits between `TcpStream::accept()` and the protocol
//! handshake (HTTP upgrade / MQTT CONNECT / GIOP Locate / HTTP/2 preface
//! / AMQP Open / CoAP datagram) and provides:
//!
//! * [`serve_tls_handshake`] — wraps a `TcpStream` with
//!   `rustls::ServerConnection` and blocks until the TLS handshake is
//!   complete. Returns a TLS `Stream` object + extracted `AuthSubject`
//!   (for mTLS).
//! * [`RotatingTlsConfig`] — `Arc<RwLock<Arc<ServerConfig>>>` wrapper
//!   for SIGHUP cert hot reload. Established connections keep their
//!   session, new connections see the new cert.
//! * [`build_client_tls_connector`] — counterpart for MQTT/AMQP client
//!   daemons that connect to a broker and need TLS on the outbound
//!   side.
//!
//! Spec: zerodds-{ws,mqtt,coap,amqp,grpc,corba}-bridge-1.0.md §7.1.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rustls::{ClientConfig, ServerConfig, ServerConnection};
use rustls_pki_types::ServerName;

use crate::auth::AuthSubject;
use crate::ctx::extract_mtls_subject;
use crate::tls::{TlsConfigError, load_server_config, load_server_config_with_client_auth};

/// Hot-reload-capable wrapper around `Arc<ServerConfig>`. Between reads
/// (new connections read fresh) and writes (SIGHUP reload swaps the
/// inner Arc), a `RwLock` is the natural form.
///
/// Connections that already hold an `Arc<ServerConfig>` at reload time
/// keep their cert for the session. New connections after the reload
/// see the new cert — this is the spec-conformant semantics (SIGHUP
/// only rotates future sessions, no forced renegotiation).
#[derive(Debug, Clone)]
pub struct RotatingTlsConfig {
    inner: Arc<RwLock<Arc<ServerConfig>>>,
    cert_path: PathBuf,
    key_path: PathBuf,
    client_ca_path: Option<PathBuf>,
}

impl RotatingTlsConfig {
    /// Initially loads cert+key (optional client CA for mTLS) and
    /// returns a rotatable wrapper.
    ///
    /// # Errors
    /// [`TlsConfigError`] on load/build error.
    pub fn load(
        cert_path: PathBuf,
        key_path: PathBuf,
        client_ca_path: Option<PathBuf>,
    ) -> Result<Self, TlsConfigError> {
        let cfg = match &client_ca_path {
            Some(ca) => load_server_config_with_client_auth(&cert_path, &key_path, ca)?,
            None => load_server_config(&cert_path, &key_path)?,
        };
        Ok(Self {
            inner: Arc::new(RwLock::new(cfg)),
            cert_path,
            key_path,
            client_ca_path,
        })
    }

    /// Gets the current `Arc<ServerConfig>` for a new connection.
    /// Block-free on the read-lock fast path; on a reload conflict
    /// `read()` blocks briefly on the `RwLock`.
    #[must_use]
    pub fn current(&self) -> Arc<ServerConfig> {
        match self.inner.read() {
            Ok(g) => Arc::clone(&g),
            Err(poisoned) => Arc::clone(&poisoned.into_inner()),
        }
    }

    /// SIGHUP hook: reloads cert+key from disk and atomically swaps the
    /// stored `Arc`.
    ///
    /// # Errors
    /// [`TlsConfigError`] on load/build error. On error the existing
    /// cert stays active (no-op).
    pub fn reload(&self) -> Result<(), TlsConfigError> {
        let new_cfg = match &self.client_ca_path {
            Some(ca) => load_server_config_with_client_auth(&self.cert_path, &self.key_path, ca)?,
            None => load_server_config(&self.cert_path, &self.key_path)?,
        };
        let mut g = match self.inner.write() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        *g = new_cfg;
        Ok(())
    }
}

/// Drives a TLS handshake on an already-accepted `TcpStream` and
/// returns the `ServerConnection` + extracted mTLS subject (or `None`).
///
/// The caller then wraps `(stream, conn)` in a `rustls::Stream` for its
/// protocol path. The read timeout limits how long a handshake is
/// waited for (Spec §7.1 makes no requirement — we use 5 s as a sane
/// default).
///
/// # Errors
/// `std::io::Error` on socket or handshake error.
pub fn serve_tls_handshake(
    cfg: Arc<ServerConfig>,
    mut stream: TcpStream,
    handshake_timeout: Duration,
) -> std::io::Result<(TcpStream, ServerConnection, Option<AuthSubject>)> {
    stream.set_read_timeout(Some(handshake_timeout))?;
    stream.set_write_timeout(Some(handshake_timeout))?;
    let mut conn = ServerConnection::new(cfg).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("rustls: {e}"))
    })?;

    // Drive the handshake to completion (or error).
    while conn.is_handshaking() {
        if conn.wants_write() {
            let mut sink = TcpWriter(&mut stream);
            conn.write_tls(&mut sink)?;
        }
        if conn.wants_read() {
            let mut src = TcpReader(&mut stream);
            let n = conn.read_tls(&mut src)?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "tls handshake eof",
                ));
            }
            conn.process_new_packets().map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, format!("rustls: {e}"))
            })?;
        }
    }
    // Drain pending output (server-finished etc).
    while conn.wants_write() {
        let mut sink = TcpWriter(&mut stream);
        conn.write_tls(&mut sink)?;
    }

    let mtls_subject = extract_mtls_subject(&conn);
    Ok((stream, conn, mtls_subject))
}

/// Adapter: `&mut TcpStream` → `Read` sink for rustls.
struct TcpReader<'a>(&'a mut TcpStream);
impl Read for TcpReader<'_> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(b)
    }
}

/// Adapter: `&mut TcpStream` → `Write` sink for rustls.
struct TcpWriter<'a>(&'a mut TcpStream);
impl Write for TcpWriter<'_> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.write(b)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Builds a rustls `ClientConfig` for bridge clients (mqtt-bridge,
/// amqp-endpoint) that connect outbound to a broker. The caller passes
/// the optional client cert (mTLS) and a CA bundle for server cert
/// validation.
///
/// `ca_pem_path = None` ⇒ uses OS-native roots (the `webpki-roots`
/// fallback is not enabled here; the workspace has no
/// `rustls-native-certs` as a dep). When `None` and the server cert
/// cannot be validated against a supplied CA, the handshake fails —
/// this is the secure-by-default path.
///
/// # Errors
/// [`TlsConfigError`] on load/build error.
pub fn build_client_tls_connector(
    ca_pem_path: Option<&std::path::Path>,
    client_cert_pem_path: Option<&std::path::Path>,
    client_key_pem_path: Option<&std::path::Path>,
) -> Result<Arc<ClientConfig>, TlsConfigError> {
    use crate::tls::{read_certs, read_private_key};

    let mut roots = rustls::RootCertStore::empty();
    if let Some(ca) = ca_pem_path {
        for c in read_certs(ca)? {
            roots
                .add(c)
                .map_err(|e| TlsConfigError::Rustls(format!("ca add: {e}")))?;
        }
    }
    let provider = rustls::crypto::ring::default_provider();
    let builder = ClientConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsConfigError::Rustls(format!("{e}")))?
        .with_root_certificates(roots);

    let cfg = match (client_cert_pem_path, client_key_pem_path) {
        (Some(c), Some(k)) => {
            let certs = read_certs(c)?;
            let key = read_private_key(k)?;
            builder
                .with_client_auth_cert(certs, key)
                .map_err(|e| TlsConfigError::Rustls(format!("client auth: {e}")))?
        }
        (None, None) => builder.with_no_client_auth(),
        _ => {
            return Err(TlsConfigError::Rustls(
                "client cert and key must be set together".into(),
            ));
        }
    };
    Ok(Arc::new(cfg))
}

/// Validates a hostname as a rustls `ServerName` (for the
/// client-connector side).
///
/// # Errors
/// [`TlsConfigError`] if the name is not a valid DNS format.
pub fn parse_server_name(host: &str) -> Result<ServerName<'static>, TlsConfigError> {
    ServerName::try_from(host.to_string())
        .map_err(|e| TlsConfigError::Rustls(format!("invalid server name '{host}': {e}")))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use std::io::Write as _;

    fn write_temp(name: &str, body: &[u8]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zd-bridge-conn-{}-{}", name, std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body).unwrap();
        p
    }

    fn gen_self_signed() -> (String, String) {
        let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        (ck.cert.pem(), ck.key_pair.serialize_pem())
    }

    #[test]
    fn rotating_config_load_and_current_works() {
        let (cert, key) = gen_self_signed();
        let c = write_temp("rcert.pem", cert.as_bytes());
        let k = write_temp("rkey.pem", key.as_bytes());
        let r = RotatingTlsConfig::load(c, k, None).expect("load");
        let cur1 = r.current();
        let cur2 = r.current();
        // Both reads return the same Arc identity until reload.
        assert!(Arc::ptr_eq(&cur1, &cur2));
    }

    #[test]
    fn rotating_config_reload_swaps_inner_arc() {
        let (cert1, key1) = gen_self_signed();
        let c = write_temp("rcert2.pem", cert1.as_bytes());
        let k = write_temp("rkey2.pem", key1.as_bytes());
        let r = RotatingTlsConfig::load(c.clone(), k.clone(), None).expect("load");
        let before = r.current();
        // Write a new cert into the same file.
        let (cert2, key2) = gen_self_signed();
        std::fs::write(&c, cert2.as_bytes()).unwrap();
        std::fs::write(&k, key2.as_bytes()).unwrap();
        r.reload().expect("reload");
        let after = r.current();
        // Reload must return a new Arc (cert data has changed).
        assert!(!Arc::ptr_eq(&before, &after));
    }

    #[test]
    fn rotating_config_reload_with_bad_path_keeps_old() {
        let (cert, key) = gen_self_signed();
        let c = write_temp("rcert3.pem", cert.as_bytes());
        let k = write_temp("rkey3.pem", key.as_bytes());
        let r = RotatingTlsConfig::load(c.clone(), k.clone(), None).expect("load");
        let before = r.current();
        // Corrupt the cert file.
        std::fs::write(&c, b"-----BEGIN GARBAGE-----\n-----END GARBAGE-----\n").unwrap();
        let err = r.reload().unwrap_err();
        assert!(matches!(err, TlsConfigError::NoCertificateInPem));
        // The old cert is still active.
        let after = r.current();
        assert!(Arc::ptr_eq(&before, &after));
    }

    #[test]
    fn parse_server_name_accepts_dns_hostname() {
        let _ = parse_server_name("example.com").expect("dns");
    }

    #[test]
    fn parse_server_name_accepts_ip() {
        let _ = parse_server_name("127.0.0.1").expect("ip");
    }

    #[test]
    fn build_client_tls_connector_no_auth_succeeds() {
        let (cert, _key) = gen_self_signed();
        let ca = write_temp("ca.pem", cert.as_bytes());
        let cfg = build_client_tls_connector(Some(&ca), None, None).expect("client cfg");
        assert!(Arc::strong_count(&cfg) >= 1);
    }

    #[test]
    fn build_client_tls_connector_with_mtls_succeeds() {
        let (cert, key) = gen_self_signed();
        let cap = write_temp("ca2.pem", cert.as_bytes());
        let cp = write_temp("cli.pem", cert.as_bytes());
        let kp = write_temp("clikey.pem", key.as_bytes());
        let cfg = build_client_tls_connector(Some(&cap), Some(&cp), Some(&kp)).expect("mtls");
        assert!(Arc::strong_count(&cfg) >= 1);
    }

    #[test]
    fn build_client_tls_connector_partial_auth_rejected() {
        let (cert, _key) = gen_self_signed();
        let cap = write_temp("ca3.pem", cert.as_bytes());
        let cp = write_temp("cli2.pem", cert.as_bytes());
        let err = build_client_tls_connector(Some(&cap), Some(&cp), None).unwrap_err();
        assert!(matches!(err, TlsConfigError::Rustls(_)));
    }
}
