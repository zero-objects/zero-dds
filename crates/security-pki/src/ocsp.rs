// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCSP (RFC 6960) for the identity certificates of DDS:Auth:PKI-DH.
//!
//! DDS-Security 1.2 §10.3.3 Table 59:
//!
//! * `validate_local_identity` "shall check a CRL and/or an OCSP
//!   (RFC 2560) responder";
//! * `begin_handshake_reply` and `process_handshake` take the optional
//!   `ocsp_status` of the token: "If the property ocsp_status is present,
//!   the operation shall verify that the OCSP response included in the
//!   property corresponds to the identity in the c.id property. [...] If
//!   that status is good and the validity interval has not been exceeded it
//!   shall accept that as proof that the IdentityCredential is still valid.
//!   If the status is revoked, the operation shall fail [...]. If the
//!   status is different from the aforementioned ones it shall behave as if
//!   the ocsp_status property was not present." Without it the operation
//!   "shall use its own means" (an OCSP query or a CRL).
//!
//! [`verify_staple`] reads an `OCSPResponse` (RFC 6960 §4.2), finds the
//! `SingleResponse` for the certificate (its `CertID` must name the issuer
//! and the serial number), checks the validity interval, and checks the
//! signature: by the issuing CA, or by a certificate the response carries
//! that the CA issued for OCSP signing (RFC 6960 §4.2.2.2). A response whose
//! signature does not verify says nothing, not even `revoked`.
//! [`query_responder`] asks the responder a certificate names in its
//! Authority Information Access extension, over HTTP (RFC 6960 Appendix A).
//!
//! What a response that cannot be had means is the setting
//! [`OcspUnreachable`]: by default the certificate is refused, the check the
//! clause asks for could not be made.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::time::Duration;

use rustls_pki_types::CertificateDer;
use x509_cert::Certificate;
use x509_cert::der::{Decode, Encode};

/// Cert status from an OCSP response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcspStatus {
    /// `good` (RFC 6960 §2.2).
    Good,
    /// `revoked`. Time and reason are not passed on.
    Revoked,
    /// `unknown`: the responder does not know the certificate.
    Unknown,
    /// The response could not be parsed.
    Malformed,
}

/// What to do when the status of an identity certificate cannot be
/// determined: no usable `ocsp_status`, and the responder the certificate
/// names did not answer with a response that verifies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OcspUnreachable {
    /// Refuse the certificate (default): §10.3.3 Table 59 asks for the check,
    /// and a check that was not made is not a pass.
    #[default]
    Fail,
    /// Accept the certificate and say so: the operation returns a note that
    /// the caller logs as a warning.
    Accept,
}

/// The OCSP settings of the certificate validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OcspPolicy {
    /// See [`OcspUnreachable`].
    pub unreachable: OcspUnreachable,
    /// How long a query to a responder may take, connect and answer.
    pub timeout: Duration,
}

impl Default for OcspPolicy {
    fn default() -> Self {
        Self {
            unreachable: OcspUnreachable::Fail,
            timeout: Duration::from_millis(2000),
        }
    }
}

/// The verdict of [`verify_staple`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcspVerdict {
    /// Status `good`, inside its validity interval, signature verified.
    Good,
    /// Status `revoked`, signature verified.
    Revoked,
    /// Anything else: not successful, another certificate, outside the
    /// interval, `unknown`, a signature that does not verify. The text says
    /// which.
    Indeterminate(String),
}

// ---------------------------------------------------------------------
// DER
// ---------------------------------------------------------------------

struct Tlv<'a> {
    tag: u8,
    content: &'a [u8],
    full: &'a [u8],
}

fn tlv<'a>(buf: &mut &'a [u8]) -> Option<Tlv<'a>> {
    let b = *buf;
    if b.len() < 2 {
        return None;
    }
    let tag = b[0];
    let first = b[1];
    let (len, header) = if first < 0x80 {
        (usize::from(first), 2)
    } else {
        let n = usize::from(first & 0x7f);
        if n == 0 || n > 4 || b.len() < 2 + n {
            return None;
        }
        let mut v = 0usize;
        for byte in &b[2..2 + n] {
            v = (v << 8) | usize::from(*byte);
        }
        (v, 2 + n)
    };
    let end = header.checked_add(len)?;
    if b.len() < end {
        return None;
    }
    let out = Tlv {
        tag,
        content: &b[header..end],
        full: &b[..end],
    };
    *buf = &b[end..];
    Some(out)
}

fn expect<'a>(buf: &mut &'a [u8], tag: u8) -> Option<Tlv<'a>> {
    let t = tlv(buf)?;
    (t.tag == tag).then_some(t)
}

fn encode(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len() + 6);
    out.push(tag);
    if content.len() < 0x80 {
        out.push(content.len() as u8);
    } else if content.len() < 0x100 {
        out.extend_from_slice(&[0x81, content.len() as u8]);
    } else if content.len() < 0x1_0000 {
        out.extend_from_slice(&[0x82, (content.len() >> 8) as u8, content.len() as u8]);
    } else {
        out.extend_from_slice(&[
            0x83,
            (content.len() >> 16) as u8,
            (content.len() >> 8) as u8,
            content.len() as u8,
        ]);
    }
    out.extend_from_slice(content);
    out
}

/// `YYYYMMDDHHMMSSZ` (GeneralizedTime, RFC 5280 §4.1.2.5.2) as Unix seconds.
fn generalized_time(text: &[u8]) -> Option<u64> {
    if text.len() != 15 || text[14] != b'Z' {
        return None;
    }
    let digits = |from: usize, to: usize| -> Option<u64> {
        let mut v = 0u64;
        for b in &text[from..to] {
            if !b.is_ascii_digit() {
                return None;
            }
            v = v * 10 + u64::from(b - b'0');
        }
        Some(v)
    };
    let (y, mo, d) = (digits(0, 4)?, digits(4, 6)?, digits(6, 8)?);
    let (h, mi, s) = (digits(8, 10)?, digits(10, 12)?, digits(12, 14)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    // Days from civil (H. Hinnant).
    let y = if mo <= 2 { y - 1 } else { y } as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (mo as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400)
        .ok()?
        .checked_add(h * 3600 + mi * 60 + s)
}

// ---------------------------------------------------------------------
// OCSPResponse
// ---------------------------------------------------------------------

const OID_BASIC: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x01, 0x01];
const OID_SHA1: &[u8] = &[0x2b, 0x0e, 0x03, 0x02, 0x1a];
const OID_SHA256: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];
/// id-kp-OCSPSigning, 1.3.6.1.5.5.7.3.9.
const EKU_OCSP_SIGNING: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x09];

struct Single {
    hash_oid: Vec<u8>,
    name_hash: Vec<u8>,
    key_hash: Vec<u8>,
    serial: Vec<u8>,
    status: OcspStatus,
    this_update: u64,
    next_update: Option<u64>,
}

