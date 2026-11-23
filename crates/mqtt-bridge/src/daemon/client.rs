// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge's MQTT client towards an external broker. Spec §4.1-§4.3,
//! MQTT 5.0 and, as the compatibility mode of Spec §1 L1, MQTT 3.1.1.
//!
//! Synchronous, on a TCP, TLS or WebSocket stream. [`MqttClient::connect_url`]
//! sends CONNECT and reads CONNACK; then SUBSCRIBE and PUBLISH in both
//! directions. What the CONNECT announces holds for the broker's packets:
//! Maximum Packet Size (0x95 beyond it), Topic Alias Maximum (the aliases
//! are resolved, 0x94 beyond it), Receive Maximum (0x93 beyond it). What the
//! CONNACK announces holds for the bridge's packets: Maximum Packet Size,
//! Maximum QoS, Retain Available, Receive Maximum, Server Keep Alive. The
//! client sends PINGREQ when it has sent nothing for the Keep Alive and
//! closes the connection when the PINGRESP does not come within half of it
//! (MQTT 5.0 §3.1.2.10).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::string::{String, ToString};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::vec::Vec;

use crate::codec::{PublishPacket, decode_publish_v, encode_publish_v};
use crate::control_packets::{
    AckBody, AuthBody, ConnectBody, DisconnectBody, SubscribeBody,
    Subscription as MqttSubscription, connect_flags, decode_ack_body, decode_auth_body,
    decode_connack_body_v, decode_disconnect_body, decode_suback_body_v, encode_ack_body_v,
    encode_auth_body, encode_connect_body_v, encode_disconnect_body_v, encode_subscribe_body_v,
};
use crate::packet::{ControlPacketType, FixedHeader};
use crate::property_set::{MALFORMED_PACKET, PropertyContext, PropertySet};
use crate::vbi::{decode_vbi, encode_vbi};
use crate::version::ProtocolVersion;

use super::auth::{Credentials, Exchange};
use super::config::DaemonConfig;
use super::log::{self, Level};
use super::session::{OutboundState, Resend, SessionState};
use super::trace::{PublishOut, Tracer, unix_ns};
use rustls::{ClientConfig, ClientConnection, StreamOwned};

/// Reason Code 0x93 Receive Maximum exceeded (MQTT 5.0 §2.4, §4.9).
pub const RECEIVE_MAXIMUM_EXCEEDED: u8 = 0x93;
/// Reason Code 0x94 Topic Alias invalid (MQTT 5.0 §2.4, §3.3.2.3.4).
pub const TOPIC_ALIAS_INVALID: u8 = 0x94;
/// Reason Code 0x95 Packet too large (MQTT 5.0 §2.4, §3.1.2.11.4).
pub const PACKET_TOO_LARGE: u8 = 0x95;
/// Reason Code 0x82 Protocol Error (MQTT 5.0 §2.4).
pub const PROTOCOL_ERROR: u8 = 0x82;
/// Reason Code 0x80 Unspecified error (MQTT 5.0 §2.4): the DISCONNECT of a
/// client whose side of an enhanced authentication failed.
pub const UNSPECIFIED_ERROR: u8 = 0x80;
/// Reason Code 0x18 Continue authentication (MQTT 5.0 §3.15.2.1).
pub const CONTINUE_AUTHENTICATION: u8 = 0x18;
/// Reason Code 0x19 Re-authenticate (MQTT 5.0 §3.15.2.1).
pub const RE_AUTHENTICATE: u8 = 0x19;

/// The user properties of the CONNECT (Spec §4.1).
pub const CONNECT_USER_PROPERTIES: [(&str, &str); 2] =
    [("zerodds_version", "1.0"), ("zerodds_role", "bridge")];

/// Error during the client lifecycle.
#[derive(Debug)]
pub enum ClientError {
    /// TCP/IO error.
    Io(String),
    /// The TLS handshake failed: the broker's certificate is not trusted or
    /// not for its name, no common protocol (Spec §2 exit 4).
    Tls(String),
    /// Wire-codec error.
    Codec(String),
    /// The read timeout elapsed before a whole packet arrived; what did
    /// arrive stays buffered for the next read.
    TimedOut,
    /// The broker refused the connection in its CONNACK.
    ConnAck {
        /// The Reason Code (MQTT 5.0 §3.2.2.2) or the Connect Return Code
        /// (MQTT 3.1.1 §3.2.2.3).
        reason: u8,
        /// The protocol of the CONNACK, which the code is one of.
        version: ProtocolVersion,
        /// The Reason String of an MQTT 5.0 CONNACK, empty without one.
        detail: String,
    },
    /// The broker broke a rule of the protocol; the client sent DISCONNECT
    /// with `reason` and closed the connection (MQTT 5.0 §4.13).
    Protocol {
        /// The Reason Code of the DISCONNECT.
        reason: u8,
        /// What the broker did.
        what: String,
    },
    /// A packet the broker's limits of its CONNACK forbid (Maximum Packet
    /// Size, Maximum QoS, Retain Available); it was not sent and the
    /// connection stands.
    NotSent(String),
    /// A QoS 1/2 PUBLISH found the broker's Receive Maximum reached and no
    /// acknowledgement freed a slot within [`SEND_SLOT_WAIT`]; it was not
    /// sent, the connection stands, and it may be sent again.
    Busy(String),
    /// The Session State of a lasting session cannot be kept: its file in
    /// `mqtt.session_dir` cannot be read or written, or another process
    /// holds it (Spec §4.1; exit 1 at the start).
    Session(String),
    /// The Message Expiry Interval of a PUBLISH had passed when it was due
    /// to go out (Spec §6); it was not sent, the connection stands.
    Expired(String),
    /// The client's side of an enhanced authentication failed (MQTT 5.0
    /// §4.12): the broker's SCRAM signature does not verify, its challenge
    /// does not continue the method (Spec §2 exit 5).
    Auth(String),
}

impl core::fmt::Display for ClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(m) => write!(f, "io: {m}"),
            Self::Tls(m) => write!(f, "tls: {m}"),
            Self::Codec(m) => write!(f, "codec: {m}"),
            Self::TimedOut => f.write_str("io: read timed out"),
            Self::ConnAck {
                reason,
                version,
                detail,
            } => {
                let kind = match version {
                    ProtocolVersion::V5 => "reason",
                    ProtocolVersion::V311 => "MQTT 3.1.1 return code",
                };
                write!(f, "connack reject: {kind} 0x{reason:02x}")?;
                if !detail.is_empty() {
                    write!(f, " ({detail})")?;
                }
                Ok(())
            }
            Self::Protocol { reason, what } => {
                write!(f, "protocol: {what}; DISCONNECT 0x{reason:02x}")
            }
            Self::NotSent(m) => write!(f, "not sent: {m}"),
            Self::Busy(m) => write!(f, "busy: {m}"),
            Self::Session(m) => write!(f, "session state: {m}"),
            Self::Expired(m) => write!(f, "expired: {m}"),
            Self::Auth(m) => write!(f, "enhanced authentication: {m}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl ClientError {
    /// Whether the broker refused the credentials or the authorization in
    /// its CONNACK: 0x86 Bad User Name or Password, 0x87 Not authorized,
    /// 0x8A Banned, 0x8C Bad authentication method of MQTT 5.0; the return
    /// codes 4 and 5 of MQTT 3.1.1 (Spec §2 exit 5).
    #[must_use]
    pub fn is_auth_refusal(&self) -> bool {
        match self {
            Self::Auth(_) => true,
            Self::ConnAck {
                reason,
                version: ProtocolVersion::V5,
                ..
            } => matches!(reason, 0x86 | 0x87 | 0x8A | 0x8C),
            Self::ConnAck {
                reason,
                version: ProtocolVersion::V311,
                ..
            } => matches!(reason, 4 | 5),
            _ => false,
        }
    }

    /// Whether another attempt may succeed: the broker cannot be reached or
    /// does not answer in time, or it refused with 0x88 Server unavailable,
    /// 0x89 Server busy, 0x97 Quota exceeded, 0x9F Connection rate exceeded
    /// (MQTT 3.1.1: 3 Server unavailable).
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Io(_) | Self::TimedOut => true,
            Self::ConnAck {
                reason,
                version: ProtocolVersion::V5,
                ..
            } => matches!(reason, 0x88 | 0x89 | 0x97 | 0x9F),
            Self::ConnAck {
                reason,
                version: ProtocolVersion::V311,
                ..
            } => *reason == 3,
            _ => false,
        }
    }
}

/// Inbound event from the broker — the caller handles PUBLISH frames
/// as DDS writes.
#[derive(Debug, Clone)]
pub enum InboundEvent {
    /// PUBLISH from another MQTT client.
    Publish {
        /// MQTT topic, a Topic Alias resolved.
        topic: String,
        /// Payload.
        payload: Vec<u8>,
        /// QoS level.
        qos: u8,
        /// The properties of the PUBLISH (MQTT 5.0 §3.3.2.3), checked
        /// against Table 2-4; empty under MQTT 3.1.1.
        properties: Box<PropertySet>,
    },
    /// Connection lost.
    Disconnected(String),
}

/// Connection-stream variant for the MQTT client layer. Plain TCP
/// vs. TLS-wrapped (Spec §7.1 — `mqtts://` path).
pub(crate) enum MqttStream {
    /// Plain TCP — `tls_enabled=false` or `mqtt://`.
    Plain(TcpStream),
    /// Client-side TLS stream with an owned connection + socket.
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
    /// MQTT in WebSocket frames over one of the two above (`ws://`,
    /// `wss://`).
    Ws(Box<super::ws_transport::WsStream>),
}

impl MqttStream {
    fn socket(&self) -> &TcpStream {
        match self {
            Self::Plain(s) => s,
            Self::Tls(s) => &s.sock,
            Self::Ws(s) => s.wire().socket(),
        }
    }
    fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        self.socket().set_read_timeout(dur)
    }
    /// Whether input is buffered above the socket, which a wait on the
    /// socket would not see: TLS records rustls has read but not handed out,
    /// WebSocket frame bytes.
    pub(crate) fn has_buffered_input(&mut self) -> bool {
        match self {
            Self::Plain(_) => false,
            Self::Tls(s) => s
                .conn
                .process_new_packets()
                .map(|st| st.plaintext_bytes_to_read() > 0)
                .unwrap_or(true),
            Self::Ws(s) => s.has_buffered_input(),
        }
    }
    fn set_write_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        self.socket().set_write_timeout(dur)
    }
    pub(super) fn shutdown_both(&mut self) {
        if let Self::Ws(s) = self {
            s.close();
        }
        let _ = self.socket().shutdown(std::net::Shutdown::Both);
    }
}

impl Read for MqttStream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(b),
            Self::Tls(s) => s.read(b),
            Self::Ws(s) => s.read(b),
        }
    }
}

impl Write for MqttStream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(b),
            Self::Tls(s) => s.write(b),
            Self::Ws(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Tls(s) => s.flush(),
            Self::Ws(s) => s.flush(),
        }
    }
}

/// How long a QoS 1/2 publish waits for a free slot under the broker's
/// Receive Maximum before it fails with [`ClientError::Busy`].
pub const SEND_SLOT_WAIT: Duration = Duration::from_secs(10);

/// What the broker's CONNACK allows the bridge (MQTT 5.0 §3.2.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerLimits {
    /// 0x21 Receive Maximum: our QoS 1/2 PUBLISH packets unacknowledged at
    /// once (65 535 when the broker names none).
    pub receive_maximum: u16,
    /// 0x27 Maximum Packet Size: the largest packet we may send; `None`
    /// without a limit.
    pub maximum_packet_size: Option<u32>,
    /// 0x24 Maximum QoS: the highest QoS of our PUBLISH packets (2 when the
    /// broker names none).
    pub maximum_qos: u8,
    /// 0x25 Retain Available: whether we may send a retained PUBLISH.
    pub retain_available: bool,
    /// 0x22 Topic Alias Maximum the broker accepts from us (0: none).
    pub topic_alias_maximum: u16,
    /// 0x28 Wildcard Subscription Available: whether a Topic Filter may
    /// carry `+` or `#` (true when the broker names none).
    pub wildcard_subscriptions: bool,
    /// 0x29 Subscription Identifier Available: whether a SUBSCRIBE may
    /// carry a Subscription Identifier (true when the broker names none).
    pub subscription_identifiers: bool,
}

impl Default for BrokerLimits {
    fn default() -> Self {
        Self {
            receive_maximum: u16::MAX,
            maximum_packet_size: None,
            maximum_qos: 2,
            retain_available: true,
            topic_alias_maximum: 0,
            wildcard_subscriptions: true,
            subscription_identifiers: true,
        }
    }
}

/// The No Local option of a subscription (MQTT 5.0 §3.8.3.1, bit 2): the
/// broker does not send the client's own PUBLISH packets back to it.
pub const NO_LOCAL: u8 = 0x04;

/// One subscription the daemon asks for (Spec §4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeEntry {
    /// The Topic Filter.
    pub filter: String,
    /// The MQTT QoS it asks for.
    pub qos: u8,
    /// Its Subscription Identifier (1 to 268 435 455).
    pub id: u32,
}

/// What the broker answered to one [`SubscribeEntry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscribeOutcome {
    /// Granted with this QoS (SUBACK Reason Code 0x00 to 0x02).
    Granted(u8),
    /// Refused with this Reason Code (0x80 or above; MQTT 3.1.1: 0x80).
    Refused(u8),
    /// Not sent: the broker's CONNACK says it has no wildcard
    /// subscriptions (MQTT 5.0 §3.2.2.3.14).
    NoWildcards,
}

/// MQTT client. Manages a TCP, TLS or WebSocket stream and the wire loop.
pub struct MqttClient {
    stream: MqttStream,
    /// The protocol of the connection.
    version: ProtocolVersion,
    /// Next packet identifier to allocate.
    next_packet_id: u16,
    /// Bytes read from the stream that do not yet form a whole packet. A
    /// read timeout can fall inside a packet; the part already read waits
    /// here instead of being lost.
    rx: Vec<u8>,
    /// The Session State (MQTT 5.0 §4.1): our QoS 1/2 PUBLISH packets the
    /// broker has not completed (§4.3.2/§4.3.3), the broker's QoS 2
    /// packets delivered and PUBREC'd whose PUBREL has not arrived (a resend
    /// with the same identifier is not delivered a second time); kept in
    /// `mqtt.session_dir` while the session outlives its connection.
    session: SessionState,
    /// Identifiers of the session whose resend after Session Present 1
    /// waits for a slot under the broker's Receive Maximum.
    resend_waiting: std::collections::VecDeque<u16>,
    /// What the broker's CONNACK allows.
    broker: BrokerLimits,
    /// Our Maximum Packet Size (`mqtt.max_packet_size`), announced in the
    /// CONNECT.
    own_max_packet_size: u32,
    /// Our Receive Maximum (`mqtt.receive_maximum`).
    own_receive_maximum: u16,
    /// Our Topic Alias Maximum (`mqtt.topic_alias_maximum`).
    own_topic_alias_maximum: u16,
    /// §3.3.2.3.4: the Topic Aliases the broker set for this connection.
    aliases: std::collections::BTreeMap<u16, String>,
    /// The Keep Alive in force: the Server Keep Alive of the CONNACK, else
    /// `mqtt.keep_alive_secs` ([MQTT-3.1.2-21]); zero switches it off.
    keep_alive: Duration,
    /// When the client last sent a packet.
    last_sent: Instant,
    /// When the PINGREQ that has not been answered went out.
    ping_sent: Option<Instant>,
    /// The Session Present flag of the CONNACK.
    session_present: bool,
    /// The Assigned Client Identifier of the CONNACK (§3.2.2.3.7).
    assigned_client_id: Option<String>,
    /// Events read while [`Self::publish`] waited for a free slot or
    /// [`Self::subscribe`] for a SUBACK, handed out by [`Self::next_event`]
    /// first.
    held: std::collections::VecDeque<InboundEvent>,
    /// The packet identifiers of the SUBSCRIBE packets whose SUBACK has not
    /// come.
    subacks_awaited: std::collections::BTreeSet<u16>,
    /// The Reason Codes of the SUBACKs that came, by packet identifier.
    subacks: std::collections::BTreeMap<u16, Vec<u8>>,
    /// The Authentication Method of the CONNECT, when it carried one
    /// (MQTT 5.0 §4.12).
    auth_method: Option<String>,
    /// The re-authentication that runs (MQTT 5.0 §4.12.1).
    reauth: Option<Exchange>,
    /// The Reason Code of the DISCONNECT that ended the connection, the
    /// broker's or the client's.
    ended_by: Option<u8>,
    /// The client identifier of the CONNECT, the `client_id` of its log
    /// lines (Spec §8.1).
    client_id: String,
    /// The tenant of the connection (Spec §1 L6), the `tenant` of its log
    /// lines (§8.1); empty for the top level.
    tenant: String,
    /// Where the spans of §8.3 go; off unless [`Self::set_tracer`].
    tracer: Tracer,
    /// The spans of the daemon's QoS 1/2 PUBLISH packets whose roundtrip
    /// has not ended, by packet identifier (Spec §8.3).
    out_spans: std::collections::BTreeMap<u16, PublishOut>,
    /// The packet identifier of the last PUBLISH [`Self::publish_until`]
    /// sent; `None` after a QoS 0 one.
    last_packet_id: Option<u16>,
}

