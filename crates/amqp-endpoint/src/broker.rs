// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A client connection to an AMQP 1.0 broker that stays open: the
//! connection the bridge daemon (`zerodds-amqp-bridge-daemon-1.0`) holds for
//! its whole run.
//!
//! [`connect`] opens the transport (TCP, or TLS with ALPN `amqp`), runs the
//! SASL layer (PLAIN, ANONYMOUS, EXTERNAL, SCRAM-SHA-256, XOAUTH2), opens the
//! connection, and returns the two halves. Sessions are begun on channels
//! the caller picks, so a broker that ends one session (RabbitMQ does for
//! a link to a queue that does not exist) leaves the others alone:
//!
//! * [`BrokerWriter`] sends: session begin and end, link attach, flow,
//!   transfer (split into frames
//!   of the negotiated `max-frame-size`), disposition, detach, the idle
//!   heartbeat, end and close;
//! * [`BrokerReader`] decodes what the broker sends into [`Inbound`]
//!   events, with multi-frame transfers put back together.
//!
//! The halves share no lock on the read path, so a reader thread blocks on
//! the socket while the writer keeps sending. The state that ties frames to
//! links (credit, delivery counts, unsettled deliveries) belongs to the
//! caller, which sees every frame in order.
//!
//! Spec sources: OASIS AMQP 1.0 Part 2 (transport: §2.3 frames, §2.4
//! connections, §2.5 sessions, §2.6 links, §2.7 performatives), Part 3
//! (messaging: §3.2 sections, §3.4 delivery states, §3.5 source/target),
//! Part 5 (SASL). SCRAM: RFC 5802 / RFC 7677. XOAUTH2: the Google/Microsoft
//! `user=…\x01auth=Bearer …\x01\x01` initial response.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::string::{String, ToString};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::vec::Vec;

use rustls::{ClientConfig, ClientConnection};
use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::{
    FrameHeader, FrameType, decode_frame_header, encode_frame_header,
};
use zerodds_amqp_bridge::performatives::{decode_performative, descriptor, encode_performative};

/// AMQP protocol header (Part 2 §2.2).
const AMQP_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x00, 0x01, 0x00, 0x00];
/// SASL protocol header (Part 5 §5.1).
const SASL_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x03, 0x01, 0x00, 0x00];
/// The ALPN protocol id of AMQP over TLS (daemon spec §7.1).
pub const ALPN_AMQP: &[u8] = b"amqp";
/// Part 2 §2.7.1: `max-frame-size` may not be negotiated below this.
const MIN_MAX_FRAME_SIZE: u32 = 512;
/// The largest frame accepted from a broker, whatever it announces.
const MAX_ACCEPTED_FRAME: u32 = 16 * 1024 * 1024;
/// The largest message reassembled from multi-frame transfers.
const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// Descriptors of the delivery states and nodes (Part 3 §3.4, §3.5).
mod desc {
    pub const SOURCE: u64 = 0x28;
    pub const TARGET: u64 = 0x29;
    pub const ERROR: u64 = 0x1d;
    pub const RECEIVED: u64 = 0x23;
    pub const ACCEPTED: u64 = 0x24;
    pub const REJECTED: u64 = 0x25;
    pub const RELEASED: u64 = 0x26;
    pub const MODIFIED: u64 = 0x27;
}

fn protocol(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn codec<E: core::fmt::Debug>(e: E) -> io::Error {
    protocol(format!("AMQP codec: {e:?}"))
}

// ============================================================================
// Transport: TCP, or TLS split into a read and a write half.
// ============================================================================

/// The socket below the connection.
enum Wire {
    Plain(TcpStream),
    Tls {
        sock: TcpStream,
        conn: Arc<Mutex<ClientConnection>>,
        /// Ciphertext read from the socket that rustls has not taken yet;
        /// only the reading half fills it.
        pending: Vec<u8>,
    },
}

impl Wire {
    fn try_clone(&self) -> io::Result<Self> {
        Ok(match self {
            Self::Plain(s) => Self::Plain(s.try_clone()?),
            Self::Tls { sock, conn, .. } => Self::Tls {
                sock: sock.try_clone()?,
                conn: Arc::clone(conn),
                pending: Vec::new(),
            },
        })
    }

    fn socket(&self) -> &TcpStream {
        match self {
            Self::Plain(s) | Self::Tls { sock: s, .. } => s,
        }
    }
}

fn lock(conn: &Mutex<ClientConnection>) -> io::Result<std::sync::MutexGuard<'_, ClientConnection>> {
    conn.lock()
        .map_err(|_| io::Error::other("TLS state poisoned by a panicked thread"))
}

impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(buf),
            Self::Tls {
                sock,
                conn,
                pending,
            } => {
                let mut raw = [0u8; 16 * 1024];
                loop {
                    {
                        let mut c = lock(conn)?;
                        match c.reader().read(buf) {
                            Ok(n) => return Ok(n),
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                            Err(e) => return Err(e),
                        }
                        // No plaintext: feed what is buffered. rustls takes
                        // at most its own buffer per call; the rest waits.
                        if !pending.is_empty() {
                            let mut rest: &[u8] = pending;
                            let taken = c.read_tls(&mut rest)?;
                            pending.drain(..taken);
                            c.process_new_packets()
                                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                            // TLS 1.3 may answer on its own (key update).
                            while c.wants_write() {
                                c.write_tls(&mut &*sock)?;
                            }
                            if taken > 0 {
                                continue;
                            }
                        }
                    }
                    // Block on the socket without the lock, so the writer
                    // half is free to send meanwhile.
                    let n = sock.read(&mut raw)?;
                    if n == 0 {
                        return Ok(0);
                    }
                    pending.extend_from_slice(&raw[..n]);
                }
            }
        }
    }
}

impl Write for Wire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(buf),
            Self::Tls { sock, conn, .. } => {
                let mut c = lock(conn)?;
                c.writer().write_all(buf)?;
                while c.wants_write() {
                    c.write_tls(&mut &*sock)?;
                }
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Tls { sock, .. } => (&*sock).flush(),
        }
    }
}

/// One frame read from the broker: its type, channel and body (the bytes
/// after the header and any extended header).
struct Frame {
    frame_type: FrameType,
    channel: u16,
    body: Vec<u8>,
}

/// Reads whole frames; bytes of a frame cut by a read timeout are kept.
struct FrameReader {
    wire: Wire,
    buf: Vec<u8>,
}

impl FrameReader {
    fn read_exact_header(&mut self, n: usize) -> io::Result<Vec<u8>> {
        let mut chunk = [0u8; 16 * 1024];
        while self.buf.len() < n {
            let got = self.wire.read(&mut chunk)?;
            if got == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the broker closed the connection",
                ));
            }
            self.buf.extend_from_slice(&chunk[..got]);
        }
        Ok(self.buf.drain(..n).collect())
    }

    /// The next frame. An empty AMQP frame is the peer's heartbeat.
    fn next(&mut self) -> io::Result<Frame> {
        let mut chunk = [0u8; 16 * 1024];
        loop {
            if self.buf.len() >= 8 {
                let mut h = [0u8; 8];
                h.copy_from_slice(&self.buf[..8]);
                let header = decode_frame_header(&h).map_err(codec)?;
                let doff = usize::from(header.doff) * 4;
                if header.size > MAX_ACCEPTED_FRAME {
                    return Err(protocol(format!(
                        "frame of {} bytes exceeds {MAX_ACCEPTED_FRAME}",
                        header.size
                    )));
                }
                if doff < 8 || header.size < doff as u32 {
                    return Err(protocol(format!(
                        "frame size {} with data offset {doff}",
                        header.size
                    )));
                }
                let size = header.size as usize;
                if self.buf.len() >= size {
                    let body = self.buf[doff..size].to_vec();
                    self.buf.drain(..size);
                    return Ok(Frame {
                        frame_type: header.frame_type,
                        channel: header.channel,
                        body,
                    });
                }
            }
            let got = self.wire.read(&mut chunk)?;
            if got == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the broker closed the connection",
                ));
            }
            self.buf.extend_from_slice(&chunk[..got]);
        }
    }
}