struct Basic<'a> {
    tbs: &'a [u8],
    signature: &'a [u8],
    certs: Vec<&'a [u8]>,
    singles: Vec<Single>,
}

fn strip_zero(mut v: &[u8]) -> &[u8] {
    while v.len() > 1 && v[0] == 0 {
        v = &v[1..];
    }
    v
}

fn parse_single(content: &[u8]) -> Option<Single> {
    let mut b = content;
    let cert_id = expect(&mut b, 0x30)?;
    let mut c = cert_id.content;
    let alg = expect(&mut c, 0x30)?;
    let mut a = alg.content;
    let hash_oid = expect(&mut a, 0x06)?.content.to_vec();
    let name_hash = expect(&mut c, 0x04)?.content.to_vec();
    let key_hash = expect(&mut c, 0x04)?.content.to_vec();
    let serial = strip_zero(expect(&mut c, 0x02)?.content).to_vec();
    let status_tlv = tlv(&mut b)?;
    let status = match status_tlv.tag {
        0x80 => OcspStatus::Good,
        0xA1 => OcspStatus::Revoked,
        0x82 => OcspStatus::Unknown,
        _ => return None,
    };
    let this_update = generalized_time(expect(&mut b, 0x18)?.content)?;
    let mut next_update = None;
    if b.first() == Some(&0xA0) {
        let explicit = tlv(&mut b)?;
        let mut inner = explicit.content;
        next_update = Some(generalized_time(expect(&mut inner, 0x18)?.content)?);
    }
    Some(Single {
        hash_oid,
        name_hash,
        key_hash,
        serial,
        status,
        this_update,
        next_update,
    })
}

fn parse_basic(response: &[u8]) -> Option<Basic<'_>> {
    let mut top = response;
    let outer = expect(&mut top, 0x30)?;
    let mut o = outer.content;
    let status = expect(&mut o, 0x0A)?;
    if status.content != [0] {
        return None;
    }
    let bytes = expect(&mut o, 0xA0)?;
    let mut b = bytes.content;
    let seq = expect(&mut b, 0x30)?;
    let mut s = seq.content;
    if expect(&mut s, 0x06)?.content != OID_BASIC {
        return None;
    }
    let basic_der = expect(&mut s, 0x04)?.content;
    let mut bd = basic_der;
    let basic = expect(&mut bd, 0x30)?;
    let mut p = basic.content;
    let tbs = expect(&mut p, 0x30)?;
    let _alg = expect(&mut p, 0x30)?;
    let sig = expect(&mut p, 0x03)?;
    let signature = sig.content.get(1..)?;
    let mut certs = Vec::new();
    if p.first() == Some(&0xA0) {
        let explicit = tlv(&mut p)?;
        let mut e = explicit.content;
        let list = expect(&mut e, 0x30)?;
        let mut l = list.content;
        while !l.is_empty() {
            certs.push(expect(&mut l, 0x30)?.full);
        }
    }
    // ResponseData
    let mut t = tbs.content;
    if t.first() == Some(&0xA0) {
        let _version = tlv(&mut t)?;
    }
    let _responder = tlv(&mut t)?;
    let _produced = expect(&mut t, 0x18)?;
    let responses = expect(&mut t, 0x30)?;
    let mut r = responses.content;
    let mut singles = Vec::new();
    while !r.is_empty() {
        let single = expect(&mut r, 0x30)?;
        singles.push(parse_single(single.content)?);
    }
    Some(Basic {
        tbs: tbs.full,
        signature,
        certs,
        singles,
    })
}

/// The status of the first `SingleResponse` of a (successful, basic) OCSP
/// response, or [`OcspStatus::Malformed`]. The signature is not checked
/// here; [`verify_staple`] does.
#[must_use]
pub fn parse_ocsp_status(response_der: &[u8]) -> OcspStatus {
    parse_basic(response_der)
        .and_then(|basic| basic.singles.first().map(|s| s.status))
        .unwrap_or(OcspStatus::Malformed)
}

/// `Ok(())` only if the first `SingleResponse` is `good`.
///
/// # Errors
/// `AuthenticationFailed` for `revoked` and `unknown`, `BadArgument` for a
/// response that does not parse.
pub fn require_good_status(
    response_der: &[u8],
) -> Result<(), zerodds_security::error::SecurityError> {
    use zerodds_security::error::{SecurityError, SecurityErrorKind};
    match parse_ocsp_status(response_der) {
        OcspStatus::Good => Ok(()),
        OcspStatus::Revoked => Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "ocsp: cert is revoked",
        )),
        OcspStatus::Unknown => Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "ocsp: responder does not know the cert",
        )),
        OcspStatus::Malformed => Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "ocsp: response not parseable",
        )),
    }
}

fn hash_with(oid: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    use sha1::Digest as _;
    if oid == OID_SHA1 {
        Some(sha1::Sha1::digest(data).to_vec())
    } else if oid == OID_SHA256 {
        Some(sha2::Sha256::digest(data).to_vec())
    } else {
        None
    }
}

/// The `CertID` fields of `cert` as issued by `issuer`, for `hash_oid`:
/// hash of the issuer's DER name, hash of its public key (the BIT STRING
/// contents), the serial number.
fn cert_id_fields(
    hash_oid: &[u8],
    cert: &Certificate,
    issuer: &Certificate,
) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let name = issuer.tbs_certificate.subject.to_der().ok()?;
    let key = issuer
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes();
    let serial = strip_zero(cert.tbs_certificate.serial_number.as_bytes()).to_vec();
    Some((
        hash_with(hash_oid, &name)?,
        hash_with(hash_oid, key)?,
        serial,
    ))
}

fn signed_by(cert_der: &[u8], message: &[u8], signature: &[u8]) -> bool {
    let der = CertificateDer::from_slice(cert_der);
    let Ok(cert) = webpki::EndEntityCert::try_from(&der) else {
        return false;
    };
    webpki::ALL_VERIFICATION_ALGS
        .iter()
        .any(|alg| cert.verify_signature(*alg, message, signature).is_ok())
}

/// A certificate the response carries is a responder the CA authorised:
/// issued by the CA and good for OCSP signing (RFC 6960 §4.2.2.2).
fn delegated_responder_ok(responder_der: &[u8], issuer_der: &[u8], now: u64) -> bool {
    let issuer = CertificateDer::from_slice(issuer_der);
    let Ok(anchor) = webpki::anchor_from_trusted_cert(&issuer) else {
        return false;
    };
    let responder = CertificateDer::from_slice(responder_der);
    let Ok(ee) = webpki::EndEntityCert::try_from(&responder) else {
        return false;
    };
    ee.verify_for_usage(
        webpki::ALL_VERIFICATION_ALGS,
        &[anchor],
        &[],
        rustls_pki_types::UnixTime::since_unix_epoch(Duration::from_secs(now)),
        webpki::KeyUsage::required(EKU_OCSP_SIGNING),
        None,
        None,
    )
    .is_ok()
}

