// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §7.1 TLS — `rustls 0.23` ServerConfig-Builder.
//!
//! Eingangspunkt: [`load_server_config`] nimmt PEM-Cert-Pfad +
//! PEM-Key-Pfad und liefert ein `Arc<rustls::ServerConfig>`. Das wird
//! pro Daemon entweder beim Start oder im SIGHUP-Reload-Pfad
//! aufgerufen.
//!
//! Cipher-Suites: rustls-Default (TLS 1.3 + TLS 1.2 mit AEAD-Suiten).
//! Spec §7.1: TLS 1.2 minimum, TLS 1.3 bevorzugt — passt.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use rustls::ServerConfig;
use rustls_pemfile::Item;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// Fehler beim Aufbau einer `ServerConfig`.
#[derive(Debug)]
pub enum TlsConfigError {
    /// Cert-File-Read schlug fehl.
    CertFileRead(String),
    /// Key-File-Read schlug fehl.
    KeyFileRead(String),
    /// Cert-PEM enthielt keine Cert-Sektion.
    NoCertificateInPem,
    /// Key-PEM enthielt keinen unterstützten Private-Key.
    NoSupportedPrivateKeyInPem,
    /// rustls-internal: ServerConfig-Build fehlgeschlagen
    /// (z.B. Cert/Key-Mismatch).
    Rustls(String),
}

impl core::fmt::Display for TlsConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CertFileRead(m) => write!(f, "cert file read: {m}"),
            Self::KeyFileRead(m) => write!(f, "key file read: {m}"),
            Self::NoCertificateInPem => f.write_str("PEM had no CERTIFICATE block"),
            Self::NoSupportedPrivateKeyInPem => {
                f.write_str("PEM had no PKCS#8 / RSA / EC private key")
            }
            Self::Rustls(m) => write!(f, "rustls build: {m}"),
        }
    }
}

impl std::error::Error for TlsConfigError {}

/// Lädt ein PEM-Cert + PEM-Key und baut eine `ServerConfig` ohne
/// Client-Auth (no-mTLS-Default; mTLS wird über
/// [`load_server_config_with_client_auth`] aktiviert).
///
/// # Errors
/// [`TlsConfigError`] bei IO-, PEM-Parse- oder Build-Fehler.
pub fn load_server_config(
    cert_pem_path: &Path,
    key_pem_path: &Path,
) -> Result<Arc<ServerConfig>, TlsConfigError> {
    let certs = read_certs(cert_pem_path)?;
    let key = read_private_key(key_pem_path)?;
    let provider = rustls::crypto::ring::default_provider();
    let cfg = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsConfigError::Rustls(format!("{e}")))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| TlsConfigError::Rustls(format!("{e}")))?;
    Ok(Arc::new(cfg))
}

/// Wie [`load_server_config`], aber mit Client-Cert-Auth (mTLS).
///
/// `client_ca_pem_path` ist eine PEM-Datei mit ein oder mehr Root-Certs,
/// die als Trust-Anchor für Client-Cert-Validation dienen. Spec §7.2:
/// im `mtls`-Auth-Mode darf nur ein TLS-Handshake erfolgreich enden,
/// dessen Client-Cert in dieser CA-Chain validiert.
///
/// # Errors
/// [`TlsConfigError`] bei IO-, PEM- oder Build-Fehler.
pub fn load_server_config_with_client_auth(
    cert_pem_path: &Path,
    key_pem_path: &Path,
    client_ca_pem_path: &Path,
) -> Result<Arc<ServerConfig>, TlsConfigError> {
    let certs = read_certs(cert_pem_path)?;
    let key = read_private_key(key_pem_path)?;
    let client_cas = read_certs(client_ca_pem_path)?;

    let mut roots = rustls::RootCertStore::empty();
    for c in client_cas {
        roots
            .add(c)
            .map_err(|e| TlsConfigError::Rustls(format!("client CA add: {e}")))?;
    }
    let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|e| TlsConfigError::Rustls(format!("client verifier: {e}")))?;

    let provider = rustls::crypto::ring::default_provider();
    let cfg = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsConfigError::Rustls(format!("{e}")))?
        .with_client_cert_verifier(verifier)
        .with_single_cert(certs, key)
        .map_err(|e| TlsConfigError::Rustls(format!("{e}")))?;
    Ok(Arc::new(cfg))
}

