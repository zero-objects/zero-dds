// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SOAP-Fault — W3C SOAP 1.2 Part 1 §5.4.
//!
//! Spec §5.4: SOAP-Fault hat fuenf Standard-Codes:
//! `VersionMismatch`, `MustUnderstand`, `DataEncodingUnknown`,
//! `Sender` (Client-Side-Error), `Receiver` (Server-Side-Error).

use alloc::format;
use alloc::string::{String, ToString};

use crate::envelope::SOAP_12_NS;

/// SOAP 1.2 Fault-Code (W3C SOAP 1.2 §5.4.1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultCode {
    /// `env:VersionMismatch`.
    VersionMismatch,
    /// `env:MustUnderstand`.
    MustUnderstand,
    /// `env:DataEncodingUnknown`.
    DataEncodingUnknown,
    /// `env:Sender` (Client-Side-Error, prev. SOAP 1.1 `Client`).
    Sender,
    /// `env:Receiver` (Server-Side-Error, prev. SOAP 1.1 `Server`).
    Receiver,
}

impl FaultCode {
    /// Spec-Name als XML-QName (`env:<Code>`).
    #[must_use]
    pub fn qname(self) -> &'static str {
        match self {
            Self::VersionMismatch => "env:VersionMismatch",
            Self::MustUnderstand => "env:MustUnderstand",
            Self::DataEncodingUnknown => "env:DataEncodingUnknown",
            Self::Sender => "env:Sender",
            Self::Receiver => "env:Receiver",
        }
    }
}

/// SOAP 1.2 Fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// Fault-Code.
    pub code: FaultCode,
    /// Reason — kurze human-readable Beschreibung.
    pub reason: String,
    /// Optional Detail-XML (Subcode-/App-spezifisch).
    pub detail: Option<String>,
    /// Optional `xml:lang` fuer den Reason-Text (default `en`).
    pub lang: String,
}

impl Fault {
    /// Konstruktor mit Default-Lang `en`.
    #[must_use]
    pub fn new(code: FaultCode, reason: &str) -> Self {
        Self {
            code,
            reason: reason.into(),
            detail: None,
            lang: "en".to_string(),
        }
    }

    /// Render zu XML — wird typisch in `Envelope.body_xml` verbaut.
    /// Spec §5.4 Skeleton.
    #[must_use]
    pub fn to_xml(&self) -> String {
        let detail = match &self.detail {
            Some(d) => format!("<env:Detail>{d}</env:Detail>"),
            None => String::new(),
        };
        format!(
            "<env:Fault xmlns:env=\"{SOAP_12_NS}\">\
<env:Code><env:Value>{}</env:Value></env:Code>\
<env:Reason><env:Text xml:lang=\"{}\">{}</env:Text></env:Reason>\
{detail}\
</env:Fault>",
            self.code.qname(),
            self.lang,
            xml_escape(&self.reason)
        )
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn fault_codes_have_correct_qnames() {
        assert_eq!(FaultCode::Sender.qname(), "env:Sender");
        assert_eq!(FaultCode::Receiver.qname(), "env:Receiver");
        assert_eq!(FaultCode::VersionMismatch.qname(), "env:VersionMismatch");
        assert_eq!(FaultCode::MustUnderstand.qname(), "env:MustUnderstand");
        assert_eq!(
            FaultCode::DataEncodingUnknown.qname(),
            "env:DataEncodingUnknown"
        );
    }

    #[test]
    fn fault_xml_contains_required_elements() {
        let f = Fault::new(FaultCode::Sender, "Bad input");
        let xml = f.to_xml();
        assert!(xml.contains("<env:Code>"));
        assert!(xml.contains("<env:Value>env:Sender</env:Value>"));
        assert!(xml.contains("<env:Reason>"));
        assert!(xml.contains("<env:Text xml:lang=\"en\">Bad input</env:Text>"));
    }

    #[test]
    fn fault_with_detail_includes_detail_block() {
        let mut f = Fault::new(FaultCode::Receiver, "Server error");
        f.detail = Some("<x:Cause>db down</x:Cause>".into());
        let xml = f.to_xml();
        assert!(xml.contains("<env:Detail><x:Cause>db down</x:Cause></env:Detail>"));
    }

    #[test]
    fn fault_reason_is_xml_escaped() {
        let f = Fault::new(FaultCode::Sender, "<bad> & worse");
        let xml = f.to_xml();
        assert!(xml.contains("&lt;bad&gt; &amp; worse"));
        assert!(!xml.contains("<bad>"));
    }

    #[test]
    fn fault_default_lang_is_en() {
        let f = Fault::new(FaultCode::Sender, "x");
        assert_eq!(f.lang, "en");
        assert!(f.to_xml().contains("xml:lang=\"en\""));
    }

    #[test]
    fn custom_lang_is_emitted() {
        let mut f = Fault::new(FaultCode::Sender, "x");
        f.lang = "de".into();
        assert!(f.to_xml().contains("xml:lang=\"de\""));
    }
}
