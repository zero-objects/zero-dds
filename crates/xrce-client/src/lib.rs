// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Client — synchronous interface without callbacks (spec §7.2).
//!
//! Crate `zerodds-xrce-client`.
//!
//! # Spec mapping
//!
//! OMG DDS-XRCE 1.0 §7.2: "XRCE Client: simplified interface, no
//! callbacks, text parameters; the session bridges sleep/wakeup
//! cycles."
//!
//! [`XrceClient`] is a synchronous state machine that builds and sends
//! real wire submessages (not a state tracker the caller has to encode
//! for itself):
//!
//! | Method              | Spec reference          |
//! |---------------------|-------------------------|
//! | [`XrceClient::new`]/[`XrceClient::connect`] | §7.8.2/§8.4.5 (CREATE_CLIENT) |
//! | [`XrceClient::create_object`] | §7.8.3 (CREATE) |
//! | [`XrceClient::delete_object`] | §7.8.3 (DELETE) |
//! | [`XrceClient::request_write`] | §7.8.4 (WRITE_DATA) |
//! | [`XrceClient::request_read`]  | §7.8.5 (READ_DATA) |
//! | [`XrceClient::disconnect`]    | §8.4.5 (DELETE(OBJECTID_CLIENT)) |
//! | [`XrceClient::poll`]          | Decodes whatever the agent sent back — `STATUS_AGENT`/`STATUS`/`DATA`, `HEARTBEAT` (answered with `ACKNACK`), `ACKNACK` (answered with a real retransmission), `FRAGMENT` (reassembled). |
//! | [`XrceClient::tick`]          | §8.4.14.1 — sends this client's own `HEARTBEAT` for any reliable stream with unacknowledged in-flight messages; without it, a lost datagram on a `reliable: true` write/create/etc. is never recovered (§8.4.14.2: the agent only ACKNACKs in response to a `HEARTBEAT`, it never polls on its own). |
//!
//! The transport is abstracted via the [`ClientTransport`] trait — concrete
//! impls (UDP/TCP/DTLS/Serial) live in `crates/xrce/src/transport_*`.
//!
//! ## Reliability (§8.4.14)
//!
//! Every stream this client uses gets its own
//! `zerodds_xrce::reliable::ReliableStreamState` — the exact type
//! `zerodds-xrce-agent`'s `SessionEngine` uses, so both ends of the
//! connection run the same protocol state machine. A request built with
//! `reliable: true` is `submit()`-ted for a real sequence number and
//! buffered for retransmission; an inbound reliable-stream message is
//! gated through `recv_data`/`drain_in_order` before it is decoded, and a
//! `WRITE_DATA`/`DATA` body too large for [`XrceClient::requested_mtu`]
//! (or the peer's own, once known) is split into `FRAGMENT` submessages on
//! send and reassembled on receive.
//!
//! Safety classification: **SAFE**.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod discovery;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_xrce::encoding::Endianness;
use zerodds_xrce::encoding::RepresentationTimestamps;
use zerodds_xrce::fragment::{AssemblerKey, DEFAULT_GC_TTL, FragmentAssembler};
use zerodds_xrce::handshake::{AgentRepresentation, ClientRepresentation, DEFAULT_MTU};
use zerodds_xrce::header::{ClientKey, MessageHeader, SessionId, StreamId};
use zerodds_xrce::object_id::{OBJECTID_CLIENT, ObjectId};
use zerodds_xrce::object_info::{
    BaseObjectReply, BaseObjectRequest, InfoMask, ObjectInfo, ResultStatusCode,
};
use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};
use zerodds_xrce::reliable::{ReliableConfig, ReliableStreamState};
use zerodds_xrce::sample_info::SampleInfo;
use zerodds_xrce::serial_number::SerialNumber16;
use zerodds_xrce::submessages::data::AnyData;
use zerodds_xrce::submessages::read_data::{
    DataDeliveryControl, DeliveryControlForm, ReadSpecification,
};
use zerodds_xrce::submessages::timestamp::TimePoint;
use zerodds_xrce::submessages::write_data::{DataFormat, WriteDataPayloadSample};
use zerodds_xrce::submessages::{
    AckNackPayload, CreateClientPayload, CreatePayload, DeletePayload, FragmentPayload,
    GetInfoPayload, HeartbeatPayload, InfoPayload, Message, ReadDataPayload, StatusAgentPayload,
    StatusPayload, Submessage, SubmessageHeader, SubmessageId, TimestampPayload,
    TimestampReplyPayload, WriteDataPayload,
};

/// Transport abstraction for the XRCE client.
///
/// Implementations (UDP/TCP/DTLS/Serial) live in
/// `crates/xrce/src/transport_*.rs`.
pub trait ClientTransport {
    /// Sends one full `Message` (header + submessages) to the agent.
    ///
    /// # Errors
    /// Transport-specific.
    fn send(&mut self, payload: &[u8]) -> Result<(), ClientError>;

    /// Polls incoming data from the agent. Does not block — if no
    /// data is available, returns `Ok(None)`. One call returns at most
    /// one datagram; [`XrceClient::poll`] calls this in a loop.
    ///
    /// # Errors
    /// Transport-specific.
    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError>;
}

/// XRCE client lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// Initial: the client knows its ClientKey but is not yet
    /// connected.
    Disconnected,
    /// Connect handshake in progress (CREATE_CLIENT sent, STATUS_AGENT
    /// not yet received).
    Connecting,
    /// Connected — operations are possible.
    Connected,
}

/// Client-specific error classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientError {
    /// Operation in the wrong lifecycle status (e.g. send-without-connect).
    InvalidState,
    /// Transport-layer error (connection reset etc.).
    Transport,
    /// The submessage could not be enqueued (wire cap reached, or the
    /// reliable sender window/`RELIABLE_MAX_PAYLOAD` was exceeded).
    QueueFull,
    /// Bytes came back from the agent that do not decode as a valid
    /// `Message`/submessage — never silently dropped, surfaced here so
    /// the caller can log or count it.
    Protocol,
}

/// One thing [`XrceClient::poll`] observed from the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientEvent {
    /// `STATUS_AGENT` with a successful [`ResultStatusCode`] — the
    /// handshake completed and [`XrceClient::state`] is now
    /// [`ClientState::Connected`].
    Connected {
        /// The agent's own `AGENT_Representation`.
        agent: AgentRepresentation,
    },
    /// `STATUS_AGENT` with a failing [`ResultStatusCode`] — the handshake
    /// was rejected; the client stays [`ClientState::Connecting`] so the
    /// caller can inspect `code` before deciding to retry or give up.
    ConnectRejected {
        /// Why the agent refused.
        code: ResultStatusCode,
    },
    /// `STATUS` — the reply to a `CREATE`/`DELETE`/`WRITE_DATA`/
    /// `READ_DATA` this client sent.
    Status {
        /// The `request_id` this status answers.
        request_id: u16,
        /// The object the operation targeted.
        object_id: ObjectId,
        /// The result.
        code: ResultStatusCode,
    },
    /// `DATA` in FORMAT_DATA — a sample delivered for a `READ_DATA` this
    /// client issued (§7.7.1's opaque `serialized_data`; the caller decodes
    /// it with its own generated type support, exactly as a real XRCE agent
    /// forwards it).
    Data {
        /// The `request_id` of the `READ_DATA` this answers.
        request_id: u16,
        /// The DataReader this sample was read from.
        reader: ObjectId,
        /// The XCDR2-encoded sample body.
        payload: Vec<u8>,
    },
    /// `DATA` in FORMAT_SAMPLE: a sample with its `SampleInfo` (§7.7.1).
    Sample {
        /// The `request_id` of the `READ_DATA` this answers.
        request_id: u16,
        /// The DataReader this sample was read from.
        reader: ObjectId,
        /// State, sequence number and time of the sample.
        info: SampleInfo,
        /// The XCDR2-encoded sample body.
        payload: Vec<u8>,
    },
    /// `DATA` in FORMAT_DATA_SEQ, FORMAT_SAMPLE_SEQ or FORMAT_PACKED_SAMPLES.
    /// The samples carry no length each (Annex B.5.2, B.5.4, B.5.5): only the
    /// type of the topic tells where one ends, so the client hands over the
    /// elements as they came. FORMAT_SAMPLE_SEQ puts a `SampleInfo` in front
    /// of each sample, FORMAT_PACKED_SAMPLES a `SampleInfoDelta` against
    /// `info_base`.
    SampleSequence {
        /// The `request_id` of the `READ_DATA` this answers.
        request_id: u16,
        /// The DataReader the samples were read from.
        reader: ObjectId,
        /// The format of the DATA.
        format: DataFormat,
        /// The number of samples in `elements`.
        sample_count: u32,
        /// The common `SampleInfo` of FORMAT_PACKED_SAMPLES; `None` otherwise.
        info_base: Option<SampleInfo>,
        /// The samples, back to back, as they are on the wire.
        elements: Vec<u8>,
    },
    /// A `TIMESTAMP` of the agent (§8.3.5.15). Answer it with
    /// [`XrceClient::send_timestamp_reply`], giving the times of this client's
    /// clock.
    TimestampReceived {
        /// The agent's time when it sent the message.
        transmit_timestamp: TimePoint,
    },
    /// The `TIMESTAMP_REPLY` to a `TIMESTAMP` this client sent
    /// ([`XrceClient::send_timestamp`], §8.3.5.16). With the time this client
    /// received it, the four give the offset of the agent's clock and the
    /// round trip.
    TimestampReply {
        /// The agent's time when it sent the reply (t2).
        transmit_timestamp: TimePoint,
        /// The agent's time when it received the `TIMESTAMP` (t1).
        receive_timestamp: TimePoint,
        /// The `transmit_timestamp` of the `TIMESTAMP` this answers (t0).
        originate_timestamp: TimePoint,
    },
    /// A `FRAGMENT` run of `stream_id` that could not be used: its
    /// reassembly failed, or the concatenated payload is not a run of
    /// submessages (DDS-XRCE 1.0 §8.3.5.14). What it carried is lost, and
    /// this says so.
    FragmentDiscarded {
        /// The stream the fragments came on.
        stream_id: u8,
    },
    /// `INFO` — the answer to a `GET_INFO` (§8.3.5.7): the `ResultStatus` and
    /// the `ObjectInfo`. A failed GET_INFO has the error in `code` and an
    /// empty `info`.
    Info {
        /// The `request_id` this answers.
        request_id: u16,
        /// The object the request named.
        object_id: ObjectId,
        /// The result.
        code: ResultStatusCode,
        /// The activity and configuration of the object.
        info: ObjectInfo,
    },
    /// An `INFO` whose `ObjectInfo` could not be read.
    InfoUnreadable {
        /// The `request_id` this answers.
        request_id: u16,
        /// The object the request named.
        object_id: ObjectId,
    },
    /// A message of a best-effort stream that is a duplicate of the last one
    /// or older was discarded (DDS-XRCE 1.0 §8.3.2.3).
    BestEffortMessageDiscarded {
        /// The stream.
        stream_id: u8,
        /// The `sequenceNr` of the discarded message.
        sequence_nr: u16,
    },
}