fn write_frame(
    wire: &mut Wire,
    frame_type: FrameType,
    channel: u16,
    body: &[u8],
) -> io::Result<()> {
    let size = u32::try_from(8 + body.len()).map_err(|_| protocol("frame larger than 4 GiB"))?;
    let header = encode_frame_header(FrameHeader {
        size,
        doff: 2,
        frame_type,
        channel,
    });
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&header);
    frame.extend_from_slice(body);
    wire.write_all(&frame)?;
    wire.flush()
}

// ============================================================================
// Value helpers.
// ============================================================================

fn fields(v: &AmqpExtValue) -> &[AmqpExtValue] {
    match v {
        AmqpExtValue::List(items) => items,
        _ => &[],
    }
}

fn field(v: &AmqpExtValue, i: usize) -> Option<&AmqpExtValue> {
    fields(v)
        .get(i)
        .filter(|x| !matches!(x, AmqpExtValue::Null))
}

fn as_u32(v: Option<&AmqpExtValue>) -> Option<u32> {
    match v? {
        AmqpExtValue::Uint(x) => Some(*x),
        AmqpExtValue::Ushort(x) => Some(u32::from(*x)),
        AmqpExtValue::Ubyte(x) => Some(u32::from(*x)),
        AmqpExtValue::Ulong(x) => u32::try_from(*x).ok(),
        _ => None,
    }
}

fn as_bool(v: Option<&AmqpExtValue>) -> Option<bool> {
    match v? {
        AmqpExtValue::Boolean(b) => Some(*b),
        _ => None,
    }
}

fn as_text(v: Option<&AmqpExtValue>) -> Option<String> {
    match v? {
        AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) => Some(s.clone()),
        _ => None,
    }
}

fn symbols(v: Option<&AmqpExtValue>) -> Vec<String> {
    match v {
        Some(AmqpExtValue::Symbol(s)) => vec![s.clone()],
        Some(AmqpExtValue::Array(items)) | Some(AmqpExtValue::List(items)) => {
            items.iter().filter_map(|x| as_text(Some(x))).collect()
        }
        _ => Vec::new(),
    }
}

/// A `symbol` multiple (Part 1 §1.4): an array of symbols, or null for
/// none (an empty array has no element constructor to encode).
fn symbol_array(names: &[&str]) -> AmqpExtValue {
    if names.is_empty() {
        return AmqpExtValue::Null;
    }
    AmqpExtValue::Array(
        names
            .iter()
            .map(|n| AmqpExtValue::Symbol((*n).to_string()))
            .collect(),
    )
}

fn described(descriptor: u64, list: Vec<AmqpExtValue>) -> AmqpExtValue {
    AmqpExtValue::Described {
        descriptor,
        value: Box::new(AmqpExtValue::List(list)),
    }
}

/// An `amqp:error` (Part 2 §2.8.14): condition and description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmqpError {
    /// The condition symbol, e.g. `amqp:not-found`.
    pub condition: String,
    /// The broker's description, if any.
    pub description: Option<String>,
}

impl core::fmt::Display for AmqpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.description {
            Some(d) => write!(f, "{}: {d}", self.condition),
            None => f.write_str(&self.condition),
        }
    }
}

fn decode_error(v: Option<&AmqpExtValue>) -> Option<AmqpError> {
    match v? {
        AmqpExtValue::Described { descriptor, value } if *descriptor == desc::ERROR => {
            Some(AmqpError {
                condition: as_text(field(value, 0)).unwrap_or_default(),
                description: as_text(field(value, 1)),
            })
        }
        _ => None,
    }
}

fn encode_error(e: &AmqpError) -> AmqpExtValue {
    described(
        desc::ERROR,
        vec![
            AmqpExtValue::Symbol(e.condition.clone()),
            e.description
                .clone()
                .map_or(AmqpExtValue::Null, AmqpExtValue::Str),
        ],
    )
}

// ============================================================================
// Public types.
// ============================================================================

/// How the client authenticates in the SASL layer (Part 5, daemon spec
/// §7.2). The broker must offer the mechanism; there is no fallback to a
/// weaker one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaslCredentials {
    /// `ANONYMOUS` (RFC 4505).
    Anonymous,
    /// `PLAIN` (RFC 4616).
    Plain {
        /// Authentication identity.
        username: String,
        /// Password.
        password: String,
    },
    /// `SCRAM-SHA-256` (RFC 7677); the server signature is verified.
    ScramSha256 {
        /// Authentication identity.
        username: String,
        /// Password.
        password: String,
    },
    /// `EXTERNAL` (RFC 4422 App. A): the identity of the TLS client
    /// certificate.
    External,
    /// `XOAUTH2`: an OAuth 2.0 bearer token.
    XOAuth2 {
        /// The user the token is for.
        username: String,
        /// The bearer token.
        token: String,
    },
}

impl SaslCredentials {
    fn mechanism(&self) -> &'static str {
        match self {
            Self::Anonymous => "ANONYMOUS",
            Self::Plain { .. } => "PLAIN",
            Self::ScramSha256 { .. } => "SCRAM-SHA-256",
            Self::External => "EXTERNAL",
            Self::XOAuth2 { .. } => "XOAUTH2",
        }
    }

    /// The authenticated subject the daemon's ACL uses (spec §7.3).
    #[must_use]
    pub fn subject(&self) -> Option<&str> {
        match self {
            Self::Plain { username, .. }
            | Self::ScramSha256 { username, .. }
            | Self::XOAuth2 { username, .. } => Some(username),
            Self::Anonymous | Self::External => None,
        }
    }
}

/// What [`connect`] needs.
#[derive(Clone)]
pub struct ConnectParams {
    /// `host:port` to connect to.
    pub addr: String,
    /// The host name for TLS (SNI and certificate check).
    pub host: String,
    /// TLS client configuration; `None` for plain TCP. ALPN `amqp` is set
    /// on a copy.
    pub tls: Option<Arc<ClientConfig>>,
    /// SASL mechanism and credentials.
    pub sasl: SaslCredentials,
    /// `open.container-id`.
    pub container_id: String,
    /// `open.hostname`; the virtual host some brokers route by.
    pub hostname: Option<String>,
    /// `open.max-frame-size` the client accepts.
    pub max_frame_size: u32,
    /// `open.channel-max`.
    pub channel_max: u16,
    /// `open.idle-time-out` in milliseconds; `0` announces none.
    pub idle_time_out_ms: u32,
    /// `open.properties` (daemon spec §4.1).
    pub properties: Vec<(String, String)>,
    /// `open.offered-capabilities` and `desired-capabilities`.
    pub capabilities: Vec<String>,
    /// TCP connect and handshake timeout.
    pub timeout: Duration,
}

