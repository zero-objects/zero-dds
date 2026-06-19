// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT 5.0 Reason Codes — Spec §2.4.
//!
//! Reason codes >= 0x80 are errors. We provide a complete
//! table of all codes from Spec Table 2-2.

use core::fmt;

/// MQTT-5.0 Reason Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReasonCode {
    /// `0` Success / Normal Disconnection / Granted QoS 0.
    Success = 0x00,
    /// `1` Granted QoS 1.
    GrantedQoS1 = 0x01,
    /// `2` Granted QoS 2.
    GrantedQoS2 = 0x02,
    /// `4` Disconnect with Will Message.
    DisconnectWithWillMessage = 0x04,
    /// `16` No Matching Subscribers.
    NoMatchingSubscribers = 0x10,
    /// `17` No Subscription Existed.
    NoSubscriptionExisted = 0x11,
    /// `24` Continue Authentication.
    ContinueAuthentication = 0x18,
    /// `25` Re-authenticate.
    Reauthenticate = 0x19,
    /// `128` Unspecified Error.
    UnspecifiedError = 0x80,
    /// `129` Malformed Packet.
    MalformedPacket = 0x81,
    /// `130` Protocol Error.
    ProtocolError = 0x82,
    /// `131` Implementation specific Error.
    ImplementationSpecificError = 0x83,
    /// `132` Unsupported Protocol Version.
    UnsupportedProtocolVersion = 0x84,
    /// `133` Client Identifier not valid.
    ClientIdentifierNotValid = 0x85,
    /// `134` Bad User Name or Password.
    BadUserNameOrPassword = 0x86,
    /// `135` Not authorized.
    NotAuthorized = 0x87,
    /// `136` Server unavailable.
    ServerUnavailable = 0x88,
    /// `137` Server busy.
    ServerBusy = 0x89,
    /// `138` Banned.
    Banned = 0x8a,
    /// `139` Server shutting down.
    ServerShuttingDown = 0x8b,
    /// `140` Bad authentication method.
    BadAuthenticationMethod = 0x8c,
    /// `141` Keep Alive timeout.
    KeepAliveTimeout = 0x8d,
    /// `142` Session taken over.
    SessionTakenOver = 0x8e,
    /// `143` Topic Filter invalid.
    TopicFilterInvalid = 0x8f,
    /// `144` Topic Name invalid.
    TopicNameInvalid = 0x90,
    /// `145` Packet Identifier in use.
    PacketIdentifierInUse = 0x91,
    /// `146` Packet Identifier not found.
    PacketIdentifierNotFound = 0x92,
    /// `147` Receive Maximum exceeded.
    ReceiveMaximumExceeded = 0x93,
    /// `148` Topic Alias invalid.
    TopicAliasInvalid = 0x94,
    /// `149` Packet too large.
    PacketTooLarge = 0x95,
    /// `150` Message rate too high.
    MessageRateTooHigh = 0x96,
    /// `151` Quota exceeded.
    QuotaExceeded = 0x97,
    /// `152` Administrative action.
    AdministrativeAction = 0x98,
    /// `153` Payload format invalid.
    PayloadFormatInvalid = 0x99,
    /// `154` Retain not supported.
    RetainNotSupported = 0x9a,
    /// `155` QoS not supported.
    QoSNotSupported = 0x9b,
    /// `156` Use another server.
    UseAnotherServer = 0x9c,
    /// `157` Server moved.
    ServerMoved = 0x9d,
    /// `158` Shared Subscriptions not supported.
    SharedSubscriptionsNotSupported = 0x9e,
    /// `159` Connection rate exceeded.
    ConnectionRateExceeded = 0x9f,
    /// `160` Maximum connect time.
    MaximumConnectTime = 0xa0,
    /// `161` Subscription Identifiers not supported.
    SubscriptionIdentifiersNotSupported = 0xa1,
    /// `162` Wildcard Subscriptions not supported.
    WildcardSubscriptionsNotSupported = 0xa2,
}

impl ReasonCode {
    /// `true` if the code >= 0x80 (Spec §2.4.1).
    #[must_use]
    pub const fn is_error(self) -> bool {
        (self as u8) >= 0x80
    }

