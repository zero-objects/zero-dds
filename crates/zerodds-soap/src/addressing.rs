// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WS-Addressing 1.0 — W3C 2006 Recommendation.
//!
//! Spec: `http://www.w3.org/TR/ws-addr-core/`.
//! Namespace: `http://www.w3.org/2005/08/addressing` (`wsa:`).

use alloc::format;
use alloc::string::String;

/// WS-Addressing-Namespace 1.0.
pub const WSA_NS: &str = "http://www.w3.org/2005/08/addressing";

/// WS-Addressing-Header. Spec §3.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AddressingHeaders {
    /// `wsa:To` — Endpoint-Reference des Empfaengers.
    pub to: Option<String>,
    /// `wsa:Action` — Operation-IRI.
    pub action: Option<String>,
    /// `wsa:MessageID` — eindeutige Message-Id.
    pub message_id: Option<String>,
    /// `wsa:RelatesTo` — bezieht sich auf eine vorherige Message-Id
    /// (typisch fuer Reply).
    pub relates_to: Option<String>,
    /// `wsa:From` — Source-Endpoint.
    pub from: Option<String>,
    /// `wsa:ReplyTo` — Endpoint, an den die Reply geschickt werden
    /// soll.
    pub reply_to: Option<String>,
    /// `wsa:FaultTo` — Endpoint fuer Faults.
    pub fault_to: Option<String>,
}

/// Erzeugt einen `<wsa:...>`-Header-Block fuer den SOAP-Envelope.
/// Spec §3.2.
#[must_use]
pub fn build_addressing_header(h: &AddressingHeaders) -> String {
    let mut out = String::new();
    if let Some(to) = &h.to {
        out.push_str(&format!("<wsa:To xmlns:wsa=\"{WSA_NS}\">{to}</wsa:To>"));
    }
    if let Some(action) = &h.action {
        out.push_str(&format!(
            "<wsa:Action xmlns:wsa=\"{WSA_NS}\">{action}</wsa:Action>"
        ));
    }
    if let Some(mid) = &h.message_id {
        out.push_str(&format!(
            "<wsa:MessageID xmlns:wsa=\"{WSA_NS}\">{mid}</wsa:MessageID>"
        ));
    }
    if let Some(r) = &h.relates_to {
        out.push_str(&format!(
            "<wsa:RelatesTo xmlns:wsa=\"{WSA_NS}\">{r}</wsa:RelatesTo>"
        ));
    }
    if let Some(f) = &h.from {
        out.push_str(&format!(
            "<wsa:From xmlns:wsa=\"{WSA_NS}\"><wsa:Address>{f}</wsa:Address></wsa:From>"
        ));
    }
    if let Some(r) = &h.reply_to {
        out.push_str(&format!(
            "<wsa:ReplyTo xmlns:wsa=\"{WSA_NS}\"><wsa:Address>{r}</wsa:Address></wsa:ReplyTo>"
        ));
    }
    if let Some(f) = &h.fault_to {
        out.push_str(&format!(
            "<wsa:FaultTo xmlns:wsa=\"{WSA_NS}\"><wsa:Address>{f}</wsa:Address></wsa:FaultTo>"
        ));
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn empty_headers_produce_empty_string() {
        let h = AddressingHeaders::default();
        assert!(build_addressing_header(&h).is_empty());
    }

    #[test]
    fn to_and_action_are_emitted() {
        let h = AddressingHeaders {
            to: Some("http://server/ep".into()),
            action: Some("http://demo/Action/Echo".into()),
            ..AddressingHeaders::default()
        };
        let out = build_addressing_header(&h);
        assert!(out.contains("<wsa:To"));
        assert!(out.contains("http://server/ep"));
        assert!(out.contains("<wsa:Action"));
        assert!(out.contains("http://demo/Action/Echo"));
    }

    #[test]
    fn message_id_and_relates_to_round_trip() {
        let h = AddressingHeaders {
            message_id: Some("uuid:1".into()),
            relates_to: Some("uuid:0".into()),
            ..AddressingHeaders::default()
        };
        let out = build_addressing_header(&h);
        assert!(out.contains("uuid:1"));
        assert!(out.contains("uuid:0"));
    }

    #[test]
    fn reply_to_and_fault_to_emit_address_subelement() {
        let h = AddressingHeaders {
            reply_to: Some("http://client/reply".into()),
            fault_to: Some("http://client/fault".into()),
            ..AddressingHeaders::default()
        };
        let out = build_addressing_header(&h);
        assert!(out.contains("<wsa:ReplyTo"));
        assert!(out.contains("<wsa:Address>http://client/reply</wsa:Address>"));
        assert!(out.contains("<wsa:FaultTo"));
        assert!(out.contains("<wsa:Address>http://client/fault</wsa:Address>"));
    }

    #[test]
    fn ns_constant_is_w3c_2005_08() {
        assert_eq!(WSA_NS, "http://www.w3.org/2005/08/addressing");
    }
}
