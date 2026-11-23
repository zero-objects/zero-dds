// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP Content-Formats: the initial entries of the registry (RFC 7252
//! §12.3, Table 9) and the bridge's own in the range 65000 to 65535 that
//! §12.3 leaves for experimental use (spec `zerodds-coap-bridge-1.0` §4.5).

/// `text/plain; charset=utf-8` (RFC 2046, RFC 3676, RFC 5147).
pub const TEXT_PLAIN_UTF8: u16 = 0;
/// `application/link-format` (RFC 6690).
pub const LINK_FORMAT: u16 = 40;
/// `application/xml` (RFC 3023).
pub const XML: u16 = 41;
/// `application/octet-stream` (RFC 2045, RFC 2046).
pub const OCTET_STREAM: u16 = 42;
/// `application/exi` (REC-exi-20140211).
pub const EXI: u16 = 47;
/// `application/json` (RFC 7159).
pub const JSON: u16 = 50;
/// `application/cbor` (RFC 7049; registered by RFC 7049, not by RFC 7252).
pub const CBOR: u16 = 60;
/// `application/x-dds-cdr2; bo=le`: an XCDR2 sample, little endian.
pub const XCDR2_LE: u16 = 65000;
/// `application/x-dds-cdr2; bo=be`: an XCDR2 sample, big endian.
pub const XCDR2_BE: u16 = 65001;
/// `application/x-dds-cdr1; bo=le`: an XCDR1 sample, little endian.
pub const XCDR1_LE: u16 = 65002;

/// The media type of a Content-Format this crate knows.
#[must_use]
pub const fn media_type(content_format: u16) -> Option<&'static str> {
    match content_format {
        TEXT_PLAIN_UTF8 => Some("text/plain; charset=utf-8"),
        LINK_FORMAT => Some("application/link-format"),
        XML => Some("application/xml"),
        OCTET_STREAM => Some("application/octet-stream"),
        EXI => Some("application/exi"),
        JSON => Some("application/json"),
        CBOR => Some("application/cbor"),
        XCDR2_LE => Some("application/x-dds-cdr2; bo=le"),
        XCDR2_BE => Some("application/x-dds-cdr2; bo=be"),
        XCDR1_LE => Some("application/x-dds-cdr1; bo=le"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 7252 §12.3 Table 9: the initial entries of the registry.
    #[test]
    fn coap_12_3_the_initial_registry_entries() {
        for (id, media) in [
            (0, "text/plain; charset=utf-8"),
            (40, "application/link-format"),
            (41, "application/xml"),
            (42, "application/octet-stream"),
            (47, "application/exi"),
            (50, "application/json"),
        ] {
            assert_eq!(media_type(id), Some(media), "Content-Format {id}");
        }
        assert_eq!(
            [TEXT_PLAIN_UTF8, LINK_FORMAT, XML, OCTET_STREAM, EXI, JSON],
            [0, 40, 41, 42, 47, 50]
        );
        assert_eq!(media_type(1), None);
    }

    /// The bridge's own Content-Formats lie in the experimental range of
    /// RFC 7252 §12.3 (65000 to 65535).
    #[test]
    fn coap_12_3_the_bridges_formats_are_in_the_experimental_range() {
        for id in [XCDR2_LE, XCDR2_BE, XCDR1_LE] {
            assert!(id >= 65000);
            assert!(media_type(id).is_some());
        }
    }
}
