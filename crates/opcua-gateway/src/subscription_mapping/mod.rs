// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OPC-UA Subscription Model Mapping — Spec §8.4.
//!
//! Definiert das Konfigurations-Datenmodell aus den Tabellen 8.8-8.15
//! sowie die Mapping-Regeln aus Tab 8.16, plus die Notification-
//! Assignment-Behavior aus §8.4.3 (DataChange + EventField + Constant).
//!
//! # Scope
//!
//! Wir liefern hier **alle spec-normativen Mapping-Regeln** als pure-
//! Rust Datentypen + Funktionen:
//!
//! * **Configuration model** (`config`) — `SubscriptionMapping`,
//!   `OpcUaInput`, `OpcUaConnection`, `SubscriptionProtocol`,
//!   `MonitoredItem` (DataItem/EventItem-Union), `DdsOutput`,
//!   `DdsDomainParticipant`, `InputOutputMapping`, `Assignment`,
//!   `FieldAssignment`, `AssignmentInput`-Union.
//! * **Variant→DDS-Type Mapping** (`variant_dds`) — Tab 8.16 in allen
//!   drei Dimensionen (Scalar / 1D-Array / Multi-Dim-Matrix) fuer alle
//!   25 BuiltinTypeKind-Werte.
//! * **NotificationMessage-Assignment-Behavior** (`behavior`) — §8.4.3
//!   Constant-/DataChange-/EventField-Assignment-Loop, der die
//!   `client_handle`-Korrelation + InputOutputMapping-Lookup + Variant-
//!   Cast-Schritt voll abbildet.
//!
//! # Was nicht hier ist
//!
//! * **OPC-UA-Stack-Integration** — externer UA-Client-Stack. Dieses
//!   Modul liefert die Daten-Stueckliste; das Daemon-Crate verbindet
//!   diese mit `opcua`/`open62541`/etc.
//! * **Wire-Encoding** — CDR fuer DDS-Outputs ist in `crates/cdr/`,
//!   OPC-UA-Wire ist im UA-Stack.
//! * **StatusChangeNotification-Behavior** — Spec §8.4.3.2.3 ist
//!   explizit "out of scope of this specification".

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
