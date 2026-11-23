// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The connection endpoint `zerodds_amqp_endpoint::peer::Peer` against the
//! rules of OASIS AMQP 1.0 Part 2: each test feeds it the frames a
//! partner would send and reads the frames it answers with.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_amqp_bridge::extended_types::AmqpExtValue as V;
use zerodds_amqp_bridge::frame::decode_frame_header;
use zerodds_amqp_bridge::performatives::{
    decode_performative, descriptor as d, encode_performative,
};
use zerodds_amqp_endpoint::link::LinkRole;
use zerodds_amqp_endpoint::peer::{
    Answer, Attached, Container, Delivery, LinkFlow, LinkId, LinkInfo, Peer, PeerConfig, SendError,
    condition,
};
use zerodds_amqp_endpoint::terminus::{
    AmqpError, DeliveryState, Outcome, Source, Target, decode_error, decode_outcome, decode_state,
    encode_state,
};
use zerodds_amqp_endpoint::transactions::{
    Coordinator, TransactionalState, TxnCapability, TxnId, flow_properties,
};

/// What the container was told.
struct App {
    attached: Vec<LinkInfo>,
    deliveries: Vec<(LinkId, Delivery)>,
    dispositions: Vec<(Vec<u8>, Outcome, bool)>,
    /// The dispositions with their delivery state as it arrived.
    states: Vec<(Vec<u8>, Option<DeliveryState>, bool)>,
    detached: Vec<(String, Vec<Vec<u8>>, Option<AmqpError>)>,
    refuse: Option<AmqpError>,
    outcome: Outcome,
    /// What the container states as `flow.available`.
    available: u32,
    /// The answer to a delivery, in place of `outcome`.
    answer: Option<Result<Answer, AmqpError>>,
    /// The error a disposition is answered with.
    refuse_disposition: Option<AmqpError>,
    /// The coordinator of this container: a link to one is answered with
    /// it. Without one, a transaction in a flow is refused.
    coordinator: Option<Coordinator>,
    /// The transaction each flow with a properties map named, by link.
    acquiring: Vec<(String, Option<TxnId>)>,
    /// Deliveries of more than one frame that began, with their state.
    begun: Vec<(u32, Option<DeliveryState>)>,
    /// Deliveries that were aborted after they began.
    aborted: Vec<u32>,
    /// Transactions a later frame of a delivery named first.
    named: Vec<(u32, TxnId)>,
}

impl App {
    fn new() -> Self {
        Self {
            attached: Vec::new(),
            deliveries: Vec::new(),
            dispositions: Vec::new(),
            detached: Vec::new(),
            states: Vec::new(),
            refuse: None,
            outcome: Outcome::Accepted,
            available: 0,
            answer: None,
            refuse_disposition: None,
            coordinator: None,
            acquiring: Vec::new(),
            begun: Vec::new(),
            aborted: Vec::new(),
            named: Vec::new(),
        }
    }
}

impl Container for App {
    fn attach(&mut self, link: &LinkInfo) -> Result<Attached, AmqpError> {
        if let Some(e) = &self.refuse {
            return Err(e.clone());
        }
        self.attached.push(link.clone());
        Ok(Attached {
            source: link.source.clone(),
            target: link.target.clone(),
            coordinator: link.coordinator.as_ref().and(self.coordinator.clone()),
        })
    }

    fn deliver(&mut self, link: &LinkInfo, delivery: Delivery) -> Result<Answer, AmqpError> {
        self.deliveries.push((link.id, delivery));
        self.answer
            .clone()
            .unwrap_or_else(|| Ok(Answer::outcome(self.outcome.clone())))
    }

    fn disposition(
        &mut self,
        _: &LinkInfo,
        tag: &[u8],
        state: Option<&DeliveryState>,
        settled: bool,
    ) -> Result<(), AmqpError> {
        if let Some(e) = &self.refuse_disposition {
            return Err(e.clone());
        }
        self.dispositions.push((
            tag.to_vec(),
            state.map_or(Outcome::None, DeliveryState::outcome),
            settled,
        ));
        self.states.push((tag.to_vec(), state.cloned(), settled));
        Ok(())
    }

    fn acquire_in(&mut self, link: &LinkInfo, txn: Option<&TxnId>) -> Result<(), AmqpError> {
        if self.coordinator.is_none() && txn.is_some() {
            return Err(AmqpError::new(
                condition::NOT_IMPLEMENTED,
                "no transactional acquisition",
            ));
        }
        self.acquiring.push((link.name.clone(), txn.cloned()));
        Ok(())
    }

    fn delivery_begun(&mut self, _: &LinkInfo, delivery_id: u32, state: Option<&DeliveryState>) {
        self.begun.push((delivery_id, state.cloned()));
    }

    fn delivery_aborted(&mut self, _: &LinkInfo, delivery_id: u32) {
        self.aborted.push(delivery_id);
    }

    fn delivery_named(&mut self, _: &LinkInfo, delivery_id: u32, txn: &TxnId) {
        self.named.push((delivery_id, txn.clone()));
    }

    fn available(&mut self, _: &LinkInfo) -> u32 {
        self.available
    }

    fn detached(&mut self, link: &LinkInfo, unsettled: Vec<Vec<u8>>, error: Option<&AmqpError>) {
        self.detached
            .push((link.name.clone(), unsettled, error.cloned()));
    }
}

/// A frame the endpoint wrote: channel, descriptor (`None` for the empty
/// frame), fields, payload.
type Out = (u16, Option<u64>, Vec<V>, Vec<u8>);

fn output(peer: &mut Peer) -> Vec<Out> {
    let mut frames = Vec::new();
    while let Some(frame) = peer.poll_output() {
        let mut header = [0u8; 8];
        header.copy_from_slice(&frame[..8]);
        let header = decode_frame_header(&header).expect("a frame header");
        assert_eq!(header.size as usize, frame.len());
        assert_eq!(header.doff, 2);
        let body = &frame[8..];
        if body.is_empty() {
            frames.push((header.channel, None, Vec::new(), Vec::new()));
            continue;
        }
        let (code, value, used) = decode_performative(body).expect("a performative");
        let V::List(fields) = value else {
            panic!("a performative is a list");
        };
        frames.push((header.channel, Some(code), fields, body[used..].to_vec()));
    }
    frames
}

fn body(code: u64, fields: Vec<V>) -> Vec<u8> {
    encode_performative(code, &V::List(fields)).expect("encode")
}

/// Field `i`, null for an absent trailing one.
fn at(fields: &[V], i: usize) -> V {
    fields.get(i).cloned().unwrap_or(V::Null)
}

fn error_of(frame: &Out, i: usize) -> AmqpError {
    decode_error(frame.2.get(i)).expect("an error in the frame")
}

fn open_fields() -> Vec<V> {
    vec![V::Str("partner".into())]
}

/// An endpoint with `open` exchanged.
fn opened(cfg: PeerConfig) -> (Peer, App) {
    let mut app = App::new();
    let mut peer = Peer::new(cfg, 0);
    peer.frame(0, &body(d::OPEN, open_fields()), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Some(d::OPEN));
    (peer, app)
}

fn begin(peer: &mut Peer, app: &mut App, channel: u16) -> Out {
    peer.frame(
        channel,
        &body(
            d::BEGIN,
            vec![V::Null, V::Uint(0), V::Uint(100), V::Uint(100)],
        ),
        0,
        app,
    );
    let mut out = output(peer);
    assert_eq!(out.len(), 1, "{out:?}");
    out.remove(0)
}

fn source(address: &str) -> V {
    Source::at(address).encode()
}

fn target(address: &str) -> V {
    Target::at(address).encode()
}

/// The partner attaches as the sender: this endpoint receives.
fn attach_sender(
    peer: &mut Peer,
    app: &mut App,
    channel: u16,
    handle: u32,
    name: &str,
) -> Vec<Out> {
    peer.frame(
        channel,
        &body(
            d::ATTACH,
            vec![
                V::Str(name.into()),
                V::Uint(handle),
                V::Boolean(false),
                V::Null,
                V::Null,
                source("from"),
                target("q"),
                V::Null,
                V::Null,
                V::Uint(0),
            ],
        ),
        0,
        app,
    );
    output(peer)
}

/// The partner attaches as the receiver: this endpoint sends.
fn attach_receiver(
    peer: &mut Peer,
    app: &mut App,
    channel: u16,
    handle: u32,
    name: &str,
) -> Vec<Out> {
    peer.frame(
        channel,
        &body(
            d::ATTACH,
            vec![
                V::Str(name.into()),
                V::Uint(handle),
                V::Boolean(true),
                V::Null,
                V::Null,
                source("q"),
                target("to"),
            ],
        ),
        0,
        app,
    );
    output(peer)
}

fn transfer(
    handle: u32,
    id: u32,
    tag: &[u8],
    settled: bool,
    more: bool,
    payload: &[u8],
) -> Vec<u8> {
    let mut b = body(
        d::TRANSFER,
        vec![
            V::Uint(handle),
            V::Uint(id),
            V::Binary(tag.to_vec()),
            V::Uint(0),
            V::Boolean(settled),
            V::Boolean(more),
        ],
    );
    b.extend_from_slice(payload);
    b
}

/// A link flow of the partner as receiver: credit and drain.
fn credit(handle: u32, delivery_count: u32, link_credit: u32, drain: bool, echo: bool) -> Vec<u8> {
    body(
        d::FLOW,
        vec![
            V::Uint(0),
            V::Uint(100),
            V::Uint(0),
            V::Uint(100),
            V::Uint(handle),
            V::Uint(delivery_count),
            V::Uint(link_credit),
            V::Null,
            V::Boolean(drain),
            V::Boolean(echo),
        ],
    )
}

// ============================================================================
// §2.4 Connections.
// ============================================================================

/// §2.4.1: open is answered with open on channel 0: container-id, the
/// frame size and channel maximum of this endpoint, half its idle
/// time-out (§2.4.5). The agreed limits are the smaller of both.
#[test]
fn amqp10_2_4_1_open_is_answered_with_the_limits_of_this_endpoint() {
    let mut app = App::new();
    let mut peer = Peer::new(
        PeerConfig {
            container_id: "me".into(),
            max_frame_size: 65_536,
            channel_max: 7,
            idle_time_out_ms: Some(30_000),
            ..PeerConfig::default()
        },
        0,
    );
    peer.frame(
        0,
        &body(
            d::OPEN,
            vec![
                V::Str("partner".into()),
                V::Null,
                V::Uint(4096),
                V::Ushort(3),
                V::Uint(10_000),
            ],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    let (channel, code, fields, _) = &out[0];
    assert_eq!((*channel, *code), (0, Some(d::OPEN)));
    assert_eq!(at(fields, 0), V::Str("me".into()));
    assert_eq!(at(fields, 2), V::Uint(65_536));
    assert_eq!(at(fields, 3), V::Ushort(7));
    assert_eq!(at(fields, 4), V::Uint(15_000));
    assert_eq!(peer.remote_container(), "partner");
    assert_eq!(peer.remote_max_frame_size(), 4096);
    // channel-max is the smaller one, 3: channel 4 is outside.
    peer.frame(
        4,
        &body(d::BEGIN, vec![V::Null, V::Uint(0), V::Uint(1), V::Uint(1)]),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::CLOSE));
    assert_eq!(error_of(&out[0], 0).condition, condition::FRAMING_ERROR);
}

/// §2.4.1: "each peer MUST send an open frame before sending any other
/// frames". A begin first is answered with open (the first frame this
/// endpoint may send) and a close with amqp:illegal-state. An open on
/// another channel and one without container-id are refused too.
#[test]
fn amqp10_2_4_1_a_frame_before_open_closes_with_illegal_state() {
    let mut app = App::new();
    let mut peer = Peer::new(PeerConfig::default(), 0);
    peer.frame(
        0,
        &body(d::BEGIN, vec![V::Null, V::Uint(0), V::Uint(1), V::Uint(1)]),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].1, Some(d::OPEN));
    assert_eq!(out[1].1, Some(d::CLOSE));
    assert_eq!(error_of(&out[1], 0).condition, condition::ILLEGAL_STATE);
    assert!(peer.is_closing());

    let mut peer = Peer::new(PeerConfig::default(), 0);
    peer.frame(1, &body(d::OPEN, open_fields()), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(error_of(&out[1], 0).condition, condition::NOT_ALLOWED);

    let mut peer = Peer::new(PeerConfig::default(), 0);
    peer.frame(0, &body(d::OPEN, vec![]), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(error_of(&out[1], 0).condition, condition::INVALID_FIELD);

    // A second open.
    let (mut peer, mut app) = opened(PeerConfig::default());
    peer.frame(0, &body(d::OPEN, open_fields()), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(error_of(&out[0], 0).condition, condition::ILLEGAL_STATE);
}

/// §2.4.3: close is answered with close, the last frame; the links of the
/// connection are gone, with the error the partner closed with.
#[test]
fn amqp10_2_4_3_close_is_answered_with_close_and_ends_the_connection() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "L");
    let error = AmqpError::new("amqp:connection:forced", "maintenance");
    peer.frame(
        0,
        &body(
            d::CLOSE,
            vec![zerodds_amqp_endpoint::terminus::encode_error(&error)],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].0, out[0].1), (0, Some(d::CLOSE)));
    assert!(out[0].2.is_empty(), "a close without an error");
    assert!(peer.is_ended());
    assert_eq!(peer.remote_close(), Some(&Some(error.clone())));
    assert_eq!(app.detached, vec![("L".to_string(), vec![], Some(error))]);
    // Nothing is written after the close.
    peer.frame(0, &body(d::BEGIN, vec![]), 0, &mut app);
    peer.tick(1_000_000, &mut app);
    assert!(output(&mut peer).is_empty());
}

