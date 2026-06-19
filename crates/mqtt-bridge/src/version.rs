// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT protocol version (§3.1.2.2 Protocol Level).
//!
//! MQTT 5.0 (level 5) and MQTT 3.1.1 (level 4) share the same control-packet
//! framing (§2.1) but differ in the variable header / payload: MQTT 3.1.1 has
//! **no property blocks** (added in 5.0), a 2-byte CONNACK, a UNSUBACK without
//! reason codes, and an empty DISCONNECT body. The version-aware codecs in
//! [`crate::control_packets`] and [`crate::codec`] branch on this enum so a
//! single broker/client can speak both wire dialects.

/// The negotiated MQTT protocol version, identified by the CONNECT Protocol
/// Level byte (§3.1.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProtocolVersion {
    /// MQTT 3.1.1 (OASIS Standard, protocol level **4**). No property blocks.
    V311,
    /// MQTT 5.0 (OASIS Standard, protocol level **5**).
    V5,
}

impl ProtocolVersion {
    /// The Protocol Level wire byte (§3.1.2.2): 4 for 3.1.1, 5 for 5.0.
    #[must_use]
    pub const fn level(self) -> u8 {
        match self {
            Self::V311 => 4,
            Self::V5 => 5,
        }
    }

    /// Maps a Protocol Level byte to a version, or `None` if unsupported.
    #[must_use]
    pub const fn from_level(level: u8) -> Option<Self> {
        match level {
            4 => Some(Self::V311),
            5 => Some(Self::V5),
            _ => None,
        }
    }

    /// The Protocol Name advertised in CONNECT (§3.1.2.1). Both 3.1.1 and 5.0
    /// use `"MQTT"`; only the older 3.1 used `"MQIsdp"` (not supported).
    #[must_use]
    pub const fn protocol_name(self) -> &'static str {
        "MQTT"
    }

    /// Whether this version carries MQTT-5.0 property blocks (§2.2.2). `true`
    /// only for 5.0 — 3.1.1 packets have no properties anywhere.
    #[must_use]
    pub const fn has_properties(self) -> bool {
        matches!(self, Self::V5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_round_trip() {
        assert_eq!(ProtocolVersion::from_level(4), Some(ProtocolVersion::V311));
        assert_eq!(ProtocolVersion::from_level(5), Some(ProtocolVersion::V5));
        assert_eq!(ProtocolVersion::V311.level(), 4);
        assert_eq!(ProtocolVersion::V5.level(), 5);
    }

    #[test]
    fn unsupported_levels_rejected() {
        assert_eq!(ProtocolVersion::from_level(3), None); // MQTT 3.1 (MQIsdp)
        assert_eq!(ProtocolVersion::from_level(0), None);
        assert_eq!(ProtocolVersion::from_level(6), None);
    }

    #[test]
    fn only_v5_has_properties() {
        assert!(ProtocolVersion::V5.has_properties());
        assert!(!ProtocolVersion::V311.has_properties());
    }
}