/// Bookkeeping for an in-flight inbound `FRAGMENT` reassembly on one
/// stream — same design as `zerodds-xrce-agent`'s `session.rs::FragmentRun`
/// (see its doc for why `next_offset` is trustworthy under reliable,
/// gap-free delivery).
struct FragmentRun {
    base_seq: u16,
    next_offset: u32,
    last_arrival: Duration,
}

/// Encodes a `[Submessage]` as `[header(4) + body]*`, no outer
/// `MessageHeader`, no inter-submessage padding — the same
/// `ReliableStreamState`-buffering / `FRAGMENT`-splitting bridge
/// `zerodds-xrce-agent`'s `session.rs` uses.
fn encode_submessages(submessages: &[Submessage]) -> Vec<u8> {
    let mut out = Vec::new();
    for sm in submessages {
        out.extend_from_slice(&sm.header.to_bytes());
        out.extend_from_slice(&sm.body);
    }
    out
}

/// Inverse of [`encode_submessages`].
fn decode_submessages(bytes: &[u8]) -> Vec<Submessage> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset + SubmessageHeader::WIRE_SIZE <= bytes.len() {
        let Ok(h) = SubmessageHeader::from_bytes(&bytes[offset..]) else {
            break;
        };
        offset += SubmessageHeader::WIRE_SIZE;
        let len = usize::from(h.submessage_length);
        if offset + len > bytes.len() {
            break;
        }
        out.push(Submessage {
            header: h,
            body: bytes[offset..offset + len].to_vec(),
        });
        offset += len;
    }
    out
}

/// Synchronous XRCE client.
pub struct XrceClient<T: ClientTransport> {
    client_key: ClientKey,
    state: ClientState,
    transport: T,
    next_request_id: u16,
    session_id: u8,
    vendor_id: [u8; 2],
    requested_mtu: u16,
    /// One per stream this client has sent or received on — sender half
    /// (`submit`/`get_in_flight`/`pending_heartbeat`) and receiver half
    /// (`recv_data`/`drain_in_order`/`pending_acknack`) both live in the
    /// same instance, exactly as `zerodds-xrce-agent`'s `session.rs`
    /// module doc describes.
    streams: BTreeMap<u8, ReliableStreamState>,
    fragments: FragmentAssembler,
    fragment_runs: BTreeMap<u8, FragmentRun>,
    /// The `sequenceNr` of the newest message taken on each best-effort
    /// stream (§8.3.2.3).
    best_effort_in: BTreeMap<u8, SerialNumber16>,
    /// How a `READ_DATA` writes its `DataDeliveryControl`.
    delivery_control_form: DeliveryControlForm,
    /// Whether this client's `CLIENT_Representation` and the
    /// `AGENT_Representation` it reads carry a timestamp member.
    representation_timestamps: RepresentationTimestamps,
    /// When `connect` sent the CREATE_CLIENT: the session timestamp the
    /// `session_time_offset` of a written sample counts from.
    #[cfg(feature = "std")]
    session_started: Option<std::time::Instant>,
}

impl<T: ClientTransport> XrceClient<T> {
    /// Constructs a client. `session_id` is this client's own choice
    /// (§8.3.2.1: `<=127` puts a `ClientKey` in every `MessageHeader`,
    /// `>=128` authenticates by source address instead — most transports
    /// use the latter). The MTU defaults to
    /// [`zerodds_xrce::handshake::DEFAULT_MTU`] until
    /// [`Self::set_requested_mtu`] changes it.
    pub fn new(client_key: ClientKey, session_id: u8, transport: T) -> Self {
        Self {
            client_key,
            state: ClientState::Disconnected,
            transport,
            next_request_id: 1,
            session_id,
            vendor_id: [0, 0],
            requested_mtu: DEFAULT_MTU,
            streams: BTreeMap::new(),
            fragments: FragmentAssembler::new(),
            fragment_runs: BTreeMap::new(),
            best_effort_in: BTreeMap::new(),
            delivery_control_form: DeliveryControlForm::Appendable,
            representation_timestamps: zerodds_xrce::encoding::representation_timestamps(),
            #[cfg(feature = "std")]
            session_started: None,
        }
    }

    /// Sets the form of the representations of the handshake for this client:
    /// [`RepresentationTimestamps::Absent`] (the IDL of Annex A and the bytes
    /// of Annex B.1, the default) or [`RepresentationTimestamps::Present`]
    /// (the prose of DDS-XRCE 1.0 §7.8.2.1: a `Time_t` after
    /// `xrce_vendor_id`). The agent has to be set the same way
    /// (`representation_timestamps:`); the bytes do not tell the forms apart.
    pub fn set_representation_timestamps(&mut self, form: RepresentationTimestamps) {
        self.representation_timestamps = form;
    }

    /// Sets how `READ_DATA` writes its `DataDeliveryControl`: with the DHEADER
    /// of an APPENDABLE struct as DDS-XRCE 1.0 §7.7.14 says (the default), or
    /// without it ([`DeliveryControlForm::Bare`]), which is what the eProsima
    /// Micro-XRCE-DDS Agent reads.
    pub fn set_delivery_control_form(&mut self, form: DeliveryControlForm) {
        self.delivery_control_form = form;
    }

    /// Sets the `xrce_vendor_id` this client announces in `CREATE_CLIENT`
    /// (default `[0, 0]`, "none registered").
    pub fn set_vendor_id(&mut self, vendor_id: [u8; 2]) {
        self.vendor_id = vendor_id;
    }

    /// Sets the MTU this client announces in `CREATE_CLIENT` and uses as
    /// its own outbound `WRITE_DATA`/`FRAGMENT` split threshold.
    pub fn set_requested_mtu(&mut self, mtu: u16) {
        self.requested_mtu = mtu;
    }

    /// The MTU this client currently announces/splits against (see
    /// [`Self::set_requested_mtu`]).
    #[must_use]
    pub fn requested_mtu(&self) -> u16 {
        self.requested_mtu
    }

    /// Current lifecycle status.
    #[must_use]
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// Returns the ClientKey.
    #[must_use]
    pub fn client_key(&self) -> ClientKey {
        self.client_key
    }

