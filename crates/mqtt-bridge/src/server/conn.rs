// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! One Network Connection of the broker server: the CONNECT and its
//! checks (§3.1.4), enhanced authentication (§4.12), the CONNACK (§3.2),
//! the packets of a session (§3.3 to §3.15) and the end of the connection
//! with its Will (§3.1.2.5) and error handling (§4.13).

use std::collections::BTreeMap;
use std::io::{self, BufReader, Read};
use std::string::String;
use std::sync::MutexGuard;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Instant;
use std::vec::Vec;

use super::inbound::{self, ConnectRefusal};
use super::{
    AuthExchange, AuthStep, ClientStream, ClientWriter, ConnEntry, ServerConfig, Shared, State,
};
use crate::broker::{
    AppMessage, DeliveryEnvelope, PubRecOutcome, QoS, Resend, Subscription, Will,
    parse_shared_filter,
};
use crate::codec::{PublishPacket, encode_publish_v};
use crate::control_packets::{
    AckBody, ConnackBody, DisconnectBody, SubackBody, encode_ack_body_v, encode_connack_body_v,
    encode_disconnect_body_v, encode_suback_body_v, encode_unsuback_body_v,
};
use crate::keep_alive::KeepAliveTracker;
use crate::net::{PacketReadError, byte0, frame_packet, read_packet_within};
use crate::packet::ControlPacketType as T;
use crate::property_set::{MALFORMED_PACKET, PROTOCOL_ERROR, PropertySet, TOPIC_ALIAS_INVALID};
use crate::topic_filter::validate_filter;
use crate::version::ProtocolVersion;

const NO_MATCHING_SUBSCRIBERS: u8 = 0x10;
const NO_SUBSCRIPTION_EXISTED: u8 = 0x11;
const CONTINUE_AUTHENTICATION: u8 = 0x18;
const REAUTHENTICATE: u8 = 0x19;
const UNSPECIFIED_ERROR: u8 = 0x80;
const SERVER_UNAVAILABLE: u8 = 0x88;
const BAD_AUTHENTICATION_METHOD: u8 = 0x8C;
const SESSION_TAKEN_OVER: u8 = 0x8E;
const TOPIC_FILTER_INVALID: u8 = 0x8F;
const TOPIC_NAME_INVALID: u8 = 0x90;
const PACKET_IDENTIFIER_NOT_FOUND: u8 = 0x92;
const RECEIVE_MAXIMUM_EXCEEDED: u8 = 0x93;
const PACKET_TOO_LARGE: u8 = 0x95;
const QUOTA_EXCEEDED: u8 = 0x97;
const PAYLOAD_FORMAT_INVALID: u8 = 0x99;
const RETAIN_NOT_SUPPORTED: u8 = 0x9A;
const QOS_NOT_SUPPORTED: u8 = 0x9B;
const SHARED_SUBSCRIPTIONS_NOT_SUPPORTED: u8 = 0x9E;
const SUBSCRIPTION_IDENTIFIERS_NOT_SUPPORTED: u8 = 0xA1;
const WILDCARD_SUBSCRIPTIONS_NOT_SUPPORTED: u8 = 0xA2;

/// What a connection's writer thread is handed.
#[derive(Debug)]
pub enum Outbound {
    /// A whole encoded packet.
    Bytes(Vec<u8>),
    /// The server's DISCONNECT: written as the last packet, then the
    /// Network Connection is closed; whatever is queued after it is not
    /// written ([MQTT-3.14.4-1]).
    Disconnect(Vec<u8>),
    /// Write what came before, then close the Network Connection.
    Close,
}

fn spawn_writer(mut w: Box<dyn ClientWriter>, rx: Receiver<Outbound>) -> JoinHandle<()> {
    thread::spawn(move || {
        for msg in rx {
            match msg {
                Outbound::Bytes(b) => {
                    if w.write_all(&b).and_then(|()| w.flush()).is_err() {
                        break;
                    }
                }
                Outbound::Disconnect(b) => {
                    // The connection closes either way.
                    let _ = w.write_all(&b).and_then(|()| w.flush());
                    break;
                }
                Outbound::Close => break,
            }
        }
        w.close();
    })
}

/// Reads with a deadline: each read waits at most until it, the Keep
/// Alive of [MQTT-3.1.2-22] (and the end of a transport's handshake).
pub(super) struct DeadlineReader<S: ClientStream> {
    pub(super) stream: S,
    pub(super) deadline: Option<Instant>,
}

impl<S: ClientStream> Read for DeadlineReader<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let timeout = match self.deadline {
            None => None,
            Some(d) => Some(
                d.checked_duration_since(Instant::now())
                    .filter(|left| !left.is_zero())
                    .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "keep alive"))?,
            ),
        };
        self.stream.set_read_timeout(timeout)?;
        self.stream.read(buf)
    }
}

fn frame(kind: T, flags: u8, body: &[u8]) -> Vec<u8> {
    // A body of the server's own packets is far below the VBI limit.
    frame_packet(byte0(kind, flags), body).unwrap_or_default()
}

fn encode_props(props: &PropertySet) -> Vec<u8> {
    // Values the server writes are its own or were read from the wire, so
    // each fits its length prefix.
    props.encode().unwrap_or_default()
}

fn connack_bytes(
    v: ProtocolVersion,
    session_present: bool,
    code: u8,
    props: &PropertySet,
) -> Vec<u8> {
    let body = encode_connack_body_v(
        &ConnackBody {
            session_present,
            reason_code: code,
            properties: encode_props(props),
        },
        v,
    )
    .unwrap_or_default();
    frame(T::ConnAck, 0, &body)
}

fn ack_bytes(v: ProtocolVersion, kind: T, pid: u16, code: u8, props: &PropertySet) -> Vec<u8> {
    let body = encode_ack_body_v(
        &AckBody {
            packet_id: pid,
            reason_code: code,
            properties: encode_props(props),
        },
        v,
    )
    .unwrap_or_default();
    frame(kind, if kind == T::PubRel { 0b0010 } else { 0 }, &body)
}

