// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What a DTLS endpoint proves and whom it trusts, and the OpenSSL context
//! built from it: DTLS 1.2 only (RFC 6347), pre-shared keys (RFC 4279),
//! X.509 certificates checked against trust anchors and the peer's name,
//! raw public keys (RFC 7250) checked against RFC 6920 names, the cookie
//! exchange of RFC 6347 §4.2.1 on the server.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use openssl::error::ErrorStack;
use openssl::hash::{MessageDigest, hash};
use openssl::ssl::{
    Ssl, SslContext, SslContextBuilder, SslFiletype, SslMethod, SslOptions, SslVerifyMode,
    SslVersion,
};
use openssl::x509::X509StoreContextRef;

use crate::ffi;
use crate::ni::KeyName;

/// A certificate chain and its private key, PEM files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateFiles {
    /// The chain, leaf first.
    pub cert_file: PathBuf,
    /// The private key (PKCS#8, SEC1 or PKCS#1).
    pub key_file: PathBuf,
}

/// The server side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerConfig {
    /// The pre-shared keys the server accepts, identity and secret.
    pub psk: Vec<(String, Vec<u8>)>,
    /// The identity hint the server sends (RFC 4279 §2); advisory.
    pub psk_identity_hint: Option<String>,
    /// The server's certificate.
    pub certificate: Option<CertificateFiles>,
    /// The server's raw public key: the PEM file of its private key; the
    /// public key goes out in place of a certificate (RFC 7250).
    pub raw_public_key: Option<PathBuf>,
    /// Without a certificate, raw public key or pre-shared key: the names
    /// of a self-signed P-256 certificate generated when the context is
    /// built, so that `coaps://` serves on a first run.
    pub self_signed: Vec<String>,
    /// The trust anchors a client certificate is checked against; with it
    /// the server asks for one.
    pub client_ca_file: Option<PathBuf>,
    /// The raw public keys of the clients the server accepts (RFC 7252
    /// §9.1.3.2.1: the access control list of identifiers); with them the
    /// server asks for one.
    pub trusted_client_keys: Vec<KeyName>,
    /// The cipher suites in preference order, IANA names; empty: the
    /// defaults of [`default_suites`] for the credentials.
    pub cipher_suites: Vec<String>,
}

/// The client side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientConfig {
    /// The pre-shared key, identity and secret.
    pub psk: Option<(String, Vec<u8>)>,
    /// The client's certificate, for a server that asks for one.
    pub certificate: Option<CertificateFiles>,
    /// The client's raw public key, the PEM file of its private key.
    pub raw_public_key: Option<PathBuf>,
    /// The trust anchors the server's certificate is checked against.
    pub ca_file: Option<PathBuf>,
    /// The raw public keys of the servers the client accepts.
    pub trusted_server_keys: Vec<KeyName>,
    /// The name or address the server's certificate must carry; without
    /// it the certificate is checked against the trust anchors only.
    pub server_name: Option<String>,
    /// As [`ServerConfig::cipher_suites`].
    pub cipher_suites: Vec<String>,
}