/// §2.4.5: this endpoint sends an empty frame within half the partner's
/// idle-time-out when it has nothing else to send, and closes with an
/// error when the partner is silent for the local time-out; an empty
/// frame of the partner counts as a frame.
#[test]
fn amqp10_2_4_5_idle_time_out_is_kept_in_both_directions() {
    let mut app = App::new();
    let mut peer = Peer::new(
        PeerConfig {
            idle_time_out_ms: Some(10_000),
            ..PeerConfig::default()
        },
        0,
    );
    let mut open = open_fields();
    open.extend([V::Null, V::Null, V::Null, V::Uint(4_000)]);
    peer.frame(0, &body(d::OPEN, open), 0, &mut app);
    output(&mut peer);

    peer.tick(1_999, &mut app);
    assert!(output(&mut peer).is_empty(), "half of 4000 ms not reached");
    peer.tick(2_000, &mut app);
    let out = output(&mut peer);
    assert_eq!(out, vec![(0, None, vec![], vec![])], "one empty frame");
    peer.tick(3_000, &mut app);
    assert!(output(&mut peer).is_empty(), "sent 1000 ms ago");

    // The partner's empty frame at 9000 ms moves the deadline to 19000.
    peer.frame(0, &[], 9_000, &mut app);
    peer.tick(18_999, &mut app);
    let out = output(&mut peer);
    assert!(out.iter().all(|f| f.1.is_none()), "{out:?}");
    peer.tick(19_000, &mut app);
    let out = output(&mut peer);
    let close = out.last().expect("a close");
    assert_eq!(close.1, Some(d::CLOSE));
    assert_eq!(
        error_of(close, 0).condition,
        condition::RESOURCE_LIMIT_EXCEEDED
    );
    assert!(peer.is_closing());
    // §2.4.5: no empty frame after the close.
    peer.tick(40_000, &mut app);
    assert!(output(&mut peer).is_empty());
}

/// §2.4.6 DISCARDING: after a close with an error "any incoming frames on
/// the connection MUST be silently discarded until the peer's close frame
/// is received".
#[test]
fn amqp10_2_4_6_after_a_close_with_an_error_frames_are_discarded_until_close() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    peer.frame(0, &body(0x99, vec![]), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(error_of(&out[0], 0).condition, condition::NOT_IMPLEMENTED);
    assert!(peer.is_closing());
    peer.frame(
        0,
        &body(d::BEGIN, vec![V::Null, V::Uint(0), V::Uint(1), V::Uint(1)]),
        0,
        &mut app,
    );
    peer.frame(0, &[0xff, 0xff], 0, &mut app);
    assert!(output(&mut peer).is_empty());
    assert!(!peer.is_ended());
    peer.frame(0, &body(d::CLOSE, vec![]), 0, &mut app);
    assert!(output(&mut peer).is_empty(), "the close was sent already");
    assert!(peer.is_ended());
    assert_eq!(
        peer.local_close().map(|e| e.condition.as_str()),
        Some(condition::NOT_IMPLEMENTED)
    );
}

/// §2.8.15: octets that are no frame body close with amqp:decode-error, a
/// frame body with a symbolic descriptor outside the specification with
/// amqp:not-implemented, a body that is no list with amqp:decode-error.
#[test]
fn amqp10_2_8_15_an_undecodable_frame_body_closes_with_its_condition() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    peer.frame(0, &[0xa1, 0x05, b'h'], 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(error_of(&out[0], 0).condition, condition::DECODE_ERROR);

    let (mut peer, mut app) = opened(PeerConfig::default());
    let mut unknown = vec![0x00, 0xa3, 0x09];
    unknown.extend_from_slice(b"amqp:nope");
    unknown.push(0x45);
    peer.frame(0, &unknown, 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(error_of(&out[0], 0).condition, condition::NOT_IMPLEMENTED);

    let (mut peer, mut app) = opened(PeerConfig::default());
    peer.frame(0, &[0x00, 0x53, 0x11, 0x40], 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(error_of(&out[0], 0).condition, condition::DECODE_ERROR);

    // open by its symbolic descriptor (Part 1 §1.5) is open.
    let mut app = App::new();
    let mut peer = Peer::new(PeerConfig::default(), 0);
    let by_code = body(d::OPEN, open_fields());
    let mut by_symbol = vec![0x00, 0xa3, 0x0e];
    by_symbol.extend_from_slice(b"amqp:open:list");
    by_symbol.extend_from_slice(&by_code[3..]);
    peer.frame(0, &by_symbol, 0, &mut app);
    assert_eq!(output(&mut peer)[0].1, Some(d::OPEN));
}

// ============================================================================
// §2.5 Sessions.
// ============================================================================

/// §2.5.1: a begin without remote-channel is answered with a begin on the
/// lowest unused outgoing channel whose remote-channel is the channel the
/// partner's begin came on; the windows and handle-max are this
/// endpoint's.
#[test]
fn amqp10_2_5_1_begin_is_answered_on_the_lowest_free_channel_with_remote_channel() {
    let (mut peer, mut app) = opened(PeerConfig {
        incoming_window: 50,
        outgoing_window: 60,
        handle_max: 9,
        ..PeerConfig::default()
    });
    let first = begin(&mut peer, &mut app, 3);
    assert_eq!((first.0, first.1), (0, Some(d::BEGIN)));
    assert_eq!(at(&first.2, 0), V::Ushort(3));
    assert_eq!(at(&first.2, 1), V::Uint(0));
    assert_eq!(at(&first.2, 2), V::Uint(50));
    assert_eq!(at(&first.2, 3), V::Uint(60));
    assert_eq!(at(&first.2, 4), V::Uint(9));
    let second = begin(&mut peer, &mut app, 1);
    assert_eq!(second.0, 1);
    assert_eq!(at(&second.2, 0), V::Ushort(1));

    // §2.5.2: end is answered with end on the session's outgoing channel,
    // which is free again.
    peer.frame(3, &body(d::END, vec![]), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].0, out[0].1), (0, Some(d::END)));
    assert!(out[0].2.is_empty());
    let third = begin(&mut peer, &mut app, 5);
    assert_eq!(third.0, 0);
    assert_eq!(at(&third.2, 0), V::Ushort(5));

    // A begin on a channel that has a session, and one that answers a
    // begin this endpoint never sent.
    peer.frame(
        5,
        &body(d::BEGIN, vec![V::Null, V::Uint(0), V::Uint(1), V::Uint(1)]),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(error_of(&out[0], 0).condition, condition::ILLEGAL_STATE);
    let (mut peer, mut app) = opened(PeerConfig::default());
    peer.frame(
        0,
        &body(
            d::BEGIN,
            vec![V::Ushort(0), V::Uint(0), V::Uint(1), V::Uint(1)],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(error_of(&out[0], 0).condition, condition::NOT_ALLOWED);
}

/// §2.5.4: "When a session is unable to process input, it MUST indicate
/// this by issuing an END with an appropriate error ... It MUST then
/// proceed to discard all incoming frames from the remote endpoint until
/// receiving the remote endpoint's corresponding end frame." Here: a flow
/// for a handle that is not attached (§2.8.17 unattached-handle).
#[test]
fn amqp10_2_5_4_a_session_error_ends_the_session_and_discards_until_end() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "L");
    peer.frame(0, &credit(7, 0, 1, false, false), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].0, out[0].1), (0, Some(d::END)));
    assert_eq!(error_of(&out[0], 0).condition, condition::UNATTACHED_HANDLE);
    // The link of the session is gone, with the error of the session.
    assert_eq!(app.detached.len(), 1);
    assert_eq!(
        app.detached[0].2.as_ref().map(|e| e.condition.as_str()),
        Some(condition::UNATTACHED_HANDLE)
    );
    // Discarded: a transfer and an attach on the ended session.
    peer.frame(0, &transfer(0, 0, b"t", true, false, b"x"), 0, &mut app);
    attach_sender(&mut peer, &mut app, 0, 1, "M");
    assert!(output(&mut peer).is_empty());
    assert!(app.deliveries.is_empty());
    // The partner's end frees the channel; another session of the
    // connection was not touched.
    peer.frame(0, &body(d::END, vec![]), 0, &mut app);
    assert!(output(&mut peer).is_empty(), "the end was sent already");
    let again = begin(&mut peer, &mut app, 0);
    assert_eq!(again.1, Some(d::BEGIN));
    assert!(!peer.is_closing());
}

/// §2.5.6: the window variables. A transfer received advances
/// next-incoming-id and takes one of the incoming window; a flow sets
/// next-incoming-id and remote-outgoing-window from the frame and
/// computes remote-incoming-window as next-incoming-id(flow) +
/// incoming-window(flow) - next-outgoing-id(endpoint); a transfer sent
/// advances next-outgoing-id and takes one of the remote incoming window.
#[test]
fn amqp10_2_5_6_the_windows_follow_transfers_and_flows() {
    let (mut peer, mut app) = opened(PeerConfig {
        incoming_window: 8,
        link_credit: 100,
        ..PeerConfig::default()
    });
    peer.frame(
        0,
        &body(d::BEGIN, vec![V::Null, V::Uint(40), V::Uint(5), V::Uint(9)]),
        0,
        &mut app,
    );
    output(&mut peer);
    let w = peer.session_windows(0).expect("a session");
    assert_eq!(
        (w.next_incoming_id, w.incoming_window, w.next_outgoing_id),
        (40, 8, 0)
    );
    assert_eq!((w.remote_incoming_window, w.remote_outgoing_window), (5, 9));

    attach_sender(&mut peer, &mut app, 0, 0, "in");
    for i in 0..3u32 {
        peer.frame(
            0,
            &transfer(0, i, &[i as u8], true, false, b"x"),
            0,
            &mut app,
        );
    }
    assert!(output(&mut peer).is_empty());
    let w = peer.session_windows(0).expect("a session");
    assert_eq!((w.next_incoming_id, w.incoming_window), (43, 5));
    assert_eq!(w.remote_outgoing_window, 6);
    // The fourth transfer leaves half the window: it is widened to 8 again
    // and the partner told (a flow without a handle).
    peer.frame(0, &transfer(0, 3, b"3", true, false, b"x"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Some(d::FLOW));
    assert_eq!(at(&out[0].2, 0), V::Uint(44));
    assert_eq!(at(&out[0].2, 1), V::Uint(8));
    assert_eq!(at(&out[0].2, 4), V::Null);

    // Sending: a link this endpoint sends on, credit 10.
    attach_receiver(&mut peer, &mut app, 0, 1, "out");
    peer.frame(0, &credit(1, 0, 10, false, false), 0, &mut app);
    let id = LinkId {
        channel: 0,
        handle: 1,
    };
    // The flow said next-incoming-id 0, incoming-window 100.
    assert_eq!(
        peer.session_windows(0).expect("s").remote_incoming_window,
        100
    );
    peer.send(id, b"a", 0, b"one", true).expect("send");
    peer.send(id, b"b", 0, b"two", true).expect("send");
    let w = peer.session_windows(0).expect("a session");
    assert_eq!((w.next_outgoing_id, w.remote_incoming_window), (2, 98));
    // A flow of the partner: it has seen one of the two (next-incoming-id
    // 1) and takes 3 more: 1 + 3 - 2 = 2.
    peer.frame(
        0,
        &body(
            d::FLOW,
            vec![V::Uint(1), V::Uint(3), V::Uint(50), V::Uint(7)],
        ),
        0,
        &mut app,
    );
    let w = peer.session_windows(0).expect("a session");
    assert_eq!(w.remote_incoming_window, 2);
    assert_eq!((w.next_incoming_id, w.remote_outgoing_window), (50, 7));
    peer.send(id, b"c", 0, b"3", true).expect("send");
    peer.send(id, b"d", 0, b"4", true).expect("send");
    assert_eq!(peer.send(id, b"e", 0, b"5", true), Err(SendError::Window));
    assert!(peer.sendable().is_empty(), "no window, nothing sendable");
}

/// §2.8.17 window-violation: a transfer beyond the incoming window ends
/// the session.
#[test]
fn amqp10_2_8_17_a_transfer_beyond_the_incoming_window_is_a_window_violation() {
    let (mut peer, mut app) = opened(PeerConfig {
        incoming_window: 0,
        ..PeerConfig::default()
    });
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "L");
    peer.frame(0, &transfer(0, 0, b"t", true, false, b"x"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::END));
    assert_eq!(error_of(&out[0], 0).condition, condition::WINDOW_VIOLATION);
    assert!(app.deliveries.is_empty());
}

