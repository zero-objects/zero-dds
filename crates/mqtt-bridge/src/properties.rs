// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT v5.0 Properties — Spec §2.2.2.

use alloc::vec::Vec;

/// Property identifier (Spec §2.2.2.2 Table 2-4) — subset of the most
/// important MQTT 5.0 properties.
///
/// The complete table has ~30 IDs; we provide named constants
/// for the most common ones and expose the `u32` value for the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyId {
    /// `1` Payload Format Indicator (PUBLISH).
    PayloadFormatIndicator,
    /// `2` Message Expiry Interval (PUBLISH).
    MessageExpiryInterval,
    /// `3` Content Type (PUBLISH).
    ContentType,
    /// `8` Response Topic (PUBLISH).
    ResponseTopic,
    /// `9` Correlation Data (PUBLISH).
    CorrelationData,
    /// `11` Subscription Identifier (PUBLISH/SUBSCRIBE).
    SubscriptionIdentifier,
    /// `17` Session Expiry Interval (CONNECT/CONNACK/DISCONNECT).
    SessionExpiryInterval,
    /// `18` Assigned Client Identifier (CONNACK).
    AssignedClientIdentifier,
    /// `19` Server Keep Alive (CONNACK).
    ServerKeepAlive,
    /// `21` Authentication Method (CONNECT/CONNACK/AUTH).
    AuthenticationMethod,
    /// `22` Authentication Data (CONNECT/CONNACK/AUTH).
    AuthenticationData,
    /// `33` Receive Maximum (CONNECT/CONNACK).
    ReceiveMaximum,
    /// `34` Topic Alias Maximum (CONNECT/CONNACK).
    TopicAliasMaximum,
    /// `35` Topic Alias (PUBLISH).
    TopicAlias,
    /// `38` User Property (any packet).
    UserProperty,
    /// Other property IDs (Spec §2.2.2.2 Table 2-4 contains more).
    Other(u32),
}

impl PropertyId {
    /// Wire identifier (VBI-encoded; here we return the u32 value
    /// BEFORE the VBI encoding).
    #[must_use]
    pub const fn to_value(self) -> u32 {
        match self {
            Self::PayloadFormatIndicator => 1,
            Self::MessageExpiryInterval => 2,
            Self::ContentType => 3,
            Self::ResponseTopic => 8,
            Self::CorrelationData => 9,
            Self::SubscriptionIdentifier => 11,
            Self::SessionExpiryInterval => 17,
            Self::AssignedClientIdentifier => 18,
            Self::ServerKeepAlive => 19,
            Self::AuthenticationMethod => 21,
            Self::AuthenticationData => 22,
            Self::ReceiveMaximum => 33,
            Self::TopicAliasMaximum => 34,
            Self::TopicAlias => 35,
            Self::UserProperty => 38,
            Self::Other(v) => v,
        }
    }

    /// Converts from the u32 value.
    #[must_use]
    pub const fn from_value(v: u32) -> Self {
        match v {
            1 => Self::PayloadFormatIndicator,
            2 => Self::MessageExpiryInterval,
            3 => Self::ContentType,
            8 => Self::ResponseTopic,
            9 => Self::CorrelationData,
            11 => Self::SubscriptionIdentifier,
            17 => Self::SessionExpiryInterval,
            18 => Self::AssignedClientIdentifier,
            19 => Self::ServerKeepAlive,
            21 => Self::AuthenticationMethod,
            22 => Self::AuthenticationData,
            33 => Self::ReceiveMaximum,
            34 => Self::TopicAliasMaximum,
            35 => Self::TopicAlias,
            38 => Self::UserProperty,
            other => Self::Other(other),
        }
    }

