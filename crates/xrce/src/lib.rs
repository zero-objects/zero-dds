// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-XRCE Wire-Codec (OMG `formal/2020-11-01`).
//!
//! Crate `zerodds-xrce`. Safety classification: **SAFE**.
//! See `docs/architecture/02_architecture.md §3` and
//! `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! ## Scope (C6.2.A — wire-lite + UDP mapping)
//!
//! - **`MessageHeader`** (§8.3.2): SessionId/StreamId/SequenceNr +
//!   optional ClientKey.
//! - **`SubmessageHeader`** (§8.3.4): SubmessageId, flags, length.
//! - **16 submessages** (§8.3.5): CREATE_CLIENT, CREATE, GET_INFO,
//!   DELETE, STATUS_AGENT, STATUS, INFO, WRITE_DATA, READ_DATA, DATA,
//!   ACKNACK, HEARTBEAT, RESET, FRAGMENT, TIMESTAMP, TIMESTAMP_REPLY.
//! - **`SerialNumber16`**: RFC-1982 comparator (§8.3.2.3).
//! - **`XrceUdpSender` / `recv_message`** (§11.2): UDP transport
//!   mapping with the default port scheme `7400 + 4 * domainId + offset`.
//!
//! ## Scope (C6.2.B — object model + reliable stack)
//!
//! - **`ObjectId` / `ObjectKind` / `ObjectVariant`** (§7.2 + §7.7).
//! - **`ObjectStore`** (§7.5): per-client `BTreeMap<ObjectId, Instance>`
//!   with CREATE reuse/replace semantics.
//! - **`ReliableStreamState`** (§8.4.10/§8.4.11): sender + receiver with
//!   HEARTBEAT tick and ACKNACK bitmap.
//! - **`FragmentAssembler`** (§8.4.13) with DoS caps + GC.
//! - **`ReadStream`** (§8.4.14): continuous read with a rate limiter.
//! - **`TransportLocator`** (§7.7.4): Small/Medium/Large/String.
//! - **`MulticastDiscovery`** (§11.2.4) on `239.255.0.2:7400`.
//!
//! ## Scope (C6.2.C — XML/file configuration)
//!
//! - **`XrceConfig`** (§7.7.3 + §9.3): object hierarchy loader for
//!   `<dds><participant>...</participant></dds>` with type/QoS-profile
//!   reuse via `zerodds-xml` (DDS-XML loader).
//! - **`to_create_messages`**: generates the CREATE submessage sequence
//!   in topological order (type → participant → topic →
//!   pub/sub → DataWriter/DataReader) with a
//!   `RepresentationByXmlString` body.
//!
//! ## Scope (C6.2.D — TCP/serial transports + TLS/DTLS)
//!
//! - **`transport_tcp`** (§11.3): `XrceTcpClient`/`XrceTcpServer` with
//!   2-byte length-prefix framing.
//! - **`transport_serial`** (Annex C): HDLC-like frame codec with
//!   byte stuffing + CRC-16-CCITT-FALSE; streaming reassembly via
//!   `SerialFramer`. Real serial IO is out of scope (no
//!   `serialport` crate dep).
//! - **`transport_tls`**: trait + skeleton for TLS-on-TCP (plug-in
//!   later).
//! - **`transport_dtls`**: `DtlsLayer` trait + `DummyDtls` pass-through;
//!   a real DTLS lib (e.g. `webrtc-dtls`) comes deployment-specific.
//! - `TransportLocatorTcp` + `TransportLocatorSerial` as additional
//!   locator variants (discriminator 0x04/0x05).
//!
//! ## Deliberately not implemented
//!
//! - Real serial port IO (no `serialport` crate dep).
//! - A real TLS/DTLS engine (plug-in later).
//! - ROS2 bridge.
//! - Live reload / hot swap of the XML config (stretch).

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod encoding;
pub mod error;
pub mod header;
pub mod serial_number;
pub mod submessages;
#[cfg(feature = "alloc")]
pub mod transport_dtls;
#[cfg(feature = "std")]
pub mod transport_serial;
#[cfg(feature = "std")]
pub mod transport_tcp;
#[cfg(feature = "alloc")]
pub mod transport_tls;
#[cfg(feature = "std")]
pub mod transport_udp;

#[cfg(feature = "alloc")]
pub mod continuous_read;
#[cfg(feature = "std")]
pub mod discovery;
#[cfg(feature = "alloc")]
pub mod fragment;
#[cfg(feature = "alloc")]
pub mod object_id;
#[cfg(feature = "alloc")]
pub mod object_info;
#[cfg(feature = "alloc")]
pub mod object_kind;
#[cfg(feature = "alloc")]
pub mod object_repr;
#[cfg(feature = "alloc")]
pub mod object_store;
#[cfg(feature = "alloc")]
pub mod reliable;
#[cfg(feature = "alloc")]
pub mod transport_locator;
#[cfg(feature = "xml-config")]
pub mod xml_config;

