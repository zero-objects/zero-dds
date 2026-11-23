// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The connection endpoint of OASIS AMQP 1.0 Part 2 as a state machine
//! without I/O: the side that answers `open`, `begin` and `attach`.
//!
//! [`Peer`] takes the body of each AMQP frame the partner sent
//! ([`Peer::frame`]) and the time ([`Peer::tick`]), and yields the frames
//! to write ([`Peer::poll_output`]). What a link is attached to is the
//! [`Container`]: it creates the terminus of an attach, takes each
//! delivery and hears of every disposition and detach.
//!
//! What is built, by section of Part 2:
//!
//! * §2.4.1 `open` answered with the limits of [`PeerConfig`]; the agreed
//!   frame size and channel maximum are the smaller of both sides.
//! * §2.4.3, §2.4.4 `close` answered; after a close with an error every
//!   frame is discarded until the partner's close (§2.4.6 DISCARDING).
//! * §2.4.5 idle time-out: an empty frame within half the partner's
//!   `idle-time-out`, a close when the partner is silent for the local one.
//! * §2.5.1 `begin` answered on the lowest free outgoing channel with
//!   `remote-channel`; §2.5.2 `end` answered; §2.5.4 a session error ends
//!   the session and discards its frames until the partner's `end`.
//! * §2.5.6 the six window variables, updated by transfer and flow; a
//!   delivery is sent as far as the partner's incoming window admits and
//!   continued with the next flow.
//! * §2.6.3 `attach` answered with the opposite role and a handle of this
//!   endpoint; a refused attach is answered with a null terminus and a
//!   closing detach that carries the error.
//! * §2.6.4 to §2.6.6 detach answered; input for a link destroyed with an
//!   error, by its handle or by the delivery-ids it held unsettled, ends
//!   the session with `amqp:session:errant-link`.
//! * §2.6.7 link credit in both roles, `drain` and `echo`; `available` in
//!   the flow of a sending link is the container's count.
//! * §2.6.12 deliveries and their settlement in both receiver settle
//!   modes; §2.6.14 multi-frame transfers in both directions, `aborted`.
//! * §2.8.15 to §2.8.18 the error conditions.
//!
//! What Part 4 (Transactions) needs of the transport: a target that is a
//! coordinator (§4.5.1) reaches the container decoded and is answered with
//! the container's; `transfer.state` and the state of a disposition are
//! [`DeliveryState`], which carries `declared` and `transactional-state`;
//! a delivery in a `transactional-state` stays unsettled until the
//! container settles it ([`Peer::settle`]); the `txn-id` of
//! `flow.properties` (§4.4.3) reaches the container; a delivery can be
//! sent with a state ([`Peer::send_with_state`]), aborted
//! ([`Peer::abort`]) and answered later ([`Answer::Later`],
//! [`Peer::answer`]).
//!
//! `no_std` + `alloc`.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::{FrameHeader, FrameType, encode_frame_header};
use zerodds_amqp_bridge::performatives::{decode_performative, descriptor, encode_performative};
use zerodds_amqp_bridge::types::TypeError;

use crate::link::LinkRole;
use crate::terminus::{
    AmqpError, DeliveryState, Source, Target, as_bool, as_text, as_u32, as_u64, decode_error,
    decode_state, encode_error, encode_state, field,
};
use crate::transactions::{Coordinator, TxnId, desc as txn_desc, flow_txn_id};

/// The lower bound of the agreed maximum frame size (Part 2 §2.8.19).
pub const MIN_MAX_FRAME_SIZE: u32 = 512;

/// The longest delivery tag (Part 2 §2.8.7).
pub const MAX_DELIVERY_TAG: usize = 32;

/// The error conditions of Part 2 §2.8.15 to §2.8.18.
pub mod condition {
    /// §2.8.15.
    pub const INTERNAL_ERROR: &str = "amqp:internal-error";
    /// §2.8.15.
    pub const NOT_FOUND: &str = "amqp:not-found";
    /// §2.8.15.
    pub const UNAUTHORIZED_ACCESS: &str = "amqp:unauthorized-access";
    /// §2.8.15.
    pub const DECODE_ERROR: &str = "amqp:decode-error";
    /// §2.8.15.
    pub const RESOURCE_LIMIT_EXCEEDED: &str = "amqp:resource-limit-exceeded";
    /// §2.8.15.
    pub const NOT_ALLOWED: &str = "amqp:not-allowed";
    /// §2.8.15.
    pub const INVALID_FIELD: &str = "amqp:invalid-field";
    /// §2.8.15.
    pub const NOT_IMPLEMENTED: &str = "amqp:not-implemented";
    /// §2.8.15.
    pub const RESOURCE_LOCKED: &str = "amqp:resource-locked";
    /// §2.8.15.
    pub const PRECONDITION_FAILED: &str = "amqp:precondition-failed";
    /// §2.8.15.
    pub const RESOURCE_DELETED: &str = "amqp:resource-deleted";
    /// §2.8.15.
    pub const ILLEGAL_STATE: &str = "amqp:illegal-state";
    /// §2.8.15.
    pub const FRAME_SIZE_TOO_SMALL: &str = "amqp:frame-size-too-small";
    /// §2.8.16.
    pub const CONNECTION_FORCED: &str = "amqp:connection:forced";
    /// §2.8.16.
    pub const FRAMING_ERROR: &str = "amqp:connection:framing-error";
    /// §2.8.16.
    pub const CONNECTION_REDIRECT: &str = "amqp:connection:redirect";
    /// §2.8.17.
    pub const WINDOW_VIOLATION: &str = "amqp:session:window-violation";
    /// §2.8.17.
    pub const ERRANT_LINK: &str = "amqp:session:errant-link";
    /// §2.8.17.
    pub const HANDLE_IN_USE: &str = "amqp:session:handle-in-use";
    /// §2.8.17.
    pub const UNATTACHED_HANDLE: &str = "amqp:session:unattached-handle";
    /// §2.8.18.
    pub const DETACH_FORCED: &str = "amqp:link:detach-forced";
    /// §2.8.18.
    pub const TRANSFER_LIMIT_EXCEEDED: &str = "amqp:link:transfer-limit-exceeded";
    /// §2.8.18.
    pub const MESSAGE_SIZE_EXCEEDED: &str = "amqp:link:message-size-exceeded";
    /// §2.8.18.
    pub const LINK_REDIRECT: &str = "amqp:link:redirect";
    /// §2.8.18.
    pub const STOLEN: &str = "amqp:link:stolen";
    /// Part 4 §4.5.8.
    pub const TRANSACTION_UNKNOWN_ID: &str = "amqp:transaction:unknown-id";
    /// Part 4 §4.5.8.
    pub const TRANSACTION_ROLLBACK: &str = "amqp:transaction:rollback";
    /// Part 4 §4.5.8.
    pub const TRANSACTION_TIMEOUT: &str = "amqp:transaction:timeout";

    /// Every error condition AMQP 1.0 defines: Part 2 §2.8.15 to §2.8.18
    /// and Part 4 §4.5.8.
    pub const DEFINED: &[&str] = &[
        INTERNAL_ERROR,
        NOT_FOUND,
        UNAUTHORIZED_ACCESS,
        DECODE_ERROR,
        RESOURCE_LIMIT_EXCEEDED,
        NOT_ALLOWED,
        INVALID_FIELD,
        NOT_IMPLEMENTED,
        RESOURCE_LOCKED,
        PRECONDITION_FAILED,
        RESOURCE_DELETED,
        ILLEGAL_STATE,
        FRAME_SIZE_TOO_SMALL,
        CONNECTION_FORCED,
        FRAMING_ERROR,
        CONNECTION_REDIRECT,
        WINDOW_VIOLATION,
        ERRANT_LINK,
        HANDLE_IN_USE,
        UNATTACHED_HANDLE,
        DETACH_FORCED,
        TRANSFER_LIMIT_EXCEEDED,
        MESSAGE_SIZE_EXCEEDED,
        LINK_REDIRECT,
        STOLEN,
        TRANSACTION_UNKNOWN_ID,
        TRANSACTION_ROLLBACK,
        TRANSACTION_TIMEOUT,
    ];
}

/// The limits this endpoint states in `open`, `begin` and `attach`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerConfig {
    /// `open.container-id`.
    pub container_id: String,
    /// `open.max-frame-size`: the largest frame this endpoint takes.
    pub max_frame_size: u32,
    /// `open.channel-max`: the highest channel number.
    pub channel_max: u16,
    /// The local idle time-out in milliseconds: the connection is closed
    /// when no frame arrives for that long. `open.idle-time-out` states
    /// half of it (§2.4.5). `None`: no time-out.
    pub idle_time_out_ms: Option<u32>,
    /// `begin.incoming-window`: transfer frames a session takes before it
    /// widens the window again.
    pub incoming_window: u32,
    /// `begin.outgoing-window`.
    pub outgoing_window: u32,
    /// `begin.handle-max`: the highest handle of a link.
    pub handle_max: u32,
    /// `attach.max-message-size`; 0 states no limit.
    pub max_message_size: u64,
    /// The link credit a receiving link grants, restored when half of it
    /// is used.
    pub link_credit: u32,
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            container_id: "zerodds".to_string(),
            max_frame_size: 1_048_576,
            channel_max: 255,
            idle_time_out_ms: None,
            incoming_window: 2048,
            outgoing_window: 2048,
            handle_max: 255,
            max_message_size: 64 * 1024 * 1024,
            link_credit: 100,
        }
    }
}

/// A link of the connection: the channel its session's frames arrive on
/// and the partner's handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkId {
    /// The incoming channel of the session.
    pub channel: u16,
    /// The partner's handle of the link.
    pub handle: u32,
}

/// What the container sees of a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkInfo {
    /// Channel and handle.
    pub id: LinkId,
    /// `attach.name`.
    pub name: String,
    /// The role of this endpoint on the link.
    pub role: LinkRole,
    /// `snd-settle-mode`: 0 unsettled, 1 settled, 2 mixed.
    pub snd_settle_mode: u8,
    /// `rcv-settle-mode`: 0 first, 1 second.
    pub rcv_settle_mode: u8,
    /// The source: the partner's on the attach, the created one after it.
    pub source: Option<Source>,
    /// The target, likewise. `None` also for a target that is no
    /// `amqp:target:list`; [`LinkInfo::target_value`] has it then.
    pub target: Option<Target>,
    /// The `target` field as the partner sent it, when it is a described
    /// value other than a target.
    pub target_value: Option<AmqpExtValue>,
    /// The target when it is a coordinator (Part 4 §4.5.1): the partner's
    /// on the attach, this endpoint's after it.
    pub coordinator: Option<Coordinator>,
    /// The partner's `unsettled` map (§2.6.13): delivery tag and state.
    /// `None` when the attach carried none, which is a new link.
    pub remote_unsettled: Option<Vec<(Vec<u8>, AmqpExtValue)>>,
    /// The partner's container (`open.container-id`).
    pub remote_container: String,
}

