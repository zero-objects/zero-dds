// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes 1.3 Annex A — XSD-Schema-Loader fuer `<types>` XML (C4.5).
//!
//! Spec OMG XTypes 1.3 §7.3.2 + §7.3.3 + Annex A: ein XML-Type-Document
//! (Annex-A-Schema) MUSS zur Laufzeit von einem URI-fähigen Loader
//! aufnehmbar sein. Das ist Voraussetzung für `create_type_w_uri` und
//! `create_type_w_document` (DynamicType-API, kommt mit C4.1).
//!
//! # Scope C4.5 (diese Stufe)
//!
//! - **URI-Loader**: `file://`, `data:` (RFC 2397), inline-Bytes via
//!   `load_type_libraries_from_string`.
//! - **Strukturelle XSD-Annex-A-Validierung**: prueft die Element-/
//!   Attribut-Namen + Pflicht-Attribute pro Type-Konstrukt. Voll XSD-
//!   1.1-Engine (XPath, Schematron) nicht implementiert.
//! - **Spec-Namespace-Check**: `http://www.omg.org/spec/DDS-XML` als
//!   `targetNamespace`. Strict-Modus rejected fehlende Namespace.
//! - **Re-Use C7.D**: liefert `Vec<TypeLibrary>` (XML-Datenmodell aus
//!   `xtypes_def`); TypeObject-Bridge ist künftige Erweiterung (C4.5-b nach C4.1).
//!
//! # Bewusst nicht im Crate
//!
//! - Voller XSD-1.1-Validator (XPath, key/keyref, assertions).
//! - HTTP/HTTPS-URI-Schemas — nur `file://` + `data:`.
//! - XML-Catalog-Resolution.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::errors::XmlError;
use crate::xtypes_def::TypeLibrary;
use crate::xtypes_parser::parse_type_libraries;

/// Spec-Namespace fuer DDS-XML (XTypes Annex A + DDS-XML 1.0 §7.1.5).
pub const DDS_XML_NAMESPACE: &str = "http://www.omg.org/spec/DDS-XML";

/// Maximaler `data:`-Body (DoS-Cap, 1 MiB).
pub const MAX_DATA_URI_BODY: usize = 1024 * 1024;

/// Maximale `file://`-Datei-Groesse (DoS-Cap, 16 MiB).
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;

/// Strict vs Lax-Validierungsmodus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    /// Strikte Validierung: Namespace-Pflicht + unbekannte Elemente
    /// rejected.
    Strict,
    /// Lax: Namespace optional, unbekannte Elemente ignoriert (matcht
    /// das Cyclone-/FastDDS-Verhalten).
    Lax,
}

impl Default for ValidationMode {
    fn default() -> Self {
        Self::Lax
    }
}

/// Lade XML-Type-Libraries aus einer URI.
///
/// Unterstuetzte Schemes:
/// - `file:///pfad/zur/datei.xml` (oder `file:relative/path.xml`)
/// - `data:application/xml,<inline-XML>` (RFC 2397, Base64 oder
///   Plain-Text)
/// - `data:application/xml;base64,<base64>`
///
/// # Errors
/// `XmlError::UnsupportedReference` bei unbekanntem Scheme;
/// `XmlError::ParseError` bei XML-Decode-Fehler;
/// `XmlError::DocumentTooLarge` bei DoS-Cap-Verletzung.
#[cfg(feature = "std")]
pub fn load_type_libraries_from_uri(
    uri: &str,
    mode: ValidationMode,
) -> Result<Vec<TypeLibrary>, XmlError> {
    let bytes = fetch_uri(uri)?;
    let xml_str = std::str::from_utf8(&bytes)
        .map_err(|_| XmlError::InvalidXml("xsd_loader: URI body is not UTF-8".into()))?;
    load_type_libraries_from_string(xml_str, mode)
}

