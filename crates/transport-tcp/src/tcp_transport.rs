// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `TcpTransport`: ZeroDDS TCP Transport 1.0 implementation.
//!
//! # Implemented
//!
//! - Length-prefix framing (4-byte BE, 16 MiB cap, see [`framing`]); the
//!   frame body is the RTPS message as on UDP.
//! - The dialect [`TcpDialect::Rtps`] (§2.5): no handshake, RTPS messages
//!   back to back, each delimited by the messageLength of its
//!   HeaderExtension (see [`rtps_stream`]).
//! - The dialect [`TcpDialect::Cyclone`] (§4.1): the stream of Cyclone
//!   DDS, with its length submessage behind each RTPS header (see
//!   [`cyclone_stream`]).
//! - The dialect [`TcpDialect::FastDds`] (§6.1): the TCP protocol of Fast
//!   DDS, RTCP, with its 14-octet header in front of each message, the
//!   bind of the connection and the logical ports opened on it (see
//!   [`rtcp`]). A destination whose locator of kind 4 or 8
//!   names a logical port is reached in it, whatever the dialect of the
//!   transport.
//! - A listener finds the dialect of a connection at its first octets
//!   (§4.2) and can be restricted to some dialects.
//! - Connections are used in both directions (§4.1): a thread reads each
//!   outgoing connection, and a send to the address an accepted
//!   connection came from is written on that connection.
//! - ZeroDDS handshake from [`handshake`] (BindConnection request/
//!   response), mandatory in the dialect `zerodds`. The listener answers
//!   Reject for a vendor that is not on its list (§3.3), a reserved flag
//!   (§3.5), a logical port that is held (§3.6) and a connection beyond
//!   its limit (§3.7); an outgoing connection claims the port of the own
//!   listener, and the frames of a claimed connection carry it as their
//!   source.
//! - TCP connection pool (`MAX_PEERS=256`) with lazy connect,
//!   exponential backoff (50 ms → 5 s) and a reconnect when the peer has
//!   closed the connection; such a connection is counted
//!   (`peer_closed_connections`).
//! - Bounded inbound queue with a Condvar-blocking `recv`.
//! - `Transport` trait impl for polymorphism with UDP.
//! - One deadline for the whole handshake on both sides, with a
//!   per-syscall slice on the accepting side.
//! - Deterministic pool eviction (lowest-key) on cap reach — a
//!   deliberate choice over LRU, since no IndexMap dep is needed and
//!   with `MAX_PEERS=256` there are no eviction-pressure problems.
//!
//! # Spec status
//!
//! See the `lib.rs` header and
//! `docs/spec-coverage/zerodds-tcp-transport-1.0.md`. DDSI-RTPS 2.5
//! defines no TCP mapping; the locator kinds are in its vendor range
//! (§9.3.2.4), and framing, handshake and connection pool are
//! ZeroDDS-specific.
//!
//! [`framing`]: crate::framing
//! [`handshake`]: crate::handshake

use std::io::{BufReader, BufWriter, Read, Write};
use std::net::{
    IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, TcpListener, TcpStream,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

extern crate alloc;
use alloc::collections::{BTreeMap, VecDeque};

use zerodds_rtps::wire_types::{Locator, TcpKindSet};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

use crate::cyclone_stream;
use crate::framing::{FramingError, read_exact_or_eof, read_frame, read_head, write_frame};
use crate::handshake::{
    HandshakeError, RejectReason, ResponseStatus, ServerPolicy, client_handshake,
    server_handshake_with,
};
use crate::rtcp;
use crate::rtps_stream;

/// Construction and operation error of a `TcpTransport`.
#[derive(Debug)]
#[non_exhaustive]
pub enum TcpTransportError {
    /// Binding the listener failed.
    Bind(std::io::Error),
    /// Accept on the listener failed.
    Accept(std::io::Error),
    /// `set_read_timeout`/`set_nonblocking` failed.
    SetTimeout(std::io::Error),
    /// Locator is not a TCP locator.
    UnsupportedLocator,
    /// Peer sent a frame larger than [`crate::framing::MAX_FRAME_SIZE`].
    FrameTooLarge {
        /// Announced length from the frame header.
        announced: u32,
    },
    /// Peer I/O error during `accept_one`.
    PeerIo(std::io::Error),
    /// The handshake of `zerodds-tcp-transport-1.0` §3 failed. The sender
    /// is probably not a ZeroDDS TCP peer or has an incompatible
    /// protocol version.
    Handshake(HandshakeError),
    /// A connection of the dialect [`TcpDialect::Rtps`] carried what is
    /// no message delimited by its HeaderExtension (§2.5).
    StreamFraming(FramingError),
    /// A connection of the dialect [`TcpDialect::Rtps`] carried a message
    /// of a vendor this listener does not take (§3.3). The dialect has no
    /// handshake to refuse it with; the connection is closed.
    VendorNotAccepted {
        /// The vendor id of the RTPS header.
        vendor: [u8; 2],
    },
    /// A connection began in a dialect this listener does not take
    /// (§4.2, [`TcpTransport::with_accepted_dialects`]). The connection
    /// is closed.
    DialectNotAccepted {
        /// The dialect found at the first octets of the connection.
        dialect: TcpDialect,
    },
    /// A connection of the dialect [`TcpDialect::FastDds`] failed (§6.1):
    /// a bind that was refused or not answered in time, a message that is
    /// none of RTCP, a checksum that does not match.
    Rtcp(rtcp::RtcpError),
}

/// How the connections of a transport are opened and their messages
/// delimited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TcpDialect {
    /// The handshake of §3, then frames with a four-octet length in front
    /// (§2.1). The default.
    #[default]
    ZeroDds,
    /// No handshake and no length in front: RTPS messages back to back,
    /// each delimited by the messageLength of its HeaderExtension
    /// (DDSI-RTPS 2.5 §8.3.3; §2.5).
    Rtps,
    /// The stream of Cyclone DDS (`ddsi_tcp`; §4.1): no handshake and no
    /// length in front, RTPS messages back to back, each with the
    /// submessage `0x81` behind its RTPS header, whose field counts the
    /// whole message. The transport puts that submessage into a message
    /// it writes and takes it out of one it reads
    /// ([`cyclone_stream`]).
    Cyclone,
    /// The TCP protocol of Fast DDS (RTCP; §6.1): a 14-octet header with
    /// the logical port in front of each message, a BindConnectionRequest
    /// at the start of a connection and an OpenLogicalPortRequest before
    /// the first message to a logical port ([`rtcp`]). Its locators are of
    /// the kinds 4 and 8 and hold the logical port in the upper 16 bits of
    /// their port.
    FastDds,
}

impl TcpDialect {
    /// Every dialect, in the order of their names in a setting.
    pub const ALL: [Self; 4] = [Self::ZeroDds, Self::Rtps, Self::Cyclone, Self::FastDds];

    /// The name of the dialect in a setting (`transport.tcp_dialect`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ZeroDds => "zerodds",
            Self::Rtps => "rtps",
            Self::Cyclone => "cyclone",
            Self::FastDds => "fastdds",
        }
    }

    /// The dialect a setting names.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|dialect| dialect.name() == name)
    }

    /// The largest message a `send` takes in this dialect: the frame
    /// limit of §2.3, in the dialect `cyclone` what Cyclone DDS reads
    /// from a stream, without the eight octets the transport adds (§4.1),
    /// in the dialect `fastdds` the 65500 octets of Fast DDS's default
    /// `maxMessageSize` (§6.1).
    #[must_use]
    pub fn max_message_size(self) -> usize {
        match self {
            Self::ZeroDds | Self::Rtps => crate::framing::MAX_FRAME_SIZE as usize,
            Self::Cyclone => cyclone_stream::MAX_MESSAGE_SIZE,
            Self::FastDds => rtcp::MAX_MESSAGE_SIZE,
        }
    }

    /// Writes one message to a connection of this dialect. A connection
    /// of the dialect `fastdds` is written through its [`rtcp::Link`],
    /// which knows the logical port; this function refuses it.
    fn write<W: Write>(self, w: &mut W, data: &[u8]) -> Result<(), FramingError> {
        match self {
            Self::ZeroDds => write_frame(w, data),
            Self::Rtps => rtps_stream::write_message(w, data),
            Self::Cyclone => cyclone_stream::write_message(w, data),
            Self::FastDds => Err(FramingError::Io(std::io::Error::other(
                "a connection of the dialect fastdds is written through its link",
            ))),
        }
    }

    /// Reads one message from a connection of this dialect. A connection
    /// of the dialect `fastdds` is read by its [`rtcp::Link`], which
    /// answers its control messages; this function refuses it.
    fn read<R: Read>(self, r: &mut R) -> Result<Vec<u8>, FramingError> {
        match self {
            Self::ZeroDds => read_frame(r),
            Self::Rtps => rtps_stream::read_message(r),
            Self::Cyclone => cyclone_stream::read_message(r),
            Self::FastDds => Err(FramingError::Io(std::io::Error::other(
                "a connection of the dialect fastdds is read by its link",
            ))),
        }
    }

    /// Whether `data` can go on a connection of this dialect, as far as
    /// that is known before a connection is: the size limit is checked by
    /// the caller.
    fn check(self, data: &[u8]) -> Result<(), FramingError> {
        match self {
            Self::ZeroDds | Self::FastDds => Ok(()),
            Self::Rtps => rtps_stream::check_message(data),
            Self::Cyclone => cyclone_stream::check_message(data),
        }
    }
}

/// The text of the `SendError::Io` for a message a dialect cannot carry.
fn unfit_message(e: &FramingError) -> &'static str {
    match e {
        FramingError::LengthMismatch { .. } => {
            "tcp message states a HeaderExtension messageLength that is not its length"
        }
        FramingError::NotRtps { .. } => "tcp message does not begin with an RTPS header",
        _ => "tcp message without a HeaderExtension messageLength",
    }
}

/// Detailed reason why a locator is not usable for TCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InvalidLocator {
    /// The kind is no TCP kind of either set (`TcpKindSet`), or not the
    /// one of the address family asked for.
    WrongKind,
    /// Port field > u16::MAX (the spec allows a u32 port, but IPv4 TCP only u16).
    PortOverflow {
        /// The u32 port value read.
        port: u32,
    },
}

impl core::fmt::Display for TcpTransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bind(e) => write!(f, "tcp bind failed: {e}"),
            Self::Accept(e) => write!(f, "tcp accept failed: {e}"),
            Self::SetTimeout(e) => write!(f, "tcp set_timeout failed: {e}"),
            Self::UnsupportedLocator => f.write_str("tcp: locator is not a TCP locator"),
            Self::FrameTooLarge { announced } => {
                write!(f, "peer announced frame of {announced} bytes (> cap)")
            }
            Self::PeerIo(e) => write!(f, "tcp peer i/o error: {e}"),
            Self::Handshake(e) => write!(f, "tcp handshake failed: {e}"),
            Self::StreamFraming(e) => write!(f, "tcp stream of RTPS messages: {e}"),
            Self::VendorNotAccepted { vendor } => {
                write!(f, "tcp peer of vendor {vendor:02x?} is not accepted")
            }
            Self::DialectNotAccepted { dialect } => {
                write!(
                    f,
                    "tcp peer speaks the dialect {}, which this listener does not take",
                    dialect.name()
                )
            }
            Self::Rtcp(e) => write!(f, "tcp connection of the dialect fastdds: {e}"),
        }
    }
}

impl std::error::Error for TcpTransportError {}

// ---------------------------------------------------------------------
// Resource limits
// ---------------------------------------------------------------------

/// Maximum number of datagrams in the inbound queue (Finding B8/#3).
pub const MAX_INBOUND_QUEUE: usize = 1024;

/// Maximum number of peers in the connection pool (Finding B8/#4). When
/// the pool is full and a new destination comes, the entry with the lowest
/// socket address is removed.
pub const MAX_PEERS: usize = 256;

/// The number of addresses whose answer [`TcpTransport::host_alias`] keeps
/// (§6.1): one per peer of the pool.
const MAX_HOST_ADDRESS_ANSWERS: usize = MAX_PEERS;

/// Backoff start value on reconnect.
const INITIAL_BACKOFF_MS: u64 = 50;
/// The least backoff after a Reject with `ResourceLimit` (§3.7): the
/// listener is full, and a place frees when one of its peers leaves, not
/// within 50 ms.
const RESOURCE_LIMIT_BACKOFF_MS: u64 = 1_000;
/// Backoff cap (5 s).
const MAX_BACKOFF_MS: u64 = 5_000;

/// How often a send looks up the pool entry of a destination anew when
/// the one it found, of another dialect, was replaced by another send
/// meanwhile (§6.1).
const PEER_ENTRY_ATTEMPTS: usize = 8;

/// Bound of an outgoing connection's `connect`.
const CLIENT_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Bound of the client side of the handshake, request and response
/// together (§3.8).
const CLIENT_HANDSHAKE_BUDGET: Duration = Duration::from_secs(2);

/// Bound of the accepting side of the handshake, request and response
/// together (§3.8). A peer on the same host answers in under 10 ms, one on
/// a LAN in under 100 ms.
const SERVER_HANDSHAKE_BUDGET: Duration = Duration::from_secs(2);

/// Bound of one read or write of the accepting side of the handshake: a
/// peer that sends nothing for this long is given up on before the budget
/// ends.
const SERVER_HANDSHAKE_SLICE: Duration = Duration::from_millis(200);

/// Bound of the exchange with a connection beyond the peer limit (§3.7):
/// its request is read and the Reject written within this time, or the
/// connection is closed without an answer.
const REFUSE_BUDGET: Duration = Duration::from_millis(250);

/// Connections beyond the peer limit that wait for their Reject at a time
/// (§3.7). One more is closed without an answer.
const REFUSE_QUEUE: usize = 16;

/// How long after it was accepted a connection beyond the peer limit is
/// answered at the latest (§3.7). One thread answers the waiting
/// connections in order, and each silent one ahead costs
/// [`REFUSE_BUDGET`]. The client gives up on the handshake after
/// [`CLIENT_HANDSHAKE_BUDGET`]; a Reject written after that reaches
/// nobody, and the listener would count it as an answer. A connection
/// whose turn comes later than this is closed without an answer.
const REFUSE_DEADLINE: Duration = Duration::from_millis(1_500);

// The Reject is written before the client stops waiting for it.
const _: () = assert!(REFUSE_DEADLINE.as_millis() < CLIENT_HANDSHAKE_BUDGET.as_millis());

/// A connection beyond the peer limit on its way to the refusing thread:
/// the stream and the instant its answer is due by ([`REFUSE_DEADLINE`]).
type Refused = (TcpStream, Instant);

/// Keepalive of an accepted connection (§3.6): a peer that vanished
/// without a FIN holds its logical-port claim until the kernel gives the
/// connection up, after `IDLE + PROBES * INTERVAL` without an answer.
const KEEPALIVE_IDLE: Duration = Duration::from_secs(10);
/// See [`KEEPALIVE_IDLE`].
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "windows",
))]
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(5);
/// See [`KEEPALIVE_IDLE`].
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
))]
const KEEPALIVE_PROBES: u32 = 3;

/// Backlog of a listener made through `socket2`
/// ([`TcpTransport::bind_v6_only`]): the value `std::net::TcpListener`
/// asks for.
const LISTEN_BACKLOG: i32 = 128;

/// Bound of a write to a peer that takes no bytes.
const PEER_WRITE_STALL: Duration = Duration::from_secs(5);

/// Pause of [`TcpTransport::serve`] after a failed `accept` (out of
/// descriptors, for one): the condition lasts, so an immediate retry
/// would only spin.
const ACCEPT_ERROR_PAUSE: Duration = Duration::from_millis(50);

/// Bound of the connection with which [`TcpTransport::close`] wakes the
/// `accept` of [`TcpTransport::serve`].
const WAKE_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);

// ---------------------------------------------------------------------
// DeadlineStream
// ---------------------------------------------------------------------

/// A connection whose reads and writes share one deadline (§3.8). A
/// socket timeout bounds one syscall; a peer that sends one octet per
/// timeout would hold a 16-octet exchange for sixteen of them. Before each
/// syscall the socket timeout is set to what is left of the budget, at
/// most `slice`; once the deadline has passed, every read and write fails
/// with `TimedOut` without touching the socket.
struct DeadlineStream<'a> {
    stream: &'a TcpStream,
    deadline: Instant,
    slice: Duration,
}

impl<'a> DeadlineStream<'a> {
    fn new(stream: &'a TcpStream, budget: Duration, slice: Duration) -> Self {
        Self::until(stream, Instant::now() + budget, slice)
    }

    /// A deadline that is running already.
    fn until(stream: &'a TcpStream, deadline: Instant, slice: Duration) -> Self {
        Self {
            stream,
            deadline,
            slice,
        }
    }

    /// What the next syscall may take.
    fn next_timeout(&self) -> std::io::Result<Duration> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "handshake exceeded its total budget",
            ));
        }
        Ok(remaining.min(self.slice))
    }
}

/// A socket timeout surfaces as `WouldBlock` or `TimedOut` depending on
/// the platform; the caller sees `TimedOut`.
fn as_timed_out(e: std::io::Error) -> std::io::Error {
    match e.kind() {
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "handshake peer made no progress in time",
        ),
        _ => e,
    }
}

impl Read for DeadlineStream<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.set_read_timeout(Some(self.next_timeout()?))?;
        let mut stream = self.stream;
        stream.read(buf).map_err(as_timed_out)
    }
}

impl Write for DeadlineStream<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.set_write_timeout(Some(self.next_timeout()?))?;
        let mut stream = self.stream;
        stream.write(buf).map_err(as_timed_out)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut stream = self.stream;
        stream.flush()
    }
}

/// A stream whose first four octets have been read to find its dialect
/// (§4.2): a read hands those out before it reads the stream.
struct Prefixed<S> {
    prefix: [u8; 4],
    taken: usize,
    stream: S,
}

impl<S: Read> Read for Prefixed<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let left = &self.prefix[self.taken.min(self.prefix.len())..];
        if left.is_empty() || buf.is_empty() {
            return self.stream.read(buf);
        }
        let n = left.len().min(buf.len());
        buf[..n].copy_from_slice(&left[..n]);
        self.taken += n;
        Ok(n)
    }
}

impl<S: Write> Write for Prefixed<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

/// Whether the peer has closed `stream`, as far as this host knows: its
/// FIN or RST has arrived. A non-blocking look at the receive side that
/// takes nothing out of it, in one syscall, since it runs before every
/// write: `recv` with `MSG_PEEK`, which leaves the octet in the receive
/// buffer, and `MSG_DONTWAIT`, which makes this one call non-blocking
/// without touching the mode of the socket.
#[cfg(unix)]
fn peer_closed(stream: &TcpStream) -> bool {
    let mut probe = [core::mem::MaybeUninit::<u8>::uninit(); 1];
    match socket2::SockRef::from(stream)
        .recv_with_flags(&mut probe, libc::MSG_PEEK | libc::MSG_DONTWAIT)
    {
        Ok(0) => true,
        Ok(_) => false,
        Err(e) => !matches!(
            e.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ),
    }
}

/// Whether the peer has closed `stream`, as far as this host knows: its
/// FIN or RST has arrived. A non-blocking look at the receive side that
/// takes nothing out of it. Where a single call cannot be made
/// non-blocking, the mode of the socket is switched around the look; a
/// socket whose mode cannot be switched (or switched back) counts as
/// closed, so that it is replaced.
#[cfg(not(unix))]
fn peer_closed(stream: &TcpStream) -> bool {
    if stream.set_nonblocking(true).is_err() {
        return true;
    }
    let mut probe = [0u8; 1];
    let closed = match stream.peek(&mut probe) {
        Ok(0) => true,
        Ok(_) => false,
        Err(e) => !matches!(
            e.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ),
    };
    stream.set_nonblocking(false).is_err() || closed
}

/// Turns TCP keepalive on for an accepted connection (§3.6): idle time
/// everywhere, probe interval and probe count where the platform has
/// them.
fn set_keepalive(stream: &TcpStream) -> std::io::Result<()> {
    let keepalive = socket2::TcpKeepalive::new().with_time(KEEPALIVE_IDLE);
    #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "windows",
    ))]
    let keepalive = keepalive.with_interval(KEEPALIVE_INTERVAL);
    #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
    ))]
    let keepalive = keepalive.with_retries(KEEPALIVE_PROBES);
    socket2::SockRef::from(stream).set_tcp_keepalive(&keepalive)
}

/// A logical-port claim (§3.6): the address of the listener the
/// connection arrived at, the address of the peer, the claimed port.
type ClaimKey = (IpAddr, IpAddr, u32);

/// The claims of the live connections of one listener, by connection
/// number.
type ClaimTable = Mutex<BTreeMap<ClaimKey, u64>>;

/// The claim of one connection; released when the connection's reading
/// ends, whichever way.
struct ClaimGuard<'a> {
    claims: &'a ClaimTable,
    connection: u64,
    held: Option<ClaimKey>,
}

impl ClaimGuard<'_> {
    /// Takes `key` for this connection unless a live connection holds it.
    fn claim(&mut self, key: ClaimKey) -> bool {
        let Ok(mut claims) = self.claims.lock() else {
            return false;
        };
        if claims.contains_key(&key) {
            return false;
        }
        claims.insert(key, self.connection);
        self.held = Some(key);
        true
    }
}

impl Drop for ClaimGuard<'_> {
    fn drop(&mut self) {
        if let Some(key) = self.held.take()
            && let Ok(mut claims) = self.claims.lock()
            && claims.get(&key) == Some(&self.connection)
        {
            claims.remove(&key);
        }
    }
}

/// Counters of the outgoing side, shared by the connections of the pool.
#[derive(Debug, Default)]
struct PoolCounters {
    /// Connections a `send` found closed by the peer and replaced (§5.1).
    closed_by_peer: AtomicU64,
    /// Connect attempts a listener answered with Reject, by reason code.
    rejections: [AtomicU64; REJECT_REASONS],
}

/// Number of reason codes of §3.3.
const REJECT_REASONS: usize = 6;

/// The text of a `SendError::Io` for a send that failed on a Reject.
fn reject_message(reason: RejectReason) -> &'static str {
    match reason {
        RejectReason::VersionMismatch => "tcp peer rejected: version mismatch",
        RejectReason::ResourceLimit => "tcp peer rejected: resource limit",
        RejectReason::LogicalPortConflict => "tcp peer rejected: logical port conflict",
        RejectReason::VendorNotAccepted => "tcp peer rejected: vendor not accepted",
        RejectReason::ReservedFlagsSet => "tcp peer rejected: reserved flags set",
        RejectReason::Unknown => "tcp peer rejected: reason unknown",
    }
}

// ---------------------------------------------------------------------
// PeerConn
// ---------------------------------------------------------------------

/// Peer connection: writer side. Wrapped per peer in its own mutex so
/// that `send()` does not hold the pool lock across blocking I/O.
struct PeerConn {
    addr: SocketAddr,
    writer: Option<BufWriter<TcpStream>>,
    backoff_ms: u64,
    last_attempt: Option<std::time::Instant>,
    /// The logical port the request claims (§3.6); 0 claims none.
    claim: u32,
    /// How the connection is opened and its messages delimited.
    dialect: TcpDialect,
    /// The reason, if the last connect attempt ended in a Reject. It
    /// stands while the backoff after that Reject throttles the next
    /// attempt, and is cleared by an attempt that ends otherwise.
    last_reject: Option<RejectReason>,
    /// Where what the peer writes on this connection goes (§4.1), and
    /// the kinds of the source locator it carries. `None` reads nothing:
    /// a connection outside a transport.
    reading: Option<(Arc<Inbound>, TcpKindSet)>,
    /// Set by the thread that reads this connection when the peer has
    /// closed it or the reading failed; a new connection has a new flag.
    read_ended: Arc<AtomicBool>,
    /// What this side is in the dialect `fastdds` (§6.1): its logical
    /// ports, its listener, its vendor id.
    rtcp_own: Option<rtcp::Own>,
    /// The connection of the dialect `fastdds`, which writes with the
    /// logical port and answers the control messages.
    link: Option<Arc<rtcp::Link>>,
    /// The WAN address of a Fast DDS locator (§6.1), connected to when
    /// the address in the peer's network does not take the connection.
    wan: Option<SocketAddr>,
    /// Why the last send of the dialect `fastdds` failed, for the text of
    /// its `SendError`.
    last_rtcp_failure: Option<&'static str>,
}

/// The text of a `SendError::Io` for a send of the dialect `fastdds`
/// that failed on the protocol (§6.1).
fn rtcp_message(e: &rtcp::RtcpError) -> &'static str {
    match e {
        rtcp::RtcpError::PortRefused { .. } => "tcp fastdds peer refused the logical port",
        rtcp::RtcpError::Timeout => "tcp fastdds peer did not answer in time",
        rtcp::RtcpError::BindRefused(_) => "tcp fastdds peer refused the bind",
        rtcp::RtcpError::Closed => "tcp fastdds connection closed",
        _ => "tcp fastdds connection failed",
    }
}

impl PeerConn {
    fn new<A: Into<SocketAddr>>(addr: A) -> Self {
        let addr = addr.into();
        Self {
            addr,
            writer: None,
            backoff_ms: 0,
            last_attempt: None,
            claim: 0,
            dialect: TcpDialect::ZeroDds,
            last_reject: None,
            reading: None,
            read_ended: Arc::new(AtomicBool::new(false)),
            rtcp_own: None,
            link: None,
            wan: None,
            last_rtcp_failure: None,
        }
    }

    /// A connection of the dialect `fastdds` whose side is `own` (§6.1).
    fn rtcp(mut self, own: rtcp::Own) -> Self {
        self.rtcp_own = Some(own);
        self
    }

    /// A connection that is tried at `wan` when the address does not take
    /// it (§6.1).
    fn with_wan(mut self, wan: Option<SocketAddr>) -> Self {
        self.wan = wan;
        self
    }

    /// A connection whose incoming side is read into `inbound` (§4.1):
    /// what the peer writes on it reaches the inbound queue with the
    /// address of the peer as its source, in the locator kinds `kinds`.
    fn reading_into(mut self, inbound: Arc<Inbound>, kinds: TcpKindSet) -> Self {
        self.reading = Some((inbound, kinds));
        self
    }

    /// Starts the thread (`zdds-tcp-conn`) that reads what the peer
    /// writes on the connection just made. It ends when the peer closes
    /// the connection or this side shuts it down.
    fn start_reading(&mut self, stream: &TcpStream) -> std::io::Result<()> {
        let Some((inbound, kinds)) = self.reading.clone() else {
            return Ok(());
        };
        let read_side = stream.try_clone()?;
        let dialect = self.dialect;
        let source = socket_locator(self.addr, kinds);
        let ended = Arc::new(AtomicBool::new(false));
        self.read_ended = Arc::clone(&ended);
        std::thread::Builder::new()
            .name(String::from("zdds-tcp-conn"))
            .spawn(move || {
                let mut reader = BufReader::new(read_side);
                if inbound.pump(&mut reader, dialect, source, None).is_err() {
                    inbound.connection_errors.fetch_add(1, Ordering::Relaxed);
                }
                ended.store(true, Ordering::Release);
            })?;
        Ok(())
    }

    /// Starts the thread (`zdds-tcp-conn`) that reads the connection of
    /// the dialect `fastdds` just bound (§6.1): it answers the control
    /// messages of the peer and hands each RTPS message for an open
    /// logical port of this side to the inbound queue. A link outside a
    /// transport is read as well, into nothing, since its control
    /// messages need answers.
    fn start_reading_link(
        &mut self,
        link: &Arc<rtcp::Link>,
        mut reader: BufReader<TcpStream>,
    ) -> std::io::Result<()> {
        let reading = self.reading.clone();
        let physical = self.addr;
        let link = Arc::clone(link);
        let ended = Arc::new(AtomicBool::new(false));
        self.read_ended = Arc::clone(&ended);
        std::thread::Builder::new()
            .name(String::from("zdds-tcp-conn"))
            .spawn(move || {
                let unread = rtcp::Counters::default();
                let result = match &reading {
                    Some((inbound, _)) => link.run(&mut reader, &inbound.rtcp, |port, data| {
                        inbound.push(ReceivedDatagram {
                            source: rtcp_source(physical, port),
                            data: data.into(),
                        });
                    }),
                    None => link.run(&mut reader, &unread, |_, _| {}),
                };
                if result.is_err()
                    && let Some((inbound, _)) = &reading
                {
                    inbound.connection_errors.fetch_add(1, Ordering::Relaxed);
                }
                ended.store(true, Ordering::Release);
            })?;
        Ok(())
    }

    /// Whether the peer has closed the connection `writer` writes on.
    /// The thread that reads the connection has seen the close when its
    /// flag is set; the look at the socket sees a close that thread has
    /// not come to yet.
    fn closed_by_peer(&self, writer: &BufWriter<TcpStream>) -> bool {
        if self.read_ended.load(Ordering::Acquire)
            || self.link.as_ref().is_some_and(|link| link.is_closed())
        {
            return true;
        }
        // Where the look switches the mode of the socket, it would fail
        // a read of the reading thread: the flag is all there is.
        #[cfg(not(unix))]
        if self.reading.is_some() {
            return false;
        }
        peer_closed(writer.get_ref())
    }

    /// A connection of `dialect`.
    fn speaking(mut self, dialect: TcpDialect) -> Self {
        self.dialect = dialect;
        self
    }

    /// A connection whose request claims `port` (§3.6).
    fn claiming(mut self, port: u32) -> Self {
        self.claim = port;
        self
    }