/// The CONNECT body the bridge sends for `cfg` with the credentials `user`
/// and `pass` and, under `auth.mode: enhanced`, the Authentication Method
/// and Data of `auth` (Spec §4.1, §7.2): MQTT 5.0 with the properties of
/// the table of §4.1, or MQTT 3.1.1 without properties.
///
/// # Errors
/// A value that does not fit its field (a user property over 65 535
/// octets cannot occur; the error is the encoder's).
pub fn connect_body(
    cfg: &DaemonConfig,
    user: Option<String>,
    pass: Option<Vec<u8>>,
    auth: Option<(String, Vec<u8>)>,
) -> Result<ConnectBody, ClientError> {
    let mut flags: u8 = 0;
    if cfg.clean_start {
        flags |= connect_flags::CLEAN_START;
    }
    if user.is_some() {
        flags |= connect_flags::USER_NAME;
    }
    if pass.is_some() {
        flags |= connect_flags::PASSWORD;
    }
    // Spec §6 Liveliness: the will of the connection, QoS 0, not retained.
    let will_topic = liveliness_topic(&cfg.own_topic_prefix, &cfg.client_id);
    if will_topic.is_some() {
        flags |= connect_flags::WILL;
    }
    let will_properties = match (cfg.protocol_version, &will_topic) {
        (ProtocolVersion::V5, Some(_)) => PropertySet {
            payload_format_indicator: Some(1),
            content_type: Some("application/json".to_string()),
            ..PropertySet::default()
        }
        .encode()
        .map_err(|()| ClientError::Codec("will properties".into()))?,
        _ => Vec::new(),
    };
    let (authentication_method, authentication_data) = match auth {
        Some((method, data)) => (Some(method), Some(data)),
        None => (None, None),
    };
    let properties = match cfg.protocol_version {
        ProtocolVersion::V5 => PropertySet {
            authentication_method,
            authentication_data,
            session_expiry_interval: Some(cfg.session_expiry_interval_secs),
            receive_maximum: Some(cfg.receive_maximum),
            maximum_packet_size: Some(cfg.max_packet_size),
            topic_alias_maximum: Some(cfg.topic_alias_maximum),
            request_response_information: Some(0),
            request_problem_information: Some(1),
            user_properties: CONNECT_USER_PROPERTIES
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            ..PropertySet::default()
        }
        .encode()
        .map_err(|()| ClientError::Codec("CONNECT properties".into()))?,
        ProtocolVersion::V311 => Vec::new(),
    };
    Ok(ConnectBody {
        protocol_name: "MQTT".to_string(),
        protocol_version: cfg.protocol_version.level(),
        connect_flags: flags,
        keep_alive: cfg.keep_alive_secs,
        properties,
        client_id: cfg.client_id.clone(),
        will_payload: will_topic
            .as_ref()
            .map(|_| liveliness_lost_json(&cfg.client_id).into_bytes())
            .unwrap_or_default(),
        will_properties,
        will_topic,
        user_name: user,
        password: pass.unwrap_or_default(),
    })
}

/// The topic of the will of the connection of `client_id` (Spec §6
/// Liveliness): `<own_prefix>/<client_id>/liveliness_lost`, `own_prefix`
/// being `mqtt.own_topic_prefix` (`$zerodds` by default, Spec §3); `None`
/// for an empty identifier (the broker assigns one after the CONNECT) and
/// for one with `+` or `#`, which a topic name cannot carry.
#[must_use]
pub fn liveliness_topic(own_prefix: &str, client_id: &str) -> Option<String> {
    (!client_id.is_empty() && !client_id.contains(['+', '#', '\0']))
        .then(|| format!("{own_prefix}/{client_id}/liveliness_lost"))
}

/// The payload of the will: `{"client_id":"<id>"}`.
#[must_use]
pub fn liveliness_lost_json(client_id: &str) -> String {
    let mut out = String::from("{\"client_id\":\"");
    for c in client_id.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push_str("\"}");
    out
}

impl MqttClient {
    /// Connects to the broker at `host:port`, sends CONNECT, blocks on
    /// CONNACK. If `tls_client_cfg = Some(...)` the TCP stream is wrapped
    /// with rustls, the handshake done before the CONNECT (Spec §7.1).
    ///
    /// # Errors
    /// `Io` on TCP/read/write error, `Tls` when the TLS handshake fails,
    /// `ConnAck` if the broker refuses the connection, `Codec` on a
    /// frame-decode error.
    pub fn connect_secure(
        host: &str,
        port: u16,
        cfg: &DaemonConfig,
        tls_client_cfg: Option<Arc<ClientConfig>>,
    ) -> Result<Self, ClientError> {
        let alpn = alpn_offer(cfg, false);
        let addr = format!("{host}:{port}");
        Self::establish(cfg, (host, port), || {
            open_stream(&addr, host, cfg, tls_client_cfg.clone(), &alpn)
        })
    }

    /// Connects to the broker a parsed `mqtt.broker_url` names: TCP or
    /// TLS below, for `ws://` and `wss://` the WebSocket opening handshake
    /// on top (MQTT 5.0 §6), then CONNECT and CONNACK. `tls_client_cfg` is
    /// used when the URL's scheme is `mqtts://` or `wss://`.
    ///
    /// # Errors
    /// See [`Self::connect_secure`]; a failed WebSocket handshake is `Io`
    /// naming what the broker's answer lacked.
    pub fn connect_url(
        url: &super::config::BrokerUrl,
        cfg: &DaemonConfig,
        tls_client_cfg: Option<Arc<ClientConfig>>,
    ) -> Result<Self, ClientError> {
        let tls = if url.tls {
            Some(tls_client_cfg.ok_or_else(|| {
                ClientError::Tls(format!(
                    "{}:// needs a TLS client configuration (mqtt.tls)",
                    if matches!(url.transport, super::config::BrokerTransport::Tcp) {
                        "mqtts"
                    } else {
                        "wss"
                    }
                ))
            })?)
        } else {
            None
        };
        let websocket = matches!(
            url.transport,
            super::config::BrokerTransport::WebSocket { .. }
        );
        let alpn = alpn_offer(cfg, websocket);
        Self::establish(cfg, (&url.host, url.port), || {
            let mut stream = open_stream(&url.socket_addr(), &url.host, cfg, tls.clone(), &alpn)?;
            if let super::config::BrokerTransport::WebSocket { path } = &url.transport {
                stream = MqttStream::Ws(Box::new(
                    super::ws_transport::WsStream::handshake(stream, &url.host_header(), path)
                        .map_err(|e| ClientError::Io(format!("websocket: {e}")))?,
                ));
            }
            Ok(stream)
        })
    }

    /// The Session State of `cfg` ([`SessionState::for_connect`]), a
    /// connection from `open`, CONNECT and CONNACK; then the session is
    /// resumed or begun ([`Self::resume`]). A broker that resumes a
    /// session this client has no state of gets its connection closed, and
    /// a new session begins with Clean Start 1 ([MQTT-3.2.2-4]); under MQTT
    /// 3.1.1, whose Clean Session 1 ends with the connection, a third
    /// connection with Clean Session 0 begins the lasting one.
    fn establish(
        cfg: &DaemonConfig,
        endpoint: (&str, u16),
        open: impl Fn() -> Result<MqttStream, ClientError>,
    ) -> Result<Self, ClientError> {
        let session = SessionState::for_connect(cfg).map_err(ClientError::Session)?;
        let mut me = Self::handshake(open()?, cfg, session, endpoint)?;
        me.refuse_present_after_clean_start(cfg.clean_start)?;
        if me.session_present && !me.session.known() {
            log::event(Level::Warn, "session_unknown")
                .tenant(&cfg.tenant)
                .field("client_id", &cfg.client_id)
                .field(
                    "reason",
                    "the broker resumed a session this client holds no state of; the connection is closed and a new session begins",
                )
                .emit();
            let mut fresh = cfg.clone();
            fresh.clean_start = true;
            let session = std::mem::take(&mut me.session);
            me.disconnect();
            me = Self::handshake(open()?, &fresh, session, endpoint)?;
            me.refuse_present_after_clean_start(true)?;
            if cfg.protocol_version == ProtocolVersion::V311 && me.session.is_lasting() {
                let session = std::mem::take(&mut me.session);
                me.disconnect();
                me = Self::handshake(open()?, cfg, session, endpoint)?;
                if me.session_present {
                    // Clean Session 1 just ended every session of the
                    // identifier.
                    me.disconnect_with(PROTOCOL_ERROR);
                    return Err(ClientError::Protocol {
                        reason: PROTOCOL_ERROR,
                        what: "Session Present 1 right after a session with Clean Session 1".into(),
                    });
                }
            }
        }
        me.resume()?;
        Ok(me)
    }

    /// [MQTT-3.2.2-2]: a CONNACK to a CONNECT with Clean Start 1 carries
    /// Session Present 0; a broker that answers 1 breaks the protocol, and
    /// the connection is closed (DISCONNECT 0x82).
    fn refuse_present_after_clean_start(&mut self, clean_start: bool) -> Result<(), ClientError> {
        if clean_start && self.session_present {
            self.disconnect_with(PROTOCOL_ERROR);
            return Err(ClientError::Protocol {
                reason: PROTOCOL_ERROR,
                what: "Session Present 1 on a CONNECT with Clean Start 1 [MQTT-3.2.2-2]".into(),
            });
        }
        Ok(())
    }

    /// CONNECT and CONNACK on an open stream, with `session` as the
    /// client's Session State; under `auth.mode: enhanced` the exchange of
    /// MQTT 5.0 §4.12 between them. `endpoint` is the broker's host and
    /// port (OAUTHBEARER names them).
    fn handshake(
        stream: MqttStream,
        cfg: &DaemonConfig,
        session: SessionState,
        endpoint: (&str, u16),
    ) -> Result<Self, ClientError> {
        let _ = stream.set_read_timeout(Some(ANSWER_TIMEOUT));
        let _ = stream.set_write_timeout(Some(ANSWER_TIMEOUT));
        let mut me = Self::over(stream, cfg);
        me.session = session;
        let mut exchange = Exchange::of(cfg, endpoint.0, endpoint.1).map_err(ClientError::Auth)?;
        me.send_connect(cfg, exchange.as_ref())?;
        me.wait_connack(exchange.as_mut())?;
        me.auth_method = exchange.map(|x| x.method().to_string());
        Ok(me)
    }

    /// A client on `stream` before its CONNECT, with the limits `cfg`
    /// announces.
    fn over(stream: MqttStream, cfg: &DaemonConfig) -> Self {
        Self {
            stream,
            version: cfg.protocol_version,
            next_packet_id: 1,
            rx: Vec::new(),
            session: SessionState::in_memory(),
            resend_waiting: std::collections::VecDeque::new(),
            broker: BrokerLimits::default(),
            own_max_packet_size: cfg.max_packet_size,
            own_receive_maximum: cfg.receive_maximum,
            own_topic_alias_maximum: match cfg.protocol_version {
                ProtocolVersion::V5 => cfg.topic_alias_maximum,
                ProtocolVersion::V311 => 0,
            },
            aliases: std::collections::BTreeMap::new(),
            keep_alive: Duration::from_secs(u64::from(cfg.keep_alive_secs)),
            last_sent: Instant::now(),
            ping_sent: None,
            session_present: false,
            assigned_client_id: None,
            held: std::collections::VecDeque::new(),
            subacks_awaited: std::collections::BTreeSet::new(),
            subacks: std::collections::BTreeMap::new(),
            auth_method: None,
            reauth: None,
            ended_by: None,
            client_id: cfg.client_id.clone(),
            tenant: cfg.tenant.clone(),
            tracer: Tracer::default(),
            out_spans: std::collections::BTreeMap::new(),
            last_packet_id: None,
        }
    }

    /// Plain-TCP connect — backward-compat path.
    ///
    /// # Errors
    /// See [`Self::connect_secure`].
    pub fn connect(host: &str, port: u16, cfg: &DaemonConfig) -> Result<Self, ClientError> {
        Self::connect_secure(host, port, cfg, None)
    }

    /// The protocol of the connection.
    #[must_use]
    pub fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// What the broker's CONNACK allows the bridge.
    #[must_use]
    pub fn broker_limits(&self) -> BrokerLimits {
        self.broker
    }

    /// The Keep Alive in force: the broker's Server Keep Alive when its
    /// CONNACK named one, else `mqtt.keep_alive_secs`.
    #[must_use]
    pub fn keep_alive(&self) -> Duration {
        self.keep_alive
    }

    /// The Session Present flag of the CONNACK: the broker resumed a
    /// session of this client identifier.
    #[must_use]
    pub fn session_present(&self) -> bool {
        self.session_present
    }

    /// The client identifier the broker assigned (§3.2.2.3.7), when the
    /// CONNECT carried an empty one.
    #[must_use]
    pub fn assigned_client_id(&self) -> Option<&str> {
        self.assigned_client_id.as_deref()
    }

    /// Writes one whole packet and notes the time for the Keep Alive.
    fn send(&mut self, frame: &[u8], what: &str) -> Result<(), ClientError> {
        self.stream
            .write_all(frame)
            .map_err(|e| ClientError::Io(format!("write {what}: {e}")))?;
        self.last_sent = Instant::now();
        Ok(())
    }

    /// The CONNECT with the credentials of `auth.mode` (Spec §7.2): User
    /// Name and Password under `password`, the Authentication Method and
    /// the first Authentication Data of `exchange` under `enhanced`,
    /// neither under `none` and `mtls`.
    fn send_connect(
        &mut self,
        cfg: &DaemonConfig,
        exchange: Option<&Exchange>,
    ) -> Result<(), ClientError> {
        let creds = Credentials::of(cfg);
        let body = connect_body(
            cfg,
            creds.connect_user_name(),
            creds.connect_password(),
            exchange.map(|x| (x.method().to_string(), x.initial())),
        )?;
        let body_bytes = encode_connect_body_v(&body, self.version)
            .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        let frame = wrap_packet(ControlPacketType::Connect, 0, &body_bytes)
            .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        self.send(&frame, "connect")
    }

    /// Sends AUTH with `reason`, the Authentication Method `method` and
    /// `data` (MQTT 5.0 §3.15).
    fn send_auth(&mut self, reason: u8, method: &str, data: Vec<u8>) -> Result<(), ClientError> {
        let properties = PropertySet {
            authentication_method: Some(method.to_string()),
            authentication_data: Some(data),
            ..PropertySet::default()
        }
        .encode()
        .map_err(|()| ClientError::Codec("AUTH properties".into()))?;
        let body = encode_auth_body(&AuthBody {
            reason_code: reason,
            properties,
        })
        .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        let frame = wrap_packet(ControlPacketType::Auth, 0, &body)
            .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        self.send(&frame, "auth")
    }

    /// The Reason Code and the properties of an AUTH of the broker, its
    /// Authentication Method checked against `method` ([MQTT-4.12.0-5]).
    fn read_auth(body: &[u8], method: &str) -> Result<(u8, PropertySet), ClientError> {
        let auth = decode_auth_body(body)
            .map_err(|e| malformed(format!("an AUTH that does not decode: {e:?}")))?;
        let props = PropertySet::parse(
            &auth.properties,
            PropertyContext::Packet(ControlPacketType::Auth),
        )
        .map_err(|code| ClientError::Protocol {
            reason: code,
            what: "AUTH properties break Table 2-4".into(),
        })?;
        if props.authentication_method.as_deref() != Some(method) {
            return Err(ClientError::Protocol {
                reason: PROTOCOL_ERROR,
                what: format!(
                    "AUTH with Authentication Method {:?}, the CONNECT's is {method} [MQTT-4.12.0-5]",
                    props.authentication_method
                ),
            });
        }
        Ok((auth.reason_code, props))
    }