    /// `u8 -> ReasonCode`.
    ///
    /// # Errors
    /// `()` if the code is unknown.
    #[allow(clippy::result_unit_err)]
    pub const fn from_u8(v: u8) -> Result<Self, ()> {
        match v {
            0x00 => Ok(Self::Success),
            0x01 => Ok(Self::GrantedQoS1),
            0x02 => Ok(Self::GrantedQoS2),
            0x04 => Ok(Self::DisconnectWithWillMessage),
            0x10 => Ok(Self::NoMatchingSubscribers),
            0x11 => Ok(Self::NoSubscriptionExisted),
            0x18 => Ok(Self::ContinueAuthentication),
            0x19 => Ok(Self::Reauthenticate),
            0x80 => Ok(Self::UnspecifiedError),
            0x81 => Ok(Self::MalformedPacket),
            0x82 => Ok(Self::ProtocolError),
            0x83 => Ok(Self::ImplementationSpecificError),
            0x84 => Ok(Self::UnsupportedProtocolVersion),
            0x85 => Ok(Self::ClientIdentifierNotValid),
            0x86 => Ok(Self::BadUserNameOrPassword),
            0x87 => Ok(Self::NotAuthorized),
            0x88 => Ok(Self::ServerUnavailable),
            0x89 => Ok(Self::ServerBusy),
            0x8a => Ok(Self::Banned),
            0x8b => Ok(Self::ServerShuttingDown),
            0x8c => Ok(Self::BadAuthenticationMethod),
            0x8d => Ok(Self::KeepAliveTimeout),
            0x8e => Ok(Self::SessionTakenOver),
            0x8f => Ok(Self::TopicFilterInvalid),
            0x90 => Ok(Self::TopicNameInvalid),
            0x91 => Ok(Self::PacketIdentifierInUse),
            0x92 => Ok(Self::PacketIdentifierNotFound),
            0x93 => Ok(Self::ReceiveMaximumExceeded),
            0x94 => Ok(Self::TopicAliasInvalid),
            0x95 => Ok(Self::PacketTooLarge),
            0x96 => Ok(Self::MessageRateTooHigh),
            0x97 => Ok(Self::QuotaExceeded),
            0x98 => Ok(Self::AdministrativeAction),
            0x99 => Ok(Self::PayloadFormatInvalid),
            0x9a => Ok(Self::RetainNotSupported),
            0x9b => Ok(Self::QoSNotSupported),
            0x9c => Ok(Self::UseAnotherServer),
            0x9d => Ok(Self::ServerMoved),
            0x9e => Ok(Self::SharedSubscriptionsNotSupported),
            0x9f => Ok(Self::ConnectionRateExceeded),
            0xa0 => Ok(Self::MaximumConnectTime),
            0xa1 => Ok(Self::SubscriptionIdentifiersNotSupported),
            0xa2 => Ok(Self::WildcardSubscriptionsNotSupported),
            _ => Err(()),
        }
    }
}

impl fmt::Display for ReasonCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}(0x{:02x})", self, *self as u8)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn success_is_zero() {
        assert_eq!(ReasonCode::Success as u8, 0);
        assert!(!ReasonCode::Success.is_error());
    }

    #[test]
    fn error_codes_have_high_bit() {
        for c in [
            ReasonCode::UnspecifiedError,
            ReasonCode::MalformedPacket,
            ReasonCode::ProtocolError,
            ReasonCode::TopicNameInvalid,
            ReasonCode::WildcardSubscriptionsNotSupported,
        ] {
            assert!(c.is_error());
            assert!((c as u8) >= 0x80);
        }
    }

    #[test]
    fn round_trip_all_codes() {
        let codes = [
            0x00, 0x01, 0x02, 0x04, 0x10, 0x11, 0x18, 0x19, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85,
            0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93,
            0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f, 0xa0, 0xa1,
            0xa2,
        ];
        for c in codes {
            assert_eq!(ReasonCode::from_u8(c).unwrap() as u8, c);
        }
    }

    #[test]
    fn unknown_code_rejected() {
        assert!(ReasonCode::from_u8(0xff).is_err());
        assert!(ReasonCode::from_u8(0x03).is_err());
    }

    #[test]
    fn granted_qos_codes_are_not_errors() {
        assert!(!ReasonCode::GrantedQoS1.is_error());
        assert!(!ReasonCode::GrantedQoS2.is_error());
    }

    #[test]
    fn display_formats_with_hex() {
        let s = alloc::format!("{}", ReasonCode::Success);
        assert!(s.contains("0x00"));
    }
}