// ============================================================================
// §2.6 Links.
// ============================================================================

/// §2.6.3: an attach is answered with an attach of the same name, the
/// opposite role, a handle of this endpoint (the lowest unused one, §2.6.2)
/// and the termini as created. A receiving endpoint grants credit with a
/// flow (§2.6.7); a sending one states its initial delivery-count.
#[test]
fn amqp10_2_6_3_attach_is_answered_with_the_opposite_role_and_a_handle() {
    let (mut peer, mut app) = opened(PeerConfig {
        link_credit: 25,
        max_message_size: 1000,
        ..PeerConfig::default()
    });
    begin(&mut peer, &mut app, 0);
    let out = attach_sender(&mut peer, &mut app, 0, 5, "in");
    assert_eq!(out.len(), 2, "{out:?}");
    let (_, code, f, _) = &out[0];
    assert_eq!(*code, Some(d::ATTACH));
    assert_eq!(at(f, 0), V::Str("in".into()));
    assert_eq!(at(f, 1), V::Uint(0), "the lowest unused handle");
    assert_eq!(at(f, 2), V::Boolean(true), "role receiver");
    assert_eq!(at(f, 5), source("from"));
    assert_eq!(at(f, 6), target("q"));
    assert_eq!(at(f, 7), V::Null, "a new link has no unsettled map");
    assert_eq!(at(f, 9), V::Null, "a receiver states no delivery-count");
    assert_eq!(at(f, 10), V::Ulong(1000));
    let (_, code, f, _) = &out[1];
    assert_eq!(*code, Some(d::FLOW));
    assert_eq!(at(f, 4), V::Uint(0), "the handle of this endpoint");
    assert_eq!(at(f, 5), V::Uint(0), "the sender's delivery-count");
    assert_eq!(at(f, 6), V::Uint(25));
    assert_eq!(app.attached[0].role, LinkRole::Receiver);
    assert_eq!(app.attached[0].remote_container, "partner");

    let out = attach_receiver(&mut peer, &mut app, 0, 6, "out");
    assert_eq!(out.len(), 1, "a sender grants no credit");
    let (_, _, f, _) = &out[0];
    assert_eq!(at(f, 1), V::Uint(1), "the next handle");
    assert_eq!(at(f, 2), V::Boolean(false), "role sender");
    assert_eq!(at(f, 9), V::Uint(0), "initial-delivery-count");
    assert_eq!(app.attached[1].role, LinkRole::Sender);
    assert_eq!(
        peer.link(LinkId {
            channel: 0,
            handle: 6
        })
        .map(|l| l.name.as_str()),
        Some("out")
    );

    // §2.8.17 handle-in-use.
    let out = attach_sender(&mut peer, &mut app, 0, 5, "again");
    assert_eq!(out[0].1, Some(d::END));
    assert_eq!(error_of(&out[0], 0).condition, condition::HANDLE_IN_USE);
}

/// §2.7.2 handle-max: "A peer that receives a handle outside the
/// supported range MUST close the connection with the framing-error
/// error-code."
#[test]
fn amqp10_2_7_2_a_handle_above_handle_max_closes_with_framing_error() {
    let (mut peer, mut app) = opened(PeerConfig {
        handle_max: 3,
        ..PeerConfig::default()
    });
    begin(&mut peer, &mut app, 0);
    let out = attach_sender(&mut peer, &mut app, 0, 4, "L");
    assert_eq!(out[0].1, Some(d::CLOSE));
    assert_eq!(error_of(&out[0], 0).condition, condition::FRAMING_ERROR);
}

/// §2.6.3: when no terminus is created "the session endpoint will still
/// create a link endpoint and issue an attach indicating that the link
/// endpoint has no associated local terminus. In this case, the session
/// endpoint MUST immediately detach the newly created link endpoint." The
/// detach closes the link and carries the error.
#[test]
fn amqp10_2_6_3_a_refused_attach_is_answered_with_a_null_terminus_and_a_closing_detach() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    app.refuse = Some(AmqpError::new(condition::NOT_FOUND, "no node q"));
    let out = attach_sender(&mut peer, &mut app, 0, 0, "in");
    assert_eq!(out.len(), 2, "{out:?}");
    assert_eq!(out[0].1, Some(d::ATTACH));
    assert_eq!(at(&out[0].2, 2), V::Boolean(true));
    assert_eq!(at(&out[0].2, 5), source("from"));
    assert_eq!(at(&out[0].2, 6), V::Null, "no target was created");
    assert_eq!(out[1].1, Some(d::DETACH));
    assert_eq!(at(&out[1].2, 0), at(&out[0].2, 1), "the same handle");
    assert_eq!(at(&out[1].2, 1), V::Boolean(true), "closed");
    assert_eq!(
        error_of(&out[1], 2),
        AmqpError::new(condition::NOT_FOUND, "no node q")
    );
    // A receiver refused: the source is the null one.
    let out = attach_receiver(&mut peer, &mut app, 0, 1, "out");
    assert_eq!(at(&out[0].2, 5), V::Null);
    assert_eq!(at(&out[0].2, 6), target("to"));
    assert_eq!(at(&out[0].2, 9), V::Uint(0));
    // The partner's detach of the refused link is its answer; the session
    // lives on.
    peer.frame(
        0,
        &body(d::DETACH, vec![V::Uint(0), V::Boolean(true)]),
        0,
        &mut app,
    );
    assert!(output(&mut peer).is_empty());
    app.refuse = None;
    let out = attach_sender(&mut peer, &mut app, 0, 0, "in");
    assert_eq!(out[0].1, Some(d::ATTACH));
    assert_eq!(at(&out[0].2, 6), target("q"));

    // An attach whose source does not decode is refused by the endpoint.
    peer.frame(
        0,
        &body(
            d::ATTACH,
            vec![
                V::Str("bad".into()),
                V::Uint(2),
                V::Boolean(true),
                V::Null,
                V::Null,
                V::Str("not a source".into()),
            ],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[1].1, Some(d::DETACH));
    assert_eq!(error_of(&out[1], 2).condition, condition::INVALID_FIELD);
}

/// §2.6.5: after a link was detached with an error, "If any input (other
/// than a detach) related to the endpoint ... be received, the session
/// MUST be terminated with an errant-link session-error", and "Receipt of
/// a pipelined attach MUST result in the session being terminated with an
/// errant-link session-error"; a resuming attach (non-null unsettled) is
/// the way back.
#[test]
fn amqp10_2_6_5_input_for_a_link_detached_with_an_error_ends_the_session() {
    // A transfer behind a refused attach.
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    app.refuse = Some(AmqpError::new(condition::NOT_FOUND, "no node"));
    attach_sender(&mut peer, &mut app, 0, 0, "in");
    peer.frame(0, &transfer(0, 0, b"t", true, false, b"x"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::END));
    assert_eq!(error_of(&out[0], 0).condition, condition::ERRANT_LINK);

    // A pipelined attach on the handle.
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    app.refuse = Some(AmqpError::new(condition::NOT_FOUND, "no node"));
    attach_sender(&mut peer, &mut app, 0, 0, "in");
    app.refuse = None;
    let out = attach_sender(&mut peer, &mut app, 0, 0, "in");
    assert_eq!(out[0].1, Some(d::END));
    assert_eq!(error_of(&out[0], 0).condition, condition::ERRANT_LINK);

    // A resuming attach: unsettled is a map, here the empty one.
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    app.refuse = Some(AmqpError::new(condition::NOT_FOUND, "no node"));
    attach_sender(&mut peer, &mut app, 0, 0, "in");
    app.refuse = None;
    peer.frame(
        0,
        &body(
            d::ATTACH,
            vec![
                V::Str("in".into()),
                V::Uint(0),
                V::Boolean(false),
                V::Null,
                V::Null,
                source("from"),
                target("q"),
                V::Map(vec![]),
                V::Null,
                V::Uint(0),
            ],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::ATTACH));
    assert_eq!(
        at(&out[0].2, 7),
        V::Map(vec![]),
        "the unsettled map of this endpoint"
    );
    assert_eq!(app.attached[0].remote_unsettled, Some(vec![]));
}

/// §2.6.1: "If an attempt is made to attach the link subsequently when it
/// is not suspended, then the link can be 'stolen', i.e., the second
/// attach succeeds and the first attach MUST then be closed with a link
/// error of stolen."
#[test]
fn amqp10_2_6_1_a_link_attached_again_is_stolen() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    begin(&mut peer, &mut app, 1);
    attach_sender(&mut peer, &mut app, 0, 0, "L");
    let out = attach_sender(&mut peer, &mut app, 1, 0, "L");
    assert_eq!(out.len(), 3, "{out:?}");
    assert_eq!((out[0].0, out[0].1), (0, Some(d::DETACH)));
    assert_eq!(at(&out[0].2, 1), V::Boolean(true));
    assert_eq!(error_of(&out[0], 2).condition, condition::STOLEN);
    assert_eq!((out[1].0, out[1].1), (1, Some(d::ATTACH)));
    assert_eq!(app.detached.len(), 1);
    // The same name in the other direction is another link.
    let out = attach_receiver(&mut peer, &mut app, 1, 1, "L");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Some(d::ATTACH));
}