    /// The broker's packets up to its CONNACK. Under enhanced
    /// authentication each AUTH 0x18 of the broker is answered with AUTH
    /// 0x18 ([MQTT-4.12.0-3]) and nothing else is sent before the CONNACK
    /// ([MQTT-3.1.2-30]); the CONNACK carries the method of the CONNECT
    /// ([MQTT-4.12.0-5]) and the data that ends the exchange (SCRAM: the
    /// server's signature, verified). An AUTH to a CONNECT without method
    /// is a protocol error ([MQTT-4.12.0-6]). A failure of the client's
    /// side ends the connection with DISCONNECT 0x80, a protocol error with
    /// its Reason Code.
    fn wait_connack(&mut self, mut exchange: Option<&mut Exchange>) -> Result<(), ClientError> {
        let result = self.wait_connack_inner(&mut exchange);
        match &result {
            Err(ClientError::Protocol { reason, .. }) => self.disconnect_with(*reason),
            Err(ClientError::Auth(_)) => self.disconnect_with(UNSPECIFIED_ERROR),
            _ => {}
        }
        result
    }

    fn wait_connack_inner(
        &mut self,
        exchange: &mut Option<&mut Exchange>,
    ) -> Result<(), ClientError> {
        let (header, body) = loop {
            let (header, body) = self.read_packet()?;
            if header.packet_type != ControlPacketType::Auth || self.version != ProtocolVersion::V5
            {
                break (header, body);
            }
            let Some(x) = exchange.as_deref_mut() else {
                return Err(ClientError::Protocol {
                    reason: PROTOCOL_ERROR,
                    what: "AUTH to a CONNECT without Authentication Method [MQTT-4.12.0-6]".into(),
                });
            };
            let method = x.method().to_string();
            let (reason, props) = Self::read_auth(&body, &method)?;
            if reason != CONTINUE_AUTHENTICATION {
                return Err(ClientError::Protocol {
                    reason: PROTOCOL_ERROR,
                    what: format!("AUTH 0x{reason:02x} before the CONNACK"),
                });
            }
            let data = x
                .answer(props.authentication_data.as_deref())
                .map_err(ClientError::Auth)?;
            self.send_auth(CONTINUE_AUTHENTICATION, &method, data)?;
        };
        if header.packet_type != ControlPacketType::ConnAck {
            return Err(ClientError::Codec(format!(
                "expected CONNACK got {:?}",
                header.packet_type
            )));
        }
        let connack = decode_connack_body_v(&body, self.version)
            .map_err(|e| malformed(format!("a CONNACK that does not decode: {e:?}")))?;
        let refused = match self.version {
            ProtocolVersion::V5 => connack.reason_code >= 0x80,
            ProtocolVersion::V311 => connack.reason_code != 0,
        };
        let props = match self.version {
            ProtocolVersion::V5 => PropertySet::parse(
                &connack.properties,
                PropertyContext::Packet(ControlPacketType::ConnAck),
            )
            .map_err(|reason| ClientError::Protocol {
                reason,
                what: "the properties of the CONNACK break Table 2-4".into(),
            })?,
            ProtocolVersion::V311 => PropertySet::default(),
        };
        if refused {
            let mut detail = props.reason_string.unwrap_or_default();
            if let Some(e) = exchange.as_deref().and_then(Exchange::reported_error) {
                if !detail.is_empty() {
                    detail.push_str("; ");
                }
                detail.push_str("OAUTHBEARER status ");
                detail.push_str(e);
            }
            return Err(ClientError::ConnAck {
                reason: connack.reason_code,
                version: self.version,
                detail,
            });
        }
        match exchange.as_deref_mut() {
            Some(x) => {
                if props.authentication_method.as_deref() != Some(x.method()) {
                    return Err(ClientError::Protocol {
                        reason: PROTOCOL_ERROR,
                        what: format!(
                            "CONNACK with Authentication Method {:?}, the CONNECT's is {} [MQTT-4.12.0-5]",
                            props.authentication_method,
                            x.method()
                        ),
                    });
                }
                x.finish(props.authentication_data.as_deref())
                    .map_err(ClientError::Auth)?;
            }
            None if props.authentication_method.is_some() => {
                return Err(ClientError::Protocol {
                    reason: PROTOCOL_ERROR,
                    what: "CONNACK with an Authentication Method the CONNECT did not carry \
                           [MQTT-4.12.0-6]"
                        .into(),
                });
            }
            None => {}
        }
        self.session_present = connack.session_present;
        if let Some(max) = props.receive_maximum {
            self.broker.receive_maximum = max;
        }
        self.broker.maximum_packet_size = props.maximum_packet_size;
        if let Some(q) = props.maximum_qos {
            self.broker.maximum_qos = q;
        }
        if let Some(r) = props.retain_available {
            self.broker.retain_available = r == 1;
        }
        self.broker.topic_alias_maximum = props.topic_alias_maximum.unwrap_or(0);
        self.broker.wildcard_subscriptions = props.wildcard_subscription_available != Some(0);
        self.broker.subscription_identifiers = props.subscription_identifier_available != Some(0);
        // [MQTT-3.1.2-21]: the Server Keep Alive replaces ours.
        if let Some(k) = props.server_keep_alive {
            self.keep_alive = Duration::from_secs(u64::from(k));
        }
        self.assigned_client_id = props.assigned_client_identifier;
        Ok(())
    }