    /// Direct transport access (e.g. for read-pull polling or test
    /// inspection).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Sends `CREATE_CLIENT` (§8.3.5.1/§7.8.2.1) and moves to
    /// [`ClientState::Connecting`]. Travels on `STREAMID_NONE`
    /// unconditionally (there is no session yet to run reliability on).
    ///
    /// # Errors
    /// [`ClientError::InvalidState`] if already connected/connecting;
    /// transport errors otherwise.
    pub fn connect(&mut self, version: [u8; 2]) -> Result<(), ClientError> {
        if self.state != ClientState::Disconnected {
            return Err(ClientError::InvalidState);
        }
        // The `client_timestamp`, when the form has one, is the session
        // timestamp of §7.7.2: the time of this call on the clock of the
        // process (zero where there is none).
        #[cfg(feature = "std")]
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|d| {
                Some(zerodds_xrce::submessages::timestamp::TimePoint {
                    seconds: i32::try_from(d.as_secs()).ok()?,
                    nanoseconds: d.subsec_nanos(),
                })
            });
        #[cfg(not(feature = "std"))]
        let timestamp = None;
        let rep = ClientRepresentation {
            timestamp,
            version,
            vendor_id: self.vendor_id,
            client_key: self.client_key,
            session_id: self.session_id,
            properties: None,
            requested_mtu: Some(self.requested_mtu),
        };
        let sm = CreateClientPayload {
            representation: rep,
        }
        .into_submessage_form(
            zerodds_xrce::encoding::Endianness::Little,
            self.representation_timestamps,
        )
        .map_err(|_| ClientError::QueueFull)?;
        // §8.3.2.1: the CREATE_CLIENT is sent where there is no session yet,
        // so its header carries SESSION_ID_NONE_*: with a ClientKey for a
        // session_id of 0 to 127, without for 128 to 255. The session the
        // payload asks for starts with the agent's STATUS_AGENT.
        let header = MessageHeader {
            session_id: SessionId(self.session_id).none_like(),
            stream_id: StreamId::NONE,
            sequence_nr: SerialNumber16::new(0),
            client_key: self.header_client_key(),
        };
        self.send_message(header, alloc::vec![sm])?;
        self.best_effort_in.clear();
        #[cfg(feature = "std")]
        {
            self.session_started = Some(std::time::Instant::now());
        }
        self.state = ClientState::Connecting;
        Ok(())
    }

    /// Submits a CREATE operation (§7.8.3/§8.3.5.2). Returns a
    /// `request_id`.
    ///
    /// # Errors
    /// [`ClientError::InvalidState`] if not connected.
    /// `extra` is the trailing field `object_id`'s own kind requires in
    /// the real wire `ObjectVariant` (§7.7.3) — `domain_id` for
    /// `OBJK_PARTICIPANT`, the parent container's `ObjectId` for
    /// `OBJK_TOPIC`/`OBJK_PUBLISHER`/`OBJK_SUBSCRIBER`/`OBJK_DATAWRITER`/
    /// `OBJK_DATAREADER`, or [`ObjectVariantExtra::default()`] for every
    /// other kind — see [`ObjectVariant::encode_for_kind`]'s doc.
    ///
    /// # Errors
    /// [`ClientError::InvalidState`] if not connected;
    /// [`ClientError::Protocol`] if `object_id`'s kind is not a real
    /// `OBJK_*` value or `extra` does not match what that kind requires.
    pub fn create_object(
        &mut self,
        object_id: ObjectId,
        representation: ObjectVariant,
        extra: ObjectVariantExtra,
        reuse: bool,
        replace: bool,
        reliable: bool,
    ) -> Result<u16, ClientError> {
        self.require_connected()?;
        let kind = object_id.kind().map_err(|_| ClientError::Protocol)?;
        let request_id = self.allocate_request_id();
        let base = BaseObjectRequest {
            request_id: request_id_octets(request_id),
            object_id,
        };
        let mut body = base.encode().to_vec();
        let encoded = representation
            .encode_for_kind(kind, extra, Endianness::Little)
            .map_err(|_| ClientError::Protocol)?;
        body.extend_from_slice(&encoded);
        let sm = CreatePayload {
            representation: body,
            reuse,
            replace,
        }
        .into_submessage()
        .map_err(|_| ClientError::QueueFull)?;
        self.send_maybe_fragmented(sm, reliable)?;
        Ok(request_id)
    }

    /// Updates an object (§7.8.3.2, Table 14, §8.4.13: "An XRCE Client uses
    /// the CREATE message with the attribute reuse set to TRUE and the
    /// attribute replace set to TRUE to indicate it wants to update the
    /// Object identified by the ObjectId"). The agent answers `STATUS_OK` for
    /// an object it replaced and `STATUS_OK_MATCHES` for one that already had
    /// the representation. Returns the `request_id`.
    ///
    /// # Errors
    /// As [`Self::create_object`].
    pub fn update_object(
        &mut self,
        object_id: ObjectId,
        representation: ObjectVariant,
        extra: ObjectVariantExtra,
        reliable: bool,
    ) -> Result<u16, ClientError> {
        self.create_object(object_id, representation, extra, true, true, reliable)
    }

    /// Sends a `TIMESTAMP` (§8.3.5.15): `transmit_timestamp` is this client's
    /// time "at the time the message is sent", on `STREAMID_NONE` as the
    /// clause says. The agent answers with a `TIMESTAMP_REPLY`
    /// ([`ClientEvent::TimestampReply`]).
    ///
    /// # Errors
    /// [`ClientError::InvalidState`] if not connected; transport errors.
    pub fn send_timestamp(&mut self, transmit_timestamp: TimePoint) -> Result<(), ClientError> {
        self.require_connected()?;
        let sm = TimestampPayload { transmit_timestamp }
            .into_submessage()
            .map_err(|_| ClientError::QueueFull)?;
        self.send_control(sm)
    }

    /// Answers a `TIMESTAMP` of the agent with a `TIMESTAMP_REPLY` (§8.3.5.16):
    /// `originate_timestamp` is the `transmit_timestamp` of that `TIMESTAMP`,
    /// `receive_timestamp` this client's time when it came, `transmit_timestamp`
    /// its time now.
    ///
    /// # Errors
    /// [`ClientError::InvalidState`] if not connected; transport errors.
    pub fn send_timestamp_reply(
        &mut self,
        originate_timestamp: TimePoint,
        receive_timestamp: TimePoint,
        transmit_timestamp: TimePoint,
    ) -> Result<(), ClientError> {
        self.require_connected()?;
        let sm = TimestampReplyPayload {
            transmit_timestamp,
            receive_timestamp,
            originate_timestamp,
        }
        .into_submessage()
        .map_err(|_| ClientError::QueueFull)?;
        self.send_control(sm)
    }

    /// Submits a DELETE operation (§7.8.3/§8.3.5.3).
    ///
    /// # Errors
    /// [`ClientError::InvalidState`].
    pub fn delete_object(
        &mut self,
        object_id: ObjectId,
        reliable: bool,
    ) -> Result<u16, ClientError> {
        self.require_connected()?;
        let request_id = self.allocate_request_id();
        let base = BaseObjectRequest {
            request_id: request_id_octets(request_id),
            object_id,
        };
        let sm = DeletePayload {
            representation: base.encode().to_vec(),
        }
        .into_submessage()
        .map_err(|_| ClientError::QueueFull)?;
        self.send_maybe_fragmented(sm, reliable)?;
        Ok(request_id)
    }

    /// Submits a GET_INFO operation (§7.8.3.3, §8.3.5.3) for `object_id` with
    /// the information `info_mask` selects. The answer is a
    /// [`ClientEvent::Info`].
    ///
    /// # Errors
    /// [`ClientError::InvalidState`] if not connected.
    pub fn request_get_info(
        &mut self,
        object_id: ObjectId,
        info_mask: InfoMask,
        reliable: bool,
    ) -> Result<u16, ClientError> {
        self.require_connected()?;
        let request_id = self.allocate_request_id();
        let base = BaseObjectRequest {
            request_id: request_id_octets(request_id),
            object_id,
        };
        let sm = GetInfoPayload::new(base, info_mask, Endianness::Little)
            .into_submessage()
            .map_err(|_| ClientError::QueueFull)?;
        self.send_maybe_fragmented(sm, reliable)?;
        Ok(request_id)
    }

    /// Asks the agent about itself (§7.8.2.2 get_info, `GET_INFO` of
    /// `OBJECTID_AGENT`), which "may be used prior to establishing a Session":
    /// allowed in any state, and sent in a header without a session
    /// (`SESSION_ID_NONE_*`, §8.3.2.1) on `STREAMID_NONE`. The answer is a
    /// [`ClientEvent::Info`] with the agent's `AGENT_ActivityInfo` (the
    /// transport addresses) and `AGENT_Representation` (version, vendor), from
    /// which a client picks an agent (§8.4.4).
    ///
    /// # Errors
    /// Transport errors.
    pub fn request_agent_info(&mut self, info_mask: InfoMask) -> Result<u16, ClientError> {
        let request_id = self.allocate_request_id();
        let base = BaseObjectRequest {
            request_id: request_id_octets(request_id),
            object_id: zerodds_xrce::object_id::OBJECTID_AGENT,
        };
        let sm = GetInfoPayload::new(base, info_mask, Endianness::Little)
            .into_submessage()
            .map_err(|_| ClientError::QueueFull)?;
        let header = MessageHeader {
            session_id: SessionId(self.session_id).none_like(),
            stream_id: StreamId::NONE,
            sequence_nr: SerialNumber16::new(0),
            client_key: self.header_client_key(),
        };
        self.send_message(header, alloc::vec![sm])?;
        Ok(request_id)
    }

    /// Submits a WRITE_DATA operation (§7.8.4/§8.3.5.9.1, `FORMAT_DATA`).
    /// A body too large for [`Self::requested_mtu`] is split into
    /// `FRAGMENT` submessages — only possible when `reliable` is `true`
    /// (spec §8.3.5.14: FRAGMENT is reliable-stream-only); an oversized
    /// best-effort write is sent whole, same as
    /// `zerodds-xrce-agentd`'s own fallback for the symmetric case.
    ///
    /// # Errors
    /// [`ClientError::InvalidState`].
    pub fn request_write(
        &mut self,
        writer: ObjectId,
        payload: &[u8],
        reliable: bool,
    ) -> Result<u16, ClientError> {
        self.require_connected()?;
        let request_id = self.allocate_request_id();
        let base = BaseObjectRequest {
            request_id: request_id_octets(request_id),
            object_id: writer,
        };
        let sm = WriteDataPayload {
            base,
            serialized_data: payload.to_vec(),
        }
        .into_submessage()
        .map_err(|_| ClientError::QueueFull)?;
        self.send_maybe_fragmented(sm, reliable)?;
        Ok(request_id)
    }

    /// Submits a WRITE_DATA operation carrying a lifecycle marker
    /// (§7.8.4/§8.3.5.9.1, `FORMAT_SAMPLE`) -- `dispose`/`unregister`
    /// select `SampleInfoFlags::INSTANCE_STATE_DISPOSED`/
    /// `_UNREGISTERED` the same way `zerodds-xrce-agent`'s
    /// `handle_write_data` reads them back out. `payload` is still the
    /// full sample body (its key fields are what identifies the
    /// instance being disposed/unregistered) -- §7.7.1: the agent only
    /// needs the key portion, but a real client sends the whole sample
    /// it has, same as this daemon's own dispose path expects.
    ///
    /// # Errors
    /// [`ClientError::InvalidState`].
    pub fn request_write_sample(
        &mut self,
        writer: ObjectId,
        payload: &[u8],
        dispose: bool,
        unregister: bool,
        reliable: bool,
    ) -> Result<u16, ClientError> {
        self.require_connected()?;
        let request_id = self.allocate_request_id();
        let base = BaseObjectRequest {
            request_id: request_id_octets(request_id),
            object_id: writer,
        };
        let mut flags = 0u8;
        if unregister {
            flags |= zerodds_xrce::sample_info::SampleInfoFlags::INSTANCE_STATE_UNREGISTERED;
        }
        if dispose {
            flags |= zerodds_xrce::sample_info::SampleInfoFlags::INSTANCE_STATE_DISPOSED;
        }
        let sm = WriteDataPayloadSample {
            base,
            info: zerodds_xrce::sample_info::SampleInfo {
                state: zerodds_xrce::sample_info::SampleInfoFlags(flags),
                sequence_number: 0,
                session_time_offset: self.session_time_offset_ms(),
            },
            serialized_data: payload.to_vec(),
        }
        .into_submessage()
        .map_err(|_| ClientError::QueueFull)?;
        self.send_maybe_fragmented(sm, reliable)?;
        Ok(request_id)
    }

    /// Milliseconds since the session timestamp, the `client_timestamp` of the
    /// `CREATE_CLIENT` this client sent (§7.7.2, Table 1), for the
    /// `session_time_offset` of a sample it writes. Measured from the moment
    /// of `connect` with the clock of the process; 0 where there is no clock
    /// (`no_std`) or `connect` has not run.
    #[cfg(feature = "std")]
    fn session_time_offset_ms(&self) -> u32 {
        self.session_started.map_or(0, |t| {
            u32::try_from(t.elapsed().as_millis()).unwrap_or(u32::MAX)
        })
    }

    /// See the `std` variant: without a clock the offset is 0.
    #[cfg(not(feature = "std"))]
    fn session_time_offset_ms(&self) -> u32 {
        0
    }

    /// Submits a READ_DATA operation (§7.8.5/§8.3.5.9.2, pull-based).
    /// `stream_id` doubles as the requested reply stream
    /// (`ReadSpecification::preferred_stream_id`) — the agent echoes it
    /// back as the stream `DATA` arrives on.
    ///
    /// # Errors
    /// [`ClientError::InvalidState`].
    #[allow(clippy::too_many_arguments)]
    pub fn request_read(
        &mut self,
        reader: ObjectId,
        stream_id: u8,
        data_format: DataFormat,
        content_filter_expression: Option<alloc::string::String>,
        delivery_control: Option<DataDeliveryControl>,
        reliable: bool,
    ) -> Result<u16, ClientError> {
        self.require_connected()?;
        let request_id = self.allocate_request_id();
        let base = BaseObjectRequest {
            request_id: request_id_octets(request_id),
            object_id: reader,
        };
        let sm = ReadDataPayload {
            base,
            read_specification: ReadSpecification {
                preferred_stream_id: stream_id,
                data_format,
                content_filter_expression,
                delivery_control,
            },
        }
        .into_submessage_with(Endianness::Little, self.delivery_control_form)
        .map_err(|_| ClientError::QueueFull)?;
        self.send_maybe_fragmented(sm, reliable)?;
        Ok(request_id)
    }

    /// Closes the session: sends `DELETE(OBJECTID_CLIENT)` (§7.8.2's
    /// disconnect path) and resets local state back to
    /// [`ClientState::Disconnected`] regardless of whether the send
    /// succeeded — the agent's own session-timeout GC is the fallback if
    /// this datagram is lost.
    pub fn disconnect(&mut self) {
        if self.state == ClientState::Connected || self.state == ClientState::Connecting {
            let request_id = self.allocate_request_id();
            let base = BaseObjectRequest {
                request_id: request_id_octets(request_id),
                object_id: OBJECTID_CLIENT,
            };
            if let Ok(sm) = (DeletePayload {
                representation: base.encode().to_vec(),
            })
            .into_submessage()
            {
                let _ = self.send_control(sm);
            }
        }
        self.state = ClientState::Disconnected;
        self.streams.clear();
        self.fragment_runs.clear();
        self.best_effort_in.clear();
    }

    /// Drains the transport and decodes whatever the agent sent —
    /// `STATUS_AGENT` (advances [`ClientState::Connecting`] to
    /// [`ClientState::Connected`]), `STATUS`, `DATA`, `HEARTBEAT`
    /// (answered with `ACKNACK`), `ACKNACK` (repairs any of this
    /// client's own NACK'd sequence numbers by resending the exact
    /// buffered bytes), and `FRAGMENT` (reassembled transparently before
    /// any of the above see it). A reliable-stream message is gated
    /// through `recv_data`/`drain_in_order` first, exactly like
    /// `zerodds-xrce-agent`'s `SessionEngine::handle_message`.
    ///
    /// # Errors
    /// [`ClientError::Protocol`] never stops the loop — one malformed
    /// datagram is skipped, not fatal; this only returns `Err` for a
    /// transport-level read failure.
    pub fn poll(&mut self, now: Duration) -> Result<Vec<ClientEvent>, ClientError> {
        let mut events = Vec::new();
        while let Some(bytes) = self.transport.try_recv()? {
            let Ok(msg) = Message::decode(&bytes) else {
                continue; // malformed datagram -- not this client's problem to crash over
            };
            self.ingest_message(&msg.header, &msg.submessages, now, &mut events);
        }
        Ok(events)
    }

    /// Drives time-based work: `HEARTBEAT` emission for every reliable
    /// stream with unacknowledged in-flight messages (§8.4.14.1: "the
    /// sender sends HeartBeat messages ... at some rate until
    /// HighestAcknowledgedSequenceNumber matches the
    /// HighestSentSequenceNumber"). Without this, a lost datagram on a
    /// reliable stream can never be recovered: §8.4.14.2 makes the
    /// *receiver* passive when it is an XRCE Agent -- "it only sends
    /// ACKNACK messages in response to receiving a HEARTBEAT" -- so
    /// nothing ever prompts the agent to notice the gap and ask for a
    /// resend unless this client says what it has sent. Call this
    /// periodically (e.g. every 100 ms, alongside [`Self::poll`]) for the
    /// duration of a session that uses any `reliable: true` operation.
    ///
    /// # Errors
    /// A transport send failure. Best-effort per stream: one stream's
    /// `HEARTBEAT` failing to build/send does not stop the others.
    pub fn tick(&mut self, now: Duration) -> Result<(), ClientError> {
        let stream_ids: Vec<u8> = self.streams.keys().copied().collect();
        for stream_id in stream_ids {
            let Some(rs) = self.streams.get_mut(&stream_id) else {
                continue;
            };
            let Some(hb) = rs.pending_heartbeat(now) else {
                continue;
            };
            let Ok(sm) = hb.into_submessage() else {
                continue;
            };
            let header = MessageHeader {
                session_id: SessionId(self.session_id),
                stream_id: StreamId::NONE,
                sequence_nr: SerialNumber16::new(0),
                client_key: self.header_client_key(),
            };
            self.send_message(header, alloc::vec![sm])?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------
    // Internal wire plumbing
    // -----------------------------------------------------------------

    fn require_connected(&self) -> Result<(), ClientError> {
        if self.state != ClientState::Connected {
            return Err(ClientError::InvalidState);
        }
        Ok(())
    }

    fn allocate_request_id(&mut self) -> u16 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        id
    }

    fn header_client_key(&self) -> Option<ClientKey> {
        if self.session_id <= 127 {
            Some(self.client_key)
        } else {
            None
        }
    }

    /// Sends a single submessage on `STREAMID_NONE`, unsequenced — for
    /// `CREATE_CLIENT`/`DELETE(OBJECTID_CLIENT)`/`ACKNACK`, none of which
    /// spec gives a retransmission contract (the handshake has no session
    /// yet to run reliability on; a disconnect racing a lost datagram
    /// falls back to the agent's own session timeout).
    fn send_control(&mut self, sm: Submessage) -> Result<(), ClientError> {
        let header = MessageHeader {
            session_id: SessionId(self.session_id),
            stream_id: StreamId::NONE,
            sequence_nr: SerialNumber16::new(0),
            client_key: self.header_client_key(),
        };
        self.send_message(header, alloc::vec![sm])
    }

    /// Sends `sm` on `stream_id`, assigning a real sequence number and
    /// buffering it for retransmission when `reliable` is `true`
    /// (§8.4.14.1). Splits into `FRAGMENT` first when the whole message
    /// would exceed [`Self::requested_mtu`] and `reliable` is `true`;
    /// otherwise sends it whole even if oversized (best-effort has no
    /// FRAGMENT contract to use).
    fn send_maybe_fragmented(&mut self, sm: Submessage, reliable: bool) -> Result<(), ClientError> {
        let stream_id = if reliable { 0x80 } else { 0x01 };
        let header_size = if self.header_client_key().is_some() {
            MessageHeader::WIRE_SIZE_WITH_KEY
        } else {
            MessageHeader::WIRE_SIZE_NO_KEY
        };
        let budget = usize::from(self.requested_mtu);
        if !reliable || header_size + sm.wire_size_unpadded() <= budget {
            return self.send_on_stream(sm, stream_id, reliable);
        }
        let Some(fragment_budget) = budget.checked_sub(header_size + SubmessageHeader::WIRE_SIZE)
        else {
            return Err(ClientError::QueueFull);
        };
        if fragment_budget == 0 {
            return Err(ClientError::QueueFull);
        }
        let encoded = encode_submessages(core::slice::from_ref(&sm));
        let chunks: Vec<&[u8]> = encoded.chunks(fragment_budget).collect();
        let last = chunks.len().saturating_sub(1);
        for (i, chunk) in chunks.into_iter().enumerate() {
            let frag_sm = FragmentPayload {
                data: chunk.to_vec(),
                last_fragment: i == last,
            }
            .into_submessage()
            .map_err(|_| ClientError::QueueFull)?;
            self.send_on_stream(frag_sm, stream_id, true)?;
        }
        Ok(())
    }

    fn send_on_stream(
        &mut self,
        sm: Submessage,
        stream_id: u8,
        reliable: bool,
    ) -> Result<(), ClientError> {
        let mut header = MessageHeader {
            session_id: SessionId(self.session_id),
            stream_id: StreamId(stream_id),
            sequence_nr: SerialNumber16::new(0),
            client_key: self.header_client_key(),
        };
        if reliable {
            let rs = self.streams.entry(stream_id).or_insert_with(|| {
                ReliableStreamState::new(StreamId(stream_id), ReliableConfig::default())
            });
            let payload = encode_submessages(core::slice::from_ref(&sm));
            let seq = rs.submit(payload).map_err(|_| ClientError::QueueFull)?;
            header.sequence_nr = seq;
        }
        self.send_message(header, alloc::vec![sm])
    }

    fn send_message(
        &mut self,
        header: MessageHeader,
        submessages: Vec<Submessage>,
    ) -> Result<(), ClientError> {
        let msg = Message {
            header,
            submessages,
        };
        let bytes = msg.encode().map_err(|_| ClientError::QueueFull)?;
        self.transport.send(&bytes)
    }

    /// Mirrors `zerodds-xrce-agent`'s `SessionEngine::handle_message`:
    /// reliable-stream messages are gated through `recv_data`/
    /// `drain_in_order` before dispatch; best-effort/`StreamId::NONE`
    /// dispatch immediately.
    fn ingest_message(
        &mut self,
        header: &MessageHeader,
        submessages: &[Submessage],
        now: Duration,
        events: &mut Vec<ClientEvent>,
    ) {
        if header.stream_id.is_reliable() {
            let rs = self.streams.entry(header.stream_id.0).or_insert_with(|| {
                ReliableStreamState::new(header.stream_id, ReliableConfig::default())
            });
            if rs
                .recv_data(header.sequence_nr, encode_submessages(submessages))
                .is_ok()
            {
                for (seq, bytes) in rs.drain_in_order() {
                    let decoded = decode_submessages(&bytes);
                    let mut h = *header;
                    h.sequence_nr = seq;
                    self.dispatch_submessages(&h, &decoded, now, events);
                }
            }
        } else {
            // §8.3.2.3: a best-effort stream is ordered and duplicates are
            // discarded, which without retransmission means only a message
            // newer than the last one is taken.
            if header.stream_id.is_best_effort() {
                let stream_id = header.stream_id.0;
                if let Some(newest) = self.best_effort_in.get(&stream_id) {
                    if !newest.wrapping_lt(header.sequence_nr) {
                        events.push(ClientEvent::BestEffortMessageDiscarded {
                            stream_id,
                            sequence_nr: header.sequence_nr.raw(),
                        });
                        return;
                    }
                }
                self.best_effort_in.insert(stream_id, header.sequence_nr);
            }
            self.dispatch_submessages(header, submessages, now, events);
        }
    }

    fn dispatch_submessages(
        &mut self,
        header: &MessageHeader,
        submessages: &[Submessage],
        now: Duration,
        events: &mut Vec<ClientEvent>,
    ) {
        let mut effective: Vec<Submessage> = Vec::with_capacity(submessages.len());
        for sm in submessages {
            if sm.header.submessage_id == SubmessageId::Fragment {
                let Ok(frag) = FragmentPayload::try_from_submessage(sm) else {
                    continue;
                };
                let stream_id = header.stream_id.0;
                let (assembler_key, offset) = match self.fragment_runs.get(&stream_id) {
                    Some(run) => (
                        AssemblerKey {
                            stream_id,
                            base_seq: run.base_seq,
                        },
                        run.next_offset,
                    ),
                    None => (
                        AssemblerKey {
                            stream_id,
                            base_seq: header.sequence_nr.raw(),
                        },
                        0,
                    ),
                };
                let frag_len = frag.data.len() as u32;
                match self.fragments.add_fragment(
                    assembler_key,
                    offset,
                    frag.last_fragment,
                    frag.data,
                    now,
                ) {
                    Ok(Some(complete)) => {
                        self.fragment_runs.remove(&stream_id);
                        // §8.3.5.14: every submessage of the run, 4-aligned
                        // (§8.3.3); only the first was taken.
                        match zerodds_xrce::submessages::decode_submessages(&complete) {
                            Ok(reassembled) if !reassembled.is_empty() => {
                                effective.extend(reassembled);
                            }
                            Ok(_) | Err(_) => {
                                events.push(ClientEvent::FragmentDiscarded { stream_id })
                            }
                        }
                    }
                    Ok(None) => {
                        self.fragment_runs.insert(
                            stream_id,
                            FragmentRun {
                                base_seq: assembler_key.base_seq,
                                next_offset: offset.saturating_add(frag_len),
                                last_arrival: now,
                            },
                        );
                    }
                    Err(_) => {
                        self.fragment_runs.remove(&stream_id);
                        events.push(ClientEvent::FragmentDiscarded { stream_id });
                    }
                }
            } else {
                effective.push(sm.clone());
            }
        }
        self.fragments.gc(now);
        self.fragment_runs
            .retain(|_, run| now.saturating_sub(run.last_arrival) <= DEFAULT_GC_TTL);

        for sm in &effective {
            self.handle_one_submessage(header, sm, events);
        }
    }

    fn handle_one_submessage(
        &mut self,
        header: &MessageHeader,
        sm: &Submessage,
        events: &mut Vec<ClientEvent>,
    ) {
        match sm.header.submessage_id {
            SubmessageId::StatusAgent => self.handle_status_agent(sm, events),
            SubmessageId::Status => Self::handle_status(sm, events),
            SubmessageId::Info => Self::handle_info(sm, events),
            SubmessageId::Data => Self::handle_data(sm, events),
            SubmessageId::Heartbeat => self.handle_heartbeat(header, sm),
            SubmessageId::AckNack => self.handle_acknack(header, sm),
            SubmessageId::Timestamp => {
                if let Ok(ts) = TimestampPayload::try_from_submessage(sm) {
                    events.push(ClientEvent::TimestampReceived {
                        transmit_timestamp: ts.transmit_timestamp,
                    });
                }
            }
            SubmessageId::TimestampReply => {
                if let Ok(reply) = TimestampReplyPayload::try_from_submessage(sm) {
                    events.push(ClientEvent::TimestampReply {
                        transmit_timestamp: reply.transmit_timestamp,
                        receive_timestamp: reply.receive_timestamp,
                        originate_timestamp: reply.originate_timestamp,
                    });
                }
            }
            _ => {} // Create/Delete/WriteData/ReadData/GetInfo: agent never sends these
        }
    }

    fn handle_status_agent(&mut self, sm: &Submessage, events: &mut Vec<ClientEvent>) {
        let Ok(payload) = StatusAgentPayload::try_from_submessage(sm) else {
            return;
        };
        // A STATUS_AGENT answers a CREATE_CLIENT; one that arrives when none
        // is pending is the answer to the `DELETE(OBJECTID_CLIENT)` that Table
        // 14 of DDS-XRCE 1.0 gives a STATUS_AGENT (an agent with
        // `delete_client_reply: status_agent`), and changes nothing here.
        if self.state != ClientState::Connecting {
            return;
        }
        // §8.3.5.5.2: the IDL of Annex A has the AGENT_Representation alone
        // (a success), the text and the eProsima agent a ResultStatus in front
        // of it; either is read.
        let e = sm.header.body_endianness();
        let Ok((result, consumed)) = payload.result_and_position(e) else {
            return;
        };
        let Ok((agent, _)) = AgentRepresentation::decode_form(
            &payload.representation,
            consumed,
            e,
            self.representation_timestamps,
        ) else {
            return;
        };
        if result.status_code.is_success() {
            self.state = ClientState::Connected;
            events.push(ClientEvent::Connected { agent });
        } else {
            events.push(ClientEvent::ConnectRejected {
                code: result.status_code,
            });
        }
    }

    fn handle_info(sm: &Submessage, events: &mut Vec<ClientEvent>) {
        let Ok(payload) = InfoPayload::try_from_submessage(sm) else {
            return;
        };
        let Ok((reply, _)) =
            BaseObjectReply::decode_with(&payload.representation, sm.header.body_endianness())
        else {
            return;
        };
        let request_id = request_id_from_octets(reply.related_request.request_id);
        let object_id = reply.related_request.object_id;
        match payload.parse(sm.header.body_endianness()) {
            Ok((reply, info)) => events.push(ClientEvent::Info {
                request_id,
                object_id,
                code: reply.result.status_code,
                info,
            }),
            Err(_) => events.push(ClientEvent::InfoUnreadable {
                request_id,
                object_id,
            }),
        }
    }

    fn handle_status(sm: &Submessage, events: &mut Vec<ClientEvent>) {
        let Ok(payload) = StatusPayload::try_from_submessage(sm) else {
            return;
        };
        let Ok((reply, _)) =
            BaseObjectReply::decode_with(&payload.representation, sm.header.body_endianness())
        else {
            return;
        };
        events.push(ClientEvent::Status {
            request_id: request_id_from_octets(reply.related_request.request_id),
            object_id: reply.related_request.object_id,
            code: reply.result.status_code,
        });
    }

    fn handle_data(sm: &Submessage, events: &mut Vec<ClientEvent>) {
        let Ok(any) = AnyData::try_from_submessage(sm) else {
            return;
        };
        let base = any.base();
        let request_id = request_id_from_octets(base.request_id);
        let reader = base.object_id;
        events.push(match any {
            AnyData::Data(p) => ClientEvent::Data {
                request_id,
                reader,
                payload: p.serialized_data,
            },
            AnyData::Sample(p) => ClientEvent::Sample {
                request_id,
                reader,
                info: p.info,
                payload: p.serialized_data,
            },
            AnyData::DataSeq(p) => ClientEvent::SampleSequence {
                request_id,
                reader,
                format: DataFormat::DataSeq,
                sample_count: p.sample_count,
                info_base: None,
                elements: p.elements,
            },
            AnyData::SampleSeq(p) => ClientEvent::SampleSequence {
                request_id,
                reader,
                format: DataFormat::SampleSeq,
                sample_count: p.sample_count,
                info_base: None,
                elements: p.elements,
            },
            AnyData::PackedSamples(p) => ClientEvent::SampleSequence {
                request_id,
                reader,
                format: DataFormat::PackedSamples,
                sample_count: p.sample_count,
                info_base: Some(p.info_base),
                elements: p.elements,
            },
        });
    }

    fn handle_heartbeat(&mut self, header: &MessageHeader, sm: &Submessage) {
        let Ok(hb) = HeartbeatPayload::try_from_submessage(sm) else {
            return;
        };
        let rs = self.streams.entry(hb.stream_id).or_insert_with(|| {
            ReliableStreamState::new(StreamId(hb.stream_id), ReliableConfig::default())
        });
        let last_seen = SerialNumber16::new(hb.last_unacked_seq_nr as u16);
        let ack = rs.pending_acknack(Some(last_seen));
        let Ok(ack_sm) = ack.into_submessage() else {
            return;
        };
        let out_header = MessageHeader {
            session_id: SessionId(self.session_id),
            stream_id: StreamId::NONE,
            sequence_nr: SerialNumber16::new(0),
            client_key: self.header_client_key(),
        };
        let _ = header; // only the stream id inside `hb` matters here
        let _ = self.send_message(out_header, alloc::vec![ack_sm]);
    }

    fn handle_acknack(&mut self, header: &MessageHeader, sm: &Submessage) {
        let Ok(ack) = AckNackPayload::try_from_submessage(sm) else {
            return;
        };
        let session_id = self.session_id;
        let client_key = self.header_client_key();
        let Some(rs) = self.streams.get_mut(&ack.stream_id) else {
            return;
        };
        let base = ack.first_unacked_seq_num as u16;
        let bitmap = u16::from_le_bytes(ack.nack_bitmap);
        let stream_id = ack.stream_id;
        rs.recv_acknack(ack);
        let mut retransmits: Vec<(MessageHeader, Vec<Submessage>)> = Vec::new();
        for i in 0u16..16 {
            if (bitmap >> i) & 1 == 0 {
                continue;
            }
            let seq = SerialNumber16::new(base.wrapping_add(i));
            let Some(bytes) = rs.get_in_flight(seq) else {
                continue;
            };
            let decoded = decode_submessages(bytes);
            if decoded.is_empty() {
                continue;
            }
            let h = MessageHeader {
                session_id: SessionId(session_id),
                stream_id: StreamId(stream_id),
                sequence_nr: seq,
                client_key,
            };
            retransmits.push((h, decoded));
        }
        let _ = header;
        for (h, sms) in retransmits {
            let _ = self.send_message(h, sms);
        }
    }
}