    fn ensure_connected(
        &mut self,
        counters: &PoolCounters,
    ) -> std::io::Result<&mut BufWriter<TcpStream>> {
        if self.writer.is_none() {
            if self.backoff_ms > 0
                && let Some(last) = self.last_attempt
                && last.elapsed() < Duration::from_millis(self.backoff_ms)
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "reconnect throttled by backoff",
                ));
            }
            self.last_attempt = Some(std::time::Instant::now());
            self.last_reject = None;
            let connected =
                TcpStream::connect_timeout(&self.addr, CLIENT_CONNECT_TIMEOUT).or_else(|e| {
                    match self.wan {
                        // §6.1: a Fast DDS locator with a WAN address names
                        // the host from outside its network as well.
                        Some(wan) => TcpStream::connect_timeout(&wan, CLIENT_CONNECT_TIMEOUT),
                        None => Err(e),
                    }
                });
            match connected {
                Ok(stream) => {
                    stream.set_nodelay(true)?;
                    // Bounded as on the accepting side: a listener that
                    // takes the connection and never answers held the
                    // sending thread for ever, and one that answers octet
                    // by octet for sixteen timeouts.
                    let mut bounded = DeadlineStream::new(
                        &stream,
                        CLIENT_HANDSHAKE_BUDGET,
                        CLIENT_HANDSHAKE_BUDGET,
                    );
                    // ZeroDDS handshake (see the crate::handshake module doc).
                    // The dialects `rtps` (§2.5) and `cyclone` (§4.1) have
                    // none: the first octets on the connection are those
                    // of the first message.
                    if self.dialect == TcpDialect::ZeroDds
                        && let Err(e) = client_handshake(&mut bounded, self.claim)
                    {
                        self.bump_backoff();
                        if let HandshakeError::Rejected { reason } = e {
                            // §3.7: the Reject is told from a connection
                            // error: it is counted by its reason, the send
                            // names it, and a full listener is asked again
                            // after a second at the earliest.
                            self.last_reject = Some(reason);
                            if let Some(count) = usize::try_from(reason.as_code())
                                .ok()
                                .and_then(|code| counters.rejections.get(code))
                            {
                                count.fetch_add(1, Ordering::Relaxed);
                            }
                            if reason == RejectReason::ResourceLimit {
                                self.backoff_ms = self.backoff_ms.max(RESOURCE_LIMIT_BACKOFF_MS);
                            }
                        }
                        return Err(std::io::Error::other(format!(
                            "tcp handshake client-side failed: {e}"
                        )));
                    }
                    stream.set_read_timeout(None)?;
                    // A peer that stops reading fails the send after this
                    // long without progress; the connection is dropped and
                    // made anew with backoff.
                    stream.set_write_timeout(Some(PEER_WRITE_STALL))?;
                    if self.dialect == TcpDialect::FastDds {
                        // §6.1: the BindConnectionRequest and its answer,
                        // under the budget of the handshake.
                        let own = self.rtcp_own.clone().ok_or_else(|| {
                            std::io::Error::other("tcp fastdds connection without its side")
                        })?;
                        let unread = rtcp::Counters::default();
                        let counters = self
                            .reading
                            .as_ref()
                            .map_or(&unread, |(inbound, _)| &inbound.rtcp);
                        let deadline = Instant::now() + CLIENT_HANDSHAKE_BUDGET;
                        let (link, reader) =
                            match rtcp::Link::connect(&stream, own, counters, deadline) {
                                Ok(bound) => bound,
                                Err(e) => {
                                    self.bump_backoff();
                                    self.last_rtcp_failure = Some(rtcp_message(&e));
                                    let _ = stream.shutdown(std::net::Shutdown::Both);
                                    return Err(std::io::Error::other(format!(
                                        "tcp fastdds bind failed: {e}"
                                    )));
                                }
                            };
                        let link = Arc::new(link);
                        if let Err(e) = self.start_reading_link(&link, reader) {
                            self.bump_backoff();
                            return Err(e);
                        }
                        self.link = Some(link);
                    } else if let Err(e) = self.start_reading(&stream) {
                        // §4.1: the peer may answer on this connection.
                        // Without the thread that reads it those answers
                        // were lost unseen, so a connection that cannot be
                        // read is none.
                        self.bump_backoff();
                        return Err(e);
                    }
                    self.writer = Some(BufWriter::new(stream));
                    self.backoff_ms = 0;
                }
                Err(e) => {
                    self.bump_backoff();
                    return Err(e);
                }
            }
        }
        self.writer
            .as_mut()
            .ok_or_else(|| std::io::Error::other("writer missing after connect"))
    }

    fn bump_backoff(&mut self) {
        self.backoff_ms = if self.backoff_ms == 0 {
            INITIAL_BACKOFF_MS
        } else {
            (self.backoff_ms.saturating_mul(2)).min(MAX_BACKOFF_MS)
        };
    }

    fn drop_writer(&mut self) {
        if let Some(link) = self.link.take() {
            link.shutdown();
        }
        if let Some(w) = self.writer.take() {
            // Signal FIN quickly instead of waiting for Drop.
            if let Ok(stream) = w.into_inner() {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
    }

    /// Writes one frame, over a new connection if there is none or the
    /// peer has closed the one there was; such a connection is counted in
    /// `counters.closed_by_peer`.
    fn send(
        &mut self,
        data: &[u8],
        logical_port: Option<u16>,
        counters: &PoolCounters,
    ) -> Result<(), FramingError> {
        // §6.1: a logical port is addressed in the dialect `fastdds` only;
        // a connection of another dialect would write the message without
        // it.
        if logical_port.is_some() && self.dialect != TcpDialect::FastDds {
            return Err(FramingError::Io(std::io::Error::other(
                "tcp logical port on a connection of another dialect than fastdds",
            )));
        }
        // A connection the peer has closed takes one more write without an
        // error and loses it. The close is no failed connect: no backoff,
        // this call connects anew. What was written into the connection
        // before its close arrived here may not have reached the peer, and
        // those sends returned `Ok`: the count is what shows it (§5.1).
        if self
            .writer
            .as_ref()
            .is_some_and(|writer| self.closed_by_peer(writer))
        {
            counters.closed_by_peer.fetch_add(1, Ordering::Relaxed);
            self.drop_writer();
        }
        let dialect = self.dialect;
        self.last_rtcp_failure = None;
        let writer = self.ensure_connected(counters)?;
        if dialect == TcpDialect::FastDds {
            let (Some(link), Some(port)) = (self.link.clone(), logical_port) else {
                return Err(FramingError::Io(std::io::Error::other(
                    "tcp fastdds send without a link or a logical port",
                )));
            };
            return match link.send(port, data, CLIENT_HANDSHAKE_BUDGET) {
                Ok(()) => Ok(()),
                // The peer refused the port or has not answered yet: the
                // connection stands.
                Err(e @ (rtcp::RtcpError::PortRefused { .. } | rtcp::RtcpError::Timeout)) => {
                    self.last_rtcp_failure = Some(rtcp_message(&e));
                    Err(FramingError::Io(std::io::Error::other(e.to_string())))
                }
                Err(e) => {
                    self.last_rtcp_failure = Some(rtcp_message(&e));
                    self.bump_backoff();
                    self.drop_writer();
                    Err(FramingError::Io(std::io::Error::other(e.to_string())))
                }
            };
        }
        if let Err(e) = dialect.write(writer, data) {
            self.bump_backoff();
            self.drop_writer();
            return Err(e);
        }
        if let Err(e) = writer.flush() {
            self.bump_backoff();
            self.drop_writer();
            return Err(FramingError::Io(e));
        }
        Ok(())
    }
}

impl Drop for PeerConn {
    /// The thread that reads the connection holds a clone of the stream:
    /// only a shutdown ends the connection and that thread with it.
    fn drop(&mut self) {
        self.drop_writer();
    }
}

impl core::fmt::Debug for PeerConn {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PeerConn").finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------
// Inbound-Queue (bounded, Condvar-wakeup)
// ---------------------------------------------------------------------

#[derive(Debug, Default)]
struct InboundState {
    queue: VecDeque<ReceivedDatagram>,
    /// Total number of frames dropped due to overflow.
    dropped: u64,
}

/// What the connections of a transport deliver. Shared with the threads
/// that read the outgoing connections, which outlive a `send`.
#[derive(Debug, Default)]
struct Inbound {
    state: Mutex<InboundState>,
    cv: Condvar,
    /// Frames and messages a peer began and did not finish (§2.4).
    truncated_frames: AtomicU64,
    /// Connections whose reading ended in an error.
    connection_errors: AtomicU64,
    /// Counts of the connections of the dialect `fastdds` (§6.1).
    rtcp: rtcp::Counters,
}

impl Inbound {
    fn push(&self, dg: ReceivedDatagram) {
        if let Ok(mut st) = self.state.lock() {
            if st.queue.len() >= MAX_INBOUND_QUEUE {
                // Over cap: drop the oldest frame (FIFO drop instead of
                // discarding the new one — old frames are mostly
                // re-requested anyway by the reliable reader layer via
                // ACKNACK).
                st.queue.pop_front();
                st.dropped = st.dropped.saturating_add(1);
            }
            st.queue.push_back(dg);
            self.cv.notify_one();
        }
    }

    /// Takes what one read of a connection of `dialect` returned: a
    /// message goes to the inbound queue with `source`, and `Ok(true)`
    /// asks for the next read; `Ok(false)` is the regular end of the
    /// connection. In the dialects without a request the vendor id of
    /// each RTPS header is held against `vendors` (§3.3).
    fn take(
        &self,
        read: Result<Vec<u8>, FramingError>,
        dialect: TcpDialect,
        source: Locator,
        vendors: Option<&[[u8; 2]]>,
    ) -> Result<bool, TcpTransportError> {
        match read {
            Ok(data) => {
                if dialect != TcpDialect::ZeroDds
                    && let Some(accepted) = vendors
                    && let Some(vendor) = data.get(6..8).and_then(|v| v.first_chunk::<2>())
                    && !accepted.contains(vendor)
                {
                    return Err(TcpTransportError::VendorNotAccepted { vendor: *vendor });
                }
                self.push(ReceivedDatagram {
                    source,
                    data: data.into(),
                });
                Ok(true)
            }
            Err(FramingError::Closed) => Ok(false),
            Err(FramingError::UnexpectedEof) => {
                self.truncated_frames.fetch_add(1, Ordering::Relaxed);
                Err(TcpTransportError::PeerIo(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    match dialect {
                        TcpDialect::ZeroDds => "peer closed the connection inside a frame",
                        _ => "peer closed the connection inside a message",
                    },
                )))
            }
            Err(FramingError::FrameTooLarge { announced }) => {
                Err(TcpTransportError::FrameTooLarge { announced })
            }
            Err(FramingError::Io(e)) => Err(TcpTransportError::PeerIo(e)),
            Err(other) => Err(TcpTransportError::StreamFraming(other)),
        }
    }

    /// Reads a connection of `dialect` into the inbound queue until it
    /// ends.
    fn pump<R: Read>(
        &self,
        r: &mut R,
        dialect: TcpDialect,
        source: Locator,
        vendors: Option<&[[u8; 2]]>,
    ) -> Result<(), TcpTransportError> {
        while self.take(dialect.read(r), dialect, source, vendors)? {}
        Ok(())
    }
}

/// An accepted connection a `send` to its source writes on (§4.1): the
/// peer connected from an address nothing listens at, and expects the
/// answers on the connection it opened.
#[derive(Debug)]
struct AnswerConn {
    stream: TcpStream,
    dialect: TcpDialect,
    /// The link of a connection of the dialect `fastdds` (§6.1).
    link: Option<Arc<rtcp::Link>>,
}

/// The accepted connections that take answers, by the address their peer
/// connected from, each with its connection number.
type AnswerTable = Mutex<BTreeMap<SocketAddr, (u64, Arc<Mutex<AnswerConn>>)>>;

/// The entry of one accepted connection in the [`AnswerTable`]; removed
/// when the connection's reading ends, whichever way.
struct AnswerGuard<'a> {
    table: &'a AnswerTable,
    addr: SocketAddr,
    connection: u64,
}

impl Drop for AnswerGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut table) = self.table.lock()
            && table
                .get(&self.addr)
                .is_some_and(|(id, _)| *id == self.connection)
        {
            table.remove(&self.addr);
        }
    }
}

// ---------------------------------------------------------------------
// TcpTransport
// ---------------------------------------------------------------------

/// TCP-based transport.
///
/// See the module doc and `docs/spec-coverage/zerodds-tcp-transport-1.0.md`
/// for spec status and wire format.
#[derive(Debug)]
pub struct TcpTransport {
    listener: TcpListener,
    local_locator: Locator,
    /// The `LocatorKind` values of the locators this transport makes: its
    /// own and the source of each received frame (§1.1).
    locator_kinds: TcpKindSet,
    /// How the connections this transport opens are opened and their
    /// messages delimited.
    dialect: TcpDialect,
    /// The dialects an accepted connection may begin in (§4.2).
    accepted_dialects: Vec<TcpDialect>,
    /// The vendor ids a connecting peer may carry in its request (§3.3);
    /// `None` accepts every vendor.
    accepted_vendors: Option<Vec<[u8; 2]>>,
    /// Accepted connections [`serve`](Self::serve) reads at a time (§3.7).
    accept_limit: usize,
    /// Whether an outgoing connection claims the port of this listener
    /// (§3.6).
    claim_logical_port: bool,
    /// The logical-port claims of the accepted connections (§3.6).
    claims: ClaimTable,
    /// Peers: each peer in its own mutex → the pool lock is only for
    /// lookup, not across blocking I/O (B8/#2).
    peers: Mutex<BTreeMap<SocketAddr, Arc<Mutex<PeerConn>>>>,
    /// The accepted connections a `send` to their source writes on
    /// (§4.1).
    answering: AnswerTable,
    inbound: Arc<Inbound>,
    /// Set by [`close`](Self::close).
    closed: AtomicBool,
    /// A clone of each stream [`serve`](Self::serve) reads, by connection
    /// number, so that `close` can shut it down.
    accepted: Mutex<BTreeMap<u64, TcpStream>>,
    next_connection: AtomicU64,
    refused_connections: AtomicU64,
    refused_unanswered: AtomicU64,
    /// Sends written on an accepted connection (§4.1).
    answered_on_accepted: AtomicU64,
    pool_counters: PoolCounters,
    /// The logical ports of this transport in the dialect `fastdds`
    /// (§6.1), the announced one first; empty takes the port of the
    /// listener.
    logical_ports: Mutex<Vec<u16>>,
    /// Whether an address is one of this host, as [`Self::host_alias`]
    /// found it.
    host_addresses: Mutex<BTreeMap<IpAddr, bool>>,
    /// The vendor id this transport names in a BindConnectionRequest of
    /// the dialect `fastdds` (§6.1).
    vendor_id: [u8; 2],
}