    /// Spec §4.3: one SUBSCRIBE per entry, with its Subscription Identifier
    /// (when the broker's CONNACK allows one, MQTT 5.0 §3.2.2.3.15) and the
    /// options QoS, No Local 1, Retain As Published 0, Retain Handling 0
    /// (MQTT 5.0 §3.8.3.1); then the SUBACK of each is awaited. Under MQTT
    /// 3.1.1 a SUBSCRIBE carries neither: its options byte is the QoS
    /// alone, the other bits are reserved there. A wildcard filter the
    /// broker's CONNACK forbids is not sent. PUBLISH packets that arrive
    /// meanwhile are kept for [`Self::next_event`].
    ///
    /// # Errors
    /// IO/Codec; [`ClientError::TimedOut`] when a SUBACK does not come
    /// within the answer timeout.
    pub fn subscribe(
        &mut self,
        entries: &[SubscribeEntry],
    ) -> Result<Vec<SubscribeOutcome>, ClientError> {
        let mut out = vec![SubscribeOutcome::NoWildcards; entries.len()];
        let mut waiting: Vec<(usize, u16)> = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            if !self.broker.wildcard_subscriptions
                && (e.filter.contains('+') || e.filter.contains('#'))
            {
                continue;
            }
            let pid = self.next_pid();
            let body_bytes = encode_subscribe_body_v(
                &SubscribeBody {
                    packet_id: pid,
                    properties: self.subscribe_properties(e.id)?,
                    subscriptions: vec![MqttSubscription {
                        topic_filter: e.filter.clone(),
                        options: subscription_options(self.version, e.qos),
                    }],
                },
                self.version,
            )
            .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
            // SUBSCRIBE Reserved-Bits MUST be 0010 (Spec §3.8.1).
            let frame = wrap_packet(ControlPacketType::Subscribe, 0b0010, &body_bytes)
                .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
            self.send(&frame, "sub")?;
            self.subacks_awaited.insert(pid);
            waiting.push((i, pid));
        }
        let deadline = Instant::now() + ANSWER_TIMEOUT;
        for (i, pid) in waiting {
            let reasons = loop {
                if let Some(r) = self.subacks.remove(&pid) {
                    break r;
                }
                if Instant::now() >= deadline {
                    return Err(ClientError::TimedOut);
                }
                match self.read_event()? {
                    Some(InboundEvent::Disconnected(why)) => {
                        return Err(ClientError::Io(format!(
                            "the broker closed the connection before its SUBACK: {why}"
                        )));
                    }
                    Some(event) => self.held.push_back(event),
                    None => {}
                }
            };
            let reason = reasons.first().copied().unwrap_or(0x80);
            out[i] = if reason >= 0x80 {
                SubscribeOutcome::Refused(reason)
            } else {
                SubscribeOutcome::Granted(reason)
            };
        }
        Ok(out)
    }

    /// The properties of a SUBSCRIBE with the Subscription Identifier `id`:
    /// none under MQTT 3.1.1 or when the broker has no Subscription
    /// Identifiers.
    fn subscribe_properties(&self, id: u32) -> Result<Vec<u8>, ClientError> {
        if self.version != ProtocolVersion::V5 || !self.broker.subscription_identifiers {
            return Ok(Vec::new());
        }
        PropertySet {
            subscription_identifiers: vec![id],
            ..PropertySet::default()
        }
        .encode()
        .map_err(|()| ClientError::Codec("SUBSCRIBE properties".into()))
    }

    /// PUBLISH a sample with `properties` (MQTT 5.0 §3.3.2.3; under MQTT
    /// 3.1.1 a PUBLISH has none and they are not sent).
    ///
    /// # Errors
    /// IO/Codec; [`ClientError::NotSent`] for a PUBLISH the broker's
    /// CONNACK forbids: a QoS above its Maximum QoS ([MQTT-3.2.2-11]), the
    /// retain flag without Retain Available ([MQTT-3.2.2-14]), a packet
    /// above its Maximum Packet Size ([MQTT-3.2.2-15]);
    /// [`ClientError::Busy`] when the broker's Receive Maximum stays
    /// reached for [`SEND_SLOT_WAIT`].
    pub fn publish(
        &mut self,
        topic: &str,
        payload: &[u8],
        qos: u8,
        retain: bool,
        properties: &PropertySet,
    ) -> Result<(), ClientError> {
        self.publish_until(topic, payload, qos, retain, properties, None)
            .map(|_| ())
    }

    /// [`Self::publish`] of a message whose Message Expiry Interval ends at
    /// `expires` (Spec §6; `None`: it has none, or its interval in
    /// `properties` holds as it is). The interval sent is the time left
    /// when the PUBLISH goes out, after a wait for a send slot; the interval
    /// sent is returned.
    ///
    /// # Errors
    /// As [`Self::publish`]; [`ClientError::Expired`] when the time has
    /// passed before the PUBLISH could go out, which then is not sent.
    pub fn publish_until(
        &mut self,
        topic: &str,
        payload: &[u8],
        qos: u8,
        retain: bool,
        properties: &PropertySet,
        expires: Option<Instant>,
    ) -> Result<Option<u32>, ClientError> {
        if qos > self.broker.maximum_qos {
            return Err(ClientError::NotSent(format!(
                "QoS {qos} on {topic} is above the broker's Maximum QoS {}",
                self.broker.maximum_qos
            )));
        }
        if retain && !self.broker.retain_available {
            return Err(ClientError::NotSent(format!(
                "a retained message on {topic}: the broker has no Retain Available"
            )));
        }
        let mut props = std::borrow::Cow::Borrowed(properties);
        if let Some(at) = expires {
            props.to_mut().message_expiry_interval = Some(expiry_left(at, topic)?);
        }
        let mut pkt = PublishPacket {
            dup: false,
            qos,
            retain,
            topic: topic.to_string(),
            packet_id: (qos > 0).then_some(0),
            properties: self.publish_properties(&props)?,
            payload: payload.to_vec(),
        };
        let size = encode_publish_v(&pkt, self.version)
            .map_err(|e| ClientError::Codec(format!("{e:?}")))?
            .len();
        if let Some(max) = self.broker.maximum_packet_size {
            if size > max as usize {
                return Err(ClientError::NotSent(format!(
                    "a PUBLISH of {size} octets on {topic} is above the broker's Maximum Packet Size {max}"
                )));
            }
        }
        if qos > 0 {
            self.wait_for_a_send_slot()?;
            if let Some(at) = expires {
                // The wait for a slot took its time: the interval is what
                // is left now (the property keeps its four octets, the
                // size checked above stays).
                props.to_mut().message_expiry_interval = Some(expiry_left(at, topic)?);
                pkt.properties = self.publish_properties(&props)?;
            }
            pkt.packet_id = Some(self.next_pid());
        }
        let bytes = encode_publish_v(&pkt, self.version)
            .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        if let Some(pid) = pkt.packet_id {
            // [MQTT-3.1.2-23]: the state is stored before the packet goes
            // out, so that a lasting session can resend it.
            self.session
                .published(pid, &bytes)
                .map_err(ClientError::Session)?;
        }
        self.last_packet_id = pkt.packet_id;
        self.send(&bytes, "pub")?;
        Ok(props.message_expiry_interval)
    }

    /// Spec §8.3: the spans of this connection's PUBLISH packets go to
    /// `tracer`.
    pub fn set_tracer(&mut self, tracer: Tracer) {
        self.tracer = tracer;
    }

    /// Whether the spans of §8.3 go anywhere.
    #[must_use]
    pub fn tracing(&self) -> bool {
        self.tracer.enabled()
    }

    /// Spec §8.3: the PUBLISH [`Self::publish_until`] just sent is the
    /// roundtrip `x`: under QoS 0 it ends now, under QoS 1 and 2 with the
    /// broker's PUBACK or PUBCOMP (or its refusal, or the end of the
    /// connection).
    pub fn trace_publish(&mut self, mut x: PublishOut) {
        if !self.tracer.enabled() {
            return;
        }
        match self.last_packet_id {
            Some(pid) if x.qos > 0 => {
                x.packet_id = Some(pid);
                self.out_spans.insert(pid, x);
            }
            _ => self.tracer.publish_out(&x, unix_ns(), None),
        }
    }

    /// Spec §8.3: the roundtrip of `pid` ended; `failure` when it did not
    /// end with a success.
    fn end_span(&mut self, pid: u16, failure: Option<&str>) {
        if let Some(x) = self.out_spans.remove(&pid) {
            self.tracer.publish_out(&x, unix_ns(), failure);
        }
    }

    /// Spec §8.3: the roundtrips that have not ended end with the
    /// connection, as failures.
    fn end_open_spans(&mut self, why: &str) {
        let open = std::mem::take(&mut self.out_spans);
        for x in open.values() {
            self.tracer.publish_out(x, unix_ns(), Some(why));
        }
    }

    /// The encoded properties of a PUBLISH: none under MQTT 3.1.1.
    fn publish_properties(&self, properties: &PropertySet) -> Result<Vec<u8>, ClientError> {
        match self.version {
            ProtocolVersion::V5 => properties
                .encode()
                .map_err(|()| ClientError::Codec("PUBLISH properties".into())),
            ProtocolVersion::V311 => Ok(Vec::new()),
        }
    }

    /// §4.9: with Receive Maximum packets unacknowledged, the next QoS 1/2
    /// PUBLISH waits for an ack. Sending on made Mosquitto (Receive
    /// Maximum 20) acknowledge ten QoS 1 messages it then did not pass on
    /// (measured, `tests/mosquitto_qos.rs`). What arrives meanwhile is kept
    /// for [`Self::next_event`]. The end of the connection ends the wait
    /// with [`ClientError::Io`]; its event is kept once, for the inbound
    /// loop that connects again (Spec §9.3). A packet of the broker that
    /// breaks the protocol or does not decode has ended the connection
    /// with its DISCONNECT ([`Self::read_event`]); any other error of a
    /// packet read here ends it as the inbound loop ends it on one: the
    /// stream is closed, and the PUBLISH waits for the next connection.
    fn wait_for_a_send_slot(&mut self) -> Result<(), ClientError> {
        let deadline = Instant::now() + SEND_SLOT_WAIT;
        while self.session.in_flight() >= usize::from(self.broker.receive_maximum) {
            if Instant::now() >= deadline {
                return Err(ClientError::Busy(format!(
                    "no ack from the broker within {SEND_SLOT_WAIT:?} with {} packets in flight",
                    self.session.in_flight()
                )));
            }
            let event = match self.read_event() {
                Ok(event) => event,
                Err(e) => {
                    self.stream.shutdown_both();
                    Some(InboundEvent::Disconnected(e.to_string()))
                }
            };
            match event {
                Some(InboundEvent::Disconnected(why)) => {
                    let error = ClientError::Io(format!(
                        "the connection ended while a PUBLISH waited for a send slot: {why}"
                    ));
                    if !self
                        .held
                        .iter()
                        .any(|e| matches!(e, InboundEvent::Disconnected(_)))
                    {
                        self.held.push_back(InboundEvent::Disconnected(why));
                    }
                    return Err(error);
                }
                Some(event) => self.held.push_back(event),
                None => {}
            }
        }
        Ok(())
    }

    /// Our QoS 1/2 PUBLISH packets the broker has not completed yet.
    #[must_use]
    pub fn pending_acks(&self) -> usize {
        self.session.in_flight()
    }

    /// The broker's QoS 2 PUBLISH packets this client answered with PUBREC
    /// whose PUBREL has not come.
    #[must_use]
    pub fn pending_releases(&self) -> usize {
        self.session.received_len()
    }

    /// The Reason Code of the DISCONNECT that ended the connection (the
    /// broker's, or the client's for a protocol error); `None` while it
    /// stands or when it ended without one.
    #[must_use]
    pub fn ended_by(&self) -> Option<u8> {
        self.ended_by
    }

    /// The Authentication Method of the connection's CONNECT, when it
    /// carried one.
    #[must_use]
    pub fn auth_method(&self) -> Option<&str> {
        self.auth_method.as_deref()
    }

    /// Starts a re-authentication with the credentials of `cfg` (MQTT 5.0
    /// §4.12.1): AUTH 0x19 with the method of the CONNECT and the first
    /// data; the broker's AUTH 0x18 are answered and its AUTH 0x00 checked
    /// as they come ([`Self::next_event`]). Meanwhile the other packets
    /// flow under the previous authentication.
    ///
    /// # Errors
    /// The connection has no enhanced authentication, `cfg` names another
    /// method, a re-authentication runs, or the AUTH cannot be written.
    pub fn reauthenticate(
        &mut self,
        cfg: &DaemonConfig,
        host: &str,
        port: u16,
    ) -> Result<(), ClientError> {
        let Some(method) = self.auth_method.clone() else {
            return Err(ClientError::Auth(
                "the connection's CONNECT carried no Authentication Method".into(),
            ));
        };
        if self.reauth.is_some() {
            return Err(ClientError::Auth("a re-authentication runs".into()));
        }
        let x = Exchange::of(cfg, host, port)
            .map_err(ClientError::Auth)?
            .ok_or_else(|| ClientError::Auth("auth.mode is not enhanced".into()))?;
        if x.method() != method {
            return Err(ClientError::Auth(format!(
                "the method {} is not the connection's {method} [MQTT-4.12.1-1]",
                x.method()
            )));
        }
        self.send_auth(RE_AUTHENTICATE, &method, x.initial())?;
        self.reauth = Some(x);
        Ok(())
    }

    /// Whether a re-authentication runs.
    #[must_use]
    pub fn reauthenticating(&self) -> bool {
        self.reauth.is_some()
    }

    /// An AUTH of the broker after the CONNACK: a step of the
    /// re-authentication that runs (MQTT 5.0 §4.12.1); any other is a
    /// protocol error ([MQTT-4.12.0-6], §4.13). A failure of the client's
    /// side ends the connection with DISCONNECT 0x80.
    fn on_auth(&mut self, body: &[u8]) -> Result<(), ClientError> {
        let Some(method) = self.auth_method.clone() else {
            return Err(ClientError::Protocol {
                reason: PROTOCOL_ERROR,
                what: "AUTH on a connection without Authentication Method [MQTT-4.12.0-6]".into(),
            });
        };
        let (reason, props) = Self::read_auth(body, &method)?;
        let Some(mut x) = self.reauth.take() else {
            return Err(ClientError::Protocol {
                reason: PROTOCOL_ERROR,
                what: format!("AUTH 0x{reason:02x} outside a re-authentication"),
            });
        };
        let failed = |e: String| ClientError::Protocol {
            reason: UNSPECIFIED_ERROR,
            what: format!("re-authentication: {e}"),
        };
        match reason {
            CONTINUE_AUTHENTICATION => {
                let data = x
                    .answer(props.authentication_data.as_deref())
                    .map_err(failed)?;
                self.send_auth(CONTINUE_AUTHENTICATION, &method, data)?;
                self.reauth = Some(x);
                Ok(())
            }
            0x00 => {
                x.finish(props.authentication_data.as_deref())
                    .map_err(failed)?;
                log::event(Level::Info, "reauth")
                    .tenant(&self.tenant)
                    .field("client_id", &self.client_id)
                    .field("method", &method)
                    .emit();
                Ok(())
            }
            other => Err(ClientError::Protocol {
                reason: PROTOCOL_ERROR,
                what: format!("AUTH 0x{other:02x} in a re-authentication"),
            }),
        }
    }

    /// Sends PUBACK/PUBREC/PUBREL/PUBCOMP (MQTT 5.0 §3.4-§3.7) with
    /// reason Success. PUBREL carries the fixed flags 0b0010 (§3.6.1).
    fn send_ack(&mut self, kind: ControlPacketType, packet_id: u16) -> Result<(), ClientError> {
        let body = encode_ack_body_v(
            &AckBody {
                packet_id,
                reason_code: 0,
                properties: Vec::new(),
            },
            self.version,
        )
        .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        let flags = if kind == ControlPacketType::PubRel {
            0b0010
        } else {
            0
        };
        let frame =
            wrap_packet(kind, flags, &body).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        self.send(&frame, "ack")
    }

    /// An acknowledgement from the broker for one of our packets.
    fn on_broker_ack(&mut self, kind: ControlPacketType, body: &[u8]) -> Result<(), ClientError> {
        let ack = decode_ack_body(body)
            .map_err(|e| malformed(format!("a {kind:?} that does not decode: {e:?}")))?;
        if ack.reason_code >= 0x80 {
            // §3.4.2.1: the broker refused the message; the flow ends here.
            log::event(Level::Warn, "publish_refused_by_broker")
                .tenant(&self.tenant)
                .field("client_id", &self.client_id)
                .number("packet_id", u64::from(ack.packet_id))
                .reason_code(ack.reason_code)
                .emit();
            self.end_span(
                ack.packet_id,
                Some(&format!(
                    "the broker refused it with reason 0x{:02x}",
                    ack.reason_code
                )),
            );
            self.complete(ack.packet_id);
            return self.resend_due();
        }
        match kind {
            ControlPacketType::PubAck | ControlPacketType::PubComp => {
                self.complete(ack.packet_id);
            }
            ControlPacketType::PubRec => {
                // §4.3.3: PUBREL answers each PUBREC, also one again for a
                // PUBREL already sent; the state changes the first time.
                if self.session.state_of(ack.packet_id) == Some(OutboundState::Published) {
                    self.session
                        .released(ack.packet_id)
                        .map_err(ClientError::Session)?;
                }
                self.send_ack(ControlPacketType::PubRel, ack.packet_id)?;
            }
            _ => {}
        }
        self.resend_due()
    }

    /// The broker completed `pid`. A record that cannot be written is
    /// logged: the flow is complete on the wire, and the PUBLISH is
    /// resent after a restart at worst.
    fn complete(&mut self, pid: u16) {
        self.end_span(pid, None);
        self.resend_waiting.retain(|p| *p != pid);
        if let Err(e) = self.session.completed(pid) {
            log::event(Level::Warn, "session_write_failed")
                .tenant(&self.tenant)
                .field("client_id", &self.client_id)
                .field("reason", &e)
                .emit();
        }
    }

    /// After a CONNACK: Session Present 1 resumes the session, whose
    /// PUBLISH packets (with DUP) and PUBREL packets go out again with
    /// their identifiers, in their order ([MQTT-4.4.0-1], [MQTT-4.6.0-1],
    /// [MQTT-4.6.0-4]), as many as the broker's Receive Maximum lets
    /// through; the rest as acknowledgements come. Session Present 0 starts
    /// a new one: the state of the old one is discarded ([MQTT-3.2.2-5]).
    fn resume(&mut self) -> Result<(), ClientError> {
        let discarded = self
            .session
            .settle(self.session_present)
            .map_err(ClientError::Session)?;
        if discarded > 0 {
            log::event(Level::Warn, "session_discarded")
                .tenant(&self.tenant)
                .field("client_id", &self.client_id)
                .number("discarded", discarded as u64)
                .field(
                    "reason",
                    "the broker holds no session of this client; the unacknowledged message flows of the last one are discarded",
                )
                .emit();
        }
        if self.session_present {
            self.resend_waiting = self
                .session
                .resend_order()
                .into_iter()
                .map(|r| match r {
                    Resend::Publish(pid, _) | Resend::Release(pid) => pid,
                })
                .collect();
            if !self.resend_waiting.is_empty() {
                log::event(Level::Info, "session_resumed")
                    .tenant(&self.tenant)
                    .field("client_id", &self.client_id)
                    .number("resent", self.resend_waiting.len() as u64)
                    .emit();
            }
        }
        self.resend_due()
    }

    /// Resends what waits while the broker's Receive Maximum has room.
    fn resend_due(&mut self) -> Result<(), ClientError> {
        while let Some(&pid) = self.resend_waiting.front() {
            let sent = self.session.in_flight() - self.resend_waiting.len();
            if sent >= usize::from(self.broker.receive_maximum) {
                break;
            }
            self.resend_waiting.pop_front();
            match self.session.state_of(pid) {
                Some(OutboundState::Released) => self.send_ack(ControlPacketType::PubRel, pid)?,
                Some(OutboundState::Published) => {
                    let Some(mut frame) = self.session.frame_of(pid).map(<[u8]>::to_vec) else {
                        continue;
                    };
                    if self
                        .broker
                        .maximum_packet_size
                        .is_some_and(|max| frame.len() > max as usize)
                    {
                        // [MQTT-3.2.2-15]: the broker's limit forbids it now.
                        log::event(Level::Warn, "resend_dropped")
                            .tenant(&self.tenant)
                            .field("client_id", &self.client_id)
                            .number("packet_id", u64::from(pid))
                            .number("bytes", frame.len() as u64)
                            .field("reason", "above the broker's Maximum Packet Size")
                            .emit();
                        self.end_span(
                            pid,
                            Some("the resend is above the broker's Maximum Packet Size"),
                        );
                        self.complete(pid);
                        continue;
                    }
                    // §3.3.1.1: DUP 1 on a resend.
                    if let Some(b0) = frame.first_mut() {
                        *b0 |= 0x08;
                    }
                    self.send(&frame, "pub dup")?;
                }
                None => {}
            }
        }
        Ok(())
    }

    /// Whether a call to [`Self::next_event`] has input to work on without
    /// reading the socket: a partial packet, or bytes buffered in TLS or
    /// WebSocket.
    pub fn has_buffered_input(&mut self) -> bool {
        !self.held.is_empty() || !self.rx.is_empty() || self.stream.has_buffered_input()
    }

    /// A handle on the socket below, to wait for input without holding the
    /// client (`TcpStream::peek`); the read timeout is the socket's.
    ///
    /// # Errors
    /// The socket cannot be duplicated.
    pub fn socket_clone(&self) -> std::io::Result<TcpStream> {
        self.stream.socket().try_clone()
    }

    /// The Keep Alive of MQTT 5.0 §3.1.2.10 ([MQTT-3.1.2-20]): a PINGREQ
    /// when the client has sent nothing for the Keep Alive; when its
    /// PINGRESP has not come within half of it, the connection is closed
    /// and reported as lost. The caller calls it at least every few hundred
    /// milliseconds.
    ///
    /// # Errors
    /// The PINGREQ cannot be written.
    pub fn keep_alive_tick(&mut self) -> Result<Option<InboundEvent>, ClientError> {
        if self.keep_alive.is_zero() {
            return Ok(None);
        }
        if let Some(sent) = self.ping_sent {
            if sent.elapsed() >= self.keep_alive / 2 {
                self.stream.shutdown_both();
                return Ok(Some(InboundEvent::Disconnected(format!(
                    "no PINGRESP within {:?} of the PINGREQ",
                    self.keep_alive / 2
                ))));
            }
            return Ok(None);
        }
        if self.last_sent.elapsed() >= self.keep_alive {
            let frame = wrap_packet(ControlPacketType::PingReq, 0, &[])
                .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
            self.send(&frame, "pingreq")?;
            self.ping_sent = Some(Instant::now());
        }
        Ok(None)
    }

    /// Blocking read for an inbound event. Returns `None` on a
    /// read timeout (the caller can build a polling loop + check a stop
    /// flag).
    ///
    /// # Errors
    /// IO/Codec — for EOF/closed stream we return
    /// `Disconnected`.
    pub fn next_event(&mut self) -> Result<Option<InboundEvent>, ClientError> {
        if let Some(event) = self.held.pop_front() {
            return Ok(Some(event));
        }
        self.read_event()
    }

    /// One packet from the broker, handled; the event it carries, if any.
    /// A packet that breaks a rule of the protocol ends the connection with
    /// a DISCONNECT naming the rule, one that does not decode with
    /// DISCONNECT 0x81 Malformed Packet (MQTT 5.0 §4.13.1).
    fn read_event(&mut self) -> Result<Option<InboundEvent>, ClientError> {
        match self.read_event_inner() {
            Err(ClientError::Protocol { reason, what }) => {
                self.disconnect_with(reason);
                self.ended_by = Some(reason);
                Ok(Some(InboundEvent::Disconnected(
                    ClientError::Protocol { reason, what }.to_string(),
                )))
            }
            other => other,
        }
    }

    fn read_event_inner(&mut self) -> Result<Option<InboundEvent>, ClientError> {
        let (header, body) = match self.read_packet_nonblocking() {
            Ok(p) => p,
            Err(ClientError::TimedOut) => return Ok(None),
            Err(ClientError::Io(m)) => {
                return Ok(Some(InboundEvent::Disconnected(m)));
            }
            Err(other) => return Err(other),
        };
        match header.packet_type {
            ControlPacketType::Publish => self.on_publish(&header, &body),
            ControlPacketType::PubAck | ControlPacketType::PubRec | ControlPacketType::PubComp => {
                self.on_broker_ack(header.packet_type, &body)?;
                Ok(None)
            }
            ControlPacketType::PubRel => {
                // §4.3.3: the broker releases a QoS 2 message; PUBCOMP ends
                // the flow and frees the identifier.
                let ack = decode_ack_body(&body)
                    .map_err(|e| malformed(format!("a PUBREL that does not decode: {e:?}")))?;
                self.session
                    .received_released(ack.packet_id)
                    .map_err(ClientError::Session)?;
                self.send_ack(ControlPacketType::PubComp, ack.packet_id)?;
                Ok(None)
            }
            ControlPacketType::PingResp => {
                self.ping_sent = None;
                Ok(None)
            }
            ControlPacketType::SubAck => {
                let ack = decode_suback_body_v(&body, self.version)
                    .map_err(|e| malformed(format!("a SUBACK that does not decode: {e:?}")))?;
                if self.subacks_awaited.remove(&ack.packet_id) {
                    self.subacks.insert(ack.packet_id, ack.reason_codes);
                }
                Ok(None)
            }
            ControlPacketType::Auth if self.version == ProtocolVersion::V5 => {
                self.on_auth(&body)?;
                Ok(None)
            }
            ControlPacketType::Disconnect => {
                let reason = decode_disconnect_body(&body).map_or(0, |d| d.reason_code);
                self.ended_by = Some(reason);
                Ok(Some(InboundEvent::Disconnected(format!(
                    "broker disconnect (reason 0x{reason:02x})"
                ))))
            }
            _ => Ok(None),
        }
    }

    /// A PUBLISH of the broker: its properties checked, its Topic Alias
    /// resolved, QoS 1 acknowledged, QoS 2 received.
    fn on_publish(
        &mut self,
        header: &FixedHeader,
        body: &[u8],
    ) -> Result<Option<InboundEvent>, ClientError> {
        // Rebuild full frame for decode_publish (which expects fixed header + body).
        let mut full = Vec::with_capacity(5 + body.len());
        full.push((ControlPacketType::Publish.to_bits() << 4) | (header.flags & 0x0F));
        let len_u32 =
            u32::try_from(body.len()).map_err(|_| ClientError::Codec("len".to_string()))?;
        full.extend_from_slice(
            &encode_vbi(len_u32).ok_or_else(|| ClientError::Codec("vbi".to_string()))?,
        );
        full.extend_from_slice(body);
        let (_, pkt) = decode_publish_v(&full, self.version)
            .map_err(|e| malformed(format!("a PUBLISH that does not decode: {e:?}")))?;
        let properties = match self.version {
            ProtocolVersion::V5 => PropertySet::parse(
                &pkt.properties,
                PropertyContext::Packet(ControlPacketType::Publish),
            )
            .map_err(|reason| ClientError::Protocol {
                reason,
                what: format!(
                    "the properties of a PUBLISH on `{}` break Table 2-4",
                    pkt.topic
                ),
            })?,
            ProtocolVersion::V311 => PropertySet::default(),
        };
        let topic = self.resolve_topic(pkt.topic, properties.topic_alias)?;
        // §4.3.2 / §4.3.3: QoS 1 is answered with PUBACK, QoS 2
        // with PUBREC. Without them the broker's inflight window
        // filled and it stopped sending: 20 of 60 QoS 1 messages
        // arrived from Mosquitto, the rest never, with no error
        // (measured, `tests/mosquitto_qos.rs`).
        match (pkt.qos, pkt.packet_id) {
            (1, Some(pid)) => self.send_ack(ControlPacketType::PubAck, pid)?,
            (2, Some(pid)) => {
                if !self.session.has_received(pid)
                    && self.session.received_len() >= usize::from(self.own_receive_maximum)
                {
                    // §4.9: more unreleased QoS 2 messages than our Receive
                    // Maximum.
                    return Err(ClientError::Protocol {
                        reason: RECEIVE_MAXIMUM_EXCEEDED,
                        what: format!(
                            "a QoS 2 PUBLISH beyond the Receive Maximum {}",
                            self.own_receive_maximum
                        ),
                    });
                }
                let first = self.session.received(pid).map_err(ClientError::Session)?;
                self.send_ack(ControlPacketType::PubRec, pid)?;
                if !first {
                    // A resend of a message already delivered.
                    return Ok(None);
                }
            }
            _ => {}
        }
        Ok(Some(InboundEvent::Publish {
            topic,
            payload: pkt.payload,
            qos: pkt.qos,
            properties: Box::new(properties),
        }))
    }

    /// §3.3.2.3.4: the topic of a PUBLISH with `alias`: an alias above our
    /// Topic Alias Maximum is 0x94; a Topic Name sets the alias, an empty one
    /// takes the topic the alias was set to (§3.3.4: a Protocol Error when
    /// it was never set).
    fn resolve_topic(&mut self, topic: String, alias: Option<u16>) -> Result<String, ClientError> {
        let Some(alias) = alias else {
            if topic.is_empty() {
                return Err(ClientError::Protocol {
                    reason: PROTOCOL_ERROR,
                    what: "a PUBLISH with an empty Topic Name and no Topic Alias".into(),
                });
            }
            return Ok(topic);
        };
        if alias > self.own_topic_alias_maximum {
            return Err(ClientError::Protocol {
                reason: TOPIC_ALIAS_INVALID,
                what: format!(
                    "Topic Alias {alias} above the Topic Alias Maximum {}",
                    self.own_topic_alias_maximum
                ),
            });
        }
        if topic.is_empty() {
            return self
                .aliases
                .get(&alias)
                .cloned()
                .ok_or_else(|| ClientError::Protocol {
                    reason: PROTOCOL_ERROR,
                    what: format!("Topic Alias {alias} was never set"),
                });
        }
        self.aliases.insert(alias, topic.clone());
        Ok(topic)
    }

    /// After a lost connection: the stream is closed and the Session State
    /// let go, so that a new connection under the same client identifier
    /// takes it up again from `mqtt.session_dir` (Spec §9.3; a session
    /// that ends with its connection has nothing to take up).
    pub fn release(&mut self) {
        self.end_open_spans("the connection was lost before the acknowledgement");
        self.stream.shutdown_both();
        self.session = SessionState::in_memory();
        self.resend_waiting.clear();
    }

    /// Sends DISCONNECT with reason 0x00 and closes the stream.
    pub fn graceful_disconnect(mut self) {
        self.disconnect();
    }

    /// [`Self::graceful_disconnect`] on a client that stays owned (the
    /// daemon's sits behind a mutex).
    pub fn disconnect(&mut self) {
        self.disconnect_with(0);
    }

    /// DISCONNECT with `reason` (MQTT 3.1.1: without one), then the stream
    /// is closed.
    fn disconnect_with(&mut self, reason: u8) {
        self.end_open_spans("the connection ended before the acknowledgement");
        let body = DisconnectBody {
            reason_code: reason,
            properties: Vec::new(),
        };
        if let Ok(body_bytes) = encode_disconnect_body_v(&body, self.version) {
            if let Ok(frame) = wrap_packet(ControlPacketType::Disconnect, 0, &body_bytes) {
                let _ = self.stream.write_all(&frame);
            }
        }
        self.stream.shutdown_both();
    }

    fn next_pid(&mut self) -> u16 {
        loop {
            let p = self.next_packet_id;
            self.next_packet_id = self.next_packet_id.wrapping_add(1);
            if self.next_packet_id == 0 {
                self.next_packet_id = 1;
            }
            // An identifier still in flight is not reused (§2.2.1).
            if !self.session.has(p) {
                return p;
            }
        }
    }

    fn read_packet(&mut self) -> Result<(FixedHeader, Vec<u8>), ClientError> {
        // Blocking — the read timeout from set_read_timeout applies.
        self.read_packet_inner()
    }

    fn read_packet_nonblocking(&mut self) -> Result<(FixedHeader, Vec<u8>), ClientError> {
        // With set_read_timeout, Read::read returns an Err(WouldBlock|TimedOut).
        self.read_packet_inner()
    }

    fn read_packet_inner(&mut self) -> Result<(FixedHeader, Vec<u8>), ClientError> {
        let mut chunk = [0u8; 4096];
        loop {
            if let Some(packet) = self.take_buffered_packet()? {
                return Ok(packet);
            }
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(ClientError::Io("read: connection closed".to_string())),
                Ok(n) => self.rx.extend_from_slice(&chunk[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(ClientError::TimedOut);
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(ClientError::Io(format!("read: {e}"))),
            }
        }
    }

    /// The first whole packet in `rx`, removed from it; `None` while the
    /// buffer holds only part of one.
    fn take_buffered_packet(&mut self) -> Result<Option<(FixedHeader, Vec<u8>)>, ClientError> {
        if self.rx.len() < 2 {
            return Ok(None);
        }
        let (remaining, vbi_len) = match decode_vbi(&self.rx[1..]) {
            Ok(v) => v,
            Err(crate::vbi::VbiError::Truncated) => return Ok(None),
            Err(e) => {
                return Err(malformed(format!(
                    "a Remaining Length that does not decode: {e:?}"
                )));
            }
        };
        let header_len = 1 + vbi_len;
        let total = header_len + remaining as usize;
        // §3.1.2.11.4 ([MQTT-3.1.2-24]): the broker must not send a packet
        // above the Maximum Packet Size of our CONNECT; one that announces
        // more is refused with 0x95 before it is buffered, so a broker
        // cannot make the bridge allocate up to 256 MB from a 5-byte header.
        if total > self.own_max_packet_size as usize {
            return Err(ClientError::Protocol {
                reason: PACKET_TOO_LARGE,
                what: format!(
                    "a packet of {total} octets above the Maximum Packet Size {}",
                    self.own_max_packet_size
                ),
            });
        }
        if self.rx.len() < total {
            return Ok(None);
        }
        let byte0 = self.rx[0];
        let body = self.rx[header_len..total].to_vec();
        self.rx.drain(..total);
        let pt_bits = (byte0 >> 4) & 0x0F;
        let packet_type = ControlPacketType::from_bits(pt_bits)
            .ok_or_else(|| malformed(format!("the reserved packet type {pt_bits}")))?;
        Ok(Some((
            FixedHeader {
                packet_type,
                flags: byte0 & 0x0F,
                remaining_length: remaining,
            },
            body,
        )))
    }
}

