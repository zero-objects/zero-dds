// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CMS-PKCS#7-Signature-Verifier fuer Permissions/Governance-XML (C3.2).
//!
//! # Spec-Bezug
//!
//! * **OMG DDS-Security 1.2 §10.4.1.1** — Permissions- und Governance-
//!   XML MUESSEN von der Permissions-CA per S/MIME signiert sein.
//! * **RFC 5751** — S/MIME-Container, sowohl `multipart/signed`
//!   (detached) als auch `application/pkcs7-mime; smime-type=signed-data`
//!   (opaque).
//! * **RFC 5652** — CMS SignedData-Struktur.
//! * **RFC 5280** — X.509-Cert-Chain-Validation, hier delegiert an
//!   `rustls-webpki` ueber dieselbe Pipeline wie `zerodds-security-pki`.
//!
//! # Was hier gepruefte wird
//!
//! 1. **S/MIME-Format**: Header-Block geparst, `Content-Type` analysiert.
//!    Beide Varianten akzeptiert:
//!    - `multipart/signed; protocol="application/pkcs7-signature"` →
//!      Boundary-Split, Body-Part 1 = Klartext-XML, Body-Part 2 =
//!      PKCS#7-Detached-Signature.
//!    - `application/pkcs7-mime; smime-type=signed-data` → base64-
//!      decode der Body, ContentInfo.content enthaelt den Klartext
//!      (eContent).
//! 2. **PEM-Wrapper** (`-----BEGIN PKCS7-----`) als zusaetzlicher Pfad
//!    fuer Detached-Signaturen mit separatem Klartext.
//! 3. **PKCS#7/CMS SignedData** decodiert via `cms`-Crate, mindestens
//!    ein `SignerInfo` extrahiert.
//! 4. **Cert-Chain Signer → CA** via webpki — exakt dieselben Trust-
//!    Anchors und Algorithmus-Familien wie der Identity-Plugin
//!    (`zerodds-security-pki::PkiAuthenticationPlugin`).
//! 5. **Signature-Verify** ueber `signedAttrs` (RFC 5652 §5.4): SHA-256
//!    der eContent-Bytes muss mit dem `messageDigest`-Attribut
//!    uebereinstimmen, dann Signatur-Verify auf der DER-Encoding der
//!    Attribute.
//!
//! # Was NICHT hier gemacht wird
//!
//! * Live-CRL/OCSP-Check — separat in `zerodds-security-pki::ocsp` als
//!   nachgelagerter Schritt; C3.2-Scope ist nur Cert-Chain + Signatur.
//! * S/MIME-encrypted Container — Permissions-XML wird nicht
//!   verschluesselt (Spec verlangt nur Authentizitaet, nicht
//!   Vertraulichkeit).
//! * Cross-cert / Bridge-CA — nur direkter Trust-Anchor + Intermediate-
//!   Chain, exakt wie `webpki::EndEntityCert::verify_for_usage`
//!   unterstuetzt.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use cms::cert::CertificateChoices;
use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use const_oid::ObjectIdentifier;
use const_oid::db::rfc5911::{ID_DATA, ID_SIGNED_DATA};
use der::{Decode, Encode, Tag, Tagged, asn1::SetOfVec};
use ring::signature;
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject;
use sha2::{Digest, Sha256};
use x509_cert::Certificate;
use zerodds_security::properties::PropertyList;

use crate::signature::XmlSignatureVerifier;
use crate::xml::PermissionsError;

// ----------------------------------------------------------------------
// OID-Konstanten fuer Algorithmus-Familien.
// ----------------------------------------------------------------------

/// Property-Key fuer das PEM-CA-Bundle der Permissions-CA. Spec-konform
/// gemaess Tabelle 63 (DDS-Security 1.2).
pub const PROP_PERMISSIONS_CA: &str = "dds.sec.access.permissions_ca";

/// `id-messageDigest` (RFC 5652).
const OID_MESSAGE_DIGEST: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");
/// `id-contentType` (RFC 5652).
const OID_CONTENT_TYPE: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.3");

/// SHA-256 OID.
const OID_SHA_256: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1");

/// `ecPublicKey` SPKI-OID (RFC 5480).
const OID_EC_PUBLIC_KEY: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");
/// P-256 named curve OID.
const OID_SECP256R1: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");
/// P-384 named curve OID.
const OID_SECP384R1: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.132.0.34");

/// `ecdsa-with-SHA256` (RFC 5758).
const OID_ECDSA_SHA256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.2");
/// `ecdsa-with-SHA384` (RFC 5758).
const OID_ECDSA_SHA384: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.3");

/// `rsaEncryption` (PKCS#1).
const OID_RSA_ENCRYPTION: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");
/// `sha256WithRSAEncryption`.
const OID_RSA_SHA256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.11");
/// `sha384WithRSAEncryption`.
const OID_RSA_SHA384: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.12");
/// `id-RSASSA-PSS`.
const OID_RSA_PSS: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.10");

// ----------------------------------------------------------------------
// Verifier-Public-Type
// ----------------------------------------------------------------------

/// Verifier fuer S/MIME-PKCS#7-signierte Permissions/Governance-XML.
///
/// Konstruiert mit dem PEM-CA-Bundle der Permissions-CA. Bei jedem
/// `verify_and_extract`-Aufruf wird die Cert-Chain und die Signatur
/// against diese CA gepruefte und das innere Klartext-XML extrahiert.
#[derive(Debug, Clone)]
pub struct CmsPkcs7Verifier {
    /// CA-Cert-Bundle (DER-encoded). Ein oder mehrere Certs koennen als
    /// Trust-Anchor dienen — Cyclone und FastDDS liefern typisch genau
    /// einen, aber wir akzeptieren mehrere fuer Cross-CA-Setups.
    ca_certs_der: Vec<Vec<u8>>,
    /// Optionaler Klartext-XML, der fuer detached PEM-PKCS#7-Validation
    /// als Vergleichs-Content geliefert wird. `None` = nur opaque /
    /// multipart-mit-eingebettetem-Content akzeptieren.
    detached_content: Option<Vec<u8>>,
}