/// The phase of [`connect`] that failed; the daemon's exit code follows
/// it (spec §2: 2 TCP, 4 TLS, 5 SASL, 6 open refused).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectStage {
    /// Name resolution and TCP connect.
    Tcp,
    /// The TLS handshake.
    Tls,
    /// The SASL layer.
    Sasl,
    /// The AMQP protocol header and `open`.
    Open,
}

/// A failed [`connect`]: the phase and the error.
#[derive(Debug)]
pub struct ConnectError {
    /// Where it failed.
    pub stage: ConnectStage,
    /// Why.
    pub error: io::Error,
}

impl core::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let stage = match self.stage {
            ConnectStage::Tcp => "tcp",
            ConnectStage::Tls => "tls",
            ConnectStage::Sasl => "sasl",
            ConnectStage::Open => "open",
        };
        write!(f, "{stage}: {}", self.error)
    }
}

impl std::error::Error for ConnectError {}

/// What the broker's `open` said.
#[derive(Debug, Clone)]
pub struct RemoteOpen {
    /// The broker's container id.
    pub container_id: String,
    /// The frame size both sides use: the smaller of the two offers.
    pub max_frame_size: u32,
    /// The broker's idle time-out: the client sends a frame at least this
    /// often (half of it, per Part 2 §2.4.5).
    pub idle_time_out_ms: Option<u32>,
    /// The broker's offered capabilities.
    pub offered_capabilities: Vec<String>,
    /// The highest channel both sides accept.
    pub channel_max: u16,
}

/// The outcome of a delivery (Part 3 §3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// `accepted`.
    Accepted,
    /// `rejected`, with the broker's error.
    Rejected(Option<AmqpError>),
    /// `released`: not processed, may be sent again.
    Released,
    /// `modified`.
    Modified {
        /// The delivery failed and counts towards redelivery.
        delivery_failed: bool,
        /// Must not be delivered to this link again.
        undeliverable_here: bool,
    },
    /// A non-terminal `received` state or none.
    None,
}

fn decode_outcome(v: Option<&AmqpExtValue>) -> Outcome {
    match v {
        Some(AmqpExtValue::Described { descriptor, value }) => match *descriptor {
            desc::ACCEPTED => Outcome::Accepted,
            desc::REJECTED => Outcome::Rejected(decode_error(field(value, 0))),
            desc::RELEASED => Outcome::Released,
            desc::MODIFIED => Outcome::Modified {
                delivery_failed: as_bool(field(value, 0)).unwrap_or(false),
                undeliverable_here: as_bool(field(value, 1)).unwrap_or(false),
            },
            desc::RECEIVED => Outcome::None,
            _ => Outcome::None,
        },
        _ => Outcome::None,
    }
}

fn encode_outcome(o: &Outcome) -> AmqpExtValue {
    match o {
        Outcome::Accepted => described(desc::ACCEPTED, vec![]),
        Outcome::Rejected(e) => described(
            desc::REJECTED,
            vec![e.as_ref().map_or(AmqpExtValue::Null, encode_error)],
        ),
        Outcome::Released => described(desc::RELEASED, vec![]),
        Outcome::Modified {
            delivery_failed,
            undeliverable_here,
        } => described(
            desc::MODIFIED,
            vec![
                AmqpExtValue::Boolean(*delivery_failed),
                AmqpExtValue::Boolean(*undeliverable_here),
            ],
        ),
        Outcome::None => AmqpExtValue::Null,
    }
}

/// A link to attach (Part 2 §2.7.3).
#[derive(Debug, Clone)]
pub struct LinkSpec {
    /// Link name, unique within the connection.
    pub name: String,
    /// The client's handle for the link.
    pub handle: u32,
    /// `true`: the client sends on this link (`role = sender`).
    pub sender: bool,
    /// The node address: `target` of a sender, `source` of a receiver.
    pub address: String,
    /// `snd-settle-mode`: 0 unsettled, 1 settled, 2 mixed.
    pub snd_settle_mode: u8,
    /// `rcv-settle-mode`: 0 first, 1 second.
    pub rcv_settle_mode: u8,
    /// Terminus durability (Part 3 §3.5.5): 0 none, 1 configuration,
    /// 2 unsettled-state.
    pub durable: u32,
    /// `attach.properties`.
    pub properties: Vec<(String, String)>,
    /// `attach.desired-capabilities`.
    pub desired_capabilities: Vec<String>,
}

/// A frame from the broker, decoded. `channel` is always the client's
/// channel of the session, whatever channel the broker sent on.
#[derive(Debug, Clone)]
pub enum Inbound {
    /// The broker began its end of a session the client began.
    Begin {
        /// The session.
        channel: u16,
        /// The broker's `next-outgoing-id`.
        next_outgoing_id: u32,
        /// The broker's `incoming-window`.
        incoming_window: u32,
    },
    /// The broker attached its end of a link (named like ours).
    Attach {
        /// The session.
        channel: u16,
        /// Link name.
        name: String,
        /// The broker's handle; its later frames for the link carry it.
        remote_handle: u32,
        /// The broker's node: `false` when it refused ours (a detach with
        /// the reason follows).
        has_node: bool,
        /// `initial-delivery-count` of a broker sending on the link.
        initial_delivery_count: Option<u32>,
    },
    /// Flow state (Part 2 §2.7.4); link fields when `remote_handle` is set.
    Flow {
        /// The session.
        channel: u16,
        /// Session: the broker's `next-incoming-id`.
        next_incoming_id: Option<u32>,
        /// Session: the broker's `incoming-window`.
        incoming_window: u32,
        /// The broker's handle of the link, for link flow state.
        remote_handle: Option<u32>,
        /// Link: `delivery-count`.
        delivery_count: Option<u32>,
        /// Link: `link-credit`.
        link_credit: Option<u32>,
        /// Link: `echo`.
        echo: bool,
    },
    /// A whole message, reassembled from one or more transfer frames.
    Transfer {
        /// The session.
        channel: u16,
        /// The broker's handle of the link.
        remote_handle: u32,
        /// The broker's delivery id.
        delivery_id: u32,
        /// The broker settled it on sending.
        settled: bool,
        /// The encoded message sections.
        message: Vec<u8>,
    },
    /// Settlement of deliveries in `first..=last`.
    Disposition {
        /// The session.
        channel: u16,
        /// `true`: the broker acts as receiver, the ids are the client's.
        role_receiver: bool,
        /// First delivery id.
        first: u32,
        /// Last delivery id (inclusive).
        last: u32,
        /// Settled by the broker.
        settled: bool,
        /// The delivery state.
        outcome: Outcome,
    },
    /// The broker detached a link.
    Detach {
        /// The session.
        channel: u16,
        /// The broker's handle.
        remote_handle: u32,
        /// `closed` flag.
        closed: bool,
        /// The reason, if the broker gave one.
        error: Option<AmqpError>,
    },
    /// The broker ended a session.
    End {
        /// The session.
        channel: u16,
        /// The reason, if the broker gave one.
        error: Option<AmqpError>,
    },
    /// The broker closed the connection.
    Close(Option<AmqpError>),
    /// An empty frame: the broker is alive.
    Heartbeat,
}