/// The options byte of a subscription (MQTT 5.0 §3.8.3.1): the QoS and No
/// Local 1 (Spec §4.3), Retain As Published 0, Retain Handling 0; under
/// MQTT 3.1.1 the QoS alone, its other bits are reserved (MQTT 3.1.1
/// §3.8.3.1).
#[must_use]
pub fn subscription_options(version: ProtocolVersion, qos: u8) -> u8 {
    match version {
        ProtocolVersion::V5 => (qos & 0x03) | NO_LOCAL,
        ProtocolVersion::V311 => qos & 0x03,
    }
}

/// The Message Expiry Interval of a message whose interval ends at `at`
/// (Spec §6): the time left in seconds, rounded, at least 1.
///
/// # Errors
/// [`ClientError::Expired`] when no time is left.
fn expiry_left(at: Instant, topic: &str) -> Result<u32, ClientError> {
    let left = at
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| {
            ClientError::Expired(format!(
                "the Message Expiry Interval of a message on {topic} has passed"
            ))
        })?;
    let nanos = u64::try_from(left.as_nanos()).unwrap_or(u64::MAX);
    Ok(super::pump::message_expiry_of(nanos).unwrap_or(1))
}

/// A packet of the broker whose octets do not decode is a Malformed
/// Packet: the client ends the connection with DISCONNECT 0x81 (MQTT 5.0
/// §4.13.1); `what` names the packet and the fault.
fn malformed(what: String) -> ClientError {
    ClientError::Protocol {
        reason: MALFORMED_PACKET,
        what,
    }
}

/// Helper: MQTT-Frame zusammenbauen (FixedHeader + Body).
fn wrap_packet(
    packet_type: ControlPacketType,
    flags: u8,
    body: &[u8],
) -> Result<Vec<u8>, crate::codec::CodecError> {
    let mut out = Vec::with_capacity(5 + body.len());
    let byte0 = (packet_type.to_bits() << 4) | (flags & 0x0F);
    out.push(byte0);
    let len_u32 = u32::try_from(body.len())
        .map_err(|_| crate::codec::CodecError::Vbi(crate::vbi::VbiError::Malformed))?;
    let vbi = encode_vbi(len_u32).ok_or(crate::codec::CodecError::Vbi(
        crate::vbi::VbiError::Malformed,
    ))?;
    out.extend_from_slice(&vbi);
    out.extend_from_slice(body);
    Ok(out)
}

/// The ALPN protocol of MQTT directly on TLS (Spec §7.1, RFC 7301; the
/// IANA registry lists `mqtt`).
const ALPN_MQTT: &str = "mqtt";

/// The ALPN protocol below `wss://`: the TLS channel first carries the
/// HTTP/1.1 upgrade of RFC 6455 §4.
const ALPN_HTTP_1_1: &str = "http/1.1";

/// The ALPN protocol names the TLS handshake offers: `mqtt.tls.alpn`, else
/// `mqtt` (`http/1.1` below WebSocket).
fn alpn_offer(cfg: &DaemonConfig, websocket: bool) -> Vec<Vec<u8>> {
    match &cfg.broker_tls_alpn {
        Some(list) => list.iter().map(|p| p.as_bytes().to_vec()).collect(),
        None if websocket => vec![ALPN_HTTP_1_1.as_bytes().to_vec()],
        None => vec![ALPN_MQTT.as_bytes().to_vec()],
    }
}

/// How long an attempt waits for the TCP connection of one address.
const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long an attempt waits for each answer of the broker: a TLS record,
/// the WebSocket upgrade, the CONNACK.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

/// TCP to `addr` (each address it resolves to, at most
/// [`TCP_CONNECT_TIMEOUT`] each).
fn connect_tcp(addr: &str) -> Result<TcpStream, ClientError> {
    use std::net::ToSocketAddrs;
    let addrs = addr
        .to_socket_addrs()
        .map_err(|e| ClientError::Io(format!("resolve {addr}: {e}")))?;
    let mut last = None;
    for sa in addrs {
        match TcpStream::connect_timeout(&sa, TCP_CONNECT_TIMEOUT) {
            Ok(s) => return Ok(s),
            Err(e) => last = Some(e),
        }
    }
    Err(ClientError::Io(match last {
        Some(e) => format!("connect: {e}"),
        None => format!("resolve {addr}: no address"),
    }))
}

/// An I/O error of the TLS handshake: rustls reports a TLS refusal (an
/// alert, a certificate it does not trust or not for the name, no common
/// protocol) as `InvalidData`, which is [`ClientError::Tls`] (Spec §2 exit
/// 4). A broker that does not answer in time is [`ClientError::TimedOut`],
/// one that closes or resets the connection [`ClientError::Io`]; both are
/// tried again (§9.1 step 6).
fn handshake_error(addr: &str, e: &std::io::Error) -> ClientError {
    match e.kind() {
        std::io::ErrorKind::InvalidData => ClientError::Tls(format!("handshake with {addr}: {e}")),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => ClientError::TimedOut,
        _ => ClientError::Io(format!("TLS handshake with {addr}: {e}")),
    }
}

/// TCP to `addr`, wrapped in TLS when `tls_client_cfg` is set, offering
/// `alpn`; the server name is `mqtt.tls.server_name`, else `host`. A broker
/// without ALPN ignores the offer (RFC 7301 §3.1). The TLS handshake is
/// done here, so a certificate the bridge does not trust is a
/// [`ClientError::Tls`] and not a failed write of the CONNECT.
fn open_stream(
    addr: &str,
    host: &str,
    cfg: &DaemonConfig,
    tls_client_cfg: Option<Arc<ClientConfig>>,
    alpn: &[Vec<u8>],
) -> Result<MqttStream, ClientError> {
    let mut tcp = connect_tcp(addr)?;
    tcp.set_read_timeout(Some(ANSWER_TIMEOUT))
        .map_err(|e| ClientError::Io(format!("set timeout: {e}")))?;
    tcp.set_write_timeout(Some(ANSWER_TIMEOUT))
        .map_err(|e| ClientError::Io(format!("set timeout: {e}")))?;
    Ok(match tls_client_cfg {
        Some(client_cfg) => {
            let mut client_cfg = (*client_cfg).clone();
            client_cfg.alpn_protocols = alpn.to_vec();
            let client_cfg = Arc::new(client_cfg);
            let server_name_str = if cfg.broker_tls_server_name.is_empty() {
                host.to_string()
            } else {
                cfg.broker_tls_server_name.clone()
            };
            let server_name = rustls::pki_types::ServerName::try_from(server_name_str.clone())
                .map_err(|e| ClientError::Tls(format!("server name '{server_name_str}': {e}")))?;
            let mut conn = ClientConnection::new(client_cfg, server_name)
                .map_err(|e| ClientError::Tls(format!("rustls client conn: {e}")))?;
            while conn.is_handshaking() {
                conn.complete_io(&mut tcp)
                    .map_err(|e| handshake_error(addr, &e))?;
            }
            MqttStream::Tls(Box::new(StreamOwned::new(conn, tcp)))
        }
        None => MqttStream::Plain(tcp),
    })
}

/// Connects to the broker of `url`, waiting `mqtt.reconnect` between the
/// attempts while the error is transient ([`ClientError::is_transient`]),
/// until `mqtt.connect_timeout_secs` is up (0, or a time beyond what the
/// clock holds: without end). Spec §9.1 step 6. A wait that would pass that time is shortened to it, and the
/// last attempt starts there; an attempt waits at most
/// `TCP_CONNECT_TIMEOUT` for the TCP connection of each address and
/// `ANSWER_TIMEOUT` for each answer of the broker. `on_retry` is told
/// each failed attempt and the wait before the next.
///
/// # Errors
/// The first error that is not transient; the error of the attempt that
/// ended at or after the time when the time is up; `Ok(None)` when `stop`
/// was set.
pub fn connect_with_backoff(
    url: &super::config::BrokerUrl,
    cfg: &DaemonConfig,
    tls_client_cfg: Option<Arc<ClientConfig>>,
    stop: &AtomicBool,
    mut on_retry: impl FnMut(&ClientError, Duration),
) -> Result<Option<MqttClient>, ClientError> {
    // A time the clock cannot hold is without end, as 0 is.
    let deadline = (cfg.connect_timeout_secs > 0)
        .then(|| Instant::now().checked_add(Duration::from_secs(cfg.connect_timeout_secs)))
        .flatten();
    let mut delay = cfg.reconnect.first_delay();
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let e = match MqttClient::connect_url(url, cfg, tls_client_cfg.clone()) {
            Ok(c) => return Ok(Some(c)),
            Err(e) => e,
        };
        if !e.is_transient() {
            return Err(e);
        }
        let wait = match deadline {
            None => delay,
            Some(d) => match d.checked_duration_since(Instant::now()) {
                Some(left) if !left.is_zero() => delay.min(left),
                _ => return Err(e),
            },
        };
        on_retry(&e, wait);
        let until = Instant::now() + wait;
        while Instant::now() < until {
            if stop.load(Ordering::SeqCst) {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(20).min(until - Instant::now()));
        }
        delay = cfg.reconnect.next_delay(delay);
    }
}

