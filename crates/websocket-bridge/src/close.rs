// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Close-frame status codes — RFC 6455 §7.4.

use alloc::string::String;
use alloc::vec::Vec;

/// Close-Status-Code (RFC 6455 §7.4.1 + §7.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CloseCode {
    /// `1000` Normal Closure.
    Normal = 1000,
    /// `1001` Going Away.
    GoingAway = 1001,
    /// `1002` Protocol Error.
    ProtocolError = 1002,
    /// `1003` Unsupported Data.
    UnsupportedData = 1003,
    /// `1005` No Status Received (reserved, NOT on the wire).
    NoStatusReceived = 1005,
    /// `1006` Abnormal Closure (reserved, NOT on the wire).
    AbnormalClosure = 1006,
    /// `1007` Invalid Frame Payload Data (UTF-8).
    InvalidPayloadData = 1007,
    /// `1008` Policy Violation.
    PolicyViolation = 1008,
    /// `1009` Message Too Big.
    MessageTooBig = 1009,
    /// `1010` Mandatory Extension (Client only).
    MandatoryExtension = 1010,
    /// `1011` Internal Error.
    InternalError = 1011,
    /// `1012` Service Restart.
    ServiceRestart = 1012,
    /// `1013` Try Again Later.
    TryAgainLater = 1013,
    /// `1014` Bad Gateway.
    BadGateway = 1014,
    /// `1015` TLS Handshake Failure (reserved, NOT on the wire).
    TlsHandshakeFailure = 1015,
}

impl CloseCode {
    /// Wire value.
    #[must_use]
    pub const fn to_u16(self) -> u16 {
        self as u16
    }

    /// `u16 -> CloseCode`.
    ///
    /// # Errors
    /// `()` if the code is unknown or reserved-but-not-handled.
    #[allow(clippy::result_unit_err)]
    pub const fn from_u16(v: u16) -> Result<Self, ()> {
        match v {
            1000 => Ok(Self::Normal),
            1001 => Ok(Self::GoingAway),
            1002 => Ok(Self::ProtocolError),
            1003 => Ok(Self::UnsupportedData),
            1005 => Ok(Self::NoStatusReceived),
            1006 => Ok(Self::AbnormalClosure),
            1007 => Ok(Self::InvalidPayloadData),
            1008 => Ok(Self::PolicyViolation),
            1009 => Ok(Self::MessageTooBig),
            1010 => Ok(Self::MandatoryExtension),
            1011 => Ok(Self::InternalError),
            1012 => Ok(Self::ServiceRestart),
            1013 => Ok(Self::TryAgainLater),
            1014 => Ok(Self::BadGateway),
            1015 => Ok(Self::TlsHandshakeFailure),
            _ => Err(()),
        }
    }

    /// Spec §7.4.2: codes 1004, 1005, 1006, 1015 are reserved and
    /// must NOT appear on the wire.
    #[must_use]
    pub const fn is_reserved(self) -> bool {
        matches!(
            self,
            Self::NoStatusReceived | Self::AbnormalClosure | Self::TlsHandshakeFailure
        )
    }
}

// ---------------------------------------------------------------------------
// §7.4 status code range validation
// ---------------------------------------------------------------------------

/// Spec §7.4 status code range classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCodeRange {
    /// 0..=999 — forbidden (not assigned).
    Invalid,
    /// 1000..=2999 — protocol-reserved (`is_protocol_assigned` subset).
    /// Non-assigned values in this range are invalid.
    ProtocolReserved,
    /// 3000..=3999 — library/framework-defined.
    LibraryDefined,
    /// 4000..=4999 — application-defined.
    ApplicationDefined,
    /// 5000+ — forbidden.
    OutOfRange,
}

/// Spec §7.4 — classifies a wire status code.
#[must_use]
pub const fn classify_status_code(code: u16) -> StatusCodeRange {
    match code {
        0..=999 => StatusCodeRange::Invalid,
        1000..=2999 => StatusCodeRange::ProtocolReserved,
        3000..=3999 => StatusCodeRange::LibraryDefined,
        4000..=4999 => StatusCodeRange::ApplicationDefined,
        _ => StatusCodeRange::OutOfRange,
    }
}

/// Spec §7.4.2 — code 1004 is reserved-but-not-on-wire (along with
/// 1005, 1006, 1015). Returns `true` if the wire code is in this
/// forbidden set.
#[must_use]
pub const fn is_forbidden_on_wire(code: u16) -> bool {
    matches!(code, 1004 | 1005 | 1006 | 1015)
}

