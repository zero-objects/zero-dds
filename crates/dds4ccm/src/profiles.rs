// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The QoS policies of the two DDS patterns (Annex E, non-normative), as
//! QoS profiles for the attribute `qos_profile` of a connector.
//!
//! Annex E has four tables, one per role: observable and observer of the
//! pattern State Transfer (§7.3.2), supplier and consumer of the pattern
//! Event Transfer (§7.3.3). Each gives, for the Topic, the DataReader, the
//! DataWriter, the Subscriber and the Publisher of the role, the value of
//! every QoS policy that is relevant, and says whether the designer may
//! change it or the pattern requires it.
//!
//! A connector has one `qos_profile` for all its ports, so the two tables
//! of a pattern are one profile here: the DataWriter and the Publisher of
//! the writing role, the DataReader and the Subscriber of the reading
//! role, and the Topic, which both tables give the same values.
//!
//! | Policy | [`STATE_TRANSFER`] | [`EVENT_TRANSFER`] |
//! |---|---|---|
//! | DURABILITY | `TRANSIENT_LOCAL` (required; or `TRANSIENT`) | `VOLATILE` |
//! | RELIABILITY | `RELIABLE` (required) | `BEST_EFFORT` |
//! | HISTORY | `KEEP_LAST`, depth 1 (required) | `KEEP_ALL` (required) |
//! | DESTINATION_ORDER | `BY_SOURCE_TIMESTAMP` (required) | `BY_SOURCE_TIMESTAMP` (required) |
//! | PRESENTATION | `INSTANCE`, `ordered_access` (required) | `INSTANCE`, `ordered_access` (required) |
//! | WRITER_DATA_LIFECYCLE | not in the table | `autodispose_unregistered_instances` FALSE (required) |
//!
//! Every other policy of the tables has the default of DDS.
//!
//! A connector does not apply these profiles by itself. Without a
//! `qos_profile` its entities have the default QoS of DDS and the HISTORY
//! kind of its pattern (§7.3.2, §7.3.3); the designer who wants the values
//! of Annex E sets `qos_profile` to one of the documents, or to a document
//! derived from it. Two consequences of the values the annex requires:
//!
//! - PRESENTATION and DESTINATION_ORDER are checked between a DataWriter
//!   and a DataReader. A port of a connector with one of these profiles
//!   and a DDS application with the default QoS match in one direction
//!   only: its DataWriters serve a default DataReader, its DataReaders
//!   are not served by a default DataWriter.
//! - A default Publisher or Subscriber of a container or a component
//!   suits such a port only if its profile sets the same PRESENTATION
//!   (§7.4.3); otherwise the port gets a Publisher or Subscriber of its
//!   own.
//!
//! ```
//! use zerodds_dds4ccm::{Profile, profiles};
//!
//! let state = Profile::read(profiles::STATE_TRANSFER).expect("the document reads");
//! assert_eq!(state.name(), Some("(dds)::StateTransfer"));
//! ```

/// The pattern State Transfer (§7.3.2, connector `DDS_State`): the tables
/// "Observable / State Pattern" and "Observer / State Pattern" of Annex E.
pub const STATE_TRANSFER: &str = include_str!("../profiles/state_transfer.xml");

/// The pattern Event Transfer (§7.3.3, connector `DDS_Event`): the tables
/// "Supplier / Event Pattern" and "Consumer / Event Pattern" of Annex E.
pub const EVENT_TRANSFER: &str = include_str!("../profiles/event_transfer.xml");