/// Aux: run loop for the client thread that fetches inbound
/// events. Terminates when `stop` is set or the stream
/// disconnects.
pub fn run_inbound_loop<F>(mut client: MqttClient, stop: Arc<AtomicBool>, mut on_event: F)
where
    F: FnMut(InboundEvent),
{
    while !stop.load(Ordering::SeqCst) {
        match client.keep_alive_tick() {
            Ok(Some(lost)) => {
                on_event(lost);
                return;
            }
            Ok(None) => {}
            Err(e) => {
                on_event(InboundEvent::Disconnected(format!("client error: {e}")));
                break;
            }
        }
        match client.next_event() {
            Ok(Some(InboundEvent::Disconnected(reason))) => {
                on_event(InboundEvent::Disconnected(reason));
                break;
            }
            Ok(Some(ev)) => on_event(ev),
            Ok(None) => continue,
            Err(e) => {
                on_event(InboundEvent::Disconnected(format!("client error: {e}")));
                break;
            }
        }
    }
    client.graceful_disconnect();
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn test_client(sock: TcpStream, cfg: &DaemonConfig) -> MqttClient {
        MqttClient::over(MqttStream::Plain(sock), cfg)
    }

    #[test]
    fn mqttbridge_9_2_backoff_connect_aborts_when_stop_set() {
        let stop = AtomicBool::new(true);
        let cfg = DaemonConfig::default_for_dev();
        let url = super::super::config::parse_broker_url("mqtt://127.0.0.1:1").unwrap();
        let t0 = Instant::now();
        let r = connect_with_backoff(&url, &cfg, None, &stop, |_, _| {});
        assert!(matches!(r, Ok(None)), "{:?}", r.err());
        assert!(t0.elapsed() < Duration::from_millis(200));
    }

    /// Spec §9.1: an unreachable broker is tried again after the waits of
    /// `mqtt.reconnect` until `mqtt.connect_timeout_secs` is up: the wait
    /// that would pass it is shortened, the last attempt is at the timeout;
    /// then the last error.
    #[test]
    fn mqttbridge_9_1_an_unreachable_broker_is_retried_until_the_connect_timeout() {
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.connect_timeout_secs = 1;
        cfg.reconnect.initial_delay_ms = 100;
        cfg.reconnect.max_delay_ms = 400;
        let url =
            super::super::config::parse_broker_url(&format!("mqtt://127.0.0.1:{port}")).unwrap();
        let stop = AtomicBool::new(false);
        let mut waits = Vec::new();
        let t0 = Instant::now();
        let r = connect_with_backoff(&url, &cfg, None, &stop, |_, d| waits.push(d.as_millis()));
        let took = t0.elapsed();
        assert!(matches!(r, Err(ClientError::Io(_))), "{:?}", r.err());
        // 100 + 200 + 400 ms leave 300 ms; the fourth wait is that rest.
        assert_eq!(waits.len(), 4, "{waits:?}");
        assert_eq!(waits[..3], [100, 200, 400], "{waits:?}");
        assert!((200..=300).contains(&waits[3]), "{waits:?}");
        assert!(took >= Duration::from_secs(1), "{took:?}");
        assert!(took < Duration::from_millis(1500), "{took:?}");
    }

    /// Spec §3, §9.1: a `mqtt.connect_timeout_secs` beyond what the clock
    /// holds (the configuration takes every u64) retries without end, as
    /// 0 does: the waits are not shortened, and only `stop` ends it.
    #[test]
    fn mqttbridge_9_1_a_connect_timeout_beyond_the_clock_retries_without_end() {
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        let mut cfg =
            DaemonConfig::load_from_str("mqtt:\n  connect_timeout_secs: 18446744073709551615\n")
                .unwrap();
        assert_eq!(cfg.connect_timeout_secs, u64::MAX);
        cfg.reconnect.initial_delay_ms = 50;
        cfg.reconnect.max_delay_ms = 50;
        let url =
            super::super::config::parse_broker_url(&format!("mqtt://127.0.0.1:{port}")).unwrap();
        let stop = AtomicBool::new(false);
        let mut waits = Vec::new();
        let r = connect_with_backoff(&url, &cfg, None, &stop, |_, d| {
            waits.push(d.as_millis());
            if waits.len() == 3 {
                stop.store(true, Ordering::SeqCst);
            }
        });
        assert!(matches!(r, Ok(None)), "{:?}", r.err());
        assert_eq!(waits, [50, 50, 50]);
    }

    /// Spec §9.1: a broker that comes up after the last whole wait of
    /// `mqtt.reconnect` and before `mqtt.connect_timeout_secs` is reached by
    /// the attempt at the timeout. With 1 s, 400 ms and the factor 2 the
    /// attempts are at 0, 0.4 and 1.0 s; the broker listens from 0.7 s.
    #[test]
    fn mqttbridge_9_1_a_broker_that_comes_up_before_the_connect_timeout_is_reached() {
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = free.local_addr().unwrap();
        drop(free);
        let late = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(700));
            let listener = TcpListener::bind(addr).unwrap();
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            crate::net::read_packet(&mut sock).unwrap();
            sock.write_all(&connack(&PropertySet::default())).unwrap();
            let _ = crate::net::read_packet(&mut sock);
        });
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.connect_timeout_secs = 1;
        cfg.reconnect.initial_delay_ms = 400;
        cfg.reconnect.max_delay_ms = 30_000;
        let url = super::super::config::parse_broker_url(&format!("mqtt://{addr}")).unwrap();
        let stop = AtomicBool::new(false);
        let mut waits = Vec::new();
        let t0 = Instant::now();
        let r = connect_with_backoff(&url, &cfg, None, &stop, |_, d| waits.push(d.as_millis()));
        let took = t0.elapsed();
        let client = r.unwrap().expect("connected");
        assert_eq!(waits.len(), 2, "{waits:?}");
        assert_eq!(waits[0], 400, "{waits:?}");
        assert!((500..=600).contains(&waits[1]), "{waits:?}");
        assert!(took >= Duration::from_millis(950), "{took:?}");
        client.graceful_disconnect();
        late.join().unwrap();
    }

    /// Spec §9.1 step 6, §2: a broker that accepts TCP and does not answer
    /// the ClientHello is a broker that does not answer in time, a broker
    /// that closes the connection during the handshake one that cannot be
    /// reached; both are tried again. A TLS refusal (exit 4) is not.
    #[test]
    fn mqttbridge_9_1_a_tls_handshake_without_an_answer_is_tried_again() {
        let silent = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = silent.local_addr().unwrap();
        let held = std::thread::spawn(move || {
            let (sock, _) = silent.accept().unwrap();
            std::thread::sleep(Duration::from_millis(6000));
            drop(sock);
        });
        let tls = Arc::new(
            ClientConfig::builder_with_provider(Arc::new(zerodds_bridge_security::tls_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth(),
        );
        let cfg = DaemonConfig::default_for_dev();
        let t0 = Instant::now();
        let r = open_stream(
            &addr.to_string(),
            "localhost",
            &cfg,
            Some(Arc::clone(&tls)),
            &[],
        );
        let e = r.err().expect("no answer");
        assert!(matches!(e, ClientError::TimedOut), "{e}");
        assert!(e.is_transient());
        assert!(t0.elapsed() >= ANSWER_TIMEOUT, "{:?}", t0.elapsed());
        held.join().unwrap();

        let closing = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = closing.local_addr().unwrap();
        let closer = std::thread::spawn(move || {
            let (mut sock, _) = closing.accept().unwrap();
            let mut hello = [0u8; 16];
            let _ = sock.read(&mut hello);
            drop(sock);
        });
        let e = open_stream(&addr.to_string(), "localhost", &cfg, Some(tls), &[])
            .err()
            .expect("closed");
        assert!(matches!(e, ClientError::Io(_)), "{e}");
        assert!(e.is_transient());
        closer.join().unwrap();
        assert!(
            !handshake_error(
                "x",
                &std::io::Error::new(std::io::ErrorKind::InvalidData, "alert")
            )
            .is_transient()
        );
    }

    #[test]
    fn wrap_packet_publish() {
        let body = b"\x00\x03foo".to_vec();
        let f = wrap_packet(ControlPacketType::Publish, 0, &body).unwrap();
        // Erstes Byte: 0x30 (PUBLISH = 3 << 4, flags = 0).
        assert_eq!(f[0], 0x30);
        // Restbytes: VBI(5) + body.
        assert_eq!(f[1], 5);
        assert_eq!(&f[2..], &body[..]);
    }

    #[test]
    fn wrap_packet_subscribe_has_reserved_bits() {
        let f = wrap_packet(ControlPacketType::Subscribe, 0b0010, b"x").unwrap();
        assert_eq!(f[0] & 0x0F, 0b0010);
    }

    /// Spec §4.1: the CONNECT carries the properties of the table, in the
    /// octets of MQTT 5.0 §3.1.2.11 and §2.2.2.2 (identifier, value; the
    /// order of Table 2-4).
    #[test]
    fn mqttbridge_4_1_the_connect_carries_the_properties_of_the_table() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.session_expiry_interval_secs = 86_400;
        cfg.receive_maximum = 1000;
        cfg.max_packet_size = 1_048_576;
        cfg.topic_alias_maximum = 100;
        let body = connect_body(&cfg, None, None, None).unwrap();
        assert_eq!(body.protocol_version, 5);
        let mut want = vec![0x11, 0x00, 0x01, 0x51, 0x80]; // Session Expiry 86400
        want.extend([0x17, 0x01]); // Request Problem Information 1
        want.extend([0x19, 0x00]); // Request Response Information 0
        want.extend([0x21, 0x03, 0xE8]); // Receive Maximum 1000
        want.extend([0x22, 0x00, 0x64]); // Topic Alias Maximum 100
        for (k, v) in [("zerodds_version", "1.0"), ("zerodds_role", "bridge")] {
            want.push(0x26);
            want.extend((k.len() as u16).to_be_bytes());
            want.extend(k.as_bytes());
            want.extend((v.len() as u16).to_be_bytes());
            want.extend(v.as_bytes());
        }
        want.extend([0x27, 0x00, 0x10, 0x00, 0x00]); // Maximum Packet Size 1048576
        assert_eq!(body.properties, want);
        let set = PropertySet::parse(
            &body.properties,
            PropertyContext::Packet(ControlPacketType::Connect),
        )
        .unwrap();
        assert_eq!(set.session_expiry_interval, Some(86_400));
        assert_eq!(set.authentication_method, None);
    }

    /// Spec §1 L1: in the compatibility mode the CONNECT is MQTT 3.1.1:
    /// Protocol Level 4 and no property block (MQTT 3.1.1 §3.1.2).
    #[test]
    fn mqttbridge_1_l1_the_connect_of_the_3_1_1_mode() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.protocol_version = ProtocolVersion::V311;
        cfg.client_id = "c".into();
        cfg.keep_alive_secs = 30;
        let body = connect_body(&cfg, None, None, None).unwrap();
        let bytes = encode_connect_body_v(&body, ProtocolVersion::V311).unwrap();
        // "MQTT", level 4, flags Clean Session and Will (§6), Keep Alive 30,
        // client id "c", the will topic and the will message.
        let mut want = vec![0, 4, b'M', b'Q', b'T', b'T', 4, 0x06, 0, 30, 0, 1, b'c'];
        let topic = b"$zerodds/c/liveliness_lost";
        want.extend((topic.len() as u16).to_be_bytes());
        want.extend(topic);
        let payload = br#"{"client_id":"c"}"#;
        want.extend((payload.len() as u16).to_be_bytes());
        want.extend(payload);
        assert_eq!(bytes, want);
    }

    /// Spec §6 Liveliness: the CONNECT carries the will on
    /// `<own_topic_prefix>/<client_id>/liveliness_lost` (here the default
    /// `$zerodds`), QoS 0, not retained, with
    /// Payload Format Indicator 1 and Content Type `application/json`; an
    /// empty client identifier or one with a wildcard has none.
    #[test]
    fn mqttbridge_6_the_connect_carries_the_liveliness_will() {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.client_id = "bridge-\"7\"".into();
        let body = connect_body(&cfg, None, None, None).unwrap();
        assert_eq!(
            body.connect_flags
                & (connect_flags::WILL | connect_flags::WILL_QOS_MASK | connect_flags::WILL_RETAIN),
            connect_flags::WILL
        );
        assert_eq!(
            body.will_topic.as_deref(),
            Some("$zerodds/bridge-\"7\"/liveliness_lost")
        );
        assert_eq!(body.will_payload, br#"{"client_id":"bridge-\"7\""}"#);
        let props = PropertySet::parse(&body.will_properties, PropertyContext::Will).unwrap();
        assert_eq!(props.payload_format_indicator, Some(1));
        assert_eq!(props.content_type.as_deref(), Some("application/json"));
        for id in ["", "a+b", "a#"] {
            cfg.client_id = id.into();
            let body = connect_body(&cfg, None, None, None).unwrap();
            assert_eq!(body.connect_flags & connect_flags::WILL, 0, "{id}");
            assert_eq!(body.will_topic, None);
        }
        // Spec §3: under `mqtt.own_topic_prefix` (a broker that keeps the
        // topics beginning with `$` to itself refuses a will there).
        cfg.client_id = "b".into();
        cfg.own_topic_prefix = "plant/zerodds".into();
        let body = connect_body(&cfg, None, None, None).unwrap();
        assert_eq!(
            body.will_topic.as_deref(),
            Some("plant/zerodds/b/liveliness_lost")
        );
    }

    /// A broker on a loopback socket that answers the CONNECT with
    /// `connack`, then runs `then` on the connection.
    fn broker(
        connack: Vec<u8>,
        then: impl FnOnce(TcpStream) + Send + 'static,
    ) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let h = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            crate::net::read_packet(&mut sock).unwrap();
            sock.write_all(&connack).unwrap();
            then(sock);
        });
        (addr, h)
    }

    fn connack(props: &PropertySet) -> Vec<u8> {
        let p = props.encode().unwrap();
        let mut body = vec![0, 0];
        body.extend(encode_vbi(p.len() as u32).unwrap());
        body.extend(p);
        wrap_packet(ControlPacketType::ConnAck, 0, &body).unwrap()
    }

    fn connect_to(addr: std::net::SocketAddr, cfg: &DaemonConfig) -> MqttClient {
        MqttClient::connect("127.0.0.1", addr.port(), cfg).unwrap()
    }

    /// Spec §1 L1, MQTT 5.0 §3.1.2.10 ([MQTT-3.1.2-20]): an idle client
    /// sends PINGREQ after the Keep Alive; without a PINGRESP it closes the
    /// connection after half of it more and reports it lost.
    #[test]
    fn mqttbridge_1_l1_an_idle_client_pings_and_a_missing_pingresp_ends_the_connection() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack(&PropertySet::default()), move |mut sock| {
            let t0 = Instant::now();
            let (byte0, body) = crate::net::read_packet(&mut sock).unwrap();
            tx.send((byte0, body.len(), t0.elapsed())).unwrap();
            // No PINGRESP: the client gives up and closes.
            let mut rest = [0u8; 8];
            let closed = matches!(sock.read(&mut rest), Ok(0) | Err(_));
            tx.send((u8::from(closed), 0, t0.elapsed())).unwrap();
        });
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.keep_alive_secs = 1;
        let mut c = connect_to(addr, &cfg);
        let t0 = Instant::now();
        let lost = loop {
            if let Some(ev) = c.keep_alive_tick().unwrap() {
                break ev;
            }
            assert!(t0.elapsed() < Duration::from_secs(4), "no end");
            std::thread::sleep(Duration::from_millis(50));
        };
        let (byte0, len, at) = rx.recv().unwrap();
        assert_eq!((byte0, len), (0xC0, 0), "PINGREQ");
        assert!(at >= Duration::from_millis(900), "{at:?}");
        assert!(matches!(lost, InboundEvent::Disconnected(ref m) if m.contains("PINGRESP")));
        assert!(
            t0.elapsed() >= Duration::from_millis(1400),
            "{:?}",
            t0.elapsed()
        );
        let (closed, _, _) = rx.recv().unwrap();
        assert_eq!(closed, 1);
        h.join().unwrap();
    }

    /// A PINGRESP clears the PINGREQ: the client keeps the connection.
    #[test]
    fn mqttbridge_1_l1_a_pingresp_keeps_the_connection() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack(&PropertySet::default()), move |mut sock| {
            while let Ok((byte0, _)) = crate::net::read_packet(&mut sock) {
                tx.send(byte0).unwrap();
                if byte0 == 0xC0 {
                    sock.write_all(&[0xD0, 0x00]).unwrap();
                }
            }
        });
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.keep_alive_secs = 1;
        let mut c = connect_to(addr, &cfg);
        c.stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_millis(3300) {
            assert!(c.keep_alive_tick().unwrap().is_none(), "lost");
            assert!(
                !matches!(c.next_event().unwrap(), Some(InboundEvent::Disconnected(_))),
                "lost"
            );
        }
        drop(c);
        h.join().unwrap();
        let pings: Vec<u8> = rx.iter().collect();
        assert_eq!(pings, [0xC0; 3], "three PINGREQs in 3.3 s, each answered");
    }

    /// Spec §4.1, [MQTT-3.1.2-21]: the Server Keep Alive of the CONNACK
    /// replaces `mqtt.keep_alive_secs`; the PINGREQ comes after it.
    #[test]
    fn mqttbridge_4_1_the_server_keep_alive_replaces_the_configured_one() {
        let (tx, rx) = std::sync::mpsc::channel();
        let props = PropertySet {
            server_keep_alive: Some(1),
            ..PropertySet::default()
        };
        let (addr, h) = broker(connack(&props), move |mut sock| {
            let t0 = Instant::now();
            let (byte0, _) = crate::net::read_packet(&mut sock).unwrap();
            tx.send((byte0, t0.elapsed())).unwrap();
        });
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.keep_alive_secs = 60;
        let mut c = connect_to(addr, &cfg);
        assert_eq!(c.keep_alive(), Duration::from_secs(1));
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_millis(1500) {
            let _ = c.keep_alive_tick().unwrap();
            std::thread::sleep(Duration::from_millis(50));
        }
        let (byte0, at) = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(byte0, 0xC0);
        assert!(at < Duration::from_millis(1400), "{at:?}");
        h.join().unwrap();
    }

    /// Spec §4.1: the limits of the CONNACK hold for the bridge's PUBLISH
    /// packets: Maximum QoS ([MQTT-3.2.2-11]), Retain Available
    /// ([MQTT-3.2.2-14]), Maximum Packet Size ([MQTT-3.2.2-15]); such a
    /// PUBLISH is not sent, the connection stands. The CONNACK's Assigned
    /// Client Identifier is read (its Session Present in
    /// `mqttbridge_4_1_a_resumed_session_resends_its_state`).
    #[test]
    fn mqttbridge_4_1_the_limits_of_the_connack_hold_for_the_publish() {
        let props = PropertySet {
            maximum_qos: Some(1),
            retain_available: Some(0),
            maximum_packet_size: Some(64),
            assigned_client_identifier: Some("auto-7".into()),
            ..PropertySet::default()
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack(&props), move |mut sock| {
            while let Ok((byte0, body)) = crate::net::read_packet(&mut sock) {
                tx.send((byte0, body)).unwrap();
            }
        });
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.client_id = String::new();
        let mut c = connect_to(addr, &cfg);
        assert!(!c.session_present());
        assert_eq!(c.assigned_client_id(), Some("auto-7"));
        let none = PropertySet::default();
        for (qos, retain, len) in [(2u8, false, 4usize), (0, true, 4), (0, false, 100)] {
            let e = c
                .publish("t", &vec![0; len], qos, retain, &none)
                .unwrap_err();
            assert!(matches!(e, ClientError::NotSent(_)), "{e}");
        }
        c.publish("t", &[1, 2, 3], 1, false, &none).unwrap();
        c.disconnect();
        let got: Vec<u8> = rx.iter().map(|(b, _)| b).collect();
        // One PUBLISH QoS 1 (0x32), then DISCONNECT (0xE0).
        assert_eq!(got, [0x32, 0xE0]);
        h.join().unwrap();
    }

    /// The PUBLISH of the client carries the property set it is given.
    #[test]
    fn mqttbridge_4_2_the_publish_carries_its_properties() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack(&PropertySet::default()), move |mut sock| {
            let (byte0, body) = crate::net::read_packet(&mut sock).unwrap();
            tx.send((byte0, body)).unwrap();
        });
        let mut c = connect_to(addr, &DaemonConfig::default_for_dev());
        let props = PropertySet {
            payload_format_indicator: Some(0),
            content_type: Some("application/x-dds-cdr2".into()),
            user_properties: vec![("zerodds_type".into(), "T".into())],
            ..PropertySet::default()
        };
        c.publish("t", &[0, 7, 0, 0, 9], 0, false, &props).unwrap();
        let (byte0, body) = rx.recv().unwrap();
        let mut full = vec![byte0];
        full.extend(encode_vbi(body.len() as u32).unwrap());
        full.extend(body);
        let (_, pkt) = crate::codec::decode_publish(&full).unwrap();
        assert_eq!(pkt.properties, props.encode().unwrap());
        assert_eq!(pkt.payload, [0, 7, 0, 0, 9]);
        h.join().unwrap();
    }

    fn publish_frame(topic: &str, qos: u8, pid: Option<u16>, props: &PropertySet) -> Vec<u8> {
        crate::codec::encode_publish(&PublishPacket {
            dup: false,
            qos,
            retain: false,
            topic: topic.into(),
            packet_id: pid,
            properties: props.encode().unwrap(),
            payload: vec![0, 1, 0, 0, 5],
        })
        .unwrap()
    }

    /// Reads events until one arrives.
    fn event(c: &mut MqttClient) -> InboundEvent {
        let t0 = Instant::now();
        loop {
            if let Some(ev) = c.next_event().unwrap() {
                return ev;
            }
            assert!(t0.elapsed() < Duration::from_secs(5), "no event");
        }
    }

    /// Spec §4.1, MQTT 5.0 §3.3.2.3.4: a Topic Alias of the broker within
    /// our Topic Alias Maximum is resolved; one above it ends the
    /// connection with DISCONNECT 0x94.
    #[test]
    fn mqttbridge_4_1_an_inbound_topic_alias_is_resolved() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack(&PropertySet::default()), move |mut sock| {
            let alias = |a| PropertySet {
                topic_alias: Some(a),
                ..PropertySet::default()
            };
            sock.write_all(&publish_frame("plant/a", 0, None, &alias(3)))
                .unwrap();
            sock.write_all(&publish_frame("", 0, None, &alias(3)))
                .unwrap();
            sock.write_all(&publish_frame("x", 0, None, &alias(101)))
                .unwrap();
            let (byte0, body) = crate::net::read_packet(&mut sock).unwrap();
            tx.send((byte0, body)).unwrap();
        });
        let mut c = connect_to(addr, &DaemonConfig::default_for_dev());
        for _ in 0..2 {
            match event(&mut c) {
                InboundEvent::Publish { topic, .. } => assert_eq!(topic, "plant/a"),
                other => panic!("{other:?}"),
            }
        }
        assert!(matches!(event(&mut c), InboundEvent::Disconnected(m) if m.contains("0x94")));
        let (byte0, body) = rx.recv().unwrap();
        assert_eq!((byte0, body[0]), (0xE0, TOPIC_ALIAS_INVALID));
        h.join().unwrap();
    }

    /// Spec §4.1, [MQTT-3.1.2-24]: a packet above our Maximum Packet Size
    /// ends the connection with DISCONNECT 0x95 before it is read whole.
    #[test]
    fn mqttbridge_4_1_a_packet_above_the_maximum_packet_size_is_0x95() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack(&PropertySet::default()), move |mut sock| {
            let big = crate::codec::encode_publish(&PublishPacket {
                dup: false,
                qos: 0,
                retain: false,
                topic: "t".into(),
                packet_id: None,
                properties: Vec::new(),
                payload: vec![0; 200],
            })
            .unwrap();
            sock.write_all(&big).unwrap();
            let (byte0, body) = crate::net::read_packet(&mut sock).unwrap();
            tx.send((byte0, body)).unwrap();
        });
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.max_packet_size = 128;
        let mut c = connect_to(addr, &cfg);
        assert!(matches!(event(&mut c), InboundEvent::Disconnected(m) if m.contains("0x95")));
        let (byte0, body) = rx.recv().unwrap();
        assert_eq!((byte0, body[0]), (0xE0, PACKET_TOO_LARGE));
        h.join().unwrap();
    }

    /// Spec §4.1, MQTT 5.0 §4.9: more unreleased QoS 2 messages of the
    /// broker than our Receive Maximum end the connection with 0x93.
    #[test]
    fn mqttbridge_4_1_qos_2_beyond_our_receive_maximum_is_0x93() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack(&PropertySet::default()), move |mut sock| {
            let none = PropertySet::default();
            for pid in 1..=3u16 {
                sock.write_all(&publish_frame("t", 2, Some(pid), &none))
                    .unwrap();
            }
            while let Ok((byte0, body)) = crate::net::read_packet(&mut sock) {
                tx.send((byte0, body)).unwrap();
            }
        });
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.receive_maximum = 2;
        let mut c = connect_to(addr, &cfg);
        for _ in 0..2 {
            assert!(matches!(event(&mut c), InboundEvent::Publish { .. }));
        }
        assert!(matches!(event(&mut c), InboundEvent::Disconnected(m) if m.contains("0x93")));
        let got: Vec<(u8, u8)> = rx.iter().map(|(b, body)| (b, body[0])).collect();
        // PUBREC 1, PUBREC 2, DISCONNECT 0x93.
        assert_eq!(got.len(), 3, "{got:?}");
        assert_eq!(got[2], (0xE0, RECEIVE_MAXIMUM_EXCEEDED));
        h.join().unwrap();
    }

    /// The properties of a broker's PUBLISH reach the event; under MQTT
    /// 3.1.1 a PUBLISH has none and is read without a property block.
    #[test]
    fn mqttbridge_4_2_the_properties_of_an_inbound_publish_reach_the_event() {
        let props = PropertySet {
            content_type: Some("application/x-dds-cdr".into()),
            user_properties: vec![("zerodds_source_ts_ns".into(), "5".into())],
            ..PropertySet::default()
        };
        let frame = publish_frame("t", 0, None, &props);
        let (addr, h) = broker(connack(&PropertySet::default()), move |mut sock| {
            sock.write_all(&frame).unwrap();
        });
        let mut c = connect_to(addr, &DaemonConfig::default_for_dev());
        match event(&mut c) {
            InboundEvent::Publish { properties, .. } => assert_eq!(*properties, props),
            other => panic!("{other:?}"),
        }
        h.join().unwrap();

        let v311 = crate::codec::encode_publish_v(
            &PublishPacket {
                dup: false,
                qos: 1,
                retain: false,
                topic: "t".into(),
                packet_id: Some(9),
                properties: Vec::new(),
                payload: vec![0, 1, 0, 0, 5],
            },
            ProtocolVersion::V311,
        )
        .unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(vec![0x20, 0x02, 0x00, 0x00], move |mut sock| {
            sock.write_all(&v311).unwrap();
            let (byte0, body) = crate::net::read_packet(&mut sock).unwrap();
            tx.send((byte0, body)).unwrap();
        });
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.protocol_version = ProtocolVersion::V311;
        let mut c = connect_to(addr, &cfg);
        match event(&mut c) {
            InboundEvent::Publish {
                payload,
                properties,
                ..
            } => {
                assert_eq!(payload, [0, 1, 0, 0, 5]);
                assert_eq!(*properties, PropertySet::default());
            }
            other => panic!("{other:?}"),
        }
        // The MQTT 3.1.1 PUBACK: the packet identifier alone.
        assert_eq!(rx.recv().unwrap(), (0x40, vec![0, 9]));
        h.join().unwrap();
    }

    /// Spec §2 exits 5 on a refusal of the credentials or the
    /// authorization; MQTT 3.1.1 return codes 4 and 5 are those.
    #[test]
    fn mqttbridge_2_which_connack_refusal_is_an_auth_error() {
        let refusal = |reason, version| ClientError::ConnAck {
            reason,
            version,
            detail: String::new(),
        };
        for r in [0x86, 0x87, 0x8A, 0x8C] {
            assert!(refusal(r, ProtocolVersion::V5).is_auth_refusal());
        }
        for r in [0x80, 0x84, 0x85, 0x88, 0x9C] {
            assert!(!refusal(r, ProtocolVersion::V5).is_auth_refusal());
        }
        assert!(refusal(4, ProtocolVersion::V311).is_auth_refusal());
        assert!(refusal(5, ProtocolVersion::V311).is_auth_refusal());
        assert!(!refusal(2, ProtocolVersion::V311).is_auth_refusal());
        assert!(refusal(0x88, ProtocolVersion::V5).is_transient());
        assert!(refusal(3, ProtocolVersion::V311).is_transient());
        assert!(!refusal(0x87, ProtocolVersion::V5).is_transient());
    }

    /// A read timeout inside a packet: the first half stays buffered and the
    /// packet is read whole once the rest arrives. With `read_exact` the
    /// first half was dropped and the stream read from the middle of a
    /// packet.
    #[test]
    fn a_read_timeout_inside_a_packet_keeps_the_part_already_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let frame = crate::codec::encode_publish(&PublishPacket {
            dup: false,
            qos: 0,
            retain: false,
            topic: "t/half".to_string(),
            packet_id: None,
            properties: Vec::new(),
            payload: vec![7u8; 300],
        })
        .unwrap();
        let (first, rest) = frame.split_at(frame.len() / 2);
        let (first, rest) = (first.to_vec(), rest.to_vec());
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            sock.write_all(&first).unwrap();
            std::thread::sleep(Duration::from_millis(400));
            sock.write_all(&rest).unwrap();
            std::thread::sleep(Duration::from_millis(400));
        });
        let sock = TcpStream::connect(addr).unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(150)))
            .unwrap();
        let mut client = test_client(sock, &DaemonConfig::default_for_dev());
        let mut timeouts = 0;
        let got = loop {
            match client.next_event().unwrap() {
                None => timeouts += 1,
                Some(InboundEvent::Publish { topic, payload, .. }) => break (topic, payload),
                Some(other) => panic!("{other:?}"),
            }
            assert!(timeouts < 20, "no PUBLISH");
        };
        assert!(timeouts >= 1, "the timeout fell inside the packet");
        assert_eq!(got.0, "t/half");
        assert_eq!(got.1, vec![7u8; 300]);
        server.join().unwrap();
    }

    /// A configuration of a session that outlives its connection, its
    /// state in a fresh directory of this test process.
    fn lasting_cfg(leaf: &str, client_id: &str) -> DaemonConfig {
        let dir = std::env::temp_dir()
            .join(format!("zerodds-mqtt-client-{}", std::process::id()))
            .join(leaf);
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.client_id = client_id.to_string();
        cfg.clean_start = false;
        cfg.session_expiry_interval_secs = 300;
        cfg.session_dir = dir.display().to_string();
        cfg
    }

    /// A CONNACK with Session Present `present` and no properties.
    fn connack_present(present: bool) -> Vec<u8> {
        wrap_packet(ControlPacketType::ConnAck, 0, &[u8::from(present), 0, 0]).unwrap()
    }

    /// The Clean Start flag and the packet type of what a broker reads.
    fn clean_start_of(body: &[u8]) -> bool {
        crate::control_packets::decode_connect_body(body)
            .unwrap()
            .connect_flags
            & connect_flags::CLEAN_START
            != 0
    }

    /// Spec §4.1, [MQTT-3.1.2-23], [MQTT-4.4.0-1], [MQTT-4.6.0-1],
    /// [MQTT-4.6.0-4]: the state an earlier client left in
    /// `mqtt.session_dir` is resent after Session Present 1 — the QoS 1
    /// PUBLISH with DUP and its identifier, then the PUBREL of the released
    /// QoS 2 one; a new PUBLISH takes an identifier not in use. A QoS 2
    /// PUBLISH of the broker answered with PUBREC before the restart is
    /// not delivered again ([MQTT-4.3.3-10]); a new one is.
    #[test]
    fn mqttbridge_4_1_a_resumed_session_resends_its_state() {
        let cfg = lasting_cfg("resume", "resume-cli");
        {
            let mut s = SessionState::for_connect(&cfg).unwrap();
            s.settle(false).unwrap();
            s.published(
                1,
                &publish_frame("t/one", 1, Some(1), &PropertySet::default()),
            )
            .unwrap();
            s.published(
                2,
                &publish_frame("t/two", 2, Some(2), &PropertySet::default()),
            )
            .unwrap();
            s.released(2).unwrap();
            assert!(s.received(40).unwrap());
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let broker = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let (_, body) = crate::net::read_packet(&mut sock).unwrap();
            assert!(!clean_start_of(&body), "Clean Start 0");
            sock.write_all(&connack_present(true)).unwrap();
            for _ in 0..3 {
                tx.send(crate::net::read_packet(&mut sock).unwrap())
                    .unwrap();
            }
            // The broker's QoS 2 PUBLISH 40 again (DUP), then a new one, 41.
            let mut again = publish_frame("t/in", 2, Some(40), &PropertySet::default());
            again[0] |= 0x08;
            sock.write_all(&again).unwrap();
            sock.write_all(&publish_frame("t/in", 2, Some(41), &PropertySet::default()))
                .unwrap();
            for _ in 0..2 {
                tx.send(crate::net::read_packet(&mut sock).unwrap())
                    .unwrap();
            }
            sock.write_all(&[0x40, 0x02, 0x00, 0x01]).unwrap(); // PUBACK 1
            sock.write_all(&[0x70, 0x02, 0x00, 0x02]).unwrap(); // PUBCOMP 2
            sock.write_all(&[0x40, 0x02, 0x00, 0x03]).unwrap(); // PUBACK 3
            let _ = crate::net::read_packet(&mut sock);
        });
        let mut c = MqttClient::connect("127.0.0.1", addr.port(), &cfg).unwrap();
        assert!(c.session_present());
        c.publish("t/new", &[9], 1, false, &PropertySet::default())
            .unwrap();
        let (b0, body) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(b0, 0x30 | 0x08 | 0x02, "PUBLISH, DUP, QoS 1");
        let mut full = vec![b0];
        full.extend(encode_vbi(u32::try_from(body.len()).unwrap()).unwrap());
        full.extend(&body);
        let (_, p) = crate::codec::decode_publish(&full).unwrap();
        assert_eq!(
            (p.topic.as_str(), p.packet_id, p.dup),
            ("t/one", Some(1), true)
        );
        let (b0, body) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!((b0, &body[..2]), (0x62, &[0x00, 0x02][..]), "PUBREL 2");
        let (b0, body) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(b0 & 0xF0, 0x30, "the new PUBLISH");
        let mut full = vec![b0];
        full.extend(encode_vbi(u32::try_from(body.len()).unwrap()).unwrap());
        full.extend(&body);
        let (_, p) = crate::codec::decode_publish(&full).unwrap();
        assert_eq!((p.topic.as_str(), p.packet_id), ("t/new", Some(3)));
        let delivered = event(&mut c);
        assert!(
            matches!(&delivered, InboundEvent::Publish { topic, .. } if topic == "t/in"),
            "{delivered:?}"
        );
        let mut pubrecs = Vec::new();
        for _ in 0..2 {
            let (b0, body) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(b0, 0x50, "PUBREC");
            pubrecs.push(u16::from_be_bytes([body[0], body[1]]));
        }
        assert_eq!(pubrecs, [40, 41]);
        let t0 = Instant::now();
        while c.pending_acks() > 0 {
            assert!(t0.elapsed() < Duration::from_secs(5), "acks");
            if let Some(InboundEvent::Publish { .. }) = c.next_event().unwrap() {
                panic!("40 delivered a second time");
            }
        }
        c.graceful_disconnect();
        broker.join().unwrap();
        let s = SessionState::for_connect(&cfg).unwrap();
        assert!(s.resend_order().is_empty(), "completed in the file too");
        assert!(s.has_received(40) && s.has_received(41), "no PUBREL yet");
    }

    /// MQTT 5.0 §4.9: the resend after Session Present 1 holds the broker's
    /// Receive Maximum (1 here): the second PUBLISH goes out again once the
    /// first one is acknowledged, not before.
    #[test]
    fn mqttbridge_4_1_the_resend_holds_the_receive_maximum() {
        let cfg = lasting_cfg("resend-rm", "resend-rm-cli");
        {
            let mut s = SessionState::for_connect(&cfg).unwrap();
            s.settle(false).unwrap();
            for pid in [11u16, 12] {
                s.published(
                    pid,
                    &publish_frame("t/rm", 1, Some(pid), &PropertySet::default()),
                )
                .unwrap();
            }
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let props = PropertySet {
            receive_maximum: Some(1),
            ..PropertySet::default()
        };
        let mut ack = connack(&props);
        ack[2] = 1; // Session Present
        let (addr, h) = broker(ack, move |mut sock| {
            // The body: topic length, `t/rm`, the packet identifier.
            let (_, body) = crate::net::read_packet(&mut sock).unwrap();
            tx.send(u16::from_be_bytes([body[6], body[7]])).unwrap();
            // Nothing more may come before the PUBACK.
            sock.set_read_timeout(Some(Duration::from_millis(300)))
                .unwrap();
            assert!(
                crate::net::read_packet(&mut sock).is_err(),
                "a second resend"
            );
            sock.write_all(&[0x40, 0x02, 0x00, 11]).unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let (_, body) = crate::net::read_packet(&mut sock).unwrap();
            tx.send(u16::from_be_bytes([body[6], body[7]])).unwrap();
            sock.write_all(&[0x40, 0x02, 0x00, 12]).unwrap();
            let _ = crate::net::read_packet(&mut sock);
        });
        let mut c = MqttClient::connect("127.0.0.1", addr.port(), &cfg).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), 11);
        let t0 = Instant::now();
        while c.pending_acks() > 0 {
            assert!(t0.elapsed() < Duration::from_secs(5), "acks");
            let _ = c.next_event().unwrap();
        }
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), 12);
        c.graceful_disconnect();
        h.join().unwrap();
    }

    /// Spec §6: the time left of a lifespan beyond `u64::MAX` ns (a sample
    /// that arrived just now under the longest `qos.lifespan_ms`) is the
    /// largest Message Expiry Interval; the rounding used to overflow.
    #[test]
    fn mqttbridge_6_the_time_left_of_the_longest_lifespan_is_the_largest_interval() {
        let at = Instant::now() + Duration::from_secs(600 * 365 * 86_400);
        assert_eq!(expiry_left(at, "t").unwrap(), u32::MAX);
        let at = Instant::now() + Duration::from_millis(2_900);
        assert_eq!(expiry_left(at, "t").unwrap(), 3);
        let gone = Instant::now()
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        assert!(matches!(
            expiry_left(gone, "t"),
            Err(ClientError::Expired(_))
        ));
    }

    /// Spec §6, §9.3: a QoS 1 PUBLISH that waits for a send slot (the
    /// broker's Receive Maximum 1 is reached) ends with an I/O error when
    /// the broker closes the connection, not after [`SEND_SLOT_WAIT`] as
    /// `Busy`; the event of the end is kept once for the inbound loop.
    #[test]
    fn mqttbridge_9_3_the_end_of_the_connection_ends_the_wait_for_a_send_slot() {
        let props = PropertySet {
            receive_maximum: Some(1),
            ..PropertySet::default()
        };
        let (addr, h) = broker(connack(&props), |mut sock| {
            // The first QoS 1 PUBLISH stays unacknowledged; then the
            // broker closes.
            crate::net::read_packet(&mut sock).unwrap();
            drop(sock);
        });
        let mut c = connect_to(addr, &DaemonConfig::default_for_dev());
        let none = PropertySet::default();
        c.publish("t/slot", &[1], 1, false, &none).unwrap();
        h.join().unwrap();
        let t0 = Instant::now();
        for _ in 0..2 {
            let e = c.publish("t/slot", &[2], 1, false, &none).unwrap_err();
            assert!(matches!(e, ClientError::Io(_)), "{e}");
        }
        assert!(t0.elapsed() < ANSWER_TIMEOUT, "{:?}", t0.elapsed());
        assert_eq!(c.held.len(), 1, "one event of the end");
        assert!(matches!(
            c.next_event().unwrap(),
            Some(InboundEvent::Disconnected(_))
        ));
        assert_eq!(c.pending_acks(), 1);
    }

    /// Spec §9.3, MQTT 5.0 §4.13.1: a PUBACK that does not decode, read
    /// while a QoS 1 PUBLISH waits for a send slot, ends the connection
    /// with DISCONNECT 0x81 and the wait with an I/O error, on which the
    /// pump keeps the sample for the next connection; the waiting PUBLISH
    /// does not go out, and the event of the end is kept once for the
    /// inbound loop.
    #[test]
    fn mqttbridge_9_3_a_malformed_ack_ends_the_wait_for_a_send_slot_and_the_connection() {
        let props = PropertySet {
            receive_maximum: Some(1),
            ..PropertySet::default()
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack(&props), move |mut sock| {
            // The first QoS 1 PUBLISH; its PUBACK lacks the second octet
            // of the Packet Identifier.
            crate::net::read_packet(&mut sock).unwrap();
            sock.write_all(&[0x40, 0x01, 0x00]).unwrap();
            while let Ok(packet) = crate::net::read_packet(&mut sock) {
                tx.send(packet).unwrap();
            }
        });
        let mut c = connect_to(addr, &DaemonConfig::default_for_dev());
        let none = PropertySet::default();
        c.publish("t/slot", &[1], 1, false, &none).unwrap();
        let e = c.publish("t/slot", &[2], 1, false, &none).unwrap_err();
        assert!(matches!(e, ClientError::Io(_)), "{e}");
        assert_eq!(c.ended_by(), Some(MALFORMED_PACKET));
        h.join().unwrap();
        let after: Vec<(u8, Vec<u8>)> = rx.iter().collect();
        assert_eq!(after.len(), 1, "the DISCONNECT alone, then the close");
        assert_eq!(
            (after[0].0, after[0].1.first().copied()),
            (0xE0, Some(MALFORMED_PACKET))
        );
        assert_eq!(c.held.len(), 1, "one event of the end");
        let ev = c.next_event().unwrap();
        assert!(
            matches!(&ev, Some(InboundEvent::Disconnected(m)) if m.contains("PubAck")),
            "{ev:?}"
        );
        assert_eq!(c.pending_acks(), 1, "the first PUBLISH, not completed");
    }

    /// MQTT 5.0 §4.13.1: a packet of the broker that does not decode (a
    /// PUBLISH whose Topic Name is cut short, the reserved packet type 0)
    /// ends the connection with DISCONNECT 0x81 Malformed Packet; the
    /// inbound loop gets the end as an event and connects again.
    #[test]
    fn mqttbridge_4_1_a_packet_that_does_not_decode_ends_the_connection_with_0x81() {
        for bad in [vec![0x32, 0x03, 0x00, 0x05, b'a'], vec![0x00, 0x00]] {
            let (tx, rx) = std::sync::mpsc::channel();
            let (addr, h) = broker(connack(&PropertySet::default()), move |mut sock| {
                sock.write_all(&bad).unwrap();
                tx.send(crate::net::read_packet(&mut sock).ok()).unwrap();
            });
            let mut c = connect_to(addr, &DaemonConfig::default_for_dev());
            let ev = event(&mut c);
            assert!(
                matches!(&ev, InboundEvent::Disconnected(m) if m.contains("DISCONNECT 0x81")),
                "{ev:?}"
            );
            assert_eq!(c.ended_by(), Some(MALFORMED_PACKET));
            let (b0, body) = rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .expect("DISCONNECT");
            assert_eq!((b0, body.first().copied()), (0xE0, Some(MALFORMED_PACKET)));
            h.join().unwrap();
        }
    }

    /// [MQTT-3.2.2-2]: Session Present 1 to a CONNECT with Clean Start 1
    /// breaks the protocol; the client closes with DISCONNECT 0x82 and the
    /// connect fails.
    #[test]
    fn mqttbridge_4_1_session_present_after_clean_start_is_a_protocol_error() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack_present(true), move |mut sock| {
            tx.send(crate::net::read_packet(&mut sock).ok()).unwrap();
        });
        let cfg = DaemonConfig::default_for_dev();
        let e = MqttClient::connect("127.0.0.1", addr.port(), &cfg)
            .err()
            .expect("refused");
        assert!(
            matches!(
                e,
                ClientError::Protocol {
                    reason: PROTOCOL_ERROR,
                    ..
                }
            ),
            "{e}"
        );
        let (b0, body) = rx.recv().unwrap().expect("DISCONNECT");
        assert_eq!((b0, body.first().copied()), (0xE0, Some(PROTOCOL_ERROR)));
        h.join().unwrap();
    }

    /// [MQTT-3.2.2-4]: a broker that resumes a session the client holds no
    /// state of gets its connection closed; the client begins a new session
    /// with Clean Start 1. Under MQTT 3.1.1 a third CONNECT with Clean
    /// Session 0 begins the lasting one. Then the client holds the state.
    #[test]
    fn mqttbridge_4_1_session_present_without_state_begins_a_new_session() {
        for (version, connects) in [(ProtocolVersion::V5, 2usize), (ProtocolVersion::V311, 3)] {
            let mut cfg = lasting_cfg(&format!("unknown-{connects}"), "unknown-cli");
            cfg.protocol_version = version;
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let broker = std::thread::spawn(move || {
                let mut flags = Vec::new();
                for n in 0..connects {
                    let (mut sock, _) = listener.accept().unwrap();
                    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                    let (_, body) = crate::net::read_packet(&mut sock).unwrap();
                    let clean = if version == ProtocolVersion::V5 {
                        clean_start_of(&body)
                    } else {
                        // MQTT 3.1.1: name, level, flags (§3.1.2).
                        body[7] & connect_flags::CLEAN_START != 0
                    };
                    flags.push(clean);
                    let present = n == 0;
                    let ack = if version == ProtocolVersion::V5 {
                        connack_present(present)
                    } else {
                        wrap_packet(ControlPacketType::ConnAck, 0, &[u8::from(present), 0]).unwrap()
                    };
                    sock.write_all(&ack).unwrap();
                    if n + 1 < connects {
                        // The client closes this connection.
                        let mut rest = Vec::new();
                        let _ = sock.read_to_end(&mut rest);
                    } else {
                        let _ = crate::net::read_packet(&mut sock);
                    }
                }
                flags
            });
            let c = MqttClient::connect("127.0.0.1", addr.port(), &cfg).unwrap();
            assert!(!c.session_present(), "{version:?}");
            assert!(c.session.known());
            c.graceful_disconnect();
            let flags = broker.join().unwrap();
            let want: &[bool] = if connects == 2 {
                &[false, true]
            } else {
                &[false, true, false]
            };
            assert_eq!(flags, want, "{version:?}: the Clean Start of each CONNECT");
            assert!(super::super::session::session_file(&cfg).exists());
        }
    }

    /// A SUBACK with `reasons` for `packet_id`.
    fn suback(packet_id: u16, reasons: &[u8]) -> Vec<u8> {
        let mut body = packet_id.to_be_bytes().to_vec();
        body.push(0);
        body.extend_from_slice(reasons);
        wrap_packet(ControlPacketType::SubAck, 0, &body).unwrap()
    }

    /// Spec §4.3, MQTT 5.0 §3.8: one SUBSCRIBE per filter, each with its
    /// Subscription Identifier (property 0x0B) and the options QoS, No Local
    /// 1, Retain As Published 0, Retain Handling 0, octet for octet; the
    /// SUBACK of each is read: a granted QoS, a refusal.
    #[test]
    fn mqttbridge_4_3_the_subscribe_carries_identifier_and_no_local() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(connack(&PropertySet::default()), move |mut sock| {
            for answer in [1u8, 0x87] {
                let (byte0, body) = crate::net::read_packet(&mut sock).unwrap();
                let pid = u16::from_be_bytes([body[0], body[1]]);
                tx.send((byte0, body)).unwrap();
                sock.write_all(&suback(pid, &[answer])).unwrap();
            }
            let _ = crate::net::read_packet(&mut sock);
        });
        let mut c = connect_to(addr, &DaemonConfig::default_for_dev());
        let got = c
            .subscribe(&[
                SubscribeEntry {
                    filter: "chat/message".into(),
                    qos: 2,
                    id: 1,
                },
                SubscribeEntry {
                    filter: "s/+".into(),
                    qos: 0,
                    id: 200,
                },
            ])
            .unwrap();
        assert_eq!(
            got,
            [
                SubscribeOutcome::Granted(1),
                SubscribeOutcome::Refused(0x87)
            ]
        );
        let (b0, first) = rx.recv().unwrap();
        assert_eq!(b0, 0x82, "SUBSCRIBE with the reserved flags 0010");
        let mut want = first[..2].to_vec();
        // Properties: length 2, Subscription Identifier (0x0B) 1.
        want.extend([0x02, 0x0B, 0x01]);
        want.extend([0x00, 0x0C]);
        want.extend(b"chat/message");
        // QoS 2, No Local 1, Retain As Published 0, Retain Handling 0.
        want.push(0b0000_0110);
        assert_eq!(first, want);
        let (_, second) = rx.recv().unwrap();
        let mut want = second[..2].to_vec();
        // 200 is the variable byte integer C8 01.
        want.extend([0x03, 0x0B, 0xC8, 0x01]);
        want.extend([0x00, 0x03]);
        want.extend(b"s/+");
        want.push(0b0000_0100);
        assert_eq!(second, want);
        assert_ne!(first[..2], second[..2], "two packet identifiers");
        c.graceful_disconnect();
        h.join().unwrap();
    }

    /// Spec §4.3 with MQTT 5.0 §3.2.2.3.14/15 and MQTT 3.1.1 §3.8.3.1: a
    /// broker without Subscription Identifiers gets none, a wildcard filter
    /// is not sent to a broker without wildcard subscriptions; under MQTT
    /// 3.1.1 the options byte is the QoS alone.
    #[test]
    fn mqttbridge_4_3_what_the_broker_or_the_protocol_does_not_have_is_not_sent() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (addr, h) = broker(
            connack(&PropertySet {
                subscription_identifier_available: Some(0),
                wildcard_subscription_available: Some(0),
                ..PropertySet::default()
            }),
            move |mut sock| {
                let (_, body) = crate::net::read_packet(&mut sock).unwrap();
                let pid = u16::from_be_bytes([body[0], body[1]]);
                tx.send(body).unwrap();
                sock.write_all(&suback(pid, &[0])).unwrap();
                let _ = crate::net::read_packet(&mut sock);
            },
        );
        let mut c = connect_to(addr, &DaemonConfig::default_for_dev());
        let got = c
            .subscribe(&[
                SubscribeEntry {
                    filter: "a/+".into(),
                    qos: 1,
                    id: 1,
                },
                SubscribeEntry {
                    filter: "a/b".into(),
                    qos: 1,
                    id: 2,
                },
            ])
            .unwrap();
        assert_eq!(
            got,
            [SubscribeOutcome::NoWildcards, SubscribeOutcome::Granted(0)]
        );
        let body = rx.recv().unwrap();
        let mut want = body[..2].to_vec();
        want.extend([0x00, 0x00, 0x03]);
        want.extend(b"a/b");
        want.push(0b0000_0101);
        assert_eq!(body, want, "no property, the options QoS 1 and No Local");
        c.graceful_disconnect();
        h.join().unwrap();
        assert_eq!(subscription_options(ProtocolVersion::V311, 2), 0x02);
        assert_eq!(subscription_options(ProtocolVersion::V5, 2), 0x06);
    }
}
