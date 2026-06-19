// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! YANG PSM — Spec §7.3.3.
//!
//! Maps the DDS-TSN configuration model (§7.2) onto the YANG data-
//! module definitions for talker, listener and status groups specified
//! in IEEE 802.1Q §46.3. This representation is
//! the UNI exchange (User/Network Interface) between a CUC
//! (Central User Configuration) and a CNC (Central Network
//! Configuration): the CUC sends talker/listener groups as a request,
//! the CNC answers with status groups.
//!
//! The normative transformation rules from §7.3.3 are implemented:
//!
//! * `stream-id-type` ([`StreamId`]) — 6-octet node MAC + 16-bit
//!   DataWriter ID.
//! * `group-talker` ([`GroupTalker`]) — `stream-rank` (=1),
//!   `end-station-interfaces`, optional `data-frame-specification`,
//!   `traffic-specification` (interval as numerator/denominator),
//!   optional `user-to-network-requirements`, `interface-capabilities`.
//! * `group-listener` ([`GroupListener`]) — `end-station-interfaces`,
//!   optional `user-to-network-requirements`, `interface-capabilities`.
//!
//! `interface-capabilities` (vlan-tag-capable + the 802.1CB lists) and
//! the interface name describe capabilities of the underlying
//! infrastructure, not of the DDS configuration — they are passed to the
//! transform as [`InterfaceCapabilities`].

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::data_frame::{DataFrameSpecification, IPv4Tuple, IPv6Tuple};
use crate::network_requirements::NetworkRequirements;
use crate::stream::{TsnConfiguration, TsnListener, TsnTalker};
use crate::time_aware::TimeAware;
use crate::traffic::TrafficSpecification;

// -------------------------------------------------------------------
// stream-id-type — §7.3.3 (IEEE 802.1Qcc StreamID).
// -------------------------------------------------------------------

/// Spec §7.3.3 `stream-id-type` — unique stream identifier.
///
/// The first six octets are the MAC address of the `Node` that (via
/// `node_ref`) is associated with the `Deployment` instantiating the
/// `TsnTalker`. The last two octets are a 16-bit number that
/// uniquely identifies the `DataWriter` within the
/// deployment node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamId {
    /// Node MAC (first 6 octets).
    pub mac: [u8; 6],
    /// 16-bit DataWriter identifier (last 2 octets).
    pub unique_id: u16,
}

impl StreamId {
    /// YANG string form per IEEE 802.1Qcc: eight octets, separated by `-`,
    /// uppercase hex (`AA-BB-CC-DD-EE-FF-NN-NN`).
    #[must_use]
    pub fn to_yang_string(self) -> String {
        let [a, b, c, d, e, f] = self.mac;
        let hi = (self.unique_id >> 8) as u8;
        let lo = (self.unique_id & 0xff) as u8;
        format!("{a:02X}-{b:02X}-{c:02X}-{d:02X}-{e:02X}-{f:02X}-{hi:02X}-{lo:02X}")
    }
}

// -------------------------------------------------------------------
// interface-capabilities — §7.3.3 (infrastructure capabilities).
// -------------------------------------------------------------------

/// Spec §7.3.3 `interface-capabilities` + interface name.
///
/// Describes capabilities of the underlying TSN infrastructure that
/// cannot be derived from the DDS configuration (IEEE 802.1Q
/// §46.2.3.7.x). The caller passes them to the transform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceCapabilities {
    /// Name of the network interface (e.g. `"eth0"`) for the
    /// `end-station-interfaces` list.
    pub interface_name: String,
    /// `vlan-tag-capable` (IEEE 802.1Q §46.2.3.7.1).
    pub vlan_tag_capable: bool,
    /// `cb-stream-iden-type-list` (IEEE 802.1CB §46.2.3.7.2). Empty =
    /// no FRER stream-identification support.
    pub cb_stream_iden_type_list: Vec<u32>,
    /// `cb-sequence-type-list` (IEEE 802.1CB §46.2.3.7.3). Empty = no
    /// FRER sequencing support.
    pub cb_sequence_type_list: Vec<u32>,
}

impl Default for InterfaceCapabilities {
    fn default() -> Self {
        Self {
            interface_name: String::new(),
            vlan_tag_capable: true,
            cb_stream_iden_type_list: Vec::new(),
            cb_sequence_type_list: Vec::new(),
        }
    }
}