/// Spec §7.4.2 — validates a wire status code as legal.
///
/// # Errors
/// `()` if the code is outside any allowed range or violates one of
/// the reserved-but-not-on-wire restrictions.
#[allow(clippy::result_unit_err)]
pub const fn validate_wire_status_code(code: u16) -> Result<(), ()> {
    if is_forbidden_on_wire(code) {
        return Err(());
    }
    match classify_status_code(code) {
        StatusCodeRange::ProtocolReserved
        | StatusCodeRange::LibraryDefined
        | StatusCodeRange::ApplicationDefined => Ok(()),
        StatusCodeRange::Invalid | StatusCodeRange::OutOfRange => Err(()),
    }
}

/// Close-frame payload (code + optional reason).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosePayload {
    /// Status code.
    pub code: CloseCode,
    /// UTF-8 reason (max 123 bytes — Spec §5.5.1).
    pub reason: String,
}

/// Encode to close-frame payload bytes (2-byte BE code + UTF-8 reason).
#[must_use]
pub fn encode_close_payload(payload: &ClosePayload) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + payload.reason.len());
    out.extend_from_slice(&payload.code.to_u16().to_be_bytes());
    out.extend_from_slice(payload.reason.as_bytes());
    out
}

/// Decode from close-frame payload bytes.
///
/// # Errors
/// `()` if the payload < 2 bytes or UTF-8-invalid or a reserved code.
#[allow(clippy::result_unit_err)]
pub fn decode_close_payload(bytes: &[u8]) -> Result<ClosePayload, ()> {
    if bytes.is_empty() {
        // Spec §5.5.1: an empty close payload is allowed → no status.
        return Err(());
    }
    if bytes.len() < 2 {
        return Err(());
    }
    let code_u16 = u16::from_be_bytes([bytes[0], bytes[1]]);
    let code = CloseCode::from_u16(code_u16)?;
    if code.is_reserved() {
        return Err(());
    }
    let reason = core::str::from_utf8(&bytes[2..])
        .map_err(|_| ())?
        .to_string();
    if reason.len() > 123 {
        return Err(());
    }
    Ok(ClosePayload { code, reason })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn standard_codes_round_trip() {
        for c in [
            CloseCode::Normal,
            CloseCode::GoingAway,
            CloseCode::ProtocolError,
            CloseCode::UnsupportedData,
            CloseCode::InvalidPayloadData,
            CloseCode::PolicyViolation,
            CloseCode::MessageTooBig,
            CloseCode::MandatoryExtension,
            CloseCode::InternalError,
            CloseCode::ServiceRestart,
            CloseCode::TryAgainLater,
            CloseCode::BadGateway,
        ] {
            assert_eq!(CloseCode::from_u16(c.to_u16()).unwrap(), c);
        }
    }

    #[test]
    fn reserved_codes_flag_correctly() {
        assert!(CloseCode::NoStatusReceived.is_reserved());
        assert!(CloseCode::AbnormalClosure.is_reserved());
        assert!(CloseCode::TlsHandshakeFailure.is_reserved());
        assert!(!CloseCode::Normal.is_reserved());
    }

    #[test]
    fn unknown_code_rejected() {
        assert!(CloseCode::from_u16(2999).is_err());
    }

    #[test]
    fn round_trip_payload_with_reason() {
        let p = ClosePayload {
            code: CloseCode::Normal,
            reason: "bye".into(),
        };
        let buf = encode_close_payload(&p);
        assert_eq!(buf[0..2], [0x03, 0xe8]); // 1000 BE
        let back = decode_close_payload(&buf).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn round_trip_payload_no_reason() {
        let p = ClosePayload {
            code: CloseCode::GoingAway,
            reason: String::new(),
        };
        let buf = encode_close_payload(&p);
        let back = decode_close_payload(&buf).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn decode_reserved_code_rejected() {
        let buf = [0x03, 0xed]; // 1005 (NoStatusReceived) — reserved
        assert!(decode_close_payload(&buf).is_err());
    }

    #[test]
    fn decode_short_payload_rejected() {
        assert!(decode_close_payload(&[]).is_err());
        assert!(decode_close_payload(&[0x03]).is_err());
    }

    #[test]
    fn reason_too_long_rejected() {
        let mut buf = alloc::vec![0x03, 0xe8];
        buf.extend(std::iter::repeat_n(b'a', 124));
        assert!(decode_close_payload(&buf).is_err());
    }

    // ---------------------------------------------------------------
    // §7.4 Status-Code-Range-Validation
    // ---------------------------------------------------------------

    #[test]
    fn classify_status_code_recognizes_protocol_range() {
        assert_eq!(
            classify_status_code(1000),
            StatusCodeRange::ProtocolReserved
        );
        assert_eq!(
            classify_status_code(2999),
            StatusCodeRange::ProtocolReserved
        );
    }

    #[test]
    fn classify_status_code_recognizes_library_range() {
        assert_eq!(classify_status_code(3000), StatusCodeRange::LibraryDefined);
        assert_eq!(classify_status_code(3999), StatusCodeRange::LibraryDefined);
    }

    #[test]
    fn classify_status_code_recognizes_app_range() {
        assert_eq!(
            classify_status_code(4000),
            StatusCodeRange::ApplicationDefined
        );
        assert_eq!(
            classify_status_code(4999),
            StatusCodeRange::ApplicationDefined
        );
    }

    #[test]
    fn classify_status_code_recognizes_invalid_below_1000() {
        assert_eq!(classify_status_code(0), StatusCodeRange::Invalid);
        assert_eq!(classify_status_code(999), StatusCodeRange::Invalid);
    }

    #[test]
    fn classify_status_code_recognizes_out_of_range_above_5000() {
        assert_eq!(classify_status_code(5000), StatusCodeRange::OutOfRange);
    }

    #[test]
    fn is_forbidden_on_wire_covers_all_four() {
        assert!(is_forbidden_on_wire(1004));
        assert!(is_forbidden_on_wire(1005));
        assert!(is_forbidden_on_wire(1006));
        assert!(is_forbidden_on_wire(1015));
        assert!(!is_forbidden_on_wire(1000));
    }

    #[test]
    fn validate_wire_status_code_accepts_normal() {
        assert!(validate_wire_status_code(1000).is_ok());
        assert!(validate_wire_status_code(3000).is_ok());
        assert!(validate_wire_status_code(4500).is_ok());
    }

    #[test]
    fn validate_wire_status_code_rejects_forbidden() {
        assert!(validate_wire_status_code(1004).is_err());
        assert!(validate_wire_status_code(1005).is_err());
        assert!(validate_wire_status_code(1006).is_err());
        assert!(validate_wire_status_code(1015).is_err());
    }

    #[test]
    fn validate_wire_status_code_rejects_out_of_range() {
        assert!(validate_wire_status_code(0).is_err());
        assert!(validate_wire_status_code(999).is_err());
        assert!(validate_wire_status_code(5000).is_err());
    }

    // ---------------------------------------------------------------
    // §7.1, §7.2, §7.3 Close-Handshake State-Machine
    // ---------------------------------------------------------------

    #[test]
    fn handshake_starts_in_open_state() {
        let h = CloseHandshake::new();
        assert_eq!(h.state(), CloseState::Open);
        assert!(!h.is_closed());
    }

    #[test]
    fn initiator_send_close_transitions_to_closing() {
        let mut h = CloseHandshake::new();
        h.initiator_send_close(CloseCode::Normal).expect("ok");
        assert_eq!(h.state(), CloseState::ClosingInitiator);
    }

    #[test]
    fn initiator_recv_close_response_transitions_to_closed() {
        let mut h = CloseHandshake::new();
        h.initiator_send_close(CloseCode::Normal).expect("ok");
        h.recv_close_response(CloseCode::Normal).expect("ok");
        assert_eq!(h.state(), CloseState::Closed);
        assert!(h.is_closed());
    }

    #[test]
    fn responder_recv_close_transitions_to_closing_responder() {
        let mut h = CloseHandshake::new();
        h.responder_recv_close(CloseCode::Normal).expect("ok");
        assert_eq!(h.state(), CloseState::ClosingResponder);
    }

    #[test]
    fn responder_send_close_response_completes_normally() {
        let mut h = CloseHandshake::new();
        h.responder_recv_close(CloseCode::GoingAway).expect("ok");
        h.responder_send_close_response().expect("ok");
        assert_eq!(h.state(), CloseState::Closed);
    }

    #[test]
    fn fail_marks_abnormal_closure() {
        let mut h = CloseHandshake::new();
        h.fail("transport error");
        assert_eq!(h.state(), CloseState::Failed);
        assert!(h.is_closed());
        assert_eq!(h.failure_reason(), Some("transport error"));
    }

    #[test]
    fn second_close_send_in_closing_is_rejected() {
        let mut h = CloseHandshake::new();
        h.initiator_send_close(CloseCode::Normal).expect("ok");
        assert!(h.initiator_send_close(CloseCode::Normal).is_err());
    }

    #[test]
    fn recv_close_in_open_state_is_responder_path() {
        let mut h = CloseHandshake::new();
        // No initiator_send_close happened first
        assert!(h.recv_close_response(CloseCode::Normal).is_err());
    }
}

// ---------------------------------------------------------------------------
// §7.1 / §7.2 / §7.3 Close-Handshake State-Machine
// ---------------------------------------------------------------------------

/// Spec §7.1 / §7.2 / §7.3 — close-handshake state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseState {
    /// The connection is open, frames are exchanged freely.
    Open,
    /// We have sent Close and are waiting for the response. (Spec §7.1.2)
    ClosingInitiator,
    /// The peer has sent Close, we must respond with Close.
    /// (Spec §7.1.4)
    ClosingResponder,
    /// Close handshake completed — a TCP close may follow.
    /// (Spec §7.1.1 Normal Closure)
    Closed,
    /// The connection was terminated abnormally. (Spec §7.1.7)
    Failed,
}

/// Close-handshake state machine.
#[derive(Debug, Clone)]
pub struct CloseHandshake {
    state: CloseState,
    sent_code: Option<CloseCode>,
    received_code: Option<CloseCode>,
    failure_reason: Option<String>,
}

impl Default for CloseHandshake {
    fn default() -> Self {
        Self::new()
    }
}

impl CloseHandshake {
    /// Constructor — starts in the open state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: CloseState::Open,
            sent_code: None,
            received_code: None,
            failure_reason: None,
        }
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> CloseState {
        self.state
    }

    /// `true` if the connection is (closed or failed).
    #[must_use]
    pub fn is_closed(&self) -> bool {
        matches!(self.state, CloseState::Closed | CloseState::Failed)
    }

    /// Failure reason on abnormal closure (§7.1.7).
    #[must_use]
    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }

    /// Spec §7.1.2 — the initiator sends a close frame with a status code.
    /// Transitions from `Open` → `ClosingInitiator`.
    ///
    /// # Errors
    /// `()` if not in the `Open` state.
    #[allow(clippy::result_unit_err)]
    pub fn initiator_send_close(&mut self, code: CloseCode) -> Result<(), ()> {
        if self.state != CloseState::Open {
            return Err(());
        }
        self.state = CloseState::ClosingInitiator;
        self.sent_code = Some(code);
        Ok(())
    }

    /// Spec §7.1.3 — the initiator receives the close response.
    /// Transitions from `ClosingInitiator` → `Closed`.
    ///
    /// # Errors
    /// `()` if not in the `ClosingInitiator` state.
    #[allow(clippy::result_unit_err)]
    pub fn recv_close_response(&mut self, code: CloseCode) -> Result<(), ()> {
        if self.state != CloseState::ClosingInitiator {
            return Err(());
        }
        self.received_code = Some(code);
        self.state = CloseState::Closed;
        Ok(())
    }

    /// Spec §7.1.4 — the responder receives a close frame from the peer.
    /// Transitions from `Open` → `ClosingResponder`.
    ///
    /// # Errors
    /// `()` if not in the `Open` state.
    #[allow(clippy::result_unit_err)]
    pub fn responder_recv_close(&mut self, code: CloseCode) -> Result<(), ()> {
        if self.state != CloseState::Open {
            return Err(());
        }
        self.received_code = Some(code);
        self.state = CloseState::ClosingResponder;
        Ok(())
    }

    /// Spec §7.1.4 — the responder replies with a close frame.
    /// Transitions from `ClosingResponder` → `Closed`.
    ///
    /// # Errors
    /// `()` if not in the `ClosingResponder` state.
    #[allow(clippy::result_unit_err)]
    pub fn responder_send_close_response(&mut self) -> Result<(), ()> {
        if self.state != CloseState::ClosingResponder {
            return Err(());
        }
        // Echo back received code per Spec §7.1.4.
        self.sent_code = self.received_code;
        self.state = CloseState::Closed;
        Ok(())
    }

    /// Spec §7.1.7 — abnormal closure (TCP reset, crash, etc.).
    /// Transitions to `Failed` with a reason.
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.state = CloseState::Failed;
        self.failure_reason = Some(reason.into());
    }
}