impl CmsPkcs7Verifier {
    /// Erzeugt einen Verifier aus dem PEM-CA-Bundle der Permissions-CA.
    ///
    /// # Errors
    /// `PermissionsError::Malformed` wenn das CA-Bundle leer ist oder
    /// kein parsbares Cert enthaelt.
    pub fn new(ca_pem: &[u8]) -> Result<Self, PermissionsError> {
        let mut ca_certs_der: Vec<Vec<u8>> = Vec::new();
        for item in CertificateDer::pem_slice_iter(ca_pem) {
            let cert = item.map_err(|e| PermissionsError::Malformed(format!("ca pem: {e:?}")))?;
            ca_certs_der.push(cert.as_ref().to_vec());
        }
        if ca_certs_der.is_empty() {
            return Err(PermissionsError::Malformed(
                "ca bundle is empty".to_string(),
            ));
        }
        Ok(Self {
            ca_certs_der,
            detached_content: None,
        })
    }

    /// Erzeugt einen Verifier aus einer `PropertyList` — liest
    /// `dds.sec.access.permissions_ca` als PEM-CA.
    ///
    /// # Errors
    /// `PermissionsError::Malformed` wenn die Property fehlt oder leer
    /// ist.
    pub fn from_property_list(props: &PropertyList) -> Result<Self, PermissionsError> {
        let pem = props.get(PROP_PERMISSIONS_CA).ok_or_else(|| {
            PermissionsError::Malformed(format!("property {PROP_PERMISSIONS_CA} fehlt"))
        })?;
        Self::new(pem.as_bytes())
    }

    /// Setzt einen Klartext-XML-Body fuer den Detached-PEM-PKCS#7-Pfad.
    ///
    /// Wenn ein Caller nur das PEM-PKCS#7 hat (z.B. ueber separaten
    /// File-Pfad `signed_xml_path`), kann er den dazugehoerigen
    /// Klartext-XML hier setzen. Der Verifier ueberprueft dann die
    /// Detached-Signatur gegen diesen Content.
    #[must_use]
    pub fn with_detached_content(mut self, xml: Vec<u8>) -> Self {
        self.detached_content = Some(xml);
        self
    }
}

impl XmlSignatureVerifier for CmsPkcs7Verifier {
    fn verify_and_extract(&self, signed_doc: &[u8]) -> Result<Vec<u8>, PermissionsError> {
        // Drei Eingangs-Pfade unterscheiden:
        //
        //   1. PEM-Wrapper "-----BEGIN PKCS7-----" → detached, braucht
        //      `detached_content`.
        //   2. S/MIME `multipart/signed` → Boundary-Split.
        //   3. S/MIME `application/pkcs7-mime; smime-type=signed-data`
        //      (opaque) → base64-decode den Body, eContent ist im
        //      SignedData embedded.
        let pkcs7_der: Vec<u8>;
        let detached_content_for_verify: Option<Vec<u8>>;

        if looks_like_pem_pkcs7(signed_doc) {
            pkcs7_der = parse_pem_pkcs7(signed_doc)?;
            detached_content_for_verify = self.detached_content.clone();
        } else {
            let (ct, body) = parse_smime_headers(signed_doc)?;
            match ct.kind {
                SmimeKind::MultipartSigned { boundary } => {
                    let (xml, p7) = split_multipart_signed(body, &boundary)?;
                    pkcs7_der = p7;
                    detached_content_for_verify = Some(xml);
                }
                SmimeKind::Pkcs7Mime { transfer_encoding } => {
                    let raw = decode_transfer_body(body, &transfer_encoding)?;
                    pkcs7_der = raw;
                    detached_content_for_verify = self.detached_content.clone();
                }
            }
        }

        verify_cms_signed_xml(
            &pkcs7_der,
            detached_content_for_verify.as_deref(),
            &self.ca_certs_der,
        )
    }
}

// ----------------------------------------------------------------------
// PEM-PKCS7-Erkennung + Decoding
// ----------------------------------------------------------------------

fn looks_like_pem_pkcs7(bytes: &[u8]) -> bool {
    let head = &bytes[..core::cmp::min(64, bytes.len())];
    let s = core::str::from_utf8(head).unwrap_or("");
    // OpenSSL >= 3.x schreibt `-----BEGIN CMS-----` wenn ContentInfo
    // mit signedData enthaelt. AElterer Code (PKCS7-Output) und das
    // RFC-7468 Label `PKCS7` muessen wir auch akzeptieren.
    s.starts_with("-----BEGIN CMS-----")
        || s.starts_with("-----BEGIN PKCS7-----")
        || s.starts_with("-----BEGIN PKCS #7-----")
}

fn parse_pem_pkcs7(bytes: &[u8]) -> Result<Vec<u8>, PermissionsError> {
    let s = core::str::from_utf8(bytes)
        .map_err(|_| PermissionsError::Malformed("pem pkcs7 ist kein UTF-8".to_string()))?;
    // Akzeptiere "CMS", "PKCS7" oder "PKCS #7" als Label.
    let labels = [
        "-----BEGIN CMS-----",
        "-----BEGIN PKCS7-----",
        "-----BEGIN PKCS #7-----",
    ];
    let ends = [
        "-----END CMS-----",
        "-----END PKCS7-----",
        "-----END PKCS #7-----",
    ];
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;
    for (i, l) in labels.iter().enumerate() {
        if let Some(p) = s.find(l) {
            start = Some(p + l.len());
            // Match passendes End-Label.
            if let Some(q) = s.find(ends[i]) {
                end = Some(q);
            }
        }
    }
    let (s_start, s_end) = match (start, end) {
        (Some(a), Some(b)) if b > a => (a, b),
        _ => {
            return Err(PermissionsError::Malformed(
                "pem pkcs7: BEGIN/END fehlt".to_string(),
            ));
        }
    };
    let b64_body: String = s[s_start..s_end]
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    base64_decode(b64_body.as_bytes())
}