/// Why a configuration does not make a context.
#[derive(Debug)]
pub enum ConfigError {
    /// A certificate and a raw public key on one side: one key pair each.
    TwoKeys,
    /// A client with a certificate or raw public key mode but nothing to
    /// check the server with, or a server without any credential.
    NoTrust(&'static str),
    /// A cipher suite this library does not know.
    UnknownSuite(String),
    /// OpenSSL refused a file or setting.
    Ssl(ErrorStack),
    /// The socket could not be bound.
    Io(std::io::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TwoKeys => write!(
                f,
                "a certificate and a raw public key: one key pair per side"
            ),
            Self::NoTrust(what) => write!(f, "{what}"),
            Self::UnknownSuite(s) => write!(f, "unknown cipher suite `{s}`"),
            Self::Ssl(e) => write!(f, "openssl: {e}"),
            Self::Io(e) => write!(f, "socket: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<ErrorStack> for ConfigError {
    fn from(e: ErrorStack) -> Self {
        Self::Ssl(e)
    }
}

/// The IANA names of the suites and OpenSSL's names for them.
const SUITES: [(&str, &str); 8] = [
    ("TLS_PSK_WITH_AES_128_CCM_8", "PSK-AES128-CCM8"),
    ("TLS_PSK_WITH_AES_128_CCM", "PSK-AES128-CCM"),
    ("TLS_PSK_WITH_AES_128_GCM_SHA256", "PSK-AES128-GCM-SHA256"),
    (
        "TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256",
        "ECDHE-PSK-AES128-CBC-SHA256",
    ),
    (
        "TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8",
        "ECDHE-ECDSA-AES128-CCM8",
    ),
    ("TLS_ECDHE_ECDSA_WITH_AES_128_CCM", "ECDHE-ECDSA-AES128-CCM"),
    (
        "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
        "ECDHE-ECDSA-AES128-GCM-SHA256",
    ),
    (
        "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
        "ECDHE-RSA-AES128-GCM-SHA256",
    ),
];

/// OpenSSL's name of the suite with the IANA name `iana`.
#[must_use]
pub fn openssl_name(iana: &str) -> Option<&'static str> {
    SUITES.iter().find(|(i, _)| *i == iana).map(|(_, o)| *o)
}

/// The default suites: for a pre-shared key TLS_PSK_WITH_AES_128_CCM_8
/// (RFC 7252 §9.1.3.1), for a certificate or raw public key
/// TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8 (§9.1.3.2, §9.1.3.3), each followed
/// by CCM and GCM, which a peer offers that leaves CCM-8 out (OpenSSL 3 by
/// default).
#[must_use]
pub fn default_suites(psk: bool, public_key: bool) -> Vec<String> {
    let mut out = Vec::new();
    if psk {
        out.extend(
            [
                "TLS_PSK_WITH_AES_128_CCM_8",
                "TLS_PSK_WITH_AES_128_CCM",
                "TLS_PSK_WITH_AES_128_GCM_SHA256",
            ]
            .map(String::from),
        );
    }
    if public_key {
        out.extend(
            [
                "TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8",
                "TLS_ECDHE_ECDSA_WITH_AES_128_CCM",
                "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
                "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
            ]
            .map(String::from),
        );
    }
    out
}

fn cipher_list(suites: &[String]) -> Result<String, ConfigError> {
    let names = suites
        .iter()
        .map(|s| openssl_name(s).ok_or_else(|| ConfigError::UnknownSuite(s.clone())))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names.join(":"))
}

fn base(method: SslMethod, suites: &[String]) -> Result<SslContextBuilder, ConfigError> {
    let mut b = SslContextBuilder::new(method)?;
    b.set_min_proto_version(Some(SslVersion::DTLS1_2))?;
    b.set_max_proto_version(Some(SslVersion::DTLS1_2))?;
    // The MTU is the one the session sets (RFC 7252 §4.6), not one OpenSSL
    // asks the I/O for.
    b.set_options(SslOptions::NO_QUERY_MTU);
    ffi::allow_ccm8(&mut b);
    b.set_cipher_list(&cipher_list(suites)?)?;
    // RFC 7252 §9.1.3.2: secp256r1 MUST be supported.
    b.set_groups_list("P-256:X25519:P-384")?;
    Ok(b)
}

/// The verify callback: a raw public key must be one of `trusted`, a
/// certificate must have passed OpenSSL's check against the trust
/// anchors, which only counts when there are any.
fn verifier(
    trusted: Vec<KeyName>,
    anchors: bool,
) -> impl Fn(bool, &mut X509StoreContextRef) -> bool + Send + Sync + 'static {
    move |preverified, ctx| match ffi::store_ctx_rpk_spki(ctx) {
        Some(spki) => trusted.iter().any(|name| name.names(&spki)),
        None => preverified && anchors,
    }
}

/// The index of the peer address in an `Ssl`, for the cookie.
pub(crate) fn peer_index() -> Result<openssl::ex_data::Index<Ssl, SocketAddr>, ErrorStack> {
    static INDEX: OnceLock<openssl::ex_data::Index<Ssl, SocketAddr>> = OnceLock::new();
    if let Some(i) = INDEX.get() {
        return Ok(*i);
    }
    let i = Ssl::new_ex_index()?;
    Ok(*INDEX.get_or_init(|| i))
}

/// RFC 6347 §4.2.1: a cookie bound to the client's address, a hash of a
/// secret of this server and the address.
fn cookie(secret: &[u8], peer: Option<&SocketAddr>) -> Vec<u8> {
    let mut input = secret.to_vec();
    if let Some(p) = peer {
        input.extend_from_slice(p.to_string().as_bytes());
    }
    hash(MessageDigest::sha256(), &input).map_or_else(|_| vec![0; 16], |d| d[..16].to_vec())
}