impl TcpTransport {
    /// Binds to the given address. `port=0` = OS-chosen.
    ///
    /// # Errors
    /// `TcpTransportError::Bind`.
    pub fn bind_v4(addr: Ipv4Addr, port: u16) -> Result<Self, TcpTransportError> {
        let bind_addr = SocketAddrV4::new(addr, port);
        let listener = TcpListener::bind(bind_addr).map_err(TcpTransportError::Bind)?;
        let local = match listener.local_addr().map_err(TcpTransportError::Bind)? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                return Err(TcpTransportError::Bind(std::io::Error::other(
                    "got V6 address on V4 bind",
                )));
            }
        };
        let local_locator = Locator::tcp_v4(local.ip().octets(), u32::from(local.port()));
        Ok(Self::with_listener(listener, local_locator))
    }

    /// Binds a TCPv6 listener socket on `addr:port`. `addr = ::1`
    /// for loopback, `addr = ::` (UNSPECIFIED) for all-interface.
    /// `port = 0` lets the OS choose the port.
    ///
    /// # Errors
    /// `TcpTransportError::Bind`.
    pub fn bind_v6(addr: Ipv6Addr, port: u16) -> Result<Self, TcpTransportError> {
        let bind_addr = SocketAddrV6::new(addr, port, 0, 0);
        let listener = TcpListener::bind(bind_addr).map_err(TcpTransportError::Bind)?;
        let local = match listener.local_addr().map_err(TcpTransportError::Bind)? {
            SocketAddr::V6(v6) => v6,
            SocketAddr::V4(_) => {
                return Err(TcpTransportError::Bind(std::io::Error::other(
                    "got V4 address on V6 bind",
                )));
            }
        };
        let local_locator = Locator::tcp_v6(local.ip().octets(), u32::from(local.port()));
        Ok(Self::with_listener(listener, local_locator))
    }

    /// Binds a TCPv6 listener socket on `addr:port` that takes IPv6
    /// connections only (`IPV6_V6ONLY`). A socket on `::` takes the IPv4
    /// connections of its port as well where the system is dual-stack,
    /// and then a TCPv4 listener cannot hold the same port; this one
    /// leaves the port's IPv4 side to it.
    ///
    /// # Errors
    /// `TcpTransportError::Bind`.
    pub fn bind_v6_only(addr: Ipv6Addr, port: u16) -> Result<Self, TcpTransportError> {
        let bind_addr = SocketAddrV6::new(addr, port, 0, 0);
        let socket = socket2::Socket::new(
            socket2::Domain::IPV6,
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )
        .map_err(TcpTransportError::Bind)?;
        socket.set_only_v6(true).map_err(TcpTransportError::Bind)?;
        // As `TcpListener::bind` does on Unix: a listener that is started
        // again takes its port while connections of the last one linger.
        #[cfg(unix)]
        socket
            .set_reuse_address(true)
            .map_err(TcpTransportError::Bind)?;
        socket
            .bind(&SocketAddr::V6(bind_addr).into())
            .map_err(TcpTransportError::Bind)?;
        socket
            .listen(LISTEN_BACKLOG)
            .map_err(TcpTransportError::Bind)?;
        let listener: TcpListener = socket.into();
        let local = match listener.local_addr().map_err(TcpTransportError::Bind)? {
            SocketAddr::V6(v6) => v6,
            SocketAddr::V4(_) => {
                return Err(TcpTransportError::Bind(std::io::Error::other(
                    "got V4 address on V6 bind",
                )));
            }
        };
        let local_locator = Locator::tcp_v6(local.ip().octets(), u32::from(local.port()));
        Ok(Self::with_listener(listener, local_locator))
    }

    fn with_listener(listener: TcpListener, local_locator: Locator) -> Self {
        Self {
            listener,
            local_locator,
            locator_kinds: TcpKindSet::ZeroDds,
            dialect: TcpDialect::ZeroDds,
            accepted_dialects: TcpDialect::ALL.to_vec(),
            accepted_vendors: None,
            accept_limit: MAX_PEERS,
            claim_logical_port: true,
            claims: Mutex::new(BTreeMap::new()),
            peers: Mutex::new(BTreeMap::new()),
            answering: Mutex::new(BTreeMap::new()),
            inbound: Arc::new(Inbound::default()),
            closed: AtomicBool::new(false),
            accepted: Mutex::new(BTreeMap::new()),
            next_connection: AtomicU64::new(0),
            refused_connections: AtomicU64::new(0),
            refused_unanswered: AtomicU64::new(0),
            answered_on_accepted: AtomicU64::new(0),
            pool_counters: PoolCounters::default(),
            logical_ports: Mutex::new(Vec::new()),
            host_addresses: Mutex::new(BTreeMap::new()),
            vendor_id: crate::handshake::VENDOR_ID_ZERODDS,
        }
    }

    /// Sets the `LocatorKind` values of the locators this transport makes
    /// (§1.1): [`TcpKindSet::ZeroDds`], the default, is `0x01F0_0004` and
    /// `0x01F0_0005`; [`TcpKindSet::Kinds4And8`] is 4 and 8, the values of
    /// Cyclone DDS and Fast DDS. The setting is this transport's: another
    /// transport of the process keeps its own. A destination is taken
    /// with either set.
    #[must_use]
    pub fn with_locator_kinds(mut self, kinds: TcpKindSet) -> Self {
        self.locator_kinds = kinds;
        self.local_locator = self.local_locator.with_tcp_kind_set(kinds);
        self
    }

    /// The `LocatorKind` values of the locators this transport makes: 4
    /// and 8 in the dialect `fastdds`, which knows no others (§6.1).
    #[must_use]
    pub fn locator_kinds(&self) -> TcpKindSet {
        if self.dialect == TcpDialect::FastDds {
            TcpKindSet::Kinds4And8
        } else {
            self.locator_kinds
        }
    }

    /// Sets the logical port of this transport in the dialect `fastdds`
    /// (§6.1): the port its locator names in the upper 16 bits, and the
    /// one an OpenLogicalPortRequest of a peer is granted for. Without
    /// this call it is the port of the listener. A runtime gives it the
    /// metatraffic unicast port of its participant, the port Fast DDS
    /// asks at for an initial peer that names no logical port.
    #[must_use]
    pub fn with_logical_port(self, port: u16) -> Self {
        self.set_logical_ports(&[port]);
        self
    }

    /// Sets the logical ports of this transport in the dialect `fastdds`
    /// (§6.1): the first is the one its locator names, and each is granted
    /// to an OpenLogicalPortRequest of a peer. A runtime gives the
    /// metatraffic and the user unicast port of its participant, the ports
    /// Fast DDS uses for a participant over TCP. The ports hold for the
    /// connections made after the call; an empty list takes the port of
    /// the listener again. Port 0, which marks a control message, is no
    /// logical port and is left out.
    pub fn set_logical_ports(&self, ports: &[u16]) {
        if let Ok(mut own) = self.logical_ports.lock() {
            own.clear();
            for port in ports {
                if *port != 0 && !own.contains(port) {
                    own.push(*port);
                }
            }
        }
    }

    /// The logical port of this transport in the dialect `fastdds`: the
    /// one its locator names.
    #[must_use]
    pub fn logical_port(&self) -> u16 {
        self.logical_ports()
            .first()
            .copied()
            .unwrap_or_else(|| self.listener_port())
    }

    /// The logical ports of this transport in the dialect `fastdds`, the
    /// one its locator names first.
    #[must_use]
    pub fn logical_ports(&self) -> Vec<u16> {
        match self.logical_ports.lock() {
            Ok(own) if !own.is_empty() => own.clone(),
            _ => alloc::vec![self.listener_port()],
        }
    }

    /// Sets the vendor id a BindConnectionRequest of the dialect `fastdds`
    /// names (§6.1); the default is ZeroDDS's, `01 f0`.
    #[must_use]
    pub fn with_vendor_id(mut self, vendor_id: [u8; 2]) -> Self {
        self.vendor_id = vendor_id;
        self
    }

    /// `addr` at the loopback address of its family when its address is
    /// one of this host (§6.1): Fast DDS takes every address of its host
    /// for one, binds one connection between two participants and answers
    /// a second bind from another address of the same host with
    /// `ServerError`. An address is one of this host when a socket can be
    /// bound to it; the answer is kept per address, for at most
    /// [`MAX_HOST_ADDRESS_ANSWERS`] addresses. The addresses come from the
    /// locators peers announce; beyond the bound the lowest address is
    /// forgotten, as the pool forgets its lowest peer, and is asked again
    /// when a send names it.
    fn host_alias(&self, addr: SocketAddr) -> SocketAddr {
        let ip = addr.ip();
        if ip.is_loopback() {
            return addr;
        }
        let local = match self.host_addresses.lock() {
            Ok(mut known) => match known.get(&ip) {
                Some(local) => *local,
                None => {
                    let local = std::net::UdpSocket::bind(SocketAddr::new(ip, 0)).is_ok();
                    if known.len() >= MAX_HOST_ADDRESS_ANSWERS {
                        known.pop_first();
                    }
                    known.insert(ip, local);
                    local
                }
            },
            Err(_) => false,
        };
        if !local {
            return addr;
        }
        let loopback = match ip {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::LOCALHOST),
        };
        SocketAddr::new(loopback, addr.port())
    }

    /// The TCP port of the listener. In the dialect `fastdds` it is the
    /// lower half of the port of [`Self::local_locator`], whose upper half
    /// is the logical port (§6.1).
    #[must_use]
    pub fn listener_port(&self) -> u16 {
        u16::try_from(self.local_locator.port).unwrap_or(0)
    }

    /// This side of a connection of the dialect `fastdds` (§6.1).
    fn rtcp_own(&self) -> rtcp::Own {
        rtcp::Own {
            ports: self.logical_ports(),
            listener_port: self.listener_port(),
            vendor_id: self.vendor_id,
        }
    }

    /// Sets the dialect of the connections this transport opens:
    /// [`TcpDialect::ZeroDds`], the default, is the handshake of §3 and
    /// the frames of §2.1; [`TcpDialect::Rtps`] is RTPS messages back to
    /// back, each delimited by the messageLength of its HeaderExtension
    /// (§2.5); [`TcpDialect::Cyclone`] is the stream of Cyclone DDS
    /// (§4.1). In `Rtps` a `send` takes only a message that states its own
    /// length so and refuses any other; the transport adds nothing to a
    /// message. In `Cyclone` the transport puts the length submessage
    /// into each message and takes at most
    /// [`cyclone_stream::MAX_MESSAGE_SIZE`] octets. The dialect of a
    /// connection this transport accepts is found at its first octets
    /// ([`with_accepted_dialects`](Self::with_accepted_dialects)).
    #[must_use]
    pub fn with_dialect(mut self, dialect: TcpDialect) -> Self {
        self.dialect = dialect;
        self
    }

    /// The dialect of the connections this transport opens.
    #[must_use]
    pub fn dialect(&self) -> TcpDialect {
        self.dialect
    }

    /// The dialect a `send` to `dest` is written in now: the one of the
    /// live accepted connection `dest` is the source of (§4.1), otherwise
    /// [`dialect`](Self::dialect). A layer above that puts something into
    /// the message for one dialect (the HeaderExtension with the
    /// messageLength of the dialect `rtps`, §2.5) asks here per
    /// destination. The connection may end between this call and the
    /// `send`; the `send` then goes the way of a destination without one.
    #[must_use]
    pub fn dialect_towards(&self, dest: &Locator) -> TcpDialect {
        let Ok(dest) = destination(dest) else {
            return self.dialect;
        };
        if dest.logical_port.is_some() {
            return TcpDialect::FastDds;
        }
        let addr = dest.addr;
        self.answering
            .lock()
            .ok()
            .and_then(|table| table.get(&addr).map(|(_, conn)| Arc::clone(conn)))
            .and_then(|conn| conn.lock().ok().map(|conn| conn.dialect))
            .unwrap_or(self.dialect)
    }

    /// Limits the dialects a connection to this listener may begin in
    /// (§4.2). The listener finds the dialect of a connection at its
    /// first octets: `ZDDS` is the request of §3, `RTPS` a stream of RTPS
    /// messages, whose first submessage tells the dialect `cyclone`
    /// (`0x81`) from the dialect `rtps` (the HeaderExtension). Without
    /// this call every dialect is taken; an empty list takes none.
    #[must_use]
    pub fn with_accepted_dialects(mut self, dialects: &[TcpDialect]) -> Self {
        self.accepted_dialects = dialects.to_vec();
        self
    }

    /// The dialects a connection to this listener may begin in.
    #[must_use]
    pub fn accepted_dialects(&self) -> &[TcpDialect] {
        &self.accepted_dialects
    }

    fn accepts(&self, dialect: TcpDialect) -> bool {
        self.accepted_dialects.contains(&dialect)
    }

    /// Limits the vendors whose connections this listener takes (§3.3):
    /// a request with a vendor id that is not in `vendors` is answered
    /// with Reject/`VendorNotAccepted`; in the dialect
    /// [`TcpDialect::Rtps`], which has no request, a connection is closed
    /// on the first message whose RTPS header carries another vendor id.
    /// An empty list accepts no vendor. Without this call every vendor is
    /// accepted.
    #[must_use]
    pub fn with_accepted_vendors(mut self, vendors: Vec<[u8; 2]>) -> Self {
        self.accepted_vendors = Some(vendors);
        self
    }

    /// The vendor ids this listener takes connections from; `None` is
    /// every vendor.
    #[must_use]
    pub fn accepted_vendors(&self) -> Option<&[[u8; 2]]> {
        self.accepted_vendors.as_deref()
    }

    /// Sets how many accepted connections [`serve`](Self::serve) reads at
    /// a time (§3.7); the default is [`MAX_PEERS`]. A connection beyond
    /// the limit is answered with Reject/`ResourceLimit`.
    #[must_use]
    pub fn with_accept_limit(mut self, limit: usize) -> Self {
        self.accept_limit = limit;
        self
    }

    /// The number of accepted connections [`serve`](Self::serve) reads at
    /// a time.
    #[must_use]
    pub fn accept_limit(&self) -> usize {
        self.accept_limit
    }

    /// Whether the request of an outgoing connection claims the port of
    /// this transport's listener as its logical port (§3.6). The default
    /// is `true`; with `false` the request carries 0, no claim, and the
    /// listener at the other end names the frames of the connection by
    /// its ephemeral port. That is the setting for hosts behind one
    /// translated address, whose claims of one port would collide.
    #[must_use]
    pub fn with_logical_port_claim(mut self, claim: bool) -> Self {
        self.claim_logical_port = claim;
        self
    }

    /// Whether an outgoing connection claims the port of this listener.
    #[must_use]
    pub fn claims_logical_port(&self) -> bool {
        self.claim_logical_port
    }

    /// The logical port an outgoing connection claims: the port of this
    /// listener, or 0 for none.
    fn outgoing_claim(&self) -> u32 {
        if self.claim_logical_port {
            self.local_locator.port
        } else {
            0
        }
    }

    /// Local locator. In the dialect `fastdds` it is of kind 4 or 8 and
    /// holds the logical port in the upper 16 bits of its port (§6.1).
    #[must_use]
    pub fn local_locator(&self) -> Locator {
        if self.dialect == TcpDialect::FastDds {
            let mut locator = self.local_locator.with_tcp_kind_set(TcpKindSet::Kinds4And8);
            locator.port = (u32::from(self.logical_port()) << 16) | u32::from(self.listener_port());
            locator
        } else {
            self.local_locator
        }
    }

    /// Accepts an incoming connection and reads all frames from it into
    /// the inbound queue until the peer closes the connection.
    ///
    /// In phase 1 this is a blocking function; tests + apps drive it
    /// explicitly (or in their own thread). Phase 2 delivers a
    /// background accept thread.
    ///
    /// # Errors
    /// - `Accept(io::Error)` if the listener fails,
    /// - `Handshake(_)` if the peer does not complete the handshake of §3
    ///   within its budget, or is rejected,
    /// - `FrameTooLarge { announced }` if the peer announces an
    ///   oversized frame,
    /// - `PeerIo(io::Error)` on other read errors; of kind `UnexpectedEof`
    ///   if the peer closed the connection inside a frame (§2.4). The
    ///   frames read before the error are in the inbound queue.
    pub fn accept_one(&self) -> Result<(), TcpTransportError> {
        let (stream, peer) = self.listener.accept().map_err(TcpTransportError::Accept)?;
        let id = self.next_connection.fetch_add(1, Ordering::Relaxed);
        self.read_connection(stream, peer, id)
    }

    /// Accepts connections until [`close`](Self::close) and reads each in
    /// a thread of its own (`zdds-tcp-conn`), handshake included: the
    /// frames of every connected peer reach the inbound queue, where
    /// [`accept_one`](Self::accept_one) in a loop read only the first
    /// peer until it disconnected and left every other one unread. At most
    /// [`accept_limit`](Self::accept_limit) connections are read at a
    /// time. One beyond that is counted
    /// ([`refused_connections`](Self::refused_connections)) and handed to
    /// a thread of its own (`zdds-tcp-refuse`), which reads its request
    /// and answers Reject/`ResourceLimit` (§3.7); the accepting loop reads
    /// no octet of it. A connection whose reading ended in an error is
    /// counted ([`connection_errors`](Self::connection_errors)). Returns
    /// after `close`, once the reading threads have ended.
    pub fn serve(self: &Arc<Self>) {
        let mut readers: Vec<std::thread::JoinHandle<()>> = Vec::new();
        let (refuse_tx, refuse_rx) = std::sync::mpsc::sync_channel::<Refused>(REFUSE_QUEUE);
        // If the thread cannot be made, the receiver is dropped with the
        // closure and `refuse_beyond_limit` closes every connection it is
        // given without an answer, counted.
        let refuser = {
            let this = Arc::clone(self);
            std::thread::Builder::new()
                .name(String::from("zdds-tcp-refuse"))
                .spawn(move || {
                    for (stream, due) in refuse_rx {
                        this.answer_resource_limit(&stream, due);
                    }
                })
                .ok()
        };
        loop {
            let accepted = self.listener.accept();
            readers.retain(|reader| !reader.is_finished());
            let (stream, peer) = match accepted {
                Ok(pair) => pair,
                Err(_) if self.closed.load(Ordering::Acquire) => break,
                Err(_) => {
                    std::thread::sleep(ACCEPT_ERROR_PAUSE);
                    continue;
                }
            };
            let id = self.next_connection.fetch_add(1, Ordering::Relaxed);
            let registered = {
                let Ok(mut open) = self.accepted.lock() else {
                    break;
                };
                // Under the lock `close` takes to shut the streams down: a
                // stream registered here is one it reaches.
                if self.closed.load(Ordering::Acquire) {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    break;
                }
                let clone = if open.len() < self.accept_limit() {
                    stream.try_clone().ok()
                } else {
                    None
                };
                match clone {
                    Some(clone) => {
                        open.insert(id, clone);
                        true
                    }
                    None => false,
                }
            };
            if !registered {
                self.refuse_beyond_limit(&refuse_tx, stream);
                continue;
            }
            let this = Arc::clone(self);
            let spawned = std::thread::Builder::new()
                .name(String::from("zdds-tcp-conn"))
                .spawn(move || {
                    if this.read_connection(stream, peer, id).is_err() {
                        this.inbound
                            .connection_errors
                            .fetch_add(1, Ordering::Relaxed);
                    }
                    this.forget_connection(id);
                });
            match spawned {
                Ok(reader) => readers.push(reader),
                // The closure, and the stream in it, is dropped; the
                // registered clone is the connection still.
                Err(_) => self.refuse_unstarted(&refuse_tx, id),
            }
        }
        drop(refuse_tx);
        if let Some(refuser) = refuser {
            let _ = refuser.join();
        }
        for reader in readers {
            let _ = reader.join();
        }
    }

    /// Refuses the connection `id`, which has its place among the
    /// connections that are read and no thread to read it: the thread
    /// could not be started (§3.7). Its place is given back and the
    /// registered stream, which is the connection, goes the way of one
    /// beyond the limit. Without a registered stream there is nothing to
    /// answer on; the connection is counted as refused and as unanswered.
    fn refuse_unstarted(&self, queue: &SyncSender<Refused>, id: u64) {
        let registered = self
            .accepted
            .lock()
            .ok()
            .and_then(|mut open| open.remove(&id));
        match registered {
            Some(stream) => self.refuse_beyond_limit(queue, stream),
            None => {
                self.refused_connections.fetch_add(1, Ordering::Relaxed);
                self.refused_unanswered.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Counts a connection this listener has no place for and hands it to
    /// the refusing thread (§3.7), with the instant its answer is due by
    /// ([`REFUSE_DEADLINE`] from now). Never reads it: the accepting loop
    /// goes on at once. If [`REFUSE_QUEUE`] connections wait for their
    /// Reject already, this one is closed without an answer and counted
    /// in [`refused_unanswered`](Self::refused_unanswered) as well.
    fn refuse_beyond_limit(&self, queue: &SyncSender<Refused>, stream: TcpStream) {
        self.refused_connections.fetch_add(1, Ordering::Relaxed);
        let due = Instant::now() + REFUSE_DEADLINE;
        // Only the dialect `zerodds` has a response to refuse with; a
        // listener that does not take it closes at once (§2.5, §4.2).
        let handed = if self.accepts(TcpDialect::ZeroDds) {
            queue.try_send((stream, due))
        } else {
            Err(TrySendError::Disconnected((stream, due)))
        };
        if let Err(TrySendError::Full((stream, _)) | TrySendError::Disconnected((stream, _))) =
            handed
        {
            // Counted before the close: a peer that reads the end of the
            // connection finds the count.
            self.refused_unanswered.fetch_add(1, Ordering::Relaxed);
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }

    /// Reads the request of a connection beyond the peer limit and answers
    /// it (§3.7), within [`REFUSE_BUDGET`] and not later than `due`:
    /// Reject/`ResourceLimit` for a request that passes the checks before
    /// the limit, the Reject of the check it fails otherwise (§3.3). The
    /// request is read first: closing a connection with unread input
    /// resets it, and a reset can cost the peer the response. A peer that
    /// sends no whole request in time, or none of §3.1, and one whose turn
    /// comes after `due`, gets no answer and is counted in
    /// [`refused_unanswered`](Self::refused_unanswered).
    fn answer_resource_limit(&self, stream: &TcpStream, due: Instant) {
        let budget = REFUSE_BUDGET.min(due.saturating_duration_since(Instant::now()));
        let answered = !self.closed.load(Ordering::Acquire) && !budget.is_zero() && {
            let _ = stream.set_nodelay(true);
            let mut bounded = DeadlineStream::new(stream, budget, budget);
            let mut policy = ServerPolicy {
                accepted_vendors: self.accepted_vendors.as_deref(),
                at_peer_limit: true,
                claim: None,
            };
            server_handshake_with(&mut bounded, &mut policy).is_ok()
        };
        // Counted before the close: a peer that reads the end of the
        // connection finds the count.
        if !answered {
            self.refused_unanswered.fetch_add(1, Ordering::Relaxed);
        }
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }

    fn forget_connection(&self, id: u64) {
        if let Ok(mut open) = self.accepted.lock()
            && let Some(stream) = open.remove(&id)
        {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }

    /// Connections [`serve`](Self::serve) had no place for: beyond
    /// [`accept_limit`](Self::accept_limit), or because no thread or
    /// stream clone could be made for them. Each is answered with a
    /// Reject (§3.7) unless it is counted in
    /// [`refused_unanswered`](Self::refused_unanswered) too.
    #[must_use]
    pub fn refused_connections(&self) -> u64 {
        self.refused_connections.load(Ordering::Relaxed)
    }

    /// Those of [`refused_connections`](Self::refused_connections) that
    /// were closed without a Reject: the peer sent no whole request of
    /// §3.1 within 250 ms, the response could not be written, 16 refused
    /// connections waited for their answer already, the connection's turn
    /// came later than 1.5 s after it was accepted, or the transport was
    /// closed.
    #[must_use]
    pub fn refused_unanswered(&self) -> u64 {
        self.refused_unanswered.load(Ordering::Relaxed)
    }

    /// Outgoing connect attempts a listener answered with a Reject of
    /// `reason` (§3.3, §3.7). A Reject is no connection error: the send
    /// that met it fails with a `SendError::Io` that names the reason.
    #[must_use]
    pub fn connect_rejections(&self, reason: RejectReason) -> u64 {
        usize::try_from(reason.as_code())
            .ok()
            .and_then(|code| self.pool_counters.rejections.get(code))
            .map_or(0, |count| count.load(Ordering::Relaxed))
    }

    /// The logical ports the live accepted connections hold (§3.6), each
    /// with the address of the peer that claimed it.
    #[must_use]
    pub fn claimed_logical_ports(&self) -> Vec<(IpAddr, u32)> {
        self.claims
            .lock()
            .map(|claims| {
                claims
                    .keys()
                    .map(|(_, peer, port)| (*peer, *port))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Frames a peer began and did not finish: the connection ended
    /// inside the length or the body (§2.4). No part of such a frame
    /// reaches the inbound queue.
    #[must_use]
    pub fn truncated_frames(&self) -> u64 {
        self.inbound.truncated_frames.load(Ordering::Relaxed)
    }

    /// Connections whose reading ended in an error, those
    /// [`serve`](Self::serve) accepted and the outgoing ones: a failed or
    /// rejected handshake, a dialect or vendor this listener does not
    /// take, a frame above the limit, a connection closed inside a frame,
    /// a read error. The close of a connection on a frame boundary is not
    /// one.
    #[must_use]
    pub fn connection_errors(&self) -> u64 {
        self.inbound.connection_errors.load(Ordering::Relaxed)
    }

    /// Sends that were written on an accepted connection (§4.1): the
    /// destination was the address a live accepted connection came from.
    #[must_use]
    pub fn answered_on_accepted(&self) -> u64 {
        self.answered_on_accepted.load(Ordering::Relaxed)
    }

    /// RTPS messages a peer of the dialect `fastdds` wrote for a logical
    /// port this transport has not open (§6.1). Each was answered with a
    /// LogicalPortIsClosedRequest, which makes Fast DDS ask for the port
    /// again; none reached the inbound queue.
    #[must_use]
    pub fn rtcp_closed_port_messages(&self) -> u64 {
        self.inbound
            .rtcp
            .closed_port_messages
            .load(Ordering::Relaxed)
    }

    /// Logical ports a peer of the dialect `fastdds` refused to open for a
    /// `send` of this transport (§6.1); each such `send` failed with a
    /// `SendError::Io` that says so.
    #[must_use]
    pub fn rtcp_refused_ports(&self) -> u64 {
        self.inbound.rtcp.refused_ports.load(Ordering::Relaxed)
    }

    /// Responses of a peer of the dialect `fastdds` to no request of this
    /// transport (§6.1).
    #[must_use]
    pub fn rtcp_unmatched_responses(&self) -> u64 {
        self.inbound
            .rtcp
            .unmatched_responses
            .load(Ordering::Relaxed)
    }

    /// Control messages of a peer of the dialect `fastdds` this transport
    /// answered with `BadRequest` (§6.1): an unknown kind, a length that
    /// is not the message's, a payload that does not fit.
    #[must_use]
    pub fn rtcp_bad_requests(&self) -> u64 {
        self.inbound.rtcp.bad_requests.load(Ordering::Relaxed)
    }

    /// Outgoing connections a `send` found closed by the peer and
    /// replaced (§5.1). A frame written into such a connection before its
    /// close arrived at this host may not have reached the peer, and the
    /// `send` that wrote it returned `Ok`: `Ok` says that the frame was
    /// handed to the TCP stack. How many frames that were is not known on
    /// this side; a connection closed without a `send` after it is not
    /// counted.
    #[must_use]
    pub fn peer_closed_connections(&self) -> u64 {
        self.pool_counters.closed_by_peer.load(Ordering::Relaxed)
    }

    /// Ends [`serve`](Self::serve) and every connection of this
    /// transport. The accepted streams and the outgoing ones are shut
    /// down, so a peer reads EOF at once instead of when this process
    /// ends. A connection to the own listener wakes the `accept`, which
    /// has no timeout. A `recv` waiting on the inbound queue returns, and
    /// a later `send` or `recv` fails. Idempotent.
    pub fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        // §6.1: a connection of the dialect `fastdds` ends with an
        // UnbindConnectionRequest, as Fast DDS ends its own.
        let links: Vec<Arc<rtcp::Link>> = self
            .answering
            .lock()
            .map(|table| {
                table
                    .values()
                    .filter_map(|(_, conn)| conn.try_lock().ok().and_then(|c| c.link.clone()))
                    .collect()
            })
            .unwrap_or_default();
        for link in links {
            link.unbind();
        }
        if let Ok(pool) = self.peers.lock() {
            for conn in pool.values() {
                if let Ok(conn) = conn.try_lock()
                    && let Some(link) = &conn.link
                {
                    link.unbind();
                }
            }
        }
        if let Ok(open) = self.accepted.lock() {
            for stream in open.values() {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
        if let Ok(mut pool) = self.peers.lock() {
            for conn in core::mem::take(&mut *pool).into_values() {
                // A send blocked on a full peer buffer holds the lock; its
                // stream closes when that send drops the last reference.
                if let Ok(mut conn) = conn.try_lock() {
                    conn.drop_writer();
                }
            }
        }
        if let Ok(addr) = self.listener.local_addr() {
            let _ = TcpStream::connect_timeout(&wake_address(addr), WAKE_CONNECT_TIMEOUT);
        }
        // Under the lock: a `recv` between its check of `closed` and its
        // wait would otherwise miss the notification.
        let _queue = self.inbound.state.lock();
        self.inbound.cv.notify_all();
    }

    fn read_connection(
        &self,
        stream: TcpStream,
        peer: SocketAddr,
        id: u64,
    ) -> Result<(), TcpTransportError> {
        stream
            .set_nodelay(true)
            .map_err(TcpTransportError::Accept)?;
        // §3.6: a peer that vanished without a FIN is found by the
        // keepalive probes, and its claim released with the connection.
        set_keepalive(&stream).map_err(TcpTransportError::SetTimeout)?;
        // §4.2: the first four octets tell the dialect, read under the
        // deadline of the handshake (§3.8). `ZDDS` is the request of §3,
        // `RTPS` the first message of a stream.
        let zerodds = self.accepts(TcpDialect::ZeroDds);
        let deadline = Instant::now() + SERVER_HANDSHAKE_BUDGET;
        let mut first = [0u8; 4];
        let read = read_head(
            &mut DeadlineStream::until(&stream, deadline, SERVER_HANDSHAKE_SLICE),
            &mut first,
        );
        match read {
            Ok(()) => {}
            // A listener that takes no request of §3 expects none: a
            // connection that ends before its first message has ended on
            // a message boundary (§2.4).
            Err(FramingError::Closed) if !zerodds => return Ok(()),
            Err(FramingError::Closed | FramingError::UnexpectedEof) if zerodds => {
                return Err(TcpTransportError::Handshake(HandshakeError::Io(
                    std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "peer closed the connection before its first four octets",
                    ),
                )));
            }
            Err(FramingError::Io(e)) if zerodds => {
                return Err(TcpTransportError::Handshake(HandshakeError::Io(e)));
            }
            Err(other) => {
                return self
                    .inbound
                    .take(
                        Err(other),
                        TcpDialect::Rtps,
                        socket_locator(peer, self.locator_kinds),
                        None,
                    )
                    .map(|_| ());
            }
        }
        if &first == b"ZDDS" && zerodds {
            return self.read_zerodds_connection(stream, first, deadline, peer, id);
        }
        if &first == b"RTPS"
            && (self.accepts(TcpDialect::Rtps) || self.accepts(TcpDialect::Cyclone))
        {
            return self.read_message_stream(stream, first, peer, id);
        }
        if first == rtcp::MAGIC {
            if !self.accepts(TcpDialect::FastDds) {
                return Err(TcpTransportError::DialectNotAccepted {
                    dialect: TcpDialect::FastDds,
                });
            }
            return self.read_rtcp_connection(stream, deadline, peer, id);
        }
        // Neither: the error of the dialect this listener is there for.
        if zerodds {
            Err(TcpTransportError::Handshake(HandshakeError::BadMagic {
                got: first,
            }))
        } else {
            Err(TcpTransportError::StreamFraming(FramingError::NotRtps {
                got: first,
            }))
        }
    }

    /// Reads a connection of the dialect [`TcpDialect::ZeroDds`] whose
    /// `first` four octets, `ZDDS`, have been read: the rest of the
    /// handshake of §3 until `deadline`, then the frames of §2.1.
    fn read_zerodds_connection(
        &self,
        stream: TcpStream,
        first: [u8; 4],
        deadline: Instant,
        peer: SocketAddr,
        id: u64,
    ) -> Result<(), TcpTransportError> {
        let local_ip = stream.local_addr().map_err(TcpTransportError::Accept)?.ip();
        // Released when this function returns, whichever way.
        let mut held = ClaimGuard {
            claims: &self.claims,
            connection: id,
            held: None,
        };
        // ZeroDDS handshake under one deadline (§3.8): a peer that sends
        // its request octet by octet is given up on when the budget ends,
        // before a response is written. A Reject has its own error, so
        // the caller tells it from the end of a connection.
        let mut bounded = Prefixed {
            prefix: first,
            taken: 0,
            stream: DeadlineStream::until(&stream, deadline, SERVER_HANDSHAKE_SLICE),
        };
        let mut claim = |port: u32| held.claim((local_ip, peer.ip(), port));
        let mut policy = ServerPolicy {
            accepted_vendors: self.accepted_vendors.as_deref(),
            at_peer_limit: false,
            claim: Some(&mut claim),
        };
        let request = match server_handshake_with(&mut bounded, &mut policy) {
            Ok((request, resp)) => {
                if let ResponseStatus::Reject(reason) = resp.status {
                    return Err(TcpTransportError::Handshake(HandshakeError::Rejected {
                        reason,
                    }));
                }
                request
            }
            Err(e) => return Err(TcpTransportError::Handshake(e)),
        };
        // After a successful handshake: lift the timeout again
        // (the normal frame-pump regime blocks for expectedly long).
        stream
            .set_read_timeout(None)
            .map_err(TcpTransportError::Accept)?;
        stream
            .set_write_timeout(None)
            .map_err(TcpTransportError::Accept)?;
        // §3.6: the connection is the claimed port's. Its frames carry the
        // peer's address with that port as their source, the locator of
        // the peer's listener; a send to it connects to that listener.
        // Without a claim the port is the ephemeral one the peer connected
        // from: nothing listens there, and a send to it is written on
        // this connection (§4.1).
        let (source_port, _answering) = if request.logical_port != 0 {
            (request.logical_port, None)
        } else {
            (
                u32::from(peer.port()),
                Some(self.answer_on(&stream, peer, TcpDialect::ZeroDds, id)?),
            )
        };
        let source_locator = match peer.ip() {
            IpAddr::V4(v4) => Locator::tcp_v4(v4.octets(), source_port),
            IpAddr::V6(v6) => Locator::tcp_v6(v6.octets(), source_port),
        }
        .with_tcp_kind_set(self.locator_kinds);
        let mut reader = BufReader::new(stream);
        self.inbound
            .pump(&mut reader, TcpDialect::ZeroDds, source_locator, None)
    }

    /// Reads a connection that began with `RTPS` (§4.2): no handshake,
    /// RTPS messages back to back. The first submessage of the first
    /// message tells the dialect: `0x81` is the length submessage of
    /// Cyclone DDS (§4.1), the HeaderExtension the dialect `rtps` (§2.5).
    /// Every later message of the connection has to be of that dialect.
    /// The source of a message is the address the peer connected from,
    /// and a send to it is written on this connection (§4.1). A
    /// connection that carries anything else is closed: a stream has no
    /// second way to find the next message.
    fn read_message_stream(
        &self,
        stream: TcpStream,
        first: [u8; 4],
        peer: SocketAddr,
        id: u64,
    ) -> Result<(), TcpTransportError> {
        stream
            .set_read_timeout(None)
            .map_err(TcpTransportError::Accept)?;
        stream
            .set_write_timeout(None)
            .map_err(TcpTransportError::Accept)?;
        let source = socket_locator(peer, self.locator_kinds);
        let vendors = self.accepted_vendors.as_deref();
        let answer_side = stream.try_clone().map_err(TcpTransportError::Accept)?;
        let mut reader = BufReader::new(stream);
        // Both dialects begin a message with 28 octets, and the
        // submessage id is octet 20.
        let mut head = [0u8; rtps_stream::STREAM_HEAD_LEN];
        head[..4].copy_from_slice(&first);
        if let Err(e) = read_exact_or_eof(&mut reader, &mut head[4..]) {
            return self
                .inbound
                .take(Err(e), TcpDialect::Rtps, source, vendors)
                .map(|_| ());
        }
        let dialect = match head[20] {
            cyclone_stream::MSG_LEN_SUBMESSAGE_ID => TcpDialect::Cyclone,
            0x00 => TcpDialect::Rtps,
            _ => {
                return Err(TcpTransportError::StreamFraming(
                    FramingError::NoMessageLength,
                ));
            }
        };
        if !self.accepts(dialect) {
            return Err(TcpTransportError::DialectNotAccepted { dialect });
        }
        let message = match dialect {
            TcpDialect::Cyclone => cyclone_stream::read_body(&head, &mut reader),
            _ => rtps_stream::read_body(&head, &mut reader),
        };
        // Before the first message is handed on: whoever reads it may
        // answer at once.
        let _answering = self.answer_on(&answer_side, peer, dialect, id)?;
        if !self.inbound.take(message, dialect, source, vendors)? {
            return Ok(());
        }
        self.inbound.pump(&mut reader, dialect, source, vendors)
    }

    /// Reads a connection that began with `RTCP`, the dialect `fastdds`
    /// (§6.1): the BindConnectionRequest of the peer before `deadline`
    /// (§3.8), answered with this listener's locator, then the messages of
    /// the connection. A request of a vendor this listener does not take
    /// (§3.3) is answered with `ServerError`, the code Fast DDS has for a
    /// request it does not serve; one of another protocol version with
    /// `IncompatibleVersion`. The connection is the one a `send` to the
    /// peer's listener, as its request names it, writes on (§4.1); the
    /// source of a message is that listener with the logical port the
    /// message was for.
    fn read_rtcp_connection(
        &self,
        stream: TcpStream,
        deadline: Instant,
        peer: SocketAddr,
        id: u64,
    ) -> Result<(), TcpTransportError> {
        let vendors = self.accepted_vendors.as_deref();
        let (link, mut reader, request) = rtcp::Link::accept(
            &stream,
            self.rtcp_own(),
            deadline,
            |request| match vendors {
                Some(accepted) if !accepted.contains(&request.vendor_id) => {
                    rtcp::ResponseCode::ServerError
                }
                _ => rtcp::ResponseCode::Ok,
            },
        )
        .map_err(TcpTransportError::Rtcp)?;
        stream
            .set_write_timeout(None)
            .map_err(TcpTransportError::Accept)?;
        // §6.1: the peer's listener at the loopback address when the peer
        // is on this host, as a send to a Fast DDS locator of this host
        // goes there.
        let listener = self.host_alias(SocketAddr::new(
            peer.ip(),
            request.transport_locator.physical_port(),
        ));
        let link = Arc::new(link);
        let _answering = self.answer_on_link(&stream, listener, &link, id)?;
        let inbound = &self.inbound;
        link.run(&mut reader, &inbound.rtcp, |port, data| {
            inbound.push(ReceivedDatagram {
                source: rtcp_source(listener, port),
                data: data.into(),
            });
        })
        .map_err(TcpTransportError::Rtcp)
    }

    /// Makes the accepted connection of the dialect `fastdds` the one a
    /// `send` to `listener` writes on (§4.1, §6.1), until the returned
    /// guard is dropped.
    fn answer_on_link(
        &self,
        stream: &TcpStream,
        listener: SocketAddr,
        link: &Arc<rtcp::Link>,
        id: u64,
    ) -> Result<AnswerGuard<'_>, TcpTransportError> {
        link.with_stream(|s| s.set_write_timeout(Some(PEER_WRITE_STALL)))
            .transpose()
            .map_err(TcpTransportError::SetTimeout)?;
        let stream = stream.try_clone().map_err(TcpTransportError::Accept)?;
        let addr = pool_address(listener);
        let conn = Arc::new(Mutex::new(AnswerConn {
            stream,
            dialect: TcpDialect::FastDds,
            link: Some(Arc::clone(link)),
        }));
        self.answering
            .lock()
            .map_err(|_| TcpTransportError::Accept(std::io::Error::other("answer table poisoned")))?
            .insert(addr, (id, conn));
        Ok(AnswerGuard {
            table: &self.answering,
            addr,
            connection: id,
        })
    }

    /// Makes the accepted connection `stream` the one a `send` to `peer`
    /// writes on (§4.1), in `dialect`, until the returned guard is
    /// dropped. A write on it that makes no progress for
    /// [`PEER_WRITE_STALL`] fails.
    fn answer_on(
        &self,
        stream: &TcpStream,
        peer: SocketAddr,
        dialect: TcpDialect,
        id: u64,
    ) -> Result<AnswerGuard<'_>, TcpTransportError> {
        let stream = stream.try_clone().map_err(TcpTransportError::Accept)?;
        stream
            .set_write_timeout(Some(PEER_WRITE_STALL))
            .map_err(TcpTransportError::SetTimeout)?;
        let addr = pool_address(peer);
        let conn = Arc::new(Mutex::new(AnswerConn {
            stream,
            dialect,
            link: None,
        }));
        self.answering
            .lock()
            .map_err(|_| TcpTransportError::Accept(std::io::Error::other("answer table poisoned")))?
            .insert(addr, (id, conn));
        Ok(AnswerGuard {
            table: &self.answering,
            addr,
            connection: id,
        })
    }

    /// Writes `data` on the accepted connection `conn` (§4.1). A message
    /// the dialect of the connection cannot carry is refused before an
    /// octet is written; a failed write shuts the connection down, since
    /// part of a message may be on the stream.
    fn send_on_accepted(
        &self,
        conn: &Mutex<AnswerConn>,
        data: &[u8],
        logical_port: Option<u16>,
    ) -> Result<(), SendError> {
        let conn = conn.lock().map_err(|_| SendError::Io {
            message: "accepted conn poisoned",
        })?;
        let dialect = conn.dialect;
        if let Some(link) = &conn.link {
            // §6.1: an accepted connection of the dialect `fastdds` takes a
            // message for a logical port, which it asks the peer for.
            let Some(port) = logical_port else {
                return Err(SendError::Io {
                    message: "tcp locator without a logical port towards a fastdds connection",
                });
            };
            if data.len() > rtcp::MAX_MESSAGE_SIZE {
                return Err(SendError::PayloadTooLarge {
                    size: data.len(),
                    limit: rtcp::MAX_MESSAGE_SIZE,
                });
            }
            let link = Arc::clone(link);
            drop(conn);
            return match link.send(port, data, CLIENT_HANDSHAKE_BUDGET) {
                Ok(()) => {
                    self.answered_on_accepted.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }
                Err(e) => Err(SendError::Io {
                    message: rtcp_message(&e),
                }),
            };
        }
        if logical_port.is_some() {
            return Err(SendError::Io {
                message: "tcp locator with a logical port towards a connection of another dialect",
            });
        }
        if data.len() > dialect.max_message_size() {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: dialect.max_message_size(),
            });
        }
        if let Err(e) = dialect.check(data) {
            return Err(SendError::Io {
                message: unfit_message(&e),
            });
        }
        let mut wire = Vec::with_capacity(data.len() + cyclone_stream::MSG_LEN_SIZE);
        let written = dialect
            .write(&mut wire, data)
            .and_then(|()| Ok((&conn.stream).write_all(&wire)?));
        match written {
            Ok(()) => {
                self.answered_on_accepted.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(_) => {
                let _ = conn.stream.shutdown(std::net::Shutdown::Both);
                Err(SendError::Io {
                    message: "tcp write on an accepted connection failed",
                })
            }
        }
    }

    #[cfg(test)]
    fn push_inbound(&self, dg: ReceivedDatagram) {
        self.inbound.push(dg);
    }

    /// Number of frames discarded due to queue overflow.
    #[must_use]
    pub fn dropped_frames(&self) -> u64 {
        self.inbound
            .state
            .lock()
            .map(|s| s.dropped)
            .unwrap_or_default()
    }

    /// Non-blocking pop from the inbound queue. Useful for tests and
    /// callsites that want polling control themselves (vs. the blocking
    /// [`Transport::recv`]).
    ///
    /// # Errors
    /// [`RecvError::Timeout`] if the queue is empty.
    pub fn try_recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let mut st = self.inbound.state.lock().map_err(|_| RecvError::Io {
            message: "inbound queue poisoned",
        })?;
        st.queue.pop_front().ok_or(RecvError::Timeout)
    }
}

// ---------------------------------------------------------------------
// Transport-Trait
// ---------------------------------------------------------------------

impl Transport for TcpTransport {
    /// The largest message a connection this transport opens carries
    /// ([`TcpDialect::max_message_size`]): the frame limit of §2.3, and in
    /// the dialect `cyclone` what Cyclone DDS reads (§4.1).
    fn max_message_size(&self) -> usize {
        self.dialect.max_message_size()
    }

    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(SendError::Io {
                message: "tcp transport closed",
            });
        }
        let Destination {
            mut addr,
            logical_port,
            wan,
        } = match destination(dest) {
            Ok(d) => d,
            Err(InvalidLocator::WrongKind) => return Err(SendError::UnsupportedLocator),
            Err(InvalidLocator::PortOverflow { port }) => {
                return Err(SendError::Io {
                    message: if port > u16::MAX as u32 {
                        "tcp locator port > u16::MAX"
                    } else {
                        "tcp locator port invalid"
                    },
                });
            }
        };
        // §6.1: Fast DDS holds one connection per pair of participants
        // and takes each address of its own host for that host. A Fast
        // DDS locator at an address of this host is reached at the
        // loopback address, on the connection that is there already.
        if logical_port.is_some() {
            addr = self.host_alias(addr);
        }
        // §2.3: the size is known before a connection is; a payload above
        // the limit is refused without a connect and without a pool entry.
        if data.len() > crate::framing::MAX_FRAME_SIZE as usize {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: crate::framing::MAX_FRAME_SIZE as usize,
            });
        }
        // §4.1: the address a live accepted connection came from is one
        // nothing listens at. The answer goes on that connection, in its
        // dialect.
        let answering = self
            .answering
            .lock()
            .map_err(|_| SendError::Io {
                message: "answer table poisoned",
            })?
            .get(&addr)
            .map(|(_, conn)| Arc::clone(conn));
        if let Some(conn) = answering {
            return self.send_on_accepted(&conn, data, logical_port);
        }
        // §6.1: a locator that names a logical port is one of Fast DDS's
        // TCP transport, reached in its dialect. One without a logical
        // port cannot be addressed in that dialect.
        let dialect = if logical_port.is_some() {
            TcpDialect::FastDds
        } else if self.dialect == TcpDialect::FastDds {
            return Err(SendError::Io {
                message: "tcp locator without the logical port the dialect fastdds addresses",
            });
        } else {
            self.dialect
        };
        // §4.1: Cyclone DDS drops a connection on a message above what it
        // reads; such a message is refused here, and so is one above what
        // Fast DDS reads (§6.1), without a connect.
        if data.len() > dialect.max_message_size() {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: dialect.max_message_size(),
            });
        }
        // §2.5: the transport adds nothing to a message of the dialect
        // `rtps`. One that does not state its own length cannot go on a
        // stream of this dialect and is refused here, without a connect;
        // so is, in the dialect `cyclone`, what has no RTPS header to put
        // the length submessage behind.
        if let Err(e) = dialect.check(data) {
            return Err(SendError::Io {
                message: unfit_message(&e),
            });
        }
        // Step 1: peer lookup/insert under the pool lock, an entry of the
        // dialect of the destination.
        let peer_arc = self.peer_entry(addr, dialect, wan)?;
        // Step 2: release the pool lock, then take the peer lock for I/O.
        let mut conn = peer_arc.lock().map_err(|_| SendError::Io {
            message: "peer conn poisoned",
        })?;
        match conn.send(data, logical_port, &self.pool_counters) {
            Ok(()) => Ok(()),
            Err(FramingError::FrameTooLarge { announced }) => Err(SendError::PayloadTooLarge {
                size: announced as usize,
                limit: dialect.max_message_size(),
            }),
            // §3.7: a Reject is told from a connection error. The text
            // stands while the backoff after the Reject throttles the
            // next attempt.
            Err(_) => Err(SendError::Io {
                message: conn
                    .last_rtcp_failure
                    .or_else(|| conn.last_reject.map(reject_message))
                    .unwrap_or("tcp write failed"),
            }),
        }
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        // Inbound queue: mutex + condvar (no MPSC channel) —
        // `Condvar::wait_timeout(guard)` drops the mutex atomically and
        // re-acquires it on wake-up. The push path `push_inbound()`
        // holds the mutex only for "enqueue + notify_one"
        // (non-blocking towards the reader).
        //
        // A second without a datagram returns `Timeout`, as a socket's read
        // timeout does: whoever loops on `recv` (a layered transport's leg
        // thread, a runtime receive loop) checks its stop flag between
        // calls and is joined on shutdown. An unbounded `wait` kept that
        // thread, and the join, waiting for ever.
        let mut st = self.inbound.state.lock().map_err(|_| RecvError::Io {
            message: "inbound queue poisoned",
        })?;
        loop {
            if let Some(dg) = st.queue.pop_front() {
                return Ok(dg);
            }
            // An error, not `Timeout`: a caller that loops on `recv` until
            // its own stop flag pauses on it instead of spinning.
            if self.closed.load(Ordering::Acquire) {
                return Err(RecvError::Io {
                    message: "tcp transport closed",
                });
            }
            let (guard, wait) = self
                .inbound
                .cv
                .wait_timeout(st, core::time::Duration::from_secs(1))
                .map_err(|_| RecvError::Io {
                    message: "inbound queue poisoned",
                })?;
            st = guard;
            if wait.timed_out() && st.queue.is_empty() {
                return Err(RecvError::Timeout);
            }
        }
    }

    fn local_locator(&self) -> Locator {
        Self::local_locator(self)
    }

    fn close(&self) {
        Self::close(self);
    }
}