    /// Spec §2.2.2.2 Table 2-4 — value type per property ID.
    /// The caller decodes the `Property::value` slice according to this
    /// classification.
    #[must_use]
    pub const fn value_kind(self) -> PropertyValueKind {
        match self {
            Self::PayloadFormatIndicator => PropertyValueKind::Byte,
            Self::MessageExpiryInterval => PropertyValueKind::FourByteInt,
            Self::ContentType => PropertyValueKind::Utf8String,
            Self::ResponseTopic => PropertyValueKind::Utf8String,
            Self::CorrelationData => PropertyValueKind::BinaryData,
            Self::SubscriptionIdentifier => PropertyValueKind::VariableByteInt,
            Self::SessionExpiryInterval => PropertyValueKind::FourByteInt,
            Self::AssignedClientIdentifier => PropertyValueKind::Utf8String,
            Self::ServerKeepAlive => PropertyValueKind::TwoByteInt,
            Self::AuthenticationMethod => PropertyValueKind::Utf8String,
            Self::AuthenticationData => PropertyValueKind::BinaryData,
            Self::ReceiveMaximum => PropertyValueKind::TwoByteInt,
            Self::TopicAliasMaximum => PropertyValueKind::TwoByteInt,
            Self::TopicAlias => PropertyValueKind::TwoByteInt,
            Self::UserProperty => PropertyValueKind::Utf8StringPair,
            Self::Other(_) => PropertyValueKind::BinaryData,
        }
    }
}

/// Spec §2.1.3 / §2.2.2.2 — property value-type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyValueKind {
    /// 1 Byte (`uint8`).
    Byte,
    /// 2 Byte BE (`uint16`).
    TwoByteInt,
    /// 4 Byte BE (`uint32`).
    FourByteInt,
    /// VBI-encoded `uint32` (1-4 Byte).
    VariableByteInt,
    /// 2-Byte-Length-prefixed UTF-8.
    Utf8String,
    /// Twin Utf8String (Key + Value).
    Utf8StringPair,
    /// 2-Byte-Length-prefixed Bytes.
    BinaryData,
}