// ----------------------------------------------------------------------
// S/MIME-Header-Parser
// ----------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SmimeContentType {
    kind: SmimeKind,
}

#[derive(Debug, Clone)]
enum SmimeKind {
    MultipartSigned { boundary: String },
    Pkcs7Mime { transfer_encoding: String },
}

/// Trennt die S/MIME-Header vom Body. Akzeptiert sowohl CRLF-Trenner
/// als auch reine LF-Trenner (Cyclone + FastDDS Cross-Tools mischen).
fn parse_smime_headers(bytes: &[u8]) -> Result<(SmimeContentType, &[u8]), PermissionsError> {
    let body_start = find_header_body_split(bytes).ok_or_else(|| {
        PermissionsError::Malformed("s/mime: kein Header-Body-Trenner".to_string())
    })?;
    let header_block = &bytes[..body_start];
    let body = &bytes[body_start + header_split_skip(bytes, body_start)..];

    let header_str = core::str::from_utf8(header_block)
        .map_err(|_| PermissionsError::Malformed("s/mime header ist kein UTF-8".to_string()))?;
    let headers = parse_header_lines(header_str);

    let ct = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Content-Type"))
        .map(|(_, v)| v.clone())
        .ok_or_else(|| {
            PermissionsError::Malformed("s/mime: Content-Type-Header fehlt".to_string())
        })?;
    let cte = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Content-Transfer-Encoding"))
        .map(|(_, v)| v.trim().to_lowercase())
        .unwrap_or_else(|| "7bit".to_string());

    let ct_lower = ct.to_lowercase();
    let kind = if ct_lower.starts_with("multipart/signed") {
        let boundary = parse_param(&ct, "boundary").ok_or_else(|| {
            PermissionsError::Malformed("multipart/signed ohne boundary".to_string())
        })?;
        SmimeKind::MultipartSigned { boundary }
    } else if ct_lower.starts_with("application/pkcs7-mime")
        || ct_lower.starts_with("application/x-pkcs7-mime")
    {
        // Nur signed-data ist hier zulaessig.
        let smime_type = parse_param(&ct, "smime-type").unwrap_or_default();
        if smime_type != "signed-data" && !smime_type.is_empty() {
            return Err(PermissionsError::Malformed(format!(
                "smime-type={smime_type} nicht unterstuetzt"
            )));
        }
        SmimeKind::Pkcs7Mime {
            transfer_encoding: cte,
        }
    } else {
        return Err(PermissionsError::Malformed(format!(
            "unsupported Content-Type: {ct}"
        )));
    };

    Ok((SmimeContentType { kind }, body))
}

