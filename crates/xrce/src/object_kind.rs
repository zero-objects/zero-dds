// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE-Object-Kinds (Spec §7.2 Table 4).
//!
//! Each object variant has a 4-bit code carried by the lower 4 bits
//! of the `ObjectId` (see `crate::object_id`). The constants chosen here
//! correspond to the `OBJK_*` values of the DDS-XRCE spec
//! `formal/2020-11-01`.
//!
//! We assign a `pub const u8` for each spec value, plus a
//! convenience enum with `from_u8` / `to_u8`. The enum gives
//! `match` exhaustiveness in our own code, while the wire round-trip
//! uses the `u8` representative directly.

use crate::error::XrceError;

/// Reserved object kind: no valid object (Spec §7.2.1).
pub const OBJK_INVALID: u8 = 0x00;
/// `OBJK_PARTICIPANT` — DomainParticipant.
pub const OBJK_PARTICIPANT: u8 = 0x01;
/// `OBJK_TOPIC` — topic definition.
pub const OBJK_TOPIC: u8 = 0x02;
/// `OBJK_PUBLISHER` — Publisher.
pub const OBJK_PUBLISHER: u8 = 0x03;
/// `OBJK_SUBSCRIBER` — Subscriber.
pub const OBJK_SUBSCRIBER: u8 = 0x04;
/// `OBJK_DATAWRITER` — DataWriter.
pub const OBJK_DATAWRITER: u8 = 0x05;
/// `OBJK_DATAREADER` — DataReader.
pub const OBJK_DATAREADER: u8 = 0x06;
/// `OBJK_TYPE` — type description (Spec §7.5.2).
pub const OBJK_TYPE: u8 = 0x0A;
/// `OBJK_QOSPROFILE` — QoS profile (Spec §7.5.2).
pub const OBJK_QOSPROFILE: u8 = 0x0B;
/// `OBJK_APPLICATION` — application container (Spec §7.5.2).
pub const OBJK_APPLICATION: u8 = 0x0C;
/// `OBJK_AGENT` — agent singleton (Spec §7.5.2.1).
pub const OBJK_AGENT: u8 = 0x0D;
/// `OBJK_CLIENT` — client object, represents the ProxyClient (§7.5.1).
pub const OBJK_CLIENT: u8 = 0x0E;
/// `OBJK_DOMAIN` — domain kind (Spec §7.5.2).
pub const OBJK_DOMAIN: u8 = 0x0F;

/// Convenience enum of all object kinds defined in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum ObjectKind {
    Invalid = OBJK_INVALID,
    Participant = OBJK_PARTICIPANT,
    Topic = OBJK_TOPIC,
    Publisher = OBJK_PUBLISHER,
    Subscriber = OBJK_SUBSCRIBER,
    DataWriter = OBJK_DATAWRITER,
    DataReader = OBJK_DATAREADER,
    Type = OBJK_TYPE,
    QosProfile = OBJK_QOSPROFILE,
    Application = OBJK_APPLICATION,
    Agent = OBJK_AGENT,
    Client = OBJK_CLIENT,
    Domain = OBJK_DOMAIN,
}

impl ObjectKind {
    /// Raw 4-bit code as `u8`.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Converts a 4-bit value. Values outside the spec → error.
    ///
    /// # Errors
    /// `ValueOutOfRange` if `byte` corresponds to no `OBJK_*`. Values
    /// `> 0x0F` are rejected as well — the ObjectId bit layer
    /// normally ensures this.
    pub fn from_u8(byte: u8) -> Result<Self, XrceError> {
        match byte {
            OBJK_INVALID => Ok(Self::Invalid),
            OBJK_PARTICIPANT => Ok(Self::Participant),
            OBJK_TOPIC => Ok(Self::Topic),
            OBJK_PUBLISHER => Ok(Self::Publisher),
            OBJK_SUBSCRIBER => Ok(Self::Subscriber),
            OBJK_DATAWRITER => Ok(Self::DataWriter),
            OBJK_DATAREADER => Ok(Self::DataReader),
            OBJK_TYPE => Ok(Self::Type),
            OBJK_QOSPROFILE => Ok(Self::QosProfile),
            OBJK_APPLICATION => Ok(Self::Application),
            OBJK_AGENT => Ok(Self::Agent),
            OBJK_CLIENT => Ok(Self::Client),
            OBJK_DOMAIN => Ok(Self::Domain),
            _ => Err(XrceError::ValueOutOfRange {
                message: "object kind not in DDS-XRCE spec",
            }),
        }
    }

    /// `true` when the object kind is a DDS endpoint
    /// (DataWriter or DataReader).
    #[must_use]
    pub fn is_endpoint(self) -> bool {
        matches!(self, Self::DataWriter | Self::DataReader)
    }

    /// `true` when the object kind is a DDS container
    /// (Publisher / Subscriber / Participant).
    #[must_use]
    pub fn is_container(self) -> bool {
        matches!(self, Self::Publisher | Self::Subscriber | Self::Participant)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn all_spec_kinds_roundtrip() {
        for k in [
            ObjectKind::Invalid,
            ObjectKind::Participant,
            ObjectKind::Topic,
            ObjectKind::Publisher,
            ObjectKind::Subscriber,
            ObjectKind::DataWriter,
            ObjectKind::DataReader,
            ObjectKind::Type,
            ObjectKind::QosProfile,
            ObjectKind::Application,
            ObjectKind::Agent,
            ObjectKind::Client,
            ObjectKind::Domain,
        ] {
            assert_eq!(ObjectKind::from_u8(k.to_u8()).unwrap(), k);
        }
    }

    #[test]
    fn unknown_byte_rejected() {
        // 0x07 is in the 4-bit range but not in the spec
        assert!(ObjectKind::from_u8(0x07).is_err());
        // > 0x0F → out of 4-bit-range
        assert!(ObjectKind::from_u8(0x10).is_err());
        assert!(ObjectKind::from_u8(0xFF).is_err());
    }

    #[test]
    fn endpoint_classification() {
        assert!(ObjectKind::DataWriter.is_endpoint());
        assert!(ObjectKind::DataReader.is_endpoint());
        assert!(!ObjectKind::Topic.is_endpoint());
        assert!(!ObjectKind::Participant.is_endpoint());
    }

    #[test]
    fn container_classification() {
        assert!(ObjectKind::Publisher.is_container());
        assert!(ObjectKind::Subscriber.is_container());
        assert!(ObjectKind::Participant.is_container());
        assert!(!ObjectKind::DataWriter.is_container());
        assert!(!ObjectKind::Topic.is_container());
    }

    #[test]
    fn raw_const_values_match_spec() {
        // Sanity check: Spec §7.2 Table 4
        assert_eq!(OBJK_PARTICIPANT, 0x01);
        assert_eq!(OBJK_TOPIC, 0x02);
        assert_eq!(OBJK_DATAWRITER, 0x05);
        assert_eq!(OBJK_DATAREADER, 0x06);
        assert_eq!(OBJK_AGENT, 0x0D);
        assert_eq!(OBJK_CLIENT, 0x0E);
    }
}
