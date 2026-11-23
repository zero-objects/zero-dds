// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OMG DDS for Lightweight CCM 1.1 — the DDS-DCPS and DDS-DLRL ports.
//!
//! Crate `zerodds-dds4ccm`. Safety classification: **STANDARD**.
//! Spec `formal/2012-02-01` (`internal/standards/cache/omg/dds4ccm-1.1.pdf`).
//!
//! # Scope
//!
//! The connector fragments of the normative DDS-DCPS ports (§7.2.2) over
//! the DCPS entities of `zerodds-dcps`:
//!
//! - [`ccm_dds`]: the non-typed part of the IDL module `CCM_DDS` (Annex A):
//!   `ReadInfo`, `QueryFilter`, the exceptions, the listener controls,
//!   `ContentFilterSetting`, `PortStatusListener`,
//!   `ConnectorStatusListener`.
//! - [`ccm_dds_typed`]: the template module `CCM_DDS::Typed<T, TSeq>`:
//!   `InstanceHandleManager`, `Writer`, `Updater`, `Reader`, `Getter`,
//!   `Listener`, `StateListener`.
//! - [`writer`], [`reader`], [`listener`], [`status`]: the fragments behind
//!   those interfaces (§7.2.2.1).
//! - [`ports`]: the extended ports `DDS_Write`, `DDS_Update`, `DDS_Read`,
//!   `DDS_Get`, `DDS_Listen`, `DDS_StateListen` (§7.2.2.2).
//! - [`connector`]: the connectors `DDS_Base`, `DDS_TopicBase`,
//!   `DDS_State` and `DDS_Event` (§7.3).
//! - [`entities`]: the DCPS entities of the ports, allocated to the port,
//!   the component or the container (§7.4.1), and the QoS profiles of
//!   connector, component and container (§7.4.3).
//! - [`threading`]: the threading policy of port, component and container
//!   (§7.4.4).
//! - [`profiles`]: the QoS policies of the two patterns (Annex E) as QoS
//!   profiles for the attribute `qos_profile` of a connector.
//! - [`dlrl`]: the DDS-DLRL ports and connectors (§8): the interface
//!   `CacheOperation`, the home receptacles, the port types of the
//!   composition rule, the DLRL connector on `DDS_Base`, over the DLRL
//!   entities of `zerodds-dlrl`.
//!
//! A fragment has the DataWriter or the DataReader that is "entirely
//! dedicated to" its port (§7.4.1); the connector that owns the topic, the
//! domain and the QoS profile creates it, in the Publisher or Subscriber
//! and the DomainParticipant of the port, the component or the container.
//!
//! # Example
//!
//! ```
//! use zerodds_dds4ccm::ccm_dds::{ListenerMode, QueryFilter, UNLIMITED};
//!
//! // No query, and a listener that is not called until the component
//! // enables it.
//! assert!(QueryFilter::default().is_empty());
//! assert_eq!(ListenerMode::default(), ListenerMode::NotEnabled);
//! assert_eq!(UNLIMITED, 0);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod ccm_dds;
pub mod ccm_dds_typed;
pub mod connector;
pub mod dlrl;
pub mod entities;
mod instance_status;
pub mod listener;
pub mod ports;
pub mod profiles;
pub mod reader;
mod reader_core;
pub mod receptacle;
pub mod status;
pub mod threading;
pub mod writer;

pub use ccm_dds::{
    AccessStatus, AlreadyCreated, ConnectorStatusListener, ContentFilterSetting,
    DataListenerControl, DataNumber, DdsEntity, InstanceStatus, InternalError, ListenerMode,
    NonChangeable, NonExistent, PortStatusListener, QueryFilter, ReadInfo, ReaderError,
    StateListenerControl, UNLIMITED, UpdaterError,
};
pub use ccm_dds_typed::{
    Getter, InstanceHandleManager, Listener, Reader, StateListener, Updater, Writer,
};
pub use connector::{ConnectorError, DdsBase, DdsEvent, DdsState, DdsTopicBase};
pub use dlrl::{
    CacheOperation, DeclarationError, DlrlConnector, DlrlPort, DlrlPortType, HomeReceptacle,
};
pub use entities::{Component, Container, LevelError, Profile, ProfileError};
pub use listener::{DataListenerControlFragment, StateListenerControlFragment};
pub use ports::{DdsGet, DdsListen, DdsRead, DdsStateListen, DdsUpdate, DdsWrite};
pub use reader::{GetterFragment, ReaderFragment};
pub use receptacle::Receptacle;
pub use status::{ErrorListener, ReaderStatusListener, TopicStatusListener, WriterStatusListener};
pub use threading::{ThreadPool, ThreadingPolicy};
pub use writer::{UpdaterFragment, WriterFragment};
