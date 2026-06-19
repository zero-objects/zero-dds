// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DataFrameSpecification — Spec §7.2.3 Tab 7.19/7.22/7.23.

use crate::mac::MacAddress;

/// Spec §7.2.3 Tab 7.22 — IPv4 5-tuple filter (plus DSCP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IPv4Tuple {
    /// `source_ip_address` (4 bytes BE). `0.0.0.0` means: ignore the
    /// source IP in stream identification.
    pub source_ip: [u8; 4],
    /// `destination_ip_address` (4 bytes BE).
    pub destination_ip: [u8; 4],
    /// `dscp` (`UInt8`) — Differentiated Services Code Point (RFC 2474).
    /// Value `64` means: ignore DSCP in stream identification.
    pub dscp: u8,
    /// `protocol` (`UInt16` in the spec; IANA number, 6=TCP, 17=UDP).
    pub protocol: u8,
    /// `source_port` (UDP/TCP).
    pub source_port: u16,
    /// `destination_port` (UDP/TCP).
    pub destination_port: u16,
}

/// Spec §7.2.3 Tab 7.23 — IPv6 5-tuple filter (plus DSCP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IPv6Tuple {
    /// `source_ip_address` (16 bytes BE). The unspecified address
    /// `::` means: ignore the source IP in stream identification.
    pub source_ip: [u8; 16],
    /// `destination_ip_address` (16 bytes BE).
    pub destination_ip: [u8; 16],
    /// `dscp` (`UInt8`) — RFC 2474. Value `64` = ignore.
    pub dscp: u8,
    /// `protocol` (IANA number).
    pub protocol: u8,
    /// `source_port`.
    pub source_port: u16,
    /// `destination_port`.
    pub destination_port: u16,
}

/// Spec §7.2.3 Tab 7.19 — `DataFrameSpecification` variant.
///
/// Describes the frame header filter with which the TSN bridge
/// assigns frames to the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataFrameSpecification {
    /// MAC-only filter (Ethernet PSM, Spec Annex A).
    Mac {
        /// Source MAC.
        source: MacAddress,
        /// Destination MAC.
        destination: MacAddress,
    },
    /// IPv4 5-tuple filter (UDP/IP PSM).
    IPv4(IPv4Tuple),
    /// IPv6 5-tuple filter (UDP/IP PSM).
    IPv6(IPv6Tuple),
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_tuple_carries_5_tuple_fields() {
        let t = IPv4Tuple {
            source_ip: [10, 0, 0, 1],
            destination_ip: [10, 0, 0, 2],
            dscp: 46, // Expedited Forwarding.
            protocol: 17,
            source_port: 1024,
            destination_port: 7400,
        };
        assert_eq!(t.protocol, 17);
        assert_eq!(t.destination_port, 7400);
        assert_eq!(t.dscp, 46);
    }

    #[test]
    fn ipv6_tuple_uses_16_byte_addresses() {
        let mut src = [0u8; 16];
        src[15] = 1;
        let t = IPv6Tuple {
            source_ip: src,
            destination_ip: [0xFFu8; 16],
            dscp: 64, // 64 = ignore.
            protocol: 17,
            source_port: 1024,
            destination_port: 7401,
        };
        assert_eq!(t.source_ip[15], 1);
        assert_eq!(t.destination_ip, [0xFF; 16]);
        assert_eq!(t.dscp, 64);
    }

    #[test]
    fn mac_specification_carries_source_and_destination() {
        let spec = DataFrameSpecification::Mac {
            source: MacAddress::new([1, 2, 3, 4, 5, 6]),
            destination: MacAddress::new([7, 8, 9, 10, 11, 12]),
        };
        match spec {
            DataFrameSpecification::Mac {
                source,
                destination,
            } => {
                assert_eq!(source.0, [1, 2, 3, 4, 5, 6]);
                assert_eq!(destination.0, [7, 8, 9, 10, 11, 12]);
            }
            _ => panic!("expected mac"),
        }
    }
}