// ============================================================================
// Connect: transport, SASL, open, begin.
// ============================================================================

/// Opens a connection; sessions follow with [`BrokerWriter::begin`].
///
/// # Errors
/// I/O and TLS errors; the SASL outcome when authentication fails, naming
/// the mechanism; a protocol error when the broker's answers do not follow
/// Part 2.
pub fn connect(
    params: &ConnectParams,
) -> Result<(BrokerWriter, BrokerReader, RemoteOpen), ConnectError> {
    let at = |stage: ConnectStage| move |error: io::Error| ConnectError { stage, error };
    let addrs: Vec<_> = params
        .addr
        .to_socket_addrs()
        .map_err(at(ConnectStage::Tcp))?
        .collect();
    let mut last = io::Error::new(io::ErrorKind::AddrNotAvailable, "no address to connect to");
    let mut tcp = None;
    for a in addrs {
        match TcpStream::connect_timeout(&a, params.timeout) {
            Ok(s) => {
                tcp = Some(s);
                break;
            }
            Err(e) => last = e,
        }
    }
    let tcp = tcp.ok_or(last).map_err(at(ConnectStage::Tcp))?;
    let timeouts = || -> io::Result<()> {
        tcp.set_nodelay(true)?;
        tcp.set_read_timeout(Some(params.timeout))?;
        tcp.set_write_timeout(Some(params.timeout))
    };
    timeouts().map_err(at(ConnectStage::Tcp))?;
    let wire = match &params.tls {
        None => Wire::Plain(tcp),
        Some(cfg) => {
            let mut cfg = (**cfg).clone();
            cfg.alpn_protocols = vec![ALPN_AMQP.to_vec()];
            let name = rustls::pki_types::ServerName::try_from(params.host.clone())
                .map_err(|e| protocol(format!("TLS server name {}: {e}", params.host)))
                .map_err(at(ConnectStage::Tls))?;
            let mut conn = ClientConnection::new(Arc::new(cfg), name)
                .map_err(|e| io::Error::other(format!("TLS: {e}")))
                .map_err(at(ConnectStage::Tls))?;
            let mut sock = tcp;
            while conn.is_handshaking() {
                conn.complete_io(&mut sock).map_err(at(ConnectStage::Tls))?;
            }
            Wire::Tls {
                sock,
                conn: Arc::new(Mutex::new(conn)),
                pending: Vec::new(),
            }
        }
    };
    let mut reader = FrameReader {
        wire: wire.try_clone().map_err(at(ConnectStage::Tcp))?,
        buf: Vec::new(),
    };
    let mut wire = wire;

    sasl(&mut wire, &mut reader, &params.sasl).map_err(at(ConnectStage::Sasl))?;
    let (wire, reader, remote_open) =
        open_connection(params, wire, reader).map_err(at(ConnectStage::Open))?;
    let shared = Arc::new(Mutex::new(Shared::default()));
    Ok((
        BrokerWriter {
            wire,
            max_frame_size: remote_open.max_frame_size,
            sessions: BTreeMap::new(),
            shared: Arc::clone(&shared),
        },
        BrokerReader {
            frames: reader,
            partial: None,
            shared,
        },
        remote_open,
    ))
}

/// The AMQP protocol header and `open` (Part 2 §2.2, §2.7.1).
fn open_connection(
    params: &ConnectParams,
    mut wire: Wire,
    mut reader: FrameReader,
) -> io::Result<(Wire, FrameReader, RemoteOpen)> {
    wire.write_all(&AMQP_HEADER)?;
    let peer = reader.read_exact_header(8)?;
    if peer[..] != AMQP_HEADER[..] {
        return Err(protocol(format!(
            "the broker answered protocol header {peer:02x?} to AMQP 1.0.0"
        )));
    }
    let open = open_body(params)?;
    write_frame(&mut wire, FrameType::Amqp, 0, &open)?;
    let remote_open = loop {
        let f = reader.next()?;
        if f.body.is_empty() {
            continue;
        }
        let (d, v, _) = decode_performative(&f.body).map_err(codec)?;
        match d {
            descriptor::OPEN => {
                let theirs = as_u32(field(&v, 2)).unwrap_or(u32::MAX);
                break RemoteOpen {
                    container_id: as_text(field(&v, 0)).unwrap_or_default(),
                    max_frame_size: theirs.min(params.max_frame_size).max(MIN_MAX_FRAME_SIZE),
                    idle_time_out_ms: as_u32(field(&v, 4)).filter(|t| *t > 0),
                    offered_capabilities: symbols(field(&v, 7)),
                    channel_max: match field(&v, 3) {
                        Some(AmqpExtValue::Ushort(c)) => (*c).min(params.channel_max),
                        _ => params.channel_max,
                    },
                };
            }
            descriptor::CLOSE => {
                return Err(protocol(format!(
                    "the broker closed the connection at open: {}",
                    decode_error(field(&v, 0))
                        .map_or_else(|| "no reason".into(), |e| e.to_string())
                )));
            }
            other => {
                return Err(protocol(format!(
                    "expected open, got performative {other:#x}"
                )));
            }
        }
    };
    // From here the reader thread blocks until the broker sends; the idle
    // time-out bounds how long that may take.
    let read_timeout = (params.idle_time_out_ms > 0)
        .then(|| Duration::from_millis(u64::from(params.idle_time_out_ms) * 2));
    wire.socket().set_read_timeout(read_timeout)?;
    Ok((wire, reader, remote_open))
}

fn open_body(params: &ConnectParams) -> io::Result<Vec<u8>> {
    let properties = AmqpExtValue::Map(
        params
            .properties
            .iter()
            .map(|(k, v)| {
                (
                    AmqpExtValue::Symbol(k.clone()),
                    AmqpExtValue::Str(v.clone()),
                )
            })
            .collect(),
    );
    let caps: Vec<&str> = params.capabilities.iter().map(String::as_str).collect();
    encode_performative(
        descriptor::OPEN,
        &AmqpExtValue::List(vec![
            AmqpExtValue::Str(params.container_id.clone()),
            params
                .hostname
                .clone()
                .map_or(AmqpExtValue::Null, AmqpExtValue::Str),
            AmqpExtValue::Uint(params.max_frame_size.max(MIN_MAX_FRAME_SIZE)),
            AmqpExtValue::Ushort(params.channel_max),
            if params.idle_time_out_ms > 0 {
                AmqpExtValue::Uint(params.idle_time_out_ms)
            } else {
                AmqpExtValue::Null
            },
            AmqpExtValue::Null, // outgoing-locales
            AmqpExtValue::Null, // incoming-locales
            symbol_array(&caps),
            symbol_array(&caps),
            properties,
        ]),
    )
    .map_err(codec)
}

fn random_nonce() -> io::Result<String> {
    let mut bytes = [0u8; 18];
    zerodds_bridge_security::tls_provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| io::Error::other("no random source for the SCRAM nonce"))?;
    Ok(crate::scram::b64(&bytes))
}