/// How far a response may lie in the future (clock difference), seconds.
const CLOCK_SKEW_SECS: u64 = 300;

/// Verifies an `OCSPResponse` for `cert_der`, issued by `issuer_der`, at
/// `now` (Unix seconds). See the module documentation for what is checked.
#[must_use]
pub fn verify_staple(
    response_der: &[u8],
    cert_der: &[u8],
    issuer_der: &[u8],
    now: u64,
) -> OcspVerdict {
    let Some(basic) = parse_basic(response_der) else {
        return OcspVerdict::Indeterminate(
            "the response is not a successful basic OCSPResponse".into(),
        );
    };
    let (Ok(cert), Ok(issuer)) = (
        Certificate::from_der(cert_der),
        Certificate::from_der(issuer_der),
    ) else {
        return OcspVerdict::Indeterminate("the certificate or its issuer does not parse".into());
    };
    let Some(single) = basic.singles.iter().find(|single| {
        cert_id_fields(&single.hash_oid, &cert, &issuer).is_some_and(|(name, key, serial)| {
            name == single.name_hash && key == single.key_hash && serial == single.serial
        })
    }) else {
        return OcspVerdict::Indeterminate(
            "no response in it is for this certificate and issuer".into(),
        );
    };
    if single.this_update > now.saturating_add(CLOCK_SKEW_SECS) {
        return OcspVerdict::Indeterminate("thisUpdate is in the future".into());
    }
    if single.next_update.is_some_and(|next| now > next) {
        return OcspVerdict::Indeterminate("the validity interval (nextUpdate) is exceeded".into());
    }
    let authorised = signed_by(issuer_der, basic.tbs, basic.signature)
        || basic.certs.iter().any(|responder| {
            delegated_responder_ok(responder, issuer_der, now)
                && signed_by(responder, basic.tbs, basic.signature)
        });
    if !authorised {
        return OcspVerdict::Indeterminate(
            "the signature is not the issuer's or an authorised responder's".into(),
        );
    }
    match single.status {
        OcspStatus::Good => OcspVerdict::Good,
        OcspStatus::Revoked => OcspVerdict::Revoked,
        _ => OcspVerdict::Indeterminate("the status is unknown".into()),
    }
}

// ---------------------------------------------------------------------
// Responder
// ---------------------------------------------------------------------

/// The `OCSPRequest` (RFC 6960 §4.1) for `cert_der` as issued by
/// `issuer_der`, with a SHA-1 `CertID` (what every responder answers).
#[must_use]
pub fn build_request(cert_der: &[u8], issuer_der: &[u8]) -> Option<Vec<u8>> {
    let cert = Certificate::from_der(cert_der).ok()?;
    let issuer = Certificate::from_der(issuer_der).ok()?;
    let (name, key, serial) = cert_id_fields(OID_SHA1, &cert, &issuer)?;
    let mut alg = encode(0x06, OID_SHA1);
    alg.extend_from_slice(&[0x05, 0x00]);
    let mut cert_id = encode(0x30, &alg);
    cert_id.extend(encode(0x04, &name));
    cert_id.extend(encode(0x04, &key));
    let mut serial_der = serial;
    if serial_der.first().is_some_and(|b| b & 0x80 != 0) {
        serial_der.insert(0, 0);
    }
    cert_id.extend(encode(0x02, &serial_der));
    let request = encode(0x30, &encode(0x30, &cert_id));
    let request_list = encode(0x30, &request);
    Some(encode(0x30, &encode(0x30, &request_list)))
}

/// The OCSP responder URLs a certificate names in its Authority Information
/// Access extension (RFC 5280 §4.2.2.1).
#[must_use]
pub fn responder_urls(cert_der: &[u8]) -> Vec<String> {
    use x509_cert::ext::pkix::AuthorityInfoAccessSyntax;
    use x509_cert::ext::pkix::name::GeneralName;
    let Ok(cert) = Certificate::from_der(cert_der) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ext in cert.tbs_certificate.extensions.iter().flatten() {
        if ext.extn_id.to_string() != "1.3.6.1.5.5.7.1.1" {
            continue;
        }
        let Ok(aia) = AuthorityInfoAccessSyntax::from_der(ext.extn_value.as_bytes()) else {
            continue;
        };
        for access in aia.0 {
            if access.access_method.to_string() == "1.3.6.1.5.5.7.48.1" {
                if let GeneralName::UniformResourceIdentifier(uri) = access.access_location {
                    out.push(uri.to_string());
                }
            }
        }
    }
    out
}

/// POSTs `request` to the responder `url` and returns the body of the `200`
/// answer. RFC 6960 Appendix A.1 describes the transport as HTTP and does not
/// limit the scheme; `http://` URLs are sent in the clear, `https://` URLs
/// over TLS (rustls, with the `ocsp-https` feature, which is on by default).
/// The TLS server certificate has to validate against `tls_trust_anchors_der`,
/// the trust anchors of the identity CA: the responder belongs to the CA of
/// the identities. An OCSP response is signed and checked in any case
/// ([`verify_staple`]); TLS protects the exchange on the way.
///
/// # Errors
/// The reason as text: a scheme that is neither, the connection, the TLS
/// handshake and its certificate check, a timeout, a status other than 200,
/// an answer over 1 MiB.
pub fn query_responder(
    url: &str,
    request: &[u8],
    timeout: Duration,
    tls_trust_anchors_der: &[Vec<u8>],
) -> Result<Vec<u8>, String> {
    use std::net::{TcpStream, ToSocketAddrs};
    let (secure, rest) = if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else {
        return Err(alloc::format!(
            "'{url}' is neither an http:// nor an https:// URL"
        ));
    };
    let (authority, path) = match rest.find('/') {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, "/"),
    };
    let host_port = if authority.contains(':') {
        authority.to_string()
    } else {
        alloc::format!("{authority}:{}", if secure { 443 } else { 80 })
    };
    let host = authority.split(':').next().unwrap_or(authority);
    let addr = host_port
        .to_socket_addrs()
        .map_err(|e| alloc::format!("resolving {host_port}: {e}"))?
        .next()
        .ok_or_else(|| alloc::format!("{host_port} has no address"))?;
    let stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| alloc::format!("connecting to {host_port}: {e}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|e| alloc::format!("socket: {e}"))?;
    let head = alloc::format!(
        "POST {path} HTTP/1.0\r\nHost: {host}\r\nContent-Type: application/ocsp-request\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        request.len()
    );
    let exchanged = if secure {
        tls_exchange(
            stream,
            host,
            tls_trust_anchors_der,
            head.as_bytes(),
            request,
        )?
    } else {
        plain_exchange(stream, head.as_bytes(), request)?
    };
    let answer = exchanged.map_err(|e| alloc::format!("{host_port}: {e}"))?;
    let split = answer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("no HTTP header in the answer")?;
    let status_line = core::str::from_utf8(&answer[..split])
        .ok()
        .and_then(|h| h.lines().next())
        .unwrap_or("");
    if status_line.split_whitespace().nth(1) != Some("200") {
        return Err(alloc::format!("responder answered '{status_line}'"));
    }
    Ok(answer[split + 4..].to_vec())
}