/// A complete delivery this endpoint received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// The delivery-id of the session.
    pub delivery_id: u32,
    /// The delivery tag.
    pub tag: Vec<u8>,
    /// `message-format`.
    pub message_format: u32,
    /// The sender settled it with the transfer.
    pub settled: bool,
    /// The message: the payload of all its transfer frames.
    pub payload: Vec<u8>,
    /// `transfer.state` of the first transfer frame, when the sender set
    /// one.
    pub state: Option<DeliveryState>,
    /// A later transfer frame of the delivery carried another `state` than
    /// the first, or none where the first had one, or one where the first
    /// had none (Part 4 §4.4.1: "all frames MUST be explicitly associated
    /// with the same transaction").
    pub state_differs: bool,
    /// The transactions a `transactional-state` of any transfer frame of
    /// the delivery named, the first frame's first, each once.
    pub txns: Vec<TxnId>,
}

/// What a container created for an attach.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Attached {
    /// The source as created.
    pub source: Option<Source>,
    /// The target as created.
    pub target: Option<Target>,
    /// For a link whose target is a coordinator: the coordinator of this
    /// container with the capabilities it has (Part 4 §4.5.1: "When sent
    /// by the resource ... defined the actual capabilities of the
    /// coordinator").
    pub coordinator: Option<Coordinator>,
}

impl From<(Option<Source>, Option<Target>)> for Attached {
    fn from((source, target): (Option<Source>, Option<Target>)) -> Self {
        Self {
            source,
            target,
            coordinator: None,
        }
    }
}

/// How a container answers a delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The state of the disposition for a delivery the sender did not
    /// settle; `None` sends a disposition only where the link settles
    /// with it (receiver settle mode first), without a state.
    State(Option<DeliveryState>),
    /// No disposition now: the container answers with [`Peer::answer`].
    /// Until then the delivery is unsettled.
    Later,
}

impl Answer {
    /// The answer with the state of an outcome.
    #[must_use]
    pub fn outcome(outcome: crate::terminus::Outcome) -> Self {
        Self::State(DeliveryState::of(outcome))
    }
}

/// What links attach to.
pub trait Container {
    /// The partner attaches `link`. `Ok`: source and target as created;
    /// they are the ones of the answering attach (§2.6.3: "the endpoint
    /// MUST report the actual properties of the terminus as created").
    /// `Err`: no terminus; the attach is answered with a null terminus
    /// and a closing detach with the error.
    ///
    /// # Errors
    /// The error of the detach.
    fn attach(&mut self, link: &LinkInfo) -> Result<Attached, AmqpError>;

    /// A delivery arrived on a link this endpoint receives on.
    ///
    /// # Errors
    /// The link is destroyed with the error (§2.6.5) and no disposition is
    /// sent for the delivery.
    fn deliver(&mut self, link: &LinkInfo, delivery: Delivery) -> Result<Answer, AmqpError>;

    /// The partner's disposition for a delivery this endpoint sent. A
    /// terminal outcome the partner did not settle is settled by the
    /// endpoint after this call; a `transactional-state` (Part 4 §4.5.6)
    /// is not: the delivery stays unsettled until [`Peer::settle`].
    ///
    /// # Errors
    /// The link is destroyed with the error.
    fn disposition(
        &mut self,
        link: &LinkInfo,
        tag: &[u8],
        state: Option<&DeliveryState>,
        settled: bool,
    ) -> Result<(), AmqpError>;

    /// A flow for a link this endpoint sends on carried a properties map
    /// (Part 4 §4.4.3): `txn` is its `txn-id` entry, the transaction the
    /// messages the link acquires from now on belong to; `None` for a map
    /// without the entry, which ends that association.
    ///
    /// # Errors
    /// The link is destroyed with the error (§4.4.3: "If the link endpoint
    /// does not support transactional acquisition, the link MUST be
    /// terminated with a not-implemented error").
    fn acquire_in(&mut self, link: &LinkInfo, txn: Option<&TxnId>) -> Result<(), AmqpError>;

    /// The first transfer frame of a delivery of more than one arrived
    /// (§2.6.14), with this `transfer.state`. [`Container::deliver`]
    /// follows with the last frame, [`Container::delivery_aborted`] if the
    /// sender aborts it, [`Container::detached`] if the link goes first.
    fn delivery_begun(&mut self, link: &LinkInfo, delivery_id: u32, state: Option<&DeliveryState>) {
        let _ = (link, delivery_id, state);
    }

    /// A later transfer frame of the delivery [`Container::delivery_begun`]
    /// announced named `txn` in a `transactional-state`, and no frame
    /// before it did (Part 4 §4.4.1: a partial delivery posted against
    /// that transaction). The delivery ends as announced there.
    fn delivery_named(&mut self, link: &LinkInfo, delivery_id: u32, txn: &TxnId) {
        let _ = (link, delivery_id, txn);
    }

    /// The sender aborted the delivery [`Container::delivery_begun`]
    /// announced (§2.6.14).
    fn delivery_aborted(&mut self, link: &LinkInfo, delivery_id: u32) {
        let _ = (link, delivery_id);
    }

    /// The messages that wait for credit at the source of a link this
    /// endpoint sends on: `flow.available` (§2.7.4, "the number of
    /// messages awaiting credit at the link sender endpoint"), stated in
    /// every flow this endpoint sends for the link.
    fn available(&mut self, link: &LinkInfo) -> u32;

    /// The link is gone: detached by the partner, destroyed with `error`
    /// by this endpoint, or ended with its session or connection.
    /// `unsettled` are the tags of the deliveries this endpoint sent on it
    /// that the partner had not settled.
    fn detached(&mut self, link: &LinkInfo, unsettled: Vec<Vec<u8>>, error: Option<&AmqpError>);
}

/// Why [`Peer::send`] did not send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// No such link, or this endpoint is not its sender, or the
    /// connection or session is ending.
    NoLink,
    /// The link has no credit (§2.6.7: "a sender MUST NOT send more
    /// messages").
    NoCredit,
    /// The partner's incoming window is zero (§2.5.6): no frame of the
    /// message can be sent now.
    Window,
    /// The link is in the middle of a delivery whose remaining frames wait
    /// for the partner's incoming window (§2.6.14: the deliveries of one
    /// link are not interleaved).
    InProgress,
    /// The delivery tag is longer than 32 octets (§2.8.7).
    Tag,
    /// The message is larger than the partner's `max-message-size`.
    MessageTooLarge,
    /// The agreed frame size has no room for the transfer and one octet
    /// of the message (§2.8.15 `frame-size-too-small`).
    FrameSizeTooSmall,
}

/// The six variables of session flow control (Part 2 §2.5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionWindows {
    /// The transfer-id expected of the next incoming transfer frame.
    pub next_incoming_id: u32,
    /// Transfer frames this endpoint takes now.
    pub incoming_window: u32,
    /// The transfer-id of the next transfer frame this endpoint sends.
    pub next_outgoing_id: u32,
    /// Transfer frames this endpoint states it can send.
    pub outgoing_window: u32,
    /// Transfer frames the partner takes now.
    pub remote_incoming_window: u32,
    /// Transfer frames the partner stated it can send.
    pub remote_outgoing_window: u32,
}

/// The flow-control state of a link (Part 2 §2.6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkFlow {
    /// The sender's delivery-count as this endpoint knows it.
    pub delivery_count: u32,
    /// The credit of the link.
    pub link_credit: u32,
    /// The receiver asked the sender to drain.
    pub drain: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnState {
    /// Headers exchanged, no `open` from the partner yet.
    AwaitOpen,
    /// Both `open` frames exchanged.
    Opened,
    /// This endpoint sent `close`; frames are discarded until the
    /// partner's.
    Discarding,
    /// Both `close` frames exchanged, or the transport is gone.
    End,
}

struct Partial {
    delivery_id: u32,
    tag: Vec<u8>,
    message_format: u32,
    settled: bool,
    rcv_settle_mode: Option<u8>,
    state: Option<DeliveryState>,
    /// `transfer.state` of the first frame as it arrived.
    state_field: Option<AmqpExtValue>,
    state_differs: bool,
    /// The transactions its frames named.
    txns: Vec<TxnId>,
    /// [`Container::delivery_begun`] was called for it.
    announced: bool,
    resume: bool,
    payload: Vec<u8>,
}

/// A delivery this endpoint is in the middle of sending (§2.6.14).
struct Sending {
    delivery_id: u32,
    tag: Vec<u8>,
    message_format: u32,
    settled: bool,
    /// `transfer.state`, on every frame of the delivery.
    state: Option<AmqpExtValue>,
    payload: Vec<u8>,
    /// Octets of `payload` already sent.
    sent: usize,
    /// The next frame of the delivery is the abort ([`Peer::abort`]).
    aborting: bool,
    /// Octets of the payload one transfer frame carries.
    chunk: usize,
}

struct Link {
    info: LinkInfo,
    local_handle: u32,
    /// The sender's delivery-count as this endpoint knows it.
    delivery_count: u32,
    /// The delivery-count of this endpoint's own attach (sender role).
    initial_delivery_count: u32,
    link_credit: u32,
    drain: bool,
    /// The partner's `max-message-size`; 0 is no limit.
    remote_max_message_size: u64,
    partial: Option<Partial>,
    sending: Option<Sending>,
}

struct Session {
    outgoing_channel: u16,
    next_incoming_id: u32,
    incoming_window: u32,
    next_outgoing_id: u32,
    initial_outgoing_id: u32,
    remote_incoming_window: u32,
    remote_outgoing_window: u32,
    remote_handle_max: u32,
    links: BTreeMap<u32, Link>,
    local_handles: BTreeSet<u32>,
    /// Partner handles of links this endpoint destroyed with an error
    /// and whose detach the partner has not answered (§2.6.5).
    errant: BTreeSet<u32>,
    /// The delivery-ids such a link held when it was destroyed, with its
    /// handle: the ones the partner assigned (deliveries received and not
    /// settled, and one in the middle of its transfers) ...
    errant_received: BTreeMap<u32, u32>,
    /// ... and the ones this endpoint assigned (deliveries sent and not
    /// settled, and one in the middle of its transfers). A disposition
    /// that names one of them is input for the destroyed link (§2.6.5).
    errant_sent: BTreeMap<u32, u32>,
    /// The session was ended with an error; its frames are discarded
    /// until the partner's `end` (§2.5.4).
    discarding: bool,
    next_delivery_id: u32,
    /// Deliveries received and not settled: delivery-id to handle and tag.
    incoming_unsettled: BTreeMap<u32, (u32, Vec<u8>)>,
    /// Deliveries the container answers later ([`Answer::Later`]), with
    /// whether the answer settles them (receiver settle mode first).
    awaiting_answer: BTreeMap<u32, bool>,
    /// Deliveries sent and not settled.
    outgoing_unsettled: BTreeMap<u32, (u32, Vec<u8>)>,
}

