// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PID constants for QoS policies (DDSI-RTPS §9.6.3.2).
//!
//! Each QoS value is encapsulated in a SEDP ParameterList as a
//! `{ pid, length, value }` triple. The PIDs are fixed in
//! DDSI-RTPS 2.5 Table 9.9 "Data representations for built-in endpoints".
//! We duplicate them here (instead of importing zerodds-rtps),
//! to keep `zerodds-qos` independent of `zerodds-rtps`.

/// DDSI-RTPS Table 9.9. 16-bit parameter IDs.
///
/// The values are OMG-public and stable across all vendors.
#[non_exhaustive]
pub struct Pid;

impl Pid {
    // ---------------------------------------------------------------------
    // QoS-Policies (§9.6.3.2).
    // ---------------------------------------------------------------------
    /// `PID_USER_DATA`.
    pub const USER_DATA: u16 = 0x002c;
    /// `PID_TOPIC_DATA`.
    pub const TOPIC_DATA: u16 = 0x002e;
    /// `PID_GROUP_DATA`.
    pub const GROUP_DATA: u16 = 0x002d;
    /// `PID_DURABILITY`.
    pub const DURABILITY: u16 = 0x001d;
    /// `PID_DURABILITY_SERVICE`.
    pub const DURABILITY_SERVICE: u16 = 0x001e;
    /// `PID_DEADLINE`.
    pub const DEADLINE: u16 = 0x0023;
    /// `PID_LATENCY_BUDGET`.
    pub const LATENCY_BUDGET: u16 = 0x0027;
    /// `PID_LIVELINESS`.
    pub const LIVELINESS: u16 = 0x001b;
    /// `PID_RELIABILITY`.
    pub const RELIABILITY: u16 = 0x001a;
    /// `PID_LIFESPAN`.
    pub const LIFESPAN: u16 = 0x002b;
    /// `PID_DESTINATION_ORDER`.
    pub const DESTINATION_ORDER: u16 = 0x0025;
    /// `PID_HISTORY`.
    pub const HISTORY: u16 = 0x0040;
    /// `PID_RESOURCE_LIMITS`.
    pub const RESOURCE_LIMITS: u16 = 0x0041;
    /// `PID_OWNERSHIP`.
    pub const OWNERSHIP: u16 = 0x001f;
    /// `PID_OWNERSHIP_STRENGTH`.
    pub const OWNERSHIP_STRENGTH: u16 = 0x0006;
    /// `PID_PRESENTATION`.
    pub const PRESENTATION: u16 = 0x0021;
    /// `PID_PARTITION`.
    pub const PARTITION: u16 = 0x0029;
    /// `PID_TIME_BASED_FILTER`.
    pub const TIME_BASED_FILTER: u16 = 0x0004;
    /// `PID_TRANSPORT_PRIORITY`.
    pub const TRANSPORT_PRIORITY: u16 = 0x0049;
    /// `PID_READER_DATA_LIFECYCLE` — OMG has not standardized fixed PIDs
    /// for DataLifecycle policies; we mark the values with
    /// the vendor flag (`0x8000`), so that foreign stacks (Cyclone, Fast-DDS)
    /// can ignore unknown PIDs per RTPS §9.6.3.2.1, instead of
    /// inferring a standardized value.
    pub const READER_DATA_LIFECYCLE: u16 = 0x8046;
    /// `PID_WRITER_DATA_LIFECYCLE` — likewise with the vendor flag.
    pub const WRITER_DATA_LIFECYCLE: u16 = 0x8045;

    // ---------------------------------------------------------------------
    // Sentinel.
    // ---------------------------------------------------------------------
    /// `PID_SENTINEL` — marks the end of a ParameterList.
    pub const SENTINEL: u16 = 0x0001;
}
