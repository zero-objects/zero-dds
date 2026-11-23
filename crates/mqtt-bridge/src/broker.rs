// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT 5.0 broker logic — Spec §3 + §4.
//!
//! The session state of §4.1 and the message routing of §3.3.4, §4.5 to
//! §4.8, without a network. Time is the caller's monotonic clock in
//! milliseconds (`now_ms`); the `no_std` engine has no clock of its own.
//!
//! * Sessions (§4.1): Clean Start, Session Expiry Interval, Session
//!   Present; the session state a server keeps — subscriptions, the
//!   messages pending transmission, the QoS 1 and QoS 2 messages sent and
//!   not completely acknowledged, the QoS 2 messages received and not
//!   completely acknowledged, the Will Message and the Will Delay.
//! * Routing (§3.3.4): one copy per session for its overlapping
//!   Non-shared Subscriptions, with the highest granted QoS and every
//!   Subscription Identifier; No Local, Retain As Published, the
//!   properties of §3.3.2.3 unaltered, Message Expiry.
//! * Retained messages (§3.3.1.3) with Retain Handling at SUBSCRIBE.
//! * Shared Subscriptions (§4.8.2): one session of a group per message.
//! * Flow control (§4.9): [`Broker::take_sendable`] starts at most
//!   Receive Maximum QoS 1 and QoS 2 deliveries.
//! * Retransmission (§4.4): only at reconnect, [`Broker::resend`], within
//!   the Receive Maximum of the new Network Connection.
//!
//! Publications return [`DeliveryEnvelope`]s; [`Broker::deliver`] adds
//! them to the session state of their targets (§4.5). A session limit on
//! the messages pending transmission ends a session that a QoS 1 or QoS 2
//! message finds full ([`Delivered::over_limit`], §4.1.1).

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::string::String;
use alloc::vec::Vec;

use crate::topic_filter::{TopicFilterError, matches, validate_filter};

/// QoS level (Spec §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum QoS {
    /// `0` At-Most-Once.
    #[default]
    AtMostOnce = 0,
    /// `1` At-Least-Once.
    AtLeastOnce = 1,
    /// `2` Exactly-Once.
    ExactlyOnce = 2,
}

impl QoS {
    /// Wire value.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// `u8 -> QoS`.
    ///
    /// # Errors
    /// `()` if the value is not 0/1/2.
    #[allow(clippy::result_unit_err)]
    pub const fn from_u8(v: u8) -> Result<Self, ()> {
        match v {
            0 => Ok(Self::AtMostOnce),
            1 => Ok(Self::AtLeastOnce),
            2 => Ok(Self::ExactlyOnce),
            _ => Err(()),
        }
    }
}

/// The properties of an Application Message a server forwards unaltered
/// (§3.3.2.3, [MQTT-3.3.2-4], [MQTT-3.3.2-15] to [MQTT-3.3.2-20]). The
/// Message Expiry Interval is the value received; the server sends what is
/// left of it ([MQTT-3.3.2-6]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageProperties {
    /// Payload Format Indicator.
    pub payload_format_indicator: Option<u8>,
    /// Message Expiry Interval, seconds, as received.
    pub message_expiry_interval: Option<u32>,
    /// Content Type.
    pub content_type: Option<String>,
    /// Response Topic.
    pub response_topic: Option<String>,
    /// Correlation Data.
    pub correlation_data: Option<Vec<u8>>,
    /// User Properties, in their order ([MQTT-3.3.2-18]).
    pub user_properties: Vec<(String, String)>,
}

/// Will message — registered by the client on CONNECT. Spec §3.1.2.5,
/// §3.1.3.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Will {
    /// Will topic.
    pub topic: String,
    /// Will payload.
    pub payload: Vec<u8>,
    /// Will QoS.
    pub qos: QoS,
    /// Will retain flag.
    pub retain: bool,
    /// The Will Properties that travel with the message (§3.1.3.2).
    pub properties: MessageProperties,
    /// Will Delay Interval, seconds (§3.1.3.2.2).
    pub delay_interval: u32,
}

/// Subscription entry (§3.8.3.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Subscription {
    /// The Topic Filter as subscribed, `$share/{ShareName}/{filter}` for a
    /// Shared Subscription.
    pub filter: String,
    /// Maximum QoS granted for this subscription.
    pub max_qos: QoS,
    /// `No Local` (§3.8.3.1).
    pub no_local: bool,
    /// `Retain As Published` (§3.8.3.1).
    pub retain_as_published: bool,
    /// `Retain Handling` 0, 1 or 2 (§3.8.3.1).
    pub retain_handling: u8,
    /// The Subscription Identifier of the SUBSCRIBE (§3.8.2.1.2).
    pub subscription_identifier: Option<u32>,
}

impl Subscription {
    /// The ShareName and the filter of a Shared Subscription, `None` for a
    /// Non-shared one.
    #[must_use]
    pub fn shared(&self) -> Option<(&str, &str)> {
        parse_shared_filter(&self.filter).ok().flatten()
    }

    /// The filter a topic is matched against: without `$share/{ShareName}/`.
    #[must_use]
    pub fn match_filter(&self) -> &str {
        self.shared().map_or(self.filter.as_str(), |(_, f)| f)
    }
}

/// Splits a Shared Subscription Topic Filter (§4.8.2):
/// `Ok(Some((share_name, filter)))` for `$share/{ShareName}/{filter}`,
/// `Ok(None)` for a filter that does not start with `$share/`.
///
/// # Errors
/// A `$share/` filter without a ShareName of at least one character, a
/// ShareName with `+` or `#`, or without a `/` and a Topic Filter after it
/// ([MQTT-4.8.2-1], [MQTT-4.8.2-2]); the filter part invalid by §4.7.
pub fn parse_shared_filter(filter: &str) -> Result<Option<(&str, &str)>, TopicFilterError> {
    let Some(rest) = filter.strip_prefix("$share/") else {
        return Ok(None);
    };
    let (name, inner) = rest.split_once('/').ok_or(TopicFilterError::Empty)?;
    if name.is_empty() {
        return Err(TopicFilterError::Empty);
    }
    if name.contains('+') || name.contains('#') {
        return Err(TopicFilterError::WildcardMixedWithChars);
    }
    validate_filter(inner)?;
    Ok(Some((name, inner)))
}

/// A message on its way to one subscriber, as the session state holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    /// Topic Name.
    pub topic: String,
    /// Payload.
    pub payload: Vec<u8>,
    /// QoS of this delivery.
    pub qos: QoS,
    /// RETAIN flag of this delivery.
    pub retain: bool,
    /// The forwarded properties.
    pub properties: MessageProperties,
    /// The Subscription Identifiers of the matching subscriptions.
    pub subscription_identifiers: Vec<u32>,
    /// When the message expires, on the caller's clock.
    pub expires_at_ms: Option<u64>,
}

/// A QoS 1 or QoS 2 delivery sent and not completely acknowledged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InFlight {
    /// Packet Identifier.
    pub packet_id: u16,
    /// The message (for a retransmission at reconnect).
    pub message: Outgoing,
    /// QoS 2: PUBREC received and PUBREL sent, PUBCOMP awaited.
    pub released: bool,
    /// Sent on an earlier Network Connection and not yet again on this
    /// one (§4.4); it waits for the send quota of this connection.
    pub resend_pending: bool,
}

/// What a reconnecting client is sent again (§4.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resend {
    /// The PUBLISH, with DUP set.
    Publish(u16, Box<Outgoing>),
    /// The PUBREL.
    PubRel(u16),
}