/// The SASL layer (Part 5 §5.3): the broker offers mechanisms, the client
/// picks the configured one, answers challenges, reads the outcome.
fn sasl(wire: &mut Wire, reader: &mut FrameReader, creds: &SaslCredentials) -> io::Result<()> {
    use zerodds_amqp_bridge::performatives::{
        sasl_init, sasl_mechanisms_from_body, sasl_outcome_code, sasl_plain_response, sasl_response,
    };
    wire.write_all(&SASL_HEADER)?;
    let peer = reader.read_exact_header(8)?;
    if peer[..] != SASL_HEADER[..] {
        return Err(protocol(format!(
            "the broker answered {peer:02x?} to the SASL header; it does not offer SASL"
        )));
    }
    let next_sasl = |reader: &mut FrameReader| -> io::Result<(u64, AmqpExtValue)> {
        let f = reader.next()?;
        if f.frame_type != FrameType::Sasl {
            return Err(protocol("expected a SASL frame"));
        }
        let (d, v, _) = decode_performative(&f.body).map_err(codec)?;
        Ok((d, v))
    };
    let (d, v) = next_sasl(reader)?;
    if d != descriptor::SASL_MECHANISMS {
        return Err(protocol("expected sasl-mechanisms"));
    }
    let offered = sasl_mechanisms_from_body(&v);
    let mech = creds.mechanism();
    if !offered.iter().any(|m| m == mech) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "the broker offers SASL {}, not the configured {mech}",
                offered.join(", ")
            ),
        ));
    }
    let mut scram: Option<(String, String, String)> = None; // (client-first-bare, nonce, password)
    let initial: Vec<u8> = match creds {
        SaslCredentials::Anonymous => Vec::new(),
        SaslCredentials::External => Vec::new(),
        SaslCredentials::Plain { username, password } => sasl_plain_response(username, password),
        SaslCredentials::XOAuth2 { username, token } => {
            format!("user={username}\x01auth=Bearer {token}\x01\x01").into_bytes()
        }
        SaslCredentials::ScramSha256 { username, password } => {
            let nonce = random_nonce()?;
            let user = username.replace('=', "=3D").replace(',', "=2C");
            let bare = format!("n={user},r={nonce}");
            let first = format!("n,,{bare}");
            scram = Some((bare, nonce, password.clone()));
            first.into_bytes()
        }
    };
    let init = sasl_init(mech, &initial).map_err(codec)?;
    write_frame(wire, FrameType::Sasl, 0, &init)?;
    let mut server_signature: Option<[u8; 32]> = None;
    loop {
        let (d, v) = next_sasl(reader)?;
        match d {
            descriptor::SASL_CHALLENGE => {
                let challenge = match field(&v, 0) {
                    Some(AmqpExtValue::Binary(b)) => b.clone(),
                    _ => Vec::new(),
                };
                let Some((bare, nonce, password)) = scram.as_ref() else {
                    return Err(protocol(format!("unexpected SASL challenge for {mech}")));
                };
                let server_first = String::from_utf8(challenge)
                    .map_err(|_| protocol("SCRAM server-first is not UTF-8"))?;
                let (fin, signature) = scram_client_final(bare, nonce, &server_first, password)?;
                server_signature = Some(signature);
                let resp = sasl_response(fin.as_bytes()).map_err(codec)?;
                write_frame(wire, FrameType::Sasl, 0, &resp)?;
            }
            descriptor::SASL_OUTCOME => {
                return match sasl_outcome_code(&v) {
                    Some(0) => {
                        if let Some(expected) = server_signature {
                            let data = match field(&v, 1) {
                                Some(AmqpExtValue::Binary(b)) => {
                                    String::from_utf8_lossy(b).to_string()
                                }
                                _ => String::new(),
                            };
                            let v_attr = data.split(',').find_map(|p| p.strip_prefix("v="));
                            let got = v_attr.and_then(crate::scram::b64_decode);
                            if got.as_deref() != Some(&expected[..]) {
                                return Err(io::Error::new(
                                    io::ErrorKind::PermissionDenied,
                                    "SCRAM server signature does not verify; the broker does not know the password",
                                ));
                            }
                        }
                        Ok(())
                    }
                    Some(code) => Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!("SASL {mech} refused, outcome code {code}"),
                    )),
                    None => Err(protocol("sasl-outcome without a code")),
                };
            }
            other => return Err(protocol(format!("unexpected SASL performative {other:#x}"))),
        }
    }
}

/// The SCRAM-SHA-256 client-final message for a server-first challenge,
/// and the server signature the outcome must carry (RFC 5802 §3, §5.1;
/// RFC 7677). `client_first_bare` is `n=<user>,r=<nonce>`.
///
/// # Errors
/// A server-first without nonce, salt or iteration count, a server nonce
/// that does not extend the client's, fewer than 4096 iterations
/// (RFC 7677 §4).
pub fn scram_client_final(
    client_first_bare: &str,
    client_nonce: &str,
    server_first: &str,
    password: &str,
) -> io::Result<(String, [u8; 32])> {
    let attr = |k: &str| {
        server_first
            .split(',')
            .find_map(|p| p.strip_prefix(k))
            .map(str::to_string)
    };
    let full_nonce = attr("r=").ok_or_else(|| protocol("SCRAM server-first without r="))?;
    if !full_nonce.starts_with(client_nonce) || full_nonce.len() == client_nonce.len() {
        return Err(protocol(
            "SCRAM server nonce does not extend the client nonce",
        ));
    }
    let salt = attr("s=")
        .and_then(|s| crate::scram::b64_decode(&s))
        .ok_or_else(|| protocol("SCRAM server-first without a salt"))?;
    let iterations: u32 = attr("i=")
        .and_then(|s| s.parse().ok())
        .filter(|i| *i >= 4096)
        .ok_or_else(|| protocol("SCRAM iteration count missing or below 4096"))?;
    let without_proof = format!("c=biws,r={full_nonce}");
    let auth_message = crate::scram::auth_message(client_first_bare, server_first, &without_proof);
    let proof = crate::scram::client_proof(password, &salt, iterations, &auth_message);
    let signature = crate::scram::ScramServerSecrets::from_password(password, &salt, iterations)
        .server_signature(&auth_message);
    Ok((
        format!("{without_proof},p={}", crate::scram::b64(&proof)),
        signature,
    ))
}

// ============================================================================
// Writer half.
// ============================================================================

/// What the two halves share: per session of the client, the transfer-id
/// of the broker's next transfer frame (the reader counts, the writer
/// reports it in flows), and the broker's channel of each session.
#[derive(Default)]
struct Shared {
    next_incoming_id: BTreeMap<u16, u32>,
    /// The broker's channel → the client's.
    channel_of: BTreeMap<u16, u16>,
}

fn shared(m: &Mutex<Shared>) -> io::Result<std::sync::MutexGuard<'_, Shared>> {
    m.lock()
        .map_err(|_| io::Error::other("session state poisoned by a panicked thread"))
}

/// The client's state of one session (Part 2 §2.5.6).
struct SessionOut {
    /// The transfer-id of the next transfer frame.
    next_outgoing_id: u32,
    /// The delivery-id of the next delivery (Part 2 §2.6.12).
    next_delivery_id: u32,
    /// Transfer frames the broker's window still admits.
    remote_incoming_window: u32,
}

/// The sending half of an open connection.
pub struct BrokerWriter {
    wire: Wire,
    max_frame_size: u32,
    sessions: BTreeMap<u16, SessionOut>,
    shared: Arc<Mutex<Shared>>,
}

impl BrokerWriter {
    /// The negotiated frame size.
    #[must_use]
    pub fn max_frame_size(&self) -> u32 {
        self.max_frame_size
    }