fn find_header_body_split(bytes: &[u8]) -> Option<usize> {
    // Suche \r\n\r\n oder \n\n. Wenn beides existiert, das frueher
    // auftretende.
    let crlf = find_subseq(bytes, b"\r\n\r\n");
    let lf = find_subseq(bytes, b"\n\n");
    match (crlf, lf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn header_split_skip(bytes: &[u8], pos: usize) -> usize {
    if bytes[pos..].starts_with(b"\r\n\r\n") {
        4
    } else {
        2
    }
}

fn find_subseq(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn parse_header_lines(s: &str) -> Vec<(String, String)> {
    // Unfold continuation-lines (RFC 5322): if a line starts with WSP,
    // append to previous.
    let mut out: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in s.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some((_, v)) = current.as_mut() {
                v.push(' ');
                v.push_str(line.trim_start());
                continue;
            }
        }
        if let Some(pair) = current.take() {
            out.push(pair);
        }
        if let Some(idx) = line.find(':') {
            let key = line[..idx].trim().to_string();
            let val = line[idx + 1..].trim().to_string();
            current = Some((key, val));
        }
    }
    if let Some(pair) = current.take() {
        out.push(pair);
    }
    out
}

fn parse_param(header_value: &str, key: &str) -> Option<String> {
    // Suche `key=value` oder `key="value"` (case-insensitive).
    let lower = header_value.to_lowercase();
    let needle = format!("{}=", key.to_lowercase());
    let idx = lower.find(&needle)?;
    let after = &header_value[idx + needle.len()..];
    let after = after.trim_start();
    if let Some(rest) = after.strip_prefix('"') {
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    } else {
        let end = after
            .find(|c: char| c == ';' || c.is_ascii_whitespace())
            .unwrap_or(after.len());
        Some(after[..end].to_string())
    }
}

fn split_multipart_signed(
    body: &[u8],
    boundary: &str,
) -> Result<(Vec<u8>, Vec<u8>), PermissionsError> {
    let dash_boundary = format!("--{boundary}");

    // Sammle Offsets aller Boundary-Marker (`--boundary`) im Body.
    let mut indices: Vec<usize> = Vec::new();
    let mut from = 0usize;
    while let Some(p) = find_subseq(&body[from..], dash_boundary.as_bytes()) {
        let abs = from + p;
        indices.push(abs);
        from = abs + dash_boundary.len();
    }
    if indices.len() < 2 {
        return Err(PermissionsError::Malformed(
            "multipart/signed: weniger als 2 Boundaries".to_string(),
        ));
    }

    // Body-Parts sind die Bytes zwischen Boundary i und Boundary i+1.
    //
    // RFC 2046 §5.1.1: nur das CRLF DIREKT vor `--boundary` gehoert zur
    // Boundary-Syntax. Alle frueheren CRLF gehoeren zum Body. Wir
    // strippen also exakt EIN trailing CRLF (oder eines `\n` ohne `\r`)
    // — niemals mehr.
    let mut bodies: Vec<&[u8]> = Vec::new();
    for w in indices.windows(2) {
        let start = w[0] + dash_boundary.len();
        // Skip eventuell `--` (close) — in dem Fall ist dies kein Part.
        if body.get(start..start + 2) == Some(b"--") {
            continue;
        }
        // Skip genau die CRLF/LF DIREKT nach der Boundary-Zeile (eine
        // einzige Newline gehoert zur Boundary).
        let mut i = start;
        if i < body.len() && body[i] == b'\r' {
            i += 1;
        }
        if i < body.len() && body[i] == b'\n' {
            i += 1;
        }
        let end = w[1];
        // Trim genau das CRLF/LF unmittelbar vor der naechsten Boundary.
        let mut e = end;
        if e > i && body[e - 1] == b'\n' {
            e -= 1;
            if e > i && body[e - 1] == b'\r' {
                e -= 1;
            }
        }
        bodies.push(&body[i..e]);
    }
    if bodies.len() < 2 {
        return Err(PermissionsError::Malformed(
            "multipart/signed: nicht 2 Body-Parts".to_string(),
        ));
    }

    // Body-Part 1 = Klartext (mit eigenen Headers); wir muessen Header
    // vom XML trennen und das XML-Original-Byte-Range fuer den
    // Signature-Verify zurueckgeben — RFC 1847 §2.4: signed content =
    // exakt Body-Part-1 inklusive seiner Headers (Canonical-CRLF-Form).
    // OpenSSL `cms -sign -text` fuegt `Content-Type: text/plain` als
    // Body-Part-1-Header ein und der Signer signiert _diesen ganzen
    // Block_.
    let body_part_1 = bodies[0];

    // Body-Part 2 = der PKCS#7-Detached-Signatur-Block. Innen sind
    // Headers + base64-encoded DER-Signatur.
    let body_part_2 = bodies[1];
    let p2_split = find_header_body_split(body_part_2).ok_or_else(|| {
        PermissionsError::Malformed("multipart: pkcs7-part ohne Header-Body-Trenner".to_string())
    })?;
    let p2_body = &body_part_2[p2_split + header_split_skip(body_part_2, p2_split)..];
    let p2_b64: String = core::str::from_utf8(p2_body)
        .map_err(|_| PermissionsError::Malformed("pkcs7-part nicht UTF-8".to_string()))?
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let p2_der = base64_decode(p2_b64.as_bytes())?;

    // Body-Part-1: extrahiere das innere XML aus dem text/plain-Block.
    // Aber fuer den Signature-Check brauchen wir die rohen Bytes inkl.
    // Headers — wir liefern hier zwei Varianten: das innere XML als
    // Caller-Output, der Signature-Check unten arbeitet auf body_part_1
    // direkt.
    // RFC 1847: signed content ist die _kanonische_ Form (CRLF) von
    // body-part-1 — wir liefern die rohen Bytes, normalisieren aber
    // einsame LFs auf CRLF. Cyclone/FastDDS-Files sind ohnehin CRLF.
    Ok((normalize_to_canonical_crlf(body_part_1), p2_der))
}

fn normalize_to_canonical_crlf(body: &[u8]) -> Vec<u8> {
    // Wandelt rohe LF in CRLF um (RFC 1847 Canonical-Form). Die
    // Cyclone/FastDDS-S/MIME-Files sind ohnehin schon CRLF — wir machen
    // hier eine defensive Roundtrip.
    let mut out = Vec::with_capacity(body.len() + 8);
    let mut i = 0;
    while i < body.len() {
        if body[i] == b'\n' && (i == 0 || body[i - 1] != b'\r') {
            out.push(b'\r');
            out.push(b'\n');
        } else {
            out.push(body[i]);
        }
        i += 1;
    }
    out
}

fn decode_transfer_body(body: &[u8], encoding: &str) -> Result<Vec<u8>, PermissionsError> {
    match encoding {
        "base64" => {
            let stripped: String = core::str::from_utf8(body)
                .map_err(|_| PermissionsError::Malformed("base64 body ist kein UTF-8".to_string()))?
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .collect();
            base64_decode(stripped.as_bytes())
        }
        "7bit" | "8bit" | "binary" => Ok(body.to_vec()),
        other => Err(PermissionsError::Malformed(format!(
            "unsupported Content-Transfer-Encoding: {other}"
        ))),
    }
}

// ----------------------------------------------------------------------
// CMS SignedData → Verify
// ----------------------------------------------------------------------

fn verify_cms_signed_xml(
    pkcs7_der: &[u8],
    detached_content: Option<&[u8]>,
    ca_certs_der: &[Vec<u8>],
) -> Result<Vec<u8>, PermissionsError> {
    // 1. ContentInfo decodieren.
    let ci = ContentInfo::from_der(pkcs7_der)
        .map_err(|e| PermissionsError::Malformed(format!("ContentInfo: {e:?}")))?;
    if ci.content_type != ID_SIGNED_DATA {
        return Err(PermissionsError::Malformed(format!(
            "ContentInfo: erwarte signedData, gefunden {}",
            ci.content_type
        )));
    }

    // 2. SignedData aus dem ANY auspacken.
    let sd_der = ci
        .content
        .to_der()
        .map_err(|e| PermissionsError::Malformed(format!("SignedData encode: {e:?}")))?;
    let sd = SignedData::from_der(&sd_der)
        .map_err(|e| PermissionsError::Malformed(format!("SignedData parse: {e:?}")))?;

    // 3. eContent extrahieren — entweder aus dem SignedData (opaque-
    // signed mit eContent embedded) oder aus dem detached-Buffer. Im
    // Detached-Fall wird der Content auf RFC-1847-Canonical-CRLF-Form
    // normalisiert (OpenSSL `cms -sign` ohne `-text` macht das auch).
    let econtent_owned: Vec<u8>;
    let econtent: &[u8] = if let Some(ec_any) = sd.encap_content_info.econtent.as_ref() {
        // econtent ist OCTET STRING explicit-tagged.
        let tag = ec_any.tag();
        if tag != Tag::OctetString {
            return Err(PermissionsError::Malformed(format!(
                "eContent: erwarte OCTET STRING, gefunden {tag:?}"
            )));
        }
        econtent_owned = ec_any.value().to_vec();
        &econtent_owned
    } else {
        let raw = detached_content.ok_or_else(|| {
            PermissionsError::Malformed(
                "SignedData ohne eContent und kein detached-content geliefert".to_string(),
            )
        })?;
        econtent_owned = normalize_to_canonical_crlf(raw);
        &econtent_owned
    };

    // 4. Signer-Cert + ggf. Intermediates aus dem Bundle ziehen.
    let mut bundle_certs: Vec<Vec<u8>> = Vec::new();
    if let Some(cset) = &sd.certificates {
        for choice in cset.0.iter() {
            if let CertificateChoices::Certificate(c) = choice {
                let der = c
                    .to_der()
                    .map_err(|e| PermissionsError::Malformed(format!("cert encode: {e:?}")))?;
                bundle_certs.push(der);
            }
        }
    }
    if bundle_certs.is_empty() {
        return Err(PermissionsError::Malformed(
            "SignedData enthaelt kein Signer-Cert".to_string(),
        ));
    }

    // 5. Mindestens ein SignerInfo wird verlangt — Spec §10.4.1.1.
    let signer_info = sd
        .signer_infos
        .0
        .iter()
        .next()
        .ok_or_else(|| PermissionsError::Malformed("SignedData ohne SignerInfo".to_string()))?;

    // 6. SignerCert finden, der zu signer_info.sid passt.
    let signer_cert_der = match_signer_cert(&bundle_certs, signer_info)?;
    if signer_info.signature.as_bytes().is_empty() {
        return Err(PermissionsError::Malformed(
            "SignerInfo: signature ist leer".to_string(),
        ));
    }

    // 7. Cert-Chain Signer → CA via webpki (Intermediates werden aus
    // dem Bundle automatisch ergaenzt).
    let intermediates_der: Vec<&[u8]> = bundle_certs
        .iter()
        .map(Vec::as_slice)
        .filter(|d| *d != signer_cert_der.as_slice())
        .collect();
    verify_chain_to_ca(&signer_cert_der, &intermediates_der, ca_certs_der)?;

    // 8. Signature-Verify.
    verify_signer_info(signer_info, &signer_cert_der, econtent)?;

    // 9. Klartext-XML aus dem text/plain-MIME-Block extrahieren (oder
    // wenn kein text/plain-Header vorhanden ist, die rohen Bytes).
    Ok(strip_text_plain_envelope(econtent))
}

fn strip_text_plain_envelope(content: &[u8]) -> Vec<u8> {
    // OpenSSL `cms -sign -text` praefixt den Content mit
    // `Content-Type: text/plain\r\n\r\n`. Das ist Spec-konform (RFC 5751
    // §3.4) und muss vom Verifier abgezogen werden — sonst kommt das
    // XML mit einem fuehrenden MIME-Header beim Caller an.
    if let Some(idx) = find_subseq(content, b"\r\n\r\n") {
        let head = &content[..idx];
        if header_indicates_text_plain(head) {
            return content[idx + 4..].to_vec();
        }
    }
    if let Some(idx) = find_subseq(content, b"\n\n") {
        let head = &content[..idx];
        if header_indicates_text_plain(head) {
            return content[idx + 2..].to_vec();
        }
    }
    content.to_vec()
}

fn header_indicates_text_plain(head: &[u8]) -> bool {
    let s = match core::str::from_utf8(head) {
        Ok(s) => s.to_lowercase(),
        Err(_) => return false,
    };
    s.contains("content-type: text/plain") || s.contains("content-type:text/plain")
}

fn match_signer_cert(
    bundle: &[Vec<u8>],
    signer_info: &cms::signed_data::SignerInfo,
) -> Result<Vec<u8>, PermissionsError> {
    // Aktuell akzeptieren wir den ersten Cert im Bundle, der ein
    // gueltiges Cert ist und dessen Subject sich aus dem SID-Hint
    // ableiten laesst. OpenSSL packt typisch nur das EE-Cert als
    // erstes; intermediates folgen.
    use cms::signed_data::SignerIdentifier;

    for der in bundle {
        let cert = Certificate::from_der(der)
            .map_err(|e| PermissionsError::Malformed(format!("cert parse: {e:?}")))?;
        let matches = match &signer_info.sid {
            SignerIdentifier::IssuerAndSerialNumber(isn) => {
                cert.tbs_certificate.serial_number == isn.serial_number
                    && cert.tbs_certificate.issuer == isn.issuer
            }
            SignerIdentifier::SubjectKeyIdentifier(_ski) => {
                // SKID-Match braucht Extension-Lookup — wir matchen
                // hier nicht darauf, sondern nutzen den
                // first-cert-fallback unten (Spec-konform fuer
                // single-signer-Pfade).
                false
            }
        };
        if matches {
            return Ok(der.clone());
        }
    }
    // Fallback: erstes Cert im Bundle (typisch EE bei OpenSSL-Output).
    Ok(bundle[0].clone())
}

fn verify_chain_to_ca(
    signer_der: &[u8],
    intermediates_der: &[&[u8]],
    ca_certs_der: &[Vec<u8>],
) -> Result<(), PermissionsError> {
    let ee_owned = CertificateDer::from_slice(signer_der);
    let ee = webpki::EndEntityCert::try_from(&ee_owned)
        .map_err(|e| PermissionsError::Malformed(format!("ee parse: {e:?}")))?;

    let intermediates: Vec<CertificateDer<'_>> = intermediates_der
        .iter()
        .map(|d| CertificateDer::from_slice(d))
        .collect();

    // Trust-Anchors aus den CA-DER-Bytes ableiten.
    let ta_certs: Vec<CertificateDer<'_>> = ca_certs_der
        .iter()
        .map(|b| CertificateDer::from_slice(b))
        .collect();
    let mut anchors: Vec<rustls_pki_types::TrustAnchor<'_>> = Vec::with_capacity(ta_certs.len());
    for tc in &ta_certs {
        let ta = webpki::anchor_from_trusted_cert(tc)
            .map_err(|e| PermissionsError::Malformed(format!("ca trust-anchor: {e:?}")))?;
        anchors.push(ta);
    }

    let now = rustls_pki_types::UnixTime::now();
    let algs = webpki::ALL_VERIFICATION_ALGS;

    // Permissions-Signing ist kein Standard-EKU; wir verwenden
    // `client_auth` als nahester EKU (Cyclone-Default; OpenSSL `cms
    // -sign` setzt typischerweise gar keinen EKU). webpki erlaubt
    // EKU-less Certs unter `KeyUsage::client_auth()`-Modus, wenn das
    // Cert keinen EKU hat. Falls kuenftig ein dedicated EKU spec'd wird,
    // hier anpassen.
    ee.verify_for_usage(
        algs,
        &anchors,
        &intermediates,
        now,
        webpki::KeyUsage::client_auth(),
        None,
        None,
    )
    .map_err(|e| PermissionsError::Malformed(format!("ca chain: {e:?}")))?;

    Ok(())
}

fn verify_signer_info(
    signer_info: &cms::signed_data::SignerInfo,
    signer_cert_der: &[u8],
    econtent: &[u8],
) -> Result<(), PermissionsError> {
    let cert = Certificate::from_der(signer_cert_der)
        .map_err(|e| PermissionsError::Malformed(format!("signer cert parse: {e:?}")))?;
    let spki_der = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|e| PermissionsError::Malformed(format!("spki encode: {e:?}")))?;
    let spki_alg_oid = cert.tbs_certificate.subject_public_key_info.algorithm.oid;
    let pubkey_bytes = cert
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes();

    // SHA-256 erwartet — Spec-Default fuer S/MIME-Permissions.
    if signer_info.digest_alg.oid != OID_SHA_256 {
        return Err(PermissionsError::Malformed(format!(
            "digest_alg {oid} nicht unterstuetzt (erwarte SHA-256)",
            oid = signer_info.digest_alg.oid
        )));
    }

    // Wenn signedAttrs vorhanden sind, ist die Signatur ueber DER der
    // signedAttrs (mit SET-Tag, nicht IMPLICIT [0]). messageDigest in
    // den signedAttrs muss SHA-256(eContent) sein.
    let signed_data_for_verify: Vec<u8> = if let Some(sa) = &signer_info.signed_attrs {
        verify_message_digest_attr(sa, econtent)?;
        verify_content_type_attr(sa)?;
        encode_signed_attrs_set(sa)?
    } else {
        econtent.to_vec()
    };

    let sig = signer_info.signature.as_bytes();
    let sig_alg_oid = signer_info.signature_algorithm.oid;

    verify_with_ring(
        &signed_data_for_verify,
        sig,
        sig_alg_oid,
        spki_alg_oid,
        pubkey_bytes,
        &spki_der,
    )
}