impl InterfaceCapabilities {
    /// Constructs capabilities with the given interface name and the
    /// defaults (VLAN-capable, no CB lists).
    #[must_use]
    pub fn with_interface(interface_name: impl Into<String>) -> Self {
        Self {
            interface_name: interface_name.into(),
            ..Self::default()
        }
    }
}

// -------------------------------------------------------------------
// Shared group building blocks.
// -------------------------------------------------------------------

/// Spec §7.3.3 `end-station-interfaces` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndStationInterface {
    /// MAC of the `Node` (via `node_ref`).
    pub mac: [u8; 6],
    /// Interface name.
    pub interface_name: String,
}

/// Spec §7.3.3 `user-to-network-requirements` container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserToNetworkRequirements {
    /// `num-seamless-trees`.
    pub num_seamless_trees: u8,
    /// `max-latency` (nanoseconds).
    pub max_latency_ns: u32,
}

impl From<NetworkRequirements> for UserToNetworkRequirements {
    fn from(nr: NetworkRequirements) -> Self {
        Self {
            num_seamless_trees: nr.num_seamless_trees,
            max_latency_ns: nr.max_latency_ns,
        }
    }
}

/// Spec §7.3.3 `interval` container — period as a fraction of a
/// second (`numerator`/`denominator`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    /// `numerator`.
    pub numerator: u32,
    /// `denominator`.
    pub denominator: u32,
}

impl Interval {
    /// Maps a period in nanoseconds to a reduced fraction
    /// `numerator/denominator` of a second (1 s = 1e9 ns).
    #[must_use]
    pub fn from_nanoseconds(period_ns: u64) -> Self {
        // period_ns / 1_000_000_000 seconds, reduced via GCD.
        let denom: u64 = 1_000_000_000;
        let g = gcd(period_ns, denom).max(1);
        let mut num = period_ns / g;
        let mut den = denom / g;
        // Clamp into u32 (periods > ~4.29 s are unrealistic for TSN).
        while num > u64::from(u32::MAX) || den > u64::from(u32::MAX) {
            num /= 2;
            den = den.max(1) / 2;
        }
        Self {
            numerator: num as u32,
            denominator: den.max(1) as u32,
        }
    }
}

const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Spec §7.3.3 `time-aware` container of the `traffic-specification`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GroupTimeAware {
    /// `earliest-transmit-offset` (ns).
    pub earliest_transmit_offset: u32,
    /// `latest-transmit-offset` (ns).
    pub latest_transmit_offset: u32,
    /// `jitter` (ns).
    pub jitter: u32,
}

impl GroupTimeAware {
    /// §7.3.3: if `time_aware` is unspecified in the `TsnTalker`, all
    /// three leaves are set to zero; otherwise from [`TimeAware`].
    #[must_use]
    fn from_optional(ta: Option<TimeAware>) -> Self {
        match ta {
            None => Self::default(),
            Some(t) => Self {
                earliest_transmit_offset: clamp_u32(t.earliest_transmit_offset_ns),
                latest_transmit_offset: clamp_u32(t.latest_transmit_offset_ns),
                jitter: clamp_u32(t.jitter_ns),
            },
        }
    }
}

fn clamp_u32(v: u64) -> u32 {
    if v > u64::from(u32::MAX) {
        u32::MAX
    } else {
        v as u32
    }
}

/// Spec §7.3.3 `traffic-specification` container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupTrafficSpecification {
    /// `interval`.
    pub interval: Interval,
    /// `max-frames-per-interval`.
    pub max_frames_per_interval: u32,
    /// `max-frame-size`.
    pub max_frame_size: u32,
    /// `transmission-selection` (0=SP, 1=CBS, 2=ETS, 3=ATS).
    pub transmission_selection: u8,
    /// `time-aware`.
    pub time_aware: GroupTimeAware,
}

impl GroupTrafficSpecification {
    #[must_use]
    fn from_spec(ts: TrafficSpecification, ta: Option<TimeAware>) -> Self {
        Self {
            interval: Interval::from_nanoseconds(ts.interval_nanoseconds),
            max_frames_per_interval: ts.max_frames_per_interval,
            max_frame_size: ts.max_frame_size,
            transmission_selection: ts.transmission_selection as u8,
            time_aware: GroupTimeAware::from_optional(ta),
        }
    }
}

/// Spec §7.3.3 `group-ieee802-mac-addresses`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupMacAddresses {
    /// `destination-mac-address`.
    pub destination: [u8; 6],
    /// `source-mac-address`.
    pub source: [u8; 6],
}