pub(crate) fn read_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, TlsConfigError> {
    let f = File::open(path)
        .map_err(|e| TlsConfigError::CertFileRead(format!("{}: {e}", path.display())))?;
    let mut br = BufReader::new(f);
    let mut out = Vec::new();
    for item in rustls_pemfile::read_all(&mut br) {
        let item = item.map_err(|e| TlsConfigError::CertFileRead(format!("{e}")))?;
        if let Item::X509Certificate(d) = item {
            out.push(d);
        }
    }
    if out.is_empty() {
        return Err(TlsConfigError::NoCertificateInPem);
    }
    Ok(out)
}

pub(crate) fn read_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, TlsConfigError> {
    let f = File::open(path)
        .map_err(|e| TlsConfigError::KeyFileRead(format!("{}: {e}", path.display())))?;
    let mut br = BufReader::new(f);
    for item in rustls_pemfile::read_all(&mut br) {
        let item = item.map_err(|e| TlsConfigError::KeyFileRead(format!("{e}")))?;
        match item {
            Item::Pkcs8Key(k) => return Ok(PrivateKeyDer::Pkcs8(k)),
            Item::Pkcs1Key(k) => return Ok(PrivateKeyDer::Pkcs1(k)),
            Item::Sec1Key(k) => return Ok(PrivateKeyDer::Sec1(k)),
            _ => {}
        }
    }
    let _ = PrivatePkcs8KeyDer::from(Vec::<u8>::new()); // touch type so rustdoc resolves the import even if unused
    Err(TlsConfigError::NoSupportedPrivateKeyInPem)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(name: &str, body: &[u8]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zd-bridge-sec-{}-{}", name, std::process::id()));
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
    fn load_self_signed_cert_succeeds() {
        let (cert_pem, key_pem) = gen_self_signed();
        let c = write_temp("cert.pem", cert_pem.as_bytes());
        let k = write_temp("key.pem", key_pem.as_bytes());
        let cfg = load_server_config(&c, &k).expect("ServerConfig");
        // Smoke: rustls validiert Cert/Key intern beim with_single_cert-Aufruf.
        assert!(Arc::strong_count(&cfg) >= 1);
    }

    #[test]
    fn missing_cert_file_returns_err() {
        let p = std::path::PathBuf::from("/no/such/file.pem");
        let err = load_server_config(&p, &p).unwrap_err();
        assert!(matches!(err, TlsConfigError::CertFileRead(_)));
    }

    #[test]
    fn empty_pem_rejected_as_no_cert() {
        let c = write_temp(
            "empty.pem",
            b"-----BEGIN GARBAGE-----\nXX\n-----END GARBAGE-----\n",
        );
        let k = c.clone();
        let err = load_server_config(&c, &k).unwrap_err();
        assert!(matches!(err, TlsConfigError::NoCertificateInPem));
    }

    #[test]
    fn key_pem_without_supported_block_rejected() {
        let (cert_pem, _) = gen_self_signed();
        let c = write_temp("c2.pem", cert_pem.as_bytes());
        let k = write_temp(
            "k2.pem",
            b"-----BEGIN GARBAGE-----\nXX\n-----END GARBAGE-----\n",
        );
        let err = load_server_config(&c, &k).unwrap_err();
        assert!(matches!(err, TlsConfigError::NoSupportedPrivateKeyInPem));
    }

    #[test]
    fn mtls_config_loads_with_client_ca() {
        let (cert_pem, key_pem) = gen_self_signed();
        let c = write_temp("c3.pem", cert_pem.as_bytes());
        let k = write_temp("k3.pem", key_pem.as_bytes());
        // Re-use server cert as the client-CA bundle for the test —
        // the API only checks DER-decode + chain-build, not trust path.
        let ca = write_temp("ca.pem", cert_pem.as_bytes());
        let cfg = load_server_config_with_client_auth(&c, &k, &ca).expect("mtls cfg");
        assert!(Arc::strong_count(&cfg) >= 1);
    }
}