/// The connection endpoint.
pub struct Peer {
    cfg: PeerConfig,
    state: ConnState,
    open_sent: bool,
    remote_max_frame: u32,
    channel_max: u16,
    remote_idle_ms: Option<u32>,
    remote_container: String,
    /// No `open` of the partner was accepted yet.
    remote_container_unknown: bool,
    sessions: BTreeMap<u16, Session>,
    outgoing_channels: BTreeSet<u16>,
    out: VecDeque<Vec<u8>>,
    last_received_ms: u64,
    last_sent_ms: u64,
    now_ms: u64,
    remote_close: Option<Option<AmqpError>>,
    local_close: Option<AmqpError>,
    fault: Option<String>,
}

/// The frames a handler decided to send, written once the borrow of the
/// session it read from has ended.
type Pending = Vec<(u16, u64, Vec<AmqpExtValue>, Vec<u8>)>;

fn serial_le(a: u32, b: u32) -> bool {
    // RFC 1982: a <= b when b - a is in the lower half of the number space.
    b.wrapping_sub(a) < 0x8000_0000
}

fn opt<T>(v: Option<T>, f: impl FnOnce(T) -> AmqpExtValue) -> AmqpExtValue {
    v.map_or(AmqpExtValue::Null, f)
}

impl Peer {
    /// An endpoint whose protocol headers were exchanged at `now_ms`.
    #[must_use]
    pub fn new(cfg: PeerConfig, now_ms: u64) -> Self {
        Self {
            cfg,
            state: ConnState::AwaitOpen,
            open_sent: false,
            remote_max_frame: MIN_MAX_FRAME_SIZE,
            channel_max: 0,
            remote_idle_ms: None,
            remote_container: String::new(),
            remote_container_unknown: true,
            sessions: BTreeMap::new(),
            outgoing_channels: BTreeSet::new(),
            out: VecDeque::new(),
            last_received_ms: now_ms,
            last_sent_ms: now_ms,
            now_ms,
            remote_close: None,
            local_close: None,
            fault: None,
        }
    }

    // ------------------------------------------------------------------
    // Output.
    // ------------------------------------------------------------------

    /// The next frame to write, header and body.
    pub fn poll_output(&mut self) -> Option<Vec<u8>> {
        self.out.pop_front()
    }

    fn push_frame(&mut self, channel: u16, body: &[u8]) {
        let Ok(size) = u32::try_from(8 + body.len()) else {
            self.fault = Some("a frame larger than 4 GiB".to_string());
            self.state = ConnState::End;
            return;
        };
        let header = encode_frame_header(FrameHeader {
            size,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel,
        });
        let mut frame = Vec::with_capacity(8 + body.len());
        frame.extend_from_slice(&header);
        frame.extend_from_slice(body);
        self.out.push_back(frame);
        self.last_sent_ms = self.now_ms;
    }

    fn emit(&mut self, channel: u16, code: u64, list: Vec<AmqpExtValue>, payload: &[u8]) {
        match encode_performative(code, &AmqpExtValue::List(list)) {
            Ok(mut body) => {
                body.extend_from_slice(payload);
                self.push_frame(channel, &body);
            }
            Err(e) => {
                // Only a value that has no encoding gets here (a string
                // or list beyond 4 GiB). Nothing more can be said to the
                // partner; the driver drops the transport.
                self.fault = Some(format!("frame body {code:#x} has no encoding: {e}"));
                self.state = ConnState::End;
            }
        }
    }

    fn emit_all(&mut self, pending: Pending) {
        for (channel, code, list, payload) in pending {
            self.emit(channel, code, list, &payload);
        }
    }

    // ------------------------------------------------------------------
    // State the driver reads.
    // ------------------------------------------------------------------

    /// Both `close` frames are exchanged, or the endpoint gave up: the
    /// transport can be closed once the output is written.
    #[must_use]
    pub fn is_ended(&self) -> bool {
        self.state == ConnState::End
    }

    /// The partner's `open` arrived.
    #[must_use]
    pub fn open_received(&self) -> bool {
        self.open_sent && !self.remote_container_unknown
    }

    /// This endpoint sent `close` and waits for the partner's.
    #[must_use]
    pub fn is_closing(&self) -> bool {
        self.state == ConnState::Discarding
    }

    /// The error of the partner's `close`: `None` before it arrived,
    /// `Some(None)` for a close without an error.
    #[must_use]
    pub fn remote_close(&self) -> Option<&Option<AmqpError>> {
        self.remote_close.as_ref()
    }

    /// The error this endpoint closed the connection with.
    #[must_use]
    pub fn local_close(&self) -> Option<&AmqpError> {
        self.local_close.as_ref()
    }

    /// A value this endpoint could not encode; the transport is to be
    /// dropped.
    #[must_use]
    pub fn fault(&self) -> Option<&str> {
        self.fault.as_deref()
    }

    /// The partner's `open.container-id`.
    #[must_use]
    pub fn remote_container(&self) -> &str {
        &self.remote_container
    }

    /// The frame size agreed for frames this endpoint sends.
    #[must_use]
    pub fn remote_max_frame_size(&self) -> u32 {
        self.remote_max_frame
    }

    /// The link `id` names.
    #[must_use]
    pub fn link(&self, id: LinkId) -> Option<&LinkInfo> {
        self.sessions
            .get(&id.channel)
            .and_then(|s| s.links.get(&id.handle))
            .map(|l| &l.info)
    }

    /// The flow-control state of the session on `channel` (§2.5.6).
    #[must_use]
    pub fn session_windows(&self, channel: u16) -> Option<SessionWindows> {
        self.sessions.get(&channel).map(|s| SessionWindows {
            next_incoming_id: s.next_incoming_id,
            incoming_window: s.incoming_window,
            next_outgoing_id: s.next_outgoing_id,
            outgoing_window: self.cfg.outgoing_window,
            remote_incoming_window: s.remote_incoming_window,
            remote_outgoing_window: s.remote_outgoing_window,
        })
    }

    /// The flow-control state of the link `id` (§2.6.7).
    #[must_use]
    pub fn link_flow(&self, id: LinkId) -> Option<LinkFlow> {
        self.sessions
            .get(&id.channel)
            .and_then(|s| s.links.get(&id.handle))
            .map(|l| LinkFlow {
                delivery_count: l.delivery_count,
                link_credit: l.link_credit,
                drain: l.drain,
            })
    }

