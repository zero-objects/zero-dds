// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS Extensions for Time Sensitive Networking (DDS-TSN) 1.0.
//!
//! Crate `zerodds-transport-tsn`. Safety classification: **STANDARD**.
//! Spec `formal/2024-05-16` (`docs/standards/cache/omg/dds-tsn-1.0-beta2.pdf`).
//!
//! # Scope
//!
//! We implement the **configuration model PIM** (Spec §7.2)
//! and the **DDSI-RTPS Ethernet PSM** (Spec Annex A) as a pure-Rust
//! no_std+alloc library:
//!
//! * `IEEE802VlanTag` (Spec Tab 7.21) — TPID + PCP + DEI + VID per
//!   IEEE 802.1Q.
//! * `IEEE802MacAddresses` (Spec Tab 7.20) — 6-byte MAC with
//!   multicast/broadcast detection.
//! * `TrafficSpecification` (Spec Tab 7.16) — Interval/Max-Frame-Size/
//!   Max-Frames-per-Interval/Transmission-Selection per IEEE 802.1Qcc.
//! * `TimeAware` (Spec Tab 7.17) — earliest/latest_transmit_offset +
//!   jitter per IEEE 802.1Qbv.
//! * `TsnTalker` + `TsnListener` + `TsnConfiguration` (Spec Tab 7.15 +
//!   7.24 + Figure 7.3) — stream configuration model incl.
//!   `network_requirements` + `datawriter_ref`.
//! * `NetworkRequirements` (Spec Tab 7.18/7.25) — `num_seamless_trees`
//!   + `max_latency` (IEEE 802.1CB FRER redundancy + latency bound).
//! * `DataFrameSpecification` (Spec Tab 7.19) — frame header filter
//!   with MAC + VLAN + IPv4/v6 tuple (incl. `dscp` from Tab 7.22/7.23).
//! * `Dscp` (RFC 2474) — Differentiated Services Code Point.
//! * **YANG PSM** (Spec §7.3.3, [`pim::yang`]) — transformation of the
//!   configuration model into `group-talker`/`group-listener` (UNI
//!   CUC→CNC), incl. `stream-id-type`, `interface-capabilities` and
//!   an RFC-7951 YANG-JSON renderer. The XML/JSON PSM (§7.3.1/§7.3.2) is in
//!   [`pim::xml`]/[`pim::json`].
//! * **Live AF_PACKET transport** (Spec Annex A, feature `live`,
//!   Linux) — `socket::TsnTransport` sends/receives RTPS directly in the
//!   Ethernet frame (EtherType 0x88B5); frame logic in `live_frame`.
//!
//! # What is not covered
//!
//! * **TSN UNI wire protocol** — proprietary (caller layer, dependent
//!   on the bridge vendor such as Cisco IE/Hirschmann/etc.). We provide the
//!   YANG group representation (§7.3.3); its transport over the
//!   concrete UNI protocol is the caller layer.
//! * **Hardware acceleration** (TX timestamping via
//!   `SO_TIMESTAMPING`/PHC) — caller layer with an OS-specific API.
//! * **gPTP / 802.1AS time-sync daemon** — caller layer (typically
//!   `linuxptp/ptp4l` external).
//!
//! # Hardware validation (as of 2026-06)
//!
//! The configuration model, the PSM renderers (YANG/XML/JSON) and the
//! frame/wire logic ([`ethernet_psm`] + `live_frame`) are
//! hardware-independent and fully unit-tested. **Not** verified against
//! real TSN hardware are the *time-critical guarantees*
//! themselves — time-aware shaping (IEEE 802.1Qbv), frame preemption (802.1Qbu),
//! FRER redundancy (802.1CB) and gPTP sync (802.1AS) — as well as the throughput/
//! latency of the `live` AF_PACKET path (`socket`, feature `live`). This
//! is **not a code blocker**, but a pure validation gap: TSN-capable
//! NIC/switch hardware (with a configured
//! Qbv gate schedule + PTP grandmaster) is currently missing. On such hardware the
//! `live` path is the integration point; the TSN guarantees are then provided by the
//! network (bridges) + an external `ptp4l`/`tc-taprio`, which this
//! library configures rather than enforces itself.

// The spec model is unsafe-free; only the optional `live` AF_PACKET
// transport (socket.rs) needs libc unsafe.
#![cfg_attr(not(feature = "live"), forbid(unsafe_code))]
#![warn(missing_docs)]

extern crate alloc;

pub mod data_frame;
pub mod dscp;
pub mod ethernet_psm;
pub mod mac;
pub mod network_requirements;
pub mod pim;
pub mod stream;
pub mod time_aware;
pub mod traffic;
pub mod vlan_tag;

#[cfg(feature = "std")]
pub mod config;

/// Platform-independent frame helpers for the live transport
/// (feature `live`). Testable on every platform.
#[cfg(feature = "live")]
pub mod live_frame;

/// Live AF_PACKET transport (Linux-only, feature `live`). Empty on
/// other platforms.
#[cfg(feature = "live")]
pub mod socket;

pub use data_frame::{DataFrameSpecification, IPv4Tuple, IPv6Tuple};
pub use dscp::Dscp;
pub use ethernet_psm::{ETHERTYPE_RTPS, EthernetFrameHeader};
pub use mac::MacAddress;
pub use network_requirements::NetworkRequirements;
pub use stream::{StreamIdentifier, TsnConfiguration, TsnListener, TsnTalker};
pub use time_aware::TimeAware;
pub use traffic::{TrafficSpecification, TransmissionSelection};
pub use vlan_tag::{Ieee802VlanTag, TPID_8021AD, TPID_8021Q};

#[cfg(all(feature = "live", target_os = "linux"))]
pub use socket::TsnTransport;

// Note: the legacy `config::TsnConfiguration` (old simplified
// top-level config) is deliberately NOT re-exported at the crate root, to
// avoid colliding with the spec `stream::TsnConfiguration` (Figure 7.3 talker/
// listener aggregate). It remains reachable as `config::TsnConfiguration`.
#[cfg(feature = "std")]
pub use config::{
    ConfigError, DeploymentLibrary, DomainLibrary, QosProfileEntry, TsnQosLibrary,
    parse_xml_config, render_json_config,
};