    /// How many more transfer frames the broker's window of session
    /// `channel` admits.
    #[must_use]
    pub fn remote_incoming_window(&self, channel: u16) -> u32 {
        self.sessions
            .get(&channel)
            .map_or(0, |s| s.remote_incoming_window)
    }

    fn send(&mut self, channel: u16, body: &[u8]) -> io::Result<()> {
        write_frame(&mut self.wire, FrameType::Amqp, channel, body)
    }

    fn session(&mut self, channel: u16) -> io::Result<&mut SessionOut> {
        self.sessions
            .get_mut(&channel)
            .ok_or_else(|| protocol(format!("no session on channel {channel}")))
    }

    /// Begins a session on `channel` (Part 2 §2.7.2); the broker answers
    /// with [`Inbound::Begin`]. The incoming window is as large as §2.5.6
    /// allows, so the broker's transfers are bounded by link credit alone.
    ///
    /// # Errors
    /// I/O errors.
    pub fn begin(&mut self, channel: u16) -> io::Result<()> {
        let body = encode_performative(
            descriptor::BEGIN,
            &AmqpExtValue::List(vec![
                AmqpExtValue::Null,
                AmqpExtValue::Uint(0),
                AmqpExtValue::Uint(i32::MAX as u32),
                AmqpExtValue::Uint(i32::MAX as u32),
                AmqpExtValue::Uint(u32::MAX),
            ]),
        )
        .map_err(codec)?;
        self.sessions.insert(
            channel,
            SessionOut {
                next_outgoing_id: 0,
                next_delivery_id: 0,
                remote_incoming_window: 0,
            },
        );
        self.send(channel, &body)
    }

    /// Records the broker's begin: its window for the client's transfers.
    pub fn session_begun(&mut self, channel: u16, remote_incoming_window: u32) {
        if let Some(s) = self.sessions.get_mut(&channel) {
            s.remote_incoming_window = remote_incoming_window;
        }
    }

    /// Ends session `channel`.
    ///
    /// # Errors
    /// I/O errors.
    pub fn end(&mut self, channel: u16, error: Option<&AmqpError>) -> io::Result<()> {
        self.sessions.remove(&channel);
        let body = encode_performative(
            descriptor::END,
            &AmqpExtValue::List(vec![error.map_or(AmqpExtValue::Null, encode_error)]),
        )
        .map_err(codec)?;
        self.send(channel, &body)
    }

    /// Applies a session flow from the broker (Part 2 §2.5.6):
    /// `next-incoming-id + incoming-window - next-outgoing-id`.
    pub fn apply_session_flow(
        &mut self,
        channel: u16,
        next_incoming_id: Option<u32>,
        incoming_window: u32,
    ) {
        if let Some(s) = self.sessions.get_mut(&channel) {
            let base = next_incoming_id.unwrap_or(0);
            s.remote_incoming_window = base
                .wrapping_add(incoming_window)
                .wrapping_sub(s.next_outgoing_id);
        }
    }

    /// Attaches the client's end of a link on session `channel`.
    ///
    /// # Errors
    /// I/O errors.
    pub fn attach(&mut self, channel: u16, link: &LinkSpec) -> io::Result<()> {
        let node = described(
            if link.sender {
                desc::TARGET
            } else {
                desc::SOURCE
            },
            vec![
                AmqpExtValue::Str(link.address.clone()),
                AmqpExtValue::Uint(link.durable),
            ],
        );
        let properties = AmqpExtValue::Map(
            link.properties
                .iter()
                .map(|(k, v)| {
                    (
                        AmqpExtValue::Symbol(k.clone()),
                        AmqpExtValue::Str(v.clone()),
                    )
                })
                .collect(),
        );
        let caps: Vec<&str> = link
            .desired_capabilities
            .iter()
            .map(String::as_str)
            .collect();
        let body = encode_performative(
            descriptor::ATTACH,
            &AmqpExtValue::List(vec![
                AmqpExtValue::Str(link.name.clone()),
                AmqpExtValue::Uint(link.handle),
                AmqpExtValue::Boolean(!link.sender),
                AmqpExtValue::Ubyte(link.snd_settle_mode),
                AmqpExtValue::Ubyte(link.rcv_settle_mode),
                if link.sender {
                    described(desc::SOURCE, vec![])
                } else {
                    node.clone()
                },
                if link.sender {
                    node
                } else {
                    described(desc::TARGET, vec![])
                },
                AmqpExtValue::Null,           // unsettled
                AmqpExtValue::Boolean(false), // incomplete-unsettled
                if link.sender {
                    AmqpExtValue::Uint(0)
                } else {
                    AmqpExtValue::Null
                },
                AmqpExtValue::Null, // max-message-size
                AmqpExtValue::Null, // offered-capabilities
                symbol_array(&caps),
                properties,
            ]),
        )
        .map_err(codec)?;
        self.send(channel, &body)
    }

    /// Grants `link_credit` on a receiving link whose `delivery_count` the
    /// client has counted (Part 2 §2.6.7).
    ///
    /// # Errors
    /// I/O errors.
    pub fn flow(
        &mut self,
        channel: u16,
        handle: u32,
        delivery_count: u32,
        link_credit: u32,
    ) -> io::Result<()> {
        let next_incoming = shared(&self.shared)?
            .next_incoming_id
            .get(&channel)
            .copied()
            .unwrap_or(0);
        let next_outgoing = self.session(channel)?.next_outgoing_id;
        let body = encode_performative(
            descriptor::FLOW,
            &AmqpExtValue::List(vec![
                AmqpExtValue::Uint(next_incoming),
                AmqpExtValue::Uint(i32::MAX as u32),
                AmqpExtValue::Uint(next_outgoing),
                AmqpExtValue::Uint(u32::MAX),
                AmqpExtValue::Uint(handle),
                AmqpExtValue::Uint(delivery_count),
                AmqpExtValue::Uint(link_credit),
            ]),
        )
        .map_err(codec)?;
        self.send(channel, &body)
    }

