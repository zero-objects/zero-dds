// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TSN Stream Identifier + Talker/Listener + TsnConfiguration —
//! Spec §7.2.3 Tab 7.15 + 7.24 + Figure 7.3.

use alloc::string::String;
use alloc::vec::Vec;

use crate::data_frame::DataFrameSpecification;
use crate::mac::MacAddress;
use crate::network_requirements::NetworkRequirements;
use crate::time_aware::TimeAware;
use crate::traffic::TrafficSpecification;
use crate::vlan_tag::Ieee802VlanTag;

/// IEEE 802.1CB stream identifier — Spec §7.2.3 Tab 7.15.
///
/// Unique for a TSN stream: destination MAC + VLAN tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamIdentifier {
    /// Destination MAC.
    pub destination_mac: MacAddress,
    /// VLAN tag (TPID/PCP/DEI/VID).
    pub vlan_tag: Ieee802VlanTag,
}

/// Spec §7.2.3 Tab 7.15 — `TsnTalker` (sending side).
///
/// Describes the characteristics of the traffic sent by a
/// time-critical `DataWriter` as well as its requirements on the
/// TSN infrastructure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsnTalker {
    /// `name` (Mult 1) — identifier of the talker within the
    /// `TsnConfiguration`.
    pub name: String,
    /// `stream_name` (Mult 1) — name of the TSN stream this
    /// talker is associated with. Per §8.2.2.2 it is also used as the SEDP
    /// partition name for endpoint restriction.
    pub stream_name: String,
    /// IEEE 802.1CB stream identifier (destination MAC + VLAN) for
    /// concrete L2 stream identification. A runtime convenience;
    /// `matches()` compares talker and listener through it.
    pub stream: StreamIdentifier,
    /// `traffic_specification` (Mult 1) — periodicity + frame size +
    /// shaper.
    pub traffic: TrafficSpecification,
    /// `network_requirements` (Mult 0..1) — optional requirements on
    /// the TSN infrastructure (redundancy + max. latency).
    pub network_requirements: Option<NetworkRequirements>,
    /// `data_frame_specification` (Mult 0..1) — frame header filter for
    /// stream identification. Optional; MAC+VLAN for the Ethernet PSM,
    /// IPv4/IPv6 tuple for the UDP/IP PSM.
    pub data_frame: Option<DataFrameSpecification>,
    /// `datawriter_ref` (Mult 1) — name of the associated `DataWriter`.
    pub datawriter_ref: String,
    /// `time_aware` (Mult 0..1) — `Some` for time-aware streams
    /// (IEEE 802.1Qbv); `None` for non-time-aware.
    pub time_aware: Option<TimeAware>,
}

/// Spec §7.2.3 Tab 7.24 — `TsnListener` (receiving side).
///
/// Describes the requirements of a time-critical `DataReader` on
/// the TSN infrastructure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsnListener {
    /// `name` (Mult 1).
    pub name: String,
    /// `stream_name` (Mult 1) — name of the TSN stream.
    pub stream_name: String,
    /// IEEE 802.1CB stream identifier (must match the talker).
    pub stream: StreamIdentifier,
    /// `network_requirements` (Mult 0..1).
    pub network_requirements: Option<NetworkRequirements>,
    /// `datareader_ref` (Mult 1) — name of the associated `DataReader`.
    pub datareader_ref: String,
}

impl TsnListener {
    /// Spec §7.2.3 — the listener subscribes to a stream if the stream IDs
    /// are equal.
    #[must_use]
    pub fn matches(&self, talker: &TsnTalker) -> bool {
        self.stream == talker.stream
    }
}