fn reason_body_bytes(kind: T, code: u8, props: &PropertySet) -> Vec<u8> {
    let body = encode_disconnect_body_v(
        &DisconnectBody {
            reason_code: code,
            properties: encode_props(props),
        },
        ProtocolVersion::V5,
    )
    .unwrap_or_default();
    frame(kind, 0, &body)
}

/// A DISCONNECT with `code` and `props` to an MQTT 5.0 client, then the
/// close; a 3.1.1 client has no server DISCONNECT and is closed. One
/// message to the writer carries both, so no packet another thread queues
/// meanwhile goes out after the DISCONNECT ([MQTT-3.14.4-1]); the Session
/// Expiry Interval is never in it ([MQTT-3.14.2-2]).
pub(super) fn send_disconnect(entry: &ConnEntry, code: u8, props: &PropertySet) {
    let msg = if entry.version == ProtocolVersion::V5 {
        let mut bytes = reason_body_bytes(T::Disconnect, code, props);
        if bytes.len() as u64 > u64::from(entry.maximum_packet_size) {
            bytes = reason_body_bytes(T::Disconnect, code, &PropertySet::default());
        }
        Outbound::Disconnect(bytes)
    } else {
        Outbound::Close
    };
    // The writer may have ended already; the connection is closed then.
    let _ = entry.tx.send(msg);
}

/// The PUBLISH of `msg` to the client of `entry`, or `None` when it
/// exceeds the client's Maximum Packet Size ([MQTT-3.1.2-24]). The
/// properties of §3.3.2.3 go out unaltered, the Message Expiry Interval as
/// what is left of it ([MQTT-3.3.2-6]), the Subscription Identifiers of
/// the matching subscriptions ([MQTT-3.3.4-3]). DUP is set only on a
/// retransmission ([MQTT-3.3.1-1], [MQTT-3.3.1-3]). A Topic Alias is used
/// only up to the client's Topic Alias Maximum ([MQTT-3.1.2-26],
/// [MQTT-3.1.2-27], [MQTT-3.3.2-11]) and never on a retransmission.
fn publish_bytes(
    entry: &mut ConnEntry,
    msg: &crate::broker::Outgoing,
    packet_id: Option<u16>,
    dup: bool,
    now: u64,
    cfg: &ServerConfig,
) -> Option<Vec<u8>> {
    let v5 = entry.version == ProtocolVersion::V5;
    let mut topic = msg.topic.clone();
    let mut new_alias = None;
    let mut props = PropertySet {
        payload_format_indicator: msg.properties.payload_format_indicator,
        message_expiry_interval: msg
            .expires_at_ms
            .map(|at| u32::try_from(at.saturating_sub(now).div_ceil(1000)).unwrap_or(u32::MAX)),
        content_type: msg.properties.content_type.clone(),
        response_topic: msg.properties.response_topic.clone(),
        correlation_data: msg.properties.correlation_data.clone(),
        user_properties: msg.properties.user_properties.clone(),
        subscription_identifiers: msg.subscription_identifiers.clone(),
        ..PropertySet::default()
    };
    if v5 && !dup && cfg.outbound_topic_aliases && entry.topic_alias_maximum > 0 {
        if let Some(a) = entry.aliases_out.get(&msg.topic) {
            props.topic_alias = Some(*a);
            topic.clear();
        } else if let Ok(next) = u16::try_from(entry.aliases_out.len() + 1) {
            if next <= entry.topic_alias_maximum {
                props.topic_alias = Some(next);
                new_alias = Some(next);
            }
        }
    }
    let packet = PublishPacket {
        dup,
        qos: msg.qos.to_u8(),
        retain: msg.retain,
        topic,
        packet_id,
        properties: if v5 { props.encode().ok()? } else { Vec::new() },
        payload: msg.payload.clone(),
    };
    let bytes = encode_publish_v(&packet, entry.version).ok()?;
    if bytes.len() as u64 > u64::from(entry.maximum_packet_size) {
        return None;
    }
    if let Some(a) = new_alias {
        entry.aliases_out.insert(msg.topic.clone(), a);
    }
    Some(bytes)
}

/// Sends what the session of `client_id` may send now (§4.9). A message
/// too large for the client is discarded and counts as sent
/// ([MQTT-3.1.2-25]).
pub(super) fn pump(st: &mut State, client_id: &str, now: u64, cfg: &ServerConfig) {
    let State { broker, conns, .. } = st;
    let Some(entry) = conns.get_mut(client_id) else {
        return;
    };
    loop {
        let items = broker.take_sendable(client_id, entry.receive_maximum, now);
        let mut freed = false;
        for s in items {
            match publish_bytes(entry, &s.message, s.packet_id, s.dup, now, cfg) {
                Some(b) => {
                    let _ = entry.tx.send(Outbound::Bytes(b));
                }
                None => {
                    if let Some(pid) = s.packet_id {
                        broker.complete_unsent(client_id, pid);
                        freed = true;
                    }
                }
            }
        }
        if !freed {
            break;
        }
    }
}

/// Adds `envs` to the session state of their targets ([MQTT-4.5.0-1])
/// and sends what may go now. A session that a QoS 1 or QoS 2 message
/// finds with a full queue ([`ServerConfig::max_queued_messages`]) ends
/// (§4.1.1): its connection gets DISCONNECT 0x97 Quota exceeded and is
/// closed, then its state is discarded and a Will it holds is published.
pub(super) fn route(st: &mut State, envs: Vec<DeliveryEnvelope>, now: u64, cfg: &ServerConfig) {
    let mut envs = envs;
    while !envs.is_empty() {
        let d = st.broker.deliver(core::mem::take(&mut envs));
        for client in &d.over_limit {
            if let Some(entry) = st.conns.remove(client) {
                send_disconnect(&entry, QUOTA_EXCEEDED, &PropertySet::default());
            }
            envs.extend(st.broker.end_session(client, now));
        }
        for client in &d.touched {
            pump(st, client, now, cfg);
        }
    }
}