/// The outcome of a PUBREC (§4.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PubRecOutcome {
    /// Send PUBREL.
    Release,
    /// A Reason Code of 0x80 or more: the message is acknowledged and
    /// discarded ([MQTT-4.4.0-2], [MQTT-4.8.2-6]).
    Discarded,
    /// No QoS 2 delivery with this Packet Identifier.
    NotFound,
}

/// A delivery [`Broker::take_sendable`] starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sendable {
    /// The Packet Identifier for QoS 1 and 2.
    pub packet_id: Option<u16>,
    /// The message.
    pub message: Outgoing,
    /// A retransmission of a PUBLISH sent on an earlier Network
    /// Connection, with its original Packet Identifier (§4.4).
    pub dup: bool,
}

/// What [`Broker::deliver`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Delivered {
    /// The sessions whose queue grew.
    pub touched: BTreeSet<String>,
    /// The sessions a QoS 1 or QoS 2 message found with a full queue
    /// ([`Broker::set_max_queued`]). The engine does not drop such a
    /// message and keep the session: the caller closes the session's
    /// Network Connection (DISCONNECT 0x97 Quota exceeded for an MQTT 5.0
    /// client) and then ends the session with [`Broker::end_session`]
    /// (§4.1, §4.1.1).
    pub over_limit: BTreeSet<String>,
}

/// Session state per client. Spec §4.1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Session {
    /// Client ID.
    pub client_id: String,
    /// `Clean Start` flag from the last CONNECT.
    pub clean_start: bool,
    /// Subscriptions.
    pub subscriptions: Vec<Subscription>,
    /// Will (optional).
    pub will: Option<Will>,
    /// Next packet identifier to try for outbound QoS>=1 messages.
    pub next_packet_id: u16,
    /// QoS 1 and QoS 2 deliveries sent and not completely acknowledged, in
    /// the order they were sent.
    pub in_flight: VecDeque<InFlight>,
    /// Messages pending transmission, in their order.
    pub queue: VecDeque<Outgoing>,
    /// Packet Identifiers of QoS 2 messages received and not released.
    pub inbound_qos2: BTreeSet<u16>,
    /// Session Expiry Interval, seconds; `u32::MAX` does not expire.
    pub expiry_interval: u32,
    /// A Network Connection is open.
    pub online: bool,
    /// While offline: when the session ends.
    pub expires_at_ms: Option<u64>,
    /// While offline with a delayed Will: when the Will is published.
    pub will_due_ms: Option<u64>,
}

impl Session {
    /// Constructor.
    #[must_use]
    pub fn new(client_id: String, clean_start: bool) -> Self {
        Self {
            client_id,
            clean_start,
            next_packet_id: 1,
            ..Self::default()
        }
    }

    /// A Packet Identifier no in-flight delivery uses ([MQTT-4.3.2-1],
    /// [MQTT-4.3.3-1]); `None` when all 65,535 are in use.
    pub fn allocate_packet_id(&mut self) -> Option<u16> {
        if self.in_flight.len() >= usize::from(u16::MAX) {
            return None;
        }
        loop {
            let id = if self.next_packet_id == 0 {
                1
            } else {
                self.next_packet_id
            };
            self.next_packet_id = id.wrapping_add(1);
            if !self.in_flight.iter().any(|f| f.packet_id == id) {
                return Some(id);
            }
        }
    }

    fn take_in_flight(&mut self, id: u16, released: bool) -> Option<InFlight> {
        let at = self
            .in_flight
            .iter()
            .position(|f| f.packet_id == id && f.released == released)?;
        self.in_flight.remove(at)
    }
}

/// Retained message — one entry per topic (Spec §3.3.1.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedMessage {
    /// Topic name (without wildcards).
    pub topic: String,
    /// Payload.
    pub payload: Vec<u8>,
    /// QoS at which the message was published.
    pub qos: QoS,
    /// The properties it was published with.
    pub properties: MessageProperties,
    /// When it expires, on the caller's clock.
    pub expires_at_ms: Option<u64>,
}

/// An Application Message a client published (§3.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppMessage {
    /// Topic Name.
    pub topic: String,
    /// Payload.
    pub payload: Vec<u8>,
    /// QoS.
    pub qos: QoS,
    /// RETAIN flag.
    pub retain: bool,
    /// Properties.
    pub properties: MessageProperties,
    /// The ClientID of the publishing connection, for No Local.
    pub publisher: Option<String>,
}

/// Broker state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Broker {
    sessions: BTreeMap<String, Session>,
    retained: BTreeMap<String, RetainedMessage>,
    /// Round-robin position per Shared Subscription Topic Filter.
    share_cursor: BTreeMap<String, usize>,
    /// The most messages a session holds pending transmission; `None`
    /// without limit.
    max_queued: Option<usize>,
    /// Deliveries that found the session's queue full.
    queue_overflows: u64,
}

/// A PUBLISH message to be delivered to a subscriber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryEnvelope {
    /// Target client ID.
    pub client_id: String,
    /// Topic name (as the publisher sent it).
    pub topic: String,
    /// Payload.
    pub payload: Vec<u8>,
    /// QoS = min(publish.qos, subscription.max_qos) per Spec §3.8.4.
    pub qos: QoS,
    /// `Retain` flag for this subscriber (see `retain_as_published`).
    pub retain: bool,
    /// The forwarded properties.
    pub properties: MessageProperties,
    /// The Subscription Identifiers of the matching subscriptions.
    pub subscription_identifiers: Vec<u32>,
    /// When the message expires, on the caller's clock.
    pub expires_at_ms: Option<u64>,
}

impl DeliveryEnvelope {
    fn into_outgoing(self) -> (String, Outgoing) {
        (
            self.client_id,
            Outgoing {
                topic: self.topic,
                payload: self.payload,
                qos: self.qos,
                retain: self.retain,
                properties: self.properties,
                subscription_identifiers: self.subscription_identifiers,
                expires_at_ms: self.expires_at_ms,
            },
        )
    }
}

/// The result of a CONNECT for the session (§3.1.2.4, §3.2.2.1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectOutcome {
    /// The CONNACK's Session Present flag.
    pub session_present: bool,
}

fn expiry_after(now_ms: u64, seconds: u32) -> Option<u64> {
    if seconds == u32::MAX {
        None
    } else {
        Some(now_ms + u64::from(seconds) * 1000)
    }
}

fn is_expired(at: Option<u64>, now_ms: u64) -> bool {
    at.is_some_and(|t| t <= now_ms)
}

impl Broker {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Limits the messages one session holds pending transmission
    /// (`None`, the default: no limit). At a full queue a QoS 0 delivery
    /// is not kept (§4.1 makes QoS 0 messages pending transmission
    /// optional); a QoS 1 or QoS 2 delivery puts the session in
    /// [`Delivered::over_limit`]. Both are counted
    /// ([`Broker::queue_overflows`]).
    pub fn set_max_queued(&mut self, limit: Option<usize>) {
        self.max_queued = limit;
    }

    /// Deliveries that found a session's queue full.
    #[must_use]
    pub fn queue_overflows(&self) -> u64 {
        self.queue_overflows
    }

    /// CONNECT (Spec §3.1) — brings a session online.
    /// `clean_start=true` discards an existing session; a session of a
    /// Clean Start connection ends with its connection, one without Clean
    /// Start does not expire.
    pub fn connect(&mut self, client_id: String, clean_start: bool, will: Option<Will>) {
        let expiry = if clean_start { 0 } else { u32::MAX };
        self.connect_session(&client_id, clean_start, expiry, will);
    }