/// Spec §7.3.3 `group-ieee802-vlan-tag`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupVlanTag {
    /// `priority-code-point`.
    pub priority_code_point: u8,
    /// `vlan-id`.
    pub vlan_id: u16,
}

/// Spec §7.3.3 `group-ipv4-tuple` / `group-ipv6-tuple` (shared
/// fields, addresses as byte slices).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupIpTuple {
    /// `source-ip-address` (4 or 16 bytes).
    pub source_ip: Vec<u8>,
    /// `destination-ip-address`.
    pub destination_ip: Vec<u8>,
    /// `dscp`.
    pub dscp: u8,
    /// `protocol`.
    pub protocol: u16,
    /// `source-port`.
    pub source_port: u16,
    /// `destination-port`.
    pub destination_port: u16,
}

/// Spec §7.3.3 `data-frame-specification` of the `group-talker`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupDataFrameSpecification {
    /// DDSI-RTPS Ethernet PSM: MAC addresses + VLAN tag.
    Ethernet {
        /// `group-ieee802-mac-addresses`.
        mac_addresses: GroupMacAddresses,
        /// `group-ieee802-vlan-tag`.
        vlan_tag: GroupVlanTag,
    },
    /// DDSI-RTPS UDP/IP PSM over IPv4: `group-ipv4-tuple`.
    IPv4(GroupIpTuple),
    /// DDSI-RTPS UDP/IP PSM over IPv6: `group-ipv6-tuple`.
    IPv6(GroupIpTuple),
}

impl GroupIpTuple {
    fn from_ipv4(t: IPv4Tuple) -> Self {
        Self {
            source_ip: t.source_ip.to_vec(),
            destination_ip: t.destination_ip.to_vec(),
            dscp: t.dscp,
            protocol: u16::from(t.protocol),
            source_port: t.source_port,
            destination_port: t.destination_port,
        }
    }

    fn from_ipv6(t: IPv6Tuple) -> Self {
        Self {
            source_ip: t.source_ip.to_vec(),
            destination_ip: t.destination_ip.to_vec(),
            dscp: t.dscp,
            protocol: u16::from(t.protocol),
            source_port: t.source_port,
            destination_port: t.destination_port,
        }
    }
}

// -------------------------------------------------------------------
// group-talker — §7.3.3.
// -------------------------------------------------------------------

/// Spec §7.3.3 `group-talker` grouping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupTalker {
    /// Stream identifier (`stream-id-type`).
    pub stream_id: StreamId,
    /// `stream-rank.rank` — always `1` per §7.3.3.
    pub stream_rank: u32,
    /// `end-station-interfaces`.
    pub end_station_interfaces: Vec<EndStationInterface>,
    /// `data-frame-specification` (optional).
    pub data_frame_specification: Option<GroupDataFrameSpecification>,
    /// `traffic-specification`.
    pub traffic_specification: GroupTrafficSpecification,
    /// `user-to-network-requirements` (optional).
    pub user_to_network_requirements: Option<UserToNetworkRequirements>,
    /// `interface-capabilities`.
    pub interface_capabilities: InterfaceCapabilities,
}

impl GroupTalker {
    /// §7.3.3 transform: maps a [`TsnTalker`] to an
    /// equivalent talker group.
    ///
    /// * `node_mac` — MAC of the deployment `Node` (via `node_ref`); forms
    ///   the first six octets of the `stream-id` and the
    ///   `end-station-interfaces` MAC.
    /// * `datawriter_id` — 16-bit identifier of the associated `DataWriter`
    ///   within the node; the last two octets of the `stream-id`.
    /// * `caps` — infrastructure capabilities + interface name.
    ///
    /// The VLAN information of the Ethernet `data-frame-specification` is
    /// taken from the talker's 802.1CB `StreamIdentifier`
    /// (`talker.stream.vlan_tag`).
    #[must_use]
    pub fn from_talker(
        talker: &TsnTalker,
        node_mac: [u8; 6],
        datawriter_id: u16,
        caps: InterfaceCapabilities,
    ) -> Self {
        let data_frame_specification = talker.data_frame.map(|df| match df {
            DataFrameSpecification::Mac {
                source,
                destination,
            } => GroupDataFrameSpecification::Ethernet {
                mac_addresses: GroupMacAddresses {
                    destination: destination.0,
                    source: source.0,
                },
                vlan_tag: GroupVlanTag {
                    priority_code_point: talker.stream.vlan_tag.pcp,
                    vlan_id: talker.stream.vlan_tag.vid,
                },
            },
            DataFrameSpecification::IPv4(t) => {
                GroupDataFrameSpecification::IPv4(GroupIpTuple::from_ipv4(t))
            }
            DataFrameSpecification::IPv6(t) => {
                GroupDataFrameSpecification::IPv6(GroupIpTuple::from_ipv6(t))
            }
        });

        Self {
            stream_id: StreamId {
                mac: node_mac,
                unique_id: datawriter_id,
            },
            stream_rank: 1,
            end_station_interfaces: alloc::vec![EndStationInterface {
                mac: node_mac,
                interface_name: caps.interface_name.clone(),
            }],
            data_frame_specification,
            traffic_specification: GroupTrafficSpecification::from_spec(
                talker.traffic,
                talker.time_aware,
            ),
            user_to_network_requirements: talker
                .network_requirements
                .map(UserToNetworkRequirements::from),
            interface_capabilities: caps,
        }
    }
}