/// A request id as the two octets a `BaseObjectRequest` carries.
///
/// DDS-XRCE 1.0 gives `RequestId` as `octet[2]` and never as an
/// integer, so the wire has no byte order to get wrong — the pair goes
/// out as it stands, which is why `crates/xrce`'s submessages take
/// `[u8; 2]` and the Annex B.3.1 example pins `{0xAA, 0x01}` literally.
///
/// The question only appears one step earlier: this client hands out a
/// `u16` counter, and something has to turn it into that pair. Nothing
/// said which way round, and the ports diverged the first time two of
/// them were written in parallel — three chose big-endian and three
/// little-endian, and every one of them passed its own tests, because a
/// test that pins `{0xAA, 0x01}` and reads it back as `0x01AA` is
/// consistent with itself whichever way the counter is spelled.
///
/// Big-endian, and stated here so no further port has to decide: it is
/// what the committed fixtures in `endpoints/testdata/xrce-client`
/// already hold (`request_id: [0x00, 0x01]` for the first request), and
/// it reads in the order the octets travel.
///
/// An application that wants a different pairing is free to build its
/// own `[u8; 2]` — the wire carries whatever it is given. This is the
/// answer for the ports of this client, so that they agree.
#[must_use]
pub const fn request_id_octets(id: u16) -> [u8; 2] {
    id.to_be_bytes()
}