/// How a connection ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    /// The network failed, the client closed, the Keep Alive passed, or
    /// the session went to another connection or ended (a takeover,
    /// 0x97) and this one was closed.
    Network,
    /// A Malformed Packet, Protocol Error or other error with this Reason
    /// Code (§4.13).
    Error(u8),
    /// A DISCONNECT of the client.
    Client { publish_will: bool },
}

struct Connection<'a> {
    shared: &'a Shared,
    tx: Sender<Outbound>,
    id: u64,
    version: ProtocolVersion,
    client_id: String,
    /// The Keep Alive in force (§3.1.2.10, §3.2.2.3.14).
    keep_alive: KeepAliveTracker,
    /// The Authentication Method of the CONNECT (§4.12).
    auth_method: Option<String>,
    /// A re-authentication in progress (§4.12.1).
    auth: Option<Box<dyn AuthExchange>>,
    /// The client's Topic Aliases on this connection ([MQTT-3.3.2-7]).
    aliases_in: BTreeMap<u16, String>,
    /// Request Problem Information of the CONNECT (§3.1.2.11.7).
    problem_info: bool,
    /// The client's Maximum Packet Size.
    max_packet_out: u32,
    /// The Session Expiry Interval the CONNECT asked for, before the
    /// server's maximum (§3.14.2.2.2).
    connect_session_expiry: u32,
}

/// Serves one Network Connection to its end.
pub(super) fn serve<S: ClientStream>(shared: &Shared, stream: S) {
    let Ok(writer) = stream.writer() else {
        return;
    };
    let (tx, rx) = channel();
    let writer_join = spawn_writer(writer, rx);
    let mut reader = BufReader::new(DeadlineReader {
        stream,
        deadline: None,
    });
    let mut conn = Connection {
        shared,
        tx,
        id: shared.next_conn_id(),
        version: ProtocolVersion::V5,
        client_id: String::new(),
        keep_alive: KeepAliveTracker::new(0),
        auth_method: None,
        auth: None,
        aliases_in: BTreeMap::new(),
        problem_info: true,
        max_packet_out: u32::MAX,
        connect_session_expiry: 0,
    };
    if conn.open(&mut reader) {
        let end = conn.run(&mut reader);
        conn.close(end);
    } else {
        let _ = conn.tx.send(Outbound::Close);
    }
    drop(conn);
    let _ = writer_join.join();
}

impl<'a> Connection<'a> {
    fn cfg(&self) -> &ServerConfig {
        &self.shared.config
    }

    /// Whether this connection still holds the session of its ClientID:
    /// a takeover ([MQTT-3.1.4-3]) and the end of the session with 0x97
    /// (§4.1.1) remove its entry.
    fn holds_session(&self, st: &State) -> bool {
        st.conns
            .get(&self.client_id)
            .is_some_and(|e| e.id == self.id)
    }

