// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §7.1 TLS — `rustls 0.23` ServerConfig builder.
//!
//! Entry point: [`load_server_config`] takes a PEM cert path + PEM key
//! path and returns an `Arc<rustls::ServerConfig>`. This is called per
//! daemon either at startup or in the SIGHUP reload path.
//!
//! Cipher suites: rustls default (TLS 1.3 + TLS 1.2 with AEAD suites).
//! Spec §7.1: TLS 1.2 minimum, TLS 1.3 preferred — matches.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use rustls::ServerConfig;
use rustls_pemfile::Item;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// Error while building a `ServerConfig`.
#[derive(Debug)]
pub enum TlsConfigError {
    /// Cert file read failed.
    CertFileRead(String),
    /// Key file read failed.
    KeyFileRead(String),
    /// Cert PEM contained no certificate section.
    NoCertificateInPem,
    /// Key PEM contained no supported private key.
    NoSupportedPrivateKeyInPem,
    /// rustls-internal: ServerConfig build failed
    /// (e.g. cert/key mismatch).
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

/// Loads a PEM cert + PEM key and builds a `ServerConfig` without
/// client auth (no-mTLS default; mTLS is enabled via
/// [`load_server_config_with_client_auth`]).
///
/// # Errors
/// [`TlsConfigError`] on IO, PEM-parse, or build error.
pub fn load_server_config(
    cert_pem_path: &Path,
    key_pem_path: &Path,
) -> Result<Arc<ServerConfig>, TlsConfigError> {
    let certs = read_certs(cert_pem_path)?;
    let key = read_private_key(key_pem_path)?;
    let provider = crate::tls_provider();
    let cfg = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsConfigError::Rustls(format!("{e}")))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| TlsConfigError::Rustls(format!("{e}")))?;
    Ok(Arc::new(cfg))
}

/// Like [`load_server_config`], but with client cert auth (mTLS).
///
/// `client_ca_pem_path` is a PEM file with one or more root certs that
/// serve as the trust anchor for client cert validation. Spec §7.2: in
/// `mtls` auth mode, only a TLS handshake whose client cert validates
/// against this CA chain may complete successfully.
///
/// # Errors
/// [`TlsConfigError`] on IO, PEM, or build error.
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
    // The client-cert verifier MUST be built with the explicit provider too —
    // its default builder pulls the *process-level* CryptoProvider, which is
    // ambiguous (panics) when rustls carries both ring and aws-lc-rs.
    let provider = Arc::new(crate::tls_provider());
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots),
        provider.clone(),
    )
    .build()
    .map_err(|e| TlsConfigError::Rustls(format!("client verifier: {e}")))?;

    let cfg = ServerConfig::builder_with_provider(provider)
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
        // Smoke: rustls validates cert/key internally in the with_single_cert call.
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