    /// Sends one message on a sending link of session `channel`; returns
    /// the delivery id. The message is split into frames of the negotiated
    /// size (`more`).
    ///
    /// # Errors
    /// I/O errors; `WouldBlock` when the broker's session window has no
    /// room for the frames the message needs.
    pub fn transfer(
        &mut self,
        channel: u16,
        handle: u32,
        delivery_tag: &[u8],
        message: &[u8],
        settled: bool,
    ) -> io::Result<u32> {
        let max_frame = self.max_frame_size as usize;
        let session = self.session(channel)?;
        let delivery_id = session.next_delivery_id;
        let window = session.remote_incoming_window as usize;
        let head = |more: bool, first: bool| -> io::Result<Vec<u8>> {
            let list = if first {
                vec![
                    AmqpExtValue::Uint(handle),
                    AmqpExtValue::Uint(delivery_id),
                    AmqpExtValue::Binary(delivery_tag.to_vec()),
                    AmqpExtValue::Uint(0), // message-format
                    AmqpExtValue::Boolean(settled),
                    AmqpExtValue::Boolean(more),
                ]
            } else {
                vec![
                    AmqpExtValue::Uint(handle),
                    AmqpExtValue::Null,
                    AmqpExtValue::Null,
                    AmqpExtValue::Null,
                    AmqpExtValue::Null,
                    AmqpExtValue::Boolean(more),
                ]
            };
            encode_performative(descriptor::TRANSFER, &AmqpExtValue::List(list)).map_err(codec)
        };
        // Room per frame: the negotiated size less the frame header and the
        // performative (its encoded size with `more = true`, the larger).
        let overhead = 8 + head(true, true)?.len();
        let room = max_frame
            .checked_sub(overhead)
            .filter(|r| *r > 0)
            .ok_or_else(|| protocol("max-frame-size leaves no room for a payload"))?;
        let frames = message.len().div_ceil(room).max(1);
        if window < frames {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "the broker's session window is full",
            ));
        }
        let mut chunks = message.chunks(room).peekable();
        let mut first = true;
        loop {
            let chunk = chunks.next().unwrap_or(&[]);
            let more = chunks.peek().is_some();
            let mut body = head(more, first)?;
            body.extend_from_slice(chunk);
            self.send(channel, &body)?;
            let session = self.session(channel)?;
            session.remote_incoming_window = session.remote_incoming_window.saturating_sub(1);
            session.next_outgoing_id = session.next_outgoing_id.wrapping_add(1);
            first = false;
            if !more {
                break;
            }
        }
        let session = self.session(channel)?;
        session.next_delivery_id = session.next_delivery_id.wrapping_add(1);
        Ok(delivery_id)
    }

    /// Settles the broker's deliveries `first..=last` of session `channel`
    /// with `outcome`.
    ///
    /// # Errors
    /// I/O errors.
    pub fn disposition(
        &mut self,
        channel: u16,
        first: u32,
        last: u32,
        outcome: &Outcome,
    ) -> io::Result<()> {
        let body = encode_performative(
            descriptor::DISPOSITION,
            &AmqpExtValue::List(vec![
                AmqpExtValue::Boolean(true), // role: receiver
                AmqpExtValue::Uint(first),
                AmqpExtValue::Uint(last),
                AmqpExtValue::Boolean(true),
                encode_outcome(outcome),
            ]),
        )
        .map_err(codec)?;
        self.send(channel, &body)
    }

    /// Detaches the client's end of a link.
    ///
    /// # Errors
    /// I/O errors.
    pub fn detach(
        &mut self,
        channel: u16,
        handle: u32,
        error: Option<&AmqpError>,
    ) -> io::Result<()> {
        let body = encode_performative(
            descriptor::DETACH,
            &AmqpExtValue::List(vec![
                AmqpExtValue::Uint(handle),
                AmqpExtValue::Boolean(true),
                error.map_or(AmqpExtValue::Null, encode_error),
            ]),
        )
        .map_err(codec)?;
        self.send(channel, &body)
    }

    /// An empty frame (Part 2 §2.4.5): keeps the broker's idle time-out
    /// from expiring.
    ///
    /// # Errors
    /// I/O errors.
    pub fn heartbeat(&mut self) -> io::Result<()> {
        write_frame(&mut self.wire, FrameType::Amqp, 0, &[])
    }

    /// Ends every session and closes the connection; the socket is shut
    /// down after the close frame.
    pub fn close(mut self, error: Option<&AmqpError>) {
        let channels: Vec<u16> = self.sessions.keys().copied().collect();
        for c in channels {
            let _ = self.end(c, None);
        }
        let err = error.map_or(AmqpExtValue::Null, encode_error);
        if let Ok(close) = encode_performative(descriptor::CLOSE, &AmqpExtValue::List(vec![err])) {
            let _ = self.send(0, &close);
        }
        let _ = self.wire.socket().shutdown(std::net::Shutdown::Both);
    }
}

// ============================================================================
// Reader half.
// ============================================================================

/// A multi-frame transfer being put back together.
struct Partial {
    channel: u16,
    remote_handle: u32,
    delivery_id: u32,
    settled: bool,
    message: Vec<u8>,
}

/// The receiving half of an open connection.
pub struct BrokerReader {
    frames: FrameReader,
    partial: Option<Partial>,
    shared: Arc<Mutex<Shared>>,
}