/// Spec §7.2.3 Figure 7.3 — `TsnConfiguration`.
///
/// Aggregate root of the TSN configuration model: defines the set
/// of `TsnTalker` and `TsnListener` and associates them with time-
/// sensitive `DataWriter`s and `DataReader`s. Referenced by a
/// [`crate::pim::deployment::DeploymentConfiguration`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TsnConfiguration {
    /// Name of this `TsnConfiguration` (reference target of
    /// `DeploymentConfiguration.tsn_configuration_ref`).
    pub name: String,
    /// `tsn_talker` (Mult 0..*).
    pub tsn_talkers: Vec<TsnTalker>,
    /// `tsn_listener` (Mult 0..*).
    pub tsn_listeners: Vec<TsnListener>,
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

    fn make_traffic() -> TrafficSpecification {
        TrafficSpecification {
            interval_nanoseconds: 1_000_000,
            max_frame_size: 1500,
            max_frames_per_interval: 1,
            transmission_selection: TransmissionSelection::StrictPriority,
        }
    }

    #[test]
    fn matching_stream_ids_match() {
        let s = make_stream(100);
        let talker = TsnTalker {
            name: String::from("t1"),
            stream_name: String::from("ControlStream"),
            stream: s,
            traffic: make_traffic(),
            network_requirements: None,
            data_frame: Some(DataFrameSpecification::Mac {
                source: MacAddress::new([2; 6]),
                destination: MacAddress::new([1; 6]),
            }),
            datawriter_ref: String::from("dw1"),
            time_aware: None,
        };
        let listener = TsnListener {
            name: String::from("l1"),
            stream_name: String::from("ControlStream"),
            stream: s,
            network_requirements: None,
            datareader_ref: String::from("dr1"),
        };
        assert!(listener.matches(&talker));
    }

    #[test]
    fn different_vlan_streams_do_not_match() {
        let talker = TsnTalker {
            name: String::from("t1"),
            stream_name: String::from("S1"),
            stream: make_stream(100),
            traffic: TrafficSpecification {
                interval_nanoseconds: 1_000_000,
                max_frame_size: 64,
                max_frames_per_interval: 1,
                transmission_selection: TransmissionSelection::StrictPriority,
            },
            network_requirements: None,
            data_frame: Some(DataFrameSpecification::IPv4(IPv4Tuple {
                source_ip: [192, 168, 1, 1],
                destination_ip: [192, 168, 1, 2],
                dscp: 46,
                protocol: 17, // UDP.
                source_port: 1234,
                destination_port: 5678,
            })),
            datawriter_ref: String::from("dw1"),
            time_aware: None,
        };
        let listener = TsnListener {
            name: String::from("l1"),
            stream_name: String::from("S1"),
            stream: make_stream(200),
            network_requirements: None,
            datareader_ref: String::from("dr1"),
        };
        assert!(!listener.matches(&talker));
    }

    #[test]
    fn time_aware_talker_can_be_time_critical() {
        // Spec §7.2.3 — TimeAware = Some(...) signals 802.1Qbv.
        let talker = TsnTalker {
            name: String::from("t_time"),
            stream_name: String::from("TimeCritical"),
            stream: make_stream(50),
            traffic: TrafficSpecification {
                interval_nanoseconds: 100_000,
                max_frame_size: 256,
                max_frames_per_interval: 1,
                transmission_selection: TransmissionSelection::StrictPriority,
            },
            network_requirements: None,
            data_frame: Some(DataFrameSpecification::Mac {
                source: MacAddress::new([2; 6]),
                destination: MacAddress::new([1; 6]),
            }),
            datawriter_ref: String::from("dw_time"),
            time_aware: Some(TimeAware {
                earliest_transmit_offset_ns: 1_000,
                latest_transmit_offset_ns: 2_000,
                jitter_ns: 100,
            }),
        };
        assert!(talker.time_aware.is_some());
        assert!(talker.time_aware.expect("ta").is_valid());
    }

    #[test]
    fn talker_carries_optional_network_requirements_per_tab_7_15() {
        let talker = TsnTalker {
            name: String::from("t1"),
            stream_name: String::from("S1"),
            stream: make_stream(10),
            traffic: make_traffic(),
            network_requirements: Some(NetworkRequirements::new(2, 250_000)),
            data_frame: None,
            datawriter_ref: String::from("dw1"),
            time_aware: None,
        };
        let nr = talker.network_requirements.expect("nr");
        assert_eq!(nr.num_seamless_trees, 2);
        assert_eq!(nr.max_latency_ns, 250_000);
        // data_frame_specification is Mult 0..1 — None is valid.
        assert!(talker.data_frame.is_none());
    }

    #[test]
    fn listener_carries_optional_network_requirements_per_tab_7_24() {
        let listener = TsnListener {
            name: String::from("l1"),
            stream_name: String::from("S1"),
            stream: make_stream(10),
            network_requirements: Some(NetworkRequirements::new(0, 1_000_000)),
            datareader_ref: String::from("dr1"),
        };
        let nr = listener.network_requirements.expect("nr");
        assert_eq!(nr.max_latency_ns, 1_000_000);
        assert!(!nr.requires_seamless_redundancy());
    }

    #[test]
    fn tsn_configuration_aggregates_talkers_and_listeners() {
        let cfg = TsnConfiguration {
            name: String::from("PerimeterTSN"),
            tsn_talkers: alloc::vec![TsnTalker {
                name: String::from("t1"),
                stream_name: String::from("S1"),
                stream: make_stream(10),
                traffic: make_traffic(),
                network_requirements: None,
                data_frame: None,
                datawriter_ref: String::from("dw1"),
                time_aware: None,
            }],
            tsn_listeners: alloc::vec![TsnListener {
                name: String::from("l1"),
                stream_name: String::from("S1"),
                stream: make_stream(10),
                network_requirements: None,
                datareader_ref: String::from("dr1"),
            }],
        };
        assert_eq!(cfg.tsn_talkers.len(), 1);
        assert_eq!(cfg.tsn_listeners.len(), 1);
        assert!(cfg.tsn_listeners[0].matches(&cfg.tsn_talkers[0]));
    }
}