    /// The links this endpoint can send on now: each with its credit and
    /// its drain flag.
    #[must_use]
    pub fn sendable(&self) -> Vec<(LinkId, u32, bool)> {
        if self.state != ConnState::Opened {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (channel, session) in &self.sessions {
            if session.discarding || session.remote_incoming_window == 0 {
                continue;
            }
            for (handle, link) in &session.links {
                if link.info.role == LinkRole::Sender
                    && link.link_credit > 0
                    && link.sending.is_none()
                {
                    out.push((
                        LinkId {
                            channel: *channel,
                            handle: *handle,
                        },
                        link.link_credit,
                        link.drain,
                    ));
                }
            }
        }
        out
    }

    // ------------------------------------------------------------------
    // Time.
    // ------------------------------------------------------------------

    /// Advances the clock (§2.4.5): closes the connection when the
    /// partner was silent for the local idle time-out, and sends an empty
    /// frame when this endpoint was silent for half the partner's.
    pub fn tick<C: Container + ?Sized>(&mut self, now_ms: u64, app: &mut C) {
        self.now_ms = now_ms;
        if !matches!(self.state, ConnState::AwaitOpen | ConnState::Opened) {
            return;
        }
        if let Some(limit) = self.cfg.idle_time_out_ms
            && now_ms.saturating_sub(self.last_received_ms) >= u64::from(limit)
        {
            self.fail_connection(
                condition::RESOURCE_LIMIT_EXCEEDED,
                format!("no frame for {limit} ms (idle time-out)"),
                app,
            );
            return;
        }
        if self.state == ConnState::Opened
            && let Some(remote) = self.remote_idle_ms.filter(|r| *r > 0)
            && now_ms.saturating_sub(self.last_sent_ms) >= u64::from(remote / 2)
        {
            self.push_frame(0, &[]);
        }
    }

    // ------------------------------------------------------------------
    // Errors.
    // ------------------------------------------------------------------

    fn open_fields(&self) -> Vec<AmqpExtValue> {
        vec![
            AmqpExtValue::Str(self.cfg.container_id.clone()),
            AmqpExtValue::Null,
            AmqpExtValue::Uint(self.cfg.max_frame_size.max(MIN_MAX_FRAME_SIZE)),
            AmqpExtValue::Ushort(self.cfg.channel_max),
            // §2.4.5: "the value in idle-time-out SHOULD be half the
            // peer's actual timeout threshold".
            opt(self.cfg.idle_time_out_ms, |t| AmqpExtValue::Uint(t / 2)),
        ]
    }

    /// Closes the connection with an error (§2.4.3) and discards what
    /// arrives until the partner's close (§2.4.6 DISCARDING). The links
    /// are gone with it.
    pub fn fail_connection<C: Container + ?Sized>(
        &mut self,
        condition: &str,
        description: String,
        app: &mut C,
    ) {
        self.close(Some(AmqpError::new(condition, description)), app);
    }

    /// Closes the connection (§2.4.3): `close`, with `error` if any, is
    /// the last frame this endpoint writes.
    pub fn close<C: Container + ?Sized>(&mut self, error: Option<AmqpError>, app: &mut C) {
        if matches!(self.state, ConnState::Discarding | ConnState::End) {
            return;
        }
        if !self.open_sent {
            // §2.4.1: open is the first frame; §2.4.6: HDR_EXCH admits
            // no other.
            let open = self.open_fields();
            self.emit(0, descriptor::OPEN, open, &[]);
            self.open_sent = true;
        }
        let list = match &error {
            Some(e) => vec![encode_error(e)],
            None => Vec::new(),
        };
        self.emit(0, descriptor::CLOSE, list, &[]);
        self.teardown_all(error.as_ref(), app);
        self.local_close = error;
        if self.state != ConnState::End {
            self.state = ConnState::Discarding;
        }
    }

    /// The transport ended without a close: every link is gone.
    pub fn connection_lost<C: Container + ?Sized>(&mut self, app: &mut C) {
        self.teardown_all(None, app);
        self.state = ConnState::End;
    }

    fn teardown_all<C: Container + ?Sized>(&mut self, error: Option<&AmqpError>, app: &mut C) {
        let sessions = core::mem::take(&mut self.sessions);
        self.outgoing_channels.clear();
        for (_, mut session) in sessions {
            Self::teardown_session(&mut session, error, app);
        }
    }

    fn teardown_session<C: Container + ?Sized>(
        session: &mut Session,
        error: Option<&AmqpError>,
        app: &mut C,
    ) {
        let links = core::mem::take(&mut session.links);
        for (handle, link) in links {
            let unsettled = Self::forget_link(session, handle);
            app.detached(&link.info, unsettled, error);
        }
        session.local_handles.clear();
    }

    /// Drops the unsettled state of the link with the partner's handle
    /// `handle`; returns the tags of what this endpoint sent on it.
    fn forget_link(session: &mut Session, handle: u32) -> Vec<Vec<u8>> {
        let Session {
            incoming_unsettled,
            awaiting_answer,
            ..
        } = session;
        incoming_unsettled.retain(|delivery, (h, _)| {
            if *h == handle {
                awaiting_answer.remove(delivery);
                false
            } else {
                true
            }
        });
        let mut tags = Vec::new();
        session.outgoing_unsettled.retain(|_, (h, tag)| {
            if *h == handle {
                tags.push(core::mem::take(tag));
                false
            } else {
                true
            }
        });
        tags
    }

    /// Ends the session on `channel` with an error (§2.5.4): `end` with
    /// the error, then every frame of the session is discarded until the
    /// partner's `end`.
    fn fail_session<C: Container + ?Sized>(
        &mut self,
        channel: u16,
        condition: &str,
        description: String,
        app: &mut C,
    ) {
        let error = AmqpError::new(condition, description);
        let Some(session) = self.sessions.get_mut(&channel) else {
            return;
        };
        if session.discarding {
            return;
        }
        session.discarding = true;
        let outgoing = session.outgoing_channel;
        Self::teardown_session(session, Some(&error), app);
        // §2.5.5 END_SENT: no outgoing channel any more.
        self.outgoing_channels.remove(&outgoing);
        self.emit(outgoing, descriptor::END, vec![encode_error(&error)], &[]);
    }

    /// Destroys the link with an error (§2.6.5): a closing detach with
    /// the error; input for the link other than a detach then ends the
    /// session.
    fn fail_link<C: Container + ?Sized>(
        &mut self,
        id: LinkId,
        condition: &str,
        description: String,
        app: &mut C,
    ) {
        self.detach(id, Some(AmqpError::new(condition, description)), app);
    }

    /// The partner answered the detach of the link on `handle`, or resumes
    /// it: nothing of it is errant any more.
    fn forgive(session: &mut Session, handle: u32) -> bool {
        session.errant_received.retain(|_, h| *h != handle);
        session.errant_sent.retain(|_, h| *h != handle);
        session.errant.remove(&handle)
    }

    /// Detaches the link from this side and closes it (§2.6.6), with
    /// `error` if the cause is one (§2.6.5).
    pub fn detach<C: Container + ?Sized>(
        &mut self,
        id: LinkId,
        error: Option<AmqpError>,
        app: &mut C,
    ) {
        let Some(session) = self.sessions.get_mut(&id.channel) else {
            return;
        };
        let Some(link) = session.links.remove(&id.handle) else {
            return;
        };
        session.local_handles.remove(&link.local_handle);
        session.errant.insert(id.handle);
        if error.is_some() {
            // §2.6.5: input "related to the endpoint either via the input
            // handle or delivery-ids" ends the session. The delivery-ids a
            // disposition can still name are the ones not settled.
            let received = session
                .incoming_unsettled
                .iter()
                .filter(|(_, (h, _))| *h == id.handle)
                .map(|(delivery, _)| *delivery)
                .chain(link.partial.as_ref().map(|p| p.delivery_id));
            session
                .errant_received
                .extend(received.map(|delivery| (delivery, id.handle)));
            let sent = session
                .outgoing_unsettled
                .iter()
                .filter(|(_, (h, _))| *h == id.handle)
                .map(|(delivery, _)| *delivery)
                .chain(link.sending.as_ref().map(|s| s.delivery_id));
            session
                .errant_sent
                .extend(sent.map(|delivery| (delivery, id.handle)));
        }
        let unsettled = Self::forget_link(session, id.handle);
        let outgoing = session.outgoing_channel;
        let list = vec![
            AmqpExtValue::Uint(link.local_handle),
            AmqpExtValue::Boolean(true),
            opt(error.as_ref(), encode_error),
        ];
        self.emit(outgoing, descriptor::DETACH, list, &[]);
        app.detached(&link.info, unsettled, error.as_ref());
    }

    // ------------------------------------------------------------------
    // Input.
    // ------------------------------------------------------------------

    /// One AMQP frame of the partner: its channel and its body (empty for
    /// the empty frame of §2.4.5).
    pub fn frame<C: Container + ?Sized>(
        &mut self,
        channel: u16,
        body: &[u8],
        now_ms: u64,
        app: &mut C,
    ) {
        self.now_ms = now_ms;
        self.last_received_ms = now_ms;
        if self.state == ConnState::End || body.is_empty() {
            return;
        }
        let (code, value, used) = match decode_performative(body) {
            Ok(v) => v,
            Err(e) => {
                if self.state == ConnState::Discarding {
                    return;
                }
                let (condition, text) = match &e {
                    TypeError::UnknownDescriptor(symbol) => (
                        condition::NOT_IMPLEMENTED,
                        format!("frame body {symbol} is not one of AMQP 1.0"),
                    ),
                    other => (condition::DECODE_ERROR, format!("frame body: {other}")),
                };
                self.fail_connection(condition, text, app);
                return;
            }
        };
        if self.state == ConnState::Discarding {
            // §2.4.6: "any incoming frames on the connection MUST be
            // silently discarded until the peer's close frame is
            // received".
            if code == descriptor::CLOSE {
                self.remote_close = Some(decode_error(field(&value, 0)));
                self.state = ConnState::End;
            }
            return;
        }
        if !matches!(value, AmqpExtValue::List(_)) {
            self.fail_connection(
                condition::DECODE_ERROR,
                format!("frame body {code:#x} is not a list"),
                app,
            );
            return;
        }
        let payload = &body[used..];
        if self.state == ConnState::AwaitOpen {
            if code == descriptor::OPEN {
                self.on_open(channel, &value, app);
            } else {
                // §2.4.1: "each peer MUST send an open frame before
                // sending any other frames".
                self.fail_connection(
                    condition::ILLEGAL_STATE,
                    format!("frame body {code:#x} before open"),
                    app,
                );
            }
            return;
        }
        match code {
            descriptor::OPEN => {
                self.fail_connection(condition::ILLEGAL_STATE, "a second open".to_string(), app)
            }
            descriptor::CLOSE => {
                self.remote_close = Some(decode_error(field(&value, 0)));
                self.emit(0, descriptor::CLOSE, Vec::new(), &[]);
                let error = self.remote_close.clone().flatten();
                self.teardown_all(error.as_ref(), app);
                self.state = ConnState::End;
            }
            descriptor::BEGIN => self.on_begin(channel, &value, app),
            descriptor::END
            | descriptor::ATTACH
            | descriptor::FLOW
            | descriptor::TRANSFER
            | descriptor::DISPOSITION
            | descriptor::DETACH => {
                if channel > self.channel_max {
                    self.fail_connection(
                        condition::FRAMING_ERROR,
                        format!("channel {channel} above channel-max {}", self.channel_max),
                        app,
                    );
                    return;
                }
                let Some(session) = self.sessions.get(&channel) else {
                    self.fail_connection(
                        condition::NOT_ALLOWED,
                        format!("frame body {code:#x} on channel {channel}, which has no session"),
                        app,
                    );
                    return;
                };
                if session.discarding {
                    // §2.5.4: discard until the partner's end.
                    if code == descriptor::END {
                        self.sessions.remove(&channel);
                    }
                    return;
                }
                match code {
                    descriptor::END => self.on_end(channel, &value, app),
                    descriptor::ATTACH => self.on_attach(channel, &value, app),
                    descriptor::FLOW => {
                        self.on_flow(channel, &value, app);
                        // §2.5.6: the flow may have widened the partner's
                        // incoming window.
                        self.resume_transfers(channel);
                    }
                    descriptor::TRANSFER => self.on_transfer(channel, &value, payload, app),
                    descriptor::DISPOSITION => self.on_disposition(channel, &value, app),
                    _ => self.on_detach(channel, &value, app),
                }
            }
            other => self.fail_connection(
                condition::NOT_IMPLEMENTED,
                format!("frame body {other:#x} is no performative of an AMQP frame"),
                app,
            ),
        }
    }

    fn on_open<C: Container + ?Sized>(&mut self, channel: u16, value: &AmqpExtValue, app: &mut C) {
        if channel != 0 {
            self.fail_connection(
                condition::NOT_ALLOWED,
                format!("open on channel {channel}"),
                app,
            );
            return;
        }
        let Some(container) = as_text(field(value, 0)) else {
            self.fail_connection(
                condition::INVALID_FIELD,
                "open without container-id".to_string(),
                app,
            );
            return;
        };
        let max_frame = as_u32(field(value, 2)).unwrap_or(u32::MAX);
        if max_frame < MIN_MAX_FRAME_SIZE {
            self.fail_connection(
                condition::INVALID_FIELD,
                format!("open.max-frame-size {max_frame} is below {MIN_MAX_FRAME_SIZE}"),
                app,
            );
            return;
        }
        let remote_channel_max = as_u32(field(value, 3))
            .and_then(|c| u16::try_from(c).ok())
            .unwrap_or(u16::MAX);
        self.remote_container = container;
        self.remote_container_unknown = false;
        self.remote_max_frame = max_frame;
        self.channel_max = remote_channel_max.min(self.cfg.channel_max);
        self.remote_idle_ms = as_u32(field(value, 4));
        let open = self.open_fields();
        self.emit(0, descriptor::OPEN, open, &[]);
        self.open_sent = true;
        self.state = ConnState::Opened;
    }

    fn on_begin<C: Container + ?Sized>(&mut self, channel: u16, value: &AmqpExtValue, app: &mut C) {
        if channel > self.channel_max {
            // §2.7.1 channel-max: "A peer that receives a channel number
            // outside the supported range MUST close the connection with
            // the framing-error error-code."
            self.fail_connection(
                condition::FRAMING_ERROR,
                format!("channel {channel} above channel-max {}", self.channel_max),
                app,
            );
            return;
        }
        if self.sessions.contains_key(&channel) {
            self.fail_connection(
                condition::ILLEGAL_STATE,
                format!("begin on channel {channel}, which has a session"),
                app,
            );
            return;
        }
        if field(value, 0).is_some() {
            // §2.5.1: remote-channel answers a begin of this endpoint,
            // and this endpoint begins none.
            self.fail_connection(
                condition::NOT_ALLOWED,
                "begin with remote-channel: this endpoint began no session".to_string(),
                app,
            );
            return;
        }
        let (Some(next_outgoing_id), Some(incoming_window), Some(outgoing_window)) = (
            as_u32(field(value, 1)),
            as_u32(field(value, 2)),
            as_u32(field(value, 3)),
        ) else {
            self.fail_connection(
                condition::INVALID_FIELD,
                "begin without next-outgoing-id, incoming-window or outgoing-window".to_string(),
                app,
            );
            return;
        };
        // §2.5.1: "it is RECOMMENDED that implementations always assign
        // the lowest available unused channel number".
        let Some(outgoing) = (0..=self.channel_max).find(|c| !self.outgoing_channels.contains(c))
        else {
            self.fail_connection(
                condition::RESOURCE_LIMIT_EXCEEDED,
                format!("no free channel up to channel-max {}", self.channel_max),
                app,
            );
            return;
        };
        self.outgoing_channels.insert(outgoing);
        self.sessions.insert(
            channel,
            Session {
                outgoing_channel: outgoing,
                next_incoming_id: next_outgoing_id,
                incoming_window: self.cfg.incoming_window,
                next_outgoing_id: 0,
                initial_outgoing_id: 0,
                remote_incoming_window: incoming_window,
                remote_outgoing_window: outgoing_window,
                remote_handle_max: as_u32(field(value, 4)).unwrap_or(u32::MAX),
                links: BTreeMap::new(),
                local_handles: BTreeSet::new(),
                errant: BTreeSet::new(),
                errant_received: BTreeMap::new(),
                errant_sent: BTreeMap::new(),
                discarding: false,
                next_delivery_id: 0,
                incoming_unsettled: BTreeMap::new(),
                awaiting_answer: BTreeMap::new(),
                outgoing_unsettled: BTreeMap::new(),
            },
        );
        let list = vec![
            AmqpExtValue::Ushort(channel),
            AmqpExtValue::Uint(0),
            AmqpExtValue::Uint(self.cfg.incoming_window),
            AmqpExtValue::Uint(self.cfg.outgoing_window),
            AmqpExtValue::Uint(self.cfg.handle_max),
        ];
        self.emit(outgoing, descriptor::BEGIN, list, &[]);
    }

    fn on_end<C: Container + ?Sized>(&mut self, channel: u16, value: &AmqpExtValue, app: &mut C) {
        let Some(mut session) = self.sessions.remove(&channel) else {
            return;
        };
        let error = decode_error(field(value, 0));
        Self::teardown_session(&mut session, error.as_ref(), app);
        self.outgoing_channels.remove(&session.outgoing_channel);
        self.emit(session.outgoing_channel, descriptor::END, Vec::new(), &[]);
    }

    /// The flow frame of the session on `channel`, for the link with the
    /// partner's handle `handle` if given (§2.7.4). `available` is the
    /// container's count for a link this endpoint sends on.
    fn flow_fields(
        session: &Session,
        outgoing_window: u32,
        handle: Option<u32>,
        available: u32,
    ) -> Vec<AmqpExtValue> {
        let mut list = vec![
            AmqpExtValue::Uint(session.next_incoming_id),
            AmqpExtValue::Uint(session.incoming_window),
            AmqpExtValue::Uint(session.next_outgoing_id),
            AmqpExtValue::Uint(outgoing_window),
        ];
        if let Some(link) = handle.and_then(|h| session.links.get(&h)) {
            list.push(AmqpExtValue::Uint(link.local_handle));
            list.push(AmqpExtValue::Uint(link.delivery_count));
            list.push(AmqpExtValue::Uint(link.link_credit));
            // §2.7.4 available: "Only the sender can independently set
            // this value."
            list.push(if link.info.role == LinkRole::Sender {
                AmqpExtValue::Uint(available)
            } else {
                AmqpExtValue::Null
            });
            list.push(AmqpExtValue::Boolean(link.drain));
        }
        list
    }

    fn on_attach<C: Container + ?Sized>(
        &mut self,
        channel: u16,
        value: &AmqpExtValue,
        app: &mut C,
    ) {
        let (Some(name), Some(handle), Some(remote_is_receiver)) = (
            as_text(field(value, 0)),
            as_u32(field(value, 1)),
            as_bool(field(value, 2)),
        ) else {
            self.fail_connection(
                condition::INVALID_FIELD,
                "attach without name, handle or role".to_string(),
                app,
            );
            return;
        };
        if handle > self.cfg.handle_max {
            // §2.7.2 handle-max: "A peer that receives a handle outside
            // the supported range MUST close the connection with the
            // framing-error error-code."
            self.fail_connection(
                condition::FRAMING_ERROR,
                format!("handle {handle} above handle-max {}", self.cfg.handle_max),
                app,
            );
            return;
        }
        let unsettled_field = field(value, 7);
        let remote_unsettled = match unsettled_field {
            None => None,
            Some(AmqpExtValue::Map(pairs)) => Some(
                pairs
                    .iter()
                    .filter_map(|(k, v)| match k {
                        AmqpExtValue::Binary(tag) => Some((tag.clone(), v.clone())),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            Some(other) => {
                self.fail_connection(
                    condition::INVALID_FIELD,
                    format!("attach.unsettled {other:?} is not a map"),
                    app,
                );
                return;
            }
        };
        let Some(session) = self.sessions.get_mut(&channel) else {
            return;
        };
        if session.errant.contains(&handle) {
            if remote_unsettled.is_none() {
                // §2.6.5: "Receipt of a pipelined attach MUST result in
                // the session being terminated with an errant-link
                // session-error."
                self.fail_session(
                    channel,
                    condition::ERRANT_LINK,
                    format!("attach on handle {handle} of a link detached with an error"),
                    app,
                );
                return;
            }
            // A resuming attach (non-null unsettled) restores the link.
            Self::forgive(session, handle);
        }
        if session.links.contains_key(&handle) {
            self.fail_session(
                channel,
                condition::HANDLE_IN_USE,
                format!("attach on handle {handle}, which is attached"),
                app,
            );
            return;
        }
        let role = if remote_is_receiver {
            LinkRole::Sender
        } else {
            LinkRole::Receiver
        };
        let Some(local_handle) = (0..=session.remote_handle_max)
            .take(4096)
            .find(|h| !session.local_handles.contains(h))
        else {
            self.fail_session(
                channel,
                condition::RESOURCE_LIMIT_EXCEEDED,
                "no free handle for the link".to_string(),
                app,
            );
            return;
        };
        let outgoing = session.outgoing_channel;
        let snd_settle_mode = as_u32(field(value, 3)).unwrap_or(2);
        let rcv_settle_mode = as_u32(field(value, 4)).unwrap_or(0);
        let source = Source::decode(field(value, 5));
        let target_field = field(value, 6);
        let mut coordinator = None;
        let (target, target_value) = match Target::decode(target_field) {
            Ok(t) => (Ok(t), None),
            Err(e) => match target_field {
                // Part 4 §4.5.1: a coordinator "provides" target.
                Some(AmqpExtValue::Described { descriptor, .. })
                    if *descriptor == txn_desc::COORDINATOR =>
                {
                    match Coordinator::decode(target_field) {
                        Ok(c) => {
                            coordinator = c;
                            (Ok(None), None)
                        }
                        Err(malformed) => (Err(malformed), None),
                    }
                }
                // A described value of another type is the container's to
                // judge. A value under the descriptor of a target that is
                // not one is a malformed target.
                Some(AmqpExtValue::Described { descriptor, .. })
                    if *descriptor != crate::terminus::desc::TARGET =>
                {
                    (Ok(None), target_field.cloned())
                }
                Some(AmqpExtValue::DescribedSymbol { .. }) => (Ok(None), target_field.cloned()),
                _ => (Err(e), None),
            },
        };
        let initial_delivery_count = as_u32(field(value, 9));
        let mut refusal = None;
        if snd_settle_mode > 2 || rcv_settle_mode > 1 {
            refusal = Some(AmqpError::new(
                condition::INVALID_FIELD,
                format!("settle modes {snd_settle_mode} and {rcv_settle_mode}"),
            ));
        } else if role == LinkRole::Receiver && initial_delivery_count.is_none() {
            refusal = Some(AmqpError::new(
                condition::INVALID_FIELD,
                "attach of a sender without initial-delivery-count",
            ));
        }
        let (source, target) = match (source, target) {
            (Ok(s), Ok(t)) => (s, t),
            (Err(e), _) | (_, Err(e)) => {
                refusal.get_or_insert(AmqpError::new(condition::INVALID_FIELD, e));
                (None, None)
            }
        };
        let mut info = LinkInfo {
            id: LinkId { channel, handle },
            name,
            role,
            snd_settle_mode: u8::try_from(snd_settle_mode).unwrap_or(2),
            rcv_settle_mode: u8::try_from(rcv_settle_mode).unwrap_or(0),
            source,
            target,
            target_value,
            coordinator,
            remote_unsettled,
            remote_container: self.remote_container.clone(),
        };
        let created = match refusal {
            Some(e) => Err(e),
            None => app.attach(&info),
        };
        let role_field = AmqpExtValue::Boolean(role == LinkRole::Receiver);
        match created {
            Err(error) => {
                // §2.6.3: "the session endpoint will still create a link
                // endpoint and issue an attach indicating that the link
                // endpoint has no associated local terminus. In this case,
                // the session endpoint MUST immediately detach the newly
                // created link endpoint."
                let echo = |i: usize| field(value, i).cloned().unwrap_or(AmqpExtValue::Null);
                let (source_field, target_field) = match role {
                    LinkRole::Sender => (AmqpExtValue::Null, echo(6)),
                    LinkRole::Receiver => (echo(5), AmqpExtValue::Null),
                };
                let mut attach = vec![
                    AmqpExtValue::Str(info.name.clone()),
                    AmqpExtValue::Uint(local_handle),
                    role_field,
                    AmqpExtValue::Ubyte(info.snd_settle_mode),
                    AmqpExtValue::Ubyte(info.rcv_settle_mode),
                    source_field,
                    target_field,
                ];
                if role == LinkRole::Sender {
                    attach.extend([
                        AmqpExtValue::Null,
                        AmqpExtValue::Null,
                        AmqpExtValue::Uint(0),
                    ]);
                }
                let detach = vec![
                    AmqpExtValue::Uint(local_handle),
                    AmqpExtValue::Boolean(true),
                    encode_error(&error),
                ];
                if let Some(session) = self.sessions.get_mut(&channel) {
                    session.errant.insert(handle);
                }
                self.emit(outgoing, descriptor::ATTACH, attach, &[]);
                self.emit(outgoing, descriptor::DETACH, detach, &[]);
            }
            Ok(Attached {
                source,
                target,
                coordinator,
            }) => {
                // §2.6.1: a link attached a second time while it is
                // attached is stolen: the first attachment is closed with
                // amqp:link:stolen.
                let stolen: Vec<LinkId> = self
                    .sessions
                    .iter()
                    .flat_map(|(c, s)| {
                        s.links
                            .iter()
                            .filter(|(_, l)| l.info.name == info.name && l.info.role == role)
                            .map(|(h, _)| LinkId {
                                channel: *c,
                                handle: *h,
                            })
                    })
                    .collect();
                for id in stolen {
                    self.fail_link(
                        id,
                        condition::STOLEN,
                        format!("link {} was attached again", info.name),
                        app,
                    );
                }
                let target_answer = match (&target, &coordinator, &info.target_value) {
                    (Some(t), _, _) => t.encode(),
                    (None, Some(c), _) => c.encode(),
                    (None, None, Some(raw)) => raw.clone(),
                    (None, None, None) => AmqpExtValue::Null,
                };
                info.source = source;
                info.target = target;
                info.coordinator = coordinator;
                let mut attach = vec![
                    AmqpExtValue::Str(info.name.clone()),
                    AmqpExtValue::Uint(local_handle),
                    role_field,
                    AmqpExtValue::Ubyte(info.snd_settle_mode),
                    AmqpExtValue::Ubyte(info.rcv_settle_mode),
                    opt(info.source.as_ref(), Source::encode),
                    target_answer,
                    // §2.6.3: a terminus this endpoint holds states its
                    // unsettled deliveries. It holds none across an
                    // attach: null for a new link, the empty map for a
                    // resuming one.
                    if info.remote_unsettled.is_some() {
                        AmqpExtValue::Map(Vec::new())
                    } else {
                        AmqpExtValue::Null
                    },
                    AmqpExtValue::Null,
                    if role == LinkRole::Sender {
                        AmqpExtValue::Uint(0)
                    } else {
                        AmqpExtValue::Null
                    },
                    if self.cfg.max_message_size == 0 {
                        AmqpExtValue::Null
                    } else {
                        AmqpExtValue::Ulong(self.cfg.max_message_size)
                    },
                ];
                while matches!(attach.last(), Some(AmqpExtValue::Null)) {
                    attach.pop();
                }
                let link = Link {
                    info,
                    local_handle,
                    delivery_count: initial_delivery_count.unwrap_or(0),
                    initial_delivery_count: 0,
                    link_credit: if role == LinkRole::Receiver {
                        self.cfg.link_credit
                    } else {
                        0
                    },
                    drain: false,
                    remote_max_message_size: as_u64(field(value, 10)).unwrap_or(0),
                    partial: None,
                    sending: None,
                };
                let outgoing_window = self.cfg.outgoing_window;
                let Some(session) = self.sessions.get_mut(&channel) else {
                    return;
                };
                session.local_handles.insert(local_handle);
                session.links.insert(handle, link);
                let flow = (role == LinkRole::Receiver)
                    .then(|| Self::flow_fields(session, outgoing_window, Some(handle), 0));
                self.emit(outgoing, descriptor::ATTACH, attach, &[]);
                if let Some(flow) = flow {
                    // §2.6.7: the receiver grants the credit.
                    self.emit(outgoing, descriptor::FLOW, flow, &[]);
                }
            }
        }
    }

    fn on_flow<C: Container + ?Sized>(&mut self, channel: u16, value: &AmqpExtValue, app: &mut C) {
        let (Some(incoming_window), Some(next_outgoing_id), Some(remote_outgoing_window)) = (
            as_u32(field(value, 1)),
            as_u32(field(value, 2)),
            as_u32(field(value, 3)),
        ) else {
            self.fail_connection(
                condition::INVALID_FIELD,
                "flow without incoming-window, next-outgoing-id or outgoing-window".to_string(),
                app,
            );
            return;
        };
        let outgoing_window = self.cfg.outgoing_window;
        let Some(session) = self.sessions.get_mut(&channel) else {
            return;
        };
        // §2.5.6 "receiving a flow".
        session.next_incoming_id = next_outgoing_id;
        session.remote_outgoing_window = remote_outgoing_window;
        let base = as_u32(field(value, 0)).unwrap_or(session.initial_outgoing_id);
        let window = base
            .wrapping_add(incoming_window)
            .wrapping_sub(session.next_outgoing_id);
        session.remote_incoming_window = if window < 0x8000_0000 { window } else { 0 };
        let echo = as_bool(field(value, 9)).unwrap_or(false);
        let outgoing = session.outgoing_channel;
        let Some(handle) = as_u32(field(value, 4)) else {
            if echo {
                let list = Self::flow_fields(session, outgoing_window, None, 0);
                self.emit(outgoing, descriptor::FLOW, list, &[]);
            }
            return;
        };
        if session.errant.contains(&handle) {
            self.fail_session(
                channel,
                condition::ERRANT_LINK,
                format!("flow for handle {handle} of a link detached with an error"),
                app,
            );
            return;
        }
        let Some(link) = session.links.get_mut(&handle) else {
            self.fail_session(
                channel,
                condition::UNATTACHED_HANDLE,
                format!("flow for handle {handle}, which is not attached"),
                app,
            );
            return;
        };
        let delivery_count = as_u32(field(value, 5));
        match link.info.role {
            LinkRole::Sender => {
                // §2.6.7: link-credit_snd := delivery-count_rcv +
                // link-credit_rcv - delivery-count_snd; without a
                // delivery-count the one of this endpoint's attach.
                if let Some(credit) = as_u32(field(value, 6)) {
                    let limit = delivery_count
                        .unwrap_or(link.initial_delivery_count)
                        .wrapping_add(credit);
                    let left = limit.wrapping_sub(link.delivery_count);
                    link.link_credit = if left < 0x8000_0000 { left } else { 0 };
                }
                link.drain = as_bool(field(value, 8)).unwrap_or(false);
                // Part 4 §4.4.3: the transaction the link acquires in "is
                // determined by the controller by specifying a txn-id entry
                // in the properties map of the flow frame".
                if let Some(properties) = field(value, 10) {
                    let asked = flow_txn_id(properties)
                        .map_err(|e| AmqpError::new(condition::INVALID_FIELD, e))
                        .and_then(|txn| app.acquire_in(&link.info, txn.as_ref()));
                    if let Err(error) = asked {
                        self.detach(LinkId { channel, handle }, Some(error), app);
                        return;
                    }
                }
            }
            LinkRole::Receiver => {
                // The sender's flow state: its delivery-count moves (a
                // drained link), the limit stays.
                if let Some(count) = delivery_count {
                    let limit = link.delivery_count.wrapping_add(link.link_credit);
                    let left = limit.wrapping_sub(count);
                    link.delivery_count = count;
                    link.link_credit = if left < 0x8000_0000 { left } else { 0 };
                }
            }
        }
        if echo {
            let available = if link.info.role == LinkRole::Sender {
                app.available(&link.info)
            } else {
                0
            };
            let list = Self::flow_fields(session, outgoing_window, Some(handle), available);
            self.emit(outgoing, descriptor::FLOW, list, &[]);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn on_transfer<C: Container + ?Sized>(
        &mut self,
        channel: u16,
        value: &AmqpExtValue,
        payload: &[u8],
        app: &mut C,
    ) {
        let Some(handle) = as_u32(field(value, 0)) else {
            self.fail_connection(
                condition::INVALID_FIELD,
                "transfer without handle".to_string(),
                app,
            );
            return;
        };
        let cfg_window = self.cfg.incoming_window;
        let cfg_credit = self.cfg.link_credit;
        let outgoing_window = self.cfg.outgoing_window;
        let max_message = self.cfg.max_message_size;
        let Some(session) = self.sessions.get_mut(&channel) else {
            return;
        };
        if session.incoming_window == 0 {
            self.fail_session(
                channel,
                condition::WINDOW_VIOLATION,
                "transfer beyond the incoming window".to_string(),
                app,
            );
            return;
        }
        // §2.5.6 "receiving a transfer".
        session.next_incoming_id = session.next_incoming_id.wrapping_add(1);
        session.incoming_window -= 1;
        session.remote_outgoing_window = session.remote_outgoing_window.saturating_sub(1);
        if session.errant.contains(&handle) {
            self.fail_session(
                channel,
                condition::ERRANT_LINK,
                format!("transfer for handle {handle} of a link detached with an error"),
                app,
            );
            return;
        }
        let id = LinkId { channel, handle };
        let Some(link) = session.links.get_mut(&handle) else {
            self.fail_session(
                channel,
                condition::UNATTACHED_HANDLE,
                format!("transfer for handle {handle}, which is not attached"),
                app,
            );
            return;
        };
        if link.info.role != LinkRole::Receiver {
            self.fail_link(
                id,
                condition::NOT_ALLOWED,
                "transfer on a link this endpoint sends on".to_string(),
                app,
            );
            return;
        }
        let delivery_id = as_u32(field(value, 1));
        let settled = as_bool(field(value, 4));
        let more = as_bool(field(value, 5)).unwrap_or(false);
        let aborted = as_bool(field(value, 9)).unwrap_or(false);
        // A transaction a later frame names first (Part 4 §4.4.1).
        let mut newly_named = None;
        match &mut link.partial {
            None => {
                let tag = match field(value, 2) {
                    Some(AmqpExtValue::Binary(tag)) => Some(tag.clone()),
                    _ => None,
                };
                let (Some(delivery_id), Some(tag)) = (delivery_id, tag) else {
                    self.fail_link(
                        id,
                        condition::INVALID_FIELD,
                        "first transfer of a delivery without delivery-id or delivery-tag"
                            .to_string(),
                        app,
                    );
                    return;
                };
                if tag.len() > MAX_DELIVERY_TAG {
                    self.fail_link(
                        id,
                        condition::INVALID_FIELD,
                        format!("delivery-tag of {} octets", tag.len()),
                        app,
                    );
                    return;
                }
                if link.link_credit == 0 {
                    self.fail_link(
                        id,
                        condition::TRANSFER_LIMIT_EXCEEDED,
                        "transfer without link credit".to_string(),
                        app,
                    );
                    return;
                }
                let state_field = field(value, 7).cloned();
                let state = match decode_state(state_field.as_ref()) {
                    Ok(state) => state,
                    Err(e) => {
                        self.fail_link(
                            id,
                            condition::INVALID_FIELD,
                            format!("transfer.state: {e}"),
                            app,
                        );
                        return;
                    }
                };
                link.link_credit -= 1;
                link.delivery_count = link.delivery_count.wrapping_add(1);
                let txns = match &state {
                    Some(DeliveryState::Transactional(t)) => vec![t.txn_id.clone()],
                    _ => Vec::new(),
                };
                link.partial = Some(Partial {
                    delivery_id,
                    tag,
                    message_format: as_u32(field(value, 3)).unwrap_or(0),
                    settled: settled.unwrap_or(false),
                    rcv_settle_mode: as_u32(field(value, 6)).and_then(|m| u8::try_from(m).ok()),
                    state,
                    state_field,
                    state_differs: false,
                    txns,
                    announced: false,
                    resume: as_bool(field(value, 8)).unwrap_or(false),
                    payload: Vec::new(),
                });
            }
            Some(partial) => {
                if field(value, 7) != partial.state_field.as_ref() {
                    partial.state_differs = true;
                    match decode_state(field(value, 7)) {
                        Ok(Some(DeliveryState::Transactional(t))) => {
                            if !partial.txns.contains(&t.txn_id) {
                                partial.txns.push(t.txn_id.clone());
                                newly_named = Some(t.txn_id);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            self.fail_link(
                                id,
                                condition::INVALID_FIELD,
                                format!("transfer.state: {e}"),
                                app,
                            );
                            return;
                        }
                    }
                }
                if delivery_id.is_some_and(|d| d != partial.delivery_id) {
                    // §2.6.14: "messages transferred along a single link
                    // MUST NOT be interleaved".
                    self.fail_link(
                        id,
                        condition::NOT_ALLOWED,
                        "a second delivery before the end of the first".to_string(),
                        app,
                    );
                    return;
                }
                // §2.7.5 settled: true once any transfer of the delivery
                // said so.
                partial.settled |= settled.unwrap_or(false);
            }
        }
        let Some(partial) = link.partial.as_mut() else {
            return;
        };
        partial.payload.extend_from_slice(payload);
        if max_message != 0 && partial.payload.len() as u64 > max_message {
            self.fail_link(
                id,
                condition::MESSAGE_SIZE_EXCEEDED,
                format!("message above max-message-size {max_message}"),
                app,
            );
            return;
        }
        let mut pending: Pending = Vec::new();
        let mut destroy = None;
        let outgoing = session.outgoing_channel;
        if aborted {
            // §2.6.14: "the receiver MUST discard the message data that
            // was transferred prior to the abort".
            if let Some(dropped) = link.partial.take()
                && dropped.announced
            {
                app.delivery_aborted(&link.info, dropped.delivery_id);
            }
        } else if more {
            if !partial.announced {
                partial.announced = true;
                app.delivery_begun(&link.info, partial.delivery_id, partial.state.as_ref());
            }
            if let Some(txn) = &newly_named {
                app.delivery_named(&link.info, partial.delivery_id, txn);
            }
        } else {
            let Some(done) = link.partial.take() else {
                return;
            };
            let mode = done.rcv_settle_mode.unwrap_or(link.info.rcv_settle_mode);
            if mode > 1 || (mode == 1 && link.info.rcv_settle_mode == 0) {
                // §2.7.5 rcv-settle-mode: "If the negotiated link value is
                // first, then it is illegal to set this field to second."
                self.fail_link(
                    id,
                    condition::INVALID_FIELD,
                    format!("transfer.rcv-settle-mode {mode} on a link that settles first"),
                    app,
                );
                return;
            }
            let known = session
                .incoming_unsettled
                .values()
                .any(|(h, tag)| *h == handle && *tag == done.tag);
            if done.resume && !known {
                // §2.6.13: a resumed delivery only the source holds
                // unsettled "MUST be ignored by the receiver".
            } else {
                let settled = done.settled;
                let delivery_id = done.delivery_id;
                let tag = done.tag.clone();
                let answer = app.deliver(
                    &link.info,
                    Delivery {
                        delivery_id,
                        tag: done.tag,
                        message_format: done.message_format,
                        settled,
                        payload: done.payload,
                        state: done.state,
                        state_differs: done.state_differs,
                        txns: done.txns,
                    },
                );
                // §2.6.12: mode first settles with the disposition; mode
                // second waits for the sender to settle.
                let settle_now = mode == 0;
                match answer {
                    Err(error) => destroy = Some(error),
                    Ok(_) if settled => {}
                    Ok(Answer::Later) => {
                        session
                            .incoming_unsettled
                            .insert(delivery_id, (handle, tag));
                        session.awaiting_answer.insert(delivery_id, settle_now);
                    }
                    Ok(Answer::State(state)) => {
                        if !settle_now {
                            session
                                .incoming_unsettled
                                .insert(delivery_id, (handle, tag));
                        }
                        if settle_now || state.is_some() {
                            pending.push((
                                outgoing,
                                descriptor::DISPOSITION,
                                vec![
                                    AmqpExtValue::Boolean(true),
                                    AmqpExtValue::Uint(delivery_id),
                                    AmqpExtValue::Null,
                                    AmqpExtValue::Boolean(settle_now),
                                    opt(state.as_ref(), encode_state),
                                ],
                                Vec::new(),
                            ));
                        }
                    }
                }
            }
        }
        // §2.6.7, §2.5.6: restore the credit and the window once half is
        // used; one flow carries both.
        let mut flow_for = None;
        if destroy.is_none()
            && let Some(link) = session.links.get_mut(&handle)
            && link.link_credit <= cfg_credit / 2
            && link.partial.is_none()
        {
            link.link_credit = cfg_credit;
            flow_for = Some(Some(handle));
        }
        if session.incoming_window <= cfg_window / 2 {
            session.incoming_window = cfg_window;
            flow_for.get_or_insert(None);
        }
        if let Some(handle) = flow_for {
            pending.push((
                outgoing,
                descriptor::FLOW,
                Self::flow_fields(session, outgoing_window, handle, 0),
                Vec::new(),
            ));
        }
        self.emit_all(pending);
        if let Some(error) = destroy {
            self.detach(id, Some(error), app);
        }
    }

    fn on_disposition<C: Container + ?Sized>(
        &mut self,
        channel: u16,
        value: &AmqpExtValue,
        app: &mut C,
    ) {
        let (Some(from_receiver), Some(first)) =
            (as_bool(field(value, 0)), as_u32(field(value, 1)))
        else {
            self.fail_connection(
                condition::INVALID_FIELD,
                "disposition without role or first".to_string(),
                app,
            );
            return;
        };
        let last = as_u32(field(value, 2)).unwrap_or(first);
        let settled = as_bool(field(value, 3)).unwrap_or(false);
        let state_field = field(value, 4);
        let state = match decode_state(state_field) {
            Ok(state) => state,
            Err(e) => {
                self.fail_connection(
                    condition::INVALID_FIELD,
                    format!("disposition.state: {e}"),
                    app,
                );
                return;
            }
        };
        let Some(session) = self.sessions.get_mut(&channel) else {
            return;
        };
        let outgoing = session.outgoing_channel;
        let in_range = |id: &u32| serial_le(first, *id) && serial_le(*id, last);
        // §2.6.5: "Should any input (other than a detach) related to the
        // endpoint either via the input handle or delivery-ids be
        // received, the session MUST be terminated with an errant-link
        // session-error."
        let errant = if from_receiver {
            &session.errant_sent
        } else {
            &session.errant_received
        };
        if let Some((delivery, handle)) = errant.iter().find(|(id, _)| in_range(id)) {
            let text = format!(
                "disposition for delivery {delivery} of the link on handle {handle}, \
                 which was detached with an error"
            );
            self.fail_session(channel, condition::ERRANT_LINK, text, app);
            return;
        }
        if !from_receiver {
            // The sender's word on deliveries this endpoint received:
            // settled ends mode second (§2.6.12).
            if settled {
                session.incoming_unsettled.retain(|id, _| !in_range(id));
                session.awaiting_answer.retain(|id, _| !in_range(id));
            }
            return;
        }
        let ids: Vec<u32> = session
            .outgoing_unsettled
            .keys()
            .copied()
            .filter(in_range)
            .collect();
        let mut pending: Pending = Vec::new();
        let mut destroy: Vec<(u32, AmqpError)> = Vec::new();
        for id in ids {
            let Some((handle, tag)) = session.outgoing_unsettled.get(&id).cloned() else {
                continue;
            };
            if destroy.iter().any(|(h, _)| *h == handle) {
                continue;
            }
            let Some(link) = session.links.get(&handle) else {
                continue;
            };
            if let Err(error) = app.disposition(&link.info, &tag, state.as_ref(), settled) {
                destroy.push((handle, error));
                continue;
            }
            if settled {
                session.outgoing_unsettled.remove(&id);
            } else if matches!(
                state,
                Some(DeliveryState::Outcome(_) | DeliveryState::Declared(_))
            ) {
                // §2.6.12: the receiver reached a terminal state and
                // waits; the sender settles. A transactional-state is none
                // (Part 4 §4.4.2): its outcome applies with the discharge.
                session.outgoing_unsettled.remove(&id);
                pending.push((
                    outgoing,
                    descriptor::DISPOSITION,
                    vec![
                        AmqpExtValue::Boolean(false),
                        AmqpExtValue::Uint(id),
                        AmqpExtValue::Null,
                        AmqpExtValue::Boolean(true),
                        state_field.cloned().unwrap_or(AmqpExtValue::Null),
                    ],
                    Vec::new(),
                ));
            }
        }
        self.emit_all(pending);
        for (handle, error) in destroy {
            self.detach(LinkId { channel, handle }, Some(error), app);
        }
    }

    fn on_detach<C: Container + ?Sized>(
        &mut self,
        channel: u16,
        value: &AmqpExtValue,
        app: &mut C,
    ) {
        let Some(handle) = as_u32(field(value, 0)) else {
            self.fail_connection(
                condition::INVALID_FIELD,
                "detach without handle".to_string(),
                app,
            );
            return;
        };
        let closed = as_bool(field(value, 1)).unwrap_or(false);
        let error = decode_error(field(value, 2));
        let Some(session) = self.sessions.get_mut(&channel) else {
            return;
        };
        if Self::forgive(session, handle) {
            // The answer to the detach of this endpoint (§2.6.5).
            return;
        }
        let Some(link) = session.links.remove(&handle) else {
            self.fail_session(
                channel,
                condition::UNATTACHED_HANDLE,
                format!("detach of handle {handle}, which is not attached"),
                app,
            );
            return;
        };
        session.local_handles.remove(&link.local_handle);
        let unsettled = Self::forget_link(session, handle);
        let outgoing = session.outgoing_channel;
        let mut list = vec![AmqpExtValue::Uint(link.local_handle)];
        if closed {
            list.push(AmqpExtValue::Boolean(true));
        }
        self.emit(outgoing, descriptor::DETACH, list, &[]);
        app.detached(&link.info, unsettled, error.as_ref());
    }

    // ------------------------------------------------------------------
    // Sending.
    // ------------------------------------------------------------------

    /// Sends one message on a link this endpoint is the sender of, in as
    /// many transfer frames as the agreed frame size asks for (§2.6.14).
    /// The frames the partner's incoming window admits are written now,
    /// the others when a flow widens it (§2.5.6); until then the link
    /// takes no other delivery. `settled` is taken for a link in settle
    /// mode mixed; a link that settles always or never has no choice
    /// (§2.8.2). Returns the delivery-id.
    ///
    /// # Errors
    /// See [`SendError`]; nothing is sent then.
    pub fn send(
        &mut self,
        id: LinkId,
        tag: &[u8],
        message_format: u32,
        payload: &[u8],
        settled: bool,
    ) -> Result<u32, SendError> {
        self.send_with_state(id, tag, message_format, payload, settled, None)
    }

    /// [`Peer::send`] with a `transfer.state`, which every transfer frame
    /// of the delivery carries (Part 4 §4.4.3: a message acquired in a
    /// transaction is sent with a `transactional-state` that names it).
    ///
    /// # Errors
    /// See [`SendError`]; nothing is sent then.
    pub fn send_with_state(
        &mut self,
        id: LinkId,
        tag: &[u8],
        message_format: u32,
        payload: &[u8],
        settled: bool,
        state: Option<&DeliveryState>,
    ) -> Result<u32, SendError> {
        if self.state != ConnState::Opened {
            return Err(SendError::NoLink);
        }
        if tag.len() > MAX_DELIVERY_TAG {
            return Err(SendError::Tag);
        }
        let max_body = self.remote_max_frame.max(MIN_MAX_FRAME_SIZE) as usize - 8;
        let session = self
            .sessions
            .get_mut(&id.channel)
            .filter(|s| !s.discarding)
            .ok_or(SendError::NoLink)?;
        let link = session
            .links
            .get_mut(&id.handle)
            .filter(|l| l.info.role == LinkRole::Sender)
            .ok_or(SendError::NoLink)?;
        if link.sending.is_some() {
            return Err(SendError::InProgress);
        }
        if link.link_credit == 0 {
            return Err(SendError::NoCredit);
        }
        if link.remote_max_message_size != 0 && payload.len() as u64 > link.remote_max_message_size
        {
            return Err(SendError::MessageTooLarge);
        }
        let settled = match link.info.snd_settle_mode {
            0 => false,
            1 => true,
            _ => settled,
        };
        let delivery_id = session.next_delivery_id;
        let mut sending = Sending {
            delivery_id,
            tag: tag.to_vec(),
            message_format,
            settled,
            state: state.map(encode_state),
            payload: Vec::new(),
            sent: 0,
            aborting: false,
            chunk: 0,
        };
        // The performative has the same length with more true or false.
        let head = encode_performative(
            descriptor::TRANSFER,
            &AmqpExtValue::List(Self::transfer_fields(link.local_handle, &sending, true)),
        )
        .map_err(|_| SendError::Tag)?
        .len();
        if head >= max_body {
            return Err(SendError::FrameSizeTooSmall);
        }
        if session.remote_incoming_window == 0 {
            return Err(SendError::Window);
        }
        sending.chunk = max_body - head;
        sending.payload = payload.to_vec();
        // §2.6.7: the delivery takes its credit with its first frame.
        link.sending = Some(sending);
        link.delivery_count = link.delivery_count.wrapping_add(1);
        link.link_credit -= 1;
        session.next_delivery_id = session.next_delivery_id.wrapping_add(1);
        if !settled {
            session
                .outgoing_unsettled
                .insert(delivery_id, (id.handle, tag.to_vec()));
        }
        let mut pending: Pending = Vec::new();
        Self::transfer_frames(session, id.handle, &mut pending);
        self.emit_all(pending);
        Ok(delivery_id)
    }

    fn transfer_fields(local_handle: u32, sending: &Sending, more: bool) -> Vec<AmqpExtValue> {
        let mut list = vec![
            AmqpExtValue::Uint(local_handle),
            AmqpExtValue::Uint(sending.delivery_id),
            AmqpExtValue::Binary(sending.tag.clone()),
            AmqpExtValue::Uint(sending.message_format),
            AmqpExtValue::Boolean(sending.settled),
            AmqpExtValue::Boolean(more),
        ];
        if let Some(state) = &sending.state {
            // rcv-settle-mode is not set; state is the field after it.
            list.push(AmqpExtValue::Null);
            list.push(state.clone());
        }
        list
    }

    /// Writes the transfer frames of the delivery the link on `handle` is
    /// in the middle of that the partner's incoming window admits (§2.5.6
    /// "sending a transfer": the window is counted per transfer frame).
    fn transfer_frames(session: &mut Session, handle: u32, pending: &mut Pending) {
        let outgoing = session.outgoing_channel;
        let Some(link) = session.links.get_mut(&handle) else {
            return;
        };
        while session.remote_incoming_window > 0 {
            let Some(sending) = link.sending.as_mut() else {
                return;
            };
            if sending.aborting {
                // §2.7.5: resume (8) is not set; aborted is field 9.
                let mut list = Self::transfer_fields(link.local_handle, sending, false);
                list.resize(9, AmqpExtValue::Null);
                list.push(AmqpExtValue::Boolean(true));
                pending.push((outgoing, descriptor::TRANSFER, list, Vec::new()));
                session.next_outgoing_id = session.next_outgoing_id.wrapping_add(1);
                session.remote_incoming_window -= 1;
                link.sending = None;
                return;
            }
            let rest = &sending.payload[sending.sent..];
            let take = rest.len().min(sending.chunk);
            let more = take < rest.len();
            pending.push((
                outgoing,
                descriptor::TRANSFER,
                Self::transfer_fields(link.local_handle, sending, more),
                rest[..take].to_vec(),
            ));
            sending.sent += take;
            session.next_outgoing_id = session.next_outgoing_id.wrapping_add(1);
            session.remote_incoming_window -= 1;
            if !more {
                link.sending = None;
            }
        }
    }

    /// Continues the delivery a link of the session on `channel` is in the
    /// middle of, as far as the partner's incoming window admits. A
    /// delivery is cut only when the window is zero, and no delivery
    /// starts then, so a session has at most one such link at a time.
    fn resume_transfers(&mut self, channel: u16) {
        if self.state != ConnState::Opened {
            return;
        }
        let Some(session) = self.sessions.get_mut(&channel).filter(|s| !s.discarding) else {
            return;
        };
        let waiting: Vec<u32> = session
            .links
            .iter()
            .filter(|(_, l)| l.sending.is_some())
            .map(|(h, _)| *h)
            .collect();
        let mut pending: Pending = Vec::new();
        for handle in waiting {
            Self::transfer_frames(session, handle, &mut pending);
        }
        self.emit_all(pending);
    }

    /// Aborts the delivery the link is in the middle of (§2.6.14: a
    /// transfer with `aborted`; "the receiver MUST discard the message data
    /// that was transferred prior to the abort"). A link is in the middle
    /// of a delivery only while the partner's incoming window is zero, and
    /// the abort is a transfer frame: it is written when a flow widens the
    /// window, in place of the rest of the message, and
    /// [`Peer::is_sending`] is true until then. The delivery is gone on
    /// both sides: no disposition follows. Returns its tag; `None` when
    /// the link is in the middle of none.
    pub fn abort(&mut self, id: LinkId) -> Option<Vec<u8>> {
        if self.state != ConnState::Opened {
            return None;
        }
        let session = self
            .sessions
            .get_mut(&id.channel)
            .filter(|s| !s.discarding)?;
        let sending = session.links.get_mut(&id.handle)?.sending.as_mut()?;
        sending.aborting = true;
        let delivery_id = sending.delivery_id;
        let tag = sending.tag.clone();
        session.outgoing_unsettled.remove(&delivery_id);
        let mut pending: Pending = Vec::new();
        Self::transfer_frames(session, id.handle, &mut pending);
        self.emit_all(pending);
        Some(tag)
    }

    /// Settles a delivery this endpoint sent on the link, with `state` if
    /// any (§2.6.12; Part 4 §4.4.2: "On a successful discharge, the
    /// resource will apply the given outcome and can immediately settle
    /// the transfers"). `false` when the link has no such unsettled
    /// delivery.
    pub fn settle(&mut self, id: LinkId, tag: &[u8], state: Option<&DeliveryState>) -> bool {
        if self.state != ConnState::Opened {
            return false;
        }
        let Some(session) = self.sessions.get_mut(&id.channel).filter(|s| !s.discarding) else {
            return false;
        };
        let Some(delivery_id) = session
            .outgoing_unsettled
            .iter()
            .find(|(_, (handle, t))| *handle == id.handle && t.as_slice() == tag)
            .map(|(delivery_id, _)| *delivery_id)
        else {
            return false;
        };
        session.outgoing_unsettled.remove(&delivery_id);
        let outgoing = session.outgoing_channel;
        let list = vec![
            AmqpExtValue::Boolean(false),
            AmqpExtValue::Uint(delivery_id),
            AmqpExtValue::Null,
            AmqpExtValue::Boolean(true),
            opt(state, encode_state),
        ];
        self.emit(outgoing, descriptor::DISPOSITION, list, &[]);
        true
    }

    /// The answer to a delivery the container took with [`Answer::Later`]:
    /// the disposition with `state`, settled where the link settles with
    /// it (receiver settle mode first). `false` when the delivery waits
    /// for no answer: the sender settled it, or its link is gone.
    pub fn answer(&mut self, id: LinkId, delivery_id: u32, state: Option<&DeliveryState>) -> bool {
        if self.state != ConnState::Opened {
            return false;
        }
        let Some(session) = self.sessions.get_mut(&id.channel).filter(|s| !s.discarding) else {
            return false;
        };
        if !matches!(session.incoming_unsettled.get(&delivery_id), Some((handle, _)) if *handle == id.handle)
        {
            return false;
        }
        let Some(settle) = session.awaiting_answer.remove(&delivery_id) else {
            return false;
        };
        if settle {
            session.incoming_unsettled.remove(&delivery_id);
        }
        if settle || state.is_some() {
            let outgoing = session.outgoing_channel;
            let list = vec![
                AmqpExtValue::Boolean(true),
                AmqpExtValue::Uint(delivery_id),
                AmqpExtValue::Null,
                AmqpExtValue::Boolean(settle),
                opt(state, encode_state),
            ];
            self.emit(outgoing, descriptor::DISPOSITION, list, &[]);
        }
        true
    }

    /// Whether the link is in the middle of a delivery whose remaining
    /// frames wait for the partner's incoming window.
    #[must_use]
    pub fn is_sending(&self, id: LinkId) -> bool {
        self.sending(id).is_some()
    }

    /// The tag of the delivery the link is in the middle of.
    #[must_use]
    pub fn sending(&self, id: LinkId) -> Option<&[u8]> {
        self.sessions
            .get(&id.channel)
            .and_then(|s| s.links.get(&id.handle))
            .and_then(|l| l.sending.as_ref())
            .map(|s| s.tag.as_slice())
    }

    /// A sending link whose partner asked to drain has nothing more to
    /// send (§2.6.7): "the sender MUST advance its delivery-count until
    /// link-credit is zero, and send its updated flow state to the
    /// receiver".
    pub fn drained<C: Container + ?Sized>(&mut self, id: LinkId, app: &mut C) {
        let outgoing_window = self.cfg.outgoing_window;
        let Some(session) = self.sessions.get_mut(&id.channel) else {
            return;
        };
        let Some(link) = session.links.get_mut(&id.handle) else {
            return;
        };
        if link.info.role != LinkRole::Sender
            || !link.drain
            || link.link_credit == 0
            || link.sending.is_some()
        {
            return;
        }
        link.delivery_count = link.delivery_count.wrapping_add(link.link_credit);
        link.link_credit = 0;
        let available = app.available(&link.info);
        let outgoing = session.outgoing_channel;
        let list = Self::flow_fields(session, outgoing_window, Some(id.handle), available);
        self.emit(outgoing, descriptor::FLOW, list, &[]);
    }
}
