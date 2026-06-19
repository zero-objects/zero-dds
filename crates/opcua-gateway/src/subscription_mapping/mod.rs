// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OPC-UA Subscription Model Mapping — Spec §8.4.
//!
//! Defines the configuration data model from tables 8.8-8.15
//! as well as the mapping rules from Tab 8.16, plus the notification
//! assignment behavior from §8.4.3 (DataChange + EventField + Constant).
//!
//! # Scope
//!
//! Here we provide **all spec-normative mapping rules** as pure-
//! Rust data types + functions:
//!
//! * **Configuration model** (`config`) — `SubscriptionMapping`,
//!   `OpcUaInput`, `OpcUaConnection`, `SubscriptionProtocol`,
//!   `MonitoredItem` (DataItem/EventItem union), `DdsOutput`,
//!   `DdsDomainParticipant`, `InputOutputMapping`, `Assignment`,
//!   `FieldAssignment`, `AssignmentInput` union.
//! * **Variant→DDS type mapping** (`variant_dds`) — Tab 8.16 in all
//!   three dimensions (scalar / 1D-array / multi-dim matrix) for all
//!   25 BuiltinTypeKind values.
//! * **NotificationMessage assignment behavior** (`behavior`) — §8.4.3
//!   constant-/DataChange-/EventField-assignment loop, which fully
//!   maps the `client_handle` correlation + InputOutputMapping lookup +
//!   variant cast step.
//!
//! # What is not here
//!
//! * **OPC-UA stack integration** — an external UA client stack. This
//!   module provides the data bill of materials; the daemon crate connects
//!   it to `opcua`/`open62541`/etc.
//! * **Wire encoding** — CDR for DDS outputs is in `crates/cdr/`,
//!   OPC-UA wire is in the UA stack.
//! * **StatusChangeNotification behavior** — Spec §8.4.3.2.3 is
//!   explicitly "out of scope of this specification".

pub mod behavior;
pub mod config;
pub mod variant_dds;

pub use behavior::{
    AssignmentError, NotificationContext, apply_constant_assignment,
    apply_data_change_notification, apply_event_notification,
};
pub use config::{
    Assignment, AssignmentInput, AssignmentKind, DataItem, DataItemRef, DdsDomainParticipant,
    DdsOutput, DdsRegisterType, EventFieldRef, EventItem, FieldAssignment, InputOutputMapping,
    MonitoredItem, MonitoredItemKind, OpcUaConnection, OpcUaConnectionConfig, OpcUaInput,
    SubscriptionMapping, SubscriptionProtocol,
};
pub use variant_dds::{ArrayShape, DdsTypeRef, map_variant_to_dds};