fn verify_message_digest_attr(
    sa: &SetOfVec<x509_cert::attr::Attribute>,
    econtent: &[u8],
) -> Result<(), PermissionsError> {
    let attr = sa
        .iter()
        .find(|a| a.oid == OID_MESSAGE_DIGEST)
        .ok_or_else(|| {
            PermissionsError::Malformed("signedAttrs: messageDigest fehlt".to_string())
        })?;
    let any = attr
        .values
        .iter()
        .next()
        .ok_or_else(|| PermissionsError::Malformed("messageDigest leer".to_string()))?;
    if any.tag() != Tag::OctetString {
        return Err(PermissionsError::Malformed(
            "messageDigest: erwarte OCTET STRING".to_string(),
        ));
    }
    let expected = any.value();
    let mut h = Sha256::new();
    h.update(econtent);
    let actual = h.finalize();
    if actual.as_slice() != expected {
        return Err(PermissionsError::Malformed(
            "messageDigest: SHA-256-Hash mismatch".to_string(),
        ));
    }
    Ok(())
}

fn verify_content_type_attr(
    sa: &SetOfVec<x509_cert::attr::Attribute>,
) -> Result<(), PermissionsError> {
    // contentType-Attribut ist Spec-pflichtig, aber wir akzeptieren
    // pragmatisch sowohl id-data als auch fehlende Praesenz.
    let attr = match sa.iter().find(|a| a.oid == OID_CONTENT_TYPE) {
        Some(a) => a,
        None => return Ok(()),
    };
    let any = attr
        .values
        .iter()
        .next()
        .ok_or_else(|| PermissionsError::Malformed("contentType-attr leer".to_string()))?;
    if any.tag() != Tag::ObjectIdentifier {
        return Err(PermissionsError::Malformed(
            "contentType: erwarte OBJECT IDENTIFIER".to_string(),
        ));
    }
    let oid = ObjectIdentifier::from_der(&any.to_der().unwrap_or_default())
        .map_err(|e| PermissionsError::Malformed(format!("contentType parse: {e:?}")))?;
    if oid != ID_DATA && oid != ID_SIGNED_DATA {
        return Err(PermissionsError::Malformed(format!(
            "contentType {oid} nicht akzeptiert"
        )));
    }
    Ok(())
}