/// §2.6.4, §2.6.6: a detach is answered with a detach on the handle of
/// this endpoint, closing when the partner's closes; the handles are free
/// again. A detach of a handle that is not attached is a session error.
#[test]
fn amqp10_2_6_6_detach_is_answered_and_frees_the_handle() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 4, "a");
    attach_sender(&mut peer, &mut app, 0, 5, "b");
    peer.frame(
        0,
        &body(d::DETACH, vec![V::Uint(4), V::Boolean(true)]),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(at(&out[0].2, 0), V::Uint(0));
    assert_eq!(at(&out[0].2, 1), V::Boolean(true));
    // A non-closing detach (§2.6.4) is answered with a non-closing one.
    let error = AmqpError::new("amqp:link:detach-forced", "bye");
    peer.frame(
        0,
        &body(
            d::DETACH,
            vec![
                V::Uint(5),
                V::Null,
                zerodds_amqp_endpoint::terminus::encode_error(&error),
            ],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(at(&out[0].2, 0), V::Uint(1));
    assert_eq!(at(&out[0].2, 1), V::Null);
    assert_eq!(
        app.detached,
        vec![
            ("a".to_string(), vec![], None),
            ("b".to_string(), vec![], Some(error))
        ]
    );
    // Both handles are free: the partner's 4, this endpoint's 0.
    let out = attach_sender(&mut peer, &mut app, 0, 4, "c");
    assert_eq!(at(&out[0].2, 1), V::Uint(0));

    peer.frame(0, &body(d::DETACH, vec![V::Uint(9)]), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::END));
    assert_eq!(error_of(&out[0], 0).condition, condition::UNATTACHED_HANDLE);
}

/// §2.6.7: a sender sends only within the link credit, which it computes
/// as delivery-count(rcv) + link-credit(rcv) - delivery-count(snd); with
/// drain set and nothing to send it advances its delivery-count by the
/// credit left and sends its flow state; echo asks for the flow state.
#[test]
fn amqp10_2_6_7_a_sender_sends_within_the_credit_and_drains() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_receiver(&mut peer, &mut app, 0, 3, "out");
    let id = LinkId {
        channel: 0,
        handle: 3,
    };
    assert_eq!(peer.send(id, b"a", 0, b"x", true), Err(SendError::NoCredit));
    assert!(peer.sendable().is_empty());
    peer.frame(0, &credit(3, 0, 2, false, false), 0, &mut app);
    assert!(output(&mut peer).is_empty(), "no echo asked");
    assert_eq!(peer.sendable(), vec![(id, 2, false)]);
    assert_eq!(peer.send(id, b"a", 0, b"one", true), Ok(0));
    assert_eq!(peer.send(id, b"b", 0, b"two", true), Ok(1));
    assert_eq!(peer.send(id, b"c", 0, b"x", true), Err(SendError::NoCredit));
    let out = output(&mut peer);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].1, Some(d::TRANSFER));
    assert_eq!(at(&out[0].2, 0), V::Uint(0), "the handle of this endpoint");
    assert_eq!(at(&out[0].2, 1), V::Uint(0));
    assert_eq!(at(&out[0].2, 2), V::Binary(b"a".to_vec()));
    assert_eq!(at(&out[0].2, 4), V::Boolean(true));
    assert_eq!(at(&out[0].2, 5), V::Boolean(false));
    assert_eq!(out[0].3, b"one");
    assert_eq!(at(&out[1].2, 1), V::Uint(1));

    // The receiver has counted one of the two and grants 5: 1 + 5 - 2 = 4.
    app.available = 7;
    peer.frame(0, &credit(3, 1, 5, true, true), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1, "the echo");
    assert_eq!(out[0].1, Some(d::FLOW));
    assert_eq!(at(&out[0].2, 5), V::Uint(2), "delivery-count");
    assert_eq!(at(&out[0].2, 6), V::Uint(4), "link-credit");
    assert_eq!(at(&out[0].2, 7), V::Uint(7), "available");
    app.available = 0;
    assert_eq!(at(&out[0].2, 8), V::Boolean(true), "drain");
    assert_eq!(peer.sendable(), vec![(id, 4, true)]);
    peer.send(id, b"c", 0, b"three", true).expect("send");
    output(&mut peer);
    // Nothing more to send: drained.
    peer.drained(id, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Some(d::FLOW));
    assert_eq!(at(&out[0].2, 4), V::Uint(0));
    assert_eq!(
        at(&out[0].2, 5),
        V::Uint(6),
        "3 sent and 3 credits consumed"
    );
    assert_eq!(at(&out[0].2, 6), V::Uint(0));
    assert_eq!(
        peer.link_flow(id),
        Some(LinkFlow {
            delivery_count: 6,
            link_credit: 0,
            drain: true
        })
    );
    peer.drained(id, &mut app);
    assert!(output(&mut peer).is_empty(), "no credit left to drain");

    // A session flow with echo (no handle) is answered without a handle.
    peer.frame(
        0,
        &body(
            d::FLOW,
            vec![
                V::Uint(3),
                V::Uint(100),
                V::Uint(0),
                V::Uint(100),
                V::Null,
                V::Null,
                V::Null,
                V::Null,
                V::Null,
                V::Boolean(true),
            ],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::FLOW));
    assert_eq!(out[0].2.len(), 4);
    assert_eq!(at(&out[0].2, 2), V::Uint(3), "next-outgoing-id");
}

