// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MTOM/XOP — Message Transmission Optimization Mechanism.
//!
//! W3C MTOM Recommendation (`http://www.w3.org/2005/REC-soap12-mtom-20050125/`)
//! + XOP (`http://www.w3.org/TR/xop10/`).
//!
//! Erlaubt das Anhaengen binaerer Daten an SOAP-Messages via
//! MIME multipart/related container instead of Base64 inlining.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Eine MTOM/XOP-Attachment-Part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// Content-Id (z.B. `<image1@demo>`).
    pub content_id: String,
    /// MIME-Type (z.B. `image/png`).
    pub content_type: String,
    /// Bytes.
    pub bytes: Vec<u8>,
}

/// Komplette MTOM-Message: SOAP-Envelope + Attachments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtomMessage {
    /// Boundary string for the multipart container.
    pub boundary: String,
    /// SOAP-Envelope-XML (Root-Part).
    pub envelope_xml: String,
    /// Attachment list.
    pub attachments: Vec<Attachment>,
}

/// Encodes an MTOM message into a multipart/related body.
/// Spec MTOM §4.
#[must_use]
pub fn encode_mtom_packaging(msg: &MtomMessage) -> String {
    let mut out = String::new();
    let b = &msg.boundary;
    // Root-Part — application/xop+xml.
    out.push_str(&format!("--{b}\r\n"));
    out.push_str(
        "Content-Type: application/xop+xml; charset=UTF-8; type=\"application/soap+xml\"\r\n",
    );
    out.push_str("Content-Transfer-Encoding: 8bit\r\n");
    out.push_str("Content-ID: <root.message@zerodds-soap>\r\n\r\n");
    out.push_str(&msg.envelope_xml);
    out.push_str("\r\n");
    // Attachment-Parts.
    for att in &msg.attachments {
        out.push_str(&format!("--{b}\r\n"));
        out.push_str(&format!("Content-Type: {}\r\n", att.content_type));
        out.push_str("Content-Transfer-Encoding: binary\r\n");
        out.push_str(&format!("Content-ID: {}\r\n\r\n", att.content_id));
        // We can't push raw bytes through String, so we use base64-equivalent
        // representation: hex. The caller layer can substitute the real
        // binary serialization on demand.
        out.push_str(&hex_encode(&att.bytes));
        out.push_str("\r\n");
    }
    out.push_str(&format!("--{b}--\r\n"));
    out
}

/// Creates a `<xop:Include href="cid:...">` element reference in
/// a body XML where a binary field would be inlined.
/// Spec XOP §4.1.
#[must_use]
pub fn xop_include(content_id: &str) -> String {
    format!(
        r#"<xop:Include xmlns:xop="http://www.w3.org/2004/08/xop/include" href="cid:{}"/>"#,
        content_id.trim_start_matches('<').trim_end_matches('>')
    )
}

fn hex_encode(b: &[u8]) -> String {
    let mut out = String::with_capacity(b.len() * 2);
    for byte in b {
        let hi = (byte >> 4) & 0x0f;
        let lo = byte & 0x0f;
        out.push(hex_digit(hi));
        out.push(hex_digit(lo));
    }
    out
}

fn hex_digit(n: u8) -> char {
    match n {
        0..=9 => char::from(b'0' + n),
        10..=15 => char::from(b'A' + n - 10),
        _ => '?',
    }
}

impl MtomMessage {
    /// Constructor with a default boundary.
    #[must_use]
    pub fn new(envelope_xml: String) -> Self {
        Self {
            boundary: "MIME_boundary_dds_soap".to_string(),
            envelope_xml,
            attachments: Vec::new(),
        }
    }

    /// Add an attachment.
    pub fn attach(&mut self, att: Attachment) {
        self.attachments.push(att);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn sample_attachment() -> Attachment {
        Attachment {
            content_id: "<image1@demo>".into(),
            content_type: "image/png".into(),
            bytes: alloc::vec![0xCA, 0xFE, 0xBA, 0xBE],
        }
    }

    #[test]
    fn package_starts_with_xop_root_part() {
        let mut msg = MtomMessage::new("<env/>".into());
        msg.attach(sample_attachment());
        let s = encode_mtom_packaging(&msg);
        assert!(s.contains("application/xop+xml"));
        assert!(s.contains("Content-ID: <root.message@zerodds-soap>"));
    }

    #[test]
    fn package_includes_attachment_part() {
        let mut msg = MtomMessage::new("<env/>".into());
        msg.attach(sample_attachment());
        let s = encode_mtom_packaging(&msg);
        assert!(s.contains("Content-Type: image/png"));
        assert!(s.contains("Content-ID: <image1@demo>"));
        assert!(s.contains("CAFEBABE"));
    }

    #[test]
    fn package_terminates_with_closing_boundary() {
        let msg = MtomMessage::new("<env/>".into());
        let s = encode_mtom_packaging(&msg);
        assert!(s.ends_with("--MIME_boundary_dds_soap--\r\n"));
    }

    #[test]
    fn xop_include_strips_angle_brackets() {
        let s = xop_include("<image1@demo>");
        assert!(s.contains("cid:image1@demo"));
        assert!(!s.contains("<image1"));
    }

    #[test]
    fn xop_include_namespace_is_correct() {
        let s = xop_include("x");
        assert!(s.contains("http://www.w3.org/2004/08/xop/include"));
    }

    #[test]
    fn empty_attachments_still_produce_root_part() {
        let msg = MtomMessage::new("<env/>".into());
        let s = encode_mtom_packaging(&msg);
        assert!(s.contains("application/xop+xml"));
        assert!(s.contains("<env/>"));
    }
}
