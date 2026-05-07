// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE-Object-Kinds (Spec §7.2 Table 4).
//!
//! Jede Object-Variante hat einen 4-Bit-Code, den die niedrigeren 4 Bits
//! der `ObjectId` tragen (siehe `crate::object_id`). Die hier gewaehlten
//! Konstanten entsprechen den `OBJK_*`-Werten der DDS-XRCE-Spec
//! `formal/2020-11-01`.
//!
//! Wir vergeben fuer jeden Spec-Wert einen `pub const u8`, plus eine
//! Convenience-Enum mit `from_u8` / `to_u8`. Die Enum bricht
//! `match`-Exhaustiveness in eigenem Code, der Wire-Round-Trip nutzt
//! aber den `u8`-Repraesentanten direkt.

use crate::error::XrceError;

/// Reservierte Object-Kind: kein gueltiges Objekt (Spec §7.2.1).
pub const OBJK_INVALID: u8 = 0x00;
/// `OBJK_PARTICIPANT` — DomainParticipant.
pub const OBJK_PARTICIPANT: u8 = 0x01;
/// `OBJK_TOPIC` — Topic-Definition.
pub const OBJK_TOPIC: u8 = 0x02;
/// `OBJK_PUBLISHER` — Publisher.
pub const OBJK_PUBLISHER: u8 = 0x03;
/// `OBJK_SUBSCRIBER` — Subscriber.
pub const OBJK_SUBSCRIBER: u8 = 0x04;
/// `OBJK_DATAWRITER` — DataWriter.
pub const OBJK_DATAWRITER: u8 = 0x05;
/// `OBJK_DATAREADER` — DataReader.
pub const OBJK_DATAREADER: u8 = 0x06;
/// `OBJK_TYPE` — Type-Description (Spec §7.5.2).
pub const OBJK_TYPE: u8 = 0x0A;
/// `OBJK_QOSPROFILE` — QoS-Profile (Spec §7.5.2).
pub const OBJK_QOSPROFILE: u8 = 0x0B;
/// `OBJK_APPLICATION` — Application-Container (Spec §7.5.2).
pub const OBJK_APPLICATION: u8 = 0x0C;
/// `OBJK_AGENT` — Agent-Singleton (Spec §7.5.2.1).
pub const OBJK_AGENT: u8 = 0x0D;
/// `OBJK_CLIENT` — Client-Object, repraesentiert den ProxyClient (§7.5.1).
pub const OBJK_CLIENT: u8 = 0x0E;
/// `OBJK_DOMAIN` — Domain-Kind (Spec §7.5.2).
pub const OBJK_DOMAIN: u8 = 0x0F;

/// Convenience-Enum aller in der Spec definierten Object-Kinds.
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
    /// Roher 4-Bit-Code als `u8`.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Konvertiert ein 4-Bit-Wert. Werte ausserhalb der Spec → Fehler.
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn `byte` keinem `OBJK_*` entspricht. Werte
    /// `> 0x0F` werden ebenfalls abgelehnt — die ObjectId-Bit-Layer
    /// stellt das normalerweise sicher.
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

    /// `true`, wenn der Object-Kind ein DDS-Endpoint ist
    /// (DataWriter oder DataReader).
    #[must_use]
    pub fn is_endpoint(self) -> bool {
        matches!(self, Self::DataWriter | Self::DataReader)
    }

    /// `true`, wenn der Object-Kind ein DDS-Container ist
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
        // 0x07 ist im 4-bit-Bereich aber nicht in der Spec
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
        // Sanity-Check: Spec §7.2 Table 4
        assert_eq!(OBJK_PARTICIPANT, 0x01);
        assert_eq!(OBJK_TOPIC, 0x02);
        assert_eq!(OBJK_DATAWRITER, 0x05);
        assert_eq!(OBJK_DATAREADER, 0x06);
        assert_eq!(OBJK_AGENT, 0x0D);
        assert_eq!(OBJK_CLIENT, 0x0E);
    }
}