/// §2.6.7: "If the link-credit is less than or equal to zero ... a sender
/// MUST NOT send more messages"; a receiver may "detach the link with a
/// transfer-limit-exceeded error code" (§2.8.18).
#[test]
fn amqp10_2_6_7_a_transfer_without_credit_detaches_with_transfer_limit_exceeded() {
    let (mut peer, mut app) = opened(PeerConfig {
        link_credit: 0,
        ..PeerConfig::default()
    });
    begin(&mut peer, &mut app, 0);
    let out = attach_sender(&mut peer, &mut app, 0, 0, "in");
    assert_eq!(at(&out[1].2, 6), V::Uint(0), "no credit granted");
    peer.frame(0, &transfer(0, 0, b"t", false, false, b"x"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(at(&out[0].2, 1), V::Boolean(true));
    assert_eq!(
        error_of(&out[0], 2).condition,
        condition::TRANSFER_LIMIT_EXCEEDED
    );
    assert!(app.deliveries.is_empty());
}

/// §2.6.7: the receiver restores the credit once half of it is used, with
/// a flow that carries the sender's delivery-count as counted.
#[test]
fn amqp10_2_6_7_a_receiver_restores_the_credit_when_half_is_used() {
    let (mut peer, mut app) = opened(PeerConfig {
        link_credit: 4,
        ..PeerConfig::default()
    });
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "in");
    peer.frame(0, &transfer(0, 0, b"a", true, false, b"x"), 0, &mut app);
    assert!(output(&mut peer).is_empty(), "3 of 4 left");
    peer.frame(0, &transfer(0, 1, b"b", true, false, b"x"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Some(d::FLOW));
    assert_eq!(at(&out[0].2, 5), V::Uint(2), "delivery-count");
    assert_eq!(at(&out[0].2, 6), V::Uint(4), "link-credit");
    assert_eq!(app.deliveries.len(), 2);
}

/// §2.6.12: a delivery the sender did not settle is answered with a
/// disposition of the receiver that carries the outcome: settled in
/// receiver settle mode first; in mode second unsettled until the sender
/// settles. A settled transfer gets none.
#[test]
fn amqp10_2_6_12_an_unsettled_delivery_is_answered_with_its_outcome() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "first");
    peer.frame(
        0,
        &transfer(0, 7, b"t7", false, false, b"hello"),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    let (_, code, f, _) = &out[0];
    assert_eq!(*code, Some(d::DISPOSITION));
    assert_eq!(at(f, 0), V::Boolean(true), "role receiver");
    assert_eq!(at(f, 1), V::Uint(7));
    assert_eq!(at(f, 3), V::Boolean(true), "settled");
    assert_eq!(decode_outcome(f.get(4)), Outcome::Accepted);
    let (link, delivery) = &app.deliveries[0];
    assert_eq!(link.handle, 0);
    assert_eq!(
        (
            delivery.delivery_id,
            delivery.tag.as_slice(),
            delivery.settled
        ),
        (7, &b"t7"[..], false)
    );
    assert_eq!(delivery.payload, b"hello");

    // The container rejects.
    app.outcome = Outcome::Rejected(Some(AmqpError::new(condition::DECODE_ERROR, "bad")));
    peer.frame(0, &transfer(0, 8, b"t8", false, false, b"x"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(decode_outcome(out[0].2.get(4)), app.outcome);
    // Settled by the sender: no disposition.
    peer.frame(0, &transfer(0, 9, b"t9", true, false, b"x"), 0, &mut app);
    assert!(output(&mut peer).is_empty());
    assert_eq!(app.deliveries.len(), 3);

    // Mode second (rcv-settle-mode 1).
    app.outcome = Outcome::Accepted;
    peer.frame(
        0,
        &body(
            d::ATTACH,
            vec![
                V::Str("second".into()),
                V::Uint(1),
                V::Boolean(false),
                V::Null,
                V::Ubyte(1),
                source("from"),
                target("q"),
                V::Null,
                V::Null,
                V::Uint(0),
            ],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(at(&out[0].2, 4), V::Ubyte(1), "the mode is kept");
    peer.frame(0, &transfer(1, 10, b"s", false, false, b"x"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(at(&out[0].2, 3), V::Boolean(false), "not settled yet");
    assert_eq!(decode_outcome(out[0].2.get(4)), Outcome::Accepted);
    // The same tag again with resume: this endpoint holds it unsettled,
    // so it is a delivery (§2.6.13).
    peer.frame(
        0,
        &body(
            d::DISPOSITION,
            vec![V::Boolean(false), V::Uint(10), V::Null, V::Boolean(true)],
        ),
        0,
        &mut app,
    );
    assert!(output(&mut peer).is_empty());

    // A transfer that asks for mode second on a link that settles first.
    let mut illegal = body(
        d::TRANSFER,
        vec![
            V::Uint(0),
            V::Uint(11),
            V::Binary(b"x".to_vec()),
            V::Uint(0),
            V::Boolean(false),
            V::Boolean(false),
            V::Ubyte(1),
        ],
    );
    illegal.push(b'x');
    peer.frame(0, &illegal, 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(error_of(&out[0], 2).condition, condition::INVALID_FIELD);
}

/// §2.6.12, the sending side: the receiver's disposition reaches the
/// container with the delivery tag; a terminal state the receiver has not
/// settled is settled by this endpoint with a disposition of the sender;
/// the tags of what stays unsettled are handed over when the link goes.
#[test]
fn amqp10_2_6_12_a_disposition_for_what_this_endpoint_sent_is_settled() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    peer.frame(0, &credit(0, 0, 10, false, false), 0, &mut app);
    for tag in [b"a", b"b", b"c", b"d"] {
        peer.send(id, tag, 0, b"m", false).expect("send");
    }
    let out = output(&mut peer);
    assert_eq!(at(&out[0].2, 4), V::Boolean(false), "unsettled");
    // Deliveries 0 and 1 accepted and settled by the receiver.
    peer.frame(
        0,
        &body(
            d::DISPOSITION,
            vec![
                V::Boolean(true),
                V::Uint(0),
                V::Uint(1),
                V::Boolean(true),
                zerodds_amqp_endpoint::terminus::encode_outcome(&Outcome::Accepted),
            ],
        ),
        0,
        &mut app,
    );
    assert!(output(&mut peer).is_empty());
    assert_eq!(
        app.dispositions,
        vec![
            (b"a".to_vec(), Outcome::Accepted, true),
            (b"b".to_vec(), Outcome::Accepted, true)
        ]
    );
    // Delivery 2 released, not settled: this endpoint settles.
    peer.frame(
        0,
        &body(
            d::DISPOSITION,
            vec![
                V::Boolean(true),
                V::Uint(2),
                V::Null,
                V::Boolean(false),
                zerodds_amqp_endpoint::terminus::encode_outcome(&Outcome::Released),
            ],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Some(d::DISPOSITION));
    assert_eq!(at(&out[0].2, 0), V::Boolean(false), "role sender");
    assert_eq!(at(&out[0].2, 1), V::Uint(2));
    assert_eq!(at(&out[0].2, 3), V::Boolean(true));
    assert_eq!(decode_outcome(out[0].2.get(4)), Outcome::Released);
    assert_eq!(
        app.dispositions[2],
        (b"c".to_vec(), Outcome::Released, false)
    );
    // The link is detached with delivery 3 unsettled.
    peer.frame(
        0,
        &body(d::DETACH, vec![V::Uint(0), V::Boolean(true)]),
        0,
        &mut app,
    );
    assert_eq!(
        app.detached,
        vec![("out".to_string(), vec![b"d".to_vec()], None)]
    );
    assert_eq!(output(&mut peer)[0].1, Some(d::DETACH));

    // Settle modes (§2.8.2): a link that settles always, one that never.
    for (mode, expect) in [(1u8, true), (0u8, false)] {
        peer.frame(
            0,
            &body(
                d::ATTACH,
                vec![
                    V::Str(format!("m{mode}")),
                    V::Uint(u32::from(mode)),
                    V::Boolean(true),
                    V::Ubyte(mode),
                    V::Null,
                    source("q"),
                    target("to"),
                ],
            ),
            0,
            &mut app,
        );
        let out = output(&mut peer);
        assert_eq!(at(&out[0].2, 3), V::Ubyte(mode));
        let id = LinkId {
            channel: 0,
            handle: u32::from(mode),
        };
        peer.frame(0, &credit(u32::from(mode), 0, 1, false, false), 0, &mut app);
        peer.send(id, b"t", 0, b"m", !expect).expect("send");
        let out = output(&mut peer);
        assert_eq!(at(&out[0].2, 4), V::Boolean(expect), "mode {mode}");
    }
}

/// §2.6.14: a message in several transfer frames is one delivery; the
/// continuation frames may leave delivery-id and tag out; an aborted
/// delivery is discarded; a second delivery before the end of the first
/// is refused.
#[test]
fn amqp10_2_6_14_a_message_in_several_transfers_is_one_delivery() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "in");
    peer.frame(0, &transfer(0, 0, b"t", false, true, b"one "), 0, &mut app);
    assert!(app.deliveries.is_empty());
    let mut middle = body(
        d::TRANSFER,
        vec![
            V::Uint(0),
            V::Null,
            V::Null,
            V::Null,
            V::Null,
            V::Boolean(true),
        ],
    );
    middle.extend_from_slice(b"two ");
    peer.frame(0, &middle, 0, &mut app);
    let mut last = body(d::TRANSFER, vec![V::Uint(0)]);
    last.extend_from_slice(b"three");
    peer.frame(0, &last, 0, &mut app);
    assert_eq!(app.deliveries.len(), 1);
    assert_eq!(app.deliveries[0].1.payload, b"one two three");
    assert_eq!(app.deliveries[0].1.tag, b"t");
    let out = output(&mut peer);
    assert_eq!(out.len(), 1, "one disposition for the delivery");
    assert_eq!(at(&out[0].2, 1), V::Uint(0));
    // Three frames took three of the session window, one of the credit.
    assert_eq!(peer.session_windows(0).expect("s").next_incoming_id, 3);
    assert_eq!(
        peer.link_flow(LinkId {
            channel: 0,
            handle: 0
        })
        .map(|f| f.delivery_count),
        Some(1)
    );

    // Aborted: the data before the abort is discarded, no delivery.
    peer.frame(0, &transfer(0, 1, b"u", false, true, b"half"), 0, &mut app);
    let aborted = body(
        d::TRANSFER,
        vec![
            V::Uint(0),
            V::Null,
            V::Null,
            V::Null,
            V::Null,
            V::Null,
            V::Null,
            V::Null,
            V::Null,
            V::Boolean(true),
        ],
    );
    peer.frame(0, &aborted, 0, &mut app);
    assert_eq!(app.deliveries.len(), 1);
    assert!(output(&mut peer).is_empty());
    // The next delivery is whole.
    peer.frame(0, &transfer(0, 2, b"v", true, false, b"whole"), 0, &mut app);
    assert_eq!(app.deliveries[1].1.payload, b"whole");

    // Interleaved on one link.
    peer.frame(0, &transfer(0, 3, b"w", true, true, b"a"), 0, &mut app);
    peer.frame(0, &transfer(0, 4, b"x", true, false, b"b"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(error_of(&out[0], 2).condition, condition::NOT_ALLOWED);
}

/// §2.6.14, the sending side: a message above the agreed frame size goes
/// out in several transfer frames, none larger than the partner's
/// max-frame-size, `more` set on all but the last; the payloads are the
/// message.
#[test]
fn amqp10_2_6_14_a_message_above_the_frame_size_is_sent_in_several_transfers() {
    let mut app = App::new();
    let mut peer = Peer::new(PeerConfig::default(), 0);
    let mut open = open_fields();
    open.extend([V::Null, V::Uint(512)]);
    peer.frame(0, &body(d::OPEN, open), 0, &mut app);
    output(&mut peer);
    begin(&mut peer, &mut app, 0);
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    peer.frame(0, &credit(0, 0, 1, false, false), 0, &mut app);
    let message: Vec<u8> = (0..1500u32).map(|i| (i % 251) as u8).collect();
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    peer.send(id, b"big", 0, &message, false).expect("send");
    let mut raw = Vec::new();
    while let Some(frame) = peer.poll_output() {
        assert!(frame.len() <= 512, "frame of {} octets", frame.len());
        raw.push(frame);
    }
    assert!(raw.len() >= 3, "{} frames", raw.len());
    let mut whole = Vec::new();
    for (i, frame) in raw.iter().enumerate() {
        let (code, value, used) = decode_performative(&frame[8..]).expect("transfer");
        assert_eq!(code, d::TRANSFER);
        let V::List(f) = value else { panic!("a list") };
        assert_eq!(at(&f, 1), V::Uint(0), "one delivery-id");
        assert_eq!(at(&f, 5), V::Boolean(i + 1 < raw.len()), "more");
        whole.extend_from_slice(&frame[8 + used..]);
    }
    assert_eq!(whole, message);
    let w = peer.session_windows(0).expect("s");
    assert_eq!(w.next_outgoing_id as usize, raw.len());
    // One delivery took one credit.
    assert_eq!(peer.link_flow(id).map(|f| f.delivery_count), Some(1));
    // The partner's max-message-size is kept (attach field 10).
    peer.frame(
        0,
        &body(
            d::ATTACH,
            vec![
                V::Str("small".into()),
                V::Uint(1),
                V::Boolean(true),
                V::Null,
                V::Null,
                source("q"),
                target("to"),
                V::Null,
                V::Null,
                V::Null,
                V::Ulong(10),
            ],
        ),
        0,
        &mut app,
    );
    peer.frame(0, &credit(1, 0, 1, false, false), 0, &mut app);
    output(&mut peer);
    let small = LinkId {
        channel: 0,
        handle: 1,
    };
    assert_eq!(
        peer.send(small, b"t", 0, &[0u8; 11], true),
        Err(SendError::MessageTooLarge)
    );
    assert_eq!(
        peer.send(small, &[0u8; 33], 0, b"x", true),
        Err(SendError::Tag)
    );
    assert!(output(&mut peer).is_empty());
}

/// §2.8.18 message-size-exceeded: a message above this endpoint's
/// max-message-size detaches the link.
#[test]
fn amqp10_2_8_18_a_message_above_max_message_size_detaches_the_link() {
    let (mut peer, mut app) = opened(PeerConfig {
        max_message_size: 8,
        ..PeerConfig::default()
    });
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "in");
    peer.frame(0, &transfer(0, 0, b"t", false, true, b"12345"), 0, &mut app);
    peer.frame(0, &transfer(0, 0, b"t", false, false, b"6789"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(
        error_of(&out[0], 2).condition,
        condition::MESSAGE_SIZE_EXCEEDED
    );
    assert!(app.deliveries.is_empty());
}

/// §2.6.13: a delivery sent with `resume` that this endpoint does not
/// hold unsettled "MUST be ignored by the receiver".
#[test]
fn amqp10_2_6_13_a_resumed_delivery_this_endpoint_does_not_hold_is_ignored() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "in");
    let mut resumed = body(
        d::TRANSFER,
        vec![
            V::Uint(0),
            V::Uint(0),
            V::Binary(b"old".to_vec()),
            V::Uint(0),
            V::Boolean(false),
            V::Boolean(false),
            V::Null,
            V::Null,
            V::Boolean(true),
        ],
    );
    resumed.extend_from_slice(b"again");
    peer.frame(0, &resumed, 0, &mut app);
    assert!(app.deliveries.is_empty());
    assert!(output(&mut peer).is_empty());
    // Without resume the same frame is a delivery.
    peer.frame(0, &transfer(0, 1, b"new", false, false, b"m"), 0, &mut app);
    assert_eq!(app.deliveries.len(), 1);
}

/// §2.4.3 from this side: the connection lost or closed by this endpoint
/// hands every link back with what was unsettled.
#[test]
fn amqp10_2_4_3_a_lost_connection_hands_the_links_back() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    peer.frame(0, &credit(0, 0, 5, false, false), 0, &mut app);
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    peer.send(id, b"k", 0, b"m", false).expect("send");
    peer.connection_lost(&mut app);
    assert!(peer.is_ended());
    assert_eq!(
        app.detached,
        vec![("out".to_string(), vec![b"k".to_vec()], None)]
    );
    assert_eq!(peer.send(id, b"l", 0, b"m", false), Err(SendError::NoLink));
}

/// §2.8.19 MIN-MAX-FRAME-SIZE with §2.7.1: "Both peers MUST accept frames
/// of up to 512 (MIN-MAX-FRAME-SIZE) octets." A partner that states less
/// is refused; one that states 512 gets frames of at most 512 octets, and
/// this endpoint never states less than 512 itself.
#[test]
fn amqp10_2_8_19_the_frame_size_is_never_below_512() {
    let mut app = App::new();
    let mut peer = Peer::new(
        PeerConfig {
            max_frame_size: 100,
            ..PeerConfig::default()
        },
        0,
    );
    let mut open = open_fields();
    open.extend([V::Null, V::Uint(511)]);
    peer.frame(0, &body(d::OPEN, open), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::OPEN));
    assert_eq!(
        at(&out[0].2, 2),
        V::Uint(512),
        "the floor of what is stated"
    );
    assert_eq!(out[1].1, Some(d::CLOSE));
    assert_eq!(error_of(&out[1], 0).condition, condition::INVALID_FIELD);

    let mut peer = Peer::new(PeerConfig::default(), 0);
    let mut open = open_fields();
    open.extend([V::Null, V::Uint(512)]);
    peer.frame(0, &body(d::OPEN, open), 0, &mut app);
    output(&mut peer);
    assert_eq!(peer.remote_max_frame_size(), 512);
}

/// A flow of the partner with its session window and, for the link on
/// `handle`, its delivery-count and credit.
fn flow(
    next_incoming_id: u32,
    incoming_window: u32,
    handle: u32,
    delivery_count: u32,
    link_credit: u32,
    echo: bool,
) -> Vec<u8> {
    body(
        d::FLOW,
        vec![
            V::Uint(next_incoming_id),
            V::Uint(incoming_window),
            V::Uint(0),
            V::Uint(100),
            V::Uint(handle),
            V::Uint(delivery_count),
            V::Uint(link_credit),
            V::Null,
            V::Boolean(false),
            V::Boolean(echo),
        ],
    )
}

/// The transfer frames in `out`: delivery-id, `more` and payload of each.
fn transfers(out: &[Out]) -> Vec<(V, V, Vec<u8>)> {
    out.iter()
        .map(|(_, code, fields, payload)| {
            assert_eq!(*code, Some(d::TRANSFER), "{fields:?}");
            (at(fields, 1), at(fields, 5), payload.clone())
        })
        .collect()
}

/// Opens the connection with a partner that takes frames of 512 octets
/// and begins a session whose partner takes two transfer frames
/// (incoming window 2, §2.5.6).
fn open_512_octet_frames_two_transfers(peer: &mut Peer, app: &mut App) {
    let mut open = open_fields();
    open.extend([V::Null, V::Uint(512)]);
    peer.frame(0, &body(d::OPEN, open), 0, app);
    output(peer);
    peer.frame(
        0,
        &body(
            d::BEGIN,
            vec![V::Null, V::Uint(0), V::Uint(2), V::Uint(100)],
        ),
        0,
        app,
    );
    output(peer);
}

/// §2.5.6: the incoming window counts transfer frames ("sending a
/// transfer": remote-incoming-window is decremented with each one). A
/// message of five frames for a partner whose window is two goes out two
/// frames at a time, each time a flow opens the window; §2.6.14: no other
/// delivery of the link comes between its frames.
#[test]
fn amqp10_2_5_6_a_delivery_above_the_window_is_continued_after_each_flow() {
    let mut app = App::new();
    let mut peer = Peer::new(PeerConfig::default(), 0);
    open_512_octet_frames_two_transfers(&mut peer, &mut app);
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    attach_receiver(&mut peer, &mut app, 0, 1, "other");
    peer.frame(0, &flow(0, 2, 0, 0, 5, false), 0, &mut app);
    peer.frame(0, &flow(0, 2, 1, 0, 5, false), 0, &mut app);
    assert!(output(&mut peer).is_empty());
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    let other = LinkId {
        channel: 0,
        handle: 1,
    };
    let message: Vec<u8> = (0..2200u32).map(|i| (i % 251) as u8).collect();
    assert_eq!(peer.send(id, b"big", 0, &message, false), Ok(0));
    let mut sent = transfers(&output(&mut peer));
    assert_eq!(sent.len(), 2, "the window admits two frames");
    assert!(peer.is_sending(id));
    let w = peer.session_windows(0).expect("a session");
    assert_eq!((w.next_outgoing_id, w.remote_incoming_window), (2, 0));
    // The delivery has its credit and its delivery-id from the first frame.
    assert_eq!(
        peer.link_flow(id)
            .map(|f| (f.delivery_count, f.link_credit)),
        Some((1, 4))
    );
    // Nothing starts while the window is closed, on this link or another.
    assert!(peer.sendable().is_empty());
    assert_eq!(
        peer.send(id, b"next", 0, b"x", false),
        Err(SendError::InProgress)
    );
    assert_eq!(
        peer.send(other, b"o", 0, b"x", false),
        Err(SendError::Window)
    );
    assert!(output(&mut peer).is_empty());
    // The partner has taken the two and takes two more.
    peer.frame(0, &flow(2, 2, 0, 0, 5, false), 0, &mut app);
    let more = transfers(&output(&mut peer));
    assert_eq!(more.len(), 2);
    sent.extend(more);
    assert!(peer.is_sending(id));
    // A session flow without a handle opens the window as well: the rest.
    peer.frame(
        0,
        &body(
            d::FLOW,
            vec![V::Uint(4), V::Uint(10), V::Uint(0), V::Uint(100)],
        ),
        0,
        &mut app,
    );
    let rest = transfers(&output(&mut peer));
    assert_eq!(rest.len(), 1, "the fifth frame");
    sent.extend(rest);
    assert!(!peer.is_sending(id));
    assert_eq!(sent.len(), 5);
    let mut whole = Vec::new();
    for (i, (delivery_id, more, payload)) in sent.iter().enumerate() {
        assert_eq!(*delivery_id, V::Uint(0), "one delivery");
        assert_eq!(*more, V::Boolean(i < 4), "more on all but the last");
        whole.extend_from_slice(payload);
    }
    assert_eq!(whole, message);
    let w = peer.session_windows(0).expect("a session");
    assert_eq!((w.next_outgoing_id, w.remote_incoming_window), (5, 9));
    // The link takes the next delivery, and so does the other one.
    assert_eq!(peer.sendable().len(), 2);
    assert_eq!(peer.send(id, b"next", 0, b"x", false), Ok(1));
    assert_eq!(peer.send(other, b"o", 0, b"y", false), Ok(2));
    assert_eq!(transfers(&output(&mut peer)).len(), 2);
    // A link detached in the middle of a delivery sends nothing more of it.
    let long: Vec<u8> = vec![7; 6000];
    assert_eq!(peer.send(id, b"cut", 0, &long, false), Ok(3));
    assert_eq!(transfers(&output(&mut peer)).len(), 7);
    assert!(peer.is_sending(id));
    peer.frame(
        0,
        &body(d::DETACH, vec![V::Uint(0), V::Boolean(true)]),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Some(d::DETACH));
    peer.frame(
        0,
        &body(
            d::FLOW,
            vec![V::Uint(12), V::Uint(10), V::Uint(0), V::Uint(100)],
        ),
        0,
        &mut app,
    );
    assert!(output(&mut peer).is_empty(), "no frame of the cut delivery");
    assert_eq!(
        app.detached.last().map(|d| d.1.clone()),
        Some(vec![b"big".to_vec(), b"next".to_vec(), b"cut".to_vec()]),
        "what the link left unsettled, the cut delivery among it"
    );
}

/// §2.6.7 with §2.5.6: a link asked to drain is not drained while a
/// delivery of it waits for the window; its credit is not given up for a
/// message that is still on its way.
#[test]
fn amqp10_2_6_7_a_link_in_the_middle_of_a_delivery_is_not_drained() {
    let mut app = App::new();
    let mut peer = Peer::new(PeerConfig::default(), 0);
    let mut open = open_fields();
    open.extend([V::Null, V::Uint(512)]);
    peer.frame(0, &body(d::OPEN, open), 0, &mut app);
    output(&mut peer);
    peer.frame(
        0,
        &body(
            d::BEGIN,
            vec![V::Null, V::Uint(0), V::Uint(1), V::Uint(100)],
        ),
        0,
        &mut app,
    );
    output(&mut peer);
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    // Credit 3 and drain.
    peer.frame(
        0,
        &body(
            d::FLOW,
            vec![
                V::Uint(0),
                V::Uint(1),
                V::Uint(0),
                V::Uint(100),
                V::Uint(0),
                V::Uint(0),
                V::Uint(3),
                V::Null,
                V::Boolean(true),
            ],
        ),
        0,
        &mut app,
    );
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    assert_eq!(peer.send(id, b"t", 0, &[1u8; 1000], false), Ok(0));
    assert_eq!(transfers(&output(&mut peer)).len(), 1);
    peer.drained(id, &mut app);
    assert!(output(&mut peer).is_empty(), "the delivery is not complete");
    assert_eq!(
        peer.link_flow(id)
            .map(|f| (f.delivery_count, f.link_credit)),
        Some((1, 2))
    );
}

/// §2.7.4 available: "The number of messages awaiting credit at the link
/// sender endpoint. Only the sender can independently set this value."
/// A flow this endpoint sends for a link it sends on states the count of
/// the container; for a link it receives on the field is null.
#[test]
fn amqp10_2_7_4_a_flow_of_the_sender_states_the_messages_that_wait_for_credit() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    app.available = 3;
    // No credit, and the partner asks for the flow state.
    peer.frame(0, &credit(0, 0, 0, false, true), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Some(d::FLOW));
    assert_eq!(at(&out[0].2, 6), V::Uint(0), "link-credit");
    assert_eq!(at(&out[0].2, 7), V::Uint(3), "available");
    // Drained with one message left that the link may not get.
    peer.frame(0, &credit(0, 0, 2, true, false), 0, &mut app);
    app.available = 1;
    peer.drained(
        LinkId {
            channel: 0,
            handle: 0,
        },
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::FLOW));
    assert_eq!(at(&out[0].2, 5), V::Uint(2), "delivery-count");
    assert_eq!(at(&out[0].2, 7), V::Uint(1), "available");
    // A link this endpoint receives on: the flow that grants the credit
    // and the echo leave the field to the sender.
    let out = attach_sender(&mut peer, &mut app, 0, 1, "in");
    assert_eq!(out[1].1, Some(d::FLOW));
    assert_eq!(at(&out[1].2, 7), V::Null);
    peer.frame(
        0,
        &body(
            d::FLOW,
            vec![
                V::Uint(0),
                V::Uint(100),
                V::Uint(0),
                V::Uint(100),
                V::Uint(1),
                V::Uint(0),
                V::Null,
                V::Uint(9),
                V::Null,
                V::Boolean(true),
            ],
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::FLOW));
    assert_eq!(at(&out[0].2, 7), V::Null);
}

/// The partner attaches as the sender of a link that settles second.
fn attach_sender_second(peer: &mut Peer, app: &mut App, handle: u32, name: &str) -> Vec<Out> {
    peer.frame(
        0,
        &body(
            d::ATTACH,
            vec![
                V::Str(name.into()),
                V::Uint(handle),
                V::Boolean(false),
                V::Ubyte(0),
                V::Ubyte(1),
                source("from"),
                target("q"),
                V::Null,
                V::Null,
                V::Uint(0),
            ],
        ),
        0,
        app,
    );
    output(peer)
}

fn disposition(from_receiver: bool, first: u32, last: Option<u32>, settled: bool) -> Vec<u8> {
    body(
        d::DISPOSITION,
        vec![
            V::Boolean(from_receiver),
            V::Uint(first),
            last.map_or(V::Null, V::Uint),
            V::Boolean(settled),
            zerodds_amqp_endpoint::terminus::encode_outcome(&Outcome::Accepted),
        ],
    )
}

/// §2.6.5: "Should any input (other than a detach) related to the endpoint
/// either via the input handle or delivery-ids be received, the session
/// MUST be terminated with an errant-link session-error." The delivery-ids:
/// a disposition of the partner that names a delivery this endpoint held
/// unsettled on a link it destroyed with an error ends the session; one
/// for another link's delivery does not, and neither does one after the
/// partner answered the detach.
#[test]
fn amqp10_2_6_5_a_disposition_for_a_link_detached_with_an_error_ends_the_session() {
    // A link this endpoint receives on, settling second: delivery 0 stays
    // unsettled; delivery 1 is above max-message-size and destroys it.
    let destroyed_receiver = || {
        let (mut peer, mut app) = opened(PeerConfig {
            max_message_size: 8,
            ..PeerConfig::default()
        });
        begin(&mut peer, &mut app, 0);
        attach_sender_second(&mut peer, &mut app, 0, "in");
        attach_sender_second(&mut peer, &mut app, 1, "kept");
        peer.frame(0, &transfer(0, 0, b"a", false, false, b"m"), 0, &mut app);
        peer.frame(0, &transfer(1, 1, b"k", false, false, b"m"), 0, &mut app);
        let out = output(&mut peer);
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0].1, Some(d::DISPOSITION));
        assert_eq!(at(&out[0].2, 3), V::Boolean(false), "not settled yet");
        peer.frame(
            0,
            &transfer(0, 2, b"b", false, false, b"123456789"),
            0,
            &mut app,
        );
        let out = output(&mut peer);
        assert_eq!(out[0].1, Some(d::DETACH));
        assert_eq!(
            error_of(&out[0], 2).condition,
            condition::MESSAGE_SIZE_EXCEEDED
        );
        (peer, app)
    };
    // The sender settles delivery 0 of the destroyed link.
    let (mut peer, mut app) = destroyed_receiver();
    peer.frame(0, &disposition(false, 0, None, true), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0].1, Some(d::END));
    let error = error_of(&out[0], 0);
    assert_eq!(error.condition, condition::ERRANT_LINK);
    assert!(
        error
            .description
            .as_deref()
            .is_some_and(|t| t.contains("delivery 0")),
        "{error:?}"
    );
    // A range that covers it among others.
    let (mut peer, mut app) = destroyed_receiver();
    peer.frame(0, &disposition(false, 0, Some(1), true), 0, &mut app);
    assert_eq!(output(&mut peer)[0].1, Some(d::END));
    // The delivery of the other link alone: no input for the destroyed one.
    let (mut peer, mut app) = destroyed_receiver();
    peer.frame(0, &disposition(false, 1, None, true), 0, &mut app);
    assert!(output(&mut peer).is_empty());
    // The same number in the other direction is another delivery.
    peer.frame(0, &disposition(true, 0, None, true), 0, &mut app);
    assert!(output(&mut peer).is_empty());
    // After the partner's detach the link is gone on both sides.
    peer.frame(
        0,
        &body(d::DETACH, vec![V::Uint(0), V::Boolean(true)]),
        0,
        &mut app,
    );
    peer.frame(0, &disposition(false, 0, None, true), 0, &mut app);
    assert!(output(&mut peer).is_empty());

    // A link this endpoint sends on: delivery 0 is unsettled when a
    // transfer of the partner on it destroys the link.
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    peer.frame(0, &credit(0, 0, 5, false, false), 0, &mut app);
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    assert_eq!(peer.send(id, b"t", 0, b"m", false), Ok(0));
    output(&mut peer);
    peer.frame(0, &transfer(0, 0, b"x", true, false, b"m"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(error_of(&out[0], 2).condition, condition::NOT_ALLOWED);
    assert_eq!(
        app.detached.last().map(|d| d.1.clone()),
        Some(vec![b"t".to_vec()])
    );
    // The receiver's disposition for the delivery of the destroyed link.
    peer.frame(0, &disposition(true, 0, None, true), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::END));
    assert_eq!(error_of(&out[0], 0).condition, condition::ERRANT_LINK);
    assert!(app.dispositions.is_empty(), "the container hears of none");
}

/// §3.5.4 with §2.6.3: a value under the descriptor of a target that is
/// not one (durable 7) is refused with amqp:invalid-field and the name of
/// the field, in both roles; a described value of another type (Part 4:
/// a coordinator, 0x30) is handed to the container as it came.
#[test]
fn amqp10_3_5_4_a_malformed_target_is_refused_with_the_field() {
    let malformed = V::Described {
        descriptor: 0x29,
        value: Box::new(V::List(vec![V::Str("q".into()), V::Uint(7)])),
    };
    let attach = |role_receiver: bool, handle: u32, target: V| {
        body(
            d::ATTACH,
            vec![
                V::Str(format!("L{handle}")),
                V::Uint(handle),
                V::Boolean(role_receiver),
                V::Null,
                V::Null,
                source("q"),
                target,
                V::Null,
                V::Null,
                V::Uint(0),
            ],
        )
    };
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    for (handle, role_receiver) in [(0, false), (1, true)] {
        peer.frame(
            0,
            &attach(role_receiver, handle, malformed.clone()),
            0,
            &mut app,
        );
        let out = output(&mut peer);
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0].1, Some(d::ATTACH));
        assert_eq!(out[1].1, Some(d::DETACH));
        let error = error_of(&out[1], 2);
        assert_eq!(error.condition, condition::INVALID_FIELD);
        assert!(
            error
                .description
                .as_deref()
                .is_some_and(|t| t.contains("target.durable")),
            "{error:?}"
        );
    }
    assert!(app.attached.is_empty(), "the container is not asked");
    // A described value of another type is no malformed target: the
    // container judges it.
    let other = V::Described {
        descriptor: 0x1234,
        value: Box::new(V::List(vec![])),
    };
    peer.frame(0, &attach(false, 2, other.clone()), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::ATTACH));
    assert_eq!(app.attached.len(), 1);
    assert_eq!(app.attached[0].target, None);
    assert_eq!(app.attached[0].coordinator, None);
    assert_eq!(app.attached[0].target_value, Some(other.clone()));
    assert_eq!(at(&out[0].2, 6), other);
}

