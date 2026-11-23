// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT v5.0 Properties — Spec §2.2.2.

use alloc::vec::Vec;

/// Property identifier (Spec §2.2.2.2 Table 2-4): one variant for each of
/// the 27 identifiers of the table, and [`PropertyId::Other`] for a value
/// the table does not define.
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
    /// `23` Request Problem Information (CONNECT).
    RequestProblemInformation,
    /// `24` Will Delay Interval (Will Properties).
    WillDelayInterval,
    /// `25` Request Response Information (CONNECT).
    RequestResponseInformation,
    /// `26` Response Information (CONNACK).
    ResponseInformation,
    /// `28` Server Reference (CONNACK/DISCONNECT).
    ServerReference,
    /// `31` Reason String (CONNACK/PUBACK/PUBREC/PUBREL/PUBCOMP/SUBACK/
    /// UNSUBACK/DISCONNECT/AUTH).
    ReasonString,
    /// `33` Receive Maximum (CONNECT/CONNACK).
    ReceiveMaximum,
    /// `34` Topic Alias Maximum (CONNECT/CONNACK).
    TopicAliasMaximum,
    /// `35` Topic Alias (PUBLISH).
    TopicAlias,
    /// `36` Maximum QoS (CONNACK).
    MaximumQos,
    /// `37` Retain Available (CONNACK).
    RetainAvailable,
    /// `38` User Property (any packet).
    UserProperty,
    /// `39` Maximum Packet Size (CONNECT/CONNACK).
    MaximumPacketSize,
    /// `40` Wildcard Subscription Available (CONNACK).
    WildcardSubscriptionAvailable,
    /// `41` Subscription Identifier Available (CONNACK).
    SubscriptionIdentifierAvailable,
    /// `42` Shared Subscription Available (CONNACK).
    SharedSubscriptionAvailable,
    /// An identifier Table 2-4 does not define. A packet that carries one
    /// is a Malformed Packet (§2.2.2.2); it has no value type.
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
            Self::RequestProblemInformation => 23,
            Self::WillDelayInterval => 24,
            Self::RequestResponseInformation => 25,
            Self::ResponseInformation => 26,
            Self::ServerReference => 28,
            Self::ReasonString => 31,
            Self::ReceiveMaximum => 33,
            Self::TopicAliasMaximum => 34,
            Self::TopicAlias => 35,
            Self::MaximumQos => 36,
            Self::RetainAvailable => 37,
            Self::UserProperty => 38,
            Self::MaximumPacketSize => 39,
            Self::WildcardSubscriptionAvailable => 40,
            Self::SubscriptionIdentifierAvailable => 41,
            Self::SharedSubscriptionAvailable => 42,
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
            23 => Self::RequestProblemInformation,
            24 => Self::WillDelayInterval,
            25 => Self::RequestResponseInformation,
            26 => Self::ResponseInformation,
            28 => Self::ServerReference,
            31 => Self::ReasonString,
            33 => Self::ReceiveMaximum,
            34 => Self::TopicAliasMaximum,
            35 => Self::TopicAlias,
            36 => Self::MaximumQos,
            37 => Self::RetainAvailable,
            38 => Self::UserProperty,
            39 => Self::MaximumPacketSize,
            40 => Self::WildcardSubscriptionAvailable,
            41 => Self::SubscriptionIdentifierAvailable,
            42 => Self::SharedSubscriptionAvailable,
            other => Self::Other(other),
        }
    }

    /// Spec §2.2.2.2 Table 2-4 — the value type of the identifier, `None`
    /// for an identifier the table does not define ([`PropertyId::Other`]),
    /// whose value cannot be delimited. The caller decodes the
    /// `Property::value` slice according to this classification.
    #[must_use]
    pub const fn value_kind(self) -> Option<PropertyValueKind> {
        use PropertyValueKind as K;
        Some(match self {
            Self::PayloadFormatIndicator => K::Byte,
            Self::MessageExpiryInterval => K::FourByteInt,
            Self::ContentType => K::Utf8String,
            Self::ResponseTopic => K::Utf8String,
            Self::CorrelationData => K::BinaryData,
            Self::SubscriptionIdentifier => K::VariableByteInt,
            Self::SessionExpiryInterval => K::FourByteInt,
            Self::AssignedClientIdentifier => K::Utf8String,
            Self::ServerKeepAlive => K::TwoByteInt,
            Self::AuthenticationMethod => K::Utf8String,
            Self::AuthenticationData => K::BinaryData,
            Self::RequestProblemInformation => K::Byte,
            Self::WillDelayInterval => K::FourByteInt,
            Self::RequestResponseInformation => K::Byte,
            Self::ResponseInformation => K::Utf8String,
            Self::ServerReference => K::Utf8String,
            Self::ReasonString => K::Utf8String,
            Self::ReceiveMaximum => K::TwoByteInt,
            Self::TopicAliasMaximum => K::TwoByteInt,
            Self::TopicAlias => K::TwoByteInt,
            Self::MaximumQos => K::Byte,
            Self::RetainAvailable => K::Byte,
            Self::UserProperty => K::Utf8StringPair,
            Self::MaximumPacketSize => K::FourByteInt,
            Self::WildcardSubscriptionAvailable => K::Byte,
            Self::SubscriptionIdentifierAvailable => K::Byte,
            Self::SharedSubscriptionAvailable => K::Byte,
            Self::Other(_) => return None,
        })
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

    /// §2.2.2.2, Table 2-4: the 27 identifiers, each with its name, its
    /// value both ways and its type, row by row as the table gives them.
    #[test]
    fn mqtt5_2_2_2_all_27_identifiers_of_table_2_4_and_their_types() {
        use PropertyValueKind as K;
        let table = [
            (PropertyId::PayloadFormatIndicator, 1, K::Byte),
            (PropertyId::MessageExpiryInterval, 2, K::FourByteInt),
            (PropertyId::ContentType, 3, K::Utf8String),
            (PropertyId::ResponseTopic, 8, K::Utf8String),
            (PropertyId::CorrelationData, 9, K::BinaryData),
            (PropertyId::SubscriptionIdentifier, 11, K::VariableByteInt),
            (PropertyId::SessionExpiryInterval, 17, K::FourByteInt),
            (PropertyId::AssignedClientIdentifier, 18, K::Utf8String),
            (PropertyId::ServerKeepAlive, 19, K::TwoByteInt),
            (PropertyId::AuthenticationMethod, 21, K::Utf8String),
            (PropertyId::AuthenticationData, 22, K::BinaryData),
            (PropertyId::RequestProblemInformation, 23, K::Byte),
            (PropertyId::WillDelayInterval, 24, K::FourByteInt),
            (PropertyId::RequestResponseInformation, 25, K::Byte),
            (PropertyId::ResponseInformation, 26, K::Utf8String),
            (PropertyId::ServerReference, 28, K::Utf8String),
            (PropertyId::ReasonString, 31, K::Utf8String),
            (PropertyId::ReceiveMaximum, 33, K::TwoByteInt),
            (PropertyId::TopicAliasMaximum, 34, K::TwoByteInt),
            (PropertyId::TopicAlias, 35, K::TwoByteInt),
            (PropertyId::MaximumQos, 36, K::Byte),
            (PropertyId::RetainAvailable, 37, K::Byte),
            (PropertyId::UserProperty, 38, K::Utf8StringPair),
            (PropertyId::MaximumPacketSize, 39, K::FourByteInt),
            (PropertyId::WildcardSubscriptionAvailable, 40, K::Byte),
            (PropertyId::SubscriptionIdentifierAvailable, 41, K::Byte),
            (PropertyId::SharedSubscriptionAvailable, 42, K::Byte),
        ];
        assert_eq!(table.len(), 27);
        for (id, value, kind) in table {
            assert_eq!(id.to_value(), value, "{id:?}");
            assert_eq!(PropertyId::from_value(value), id);
            assert_eq!(id.value_kind(), Some(kind), "{id:?}");
        }
        // Every value between 1 and 42 that is not in the table is Other,
        // and every identifier of the table is named.
        let named = table.iter().map(|(_, v, _)| *v).collect::<Vec<u32>>();
        for v in 0..=42u32 {
            let id = PropertyId::from_value(v);
            assert_eq!(
                matches!(id, PropertyId::Other(_)),
                !named.contains(&v),
                "{v}"
            );
        }
    }

    /// §2.2.2.2: an identifier outside Table 2-4 keeps its value and has
    /// no value type, so its value cannot be delimited (a Malformed
    /// Packet).
    #[test]
    fn mqtt5_2_2_2_an_identifier_outside_table_2_4_has_no_value_type() {
        for v in [0u32, 4, 7, 10, 12, 16, 20, 27, 29, 30, 32, 43, 99] {
            assert_eq!(PropertyId::from_value(v), PropertyId::Other(v));
            assert_eq!(PropertyId::Other(v).to_value(), v);
            assert_eq!(PropertyId::Other(v).value_kind(), None, "{v}");
        }
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
