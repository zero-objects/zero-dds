// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PID-Constants fuer QoS-Policies (DDSI-RTPS §9.6.3.2).
//!
//! Jeder QoS-Wert wird in einer SEDP-ParameterList als ein
//! `{ pid, length, value }`-Tripel gekapselt. Die PIDs sind in
//! DDSI-RTPS 2.5 Table 9.9 "Data representations for built-in endpoints"
//! festgelegt. Wir duplizieren sie hier (statt zerodds-rtps zu importieren),
//! um `zerodds-qos` unabhaengig von `zerodds-rtps` zu halten.

/// DDSI-RTPS Table 9.9. 16-bit Parameter-IDs.
///
/// Die Werte sind OMG-publik und stabil ueber alle Vendors.
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
    /// `PID_READER_DATA_LIFECYCLE` — OMG hat fuer DataLifecycle-Policies
    /// keine festen PIDs standardisiert; wir kennzeichnen die Werte mit
    /// dem Vendor-Flag (`0x8000`), damit fremde Stacks (Cyclone, Fast-DDS)
    /// unbekannte PIDs gem. RTPS §9.6.3.2.1 ignorieren koennen, statt
    /// auf einen standardisierten Wert zu schliessen.
    pub const READER_DATA_LIFECYCLE: u16 = 0x8046;
    /// `PID_WRITER_DATA_LIFECYCLE` — analog Vendor-Flag.
    pub const WRITER_DATA_LIFECYCLE: u16 = 0x8045;

    // ---------------------------------------------------------------------
    // Sentinel.
    // ---------------------------------------------------------------------
    /// `PID_SENTINEL` — markiert Ende einer ParameterList.
    pub const SENTINEL: u16 = 0x0001;
}