impl PropertyValueKind {
    /// Spec §1.5.4 — `uint16` decode (BE).
    ///
    /// # Errors
    /// `()` if fewer than 2 bytes.
    #[allow(clippy::result_unit_err)]
    pub fn decode_two_byte_int(bytes: &[u8]) -> Result<u16, ()> {
        if bytes.len() < 2 {
            return Err(());
        }
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    /// Spec §1.5.5 — `uint32` decode (BE).
    ///
    /// # Errors
    /// `()` if fewer than 4 bytes.
    #[allow(clippy::result_unit_err)]
    pub fn decode_four_byte_int(bytes: &[u8]) -> Result<u32, ()> {
        if bytes.len() < 4 {
            return Err(());
        }
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Spec §1.5.6 — UTF-8 string decode (2-byte length prefix).
    ///
    /// # Errors
    /// `()` on truncation or invalid UTF-8.
    #[allow(clippy::result_unit_err)]
    pub fn decode_utf8_string(bytes: &[u8]) -> Result<&str, ()> {
        if bytes.len() < 2 {
            return Err(());
        }
        let len = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        if bytes.len() < 2 + len {
            return Err(());
        }
        core::str::from_utf8(&bytes[2..2 + len]).map_err(|_| ())
    }

    /// Spec §1.5.7 — binary-data decode (2-byte length prefix).
    ///
    /// # Errors
    /// `()` on truncation.
    #[allow(clippy::result_unit_err)]
    pub fn decode_binary_data(bytes: &[u8]) -> Result<&[u8], ()> {
        if bytes.len() < 2 {
            return Err(());
        }
        let len = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        if bytes.len() < 2 + len {
            return Err(());
        }
        Ok(&bytes[2..2 + len])
    }
}

/// `Property` — identifier + value. The value form depends on the
/// identifier (Spec §2.2.2.2 Table 2-4); we model it as an
/// opaque `Vec<u8>` (raw wire bytes) for codec genericity. The caller
/// interprets the value content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Identifier.
    pub id: PropertyId,
    /// Raw wire bytes of the value field (the format is identifier-specific:
    /// uint8/uint16/uint32/VBI/UTF-8 String/Binary Data/UTF-8 String
    /// Pair).
    pub value: Vec<u8>,
}

impl Property {
    /// Constructor.
    #[must_use]
    pub const fn new(id: PropertyId, value: Vec<u8>) -> Self {
        Self { id, value }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn property_id_round_trip_for_all_named() {
        for id in [
            PropertyId::PayloadFormatIndicator,
            PropertyId::MessageExpiryInterval,
            PropertyId::ContentType,
            PropertyId::ResponseTopic,
            PropertyId::CorrelationData,
            PropertyId::SubscriptionIdentifier,
            PropertyId::SessionExpiryInterval,
            PropertyId::AssignedClientIdentifier,
            PropertyId::ServerKeepAlive,
            PropertyId::AuthenticationMethod,
            PropertyId::AuthenticationData,
            PropertyId::ReceiveMaximum,
            PropertyId::TopicAliasMaximum,
            PropertyId::TopicAlias,
            PropertyId::UserProperty,
        ] {
            assert_eq!(PropertyId::from_value(id.to_value()), id);
        }
    }

    #[test]
    fn property_id_unknown_yields_other_variant() {
        // Spec §2.2.2.2 — future IDs are kept as Other.
        assert_eq!(PropertyId::from_value(99), PropertyId::Other(99));
    }

    #[test]
    fn well_known_property_ids_match_spec_values() {
        // Spec §2.2.2.2 Table 2-4.
        assert_eq!(PropertyId::PayloadFormatIndicator.to_value(), 1);
        assert_eq!(PropertyId::ContentType.to_value(), 3);
        assert_eq!(PropertyId::SubscriptionIdentifier.to_value(), 11);
        assert_eq!(PropertyId::SessionExpiryInterval.to_value(), 17);
        assert_eq!(PropertyId::TopicAlias.to_value(), 35);
        assert_eq!(PropertyId::UserProperty.to_value(), 38);
    }

    #[test]
    fn value_kind_matches_table_2_4() {
        // Spec §2.2.2.2 Table 2-4.
        assert_eq!(
            PropertyId::PayloadFormatIndicator.value_kind(),
            PropertyValueKind::Byte
        );
        assert_eq!(
            PropertyId::MessageExpiryInterval.value_kind(),
            PropertyValueKind::FourByteInt
        );
        assert_eq!(
            PropertyId::ContentType.value_kind(),
            PropertyValueKind::Utf8String
        );
        assert_eq!(
            PropertyId::CorrelationData.value_kind(),
            PropertyValueKind::BinaryData
        );
        assert_eq!(
            PropertyId::SubscriptionIdentifier.value_kind(),
            PropertyValueKind::VariableByteInt
        );
        assert_eq!(
            PropertyId::ReceiveMaximum.value_kind(),
            PropertyValueKind::TwoByteInt
        );
        assert_eq!(
            PropertyId::UserProperty.value_kind(),
            PropertyValueKind::Utf8StringPair
        );
    }

    #[test]
    fn decode_two_byte_int_round_trip() {
        let bytes = 0xCAFEu16.to_be_bytes();
        assert_eq!(
            PropertyValueKind::decode_two_byte_int(&bytes).expect("ok"),
            0xCAFE
        );
    }

    #[test]
    fn decode_two_byte_int_truncated_rejected() {
        assert!(PropertyValueKind::decode_two_byte_int(&[0x01]).is_err());
    }

    #[test]
    fn decode_four_byte_int_round_trip() {
        let bytes = 0xDEADBEEFu32.to_be_bytes();
        assert_eq!(
            PropertyValueKind::decode_four_byte_int(&bytes).expect("ok"),
            0xDEADBEEF
        );
    }

    #[test]
    fn decode_utf8_string_round_trip() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&5u16.to_be_bytes());
        bytes.extend_from_slice(b"hello");
        assert_eq!(
            PropertyValueKind::decode_utf8_string(&bytes).expect("ok"),
            "hello"
        );
    }

    #[test]
    fn decode_utf8_string_invalid_utf8_rejected() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.push(0xff);
        assert!(PropertyValueKind::decode_utf8_string(&bytes).is_err());
    }

    #[test]
    fn decode_utf8_string_truncated_rejected() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&10u16.to_be_bytes());
        bytes.extend_from_slice(b"hi"); // claims 10, gives 2
        assert!(PropertyValueKind::decode_utf8_string(&bytes).is_err());
    }

    #[test]
    fn decode_binary_data_round_trip() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&3u16.to_be_bytes());
        bytes.extend_from_slice(&[0x01, 0x02, 0x03]);
        assert_eq!(
            PropertyValueKind::decode_binary_data(&bytes).expect("ok"),
            &[0x01, 0x02, 0x03]
        );
    }
}
