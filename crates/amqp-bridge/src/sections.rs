// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 1.0 Message Sections — Spec `amqp-1.0-messaging` §3.
//!
//! The 7 message sections (Header, Delivery-Annotations, Message-
//! Annotations, Properties, Application-Properties, Body, Footer)
//! are described composites with a descriptor code (ulong) + body
//! (list or variant). [`validate_section_sequence`] enforces the
//! AMQP-Messaging §3.2 ordering constraint.
//!
//! Cross-Ref: DDS-AMQP-1.0 §8.2 Properties-Section-Mapping + §8.3
//! Application-Properties-Mapping.

use alloc::vec::Vec;

use crate::extended_types::AmqpExtValue;
use crate::performatives::{decode_performative, encode_performative};
use crate::types::TypeError;

/// Spec §3.2 — message-section descriptor codes.
pub mod descriptor {
    /// `0x70` — Header.
    pub const HEADER: u64 = 0x0000_0000_0000_0070;
    /// `0x71` — Delivery-Annotations.
    pub const DELIVERY_ANNOTATIONS: u64 = 0x0000_0000_0000_0071;
    /// `0x72` — Message-Annotations.
    pub const MESSAGE_ANNOTATIONS: u64 = 0x0000_0000_0000_0072;
    /// `0x73` — Properties.
    pub const PROPERTIES: u64 = 0x0000_0000_0000_0073;
    /// `0x74` — Application-Properties.
    pub const APPLICATION_PROPERTIES: u64 = 0x0000_0000_0000_0074;
    /// `0x75` — Data (raw octet body).
    pub const DATA: u64 = 0x0000_0000_0000_0075;
    /// `0x76` — Amqp-Sequence (sequence body).
    pub const AMQP_SEQUENCE: u64 = 0x0000_0000_0000_0076;
    /// `0x77` — Amqp-Value (typed value body).
    pub const AMQP_VALUE: u64 = 0x0000_0000_0000_0077;
    /// `0x78` — Footer.
    pub const FOOTER: u64 = 0x0000_0000_0000_0078;
}

/// Spec §3.2 — high-level Message-Section enum.
#[derive(Debug, Clone, PartialEq)]
pub enum MessageSection {
    /// Spec §3.2.1
    Header(AmqpExtValue),
    /// Spec §3.2.2
    DeliveryAnnotations(AmqpExtValue),
    /// Spec §3.2.3
    MessageAnnotations(AmqpExtValue),
    /// Spec §3.2.4
    Properties(AmqpExtValue),
    /// Spec §3.2.5
    ApplicationProperties(AmqpExtValue),
    /// Spec §3.2.6 Data
    Data(Vec<u8>),
    /// Spec §3.2.7 Amqp-Sequence
    AmqpSequence(Vec<AmqpExtValue>),
    /// Spec §3.2.8 Amqp-Value
    AmqpValue(AmqpExtValue),
    /// Spec §3.2.9 Footer
    Footer(AmqpExtValue),
}

impl MessageSection {
    /// Encodes section to wire format.
    ///
    /// # Errors
    /// see [`TypeError`].
    pub fn encode(&self) -> Result<Vec<u8>, TypeError> {
        let (desc, body) = match self {
            Self::Header(v) => (descriptor::HEADER, v.clone()),
            Self::DeliveryAnnotations(v) => (descriptor::DELIVERY_ANNOTATIONS, v.clone()),
            Self::MessageAnnotations(v) => (descriptor::MESSAGE_ANNOTATIONS, v.clone()),
            Self::Properties(v) => (descriptor::PROPERTIES, v.clone()),
            Self::ApplicationProperties(v) => (descriptor::APPLICATION_PROPERTIES, v.clone()),
            Self::Data(b) => (descriptor::DATA, AmqpExtValue::Binary(b.clone())),
            Self::AmqpSequence(items) => {
                (descriptor::AMQP_SEQUENCE, AmqpExtValue::List(items.clone()))
            }
            Self::AmqpValue(v) => (descriptor::AMQP_VALUE, v.clone()),
            Self::Footer(v) => (descriptor::FOOTER, v.clone()),
        };
        encode_performative(desc, &body)
    }