// -------------------------------------------------------------------
// group-listener — §7.3.3.
// -------------------------------------------------------------------

/// Spec §7.3.3 `group-listener` grouping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupListener {
    /// `end-station-interfaces`.
    pub end_station_interfaces: Vec<EndStationInterface>,
    /// `user-to-network-requirements` (optional).
    pub user_to_network_requirements: Option<UserToNetworkRequirements>,
    /// `interface-capabilities`.
    pub interface_capabilities: InterfaceCapabilities,
}

impl GroupListener {
    /// §7.3.3 transform: maps a [`TsnListener`] to an
    /// equivalent listener group.
    #[must_use]
    pub fn from_listener(
        listener: &TsnListener,
        node_mac: [u8; 6],
        caps: InterfaceCapabilities,
    ) -> Self {
        Self {
            end_station_interfaces: alloc::vec![EndStationInterface {
                mac: node_mac,
                interface_name: caps.interface_name.clone(),
            }],
            user_to_network_requirements: listener
                .network_requirements
                .map(UserToNetworkRequirements::from),
            interface_capabilities: caps,
        }
    }
}

// -------------------------------------------------------------------
// TsnConfiguration → Groups (§7.3.3.1, whole config).
// -------------------------------------------------------------------

/// Result of the §7.3.3.1 transformation of a [`TsnConfiguration`]: the
/// talker and listener groups that a CUC sends as a request to the
/// CNC.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TalkerListenerGroups {
    /// One `group-talker` per `TsnTalker`.
    pub talkers: Vec<GroupTalker>,
    /// One `group-listener` per `TsnListener`.
    pub listeners: Vec<GroupListener>,
}

/// §7.3.3.1 transform of a whole [`TsnConfiguration`] into talker and
/// listener groups.
///
/// All end stations of the deployment node share the same
/// `node_mac` and `caps`. The DataWriter IDs are assigned sequentially
/// in talker order (0, 1, 2, …) — unique within
/// the deployment node, as §7.3.3 requires.
#[must_use]
pub fn talker_listener_groups(
    cfg: &TsnConfiguration,
    node_mac: [u8; 6],
    caps: &InterfaceCapabilities,
) -> TalkerListenerGroups {
    let talkers = cfg
        .tsn_talkers
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let id = u16::try_from(i).unwrap_or(u16::MAX);
            GroupTalker::from_talker(t, node_mac, id, caps.clone())
        })
        .collect();
    let listeners = cfg
        .tsn_listeners
        .iter()
        .map(|l| GroupListener::from_listener(l, node_mac, caps.clone()))
        .collect();
    TalkerListenerGroups { talkers, listeners }
}

// -------------------------------------------------------------------
// YANG-JSON renderer (RFC 7951) — gated on std (like pim::json).
// -------------------------------------------------------------------

#[cfg(feature = "std")]
mod render {
    use super::{
        EndStationInterface, GroupDataFrameSpecification, GroupIpTuple, GroupListener, GroupTalker,
        InterfaceCapabilities, TalkerListenerGroups,
    };
    use alloc::format;
    use alloc::string::String;