pub use error::XrceError;
pub use header::{
    CLIENT_KEY_LEN, ClientKey, MessageHeader, SESSION_ID_NONE_WITH_CLIENT_KEY,
    SESSION_ID_NONE_WITHOUT_CLIENT_KEY, SessionId, StreamId,
};
pub use serial_number::SerialNumber16;
pub use submessages::{
    DOSC_MAX_PAYLOAD_SIZE, DOSC_MAX_SUBMESSAGES, FLAG_E_LITTLE_ENDIAN, Message, Submessage,
    SubmessageHeader, SubmessageId,
};
#[cfg(feature = "std")]
pub use transport_udp::{
    XrceUdpSender, agent_default_port, client_default_port, recv_message, send_message,
};

#[cfg(feature = "alloc")]
pub use continuous_read::{DeliveryControl, PendingSample, ReadStream};
#[cfg(feature = "std")]
pub use discovery::{MulticastDiscovery, XRCE_DISCOVERY_GROUP, XRCE_DISCOVERY_PORT};
#[cfg(feature = "alloc")]
pub use fragment::{
    AssemblerKey, FragmentAssembler, MAX_FRAGMENTS_PER_STREAM, MAX_PENDING_STREAMS,
    MAX_TOTAL_PAYLOAD,
};
#[cfg(feature = "alloc")]
pub use object_id::{KIND_MASK_BIT, OBJECTID_AGENT, OBJECTID_CLIENT, OBJECTID_INVALID, ObjectId};
#[cfg(feature = "alloc")]
pub use object_kind::{
    OBJK_AGENT, OBJK_APPLICATION, OBJK_CLIENT, OBJK_DATAREADER, OBJK_DATAWRITER, OBJK_DOMAIN,
    OBJK_INVALID, OBJK_PARTICIPANT, OBJK_PUBLISHER, OBJK_QOSPROFILE, OBJK_SUBSCRIBER, OBJK_TOPIC,
    OBJK_TYPE, ObjectKind,
};
#[cfg(feature = "alloc")]
pub use object_repr::{ObjectVariant, REPR_MAX_INLINE_BYTES};
#[cfg(feature = "alloc")]
pub use object_store::{CreateOutcome, CreationMode, ObjectInstance, ObjectStore};
#[cfg(feature = "alloc")]
pub use reliable::{
    DEFAULT_HEARTBEAT_PERIOD, RECEIVER_BUFFER_CAP, RELIABLE_MAX_PAYLOAD, ReliableConfig,
    ReliableStreamState, SENDER_WINDOW_CAP,
};
#[cfg(feature = "alloc")]
pub use transport_dtls::{DtlsError, DtlsLayer, DummyDtls};
#[cfg(feature = "alloc")]
pub use transport_locator::{
    TRANSPORT_LOCATOR_SERIAL_DEVICE_MAX, TRANSPORT_LOCATOR_STRING_MAX, TransportLocator,
    TransportLocatorLarge, TransportLocatorMedium, TransportLocatorSerial, TransportLocatorSmall,
    TransportLocatorTcp,
};
#[cfg(feature = "std")]
pub use transport_serial::{
    ESCAPE_BYTE, FLAG_BYTE, STUFF_XOR, SerialError, SerialFramer, crc16_ccitt_false,
    decode_frame as serial_decode_frame, decode_frame_inner as serial_decode_frame_inner,
    encode_message as serial_encode_message, encode_payload as serial_encode_payload,
};
#[cfg(feature = "std")]
pub use transport_tcp::{TCP_LENGTH_PREFIX_SIZE, XrceTcpClient, XrceTcpServer};
#[cfg(feature = "alloc")]
pub use transport_tls::{XrceTlsClient, XrceTlsServer, XrceTlsStream};
#[cfg(all(feature = "std", feature = "xml-config"))]
pub use xml_config::load_xrce_config_from_file;
#[cfg(feature = "xml-config")]
pub use xml_config::{
    CreateMessage, DataReaderConfig, DataWriterConfig, InMemoryQosResolver, MAX_HIERARCHY_DEPTH,
    MAX_TYPES_PER_FILE, ParticipantConfig, PublisherConfig, QosProfileResolver, SubscriberConfig,
    TopicConfig, TypeConfig, XrceConfig, XrceXmlError, load_xrce_config,
};