/// Inverse of [`request_id_octets`] — turns a received `request_id`
/// octet pair back into the `u16` this client's own counter handed out
/// (or, for a request this client did not itself send the octets for,
/// whatever a peer speaking the same big-endian convention meant).
#[must_use]
pub const fn request_id_from_octets(octets: [u8; 2]) -> u16 {
    u16::from_be_bytes(octets)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::string::ToString;
    use alloc::vec::Vec;
    use core::cell::RefCell;
    use zerodds_xrce::header::CLIENT_KEY_LEN;
    use zerodds_xrce::object_info::ResultStatus;
    use zerodds_xrce::object_kind::ObjectKind;

    /// The counter's octets, pinned so a port has a reference rather
    /// than a choice.
    ///
    /// The first request is `{0x00, 0x01}` — which is what
    /// `endpoints/testdata/xrce-client/xrce_client_create.bin` holds,
    /// and what every port of this client must reproduce.
    #[test]
    fn a_request_id_is_two_octets_most_significant_first() {
        assert_eq!(request_id_octets(1), [0x00, 0x01]);
        assert_eq!(request_id_octets(0x01AA), [0x01, 0xAA]);
        assert_eq!(request_id_octets(u16::MAX), [0xFF, 0xFF]);
        // The Annex B.3.1 pair, read back as the counter that produces it.
        assert_eq!(request_id_octets(0xAA01), [0xAA, 0x01]);
        assert_eq!(request_id_from_octets([0xAA, 0x01]), 0xAA01);
        assert_eq!(request_id_from_octets(request_id_octets(42)), 42);
    }

    /// Mock transport: collects sent datagrams, and lets a test queue up
    /// bytes for `try_recv` to hand back (a tiny in-memory loopback, no
    /// real socket).
    #[derive(Clone)]
    struct MockTransport {
        sent: Rc<RefCell<Vec<Vec<u8>>>>,
        inbox: Rc<RefCell<Vec<Vec<u8>>>>,
    }
    impl MockTransport {
        fn new() -> Self {
            Self {
                sent: Rc::new(RefCell::new(Vec::new())),
                inbox: Rc::new(RefCell::new(Vec::new())),
            }
        }
        fn queue_inbound(&self, bytes: Vec<u8>) {
            self.inbox.borrow_mut().push(bytes);
        }
        fn sent_messages(&self) -> Vec<Message> {
            self.sent
                .borrow()
                .iter()
                .filter_map(|b| Message::decode(b).ok())
                .collect()
        }
    }
    impl ClientTransport for MockTransport {
        fn send(&mut self, payload: &[u8]) -> Result<(), ClientError> {
            self.sent.borrow_mut().push(payload.to_vec());
            Ok(())
        }
        fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
            Ok(if self.inbox.borrow().is_empty() {
                None
            } else {
                Some(self.inbox.borrow_mut().remove(0))
            })
        }
    }

    fn key() -> ClientKey {
        ClientKey([0xAB; CLIENT_KEY_LEN])
    }

    fn client() -> XrceClient<MockTransport> {
        XrceClient::new(key(), 0x81, MockTransport::new())
    }

    #[test]
    fn new_client_starts_disconnected() {
        let c = client();
        assert_eq!(c.state(), ClientState::Disconnected);
        assert_eq!(c.client_key(), key());
    }

    #[test]
    fn connect_sends_a_real_create_client_and_moves_to_connecting() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        c.connect([0x01, 0x00]).expect("connect");
        assert_eq!(c.state(), ClientState::Connecting);
        let sent = transport.sent_messages();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].submessages.len(), 1);
        assert_eq!(
            sent[0].submessages[0].header.submessage_id,
            SubmessageId::CreateClient
        );
        let payload = CreateClientPayload::try_from_submessage(&sent[0].submessages[0]).unwrap();
        let rep = payload.representation;
        assert_eq!(rep.client_key, key());
        assert_eq!(rep.session_id, 0x81);
        assert_eq!(rep.requested_mtu, Some(DEFAULT_MTU));
    }

    #[test]
    fn double_connect_rejected() {
        let mut c = client();
        c.connect([0x01, 0x00]).expect("first");
        let err = c.connect([0x01, 0x00]).expect_err("second");
        assert_eq!(err, ClientError::InvalidState);
    }

    #[test]
    fn create_without_connect_rejected() {
        let mut c = client();
        let oid = ObjectId::from_raw(0x0010);
        let v = ObjectVariant::ByReference("topic-ref".to_string());
        let err = c
            .create_object(oid, v, ObjectVariantExtra::default(), false, false, true)
            .expect_err("disconnected");
        assert_eq!(err, ClientError::InvalidState);
    }

    #[test]
    fn write_without_connect_rejected() {
        let mut c = client();
        let oid = ObjectId::from_raw(0x0010);
        let err = c.request_write(oid, b"x", true).expect_err("disconnected");
        assert_eq!(err, ClientError::InvalidState);
    }

    #[test]
    fn read_without_connect_rejected() {
        let mut c = client();
        let oid = ObjectId::from_raw(0x0010);
        let err = c
            .request_read(oid, 0x80, DataFormat::Data, None, None, true)
            .expect_err("disconnected");
        assert_eq!(err, ClientError::InvalidState);
    }

    /// Drives a full connect handshake via a real `STATUS_AGENT` reply
    /// (the same bytes shape `zerodds-xrce-agent`'s `agent_status_reply`
    /// builds: leading `ResultStatus` then `AGENT_Representation`), then
    /// checks the client is really connected and every op sends a real,
    /// decodable submessage with strictly increasing request ids.
    fn connect_and_ack(c: &mut XrceClient<MockTransport>, transport: &MockTransport) {
        c.connect([0x01, 0x00]).expect("connect");
        let agent_rep = AgentRepresentation {
            timestamp: None,
            version: [0x01, 0x00],
            vendor_id: [0, 0],
            properties: None,
        }
        .encode()
        .unwrap();
        let mut body = ResultStatus::ok().encode().to_vec();
        body.extend_from_slice(&agent_rep);
        let sm = StatusAgentPayload {
            representation: body,
        }
        .into_submessage()
        .unwrap();
        let header = MessageHeader {
            session_id: SessionId(0x80),
            stream_id: StreamId::NONE,
            sequence_nr: SerialNumber16::new(0),
            client_key: None,
        };
        let msg = Message {
            header,
            submessages: alloc::vec![sm],
        };
        transport.queue_inbound(msg.encode().unwrap());
        let events = c.poll(Duration::ZERO).unwrap();
        assert_eq!(
            events,
            alloc::vec![ClientEvent::Connected {
                agent: AgentRepresentation {
                    timestamp: None,
                    version: [0x01, 0x00],
                    vendor_id: [0, 0],
                    properties: None,
                }
            }]
        );
        assert_eq!(c.state(), ClientState::Connected);
    }

    #[test]
    fn full_lifecycle_creates_unique_request_ids_and_real_submessages() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);

        let oid = ObjectId::new(0x0010, ObjectKind::Topic).unwrap();
        let r1 = c
            .create_object(
                oid,
                ObjectVariant::ByReference("a".to_string()),
                ObjectVariantExtra {
                    domain_id: None,
                    parent: Some(ObjectId::new(0x0001, ObjectKind::Participant).unwrap()),
                },
                false,
                false,
                true,
            )
            .expect("create");
        let r2 = c.delete_object(oid, true).expect("delete");
        let r3 = c.request_write(oid, b"x", true).expect("write");
        let r4 = c
            .request_read(oid, 0x80, DataFormat::Data, None, None, true)
            .expect("read");
        // Request IDs are strictly monotonic, continuing from CREATE_CLIENT/
        // the handshake's own implicit request id 0 not consuming a slot.
        assert_eq!(r1, 1);
        assert_eq!(r2, 2);
        assert_eq!(r3, 3);
        assert_eq!(r4, 4);

        let sent = transport.sent_messages();
        // [0]=CREATE_CLIENT, [1]=CREATE, [2]=DELETE, [3]=WRITE_DATA, [4]=READ_DATA
        assert_eq!(sent.len(), 5);
        let ids = [
            SubmessageId::CreateClient,
            SubmessageId::Create,
            SubmessageId::Delete,
            SubmessageId::WriteData,
            SubmessageId::ReadData,
        ];
        for (msg, expected) in sent.iter().zip(ids.iter()) {
            assert_eq!(msg.submessages[0].header.submessage_id, *expected);
        }
        // Every reliable message after the handshake got a real,
        // strictly increasing sequence number -- not sequence_nr=0.
        let seqs: Vec<u16> = sent[1..]
            .iter()
            .map(|m| m.header.sequence_nr.raw())
            .collect();
        for w in seqs.windows(2) {
            assert!(w[1] > w[0], "sequence numbers must strictly increase");
        }
    }

    /// `create_object` must build the real wire `ObjectVariant` (§7.7.3:
    /// outer `ObjectKind` byte + kind-specific trailing field, not just
    /// the inner `RepresentationFormat` payload) -- the same framing
    /// `zerodds-xrce-agent`'s `handle_create` now requires (a byte-oracle
    /// fix against real eProsima `CREATE` traffic; see
    /// `crates/xrce/src/object_repr.rs`'s module doc). Round-trips a
    /// `Participant` (carries `domain_id`) and a `Subscriber` (carries a
    /// parent `ObjectId`) CREATE through `ObjectVariant::decode_for_kind`
    /// to prove the bytes this client actually sends decode correctly.
    #[test]
    fn create_object_builds_the_real_two_level_object_variant() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);

        let participant = ObjectId::new(0x001, ObjectKind::Participant).unwrap();
        c.create_object(
            participant,
            ObjectVariant::ByXmlString("<dds/>".to_string()),
            ObjectVariantExtra {
                domain_id: Some(0),
                parent: None,
            },
            false,
            false,
            true,
        )
        .expect("create participant");

        let subscriber = ObjectId::new(0x004, ObjectKind::Subscriber).unwrap();
        c.create_object(
            subscriber,
            ObjectVariant::ByXmlString(alloc::string::String::new()),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(participant),
            },
            false,
            false,
            true,
        )
        .expect("create subscriber");

        let sent = transport.sent_messages();
        let creates: Vec<_> = sent
            .iter()
            .filter(|m| m.submessages[0].header.submessage_id == SubmessageId::Create)
            .collect();
        assert_eq!(creates.len(), 2);

        let payload = CreatePayload::try_from_submessage(&creates[0].submessages[0]).unwrap();
        let (base, n) = BaseObjectRequest::decode(&payload.representation).unwrap();
        assert_eq!(base.object_id, participant);
        let (kind, variant, extra, consumed) =
            ObjectVariant::decode_for_kind(&payload.representation[n..], Endianness::Little)
                .unwrap();
        assert_eq!(kind, ObjectKind::Participant);
        assert_eq!(variant, ObjectVariant::ByXmlString("<dds/>".to_string()));
        assert_eq!(extra.domain_id, Some(0));
        assert_eq!(extra.parent, None);
        assert_eq!(consumed, payload.representation.len() - n);

        let payload = CreatePayload::try_from_submessage(&creates[1].submessages[0]).unwrap();
        let (base, n) = BaseObjectRequest::decode(&payload.representation).unwrap();
        assert_eq!(base.object_id, subscriber);
        let (kind, variant, extra, consumed) =
            ObjectVariant::decode_for_kind(&payload.representation[n..], Endianness::Little)
                .unwrap();
        assert_eq!(kind, ObjectKind::Subscriber);
        assert_eq!(
            variant,
            ObjectVariant::ByXmlString(alloc::string::String::new())
        );
        assert_eq!(extra.domain_id, None);
        assert_eq!(extra.parent, Some(participant));
        assert_eq!(consumed, payload.representation.len() - n);
    }

    /// `request_write_sample` must build a real `FORMAT_SAMPLE` WRITE_DATA
    /// with the dispose/unregister bits `zerodds-xrce-agent`'s
    /// `handle_write_data` reads back via `SampleInfoFlags`.
    #[test]
    fn request_write_sample_sets_the_dispose_flag() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);
        let writer = ObjectId::new(0x0030, ObjectKind::DataWriter).unwrap();
        c.request_write_sample(writer, b"abc", true, false, true)
            .expect("write sample");

        let sent = transport.sent_messages();
        let write = sent.last().unwrap();
        assert_eq!(
            write.submessages[0].header.submessage_id,
            SubmessageId::WriteData
        );
        let payload =
            zerodds_xrce::submessages::write_data::WriteDataPayloadSample::try_from_submessage(
                &write.submessages[0],
            )
            .unwrap();
        assert!(payload.info.state.is_disposed());
        assert!(!payload.info.state.is_unregistered());
        assert_eq!(payload.serialized_data, b"abc");
    }

    #[test]
    fn disconnect_sends_delete_client_and_clears_state() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);
        c.disconnect();
        assert_eq!(c.state(), ClientState::Disconnected);
        let sent = transport.sent_messages();
        let last = sent.last().unwrap();
        assert_eq!(
            last.submessages[0].header.submessage_id,
            SubmessageId::Delete
        );
        let payload = DeletePayload::try_from_submessage(&last.submessages[0]).unwrap();
        let (base, _) = BaseObjectRequest::decode(&payload.representation).unwrap();
        assert_eq!(base.object_id, OBJECTID_CLIENT);
    }

    /// §7.8.2.2: get_info of the agent "may be used prior to establishing a
    /// Session". `request_agent_info` works in any state and sends a GET_INFO
    /// for the agent, with the InfoMask, in a header without a session.
    #[test]
    fn request_agent_info_works_before_the_session_and_sends_the_mask() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        let id = c.request_agent_info(InfoMask::ALL).expect("before connect");
        assert_eq!(c.state(), ClientState::Disconnected);
        let sent = transport.sent_messages();
        assert_eq!(sent[0].header.session_id, SessionId(0x80));
        assert_eq!(sent[0].header.stream_id, StreamId::NONE);
        let payload = GetInfoPayload::try_from_submessage(&sent[0].submessages[0]).unwrap();
        let (base, mask) = payload.parse(Endianness::Little).unwrap();
        assert_eq!(base.object_id, zerodds_xrce::object_id::OBJECTID_AGENT);
        assert_eq!(request_id_from_octets(base.request_id), id);
        assert_eq!(mask, InfoMask::ALL);
    }

    /// `request_get_info` asks about an object of the session and needs one.
    #[test]
    fn request_get_info_needs_a_session_and_names_the_object() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        let object = ObjectId::new(0x0030, ObjectKind::DataWriter).unwrap();
        assert_eq!(
            c.request_get_info(object, InfoMask::ALL, true),
            Err(ClientError::InvalidState)
        );
        connect_and_ack(&mut c, &transport);
        let id = c
            .request_get_info(object, InfoMask(InfoMask::INFO_ACTIVITY), true)
            .unwrap();
        let sent = transport.sent_messages();
        let payload =
            GetInfoPayload::try_from_submessage(&sent.last().unwrap().submessages[0]).unwrap();
        let (base, mask) = payload.parse(Endianness::Little).unwrap();
        assert_eq!(base.object_id, object);
        assert_eq!(request_id_from_octets(base.request_id), id);
        assert!(mask.activity() && !mask.configuration());
    }

    /// §8.3.5.7: an INFO becomes a `ClientEvent::Info` with the status, the
    /// activity and the configuration; an INFO of a failed GET_INFO has the
    /// error in `code`.
    #[test]
    fn an_info_is_read_into_an_event() {
        use zerodds_xrce::object_info::{ActivityInfoVariant, ObjectConfig};
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        let agent_object = zerodds_xrce::object_id::OBJECTID_AGENT;
        let info = ObjectInfo {
            activity: Some(ActivityInfoVariant::Agent {
                availability: 1,
                address_seq: alloc::vec![
                    zerodds_xrce::transport_locator::TransportLocator::Medium(
                        zerodds_xrce::transport_locator::TransportLocatorMedium {
                            address: [10, 0, 0, 1],
                            port: 2018,
                        }
                    )
                ],
            }),
            config: Some(ObjectConfig::Agent(AgentRepresentation {
                timestamp: None,
                version: [1, 0],
                vendor_id: [1, 15],
                properties: None,
            })),
        };
        let info_message = |status: ResultStatusCode, info: &ObjectInfo, e: Endianness| {
            let reply = BaseObjectReply {
                related_request: BaseObjectRequest {
                    request_id: request_id_octets(5),
                    object_id: agent_object,
                },
                result: ResultStatus {
                    status_code: status,
                    implementation_status: 0,
                },
            };
            let sm = InfoPayload::new(reply, info, e)
                .unwrap()
                .into_submessage_with(e)
                .unwrap();
            Message {
                header: MessageHeader {
                    session_id: SessionId(0x80),
                    stream_id: StreamId::NONE,
                    sequence_nr: SerialNumber16::new(0),
                    client_key: None,
                },
                submessages: alloc::vec![sm],
            }
            .encode()
            .unwrap()
        };
        transport.queue_inbound(info_message(
            ResultStatusCode::Ok,
            &info,
            Endianness::Little,
        ));
        transport.queue_inbound(info_message(ResultStatusCode::Ok, &info, Endianness::Big));
        transport.queue_inbound(info_message(
            ResultStatusCode::UnknownReference,
            &ObjectInfo::default(),
            Endianness::Little,
        ));
        let events = c.poll(Duration::ZERO).unwrap();
        assert_eq!(
            events,
            alloc::vec![
                ClientEvent::Info {
                    request_id: 5,
                    object_id: agent_object,
                    code: ResultStatusCode::Ok,
                    info: info.clone(),
                },
                ClientEvent::Info {
                    request_id: 5,
                    object_id: agent_object,
                    code: ResultStatusCode::Ok,
                    info,
                },
                ClientEvent::Info {
                    request_id: 5,
                    object_id: agent_object,
                    code: ResultStatusCode::UnknownReference,
                    info: ObjectInfo::default(),
                },
            ]
        );
    }

    /// DDS-XRCE 1.0 §8.3.4.2: a STATUS_AGENT with the E flag clear carries
    /// its AGENT_Representation big-endian, properties included.
    #[test]
    fn a_big_endian_status_agent_is_read_with_its_properties() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        c.connect([0x01, 0x00]).expect("connect");
        let agent = AgentRepresentation {
            timestamp: None,
            version: [0x01, 0x00],
            vendor_id: [0, 0],
            properties: Some(alloc::vec![zerodds_xrce::handshake::Property {
                name: "auth".to_string(),
                value: "ok".to_string(),
            }]),
        };
        // The representation follows the ResultStatus in the payload: its
        // PropertySeq is aligned counted from the start of the payload.
        // Both in the byte order of the message: flags 0 is big-endian, and the
        // DHEADER of the ResultStatus (APPENDABLE) is a number too.
        let mut body = alloc::vec::Vec::new();
        ResultStatus::ok().encode_into(&mut body, zerodds_xrce::encoding::Endianness::Big);
        agent
            .encode_into(&mut body, zerodds_xrce::encoding::Endianness::Big)
            .unwrap();
        let sm = Submessage::new(SubmessageId::StatusAgent, 0, body).unwrap();
        let msg = Message {
            header: MessageHeader {
                session_id: SessionId(0x80),
                stream_id: StreamId::NONE,
                sequence_nr: SerialNumber16::new(0),
                client_key: None,
            },
            submessages: alloc::vec![sm],
        };
        transport.queue_inbound(msg.encode().unwrap());
        let events = c.poll(Duration::ZERO).unwrap();
        assert_eq!(events, alloc::vec![ClientEvent::Connected { agent }]);
    }

    /// DDS-XRCE 1.0 §8.3.2.1: the CREATE_CLIENT goes where there is no
    /// session yet, in a header with SESSION_ID_NONE_*: with the ClientKey
    /// for a session_id of 0 to 127 (0x00), without for 128 to 255 (0x80);
    /// the payload asks for the session.
    #[test]
    fn create_client_travels_in_a_header_without_a_session() {
        for (asked, none, carries_key) in [(0x81u8, 0x80u8, false), (0x05, 0x00, true)] {
            let transport = MockTransport::new();
            let mut c = XrceClient::new(key(), asked, transport.clone());
            c.connect([0x01, 0x00]).expect("connect");
            let sent = transport.sent_messages();
            assert_eq!(sent[0].header.session_id, SessionId(none));
            assert_eq!(sent[0].header.client_key.is_some(), carries_key);
            let payload =
                CreateClientPayload::try_from_submessage(&sent[0].submessages[0]).unwrap();
            assert_eq!(payload.representation.session_id, asked);
        }
    }

    /// §7.8.2.1: the `client_timestamp` member is a setting of the client
    /// (default: absent, the form of Annex A); with `Present` the CREATE_CLIENT
    /// carries it after `xrce_vendor_id`, and a STATUS_AGENT is read the same way.
    #[test]
    fn the_representation_timestamp_is_an_option_of_the_client() {
        use zerodds_xrce::encoding::Endianness;
        {
            for (form, len) in [
                (RepresentationTimestamps::Absent, 13 + 1 + 2),
                (RepresentationTimestamps::Present, 8 + 12 + 5 + 1 + 2),
            ] {
                let transport = MockTransport::new();
                let mut c = XrceClient::new(key(), 0x05, transport.clone());
                c.set_representation_timestamps(form);
                c.connect([0x01, 0x00]).expect("connect");
                let sent = transport.sent_messages();
                assert_eq!(sent[0].submessages[0].body.len(), len, "{form:?}");
                // The agent's answer in the same form is read.
                let agent = AgentRepresentation {
                    timestamp: (form == RepresentationTimestamps::Present).then_some(
                        zerodds_xrce::submessages::timestamp::TimePoint {
                            seconds: 7,
                            nanoseconds: 9,
                        },
                    ),
                    version: [1, 0],
                    vendor_id: [0, 0],
                    properties: None,
                };
                let mut body = alloc::vec::Vec::new();
                ResultStatus::ok().encode_into(&mut body, Endianness::Little);
                agent
                    .encode_form(&mut body, Endianness::Little, form)
                    .unwrap();
                let sm = Submessage::new(SubmessageId::StatusAgent, 1, body).unwrap();
                let msg = Message {
                    header: MessageHeader {
                        session_id: SessionId(0x80),
                        stream_id: StreamId::NONE,
                        sequence_nr: SerialNumber16::new(0),
                        client_key: None,
                    },
                    submessages: alloc::vec![sm],
                };
                transport.queue_inbound(msg.encode().unwrap());
                let events = c.poll(Duration::ZERO).unwrap();
                assert_eq!(events, alloc::vec![ClientEvent::Connected { agent }]);
            }
        }
    }

    /// §8.3.2.3: on a best-effort stream a message that is a duplicate of the
    /// last one or older is discarded, and the client says so.
    #[test]
    fn a_best_effort_stream_discards_duplicate_and_older_messages() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);
        let reader = ObjectId::new(0x0020, ObjectKind::DataReader).unwrap();
        let data_on_stream_1 = |seq: u16, byte: u8| {
            let sm = (zerodds_xrce::submessages::data::DataPayload {
                base: BaseObjectRequest {
                    request_id: [0, 1],
                    object_id: reader,
                },
                serialized_data: alloc::vec![byte],
            })
            .into_submessage()
            .unwrap();
            Message {
                header: MessageHeader {
                    session_id: SessionId(0x81),
                    stream_id: StreamId(0x01),
                    sequence_nr: SerialNumber16::new(seq),
                    client_key: None,
                },
                submessages: alloc::vec![sm],
            }
            .encode()
            .unwrap()
        };
        for (seq, byte) in [(3, 0xA0), (3, 0xA1), (2, 0xA2), (4, 0xA3)] {
            transport.queue_inbound(data_on_stream_1(seq, byte));
        }
        let events = c.poll(Duration::ZERO).unwrap();
        let summary: Vec<(bool, Option<u8>)> = events
            .iter()
            .map(|e| match e {
                ClientEvent::Data { payload, .. } => (true, payload.first().copied()),
                ClientEvent::BestEffortMessageDiscarded { .. } => (false, None),
                other => panic!("unexpected event {other:?}"),
            })
            .collect();
        assert_eq!(
            summary,
            alloc::vec![
                (true, Some(0xA0)),
                (false, None),
                (false, None),
                (true, Some(0xA3))
            ]
        );
    }

    /// A `DATA` reply for a prior `READ_DATA` decodes into a
    /// `ClientEvent::Data` with the exact payload bytes, correlated back
    /// to the request id that asked for it.
    /// §8.4.13, Table 14: the update of an object is a CREATE with `reuse`
    /// and `replace` set.
    #[test]
    fn update_object_sends_a_create_with_reuse_and_replace() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);
        let topic = ObjectId::new(0x0020, ObjectKind::Topic).unwrap();
        c.update_object(
            topic,
            ObjectVariant::ByXmlString("<topic name=\"T\" type_name=\"U\"/>".into()),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(ObjectId::new(0x0010, ObjectKind::Participant).unwrap()),
            },
            true,
        )
        .unwrap();
        let sent = transport.sent_messages();
        let create = sent
            .iter()
            .flat_map(|m| m.submessages.iter())
            .find(|sm| sm.header.submessage_id == SubmessageId::Create)
            .expect("a CREATE");
        let payload = CreatePayload::try_from_submessage(create).unwrap();
        assert!(payload.reuse && payload.replace);
    }

    /// §8.3.5.15, §8.3.5.16: the client sends a TIMESTAMP on STREAMID_NONE and
    /// reports the TIMESTAMP_REPLY; a TIMESTAMP of the agent is reported and
    /// answered with the client's times.
    #[test]
    fn the_client_sends_and_reads_timestamps() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        assert_eq!(
            c.send_timestamp(TimePoint::default()),
            Err(ClientError::InvalidState)
        );
        connect_and_ack(&mut c, &transport);
        let t0 = TimePoint {
            seconds: 100,
            nanoseconds: 5,
        };
        c.send_timestamp(t0).unwrap();
        let sent = transport.sent_messages();
        let last = sent.last().unwrap();
        assert_eq!(last.header.stream_id, StreamId::NONE);
        let ts = TimestampPayload::try_from_submessage(&last.submessages[0]).unwrap();
        assert_eq!(ts.transmit_timestamp, t0);

        let (t1, t2) = (
            TimePoint {
                seconds: 105,
                nanoseconds: 7,
            },
            TimePoint {
                seconds: 105,
                nanoseconds: 9,
            },
        );
        let inbound = |sm: Submessage| {
            Message {
                header: MessageHeader {
                    session_id: SessionId(0x80),
                    stream_id: StreamId::NONE,
                    sequence_nr: SerialNumber16::new(0),
                    client_key: None,
                },
                submessages: alloc::vec![sm],
            }
            .encode()
            .unwrap()
        };
        transport.queue_inbound(inbound(
            TimestampReplyPayload {
                transmit_timestamp: t2,
                receive_timestamp: t1,
                originate_timestamp: t0,
            }
            .into_submessage()
            .unwrap(),
        ));
        transport.queue_inbound(inbound(
            TimestampPayload {
                transmit_timestamp: t2,
            }
            .into_submessage()
            .unwrap(),
        ));
        let events = c.poll(Duration::ZERO).unwrap();
        assert_eq!(
            events,
            alloc::vec![
                ClientEvent::TimestampReply {
                    transmit_timestamp: t2,
                    receive_timestamp: t1,
                    originate_timestamp: t0,
                },
                ClientEvent::TimestampReceived {
                    transmit_timestamp: t2,
                },
            ]
        );
        c.send_timestamp_reply(t2, t1, t0).unwrap();
        let sent = transport.sent_messages();
        let reply =
            TimestampReplyPayload::try_from_submessage(&sent.last().unwrap().submessages[0])
                .unwrap();
        assert_eq!(
            (
                reply.originate_timestamp,
                reply.receive_timestamp,
                reply.transmit_timestamp
            ),
            (t2, t1, t0)
        );
    }

    #[test]
    fn poll_decodes_a_data_reply() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);
        let reader = ObjectId::new(0x0020, ObjectKind::DataReader).unwrap();
        let r = c
            .request_read(reader, 0x80, DataFormat::Data, None, None, true)
            .unwrap();

        let data_sm = (zerodds_xrce::submessages::data::DataPayload {
            base: BaseObjectRequest {
                request_id: request_id_octets(r),
                object_id: reader,
            },
            serialized_data: alloc::vec![1, 2, 3, 4],
        })
        .into_submessage()
        .unwrap();
        let header = MessageHeader {
            session_id: SessionId(0x80),
            stream_id: StreamId(0x80),
            // The client's reliable receiver on this stream has not seen
            // any message yet, so its `expected_seq` starts at 0 -- must
            // match, or this "arrives" out of order and stays buffered.
            sequence_nr: SerialNumber16::new(0),
            client_key: None,
        };
        let msg = Message {
            header,
            submessages: alloc::vec![data_sm],
        };
        transport.queue_inbound(msg.encode().unwrap());
        let events = c.poll(Duration::ZERO).unwrap();
        assert_eq!(
            events,
            alloc::vec![ClientEvent::Data {
                request_id: r,
                reader,
                payload: alloc::vec![1, 2, 3, 4],
            }]
        );
    }

    /// §8.3.5.10.2: a DATA in the other formats reaches the application, not
    /// only FORMAT_DATA: a sample with its SampleInfo as `Sample`, the three
    /// sequence formats as `SampleSequence`.
    #[test]
    fn poll_hands_over_the_data_of_every_format() {
        use zerodds_xrce::sample_info::{SampleInfo, SampleInfoFlags};
        use zerodds_xrce::submessages::data::{
            DataPayloadDataSeq, DataPayloadPackedSamples, DataPayloadSample, DataPayloadSampleSeq,
        };
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);
        let reader = ObjectId::new(0x0020, ObjectKind::DataReader).unwrap();
        let request_id = c
            .request_read(reader, 0x80, DataFormat::Sample, None, None, true)
            .unwrap();
        let base = BaseObjectRequest {
            request_id: request_id_octets(request_id),
            object_id: reader,
        };
        let info = SampleInfo {
            state: SampleInfoFlags(0),
            sequence_number: 5,
            session_time_offset: 700,
        };
        let submessages = alloc::vec![
            DataPayloadSample {
                base,
                info,
                serialized_data: alloc::vec![1, 2, 3, 4],
            }
            .into_submessage()
            .unwrap(),
            DataPayloadDataSeq {
                base,
                sample_count: 2,
                elements: alloc::vec![1; 8],
            }
            .into_submessage()
            .unwrap(),
            DataPayloadSampleSeq {
                base,
                sample_count: 1,
                elements: alloc::vec![2; 16],
            }
            .into_submessage()
            .unwrap(),
            DataPayloadPackedSamples {
                base,
                info_base: info,
                sample_count: 1,
                elements: alloc::vec![3; 8],
            }
            .into_submessage()
            .unwrap(),
        ];
        let msg = Message {
            header: MessageHeader {
                session_id: SessionId(0x80),
                stream_id: StreamId(0x80),
                sequence_nr: SerialNumber16::new(0),
                client_key: None,
            },
            submessages,
        };
        transport.queue_inbound(msg.encode().unwrap());
        let events = c.poll(Duration::ZERO).unwrap();
        assert_eq!(
            events,
            alloc::vec![
                ClientEvent::Sample {
                    request_id,
                    reader,
                    info,
                    payload: alloc::vec![1, 2, 3, 4],
                },
                ClientEvent::SampleSequence {
                    request_id,
                    reader,
                    format: DataFormat::DataSeq,
                    sample_count: 2,
                    info_base: None,
                    elements: alloc::vec![1; 8],
                },
                ClientEvent::SampleSequence {
                    request_id,
                    reader,
                    format: DataFormat::SampleSeq,
                    sample_count: 1,
                    info_base: None,
                    elements: alloc::vec![2; 16],
                },
                ClientEvent::SampleSequence {
                    request_id,
                    reader,
                    format: DataFormat::PackedSamples,
                    sample_count: 1,
                    info_base: Some(info),
                    elements: alloc::vec![3; 8],
                },
            ]
        );
    }

    /// §8.3.5.14: the concatenated payloads "shall be interpreted as XRCE
    /// submessages". A run whose payload holds no submessage is reported as
    /// discarded, not accepted without a word.
    #[test]
    fn a_fragment_run_without_a_submessage_is_reported() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);
        let frag_sm = (zerodds_xrce::submessages::fragment::FragmentPayload {
            data: Vec::new(),
            last_fragment: true,
        })
        .into_submessage()
        .unwrap();
        let msg = Message {
            header: MessageHeader {
                session_id: SessionId(0x80),
                stream_id: StreamId(0x80),
                sequence_nr: SerialNumber16::new(0),
                client_key: None,
            },
            submessages: alloc::vec![frag_sm],
        };
        transport.queue_inbound(msg.encode().unwrap());
        let events = c.poll(Duration::ZERO).unwrap();
        assert_eq!(
            events,
            alloc::vec![ClientEvent::FragmentDiscarded {
                stream_id: StreamId(0x80).0
            }]
        );
    }

    /// A `WRITE_DATA` above the client's own MTU on a reliable stream
    /// splits into `FRAGMENT` submessages that reassemble, via the
    /// crate's own `decode_submessages`, back to the exact original
    /// bytes.
    #[test]
    fn write_above_mtu_splits_into_fragments() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        c.set_requested_mtu(32);
        connect_and_ack(&mut c, &transport);
        let writer = ObjectId::new(0x0030, ObjectKind::DataWriter).unwrap();
        let big = alloc::vec![0xCDu8; 100];
        c.request_write(writer, &big, true).unwrap();

        let sent = transport.sent_messages();
        // sent[0] = CREATE_CLIENT (handled by connect_and_ack), rest are
        // the fragments of the WRITE_DATA.
        let frags = &sent[1..];
        assert!(frags.len() > 1, "100 bytes must not fit a 32-byte MTU");
        for m in frags {
            assert_eq!(
                m.submessages[0].header.submessage_id,
                SubmessageId::Fragment
            );
        }
        let mut reassembled = Vec::new();
        for m in frags {
            let frag = FragmentPayload::try_from_submessage(&m.submessages[0]).unwrap();
            reassembled.extend_from_slice(&frag.data);
        }
        let decoded = decode_submessages(&reassembled);
        assert_eq!(decoded.len(), 1);
        let write = WriteDataPayload::try_from_submessage(&decoded[0]).unwrap();
        assert_eq!(write.serialized_data, big);
    }

    /// DDS-XRCE 1.0 §8.3.5.12: "The HEARTBEAT submessage does not belong to
    /// any stream, for this reason the MessageHeader shall have the stream_id
    /// set to STREAM_ID_NONE"; the stream it reports is in its payload.
    #[test]
    fn a_heartbeat_of_the_client_is_on_streamid_none() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);
        let writer = ObjectId::new(0x0030, ObjectKind::DataWriter).unwrap();
        c.request_write(writer, &[1, 2, 3, 4], true).unwrap();
        c.tick(Duration::from_secs(1)).unwrap();
        let sent = transport.sent_messages();
        let hb = sent
            .iter()
            .find(|m| m.submessages[0].header.submessage_id == SubmessageId::Heartbeat)
            .expect("a HEARTBEAT for the unacknowledged WRITE_DATA");
        assert_eq!(hb.header.stream_id, StreamId::NONE);
        let payload = HeartbeatPayload::try_from_submessage(&hb.submessages[0]).unwrap();
        assert_eq!(payload.stream_id, StreamId::BUILTIN_RELIABLE.0);
    }

    /// A `HEARTBEAT` from the agent must be answered with a real
    /// `ACKNACK`, not silence.
    #[test]
    fn heartbeat_is_answered_with_acknack() {
        let transport = MockTransport::new();
        let mut c = XrceClient::new(key(), 0x81, transport.clone());
        connect_and_ack(&mut c, &transport);
        let hb_sm = (HeartbeatPayload {
            first_unacked_seq_nr: 0,
            last_unacked_seq_nr: 0,
            stream_id: 0x80,
        })
        .into_submessage()
        .unwrap();
        let header = MessageHeader {
            session_id: SessionId(0x80),
            stream_id: StreamId::NONE,
            sequence_nr: SerialNumber16::new(0),
            client_key: None,
        };
        let msg = Message {
            header,
            submessages: alloc::vec![hb_sm],
        };
        transport.queue_inbound(msg.encode().unwrap());
        c.poll(Duration::ZERO).unwrap();
        let sent = transport.sent_messages();
        let last = sent.last().unwrap();
        assert_eq!(
            last.submessages[0].header.submessage_id,
            SubmessageId::AckNack
        );
    }
}
