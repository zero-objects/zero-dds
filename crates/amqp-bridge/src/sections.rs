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

    // OASIS AMQP 1.0 Part 3 (`amqp-1.0-messaging`) §3.2: each section is a
    // described type, 0x00, the descriptor code as a smallulong (0x53,
    // header 0x70 to footer 0x78), then its value (Part 1, §1.2).

    /// `section` encodes to `octets`, and `octets` decode to `section`.
    fn wire(section: MessageSection, octets: &[u8]) {
        assert_eq!(section.encode().expect("encode"), octets, "{section:?}");
        assert_eq!(
            MessageSection::decode(octets).expect("decode"),
            (section, octets.len())
        );
    }

    /// §3.2.1: header 0x70, a list (here `durable` true).
    #[test]
    fn amqp10_3_2_1_the_header_is_0x70_and_a_list() {
        wire(
            MessageSection::Header(AmqpExtValue::List(alloc::vec![AmqpExtValue::Boolean(true)])),
            &[0x00, 0x53, 0x70, 0xc0, 0x02, 0x01, 0x41],
        );
    }

    /// §3.2.4: properties 0x73, a list in field order (message-id,
    /// user-id, to, subject, ...).
    #[test]
    fn amqp10_3_2_4_the_properties_are_0x73_and_a_list_in_field_order() {
        let mut octets = alloc::vec![
            0x00, 0x53, 0x73, 0xc0, 0x14, 0x04, 0x40, 0x40, 0x40, 0xa1, 0x0e
        ];
        octets.extend_from_slice(b"TrackingResult");
        wire(
            MessageSection::Properties(AmqpExtValue::List(alloc::vec![
                AmqpExtValue::Null,
                AmqpExtValue::Null,
                AmqpExtValue::Null,
                AmqpExtValue::Str("TrackingResult".to_string()),
            ])),
            &octets,
        );
    }

    /// §3.2.5: application-properties 0x74, a map.
    #[test]
    fn amqp10_3_2_5_the_application_properties_are_0x74_and_a_map() {
        let mut octets = alloc::vec![0x00, 0x53, 0x74, 0xc1, 0x12, 0x02, 0xa1, 0x0d];
        octets.extend_from_slice(b"dds:domain-id");
        octets.extend_from_slice(&[0x54, 0x00]);
        wire(
            MessageSection::ApplicationProperties(AmqpExtValue::Map(alloc::vec![(
                AmqpExtValue::Str("dds:domain-id".to_string()),
                AmqpExtValue::Int(0),
            )])),
            &octets,
        );
    }

    /// §3.2.6: data 0x75, a binary.
    #[test]
    fn amqp10_3_2_6_data_is_0x75_and_a_binary() {
        wire(
            MessageSection::Data(alloc::vec![0xde, 0xad, 0xbe, 0xef]),
            &[0x00, 0x53, 0x75, 0xa0, 0x04, 0xde, 0xad, 0xbe, 0xef],
        );
    }

    /// §3.2.7: amqp-sequence 0x76, a list.
    #[test]
    fn amqp10_3_2_7_an_amqp_sequence_is_0x76_and_a_list() {
        wire(
            MessageSection::AmqpSequence(alloc::vec![
                AmqpExtValue::Int(1),
                AmqpExtValue::Int(2),
                AmqpExtValue::Int(3),
            ]),
            &[
                0x00, 0x53, 0x76, 0xc0, 0x07, 0x03, 0x54, 0x01, 0x54, 0x02, 0x54, 0x03,
            ],
        );
    }

    /// §3.2.8: amqp-value 0x77, any value.
    #[test]
    fn amqp10_3_2_8_an_amqp_value_is_0x77_and_any_value() {
        wire(
            MessageSection::AmqpValue(AmqpExtValue::Str("hello".to_string())),
            &[0x00, 0x53, 0x77, 0xa1, 0x05, b'h', b'e', b'l', b'l', b'o'],
        );
    }

    /// §3.2.9: footer 0x78, a map.
    #[test]
    fn amqp10_3_2_9_the_footer_is_0x78_and_a_map() {
        wire(
            MessageSection::Footer(AmqpExtValue::Map(Vec::new())),
            &[0x00, 0x53, 0x78, 0xc1, 0x01, 0x00],
        );
    }

    /// §3.2: the sections follow in the order header, delivery-annotations,
    /// message-annotations, properties, application-properties, body,
    /// footer.
    #[test]
    fn amqp10_3_2_the_sections_follow_in_their_order() {
        let ordered = alloc::vec![
            MessageSection::Header(AmqpExtValue::Null),
            MessageSection::DeliveryAnnotations(AmqpExtValue::Null),
            MessageSection::MessageAnnotations(AmqpExtValue::Null),
            MessageSection::Properties(AmqpExtValue::Null),
            MessageSection::ApplicationProperties(AmqpExtValue::Null),
            MessageSection::Data(alloc::vec![0]),
            MessageSection::Footer(AmqpExtValue::Null),
        ];
        assert!(validate_section_sequence(&ordered).is_ok());
        let orders: Vec<u8> = ordered.iter().map(MessageSection::order).collect();
        assert_eq!(orders, [0, 1, 2, 3, 4, 5, 6]);
        assert!(
            validate_section_sequence(&[
                MessageSection::Properties(AmqpExtValue::Null),
                MessageSection::Header(AmqpExtValue::Null),
            ])
            .is_err()
        );
    }
}