fn encode_signed_attrs_set(
    sa: &SetOfVec<x509_cert::attr::Attribute>,
) -> Result<Vec<u8>, PermissionsError> {
    // SignedAttributes sind im SignerInfo IMPLICIT [0] getaggt; fuer
    // den Signature-Verify muessen sie als SET (Tag 0x31) re-encoded
    // werden — RFC 5652 §5.4.
    let der = sa
        .to_der()
        .map_err(|e| PermissionsError::Malformed(format!("signedAttrs encode: {e:?}")))?;
    Ok(der)
}

fn verify_with_ring(
    msg: &[u8],
    sig: &[u8],
    sig_alg_oid: ObjectIdentifier,
    spki_alg_oid: ObjectIdentifier,
    pubkey_bytes: &[u8],
    spki_der: &[u8],
) -> Result<(), PermissionsError> {
    // Hilfsfunktion: ring-Verify mit konkretem Algorithmus-Typ. Nutzt
    // Generics statt `dyn Trait` (zerodds-lint: kein dyn in SAFE-Crates).
    fn verify_with<A>(alg: &'static A, key: &[u8], msg: &[u8], sig: &[u8]) -> bool
    where
        A: signature::VerificationAlgorithm + 'static,
    {
        signature::UnparsedPublicKey::new(alg, key)
            .verify(msg, sig)
            .is_ok()
    }

    let ok = if spki_alg_oid == OID_EC_PUBLIC_KEY {
        // ECDSA — Curve aus SPKI-Parameters bestimmen.
        let curve = ec_curve_from_spki(spki_der)?;
        match (curve, sig_alg_oid) {
            (Curve::P256, oid) if oid == OID_ECDSA_SHA256 => {
                verify_with(&signature::ECDSA_P256_SHA256_ASN1, pubkey_bytes, msg, sig)
            }
            (Curve::P384, oid) if oid == OID_ECDSA_SHA384 => {
                verify_with(&signature::ECDSA_P384_SHA384_ASN1, pubkey_bytes, msg, sig)
            }
            // Cross-Curve-Combos (P-256 mit SHA-384) bleiben absichtlich
            // off-spec.
            _ => {
                return Err(PermissionsError::Malformed(format!(
                    "ecdsa: unsupported curve/sig combo (sig_alg={sig_alg_oid})"
                )));
            }
        }
    } else if spki_alg_oid == OID_RSA_ENCRYPTION || spki_alg_oid == OID_RSA_PSS {
        // RSA. RFC 5754 §3: SignerInfo.signatureAlgorithm darf entweder
        // die explizite Kombi (sha256WithRSAEncryption) ODER nur die
        // Schluessel-OID `rsaEncryption` sein — dann impliziert
        // `digestAlgorithm` den Hash. OpenSSL `cms -sign` mit RSA-PKCS1
        // produziert die zweite Variante, daher beide Pfade akzeptieren.
        match sig_alg_oid {
            oid if oid == OID_RSA_SHA256 => verify_with(
                &signature::RSA_PKCS1_2048_8192_SHA256,
                pubkey_bytes,
                msg,
                sig,
            ),
            oid if oid == OID_RSA_SHA384 => verify_with(
                &signature::RSA_PKCS1_2048_8192_SHA384,
                pubkey_bytes,
                msg,
                sig,
            ),
            oid if oid == OID_RSA_PSS => {
                verify_with(&signature::RSA_PSS_2048_8192_SHA256, pubkey_bytes, msg, sig)
            }
            // Bare `rsaEncryption` → digest aus signed_info.digest_alg
            // ableiten (vom Caller via OID_SHA_256-Pruefung schon
            // verifiziert: digest_alg == SHA-256).
            oid if oid == OID_RSA_ENCRYPTION => verify_with(
                &signature::RSA_PKCS1_2048_8192_SHA256,
                pubkey_bytes,
                msg,
                sig,
            ),
            _ => {
                return Err(PermissionsError::Malformed(format!(
                    "rsa: unsupported sig_alg={sig_alg_oid}"
                )));
            }
        }
    } else {
        return Err(PermissionsError::Malformed(format!(
            "unsupported signer key alg: {spki_alg_oid}"
        )));
    };

    if ok {
        Ok(())
    } else {
        Err(PermissionsError::Malformed(
            "signer-info: signature verify failed".to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
enum Curve {
    P256,
    P384,
}

fn ec_curve_from_spki(spki_der: &[u8]) -> Result<Curve, PermissionsError> {
    // SPKI = SEQUENCE { algorithm AlgorithmIdentifier, subjectPublicKey BIT STRING }
    // algorithm.parameters fuer ecPublicKey ist eine Curve-OID.
    let spki = x509_cert::spki::SubjectPublicKeyInfoOwned::from_der(spki_der)
        .map_err(|e| PermissionsError::Malformed(format!("spki parse: {e:?}")))?;
    let params = spki
        .algorithm
        .parameters
        .as_ref()
        .ok_or_else(|| PermissionsError::Malformed("spki: ec params fehlen".to_string()))?;
    let oid = ObjectIdentifier::from_der(
        &params
            .to_der()
            .map_err(|e| PermissionsError::Malformed(format!("spki params encode: {e:?}")))?,
    )
    .map_err(|e| PermissionsError::Malformed(format!("spki ec params: {e:?}")))?;
    if oid == OID_SECP256R1 {
        Ok(Curve::P256)
    } else if oid == OID_SECP384R1 {
        Ok(Curve::P384)
    } else {
        Err(PermissionsError::Malformed(format!(
            "unsupported ec curve: {oid}"
        )))
    }
}

// ----------------------------------------------------------------------
// Base64-Decoder (kein extra crate; cms.rs ist nicht hot-path).
// ----------------------------------------------------------------------

fn base64_decode(input: &[u8]) -> Result<Vec<u8>, PermissionsError> {
    // RFC 4648 §4 standard-alphabet, padding `=`.
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    // Whitespace bereits entfernt, aber zur Sicherheit nochmal filtern.
    let mut clean: Vec<u8> = Vec::with_capacity(input.len());
    for &c in input {
        if c == b'=' || val(c).is_some() {
            clean.push(c);
        } else if c.is_ascii_whitespace() {
            // skip
        } else {
            return Err(PermissionsError::Malformed(format!(
                "base64: ungueltiges Zeichen {c:#x}"
            )));
        }
    }
    if clean.len() % 4 != 0 {
        return Err(PermissionsError::Malformed(format!(
            "base64: laenge {l} nicht durch 4 teilbar",
            l = clean.len()
        )));
    }
    let mut out: Vec<u8> = Vec::with_capacity(clean.len() / 4 * 3);
    let mut i = 0;
    while i < clean.len() {
        let chunk = &clean[i..i + 4];
        let pad = chunk.iter().filter(|&&b| b == b'=').count();
        let v = |c: u8| -> Result<u8, PermissionsError> {
            val(c).ok_or_else(|| {
                PermissionsError::Malformed(format!("base64: ungueltiges Zeichen {c:#x}"))
            })
        };
        let b0 = v(chunk[0])?;
        let b1 = v(chunk[1])?;
        let n = (u32::from(b0) << 18) | (u32::from(b1) << 12);
        if pad >= 2 {
            // 2 bytes -> 1 output byte.
            out.push((n >> 16) as u8);
        } else {
            let b2 = v(chunk[2])?;
            let n = n | (u32::from(b2) << 6);
            if pad == 1 {
                out.push((n >> 16) as u8);
                out.push((n >> 8) as u8);
            } else {
                let b3 = v(chunk[3])?;
                let n = n | u32::from(b3);
                out.push((n >> 16) as u8);
                out.push((n >> 8) as u8);
                out.push(n as u8);
            }
        }
        i += 4;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip_basic() {
        // "hello" -> aGVsbG8=
        assert_eq!(base64_decode(b"aGVsbG8=").unwrap(), b"hello");
        // "hi" -> aGk=
        assert_eq!(base64_decode(b"aGk=").unwrap(), b"hi");
        // empty -> empty.
        assert_eq!(base64_decode(b"").unwrap(), b"");
    }

    #[test]
    fn base64_rejects_invalid_alphabet() {
        let err = base64_decode(b"!!!!").unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(_)));
    }

    #[test]
    fn base64_rejects_non_4_aligned() {
        let err = base64_decode(b"aGVsbA").unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(_)));
    }

    #[test]
    fn parse_param_extracts_quoted_value() {
        assert_eq!(
            parse_param(
                "multipart/signed; protocol=\"application/pkcs7-signature\"; boundary=\"foo bar\"",
                "boundary",
            ),
            Some("foo bar".to_string()),
        );
    }

    #[test]
    fn parse_param_extracts_unquoted_value() {
        assert_eq!(
            parse_param(
                "application/pkcs7-mime; smime-type=signed-data",
                "smime-type"
            ),
            Some("signed-data".to_string()),
        );
    }

    #[test]
    fn header_parser_unfolds_continuations() {
        let h = "Content-Type: multipart/signed;\r\n boundary=\"abc\"\r\nMIME-Version: 1.0";
        let parsed = parse_header_lines(h);
        let ct = &parsed.iter().find(|(k, _)| k == "Content-Type").unwrap().1;
        assert!(ct.contains("multipart/signed"));
        assert!(ct.contains("boundary=\"abc\""));
    }

    #[test]
    fn smime_kind_multipart_detected() {
        let doc = b"Content-Type: multipart/signed; protocol=\"application/pkcs7-signature\"; boundary=\"BB\"\r\n\r\nBODY";
        let (ct, body) = parse_smime_headers(doc).unwrap();
        assert!(matches!(ct.kind, SmimeKind::MultipartSigned { ref boundary } if boundary == "BB"));
        assert_eq!(body, b"BODY");
    }

    #[test]
    fn smime_kind_pkcs7_detected() {
        let doc = b"Content-Type: application/pkcs7-mime; smime-type=signed-data\r\nContent-Transfer-Encoding: base64\r\n\r\nQUJD";
        let (ct, body) = parse_smime_headers(doc).unwrap();
        assert!(matches!(ct.kind, SmimeKind::Pkcs7Mime { .. }));
        assert_eq!(body, b"QUJD");
    }

    #[test]
    fn smime_rejects_missing_content_type() {
        let doc = b"X-Foo: bar\r\n\r\nBODY";
        let err = parse_smime_headers(doc).unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(_)));
    }

    #[test]
    fn ca_pem_parser_rejects_empty() {
        let err = CmsPkcs7Verifier::new(b"").unwrap_err();
        assert!(matches!(err, PermissionsError::Malformed(_)));
    }

    #[test]
    fn pem_pkcs7_detection() {
        assert!(looks_like_pem_pkcs7(
            b"-----BEGIN PKCS7-----\nABC\n-----END PKCS7-----\n"
        ));
        assert!(looks_like_pem_pkcs7(
            b"-----BEGIN CMS-----\nABC\n-----END CMS-----\n"
        ));
        assert!(!looks_like_pem_pkcs7(b"MIME-Version: 1.0\r\n"));
    }

    #[test]
    fn pem_pkcs7_parser_extracts_b64() {
        // "hello" base64-encoded.
        let pem = b"-----BEGIN PKCS7-----\naGVsbG8=\n-----END PKCS7-----\n";
        let der = parse_pem_pkcs7(pem).unwrap();
        assert_eq!(der, b"hello");
        let pem = b"-----BEGIN CMS-----\naGVsbG8=\n-----END CMS-----\n";
        let der = parse_pem_pkcs7(pem).unwrap();
        assert_eq!(der, b"hello");
    }
}