impl TcpTransport {
    /// The pool entry for `addr` in `dialect`, made when there is none.
    ///
    /// A destination reached in another dialect than the entry's (a Fast
    /// DDS locator at the address of another peer's listener, §6.1) gets
    /// an entry of its own dialect. The dialect of an entry is fixed when
    /// it is made; it is read under the entry's lock, which another send
    /// may hold for its I/O, so the pool lock is not held while waiting
    /// for it. An entry replaced meanwhile by another send is looked up
    /// again, at most [`PEER_ENTRY_ATTEMPTS`] times.
    fn peer_entry(
        &self,
        addr: SocketAddr,
        dialect: TcpDialect,
        wan: Option<SocketAddr>,
    ) -> Result<Arc<Mutex<PeerConn>>, SendError> {
        for _ in 0..PEER_ENTRY_ATTEMPTS {
            let candidate = self.pool_lookup(addr, dialect, wan)?;
            let known = candidate
                .lock()
                .map_err(|_| SendError::Io {
                    message: "peer conn poisoned",
                })?
                .dialect;
            if known == dialect {
                return Ok(candidate);
            }
            let mut pool = self.peers.lock().map_err(|_| SendError::Io {
                message: "peer pool poisoned",
            })?;
            if pool
                .get(&addr)
                .is_some_and(|entry| Arc::ptr_eq(entry, &candidate))
            {
                pool.remove(&addr);
            }
        }
        Err(SendError::Io {
            message: "tcp peer entry of another dialect replaced by concurrent sends",
        })
    }

    /// The pool entry for `addr` as it is, or a new one in `dialect`.
    fn pool_lookup(
        &self,
        addr: SocketAddr,
        dialect: TcpDialect,
        wan: Option<SocketAddr>,
    ) -> Result<Arc<Mutex<PeerConn>>, SendError> {
        let mut pool = self.peers.lock().map_err(|_| SendError::Io {
            message: "peer pool poisoned",
        })?;
        if pool.len() >= MAX_PEERS && !pool.contains_key(&addr) {
            // Pool eviction strategy: deterministic lowest-key
            // (smallest SocketAddrV4). A deliberate choice over LRU,
            // because:
            // - With MAX_PEERS=256 eviction pressure is low anyway
            //   (a real-world DDS domain rarely has >100 peers).
            // - Lowest-key is stable + deterministic — good for
            //   reproducible debugging.
            // - Avoids the IndexMap dep / per-peer timestamp-tracking
            //   overhead.
            if let Some(victim) = pool.keys().next().copied() {
                pool.remove(&victim);
            }
        }
        let claim = self.outgoing_claim();
        let own = self.rtcp_own();
        Ok(pool
            .entry(addr)
            .or_insert_with(|| {
                Arc::new(Mutex::new(
                    PeerConn::new(addr)
                        .claiming(claim)
                        .speaking(dialect)
                        .rtcp(own)
                        .with_wan(wan)
                        .reading_into(Arc::clone(&self.inbound), self.locator_kinds()),
                ))
            })
            .clone())
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// The address at which this host reaches a listener bound to `addr`:
/// loopback for the unspecified address.
fn wake_address(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V4(v4) if v4.ip().is_unspecified() => {
            SocketAddr::from((Ipv4Addr::LOCALHOST, v4.port()))
        }
        SocketAddr::V6(v6) if v6.ip().is_unspecified() => {
            SocketAddr::from((Ipv6Addr::LOCALHOST, v6.port()))
        }
        other => other,
    }
}

/// The key of a peer address in the pool and in the answer table: the
/// address and port alone, as [`locator_to_socket`] makes them.
fn pool_address(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V4(_) => addr,
        SocketAddr::V6(v6) => SocketAddr::V6(SocketAddrV6::new(*v6.ip(), v6.port(), 0, 0)),
    }
}

/// The source of a message of the dialect `fastdds` (§6.1): the peer's
/// listener `listener` with the logical port the message was for, as
/// Fast DDS names the source of a message; of kind 4 or 8, the kinds
/// whose locators hold a logical port.
fn rtcp_source(listener: SocketAddr, logical_port: u16) -> Locator {
    let mut locator = socket_locator(listener, TcpKindSet::Kinds4And8);
    locator.port |= u32::from(logical_port) << 16;
    locator
}

/// Where a `send` goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Destination {
    /// The socket address connected to.
    addr: SocketAddr,
    /// The logical port of a Fast DDS locator (§6.1).
    logical_port: Option<u16>,
    /// The WAN address of a Fast DDS TCPv4 locator (§6.1), where it is
    /// set and is not the address in the host's network.
    wan: Option<SocketAddr>,
}

/// The destination of a TCP locator (§1.1, §6.1). A locator of the kinds
/// 4 and 8 whose port is above 65535 is one of Fast DDS: the lower 16
/// bits are the physical port, the TCP port of the listener, the upper 16
/// the logical port. In a TCPv4 one of them the octets 12 to 15 are the
/// address connected to and 8 to 11 the WAN address. Every other TCP
/// locator names its address and port.
fn destination(loc: &Locator) -> Result<Destination, InvalidLocator> {
    if loc.kind.tcp_kind_set() == Some(TcpKindSet::Kinds4And8) && loc.port > u32::from(u16::MAX) {
        let physical = (loc.port & 0xFFFF) as u16;
        let logical = (loc.port >> 16) as u16;
        let (addr, wan) = if loc.kind.is_tcp_v4() {
            let a = &loc.address;
            let lan = Ipv4Addr::new(a[12], a[13], a[14], a[15]);
            let wan = Ipv4Addr::new(a[8], a[9], a[10], a[11]);
            let wan = (!wan.is_unspecified() && wan != lan)
                .then(|| SocketAddr::V4(SocketAddrV4::new(wan, physical)));
            (SocketAddr::V4(SocketAddrV4::new(lan, physical)), wan)
        } else {
            let ip = Ipv6Addr::from(loc.address);
            (SocketAddr::V6(SocketAddrV6::new(ip, physical, 0, 0)), None)
        };
        return Ok(Destination {
            addr,
            logical_port: Some(logical),
            wan,
        });
    }
    Ok(Destination {
        addr: locator_to_socket(loc)?,
        logical_port: None,
        wan: None,
    })
}

/// The TCP locator of a socket address, in the locator kinds `kinds`.
fn socket_locator(addr: SocketAddr, kinds: TcpKindSet) -> Locator {
    match addr {
        SocketAddr::V4(v4) => Locator::tcp_v4(v4.ip().octets(), u32::from(v4.port())),
        SocketAddr::V6(v6) => Locator::tcp_v6(v6.ip().octets(), u32::from(v6.port())),
    }
    .with_tcp_kind_set(kinds)
}

fn locator_to_socket_v4(loc: &Locator) -> Result<SocketAddrV4, InvalidLocator> {
    if !loc.kind.is_tcp_v4() {
        return Err(InvalidLocator::WrongKind);
    }
    let addr = &loc.address[12..16];
    let ip = Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]);
    let port =
        u16::try_from(loc.port).map_err(|_| InvalidLocator::PortOverflow { port: loc.port })?;
    Ok(SocketAddrV4::new(ip, port))
}

