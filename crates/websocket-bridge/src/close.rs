// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Close-frame status codes — RFC 6455 §7.4.

use alloc::string::{String, ToString};
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

impl From<CloseCode> for u16 {
    fn from(code: CloseCode) -> Self {
        code.to_u16()
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

/// Spec §7.4.2 — validates a wire status code as legal: a code the
/// protocol defines (§7.4.1 and the IANA WebSocket Close Code Number
/// Registry of §11.7: 1000 to 1003, 1007 to 1014), or one of the ranges
/// left to libraries (3000 to 3999) and applications (4000 to 4999).
/// 1000 to 2999 are "reserved for definition by this protocol": a number
/// in that range that nothing defines is not a status code.
///
/// # Errors
/// `()` if the code is outside any allowed range, is not defined, or
/// violates one of the reserved-but-not-on-wire restrictions.
#[allow(clippy::result_unit_err)]
pub const fn validate_wire_status_code(code: u16) -> Result<(), ()> {
    if is_forbidden_on_wire(code) {
        return Err(());
    }
    match classify_status_code(code) {
        StatusCodeRange::ProtocolReserved => match CloseCode::from_u16(code) {
            Ok(_) => Ok(()),
            Err(()) => Err(()),
        },
        StatusCodeRange::LibraryDefined | StatusCodeRange::ApplicationDefined => Ok(()),
        StatusCodeRange::Invalid | StatusCodeRange::OutOfRange => Err(()),
    }
}

/// Why the body of a received Close frame is invalid (Spec §5.5.1, §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseBodyError {
    /// One octet: "If there is a body, the first two bytes of the body
    /// MUST be a 2-byte unsigned integer" (§5.5.1).
    TruncatedStatus,
    /// A status code that must not appear in a Close frame (§7.4.1,
    /// §7.4.2; [`validate_wire_status_code`]).
    StatusNotAllowed(u16),
    /// The reason is not UTF-8 (§5.5.1: "UTF-8-encoded data").
    ReasonNotUtf8,
}

impl CloseBodyError {
    /// Spec §7.4.1 — the status code the connection is failed with: 1007
    /// for a reason that is not UTF-8, 1002 for the status code.
    #[must_use]
    pub const fn close_code(self) -> CloseCode {
        match self {
            Self::TruncatedStatus | Self::StatusNotAllowed(_) => CloseCode::ProtocolError,
            Self::ReasonNotUtf8 => CloseCode::InvalidPayloadData,
        }
    }
}

impl core::fmt::Display for CloseBodyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TruncatedStatus => f.write_str("close body of one octet"),
            Self::StatusNotAllowed(c) => write!(f, "close status code {c} is not allowed"),
            Self::ReasonNotUtf8 => f.write_str("close reason is not UTF-8"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CloseBodyError {}

/// The body of a received Close frame (Spec §5.5.1): the status code and
/// the reason, `None` for the empty body, which carries no status code
/// (§7.1.5 reads it as 1005). Every code [`validate_wire_status_code`]
/// allows is accepted, the library and application ranges among them,
/// which [`decode_close_payload`] with its [`CloseCode`] cannot carry.
///
/// # Errors
/// See [`CloseBodyError`].
pub fn parse_close_body(bytes: &[u8]) -> Result<Option<(u16, String)>, CloseBodyError> {
    match bytes {
        [] => Ok(None),
        [_] => Err(CloseBodyError::TruncatedStatus),
        [hi, lo, reason @ ..] => {
            let code = u16::from_be_bytes([*hi, *lo]);
            if validate_wire_status_code(code).is_err() {
                return Err(CloseBodyError::StatusNotAllowed(code));
            }
            let reason = core::str::from_utf8(reason).map_err(|_| CloseBodyError::ReasonNotUtf8)?;
            Ok(Some((code, reason.to_string())))
        }
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
    fn ws_7_4_1_the_status_codes_are_the_numbers_of_the_rfc_and_the_registry() {
        // RFC 6455 §7.4.1; 1012 to 1014 from the IANA WebSocket Close Code
        // Number Registry (§11.7).
        for (c, number) in [
            (CloseCode::Normal, 1000),
            (CloseCode::GoingAway, 1001),
            (CloseCode::ProtocolError, 1002),
            (CloseCode::UnsupportedData, 1003),
            (CloseCode::InvalidPayloadData, 1007),
            (CloseCode::PolicyViolation, 1008),
            (CloseCode::MessageTooBig, 1009),
            (CloseCode::MandatoryExtension, 1010),
            (CloseCode::InternalError, 1011),
            (CloseCode::ServiceRestart, 1012),
            (CloseCode::TryAgainLater, 1013),
            (CloseCode::BadGateway, 1014),
        ] {
            assert_eq!(c.to_u16(), number);
            assert_eq!(CloseCode::from_u16(number).unwrap(), c);
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
    fn ws_5_5_1_a_close_body_without_reason_is_the_code_alone() {
        let p = ClosePayload {
            code: CloseCode::GoingAway,
            reason: String::new(),
        };
        assert_eq!(encode_close_payload(&p), [0x03, 0xe9]); // 1001 BE
        assert_eq!(decode_close_payload(&[0x03, 0xe9]).unwrap(), p);
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
    fn ws_7_4_classify_status_code_recognizes_protocol_range() {
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
    fn ws_7_4_classify_status_code_recognizes_library_range() {
        assert_eq!(classify_status_code(3000), StatusCodeRange::LibraryDefined);
        assert_eq!(classify_status_code(3999), StatusCodeRange::LibraryDefined);
    }

    #[test]
    fn ws_7_4_classify_status_code_recognizes_app_range() {
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
    fn ws_7_4_classify_status_code_recognizes_invalid_below_1000() {
        assert_eq!(classify_status_code(0), StatusCodeRange::Invalid);
        assert_eq!(classify_status_code(999), StatusCodeRange::Invalid);
    }

    #[test]
    fn ws_7_4_classify_status_code_recognizes_out_of_range_above_5000() {
        assert_eq!(classify_status_code(5000), StatusCodeRange::OutOfRange);
    }

    #[test]
    fn ws_7_4_is_forbidden_on_wire_covers_all_four() {
        assert!(is_forbidden_on_wire(1004));
        assert!(is_forbidden_on_wire(1005));
        assert!(is_forbidden_on_wire(1006));
        assert!(is_forbidden_on_wire(1015));
        assert!(!is_forbidden_on_wire(1000));
    }

    #[test]
    fn ws_7_4_validate_wire_status_code_accepts_normal() {
        assert!(validate_wire_status_code(1000).is_ok());
        assert!(validate_wire_status_code(3000).is_ok());
        assert!(validate_wire_status_code(4500).is_ok());
    }

    #[test]
    fn ws_7_4_validate_wire_status_code_rejects_forbidden() {
        assert!(validate_wire_status_code(1004).is_err());
        assert!(validate_wire_status_code(1005).is_err());
        assert!(validate_wire_status_code(1006).is_err());
        assert!(validate_wire_status_code(1015).is_err());
    }

    #[test]
    fn ws_7_4_validate_wire_status_code_rejects_out_of_range() {
        assert!(validate_wire_status_code(0).is_err());
        assert!(validate_wire_status_code(999).is_err());
        assert!(validate_wire_status_code(5000).is_err());
    }

    #[test]
    fn ws_7_4_2_a_code_of_the_protocol_range_that_nothing_defines_is_refused() {
        // §7.4.2: 1000 to 2999 are reserved for definition by the protocol
        // and public specifications; 1016, 1100, 2000 and 2999 are defined
        // nowhere. The registered ones pass.
        for code in [1016, 1100, 2000, 2999] {
            assert!(validate_wire_status_code(code).is_err(), "{code}");
        }
        for code in [
            1000, 1001, 1002, 1003, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014,
        ] {
            assert!(validate_wire_status_code(code).is_ok(), "{code}");
        }
    }

    #[test]
    fn ws_5_5_1_a_close_body_is_empty_or_a_status_code_and_a_utf8_reason() {
        assert_eq!(parse_close_body(&[]), Ok(None));
        assert_eq!(
            parse_close_body(&[0x03, 0xe8]),
            Ok(Some((1000, String::new())))
        );
        assert_eq!(
            parse_close_body(b"\x0f\xa0bye"),
            Ok(Some((4000, "bye".to_string())))
        );
        // One octet is no status code.
        let err = parse_close_body(&[0x03]).unwrap_err();
        assert_eq!(err, CloseBodyError::TruncatedStatus);
        assert_eq!(err.close_code().to_u16(), 1002);
        // 1005 must not be sent (§7.4.1), 999 is below every range.
        for code in [999u16, 1004, 1005, 1006, 1015, 1016, 5000, 65535] {
            let err = parse_close_body(&code.to_be_bytes()).unwrap_err();
            assert_eq!(err, CloseBodyError::StatusNotAllowed(code));
            assert_eq!(err.close_code().to_u16(), 1002);
        }
        // A reason that is not UTF-8 is invalid payload data (1007).
        let err = parse_close_body(&[0x03, 0xe8, 0xce, 0xba, 0xe1, 0xbd]).unwrap_err();
        assert_eq!(err, CloseBodyError::ReasonNotUtf8);
        assert_eq!(err.close_code().to_u16(), 1007);
    }

    // ---------------------------------------------------------------
    // §7.1, §7.2, §7.3 Close-Handshake State-Machine
    // ---------------------------------------------------------------

    #[test]
    fn ws_7_1_handshake_starts_in_open_state() {
        let h = CloseHandshake::new();
        assert_eq!(h.state(), CloseState::Open);
        assert!(!h.is_closed());
    }

    #[test]
    fn ws_7_1_initiator_send_close_transitions_to_closing() {
        let mut h = CloseHandshake::new();
        h.initiator_send_close(CloseCode::Normal).expect("ok");
        assert_eq!(h.state(), CloseState::ClosingInitiator);
    }

    #[test]
    fn ws_7_1_initiator_recv_close_response_transitions_to_closed() {
        let mut h = CloseHandshake::new();
        h.initiator_send_close(CloseCode::Normal).expect("ok");
        h.recv_close_response(CloseCode::Normal).expect("ok");
        assert_eq!(h.state(), CloseState::Closed);
        assert!(h.is_closed());
    }

    #[test]
    fn ws_7_1_responder_recv_close_transitions_to_closing_responder() {
        let mut h = CloseHandshake::new();
        h.responder_recv_close(CloseCode::Normal).expect("ok");
        assert_eq!(h.state(), CloseState::ClosingResponder);
    }

    #[test]
    fn ws_7_1_responder_send_close_response_completes_normally() {
        let mut h = CloseHandshake::new();
        h.responder_recv_close(CloseCode::GoingAway).expect("ok");
        h.responder_send_close_response().expect("ok");
        assert_eq!(h.state(), CloseState::Closed);
    }

    #[test]
    fn ws_7_2_fail_marks_abnormal_closure() {
        let mut h = CloseHandshake::new();
        h.fail("transport error");
        assert_eq!(h.state(), CloseState::Failed);
        assert!(h.is_closed());
        assert_eq!(h.failure_reason(), Some("transport error"));
    }

    #[test]
    fn ws_7_1_second_close_send_in_closing_is_rejected() {
        let mut h = CloseHandshake::new();
        h.initiator_send_close(CloseCode::Normal).expect("ok");
        assert!(h.initiator_send_close(CloseCode::Normal).is_err());
    }

    #[test]
    fn ws_7_1_recv_close_in_open_state_is_responder_path() {
        let mut h = CloseHandshake::new();
        // No initiator_send_close happened first
        assert!(h.recv_close_response(CloseCode::Normal).is_err());
    }

    #[test]
    fn ws_7_1_the_responder_echoes_the_code_it_received() {
        // §5.5.1: the endpoint "typically echos the status code it
        // received"; an application code (4000 to 4999) is one too.
        let mut h = CloseHandshake::new();
        h.responder_recv_close(4321u16).expect("ok");
        assert_eq!(h.received_code(), Some(4321));
        assert_eq!(h.sent_code(), None);
        h.responder_send_close_response().expect("ok");
        assert_eq!(h.sent_code(), Some(4321));
        assert!(h.is_closed());
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
    sent_code: Option<u16>,
    received_code: Option<u16>,
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

    /// The status code of the Close frame this endpoint sent.
    #[must_use]
    pub fn sent_code(&self) -> Option<u16> {
        self.sent_code
    }

    /// The status code of the Close frame the peer sent (§7.1.5: 1005 for
    /// a Close frame without one).
    #[must_use]
    pub fn received_code(&self) -> Option<u16> {
        self.received_code
    }

    /// Spec §7.1.2 — the initiator sends a close frame with a status code.
    /// Transitions from `Open` → `ClosingInitiator`.
    ///
    /// # Errors
    /// `()` if not in the `Open` state.
    #[allow(clippy::result_unit_err)]
    pub fn initiator_send_close(&mut self, code: impl Into<u16>) -> Result<(), ()> {
        if self.state != CloseState::Open {
            return Err(());
        }
        self.state = CloseState::ClosingInitiator;
        self.sent_code = Some(code.into());
        Ok(())
    }

    /// Spec §7.1.3 — the initiator receives the close response.
    /// Transitions from `ClosingInitiator` → `Closed`.
    ///
    /// # Errors
    /// `()` if not in the `ClosingInitiator` state.
    #[allow(clippy::result_unit_err)]
    pub fn recv_close_response(&mut self, code: impl Into<u16>) -> Result<(), ()> {
        if self.state != CloseState::ClosingInitiator {
            return Err(());
        }
        self.received_code = Some(code.into());
        self.state = CloseState::Closed;
        Ok(())
    }

    /// Spec §7.1.4 — the responder receives a close frame from the peer.
    /// Transitions from `Open` → `ClosingResponder`.
    ///
    /// # Errors
    /// `()` if not in the `Open` state.
    #[allow(clippy::result_unit_err)]
    pub fn responder_recv_close(&mut self, code: impl Into<u16>) -> Result<(), ()> {
        if self.state != CloseState::Open {
            return Err(());
        }
        self.received_code = Some(code.into());
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