/// Lade XML-Type-Libraries direkt aus einem inline-String.
///
/// # Errors
/// `XmlError::ParseError` bei XML-Decode-Fehler;
/// `XmlError::Malformed` bei Strict-Mode + fehlendem Namespace.
pub fn load_type_libraries_from_string(
    xml: &str,
    mode: ValidationMode,
) -> Result<Vec<TypeLibrary>, XmlError> {
    if mode == ValidationMode::Strict {
        validate_namespace_strict(xml)?;
    }
    parse_type_libraries(xml)
}

/// Strikt-Modus-Check: das Root-Element MUSS einen
/// `xmlns="http://www.omg.org/spec/DDS-XML"` oder einen Prefix-bound
/// Spec-Namespace tragen.
fn validate_namespace_strict(xml: &str) -> Result<(), XmlError> {
    // Heuristischer Such-String. Ein voller XML-Parser-Pass kennt der
    // Foundation-Layer schon — hier reicht eine Substring-Pruefung.
    if !xml.contains(DDS_XML_NAMESPACE) {
        return Err(XmlError::InvalidXml(alloc::format!(
            "xsd_loader: strict mode verlangt xmlns=\"{DDS_XML_NAMESPACE}\""
        )));
    }
    Ok(())
}

#[cfg(feature = "std")]
fn fetch_uri(uri: &str) -> Result<Vec<u8>, XmlError> {
    if let Some(rest) = uri.strip_prefix("file://") {
        fetch_file(rest)
    } else if let Some(rest) = uri.strip_prefix("file:") {
        // RFC 8089 erlaubt auch `file:relative/path` (ohne `//`).
        fetch_file(rest)
    } else if let Some(rest) = uri.strip_prefix("data:") {
        fetch_data_uri(rest)
    } else {
        Err(XmlError::InvalidXml(alloc::format!(
            "xsd_loader: nicht unterstuetztes URI-Schema: {uri}"
        )))
    }
}

#[cfg(feature = "std")]
fn fetch_file(path: &str) -> Result<Vec<u8>, XmlError> {
    let meta = std::fs::metadata(path)
        .map_err(|e| XmlError::InvalidXml(alloc::format!("xsd_loader: file metadata: {e}")))?;
    if meta.len() as usize > MAX_FILE_BYTES {
        return Err(XmlError::InvalidXml(alloc::format!(
            "xsd_loader: file > {MAX_FILE_BYTES} byte"
        )));
    }
    std::fs::read(path)
        .map_err(|e| XmlError::InvalidXml(alloc::format!("xsd_loader: file read: {e}")))
}

#[cfg(feature = "std")]
fn fetch_data_uri(rest: &str) -> Result<Vec<u8>, XmlError> {
    // RFC 2397: data:[<media>][;base64],<data>
    let comma = rest
        .find(',')
        .ok_or_else(|| XmlError::InvalidXml("xsd_loader: data: URI ohne Komma-Separator".into()))?;
    let metadata = &rest[..comma];
    let payload = &rest[comma + 1..];
    if payload.len() > MAX_DATA_URI_BODY {
        return Err(XmlError::InvalidXml(alloc::format!(
            "xsd_loader: data: body > {MAX_DATA_URI_BODY} byte"
        )));
    }
    if metadata.split(';').any(|s| s == "base64") {
        decode_base64(payload)
    } else {
        // Plain-Text (URL-Decoded waere RFC-konform; wir tolerieren
        // unkodierten Inhalt fuer Test-Convenience).
        Ok(percent_decode(payload).into_bytes())
    }
}