/// The request and the whole answer over a plain TCP connection.
fn plain_exchange(
    mut stream: std::net::TcpStream,
    head: &[u8],
    request: &[u8],
) -> Result<Result<Vec<u8>, String>, String> {
    use std::io::{Read, Write};
    let sent = stream
        .write_all(head)
        .and_then(|()| stream.write_all(request));
    if let Err(e) = sent {
        return Ok(Err(alloc::format!("sending: {e}")));
    }
    let mut answer = Vec::new();
    Ok(stream
        .take(1 << 20)
        .read_to_end(&mut answer)
        .map(|_| answer)
        .map_err(|e| alloc::format!("reading: {e}")))
}

/// The request and the whole answer over TLS, the server certificate
/// validated against the trust anchors.
#[cfg(feature = "ocsp-https")]
fn tls_exchange(
    stream: std::net::TcpStream,
    host: &str,
    trust_anchors_der: &[Vec<u8>],
    head: &[u8],
    request: &[u8],
) -> Result<Result<Vec<u8>, String>, String> {
    use std::io::{ErrorKind, Read, Write};
    use std::sync::Arc;
    let mut roots = rustls::RootCertStore::empty();
    for anchor in trust_anchors_der {
        roots
            .add(rustls::pki_types::CertificateDer::from(anchor.clone()))
            .map_err(|e| alloc::format!("trust anchor: {e}"))?;
    }
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| alloc::format!("TLS versions: {e}"))?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| alloc::format!("server name '{host}': {e}"))?;
    let connection = rustls::ClientConnection::new(Arc::new(config), server_name)
        .map_err(|e| alloc::format!("TLS: {e}"))?;
    let mut tls = rustls::StreamOwned::new(connection, stream);
    let sent = tls
        .write_all(head)
        .and_then(|()| tls.write_all(request))
        .and_then(|()| tls.flush());
    if let Err(e) = sent {
        return Ok(Err(alloc::format!("TLS handshake or sending: {e}")));
    }
    let mut answer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match tls.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                answer.extend_from_slice(&chunk[..n]);
                if answer.len() > 1 << 20 {
                    break;
                }
            }
            // A server that closes without close_notify after its answer.
            Err(e) if e.kind() == ErrorKind::UnexpectedEof && !answer.is_empty() => break,
            Err(e) => return Ok(Err(alloc::format!("reading: {e}"))),
        }
    }
    Ok(Ok(answer))
}

/// Without the `ocsp-https` feature the crate has no TLS client.
#[cfg(not(feature = "ocsp-https"))]
fn tls_exchange(
    _stream: std::net::TcpStream,
    _host: &str,
    _trust_anchors_der: &[Vec<u8>],
    _head: &[u8],
    _request: &[u8],
) -> Result<Result<Vec<u8>, String>, String> {
    Err("an https:// responder needs the feature `ocsp-https` of zerodds-security-pki".into())
}

/// How an identity certificate's revocation status was settled by OCSP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcspOutcome {
    /// Nothing to ask: no `ocsp_status`, no responder in the certificate (a
    /// configured CRL covers the rest).
    NotChecked,
    /// A response, stapled or from the responder, said `good`; it is kept to
    /// be sent as the `ocsp_status` of our own handshake tokens.
    Good(Vec<u8>),
    /// The status could not be determined and [`OcspUnreachable::Accept`]
    /// let the certificate pass; the text says why.
    AcceptedUndetermined(String),
}

/// The OCSP part of the validation of the identity certificate `cert_der`,
/// whose issuer is among `anchors_der`.
///
/// # Errors
/// The certificate is `revoked`, or its status could not be determined and
/// the policy is [`OcspUnreachable::Fail`].
pub fn check_identity(
    cert_der: &[u8],
    anchors_der: &[Vec<u8>],
    staple: Option<&[u8]>,
    has_crl: bool,
    policy: &OcspPolicy,
    now: u64,
) -> Result<OcspOutcome, String> {
    let issuer = Certificate::from_der(cert_der).ok().and_then(|cert| {
        anchors_der.iter().find(|anchor| {
            Certificate::from_der(anchor)
                .is_ok_and(|a| a.tbs_certificate.subject == cert.tbs_certificate.issuer)
        })
    });
    let Some(issuer) = issuer else {
        return Ok(OcspOutcome::NotChecked);
    };
    // The response that came with the token.
    if let Some(staple) = staple.filter(|s| !s.is_empty()) {
        match verify_staple(staple, cert_der, issuer, now) {
            OcspVerdict::Good => return Ok(OcspOutcome::Good(staple.to_vec())),
            OcspVerdict::Revoked => {
                return Err("the OCSP response says the certificate is revoked".into());
            }
            // "behave as if the ocsp_status property was not present"
            OcspVerdict::Indeterminate(_) => {}
        }
    }
    // Its own means: the responder the certificate names.
    let urls = responder_urls(cert_der);
    if urls.is_empty() {
        return Ok(OcspOutcome::NotChecked);
    }
    let Some(request) = build_request(cert_der, issuer) else {
        return Ok(OcspOutcome::NotChecked);
    };
    let mut why = String::new();
    for url in &urls {
        match query_responder(url, &request, policy.timeout, anchors_der) {
            Ok(response) => match verify_staple(&response, cert_der, issuer, now) {
                OcspVerdict::Good => return Ok(OcspOutcome::Good(response)),
                OcspVerdict::Revoked => {
                    return Err(alloc::format!(
                        "the OCSP responder {url} says the certificate is revoked"
                    ));
                }
                OcspVerdict::Indeterminate(reason) => {
                    why = alloc::format!("responder {url}: {reason}");
                }
            },
            Err(reason) => why = alloc::format!("responder {url}: {reason}"),
        }
    }
    // A CRL that was checked settles it where the responder cannot.
    if has_crl {
        return Ok(OcspOutcome::NotChecked);
    }
    match policy.unreachable {
        OcspUnreachable::Fail => Err(alloc::format!(
            "the revocation status of the certificate could not be determined ({why}); \
             org.zerodds.sec.auth.ocsp_unreachable=accept lets it pass"
        )),
        OcspUnreachable::Accept => Ok(OcspOutcome::AcceptedUndetermined(why)),
    }
}

