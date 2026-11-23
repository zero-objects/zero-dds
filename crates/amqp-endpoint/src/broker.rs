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
//! Part 4 (transactions: a link to the coordinator, transfers,
//! dispositions and flows that name a transaction; the controller on top
//! is [`crate::txn`]), Part 5 (SASL). SCRAM: RFC 5802 / RFC 7677. XOAUTH2: the Google/Microsoft
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

mod sasl;

pub use crate::terminus::{AmqpError, DeliveryState, Outcome};
use crate::terminus::{
    OUTCOME_SYMBOLS, Source, as_bool, as_text, as_u32, decode_error, decode_state_up_to, desc,
    described, encode_error, encode_outcome, encode_state, field, symbol_array, symbols,
};
use crate::transactions::{Coordinator, TxnCapability, TxnId, flow_properties};
pub use sasl::{SaslCredentials, scram_client_final};

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
// Public types.
// ============================================================================

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
    /// `Some`: the link is a control link (Part 4 §4.2): its target is a
    /// coordinator with these desired capabilities in place of a node at
    /// `address`, and its source supports the four outcomes, `rejected`
    /// among them ("Transaction controllers SHOULD establish a control
    /// link that allows the rejected outcome"). The client sends on it.
    pub coordinator: Option<Vec<TxnCapability>>,
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
        /// The broker's target when it is a coordinator (Part 4 §4.5.1),
        /// with the capabilities the coordinator has.
        coordinator: Option<Coordinator>,
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
        /// Link: `drain`.
        drain: bool,
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
        /// `transfer.state` of the first frame (Part 4 §4.4.3: a
        /// `transactional-state` for a message acquired in a transaction).
        state: Option<DeliveryState>,
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
        /// The delivery state as an outcome of Part 3; [`Outcome::None`]
        /// for `received` and for the states of Part 4.
        outcome: Outcome,
        /// The delivery state, with `declared` and `transactional-state`
        /// (Part 4 §4.5.5, §4.5.6).
        state: Option<DeliveryState>,
        /// The `message-annotations` of a `modified` outcome (Part 3
        /// §3.4.5), to combine with the message's before it is sent again;
        /// empty for any other state.
        annotations: Vec<(AmqpExtValue, AmqpExtValue)>,
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

    sasl::exchange(&mut wire, &mut reader, &params.sasl).map_err(at(ConnectStage::Sasl))?;
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
            txn_id_limit: TxnId::MAX_LEN,
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

/// The credit of a sending link after the receiver's flow (Part 2 §2.6.7):
/// "link-credit(snd) := delivery-count(rcv) + link-credit(rcv) -
/// delivery-count(snd)", where a flow without delivery-count (the receiver
/// has not seen the sender's attach) counts from the sender's
/// `initial_delivery_count`. The counts are serial numbers (RFC 1982): a
/// limit behind `delivery_count`, which a flow sent before the receiver saw
/// the deliveries in flight states, is no credit, not a wrapped one.
#[must_use]
pub fn sender_credit(
    receiver_count: Option<u32>,
    link_credit: u32,
    delivery_count: u32,
    initial_delivery_count: u32,
) -> u32 {
    let left = receiver_count
        .unwrap_or(initial_delivery_count)
        .wrapping_add(link_credit)
        .wrapping_sub(delivery_count);
    if left < 0x8000_0000 { left } else { 0 }
}