/// §1.6.24 with §2.6.3: a value the partner sent is echoed as it came. A
/// refused attach whose source holds an empty array (capabilities without
/// an element) is answered with attach and a closing detach; the session
/// and the connection live on.
#[test]
fn amqp10_2_6_3_a_refused_attach_with_an_empty_array_is_answered() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    app.refuse = Some(AmqpError::new(condition::NOT_FOUND, "no node q"));
    let mut fields = vec![V::Null; 11];
    fields[0] = V::Str("q".into());
    fields[10] = V::Array(Vec::new());
    let with_empty_array = V::Described {
        descriptor: 0x28,
        value: Box::new(V::List(fields)),
    };
    // On the wire the array has a symbol constructor: e0 02 00 a3.
    let mut frame = body(
        d::ATTACH,
        vec![
            V::Str("in".into()),
            V::Uint(0),
            V::Boolean(false),
            V::Null,
            V::Null,
            with_empty_array.clone(),
            target("q"),
            V::Null,
            V::Null,
            V::Uint(0),
        ],
    );
    let at_array = frame
        .windows(4)
        .position(|w| w == [0xe0, 0x02, 0x00, 0x40])
        .expect("the empty array");
    frame[at_array + 3] = 0xa3;
    peer.frame(0, &frame, 0, &mut app);
    assert!(peer.fault().is_none(), "{:?}", peer.fault());
    let out = output(&mut peer);
    assert_eq!(out.len(), 2, "{out:?}");
    assert_eq!(out[0].1, Some(d::ATTACH));
    assert_eq!(at(&out[0].2, 5), with_empty_array, "the source as it came");
    assert_eq!(out[1].1, Some(d::DETACH));
    assert_eq!(error_of(&out[1], 2).condition, condition::NOT_FOUND);
    assert!(!peer.is_ended());
    // The connection is usable: the next attach is answered.
    app.refuse = None;
    let out = attach_sender(&mut peer, &mut app, 0, 1, "next");
    assert_eq!(out[0].1, Some(d::ATTACH));
    assert_eq!(app.attached.len(), 1);
}