    /// CONNECT with the Session Expiry Interval of MQTT 5.0 (§3.1.2.4,
    /// §3.1.2.11.2). Clean Start discards an existing session
    /// ([MQTT-3.1.2-4]); without it an existing session resumes
    /// ([MQTT-3.1.2-5]) or a new one starts ([MQTT-3.1.2-6]). The new
    /// Network Connection cancels a delayed Will of the session
    /// ([MQTT-3.1.3-9]).
    pub fn connect_session(
        &mut self,
        client_id: &str,
        clean_start: bool,
        expiry_interval: u32,
        will: Option<Will>,
    ) -> ConnectOutcome {
        if clean_start {
            self.sessions.remove(client_id);
        }
        let session_present = self.sessions.contains_key(client_id);
        let session = self
            .sessions
            .entry(client_id.into())
            .or_insert_with(|| Session::new(client_id.into(), clean_start));
        session.clean_start = clean_start;
        session.expiry_interval = expiry_interval;
        session.online = true;
        // §4.9: the send quota starts again with each Network Connection;
        // what was sent before goes again within it ([`Broker::resend`]).
        for f in &mut session.in_flight {
            f.resend_pending = !f.released;
        }
        session.expires_at_ms = None;
        session.will_due_ms = None;
        session.will = will;
        ConnectOutcome { session_present }
    }

    /// DISCONNECT — Spec §3.14. The will is only emitted on an "abnormal"
    /// disconnect (the caller passes `with_will=true`).
    pub fn disconnect(&mut self, client_id: &str, with_will: bool) -> Vec<DeliveryEnvelope> {
        self.disconnect_session(client_id, with_will, 0)
    }

    /// The Network Connection of `client_id` closed. `publish_will` is
    /// false after a DISCONNECT with Reason Code 0x00 ([MQTT-3.14.4-3]).
    /// A session with Session Expiry Interval 0 ends now and its Will is
    /// published; otherwise the session stays ([MQTT-3.1.2-23]) and the
    /// Will is published after its Will Delay Interval or when the session
    /// ends, whichever is first ([MQTT-3.1.2-8]). Returns the Will's
    /// deliveries.
    pub fn disconnect_session(
        &mut self,
        client_id: &str,
        publish_will: bool,
        now_ms: u64,
    ) -> Vec<DeliveryEnvelope> {
        let Some(session) = self.sessions.get_mut(client_id) else {
            return Vec::new();
        };
        session.online = false;
        if !publish_will {
            // [MQTT-3.1.2-10]: removed on DISCONNECT 0x00.
            session.will = None;
        }
        let mut due = Vec::new();
        if session.expiry_interval == 0 {
            if let Some(w) = session.will.take() {
                due.push(w);
            }
            self.sessions.remove(client_id);
        } else {
            session.expires_at_ms = expiry_after(now_ms, session.expiry_interval);
            if let Some(w) = session.will.as_ref() {
                if w.delay_interval == 0 {
                    due.extend(session.will.take());
                } else {
                    session.will_due_ms = expiry_after(now_ms, w.delay_interval);
                }
            }
        }
        let mut envs = Vec::new();
        for w in due {
            envs.extend(self.publish_will(client_id, w, now_ms));
        }
        envs
    }

    fn publish_will(&mut self, client_id: &str, w: Will, now_ms: u64) -> Vec<DeliveryEnvelope> {
        // [MQTT-3.1.2-14], [MQTT-3.1.2-15]: retained as the Will Retain
        // flag says; the Will Properties travel with it, User Properties
        // in their order ([MQTT-3.1.3-10]).
        let msg = AppMessage {
            topic: w.topic,
            payload: w.payload,
            qos: w.qos,
            retain: w.retain,
            properties: w.properties,
            publisher: Some(client_id.into()),
        };
        self.publish_message(&msg, now_ms)
    }

    /// Ends the session of `client_id` now and discards its state. A Will
    /// it still holds is published, as at the end of every session
    /// ([MQTT-3.1.2-8]). Returns the Will's deliveries.
    pub fn end_session(&mut self, client_id: &str, now_ms: u64) -> Vec<DeliveryEnvelope> {
        let will = self.sessions.remove(client_id).and_then(|s| s.will);
        will.map_or_else(Vec::new, |w| self.publish_will(client_id, w, now_ms))
    }

    /// Sets the Session Expiry Interval of a session (a DISCONNECT may
    /// carry a new one, §3.14.2.2.2).
    pub fn set_session_expiry(&mut self, client_id: &str, expiry_interval: u32) {
        if let Some(s) = self.sessions.get_mut(client_id) {
            s.expiry_interval = expiry_interval;
        }
    }

    /// Publishes the Wills that are due, ends the sessions whose Session
    /// Expiry Interval has passed ([MQTT-4.1.0-2]) — publishing a Will
    /// they still hold — and deletes expired messages pending transmission
    /// ([MQTT-3.3.2-5]) and expired retained messages.
    pub fn tick(&mut self, now_ms: u64) -> Vec<DeliveryEnvelope> {
        let mut wills = Vec::new();
        let mut ended = Vec::new();
        for (id, s) in &mut self.sessions {
            if s.online {
                s.queue.retain(|m| !is_expired(m.expires_at_ms, now_ms));
                continue;
            }
            let session_ends = is_expired(s.expires_at_ms, now_ms);
            if session_ends || is_expired(s.will_due_ms, now_ms) {
                s.will_due_ms = None;
                if let Some(w) = s.will.take() {
                    wills.push((id.clone(), w));
                }
            }
            if session_ends {
                ended.push(id.clone());
            } else {
                s.queue.retain(|m| !is_expired(m.expires_at_ms, now_ms));
            }
        }
        for id in &ended {
            self.sessions.remove(id);
        }
        self.retained
            .retain(|_, r| !is_expired(r.expires_at_ms, now_ms));
        let mut envs = Vec::new();
        for (id, w) in wills {
            envs.extend(self.publish_will(&id, w, now_ms));
        }
        envs
    }

    /// The earliest time [`Broker::tick`] has something to do.
    #[must_use]
    pub fn next_deadline(&self) -> Option<u64> {
        let mut next: Option<u64> = None;
        let mut consider = |t: Option<u64>| {
            if let Some(t) = t {
                next = Some(next.map_or(t, |n| n.min(t)));
            }
        };
        for s in self.sessions.values() {
            if !s.online {
                consider(s.expires_at_ms);
                consider(s.will_due_ms);
            }
            for m in &s.queue {
                consider(m.expires_at_ms);
            }
        }
        for r in self.retained.values() {
            consider(r.expires_at_ms);
        }
        next
    }

