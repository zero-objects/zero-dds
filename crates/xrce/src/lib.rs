// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-XRCE Wire-Codec (OMG `formal/2020-11-01`).
//!
//! Crate `zerodds-xrce`. Safety classification: **SAFE**.
//! Siehe `docs/architecture/02_architecture.md §3` und
//! `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! ## Scope (C6.2.A — Wire-Lite + UDP-Mapping)
//!
//! - **`MessageHeader`** (§8.3.2): SessionId/StreamId/SequenceNr +
//!   optionalem ClientKey.
//! - **`SubmessageHeader`** (§8.3.4): SubmessageId, Flags, Length.
//! - **16 Submessages** (§8.3.5): CREATE_CLIENT, CREATE, GET_INFO,
//!   DELETE, STATUS_AGENT, STATUS, INFO, WRITE_DATA, READ_DATA, DATA,
//!   ACKNACK, HEARTBEAT, RESET, FRAGMENT, TIMESTAMP, TIMESTAMP_REPLY.
//! - **`SerialNumber16`**: RFC-1982-Comparator (§8.3.2.3).
//! - **`XrceUdpSender` / `recv_message`** (§11.2): UDP-Transport-
//!   Mapping mit Default-Port-Schema `7400 + 4 * domainId + offset`.
//!
//! ## Scope (C6.2.B — Object-Model + Reliable-Stack)
//!
//! - **`ObjectId` / `ObjectKind` / `ObjectVariant`** (§7.2 + §7.7).
//! - **`ObjectStore`** (§7.5): pro-Client `BTreeMap<ObjectId, Instance>`
//!   mit CREATE-Reuse/Replace-Semantik.
//! - **`ReliableStreamState`** (§8.4.10/§8.4.11): Sender + Receiver mit
//!   HEARTBEAT-Tick und ACKNACK-Bitmap.
//! - **`FragmentAssembler`** (§8.4.13) mit DoS-Caps + GC.
//! - **`ReadStream`** (§8.4.14): Continuous-Read mit Rate-Limiter.
//! - **`TransportLocator`** (§7.7.4): Small/Medium/Large/String.
//! - **`MulticastDiscovery`** (§11.2.4) auf `239.255.0.2:7400`.
//!
//! ## Scope (C6.2.C — XML/File-Configuration)
//!
//! - **`XrceConfig`** (§7.7.3 + §9.3): Object-Hierarchy-Loader fuer
//!   `<dds><participant>...</participant></dds>` mit Type/QoS-Profile-
//!   Reuse via `zerodds-xml` (DDS-XML-Loader).
//! - **`to_create_messages`**: generiert die CREATE-Submessage-Sequence
//!   in topologischer Reihenfolge (Type → Participant → Topic →
//!   Pub/Sub → DataWriter/DataReader) mit
//!   `RepresentationByXmlString`-Body.
//!
//! ## Scope (C6.2.D — TCP/Serial-Transports + TLS/DTLS)
//!
//! - **`transport_tcp`** (§11.3): `XrceTcpClient`/`XrceTcpServer` mit
//!   2-Byte-Length-Prefix-Framing.
//! - **`transport_serial`** (Annex C): HDLC-aehnlicher Frame-Codec mit
//!   Byte-Stuffing + CRC-16-CCITT-FALSE; Streaming-Reassembly via
//!   `SerialFramer`. Echte Serial-IO ist nicht im Scope (kein
//!   `serialport`-Crate-Dep).
//! - **`transport_tls`**: Trait + Skeleton fuer TLS-on-TCP (Plug-In
//!   spaeter).
//! - **`transport_dtls`**: `DtlsLayer`-Trait + `DummyDtls`-Pass-Through;
//!   echte DTLS-Lib (z.B. `webrtc-dtls`) kommt deployment-spezifisch.
//! - `TransportLocatorTcp` + `TransportLocatorSerial` als zusaetzliche
//!   Locator-Varianten (Discriminator 0x04/0x05).
//!
//! ## Bewusst nicht implementiert
//!
//! - Echte Serialport-IO (kein `serialport`-Crate-Dep).
//! - Echte TLS-/DTLS-Engine (Plug-In spaeter).
//! - ROS2-Bridge.
//! - Live-Reload / Hot-Swap der XML-Config (Stretch).

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