    /// Decodes a single section from bytes.
    ///
    /// Returns (section, consumed_bytes).
    ///
    /// # Errors
    /// see [`TypeError`].
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), TypeError> {
        let (desc, body, n) = decode_performative(bytes)?;
        let s = match desc {
            descriptor::HEADER => Self::Header(body),
            descriptor::DELIVERY_ANNOTATIONS => Self::DeliveryAnnotations(body),
            descriptor::MESSAGE_ANNOTATIONS => Self::MessageAnnotations(body),
            descriptor::PROPERTIES => Self::Properties(body),
            descriptor::APPLICATION_PROPERTIES => Self::ApplicationProperties(body),
            descriptor::DATA => match body {
                AmqpExtValue::Binary(b) => Self::Data(b),
                _ => return Err(TypeError::UnsupportedFormatCode(0)),
            },
            descriptor::AMQP_SEQUENCE => match body {
                AmqpExtValue::List(items) => Self::AmqpSequence(items),
                _ => return Err(TypeError::UnsupportedFormatCode(0)),
            },
            descriptor::AMQP_VALUE => Self::AmqpValue(body),
            descriptor::FOOTER => Self::Footer(body),
            _ => return Err(TypeError::UnsupportedFormatCode(0)),
        };
        Ok((s, n))
    }

    /// Spec §3.2 — Section-Sequencing-Order-Index.
    /// Sections MUST appear in this order: header, delivery-annotations,
    /// message-annotations, properties, application-properties, body
    /// (one of data/sequence/value), footer.
    #[must_use]
    pub fn order(&self) -> u8 {
        match self {
            Self::Header(_) => 0,
            Self::DeliveryAnnotations(_) => 1,
            Self::MessageAnnotations(_) => 2,
            Self::Properties(_) => 3,
            Self::ApplicationProperties(_) => 4,
            Self::Data(_) | Self::AmqpSequence(_) | Self::AmqpValue(_) => 5,
            Self::Footer(_) => 6,
        }
    }
}

/// Validates that a Section sequence appears in canonical order
/// (Spec §3.2 sequencing-rule).
///
/// # Errors
/// `Truncated` (reused as a generic sequencing error) if the
/// order is violated.
pub fn validate_section_sequence(sections: &[MessageSection]) -> Result<(), TypeError> {
    let mut last_order = 0;
    for s in sections {
        let o = s.order();
        if o < last_order {
            return Err(TypeError::Truncated);
        }
        last_order = o;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn round(s: MessageSection) {
        let bytes = s.encode().expect("encode");
        let (parsed, _) = MessageSection::decode(&bytes).expect("decode");
        assert_eq!(parsed, s);
    }

    #[test]
    fn header_section_round_trips() {
        round(MessageSection::Header(AmqpExtValue::List(alloc::vec![
            AmqpExtValue::Boolean(true), // durable
        ])));
    }

    #[test]
    fn properties_section_round_trips_with_subject() {
        round(MessageSection::Properties(AmqpExtValue::List(alloc::vec![
            AmqpExtValue::Null,                              // message-id
            AmqpExtValue::Null,                              // user-id
            AmqpExtValue::Null,                              // to
            AmqpExtValue::Str("TrackingResult".to_string()), // subject
        ])));
    }

    #[test]
    fn application_properties_round_trips() {
        round(MessageSection::ApplicationProperties(AmqpExtValue::Map(
            alloc::vec![(
                AmqpExtValue::Str("dds:domain-id".to_string()),
                AmqpExtValue::Int(0),
            )],
        )));
    }

    #[test]
    fn data_section_round_trips_binary() {
        round(MessageSection::Data(alloc::vec![0xDE, 0xAD, 0xBE, 0xEF]));
    }

    #[test]
    fn amqp_value_section_round_trips() {
        round(MessageSection::AmqpValue(AmqpExtValue::Str(
            "hello".to_string(),
        )));
    }

    #[test]
    fn amqp_sequence_round_trips() {
        round(MessageSection::AmqpSequence(alloc::vec![
            AmqpExtValue::Int(1),
            AmqpExtValue::Int(2),
            AmqpExtValue::Int(3),
        ]));
    }

    #[test]
    fn footer_section_round_trips() {
        round(MessageSection::Footer(AmqpExtValue::Map(Vec::new())));
    }

    #[test]
    fn canonical_order_passes_validator() {
        let seq = alloc::vec![
            MessageSection::Header(AmqpExtValue::Null),
            MessageSection::Properties(AmqpExtValue::Null),
            MessageSection::Data(alloc::vec![0]),
            MessageSection::Footer(AmqpExtValue::Null),
        ];
        assert!(validate_section_sequence(&seq).is_ok());
    }

    #[test]
    fn out_of_order_fails_validator() {
        // Properties before Header is wrong.
        let seq = alloc::vec![
            MessageSection::Properties(AmqpExtValue::Null),
            MessageSection::Header(AmqpExtValue::Null),
        ];
        assert!(validate_section_sequence(&seq).is_err());
    }

    #[test]
    fn all_seven_sections_have_unique_order_indexes() {
        let order_set: alloc::collections::BTreeSet<_> = alloc::vec![
            MessageSection::Header(AmqpExtValue::Null).order(),
            MessageSection::DeliveryAnnotations(AmqpExtValue::Null).order(),
            MessageSection::MessageAnnotations(AmqpExtValue::Null).order(),
            MessageSection::Properties(AmqpExtValue::Null).order(),
            MessageSection::ApplicationProperties(AmqpExtValue::Null).order(),
            MessageSection::Data(Vec::new()).order(),
            MessageSection::Footer(AmqpExtValue::Null).order(),
        ]
        .into_iter()
        .collect();
        assert_eq!(order_set.len(), 7);
    }
}
