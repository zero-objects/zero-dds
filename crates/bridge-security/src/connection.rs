// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Connection-Wireup-Helpers für die sechs Bridge-Daemons.
//!
//! Diese Schicht sitzt zwischen `TcpStream::accept()` und dem
//! Protokoll-Handshake (HTTP-Upgrade / MQTT-CONNECT / GIOP-Locate /
//! HTTP/2-Preface / AMQP-Open / CoAP-Datagram) und liefert:
//!
//! * [`serve_tls_handshake`] — wraps eine `TcpStream` mit
//!   `rustls::ServerConnection` und blockt bis der TLS-Handshake
//!   abgeschlossen ist. Liefert ein TLS-`Stream`-Objekt + extracted
//!   `AuthSubject` (für mTLS).
//! * [`RotatingTlsConfig`] — `Arc<RwLock<Arc<ServerConfig>>>`-Wrapper
//!   für SIGHUP-Cert-Hot-Reload. Bestandene Connections behalten ihre
//!   Session, neue Connections sehen den neuen Cert.
//! * [`build_client_tls_connector`] — Pendant für MQTT/AMQP-Client-
//!   Daemons die zu einem Broker connecten und TLS auf der Out-Bound-
//!   Seite brauchen.
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

/// Hot-Reload-fähiger Wrapper um `Arc<ServerConfig>`. Zwischen Reads
/// (neue Connections lesen frisch) und Writes (SIGHUP-Reload tauscht
/// das Inner-Arc aus) ist ein `RwLock` die natürliche Form.
///
/// Connections, die zum Reload-Zeitpunkt schon einen `Arc<ServerConfig>`
/// halten, behalten ihren Cert für die Session. Neue Connections nach
/// dem Reload sehen den neuen Cert — das ist die Spec-konforme Semantik
/// (SIGHUP rotiert nur zukünftige Sessions, kein Forced-Renegotiation).
#[derive(Debug, Clone)]
pub struct RotatingTlsConfig {
    inner: Arc<RwLock<Arc<ServerConfig>>>,
    cert_path: PathBuf,
    key_path: PathBuf,
    client_ca_path: Option<PathBuf>,
}

impl RotatingTlsConfig {
    /// Lädt initial Cert+Key (optional Client-CA für mTLS) und gibt
    /// einen rotierbaren Wrapper zurück.
    ///
    /// # Errors
    /// [`TlsConfigError`] bei Lade-/Build-Fehler.
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

    /// Holt den aktuellen `Arc<ServerConfig>` für eine neue Connection.
    /// Block-frei beim Read-Lock-Fast-Path; bei Reload-Konflikt
    /// blockiert `read()` kurz auf den `RwLock`.
    #[must_use]
    pub fn current(&self) -> Arc<ServerConfig> {
        match self.inner.read() {
            Ok(g) => Arc::clone(&g),
            Err(poisoned) => Arc::clone(&poisoned.into_inner()),
        }
    }

    /// SIGHUP-Hook: Lädt Cert+Key neu von Disk und tauscht den
    /// gespeicherten `Arc` atomisch aus.
    ///
    /// # Errors
    /// [`TlsConfigError`] bei Lade-/Build-Fehler. Beim Fehler bleibt
    /// der bestehende Cert aktiv (no-op).
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

/// Treibt einen TLS-Handshake auf einer bereits-akzeptierten
/// `TcpStream` und liefert die `ServerConnection` + extracted
/// mTLS-Subject (oder `None`).
///
/// Der Caller wraps anschließend `(stream, conn)` in ein
/// `rustls::Stream` für seinen Protokoll-Pfad. Der Read-Timeout
/// limitiert wie lange auf einen Handshake gewartet wird (Spec §7.1
/// macht keine Vorgabe — wir nehmen 5 s als sane default).
///
/// # Errors
/// `std::io::Error` bei Socket- oder Handshake-Fehler.
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

    // Treibt den Handshake bis zum Ende (oder Fehler).
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

/// Adapter: `&mut TcpStream` → `Read`-Sink für rustls.
struct TcpReader<'a>(&'a mut TcpStream);
impl Read for TcpReader<'_> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(b)
    }
}

/// Adapter: `&mut TcpStream` → `Write`-Sink für rustls.
struct TcpWriter<'a>(&'a mut TcpStream);
impl Write for TcpWriter<'_> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.write(b)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Baut eine rustls-`ClientConfig` für Bridge-Clients (mqtt-bridge,
/// amqp-endpoint), die Out-Bound zu einem Broker connecten. Der
/// Aufrufer übergibt den optionalen Client-Cert (mTLS) und einen
/// CA-Bundle für Server-Cert-Validation.
///
/// `ca_pem_path = None` ⇒ benutzt OS-Native-Roots (über `webpki-roots`
/// fallback ist hier nicht aktiviert; im Workspace gibt es kein
/// `rustls-native-certs` als Dep). Wenn `None` und der Server-Cert
/// nicht gegen einen mitgegebenen CA validiert werden kann, scheitert
/// der Handshake — das ist der secure-by-default-Pfad.
///
/// # Errors
/// [`TlsConfigError`] bei Lade-/Build-Fehler.
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

/// Validiert einen Hostname als rustls-`ServerName` (für die
/// Client-Connector-Seite).
///
/// # Errors
/// [`TlsConfigError`] wenn der Name kein gültiges DNS-Format hat.
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
        // Beide Reads liefern dieselbe Arc-Identität bis Reload.
        assert!(Arc::ptr_eq(&cur1, &cur2));
    }

    #[test]
    fn rotating_config_reload_swaps_inner_arc() {
        let (cert1, key1) = gen_self_signed();
        let c = write_temp("rcert2.pem", cert1.as_bytes());
        let k = write_temp("rkey2.pem", key1.as_bytes());
        let r = RotatingTlsConfig::load(c.clone(), k.clone(), None).expect("load");
        let before = r.current();
        // Schreib einen neuen Cert in dieselbe Datei.
        let (cert2, key2) = gen_self_signed();
        std::fs::write(&c, cert2.as_bytes()).unwrap();
        std::fs::write(&k, key2.as_bytes()).unwrap();
        r.reload().expect("reload");
        let after = r.current();
        // Reload muss einen neuen Arc liefern (Cert-Daten haben sich geändert).
        assert!(!Arc::ptr_eq(&before, &after));
    }

    #[test]
    fn rotating_config_reload_with_bad_path_keeps_old() {
        let (cert, key) = gen_self_signed();
        let c = write_temp("rcert3.pem", cert.as_bytes());
        let k = write_temp("rkey3.pem", key.as_bytes());
        let r = RotatingTlsConfig::load(c.clone(), k.clone(), None).expect("load");
        let before = r.current();
        // Korrumpiere die Cert-Datei.
        std::fs::write(&c, b"-----BEGIN GARBAGE-----\n-----END GARBAGE-----\n").unwrap();
        let err = r.reload().unwrap_err();
        assert!(matches!(err, TlsConfigError::NoCertificateInPem));
        // Alter Cert ist immer noch aktiv.
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
