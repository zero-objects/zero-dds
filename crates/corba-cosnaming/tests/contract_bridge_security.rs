// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-cosnaming` with `zerodds-bridge-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-bridge-security`: `tls::load_server_config` and `load_server_config_with_client_auth` (SSLIOP of the naming daemon).
//!
//! Oracles: RFC 7468 §5 (a certificate is the PEM label `CERTIFICATE`), with a certificate and key made by `openssl req`.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use zerodds_bridge_security::{TlsConfigError, load_server_config};

/// `openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -subj
/// "/O=Example/CN=alice"` (OpenSSL 4.0.3).
const CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\nMIIBeTCCASCgAwIBAgIUNYvSwdb4nEI1QfcU6A3VHMe8RIQwCgYIKoZIzj0EAwIw\nIjEQMA4GA1UECgwHRXhhbXBsZTEOMAwGA1UEAwwFYWxpY2UwIBcNMjYxMTAxMjMx\nNzQxWhgPMjEyNjEwMDgyMzE3NDFaMCIxEDAOBgNVBAoMB0V4YW1wbGUxDjAMBgNV\nBAMMBWFsaWNlMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEXjZywaxMDiJUet1T\n1J4T3v2SVAyDDMEna5jzLGm96fNXCSH3lrCHtDTSKDrKZfDOOVktt9WV5VLQjMnw\nuTDqlKMyMDAwHQYDVR0OBBYEFHlA+hOEKTwWrLHBF/JFiCzLdVGcMA8GA1UdEwEB\n/wQFMAMBAf8wCgYIKoZIzj0EAwIDRwAwRAIgBJDU1K8HqMWikQpg39J+5+yeo1DN\n2+5ADkJJvNABPKgCIERq4rHsCOs0FM9GaH9NUc8tob9jmSbp1qce2uqsJWhv\n-----END CERTIFICATE-----\n";

/// The key of [`CERT_PEM`].
const KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg7FPbKMz31lhIvf+O\nbfBzKB4DqZAF63PYgc8VbqS1FamhRANCAAReNnLBrEwOIlR63VPUnhPe/ZJUDIMM\nwSdrmPMsab3p81cJIfeWsIe0NNIoOspl8M45WS231ZXlUtCMyfC5MOqU\n-----END PRIVATE KEY-----\n";

/// A directory of this test with the fixture certificate and key.
fn pem_files(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "zerodds-contract-bridge-security-{}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let (cert, key) = (dir.join("cert.pem"), dir.join("key.pem"));
    std::fs::write(&cert, CERT_PEM).unwrap();
    std::fs::write(&key, KEY_PEM).unwrap();
    (dir, cert, key)
}

#[test]
fn corbabridge_7_1_a_server_config_loads_from_a_certificate_and_its_key() {
    let (dir, cert, key) = pem_files("tlsload");
    let loaded = load_server_config(&cert, &key);
    assert!(loaded.is_ok(), "{:?}", loaded.err());
    // A file without a `CERTIFICATE` block is no certificate.
    let not_a_cert = dir.join("not-a-cert.pem");
    std::fs::write(&not_a_cert, KEY_PEM).unwrap();
    assert!(matches!(
        load_server_config(&not_a_cert, &key),
        Err(TlsConfigError::NoCertificateInPem)
    ));
    assert!(matches!(
        load_server_config(&dir.join("missing.pem"), &key),
        Err(TlsConfigError::CertFileRead(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