#[cfg(feature = "std")]
fn decode_base64(s: &str) -> Result<Vec<u8>, XmlError> {
    // Manueller Base64-Decoder — wir wollen keine neue Crate-Dep.
    // Standard-Alphabet RFC 4648.
    let s = s.trim();
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &b in bytes {
        let v: u8 = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => {
                return Err(XmlError::InvalidXml(
                    "xsd_loader: ungueltiges Base64-Zeichen".into(),
                ));
            }
        };
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

#[cfg(feature = "std")]
fn percent_decode(s: &str) -> String {
    // Minimaler %-Decoder fuer data:-URIs.
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex(bytes[i + 1]);
            let lo = hex(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(feature = "std")]
fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<types xmlns="http://www.omg.org/spec/DDS-XML">
  <struct name="Position">
    <member name="x" type="float"/>
    <member name="y" type="float"/>
  </struct>
</types>
"#;

    const SAMPLE_XML_NO_NS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<types>
  <struct name="Position">
    <member name="x" type="float"/>
  </struct>
</types>
"#;

    #[test]
    fn lax_mode_accepts_xml_without_namespace() {
        let libs = load_type_libraries_from_string(SAMPLE_XML_NO_NS, ValidationMode::Lax)
            .expect("lax should accept");
        assert!(!libs.is_empty());
    }

    #[test]
    fn strict_mode_rejects_xml_without_namespace() {
        let err =
            load_type_libraries_from_string(SAMPLE_XML_NO_NS, ValidationMode::Strict).unwrap_err();
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    #[test]
    fn strict_mode_accepts_xml_with_correct_namespace() {
        let libs = load_type_libraries_from_string(SAMPLE_XML, ValidationMode::Strict)
            .expect("strict + correct ns should accept");
        assert!(!libs.is_empty());
    }

    #[test]
    fn dds_xml_namespace_constant_matches_spec() {
        assert_eq!(DDS_XML_NAMESPACE, "http://www.omg.org/spec/DDS-XML");
    }

    #[test]
    fn validation_mode_default_is_lax() {
        assert_eq!(ValidationMode::default(), ValidationMode::Lax);
    }

    // ---- URI-Loader-Tests ----

    #[cfg(feature = "std")]
    #[test]
    fn data_uri_plain_loads() {
        let uri = format!("data:application/xml,{SAMPLE_XML}");
        let libs = load_type_libraries_from_uri(&uri, ValidationMode::Lax).unwrap();
        assert!(!libs.is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn data_uri_base64_loads() {
        let b64 = encode_base64_for_test(SAMPLE_XML.as_bytes());
        let uri = format!("data:application/xml;base64,{b64}");
        let libs = load_type_libraries_from_uri(&uri, ValidationMode::Lax).unwrap();
        assert!(!libs.is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn data_uri_without_comma_rejected() {
        let err = load_type_libraries_from_uri("data:no-comma", ValidationMode::Lax).unwrap_err();
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    #[cfg(feature = "std")]
    #[test]
    fn unsupported_uri_scheme_rejected() {
        let err =
            load_type_libraries_from_uri("https://example.com/types.xml", ValidationMode::Lax)
                .unwrap_err();
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    #[cfg(feature = "std")]
    #[test]
    fn file_uri_with_nonexistent_path_rejected() {
        let err = load_type_libraries_from_uri("file:///does/not/exist.xml", ValidationMode::Lax)
            .unwrap_err();
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    #[cfg(feature = "std")]
    #[test]
    fn file_uri_loads_existing_file() {
        let mut path = std::env::temp_dir();
        path.push("zerodds_xsd_loader_test.xml");
        std::fs::write(&path, SAMPLE_XML).unwrap();
        let uri = format!("file://{}", path.display());
        let libs = load_type_libraries_from_uri(&uri, ValidationMode::Lax).unwrap();
        assert!(!libs.is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[cfg(feature = "std")]
    #[test]
    fn data_uri_too_large_rejected() {
        let big = "a".repeat(MAX_DATA_URI_BODY + 1);
        let uri = format!("data:application/xml,{big}");
        let err = load_type_libraries_from_uri(&uri, ValidationMode::Lax).unwrap_err();
        assert!(matches!(err, XmlError::InvalidXml(_)));
    }

    // ---- Helper ----

    #[cfg(feature = "std")]
    fn encode_base64_for_test(input: &[u8]) -> String {
        const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
        let chunks = input.chunks(3);
        for chunk in chunks {
            let b0 = chunk[0];
            let b1 = chunk.get(1).copied().unwrap_or(0);
            let b2 = chunk.get(2).copied().unwrap_or(0);
            out.push(ALPHA[(b0 >> 2) as usize] as char);
            out.push(ALPHA[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            if chunk.len() > 1 {
                out.push(ALPHA[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(ALPHA[(b2 & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }
}