/// The initial-delivery-count the attach of a sending link states (Part 2
/// §2.7.3); [`BrokerWriter::attach`] sends it.
pub const INITIAL_DELIVERY_COUNT: u32 = 0;

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
        self.attach_with_target(channel, link, None)
    }

    /// [`BrokerWriter::attach`] of a receiving link whose own target is
    /// the node `target`: the address a request names as its `reply-to`
    /// (AMQP CBS, a management node), where the broker sends the answers.
    ///
    /// # Errors
    /// As [`BrokerWriter::attach`]; `InvalidInput` for a sending link.
    pub fn attach_receiver_to(
        &mut self,
        channel: u16,
        link: &LinkSpec,
        target: &str,
    ) -> io::Result<()> {
        if link.sender {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the target of a sending link is its node",
            ));
        }
        self.attach_with_target(channel, link, Some(target))
    }

    fn attach_with_target(
        &mut self,
        channel: u16,
        link: &LinkSpec,
        own_target: Option<&str>,
    ) -> io::Result<()> {
        if link.coordinator.is_some() && !link.sender {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a control link is one the client sends on (Part 4 §4.2)",
            ));
        }
        let node = match &link.coordinator {
            Some(capabilities) => Coordinator {
                capabilities: capabilities.clone(),
            }
            .encode(),
            None => described(
                if link.sender {
                    desc::TARGET
                } else {
                    desc::SOURCE
                },
                vec![
                    AmqpExtValue::Str(link.address.clone()),
                    AmqpExtValue::Uint(link.durable),
                ],
            ),
        };
        let own_source = if link.coordinator.is_some() {
            Source {
                outcomes: OUTCOME_SYMBOLS.iter().map(|s| (*s).to_string()).collect(),
                ..Source::default()
            }
            .encode()
        } else {
            described(desc::SOURCE, vec![])
        };
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
                    own_source
                } else {
                    node.clone()
                },
                if link.sender {
                    node
                } else {
                    described(
                        desc::TARGET,
                        own_target.map_or_else(Vec::new, |t| vec![AmqpExtValue::Str(t.into())]),
                    )
                },
                AmqpExtValue::Null,           // unsettled
                AmqpExtValue::Boolean(false), // incomplete-unsettled
                if link.sender {
                    AmqpExtValue::Uint(INITIAL_DELIVERY_COUNT)
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
        self.flow_in_txn(channel, handle, delivery_count, link_credit, false, None)
    }

    /// [`BrokerWriter::flow`] with `drain` and the transaction the broker
    /// is to acquire the messages of this link in (Part 4 §4.4.3: the
    /// `txn-id` entry of `flow.properties`). `Some(None)` sends a
    /// properties map without the entry, which ends the association;
    /// `None` sends no properties. Before the discharge "the controller
    /// SHOULD ensure there is no outstanding credit at the sender", with
    /// `drain` or with a credit of zero.
    ///
    /// # Errors
    /// I/O errors.
    pub fn flow_in_txn(
        &mut self,
        channel: u16,
        handle: u32,
        delivery_count: u32,
        link_credit: u32,
        drain: bool,
        txn: Option<Option<&TxnId>>,
    ) -> io::Result<()> {
        let mut list = self.session_flow_fields(channel)?;
        list.extend([
            AmqpExtValue::Uint(handle),
            AmqpExtValue::Uint(delivery_count),
            AmqpExtValue::Uint(link_credit),
        ]);
        if drain || txn.is_some() {
            // available (7) is not set; drain is field 8.
            list.push(AmqpExtValue::Null);
            list.push(AmqpExtValue::Boolean(drain));
        }
        if let Some(txn) = txn {
            // echo (9) is not set; properties is field 10.
            list.push(AmqpExtValue::Null);
            list.push(txn.map_or_else(|| AmqpExtValue::Map(Vec::new()), flow_properties));
        }
        let body =
            encode_performative(descriptor::FLOW, &AmqpExtValue::List(list)).map_err(codec)?;
        self.send(channel, &body)
    }

    /// The flow state of a sending link (Part 2 §2.7.4): its
    /// `delivery_count`, the `link_credit` it holds, the messages
    /// `available` to it, its drain mode; `echo` not set, so an answer to
    /// an echo asks for none. Sent when the receiver asked to drain (Part
    /// 2 §2.6.7) or for the link's state.
    ///
    /// # Errors
    /// I/O errors.
    pub fn sender_flow(
        &mut self,
        channel: u16,
        handle: u32,
        delivery_count: u32,
        link_credit: u32,
        available: u32,
        drain: bool,
    ) -> io::Result<()> {
        let mut list = self.session_flow_fields(channel)?;
        list.extend([
            AmqpExtValue::Uint(handle),
            AmqpExtValue::Uint(delivery_count),
            AmqpExtValue::Uint(link_credit),
            AmqpExtValue::Uint(available),
            AmqpExtValue::Boolean(drain),
        ]);
        let body =
            encode_performative(descriptor::FLOW, &AmqpExtValue::List(list)).map_err(codec)?;
        self.send(channel, &body)
    }

    /// The flow state of session `channel` alone, without a link (Part 2
    /// §2.7.4): the answer to a flow with `echo` and no handle.
    ///
    /// # Errors
    /// I/O errors; no session on `channel`.
    pub fn session_flow(&mut self, channel: u16) -> io::Result<()> {
        let list = self.session_flow_fields(channel)?;
        let body =
            encode_performative(descriptor::FLOW, &AmqpExtValue::List(list)).map_err(codec)?;
        self.send(channel, &body)
    }

    /// The four session fields of a flow (Part 2 §2.7.4): the transfer-id
    /// of the broker's next transfer, the client's incoming window (as
    /// large as §2.5.6 allows, as in its begin), its next outgoing id and
    /// its outgoing window.
    fn session_flow_fields(&mut self, channel: u16) -> io::Result<Vec<AmqpExtValue>> {
        let next_incoming = shared(&self.shared)?
            .next_incoming_id
            .get(&channel)
            .copied()
            .unwrap_or(0);
        let next_outgoing = self.session(channel)?.next_outgoing_id;
        Ok(vec![
            AmqpExtValue::Uint(next_incoming),
            AmqpExtValue::Uint(i32::MAX as u32),
            AmqpExtValue::Uint(next_outgoing),
            AmqpExtValue::Uint(u32::MAX),
        ])
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
        self.transfer_with_state(channel, handle, delivery_tag, message, settled, None)
    }

    /// [`BrokerWriter::transfer`] with a `transfer.state` (Part 4 §4.4.1:
    /// a `transactional-state` posts the message to its transaction). The
    /// state is on every frame of the delivery: "if delivery is split
    /// across several transfer frames then all frames MUST be explicitly
    /// associated with the same transaction".
    ///
    /// # Errors
    /// As [`BrokerWriter::transfer`].
    pub fn transfer_with_state(
        &mut self,
        channel: u16,
        handle: u32,
        delivery_tag: &[u8],
        message: &[u8],
        settled: bool,
        state: Option<&DeliveryState>,
    ) -> io::Result<u32> {
        let state = state.map(encode_state);
        let max_frame = self.max_frame_size as usize;
        let session = self.session(channel)?;
        let delivery_id = session.next_delivery_id;
        let window = session.remote_incoming_window as usize;
        let head = |more: bool, first: bool| -> io::Result<Vec<u8>> {
            let mut list = if first {
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
            if let Some(state) = &state {
                // rcv-settle-mode (6) is not set; state is field 7.
                list.push(AmqpExtValue::Null);
                list.push(state.clone());
            }
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
        self.disposition_settled(channel, first, last, true, outcome)
    }

    /// [`BrokerWriter::disposition`] that settles only when `settled`: on a
    /// link whose receiver settle mode is `second` (Part 2 §2.8.3) the
    /// client sends the outcome unsettled and settles once the broker, the
    /// sender, has settled.
    ///
    /// # Errors
    /// I/O errors.
    pub fn disposition_settled(
        &mut self,
        channel: u16,
        first: u32,
        last: u32,
        settled: bool,
        outcome: &Outcome,
    ) -> io::Result<()> {
        let body = encode_performative(
            descriptor::DISPOSITION,
            &AmqpExtValue::List(vec![
                AmqpExtValue::Boolean(true), // role: receiver
                AmqpExtValue::Uint(first),
                AmqpExtValue::Uint(last),
                AmqpExtValue::Boolean(settled),
                encode_outcome(outcome),
            ]),
        )
        .map_err(codec)?;
        self.send(channel, &body)
    }

    /// The state of the broker's deliveries `first..=last` of session
    /// `channel` as this receiver holds it, settled or not (Part 4 §4.4.2:
    /// a `transactional-state` with the outcome associates the outcome
    /// with its transaction).
    ///
    /// # Errors
    /// I/O errors.
    pub fn disposition_with_state(
        &mut self,
        channel: u16,
        first: u32,
        last: u32,
        settled: bool,
        state: &DeliveryState,
    ) -> io::Result<()> {
        let body = encode_performative(
            descriptor::DISPOSITION,
            &AmqpExtValue::List(vec![
                AmqpExtValue::Boolean(true), // role: receiver
                AmqpExtValue::Uint(first),
                AmqpExtValue::Uint(last),
                AmqpExtValue::Boolean(settled),
                encode_state(state),
            ]),
        )
        .map_err(codec)?;
        self.send(channel, &body)
    }

    /// Settles the client's own deliveries `first..=last` of session
    /// `channel` (Part 2 §2.6.12: the sender settles a delivery whose
    /// receiver reached a terminal state without settling).
    ///
    /// # Errors
    /// I/O errors.
    pub fn settle(&mut self, channel: u16, first: u32, last: u32) -> io::Result<()> {
        let body = encode_performative(
            descriptor::DISPOSITION,
            &AmqpExtValue::List(vec![
                AmqpExtValue::Boolean(false), // role: sender
                AmqpExtValue::Uint(first),
                AmqpExtValue::Uint(last),
                AmqpExtValue::Boolean(true),
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
    state: Option<DeliveryState>,
    message: Vec<u8>,
}

/// The receiving half of an open connection.
pub struct BrokerReader {
    frames: FrameReader,
    partial: Option<Partial>,
    shared: Arc<Mutex<Shared>>,
    /// The longest txn-id the reader takes in a `declared` or a
    /// `transactional-state`; 32 octets (Part 4 §4.5.4) unless
    /// [`BrokerReader::set_txn_id_limit`] says otherwise.
    txn_id_limit: usize,
}

impl BrokerReader {
    /// Takes txn-ids of up to `octets` octets in the delivery states of
    /// the broker in place of the 32 of Part 4 §4.5.4 (a value below 32
    /// keeps 32): for a resource that issues longer identifiers, as
    /// Apache ActiveMQ Artemis does with the 36 octets of a UUID's text.
    /// A longer txn-id stays a protocol error of the read.
    pub fn set_txn_id_limit(&mut self, octets: usize) {
        self.txn_id_limit = octets.max(TxnId::MAX_LEN);
    }

    /// How long [`BrokerReader::next_event`] waits for octets of the
    /// broker before it returns `WouldBlock` or `TimedOut`; octets of a
    /// frame read in part are kept for the next call.
    ///
    /// # Errors
    /// The error of the socket option.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.frames.wire.socket().set_read_timeout(timeout)
    }

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
                    coordinator: Coordinator::decode(field(&v, 6))
                        .map_err(|e| protocol(format!("attach.target: {e}")))?,
                },
                descriptor::FLOW => Inbound::Flow {
                    channel,
                    next_incoming_id: as_u32(field(&v, 0)),
                    incoming_window: as_u32(field(&v, 1)).unwrap_or(0),
                    remote_handle: as_u32(field(&v, 4)),
                    delivery_count: as_u32(field(&v, 5)),
                    link_credit: as_u32(field(&v, 6)),
                    echo: as_bool(field(&v, 9)).unwrap_or(false),
                    drain: as_bool(field(&v, 8)).unwrap_or(false),
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
                            state: decode_state_up_to(field(&v, 7), self.txn_id_limit)
                                .map_err(|e| protocol(format!("transfer.state: {e}")))?,
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
                        state: part.state,
                    }
                }
                descriptor::DISPOSITION => {
                    let first = as_u32(field(&v, 1))
                        .ok_or_else(|| protocol("disposition without first"))?;
                    let state = decode_state_up_to(field(&v, 4), self.txn_id_limit)
                        .map_err(|e| protocol(format!("disposition.state: {e}")))?;
                    let annotations = crate::terminus::modified_annotations(field(&v, 4))
                        .map_err(|e| protocol(format!("disposition.state: {e}")))?;
                    Inbound::Disposition {
                        annotations,
                        channel,
                        role_receiver: as_bool(field(&v, 0)).unwrap_or(false),
                        first,
                        last: as_u32(field(&v, 2)).unwrap_or(first),
                        settled: as_bool(field(&v, 3)).unwrap_or(false),
                        outcome: state.as_ref().map_or(Outcome::None, DeliveryState::outcome),
                        state,
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
pub(crate) mod tests {
    use super::*;
    use crate::terminus::decode_state;

    /// AMQP 1.0 Part 3 §3.4: the outcomes are described lists, accepted
    /// 0x24, rejected 0x25 with its error (Part 2 §2.8.14, 0x1d: condition
    /// symbol, description string), released 0x26, modified 0x27 with
    /// delivery-failed and undeliverable-here.
    #[test]
    fn amqp10_3_4_the_outcomes_are_their_described_lists() {
        let mut rejected = vec![0x00, 0x53, 0x25, 0xc0, 0x24, 0x01];
        rejected.extend_from_slice(&[0x00, 0x53, 0x1d, 0xc0, 0x1e, 0x02, 0xa3, 0x11]);
        rejected.extend_from_slice(b"amqp:decode-error");
        rejected.extend_from_slice(&[0xa1, 0x08]);
        rejected.extend_from_slice(b"bad body");
        for (o, octets) in [
            (Outcome::Accepted, vec![0x00, 0x53, 0x24, 0x45]),
            (Outcome::Released, vec![0x00, 0x53, 0x26, 0x45]),
            (
                Outcome::Rejected(Some(AmqpError {
                    condition: "amqp:decode-error".into(),
                    description: Some("bad body".into()),
                })),
                rejected,
            ),
            (
                Outcome::Modified {
                    delivery_failed: true,
                    undeliverable_here: false,
                },
                vec![0x00, 0x53, 0x27, 0xc0, 0x03, 0x02, 0x41, 0x42],
            ),
        ] {
            assert_eq!(encode_outcome(&o).encode().unwrap(), octets, "{o:?}");
            let (value, _) = AmqpExtValue::decode(&octets).unwrap();
            assert_eq!(crate::terminus::decode_outcome(Some(&value)), o);
        }
    }

    pub(crate) fn pair() -> (TcpStream, TcpStream) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::thread::spawn(move || TcpStream::connect(addr).unwrap());
        let (server, _) = listener.accept().unwrap();
        (client.join().unwrap(), server)
    }

    /// A writer on `client` with session 3 begun, and what it writes as
    /// frames: descriptor, fields, payload.
    pub(crate) struct Written {
        frames: FrameReader,
    }

    impl Written {
        pub(crate) fn next(&mut self) -> (u64, Vec<AmqpExtValue>, Vec<u8>) {
            let frame = self.frames.next().unwrap();
            let (code, value, used) = decode_performative(&frame.body).unwrap();
            let AmqpExtValue::List(fields) = value else {
                panic!("a performative is a list");
            };
            (code, fields, frame.body[used..].to_vec())
        }
    }

    /// A writer with session 3 begun (window 100, frames of 512 octets),
    /// and the reader of what it writes.
    pub(crate) fn writer_and_written() -> (BrokerWriter, Written) {
        let (client, server) = pair();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let writer = writer_with_session(client, 100, Arc::new(Mutex::new(Shared::default())));
        (
            writer,
            Written {
                frames: FrameReader {
                    wire: Wire::Plain(server),
                    buf: Vec::new(),
                },
            },
        )
    }

    /// A writer without a session (frames of 512 octets), and the reader of
    /// what it writes: for the daemon's link state, which begins its
    /// sessions itself.
    pub(crate) fn bare_writer_and_written() -> (BrokerWriter, Written) {
        let (client, server) = pair();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        (
            BrokerWriter {
                wire: Wire::Plain(client),
                max_frame_size: 512,
                sessions: BTreeMap::new(),
                shared: Arc::new(Mutex::new(Shared::default())),
            },
            Written {
                frames: FrameReader {
                    wire: Wire::Plain(server),
                    buf: Vec::new(),
                },
            },
        )
    }

    impl Written {
        /// The next frame with its channel: channel, descriptor, fields.
        pub(crate) fn next_on(&mut self) -> (u16, u64, Vec<AmqpExtValue>) {
            let frame = self.frames.next().unwrap();
            let (code, value, _) = decode_performative(&frame.body).unwrap();
            let AmqpExtValue::List(fields) = value else {
                panic!("a performative is a list");
            };
            (frame.channel, code, fields)
        }

        /// The next frame with its channel, as [`Written::next_on`], or
        /// `None` when the writer wrote nothing within 200 ms.
        pub(crate) fn try_next_on(&mut self) -> Option<(u16, u64, Vec<AmqpExtValue>)> {
            self.frames
                .wire
                .socket()
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let got = self.frames.next();
            self.frames
                .wire
                .socket()
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let frame = got.ok()?;
            let (code, value, _) = decode_performative(&frame.body).unwrap();
            let AmqpExtValue::List(fields) = value else {
                panic!("a performative is a list");
            };
            Some((frame.channel, code, fields))
        }

        /// Whether the writer wrote nothing more within 200 ms.
        pub(crate) fn nothing_more(&mut self) -> bool {
            let socket = self.frames.wire.socket();
            socket
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let got = self.frames.next();
            self.frames
                .wire
                .socket()
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            got.is_err()
        }
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
    fn amqpbridge_4_3_a_message_above_the_frame_size_is_split_and_reassembled() {
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
            txn_id_limit: TxnId::MAX_LEN,
        };
        match reader.next_event().unwrap() {
            Inbound::Transfer {
                channel,
                remote_handle,
                delivery_id,
                settled,
                message: got,
                state,
            } => {
                assert_eq!(
                    (channel, remote_handle, delivery_id, settled),
                    (3, 4, 7, false)
                );
                assert_eq!(got, message);
                assert_eq!(state, None);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            shared.lock().unwrap().next_incoming_id[&3],
            frames,
            "one transfer-id per frame"
        );
    }

    fn txn(octets: &[u8]) -> TxnId {
        TxnId::new(octets).unwrap()
    }

    fn in_txn(id: &TxnId, outcome: Option<Outcome>) -> DeliveryState {
        DeliveryState::Transactional(crate::transactions::TransactionalState {
            txn_id: id.clone(),
            outcome,
        })
    }

    /// Part 4 §4.4.1: "if delivery is split across several transfer frames
    /// then all frames MUST be explicitly associated with the same
    /// transaction": a posted message of several frames carries the
    /// transactional-state on each; a message without a state carries
    /// none.
    #[test]
    fn amqp10_4_4_1_a_posted_message_names_the_transaction_on_every_frame() {
        let (mut writer, mut written) = writer_and_written();
        let message: Vec<u8> = (0..2000u32).map(|i| (i % 253) as u8).collect();
        let state = in_txn(&txn(&[0, 9]), None);
        writer
            .transfer_with_state(3, 4, b"tag", &message, false, Some(&state))
            .unwrap();
        let frames = writer.sessions[&3].next_outgoing_id;
        assert!(frames >= 4, "{frames} frames for 2000 octets at 512");
        let mut got = Vec::new();
        for i in 0..frames {
            let (code, fields, payload) = written.next();
            assert_eq!(code, descriptor::TRANSFER);
            assert_eq!(
                decode_state(fields.get(7)),
                Ok(Some(state.clone())),
                "frame {i}"
            );
            assert_eq!(
                as_bool(fields.get(5)),
                Some(i + 1 < frames),
                "more on all but the last"
            );
            got.extend(payload);
        }
        assert_eq!(got, message);
        writer.transfer(3, 4, b"plain", b"m", true).unwrap();
        let (_, fields, _) = written.next();
        assert_eq!(fields.len(), 6, "no state field");
    }

    fn symbol_list(v: Option<&AmqpExtValue>) -> Vec<String> {
        match v {
            Some(AmqpExtValue::Array(items)) => items
                .iter()
                .map(|i| match i {
                    AmqpExtValue::Symbol(s) => s.clone(),
                    other => panic!("not a symbol: {other:?}"),
                })
                .collect(),
            other => panic!("not an array: {other:?}"),
        }
    }

    fn symbol_map(v: Option<&AmqpExtValue>) -> Vec<(String, String)> {
        match v {
            Some(AmqpExtValue::Map(entries)) => entries
                .iter()
                .map(|(k, v)| match (k, v) {
                    (AmqpExtValue::Symbol(k), AmqpExtValue::Str(v)) => (k.clone(), v.clone()),
                    other => panic!("not symbol -> string: {other:?}"),
                })
                .collect(),
            other => panic!("not a map: {other:?}"),
        }
    }

    /// Daemon spec §4.1: the open carries container-id, hostname,
    /// max-frame-size, channel-max and idle-time-out from the parameters
    /// (Part 2 §2.7.1 field order), the capability in both lists and the
    /// properties as a map with symbol keys; an idle time-out of 0 is no
    /// field.
    #[test]
    fn amqpbridge_4_1_the_open_carries_the_fields_of_the_spec() {
        let mut params = ConnectParams {
            addr: "127.0.0.1:5672".into(),
            host: "127.0.0.1".into(),
            tls: None,
            sasl: SaslCredentials::Anonymous,
            container_id: "zerodds-bridge-prod-01".into(),
            hostname: Some("vhost:prod".into()),
            max_frame_size: 65_536,
            channel_max: 256,
            idle_time_out_ms: 30_000,
            properties: vec![
                ("zerodds_version".into(), "1.0".into()),
                ("zerodds_role".into(), "bridge".into()),
            ],
            capabilities: vec!["AMQP_DDS_BRIDGE".into()],
            timeout: Duration::from_secs(1),
        };
        let (code, value, _) = decode_performative(&open_body(&params).unwrap()).unwrap();
        assert_eq!(code, descriptor::OPEN);
        let AmqpExtValue::List(f) = value else {
            panic!("a list")
        };
        assert_eq!(f.len(), 10);
        assert_eq!(f[0], AmqpExtValue::Str("zerodds-bridge-prod-01".into()));
        assert_eq!(f[1], AmqpExtValue::Str("vhost:prod".into()));
        assert_eq!(f[2], AmqpExtValue::Uint(65_536));
        assert_eq!(f[3], AmqpExtValue::Ushort(256));
        assert_eq!(f[4], AmqpExtValue::Uint(30_000));
        assert_eq!((&f[5], &f[6]), (&AmqpExtValue::Null, &AmqpExtValue::Null));
        assert_eq!(symbol_list(f.get(7)), ["AMQP_DDS_BRIDGE"], "offered");
        assert_eq!(symbol_list(f.get(8)), ["AMQP_DDS_BRIDGE"], "desired");
        assert_eq!(
            symbol_map(f.get(9)),
            [
                ("zerodds_version".to_string(), "1.0".to_string()),
                ("zerodds_role".to_string(), "bridge".to_string())
            ]
        );
        params.idle_time_out_ms = 0;
        params.hostname = None;
        let (_, value, _) = decode_performative(&open_body(&params).unwrap()).unwrap();
        assert_eq!(field(&value, 4), None, "no idle time-out");
        assert_eq!(field(&value, 1), None, "no hostname");
    }

    /// Daemon spec §4.2: the attach of a sending and of a receiving link:
    /// name, handle, role, both settle modes, the node with the address and
    /// the terminus durability on the side the role names, the
    /// initial-delivery-count of a sender, the desired capability and the
    /// properties as a map with symbol keys.
    #[test]
    fn amqpbridge_4_2_the_attach_carries_name_role_node_settle_modes_properties_and_capability() {
        let (mut writer, mut written) = writer_and_written();
        let mut spec = LinkSpec {
            name: "cid/T/sender".into(),
            handle: 2,
            sender: true,
            address: "/queues/t".into(),
            snd_settle_mode: 0,
            rcv_settle_mode: 1,
            durable: 1,
            properties: vec![
                ("zerodds_topic".into(), "T".into()),
                ("zerodds_type".into(), "TType".into()),
            ],
            desired_capabilities: vec!["dds.cdr2".into()],
            coordinator: None,
        };
        let node = |v: Option<&AmqpExtValue>, code: u64| match v {
            Some(AmqpExtValue::Described { descriptor, value }) => {
                assert_eq!(*descriptor, code);
                (field(value, 0).cloned(), field(value, 1).cloned())
            }
            other => panic!("not a terminus: {other:?}"),
        };
        writer.attach(3, &spec).unwrap();
        let (code, f, _) = written.next();
        assert_eq!(code, descriptor::ATTACH);
        assert_eq!(f[0], AmqpExtValue::Str("cid/T/sender".into()));
        assert_eq!(f[1], AmqpExtValue::Uint(2));
        assert_eq!(f[2], AmqpExtValue::Boolean(false), "role sender");
        assert_eq!(
            (&f[3], &f[4]),
            (&AmqpExtValue::Ubyte(0), &AmqpExtValue::Ubyte(1))
        );
        assert_eq!(
            node(f.get(6), desc::TARGET),
            (
                Some(AmqpExtValue::Str("/queues/t".into())),
                Some(AmqpExtValue::Uint(1))
            )
        );
        assert_eq!(f[9], AmqpExtValue::Uint(0), "initial-delivery-count");
        assert_eq!(symbol_list(f.get(12)), ["dds.cdr2"]);
        assert_eq!(
            symbol_map(f.get(13)),
            [
                ("zerodds_topic".to_string(), "T".to_string()),
                ("zerodds_type".to_string(), "TType".to_string())
            ]
        );
        spec.sender = false;
        spec.name = "cid/T/receiver".into();
        writer.attach(3, &spec).unwrap();
        let (_, f, _) = written.next();
        assert_eq!(f[2], AmqpExtValue::Boolean(true), "role receiver");
        assert_eq!(
            node(f.get(5), desc::SOURCE),
            (
                Some(AmqpExtValue::Str("/queues/t".into())),
                Some(AmqpExtValue::Uint(1))
            )
        );
        assert_eq!(f[9], AmqpExtValue::Null, "a receiver has no delivery count");
    }

    /// Part 2 §2.8.3, mode second: the receiver's disposition carries the
    /// outcome unsettled; the settled form is the one of mode first.
    #[test]
    fn amqpbridge_3_a_disposition_in_mode_second_leaves_the_delivery_unsettled() {
        let (mut writer, mut written) = writer_and_written();
        writer
            .disposition_settled(3, 4, 4, false, &Outcome::Accepted)
            .unwrap();
        let (code, f, _) = written.next();
        assert_eq!(code, descriptor::DISPOSITION);
        assert_eq!(as_bool(f.first()), Some(true), "role receiver");
        assert_eq!(as_bool(f.get(3)), Some(false), "not settled");
        assert_eq!(crate::terminus::decode_outcome(f.get(4)), Outcome::Accepted);
        writer.disposition(3, 5, 5, &Outcome::Released).unwrap();
        let (_, f, _) = written.next();
        assert_eq!(as_bool(f.get(3)), Some(true), "settled");
    }

    /// Part 4 §4.2: the attach of a control link has a coordinator with
    /// the desired capabilities as its target and a source that supports
    /// the four outcomes, rejected among them; a control link the client
    /// would receive on is refused.
    #[test]
    fn amqp10_4_2_the_attach_of_a_control_link_has_a_coordinator_as_its_target() {
        let (mut writer, mut written) = writer_and_written();
        let mut spec = LinkSpec {
            name: "txn-ctl".into(),
            handle: 0,
            sender: true,
            address: String::new(),
            snd_settle_mode: 0,
            rcv_settle_mode: 0,
            durable: 0,
            properties: Vec::new(),
            desired_capabilities: Vec::new(),
            coordinator: Some(vec![TxnCapability::LocalTransactions]),
        };
        writer.attach(3, &spec).unwrap();
        let (code, fields, _) = written.next();
        assert_eq!(code, descriptor::ATTACH);
        assert_eq!(as_bool(fields.get(2)), Some(false), "role sender");
        assert_eq!(
            Coordinator::decode(fields.get(6)),
            Ok(Some(Coordinator {
                capabilities: vec![TxnCapability::LocalTransactions]
            }))
        );
        let source = Source::decode(fields.get(5))
            .expect("a source")
            .expect("not null");
        assert_eq!(source.outcomes, OUTCOME_SYMBOLS);
        assert!(source.outcomes.iter().any(|o| o == "amqp:rejected:list"));
        spec.sender = false;
        let refused = writer.attach(3, &spec).unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
    }

    /// Part 4 §4.4.2: a disposition with a transactional-state, settled
    /// or not; Part 2 §2.6.12: the settlement of the client's own
    /// deliveries. Part 4 §4.4.3: the txn-id in the properties of a flow,
    /// a properties map without it, and a flow without properties.
    #[test]
    fn amqp10_4_4_dispositions_and_flows_name_the_transaction() {
        let (mut writer, mut written) = writer_and_written();
        let id = txn(&[5]);
        let state = in_txn(&id, Some(Outcome::Accepted));
        writer
            .disposition_with_state(3, 11, 20, false, &state)
            .unwrap();
        let (code, fields, _) = written.next();
        assert_eq!(code, descriptor::DISPOSITION);
        assert_eq!(as_bool(fields.first()), Some(true), "role receiver");
        assert_eq!(
            (as_u32(fields.get(1)), as_u32(fields.get(2))),
            (Some(11), Some(20))
        );
        assert_eq!(as_bool(fields.get(3)), Some(false), "not settled");
        assert_eq!(decode_state(fields.get(4)), Ok(Some(state)));
        writer.settle(3, 7, 8).unwrap();
        let (_, fields, _) = written.next();
        assert_eq!(as_bool(fields.first()), Some(false), "role sender");
        assert_eq!(
            (as_u32(fields.get(1)), as_u32(fields.get(2))),
            (Some(7), Some(8))
        );
        assert_eq!(as_bool(fields.get(3)), Some(true), "settled");
        assert_eq!(fields.len(), 4, "no state");
        // Flows.
        writer
            .flow_in_txn(3, 2, 4, 10, true, Some(Some(&id)))
            .unwrap();
        let (code, fields, _) = written.next();
        assert_eq!(code, descriptor::FLOW);
        assert_eq!(as_u32(fields.get(4)), Some(2), "handle");
        assert_eq!(as_u32(fields.get(5)), Some(4), "delivery-count");
        assert_eq!(as_u32(fields.get(6)), Some(10), "link-credit");
        assert_eq!(as_bool(fields.get(8)), Some(true), "drain");
        assert_eq!(
            crate::transactions::flow_txn_id(&fields[10]),
            Ok(Some(id.clone()))
        );
        writer.flow_in_txn(3, 2, 4, 10, false, Some(None)).unwrap();
        let (_, fields, _) = written.next();
        assert_eq!(fields[10], AmqpExtValue::Map(Vec::new()));
        writer.flow(3, 2, 4, 10).unwrap();
        let (_, fields, _) = written.next();
        assert_eq!(fields.len(), 7, "no drain, no properties");
    }

    /// Part 4 §4.5: the reader hands on the coordinator of an attach, the
    /// state of a transfer and the state of a disposition; a
    /// transactional-state without a txn-id is a protocol error.
    #[test]
    fn amqp10_4_5_the_reader_decodes_coordinator_and_transactional_states() {
        let (client, server) = pair();
        let shared = Arc::new(Mutex::new(Shared::default()));
        shared.lock().unwrap().channel_of.insert(0, 3);
        let mut reader = BrokerReader {
            frames: FrameReader {
                wire: Wire::Plain(client),
                buf: Vec::new(),
            },
            partial: None,
            shared,
            txn_id_limit: TxnId::MAX_LEN,
        };
        let mut wire = Wire::Plain(server);
        let mut write = |code: u64, fields: Vec<AmqpExtValue>, payload: &[u8]| {
            let mut body = encode_performative(code, &AmqpExtValue::List(fields)).unwrap();
            body.extend_from_slice(payload);
            write_frame(&mut wire, FrameType::Amqp, 0, &body).unwrap();
        };
        let own = Coordinator {
            capabilities: vec![
                TxnCapability::LocalTransactions,
                TxnCapability::MultiTxnsPerSsn,
            ],
        };
        write(
            descriptor::ATTACH,
            vec![
                AmqpExtValue::Str("txn-ctl".into()),
                AmqpExtValue::Uint(5),
                AmqpExtValue::Boolean(true),
                AmqpExtValue::Null,
                AmqpExtValue::Null,
                AmqpExtValue::Null,
                own.encode(),
            ],
            &[],
        );
        match reader.next_event().unwrap() {
            Inbound::Attach {
                name,
                remote_handle,
                has_node,
                coordinator,
                ..
            } => {
                assert_eq!(
                    (name.as_str(), remote_handle, has_node),
                    ("txn-ctl", 5, true)
                );
                assert_eq!(coordinator, Some(own));
            }
            other => panic!("{other:?}"),
        }
        let id = txn(&[1]);
        write(
            descriptor::TRANSFER,
            vec![
                AmqpExtValue::Uint(6),
                AmqpExtValue::Uint(11),
                AmqpExtValue::Binary(vec![1]),
                AmqpExtValue::Uint(0),
                AmqpExtValue::Boolean(false),
                AmqpExtValue::Boolean(false),
                AmqpExtValue::Null,
                encode_state(&in_txn(&id, None)),
            ],
            b"m",
        );
        match reader.next_event().unwrap() {
            Inbound::Transfer { state, message, .. } => {
                assert_eq!(state, Some(in_txn(&id, None)));
                assert_eq!(message, b"m");
            }
            other => panic!("{other:?}"),
        }
        for state in [
            DeliveryState::Declared(id.clone()),
            in_txn(&id, Some(Outcome::Accepted)),
        ] {
            write(
                descriptor::DISPOSITION,
                vec![
                    AmqpExtValue::Boolean(true),
                    AmqpExtValue::Uint(0),
                    AmqpExtValue::Null,
                    AmqpExtValue::Boolean(true),
                    encode_state(&state),
                ],
                &[],
            );
            match reader.next_event().unwrap() {
                Inbound::Disposition {
                    outcome,
                    state: got,
                    settled,
                    ..
                } => {
                    assert_eq!(got, Some(state));
                    assert_eq!(outcome, Outcome::None, "no outcome of Part 3");
                    assert!(settled);
                }
                other => panic!("{other:?}"),
            }
        }
        write(
            descriptor::DISPOSITION,
            vec![
                AmqpExtValue::Boolean(true),
                AmqpExtValue::Uint(0),
                AmqpExtValue::Null,
                AmqpExtValue::Boolean(true),
                described(0x34, vec![]),
            ],
            &[],
        );
        let error = reader.next_event().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("txn-id"), "{error}");
    }

    /// Part 3 §3.4.5: the reader hands on the message-annotations of a
    /// `modified` outcome; another outcome has none; a field that is not a
    /// map, or a key that is not a symbol, is a protocol error.
    #[test]
    fn amqpbridge_4_5_the_reader_hands_on_the_annotations_of_modified() {
        let read = |state: AmqpExtValue| {
            let (client, server) = pair();
            let shared = Arc::new(Mutex::new(Shared::default()));
            shared.lock().unwrap().channel_of.insert(0, 3);
            let mut reader = BrokerReader {
                frames: FrameReader {
                    wire: Wire::Plain(client),
                    buf: Vec::new(),
                },
                partial: None,
                shared,
                txn_id_limit: TxnId::MAX_LEN,
            };
            let body = encode_performative(
                descriptor::DISPOSITION,
                &AmqpExtValue::List(vec![
                    AmqpExtValue::Boolean(true),
                    AmqpExtValue::Uint(0),
                    AmqpExtValue::Null,
                    AmqpExtValue::Boolean(true),
                    state,
                ]),
            )
            .unwrap();
            write_frame(&mut Wire::Plain(server), FrameType::Amqp, 0, &body).unwrap();
            match reader.next_event() {
                Ok(Inbound::Disposition {
                    outcome,
                    annotations,
                    ..
                }) => Ok((outcome, annotations)),
                Ok(other) => panic!("{other:?}"),
                Err(e) => Err(e.to_string()),
            }
        };
        let entry = (
            AmqpExtValue::Symbol("x-opt-reason".into()),
            AmqpExtValue::Str("full".into()),
        );
        let modified = |annotations: AmqpExtValue| {
            described(
                desc::MODIFIED,
                vec![
                    AmqpExtValue::Boolean(true),
                    AmqpExtValue::Boolean(false),
                    annotations,
                ],
            )
        };
        assert_eq!(
            read(modified(AmqpExtValue::Map(vec![entry.clone()]))),
            Ok((
                Outcome::Modified {
                    delivery_failed: true,
                    undeliverable_here: false
                },
                vec![entry]
            ))
        );
        assert_eq!(
            read(described(desc::RELEASED, vec![])),
            Ok((Outcome::Released, vec![]))
        );
        let error = read(modified(AmqpExtValue::Uint(1))).unwrap_err();
        assert!(error.contains("not a map"), "{error}");
        let error = read(modified(AmqpExtValue::Map(vec![(
            AmqpExtValue::Str("k".into()),
            AmqpExtValue::Null,
        )])))
        .unwrap_err();
        assert!(error.contains("not a symbol"), "{error}");
    }

    /// Part 4 §4.5.4: a `declared` with a txn-id of 36 octets (what
    /// Apache ActiveMQ Artemis sends) is a protocol error of the reader by
    /// default; after `set_txn_id_limit(36)` the reader takes it, and one
    /// octet more is an error again.
    #[test]
    fn amqp10_4_5_4_the_reader_takes_longer_txn_ids_only_when_told() {
        let read = |octets: usize, limit: Option<usize>| {
            let (client, server) = pair();
            let shared = Arc::new(Mutex::new(Shared::default()));
            shared.lock().unwrap().channel_of.insert(0, 3);
            let mut reader = BrokerReader {
                frames: FrameReader {
                    wire: Wire::Plain(client),
                    buf: Vec::new(),
                },
                partial: None,
                shared,
                txn_id_limit: TxnId::MAX_LEN,
            };
            if let Some(limit) = limit {
                reader.set_txn_id_limit(limit);
            }
            let declared = described(0x33, vec![AmqpExtValue::Binary(vec![b'a'; octets])]);
            let body = encode_performative(
                descriptor::DISPOSITION,
                &AmqpExtValue::List(vec![
                    AmqpExtValue::Boolean(true),
                    AmqpExtValue::Uint(0),
                    AmqpExtValue::Null,
                    AmqpExtValue::Boolean(true),
                    declared,
                ]),
            )
            .unwrap();
            write_frame(&mut Wire::Plain(server), FrameType::Amqp, 0, &body).unwrap();
            match reader.next_event() {
                Ok(Inbound::Disposition {
                    state: Some(DeliveryState::Declared(id)),
                    ..
                }) => Ok(id.as_bytes().len()),
                Ok(other) => panic!("{other:?}"),
                Err(e) => Err(e.to_string()),
            }
        };
        assert_eq!(read(32, None), Ok(32));
        let error = read(36, None).unwrap_err();
        assert!(error.contains("36 octets, more than 32"), "{error}");
        assert_eq!(read(36, Some(36)), Ok(36));
        let error = read(37, Some(36)).unwrap_err();
        assert!(error.contains("37 octets, more than 36"), "{error}");
        // A bound below the one of the specification keeps 32.
        assert_eq!(read(32, Some(4)), Ok(32));
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