/// A self-signed P-256 certificate for `names`, valid for a year.
fn self_signed(
    names: &[String],
) -> Result<
    (
        openssl::x509::X509,
        openssl::pkey::PKey<openssl::pkey::Private>,
    ),
    ErrorStack,
> {
    use openssl::asn1::Asn1Time;
    use openssl::bn::{BigNum, MsbOption};
    use openssl::ec::{EcGroup, EcKey};
    use openssl::nid::Nid;
    use openssl::x509::extension::SubjectAlternativeName;
    use openssl::x509::{X509, X509NameBuilder};
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;
    let key = openssl::pkey::PKey::from_ec_key(EcKey::generate(&group)?)?;
    let mut name = X509NameBuilder::new()?;
    name.append_entry_by_text("CN", names.first().map_or("localhost", String::as_str))?;
    let name = name.build();
    let mut serial = BigNum::new()?;
    serial.rand(64, MsbOption::MAYBE_ZERO, false)?;
    let mut b = X509::builder()?;
    b.set_version(2)?;
    let serial = serial.to_asn1_integer()?;
    b.set_serial_number(&serial)?;
    b.set_subject_name(&name)?;
    b.set_issuer_name(&name)?;
    b.set_pubkey(&key)?;
    let not_before = Asn1Time::days_from_now(0)?;
    let not_after = Asn1Time::days_from_now(365)?;
    b.set_not_before(&not_before)?;
    b.set_not_after(&not_after)?;
    let mut san = SubjectAlternativeName::new();
    for n in names {
        if n.parse::<IpAddr>().is_ok() {
            san.ip(n);
        } else {
            san.dns(n);
        }
    }
    let san = san.build(&b.x509v3_context(None, None))?;
    b.append_extension(san)?;
    b.sign(&key, MessageDigest::sha256())?;
    Ok((b.build(), key))
}

impl ServerConfig {
    /// The OpenSSL context of the server.
    ///
    /// # Errors
    /// See [`ConfigError`].
    pub fn context(&self) -> Result<SslContext, ConfigError> {
        if self.certificate.is_some() && self.raw_public_key.is_some() {
            return Err(ConfigError::TwoKeys);
        }
        let generated = self.certificate.is_none()
            && self.raw_public_key.is_none()
            && !self.self_signed.is_empty();
        let public_key = self.certificate.is_some() || self.raw_public_key.is_some() || generated;
        if self.psk.is_empty() && !public_key {
            return Err(ConfigError::NoTrust(
                "a DTLS server needs a pre-shared key, a certificate or a raw public key",
            ));
        }
        let suites = if self.cipher_suites.is_empty() {
            default_suites(!self.psk.is_empty(), public_key)
        } else {
            self.cipher_suites.clone()
        };
        let mut b = base(SslMethod::dtls_server(), &suites)?;
        if !self.psk.is_empty() {
            let store = self.psk.clone();
            b.set_psk_server_callback(move |_ssl, identity, psk| {
                let secret = identity
                    .and_then(|id| store.iter().find(|(i, _)| i.as_bytes() == id))
                    .map(|(_, s)| s)
                    .ok_or_else(ErrorStack::get)?;
                let n = secret.len().min(psk.len());
                psk[..n].copy_from_slice(&secret[..n]);
                Ok(n)
            });
            if let Some(hint) = &self.psk_identity_hint {
                ffi::set_psk_identity_hint(&mut b, hint)?;
            }
        }
        if let Some(c) = &self.certificate {
            b.set_certificate_chain_file(&c.cert_file)?;
            b.set_private_key_file(&c.key_file, SslFiletype::PEM)?;
            b.check_private_key()?;
        }
        if generated {
            let (cert, key) = self_signed(&self.self_signed)?;
            b.set_certificate(&cert)?;
            b.set_private_key(&key)?;
        }
        if let Some(key) = &self.raw_public_key {
            // Without a certificate OpenSSL takes the public key from the
            // private key (SSL_get0_peer_rpk(3)).
            b.set_private_key_file(key, SslFiletype::PEM)?;
            ffi::set_cert_types(&mut b, None, Some(&[ffi::CERT_TYPE_RPK]))?;
        }
        let anchors = self.client_ca_file.is_some();
        if anchors || !self.trusted_client_keys.is_empty() {
            if let Some(ca) = &self.client_ca_file {
                b.set_ca_file(ca)?;
            }
            let mut types = Vec::new();
            if !self.trusted_client_keys.is_empty() {
                types.push(ffi::CERT_TYPE_RPK);
            }
            if anchors {
                types.push(ffi::CERT_TYPE_X509);
            }
            ffi::set_cert_types(&mut b, Some(&types), None)?;
            b.set_verify_callback(
                SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT,
                verifier(self.trusted_client_keys.clone(), anchors),
            );
        }
        // RFC 6347 §4.2.1: the HelloVerifyRequest cookie against spoofed
        // sources.
        let mut secret = [0u8; 32];
        openssl::rand::rand_bytes(&mut secret)?;
        let secret = Arc::new(secret);
        let index = peer_index()?;
        b.set_options(SslOptions::COOKIE_EXCHANGE);
        let gen_secret = Arc::clone(&secret);
        b.set_cookie_generate_cb(move |ssl, out| {
            let c = cookie(&gen_secret[..], ssl.ex_data(index));
            let n = c.len().min(out.len());
            out[..n].copy_from_slice(&c[..n]);
            Ok(n)
        });
        b.set_cookie_verify_cb(move |ssl, given| cookie(&secret[..], ssl.ex_data(index)) == given);
        Ok(b.build())
    }
}

