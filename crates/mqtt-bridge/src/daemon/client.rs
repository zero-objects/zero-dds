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
    ConnectBody, DisconnectBody, SubscribeBody, Subscription as MqttSubscription, connect_flags,
    encode_connect_body, encode_disconnect_body, encode_subscribe_body,
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
}

#[cfg(feature = "daemon")]
impl MqttStream {
    fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_read_timeout(dur),
            Self::Tls(s) => s.sock.set_read_timeout(dur),
        }
    }
    fn set_write_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.set_write_timeout(dur),
            Self::Tls(s) => s.sock.set_write_timeout(dur),
        }
    }
    fn shutdown_both(&mut self) {
        match self {
            Self::Plain(s) => {
                let _ = s.shutdown(std::net::Shutdown::Both);
            }
            Self::Tls(s) => {
                let _ = s.sock.shutdown(std::net::Shutdown::Both);
            }
        }
    }
}

#[cfg(feature = "daemon")]
impl Read for MqttStream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(b),
            Self::Tls(s) => s.read(b),
        }
    }
}

#[cfg(feature = "daemon")]
impl Write for MqttStream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(b),
            Self::Tls(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Tls(s) => s.flush(),
        }
    }
}

/// MQTT-5 client. Manages a TCP or TLS-wrapped stream + wire loop.
pub struct MqttClient {
    #[cfg(feature = "daemon")]
    stream: MqttStream,
    #[cfg(not(feature = "daemon"))]
    stream: TcpStream,
    /// Next packet identifier to allocate.
    next_packet_id: u16,
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
        let addr = format!("{host}:{port}");
        let tcp = match addr.parse::<std::net::SocketAddr>() {
            Ok(sa) => TcpStream::connect_timeout(&sa, Duration::from_secs(10)),
            Err(_) => TcpStream::connect(&addr),
        }
        .map_err(|e| ClientError::Io(format!("connect: {e}")))?;
        tcp.set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| ClientError::Io(format!("set timeout: {e}")))?;
        tcp.set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| ClientError::Io(format!("set timeout: {e}")))?;

        let stream = match tls_client_cfg {
            Some(client_cfg) => {
                let server_name_str = if cfg.broker_tls_server_name.is_empty() {
                    host.to_string()
                } else {
                    cfg.broker_tls_server_name.clone()
                };
                let server_name = rustls::pki_types::ServerName::try_from(server_name_str.clone())
                    .map_err(|e| {
                        ClientError::Io(format!("server name '{server_name_str}': {e}"))
                    })?;
                let conn = ClientConnection::new(client_cfg, server_name)
                    .map_err(|e| ClientError::Io(format!("rustls client conn: {e}")))?;
                MqttStream::Tls(Box::new(StreamOwned::new(conn, tcp)))
            }
            None => MqttStream::Plain(tcp),
        };
        // Nach dem Wrap ggf. nochmal Timeout setzen (idempotent).
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        let mut me = Self {
            stream,
            next_packet_id: 1,
        };
        me.send_connect(cfg)?;
        me.wait_connack()?;
        Ok(me)
    }

    /// Plain-TCP connect — backward-compat path.
    ///
    /// # Errors
    /// Siehe [`Self::connect_secure`].
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

    /// PUBLISH ein Sample.
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
        Ok(())
    }

    /// Blocking read for an inbound event. Returns `None` on a
    /// read timeout (the caller can build a polling loop + check a stop
    /// flag).
    ///
    /// # Errors
    /// IO/Codec — for EOF/closed stream we return
    /// `Disconnected`.
    pub fn next_event(&mut self) -> Result<Option<InboundEvent>, ClientError> {
        let (header, body) = match self.read_packet_nonblocking() {
            Ok(p) => p,
            Err(ClientError::Io(m)) if m.contains("WouldBlock") || m.contains("timed out") => {
                return Ok(None);
            }
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
                Ok(Some(InboundEvent::Publish {
                    topic: pkt.topic,
                    payload: pkt.payload,
                    qos: pkt.qos,
                }))
            }
            ControlPacketType::SubAck
            | ControlPacketType::PubAck
            | ControlPacketType::PubRec
            | ControlPacketType::PubRel
            | ControlPacketType::PubComp
            | ControlPacketType::PingResp => {
                // Ignore acks — that's enough for the L1 obligations.
                Ok(None)
            }
            ControlPacketType::Disconnect => Ok(Some(InboundEvent::Disconnected(
                "broker disconnect".to_string(),
            ))),
            _ => Ok(None),
        }
    }

    /// Sends DISCONNECT with reason 0x00 and closes the stream.
    pub fn graceful_disconnect(mut self) {
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
        let mut hdr = [0u8; 1];
        match self.stream.read_exact(&mut hdr) {
            Ok(()) => {}
            Err(e) => return Err(ClientError::Io(format!("read header: {e:?}"))),
        }
        // VBI lesen (1-4 bytes).
        let mut vbi_buf: Vec<u8> = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            self.stream
                .read_exact(&mut byte)
                .map_err(|e| ClientError::Io(format!("read vbi: {e:?}")))?;
            vbi_buf.push(byte[0]);
            if byte[0] & 0x80 == 0 {
                break;
            }
            if vbi_buf.len() >= 4 {
                return Err(ClientError::Codec("vbi too long".to_string()));
            }
        }
        let (remaining, _) =
            decode_vbi(&vbi_buf).map_err(|e| ClientError::Codec(format!("{e:?}")))?;
        // TCP is a stream transport: the broker-announced Remaining Length
        // (up to 268_435_455 bytes per §2.1.4) cannot be checked against
        // "bytes remaining" the way an in-memory buffer decode can —
        // reject before allocating (mirrors
        // `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE` and
        // `crate::net::MAX_MQTT_PACKET_SIZE`), so a malicious/compromised
        // broker cannot force a ~256 MB allocation from a 5-byte header.
        if remaining > crate::net::MAX_MQTT_PACKET_SIZE {
            return Err(ClientError::Codec(format!(
                "MQTT Remaining Length {remaining} exceeds MAX_MQTT_PACKET_SIZE ({})",
                crate::net::MAX_MQTT_PACKET_SIZE
            )));
        }
        let mut body = vec![0u8; remaining as usize];
        if !body.is_empty() {
            self.stream
                .read_exact(&mut body)
                .map_err(|e| ClientError::Io(format!("read body: {e:?}")))?;
        }
        let byte0 = hdr[0];
        let pt_bits = (byte0 >> 4) & 0x0F;
        let packet_type = ControlPacketType::from_bits(pt_bits)
            .ok_or_else(|| ClientError::Codec(format!("unknown packet type {pt_bits}")))?;
        let flags = byte0 & 0x0F;
        Ok((
            FixedHeader {
                packet_type,
                flags,
                remaining_length: remaining,
            },
            body,
        ))
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

    // Note: the PID-wrap logic is a pure fn on `MqttClient::next_pid`,
    // we cannot test it directly without a TcpStream without refactoring
    // the constructor. The wrap path is covered indirectly via the E2E test
    // (the broker accepts multiple SUBSCRIBE/PUBLISH without a crash).
}