    /// SUBSCRIBE (Spec §3.8) — adds subscriptions to the session.
    /// Returns a list of granted QoS per filter.
    ///
    /// # Errors
    /// Static string if the client is unknown or the filter is invalid.
    pub fn subscribe(
        &mut self,
        client_id: &str,
        subs: Vec<Subscription>,
    ) -> Result<Vec<QoS>, &'static str> {
        if !self.sessions.contains_key(client_id) {
            return Err("unknown client");
        }
        for s in &subs {
            check_filter(&s.filter).map_err(|_| "invalid filter")?;
        }
        let (granted, _) = self.subscribe_with(client_id, subs, 0)?;
        granted
            .into_iter()
            .map(|g| g.map_err(|_| "invalid filter"))
            .collect()
    }

    /// SUBSCRIBE with the options of MQTT 5.0. Each entry is handled as if
    /// it came in a SUBSCRIBE of its own ([MQTT-3.8.4-5]): an identical
    /// Non-shared filter replaces the subscription ([MQTT-3.8.4-3]); the
    /// retained messages are sent by Retain Handling ([MQTT-3.3.1-9] to
    /// [MQTT-3.3.1-11], [MQTT-3.8.4-4]), never for a Shared Subscription
    /// (§4.8.2), with RETAIN 1 and the QoS of [MQTT-3.8.4-8]. Returns per
    /// entry the granted QoS or Reason Code 0x8F, and the retained
    /// deliveries.
    ///
    /// # Errors
    /// The client has no session.
    #[allow(clippy::type_complexity)]
    pub fn subscribe_with(
        &mut self,
        client_id: &str,
        subs: Vec<Subscription>,
        now_ms: u64,
    ) -> Result<(Vec<Result<QoS, u8>>, Vec<DeliveryEnvelope>), &'static str> {
        let session = self.sessions.get_mut(client_id).ok_or("unknown client")?;
        let mut granted = Vec::with_capacity(subs.len());
        let mut envs = Vec::new();
        for s in subs {
            let Ok(shared) = parse_shared_filter(&s.filter) else {
                granted.push(Err(0x8F));
                continue;
            };
            if shared.is_none() && validate_filter(&s.filter).is_err() {
                granted.push(Err(0x8F));
                continue;
            }
            let existed = session.subscriptions.iter().any(|x| x.filter == s.filter);
            let send_retained = shared.is_none()
                && (s.retain_handling == 0 || (s.retain_handling == 1 && !existed));
            if send_retained {
                for r in self.retained.values() {
                    if is_expired(r.expires_at_ms, now_ms) || !matches(&s.filter, &r.topic) {
                        continue;
                    }
                    envs.push(DeliveryEnvelope {
                        client_id: client_id.into(),
                        topic: r.topic.clone(),
                        payload: r.payload.clone(),
                        qos: r.qos.min(s.max_qos),
                        retain: true,
                        properties: r.properties.clone(),
                        subscription_identifiers: s.subscription_identifier.into_iter().collect(),
                        expires_at_ms: r.expires_at_ms,
                    });
                }
            }
            granted.push(Ok(s.max_qos));
            session.subscriptions.retain(|x| x.filter != s.filter);
            session.subscriptions.push(s);
        }
        Ok((granted, envs))
    }

    /// Returns the retained messages a new subscriber must see
    /// (Spec §3.3.1.3).
    #[must_use]
    pub fn retained_for(&self, filter: &str) -> Vec<&RetainedMessage> {
        self.retained
            .values()
            .filter(|r| matches(filter, &r.topic))
            .collect()
    }

    /// PUBLISH (Spec §3.3) — distributes the message to all subscribers.
    /// Sets retained state if retain=true.
    pub fn publish(
        &mut self,
        topic: &str,
        payload: Vec<u8>,
        qos: QoS,
        retain: bool,
    ) -> Vec<DeliveryEnvelope> {
        let msg = AppMessage {
            topic: topic.into(),
            payload,
            qos,
            retain,
            ..AppMessage::default()
        };
        self.publish_message(&msg, 0)
    }

    /// PUBLISH of an Application Message with its properties. RETAIN 1
    /// replaces the retained message of the topic ([MQTT-3.3.1-5]), an
    /// empty payload removes it and is not stored ([MQTT-3.3.1-6],
    /// [MQTT-3.3.1-7]); RETAIN 0 leaves it ([MQTT-3.3.1-8]). Returns one
    /// delivery per session with matching Non-shared Subscriptions and one
    /// per matching Shared Subscription.
    pub fn publish_message(&mut self, msg: &AppMessage, now_ms: u64) -> Vec<DeliveryEnvelope> {
        let expires_at_ms = msg
            .properties
            .message_expiry_interval
            .map(|s| now_ms + u64::from(s) * 1000);
        if msg.retain {
            if msg.payload.is_empty() {
                self.retained.remove(&msg.topic);
            } else {
                self.retained.insert(
                    msg.topic.clone(),
                    RetainedMessage {
                        topic: msg.topic.clone(),
                        payload: msg.payload.clone(),
                        qos: msg.qos,
                        properties: msg.properties.clone(),
                        expires_at_ms,
                    },
                );
            }
        }
        self.fanout(msg, expires_at_ms)
    }

    fn envelope(
        client_id: &str,
        msg: &AppMessage,
        qos: QoS,
        retain: bool,
        ids: Vec<u32>,
        expires_at_ms: Option<u64>,
    ) -> DeliveryEnvelope {
        DeliveryEnvelope {
            client_id: client_id.into(),
            topic: msg.topic.clone(),
            payload: msg.payload.clone(),
            qos,
            retain,
            properties: msg.properties.clone(),
            subscription_identifiers: ids,
            expires_at_ms,
        }
    }

    fn fanout(&mut self, msg: &AppMessage, expires_at_ms: Option<u64>) -> Vec<DeliveryEnvelope> {
        let mut envs = Vec::new();
        let mut groups: BTreeMap<&str, Vec<ShareMember<'_>>> = BTreeMap::new();
        for session in self.sessions.values() {
            let mut hit = false;
            let mut qos = QoS::AtMostOnce;
            let mut retain = false;
            let mut ids = Vec::new();
            for sub in &session.subscriptions {
                if !matches(sub.match_filter(), &msg.topic) {
                    continue;
                }
                if sub.shared().is_some() {
                    groups
                        .entry(sub.filter.as_str())
                        .or_default()
                        .push(ShareMember {
                            client: session.client_id.as_str(),
                            sub,
                            online: session.online,
                        });
                    continue;
                }
                // [MQTT-3.8.3-3]
                if sub.no_local && msg.publisher.as_deref() == Some(session.client_id.as_str()) {
                    continue;
                }
                hit = true;
                // [MQTT-3.3.4-2]: the highest QoS of the overlapping ones.
                qos = qos.max(msg.qos.min(sub.max_qos));
                // [MQTT-3.3.1-12], [MQTT-3.3.1-13]
                retain |= msg.retain && sub.retain_as_published;
                // [MQTT-3.3.4-3], [MQTT-3.3.4-4]
                ids.extend(sub.subscription_identifier);
            }
            if hit {
                envs.push(Self::envelope(
                    &session.client_id,
                    msg,
                    qos,
                    retain,
                    ids,
                    expires_at_ms,
                ));
            }
        }
        let mut cursors = Vec::new();
        for (filter, members) in groups {
            // One session of the group (§4.8.2); a connected one first.
            let online: Vec<_> = members.iter().filter(|m| m.online).collect();
            let pool: Vec<_> = if online.is_empty() {
                members.iter().collect()
            } else {
                online
            };
            let at = self.share_cursor.get(filter).copied().unwrap_or(0);
            let m = pool[at % pool.len()];
            envs.push(Self::envelope(
                m.client,
                msg,
                msg.qos.min(m.sub.max_qos),
                msg.retain && m.sub.retain_as_published,
                m.sub.subscription_identifier.into_iter().collect(),
                expires_at_ms,
            ));
            cursors.push((String::from(filter), at.wrapping_add(1)));
        }
        self.share_cursor.extend(cursors);
        envs
    }

    /// Adds deliveries to the session state of their targets
    /// ([MQTT-4.5.0-1]). A QoS 0 delivery to a session without a Network
    /// Connection is not kept (§4.1 lets the server keep QoS 0 messages,
    /// it does not have to). At a full queue ([`Broker::set_max_queued`])
    /// a QoS 0 delivery is not kept either; a QoS 1 or QoS 2 delivery
    /// puts the session in [`Delivered::over_limit`], and the session
    /// takes no further delivery: the caller ends it.
    pub fn deliver(&mut self, envs: Vec<DeliveryEnvelope>) -> Delivered {
        let mut d = Delivered::default();
        for env in envs {
            let (client_id, out) = env.into_outgoing();
            if d.over_limit.contains(&client_id) {
                continue;
            }
            let Some(session) = self.sessions.get_mut(&client_id) else {
                continue;
            };
            if !session.online && out.qos == QoS::AtMostOnce {
                continue;
            }
            if self.max_queued.is_some_and(|m| session.queue.len() >= m) {
                self.queue_overflows += 1;
                if out.qos != QoS::AtMostOnce {
                    d.touched.remove(&client_id);
                    d.over_limit.insert(client_id);
                }
                continue;
            }
            session.queue.push_back(out);
            d.touched.insert(client_id);
        }
        d
    }

    /// The deliveries to start now: every queued QoS 0 message and as many
    /// QoS 1 and QoS 2 messages as the send quota allows — Receive Maximum
    /// minus the deliveries not completely acknowledged on this Network
    /// Connection ([MQTT-3.3.4-9], [MQTT-4.9.0-2]). PUBLISH packets of an
    /// earlier connection that still wait for their retransmission go
    /// first, with DUP and their Packet Identifier (§4.4). Expired
    /// messages are deleted ([MQTT-3.3.2-5]). A QoS 1 or 2 delivery gets
    /// an unused Packet Identifier and is in flight from here on. The rest
    /// stays queued in its order.
    pub fn take_sendable(
        &mut self,
        client_id: &str,
        receive_maximum: u16,
        now_ms: u64,
    ) -> Vec<Sendable> {
        let Some(session) = self.sessions.get_mut(client_id) else {
            return Vec::new();
        };
        let mut quota = send_quota(session, receive_maximum);
        let mut out = Vec::new();
        for f in &mut session.in_flight {
            if quota == 0 {
                break;
            }
            if f.resend_pending {
                f.resend_pending = false;
                quota -= 1;
                out.push(Sendable {
                    packet_id: Some(f.packet_id),
                    message: f.message.clone(),
                    dup: true,
                });
            }
        }
        let mut keep = VecDeque::new();
        while let Some(m) = session.queue.pop_front() {
            if is_expired(m.expires_at_ms, now_ms) {
                continue;
            }
            if m.qos == QoS::AtMostOnce {
                out.push(Sendable {
                    packet_id: None,
                    message: m,
                    dup: false,
                });
                continue;
            }
            if quota == 0 {
                keep.push_back(m);
                continue;
            }
            let Some(id) = session.allocate_packet_id() else {
                keep.push_back(m);
                continue;
            };
            quota -= 1;
            session.in_flight.push_back(InFlight {
                packet_id: id,
                message: m.clone(),
                released: false,
                resend_pending: false,
            });
            out.push(Sendable {
                packet_id: Some(id),
                message: m,
                dup: false,
            });
        }
        session.queue = keep;
        out
    }

    /// Ends a delivery the server could not send because it exceeds the
    /// client's Maximum Packet Size: it counts as completed
    /// ([MQTT-3.1.2-25]).
    pub fn complete_unsent(&mut self, client_id: &str, packet_id: u16) {
        if let Some(s) = self.sessions.get_mut(client_id) {
            s.take_in_flight(packet_id, false);
        }
    }

    /// What a reconnecting client is sent again, in the original order
    /// ([MQTT-4.4.0-1], [MQTT-4.6.0-1]): every PUBREL packet, and the
    /// unacknowledged PUBLISH packets as far as the send quota of the new
    /// Network Connection allows ([MQTT-3.3.4-9], [MQTT-4.9.0-2]; §4.9:
    /// the quota starts again with each connection). The others wait for
    /// the quota and go out through [`Broker::take_sendable`].
    pub fn resend(&mut self, client_id: &str, receive_maximum: u16) -> Vec<Resend> {
        let Some(s) = self.sessions.get_mut(client_id) else {
            return Vec::new();
        };
        let mut quota = send_quota(s, receive_maximum);
        let mut out = Vec::new();
        for f in &mut s.in_flight {
            if f.released {
                out.push(Resend::PubRel(f.packet_id));
            } else if f.resend_pending && quota > 0 {
                f.resend_pending = false;
                quota -= 1;
                out.push(Resend::Publish(f.packet_id, Box::new(f.message.clone())));
            }
        }
        out
    }

    /// PUBACK of a QoS 1 delivery, any Reason Code ([MQTT-4.4.0-2]).
    /// Returns whether one was in flight.
    pub fn puback(&mut self, client_id: &str, packet_id: u16) -> bool {
        self.sessions.get_mut(client_id).is_some_and(|s| {
            let at = s.in_flight.iter().position(|f| {
                f.packet_id == packet_id && !f.released && f.message.qos == QoS::AtLeastOnce
            });
            at.and_then(|i| s.in_flight.remove(i)).is_some()
        })
    }

    /// PUBREC of a QoS 2 delivery (§4.3.3).
    pub fn pubrec(&mut self, client_id: &str, packet_id: u16, reason: u8) -> PubRecOutcome {
        let Some(s) = self.sessions.get_mut(client_id) else {
            return PubRecOutcome::NotFound;
        };
        let Some(at) = s
            .in_flight
            .iter()
            .position(|f| f.packet_id == packet_id && f.message.qos == QoS::ExactlyOnce)
        else {
            return PubRecOutcome::NotFound;
        };
        if reason >= 0x80 {
            s.in_flight.remove(at);
            return PubRecOutcome::Discarded;
        }
        let entry = &mut s.in_flight[at];
        entry.released = true;
        // [MQTT-4.3.3-6]: the PUBLISH is not sent again.
        entry.message.payload = Vec::new();
        PubRecOutcome::Release
    }

    /// PUBCOMP of a released QoS 2 delivery. Returns whether one was
    /// awaited.
    pub fn pubcomp(&mut self, client_id: &str, packet_id: u16) -> bool {
        self.sessions
            .get_mut(client_id)
            .and_then(|s| s.take_in_flight(packet_id, true))
            .is_some()
    }

    /// A QoS 2 PUBLISH came in: `true` if its Packet Identifier was not
    /// held yet (a new Application Message), `false` for a duplicate that
    /// must not be delivered again ([MQTT-4.3.3-10]).
    pub fn qos2_received(&mut self, client_id: &str, packet_id: u16) -> bool {
        self.sessions
            .get_mut(client_id)
            .is_some_and(|s| s.inbound_qos2.insert(packet_id))
    }

    /// PUBREL: releases the Packet Identifier ([MQTT-4.3.3-12]). Returns
    /// whether it was held.
    pub fn qos2_released(&mut self, client_id: &str, packet_id: u16) -> bool {
        self.sessions
            .get_mut(client_id)
            .is_some_and(|s| s.inbound_qos2.remove(&packet_id))
    }

    /// QoS 2 messages received from the client and not released.
    #[must_use]
    pub fn inbound_qos2_count(&self, client_id: &str) -> usize {
        self.sessions
            .get(client_id)
            .map_or(0, |s| s.inbound_qos2.len())
    }

    /// UNSUBSCRIBE (Spec §3.10).
    ///
    /// # Errors
    /// Static string if the client is unknown.
    pub fn unsubscribe(
        &mut self,
        client_id: &str,
        filters: &[String],
    ) -> Result<usize, &'static str> {
        let removed = self.unsubscribe_each(client_id, filters)?;
        Ok(removed.into_iter().filter(|r| *r).count())
    }

    /// UNSUBSCRIBE, per filter whether a subscription existed and was
    /// deleted ([MQTT-3.10.4-1]). A Shared Subscription filter detaches the
    /// session from the group (§4.8.2).
    ///
    /// # Errors
    /// Static string if the client is unknown.
    pub fn unsubscribe_each(
        &mut self,
        client_id: &str,
        filters: &[String],
    ) -> Result<Vec<bool>, &'static str> {
        let session = self.sessions.get_mut(client_id).ok_or("unknown client")?;
        Ok(filters
            .iter()
            .map(|f| {
                let before = session.subscriptions.len();
                session.subscriptions.retain(|s| &s.filter != f);
                session.subscriptions.len() != before
            })
            .collect())
    }

    /// Whether a session exists for this ClientID.
    #[must_use]
    pub fn has_session(&self, client_id: &str) -> bool {
        self.sessions.contains_key(client_id)
    }

    /// Number of active sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Number of retained messages.
    #[must_use]
    pub fn retained_count(&self) -> usize {
        self.retained.len()
    }

    /// Lookup of a session.
    #[must_use]
    pub fn session(&self, client_id: &str) -> Option<&Session> {
        self.sessions.get(client_id)
    }
}

