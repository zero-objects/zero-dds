// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT-5 client against an external broker. Spec §4.1-§4.3.
//!
//! Synchronous implementation on `std::net::TcpStream`. Sends
//! CONNECT, waits for CONNACK; then SUBSCRIBE +
//! PUBLISH in/out in a loop.
//!
//! Not all MQTT-5 properties are served — the daemon needs
//! only the Spec §4 mandatory surface. Reconnect backoff is laid out
//! as a hook but is not active in this daemon variant (the
//! top-level server loop can restart the client).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::string::{String, ToString};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::vec::Vec;

use crate::codec::{PublishPacket, decode_publish, encode_publish};
use crate::control_packets::{
    AckBody, ConnectBody, DisconnectBody, SubscribeBody, Subscription as MqttSubscription,
    connect_flags, decode_ack_body, decode_connack_body, encode_ack_body, encode_connect_body,
    encode_disconnect_body, encode_subscribe_body, property_value,
};
use crate::packet::{ControlPacketType, FixedHeader};
use crate::vbi::{decode_vbi, encode_vbi};

use super::config::DaemonConfig;
#[cfg(feature = "daemon")]
use rustls::{ClientConfig, ClientConnection, StreamOwned};

/// Error during the client lifecycle.
#[derive(Debug)]
pub enum ClientError {
    /// TCP/IO error.
    Io(String),
    /// Wire-codec error.
    Codec(String),
    /// The read timeout elapsed before a whole packet arrived; what did
    /// arrive stays buffered for the next read.
    TimedOut,
    /// Broker answered with a CONNACK reason >= 0x80.
    ConnAck {
        /// Spec §3.2.2.2 reason-code.
        reason: u8,
    },
}

impl core::fmt::Display for ClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(m) => write!(f, "io: {m}"),
            Self::Codec(m) => write!(f, "codec: {m}"),
            Self::TimedOut => f.write_str("io: read timed out"),
            Self::ConnAck { reason } => write!(f, "connack reject: 0x{reason:02x}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// Inbound event from the broker — the caller handles PUBLISH frames
/// as DDS writes.
#[derive(Debug, Clone)]
pub enum InboundEvent {
    /// PUBLISH from another MQTT client.
    Publish {
        /// MQTT topic.
        topic: String,
        /// Payload.
        payload: Vec<u8>,
        /// QoS level.
        qos: u8,
    },
    /// Connection lost.
    Disconnected(String),
}

/// Connection-stream variant for the MQTT client layer. Plain TCP
/// vs. TLS-wrapped (Spec §7.1 — `mqtts://` path).
#[cfg(feature = "daemon")]
pub(crate) enum MqttStream {
    /// Plain TCP — `tls_enabled=false` or `mqtt://`.
    Plain(TcpStream),
    /// Client-side TLS stream with an owned connection + socket.
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
    /// MQTT in WebSocket frames over one of the two above (`ws://`,
    /// `wss://`).
    Ws(Box<super::ws_transport::WsStream>),
}

#[cfg(feature = "daemon")]
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
    fn shutdown_both(&mut self) {
        if let Self::Ws(s) = self {
            s.close();
        }
        let _ = self.socket().shutdown(std::net::Shutdown::Both);
    }
}

#[cfg(feature = "daemon")]
impl Read for MqttStream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(b),
            Self::Tls(s) => s.read(b),
            Self::Ws(s) => s.read(b),
        }
    }
}

#[cfg(feature = "daemon")]
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
/// Receive Maximum before it fails.
const SEND_SLOT_WAIT: Duration = Duration::from_secs(10);