#[cfg(all(test, feature = "ring-backend"))]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use rcgen::{
        BasicConstraints, CertificateParams, CustomExtension, ExtendedKeyUsagePurpose, IsCa,
        KeyPair, KeyUsagePurpose,
    };
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const NOW: u64 = 1_900_000_000;

    struct Pki {
        ca_der: Vec<u8>,
        ca_key: KeyPair,
        ca_cert: rcgen::Certificate,
    }

    fn pki() -> Pki {
        let ca_key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "OCSP Test CA");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages.push(KeyUsagePurpose::KeyCertSign);
        let ca_cert = params.self_signed(&ca_key).unwrap();
        Pki {
            ca_der: ca_cert.der().to_vec(),
            ca_key,
            ca_cert,
        }
    }

    fn leaf(pki: &Pki, responder_url: Option<&str>) -> Vec<u8> {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "alice");
        if let Some(url) = responder_url {
            let mut access = encode(0x06, &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x01]);
            access.extend(encode(0x86, url.as_bytes()));
            let content = encode(0x30, &encode(0x30, &access));
            params
                .custom_extensions
                .push(CustomExtension::from_oid_content(
                    &[1, 3, 6, 1, 5, 5, 7, 1, 1],
                    content,
                ));
        }
        params
            .signed_by(&key, &pki.ca_cert, &pki.ca_key)
            .unwrap()
            .der()
            .to_vec()
    }

    fn responder_cert(pki: &Pki, ocsp_signing: bool) -> (Vec<u8>, KeyPair) {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "OCSP Responder");
        if ocsp_signing {
            params
                .extended_key_usages
                .push(ExtendedKeyUsagePurpose::OcspSigning);
        } else {
            params
                .extended_key_usages
                .push(ExtendedKeyUsagePurpose::ClientAuth);
        }
        let cert = params.signed_by(&key, &pki.ca_cert, &pki.ca_key).unwrap();
        (cert.der().to_vec(), key)
    }

    fn generalized(secs: u64) -> Vec<u8> {
        let days = (secs / 86_400) as i64 + 719_468;
        let era = days.div_euclid(146_097);
        let doe = days - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        let rem = secs % 86_400;
        alloc::format!(
            "{y:04}{m:02}{d:02}{:02}{:02}{:02}Z",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        )
        .into_bytes()
    }

    struct Resp<'a> {
        cert_der: &'a [u8],
        issuer_der: &'a [u8],
        status_tag: u8,
        this_update: u64,
        next_update: Option<u64>,
        signer: &'a KeyPair,
        certs: Vec<Vec<u8>>,
        hash_oid: &'a [u8],
    }

    fn response(r: &Resp<'_>) -> Vec<u8> {
        let cert = Certificate::from_der(r.cert_der).unwrap();
        let issuer = Certificate::from_der(r.issuer_der).unwrap();
        let (name, key, serial) = cert_id_fields(r.hash_oid, &cert, &issuer).unwrap();
        let mut alg = encode(0x06, r.hash_oid);
        alg.extend_from_slice(&[0x05, 0x00]);
        // CertID ::= SEQUENCE { hashAlgorithm, issuerNameHash, issuerKeyHash,
        // serialNumber } (RFC 6960 §4.1.1)
        let mut id = encode(0x30, &alg);
        id.extend(encode(0x04, &name));
        id.extend(encode(0x04, &key));
        let mut serial_der = serial;
        if serial_der.first().is_some_and(|b| b & 0x80 != 0) {
            serial_der.insert(0, 0);
        }
        id.extend(encode(0x02, &serial_der));
        let mut single = encode(0x30, &id);
        single.extend(match r.status_tag {
            0xA1 => encode(0xA1, &encode(0x18, &generalized(r.this_update))),
            tag => encode(tag, &[]),
        });
        single.extend(encode(0x18, &generalized(r.this_update)));
        if let Some(next) = r.next_update {
            single.extend(encode(0xA0, &encode(0x18, &generalized(next))));
        }
        let mut data = encode(0xA2, &encode(0x04, &[0u8; 20]));
        data.extend(encode(0x18, &generalized(r.this_update)));
        data.extend(encode(0x30, &encode(0x30, &single)));
        let tbs = encode(0x30, &data);
        let rng = ring::rand::SystemRandom::new();
        let pair = ring::signature::EcdsaKeyPair::from_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_ASN1_SIGNING,
            &r.signer.serialize_der(),
            &rng,
        )
        .unwrap();
        let signature = pair.sign(&rng, &tbs).unwrap();
        let mut sig_alg = encode(0x06, &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02]);
        sig_alg = encode(0x30, &sig_alg);
        let mut bit = vec![0u8];
        bit.extend_from_slice(signature.as_ref());
        let mut basic = tbs;
        basic.extend(sig_alg);
        basic.extend(encode(0x03, &bit));
        if !r.certs.is_empty() {
            let list: Vec<u8> = r.certs.iter().flatten().copied().collect();
            basic.extend(encode(0xA0, &encode(0x30, &list)));
        }
        let basic = encode(0x30, &basic);
        let mut bytes = encode(0x06, OID_BASIC);
        bytes.extend(encode(0x04, &basic));
        let mut out = encode(0x0A, &[0]);
        out.extend(encode(0xA0, &encode(0x30, &bytes)));
        encode(0x30, &out)
    }

    fn by_ca<'a>(pki: &'a Pki, cert: &'a [u8], status_tag: u8) -> Resp<'a> {
        Resp {
            cert_der: cert,
            issuer_der: &pki.ca_der,
            status_tag,
            this_update: NOW - 60,
            next_update: Some(NOW + 3600),
            signer: &pki.ca_key,
            certs: Vec::new(),
            hash_oid: OID_SHA1,
        }
    }

    /// RFC 6960 §2.2, §4.2.2.2 and DDS-Security 1.2 §10.3.3 Table 59: a
    /// response for this certificate, in its interval, signed by its CA:
    /// `good` is good (either hash of the `CertID`), `revoked` is revoked.
    #[test]
    fn a_signed_response_for_the_certificate_gives_its_status() {
        let pki = pki();
        let cert = leaf(&pki, None);
        for hash in [OID_SHA1, OID_SHA256] {
            let mut r = by_ca(&pki, &cert, 0x80);
            r.hash_oid = hash;
            assert_eq!(
                verify_staple(&response(&r), &cert, &pki.ca_der, NOW),
                OcspVerdict::Good
            );
        }
        let revoked = response(&by_ca(&pki, &cert, 0xA1));
        assert_eq!(
            verify_staple(&revoked, &cert, &pki.ca_der, NOW),
            OcspVerdict::Revoked
        );
        assert_eq!(parse_ocsp_status(&revoked), OcspStatus::Revoked);
        assert_eq!(
            parse_ocsp_status(b"\x30\x02\x80\x00"),
            OcspStatus::Malformed
        );
        assert!(require_good_status(&revoked).is_err());
    }

    /// Responses made by `openssl ocsp` (OpenSSL 4.0.3, `-index`, `-rsigner` =
    /// the CA, SHA-1 `CertID`, byName responder ID, `thisUpdate` 2026-10-16
    /// 00:30:07 UTC) for certificates the same `openssl` issued (`good` for
    /// serial 0x1001, `revoked` for 0x1002 since 2026-01-01). RFC 6960 §4.2.1
    /// and §4.2.2.3 define the structure; the bytes are not from this code.
    #[test]
    fn responses_made_by_openssl_are_read_and_verified() {
        const OPENSSL_NOW: u64 = 1_792_110_607 + 60;
        let ca = include_bytes!("../tests/fixtures/ocsp/ca.der");
        let good_cert = include_bytes!("../tests/fixtures/ocsp/leaf_good.der");
        let revoked_cert = include_bytes!("../tests/fixtures/ocsp/leaf_revoked.der");
        let good = include_bytes!("../tests/fixtures/ocsp/response_good.der");
        let revoked = include_bytes!("../tests/fixtures/ocsp/response_revoked.der");
        assert_eq!(parse_ocsp_status(good), OcspStatus::Good);
        assert_eq!(parse_ocsp_status(revoked), OcspStatus::Revoked);
        assert_eq!(
            verify_staple(good, good_cert, ca, OPENSSL_NOW),
            OcspVerdict::Good
        );
        assert_eq!(
            verify_staple(revoked, revoked_cert, ca, OPENSSL_NOW),
            OcspVerdict::Revoked
        );
        // A response for the other certificate says nothing about this one.
        assert!(matches!(
            verify_staple(good, revoked_cert, ca, OPENSSL_NOW),
            OcspVerdict::Indeterminate(_)
        ));
    }

    /// "If the status is different from the aforementioned ones it shall
    /// behave as if the ocsp_status property was not present": a response for
    /// another certificate, outside its interval, `unknown`, or not signed by
    /// the CA or an authorised responder says nothing, a forged `revoked`
    /// included.
    #[test]
    fn a_response_that_does_not_hold_says_nothing() {
        let pki = pki();
        let cert = leaf(&pki, None);
        let other = leaf(&pki, None);
        let indeterminate = |v: OcspVerdict| matches!(v, OcspVerdict::Indeterminate(_));
        // another certificate
        let for_other = response(&by_ca(&pki, &other, 0x80));
        assert!(indeterminate(verify_staple(
            &for_other,
            &cert,
            &pki.ca_der,
            NOW
        )));
        // interval exceeded and not yet begun
        let mut old = by_ca(&pki, &cert, 0x80);
        old.next_update = Some(NOW - 1);
        assert!(indeterminate(verify_staple(
            &response(&old),
            &cert,
            &pki.ca_der,
            NOW
        )));
        let mut future = by_ca(&pki, &cert, 0x80);
        future.this_update = NOW + 10_000;
        assert!(indeterminate(verify_staple(
            &response(&future),
            &cert,
            &pki.ca_der,
            NOW
        )));
        // unknown
        assert!(indeterminate(verify_staple(
            &response(&by_ca(&pki, &cert, 0x82)),
            &cert,
            &pki.ca_der,
            NOW
        )));
        // signed by a key that is not the CA's
        let stranger = KeyPair::generate().unwrap();
        let mut forged = by_ca(&pki, &cert, 0xA1);
        forged.signer = &stranger;
        assert!(indeterminate(verify_staple(
            &response(&forged),
            &cert,
            &pki.ca_der,
            NOW
        )));
        // a changed octet in the signed part
        let mut tampered = response(&by_ca(&pki, &cert, 0x80));
        let at = tampered.len() / 2;
        tampered[at] ^= 0x01;
        assert!(indeterminate(verify_staple(
            &tampered,
            &cert,
            &pki.ca_der,
            NOW
        )));
        // not a response at all
        assert!(indeterminate(verify_staple(
            b"junk",
            &cert,
            &pki.ca_der,
            NOW
        )));
    }

    /// RFC 6960 §4.2.2.2: a responder certificate the CA issued for OCSP
    /// signing, carried in the response, may sign it; one without that
    /// purpose may not.
    #[test]
    fn a_delegated_responder_signs_only_with_the_ocsp_signing_purpose() {
        let pki = pki();
        let cert = leaf(&pki, None);
        let (good_cert, good_key) = responder_cert(&pki, true);
        let mut r = by_ca(&pki, &cert, 0x80);
        r.signer = &good_key;
        r.certs = vec![good_cert];
        assert_eq!(
            verify_staple(&response(&r), &cert, &pki.ca_der, NOW),
            OcspVerdict::Good
        );
        let (bad_cert, bad_key) = responder_cert(&pki, false);
        let mut r = by_ca(&pki, &cert, 0x80);
        r.signer = &bad_key;
        r.certs = vec![bad_cert];
        assert!(matches!(
            verify_staple(&response(&r), &cert, &pki.ca_der, NOW),
            OcspVerdict::Indeterminate(_)
        ));
    }

    /// Serves `body` once, as an HTTP 200, on a loopback port.
    fn serve(listener: TcpListener, body: Vec<u8>) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 2048];
            loop {
                let n = stream.read(&mut chunk).unwrap();
                request.extend_from_slice(&chunk[..n]);
                if let Some(at) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&request[..at]).to_ascii_lowercase();
                    let length = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if request.len() >= at + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let head = alloc::format!(
                "HTTP/1.0 200 OK\r\nContent-Type: application/ocsp-response\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
            request
        })
    }

    fn check(
        cert: &[u8],
        pki: &Pki,
        staple: Option<&[u8]>,
        policy: &OcspPolicy,
    ) -> Result<OcspOutcome, String> {
        check_identity(cert, &[pki.ca_der.clone()], staple, false, policy, NOW)
    }

    /// DDS-Security 1.2 §10.3.3 Table 59: a stapled `revoked` fails the
    /// operation, a stapled `good` is the proof, a staple that says nothing is
    /// as none; with nothing to ask, nothing is checked.
    #[test]
    fn a_stapled_status_decides_and_a_useless_one_is_none() {
        let pki = pki();
        let cert = leaf(&pki, None);
        let policy = OcspPolicy::default();
        let good = response(&by_ca(&pki, &cert, 0x80));
        let revoked = response(&by_ca(&pki, &cert, 0xA1));
        assert!(matches!(
            check(&cert, &pki, Some(&good), &policy),
            Ok(OcspOutcome::Good(_))
        ));
        assert!(check(&cert, &pki, Some(&revoked), &policy).is_err());
        assert_eq!(
            check(&cert, &pki, Some(b"junk"), &policy),
            Ok(OcspOutcome::NotChecked)
        );
        assert_eq!(
            check(&cert, &pki, None, &policy),
            Ok(OcspOutcome::NotChecked)
        );
    }

    /// The responder the certificate names is asked when no staple decides.
    /// `good` passes and is kept to be stapled, `revoked` fails.
    #[test]
    fn the_responder_in_the_certificate_is_asked() {
        let pki = pki();
        for (tag, passes) in [(0x80u8, true), (0xA1, false)] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = alloc::format!(
                "http://127.0.0.1:{}/ocsp",
                listener.local_addr().unwrap().port()
            );
            let cert = leaf(&pki, Some(&url));
            assert_eq!(responder_urls(&cert), vec![url.clone()]);
            let server = serve(listener, response(&by_ca(&pki, &cert, tag)));
            let outcome = check(&cert, &pki, None, &OcspPolicy::default());
            let request = server.join().unwrap();
            assert!(request.starts_with(b"POST /ocsp HTTP/1.0\r\n"));
            let body = &request[request.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4..];
            assert_eq!(body, build_request(&cert, &pki.ca_der).unwrap());
            assert_eq!(body[0], 0x30);
            assert_eq!(outcome.is_ok(), passes, "{outcome:?}");
            if passes {
                assert!(matches!(outcome, Ok(OcspOutcome::Good(_))));
            }
        }
    }

    /// A responder that cannot be reached: refused by default, passed with
    /// the named setting and said so, and a CRL that was checked settles it.
    #[test]
    fn an_unreachable_responder_fails_unless_the_setting_says_accept() {
        let pki = pki();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = alloc::format!(
            "http://127.0.0.1:{}/ocsp",
            listener.local_addr().unwrap().port()
        );
        drop(listener);
        let cert = leaf(&pki, Some(&url));
        let mut policy = OcspPolicy {
            timeout: Duration::from_millis(500),
            ..OcspPolicy::default()
        };
        assert_eq!(policy.unreachable, OcspUnreachable::Fail);
        let refused = check(&cert, &pki, None, &policy).unwrap_err();
        assert!(refused.contains("could not be determined"), "{refused}");
        policy.unreachable = OcspUnreachable::Accept;
        assert!(matches!(
            check(&cert, &pki, None, &policy),
            Ok(OcspOutcome::AcceptedUndetermined(_))
        ));
        let with_crl = check_identity(&cert, &[pki.ca_der.clone()], None, true, &policy, NOW);
        assert_eq!(with_crl, Ok(OcspOutcome::NotChecked));
    }

    // ----------------------------------------------------------------
    // An https:// responder, and the local validation in the background
    // ----------------------------------------------------------------

    /// The Unix time now, as the validation reads it.
    fn real_now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// A response for `cert`, valid around the time now.
    fn response_now(pki: &Pki, cert: &[u8], status_tag: u8) -> Vec<u8> {
        let mut r = by_ca(pki, cert, status_tag);
        r.this_update = real_now() - 60;
        r.next_update = Some(real_now() + 3600);
        response(&r)
    }

    /// A leaf with the responder URL, its PEM and the private key as PEM.
    fn leaf_with_key(pki: &Pki, responder_url: &str) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "alice");
        let mut access = encode(0x06, &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x01]);
        access.extend(encode(0x86, responder_url.as_bytes()));
        let content = encode(0x30, &encode(0x30, &access));
        params
            .custom_extensions
            .push(CustomExtension::from_oid_content(
                &[1, 3, 6, 1, 5, 5, 7, 1, 1],
                content,
            ));
        let cert = params.signed_by(&key, &pki.ca_cert, &pki.ca_key).unwrap();
        (
            cert.der().to_vec(),
            cert.pem().into_bytes(),
            key.serialize_pem().into_bytes(),
        )
    }

    /// A responder that thinks `delay` before it answers.
    fn serve_slowly(
        listener: TcpListener,
        delay: Duration,
        body: Vec<u8>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            std::thread::sleep(delay);
            let mut request = vec![0u8; 8192];
            let _ = stream.read(&mut request).unwrap();
            let head = alloc::format!("HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
        })
    }

    /// DDS-Security 1.2 §9.3.2.11.2: "The method shall return either
    /// VALIDATION_OK if the validation succeeds, or VALIDATION_FAILED if it
    /// fails, or VALIDATION_PENDING_RETRY if the verification has not
    /// finished. If VALIDATION_PENDING_RETRY has been returned, the operation
    /// shall be called again after a configurable delay to check the status of
    /// verification. This shall continue until the operation returns either
    /// VALIDATION_OK [...] or VALIDATION_FAILED." A certificate that names an
    /// OCSP responder is checked on a thread; the call does not wait for the
    /// responder.
    #[test]
    fn a_responder_that_is_slow_gives_pending_retry_until_the_answer_is_in() {
        use crate::PkiAuthenticationPlugin;
        use zerodds_security::authentication::{AuthenticationPlugin, ValidationResult};
        use zerodds_security::properties::{Property, PropertyList};
        let pki = pki();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = alloc::format!(
            "http://127.0.0.1:{}/ocsp",
            listener.local_addr().unwrap().port()
        );
        let (cert_der, cert_pem, key_pem) = leaf_with_key(&pki, &url);
        let server = serve_slowly(
            listener,
            Duration::from_millis(700),
            response_now(&pki, &cert_der, 0x80),
        );
        let text = |b: &[u8]| String::from_utf8(b.to_vec()).unwrap();
        let props = PropertyList::new()
            .with(Property::local(
                "dds.sec.auth.identity_certificate",
                text(&cert_pem),
            ))
            .with(Property::local(
                "dds.sec.auth.identity_ca",
                pki.ca_cert.pem(),
            ))
            .with(Property::local("dds.sec.auth.private_key", text(&key_pem)));
        let mut plugin = PkiAuthenticationPlugin::new();
        let candidate = [0x42u8; 16];
        let started = std::time::Instant::now();
        let first = plugin
            .validate_local_identity(0, &props, candidate)
            .unwrap();
        assert_eq!(first.result, ValidationResult::PendingRetry);
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "the first call did not wait for the responder"
        );
        assert_eq!(
            first.identity,
            zerodds_security::authentication::IdentityHandle::NIL
        );
        // "called again after a configurable delay"
        let mut retries = 0;
        let done = loop {
            std::thread::sleep(Duration::from_millis(100));
            let again = plugin
                .validate_local_identity(0, &props, candidate)
                .unwrap();
            if again.result == ValidationResult::Ok {
                break again;
            }
            assert_eq!(again.result, ValidationResult::PendingRetry);
            retries += 1;
            assert!(retries < 100, "the validation never finished");
        };
        server.join().unwrap();
        assert!(retries >= 1, "at least one retry before the answer");
        assert_ne!(
            done.identity,
            zerodds_security::authentication::IdentityHandle::NIL
        );
        assert_eq!(done.adjusted_participant_guid[0] & 0x80, 0x80);
        // The response that showed the certificate good is the status token's.
        let status = plugin.get_identity_status_token(done.identity).unwrap();
        let status = zerodds_security::token::DataHolder::from_cdr_le(&status).unwrap();
        assert!(
            status
                .binary_property("ocsp_status")
                .is_some_and(|r| !r.is_empty())
        );
    }

    /// The same, with a responder that says `revoked`: the call that finds the
    /// answer fails, and no identity was made.
    #[test]
    fn a_revoked_answer_ends_the_pending_validation_with_a_failure() {
        use crate::PkiAuthenticationPlugin;
        use zerodds_security::authentication::{AuthenticationPlugin, ValidationResult};
        use zerodds_security::properties::{Property, PropertyList};
        let pki = pki();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = alloc::format!(
            "http://127.0.0.1:{}/ocsp",
            listener.local_addr().unwrap().port()
        );
        let (cert_der, cert_pem, key_pem) = leaf_with_key(&pki, &url);
        let server = serve_slowly(
            listener,
            Duration::from_millis(200),
            response_now(&pki, &cert_der, 0xA1),
        );
        let text = |b: &[u8]| String::from_utf8(b.to_vec()).unwrap();
        let props = PropertyList::new()
            .with(Property::local(
                "dds.sec.auth.identity_certificate",
                text(&cert_pem),
            ))
            .with(Property::local(
                "dds.sec.auth.identity_ca",
                pki.ca_cert.pem(),
            ))
            .with(Property::local("dds.sec.auth.private_key", text(&key_pem)));
        let mut plugin = PkiAuthenticationPlugin::new();
        let first = plugin.validate_local_identity(0, &props, [1; 16]).unwrap();
        assert_eq!(first.result, ValidationResult::PendingRetry);
        let failure = loop {
            std::thread::sleep(Duration::from_millis(100));
            match plugin.validate_local_identity(0, &props, [1; 16]) {
                Ok(again) => assert_eq!(again.result, ValidationResult::PendingRetry),
                Err(e) => break e,
            }
        };
        server.join().unwrap();
        assert!(failure.detail.contains("revoked"), "{failure}");
        assert!(plugin.get_identity_token(first.identity).is_err());
    }

    /// A TLS server for the OCSP query: one connection, one answer.
    #[cfg(feature = "ocsp-https")]
    fn serve_tls(
        listener: TcpListener,
        tls_cert_der: Vec<u8>,
        tls_key_der: Vec<u8>,
        body: Vec<u8>,
    ) -> std::thread::JoinHandle<Vec<u8>> {
        use std::sync::Arc;
        std::thread::spawn(move || {
            let (tcp, _) = listener.accept().unwrap();
            let certs = vec![rustls::pki_types::CertificateDer::from(tls_cert_der)];
            let key = rustls::pki_types::PrivateKeyDer::try_from(tls_key_der).unwrap();
            let config = rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap();
            let conn = rustls::ServerConnection::new(Arc::new(config)).unwrap();
            let mut tls = rustls::StreamOwned::new(conn, tcp);
            let mut request = Vec::new();
            let mut chunk = [0u8; 2048];
            loop {
                let n = match tls.read(&mut chunk) {
                    Ok(n) => n,
                    // The client did not like the certificate.
                    Err(_) => return request,
                };
                request.extend_from_slice(&chunk[..n]);
                if let Some(at) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&request[..at]).to_ascii_lowercase();
                    let length = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if request.len() >= at + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let head = alloc::format!(
                "HTTP/1.0 200 OK\r\nContent-Type: application/ocsp-response\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            tls.write_all(head.as_bytes()).unwrap();
            tls.write_all(&body).unwrap();
            tls.flush().unwrap();
            tls.conn.send_close_notify();
            let _ = tls.flush();
            request
        })
    }

    /// The TLS certificate of a responder on 127.0.0.1, issued by `issuer`.
    #[cfg(feature = "ocsp-https")]
    fn tls_identity(issuer: &Pki) -> (Vec<u8>, Vec<u8>) {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::new()).unwrap();
        params
            .subject_alt_names
            .push(rcgen::SanType::IpAddress("127.0.0.1".parse().unwrap()));
        let cert = params
            .signed_by(&key, &issuer.ca_cert, &issuer.ca_key)
            .unwrap();
        (cert.der().to_vec(), key.serialize_der())
    }

    /// RFC 6960 Appendix A.1 does not limit the scheme: an `https://` URL of
    /// the certificate is asked over TLS, the server certificate validated
    /// against the trust anchors of the identity CA. `good` passes and is
    /// kept to be stapled, `revoked` fails.
    #[cfg(feature = "ocsp-https")]
    #[test]
    fn an_https_responder_is_asked_over_tls() {
        let pki = pki();
        for (tag, passes) in [(0x80u8, true), (0xA1, false)] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = alloc::format!(
                "https://127.0.0.1:{}/ocsp",
                listener.local_addr().unwrap().port()
            );
            let cert = leaf(&pki, Some(&url));
            let (tls_cert, tls_key) = tls_identity(&pki);
            let server = serve_tls(
                listener,
                tls_cert,
                tls_key,
                response(&by_ca(&pki, &cert, tag)),
            );
            let outcome = check(&cert, &pki, None, &OcspPolicy::default());
            let request = server.join().unwrap();
            assert!(request.starts_with(b"POST /ocsp HTTP/1.0\r\n"));
            let body = &request[request.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4..];
            assert_eq!(body, build_request(&cert, &pki.ca_der).unwrap());
            assert_eq!(outcome.is_ok(), passes, "{outcome:?}");
            if passes {
                assert!(matches!(outcome, Ok(OcspOutcome::Good(_))));
            }
        }
    }

    /// A responder whose TLS certificate the identity CA did not issue is not
    /// believed: the status is not determined, and the policy says what then.
    #[cfg(feature = "ocsp-https")]
    #[test]
    fn an_https_responder_with_a_foreign_certificate_is_not_believed() {
        let pki = pki();
        let stranger = self::pki();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = alloc::format!(
            "https://127.0.0.1:{}/ocsp",
            listener.local_addr().unwrap().port()
        );
        let cert = leaf(&pki, Some(&url));
        let (tls_cert, tls_key) = tls_identity(&stranger);
        let server = serve_tls(
            listener,
            tls_cert,
            tls_key,
            response(&by_ca(&pki, &cert, 0x80)),
        );
        let refused = check(&cert, &pki, None, &OcspPolicy::default()).unwrap_err();
        let _ = server.join().unwrap();
        assert!(refused.contains("could not be determined"), "{refused}");
        assert!(
            refused.contains("TLS") || refused.contains("certificate"),
            "{refused}"
        );
    }

    /// The scheme is http or https.
    #[test]
    fn a_responder_url_of_another_scheme_is_refused() {
        let error = query_responder(
            "ftp://127.0.0.1/ocsp",
            b"x",
            Duration::from_millis(100),
            &[],
        )
        .unwrap_err();
        assert!(
            error.contains("neither an http:// nor an https://"),
            "{error}"
        );
    }
}