/// The socket address of a TCP locator of either kind set (§1.1): the
/// kinds of the ZeroDDS dialect and 4 and 8 name the same address
/// families.
fn locator_to_socket(loc: &Locator) -> Result<SocketAddr, InvalidLocator> {
    if loc.kind.is_tcp_v4() {
        locator_to_socket_v4(loc).map(SocketAddr::V4)
    } else if loc.kind.is_tcp_v6() {
        let port =
            u16::try_from(loc.port).map_err(|_| InvalidLocator::PortOverflow { port: loc.port })?;
        let ip = Ipv6Addr::from(loc.address);
        Ok(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, 0)))
    } else {
        Err(InvalidLocator::WrongKind)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::handshake::server_handshake;

    /// An idle `recv` returns `Timeout` after a second, so a thread
    /// looping on it sees its stop flag and can be joined.
    #[test]
    fn tcp_5_4_an_idle_recv_times_out() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let started = std::time::Instant::now();
        assert!(matches!(Transport::recv(&t), Err(RecvError::Timeout)));
        let waited = started.elapsed();
        assert!(
            waited >= Duration::from_millis(950) && waited < Duration::from_secs(3),
            "recv waited {waited:?}"
        );
    }

    /// Reads the frame number `numbered_frame` wrote.
    fn frame_number_of(dg: &ReceivedDatagram) -> u32 {
        u32::from_be_bytes(dg.data[..].try_into().unwrap())
    }

    fn numbered_frame(number: u32) -> ReceivedDatagram {
        ReceivedDatagram {
            source: Locator::tcp_v4([127, 0, 0, 1], 1),
            data: alloc::sync::Arc::<[u8]>::from(number.to_be_bytes().as_slice()).into(),
        }
    }

    /// A raw peer's listener on loopback and its port.
    fn raw_listener() -> (TcpListener, u16) {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    /// Whether the pool entry for `port` on loopback holds a connection,
    /// and its backoff in milliseconds.
    fn pool_entry(t: &TcpTransport, port: u16) -> Option<(bool, u64)> {
        let addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into();
        let pool = t.peers.lock().unwrap();
        let conn = pool.get(&addr)?.lock().unwrap();
        Some((conn.writer.is_some(), conn.backoff_ms))
    }

    #[test]
    fn bind_localhost_auto_port() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let loc = Transport::local_locator(&t);
        assert!(loc.port > 0);
    }

    #[test]
    fn send_to_nonexistent_returns_io_error() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dead = Locator::tcp_v4([127, 0, 0, 1], 1);
        let err = Transport::send(&t, &dead, b"hello").unwrap_err();
        assert!(matches!(err, SendError::Io { .. }));
    }

    #[test]
    fn unsupported_locator_invalid() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let err = Transport::send(&t, &Locator::INVALID, b"x").unwrap_err();
        assert!(matches!(err, SendError::UnsupportedLocator));
    }

    /// Five frames beyond the cap: five are counted, and the five that
    /// are gone are the oldest. The queue holds the frames 5 to cap + 4 in
    /// order.
    #[test]
    fn tcp_5_4_inbound_overflow_drops_oldest_and_counts() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let cap = u32::try_from(MAX_INBOUND_QUEUE).unwrap();
        for number in 0..cap + 5 {
            t.push_inbound(numbered_frame(number));
        }
        assert_eq!(t.dropped_frames(), 5);
        for expected in 5..cap + 5 {
            assert_eq!(frame_number_of(&t.try_recv().unwrap()), expected);
        }
        assert!(matches!(t.try_recv(), Err(RecvError::Timeout)));
        assert_eq!(t.dropped_frames(), 5);
    }

    /// `try_recv` on an empty queue returns `Timeout` (review finding:
    /// previously untested error path).
    #[test]
    fn try_recv_empty_queue_is_timeout() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert!(matches!(t.try_recv(), Err(RecvError::Timeout)));
    }

    /// `try_recv` after a push returns the frame.
    #[test]
    fn try_recv_returns_pushed_frame() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        t.push_inbound(ReceivedDatagram {
            source: Locator::tcp_v4([127, 0, 0, 1], 9),
            data: alloc::sync::Arc::<[u8]>::from([7u8, 8, 9].as_slice()).into(),
        });
        let dg = t.try_recv().unwrap();
        assert_eq!(&dg.data[..], &[7u8, 8, 9]);
    }

    /// `Transport::recv` is condvar-blocking. We test the happy path by
    /// pushing a frame from a background thread; the recv caller must
    /// wake up (covers the `inbound_cv.wait` branch).
    #[test]
    fn tcp_5_4_recv_wakes_on_push_from_other_thread() {
        use std::sync::Arc as StdArc;
        use std::thread;
        use std::time::Duration as StdDuration;

        let t = StdArc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let t2 = StdArc::clone(&t);
        let h = thread::spawn(move || {
            thread::sleep(StdDuration::from_millis(30));
            t2.push_inbound(ReceivedDatagram {
                source: Locator::tcp_v4([127, 0, 0, 1], 7),
                data: alloc::sync::Arc::<[u8]>::from([0xAAu8].as_slice()).into(),
            });
        });
        let dg = Transport::recv(&*t).unwrap();
        assert_eq!(&dg.data[..], &[0xAAu8]);
        h.join().unwrap();
    }

    /// `Transport::recv` for an already-filled queue runs loop → pop,
    /// the happy path without a condvar wait.
    #[test]
    fn recv_returns_immediately_if_queue_already_full() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        t.push_inbound(ReceivedDatagram {
            source: Locator::tcp_v4([127, 0, 0, 1], 1),
            data: alloc::sync::Arc::<[u8]>::from([1u8].as_slice()).into(),
        });
        let dg = Transport::recv(&t).unwrap();
        assert_eq!(&dg.data[..], &[1u8]);
    }

    /// MAX_PEERS eviction: we fill the pool directly via the mutex
    /// innards — that saves real sockets. On the next send to a new
    /// address the smallest key must be evicted.
    #[test]
    fn tcp_5_3_max_peers_eviction_removes_first_key_on_new_peer() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        {
            let mut pool = t.peers.lock().unwrap();
            for i in 0..MAX_PEERS {
                let port = u16::try_from(10_000 + i).unwrap();
                let addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into();
                pool.insert(addr, Arc::new(Mutex::new(PeerConn::new(addr))));
            }
            assert_eq!(pool.len(), MAX_PEERS);
        }

        // Port 1 is guaranteed not in the pool; send fails with Io,
        // but the eviction branch was hit before that.
        let fresh = Locator::tcp_v4([127, 0, 0, 1], 1);
        let _ = Transport::send(&t, &fresh, b"x");

        let pool = t.peers.lock().unwrap();
        assert_eq!(pool.len(), MAX_PEERS);
        assert!(pool.contains_key(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1))));
        // Port 1 is now the smallest element → the old 10_000+ are thus
        // no longer the smallest, but the very first (10_000) is still
        // evicted (because it was the smallest before insert).
        assert!(!pool.contains_key(&SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::LOCALHOST,
            10_000
        ))));
    }

    /// MAX_PEERS branch: if the target peer is already in the pool, no
    /// eviction may happen (the `&& !contains_key` gate kicks in).
    #[test]
    fn tcp_5_3_max_peers_no_eviction_for_existing_peer() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let target_addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 2).into();
        {
            let mut pool = t.peers.lock().unwrap();
            for i in 0..(MAX_PEERS - 1) {
                let port = u16::try_from(20_000 + i).unwrap();
                let a: SocketAddr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into();
                pool.insert(a, Arc::new(Mutex::new(PeerConn::new(a))));
            }
            pool.insert(
                target_addr,
                Arc::new(Mutex::new(PeerConn::new(target_addr))),
            );
            assert_eq!(pool.len(), MAX_PEERS);
        }
        let loc = Locator::tcp_v4([127, 0, 0, 1], 2);
        let _ = Transport::send(&t, &loc, b"x");
        let pool = t.peers.lock().unwrap();
        assert_eq!(pool.len(), MAX_PEERS);
        assert!(pool.contains_key(&target_addr));
    }

    /// TCPv6: bind + accept + send/recv via ::1.
    #[test]
    fn bind_v6_send_recv_loopback() {
        use std::sync::Arc as StdArc;
        let server = StdArc::new(TcpTransport::bind_v6(Ipv6Addr::LOCALHOST, 0).unwrap());
        let port = match server.local_locator().port {
            p if p > 0 && p < 65536 => p as u16,
            _ => panic!("invalid v6 port"),
        };
        let dest = Locator::tcp_v6(Ipv6Addr::LOCALHOST.octets(), u32::from(port));
        // Background accept thread (analogous to tcp_2_4_accept_one_reads_one_frame_then_eof).
        let server_for_accept = StdArc::clone(&server);
        let h = std::thread::spawn(move || {
            let _ = server_for_accept.accept_one();
        });
        // Client: bind its own v6 listener (for peers-pool symmetry) and send.
        let client = TcpTransport::bind_v6(Ipv6Addr::LOCALHOST, 0).unwrap();
        Transport::send(&client, &dest, b"hello-v6-tcp").expect("send v6");
        // accept-thread join: reads 1 frame and exits on EOF (drop of
        // the client socket after scope end).
        drop(client);
        let _ = h.join();
        let dg = Transport::recv(&*server).expect("recv v6 frame");
        assert_eq!(&*dg.data, b"hello-v6-tcp");
    }

    /// `PeerConn::bump_backoff` follows an exponential ramp with a cap.
    #[test]
    fn tcp_5_2_peer_conn_bump_backoff_sequence() {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1);
        let mut p = PeerConn::new(addr);
        assert_eq!(p.backoff_ms, 0);
        p.bump_backoff();
        assert_eq!(p.backoff_ms, INITIAL_BACKOFF_MS);
        p.bump_backoff();
        assert_eq!(p.backoff_ms, 2 * INITIAL_BACKOFF_MS);
        for _ in 0..20 {
            p.bump_backoff();
        }
        assert_eq!(p.backoff_ms, MAX_BACKOFF_MS);
    }

    /// `ensure_connected` enforces backoff throttling as long as the
    /// `last_attempt` window is still running (WouldBlock path).
    #[test]
    fn tcp_5_2_peer_conn_backoff_throttles_connect() {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1);
        let mut p = PeerConn::new(addr);
        p.backoff_ms = 5_000;
        p.last_attempt = Some(std::time::Instant::now());
        let err = p.ensure_connected(&PoolCounters::default()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
    }

    /// `drop_writer` on a freshly constructed PeerConn is a no-op
    /// (no writer present — covers the `if let Some` None branch).
    #[test]
    fn peer_conn_drop_writer_noop_without_connection() {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1);
        let mut p = PeerConn::new(addr);
        p.drop_writer();
        assert!(p.writer.is_none());
    }

    /// DDSI-RTPS 2.5 §9.3.2.4: the TCP kinds lie in the vendor range
    /// (0x01F0_0004, 0x01F0_0005); the values 4 and 8 of Cyclone DDS and
    /// Fast DDS are the other set (`with_locator_kinds`). The transport
    /// binds these two kinds, so a renumbering there has to break here.
    #[test]
    fn rtps_9_3_2_4_locator_kinds_are_zerodds_vendor_range_values() {
        assert_eq!(
            Locator::tcp_v4([127, 0, 0, 1], 7400).kind.as_i32(),
            0x01F0_0004
        );
        assert_eq!(Locator::tcp_v6([0u8; 16], 7400).kind.as_i32(), 0x01F0_0005);
    }

    /// §1.1: a transport announces the kinds of the ZeroDDS dialect unless
    /// it is made for 4 and 8; two transports of one process carry one set
    /// each, on both address families, and the source of a received frame
    /// has the set of the transport that received it.
    #[test]
    fn tcp_1_1_a_transport_announces_the_kinds_of_its_set() {
        use zerodds_rtps::wire_types::LocatorKind;

        let own = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let other = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_locator_kinds(TcpKindSet::Kinds4And8);
        assert_eq!(own.locator_kinds(), TcpKindSet::ZeroDds);
        assert_eq!(other.locator_kinds(), TcpKindSet::Kinds4And8);
        assert_eq!(own.local_locator().kind, LocatorKind::Tcpv4);
        assert_eq!(other.local_locator().kind, LocatorKind::Tcpv4Kind4);
        assert_eq!(
            own.local_locator().to_bytes_le()[..4],
            [0x04, 0x00, 0xF0, 0x01]
        );
        assert_eq!(other.local_locator().to_bytes_le()[..4], [4, 0, 0, 0]);
        assert_eq!(Transport::local_locator(&other), other.local_locator());
        // The address and the port are the listener's under either set.
        let port = other.listener.local_addr().unwrap().port();
        assert_eq!(other.local_locator().port, u32::from(port));
        assert_eq!(other.local_locator().ipv4(), [127, 0, 0, 1]);

        let own6 = TcpTransport::bind_v6(Ipv6Addr::LOCALHOST, 0).unwrap();
        let other6 = TcpTransport::bind_v6(Ipv6Addr::LOCALHOST, 0)
            .unwrap()
            .with_locator_kinds(TcpKindSet::Kinds4And8);
        assert_eq!(own6.local_locator().kind.as_i32(), 0x01F0_0005);
        assert_eq!(other6.local_locator().kind.as_i32(), 8);

        // A frame from `own` to `other`: its source carries the set of
        // the receiver.
        let h = std::thread::spawn(move || {
            other.accept_one().unwrap();
            other.try_recv().unwrap()
        });
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port))
            .with_tcp_kind_set(TcpKindSet::Kinds4And8);
        Transport::send(&own, &dest, b"x").unwrap();
        own.close();
        let dg = h.join().unwrap();
        assert_eq!(dg.source.kind, LocatorKind::Tcpv4Kind4);
        assert_eq!(&dg.data[..], b"x");
    }

    /// §1.1: a destination of either set reaches the same listener over
    /// one connection: the pool is keyed by the socket address, not by
    /// the kind.
    #[test]
    fn tcp_1_1_a_destination_of_either_kind_set_uses_one_connection() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let port = server.local_locator().port;
        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();

        let own = Locator::tcp_v4([127, 0, 0, 1], port);
        let other = own.with_tcp_kind_set(TcpKindSet::Kinds4And8);
        assert_ne!(own, other);
        Transport::send(&client, &own, b"by-own-kind").unwrap();
        Transport::send(&client, &other, b"by-kind-4").unwrap();
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"by-own-kind"[..])
        );
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"by-kind-4"[..])
        );
        assert_eq!(client.peers.lock().unwrap().len(), 1);
        assert_eq!(server.accepted.lock().unwrap().len(), 1);

        // TCPv6 kinds do not pass as TCPv4 and the reverse.
        let six = Locator::tcp_v6(Ipv6Addr::LOCALHOST.octets(), port)
            .with_tcp_kind_set(TcpKindSet::Kinds4And8);
        assert!(matches!(
            locator_to_socket_v4(&six),
            Err(InvalidLocator::WrongKind)
        ));
        assert!(matches!(locator_to_socket(&six), Ok(SocketAddr::V6(_))));
        assert!(matches!(locator_to_socket(&other), Ok(SocketAddr::V4(_))));

        client.close();
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// `locator_to_socket_v4` returns `WrongKind` for UDP.
    #[test]
    fn tcp_1_1_locator_to_socket_v4_rejects_udp() {
        let udp = Locator::udp_v4([127, 0, 0, 1], 7400);
        assert!(matches!(
            locator_to_socket_v4(&udp),
            Err(InvalidLocator::WrongKind)
        ));
    }

    /// `locator_to_socket_v4` maps TCPv4 to SocketAddrV4.
    #[test]
    fn tcp_1_1_locator_to_socket_v4_maps_tcp() {
        let tcp = Locator::tcp_v4([10, 0, 0, 1], 7400);
        let sa = locator_to_socket_v4(&tcp).unwrap();
        assert_eq!(sa.ip(), &Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(sa.port(), 7400);
    }

    /// `locator_to_socket_v4` rejects port > u16::MAX with `PortOverflow`.
    #[test]
    fn locator_to_socket_v4_rejects_oversized_port() {
        let bad = Locator::tcp_v4([127, 0, 0, 1], u32::from(u16::MAX) + 1);
        match locator_to_socket_v4(&bad) {
            Err(InvalidLocator::PortOverflow { port }) => {
                assert_eq!(port, u32::from(u16::MAX) + 1);
            }
            other => panic!("expected PortOverflow, got {other:?}"),
        }
    }

    /// `dropped_frames` starts at 0.
    #[test]
    fn tcp_5_4_dropped_frames_zero_on_fresh_transport() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert_eq!(t.dropped_frames(), 0);
    }

    /// Push keeps FIFO order for all frames below the queue cap.
    #[test]
    fn tcp_5_4_push_preserves_fifo_order() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        for i in 0..5u8 {
            t.push_inbound(ReceivedDatagram {
                source: Locator::tcp_v4([127, 0, 0, 1], 1),
                data: alloc::sync::Arc::<[u8]>::from([i].as_slice()).into(),
            });
        }
        for expected in 0..5u8 {
            let dg = t.try_recv().unwrap();
            assert_eq!(&dg.data[..], &[expected]);
        }
        assert!(matches!(t.try_recv(), Err(RecvError::Timeout)));
    }

    /// send with a PortOverflow locator → SendError::Io.
    #[test]
    fn send_port_overflow_returns_io_error() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let bad = Locator::tcp_v4([127, 0, 0, 1], u32::from(u16::MAX) + 1);
        let err = Transport::send(&t, &bad, b"x").unwrap_err();
        assert!(matches!(err, SendError::Io { .. }));
    }

    /// send via a UDP locator must not create a pool entry
    /// (UnsupportedLocator is thrown before the pool lookup).
    #[test]
    fn tcp_5_1_send_unsupported_locator_does_not_pollute_pool() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let udp = Locator::udp_v4([127, 0, 0, 1], 7400);
        let _ = Transport::send(&t, &udp, b"x");
        assert_eq!(t.peers.lock().unwrap().len(), 0);
    }

    /// `accept_one` on a listener with a closed client returns `Ok(())`
    /// on EOF after the first frame: the close on a frame boundary is the
    /// regular end and counts as nothing.
    #[test]
    fn tcp_2_4_accept_one_reads_one_frame_then_eof() {
        use std::thread;

        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();

        let h = thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            // Complete the handshake so accept_one accepts the peer as
            // a ZeroDDS TCP peer.
            crate::handshake::client_handshake(&mut s, 0).unwrap();
            // A valid length-prefixed frame (BE length + payload).
            let payload = b"abc";
            let len = u32::try_from(payload.len()).unwrap();
            s.write_all(&len.to_be_bytes()).unwrap();
            s.write_all(payload).unwrap();
            s.shutdown(std::net::Shutdown::Both).unwrap();
        });

        server.accept_one().unwrap();
        let dg = server.try_recv().unwrap();
        assert_eq!(&dg.data[..], b"abc");
        assert_eq!(server.truncated_frames(), 0);
        h.join().unwrap();
    }

    /// Connects to `port`, completes the handshake, writes `wire` and
    /// closes the connection.
    fn peer_that_writes(port: u16, wire: &'static [u8]) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            crate::handshake::client_handshake(&mut s, 0).unwrap();
            s.write_all(wire).unwrap();
            s.shutdown(std::net::Shutdown::Both).unwrap();
        })
    }

    fn assert_unexpected_eof(result: Result<(), TcpTransportError>) {
        match result {
            Err(TcpTransportError::PeerIo(e)) => {
                assert_eq!(e.kind(), std::io::ErrorKind::UnexpectedEof);
            }
            other => panic!("expected PeerIo(UnexpectedEof), got {other:?}"),
        }
    }

    /// The peer announces ten octets, sends three and closes: `accept_one`
    /// reports it, nothing of the frame reaches the queue, and it is
    /// counted.
    #[test]
    fn tcp_2_4_accept_one_reports_eof_mid_body() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();
        let h = peer_that_writes(port, &[0, 0, 0, 10, 1, 2, 3]);

        assert_unexpected_eof(server.accept_one());
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        assert_eq!(server.truncated_frames(), 1);
        h.join().unwrap();
    }

    /// The peer sends one whole frame, then two octets of the next length
    /// and closes: the whole frame is delivered, the cut one is reported
    /// and counted.
    #[test]
    fn tcp_2_4_accept_one_reports_eof_mid_header() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();
        let h = peer_that_writes(port, &[0, 0, 0, 3, b'a', b'b', b'c', 0, 0]);

        assert_unexpected_eof(server.accept_one());
        assert_eq!(&server.try_recv().unwrap().data[..], b"abc");
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        assert_eq!(server.truncated_frames(), 1);
        h.join().unwrap();
    }

    /// `accept_one` with a peer that announces a frame larger than the
    /// cap returns `FrameTooLarge`.
    #[test]
    fn tcp_2_3_accept_one_frame_too_large_errors() {
        use std::thread;

        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();

        let h = thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            crate::handshake::client_handshake(&mut s, 0).unwrap();
            let len = crate::framing::MAX_FRAME_SIZE + 1;
            let _ = s.write_all(&len.to_be_bytes());
            let _ = s.shutdown(std::net::Shutdown::Both);
        });

        let err = server.accept_one().unwrap_err();
        assert!(matches!(err, TcpTransportError::FrameTooLarge { .. }));
        h.join().unwrap();
    }

    /// `accept_one` with a peer that sends **no** handshake (e.g. a
    /// broken HTTP probe) rejects with `TcpTransportError::Handshake`.
    #[test]
    fn tcp_3_4_accept_one_rejects_peer_without_handshake() {
        use std::thread;

        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();

        let h = thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            // Speaks like HTTP — wrong magic bytes.
            let _ = s.write_all(b"GET / HTTP/1.1\r\n\r\n12345");
            let _ = s.shutdown(std::net::Shutdown::Both);
        });

        let err = server.accept_one().unwrap_err();
        assert!(matches!(err, TcpTransportError::Handshake(_)));
        h.join().unwrap();
    }

    /// Slow-loris in the accept path: a peer opens the connection, sends
    /// one handshake byte and then stalls. `accept_one` must give up on
    /// its own — the per-syscall read slice is 200 ms and the total
    /// handshake budget 2 s, so the accept thread cannot be pinned.
    #[test]
    fn tcp_3_8_accept_one_gives_up_on_a_stalling_handshake_peer() {
        use std::sync::mpsc;
        use std::thread;

        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();

        // The child holds the connection open until the server has given
        // up; without the channel its drop would close the socket and the
        // server would see EOF instead of the stall.
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let h = thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            // One byte of a 16-byte handshake, then silence.
            let _ = s.write_all(b"Z");
            let _ = s.flush();
            let _ = release_rx.recv();
        });

        let start = std::time::Instant::now();
        let err = server.accept_one().unwrap_err();
        let elapsed = start.elapsed();
        let _ = release_tx.send(());
        h.join().unwrap();

        assert!(
            matches!(err, TcpTransportError::Handshake(_)),
            "expected a handshake error, got {err:?}"
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "a silent peer is given up on after one slice, took {elapsed:?}"
        );
    }

    /// The request of §3.1 for version `major`.0, vendor id of ZeroDDS, no
    /// flags, no logical port.
    fn request_octets(major: u8) -> [u8; 16] {
        [
            b'Z', b'D', b'D', b'S', major, 0, 0x01, 0xF0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    }

    /// A peer that sends a valid request one octet per 150 ms needs at
    /// least 2.25 s for it. Every read comes within the 200 ms slice, so
    /// only the total budget of 2 s ends it: `accept_one` returns
    /// `TimedOut` when the budget is used up and has written no response.
    /// The time taken shows that the budget is not shorter than 2 s. The
    /// error shows that it is shorter than the time the peer needs: with
    /// a longer budget, or with a timeout per read only, the request would
    /// be complete and `accept_one` would succeed. No wall-clock bound
    /// above: on a loaded runner the return comes later than the
    /// deadline.
    #[test]
    fn tcp_3_8_accept_one_gives_up_on_a_peer_sending_octet_by_octet() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();

        let h = std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            s.set_nodelay(true).unwrap();
            for octet in request_octets(1) {
                // The server closes while the request is on its way.
                if s.write_all(&[octet]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut response = [0u8; 16];
            s.read(&mut response).unwrap_or(0)
        });

        let start = Instant::now();
        let err = server.accept_one().unwrap_err();
        let elapsed = start.elapsed();
        match err {
            TcpTransportError::Handshake(HandshakeError::Io(e)) => {
                assert_eq!(e.kind(), std::io::ErrorKind::TimedOut);
            }
            other => panic!("expected a timed-out handshake, got {other:?}"),
        }
        assert!(
            elapsed >= Duration::from_millis(1900),
            "the budget is 2 s, accept_one took {elapsed:?}"
        );
        assert_eq!(h.join().unwrap(), 0, "the peer read response octets");
    }

    /// The same on the connecting side: a listener that answers the
    /// request one octet per 150 ms, which takes it at least 2.25 s. The
    /// send fails when the 2 s budget is used up and keeps no connection.
    /// As above, the time taken bounds the budget from below and the
    /// failure bounds it from above: a budget longer than the listener
    /// needs would let the handshake complete and the send succeed.
    #[test]
    fn tcp_3_8_the_client_gives_up_on_a_listener_answering_octet_by_octet() {
        let (listener, port) = raw_listener();
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.set_nodelay(true).unwrap();
            let mut request = [0u8; 16];
            s.read_exact(&mut request).unwrap();
            // BindConnectionResponse: Accept.
            for octet in [
                b'Z', b'D', b'A', b'+', 1, 0, 0x01, 0xF0, 0, 0, 0, 0, 0, 0, 0, 0,
            ] {
                if s.write_all(&[octet]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        });

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        let start = Instant::now();
        let result = Transport::send(&client, &dest, b"x");
        let elapsed = start.elapsed();
        assert!(matches!(result, Err(SendError::Io { .. })), "{result:?}");
        assert!(
            elapsed >= Duration::from_millis(1900),
            "the budget is 2 s, the send took {elapsed:?}"
        );
        assert_eq!(pool_entry(&client, port), Some((false, INITIAL_BACKOFF_MS)));
        h.join().unwrap();
    }

    /// §3.4, the server after a Reject: the peer reads the Reject and then
    /// the end of the connection, and a frame that stood behind the
    /// rejected request does not reach the inbound queue.
    #[test]
    fn tcp_3_4_the_server_closes_after_a_reject() {
        use crate::handshake::RejectReason;

        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();

        // A request of version 2.0; the peer reads to the end.
        let h = std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            s.write_all(&request_octets(2)).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut response = [0u8; 16];
            s.read_exact(&mut response).unwrap();
            let mut more = [0u8; 1];
            (response, s.read(&mut more).map_err(|e| e.kind()))
        });
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::Handshake(HandshakeError::Rejected {
                    reason: RejectReason::VersionMismatch
                })
            ),
            "{err:?}"
        );
        let (response, after) = h.join().unwrap();
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[12..], 1u32.to_be_bytes());
        assert_eq!(after, Ok(0), "the server closed after the Reject");

        // The same request with a frame right behind it, in one write.
        let h = std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            let mut wire = request_octets(2).to_vec();
            wire.extend_from_slice(&[0, 0, 0, 3, b'a', b'b', b'c']);
            s.write_all(&wire).unwrap();
            // Held open until the server has decided.
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut sink = [0u8; 32];
            while matches!(s.read(&mut sink), Ok(n) if n > 0) {}
        });
        let err = server.accept_one().unwrap_err();
        assert!(matches!(
            err,
            TcpTransportError::Handshake(HandshakeError::Rejected { .. })
        ));
        h.join().unwrap();
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
    }

    /// Sends `wire` to `port` in one write, reads the 16 octets of the
    /// response and then to the end of the connection: the response and
    /// what the read after it returned.
    fn raw_handshake_peer(
        port: u16,
        wire: Vec<u8>,
    ) -> std::thread::JoinHandle<([u8; 16], Result<usize, std::io::ErrorKind>)> {
        std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            s.write_all(&wire).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut response = [0u8; 16];
            s.read_exact(&mut response).unwrap();
            let mut more = [0u8; 1];
            (response, s.read(&mut more).map_err(|e| e.kind()))
        })
    }

    /// §3.3 over a real connection: a listener with a vendor list answers
    /// a request of another vendor with `ZDA-` and reason code 4 and
    /// closes; `client_handshake` reads it as `VendorNotAccepted`; a frame
    /// behind the refused request does not reach the queue; a listed
    /// vendor is read.
    #[test]
    fn tcp_3_3_accept_one_rejects_an_unlisted_vendor_over_a_real_connection() {
        use crate::handshake::RejectReason;

        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_accepted_vendors(vec![[0x01, 0x10]]);
        assert_eq!(server.accepted_vendors(), Some(&[[0x01, 0x10]][..]));
        let port = u16::try_from(server.local_locator().port).unwrap();

        // The ZeroDDS client: its vendor id 01 F0 is not on the list.
        let h = std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let result = crate::handshake::client_handshake(&mut s, 0);
            let mut more = [0u8; 1];
            (result, s.read(&mut more).map_err(|e| e.kind()))
        });
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::Handshake(HandshakeError::Rejected {
                    reason: RejectReason::VendorNotAccepted
                })
            ),
            "{err:?}"
        );
        let (result, after) = h.join().unwrap();
        assert!(
            matches!(
                result,
                Err(HandshakeError::Rejected {
                    reason: RejectReason::VendorNotAccepted
                })
            ),
            "{result:?}"
        );
        assert_eq!(after, Ok(0), "the server closed after the Reject");

        // The octets on the wire, with a frame behind the request.
        let mut wire = request_octets(1).to_vec();
        wire.extend_from_slice(&[0, 0, 0, 3, b'a', b'b', b'c']);
        let h = raw_handshake_peer(port, wire);
        assert!(server.accept_one().is_err());
        let (response, _) = h.join().unwrap();
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[12..], 4u32.to_be_bytes());
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));

        // A request of the listed vendor with the same frame.
        let mut wire = request_octets(1).to_vec();
        wire[6..8].copy_from_slice(&[0x01, 0x10]);
        wire.extend_from_slice(&[0, 0, 0, 3, b'a', b'b', b'c']);
        let h = std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            s.write_all(&wire).unwrap();
            let mut response = [0u8; 16];
            s.read_exact(&mut response).unwrap();
            response
        });
        server.accept_one().unwrap();
        assert_eq!(&h.join().unwrap()[..4], b"ZDA+");
        assert_eq!(&server.try_recv().unwrap().data[..], b"abc");
    }

    /// §3.5 over a real connection: a request with a reserved flag set is
    /// answered `ZDA-` with reason code 5, the server closes, and a frame
    /// behind the request does not reach the inbound queue.
    #[test]
    fn tcp_3_5_accept_one_rejects_flags_over_a_real_connection() {
        use crate::handshake::RejectReason;

        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();

        let mut wire = request_octets(1).to_vec();
        wire[11] = 0x01;
        wire.extend_from_slice(&[0, 0, 0, 3, b'a', b'b', b'c']);
        let h = raw_handshake_peer(port, wire);
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::Handshake(HandshakeError::Rejected {
                    reason: RejectReason::ReservedFlagsSet
                })
            ),
            "{err:?}"
        );
        let (response, after) = h.join().unwrap();
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[8..12], [0, 0, 0, 0]);
        assert_eq!(response[12..], 5u32.to_be_bytes());
        // The frame behind the request is unread when the server closes:
        // the peer sees the end of the connection or its reset.
        assert!(
            matches!(after, Ok(0) | Err(std::io::ErrorKind::ConnectionReset)),
            "{after:?}"
        );
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
    }

    /// §3.5 on the connecting side: a listener that accepts with a
    /// reserved flag set gets no frame; the client closes, keeps no
    /// connection and backs off.
    #[test]
    fn tcp_3_5_the_client_drops_a_connection_accepted_with_reserved_flags() {
        let (listener, port) = raw_listener();
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut request = [0u8; 16];
            s.read_exact(&mut request).unwrap();
            // BindConnectionResponse: Accept, flags = 0x0000_0002.
            s.write_all(&[
                b'Z', b'D', b'A', b'+', 1, 0, 0x01, 0xF0, 0, 0, 0, 2, 0, 0, 0, 0,
            ])
            .unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut more = [0u8; 8];
            s.read(&mut more).map_err(|e| e.kind())
        });

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        let result = Transport::send(&client, &dest, b"x");
        assert!(matches!(result, Err(SendError::Io { .. })), "{result:?}");
        assert_eq!(
            h.join().unwrap(),
            Ok(0),
            "the client closed without a frame"
        );
        assert_eq!(pool_entry(&client, port), Some((false, INITIAL_BACKOFF_MS)));
    }

    // ---- §2.5: the dialect `rtps` ----

    use crate::rtps_stream::tests::{INFO_TS, stream_message};

    /// A transport that opens and takes connections of the dialect
    /// `rtps` only.
    fn rtps_transport() -> TcpTransport {
        TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_dialect(TcpDialect::Rtps)
            .with_accepted_dialects(&[TcpDialect::Rtps])
    }

    /// Connects to `port`, writes `wire` and closes.
    fn raw_peer_that_writes(port: u16, wire: Vec<u8>) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            s.write_all(&wire).unwrap();
            // A listener that refuses the stream closes with input unread,
            // which resets the connection: the shutdown then has nothing
            // to shut down, and that is no failure of the peer.
            let _ = s.shutdown(std::net::Shutdown::Write);
            // Held until the reader is done with the connection.
            let mut sink = [0u8; 16];
            while matches!(s.read(&mut sink), Ok(n) if n > 0) {}
        })
    }

    /// §2.5 over real sockets: two transports of the dialect exchange
    /// messages that are delimited by their HeaderExtension only. On the
    /// wire there is no handshake and no length in front: the octets a
    /// raw listener reads are the messages themselves, back to back. The
    /// receiver hands each message on unchanged.
    #[test]
    fn tcp_2_5_two_transports_exchange_messages_in_the_rtps_dialect() {
        assert_eq!(
            TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .dialect(),
            TcpDialect::ZeroDds
        );
        let little = stream_message([0x01, 0xF0], true, &INFO_TS);
        let big = stream_message([0x01, 0xF0], false, &[0x5A; 3000]);

        // What goes on the wire.
        let (listener, port) = raw_listener();
        let expected = little.len() + big.len();
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut wire = vec![0u8; expected];
            s.read_exact(&mut wire).unwrap();
            // Nothing is written back and nothing more comes.
            s.set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            let mut more = [0u8; 1];
            (wire, s.read(&mut more).map_err(|e| e.kind()))
        });
        let client = rtps_transport();
        assert_eq!(client.dialect(), TcpDialect::Rtps);
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        Transport::send(&client, &dest, &little).unwrap();
        Transport::send(&client, &dest, &big).unwrap();
        let (wire, more) = h.join().unwrap();
        assert_eq!(&wire[..4], b"RTPS");
        assert_eq!(wire[..little.len()], little[..]);
        assert_eq!(wire[little.len()..], big[..]);
        assert!(
            matches!(
                more,
                Err(std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
            ),
            "{more:?}"
        );

        // Between two transports.
        let server = Arc::new(rtps_transport());
        let done = serve_in_background(&server);
        let dest = server.local_locator();
        Transport::send(&client, &dest, &big).unwrap();
        Transport::send(&client, &dest, &little).unwrap();
        let first = datagram_within(&server, Duration::from_secs(5)).unwrap();
        let second = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&first.data[..], &big[..]);
        assert_eq!(&second.data[..], &little[..]);
        // No request, so no claim: the source is the port the client
        // connected from.
        assert_ne!(first.source.port, client.local_locator().port);
        assert_eq!(first.source.ipv4(), [127, 0, 0, 1]);
        assert!(server.claimed_logical_ports().is_empty());
        assert_eq!(server.connection_errors(), 0);
        client.close();
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §2.5 on the sending side: the transport adds nothing to a message.
    /// One without a HeaderExtension that states its length is refused
    /// before a connection is made; the next message that states it goes
    /// out.
    #[test]
    fn tcp_2_5_send_refuses_a_message_without_length() {
        let (listener, port) = raw_listener();
        listener.set_nonblocking(true).unwrap();
        let client = rtps_transport();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));

        let good = stream_message([0x01, 0xF0], true, &INFO_TS);
        let mut plain = good[..20].to_vec();
        plain.extend_from_slice(&INFO_TS);
        assert_eq!(
            Transport::send(&client, &dest, &plain),
            Err(SendError::Io {
                message: "tcp message without a HeaderExtension messageLength"
            })
        );
        assert_eq!(
            Transport::send(&client, &dest, b"abc"),
            Err(SendError::Io {
                message: "tcp message without a HeaderExtension messageLength"
            })
        );
        let mut longer = good.clone();
        longer.push(0);
        assert_eq!(
            Transport::send(&client, &dest, &longer),
            Err(SendError::Io {
                message: "tcp message states a HeaderExtension messageLength that is not its length"
            })
        );
        assert!(client.peers.lock().unwrap().is_empty());
        assert!(
            matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
            "a refused message made a connection"
        );
        // The default dialect frames the same octets (§2.1); the check is
        // the dialect's.
        Transport::send(&client, &dest, &good).unwrap();
        let (mut s, _) = listener.accept().unwrap();
        s.set_nonblocking(false).unwrap();
        let mut wire = vec![0u8; good.len()];
        s.read_exact(&mut wire).unwrap();
        assert_eq!(wire, good);
    }

    /// §2.5 on the accepting side: a connection that carries something
    /// else than messages with their length is closed with its error and
    /// nothing of it reaches the queue; the messages before it stand. A
    /// listener that takes the dialect `zerodds` only refuses a stream of
    /// the dialect `rtps`, and the reverse (§4.2).
    #[test]
    fn tcp_2_5_a_connection_that_is_no_rtps_stream_is_closed() {
        let server = rtps_transport();
        let port = u16::try_from(server.local_locator().port).unwrap();
        let good = stream_message([0x01, 0xF0], true, &INFO_TS);

        // One message with its length, then one without.
        let mut wire = good.clone();
        wire.extend_from_slice(&good[..20]);
        wire.extend_from_slice(&INFO_TS);
        wire.extend_from_slice(&INFO_TS);
        let h = raw_peer_that_writes(port, wire);
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::StreamFraming(FramingError::NoMessageLength)
            ),
            "{err:?}"
        );
        drop(server.try_recv().unwrap());
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        h.join().unwrap();

        // The request of §3.1 and a frame of §2.1.
        let mut wire = request_octets(1).to_vec();
        wire.extend_from_slice(&[0, 0, 0, 12]);
        wire.extend_from_slice(&INFO_TS);
        let h = raw_peer_that_writes(port, wire);
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::StreamFraming(FramingError::NotRtps { got }) if &got == b"ZDDS"
            ),
            "{err:?}"
        );
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        h.join().unwrap();

        // A message cut inside: counted as a truncated frame (§2.4).
        let h = raw_peer_that_writes(port, good[..33].to_vec());
        assert_unexpected_eof(server.accept_one());
        assert_eq!(server.truncated_frames(), 1);
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        h.join().unwrap();

        // The reverse: a listener for the dialect `zerodds` only and an
        // RTPS stream.
        let zerodds = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_accepted_dialects(&[TcpDialect::ZeroDds]);
        let port = u16::try_from(zerodds.local_locator().port).unwrap();
        let h = raw_peer_that_writes(port, good);
        let err = zerodds.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::Handshake(HandshakeError::BadMagic { got }) if &got == b"RTPS"
            ),
            "{err:?}"
        );
        assert!(matches!(zerodds.try_recv(), Err(RecvError::Timeout)));
        h.join().unwrap();
    }

    /// §3.3 in the dialect `rtps`, which has no request: the vendor list
    /// is held against the vendor id of each message's RTPS header. A
    /// message of another vendor ends the connection and is not handed
    /// on.
    #[test]
    fn tcp_2_5_the_vendor_list_applies_to_the_rtps_header() {
        let server = rtps_transport().with_accepted_vendors(vec![[0x01, 0x10]]);
        let port = u16::try_from(server.local_locator().port).unwrap();
        let listed = stream_message([0x01, 0x10], true, &INFO_TS);
        let unlisted = stream_message([0x01, 0xF0], true, &INFO_TS);

        let mut wire = listed.clone();
        wire.extend_from_slice(&unlisted);
        wire.extend_from_slice(&listed);
        let h = raw_peer_that_writes(port, wire);
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::VendorNotAccepted {
                    vendor: [0x01, 0xF0]
                }
            ),
            "{err:?}"
        );
        assert_eq!(&server.try_recv().unwrap().data[..], &listed[..]);
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        h.join().unwrap();

        // Without a list the same stream is read to its end.
        let open = rtps_transport();
        let port = u16::try_from(open.local_locator().port).unwrap();
        let mut wire = listed.clone();
        wire.extend_from_slice(&unlisted);
        let h = raw_peer_that_writes(port, wire);
        open.accept_one().unwrap();
        assert_eq!(&open.try_recv().unwrap().data[..], &listed[..]);
        assert_eq!(&open.try_recv().unwrap().data[..], &unlisted[..]);
        h.join().unwrap();
    }

    /// §3.7 in the dialect `rtps`: there is no response to refuse with. A
    /// connection beyond the limit is closed at once and counted as
    /// refused and as unanswered.
    #[test]
    fn tcp_2_5_a_connection_beyond_the_limit_is_closed_without_an_answer() {
        let server = Arc::new(rtps_transport().with_accept_limit(1));
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);

        let mut first = TcpStream::connect(addr).unwrap();
        let message = stream_message([0x01, 0xF0], true, &INFO_TS);
        first.write_all(&message).unwrap();
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&message[..])
        );
        let mut second = TcpStream::connect(addr).unwrap();
        second
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut octet = [0u8; 1];
        assert_eq!(second.read(&mut octet).map_err(|e| e.kind()), Ok(0));
        assert_eq!(server.refused_connections(), 1);
        assert_eq!(server.refused_unanswered(), 1);
        // The first connection is still read.
        first.write_all(&message).unwrap();
        assert!(recv_within(&server, Duration::from_secs(5)).is_some());
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    // ---- §3.6: the logical-port claim ----

    /// The request of §3.1 with `logical_port`.
    fn claiming_request(logical_port: u32) -> Vec<u8> {
        let mut wire = request_octets(1).to_vec();
        wire[12..16].copy_from_slice(&logical_port.to_be_bytes());
        wire
    }

    /// A peer that connects to `port`, sends the request claiming
    /// `logical_port` and the frames `wire`, reads the response and holds
    /// the connection until `release` is dropped or sent to.
    fn claiming_peer(
        port: u16,
        logical_port: u32,
        wire: &'static [u8],
    ) -> (
        std::sync::mpsc::Sender<()>,
        std::thread::JoinHandle<[u8; 16]>,
    ) {
        let (release, held) = std::sync::mpsc::channel::<()>();
        let h = std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            s.write_all(&claiming_request(logical_port)).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut response = [0u8; 16];
            s.read_exact(&mut response).unwrap();
            if &response[..4] == b"ZDA+" {
                s.write_all(wire).unwrap();
            }
            let _ = held.recv();
            response
        });
        (release, h)
    }

    /// Waits until `condition` holds, at most 5 s.
    fn wait_for(what: &str, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < deadline, "{what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// §3.6: while a connection holds a logical port, a second connection
    /// from the same address that claims it is answered `ZDA-` with reason
    /// code 3 and closed; its frame does not arrive. The first connection
    /// stays and is read.
    #[test]
    fn tcp_3_6_a_second_connection_claiming_a_bound_port_is_rejected() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();

        let (hold_first, first) = claiming_peer(port, 7410, &[0, 0, 0, 1, b'1']);
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"1"[..])
        );
        assert_eq!(
            server.claimed_logical_ports(),
            vec![(IpAddr::V4(Ipv4Addr::LOCALHOST), 7410)]
        );

        let mut wire = claiming_request(7410);
        wire.extend_from_slice(&[0, 0, 0, 1, b'2']);
        let (response, after) = raw_handshake_peer(port, wire).join().unwrap();
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[12..], 3u32.to_be_bytes());
        assert!(
            matches!(after, Ok(0) | Err(std::io::ErrorKind::ConnectionReset)),
            "{after:?}"
        );
        wait_for("the refused connection is counted", || {
            server.connection_errors() == 1
        });
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        // The refused connection took nothing from the one that holds the
        // port.
        assert_eq!(server.claimed_logical_ports().len(), 1);
        assert_eq!(server.accepted.lock().unwrap().len(), 1);

        drop(hold_first);
        assert_eq!(&first.join().unwrap()[..4], b"ZDA+");
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §3.6: the claim ends with the connection. Once the holder has
    /// closed, the same port is granted again.
    #[test]
    fn tcp_3_6_the_claim_is_released_when_the_connection_ends() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();

        let (hold_first, first) = claiming_peer(port, 7410, &[]);
        wait_for("the first claim is held", || {
            server.claimed_logical_ports().len() == 1
        });
        drop(hold_first);
        assert_eq!(&first.join().unwrap()[..4], b"ZDA+");
        wait_for("the claim is released", || {
            server.claimed_logical_ports().is_empty()
        });

        let (hold_second, second) = claiming_peer(port, 7410, &[0, 0, 0, 1, b'2']);
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"2"[..])
        );
        drop(hold_second);
        assert_eq!(&second.join().unwrap()[..4], b"ZDA+");

        // A connection refused for another reason has claimed nothing:
        // version 2.0 with the same port, then the port is still free.
        wait_for("the second claim is released", || {
            server.claimed_logical_ports().is_empty()
        });
        let mut wire = claiming_request(7410);
        wire[4] = 2;
        let (response, _) = raw_handshake_peer(port, wire).join().unwrap();
        assert_eq!(response[12..], 1u32.to_be_bytes());
        assert!(server.claimed_logical_ports().is_empty());

        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §3.6: 0 claims nothing. Two connections with `logical_port = 0` are
    /// both read, and no claim is held.
    #[test]
    fn tcp_3_6_zero_is_no_claim() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();

        let (hold_a, a) = claiming_peer(port, 0, &[0, 0, 0, 1, b'a']);
        let (hold_b, b) = claiming_peer(port, 0, &[0, 0, 0, 1, b'b']);
        let mut got = vec![
            recv_within(&server, Duration::from_secs(5)).unwrap(),
            recv_within(&server, Duration::from_secs(5)).unwrap(),
        ];
        got.sort();
        assert_eq!(got, vec![b"a".to_vec(), b"b".to_vec()]);
        assert!(server.claimed_logical_ports().is_empty());
        assert_eq!(server.accepted.lock().unwrap().len(), 2);
        drop((hold_a, hold_b));
        assert_eq!(&a.join().unwrap()[..4], b"ZDA+");
        assert_eq!(&b.join().unwrap()[..4], b"ZDA+");
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §3.6 over a real connection: a request that claims 65536 reads
    /// `ZDA-` with reason code 3 and the end of the connection; the
    /// listener holds no claim and hands on no frame of it. A claim of
    /// 65535 is granted, and the frames of that connection carry it as
    /// the port of their source.
    #[test]
    fn tcp_3_6_a_claim_above_65535_is_rejected_over_a_real_connection() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();

        let mut wire = claiming_request(65_536);
        wire.extend_from_slice(&[0, 0, 0, 1, b'x']);
        let peer = raw_handshake_peer(port, wire);
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::Handshake(HandshakeError::Rejected {
                    reason: RejectReason::LogicalPortConflict
                })
            ),
            "{err:?}"
        );
        let (response, after) = peer.join().unwrap();
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[12..], 3u32.to_be_bytes());
        assert!(
            matches!(after, Ok(0) | Err(std::io::ErrorKind::ConnectionReset)),
            "{after:?}"
        );
        assert!(server.claimed_logical_ports().is_empty());
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));

        let h = std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            crate::handshake::client_handshake(&mut s, 65_535).unwrap();
            s.write_all(&[0, 0, 0, 1, b'y']).unwrap();
        });
        server.accept_one().unwrap();
        h.join().unwrap();
        let frame = server.try_recv().unwrap();
        assert_eq!(&frame.data[..], b"y");
        assert_eq!(frame.source.port, 65_535);
    }

    /// §3.6: the frames of a connection that claimed a port carry the
    /// peer's address with the claimed port as their source; those of a
    /// connection without a claim carry the port it connected from.
    #[test]
    fn tcp_3_6_frames_of_a_claimed_connection_carry_the_claimed_port_as_source() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_locator_kinds(TcpKindSet::Kinds4And8);
        let port = u16::try_from(server.local_locator().port).unwrap();

        let h = std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            crate::handshake::client_handshake(&mut s, 7410).unwrap();
            s.write_all(&[0, 0, 0, 1, b'c']).unwrap();
            s.local_addr().unwrap().port()
        });
        server.accept_one().unwrap();
        let ephemeral = h.join().unwrap();
        assert_ne!(u32::from(ephemeral), 7410);
        let dg = server.try_recv().unwrap();
        assert_eq!(
            dg.source,
            Locator::tcp_v4([127, 0, 0, 1], 7410).with_tcp_kind_set(TcpKindSet::Kinds4And8)
        );
        assert!(
            server.claimed_logical_ports().is_empty(),
            "released at the end"
        );

        let h = std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            crate::handshake::client_handshake(&mut s, 0).unwrap();
            s.write_all(&[0, 0, 0, 1, b'n']).unwrap();
            s.local_addr().unwrap().port()
        });
        server.accept_one().unwrap();
        let ephemeral = h.join().unwrap();
        let dg = server.try_recv().unwrap();
        assert_eq!(dg.source.port, u32::from(ephemeral));
    }

    /// §3.6 on the connecting side: the request of a transport carries the
    /// port of its own listener in octets 12 to 15, so the source of its
    /// frames at the other end is its listener's locator, and a reply sent
    /// to that source arrives. With the claim turned off the request
    /// carries 0.
    #[test]
    fn tcp_3_6_the_client_claims_its_listener_port() {
        let (listener, port) = raw_listener();
        let h = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut s, _) = listener.accept().unwrap();
                let mut request = [0u8; 16];
                s.read_exact(&mut request).unwrap();
                s.write_all(&[
                    b'Z', b'D', b'A', b'+', 1, 0, 0x01, 0xF0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])
                .unwrap();
                requests.push(request);
            }
            requests
        });
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        let claiming = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert!(claiming.claims_logical_port());
        Transport::send(&claiming, &dest, b"x").unwrap();
        let silent = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_logical_port_claim(false);
        assert!(!silent.claims_logical_port());
        Transport::send(&silent, &dest, b"x").unwrap();
        let requests = h.join().unwrap();
        assert_ne!(claiming.local_locator().port, 0);
        assert_eq!(
            requests[0][12..],
            claiming.local_locator().port.to_be_bytes()
        );
        assert_eq!(requests[1][12..], [0, 0, 0, 0]);

        // Between two transports: the source of a frame is the sender's
        // listener, and a frame sent back to it arrives there.
        let a = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let b = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done_a = serve_in_background(&a);
        let done_b = serve_in_background(&b);
        Transport::send(&*a, &b.local_locator(), b"ping").unwrap();
        let ping = datagram_within(&b, Duration::from_secs(5)).unwrap();
        assert_eq!(ping.source, a.local_locator());
        Transport::send(&*b, &ping.source, b"pong").unwrap();
        assert_eq!(
            recv_within(&a, Duration::from_secs(5)).as_deref(),
            Some(&b"pong"[..])
        );
        a.close();
        b.close();
        done_a.recv_timeout(Duration::from_secs(5)).unwrap();
        done_b.recv_timeout(Duration::from_secs(5)).unwrap();
    }

    /// §3.6: a claim is of one peer address on one listener address. The
    /// same port from another peer address, or on another address of the
    /// listener, is granted; the same triple is not, until its holder is
    /// gone, and a connection releases only its own claim.
    #[test]
    fn tcp_3_6_the_same_port_from_another_address_is_accepted() {
        let claims: ClaimTable = Mutex::new(BTreeMap::new());
        let local = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let other_local = IpAddr::V4(Ipv4Addr::new(10, 0, 1, 1));
        let peer = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7));
        let other_peer = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 8));
        let guard = |connection| ClaimGuard {
            claims: &claims,
            connection,
            held: None,
        };
        let mut first = guard(1);
        assert!(first.claim((local, peer, 7410)));
        let mut second = guard(2);
        assert!(!second.claim((local, peer, 7410)));
        assert!(second.claim((local, other_peer, 7410)));
        let mut third = guard(3);
        assert!(third.claim((other_local, peer, 7410)));
        let mut fourth = guard(4);
        assert!(fourth.claim((local, peer, 7411)));
        assert_eq!(claims.lock().unwrap().len(), 4);
        // A connection that was refused holds nothing and releases
        // nothing.
        let mut refused = guard(5);
        assert!(!refused.claim((local, peer, 7410)));
        drop(refused);
        assert_eq!(claims.lock().unwrap().get(&(local, peer, 7410)), Some(&1));
        drop(first);
        assert!(guard(6).claim((local, peer, 7410)));
    }

    /// §3.6 over real connections from two addresses of this host: the
    /// same logical port is held by 127.0.0.1 and by 127.0.0.2 at once.
    #[cfg(target_os = "linux")]
    #[test]
    fn tcp_3_6_two_peer_addresses_hold_the_same_port_at_once() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::UNSPECIFIED, 0).unwrap());
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();

        let connect_from = |source: Ipv4Addr| {
            let socket = socket2::Socket::new(
                socket2::Domain::IPV4,
                socket2::Type::STREAM,
                Some(socket2::Protocol::TCP),
            )
            .unwrap();
            socket.bind(&SocketAddr::from((source, 0)).into()).unwrap();
            socket
                .connect(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)).into())
                .unwrap();
            let mut s = TcpStream::from(socket);
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            s.write_all(&claiming_request(7410)).unwrap();
            let mut response = [0u8; 16];
            s.read_exact(&mut response).unwrap();
            (s, response)
        };
        let (_one, response) = connect_from(Ipv4Addr::LOCALHOST);
        assert_eq!(&response[..4], b"ZDA+");
        let (mut two, response) = connect_from(Ipv4Addr::new(127, 0, 0, 2));
        assert_eq!(&response[..4], b"ZDA+");
        two.write_all(&[0, 0, 0, 1, b'2']).unwrap();
        let dg = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(dg.source, Locator::tcp_v4([127, 0, 0, 2], 7410));
        let mut held = server.claimed_logical_ports();
        held.sort();
        assert_eq!(
            held,
            vec![
                (IpAddr::V4(Ipv4Addr::LOCALHOST), 7410),
                (IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)), 7410)
            ]
        );
        // A third connection from 127.0.0.2 with the port is refused.
        let (_three, response) = connect_from(Ipv4Addr::new(127, 0, 0, 2));
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[12..], 3u32.to_be_bytes());
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §3.6: an accepted connection has TCP keepalive on, so a peer that
    /// vanished without a FIN does not hold its claim for ever.
    #[test]
    fn tcp_3_6_an_accepted_connection_has_keepalive_on() {
        let (listener, port) = raw_listener();
        let client = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
        let (accepted, _) = listener.accept().unwrap();
        assert!(!socket2::SockRef::from(&accepted).keepalive().unwrap());
        set_keepalive(&accepted).unwrap();
        let socket = socket2::SockRef::from(&accepted);
        assert!(socket.keepalive().unwrap());
        #[cfg(target_os = "linux")]
        {
            assert_eq!(socket.keepalive_time().unwrap(), Duration::from_secs(10));
            assert_eq!(socket.keepalive_interval().unwrap(), Duration::from_secs(5));
            assert_eq!(socket.keepalive_retries().unwrap(), 3);
        }
        drop(client);

        // A connection a listener reads has it, in both dialects.
        for dialect in [TcpDialect::ZeroDds, TcpDialect::Rtps] {
            let server = Arc::new(
                TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                    .unwrap()
                    .with_dialect(dialect),
            );
            let done = serve_in_background(&server);
            let port = u16::try_from(server.local_locator().port).unwrap();
            let mut peer =
                TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            // The connection stays: read after the handshake, or as a
            // stream of messages that has not begun.
            if dialect == TcpDialect::ZeroDds {
                crate::handshake::client_handshake(&mut peer, 0).unwrap();
            }
            wait_for("the connection has keepalive on", || {
                let open = server.accepted.lock().unwrap();
                open.len() == 1
                    && open
                        .values()
                        .all(|stream| socket2::SockRef::from(stream).keepalive().unwrap())
            });
            drop(peer);
            server.close();
            done.recv_timeout(Duration::from_secs(5))
                .expect("serve returns after close");
        }
    }

    /// §3.6 on the connecting side: after a Reject with
    /// `LogicalPortConflict` the client keeps no connection, counts the
    /// Reject, names it in the error and tries again after the backoff;
    /// the second attempt, which the listener accepts, delivers the frame.
    #[test]
    fn tcp_3_6_the_client_retries_after_a_logical_port_conflict() {
        let (listener, port) = raw_listener();
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut request = [0u8; 16];
            s.read_exact(&mut request).unwrap();
            s.write_all(&[
                b'Z', b'D', b'A', b'-', 1, 0, 0x01, 0xF0, 0, 0, 0, 0, 0, 0, 0, 3,
            ])
            .unwrap();
            drop(s);
            let (mut s, _) = listener.accept().unwrap();
            server_handshake(&mut s).unwrap();
            read_frame(&mut s).unwrap()
        });
        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        let result = Transport::send(&client, &dest, b"first");
        assert_eq!(
            result,
            Err(SendError::Io {
                message: "tcp peer rejected: logical port conflict"
            })
        );
        assert_eq!(
            client.connect_rejections(RejectReason::LogicalPortConflict),
            1
        );
        assert_eq!(pool_entry(&client, port), Some((false, INITIAL_BACKOFF_MS)));
        let deadline = Instant::now() + Duration::from_secs(5);
        while Transport::send(&client, &dest, b"second").is_err() {
            assert!(Instant::now() < deadline, "the retry does not connect");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(h.join().unwrap(), b"second");
        assert_eq!(pool_entry(&client, port), Some((true, 0)));
        assert_eq!(
            client.connect_rejections(RejectReason::LogicalPortConflict),
            1
        );
    }

    // ---- §3.7: the Reject at the peer limit ----

    /// §3.7: a listener that reads as many connections as it may answers
    /// the next one `ZDA-` with reason code 2 and closes it; the
    /// connections it reads stay.
    #[test]
    fn tcp_3_7_serve_answers_resource_limit_beyond_the_peer_limit() {
        let server = Arc::new(
            TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_accept_limit(2),
        );
        assert_eq!(server.accept_limit(), 2);
        assert_eq!(
            TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .accept_limit(),
            MAX_PEERS
        );
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();

        let (hold_a, a) = claiming_peer(port, 0, &[0, 0, 0, 1, b'a']);
        let (hold_b, b) = claiming_peer(port, 0, &[0, 0, 0, 1, b'b']);
        for _ in 0..2 {
            assert!(recv_within(&server, Duration::from_secs(5)).is_some());
        }

        let (response, after) = raw_handshake_peer(port, request_octets(1).to_vec())
            .join()
            .unwrap();
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[4..8], [1, 0, 0x01, 0xF0]);
        assert_eq!(response[12..], 2u32.to_be_bytes());
        assert_eq!(after, Ok(0), "the listener closed after the Reject");
        assert_eq!(server.refused_connections(), 1);
        assert_eq!(server.refused_unanswered(), 0);
        assert_eq!(server.connection_errors(), 0);
        assert_eq!(server.accepted.lock().unwrap().len(), 2);

        // `client_handshake` reads it as the reason.
        let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let result = crate::handshake::client_handshake(&mut s, 0);
        assert!(
            matches!(
                result,
                Err(HandshakeError::Rejected {
                    reason: RejectReason::ResourceLimit
                })
            ),
            "{result:?}"
        );
        // A request the listener would refuse below its limit as well
        // gets that reason (§3.3): version 2.0.
        let (response, _) = raw_handshake_peer(port, request_octets(2).to_vec())
            .join()
            .unwrap();
        assert_eq!(response[12..], 1u32.to_be_bytes());
        assert_eq!(server.refused_connections(), 3);
        assert_eq!(server.refused_unanswered(), 0);

        drop((hold_a, hold_b));
        assert_eq!(&a.join().unwrap()[..4], b"ZDA+");
        assert_eq!(&b.join().unwrap()[..4], b"ZDA+");
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §3.7: the accepting loop reads no octet of a refused connection.
    /// Eight peers beyond the limit that send nothing take the refusing
    /// thread 250 ms each until their answer is overdue; a connection
    /// that finds a place in the meantime is accepted and read before
    /// they are done.
    #[test]
    fn tcp_3_7_a_silent_refused_peer_does_not_hold_the_accept_loop() {
        let server = Arc::new(
            TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_accept_limit(1),
        );
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);

        let (hold_a, a) = claiming_peer(port, 0, &[]);
        wait_for("the first connection is read", || {
            server.accepted.lock().unwrap().len() == 1
        });
        let silent: Vec<TcpStream> = (0..8).map(|_| TcpStream::connect(addr).unwrap()).collect();
        wait_for("the silent connections are refused", || {
            server.refused_connections() == 8
        });
        drop(hold_a);
        assert_eq!(&a.join().unwrap()[..4], b"ZDA+");
        wait_for("the place is free", || {
            server.accepted.lock().unwrap().is_empty()
        });

        let (hold_b, b) = claiming_peer(port, 0, &[0, 0, 0, 1, b'b']);
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"b"[..])
        );
        let unanswered_then = server.refused_unanswered();
        assert!(
            unanswered_then < 8,
            "the new connection was read only after all {unanswered_then} silent ones were given up on"
        );
        // Each silent peer is given up on and counted.
        wait_for("the silent connections are closed", || {
            server.refused_unanswered() == 8
        });
        for mut peer in silent {
            peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut octet = [0u8; 1];
            assert_eq!(
                peer.read(&mut octet).map_err(|e| e.kind()),
                Ok(0),
                "a silent refused peer reads the end of the connection, no response"
            );
        }
        drop(hold_b);
        assert_eq!(&b.join().unwrap()[..4], b"ZDA+");
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §3.7: sixteen refused connections wait for their answer at most.
    /// The seventeenth is closed without one and counted as unanswered;
    /// the sixteen stay open.
    #[test]
    fn tcp_3_7_beyond_the_refuse_queue_a_connection_is_closed_unanswered() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let (listener, port) = raw_listener();
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        // A queue nobody takes from.
        let (queue, _undrained) = std::sync::mpsc::sync_channel::<Refused>(REFUSE_QUEUE);
        let mut peers = Vec::new();
        for _ in 0..=REFUSE_QUEUE {
            peers.push(TcpStream::connect(addr).unwrap());
            let (accepted, _) = listener.accept().unwrap();
            server.refuse_beyond_limit(&queue, accepted);
        }
        assert_eq!(server.refused_connections(), 17);
        assert_eq!(server.refused_unanswered(), 1);
        let mut octet = [0u8; 1];
        let mut last = peers.pop().unwrap();
        last.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        assert_eq!(last.read(&mut octet).map_err(|e| e.kind()), Ok(0));
        for mut peer in peers {
            peer.set_read_timeout(Some(Duration::from_millis(20)))
                .unwrap();
            let kind = peer.read(&mut octet).map_err(|e| e.kind());
            assert!(
                matches!(
                    kind,
                    Err(std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
                ),
                "a queued connection is open: {kind:?}"
            );
        }
    }

    /// §3.7: a client beyond the limit whose connection waits behind
    /// silent refused ones gets its Reject while it still waits for it.
    /// Three silent connections ahead cost the refusing thread 250 ms
    /// each; the send of the client then names `ResourceLimit`, within
    /// the 2 s the client gives the handshake, and the client backs off
    /// a second. The silent ones are counted as unanswered, the client's
    /// connection is not.
    #[test]
    fn tcp_3_7_a_client_behind_silent_refused_connections_is_answered_in_time() {
        let server = Arc::new(
            TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_accept_limit(1),
        );
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        let (hold_a, a) = claiming_peer(port, 0, &[]);
        wait_for("the first connection is read", || {
            server.accepted.lock().unwrap().len() == 1
        });
        let silent: Vec<TcpStream> = (0..3).map(|_| TcpStream::connect(addr).unwrap()).collect();
        wait_for("the silent connections are refused", || {
            server.refused_connections() == 3
        });

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let began = Instant::now();
        assert_eq!(
            Transport::send(&client, &server.local_locator(), b"x"),
            Err(SendError::Io {
                message: "tcp peer rejected: resource limit",
            })
        );
        let waited = began.elapsed();
        assert!(
            waited < CLIENT_HANDSHAKE_BUDGET,
            "the Reject came after {waited:?}"
        );
        assert_eq!(client.connect_rejections(RejectReason::ResourceLimit), 1);
        assert_eq!(
            pool_entry(&client, port),
            Some((false, RESOURCE_LIMIT_BACKOFF_MS))
        );
        // The three silent ones were given up on before the client's turn.
        assert_eq!(server.refused_connections(), 4);
        assert_eq!(server.refused_unanswered(), 3);
        drop(silent);
        drop(hold_a);
        assert_eq!(&a.join().unwrap()[..4], b"ZDA+");
        client.close();
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §3.7: a refused connection is answered until 1.5 s after it was
    /// accepted. One whose turn comes at that instant or later is closed
    /// without an answer, though its request is there, and counted as
    /// unanswered: the client's 2 s for the handshake end soon after, and
    /// a Reject written then reaches nobody.
    #[test]
    fn tcp_3_7_a_refused_connection_whose_turn_comes_too_late_is_closed_unanswered() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();

        // In time.
        let peer = raw_handshake_peer(port, request_octets(1).to_vec());
        let (accepted, _) = server.listener.accept().unwrap();
        server.answer_resource_limit(&accepted, Instant::now() + REFUSE_DEADLINE);
        let (response, after) = peer.join().unwrap();
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[12..], 2u32.to_be_bytes());
        assert_eq!(after, Ok(0));
        assert_eq!(server.refused_unanswered(), 0);

        // Too late.
        let mut late = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
        late.write_all(&request_octets(1)).unwrap();
        let (accepted, _) = server.listener.accept().unwrap();
        server.answer_resource_limit(&accepted, Instant::now());
        assert_eq!(server.refused_unanswered(), 1);
        late.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut response = [0u8; 16];
        let got = late.read(&mut response).map_err(|e| e.kind());
        // Closed with the request unread: the end of the connection, or
        // its reset.
        assert!(
            matches!(got, Ok(0) | Err(std::io::ErrorKind::ConnectionReset)),
            "a connection answered too late got {got:?}"
        );
    }

    /// §3.7: a connection that has its place and no thread to read it,
    /// because the thread could not be started, is refused like one
    /// beyond the limit. Its place is free again, its answer is due
    /// within 1.5 s, and the peer reads Reject/`ResourceLimit`. Without a
    /// registered stream there is nothing to answer on: counted as
    /// refused and as unanswered.
    #[test]
    fn tcp_3_7_a_connection_without_a_reading_thread_is_refused() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();
        let (queue, waiting) = std::sync::mpsc::sync_channel::<Refused>(REFUSE_QUEUE);

        let peer = raw_handshake_peer(port, request_octets(1).to_vec());
        let (accepted, _) = server.listener.accept().unwrap();
        server.accepted.lock().unwrap().insert(7, accepted);
        let before = Instant::now();
        server.refuse_unstarted(&queue, 7);
        assert!(server.accepted.lock().unwrap().is_empty());
        assert_eq!(server.refused_connections(), 1);
        let (stream, due) = waiting.try_recv().unwrap();
        assert!(
            due >= before + REFUSE_DEADLINE,
            "the answer is due too early"
        );
        assert!(due <= Instant::now() + REFUSE_DEADLINE);
        server.answer_resource_limit(&stream, due);
        let (response, after) = peer.join().unwrap();
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[12..], 2u32.to_be_bytes());
        assert_eq!(after, Ok(0), "the listener closed after the Reject");
        assert_eq!(server.refused_unanswered(), 0);

        server.refuse_unstarted(&queue, 8);
        assert_eq!(server.refused_connections(), 2);
        assert_eq!(server.refused_unanswered(), 1);
        assert!(waiting.try_recv().is_err());
    }

    /// §3.7 on the connecting side: the Reject of a full listener is told
    /// from a connection error. The send names it, the transport counts
    /// it, and the next attempt waits a second; the send that is
    /// throttled by that wait names the Reject too and connects nothing.
    #[test]
    fn tcp_3_7_the_client_counts_the_resource_limit_reject_and_backs_off_one_second() {
        let server = Arc::new(
            TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_accept_limit(1),
        );
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();
        let (hold_a, a) = claiming_peer(port, 0, &[]);
        wait_for("the first connection is read", || {
            server.accepted.lock().unwrap().len() == 1
        });

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = server.local_locator();
        let full = Err(SendError::Io {
            message: "tcp peer rejected: resource limit",
        });
        assert_eq!(Transport::send(&client, &dest, b"x"), full);
        assert_eq!(client.connect_rejections(RejectReason::ResourceLimit), 1);
        assert_eq!(client.connect_rejections(RejectReason::Unknown), 0);
        assert_eq!(pool_entry(&client, port), Some((false, 1_000)));
        assert_eq!(server.refused_connections(), 1);
        // Throttled: no second connection reaches the listener.
        assert_eq!(Transport::send(&client, &dest, b"x"), full);
        assert_eq!(client.connect_rejections(RejectReason::ResourceLimit), 1);
        assert_eq!(server.refused_connections(), 1);

        // A connection error is not a Reject: nothing listens on port 1.
        let dead = Locator::tcp_v4([127, 0, 0, 1], 1);
        assert_eq!(
            Transport::send(&client, &dead, b"x"),
            Err(SendError::Io {
                message: "tcp write failed"
            })
        );
        assert_eq!(pool_entry(&client, 1), Some((false, INITIAL_BACKOFF_MS)));

        // The place frees; after the second the client is in.
        drop(hold_a);
        assert_eq!(&a.join().unwrap()[..4], b"ZDA+");
        let waited = Instant::now();
        let deadline = waited + Duration::from_secs(5);
        while Transport::send(&client, &dest, b"in").is_err() {
            assert!(Instant::now() < deadline, "the client does not get in");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"in"[..])
        );
        assert_eq!(pool_entry(&client, port), Some((true, 0)));
        client.close();
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §3.7: the limit counts the connections that are read. When one
    /// ends, the next connection gets its place.
    #[test]
    fn tcp_3_7_a_connection_ended_frees_a_place() {
        let server = Arc::new(
            TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_accept_limit(1),
        );
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();

        let (hold_a, a) = claiming_peer(port, 0, &[0, 0, 0, 1, b'a']);
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"a"[..])
        );
        let (response, _) = raw_handshake_peer(port, request_octets(1).to_vec())
            .join()
            .unwrap();
        assert_eq!(&response[..4], b"ZDA-");
        assert_eq!(response[12..], 2u32.to_be_bytes());

        drop(hold_a);
        assert_eq!(&a.join().unwrap()[..4], b"ZDA+");
        wait_for("the place is free", || {
            server.accepted.lock().unwrap().is_empty()
        });
        let (hold_c, c) = claiming_peer(port, 0, &[0, 0, 0, 1, b'c']);
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"c"[..])
        );
        assert_eq!(server.refused_connections(), 1);
        drop(hold_c);
        assert_eq!(&c.join().unwrap()[..4], b"ZDA+");
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §3.4, the client after a Reject: it closes the connection without
    /// a frame on it and keeps none in the pool; the next attempt waits
    /// for the backoff.
    #[test]
    fn tcp_3_4_the_client_keeps_no_connection_after_a_reject() {
        let (listener, port) = raw_listener();
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut request = [0u8; 16];
            s.read_exact(&mut request).unwrap();
            // BindConnectionResponse: Reject, VersionMismatch.
            s.write_all(&[
                b'Z', b'D', b'A', b'-', 1, 0, 0x01, 0xF0, 0, 0, 0, 0, 0, 0, 0, 1,
            ])
            .unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut more = [0u8; 8];
            (request, s.read(&mut more).map_err(|e| e.kind()))
        });

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        let result = Transport::send(&client, &dest, b"x");
        assert!(matches!(result, Err(SendError::Io { .. })), "{result:?}");

        let (request, after) = h.join().unwrap();
        assert_eq!(request[..12], request_octets(1)[..12]);
        assert_eq!(after, Ok(0), "the client closed without sending a frame");
        assert_eq!(pool_entry(&client, port), Some((false, INITIAL_BACKOFF_MS)));
    }

    /// §2.4 under `serve`: a connection closed on a frame boundary is no
    /// error; one closed inside a frame is counted as a connection error
    /// and as a truncated frame.
    #[test]
    fn tcp_2_4_serve_counts_a_connection_that_ended_inside_a_frame() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();

        peer_that_writes(port, &[0, 0, 0, 3, b'a', b'b', b'c'])
            .join()
            .unwrap();
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"abc"[..])
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !server.accepted.lock().unwrap().is_empty() {
            assert!(
                Instant::now() < deadline,
                "the clean connection is still read"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(server.connection_errors(), 0);
        assert_eq!(server.truncated_frames(), 0);

        peer_that_writes(port, &[0, 0, 0, 10, 1, 2, 3])
            .join()
            .unwrap();
        while server.connection_errors() == 0 {
            assert!(
                Instant::now() < deadline,
                "the cut connection is not counted"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(server.connection_errors(), 1);
        assert_eq!(server.truncated_frames(), 1);
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));

        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// Runs `serve` on a thread of its own; the receiver yields once
    /// `serve` has returned.
    fn serve_in_background(server: &Arc<TcpTransport>) -> std::sync::mpsc::Receiver<()> {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let server = Arc::clone(server);
        std::thread::spawn(move || {
            server.serve();
            let _ = done_tx.send(());
        });
        done_rx
    }

    fn recv_within(t: &TcpTransport, bound: Duration) -> Option<alloc::vec::Vec<u8>> {
        datagram_within(t, bound).map(|dg| dg.data.to_vec())
    }

    fn datagram_within(t: &TcpTransport, bound: Duration) -> Option<ReceivedDatagram> {
        let deadline = std::time::Instant::now() + bound;
        while std::time::Instant::now() < deadline {
            if let Ok(dg) = Transport::recv(t) {
                return Some(dg);
            }
        }
        None
    }

    /// Two peers connected at the same time are both read. `accept_one`
    /// in a loop read the second only after the first had disconnected.
    #[test]
    fn serve_reads_two_peers_connected_at_once() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let dest = Locator::tcp_v4([127, 0, 0, 1], server.local_locator().port);

        let a = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let b = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        Transport::send(&a, &dest, b"from-a").unwrap();
        Transport::send(&b, &dest, b"from-b").unwrap();

        let mut got = alloc::vec::Vec::new();
        for _ in 0..2 {
            got.push(recv_within(&server, Duration::from_secs(5)).expect("a frame"));
        }
        got.sort();
        assert_eq!(got, [b"from-a".to_vec(), b"from-b".to_vec()]);

        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
        drop((a, b));
    }

    /// `close` ends `serve` while a peer holds its connection open, and
    /// that peer reads EOF at once.
    #[test]
    fn close_ends_serve_and_the_peer_reads_eof() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();

        let mut peer = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
        crate::handshake::client_handshake(&mut peer, 0).unwrap();
        peer.write_all(&3u32.to_be_bytes()).unwrap();
        peer.write_all(b"abc").unwrap();
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).as_deref(),
            Some(&b"abc"[..])
        );

        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut buf = [0u8; 8];
        assert_eq!(peer.read(&mut buf).unwrap(), 0, "the peer reads EOF");
    }

    /// `close` ends a `serve` that has never accepted a connection.
    #[test]
    fn close_ends_an_idle_serve() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::UNSPECIFIED, 0).unwrap());
        let done = serve_in_background(&server);
        std::thread::sleep(Duration::from_millis(20));
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// `close` shuts the outgoing connections down: the receiving side's
    /// reading thread ends instead of waiting for this process to end.
    #[test]
    fn close_shuts_the_outgoing_connections_down() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let dest = Locator::tcp_v4([127, 0, 0, 1], server.local_locator().port);

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        Transport::send(&client, &dest, b"x").unwrap();
        assert!(recv_within(&server, Duration::from_secs(5)).is_some());
        assert_eq!(server.accepted.lock().unwrap().len(), 1);

        client.close();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !server.accepted.lock().unwrap().is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "the server still reads the closed connection"
            );
            std::thread::sleep(Duration::from_millis(5));
        }

        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// A listener that takes the connection and never answers the
    /// handshake fails the send instead of holding it for ever.
    #[test]
    fn a_listener_that_never_answers_the_handshake_fails_the_send() {
        let silent = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = silent.local_addr().unwrap().port();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
            let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
            let _ = result_tx.send(Transport::send(&client, &dest, b"x"));
        });
        let result = result_rx
            .recv_timeout(Duration::from_secs(6))
            .expect("the send returns");
        assert!(matches!(result, Err(SendError::Io { .. })), "{result:?}");
        drop(silent);
    }

    /// A closed transport neither sends nor waits in `recv`: `recv`
    /// returns an error at once, not `Timeout` after a second.
    #[test]
    fn tcp_5_4_a_closed_transport_neither_sends_nor_waits() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        t.close();
        let dest = Locator::tcp_v4([127, 0, 0, 1], t.local_locator().port);
        assert!(matches!(
            Transport::send(&t, &dest, b"x"),
            Err(SendError::Io { .. })
        ));
        let start = std::time::Instant::now();
        assert!(matches!(Transport::recv(&t), Err(RecvError::Io { .. })));
        assert!(start.elapsed() < Duration::from_millis(500));
    }

    /// A successful connect sets the backoff back to 0. The peer's
    /// listener stays bound for the whole test: its first connection is
    /// closed without an answer, which fails the handshake and sets the
    /// backoff to its start value; its second is answered.
    #[test]
    fn tcp_5_2_backoff_resets_after_successful_connect() {
        let (listener, port) = raw_listener();
        let peer = std::thread::spawn(move || {
            let (unanswered, _) = listener.accept().unwrap();
            drop(unanswered);
            let (mut answered, _) = listener.accept().unwrap();
            server_handshake(&mut answered).unwrap();
            read_frame(&mut answered).unwrap()
        });

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        let first = Transport::send(&client, &dest, b"a");
        assert!(matches!(first, Err(SendError::Io { .. })), "{first:?}");
        assert_eq!(pool_entry(&client, port), Some((false, INITIAL_BACKOFF_MS)));

        std::thread::sleep(Duration::from_millis(INITIAL_BACKOFF_MS + 30));
        Transport::send(&client, &dest, b"b").expect("send after the backoff");
        assert_eq!(pool_entry(&client, port), Some((true, 0)));
        assert_eq!(peer.join().unwrap(), b"b");
    }

    /// §5.1: the peer closes the connection after the first frame. The
    /// next send sees the close before it writes, connects anew in the
    /// same call, and the frame arrives on the second connection. Written
    /// into the closed connection it would be taken by the kernel and
    /// lost, with `Ok` returned.
    #[test]
    fn tcp_5_1_send_after_peer_close_reconnects_instead_of_writing_into_the_closed_connection() {
        let (listener, port) = raw_listener();
        let (closed_tx, closed_rx) = std::sync::mpsc::channel::<()>();
        let peer = std::thread::spawn(move || {
            let (mut one, _) = listener.accept().unwrap();
            server_handshake(&mut one).unwrap();
            let first = read_frame(&mut one).unwrap();
            drop(one);
            closed_tx.send(()).unwrap();

            // The second connection, if the client makes one in time.
            listener.set_nonblocking(true).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            let second = loop {
                match listener.accept() {
                    Ok((mut two, _)) => {
                        two.set_nonblocking(false).unwrap();
                        two.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                        server_handshake(&mut two).unwrap();
                        break Some(read_frame(&mut two).unwrap());
                    }
                    Err(_) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break None,
                }
            };
            (first, second)
        });

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        Transport::send(&client, &dest, b"first").unwrap();
        closed_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the peer closes its first connection");
        // Until the peer's FIN has arrived on this side.
        let addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into();
        let conn = Arc::clone(client.peers.lock().unwrap().get(&addr).unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !peer_closed(conn.lock().unwrap().writer.as_ref().unwrap().get_ref()) {
            assert!(Instant::now() < deadline, "the FIN did not arrive");
            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(client.peer_closed_connections(), 0);
        Transport::send(&client, &dest, b"second").expect("the send reconnects");
        let (first, second) = peer.join().unwrap();
        assert_eq!(first, b"first");
        assert_eq!(second.as_deref(), Some(&b"second"[..]));
        assert_eq!(client.peers.lock().unwrap().len(), 1);
        assert_eq!(pool_entry(&client, port), Some((true, 0)));
        // The replaced connection is counted.
        assert_eq!(client.peer_closed_connections(), 1);
    }

    /// §5.1: a frame written before the peer's close has arrived is handed
    /// to the TCP stack and `send` returns `Ok`; the peer closes without
    /// reading it and the frame is lost. The following send finds the
    /// connection closed, counts it and connects anew; the lost frame is
    /// not sent again, the second connection begins with the frame of
    /// that send.
    #[test]
    fn tcp_5_1_a_frame_written_before_the_close_arrived_is_lost_and_the_connection_counted() {
        let (listener, port) = raw_listener();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
        let (written_tx, written_rx) = std::sync::mpsc::channel::<()>();
        let (closed_tx, closed_rx) = std::sync::mpsc::channel::<()>();
        let peer = std::thread::spawn(move || {
            let (mut one, _) = listener.accept().unwrap();
            server_handshake(&mut one).unwrap();
            ready_tx.send(()).unwrap();
            // Closes after the client's write, without reading the frame.
            written_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            drop(one);
            closed_tx.send(()).unwrap();

            let (mut two, _) = listener.accept().unwrap();
            two.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            server_handshake(&mut two).unwrap();
            read_frame(&mut two).unwrap()
        });

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        Transport::send(&client, &dest, b"lost").expect("the open connection takes the frame");
        ready_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the peer has answered the handshake");
        // A send over the open connection counts nothing.
        Transport::send(&client, &dest, b"lost too").expect("the open connection is reused");
        assert_eq!(client.peer_closed_connections(), 0);
        written_tx.send(()).unwrap();
        closed_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the peer closes its first connection");
        // Until the peer's close (a reset: it had unread data) has arrived.
        let addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into();
        let conn = Arc::clone(client.peers.lock().unwrap().get(&addr).unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !peer_closed(conn.lock().unwrap().writer.as_ref().unwrap().get_ref()) {
            assert!(Instant::now() < deadline, "the close did not arrive");
            std::thread::sleep(Duration::from_millis(5));
        }

        Transport::send(&client, &dest, b"next").expect("the send reconnects");
        assert_eq!(peer.join().unwrap(), b"next");
        assert_eq!(client.peer_closed_connections(), 1);
        assert_eq!(pool_entry(&client, port), Some((true, 0)));
    }

    /// §5.1: the look before a write returns at once, takes nothing out of
    /// the receive side and leaves the socket blocking. Octets the peer
    /// wrote (a peer of §3 writes none after the handshake) are no close;
    /// the peer's close behind them is seen once they are read.
    #[test]
    fn tcp_5_1_the_look_for_a_close_takes_no_octet_and_keeps_the_socket_blocking() {
        let (listener, port) = raw_listener();
        let mut near = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
        let (mut far, _) = listener.accept().unwrap();
        near.set_read_timeout(Some(Duration::from_millis(300)))
            .unwrap();

        // Nothing to read, not closed: the look does not wait for the
        // read timeout.
        let started = Instant::now();
        assert!(!peer_closed(&near));
        assert!(started.elapsed() < Duration::from_millis(200));
        // The socket is still blocking: a read waits for its timeout,
        // where a non-blocking socket answers at once.
        let started = Instant::now();
        let mut one = [0u8; 1];
        let idle = near.read(&mut one).unwrap_err();
        assert!(matches!(
            idle.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ));
        assert!(started.elapsed() >= Duration::from_millis(250));

        near.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        far.write_all(b"xy").unwrap();
        far.flush().unwrap();
        near.read_exact(&mut one).unwrap();
        assert_eq!(&one, b"x");
        // One octet is left; looking twice leaves it there.
        assert!(!peer_closed(&near));
        assert!(!peer_closed(&near));
        drop(far);
        // The close is behind the unread octet.
        near.read_exact(&mut one).unwrap();
        assert_eq!(&one, b"y");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !peer_closed(&near) {
            assert!(Instant::now() < deadline, "the FIN did not arrive");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// §5.1: the pool lock is not held across blocking I/O. Peer A takes
    /// the connection and never reads; a thread sends to it until its
    /// send blocks on the full buffers. A send to peer B completes
    /// meanwhile.
    #[test]
    fn tcp_5_1_a_stalled_peer_does_not_hold_sends_to_another_peer() {
        let (stalled, stalled_port) = raw_listener();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let a = std::thread::spawn(move || {
            let (mut s, _) = stalled.accept().unwrap();
            server_handshake(&mut s).unwrap();
            // Holds the connection without reading from it.
            let _ = release_rx.recv();
        });
        let (live, live_port) = raw_listener();
        let b = std::thread::spawn(move || {
            let (mut s, _) = live.accept().unwrap();
            server_handshake(&mut s).unwrap();
            read_frame(&mut s).unwrap()
        });

        let client = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let written = Arc::new(AtomicU64::new(0));
        let flood = {
            let client = Arc::clone(&client);
            let written = Arc::clone(&written);
            std::thread::spawn(move || {
                let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(stalled_port));
                let chunk = alloc::vec![0u8; 8 * 1024];
                while Transport::send(&*client, &dest, &chunk).is_ok() {
                    written.fetch_add(1, Ordering::Relaxed);
                }
            })
        };
        // Blocked: no further frame goes out for 300 ms.
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            let before = written.load(Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(300));
            if before > 0 && written.load(Ordering::Relaxed) == before {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the send to peer A never blocked"
            );
        }
        assert!(
            !flood.is_finished(),
            "the send to peer A failed instead of blocking"
        );

        let dest_b = Locator::tcp_v4([127, 0, 0, 1], u32::from(live_port));
        let start = Instant::now();
        Transport::send(&*client, &dest_b, b"to-b").expect("send to peer B");
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(1),
            "the send to peer B waited {elapsed:?} behind peer A"
        );
        assert_eq!(b.join().unwrap(), b"to-b");
        assert!(
            !flood.is_finished(),
            "peer A was not stalled while B was served"
        );

        // Peer A drops its connection; the blocked send fails and the
        // flooding thread ends.
        release_tx.send(()).unwrap();
        a.join().unwrap();
        flood.join().unwrap();
    }

    /// Non-throttled path: a PeerConn with backoff_ms=0 set holds no
    /// timer → `ensure_connected` tries to connect directly. We expect
    /// an `io::Error` (target on port 1).
    #[test]
    fn ensure_connected_no_backoff_does_not_throttle() {
        let mut p = PeerConn::new(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1));
        assert_eq!(p.backoff_ms, 0);
        // Direct connect attempt on port 1 → Err, but NOT WouldBlock.
        let err = p.ensure_connected(&PoolCounters::default()).unwrap_err();
        assert_ne!(err.kind(), std::io::ErrorKind::WouldBlock);
        // After a failed connect, backoff rises to INITIAL_BACKOFF_MS.
        assert_eq!(p.backoff_ms, INITIAL_BACKOFF_MS);
    }

    /// `Transport::send` with a payload above `MAX_FRAME_SIZE` returns
    /// `PayloadTooLarge` with both sizes. The destination has no listener:
    /// a connect attempt would have returned `Io`.
    #[test]
    fn tcp_2_3_send_oversized_payload_returns_payload_too_large() {
        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], 1);
        let oversized = alloc::vec![0u8; (crate::framing::MAX_FRAME_SIZE as usize) + 1];
        match Transport::send(&client, &dest, &oversized) {
            Err(SendError::PayloadTooLarge { size, limit }) => {
                assert_eq!(size, oversized.len());
                assert_eq!(limit, crate::framing::MAX_FRAME_SIZE as usize);
            }
            other => panic!("expected PayloadTooLarge, got {other:?}"),
        }
    }

    /// The refusal comes before the connection: a listener at the
    /// destination sees no connect, and the pool gets no entry.
    #[test]
    fn tcp_2_3_send_oversized_payload_does_not_connect() {
        let (listener, port) = raw_listener();
        listener.set_nonblocking(true).unwrap();
        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        let oversized = alloc::vec![0u8; (crate::framing::MAX_FRAME_SIZE as usize) + 1];
        let result = Transport::send(&client, &dest, &oversized);
        assert!(
            matches!(result, Err(SendError::PayloadTooLarge { .. })),
            "{result:?}"
        );
        assert_eq!(client.peers.lock().unwrap().len(), 0);
        match listener.accept() {
            Err(e) => assert_eq!(e.kind(), std::io::ErrorKind::WouldBlock),
            Ok(_) => panic!("the send connected"),
        }
    }

    /// DDSI-RTPS 2.5 §8.4.14.1: the transport states its largest message,
    /// the frame limit of the framing.
    #[test]
    fn max_message_size_is_the_frame_limit() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert_eq!(
            Transport::max_message_size(&t),
            crate::framing::MAX_FRAME_SIZE as usize
        );
    }

    // ---- §4.1: the dialect `cyclone`, and connections used both ways ----

    use crate::cyclone_stream::tests::{
        CYCLONE_0_10_5_SPDP_NO_PORT, CYCLONE_0_10_5_SPDP_PORT_17500, plain_message, unhex,
    };

    /// `message` as the dialect `cyclone` puts it on a stream.
    fn cyclone_wire(message: &[u8]) -> Vec<u8> {
        let mut wire = Vec::new();
        cyclone_stream::write_message(&mut wire, message).unwrap();
        wire
    }

    fn cyclone_transport() -> TcpTransport {
        TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_dialect(TcpDialect::Cyclone)
    }

    /// §4.1 over real sockets. On the wire there is no handshake and no
    /// length in front: a raw listener reads the RTPS header, the length
    /// submessage `81 01 04 00` with the length of the whole stream
    /// message, then the rest, message after message, and nothing is
    /// expected back. A listener with the default settings reads such a
    /// connection and hands on the messages without the length
    /// submessage, as a UDP datagram carries them.
    #[test]
    fn tcp_4_1_two_transports_exchange_messages_in_the_cyclone_dialect() {
        let small = plain_message(&INFO_TS);
        let large = plain_message(&[0x5A; 3000]);

        let (listener, port) = raw_listener();
        let expected = small.len() + large.len() + 16;
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut wire = vec![0u8; expected];
            s.read_exact(&mut wire).unwrap();
            wire
        });
        let client = cyclone_transport();
        assert_eq!(client.dialect(), TcpDialect::Cyclone);
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        Transport::send(&client, &dest, &small).unwrap();
        Transport::send(&client, &dest, &large).unwrap();
        let wire = h.join().unwrap();
        assert_eq!(&wire[..20], &small[..20]);
        assert_eq!(&wire[20..28], &[0x81, 0x01, 0x04, 0x00, 40, 0, 0, 0]);
        assert_eq!(&wire[28..40], &INFO_TS);
        assert_eq!(&wire[40..60], &large[..20]);
        assert_eq!(&wire[60..64], &[0x81, 0x01, 0x04, 0x00]);
        assert_eq!(wire[64..68], 3028u32.to_le_bytes());
        assert_eq!(&wire[68..], &large[20..]);

        // Between two transports; the listener has the default settings.
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let dest = server.local_locator();
        Transport::send(&client, &dest, &large).unwrap();
        Transport::send(&client, &dest, &small).unwrap();
        let first = datagram_within(&server, Duration::from_secs(5)).unwrap();
        let second = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&first.data[..], &large[..]);
        assert_eq!(&second.data[..], &small[..]);
        // No request, so no claim: the source is the address the client
        // connected from.
        assert_ne!(first.source.port, client.local_locator().port);
        assert_eq!(first.source.ipv4(), [127, 0, 0, 1]);
        assert!(server.claimed_logical_ports().is_empty());
        assert_eq!(server.connection_errors(), 0);
        client.close();
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §4.1 against the software it is for: the octets a Cyclone DDS
    /// 0.10.5 participant wrote on its TCP connection (captured on the
    /// Linux test host) reach the queue of a listener with the default
    /// settings as the two SPDP messages, each without the eight octets
    /// of the length submessage.
    #[test]
    fn tcp_4_1_a_listener_reads_what_cyclone_dds_wrote() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();
        let first = unhex(CYCLONE_0_10_5_SPDP_NO_PORT);
        let second = unhex(CYCLONE_0_10_5_SPDP_PORT_17500);
        let mut wire = first.clone();
        wire.extend_from_slice(&second);
        let h = raw_peer_that_writes(port, wire);
        server.accept_one().unwrap();
        for capture in [&first, &second] {
            let dg = server.try_recv().unwrap();
            assert_eq!(dg.data.len(), capture.len() - 8);
            assert_eq!(&dg.data[..20], &capture[..20]);
            assert_eq!(&dg.data[20..], &capture[28..]);
            assert_eq!(dg.source.ipv4(), [127, 0, 0, 1]);
        }
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        h.join().unwrap();
    }

    /// A raw peer that connects to `port`, writes `hello`, then reads
    /// `expect` octets of what the listener writes on the same connection
    /// and returns them with the port it connected from.
    fn raw_peer_that_reads_an_answer(
        port: u16,
        hello: Vec<u8>,
        expect: usize,
    ) -> std::thread::JoinHandle<(u16, Vec<u8>)> {
        std::thread::spawn(move || {
            let mut s = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            s.write_all(&hello).unwrap();
            let mut answer = vec![0u8; expect];
            s.read_exact(&mut answer).unwrap();
            (s.local_addr().unwrap().port(), answer)
        })
    }

    /// §4.1: a peer that connects without a listener of its own (Cyclone
    /// DDS without `TCP/Port`) expects the answers on the connection it
    /// opened. A send to the source of a message that came over an
    /// accepted connection is written on that connection, in its dialect,
    /// and makes no connection: the address is one nothing listens at.
    /// When the connection has ended, a send there fails.
    #[test]
    fn tcp_4_1_a_send_to_the_source_of_an_accepted_connection_uses_that_connection() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();
        let hello = plain_message(&INFO_TS);
        let answer = plain_message(&[0x07, 0x01, 0x04, 0x00, 9, 8, 7, 6]);

        // A stream of the dialect `cyclone`.
        let peer = raw_peer_that_reads_an_answer(port, cyclone_wire(&hello), answer.len() + 8);
        let dg = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&dg.data[..], &hello[..]);
        assert_eq!(server.dialect(), TcpDialect::ZeroDds);
        assert_eq!(server.dialect_towards(&dg.source), TcpDialect::Cyclone);
        assert_eq!(
            server.dialect_towards(&Locator::tcp_v4([127, 0, 0, 1], u32::from(port))),
            TcpDialect::ZeroDds,
            "a destination without an accepted connection"
        );
        Transport::send(&*server, &dg.source, &answer).unwrap();
        let (from, read) = peer.join().unwrap();
        assert_eq!(dg.source.port, u32::from(from));
        assert_eq!(read, cyclone_wire(&answer));
        assert_eq!(server.answered_on_accepted(), 1);
        assert!(
            server.peers.lock().unwrap().is_empty(),
            "the send connected"
        );
        // The peer has closed: the entry goes with the connection, and the
        // address has no listener.
        let gone = std::time::Instant::now() + Duration::from_secs(5);
        while !server.answering.lock().unwrap().is_empty() {
            assert!(std::time::Instant::now() < gone, "the entry stayed");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            server.dialect_towards(&dg.source),
            TcpDialect::ZeroDds,
            "the connection has ended"
        );
        assert!(matches!(
            Transport::send(&*server, &dg.source, &answer),
            Err(SendError::Io { .. })
        ));
        assert_eq!(server.answered_on_accepted(), 1);

        // A stream of the dialect `rtps`: the answer has to state its own
        // length (§2.5), and one that does not is refused and leaves the
        // connection as it is.
        let hello_rtps = stream_message([0x01, 0xF0], true, &INFO_TS);
        let answer_rtps = stream_message([0x01, 0xF0], false, &[1, 2, 3, 4]);
        let peer = raw_peer_that_reads_an_answer(port, hello_rtps.clone(), answer_rtps.len());
        let dg = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&dg.data[..], &hello_rtps[..]);
        assert_eq!(server.dialect_towards(&dg.source), TcpDialect::Rtps);
        assert_eq!(
            Transport::send(&*server, &dg.source, &answer),
            Err(SendError::Io {
                message: "tcp message without a HeaderExtension messageLength"
            })
        );
        Transport::send(&*server, &dg.source, &answer_rtps).unwrap();
        assert_eq!(peer.join().unwrap().1, answer_rtps);

        // The dialect `zerodds` without a claim (§3.6): the answer is a
        // frame of §2.1 behind the response.
        let mut hello_zerodds = request_octets(1).to_vec();
        hello_zerodds.extend_from_slice(&[0, 0, 0, 3]);
        hello_zerodds.extend_from_slice(b"abc");
        let peer = raw_peer_that_reads_an_answer(port, hello_zerodds, 16 + 4 + 2);
        let dg = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&dg.data[..], b"abc");
        assert_eq!(server.dialect_towards(&dg.source), TcpDialect::ZeroDds);
        Transport::send(&*server, &dg.source, b"de").unwrap();
        let (_, read) = peer.join().unwrap();
        assert_eq!(&read[..4], b"ZDA+");
        assert_eq!(&read[16..], &[0, 0, 0, 2, b'd', b'e']);
        assert_eq!(server.answered_on_accepted(), 3);
        // The one pool entry is the connect that failed at the address of
        // the connection that had ended.
        assert_eq!(server.peers.lock().unwrap().len(), 1);

        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §4.1: with a claim (§3.6) the source of a connection's frames is
    /// the peer's listener, and a send to it connects there as before; the
    /// accepted connection takes no answers.
    #[test]
    fn tcp_4_1_a_claimed_connection_is_answered_at_the_claimed_listener() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let client = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let server_done = serve_in_background(&server);
        let client_done = serve_in_background(&client);
        Transport::send(&*client, &server.local_locator(), b"ping").unwrap();
        let dg = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(dg.source, client.local_locator());
        assert!(server.answering.lock().unwrap().is_empty());
        Transport::send(&*server, &dg.source, b"pong").unwrap();
        let back = datagram_within(&client, Duration::from_secs(5)).unwrap();
        assert_eq!(&back.data[..], b"pong");
        assert_eq!(back.source, server.local_locator());
        assert_eq!(server.answered_on_accepted(), 0);
        assert_eq!(server.peers.lock().unwrap().len(), 1);
        client.close();
        server.close();
        for done in [server_done, client_done] {
            done.recv_timeout(Duration::from_secs(5))
                .expect("serve returns after close");
        }
    }

    /// §4.1: what the peer writes on a connection this transport opened
    /// is read. A raw listener answers on the accepted connection in the
    /// dialect `cyclone`; the message reaches the queue of the connecting
    /// transport without the length submessage, with the listener's
    /// address as its source. The same between two transports in each
    /// dialect, with one connection between them: the connecting side
    /// claims no port, and the answer comes back on its connection.
    #[test]
    fn tcp_4_1_what_the_peer_writes_on_an_outgoing_connection_is_read() {
        let hello = plain_message(&INFO_TS);
        let answer = plain_message(&[0x07, 0x01, 0x04, 0x00, 9, 8, 7, 6]);

        let (listener, port) = raw_listener();
        let (answer_wire, hello_len) = (cyclone_wire(&answer), hello.len() + 8);
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut wire = vec![0u8; hello_len];
            s.read_exact(&mut wire).unwrap();
            s.write_all(&answer_wire).unwrap();
            // Held open until the client has read the answer and closed.
            let mut sink = [0u8; 16];
            while matches!(s.read(&mut sink), Ok(n) if n > 0) {}
        });
        let client = cyclone_transport();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        Transport::send(&client, &dest, &hello).unwrap();
        let dg = datagram_within(&client, Duration::from_secs(5)).unwrap();
        assert_eq!(&dg.data[..], &answer[..]);
        assert_eq!(dg.source, dest);
        client.close();
        h.join().unwrap();

        let rtps_hello = stream_message([0x01, 0xF0], true, &INFO_TS);
        let rtps_answer = stream_message([0x01, 0xF0], false, &[1, 2, 3, 4]);
        for (dialect, hello, answer) in [
            (TcpDialect::ZeroDds, &b"ping"[..], &b"pong"[..]),
            (TcpDialect::Rtps, &rtps_hello[..], &rtps_answer[..]),
            (TcpDialect::Cyclone, &hello[..], &answer[..]),
        ] {
            let server = Arc::new(
                TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                    .unwrap()
                    .with_dialect(dialect),
            );
            let done = serve_in_background(&server);
            let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_dialect(dialect)
                .with_logical_port_claim(false);
            Transport::send(&client, &server.local_locator(), hello).unwrap();
            let dg = datagram_within(&server, Duration::from_secs(5)).unwrap();
            assert_eq!(&dg.data[..], hello, "{dialect:?}");
            assert_ne!(dg.source.port, client.local_locator().port);
            Transport::send(&*server, &dg.source, answer).unwrap();
            let back = datagram_within(&client, Duration::from_secs(5)).unwrap();
            assert_eq!(&back.data[..], answer, "{dialect:?}");
            assert_eq!(back.source, server.local_locator(), "{dialect:?}");
            // One connection carried both.
            assert_eq!(server.accepted.lock().unwrap().len(), 1);
            assert!(server.peers.lock().unwrap().is_empty(), "{dialect:?}");
            assert_eq!(server.answered_on_accepted(), 1);
            assert_eq!(client.connection_errors(), 0);
            client.close();
            server.close();
            done.recv_timeout(Duration::from_secs(5))
                .expect("serve returns after close");
        }
    }

    /// The thread that reads an outgoing connection holds a clone of its
    /// stream. When the transport is dropped the connection is shut down:
    /// the peer reads its end, and the thread ends with it.
    #[test]
    fn tcp_4_1_an_outgoing_connection_ends_with_its_transport() {
        let (listener, port) = raw_listener();
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut wire = Vec::new();
            s.read_to_end(&mut wire).map(|_| wire).map_err(|e| e.kind())
        });
        let client = cyclone_transport();
        let message = plain_message(&INFO_TS);
        Transport::send(
            &client,
            &Locator::tcp_v4([127, 0, 0, 1], u32::from(port)),
            &message,
        )
        .unwrap();
        drop(client);
        assert_eq!(h.join().unwrap(), Ok(cyclone_wire(&message)));
    }

    /// §4.1: Cyclone DDS drops a connection on a stream message above
    /// 65536 octets. A message that would be longer with its length
    /// submessage is refused with the limit of the dialect, and what has
    /// no RTPS header to put the submessage behind with an error, both
    /// before a connection is made. The transport states the limit.
    #[test]
    fn tcp_4_1_send_refuses_what_cyclone_dds_does_not_read() {
        let (listener, port) = raw_listener();
        listener.set_nonblocking(true).unwrap();
        let client = cyclone_transport();
        assert_eq!(Transport::max_message_size(&client), 65_528);
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        let too_large = plain_message(&vec![0u8; 65_528 - 19]);
        assert_eq!(
            Transport::send(&client, &dest, &too_large),
            Err(SendError::PayloadTooLarge {
                size: 65_529,
                limit: 65_528
            })
        );
        assert_eq!(
            Transport::send(
                &client,
                &dest,
                b"no RTPS message, and long enough for a header"
            ),
            Err(SendError::Io {
                message: "tcp message does not begin with an RTPS header"
            })
        );
        assert!(client.peers.lock().unwrap().is_empty());
        assert!(
            matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
            "a refused message made a connection"
        );
        // The largest message goes out.
        let largest = plain_message(&vec![0u8; 65_528 - 20]);
        Transport::send(&client, &dest, &largest).unwrap();
        let (mut s, _) = listener.accept().unwrap();
        s.set_nonblocking(false).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut wire = vec![0u8; 65_536];
        s.read_exact(&mut wire).unwrap();
        assert_eq!(wire[24..28], 65_536u32.to_le_bytes());
    }

    // ---- §4.2: the dialect of an accepted connection ----

    /// §4.2: one listener with the default settings reads a peer of each
    /// dialect at the same time; no dialect is configured for the port.
    #[test]
    fn tcp_4_2_one_listener_reads_a_peer_of_each_dialect() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        assert_eq!(server.accepted_dialects(), &TcpDialect::ALL);
        let done = serve_in_background(&server);
        let dest = server.local_locator();

        let zerodds = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let rtps = rtps_transport();
        let cyclone = cyclone_transport();
        let rtps_message = stream_message([0x01, 0xF0], true, &INFO_TS);
        let cyclone_message = plain_message(&INFO_TS);
        Transport::send(&zerodds, &dest, b"a frame of 2.1").unwrap();
        Transport::send(&rtps, &dest, &rtps_message).unwrap();
        Transport::send(&cyclone, &dest, &cyclone_message).unwrap();

        let mut got: Vec<Vec<u8>> = (0..3)
            .map(|_| recv_within(&server, Duration::from_secs(5)).unwrap())
            .collect();
        got.sort();
        let mut expected = vec![b"a frame of 2.1".to_vec(), rtps_message, cyclone_message];
        expected.sort();
        assert_eq!(got, expected);
        assert_eq!(server.accepted.lock().unwrap().len(), 3);
        assert_eq!(server.connection_errors(), 0);
        for client in [&zerodds, &rtps, &cyclone] {
            client.close();
        }
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §4.2: first octets that begin no dialect end the connection, with
    /// the error of the dialect the listener is there for; so does a
    /// stream of RTPS messages whose first submessage is neither the
    /// length submessage of Cyclone DDS nor a HeaderExtension. Nothing of
    /// such a connection reaches the queue.
    #[test]
    fn tcp_4_2_unknown_first_octets_are_refused() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();
        let h = raw_peer_that_writes(port, b"GET / HTTP/1.1\r\n\r\n".to_vec());
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::Handshake(HandshakeError::BadMagic { got }) if &got == b"GET "
            ),
            "{err:?}"
        );
        h.join().unwrap();

        // An RTPS message as UDP carries it: INFO_TS first.
        let h = raw_peer_that_writes(port, plain_message(&INFO_TS));
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::StreamFraming(FramingError::NoMessageLength)
            ),
            "{err:?}"
        );
        h.join().unwrap();

        let streams_only = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_accepted_dialects(&[TcpDialect::Rtps, TcpDialect::Cyclone]);
        let port = u16::try_from(streams_only.local_locator().port).unwrap();
        let h = raw_peer_that_writes(port, b"GET / HTTP/1.1\r\n\r\n".to_vec());
        let err = streams_only.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::StreamFraming(FramingError::NotRtps { got }) if &got == b"GET "
            ),
            "{err:?}"
        );
        h.join().unwrap();
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        assert!(matches!(streams_only.try_recv(), Err(RecvError::Timeout)));
    }

    /// §4.2 with §3.8: the octets that tell the dialect are read under the
    /// deadline of the handshake. A peer that sends `R` and nothing more
    /// is given up on after one slice.
    #[test]
    fn tcp_4_2_detection_is_under_the_handshake_budget() {
        for dialects in [&TcpDialect::ALL[..], &[TcpDialect::Cyclone][..]] {
            let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_accepted_dialects(dialects);
            let port = u16::try_from(server.local_locator().port).unwrap();
            let (release, held) = std::sync::mpsc::channel::<()>();
            let h = std::thread::spawn(move || {
                let mut s =
                    TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
                s.write_all(b"R").unwrap();
                let _ = held.recv_timeout(Duration::from_secs(10));
            });
            let started = std::time::Instant::now();
            let err = server.accept_one().unwrap_err();
            let waited = started.elapsed();
            assert!(waited < Duration::from_secs(1), "{waited:?}");
            match err {
                TcpTransportError::Handshake(HandshakeError::Io(e))
                | TcpTransportError::PeerIo(e) => {
                    assert_eq!(e.kind(), std::io::ErrorKind::TimedOut);
                }
                other => panic!("{other:?}"),
            }
            drop(release);
            h.join().unwrap();
        }
    }

    /// §4.2: a deployment restricts the listener. One that takes the
    /// dialect `cyclone` only refuses the request of §3 and a stream of
    /// the dialect `rtps`, one for `rtps` a stream of Cyclone DDS, one for
    /// `zerodds` every RTPS stream, and one with an empty list
    /// everything.
    #[test]
    fn tcp_4_2_a_restricted_listener_refuses_the_other_dialects() {
        let listener_for = |dialects: &[TcpDialect]| {
            let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_accepted_dialects(dialects);
            let port = u16::try_from(t.local_locator().port).unwrap();
            (t, port)
        };
        let mut zerodds_wire = request_octets(1).to_vec();
        zerodds_wire.extend_from_slice(&[0, 0, 0, 1, b'x']);
        let rtps_wire = stream_message([0x01, 0xF0], true, &INFO_TS);
        let cyclone_wire = unhex(CYCLONE_0_10_5_SPDP_NO_PORT);
        let refused = |t: &TcpTransport, port: u16, wire: &[u8]| {
            let h = raw_peer_that_writes(port, wire.to_vec());
            let err = t.accept_one().unwrap_err();
            h.join().unwrap();
            assert!(matches!(t.try_recv(), Err(RecvError::Timeout)));
            err
        };

        let (cyclone, port) = listener_for(&[TcpDialect::Cyclone]);
        assert_eq!(cyclone.accepted_dialects(), &[TcpDialect::Cyclone]);
        let err = refused(&cyclone, port, &zerodds_wire);
        assert!(
            matches!(
                err,
                TcpTransportError::StreamFraming(FramingError::NotRtps { got }) if &got == b"ZDDS"
            ),
            "{err:?}"
        );
        let err = refused(&cyclone, port, &rtps_wire);
        assert!(
            matches!(
                err,
                TcpTransportError::DialectNotAccepted {
                    dialect: TcpDialect::Rtps
                }
            ),
            "{err:?}"
        );
        let h = raw_peer_that_writes(port, cyclone_wire.clone());
        cyclone.accept_one().unwrap();
        h.join().unwrap();
        assert_eq!(cyclone.try_recv().unwrap().data.len(), 320);

        let (rtps, port) = listener_for(&[TcpDialect::Rtps]);
        let err = refused(&rtps, port, &cyclone_wire);
        assert!(
            matches!(
                err,
                TcpTransportError::DialectNotAccepted {
                    dialect: TcpDialect::Cyclone
                }
            ),
            "{err:?}"
        );

        let (zerodds, port) = listener_for(&[TcpDialect::ZeroDds]);
        for wire in [&rtps_wire, &cyclone_wire] {
            let err = refused(&zerodds, port, wire);
            assert!(
                matches!(
                    err,
                    TcpTransportError::Handshake(HandshakeError::BadMagic { got }) if &got == b"RTPS"
                ),
                "{err:?}"
            );
        }

        let (none, port) = listener_for(&[]);
        for wire in [&zerodds_wire, &rtps_wire, &cyclone_wire] {
            let err = refused(&none, port, wire);
            assert!(
                matches!(
                    err,
                    TcpTransportError::StreamFraming(FramingError::NotRtps { .. })
                ),
                "{err:?}"
            );
        }
    }

    /// §4.2 with §3.3: a stream has no request. The vendor list is held
    /// against the vendor id of each RTPS header, here that of Cyclone
    /// DDS (`01 10`) in what it wrote.
    #[test]
    fn tcp_4_2_an_unlisted_vendor_in_the_rtps_header_is_refused() {
        let wire = unhex(CYCLONE_0_10_5_SPDP_NO_PORT);
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_accepted_vendors(vec![[0x01, 0xF0]]);
        let port = u16::try_from(server.local_locator().port).unwrap();
        let h = raw_peer_that_writes(port, wire.clone());
        let err = server.accept_one().unwrap_err();
        assert!(
            matches!(
                err,
                TcpTransportError::VendorNotAccepted {
                    vendor: [0x01, 0x10]
                }
            ),
            "{err:?}"
        );
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        // No entry stays for answers to the refused connection.
        assert!(server.answering.lock().unwrap().is_empty());
        h.join().unwrap();

        let listed = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_accepted_vendors(vec![[0x01, 0x10]]);
        let port = u16::try_from(listed.local_locator().port).unwrap();
        let h = raw_peer_that_writes(port, wire);
        listed.accept_one().unwrap();
        assert_eq!(listed.try_recv().unwrap().data.len(), 320);
        h.join().unwrap();
    }

    /// §4.2: the first message tells the dialect of the connection, and
    /// every later message has to be of it. A message of the other stream
    /// dialect ends the connection; the messages before it stand.
    #[test]
    fn tcp_4_2_a_connection_keeps_the_dialect_of_its_first_message() {
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();
        let cyclone = cyclone_wire(&plain_message(&INFO_TS));
        let rtps = stream_message([0x01, 0xF0], true, &INFO_TS);
        for (first, second) in [(&cyclone, &rtps), (&rtps, &cyclone)] {
            let mut wire = first.clone();
            wire.extend_from_slice(second);
            let h = raw_peer_that_writes(port, wire);
            let err = server.accept_one().unwrap_err();
            assert!(
                matches!(
                    err,
                    TcpTransportError::StreamFraming(FramingError::NoMessageLength)
                ),
                "{err:?}"
            );
            assert!(server.try_recv().is_ok());
            assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
            h.join().unwrap();
        }
    }

    /// §4.2 with §3.7: a stream dialect has no response to refuse with. A
    /// stream peer beyond the limit of a listener with the default
    /// settings is closed without an answer and counted as refused and as
    /// unanswered; its message is not read.
    #[test]
    fn tcp_4_2_a_stream_peer_beyond_the_limit_is_closed_unanswered() {
        let server = Arc::new(
            TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
                .unwrap()
                .with_accept_limit(1),
        );
        let done = serve_in_background(&server);
        let port = u16::try_from(server.local_locator().port).unwrap();
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        let wire = unhex(CYCLONE_0_10_5_SPDP_NO_PORT);

        let mut first = TcpStream::connect(addr).unwrap();
        first.write_all(&wire).unwrap();
        assert_eq!(
            recv_within(&server, Duration::from_secs(5)).map(|m| m.len()),
            Some(320)
        );
        let mut second = TcpStream::connect(addr).unwrap();
        second
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        second.write_all(&wire).unwrap();
        let mut octet = [0u8; 1];
        // The end of the connection, or its reset: the listener closed
        // with the message unread.
        assert!(matches!(
            second.read(&mut octet).map_err(|e| e.kind()),
            Ok(0) | Err(std::io::ErrorKind::ConnectionReset)
        ));
        assert_eq!(server.refused_connections(), 1);
        assert_eq!(server.refused_unanswered(), 1);
        assert!(matches!(server.try_recv(), Err(RecvError::Timeout)));
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    // ---- §6.1: the dialect `fastdds` ----

    use zerodds_rtps::wire_types::LocatorKind;

    fn fastdds_transport() -> TcpTransport {
        TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .unwrap()
            .with_dialect(TcpDialect::FastDds)
    }

    /// A Fast DDS locator: kind 4, the logical port in the upper 16 bits.
    fn fastdds_locator(address: [u8; 4], physical: u16, logical: u16) -> Locator {
        let mut locator =
            Locator::tcp_v4(address, u32::from(physical)).with_tcp_kind_set(TcpKindSet::Kinds4And8);
        locator.port |= u32::from(logical) << 16;
        locator
    }

    /// §6.1: the locator of a transport of the dialect `fastdds` is of
    /// kind 4 and holds the logical port above the port of the listener;
    /// without a logical port of its own it is the listener's.
    #[test]
    fn tcp_6_1_the_locator_of_the_dialect_holds_the_logical_port() {
        let t = fastdds_transport();
        let physical = t.listener_port();
        assert_eq!(t.local_locator().kind, LocatorKind::Tcpv4Kind4);
        assert_eq!(
            t.local_locator().port,
            (u32::from(physical) << 16) | u32::from(physical)
        );
        assert_eq!(t.locator_kinds(), TcpKindSet::Kinds4And8);
        let t = t.with_logical_port(7410);
        assert_eq!(t.local_locator().port, (7410 << 16) | u32::from(physical));
        assert_eq!(Transport::local_locator(&t), t.local_locator());
        assert_eq!(t.rtcp_own().ports, vec![7410]);
        // Another dialect keeps the locator as it was.
        let plain = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert!(plain.local_locator().port <= u32::from(u16::MAX));
    }

    /// §6.1: a locator of kind 4 or 8 with a port above 65535 is one of
    /// Fast DDS; the lower half is the TCP port, the upper the logical
    /// port, and a WAN address is kept for a second attempt.
    #[test]
    fn tcp_6_1_a_locator_with_a_logical_port_is_a_fast_dds_destination() {
        let d = destination(&fastdds_locator([10, 0, 0, 5], 5100, 7410)).unwrap();
        assert_eq!(d.addr, "10.0.0.5:5100".parse::<SocketAddr>().unwrap());
        assert_eq!(d.logical_port, Some(7410));
        assert_eq!(d.wan, None);
        let mut wan = fastdds_locator([10, 0, 0, 5], 5100, 7410);
        wan.address[8..12].copy_from_slice(&[192, 0, 2, 9]);
        let d = destination(&wan).unwrap();
        assert_eq!(d.wan, Some("192.0.2.9:5100".parse().unwrap()));
        // A WAN address equal to the address is none.
        wan.address[8..12].copy_from_slice(&[10, 0, 0, 5]);
        assert_eq!(destination(&wan).unwrap().wan, None);
        // Kind 4 with a port of 16 bits: Cyclone DDS's, no logical port.
        let cyclone =
            Locator::tcp_v4([10, 0, 0, 5], 5100).with_tcp_kind_set(TcpKindSet::Kinds4And8);
        assert_eq!(destination(&cyclone).unwrap().logical_port, None);
        // ZeroDDS's own kinds hold no logical port: above 65535 is an
        // error as before.
        assert!(matches!(
            destination(&Locator::tcp_v4([10, 0, 0, 5], (7410 << 16) | 5100)),
            Err(InvalidLocator::PortOverflow { .. })
        ));
        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert_eq!(
            server.dialect_towards(&fastdds_locator([127, 0, 0, 1], 5100, 7410)),
            TcpDialect::FastDds
        );
    }

    /// §6.1: an address of this host is reached at the loopback address,
    /// as Fast DDS takes every address of its host for one; another
    /// address stays as it is.
    #[test]
    fn tcp_6_1_an_address_of_this_host_is_reached_at_loopback() {
        let t = fastdds_transport();
        let loopback: SocketAddr = "127.0.0.1:5100".parse().unwrap();
        assert_eq!(t.host_alias(loopback), loopback);
        let elsewhere: SocketAddr = "192.0.2.1:5100".parse().unwrap();
        assert_eq!(t.host_alias(elsewhere), elsewhere);
        // The address a packet to another network leaves from, where the
        // host has one.
        let probe = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
            .and_then(|s| s.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).map(|()| s))
            .and_then(|s| s.local_addr());
        if let Ok(own) = probe
            && !own.ip().is_unspecified()
            && !own.ip().is_loopback()
        {
            assert_eq!(t.host_alias(SocketAddr::new(own.ip(), 5100)), loopback);
        }
    }

    /// §6.1: the answers `host_alias` keeps are bounded. The addresses
    /// come from the locators of peers; 762 addresses of the documentation
    /// ranges (RFC 5737), none of this host, leave `MAX_HOST_ADDRESS_ANSWERS`
    /// answers, the last ones asked. An address of this host whose answer
    /// was forgotten is asked again and still reached at loopback.
    #[test]
    fn tcp_6_1_the_answers_on_host_addresses_are_bounded() {
        let t = fastdds_transport();
        let loopback: SocketAddr = "127.0.0.1:5100".parse().unwrap();
        let probe = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
            .and_then(|s| s.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).map(|()| s))
            .and_then(|s| s.local_addr())
            .ok()
            .filter(|own| !own.ip().is_unspecified() && !own.ip().is_loopback());
        if let Some(own) = probe {
            assert_eq!(t.host_alias(SocketAddr::new(own.ip(), 5100)), loopback);
        }
        let remote: Vec<IpAddr> = [[192, 0, 2], [198, 51, 100], [203, 0, 113]]
            .into_iter()
            .flat_map(|[a, b, c]| (1..=254u8).map(move |h| IpAddr::V4(Ipv4Addr::new(a, b, c, h))))
            .collect();
        assert!(remote.len() > MAX_HOST_ADDRESS_ANSWERS);
        for ip in &remote {
            let addr = SocketAddr::new(*ip, 5100);
            assert_eq!(t.host_alias(addr), addr);
        }
        {
            let known = t.host_addresses.lock().unwrap();
            assert_eq!(known.len(), MAX_HOST_ADDRESS_ANSWERS);
            let kept = &remote[remote.len() - MAX_HOST_ADDRESS_ANSWERS..];
            assert!(kept.iter().all(|ip| known.get(ip) == Some(&false)));
            assert!(!known.contains_key(&remote[0]));
            if let Some(own) = probe
                && own.ip() < kept[0]
            {
                assert!(!known.contains_key(&own.ip()), "the lowest is forgotten");
            }
        }
        if let Some(own) = probe {
            assert_eq!(t.host_alias(SocketAddr::new(own.ip(), 5100)), loopback);
            assert_eq!(
                t.host_addresses.lock().unwrap().len(),
                MAX_HOST_ADDRESS_ANSWERS
            );
        }
    }

    /// §6.1 between two transports: the connecting one binds the
    /// connection and opens the logical port of the other, whose messages
    /// carry the connecting side's listener and the logical port as their
    /// source. The answer goes on the same connection: the accepting side
    /// asks the connecting one for its logical port and connects nowhere.
    #[test]
    fn tcp_6_1_two_transports_exchange_messages_in_the_fastdds_dialect() {
        let server = Arc::new(fastdds_transport().with_logical_port(7410));
        let done = serve_in_background(&server);
        let client = fastdds_transport().with_logical_port(7412);
        let small = plain_message(&INFO_TS);
        let large = plain_message(&[0x5A; 3000]);
        Transport::send(&client, &server.local_locator(), &small).unwrap();
        Transport::send(&client, &server.local_locator(), &large).unwrap();
        let first = datagram_within(&server, Duration::from_secs(5)).unwrap();
        let second = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&first.data[..], &small[..]);
        assert_eq!(&second.data[..], &large[..]);
        assert_eq!(
            first.source.port,
            (7410 << 16) | u32::from(client.listener_port()),
            "the client's listener and the logical port of the message"
        );
        assert_eq!(first.source.kind, LocatorKind::Tcpv4Kind4);
        // The answer to the client's locator goes on the connection.
        Transport::send(&*server, &client.local_locator(), b"answer").unwrap();
        let answer = datagram_within(&client, Duration::from_secs(5)).unwrap();
        assert_eq!(&answer.data[..], b"answer");
        assert_eq!(answer.source.port >> 16, 7412);
        assert_eq!(server.answered_on_accepted(), 1);
        assert!(
            server.peers.lock().unwrap().is_empty(),
            "the answer connected"
        );
        // A logical port the server has not open is refused, and the
        // refusal is counted.
        let other = fastdds_locator([127, 0, 0, 1], server.listener_port(), 7999);
        assert_eq!(
            Transport::send(&client, &other, &small),
            Err(SendError::Io {
                message: "tcp fastdds peer refused the logical port"
            })
        );
        assert_eq!(client.rtcp_refused_ports(), 1);
        assert_eq!(server.connection_errors(), 0);
        assert_eq!(client.connection_errors(), 0);
        client.close();
        // The client's close is an UnbindConnectionRequest: the regular
        // end of the server's connection, no error.
        let gone = std::time::Instant::now() + Duration::from_secs(5);
        while !server.answering.lock().unwrap().is_empty() {
            assert!(std::time::Instant::now() < gone, "the connection stayed");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(server.connection_errors(), 0);
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §6.1: a transport of another dialect reaches a Fast DDS locator in
    /// the dialect `fastdds`; a transport of the dialect `fastdds` refuses
    /// a locator without a logical port, and a message above what Fast DDS
    /// reads, without a connect.
    #[test]
    fn tcp_6_1_the_dialect_is_chosen_by_the_locator() {
        let server = Arc::new(fastdds_transport());
        let done = serve_in_background(&server);
        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert_eq!(client.dialect(), TcpDialect::ZeroDds);
        Transport::send(&client, &server.local_locator(), b"one").unwrap();
        let dg = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&dg.data[..], b"one");

        let fastdds = fastdds_transport();
        assert_eq!(
            Transport::send(
                &fastdds,
                &Locator::tcp_v4([127, 0, 0, 1], u32::from(server.listener_port())),
                b"x"
            ),
            Err(SendError::Io {
                message: "tcp locator without the logical port the dialect fastdds addresses"
            })
        );
        let too_long = vec![0u8; rtcp::MAX_MESSAGE_SIZE + 1];
        assert_eq!(
            Transport::send(&fastdds, &server.local_locator(), &too_long),
            Err(SendError::PayloadTooLarge {
                size: rtcp::MAX_MESSAGE_SIZE + 1,
                limit: rtcp::MAX_MESSAGE_SIZE
            })
        );
        assert!(fastdds.peers.lock().unwrap().is_empty());
        client.close();
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §6.1: where the address of a Fast DDS locator takes no connection,
    /// its WAN address is tried, and the message arrives there.
    #[test]
    fn tcp_6_1_a_locator_whose_address_refuses_is_reached_at_its_wan_address() {
        let server = Arc::new(fastdds_transport());
        let done = serve_in_background(&server);
        let port = server.listener_port();
        // The listener is bound to 127.0.0.1 alone: 127.0.0.2 refuses.
        let refusing = SocketAddr::from((Ipv4Addr::new(127, 0, 0, 2), port));
        assert!(TcpStream::connect_timeout(&refusing, Duration::from_secs(2)).is_err());
        let mut lan_only = server.local_locator();
        lan_only.address[12..16].copy_from_slice(&[127, 0, 0, 2]);
        let mut with_wan = lan_only;
        with_wan.address[8..12].copy_from_slice(&[127, 0, 0, 1]);
        assert_eq!(destination(&with_wan).unwrap().addr, refusing);
        let message = plain_message(&INFO_TS);
        // Without the WAN address the send fails.
        let lan_client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert!(Transport::send(&lan_client, &lan_only, &message).is_err());
        // With it the connection is made at the WAN address.
        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        Transport::send(&client, &with_wan, &message).unwrap();
        let got = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&got.data[..], &message[..]);
        assert_eq!(got.source.port >> 16, u32::from(server.logical_port()));
        lan_client.close();
        client.close();
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §6.1: a Fast DDS destination at the address of a pool entry of
    /// another dialect gets an entry of its own dialect, also while
    /// another send holds the entry that is there. The message goes out
    /// in the dialect `fastdds`, for its logical port.
    #[test]
    fn tcp_6_1_an_entry_of_another_dialect_in_use_is_not_taken_for_a_fast_dds_destination() {
        let server = Arc::new(fastdds_transport());
        let done = serve_in_background(&server);
        let client = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let plain = Locator::tcp_v4([127, 0, 0, 1], u32::from(server.listener_port()));
        let message = plain_message(&INFO_TS);
        Transport::send(&*client, &plain, &message).unwrap();
        let first = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(first.source.port >> 16, 0, "the dialect zerodds: {first:?}");
        let entry = Arc::clone(client.peers.lock().unwrap().values().next().unwrap());
        let held = entry.lock().unwrap();
        assert_eq!(held.dialect, TcpDialect::ZeroDds);
        let sending = {
            let client = Arc::clone(&client);
            let to = server.local_locator();
            let message = message.clone();
            std::thread::spawn(move || Transport::send(&*client, &to, &message))
        };
        // The send finds the entry while it is held.
        std::thread::sleep(Duration::from_millis(200));
        drop(held);
        assert_eq!(sending.join().unwrap(), Ok(()));
        let second = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&second.data[..], &message[..]);
        assert_eq!(
            second.source.port,
            (u32::from(server.logical_port()) << 16) | u32::from(client.listener_port()),
            "the dialect fastdds: {second:?}"
        );
        {
            let pool = client.peers.lock().unwrap();
            assert_eq!(pool.len(), 1);
            let now = pool.values().next().unwrap();
            assert!(!Arc::ptr_eq(now, &entry));
            assert_eq!(now.lock().unwrap().dialect, TcpDialect::FastDds);
        }
        client.close();
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §6.1: a connection of another dialect refuses a message for a
    /// logical port, without a connect, instead of writing it without
    /// the port.
    #[test]
    fn tcp_6_1_a_logical_port_on_a_connection_of_another_dialect_is_refused() {
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        for dialect in [TcpDialect::ZeroDds, TcpDialect::Rtps, TcpDialect::Cyclone] {
            let mut conn = PeerConn::new(listener.local_addr().unwrap()).speaking(dialect);
            let counters = PoolCounters::default();
            assert!(
                matches!(
                    conn.send(&plain_message(&INFO_TS), Some(7410), &counters),
                    Err(FramingError::Io(_))
                ),
                "{dialect:?}"
            );
            assert!(conn.writer.is_none(), "{dialect:?}: connected");
        }
        assert!(listener.accept().is_err(), "a connection was made");
    }

    /// §6.1 against the software it is for: the BindConnectionRequest a
    /// Fast DDS 3.6.1 participant wrote (captured on the Linux test host)
    /// is answered by a listener with the default settings with `Ok` and
    /// its locator; the OpenLogicalPortRequest for its logical port with
    /// `Ok`; the RTPS message behind it reaches the queue with the Fast
    /// DDS listener's port, as the request names it, in its source.
    #[test]
    fn tcp_6_1_a_listener_answers_what_fast_dds_wrote() {
        let server = Arc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let done = serve_in_background(&server);
        let port = server.listener_port();
        let bind = unhex(include_str!(
            "../tests/data/fastdds_3_6_1_rtcp_bind_request.hex"
        ));
        let mut hello = bind.clone();
        hello.extend(
            rtcp::ControlMessage {
                kind: rtcp::kind::OPEN_LOGICAL_PORT_REQUEST,
                transaction_id: [1; 12],
                code: None,
                payload: Some(rtcp::encode_port(port)),
            }
            .encode()
            .unwrap(),
        );
        let message = plain_message(&INFO_TS);
        hello.extend(rtcp::data_message(port, &message).unwrap());
        // The response to the bind is 68 octets, the one to the port 34.
        let peer = raw_peer_that_reads_an_answer(port, hello, 68 + 34);
        let dg = datagram_within(&server, Duration::from_secs(5)).unwrap();
        assert_eq!(&dg.data[..], &message[..]);
        // The captured request names the physical port 54895.
        assert_eq!(dg.source.port, (u32::from(port) << 16) | 54895);
        let (_, answer) = peer.join().unwrap();
        let mut cursor = std::io::Cursor::new(&answer);
        let bound = rtcp::read_message(&mut cursor).unwrap();
        let bound = rtcp::ControlMessage::decode(&bound.body).unwrap();
        assert_eq!(bound.kind, rtcp::kind::BIND_CONNECTION_RESPONSE);
        assert_eq!(bound.code, Some(rtcp::ResponseCode::Ok));
        let locator = rtcp::decode_locator(bound.payload.as_deref().unwrap()).unwrap();
        assert_eq!(locator.physical_port(), port);
        assert_eq!(&locator.address[12..], &[127, 0, 0, 1]);
        let opened = rtcp::read_message(&mut cursor).unwrap();
        let opened = rtcp::ControlMessage::decode(&opened.body).unwrap();
        assert_eq!(opened.kind, rtcp::kind::OPEN_LOGICAL_PORT_RESPONSE);
        assert_eq!(opened.code, Some(rtcp::ResponseCode::Ok));
        assert_eq!(opened.transaction_id, [1; 12]);
        server.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }

    /// §6.1 with §3.3 and §4.2: a listener that takes no connection of the
    /// dialect `fastdds` closes one; one that takes no vendor of the
    /// request answers the bind with `ServerError`, which the connecting
    /// transport names in its `SendError`.
    #[test]
    fn tcp_6_1_a_listener_refuses_a_dialect_or_vendor_it_does_not_take() {
        let restricted =
            Arc::new(fastdds_transport().with_accepted_dialects(&[TcpDialect::ZeroDds]));
        let done = serve_in_background(&restricted);
        let client = fastdds_transport();
        assert!(Transport::send(&client, &restricted.local_locator(), b"x").is_err());
        let gone = std::time::Instant::now() + Duration::from_secs(5);
        while restricted.connection_errors() == 0 {
            assert!(std::time::Instant::now() < gone, "the connection was taken");
            std::thread::sleep(Duration::from_millis(5));
        }
        restricted.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");

        let picky = Arc::new(fastdds_transport().with_accepted_vendors(vec![[0x01, 0x0F]]));
        let done = serve_in_background(&picky);
        let client = fastdds_transport();
        assert_eq!(
            Transport::send(&client, &picky.local_locator(), b"x"),
            Err(SendError::Io {
                message: "tcp fastdds peer refused the bind"
            })
        );
        // The vendor it takes is let in.
        let eprosima = fastdds_transport().with_vendor_id([0x01, 0x0F]);
        Transport::send(&eprosima, &picky.local_locator(), b"y").unwrap();
        assert_eq!(
            &datagram_within(&picky, Duration::from_secs(5))
                .unwrap()
                .data[..],
            b"y"
        );
        eprosima.close();
        picky.close();
        done.recv_timeout(Duration::from_secs(5))
            .expect("serve returns after close");
    }
}