    fn esc(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for ch in s.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                c => out.push(c),
            }
        }
        out
    }

    fn ip_string(bytes: &[u8]) -> String {
        if bytes.len() == 4 {
            format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
        } else {
            // IPv6: 8 groups, full form.
            let mut parts = alloc::vec::Vec::new();
            let mut i = 0;
            while i + 1 < bytes.len() {
                parts.push(format!(
                    "{:x}",
                    (u16::from(bytes[i]) << 8) | u16::from(bytes[i + 1])
                ));
                i += 2;
            }
            parts.join(":")
        }
    }

    fn mac_string(mac: [u8; 6]) -> String {
        let [a, b, c, d, e, f] = mac;
        format!("{a:02X}-{b:02X}-{c:02X}-{d:02X}-{e:02X}-{f:02X}")
    }

    fn render_end_stations(out: &mut String, ifs: &[EndStationInterface]) {
        out.push_str("\"end-station-interfaces\":[");
        for (i, esi) in ifs.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"mac-address\":\"{}\",\"interface-name\":\"{}\"}}",
                mac_string(esi.mac),
                esc(&esi.interface_name)
            ));
        }
        out.push(']');
    }

    fn render_caps(out: &mut String, caps: &InterfaceCapabilities) {
        out.push_str("\"interface-capabilities\":{");
        out.push_str(&format!(
            "\"vlan-tag-capable\":{}",
            if caps.vlan_tag_capable {
                "true"
            } else {
                "false"
            }
        ));
        out.push_str(",\"cb-stream-iden-type-list\":[");
        for (i, v) in caps.cb_stream_iden_type_list.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!("{v}"));
        }
        out.push_str("],\"cb-sequence-type-list\":[");
        for (i, v) in caps.cb_sequence_type_list.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!("{v}"));
        }
        out.push_str("]}");
    }

    fn render_ip_tuple(out: &mut String, key: &str, t: &GroupIpTuple) {
        out.push_str(&format!(
            "\"{key}\":{{\"source-ip-address\":\"{}\",\"destination-ip-address\":\"{}\",\
             \"dscp\":{},\"protocol\":{},\"source-port\":{},\"destination-port\":{}}}",
            ip_string(&t.source_ip),
            ip_string(&t.destination_ip),
            t.dscp,
            t.protocol,
            t.source_port,
            t.destination_port
        ));
    }

    fn render_data_frame(out: &mut String, df: &GroupDataFrameSpecification) {
        out.push_str("\"data-frame-specification\":{");
        match df {
            GroupDataFrameSpecification::Ethernet {
                mac_addresses,
                vlan_tag,
            } => {
                out.push_str(&format!(
                    "\"group-ieee802-mac-addresses\":{{\"destination-mac-address\":\"{}\",\
                     \"source-mac-address\":\"{}\"}},\"group-ieee802-vlan-tag\":\
                     {{\"priority-code-point\":{},\"vlan-id\":{}}}",
                    mac_string(mac_addresses.destination),
                    mac_string(mac_addresses.source),
                    vlan_tag.priority_code_point,
                    vlan_tag.vlan_id
                ));
            }
            GroupDataFrameSpecification::IPv4(t) => render_ip_tuple(out, "group-ipv4-tuple", t),
            GroupDataFrameSpecification::IPv6(t) => render_ip_tuple(out, "group-ipv6-tuple", t),
        }
        out.push('}');
    }

    fn render_talker(out: &mut String, t: &GroupTalker) {
        out.push('{');
        out.push_str(&format!(
            "\"stream-id\":\"{}\",",
            t.stream_id.to_yang_string()
        ));
        out.push_str(&format!("\"stream-rank\":{{\"rank\":{}}},", t.stream_rank));
        render_end_stations(out, &t.end_station_interfaces);
        out.push(',');
        if let Some(df) = &t.data_frame_specification {
            render_data_frame(out, df);
            out.push(',');
        }
        let ts = &t.traffic_specification;
        out.push_str(&format!(
            "\"traffic-specification\":{{\"interval\":{{\"numerator\":{},\"denominator\":{}}},\
             \"max-frames-per-interval\":{},\"max-frame-size\":{},\"transmission-selection\":{},\
             \"time-aware\":{{\"earliest-transmit-offset\":{},\"latest-transmit-offset\":{},\
             \"jitter\":{}}}}}",
            ts.interval.numerator,
            ts.interval.denominator,
            ts.max_frames_per_interval,
            ts.max_frame_size,
            ts.transmission_selection,
            ts.time_aware.earliest_transmit_offset,
            ts.time_aware.latest_transmit_offset,
            ts.time_aware.jitter
        ));
        if let Some(unr) = t.user_to_network_requirements {
            out.push_str(&format!(
                ",\"user-to-network-requirements\":{{\"num-seamless-trees\":{},\"max-latency\":{}}}",
                unr.num_seamless_trees, unr.max_latency_ns
            ));
        }
        out.push(',');
        render_caps(out, &t.interface_capabilities);
        out.push('}');
    }

    fn render_listener(out: &mut String, l: &GroupListener) {
        out.push('{');
        render_end_stations(out, &l.end_station_interfaces);
        out.push(',');
        if let Some(unr) = l.user_to_network_requirements {
            out.push_str(&format!(
                "\"user-to-network-requirements\":{{\"num-seamless-trees\":{},\"max-latency\":{}}},",
                unr.num_seamless_trees, unr.max_latency_ns
            ));
        }
        render_caps(out, &l.interface_capabilities);
        out.push('}');
    }

    impl GroupTalker {
        /// Renders this talker group as YANG-JSON (RFC 7951).
        #[must_use]
        pub fn to_yang_json(&self) -> String {
            let mut out = String::new();
            render_talker(&mut out, self);
            out
        }
    }

    impl GroupListener {
        /// Renders this listener group as YANG-JSON (RFC 7951).
        #[must_use]
        pub fn to_yang_json(&self) -> String {
            let mut out = String::new();
            render_listener(&mut out, self);
            out
        }
    }

    impl TalkerListenerGroups {
        /// Renders the complete talker/listener groups as a YANG-JSON
        /// request object with `group-talker` and `group-listener` arrays.
        #[must_use]
        pub fn to_yang_json(&self) -> String {
            let mut out = String::from("{\"group-talker\":[");
            for (i, t) in self.talkers.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                render_talker(&mut out, t);
            }
            out.push_str("],\"group-listener\":[");
            for (i, l) in self.listeners.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                render_listener(&mut out, l);
            }
            out.push_str("]}");
            out
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::data_frame::DataFrameSpecification;
    use crate::mac::MacAddress;
    use crate::stream::StreamIdentifier;
    use crate::traffic::{TrafficSpecification, TransmissionSelection};
    use crate::vlan_tag::{Ieee802VlanTag, TPID_8021Q};

    fn eth_talker() -> TsnTalker {
        TsnTalker {
            name: "t1".into(),
            stream_name: "ControlStream".into(),
            stream: StreamIdentifier {
                destination_mac: MacAddress::new([0x01, 0x80, 0xC2, 0, 0, 0x0E]),
                vlan_tag: Ieee802VlanTag::new(TPID_8021Q, 5, false, 42).expect("vlan"),
            },
            traffic: TrafficSpecification {
                interval_nanoseconds: 1_000_000, // 1 ms.
                max_frame_size: 256,
                max_frames_per_interval: 2,
                transmission_selection: TransmissionSelection::CreditBasedShaper,
            },
            network_requirements: Some(NetworkRequirements::new(2, 500_000)),
            data_frame: Some(DataFrameSpecification::Mac {
                source: MacAddress::new([0xAA; 6]),
                destination: MacAddress::new([0xBB; 6]),
            }),
            datawriter_ref: "dw_ctrl".into(),
            time_aware: Some(TimeAware {
                earliest_transmit_offset_ns: 1_000,
                latest_transmit_offset_ns: 3_000,
                jitter_ns: 200,
            }),
        }
    }

    #[test]
    fn stream_id_yang_string_is_mac_plus_16bit_id() {
        let sid = StreamId {
            mac: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            unique_id: 0x0007,
        };
        assert_eq!(sid.to_yang_string(), "00-11-22-33-44-55-00-07");
    }

    #[test]
    fn stream_id_uses_node_mac_and_datawriter_id() {
        // §7.3.3: first 6 octets = node MAC, last 2 = DataWriter ID.
        let g = GroupTalker::from_talker(
            &eth_talker(),
            [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01],
            0x00AB,
            InterfaceCapabilities::with_interface("eth0"),
        );
        assert_eq!(g.stream_id.mac, [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]);
        assert_eq!(g.stream_id.unique_id, 0x00AB);
        assert_eq!(g.stream_id.to_yang_string(), "DE-AD-BE-EF-00-01-00-AB");
    }

    #[test]
    fn talker_stream_rank_is_one_per_spec() {
        let g =
            GroupTalker::from_talker(&eth_talker(), [0; 6], 0, InterfaceCapabilities::default());
        assert_eq!(g.stream_rank, 1);
    }

    #[test]
    fn ethernet_data_frame_maps_mac_and_vlan_from_stream() {
        let g = GroupTalker::from_talker(
            &eth_talker(),
            [0; 6],
            0,
            InterfaceCapabilities::with_interface("eth0"),
        );
        match g.data_frame_specification.expect("df") {
            GroupDataFrameSpecification::Ethernet {
                mac_addresses,
                vlan_tag,
            } => {
                assert_eq!(mac_addresses.source, [0xAA; 6]);
                assert_eq!(mac_addresses.destination, [0xBB; 6]);
                // VLAN comes from talker.stream.vlan_tag (pcp=5, vid=42).
                assert_eq!(vlan_tag.priority_code_point, 5);
                assert_eq!(vlan_tag.vlan_id, 42);
            }
            _ => panic!("expected ethernet"),
        }
    }

    #[test]
    fn ipv4_data_frame_maps_tuple_including_dscp() {
        let mut t = eth_talker();
        t.data_frame = Some(DataFrameSpecification::IPv4(IPv4Tuple {
            source_ip: [10, 0, 0, 1],
            destination_ip: [10, 0, 0, 2],
            dscp: 46,
            protocol: 17,
            source_port: 7400,
            destination_port: 7401,
        }));
        let g = GroupTalker::from_talker(&t, [0; 6], 0, InterfaceCapabilities::default());
        match g.data_frame_specification.expect("df") {
            GroupDataFrameSpecification::IPv4(tuple) => {
                assert_eq!(tuple.source_ip, alloc::vec![10, 0, 0, 1]);
                assert_eq!(tuple.dscp, 46);
                assert_eq!(tuple.protocol, 17);
                assert_eq!(tuple.destination_port, 7401);
            }
            _ => panic!("expected ipv4"),
        }
    }

    #[test]
    fn absent_data_frame_yields_none() {
        let mut t = eth_talker();
        t.data_frame = None;
        let g = GroupTalker::from_talker(&t, [0; 6], 0, InterfaceCapabilities::default());
        assert!(g.data_frame_specification.is_none());
    }

    #[test]
    fn interval_is_period_as_reduced_fraction_of_second() {
        // 1 ms = 1/1000 s.
        assert_eq!(
            Interval::from_nanoseconds(1_000_000),
            Interval {
                numerator: 1,
                denominator: 1000
            }
        );
        // 125 us = 1/8000 s.
        assert_eq!(
            Interval::from_nanoseconds(125_000),
            Interval {
                numerator: 1,
                denominator: 8000
            }
        );
        // 1 s = 1/1.
        assert_eq!(
            Interval::from_nanoseconds(1_000_000_000),
            Interval {
                numerator: 1,
                denominator: 1
            }
        );
    }

    #[test]
    fn traffic_spec_maps_all_fields() {
        let g =
            GroupTalker::from_talker(&eth_talker(), [0; 6], 0, InterfaceCapabilities::default());
        let ts = g.traffic_specification;
        assert_eq!(ts.max_frames_per_interval, 2);
        assert_eq!(ts.max_frame_size, 256);
        assert_eq!(ts.transmission_selection, 1); // CBS.
        assert_eq!(ts.interval.denominator, 1000);
    }

    #[test]
    fn time_aware_unspecified_sets_zero_offsets() {
        // §7.3.3: if time_aware is None, all three leaves are 0.
        let mut t = eth_talker();
        t.time_aware = None;
        let g = GroupTalker::from_talker(&t, [0; 6], 0, InterfaceCapabilities::default());
        let ta = g.traffic_specification.time_aware;
        assert_eq!(ta.earliest_transmit_offset, 0);
        assert_eq!(ta.latest_transmit_offset, 0);
        assert_eq!(ta.jitter, 0);
    }

    #[test]
    fn time_aware_present_maps_offsets() {
        let g =
            GroupTalker::from_talker(&eth_talker(), [0; 6], 0, InterfaceCapabilities::default());
        let ta = g.traffic_specification.time_aware;
        assert_eq!(ta.earliest_transmit_offset, 1_000);
        assert_eq!(ta.latest_transmit_offset, 3_000);
        assert_eq!(ta.jitter, 200);
    }

    #[test]
    fn user_to_network_requirements_present_when_talker_has_them() {
        let g =
            GroupTalker::from_talker(&eth_talker(), [0; 6], 0, InterfaceCapabilities::default());
        let unr = g.user_to_network_requirements.expect("unr");
        assert_eq!(unr.num_seamless_trees, 2);
        assert_eq!(unr.max_latency_ns, 500_000);
    }

    #[test]
    fn user_to_network_requirements_absent_when_talker_has_none() {
        let mut t = eth_talker();
        t.network_requirements = None;
        let g = GroupTalker::from_talker(&t, [0; 6], 0, InterfaceCapabilities::default());
        assert!(g.user_to_network_requirements.is_none());
    }

    #[test]
    fn end_station_interface_carries_node_mac_and_ifname() {
        let g = GroupTalker::from_talker(
            &eth_talker(),
            [1, 2, 3, 4, 5, 6],
            0,
            InterfaceCapabilities::with_interface("eth1"),
        );
        assert_eq!(g.end_station_interfaces.len(), 1);
        assert_eq!(g.end_station_interfaces[0].mac, [1, 2, 3, 4, 5, 6]);
        assert_eq!(g.end_station_interfaces[0].interface_name, "eth1");
    }

    #[test]
    fn listener_maps_node_mac_and_requirements() {
        let l = TsnListener {
            name: "l1".into(),
            stream_name: "ControlStream".into(),
            stream: StreamIdentifier {
                destination_mac: MacAddress::new([0x01, 0x80, 0xC2, 0, 0, 0x0E]),
                vlan_tag: Ieee802VlanTag::new(TPID_8021Q, 5, false, 42).expect("vlan"),
            },
            network_requirements: Some(NetworkRequirements::new(0, 1_000_000)),
            datareader_ref: "dr1".into(),
        };
        let g = GroupListener::from_listener(
            &l,
            [9, 9, 9, 9, 9, 9],
            InterfaceCapabilities::with_interface("eth0"),
        );
        assert_eq!(g.end_station_interfaces[0].mac, [9; 6]);
        assert_eq!(
            g.user_to_network_requirements.expect("unr").max_latency_ns,
            1_000_000
        );
    }

    #[test]
    fn config_transform_assigns_sequential_datawriter_ids() {
        let cfg = TsnConfiguration {
            name: "cfg".into(),
            tsn_talkers: alloc::vec![eth_talker(), eth_talker()],
            tsn_listeners: alloc::vec![],
        };
        let groups = talker_listener_groups(
            &cfg,
            [0xAB; 6],
            &InterfaceCapabilities::with_interface("eth0"),
        );
        assert_eq!(groups.talkers.len(), 2);
        assert_eq!(groups.talkers[0].stream_id.unique_id, 0);
        assert_eq!(groups.talkers[1].stream_id.unique_id, 1);
    }

    #[cfg(feature = "std")]
    #[test]
    fn yang_json_contains_normative_keys() {
        let g = GroupTalker::from_talker(
            &eth_talker(),
            [0xDE, 0xAD, 0xBE, 0xEF, 0, 1],
            7,
            InterfaceCapabilities::with_interface("eth0"),
        );
        let json = g.to_yang_json();
        assert!(json.contains("\"stream-id\":\"DE-AD-BE-EF-00-01-00-07\""));
        assert!(json.contains("\"stream-rank\":{\"rank\":1}"));
        assert!(
            json.contains("\"group-ieee802-vlan-tag\":{\"priority-code-point\":5,\"vlan-id\":42}")
        );
        assert!(json.contains("\"transmission-selection\":1"));
        assert!(json.contains("\"num-seamless-trees\":2,\"max-latency\":500000"));
        assert!(json.contains("\"vlan-tag-capable\":true"));
        // Well-formed: balanced braces.
        assert_eq!(
            json.matches('{').count(),
            json.matches('}').count(),
            "unbalanced braces in {json}"
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn yang_json_groups_has_both_arrays() {
        let cfg = TsnConfiguration {
            name: "cfg".into(),
            tsn_talkers: alloc::vec![eth_talker()],
            tsn_listeners: alloc::vec![TsnListener {
                name: "l1".into(),
                stream_name: "ControlStream".into(),
                stream: eth_talker().stream,
                network_requirements: None,
                datareader_ref: "dr1".into(),
            }],
        };
        let json = talker_listener_groups(
            &cfg,
            [0xAB; 6],
            &InterfaceCapabilities::with_interface("eth0"),
        )
        .to_yang_json();
        assert!(json.starts_with("{\"group-talker\":["));
        assert!(json.contains("],\"group-listener\":["));
        assert_eq!(json.matches('{').count(), json.matches('}').count());
    }
}