    /// The server's state, locked, while this connection holds its
    /// session; `None` once it lost it. Its DISCONNECT is then on the way
    /// and the socket closes: a packet the connection still reads (one
    /// its reader buffered, or one that arrived before the close) is not
    /// handled against the session of the connection that replaced it.
    fn own_session(&self) -> Option<MutexGuard<'a, State>> {
        let shared: &'a Shared = self.shared;
        let st = shared.lock();
        if self.holds_session(&st) {
            Some(st)
        } else {
            None
        }
    }

    fn send(&self, bytes: Vec<u8>) {
        // The writer ends only after Close or a write error; a packet
        // queued after that has no connection to go to.
        let _ = self.tx.send(Outbound::Bytes(bytes));
    }

    fn refuse(&self, r: ConnectRefusal, props: &PropertySet) {
        if let ConnectRefusal::Connack(v, code) = r {
            // [MQTT-3.2.2-6]: Session Present 0 with a non-zero code.
            self.send(connack_bytes(v, false, code, props));
        }
    }

    fn refuse_code(&self, code: u8, props: &PropertySet) {
        self.refuse(inbound::refusal(self.version, code), props);
    }

    fn read<S: ClientStream>(
        &self,
        reader: &mut BufReader<DeadlineReader<S>>,
        deadline: Option<Instant>,
    ) -> Result<(u8, Vec<u8>), PacketReadError> {
        reader.get_mut().deadline = deadline;
        read_packet_within(reader, self.cfg().maximum_packet_size)
    }

    /// The CONNECT up to the CONNACK. `false`: refused and answered.
    #[allow(clippy::too_many_lines)]
    fn open<S: ClientStream>(&mut self, reader: &mut BufReader<DeadlineReader<S>>) -> bool {
        let shared = self.shared;
        let cfg = &shared.config;
        let connect_deadline = || Some(Instant::now() + cfg.connect_timeout);
        let Ok((b0, body)) = self.read(reader, connect_deadline()) else {
            return false;
        };
        // [MQTT-3.1.0-1]
        if b0 >> 4 != T::Connect.to_bits() {
            return false;
        }
        let c = match inbound::parse_connect(b0 & 0x0F, &body) {
            Ok(c) => c,
            Err(r) => {
                self.refuse(r, &PropertySet::default());
                return false;
            }
        };
        self.version = c.version;
        let v5 = c.version == ProtocolVersion::V5;
        if let Some(w) = &c.will {
            // [MQTT-3.2.2-12], [MQTT-3.2.2-13]
            if w.qos > cfg.maximum_qos {
                self.refuse_code(QOS_NOT_SUPPORTED, &PropertySet::default());
                return false;
            }
            if w.retain && !cfg.retain_available {
                self.refuse_code(RETAIN_NOT_SUPPORTED, &PropertySet::default());
                return false;
            }
            // §3.1.3.2.3: a Will Payload that is not the UTF-8 it says.
            if w.properties.payload_format_indicator == Some(1)
                && std::str::from_utf8(&w.payload).is_err()
            {
                self.refuse_code(PAYLOAD_FORMAT_INVALID, &PropertySet::default());
                return false;
            }
        }
        // §4.11 Server redirection.
        if let Some(r) = &cfg.redirect {
            let props = PropertySet {
                server_reference: Some(r.server_reference.clone()),
                ..PropertySet::default()
            };
            self.refuse_code(if r.permanent { 0x9D } else { 0x9C }, &props);
            return false;
        }
        let mut assigned = false;
        if c.client_id.is_empty() {
            // [MQTT-3.1.3-6]; 3.1.1 [MQTT-3.1.3-8]: only with Clean Session.
            if !cfg.allow_empty_client_id || (!v5 && !c.clean_start) {
                self.refuse_code(
                    inbound::CLIENT_IDENTIFIER_NOT_VALID,
                    &PropertySet::default(),
                );
                return false;
            }
            assigned = true;
        } else {
            self.client_id = c.client_id.clone();
        }
        // Authentication (§4.12).
        let mut auth_data_out = None;
        if let Some(method) = c.properties.authentication_method.clone() {
            let exchange = cfg
                .authenticator
                .as_ref()
                .and_then(|a| a.start(&c.client_id, &method));
            let Some(mut exchange) = exchange else {
                // [MQTT-4.12.0-1]
                self.refuse_code(BAD_AUTHENTICATION_METHOD, &PropertySet::default());
                return false;
            };
            let mut data = c.properties.authentication_data.clone();
            loop {
                match exchange.step(data.as_deref()) {
                    AuthStep::Success(d) => {
                        auth_data_out = d;
                        break;
                    }
                    AuthStep::Failure(code) => {
                        self.refuse_code(code, &PropertySet::default());
                        return false;
                    }
                    AuthStep::Continue(d) => {
                        // [MQTT-4.12.0-2], [MQTT-4.12.0-5]
                        let props = PropertySet {
                            authentication_method: Some(method.clone()),
                            authentication_data: Some(d),
                            ..PropertySet::default()
                        };
                        self.send(reason_body_bytes(T::Auth, CONTINUE_AUTHENTICATION, &props));
                        let Ok((b0, body)) = self.read(reader, connect_deadline()) else {
                            return false;
                        };
                        match T::from_bits(b0 >> 4) {
                            Some(T::Auth) => match inbound::parse_auth(b0 & 0x0F, &body) {
                                Ok((CONTINUE_AUTHENTICATION, p))
                                    if p.authentication_method.as_deref()
                                        == Some(method.as_str()) =>
                                {
                                    data = p.authentication_data;
                                }
                                Ok(_) => {
                                    self.refuse_code(PROTOCOL_ERROR, &PropertySet::default());
                                    return false;
                                }
                                Err(code) => {
                                    self.refuse_code(code, &PropertySet::default());
                                    return false;
                                }
                            },
                            Some(T::Disconnect) => return false,
                            // [MQTT-3.1.2-30]: nothing else before the CONNACK.
                            _ => {
                                self.refuse_code(PROTOCOL_ERROR, &PropertySet::default());
                                return false;
                            }
                        }
                    }
                }
            }
            self.auth_method = Some(method);
        } else if let Some(a) = &cfg.authenticator {
            if let Err(code) =
                a.authenticate(&c.client_id, c.user_name.as_deref(), c.password.as_deref())
            {
                self.refuse_code(code, &PropertySet::default());
                return false;
            }
        }

        // Session Expiry Interval: 5.0 §3.1.2.11.2; 3.1.1 Clean Session 0
        // keeps the session, 1 ends it with the connection.
        let requested = if v5 {
            c.properties.session_expiry_interval.unwrap_or(0)
        } else if c.clean_start {
            0
        } else {
            u32::MAX
        };
        let expiry = cfg
            .maximum_session_expiry
            .map_or(requested, |m| requested.min(m));
        self.connect_session_expiry = requested;
        self.problem_info = c.properties.request_problem_information != Some(0);
        self.max_packet_out = c.properties.maximum_packet_size.unwrap_or(u32::MAX);
        // [MQTT-3.2.2-21], [MQTT-3.2.2-22]: the Server Keep Alive only for a
        // client that gets it in the CONNACK; a 3.1.1 CONNACK has no
        // properties, so a 3.1.1 client's own Keep Alive holds (3.1.1
        // [MQTT-3.1.2-24]).
        let keep_alive = if v5 {
            cfg.server_keep_alive.unwrap_or(c.keep_alive)
        } else {
            c.keep_alive
        };
        self.keep_alive = KeepAliveTracker::new(keep_alive);
        let will = c.will.map(|w| Will {
            topic: w.topic,
            payload: w.payload,
            qos: w.qos,
            retain: w.retain,
            delay_interval: w.properties.will_delay_interval.unwrap_or(0),
            properties: inbound::message_properties(&w.properties),
        });

        let now = shared.now_ms();
        let mut st = shared.lock();
        // A CONNECT after `ServerHandle::shutdown`: the server is
        // not available (0x8B Server shutting down is no CONNACK Reason
        // Code, [MQTT-3.2.2-8]; 3.1.1: Return Code 0x03).
        if st.shutdown {
            drop(st);
            self.refuse_code(SERVER_UNAVAILABLE, &PropertySet::default());
            return false;
        }
        if assigned {
            self.client_id = shared.assign_client_id(&st);
        }
        let client_id = self.client_id.clone();
        // [MQTT-3.1.4-3]: the existing connection gets 0x8E and is closed;
        // its Will follows the rules of [MQTT-3.1.2-8].
        if let Some(old) = st.conns.remove(&client_id) {
            send_disconnect(&old, SESSION_TAKEN_OVER, &PropertySet::default());
            let envs = st.broker.disconnect_session(&client_id, true, now);
            route(&mut st, envs, now, cfg);
        }
        let receive_maximum = c.properties.receive_maximum.unwrap_or(u16::MAX);
        // [MQTT-3.1.4-4]
        let outcome = st
            .broker
            .connect_session(&client_id, c.clean_start, expiry, will);
        st.conns.insert(
            client_id.clone(),
            ConnEntry {
                id: self.id,
                tx: self.tx.clone(),
                version: self.version,
                receive_maximum,
                maximum_packet_size: self.max_packet_out,
                topic_alias_maximum: c.properties.topic_alias_maximum.unwrap_or(0),
                aliases_out: BTreeMap::new(),
            },
        );
        let props = if v5 {
            PropertySet {
                session_expiry_interval: (expiry != requested).then_some(expiry),
                receive_maximum: (cfg.receive_maximum != u16::MAX).then_some(cfg.receive_maximum),
                maximum_qos: (cfg.maximum_qos != QoS::ExactlyOnce)
                    .then_some(cfg.maximum_qos.to_u8()),
                retain_available: (!cfg.retain_available).then_some(0),
                maximum_packet_size: Some(cfg.maximum_packet_size),
                assigned_client_identifier: assigned.then(|| client_id.clone()),
                topic_alias_maximum: (cfg.topic_alias_maximum > 0)
                    .then_some(cfg.topic_alias_maximum),
                wildcard_subscription_available: (!cfg.wildcard_subscription_available)
                    .then_some(0),
                subscription_identifier_available: (!cfg.subscription_identifiers_available)
                    .then_some(0),
                shared_subscription_available: (!cfg.shared_subscription_available).then_some(0),
                server_keep_alive: cfg.server_keep_alive,
                // [MQTT-3.1.2-28]
                response_information: if c.properties.request_response_information == Some(1) {
                    cfg.response_information.clone()
                } else {
                    None
                },
                authentication_method: self.auth_method.clone(),
                authentication_data: auth_data_out,
                ..PropertySet::default()
            }
        } else {
            PropertySet::default()
        };
        // [MQTT-3.1.4-5], [MQTT-3.2.0-1], [MQTT-3.2.2-2], [MQTT-3.2.2-3]
        self.send(connack_bytes(
            self.version,
            outcome.session_present,
            0x00,
            &props,
        ));
        // §4.4: the unacknowledged PUBLISH and PUBREL packets again, the
        // PUBLISH packets within the client's Receive Maximum on this
        // connection (§4.9); `pump` sends the rest as the quota frees.
        for r in st.broker.resend(&client_id, receive_maximum) {
            match r {
                Resend::Publish(pid, msg) => {
                    let State { broker, conns, .. } = &mut *st;
                    let Some(entry) = conns.get_mut(&client_id) else {
                        break;
                    };
                    match publish_bytes(entry, &msg, Some(pid), true, now, cfg) {
                        Some(b) => self.send(b),
                        None => broker.complete_unsent(&client_id, pid),
                    }
                }
                Resend::PubRel(pid) => {
                    self.send(ack_bytes(
                        self.version,
                        T::PubRel,
                        pid,
                        0,
                        &PropertySet::default(),
                    ));
                }
            }
        }
        pump(&mut st, &client_id, now, cfg);
        drop(st);
        shared.notify();
        true
    }

    /// The packets of the session until the connection ends.
    fn run<S: ClientStream>(&mut self, reader: &mut BufReader<DeadlineReader<S>>) -> End {
        loop {
            // [MQTT-3.1.2-22], [MQTT-3.2.2-22]: one and a half Keep Alive
            // periods after the last packet.
            self.keep_alive.record_packet();
            let deadline = self
                .keep_alive
                .remaining()
                .map(|left| Instant::now() + left);
            let (b0, body) = match self.read(reader, deadline) {
                Ok(p) => p,
                Err(PacketReadError::TooLarge { .. }) => return End::Error(PACKET_TOO_LARGE),
                Err(PacketReadError::MalformedLength) => return End::Error(MALFORMED_PACKET),
                Err(PacketReadError::Io(_)) => return End::Network,
            };
            match self.handle(b0, &body) {
                Ok(None) => {}
                Ok(Some(end)) => return end,
                Err(code) => return End::Error(code),
            }
        }
    }

    /// Ends the connection: DISCONNECT with the Reason Code of an error
    /// ([MQTT-4.13.1-1], [MQTT-4.13.2-1]), the close, the session after it
    /// and the Will unless the client sent DISCONNECT 0x00
    /// ([MQTT-3.14.4-3]). A connection that was taken over leaves all of
    /// that to the one that took it over.
    fn close(&mut self, end: End) {
        let now = self.shared.now_ms();
        let mut st = self.shared.lock();
        if !self.holds_session(&st) {
            let _ = self.tx.send(Outbound::Close);
            return;
        }
        if let Some(entry) = st.conns.remove(&self.client_id) {
            match end {
                End::Error(code) => send_disconnect(&entry, code, &PropertySet::default()),
                _ => {
                    let _ = entry.tx.send(Outbound::Close);
                }
            }
        }
        let publish_will = !matches!(
            end,
            End::Client {
                publish_will: false
            }
        );
        let envs = st
            .broker
            .disconnect_session(&self.client_id, publish_will, now);
        route(&mut st, envs, now, &self.shared.config);
        drop(st);
        self.shared.notify();
    }

    /// A Reason String for a packet other than PUBLISH, CONNACK and
    /// DISCONNECT, only when the client did not set Request Problem
    /// Information to 0 ([MQTT-3.1.2-29]).
    fn diagnostics(&self, text: &str) -> PropertySet {
        PropertySet {
            reason_string: self.problem_info.then(|| text.into()),
            ..PropertySet::default()
        }
    }

    /// Sends the packet built with `props`, or without them where they
    /// would exceed the client's Maximum Packet Size ([MQTT-3.4.2-2],
    /// [MQTT-3.9.2-1], [MQTT-3.11.2-1] and their siblings).
    fn send_fitted(&self, build: impl Fn(&PropertySet) -> Vec<u8>, props: &PropertySet) {
        let mut bytes = build(props);
        if bytes.len() as u64 > u64::from(self.max_packet_out) {
            bytes = build(&PropertySet::default());
        }
        self.send(bytes);
    }

    fn send_ack(&self, kind: T, pid: u16, code: u8, text: Option<&str>) {
        let props = text.map_or_else(PropertySet::default, |t| self.diagnostics(t));
        let v = self.version;
        self.send_fitted(|p| ack_bytes(v, kind, pid, code, p), &props);
    }

    /// One packet of the session. `Ok(Some)` ends the connection; a packet
    /// that would change the session after the connection lost it ends
    /// the connection unhandled ([`Self::own_session`]).
    fn handle(&mut self, b0: u8, body: &[u8]) -> Result<Option<End>, u8> {
        let flags = b0 & 0x0F;
        let v = self.version;
        let Some(kind) = T::from_bits(b0 >> 4) else {
            // Type 0 is reserved (§2.1.2).
            return Err(MALFORMED_PACKET);
        };
        match kind {
            // [MQTT-3.1.0-2]
            T::Connect => Err(PROTOCOL_ERROR),
            T::ConnAck | T::SubAck | T::UnsubAck | T::PingResp => Err(PROTOCOL_ERROR),
            T::Publish => self.on_publish(flags, body),
            T::PubAck => {
                let (pid, _, _) = inbound::parse_ack(v, kind, flags, body)?;
                let now = self.shared.now_ms();
                let Some(mut st) = self.own_session() else {
                    return Ok(Some(End::Network));
                };
                if st.broker.puback(&self.client_id, pid) {
                    pump(&mut st, &self.client_id, now, self.cfg());
                }
                Ok(None)
            }
            T::PubRec => {
                let (pid, code, _) = inbound::parse_ack(v, kind, flags, body)?;
                let now = self.shared.now_ms();
                let Some(mut st) = self.own_session() else {
                    return Ok(Some(End::Network));
                };
                match st.broker.pubrec(&self.client_id, pid, code) {
                    // [MQTT-4.3.3-4]
                    PubRecOutcome::Release => self.send_ack(T::PubRel, pid, 0, None),
                    PubRecOutcome::NotFound => self.send_ack(
                        T::PubRel,
                        pid,
                        PACKET_IDENTIFIER_NOT_FOUND,
                        Some("no QoS 2 delivery with this Packet Identifier"),
                    ),
                    PubRecOutcome::Discarded => pump(&mut st, &self.client_id, now, self.cfg()),
                }
                Ok(None)
            }
            T::PubRel => {
                let (pid, _, _) = inbound::parse_ack(v, kind, flags, body)?;
                let Some(mut st) = self.own_session() else {
                    return Ok(Some(End::Network));
                };
                let found = st.broker.qos2_released(&self.client_id, pid);
                drop(st);
                // [MQTT-4.3.3-11]
                if found {
                    self.send_ack(T::PubComp, pid, 0, None);
                } else {
                    self.send_ack(
                        T::PubComp,
                        pid,
                        PACKET_IDENTIFIER_NOT_FOUND,
                        Some("no QoS 2 message with this Packet Identifier"),
                    );
                }
                Ok(None)
            }
            T::PubComp => {
                let (pid, _, _) = inbound::parse_ack(v, kind, flags, body)?;
                let now = self.shared.now_ms();
                let Some(mut st) = self.own_session() else {
                    return Ok(Some(End::Network));
                };
                if st.broker.pubcomp(&self.client_id, pid) {
                    pump(&mut st, &self.client_id, now, self.cfg());
                }
                Ok(None)
            }
            T::Subscribe => self.on_subscribe(flags, body),
            T::Unsubscribe => self.on_unsubscribe(flags, body),
            T::PingReq => {
                if flags != 0 || !body.is_empty() {
                    return Err(MALFORMED_PACKET);
                }
                // [MQTT-3.12.4-1]
                self.send(frame(T::PingResp, 0, &[]));
                Ok(None)
            }
            T::Disconnect => {
                let (code, props) = inbound::parse_disconnect(v, flags, body)?;
                if let Some(x) = props.session_expiry_interval {
                    // §3.14.2.2.2
                    if self.connect_session_expiry == 0 && x != 0 {
                        return Err(PROTOCOL_ERROR);
                    }
                    let x = self.cfg().maximum_session_expiry.map_or(x, |m| x.min(m));
                    let Some(mut st) = self.own_session() else {
                        return Ok(Some(End::Network));
                    };
                    st.broker.set_session_expiry(&self.client_id, x);
                }
                Ok(Some(End::Client {
                    publish_will: code != 0x00,
                }))
            }
            T::Auth => {
                if v != ProtocolVersion::V5 {
                    // 3.1.1: packet type 15 is reserved.
                    return Err(MALFORMED_PACKET);
                }
                self.on_auth(flags, body).map(|()| None)
            }
        }
    }

    /// The Topic Name of a PUBLISH with its Topic Alias (§3.3.2.3.4).
    fn resolve_topic(&mut self, topic: String, alias: Option<u16>) -> Result<String, u8> {
        match alias {
            None if topic.is_empty() => Err(PROTOCOL_ERROR),
            None => Ok(topic),
            // [MQTT-3.3.2-12]: up to the server's Topic Alias Maximum.
            Some(a) if a > self.cfg().topic_alias_maximum => Err(TOPIC_ALIAS_INVALID),
            Some(a) if topic.is_empty() => self.aliases_in.get(&a).cloned().ok_or(PROTOCOL_ERROR),
            Some(a) => {
                self.aliases_in.insert(a, topic.clone());
                Ok(topic)
            }
        }
    }

    fn on_publish(&mut self, flags: u8, body: &[u8]) -> Result<Option<End>, u8> {
        let p = inbound::parse_publish(self.version, flags, body)?;
        // [MQTT-3.3.4-6]
        if !p.properties.subscription_identifiers.is_empty() {
            return Err(PROTOCOL_ERROR);
        }
        // §3.2.2.3.4, §3.2.2.3.5
        if p.qos > self.cfg().maximum_qos {
            return Err(QOS_NOT_SUPPORTED);
        }
        if p.retain && !self.cfg().retain_available {
            return Err(RETAIN_NOT_SUPPORTED);
        }
        let topic = self.resolve_topic(p.topic, p.properties.topic_alias)?;
        // [MQTT-3.3.2-2], [MQTT-4.7.0-1]
        if topic.contains(['+', '#']) {
            return Err(TOPIC_NAME_INVALID);
        }
        if inbound::response_topic_invalid(&p.properties) {
            return Err(PROTOCOL_ERROR);
        }
        let ack_kind = if p.qos == QoS::ExactlyOnce {
            T::PubRec
        } else {
            T::PubAck
        };
        // §3.3.2.3.2: a payload that is not the UTF-8 it says it is.
        if p.properties.payload_format_indicator == Some(1)
            && std::str::from_utf8(&p.payload).is_err()
        {
            return match p.packet_id {
                Some(pid) => {
                    self.send_ack(
                        ack_kind,
                        pid,
                        PAYLOAD_FORMAT_INVALID,
                        Some("payload is not UTF-8"),
                    );
                    Ok(None)
                }
                None => Err(PAYLOAD_FORMAT_INVALID),
            };
        }
        let now = self.shared.now_ms();
        let shared = self.shared;
        let cfg = &shared.config;
        let Some(mut st) = self.own_session() else {
            return Ok(Some(End::Network));
        };
        if let (QoS::ExactlyOnce, Some(pid)) = (p.qos, p.packet_id) {
            let held = st
                .broker
                .session(&self.client_id)
                .is_some_and(|s| s.inbound_qos2.contains(&pid));
            if held {
                // [MQTT-4.3.3-10]: answered, not delivered again.
                drop(st);
                self.send_ack(T::PubRec, pid, 0, None);
                return Ok(None);
            }
            // §3.3.4: more unacknowledged QoS 2 messages than Receive Maximum.
            if st.broker.inbound_qos2_count(&self.client_id) >= usize::from(cfg.receive_maximum) {
                return Err(RECEIVE_MAXIMUM_EXCEEDED);
            }
            st.broker.qos2_received(&self.client_id, pid);
        }
        let msg = AppMessage {
            topic,
            payload: p.payload,
            qos: p.qos,
            retain: p.retain,
            properties: inbound::message_properties(&p.properties),
            publisher: Some(self.client_id.clone()),
        };
        // [MQTT-4.5.0-1]
        let envs = st.broker.publish_message(&msg, now);
        let matched = !envs.is_empty();
        route(&mut st, envs, now, cfg);
        drop(st);
        // [MQTT-3.3.4-1], [MQTT-4.3.2-4], [MQTT-4.3.3-8]
        if let Some(pid) = p.packet_id {
            if matched {
                self.send_ack(ack_kind, pid, 0, None);
            } else {
                self.send_ack(ack_kind, pid, NO_MATCHING_SUBSCRIBERS, None);
            }
        }
        Ok(None)
    }

    fn on_subscribe(&mut self, flags: u8, body: &[u8]) -> Result<Option<End>, u8> {
        let shared = self.shared;
        let cfg = &shared.config;
        let v5 = self.version == ProtocolVersion::V5;
        let sub = inbound::parse_subscribe(self.version, flags, body)?;
        let id = sub.properties.subscription_identifiers.first().copied();
        // §3.2.2.3.12
        if id.is_some() && !cfg.subscription_identifiers_available {
            return Err(SUBSCRIPTION_IDENTIFIERS_NOT_SUPPORTED);
        }
        let mut codes = std::vec![0u8; sub.entries.len()];
        let mut slots = Vec::new();
        let mut valid = Vec::new();
        // §3.2.2.3.11, §3.2.2.3.13: a SUBSCRIBE with a feature the CONNACK
        // announced as not supported is a Protocol Error for a 5.0 client
        // (DISCONNECT 0xA2 or 0x9E, [MQTT-4.13.1-1]); a 3.1.1 client knows
        // no such announcement and gets 0x80 for the entry.
        let unsupported = |code: u8| if v5 { Err(code) } else { Ok(Some(code)) };
        for (i, (filter, options)) in sub.entries.into_iter().enumerate() {
            let no_local = options & 0x04 != 0;
            let code = match parse_shared_filter(&filter) {
                Err(_) => Some(TOPIC_FILTER_INVALID),
                // [MQTT-3.8.3-4]
                Ok(Some(_)) if no_local => return Err(PROTOCOL_ERROR),
                Ok(Some(_)) if !cfg.shared_subscription_available => {
                    unsupported(SHARED_SUBSCRIPTIONS_NOT_SUPPORTED)?
                }
                Ok(shared) => {
                    let inner = shared.map_or(filter.as_str(), |(_, f)| f);
                    if validate_filter(inner).is_err() {
                        Some(TOPIC_FILTER_INVALID)
                    } else if !cfg.wildcard_subscription_available && inner.contains(['+', '#']) {
                        unsupported(WILDCARD_SUBSCRIPTIONS_NOT_SUPPORTED)?
                    } else {
                        None
                    }
                }
            };
            if let Some(c) = code {
                codes[i] = if v5 { c } else { UNSPECIFIED_ERROR };
                continue;
            }
            let requested = QoS::from_u8(options & 0x03).map_err(|()| PROTOCOL_ERROR)?;
            slots.push(i);
            valid.push(Subscription {
                filter,
                // [MQTT-3.2.2-10]: granted at most the server's Maximum QoS.
                max_qos: requested.min(cfg.maximum_qos),
                no_local,
                retain_as_published: options & 0x08 != 0,
                retain_handling: (options >> 4) & 0x03,
                subscription_identifier: id,
            });
        }
        let now = shared.now_ms();
        let Some(mut st) = self.own_session() else {
            return Ok(Some(End::Network));
        };
        let (granted, envs) = st
            .broker
            .subscribe_with(&self.client_id, valid, now)
            .map_err(|_| UNSPECIFIED_ERROR)?;
        for (slot, g) in slots.into_iter().zip(granted) {
            codes[slot] = match g {
                Ok(q) => q.to_u8(),
                Err(c) if v5 => c,
                Err(_) => UNSPECIFIED_ERROR,
            };
        }
        let failed = codes.iter().any(|c| *c >= 0x80);
        let props = if failed {
            self.diagnostics("a subscription was refused")
        } else {
            PropertySet::default()
        };
        // [MQTT-3.8.4-1], [MQTT-3.8.4-2], [MQTT-3.8.4-6], [MQTT-3.9.3-1]:
        // before the retained messages of the new subscriptions.
        let (v, pid) = (self.version, sub.packet_id);
        self.send_fitted(
            |p| {
                let body = encode_suback_body_v(
                    &SubackBody {
                        packet_id: pid,
                        properties: encode_props(p),
                        reason_codes: codes.clone(),
                    },
                    v,
                )
                .unwrap_or_default();
                frame(T::SubAck, 0, &body)
            },
            &props,
        );
        route(&mut st, envs, now, cfg);
        Ok(None)
    }

    fn on_unsubscribe(&mut self, flags: u8, body: &[u8]) -> Result<Option<End>, u8> {
        let u = inbound::parse_unsubscribe(self.version, flags, body)?;
        let Some(mut st) = self.own_session() else {
            return Ok(Some(End::Network));
        };
        let existed = st
            .broker
            .unsubscribe_each(&self.client_id, &u.filters)
            .map_err(|_| UNSPECIFIED_ERROR)?;
        drop(st);
        // [MQTT-3.11.3-1], [MQTT-3.11.3-2]
        let codes: Vec<u8> = u
            .filters
            .iter()
            .zip(existed)
            .map(|(f, existed)| {
                let valid = match parse_shared_filter(f) {
                    Ok(Some(_)) => true,
                    Ok(None) => validate_filter(f).is_ok(),
                    Err(_) => false,
                };
                if !valid {
                    TOPIC_FILTER_INVALID
                } else if existed {
                    0x00
                } else {
                    NO_SUBSCRIPTION_EXISTED
                }
            })
            .collect();
        let props = if codes.contains(&TOPIC_FILTER_INVALID) {
            self.diagnostics("a topic filter is invalid")
        } else {
            PropertySet::default()
        };
        // [MQTT-3.10.4-4], [MQTT-3.10.4-5]
        let (v, pid) = (self.version, u.packet_id);
        self.send_fitted(
            |p| {
                let body = encode_unsuback_body_v(
                    &SubackBody {
                        packet_id: pid,
                        properties: encode_props(p),
                        reason_codes: codes.clone(),
                    },
                    v,
                )
                .unwrap_or_default();
                frame(T::UnsubAck, 0, &body)
            },
            &props,
        );
        Ok(None)
    }

    /// Re-authentication (§4.12.1).
    fn on_auth(&mut self, flags: u8, body: &[u8]) -> Result<(), u8> {
        let (code, props) = inbound::parse_auth(flags, body)?;
        // [MQTT-4.12.0-7]: no AUTH without an Authentication Method.
        let Some(method) = self.auth_method.clone() else {
            return Err(PROTOCOL_ERROR);
        };
        // [MQTT-4.12.1-1]
        if props.authentication_method.as_deref() != Some(method.as_str()) {
            return Err(PROTOCOL_ERROR);
        }
        match code {
            REAUTHENTICATE if self.auth.is_none() => {
                let exchange = self
                    .cfg()
                    .authenticator
                    .as_ref()
                    .and_then(|a| a.start(&self.client_id, &method))
                    .ok_or(BAD_AUTHENTICATION_METHOD)?;
                self.auth = Some(exchange);
            }
            CONTINUE_AUTHENTICATION if self.auth.is_some() => {}
            _ => return Err(PROTOCOL_ERROR),
        }
        let step = match self.auth.as_mut() {
            Some(a) => a.step(props.authentication_data.as_deref()),
            None => return Err(PROTOCOL_ERROR),
        };
        let (code, data) = match step {
            AuthStep::Continue(d) => (CONTINUE_AUTHENTICATION, Some(d)),
            AuthStep::Success(d) => {
                self.auth = None;
                (0x00, d)
            }
            // [MQTT-4.12.1-2]
            AuthStep::Failure(c) => return Err(c),
        };
        // [MQTT-4.12.0-5]
        let props = PropertySet {
            authentication_method: Some(method),
            authentication_data: data,
            ..PropertySet::default()
        };
        self.send(reason_body_bytes(T::Auth, code, &props));
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// What the writer wrote, and whether it closed.
    struct Sink(Arc<Mutex<(Vec<u8>, bool)>>);

    impl io::Write for Sink {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().0.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl ClientWriter for Sink {
        fn close(&mut self) {
            self.0.lock().unwrap().1 = true;
        }
    }

    fn entry(version: ProtocolVersion, tx: Sender<Outbound>) -> ConnEntry {
        ConnEntry {
            id: 1,
            tx,
            version,
            receive_maximum: 1,
            maximum_packet_size: u32::MAX,
            topic_alias_maximum: 0,
            aliases_out: BTreeMap::new(),
        }
    }

    #[test]
    fn mqtt5_3_14_4_1_nothing_follows_the_servers_disconnect() {
        let (tx, rx) = channel();
        let e = entry(ProtocolVersion::V5, tx.clone());
        send_disconnect(&e, SESSION_TAKEN_OVER, &PropertySet::default());
        // A PUBACK another thread queues right after the DISCONNECT.
        tx.send(Outbound::Bytes(std::vec![0x40, 2, 0, 1])).unwrap();
        drop(e);
        drop(tx);
        let queued: Vec<Outbound> = rx.try_iter().collect();
        // The DISCONNECT and the close are one message: nothing can come
        // between them.
        assert_eq!(queued.len(), 2);
        assert!(matches!(queued[0], Outbound::Disconnect(_)));
        let (tx, rx) = channel();
        for m in queued {
            tx.send(m).unwrap();
        }
        drop(tx);
        let out = Arc::new(Mutex::new((Vec::new(), false)));
        spawn_writer(Box::new(Sink(Arc::clone(&out))), rx)
            .join()
            .unwrap();
        let (bytes, closed) = &*out.lock().unwrap();
        assert_eq!(
            bytes,
            &reason_body_bytes(T::Disconnect, SESSION_TAKEN_OVER, &PropertySet::default()),
            "[MQTT-3.14.4-1]"
        );
        assert!(closed);
    }

    #[test]
    fn mqtt5_3_14_4_1_a_3_1_1_client_is_closed_without_a_disconnect() {
        let (tx, rx) = channel();
        send_disconnect(
            &entry(ProtocolVersion::V311, tx),
            SESSION_TAKEN_OVER,
            &PropertySet::default(),
        );
        let queued: Vec<Outbound> = rx.try_iter().collect();
        assert_eq!(queued.len(), 1);
        assert!(matches!(queued[0], Outbound::Close));
    }
}