/// Receive Maximum minus the deliveries in flight on this Network
/// Connection: those not waiting for their retransmission (§4.9).
fn send_quota(s: &Session, receive_maximum: u16) -> usize {
    let busy = s.in_flight.iter().filter(|f| !f.resend_pending).count();
    usize::from(receive_maximum).saturating_sub(busy)
}

/// A session subscribed to a Shared Subscription.
struct ShareMember<'a> {
    client: &'a str,
    sub: &'a Subscription,
    online: bool,
}

/// A Topic Filter valid by §4.7 or §4.8.2.
fn check_filter(filter: &str) -> Result<(), TopicFilterError> {
    match parse_shared_filter(filter)? {
        Some(_) => Ok(()),
        None => validate_filter(filter),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn sub(filter: &str, qos: QoS) -> Subscription {
        Subscription {
            filter: filter.into(),
            max_qos: qos,
            ..Subscription::default()
        }
    }

    fn msg(topic: &str, qos: QoS) -> AppMessage {
        AppMessage {
            topic: topic.into(),
            payload: alloc::vec![1],
            qos,
            ..AppMessage::default()
        }
    }

    fn online(b: &mut Broker, id: &str, expiry: u32) {
        b.connect_session(id, false, expiry, None);
    }

    #[test]
    fn mqtt5_4_1_the_qos_levels_are_0_1_and_2() {
        for (q, v) in [
            (QoS::AtMostOnce, 0),
            (QoS::AtLeastOnce, 1),
            (QoS::ExactlyOnce, 2),
        ] {
            assert_eq!(q.to_u8(), v);
            assert_eq!(QoS::from_u8(v).unwrap(), q);
        }
        assert!(QoS::from_u8(3).is_err());
    }

    #[test]
    fn connect_creates_session() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        assert_eq!(b.session_count(), 1);
    }

    #[test]
    fn subscribe_then_publish_delivers() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        b.subscribe("c1", alloc::vec![sub("a/+", QoS::AtLeastOnce)])
            .unwrap();
        let envs = b.publish("a/x", alloc::vec![1, 2, 3], QoS::AtLeastOnce, false);
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].client_id, "c1");
        assert_eq!(envs[0].payload, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn publish_qos_is_min_of_pub_and_sub() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        b.subscribe("c1", alloc::vec![sub("t", QoS::AtLeastOnce)])
            .unwrap();
        // Publisher sends QoS 2, subscription max is 1 → effective 1.
        let envs = b.publish("t", alloc::vec![1], QoS::ExactlyOnce, false);
        assert_eq!(envs[0].qos, QoS::AtLeastOnce);
    }

    #[test]
    fn retained_message_persists() {
        let mut b = Broker::new();
        b.publish("t", alloc::vec![1, 2], QoS::AtMostOnce, true);
        assert_eq!(b.retained_count(), 1);
        let r = b.retained_for("#");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].payload, alloc::vec![1, 2]);
    }

    #[test]
    fn empty_payload_clears_retained() {
        let mut b = Broker::new();
        b.publish("t", alloc::vec![1], QoS::AtMostOnce, true);
        b.publish("t", alloc::vec![], QoS::AtMostOnce, true);
        assert_eq!(b.retained_count(), 0);
    }

    #[test]
    fn invalid_filter_rejected() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        assert!(
            b.subscribe("c1", alloc::vec![sub("a/#/c", QoS::AtMostOnce)])
                .is_err()
        );
    }

    #[test]
    fn unknown_client_subscribe_rejected() {
        let mut b = Broker::new();
        assert!(
            b.subscribe("ghost", alloc::vec![sub("a", QoS::AtMostOnce)])
                .is_err()
        );
    }

    #[test]
    fn unsubscribe_removes_filter() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        b.subscribe(
            "c1",
            alloc::vec![sub("a", QoS::AtMostOnce), sub("b", QoS::AtMostOnce)],
        )
        .unwrap();
        let n = b.unsubscribe("c1", &alloc::vec!["a".into()]).unwrap();
        assert_eq!(n, 1);
        assert_eq!(b.session("c1").unwrap().subscriptions.len(), 1);
    }

    #[test]
    fn disconnect_with_will_delivers_will_message() {
        let mut b = Broker::new();
        b.connect(
            "c1".into(),
            true,
            Some(Will {
                topic: "lwt".into(),
                payload: alloc::vec![9, 9],
                qos: QoS::AtLeastOnce,
                ..Will::default()
            }),
        );
        b.connect("c2".into(), true, None);
        b.subscribe("c2", alloc::vec![sub("lwt", QoS::AtLeastOnce)])
            .unwrap();
        let envs = b.disconnect("c1", true);
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].client_id, "c2");
    }

    #[test]
    fn clean_disconnect_drops_will() {
        let mut b = Broker::new();
        b.connect(
            "c1".into(),
            true,
            Some(Will {
                topic: "lwt".into(),
                ..Will::default()
            }),
        );
        let envs = b.disconnect("c1", false);
        assert!(envs.is_empty());
    }

    #[test]
    fn subscribe_replaces_existing_filter() {
        let mut b = Broker::new();
        b.connect("c1".into(), true, None);
        b.subscribe("c1", alloc::vec![sub("a", QoS::AtMostOnce)])
            .unwrap();
        b.subscribe("c1", alloc::vec![sub("a", QoS::ExactlyOnce)])
            .unwrap();
        assert_eq!(b.session("c1").unwrap().subscriptions.len(), 1);
        assert_eq!(
            b.session("c1").unwrap().subscriptions[0].max_qos,
            QoS::ExactlyOnce
        );
    }

    #[test]
    fn mqtt5_4_3_2_a_packet_identifier_in_flight_is_not_assigned_again() {
        let mut s = Session::new("c1".into(), true);
        s.next_packet_id = u16::MAX;
        let first = s.allocate_packet_id().unwrap();
        assert_eq!(first, u16::MAX);
        assert_eq!(s.allocate_packet_id(), Some(1), "wraps to 1, never 0");
        s.in_flight.push_back(InFlight {
            packet_id: 2,
            message: Outgoing {
                topic: "t".into(),
                payload: Vec::new(),
                qos: QoS::AtLeastOnce,
                retain: false,
                properties: MessageProperties::default(),
                subscription_identifiers: Vec::new(),
                expires_at_ms: None,
            },
            released: false,
            resend_pending: false,
        });
        assert_eq!(s.allocate_packet_id(), Some(3), "2 is in flight");
    }

    #[test]
    fn mqtt5_3_1_2_4_clean_start_discards_and_session_present_reports_a_resumed_session() {
        let mut b = Broker::new();
        assert!(!b.connect_session("c", false, 60, None).session_present);
        b.subscribe("c", alloc::vec![sub("t", QoS::AtLeastOnce)])
            .unwrap();
        b.disconnect_session("c", true, 0);
        let resumed = b.connect_session("c", false, 60, None);
        assert!(resumed.session_present);
        assert_eq!(b.session("c").unwrap().subscriptions.len(), 1);
        b.disconnect_session("c", true, 2);
        assert!(!b.connect_session("c", true, 60, None).session_present);
        assert!(b.session("c").unwrap().subscriptions.is_empty());
    }

    #[test]
    fn mqtt5_4_1_a_session_ends_when_its_expiry_interval_has_passed() {
        let mut b = Broker::new();
        online(&mut b, "zero", 0);
        online(&mut b, "two", 2);
        online(&mut b, "never", u32::MAX);
        for id in ["zero", "two", "never"] {
            b.disconnect_session(id, false, 1_000);
        }
        assert!(b.session("zero").is_none(), "Session Expiry Interval 0");
        assert_eq!(b.next_deadline(), Some(3_000));
        b.tick(2_999);
        assert!(b.session("two").is_some());
        b.tick(3_000);
        assert!(b.session("two").is_none());
        assert!(b.session("never").is_some());
    }

    #[test]
    fn mqtt5_3_1_2_8_a_delayed_will_follows_its_delay_or_the_end_of_the_session() {
        let will = |delay| Will {
            topic: "lwt".into(),
            payload: alloc::vec![7],
            delay_interval: delay,
            ..Will::default()
        };
        let mut b = Broker::new();
        online(&mut b, "sub", u32::MAX);
        b.subscribe("sub", alloc::vec![sub("lwt", QoS::AtMostOnce)])
            .unwrap();
        // Will Delay 5 s, Session Expiry 2 s: the end of the session comes
        // first.
        b.connect_session("a", false, 2, Some(will(5)));
        assert!(b.disconnect_session("a", true, 0).is_empty());
        assert!(b.tick(1_999).is_empty());
        assert_eq!(b.tick(2_000).len(), 1);
        // Will Delay 1 s: due after one second, the session stays.
        b.connect_session("b", false, 60, Some(will(1)));
        assert!(b.disconnect_session("b", true, 0).is_empty());
        assert_eq!(b.tick(1_000).len(), 1);
        assert!(b.session("b").is_some());
        // A new connection before the delay cancels the Will.
        b.connect_session("c", false, 60, Some(will(1)));
        b.disconnect_session("c", true, 0);
        b.connect_session("c", false, 60, None);
        assert!(b.tick(5_000).is_empty());
    }

    #[test]
    fn mqtt5_3_3_4_overlapping_subscriptions_give_one_copy_with_the_highest_qos_and_every_identifier()
     {
        let mut b = Broker::new();
        online(&mut b, "c", 0);
        let mut s1 = sub("a/+", QoS::AtLeastOnce);
        s1.subscription_identifier = Some(1);
        let mut s2 = sub("a/#", QoS::ExactlyOnce);
        s2.subscription_identifier = Some(2);
        let s3 = sub("a/b", QoS::AtMostOnce);
        b.subscribe_with("c", alloc::vec![s1, s2, s3], 0).unwrap();
        let envs = b.publish_message(&msg("a/b", QoS::ExactlyOnce), 0);
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].qos, QoS::ExactlyOnce);
        let mut ids = envs[0].subscription_identifiers.clone();
        ids.sort_unstable();
        assert_eq!(ids, alloc::vec![1, 2]);
    }

    #[test]
    fn mqtt5_3_8_3_1_no_local_and_retain_as_published() {
        let mut b = Broker::new();
        online(&mut b, "me", 0);
        online(&mut b, "other", 0);
        let mut nl = sub("t", QoS::AtMostOnce);
        nl.no_local = true;
        b.subscribe_with("me", alloc::vec![nl], 0).unwrap();
        let mut rap = sub("t", QoS::AtMostOnce);
        rap.retain_as_published = true;
        b.subscribe_with("other", alloc::vec![rap], 0).unwrap();
        let mut m = msg("t", QoS::AtMostOnce);
        m.retain = true;
        m.publisher = Some("me".into());
        let envs = b.publish_message(&m, 0);
        assert_eq!(envs.len(), 1, "No Local keeps it from its publisher");
        assert_eq!(envs[0].client_id, "other");
        assert!(envs[0].retain, "Retain As Published 1");
        b.subscribe_with("other", alloc::vec![sub("t", QoS::AtMostOnce)], 0)
            .unwrap();
        let envs = b.publish_message(&m, 0);
        assert!(!envs[0].retain, "Retain As Published 0");
    }

    #[test]
    fn mqtt5_3_3_1_3_retain_handling_0_1_2() {
        let mut b = Broker::new();
        b.publish("r/x", alloc::vec![5], QoS::AtLeastOnce, true);
        online(&mut b, "c", 0);
        let rh = |h| Subscription {
            filter: "r/#".into(),
            max_qos: QoS::AtLeastOnce,
            retain_handling: h,
            ..Subscription::default()
        };
        let (_, envs) = b.subscribe_with("c", alloc::vec![rh(1)], 0).unwrap();
        assert_eq!(envs.len(), 1, "1, new subscription");
        assert!(envs[0].retain);
        let (_, envs) = b.subscribe_with("c", alloc::vec![rh(1)], 0).unwrap();
        assert!(envs.is_empty(), "1, existing subscription");
        let (_, envs) = b.subscribe_with("c", alloc::vec![rh(0)], 0).unwrap();
        assert_eq!(envs.len(), 1, "0, always");
        let (_, envs) = b.subscribe_with("c", alloc::vec![rh(2)], 0).unwrap();
        assert!(envs.is_empty(), "2, never");
    }

    #[test]
    fn mqtt5_4_8_2_a_shared_subscription_sends_each_message_to_one_session() {
        let mut b = Broker::new();
        for id in ["s1", "s2", "plain"] {
            online(&mut b, id, 0);
        }
        b.subscribe_with("s1", alloc::vec![sub("$share/g/t/+", QoS::AtLeastOnce)], 0)
            .unwrap();
        let (_, retained) = b
            .subscribe_with("s2", alloc::vec![sub("$share/g/t/+", QoS::AtLeastOnce)], 0)
            .unwrap();
        assert!(retained.is_empty());
        b.subscribe_with("plain", alloc::vec![sub("t/+", QoS::AtLeastOnce)], 0)
            .unwrap();
        let mut shared_hits = BTreeMap::<String, usize>::new();
        for _ in 0..4 {
            let envs = b.publish_message(&msg("t/x", QoS::AtLeastOnce), 0);
            assert_eq!(envs.len(), 2, "one for the group, one non-shared");
            for e in envs.iter().filter(|e| e.client_id != "plain") {
                *shared_hits.entry(e.client_id.clone()).or_default() += 1;
            }
        }
        assert_eq!(shared_hits.get("s1"), Some(&2));
        assert_eq!(shared_hits.get("s2"), Some(&2));
        assert!(parse_shared_filter("$share//t").is_err());
        assert!(parse_shared_filter("$share/g").is_err());
        assert!(parse_shared_filter("$share/g+/t").is_err());
        assert_eq!(
            parse_shared_filter("$share/g//f").unwrap(),
            Some(("g", "/f"))
        );
    }

    #[test]
    fn mqtt5_4_9_the_send_quota_holds_back_qos_1_and_2_not_qos_0() {
        let mut b = Broker::new();
        online(&mut b, "c", 0);
        b.subscribe_with("c", alloc::vec![sub("#", QoS::ExactlyOnce)], 0)
            .unwrap();
        for (t, q) in [
            ("a", QoS::AtLeastOnce),
            ("b", QoS::ExactlyOnce),
            ("c", QoS::AtMostOnce),
        ] {
            let envs = b.publish_message(&msg(t, q), 0);
            b.deliver(envs);
        }
        let first = b.take_sendable("c", 1, 0);
        let topics: Vec<_> = first.iter().map(|s| s.message.topic.as_str()).collect();
        assert_eq!(topics, ["a", "c"]);
        assert!(b.take_sendable("c", 1, 0).is_empty(), "quota 0");
        let pid = first[0].packet_id.unwrap();
        assert!(b.puback("c", pid));
        let next = b.take_sendable("c", 1, 0);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].message.topic, "b");
    }

    #[test]
    fn mqtt5_4_4_resend_is_in_order_and_a_failed_pubrec_discards() {
        let mut b = Broker::new();
        online(&mut b, "c", 60);
        b.subscribe_with("c", alloc::vec![sub("#", QoS::ExactlyOnce)], 0)
            .unwrap();
        for t in ["one", "two", "three"] {
            let envs = b.publish_message(&msg(t, QoS::ExactlyOnce), 0);
            b.deliver(envs);
        }
        let sent = b.take_sendable("c", 10, 0);
        let ids: Vec<u16> = sent.iter().map(|s| s.packet_id.unwrap()).collect();
        assert_eq!(b.pubrec("c", ids[0], 0x00), PubRecOutcome::Release);
        assert_eq!(b.pubrec("c", ids[2], 0x80), PubRecOutcome::Discarded);
        assert_eq!(b.pubrec("c", 999, 0x00), PubRecOutcome::NotFound);
        b.disconnect_session("c", false, 0);
        b.connect_session("c", false, 60, None);
        let resend = b.resend("c", 10);
        assert_eq!(resend.len(), 2);
        assert_eq!(resend[0], Resend::PubRel(ids[0]));
        assert!(matches!(&resend[1], Resend::Publish(id, m) if *id == ids[1] && m.topic == "two"));
        assert!(b.pubcomp("c", ids[0]));
        assert!(!b.pubcomp("c", ids[0]));
    }

    #[test]
    fn mqtt5_4_3_3_a_duplicate_qos2_publish_is_not_a_new_message_until_pubrel() {
        let mut b = Broker::new();
        online(&mut b, "c", 0);
        assert!(b.qos2_received("c", 7));
        assert!(!b.qos2_received("c", 7));
        assert_eq!(b.inbound_qos2_count("c"), 1);
        assert!(b.qos2_released("c", 7));
        assert!(!b.qos2_released("c", 7));
        assert!(b.qos2_received("c", 7));
    }

    #[test]
    fn mqtt5_3_3_2_3_3_an_expired_message_is_deleted_before_it_is_sent() {
        let mut b = Broker::new();
        online(&mut b, "c", 60);
        b.subscribe_with("c", alloc::vec![sub("t", QoS::AtLeastOnce)], 0)
            .unwrap();
        b.disconnect_session("c", true, 0);
        let mut m = msg("t", QoS::AtLeastOnce);
        m.properties.message_expiry_interval = Some(1);
        let envs = b.publish_message(&m, 0);
        assert_eq!(envs[0].expires_at_ms, Some(1_000));
        b.deliver(envs);
        assert_eq!(b.next_deadline(), Some(1_000));
        b.connect_session("c", false, 60, None);
        assert!(b.take_sendable("c", 10, 1_000).is_empty());
    }

    #[test]
    fn mqtt5_4_1_qos0_is_not_kept_for_an_offline_session_and_a_full_queue_ends_the_session() {
        let mut b = Broker::new();
        b.set_max_queued(Some(1));
        online(&mut b, "c", 60);
        b.subscribe_with("c", alloc::vec![sub("t", QoS::AtLeastOnce)], 0)
            .unwrap();
        b.disconnect_session("c", true, 0);
        let e = b.publish_message(&msg("t", QoS::AtMostOnce), 0);
        assert!(b.deliver(e).touched.is_empty());
        let e = b.publish_message(&msg("t", QoS::AtLeastOnce), 0);
        assert_eq!(b.deliver(e).touched.len(), 1);
        // A QoS 1 message at the full queue is not dropped from a session
        // that goes on: the session is reported for its end.
        let e = b.publish_message(&msg("t", QoS::AtLeastOnce), 0);
        let d = b.deliver(e);
        assert!(d.touched.is_empty());
        assert_eq!(d.over_limit.iter().collect::<Vec<_>>(), ["c"]);
        assert_eq!(b.queue_overflows(), 1);
        assert!(b.end_session("c", 0).is_empty());
        assert!(!b.has_session("c"));
    }

    #[test]
    fn mqtt5_4_1_at_a_full_queue_qos0_is_not_kept_and_the_ended_session_publishes_its_will() {
        let mut b = Broker::new();
        b.set_max_queued(Some(1));
        let will = Will {
            topic: "lwt".into(),
            payload: alloc::vec![1],
            qos: QoS::AtMostOnce,
            retain: false,
            delay_interval: 60,
            properties: MessageProperties::default(),
        };
        b.connect_session("c", true, 60, Some(will));
        online(&mut b, "watch", 0);
        b.subscribe_with("c", alloc::vec![sub("t", QoS::AtLeastOnce)], 0)
            .unwrap();
        b.subscribe_with("watch", alloc::vec![sub("lwt", QoS::AtMostOnce)], 0)
            .unwrap();
        let e = b.publish_message(&msg("t", QoS::AtLeastOnce), 0);
        b.deliver(e);
        // Online and full: QoS 0 is not kept, the session goes on.
        let e = b.publish_message(&msg("t", QoS::AtMostOnce), 0);
        let d = b.deliver(e);
        assert!(d.touched.is_empty() && d.over_limit.is_empty());
        assert_eq!(b.queue_overflows(), 1);
        let e = b.publish_message(&msg("t", QoS::AtLeastOnce), 0);
        assert!(b.deliver(e).over_limit.contains("c"));
        // The end of the session publishes the Will at once ([MQTT-3.1.2-8]).
        let will = b.end_session("c", 0);
        assert_eq!(will.len(), 1);
        assert_eq!(will[0].client_id, "watch");
    }

    #[test]
    fn mqtt5_4_9_a_reconnect_resends_within_the_new_receive_maximum() {
        let mut b = Broker::new();
        online(&mut b, "c", 60);
        b.subscribe_with("c", alloc::vec![sub("t", QoS::AtLeastOnce)], 0)
            .unwrap();
        for _ in 0..3 {
            let e = b.publish_message(&msg("t", QoS::AtLeastOnce), 0);
            b.deliver(e);
        }
        let sent = b.take_sendable("c", 2, 0);
        let ids: Vec<u16> = sent.iter().map(|s| s.packet_id.unwrap()).collect();
        assert_eq!(ids.len(), 2);
        b.disconnect_session("c", true, 0);
        b.connect_session("c", false, 60, None);
        let again = b.resend("c", 1);
        assert!(matches!(&again[..], [Resend::Publish(id, _)] if *id == ids[0]));
        assert!(b.take_sendable("c", 1, 0).is_empty(), "quota 1 in use");
        assert!(b.puback("c", ids[0]));
        let next = b.take_sendable("c", 1, 0);
        assert_eq!(next.len(), 1);
        assert_eq!((next[0].packet_id, next[0].dup), (Some(ids[1]), true));
        assert!(b.puback("c", ids[1]));
        let last = b.take_sendable("c", 1, 0);
        assert_eq!(last.len(), 1);
        assert!(!last[0].dup);
        assert!(!ids.contains(&last[0].packet_id.unwrap()));
    }
}