/// MQTT-5 client. Manages a TCP or TLS-wrapped stream + wire loop.
pub struct MqttClient {
    #[cfg(feature = "daemon")]
    stream: MqttStream,
    #[cfg(not(feature = "daemon"))]
    stream: TcpStream,
    /// Next packet identifier to allocate.
    next_packet_id: u16,
    /// Bytes read from the stream that do not yet form a whole packet. A
    /// read timeout can fall inside a packet; the part already read waits
    /// here instead of being lost.
    rx: Vec<u8>,
    /// MQTT 5.0 §4.3.2/§4.3.3: our QoS 1/2 PUBLISH packets the broker has
    /// not completed, by packet identifier.
    outbound: std::collections::BTreeMap<u16, OutboundState>,
    /// §4.3.3: QoS 2 packets from the broker delivered and PUBREC'd, whose
    /// PUBREL has not arrived. A resend with the same identifier is not
    /// delivered a second time.
    inbound_qos2: std::collections::BTreeSet<u16>,
    /// §3.2.2.3.3 / §4.9: the broker's Receive Maximum from CONNACK — how
    /// many of our QoS 1/2 PUBLISH packets may be unacknowledged at once
    /// (65 535 when the broker names none).
    receive_maximum: u16,
    /// Events read while [`Self::publish`] waited for a free slot, handed
    /// out by [`Self::next_event`] first.
    held: std::collections::VecDeque<InboundEvent>,
}

/// Where one of our QoS 1/2 PUBLISH packets stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutboundState {
    /// QoS 1 waits for PUBACK, QoS 2 for PUBREC.
    Published,
    /// QoS 2: PUBREL sent, waits for PUBCOMP.
    Released,
}

impl MqttClient {
    /// Connects to the broker, sends CONNECT, blocks on CONNACK.
    /// If `tls_client_cfg = Some(...)` the TCP stream is wrapped with
    /// rustls after the connect (Spec §7.1).
    ///
    /// # Errors
    /// `Io` on TCP/read/write error. `ConnAck` if the broker
    /// rejects the connection with reason >= 0x80. `Codec` on
    /// a frame-decode error.
    #[cfg(feature = "daemon")]
    pub fn connect_secure(
        host: &str,
        port: u16,
        cfg: &DaemonConfig,
        tls_client_cfg: Option<Arc<ClientConfig>>,
    ) -> Result<Self, ClientError> {
        let stream = open_stream(
            &format!("{host}:{port}"),
            host,
            cfg,
            tls_client_cfg,
            ALPN_MQTT,
        )?;
        Self::handshake(stream, cfg)
    }