// ============================================================================
// Part 4 (Transactions): what the transport carries of it.
// ============================================================================

fn txn(octets: &[u8]) -> TxnId {
    TxnId::new(octets).expect("a txn-id")
}

fn in_txn(id: &TxnId, outcome: Option<Outcome>) -> DeliveryState {
    DeliveryState::Transactional(TransactionalState {
        txn_id: id.clone(),
        outcome,
    })
}

/// A transfer with a `state` (field 7).
fn transfer_with_state(
    handle: u32,
    id: u32,
    tag: &[u8],
    settled: bool,
    more: bool,
    state: V,
    payload: &[u8],
) -> Vec<u8> {
    let mut b = body(
        d::TRANSFER,
        vec![
            V::Uint(handle),
            V::Uint(id),
            V::Binary(tag.to_vec()),
            V::Uint(0),
            V::Boolean(settled),
            V::Boolean(more),
            V::Null,
            state,
        ],
    );
    b.extend_from_slice(payload);
    b
}

fn attach_to_coordinator(handle: u32, name: &str, coordinator: V) -> Vec<u8> {
    body(
        d::ATTACH,
        vec![
            V::Str(name.into()),
            V::Uint(handle),
            V::Boolean(false),
            V::Null,
            V::Null,
            source("ctl"),
            coordinator,
            V::Null,
            V::Null,
            V::Uint(0),
        ],
    )
}

/// Part 4 §4.5.1: a target that is a coordinator reaches the container
/// with the capabilities the partner desires, and the attach is answered
/// with the coordinator of the container ("When sent by the resource ...
/// defined the actual capabilities of the coordinator"). A coordinator
/// with a capability that is none is refused with amqp:invalid-field.
#[test]
fn amqp10_4_5_1_a_coordinator_target_is_answered_with_the_capabilities_of_the_container() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    let own = Coordinator {
        capabilities: vec![
            TxnCapability::LocalTransactions,
            TxnCapability::MultiTxnsPerSsn,
        ],
    };
    app.coordinator = Some(own.clone());
    let desired = Coordinator {
        capabilities: vec![TxnCapability::LocalTransactions],
    };
    peer.frame(
        0,
        &attach_to_coordinator(0, "txn-ctl", desired.encode()),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::ATTACH));
    assert_eq!(out[1].1, Some(d::FLOW), "the receiver grants credit");
    assert_eq!(app.attached.len(), 1);
    assert_eq!(app.attached[0].coordinator, Some(desired));
    assert_eq!(app.attached[0].target, None);
    assert_eq!(app.attached[0].target_value, None);
    assert_eq!(at(&out[0].2, 6), own.encode());
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    assert_eq!(
        peer.link(id).expect("the link").coordinator,
        Some(own),
        "the link holds the coordinator as created"
    );
    // A capability outside §4.5.7.
    let malformed = V::Described {
        descriptor: 0x30,
        value: Box::new(V::List(vec![V::Symbol("amqp:nested-transactions".into())])),
    };
    peer.frame(0, &attach_to_coordinator(1, "bad", malformed), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 2, "{out:?}");
    assert_eq!(out[1].1, Some(d::DETACH));
    let error = error_of(&out[1], 2);
    assert_eq!(error.condition, condition::INVALID_FIELD);
    assert!(
        error
            .description
            .as_deref()
            .is_some_and(|t| t.contains("amqp:nested-transactions")),
        "{error:?}"
    );
    assert_eq!(app.attached.len(), 1, "the container is not asked");
}

/// Part 4 §4.4.1, §4.5.6: the state of a transfer reaches the container
/// as a delivery state, and its answer is the state of the disposition. A
/// delivery in several frames is announced with its state when it begins;
/// a later frame without the state of the first is marked, and so is a
/// later frame with a state where the first had none; a transaction a
/// later frame names is announced and reaches the container with the
/// delivery. A transactional-state without a txn-id, on the first or on a
/// later frame, destroys the link with amqp:invalid-field.
#[test]
fn amqp10_4_4_1_the_state_of_a_transfer_reaches_the_container_and_its_answer_the_sender() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "in");
    let t = txn(&[0, 1]);
    let posted = in_txn(&t, None);
    let provisional = in_txn(&t, Some(Outcome::Accepted));
    app.answer = Some(Ok(Answer::State(Some(provisional.clone()))));
    peer.frame(
        0,
        &transfer_with_state(0, 0, b"a", false, false, encode_state(&posted), b"m"),
        0,
        &mut app,
    );
    assert_eq!(app.deliveries[0].1.state, Some(posted.clone()));
    assert!(!app.deliveries[0].1.state_differs);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DISPOSITION));
    assert_eq!(at(&out[0].2, 1), V::Uint(0));
    assert_eq!(at(&out[0].2, 3), V::Boolean(true), "mode first settles");
    assert_eq!(decode_state(out[0].2.get(4)), Ok(Some(provisional)));
    assert!(app.begun.is_empty(), "one frame is no partial delivery");
    // Two frames, both with the state.
    peer.frame(
        0,
        &transfer_with_state(0, 1, b"b", false, true, encode_state(&posted), b"12"),
        0,
        &mut app,
    );
    assert_eq!(app.begun, vec![(1, Some(posted.clone()))]);
    assert_eq!(app.deliveries.len(), 1, "not yet a delivery");
    peer.frame(
        0,
        &transfer_with_state(0, 1, b"b", false, false, encode_state(&posted), b"34"),
        0,
        &mut app,
    );
    assert_eq!(app.deliveries[1].1.payload, b"1234");
    assert!(!app.deliveries[1].1.state_differs);
    // Two frames, the second without the state.
    peer.frame(
        0,
        &transfer_with_state(0, 2, b"c", false, true, encode_state(&posted), b"12"),
        0,
        &mut app,
    );
    peer.frame(0, &transfer(0, 2, b"c", false, false, b"34"), 0, &mut app);
    assert_eq!(app.deliveries[2].1.state, Some(posted.clone()));
    assert!(app.deliveries[2].1.state_differs);
    // Aborted after it began.
    peer.frame(
        0,
        &transfer_with_state(0, 3, b"d", false, true, encode_state(&posted), b"12"),
        0,
        &mut app,
    );
    let mut abort = vec![V::Null; 10];
    abort[0] = V::Uint(0);
    abort[9] = V::Boolean(true);
    peer.frame(0, &body(d::TRANSFER, abort), 0, &mut app);
    assert_eq!(app.begun.len(), 3);
    assert_eq!(app.aborted, vec![3]);
    assert_eq!(app.deliveries.len(), 3);
    assert!(app.named.is_empty());
    assert_eq!(app.deliveries[0].1.txns, vec![t.clone()]);
    assert_eq!(app.deliveries[2].1.txns, vec![t.clone()]);
    output(&mut peer);
    // Three frames: the first without a state, the second and the third
    // each naming another transaction.
    let u = txn(&[0, 2]);
    peer.frame(0, &transfer(0, 4, b"f", false, true, b"12"), 0, &mut app);
    peer.frame(
        0,
        &transfer_with_state(0, 4, b"f", false, true, encode_state(&posted), b"34"),
        0,
        &mut app,
    );
    assert_eq!(app.begun[3], (4, None));
    assert_eq!(app.named, vec![(4, t.clone())], "announced before the end");
    peer.frame(
        0,
        &transfer_with_state(
            0,
            4,
            b"f",
            false,
            false,
            encode_state(&in_txn(&u, None)),
            b"56",
        ),
        0,
        &mut app,
    );
    assert_eq!(app.named.len(), 1, "the last frame ends the delivery");
    let late = &app.deliveries[3].1;
    assert_eq!(late.payload, b"123456");
    assert_eq!(late.state, None);
    assert!(late.state_differs);
    assert_eq!(late.txns, vec![t.clone(), u]);
    output(&mut peer);
    // A transactional-state without the mandatory txn-id.
    let no_id = V::Described {
        descriptor: 0x34,
        value: Box::new(V::List(vec![])),
    };
    peer.frame(
        0,
        &transfer_with_state(0, 5, b"e", false, false, no_id.clone(), b"m"),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(error_of(&out[0], 2).condition, condition::INVALID_FIELD);
    assert_eq!(app.deliveries.len(), 4);
    // On a later frame.
    attach_sender(&mut peer, &mut app, 0, 1, "in2");
    peer.frame(0, &transfer(1, 6, b"g", false, true, b"12"), 0, &mut app);
    peer.frame(
        0,
        &transfer_with_state(1, 6, b"g", false, false, no_id, b"34"),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    let detach = out
        .iter()
        .find(|f| f.1 == Some(d::DETACH))
        .expect("the link is destroyed");
    let error = error_of(detach, 2);
    assert_eq!(error.condition, condition::INVALID_FIELD);
    assert!(
        error
            .description
            .as_deref()
            .is_some_and(|t| t.contains("transfer.state")),
        "{error:?}"
    );
    assert_eq!(app.deliveries.len(), 4, "no delivery");
}

/// Part 2 §2.6.5 for a container: an error in place of the answer to a
/// delivery destroys the link with it, and no disposition is sent
/// (Part 4 §4.2: a coordinator that cannot reject detaches with the
/// transaction error). An answer that comes later is the disposition
/// then.
#[test]
fn amqp10_4_2_a_delivery_is_answered_with_an_error_of_the_link_or_later() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_sender(&mut peer, &mut app, 0, 0, "in");
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    // Later.
    app.answer = Some(Ok(Answer::Later));
    peer.frame(0, &transfer(0, 0, b"a", false, false, b"m"), 0, &mut app);
    assert!(output(&mut peer).is_empty(), "no disposition yet");
    let declared = DeliveryState::Declared(txn(&[9]));
    assert!(
        !peer.answer(id, 7, Some(&declared)),
        "no such delivery waits"
    );
    assert!(peer.answer(id, 0, Some(&declared)));
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DISPOSITION));
    assert_eq!(at(&out[0].2, 0), V::Boolean(true), "role receiver");
    assert_eq!(at(&out[0].2, 1), V::Uint(0));
    assert_eq!(at(&out[0].2, 3), V::Boolean(true));
    assert_eq!(decode_state(out[0].2.get(4)), Ok(Some(declared.clone())));
    assert!(!peer.answer(id, 0, Some(&declared)), "answered once");
    // The sender settles before the answer: none is sent.
    peer.frame(0, &transfer(0, 1, b"b", false, false, b"m"), 0, &mut app);
    peer.frame(
        0,
        &body(
            d::DISPOSITION,
            vec![V::Boolean(false), V::Uint(1), V::Null, V::Boolean(true)],
        ),
        0,
        &mut app,
    );
    assert!(!peer.answer(id, 1, Some(&declared)));
    assert!(output(&mut peer).is_empty());
    // An error.
    app.answer = Some(Err(AmqpError::new(
        "amqp:transaction:unknown-id",
        "no transaction 07",
    )));
    peer.frame(0, &transfer(0, 2, b"c", false, false, b"m"), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(at(&out[0].2, 1), V::Boolean(true), "closed");
    assert_eq!(
        error_of(&out[0], 2).condition,
        "amqp:transaction:unknown-id"
    );
    assert_eq!(app.detached.len(), 1);
    assert!(peer.link(id).is_none());
}

