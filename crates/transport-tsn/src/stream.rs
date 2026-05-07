// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TSN Stream Identifier + Talker/Listener — Spec §7.2.3 Tab 7.15 + 7.24.

use alloc::string::String;

use crate::data_frame::DataFrameSpecification;
use crate::mac::MacAddress;
use crate::time_aware::TimeAware;
use crate::traffic::TrafficSpecification;
use crate::vlan_tag::Ieee802VlanTag;

/// IEEE 802.1CB Stream-Identifier — Spec §7.2.3 Tab 7.15.
///
/// Eindeutig fuer einen TSN-Stream: Destination-MAC + VLAN-Tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamIdentifier {
    /// Destination MAC.
    pub destination_mac: MacAddress,
    /// VLAN-Tag (TPID/PCP/DEI/VID).
    pub vlan_tag: Ieee802VlanTag,
}

/// Spec §7.2.3 Tab 7.15 — `TsnTalker` (sendende Seite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsnTalker {
    /// Identifier-String fuer den Talker (z.B.
    /// `"tsn_talker_publisher_1"`).
    pub name: String,
    /// Stream-Identifier.
    pub stream: StreamIdentifier,
    /// Traffic-Spec (Bandwidth + Frame-Size).
    pub traffic: TrafficSpecification,
    /// `Some` fuer Time-Aware-Streams; `None` fuer Non-Time-Aware.
    pub time_aware: Option<TimeAware>,
    /// Frame-Header-Spezifikation.
    pub data_frame: DataFrameSpecification,
}

/// Spec §7.2.3 Tab 7.24 — `TsnListener` (empfangende Seite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsnListener {
    /// Identifier-String.
    pub name: String,
    /// Stream-Identifier (muss matchen mit Talker).
    pub stream: StreamIdentifier,
}

impl TsnListener {
    /// Spec §7.2.3 — Listener subscribed zu Stream wenn Stream-IDs
    /// gleich sind.
    #[must_use]
    pub fn matches(&self, talker: &TsnTalker) -> bool {
        self.stream == talker.stream
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::data_frame::IPv4Tuple;
    use crate::traffic::TransmissionSelection;
    use crate::vlan_tag::TPID_8021Q;

    fn make_stream(vid: u16) -> StreamIdentifier {
        StreamIdentifier {
            destination_mac: MacAddress::new([0x01, 0x80, 0xC2, 0x00, 0x00, 0x0E]),
            vlan_tag: Ieee802VlanTag::new(TPID_8021Q, 7, false, vid).expect("ok"),
        }
    }

    #[test]
    fn matching_stream_ids_match() {
        let s = make_stream(100);
        let talker = TsnTalker {
            name: String::from("t1"),
            stream: s,
            traffic: TrafficSpecification {
                interval_nanoseconds: 1_000_000,
                max_frame_size: 1500,
                max_frames_per_interval: 1,
                transmission_selection: TransmissionSelection::StrictPriority,
            },
            time_aware: None,
            data_frame: DataFrameSpecification::Mac {
                source: MacAddress::new([2; 6]),
                destination: MacAddress::new([1; 6]),
            },
        };
        let listener = TsnListener {
            name: String::from("l1"),
            stream: s,
        };
        assert!(listener.matches(&talker));
    }

    #[test]
    fn different_vlan_streams_do_not_match() {
        let talker = TsnTalker {
            name: String::from("t1"),
            stream: make_stream(100),
            traffic: TrafficSpecification {
                interval_nanoseconds: 1_000_000,
                max_frame_size: 64,
                max_frames_per_interval: 1,
                transmission_selection: TransmissionSelection::StrictPriority,
            },
            time_aware: None,
            data_frame: DataFrameSpecification::IPv4(IPv4Tuple {
                source_ip: [192, 168, 1, 1],
                destination_ip: [192, 168, 1, 2],
                source_port: 1234,
                destination_port: 5678,
                protocol: 17, // UDP.
            }),
        };
        let listener = TsnListener {
            name: String::from("l1"),
            stream: make_stream(200),
        };
        assert!(!listener.matches(&talker));
    }

    #[test]
    fn time_aware_talker_can_be_time_critical() {
        // Spec §7.2.3 — TimeAware = Some(...) signals 802.1Qbv.
        let talker = TsnTalker {
            name: String::from("t_time"),
            stream: make_stream(50),
            traffic: TrafficSpecification {
                interval_nanoseconds: 100_000,
                max_frame_size: 256,
                max_frames_per_interval: 1,
                transmission_selection: TransmissionSelection::StrictPriority,
            },
            time_aware: Some(TimeAware {
                earliest_transmit_offset_ns: 1_000,
                latest_transmit_offset_ns: 2_000,
                jitter_ns: 100,
            }),
            data_frame: DataFrameSpecification::Mac {
                source: MacAddress::new([2; 6]),
                destination: MacAddress::new([1; 6]),
            },
        };
        assert!(talker.time_aware.is_some());
        assert!(talker.time_aware.expect("ta").is_valid());
    }
}