impl BrokerReader {
    /// The next event from the broker, blocking until one arrives or the
    /// read timeout (twice the client's idle time-out) expires.
    ///
    /// # Errors
    /// I/O errors, the read timeout (`WouldBlock` / `TimedOut`: the broker
    /// sent nothing, not even a heartbeat), and protocol errors.
    pub fn next_event(&mut self) -> io::Result<Inbound> {
        loop {
            let f = self.frames.next()?;
            if f.frame_type != FrameType::Amqp {
                return Err(protocol("a SASL frame after the SASL layer"));
            }
            if f.body.is_empty() {
                return Ok(Inbound::Heartbeat);
            }
            let (d, v, used) = decode_performative(&f.body).map_err(codec)?;
            let payload = &f.body[used..];
            if d == descriptor::CLOSE {
                return Ok(Inbound::Close(decode_error(field(&v, 0))));
            }
            if d == descriptor::BEGIN {
                let ours = match field(&v, 0) {
                    Some(AmqpExtValue::Ushort(c)) => *c,
                    _ => return Err(protocol("a begin the client did not ask for")),
                };
                let next_outgoing_id = as_u32(field(&v, 1)).unwrap_or(0);
                let mut sh = shared(&self.shared)?;
                sh.channel_of.insert(f.channel, ours);
                sh.next_incoming_id.insert(ours, next_outgoing_id);
                return Ok(Inbound::Begin {
                    channel: ours,
                    next_outgoing_id,
                    incoming_window: as_u32(field(&v, 2)).unwrap_or(0),
                });
            }
            let channel = shared(&self.shared)?
                .channel_of
                .get(&f.channel)
                .copied()
                .ok_or_else(|| {
                    protocol(format!(
                        "a frame on channel {}, no session there",
                        f.channel
                    ))
                })?;
            let event = match d {
                descriptor::ATTACH => Inbound::Attach {
                    channel,
                    name: as_text(field(&v, 0)).unwrap_or_default(),
                    remote_handle: as_u32(field(&v, 1))
                        .ok_or_else(|| protocol("attach without handle"))?,
                    // role true = the broker receives, its target names our node.
                    has_node: if as_bool(field(&v, 2)).unwrap_or(false) {
                        field(&v, 6).is_some()
                    } else {
                        field(&v, 5).is_some()
                    },
                    initial_delivery_count: as_u32(field(&v, 9)),
                },
                descriptor::FLOW => Inbound::Flow {
                    channel,
                    next_incoming_id: as_u32(field(&v, 0)),
                    incoming_window: as_u32(field(&v, 1)).unwrap_or(0),
                    remote_handle: as_u32(field(&v, 4)),
                    delivery_count: as_u32(field(&v, 5)),
                    link_credit: as_u32(field(&v, 6)),
                    echo: as_bool(field(&v, 9)).unwrap_or(false),
                },
                descriptor::TRANSFER => {
                    if let Some(n) = shared(&self.shared)?.next_incoming_id.get_mut(&channel) {
                        *n = n.wrapping_add(1);
                    }
                    let remote_handle =
                        as_u32(field(&v, 0)).ok_or_else(|| protocol("transfer without handle"))?;
                    let more = as_bool(field(&v, 5)).unwrap_or(false);
                    let aborted = as_bool(field(&v, 9)).unwrap_or(false);
                    let mut part = match self.partial.take() {
                        Some(p) if p.channel == channel && p.remote_handle == remote_handle => p,
                        Some(_) => {
                            return Err(protocol(
                                "a transfer on another link inside an unfinished multi-frame transfer",
                            ));
                        }
                        None => Partial {
                            channel,
                            remote_handle,
                            delivery_id: as_u32(field(&v, 1)).ok_or_else(|| {
                                protocol("first transfer frame without delivery-id")
                            })?,
                            settled: as_bool(field(&v, 4)).unwrap_or(false),
                            message: Vec::new(),
                        },
                    };
                    if aborted {
                        // Part 2 §2.6.14: an aborted delivery is discarded.
                        continue;
                    }
                    if part.message.len() + payload.len() > MAX_MESSAGE_SIZE {
                        return Err(protocol(format!(
                            "a message above {MAX_MESSAGE_SIZE} bytes"
                        )));
                    }
                    part.message.extend_from_slice(payload);
                    if as_bool(field(&v, 4)).unwrap_or(false) {
                        part.settled = true;
                    }
                    if more {
                        self.partial = Some(part);
                        continue;
                    }
                    Inbound::Transfer {
                        channel,
                        remote_handle: part.remote_handle,
                        delivery_id: part.delivery_id,
                        settled: part.settled,
                        message: part.message,
                    }
                }
                descriptor::DISPOSITION => {
                    let first = as_u32(field(&v, 1))
                        .ok_or_else(|| protocol("disposition without first"))?;
                    Inbound::Disposition {
                        channel,
                        role_receiver: as_bool(field(&v, 0)).unwrap_or(false),
                        first,
                        last: as_u32(field(&v, 2)).unwrap_or(first),
                        settled: as_bool(field(&v, 3)).unwrap_or(false),
                        outcome: decode_outcome(field(&v, 4)),
                    }
                }
                descriptor::DETACH => Inbound::Detach {
                    channel,
                    remote_handle: as_u32(field(&v, 0))
                        .ok_or_else(|| protocol("detach without handle"))?,
                    closed: as_bool(field(&v, 1)).unwrap_or(false),
                    error: decode_error(field(&v, 2)),
                },
                descriptor::END => {
                    let mut sh = shared(&self.shared)?;
                    sh.channel_of.remove(&f.channel);
                    sh.next_incoming_id.remove(&channel);
                    Inbound::End {
                        channel,
                        error: decode_error(field(&v, 0)),
                    }
                }
                other => return Err(protocol(format!("unexpected performative {other:#x}"))),
            };
            return Ok(event);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// RFC 7677 §3: user `user`, password `pencil`, the client and server
    /// messages of the RFC; the client-final and the server signature are
    /// the RFC's bytes.
    #[test]
    fn scram_client_final_matches_the_rfc_7677_example() {
        let (fin, sig) = scram_client_final(
            "n=user,r=rOprNGfwEbeRWgbNEkqO",
            "rOprNGfwEbeRWgbNEkqO",
            "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096",
            "pencil",
        )
        .unwrap();
        assert_eq!(
            fin,
            "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
        );
        assert_eq!(
            crate::scram::b64(&sig),
            "6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="
        );
        // A server nonce that does not extend the client's is refused.
        assert!(scram_client_final("n=u,r=abc", "abc", "r=xyz,s=AAAA,i=4096", "p").is_err());
        assert!(scram_client_final("n=u,r=abc", "abc", "r=abcd,s=AAAA,i=1000", "p").is_err());
    }

    #[test]
    fn outcomes_round_trip_through_their_described_form() {
        for o in [
            Outcome::Accepted,
            Outcome::Released,
            Outcome::Rejected(Some(AmqpError {
                condition: "amqp:decode-error".into(),
                description: Some("bad body".into()),
            })),
            Outcome::Modified {
                delivery_failed: true,
                undeliverable_here: false,
            },
        ] {
            assert_eq!(decode_outcome(Some(&encode_outcome(&o))), o);
        }
    }

    fn pair() -> (TcpStream, TcpStream) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::thread::spawn(move || TcpStream::connect(addr).unwrap());
        let (server, _) = listener.accept().unwrap();
        (client.join().unwrap(), server)
    }

    fn writer_with_session(
        client: TcpStream,
        window: u32,
        shared: Arc<Mutex<Shared>>,
    ) -> BrokerWriter {
        let mut sessions = BTreeMap::new();
        sessions.insert(
            3,
            SessionOut {
                next_outgoing_id: 0,
                next_delivery_id: 7,
                remote_incoming_window: window,
            },
        );
        BrokerWriter {
            wire: Wire::Plain(client),
            max_frame_size: 512,
            sessions,
            shared,
        }
    }

    /// A transfer larger than one frame goes out in pieces that fit and
    /// comes back whole through the reader, on the session's channel.
    #[test]
    fn a_message_above_the_frame_size_is_split_and_reassembled() {
        let (client, server) = pair();
        let shared = Arc::new(Mutex::new(Shared::default()));
        {
            let mut sh = shared.lock().unwrap();
            // The broker's channel 9 is the client's session 3.
            sh.channel_of.insert(9, 3);
            sh.next_incoming_id.insert(3, 0);
        }
        let mut writer = writer_with_session(client, 100, Arc::clone(&shared));
        let message: Vec<u8> = (0..5000u32).map(|i| (i % 253) as u8).collect();
        let id = writer.transfer(3, 4, b"tag", &message, false).unwrap();
        assert_eq!(id, 7);
        let frames = writer.sessions[&3].next_outgoing_id;
        assert!(frames >= 10, "{frames} frames for 5000 bytes at 512");
        assert_eq!(
            writer.remote_incoming_window(3),
            100 - frames,
            "one window slot per frame"
        );
        assert_eq!(
            writer.sessions[&3].next_delivery_id, 8,
            "one delivery id per message"
        );
        // The frames went out on channel 3; the reader sees them on the
        // broker's side as channel 3 too, which must map to a session.
        shared.lock().unwrap().channel_of.insert(3, 3);
        let mut reader = BrokerReader {
            frames: FrameReader {
                wire: Wire::Plain(server),
                buf: Vec::new(),
            },
            partial: None,
            shared: Arc::clone(&shared),
        };
        match reader.next_event().unwrap() {
            Inbound::Transfer {
                channel,
                remote_handle,
                delivery_id,
                settled,
                message: got,
            } => {
                assert_eq!(
                    (channel, remote_handle, delivery_id, settled),
                    (3, 4, 7, false)
                );
                assert_eq!(got, message);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            shared.lock().unwrap().next_incoming_id[&3],
            frames,
            "one transfer-id per frame"
        );
    }

    #[test]
    fn a_full_session_window_refuses_the_transfer() {
        let (client, _server) = pair();
        let mut writer = writer_with_session(client, 1, Arc::new(Mutex::new(Shared::default())));
        let err = writer.transfer(3, 0, b"t", &[0u8; 2000], true).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
        writer.apply_session_flow(3, Some(0), 10);
        assert_eq!(writer.remote_incoming_window(3), 10);
        writer.transfer(3, 0, b"t", &[0u8; 2000], true).unwrap();
        // A transfer on a session that was never begun is an error, not a
        // frame on a channel the broker does not know.
        assert!(writer.transfer(5, 0, b"t", &[1], true).is_err());
    }
}
