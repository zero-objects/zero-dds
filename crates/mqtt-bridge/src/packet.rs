// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT Control Packet Types + Fixed Header — Spec §2.1.

/// `Control Packet Type` (Spec §2.1.2 Table 2-1) — 4 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlPacketType {
    /// `1` CONNECT — client request to connect (Spec §3.1).
    Connect,
    /// `2` CONNACK — connect acknowledgment (Spec §3.2).
    ConnAck,
    /// `3` PUBLISH — publish message (Spec §3.3).
    Publish,
    /// `4` PUBACK — publish acknowledgment (Spec §3.4).
    PubAck,
    /// `5` PUBREC — publish received (QoS 2 part 1, Spec §3.5).
    PubRec,
    /// `6` PUBREL — publish release (QoS 2 part 2, Spec §3.6).
    PubRel,
    /// `7` PUBCOMP — publish complete (QoS 2 part 3, Spec §3.7).
    PubComp,
    /// `8` SUBSCRIBE — client subscribe (Spec §3.8).
    Subscribe,
    /// `9` SUBACK — subscribe acknowledgment (Spec §3.9).
    SubAck,
    /// `10` UNSUBSCRIBE — client unsubscribe (Spec §3.10).
    Unsubscribe,
    /// `11` UNSUBACK — unsubscribe acknowledgment (Spec §3.11).
    UnsubAck,
    /// `12` PINGREQ — client ping (Spec §3.12).
    PingReq,
    /// `13` PINGRESP — server ping response (Spec §3.13).
    PingResp,
    /// `14` DISCONNECT — client/server disconnect (Spec §3.14).
    Disconnect,
    /// `15` AUTH — extended authentication (new in MQTT 5.0, Spec §3.15).
    Auth,
}

impl ControlPacketType {
    /// Wire value (4 bits, placed in the high nibble of byte 0).
    #[must_use]
    pub const fn to_bits(self) -> u8 {
        match self {
            Self::Connect => 1,
            Self::ConnAck => 2,
            Self::Publish => 3,
            Self::PubAck => 4,
            Self::PubRec => 5,
            Self::PubRel => 6,
            Self::PubComp => 7,
            Self::Subscribe => 8,
            Self::SubAck => 9,
            Self::Unsubscribe => 10,
            Self::UnsubAck => 11,
            Self::PingReq => 12,
            Self::PingResp => 13,
            Self::Disconnect => 14,
            Self::Auth => 15,
        }
    }

    /// Converts from the wire nibble.
    #[must_use]
    pub const fn from_bits(v: u8) -> Option<Self> {
        match v & 0x0F {
            1 => Some(Self::Connect),
            2 => Some(Self::ConnAck),
            3 => Some(Self::Publish),
            4 => Some(Self::PubAck),
            5 => Some(Self::PubRec),
            6 => Some(Self::PubRel),
            7 => Some(Self::PubComp),
            8 => Some(Self::Subscribe),
            9 => Some(Self::SubAck),
            10 => Some(Self::Unsubscribe),
            11 => Some(Self::UnsubAck),
            12 => Some(Self::PingReq),
            13 => Some(Self::PingResp),
            14 => Some(Self::Disconnect),
            15 => Some(Self::Auth),
            _ => None,
        }
    }
}

/// Fixed Header (Spec §2.1.1) — `<type-nibble><flags-nibble>` byte +
/// VBI Remaining-Length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedHeader {
    /// Spec §2.1.2.
    pub packet_type: ControlPacketType,
    /// Spec §2.1.3 — flags low nibble. For PUBLISH = `DUP|QoS(2)|
    /// RETAIN`. For others usually 0 (PUBREL/SUBSCRIBE/UNSUBSCRIBE
    /// have a fixed `0010`).
    pub flags: u8,
    /// Spec §2.1.4 — remaining length (VBI).
    pub remaining_length: u32,
}

impl FixedHeader {
    /// Spec §2.1.3 — for PUBLISH: extracts the DUP flag from the
    /// flag bits.
    #[must_use]
    pub const fn dup_flag(self) -> bool {
        self.flags & 0b1000 != 0
    }

    /// Spec §2.1.3 — for PUBLISH: extracts the QoS value (0..=2).
    #[must_use]
    pub const fn qos(self) -> u8 {
        (self.flags >> 1) & 0b11
    }

    /// Spec §2.1.3 — for PUBLISH: extracts the RETAIN flag.
    #[must_use]
    pub const fn retain_flag(self) -> bool {
        self.flags & 0b1 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_type_round_trip_via_bits() {
        for t in [
            ControlPacketType::Connect,
            ControlPacketType::ConnAck,
            ControlPacketType::Publish,
            ControlPacketType::PubAck,
            ControlPacketType::PubRec,
            ControlPacketType::PubRel,
            ControlPacketType::PubComp,
            ControlPacketType::Subscribe,
            ControlPacketType::SubAck,
            ControlPacketType::Unsubscribe,
            ControlPacketType::UnsubAck,
            ControlPacketType::PingReq,
            ControlPacketType::PingResp,
            ControlPacketType::Disconnect,
            ControlPacketType::Auth,
        ] {
            assert_eq!(ControlPacketType::from_bits(t.to_bits()), Some(t));
        }
    }

    #[test]
    fn packet_type_zero_is_reserved() {
        // Spec §2.1.2 Table 2-1 — Code 0 is Reserved.
        assert_eq!(ControlPacketType::from_bits(0), None);
    }

    #[test]
    fn publish_header_decodes_dup_qos_retain_flags() {
        // Spec §2.1.3 — flags layout.
        let h = FixedHeader {
            packet_type: ControlPacketType::Publish,
            flags: 0b1010, // DUP=1, QoS=1, RETAIN=0.
            remaining_length: 0,
        };
        assert!(h.dup_flag());
        assert_eq!(h.qos(), 1);
        assert!(!h.retain_flag());

        let h2 = FixedHeader {
            packet_type: ControlPacketType::Publish,
            flags: 0b0101, // DUP=0, QoS=2, RETAIN=1.
            remaining_length: 0,
        };
        assert!(!h2.dup_flag());
        assert_eq!(h2.qos(), 2);
        assert!(h2.retain_flag());
    }
}