/// Part 4 §4.4.2: a disposition with a transactional-state for a delivery
/// this endpoint sent reaches the container with the state; the delivery
/// stays unsettled until the container settles it. An error of the
/// container destroys the link. A transactional-state without a txn-id in
/// a disposition closes the connection with amqp:invalid-field.
#[test]
fn amqp10_4_4_2_a_transactional_disposition_leaves_the_delivery_unsettled_until_it_is_settled() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    peer.frame(0, &credit(0, 0, 10, false, false), 0, &mut app);
    for tag in [b"a", b"b", b"c"] {
        peer.send(id, tag, 0, b"m", false).expect("send");
    }
    output(&mut peer);
    let t = txn(&[4]);
    let retire = in_txn(&t, Some(Outcome::Accepted));
    let disposition = |first: u32, settled: bool, state: V| {
        body(
            d::DISPOSITION,
            vec![
                V::Boolean(true),
                V::Uint(first),
                V::Null,
                V::Boolean(settled),
                state,
            ],
        )
    };
    peer.frame(
        0,
        &disposition(0, false, encode_state(&retire)),
        0,
        &mut app,
    );
    assert_eq!(
        app.states,
        vec![(b"a".to_vec(), Some(retire.clone()), false)]
    );
    assert!(
        output(&mut peer).is_empty(),
        "the endpoint does not settle a transactional state"
    );
    // The same delivery again: it is still unsettled.
    peer.frame(
        0,
        &disposition(0, false, encode_state(&retire)),
        0,
        &mut app,
    );
    assert_eq!(app.states.len(), 2);
    // The container settles it (the discharge).
    assert!(peer.settle(id, b"a", Some(&retire)));
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DISPOSITION));
    assert_eq!(at(&out[0].2, 0), V::Boolean(false), "role sender");
    assert_eq!(at(&out[0].2, 1), V::Uint(0));
    assert_eq!(at(&out[0].2, 3), V::Boolean(true));
    assert_eq!(decode_state(out[0].2.get(4)), Ok(Some(retire.clone())));
    assert!(!peer.settle(id, b"a", None), "settled once");
    assert!(!peer.settle(id, b"zz", None), "no such delivery");
    // Settled by the partner with the transactional state.
    peer.frame(0, &disposition(1, true, encode_state(&retire)), 0, &mut app);
    assert_eq!(app.states[2], (b"b".to_vec(), Some(retire.clone()), true));
    assert!(!peer.settle(id, b"b", None));
    // The container answers a disposition with an error: the link goes.
    app.refuse_disposition = Some(AmqpError::new(
        "amqp:transaction:unknown-id",
        "no transaction 04",
    ));
    peer.frame(
        0,
        &disposition(2, false, encode_state(&retire)),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(
        error_of(&out[0], 2).condition,
        "amqp:transaction:unknown-id"
    );
    assert_eq!(
        app.detached,
        vec![(
            "out".to_string(),
            vec![b"c".to_vec()],
            app.refuse_disposition.clone()
        )]
    );
    // A malformed state.
    let no_id = V::Described {
        descriptor: 0x34,
        value: Box::new(V::List(vec![])),
    };
    peer.frame(0, &disposition(9, false, no_id), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::CLOSE));
    assert_eq!(error_of(&out[0], 0).condition, condition::INVALID_FIELD);
}

/// Part 4 §4.4.3: the txn-id of the properties of a flow reaches the
/// container; a properties map without it ends the association; a flow
/// without properties says nothing about it. A container without
/// transactional acquisition has the link terminated with
/// amqp:not-implemented; an entry that is no txn-id with
/// amqp:invalid-field.
#[test]
fn amqp10_4_4_3_the_txn_id_of_a_flow_reaches_the_container() {
    let (mut peer, mut app) = opened(PeerConfig::default());
    begin(&mut peer, &mut app, 0);
    app.coordinator = Some(Coordinator::default());
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    let flow = |handle: u32, properties: V| {
        body(
            d::FLOW,
            vec![
                V::Uint(0),
                V::Uint(100),
                V::Uint(0),
                V::Uint(100),
                V::Uint(handle),
                V::Uint(0),
                V::Uint(5),
                V::Null,
                V::Boolean(false),
                V::Boolean(false),
                properties,
            ],
        )
    };
    let t = txn(&[1, 2, 3]);
    peer.frame(0, &flow(0, flow_properties(&t)), 0, &mut app);
    peer.frame(0, &credit(0, 0, 5, false, false), 0, &mut app);
    peer.frame(
        0,
        &flow(0, V::Map(vec![(V::Symbol("x".into()), V::Uint(1))])),
        0,
        &mut app,
    );
    assert_eq!(
        app.acquiring,
        vec![
            ("out".to_string(), Some(t.clone())),
            ("out".to_string(), None)
        ]
    );
    assert!(output(&mut peer).is_empty());
    // An entry that is no txn-id.
    peer.frame(
        0,
        &flow(
            0,
            V::Map(vec![(V::Symbol("txn-id".into()), V::Binary(vec![0; 33]))]),
        ),
        0,
        &mut app,
    );
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(error_of(&out[0], 2).condition, condition::INVALID_FIELD);
    // A container without transactional acquisition.
    app.coordinator = None;
    attach_receiver(&mut peer, &mut app, 0, 1, "plain");
    peer.frame(0, &flow(1, flow_properties(&t)), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out[0].1, Some(d::DETACH));
    assert_eq!(error_of(&out[0], 2).condition, condition::NOT_IMPLEMENTED);
}

/// Part 4 §4.4.3 with Part 2 §2.6.14: a message sent with a state carries
/// it on every transfer frame; a delivery whose frames wait for the
/// session window is aborted with the next frame the window admits, in
/// place of the rest of the message.
#[test]
fn amqp10_4_4_3_a_delivery_sent_with_a_state_carries_it_on_every_frame_and_can_be_aborted() {
    let mut app = App::new();
    let mut peer = Peer::new(PeerConfig::default(), 0);
    open_512_octet_frames_two_transfers(&mut peer, &mut app);
    attach_receiver(&mut peer, &mut app, 0, 0, "out");
    let id = LinkId {
        channel: 0,
        handle: 0,
    };
    let session_flow = |next_incoming_id: u32, window: u32, credit: u32| {
        body(
            d::FLOW,
            vec![
                V::Uint(next_incoming_id),
                V::Uint(window),
                V::Uint(0),
                V::Uint(100),
                V::Uint(0),
                V::Uint(0),
                V::Uint(credit),
            ],
        )
    };
    peer.frame(0, &session_flow(0, 2, 10), 0, &mut app);
    let t = txn(&[8]);
    let acquired = in_txn(&t, None);
    let message = vec![7u8; 1500];
    assert_eq!(peer.abort(id), None, "nothing in progress");
    peer.send_with_state(id, b"a", 0, &message, false, Some(&acquired))
        .expect("send");
    let out = output(&mut peer);
    assert_eq!(out.len(), 2, "the window admits two frames");
    for frame in &out {
        assert_eq!(frame.1, Some(d::TRANSFER));
        assert_eq!(decode_state(frame.2.get(7)), Ok(Some(acquired.clone())));
        assert_eq!(at(&frame.2, 5), V::Boolean(true), "more");
    }
    assert!(peer.is_sending(id));
    // The abort waits for the window.
    assert_eq!(peer.abort(id), Some(b"a".to_vec()));
    assert!(output(&mut peer).is_empty());
    assert!(peer.is_sending(id));
    peer.frame(0, &session_flow(2, 5, 10), 0, &mut app);
    let out = output(&mut peer);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0].1, Some(d::TRANSFER));
    assert_eq!(at(&out[0].2, 9), V::Boolean(true), "aborted");
    assert_eq!(at(&out[0].2, 5), V::Boolean(false));
    assert!(out[0].3.is_empty(), "no more of the message");
    assert!(!peer.is_sending(id));
    assert!(!peer.settle(id, b"a", None), "an aborted delivery is gone");
    assert_eq!(
        peer.session_windows(0).expect("session").next_outgoing_id,
        3,
        "the abort is a transfer frame"
    );
    // The link sends again; a message in one frame carries the state too.
    peer.send_with_state(id, b"b", 0, b"m", false, Some(&acquired))
        .expect("send");
    let out = output(&mut peer);
    assert_eq!(out.len(), 1);
    assert_eq!(decode_state(out[0].2.get(7)), Ok(Some(acquired)));
    assert_eq!(at(&out[0].2, 2), V::Binary(b"b".to_vec()));
}