impl ClientConfig {
    /// The OpenSSL context of the client.
    ///
    /// # Errors
    /// See [`ConfigError`].
    pub fn context(&self) -> Result<SslContext, ConfigError> {
        if self.certificate.is_some() && self.raw_public_key.is_some() {
            return Err(ConfigError::TwoKeys);
        }
        let checks = self.ca_file.is_some() || !self.trusted_server_keys.is_empty();
        if self.psk.is_none() && !checks {
            return Err(ConfigError::NoTrust(
                "a DTLS client needs a pre-shared key, trust anchors or trusted raw public keys",
            ));
        }
        let suites = if self.cipher_suites.is_empty() {
            default_suites(self.psk.is_some(), checks)
        } else {
            self.cipher_suites.clone()
        };
        let mut b = base(SslMethod::dtls_client(), &suites)?;
        if let Some((identity, secret)) = self.psk.clone() {
            b.set_psk_client_callback(move |_ssl, _hint, id_out, psk| {
                let id = identity.as_bytes();
                if id.len() + 1 > id_out.len() || secret.len() > psk.len() {
                    return Err(ErrorStack::get());
                }
                id_out[..id.len()].copy_from_slice(id);
                id_out[id.len()] = 0;
                psk[..secret.len()].copy_from_slice(&secret);
                Ok(secret.len())
            });
        }
        if let Some(c) = &self.certificate {
            b.set_certificate_chain_file(&c.cert_file)?;
            b.set_private_key_file(&c.key_file, SslFiletype::PEM)?;
            b.check_private_key()?;
        }
        if let Some(key) = &self.raw_public_key {
            b.set_private_key_file(key, SslFiletype::PEM)?;
            ffi::set_cert_types(&mut b, Some(&[ffi::CERT_TYPE_RPK]), None)?;
        }
        if checks {
            if let Some(ca) = &self.ca_file {
                b.set_ca_file(ca)?;
            }
            let mut types = Vec::new();
            if !self.trusted_server_keys.is_empty() {
                types.push(ffi::CERT_TYPE_RPK);
            }
            if self.ca_file.is_some() {
                types.push(ffi::CERT_TYPE_X509);
            }
            ffi::set_cert_types(&mut b, None, Some(&types))?;
            b.set_verify_callback(
                SslVerifyMode::PEER,
                verifier(self.trusted_server_keys.clone(), self.ca_file.is_some()),
            );
        }
        Ok(b.build())
    }

    /// The `Ssl` of one connection: the server's name checked against its
    /// certificate (RFC 7252 §9.1.3.3: "the certificate MUST be checked"),
    /// an address as an IP address.
    ///
    /// # Errors
    /// OpenSSL refuses the name.
    pub fn ssl(&self, ctx: &SslContext) -> Result<Ssl, ErrorStack> {
        let mut ssl = Ssl::new(ctx)?;
        if let (Some(name), Some(_)) = (&self.server_name, &self.ca_file) {
            match name.parse::<IpAddr>() {
                Ok(ip) => ssl.param_mut().set_ip(ip)?,
                Err(_) => {
                    ssl.param_mut().set_host(name)?;
                    ssl.set_hostname(name)?;
                }
            }
        }
        Ok(ssl)
    }
}
