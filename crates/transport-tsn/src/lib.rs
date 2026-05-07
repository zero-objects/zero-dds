// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS Extensions for Time Sensitive Networking (DDS-TSN) 1.0.
//!
//! Crate `zerodds-transport-tsn`. Safety classification: **STANDARD**.
//! Spec `formal/2024-05-16` (`docs/standards/cache/omg/dds-tsn-1.0-beta2.pdf`).
//!
//! # Scope
//!
//! Wir implementieren das **Configuration-Modell PIM** (Spec §7.2)
//! und das **DDSI-RTPS-Ethernet-PSM** (Spec Annex A) als pure-Rust
//! no_std+alloc Library:
//!
//! * `IEEE802VlanTag` (Spec Tab 7.21) — TPID + PCP + DEI + VID per
//!   IEEE 802.1Q.
//! * `IEEE802MacAddresses` (Spec Tab 7.20) — 6-Byte MAC mit
//!   Multicast/Broadcast-Erkennung.
//! * `TrafficSpecification` (Spec Tab 7.16) — Interval/Max-Frame-Size/
//!   Max-Frames-per-Interval/Transmission-Selection per IEEE 802.1Qcc.
//! * `TimeAware` (Spec Tab 7.17) — earliest/latest_transmit_offset +
//!   jitter per IEEE 802.1Qbv.
//! * `TsnTalker` + `TsnListener` (Spec Tab 7.15 + 7.24) — Stream-
//!   Identifier-Modelle.
//! * `DataFrameSpecification` (Spec Tab 7.19) — Frame-Header-Filter
//!   mit MAC + VLAN + IPv4/v6-Tuple.
//! * `Dscp` (RFC 2474) — Differentiated Services Code Point.
//!
//! # Was nicht abgedeckt ist
//!
//! * **TSN-UNI-Wire-Protocol** — proprietary (Caller-Layer, abhaengig
//!   von Bridge-Vendor wie Cisco IE/Hirschmann/etc.).
//! * **YANG-PSM** (Spec §7.3) — separate `yang`-Crate (Caller-Layer).
//!   XML/JSON-PSM sind in [`config`] abgedeckt (Spec-Cycle 5
//!   Phase B Cluster 5).
//! * **Hardware-Acceleration** (TX-Timestamping per
//!   `SO_TIMESTAMPING`/PHC) — Caller-Layer mit OS-Specific-API.
//! * **gPTP / 802.1AS-Time-Sync-Daemon** — Caller-Layer (typisch
//!   `linuxptp/ptp4l` extern).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod data_frame;
pub mod dscp;
pub mod ethernet_psm;
pub mod mac;
pub mod pim;
pub mod stream;
pub mod time_aware;
pub mod traffic;
pub mod vlan_tag;

#[cfg(feature = "std")]
pub mod config;

pub use data_frame::{DataFrameSpecification, IPv4Tuple, IPv6Tuple};
pub use dscp::Dscp;
pub use ethernet_psm::{ETHERTYPE_RTPS, EthernetFrameHeader};
pub use mac::MacAddress;
pub use stream::{StreamIdentifier, TsnListener, TsnTalker};
pub use time_aware::TimeAware;
pub use traffic::{TrafficSpecification, TransmissionSelection};
pub use vlan_tag::{Ieee802VlanTag, TPID_8021AD, TPID_8021Q};

#[cfg(feature = "std")]
pub use config::{
    ConfigError, DeploymentLibrary, DomainLibrary, QosProfileEntry, TsnConfiguration,
    TsnQosLibrary, parse_xml_config, render_json_config,
};