    /// Connects to the broker a parsed `mqtt.broker_url` names: TCP or
    /// TLS below, for `ws://` and `wss://` the WebSocket opening handshake
    /// on top (MQTT 5.0 §6), then CONNECT and CONNACK. `tls_client_cfg` is
    /// used when the URL's scheme is `mqtts://` or `wss://`.
    ///
    /// # Errors
    /// See [`Self::connect_secure`]; a failed WebSocket handshake is `Io`
    /// naming what the broker's answer lacked.
    #[cfg(feature = "daemon")]
    pub fn connect_url(
        url: &super::config::BrokerUrl,
        cfg: &DaemonConfig,
        tls_client_cfg: Option<Arc<ClientConfig>>,
    ) -> Result<Self, ClientError> {
        let tls = if url.tls {
            Some(tls_client_cfg.ok_or_else(|| {
                ClientError::Io(format!(
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
        let alpn = match url.transport {
            super::config::BrokerTransport::Tcp => ALPN_MQTT,
            super::config::BrokerTransport::WebSocket { .. } => ALPN_HTTP_1_1,
        };
        let mut stream = open_stream(&url.socket_addr(), &url.host, cfg, tls, alpn)?;
        if let super::config::BrokerTransport::WebSocket { path } = &url.transport {
            stream = MqttStream::Ws(Box::new(
                super::ws_transport::WsStream::handshake(stream, &url.host_header(), path)
                    .map_err(|e| ClientError::Io(format!("websocket: {e}")))?,
            ));
        }
        Self::handshake(stream, cfg)
    }

    /// CONNECT and CONNACK on an open stream.
    #[cfg(feature = "daemon")]
    fn handshake(stream: MqttStream, cfg: &DaemonConfig) -> Result<Self, ClientError> {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        let mut me = Self {
            stream,
            next_packet_id: 1,
            rx: Vec::new(),
            outbound: std::collections::BTreeMap::new(),
            inbound_qos2: std::collections::BTreeSet::new(),
            receive_maximum: u16::MAX,
            held: std::collections::VecDeque::new(),
        };
        me.send_connect(cfg)?;
        me.wait_connack()?;
        Ok(me)
    }

    /// Plain-TCP connect — backward-compat path.
    ///
    /// # Errors
    /// See [`Self::connect_secure`].
    pub fn connect(host: &str, port: u16, cfg: &DaemonConfig) -> Result<Self, ClientError> {
        #[cfg(feature = "daemon")]
        {
            Self::connect_secure(host, port, cfg, None)
        }
        #[cfg(not(feature = "daemon"))]
        {
            let addr = format!("{host}:{port}");
            let stream = TcpStream::connect_timeout(
                &addr
                    .parse()
                    .map_err(|e| ClientError::Io(format!("addr: {e}")))?,
                Duration::from_secs(10),
            )
            .map_err(|e| ClientError::Io(format!("connect: {e}")))?;
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .map_err(|e| ClientError::Io(format!("set timeout: {e}")))?;
            let mut me = Self {
                stream,
                next_packet_id: 1,
                rx: Vec::new(),
                outbound: std::collections::BTreeMap::new(),
                inbound_qos2: std::collections::BTreeSet::new(),
                receive_maximum: u16::MAX,
                held: std::collections::VecDeque::new(),
            };
            me.send_connect(cfg)?;
            me.wait_connack()?;
            Ok(me)
        }
    }

    fn send_connect(&mut self, cfg: &DaemonConfig) -> Result<(), ClientError> {
        // Spec §7.2 — outbound credentials are computed by `auth.mode`
        // (bearer/sasl/sasl_plain/none); legacy `username`/
        // `password` remains as a fallback.
        #[cfg(feature = "daemon")]
        let (user, pass) = {
            let (u, p) = super::security::outbound_credentials(cfg);
            // If auth.mode=none but legacy `username`/`password`
            // is set: use legacy.
            let u = u.or_else(|| cfg.username.clone());
            let p = p.or_else(|| cfg.password.as_ref().map(|s| s.as_bytes().to_vec()));
            (u, p)
        };
        #[cfg(not(feature = "daemon"))]
        let (user, pass) = (
            cfg.username.clone(),
            cfg.password.as_ref().map(|s| s.as_bytes().to_vec()),
        );

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
        let body = ConnectBody {
            protocol_name: "MQTT".to_string(),
            protocol_version: 5,
            connect_flags: flags,
            keep_alive: cfg.keep_alive_secs,
            properties: Vec::new(),
            client_id: cfg.client_id.clone(),
            will_properties: Vec::new(),
            will_topic: None,
            will_payload: Vec::new(),
            user_name: user,
            password: pass.unwrap_or_default(),
        };
        let body_bytes =
            encode_connect_body(&body).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        let frame = wrap_packet(ControlPacketType::Connect, 0, &body_bytes)
            .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        self.stream
            .write_all(&frame)
            .map_err(|e| ClientError::Io(format!("write connect: {e}")))?;
        Ok(())
    }

    fn wait_connack(&mut self) -> Result<(), ClientError> {
        let (header, body) = self.read_packet()?;
        if header.packet_type != ControlPacketType::ConnAck {
            return Err(ClientError::Codec(format!(
                "expected CONNACK got {:?}",
                header.packet_type
            )));
        }
        if body.len() < 2 {
            return Err(ClientError::Codec("connack too short".to_string()));
        }
        let reason = body[1];
        if reason >= 0x80 {
            return Err(ClientError::ConnAck { reason });
        }
        let connack =
            decode_connack_body(&body).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        if let Some(v) = property_value(&connack.properties, 0x21)
            .map_err(|e| ClientError::Codec(format!("{e:?}")))?
        {
            // §3.2.2.3.3: a value of 0 is a protocol error.
            let max = u16::from_be_bytes([v[0], v[1]]);
            if max == 0 {
                return Err(ClientError::Codec("CONNACK Receive Maximum 0".into()));
            }
            self.receive_maximum = max;
        }
        Ok(())
    }

    /// SUBSCRIBE to all the given topic filters with the
    /// desired QoS.
    ///
    /// # Errors
    /// IO/Codec.
    pub fn subscribe(&mut self, filters: &[(String, u8)]) -> Result<(), ClientError> {
        if filters.is_empty() {
            return Ok(());
        }
        let pid = self.next_pid();
        let body = SubscribeBody {
            packet_id: pid,
            properties: Vec::new(),
            subscriptions: filters
                .iter()
                .map(|(filter, qos)| MqttSubscription {
                    topic_filter: filter.clone(),
                    options: *qos & 0x03,
                })
                .collect(),
        };
        let body_bytes =
            encode_subscribe_body(&body).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        // SUBSCRIBE Reserved-Bits MUST be 0010 (Spec §3.8.1).
        let frame = wrap_packet(ControlPacketType::Subscribe, 0b0010, &body_bytes)
            .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        self.stream
            .write_all(&frame)
            .map_err(|e| ClientError::Io(format!("write sub: {e}")))?;
        Ok(())
    }

    /// PUBLISH a sample.
    ///
    /// # Errors
    /// IO/Codec.
    pub fn publish(
        &mut self,
        topic: &str,
        payload: &[u8],
        qos: u8,
        retain: bool,
    ) -> Result<(), ClientError> {
        if qos > 0 {
            self.wait_for_a_send_slot()?;
        }
        let pid = if qos > 0 { Some(self.next_pid()) } else { None };
        let pkt = PublishPacket {
            dup: false,
            qos,
            retain,
            topic: topic.to_string(),
            packet_id: pid,
            properties: Vec::new(),
            payload: payload.to_vec(),
        };
        let bytes = encode_publish(&pkt).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        self.stream
            .write_all(&bytes)
            .map_err(|e| ClientError::Io(format!("write pub: {e}")))?;
        if let Some(pid) = pid {
            self.outbound.insert(pid, OutboundState::Published);
        }
        Ok(())
    }

    /// §4.9: with Receive Maximum packets unacknowledged, the next QoS 1/2
    /// PUBLISH waits for an ack. Sending on made Mosquitto (Receive
    /// Maximum 20) acknowledge ten QoS 1 messages it then did not pass on
    /// (measured, `tests/mosquitto_qos.rs`). What arrives meanwhile is kept
    /// for [`Self::next_event`].
    fn wait_for_a_send_slot(&mut self) -> Result<(), ClientError> {
        let deadline = std::time::Instant::now() + SEND_SLOT_WAIT;
        while self.outbound.len() >= usize::from(self.receive_maximum) {
            if std::time::Instant::now() >= deadline {
                return Err(ClientError::Io(format!(
                    "no ack from the broker within {SEND_SLOT_WAIT:?} with {} packets in flight",
                    self.outbound.len()
                )));
            }
            if let Some(event) = self.read_event()? {
                self.held.push_back(event);
            }
        }
        Ok(())
    }

    /// Our QoS 1/2 PUBLISH packets the broker has not completed yet.
    #[must_use]
    pub fn pending_acks(&self) -> usize {
        self.outbound.len()
    }

    /// Sends PUBACK/PUBREC/PUBREL/PUBCOMP (MQTT 5.0 §3.4-§3.7) with
    /// reason Success. PUBREL carries the fixed flags 0b0010 (§3.6.1).
    fn send_ack(&mut self, kind: ControlPacketType, packet_id: u16) -> Result<(), ClientError> {
        let body = encode_ack_body(&AckBody {
            packet_id,
            reason_code: 0,
            properties: Vec::new(),
        })
        .map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        let flags = if kind == ControlPacketType::PubRel {
            0b0010
        } else {
            0
        };
        let frame =
            wrap_packet(kind, flags, &body).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        self.stream
            .write_all(&frame)
            .map_err(|e| ClientError::Io(format!("write ack: {e}")))
    }

    /// An acknowledgement from the broker for one of our packets.
    fn on_broker_ack(&mut self, kind: ControlPacketType, body: &[u8]) -> Result<(), ClientError> {
        let ack = decode_ack_body(body).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        if ack.reason_code >= 0x80 {
            // §3.4.2.1: the broker refused the message; the flow ends here.
            eprintln!(
                "[zerodds-mqtt-bridged] broker refused packet {} with reason 0x{:02x}",
                ack.packet_id, ack.reason_code
            );
            self.outbound.remove(&ack.packet_id);
            return Ok(());
        }
        match kind {
            ControlPacketType::PubAck | ControlPacketType::PubComp => {
                self.outbound.remove(&ack.packet_id);
            }
            ControlPacketType::PubRec => {
                self.outbound.insert(ack.packet_id, OutboundState::Released);
                self.send_ack(ControlPacketType::PubRel, ack.packet_id)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Blocking read for an inbound event. Returns `None` on a
    /// read timeout (the caller can build a polling loop + check a stop
    /// flag).
    ///
    /// # Errors
    /// IO/Codec — for EOF/closed stream we return
    /// `Disconnected`.
    /// Whether a call to [`Self::next_event`] has input to work on without
    /// reading the socket: a partial packet, or bytes buffered in TLS or
    /// WebSocket.
    #[cfg(feature = "daemon")]
    pub fn has_buffered_input(&mut self) -> bool {
        !self.held.is_empty() || !self.rx.is_empty() || self.stream.has_buffered_input()
    }

    /// A handle on the socket below, to wait for input without holding the
    /// client (`TcpStream::peek`); the read timeout is the socket's.
    ///
    /// # Errors
    /// The socket cannot be duplicated.
    #[cfg(feature = "daemon")]
    pub fn socket_clone(&self) -> std::io::Result<TcpStream> {
        self.stream.socket().try_clone()
    }

    pub fn next_event(&mut self) -> Result<Option<InboundEvent>, ClientError> {
        if let Some(event) = self.held.pop_front() {
            return Ok(Some(event));
        }
        self.read_event()
    }

    /// One packet from the broker, handled; the event it carries, if any.
    fn read_event(&mut self) -> Result<Option<InboundEvent>, ClientError> {
        let (header, body) = match self.read_packet_nonblocking() {
            Ok(p) => p,
            Err(ClientError::TimedOut) => return Ok(None),
            Err(ClientError::Io(m)) => {
                return Ok(Some(InboundEvent::Disconnected(m)));
            }
            Err(other) => return Err(other),
        };
        match header.packet_type {
            ControlPacketType::Publish => {
                // Rebuild full frame for decode_publish (which expects fixed header + body).
                let mut full = Vec::with_capacity(2 + body.len());
                let byte0 = (ControlPacketType::Publish.to_bits() << 4) | (header.flags & 0x0F);
                full.push(byte0);
                let len_u32 =
                    u32::try_from(body.len()).map_err(|_| ClientError::Codec("len".to_string()))?;
                full.extend_from_slice(
                    &encode_vbi(len_u32).ok_or_else(|| ClientError::Codec("vbi".to_string()))?,
                );
                full.extend_from_slice(&body);
                let (_, pkt) =
                    decode_publish(&full).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
                // §4.3.2 / §4.3.3: QoS 1 is answered with PUBACK, QoS 2
                // with PUBREC. Without them the broker's inflight window
                // filled and it stopped sending: 20 of 60 QoS 1 messages
                // arrived from Mosquitto, the rest never, with no error
                // (measured, `tests/mosquitto_qos.rs`).
                match (pkt.qos, pkt.packet_id) {
                    (1, Some(pid)) => self.send_ack(ControlPacketType::PubAck, pid)?,
                    (2, Some(pid)) => {
                        let first = self.inbound_qos2.insert(pid);
                        self.send_ack(ControlPacketType::PubRec, pid)?;
                        if !first {
                            // A resend of a message already delivered.
                            return Ok(None);
                        }
                    }
                    _ => {}
                }
                Ok(Some(InboundEvent::Publish {
                    topic: pkt.topic,
                    payload: pkt.payload,
                    qos: pkt.qos,
                }))
            }
            ControlPacketType::PubAck | ControlPacketType::PubRec | ControlPacketType::PubComp => {
                self.on_broker_ack(header.packet_type, &body)?;
                Ok(None)
            }
            ControlPacketType::PubRel => {
                // §4.3.3: the broker releases a QoS 2 message; PUBCOMP ends
                // the flow and frees the identifier.
                let ack =
                    decode_ack_body(&body).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
                self.inbound_qos2.remove(&ack.packet_id);
                self.send_ack(ControlPacketType::PubComp, ack.packet_id)?;
                Ok(None)
            }
            ControlPacketType::SubAck | ControlPacketType::PingResp => Ok(None),
            ControlPacketType::Disconnect => Ok(Some(InboundEvent::Disconnected(
                "broker disconnect".to_string(),
            ))),
            _ => Ok(None),
        }
    }

    /// Sends DISCONNECT with reason 0x00 and closes the stream.
    pub fn graceful_disconnect(mut self) {
        self.disconnect();
    }

    /// [`Self::graceful_disconnect`] on a client that stays owned (the
    /// daemon's sits behind a mutex).
    pub fn disconnect(&mut self) {
        let body = DisconnectBody {
            reason_code: 0,
            properties: Vec::new(),
        };
        if let Ok(body_bytes) = encode_disconnect_body(&body) {
            if let Ok(frame) = wrap_packet(ControlPacketType::Disconnect, 0, &body_bytes) {
                let _ = self.stream.write_all(&frame);
            }
        }
        #[cfg(feature = "daemon")]
        {
            self.stream.shutdown_both();
        }
        #[cfg(not(feature = "daemon"))]
        {
            let _ = self.stream.shutdown(std::net::Shutdown::Both);
        }
    }

    fn next_pid(&mut self) -> u16 {
        let p = self.next_packet_id;
        self.next_packet_id = self.next_packet_id.wrapping_add(1);
        if self.next_packet_id == 0 {
            self.next_packet_id = 1;
        }
        p
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
            Err(e) => return Err(ClientError::Codec(format!("{e:?}"))),
        };
        // TCP is a stream transport: the broker-announced Remaining Length
        // (up to 268_435_455 bytes per §2.1.4) cannot be checked against
        // "bytes remaining" the way an in-memory buffer decode can —
        // reject before buffering (mirrors
        // `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE` and
        // `crate::net::MAX_MQTT_PACKET_SIZE`), so a malicious/compromised
        // broker cannot force a ~256 MB allocation from a 5-byte header.
        if remaining > crate::net::MAX_MQTT_PACKET_SIZE {
            return Err(ClientError::Codec(format!(
                "MQTT Remaining Length {remaining} exceeds MAX_MQTT_PACKET_SIZE ({})",
                crate::net::MAX_MQTT_PACKET_SIZE
            )));
        }
        let header_len = 1 + vbi_len;
        let total = header_len + remaining as usize;
        if self.rx.len() < total {
            return Ok(None);
        }
        let byte0 = self.rx[0];
        let body = self.rx[header_len..total].to_vec();
        self.rx.drain(..total);
        let pt_bits = (byte0 >> 4) & 0x0F;
        let packet_type = ControlPacketType::from_bits(pt_bits)
            .ok_or_else(|| ClientError::Codec(format!("unknown packet type {pt_bits}")))?;
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

/// Backoff configuration for reconnect attempts.
/// Spec `zerodds-mqtt-bridge-1.0` §9.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackoffConfig {
    /// Initial backoff (e.g. 100 ms).
    pub initial_ms: u64,
    /// Max. backoff (e.g. 30 s).
    pub max_ms: u64,
    /// Multiplier per failed attempt (e.g. 2).
    pub multiplier: u64,
    /// Max. attempts (`u32::MAX` = infinite).
    pub max_attempts: u32,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_ms: 100,
            max_ms: 30_000,
            multiplier: 2,
            max_attempts: u32::MAX,
        }
    }
}

impl BackoffConfig {
    /// Computes the delay for attempt `attempt` (0-based).
    /// Capped at `max_ms`.
    #[must_use]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let mut d = self.initial_ms;
        for _ in 0..attempt {
            d = d.saturating_mul(self.multiplier);
            if d >= self.max_ms {
                d = self.max_ms;
                break;
            }
        }
        Duration::from_millis(d)
    }
}

/// The ALPN protocol of MQTT directly on TLS (Spec §7.1, RFC 7301; the
/// IANA registry lists `mqtt`).
#[cfg(feature = "daemon")]
const ALPN_MQTT: &[u8] = b"mqtt";

/// The ALPN protocol below `wss://`: the TLS channel first carries the
/// HTTP/1.1 upgrade of RFC 6455 §4.
#[cfg(feature = "daemon")]
const ALPN_HTTP_1_1: &[u8] = b"http/1.1";

/// TCP to `addr`, wrapped in TLS when `tls_client_cfg` is set, offering
/// `alpn`; the server name is `mqtt.tls.server_name`, else `host`. A broker
/// without ALPN ignores the offer (RFC 7301 §3.1).
#[cfg(feature = "daemon")]
fn open_stream(
    addr: &str,
    host: &str,
    cfg: &DaemonConfig,
    tls_client_cfg: Option<Arc<ClientConfig>>,
    alpn: &[u8],
) -> Result<MqttStream, ClientError> {
    let tcp = match addr.parse::<std::net::SocketAddr>() {
        Ok(sa) => TcpStream::connect_timeout(&sa, Duration::from_secs(10)),
        Err(_) => TcpStream::connect(addr),
    }
    .map_err(|e| ClientError::Io(format!("connect: {e}")))?;
    tcp.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| ClientError::Io(format!("set timeout: {e}")))?;
    tcp.set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| ClientError::Io(format!("set timeout: {e}")))?;
    Ok(match tls_client_cfg {
        Some(client_cfg) => {
            let mut client_cfg = (*client_cfg).clone();
            client_cfg.alpn_protocols = vec![alpn.to_vec()];
            let client_cfg = Arc::new(client_cfg);
            let server_name_str = if cfg.broker_tls_server_name.is_empty() {
                host.to_string()
            } else {
                cfg.broker_tls_server_name.clone()
            };
            let server_name = rustls::pki_types::ServerName::try_from(server_name_str.clone())
                .map_err(|e| ClientError::Io(format!("server name '{server_name_str}': {e}")))?;
            let conn = ClientConnection::new(client_cfg, server_name)
                .map_err(|e| ClientError::Io(format!("rustls client conn: {e}")))?;
            MqttStream::Tls(Box::new(StreamOwned::new(conn, tcp)))
        }
        None => MqttStream::Plain(tcp),
    })
}

/// Connects to the broker with exponential backoff.
/// Spec `zerodds-mqtt-bridge-1.0` §9.3.
///
/// # Errors
/// The last `ClientError` if `max_attempts` is exhausted.
pub fn connect_with_backoff(
    host: &str,
    port: u16,
    cfg: &DaemonConfig,
    backoff: BackoffConfig,
    stop: &AtomicBool,
) -> Result<MqttClient, ClientError> {
    let mut last_err = ClientError::Io("no attempts".to_string());
    for attempt in 0..backoff.max_attempts {
        if stop.load(Ordering::SeqCst) {
            return Err(ClientError::Io("stop signaled".to_string()));
        }
        match MqttClient::connect(host, port, cfg) {
            Ok(c) => return Ok(c),
            Err(e) => {
                last_err = e;
                let d = backoff.delay_for(attempt);
                std::thread::sleep(d);
            }
        }
    }
    Err(last_err)
}

/// Connects to the broker with backoff + an optional TLS wrap (Spec §7.1).
/// Spec `zerodds-mqtt-bridge-1.0` §9.3 + §7.1.
///
/// # Errors
/// The last `ClientError` if `max_attempts` is exhausted.
#[cfg(feature = "daemon")]
pub fn connect_secure_with_backoff(
    host: &str,
    port: u16,
    cfg: &DaemonConfig,
    tls_client_cfg: Option<Arc<ClientConfig>>,
    backoff: BackoffConfig,
    stop: &AtomicBool,
) -> Result<MqttClient, ClientError> {
    let mut last_err = ClientError::Io("no attempts".to_string());
    for attempt in 0..backoff.max_attempts {
        if stop.load(Ordering::SeqCst) {
            return Err(ClientError::Io("stop signaled".to_string()));
        }
        match MqttClient::connect_secure(host, port, cfg, tls_client_cfg.clone()) {
            Ok(c) => return Ok(c),
            Err(e) => {
                last_err = e;
                let d = backoff.delay_for(attempt);
                std::thread::sleep(d);
            }
        }
    }
    Err(last_err)
}

/// Aux: run loop for the client thread that fetches inbound
/// events. Terminates when `stop` is set or the stream
/// disconnects.
pub fn run_inbound_loop<F>(mut client: MqttClient, stop: Arc<AtomicBool>, mut on_event: F)
where
    F: FnMut(InboundEvent),
{
    while !stop.load(Ordering::SeqCst) {
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
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn backoff_config_default_increments_exponentially() {
        // Spec §9.3: initial=100ms, mult=2 -> sequence 100, 200, 400, 800, ...
        let b = BackoffConfig::default();
        assert_eq!(b.delay_for(0), Duration::from_millis(100));
        assert_eq!(b.delay_for(1), Duration::from_millis(200));
        assert_eq!(b.delay_for(2), Duration::from_millis(400));
        assert_eq!(b.delay_for(3), Duration::from_millis(800));
    }

    #[test]
    fn backoff_config_caps_at_max() {
        let b = BackoffConfig {
            initial_ms: 100,
            max_ms: 1_000,
            multiplier: 2,
            max_attempts: 100,
        };
        // 100, 200, 400, 800, 1000 (gecapped), 1000, ...
        assert_eq!(b.delay_for(4), Duration::from_millis(1_000));
        assert_eq!(b.delay_for(20), Duration::from_millis(1_000));
    }

    #[test]
    fn backoff_connect_aborts_when_stop_set() {
        let stop = AtomicBool::new(true);
        let cfg = DaemonConfig::default_for_dev();
        let b = BackoffConfig::default();
        // Port 1 is unbindable; should still abort immediately due to stop.
        let r = connect_with_backoff("127.0.0.1", 1, &cfg, b, &stop);
        assert!(r.is_err());
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

    /// A read timeout inside a packet: the first half stays buffered and the
    /// packet is read whole once the rest arrives. With `read_exact` the
    /// first half was dropped and the stream read from the middle of a
    /// packet.
    #[cfg(feature = "daemon")]
    #[test]
    fn a_read_timeout_inside_a_packet_keeps_the_part_already_read() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let frame = encode_publish(&PublishPacket {
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
        let mut client = MqttClient {
            stream: MqttStream::Plain(sock),
            next_packet_id: 1,
            rx: Vec::new(),
            outbound: std::collections::BTreeMap::new(),
            inbound_qos2: std::collections::BTreeSet::new(),
            receive_maximum: u16::MAX,
            held: std::collections::VecDeque::new(),
        };
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

    // Note: the PID-wrap logic is a pure fn on `MqttClient::next_pid`,
    // we cannot test it directly without a TcpStream without refactoring
    // the constructor. The wrap path is covered indirectly via the E2E test
    // (the broker accepts multiple SUBSCRIBE/PUBLISH without a crash).
}
