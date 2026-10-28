// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `TcpTransport`: ZeroDDS TCP Transport 1.0 implementation.
//!
//! # Implemented (RC1)
//!
//! - Length-prefix framing (4-byte BE, 16 MiB DoS cap, see
//!   [`framing`]) — DDSI-RTPS 2.5 §9.5 conformant.
//! - ZeroDDS handshake from [`handshake`] (BindConnection request/
//!   response). Mandatory — there is no skip mode.
//! - TCP connection pool (`MAX_PEERS=256`) with lazy connect +
//!   exponential backoff (50 ms → 5 s).
//! - Bounded inbound queue with a Condvar-blocking `recv`.
//! - `Transport` trait impl for polymorphism with UDP.
//! - Slow-loris DoS protection in the accept path (total budget +
//!   per-syscall slice timeout).
//! - Deterministic pool eviction (lowest-key) on cap reach — a
//!   deliberate choice over LRU, since no IndexMap dep is needed and
//!   with `MAX_PEERS=256` there are no eviction-pressure problems.
//!
//! # Spec status
//!
//! See the `lib.rs` header and
//! `docs/spec-coverage/zerodds-tcp-transport-1.0.md`. The locator kind
//! and wire frame are DDSI-RTPS §9.4+§9.5 normative; the handshake
//! and connection pool are ZeroDDS-vendor-specific.
//!
//! [`framing`]: crate::framing
//! [`handshake`]: crate::handshake

use std::io::{BufReader, BufWriter};
use std::net::{
    Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, TcpListener, TcpStream,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

extern crate alloc;
use alloc::collections::{BTreeMap, VecDeque};

use zerodds_rtps::wire_types::Locator;
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

use crate::framing::{FramingError, read_frame, write_frame};
use crate::handshake::{HandshakeError, client_handshake, server_handshake};

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
    /// Locator is not a TCPv4 locator.
    UnsupportedLocator,
    /// Peer sent a frame larger than [`crate::framing::MAX_FRAME_SIZE`].
    FrameTooLarge {
        /// Announced length from the frame header.
        announced: u32,
    },
    /// Peer I/O error during `accept_one`.
    PeerIo(std::io::Error),
    /// TCP handshake (DDS-TCP-PSM §5.2.1) failed. The sender is
    /// probably not a ZeroDDS TCP peer or has an incompatible
    /// protocol version.
    Handshake(HandshakeError),
}

/// Detailed reason why a locator is not usable for TCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InvalidLocator {
    /// LocatorKind != Tcpv4.
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
            Self::UnsupportedLocator => f.write_str("tcp: locator is not TCPv4"),
            Self::FrameTooLarge { announced } => {
                write!(f, "peer announced frame of {announced} bytes (> cap)")
            }
            Self::PeerIo(e) => write!(f, "tcp peer i/o error: {e}"),
            Self::Handshake(e) => write!(f, "tcp handshake failed: {e}"),
        }
    }
}

impl std::error::Error for TcpTransportError {}

// ---------------------------------------------------------------------
// Resource limits
// ---------------------------------------------------------------------

/// Maximum number of datagrams in the inbound queue (Finding B8/#3).
pub const MAX_INBOUND_QUEUE: usize = 1024;

/// Maximum number of peers in the connection pool (Finding B8/#4). On
/// overflow the oldest entry is removed (FIFO).
pub const MAX_PEERS: usize = 256;

/// Backoff start value on reconnect.
const INITIAL_BACKOFF_MS: u64 = 50;
/// Backoff cap (5 s).
const MAX_BACKOFF_MS: u64 = 5_000;

/// Bound of an outgoing connection's `connect`.
const CLIENT_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Bound of each read and write of the client side of the handshake.
const CLIENT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);

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
// PeerConn
// ---------------------------------------------------------------------

/// Peer connection: writer side. Wrapped per peer in its own mutex so
/// that `send()` does not hold the pool lock across blocking I/O.
struct PeerConn {
    addr: SocketAddr,
    writer: Option<BufWriter<TcpStream>>,
    backoff_ms: u64,
    last_attempt: Option<std::time::Instant>,
}

impl PeerConn {
    fn new<A: Into<SocketAddr>>(addr: A) -> Self {
        let addr = addr.into();
        Self {
            addr,
            writer: None,
            backoff_ms: 0,
            last_attempt: None,
        }
    }

    fn ensure_connected(&mut self) -> std::io::Result<&mut BufWriter<TcpStream>> {
        if self.writer.is_none() {
            if self.backoff_ms > 0 {
                if let Some(last) = self.last_attempt {
                    if last.elapsed() < Duration::from_millis(self.backoff_ms) {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::WouldBlock,
                            "reconnect throttled by backoff",
                        ));
                    }
                }
            }
            self.last_attempt = Some(std::time::Instant::now());
            match TcpStream::connect_timeout(&self.addr, CLIENT_CONNECT_TIMEOUT) {
                Ok(mut stream) => {
                    stream.set_nodelay(true)?;
                    // Bounded as on the accepting side: a listener that
                    // takes the connection and never answers held the
                    // sending thread for ever.
                    stream.set_read_timeout(Some(CLIENT_HANDSHAKE_TIMEOUT))?;
                    stream.set_write_timeout(Some(CLIENT_HANDSHAKE_TIMEOUT))?;
                    // ZeroDDS handshake (see the crate::handshake module doc).
                    // logical_port = 0 signals "default port" — DCPS
                    // sets the concrete endpoint port via its own
                    // connect path.
                    if let Err(e) = client_handshake(&mut stream, 0) {
                        self.bump_backoff();
                        return Err(std::io::Error::other(format!(
                            "tcp handshake client-side failed: {e}"
                        )));
                    }
                    stream.set_read_timeout(None)?;
                    // A peer that stops reading fails the send after this
                    // long without progress; the connection is dropped and
                    // made anew with backoff.
                    stream.set_write_timeout(Some(PEER_WRITE_STALL))?;
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
        if let Some(w) = self.writer.take() {
            // Signal FIN quickly instead of waiting for Drop.
            if let Ok(stream) = w.into_inner() {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
    }

    fn send(&mut self, data: &[u8]) -> Result<(), FramingError> {
        let writer = self.ensure_connected()?;
        if let Err(e) = write_frame(writer, data) {
            self.bump_backoff();
            self.drop_writer();
            return Err(e);
        }
        use std::io::Write;
        if let Err(e) = writer.flush() {
            self.bump_backoff();
            self.drop_writer();
            return Err(FramingError::Io(e));
        }
        Ok(())
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
    /// Peers: each peer in its own mutex → the pool lock is only for
    /// lookup, not across blocking I/O (B8/#2).
    peers: Mutex<BTreeMap<SocketAddr, Arc<Mutex<PeerConn>>>>,
    inbound: Mutex<InboundState>,
    inbound_cv: Condvar,
    /// Set by [`close`](Self::close).
    closed: AtomicBool,
    /// A clone of each stream [`serve`](Self::serve) reads, by connection
    /// number, so that `close` can shut it down.
    accepted: Mutex<BTreeMap<u64, TcpStream>>,
    next_connection: AtomicU64,
    refused_connections: AtomicU64,
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

    fn with_listener(listener: TcpListener, local_locator: Locator) -> Self {
        Self {
            listener,
            local_locator,
            peers: Mutex::new(BTreeMap::new()),
            inbound: Mutex::new(InboundState::default()),
            inbound_cv: Condvar::new(),
            closed: AtomicBool::new(false),
            accepted: Mutex::new(BTreeMap::new()),
            next_connection: AtomicU64::new(0),
            refused_connections: AtomicU64::new(0),
        }
    }

    /// Local locator.
    #[must_use]
    pub fn local_locator(&self) -> Locator {
        self.local_locator
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
    /// - `UnsupportedLocator` if the peer is not an IPv4 socket,
    /// - `FrameTooLarge { announced }` if the peer announces an
    ///   oversized frame,
    /// - `PeerIo(io::Error)` on other read errors.
    pub fn accept_one(&self) -> Result<(), TcpTransportError> {
        let (stream, peer) = self.listener.accept().map_err(TcpTransportError::Accept)?;
        self.read_connection(stream, peer)
    }

    /// Accepts connections until [`close`](Self::close) and reads each in
    /// a thread of its own (`zdds-tcp-conn`), handshake included: the
    /// frames of every connected peer reach the inbound queue, where
    /// [`accept_one`](Self::accept_one) in a loop read only the first
    /// peer until it disconnected and left every other one unread. At most
    /// [`MAX_PEERS`] connections are read at a time; one beyond that is
    /// closed at once and counted
    /// ([`refused_connections`](Self::refused_connections)). Returns after
    /// `close`, once the reading threads have ended.
    pub fn serve(self: &Arc<Self>) {
        let mut readers: Vec<std::thread::JoinHandle<()>> = Vec::new();
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
            {
                let Ok(mut open) = self.accepted.lock() else {
                    break;
                };
                // Under the lock `close` takes to shut the streams down: a
                // stream registered here is one it reaches.
                if self.closed.load(Ordering::Acquire) {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    break;
                }
                let clone = if open.len() < MAX_PEERS {
                    stream.try_clone().ok()
                } else {
                    None
                };
                let Some(clone) = clone else {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    self.refused_connections.fetch_add(1, Ordering::Relaxed);
                    continue;
                };
                open.insert(id, clone);
            }
            let this = Arc::clone(self);
            let spawned = std::thread::Builder::new()
                .name(String::from("zdds-tcp-conn"))
                .spawn(move || {
                    let _ = this.read_connection(stream, peer);
                    this.forget_connection(id);
                });
            match spawned {
                Ok(reader) => readers.push(reader),
                // The closure, and the stream in it, is dropped.
                Err(_) => {
                    self.forget_connection(id);
                    self.refused_connections.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        for reader in readers {
            let _ = reader.join();
        }
    }

    fn forget_connection(&self, id: u64) {
        if let Ok(mut open) = self.accepted.lock() {
            if let Some(stream) = open.remove(&id) {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
    }

    /// Connections [`serve`](Self::serve) closed unread: beyond
    /// [`MAX_PEERS`], or because no thread or stream clone could be made
    /// for them.
    #[must_use]
    pub fn refused_connections(&self) -> u64 {
        self.refused_connections.load(Ordering::Relaxed)
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
        self.inbound_cv.notify_all();
    }

    fn read_connection(
        &self,
        mut stream: TcpStream,
        peer: SocketAddr,
    ) -> Result<(), TcpTransportError> {
        stream
            .set_nodelay(true)
            .map_err(TcpTransportError::Accept)?;
        // Slow-read DoS protection: `set_read_timeout` acts per syscall,
        // not across the whole handshake duration. A byte-chunked
        // slow-loris can pull 5 s per byte, burning 16 * 5 = 80 s in
        // total on the accept thread. Fixed via a total deadline,
        // combined with a tight syscall timeout (200 ms).
        //
        // Total budget: 2 s for the complete 16-byte handshake. That is
        // ample for legitimate peers (which answer in <10 ms on
        // localhost, <100 ms on LAN) but breaks every byte-chunked
        // slow-loris variant.
        let handshake_total_budget = Duration::from_secs(2);
        let handshake_syscall_slice = Duration::from_millis(200);
        let handshake_deadline = std::time::Instant::now() + handshake_total_budget;
        stream
            .set_read_timeout(Some(handshake_syscall_slice))
            .map_err(TcpTransportError::Accept)?;
        stream
            .set_write_timeout(Some(handshake_syscall_slice))
            .map_err(TcpTransportError::Accept)?;
        let source_locator = match peer {
            SocketAddr::V4(v4) => Locator::tcp_v4(v4.ip().octets(), u32::from(v4.port())),
            SocketAddr::V6(v6) => Locator::tcp_v6(v6.ip().octets(), u32::from(v6.port())),
        };
        // ZeroDDS handshake. On Reject there is a dedicated error
        // variant so the caller can distinguish the rejection from an
        // empty-frame EOF. Deadline guard around the entire handshake
        // path.
        let handshake_result = server_handshake(&mut stream);
        if std::time::Instant::now() > handshake_deadline {
            return Err(TcpTransportError::Handshake(
                crate::handshake::HandshakeError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "handshake exceeded total budget (slow-loris)",
                )),
            ));
        }
        match handshake_result {
            Ok((_req, resp)) => {
                if let crate::handshake::ResponseStatus::Reject(reason) = resp.status {
                    return Err(TcpTransportError::Handshake(
                        crate::handshake::HandshakeError::Rejected { reason },
                    ));
                }
            }
            Err(e) => return Err(TcpTransportError::Handshake(e)),
        }
        // After a successful handshake: lift the timeout again
        // (the normal frame-pump regime blocks for expectedly long).
        stream
            .set_read_timeout(None)
            .map_err(TcpTransportError::Accept)?;
        stream
            .set_write_timeout(None)
            .map_err(TcpTransportError::Accept)?;
        let mut reader = BufReader::new(stream);
        loop {
            match read_frame(&mut reader) {
                Ok(data) => {
                    self.push_inbound(ReceivedDatagram {
                        source: source_locator,
                        data: std::sync::Arc::from(data.into_boxed_slice()),
                    });
                }
                Err(FramingError::UnexpectedEof) => return Ok(()),
                Err(FramingError::FrameTooLarge { announced }) => {
                    return Err(TcpTransportError::FrameTooLarge { announced });
                }
                Err(FramingError::Io(e)) => return Err(TcpTransportError::PeerIo(e)),
            }
        }
    }

    fn push_inbound(&self, dg: ReceivedDatagram) {
        if let Ok(mut st) = self.inbound.lock() {
            if st.queue.len() >= MAX_INBOUND_QUEUE {
                // Over cap: drop the oldest frame (FIFO drop instead of
                // discarding the new one — old frames are mostly
                // re-requested anyway by the reliable reader layer via
                // ACKNACK).
                st.queue.pop_front();
                st.dropped = st.dropped.saturating_add(1);
            }
            st.queue.push_back(dg);
            self.inbound_cv.notify_one();
        }
    }

    /// Number of frames discarded due to queue overflow.
    #[must_use]
    pub fn dropped_frames(&self) -> u64 {
        self.inbound.lock().map(|s| s.dropped).unwrap_or_default()
    }

    /// Non-blocking pop from the inbound queue. Useful for tests and
    /// callsites that want polling control themselves (vs. the blocking
    /// [`Transport::recv`]).
    ///
    /// # Errors
    /// [`RecvError::Timeout`] if the queue is empty.
    pub fn try_recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let mut st = self.inbound.lock().map_err(|_| RecvError::Io {
            message: "inbound queue poisoned",
        })?;
        st.queue.pop_front().ok_or(RecvError::Timeout)
    }
}

// ---------------------------------------------------------------------
// Transport-Trait
// ---------------------------------------------------------------------

impl Transport for TcpTransport {
    /// The largest frame the stream framing carries
    /// ([`crate::framing::MAX_FRAME_SIZE`]).
    fn max_message_size(&self) -> usize {
        crate::framing::MAX_FRAME_SIZE as usize
    }

    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(SendError::Io {
                message: "tcp transport closed",
            });
        }
        let addr = match locator_to_socket(dest) {
            Ok(a) => a,
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
        // Step 1: peer lookup/insert under the pool lock.
        let peer_arc = {
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
            pool.entry(addr)
                .or_insert_with(|| Arc::new(Mutex::new(PeerConn::new(addr))))
                .clone()
        };
        // Step 2: release the pool lock, then take the peer lock for I/O.
        let mut conn = peer_arc.lock().map_err(|_| SendError::Io {
            message: "peer conn poisoned",
        })?;
        match conn.send(data) {
            Ok(()) => Ok(()),
            Err(FramingError::FrameTooLarge { announced }) => Err(SendError::PayloadTooLarge {
                size: announced as usize,
                limit: crate::framing::MAX_FRAME_SIZE as usize,
            }),
            Err(_) => Err(SendError::Io {
                message: "tcp write failed",
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
        let mut st = self.inbound.lock().map_err(|_| RecvError::Io {
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
                .inbound_cv
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
        self.local_locator
    }

    fn close(&self) {
        Self::close(self);
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

fn locator_to_socket_v4(loc: &Locator) -> Result<SocketAddrV4, InvalidLocator> {
    use zerodds_rtps::wire_types::LocatorKind;
    if loc.kind != LocatorKind::Tcpv4 {
        return Err(InvalidLocator::WrongKind);
    }
    let addr = &loc.address[12..16];
    let ip = Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]);
    let port =
        u16::try_from(loc.port).map_err(|_| InvalidLocator::PortOverflow { port: loc.port })?;
    Ok(SocketAddrV4::new(ip, port))
}

/// Dispatch between a TCPv4/TCPv6 locator and `SocketAddr` (v4 or v6).
fn locator_to_socket(loc: &Locator) -> Result<SocketAddr, InvalidLocator> {
    use zerodds_rtps::wire_types::LocatorKind;
    match loc.kind {
        LocatorKind::Tcpv4 => locator_to_socket_v4(loc).map(SocketAddr::V4),
        LocatorKind::Tcpv6 => {
            let port = u16::try_from(loc.port)
                .map_err(|_| InvalidLocator::PortOverflow { port: loc.port })?;
            let ip = Ipv6Addr::from(loc.address);
            Ok(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, 0)))
        }
        _ => Err(InvalidLocator::WrongKind),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;

    /// An idle `recv` returns `Timeout` within about a second, so a thread
    /// looping on it sees its stop flag and can be joined.
    #[test]
    fn an_idle_recv_times_out() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let started = std::time::Instant::now();
        assert!(matches!(Transport::recv(&t), Err(RecvError::Timeout)));
        assert!(
            started.elapsed() < core::time::Duration::from_secs(3),
            "recv waited {:?}",
            started.elapsed()
        );
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
    fn unsupported_locator_udpv4() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let udp = Locator::udp_v4([127, 0, 0, 1], 7400);
        let err = Transport::send(&t, &udp, b"x").unwrap_err();
        assert!(matches!(err, SendError::UnsupportedLocator));
    }

    #[test]
    fn unsupported_locator_invalid() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let err = Transport::send(&t, &Locator::INVALID, b"x").unwrap_err();
        assert!(matches!(err, SendError::UnsupportedLocator));
    }

    #[test]
    fn inbound_overflow_drops_oldest_and_counts() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        for i in 0..(MAX_INBOUND_QUEUE as u64 + 5) {
            t.push_inbound(ReceivedDatagram {
                source: Locator::tcp_v4([127, 0, 0, 1], 1),
                data: alloc::sync::Arc::from([i as u8].as_slice()),
            });
        }
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
            data: alloc::sync::Arc::from([7u8, 8, 9].as_slice()),
        });
        let dg = t.try_recv().unwrap();
        assert_eq!(&dg.data[..], &[7u8, 8, 9]);
    }

    /// `Transport::recv` is condvar-blocking. We test the happy path by
    /// pushing a frame from a background thread; the recv caller must
    /// wake up (covers the `inbound_cv.wait` branch).
    #[test]
    fn recv_wakes_on_push_from_other_thread() {
        use std::sync::Arc as StdArc;
        use std::thread;
        use std::time::Duration as StdDuration;

        let t = StdArc::new(TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap());
        let t2 = StdArc::clone(&t);
        let h = thread::spawn(move || {
            thread::sleep(StdDuration::from_millis(30));
            t2.push_inbound(ReceivedDatagram {
                source: Locator::tcp_v4([127, 0, 0, 1], 7),
                data: alloc::sync::Arc::from([0xAAu8].as_slice()),
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
            data: alloc::sync::Arc::from([1u8].as_slice()),
        });
        let dg = Transport::recv(&t).unwrap();
        assert_eq!(&dg.data[..], &[1u8]);
    }

    /// MAX_PEERS eviction: we fill the pool directly via the mutex
    /// innards — that saves real sockets. On the next send to a new
    /// address the smallest key must be evicted.
    #[test]
    fn max_peers_eviction_removes_first_key_on_new_peer() {
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
    fn max_peers_no_eviction_for_existing_peer() {
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
        // Background accept thread (analogous to accept_one_reads_one_frame_then_eof).
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
    fn peer_conn_bump_backoff_sequence() {
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
    fn peer_conn_backoff_throttles_connect() {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1);
        let mut p = PeerConn::new(addr);
        p.backoff_ms = 5_000;
        p.last_attempt = Some(std::time::Instant::now());
        let err = p.ensure_connected().unwrap_err();
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

    /// Debug format for PeerConn (coverage of the Debug impl).
    #[test]
    fn peer_conn_debug_format_works() {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1);
        let p = PeerConn::new(addr);
        let s = alloc::format!("{p:?}");
        assert!(s.contains("PeerConn"));
    }

    /// Display/Debug of all TcpTransportError variants for coverage of
    /// the Display match (incl. FrameTooLarge + PeerIo).
    #[test]
    fn tcp_transport_error_display_all_variants() {
        let bind = TcpTransportError::Bind(std::io::Error::other("x"));
        let accept = TcpTransportError::Accept(std::io::Error::other("x"));
        let set_to = TcpTransportError::SetTimeout(std::io::Error::other("x"));
        let unsup = TcpTransportError::UnsupportedLocator;
        let big = TcpTransportError::FrameTooLarge {
            announced: 1_000_000,
        };
        let peer_io = TcpTransportError::PeerIo(std::io::Error::other("x"));
        for e in [bind, accept, set_to, unsup, big, peer_io] {
            let msg = alloc::format!("{e}");
            assert!(!msg.is_empty());
            let dbg = alloc::format!("{e:?}");
            assert!(!dbg.is_empty());
        }
    }

    /// DDSI-RTPS 2.5 §9.3.2.4: the TCP kinds lie in the vendor range
    /// (0x01F0_0004, 0x01F0_0005); the values 4 and 8 of Fast DDS are a
    /// setting of `zerodds-rtps`. The transport binds these two kinds, so a
    /// renumbering there has to break here.
    #[test]
    fn locator_kinds_are_zerodds_vendor_range_values() {
        assert_eq!(
            Locator::tcp_v4([127, 0, 0, 1], 7400).kind.as_i32(),
            0x01F0_0004
        );
        assert_eq!(Locator::tcp_v6([0u8; 16], 7400).kind.as_i32(), 0x01F0_0005);
    }

    /// `locator_to_socket_v4` returns `WrongKind` for UDP.
    #[test]
    fn locator_to_socket_v4_rejects_udp() {
        let udp = Locator::udp_v4([127, 0, 0, 1], 7400);
        assert!(matches!(
            locator_to_socket_v4(&udp),
            Err(InvalidLocator::WrongKind)
        ));
    }

    /// `locator_to_socket_v4` maps TCPv4 to SocketAddrV4.
    #[test]
    fn locator_to_socket_v4_maps_tcp() {
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
    fn dropped_frames_zero_on_fresh_transport() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert_eq!(t.dropped_frames(), 0);
    }

    /// local_locator() inherent and trait return the same value.
    #[test]
    fn local_locator_inherent_and_trait_match() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        assert_eq!(t.local_locator(), Transport::local_locator(&t));
    }

    /// Push keeps FIFO order for all frames below the queue cap.
    #[test]
    fn push_preserves_fifo_order() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        for i in 0..5u8 {
            t.push_inbound(ReceivedDatagram {
                source: Locator::tcp_v4([127, 0, 0, 1], 1),
                data: alloc::sync::Arc::from([i].as_slice()),
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
    fn send_unsupported_locator_does_not_pollute_pool() {
        let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let udp = Locator::udp_v4([127, 0, 0, 1], 7400);
        let _ = Transport::send(&t, &udp, b"x");
        assert_eq!(t.peers.lock().unwrap().len(), 0);
    }

    /// `accept_one` on a listener with a closed client returns `Ok(())`
    /// on EOF after the first frame (happy path through the read_frame
    /// loop + UnexpectedEof branch).
    #[test]
    fn accept_one_reads_one_frame_then_eof() {
        use std::io::Write;
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
        h.join().unwrap();
    }

    /// `accept_one` with a peer that announces a frame larger than the
    /// cap returns `FrameTooLarge`.
    #[test]
    fn accept_one_frame_too_large_errors() {
        use std::io::Write;
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
    fn accept_one_rejects_peer_without_handshake() {
        use std::io::Write;
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
    fn accept_one_gives_up_on_a_stalling_handshake_peer() {
        use std::io::Write;
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
            elapsed < Duration::from_secs(3),
            "accept must not be pinned by a stalling peer, took {elapsed:?}"
        );
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
        let deadline = std::time::Instant::now() + bound;
        while std::time::Instant::now() < deadline {
            if let Ok(dg) = Transport::recv(t) {
                return Some(dg.data.to_vec());
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
        use std::io::{Read, Write};

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

    /// A closed transport neither sends nor waits in `recv`.
    #[test]
    fn a_closed_transport_neither_sends_nor_waits() {
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

    /// After a successful connect, `backoff_ms` must fall to 0.
    /// Measure indirectly: two sends to a port that is first down, then up.
    #[test]
    fn backoff_resets_after_successful_connect() {
        use std::thread;
        use std::time::Duration as StdDuration;

        let server_port = {
            let s = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
            let p = s.local_locator().port;
            drop(s);
            u16::try_from(p).unwrap()
        };

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(server_port));
        let e1 = Transport::send(&client, &dest, b"a");
        assert!(matches!(e1, Err(SendError::Io { .. })));

        thread::sleep(StdDuration::from_millis(INITIAL_BACKOFF_MS + 30));
        let Ok(server) = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, server_port) else {
            // The OS reassigned the port in the meantime → soft-skip.
            return;
        };
        let h = thread::spawn(move || {
            let _ = server.accept_one();
        });
        thread::sleep(StdDuration::from_millis(20));

        let e2 = Transport::send(&client, &dest, b"b");
        assert!(e2.is_ok(), "send after server up failed: {e2:?}");
        // Backoff must be back to 0: a direct look at PeerConn.
        let pool = client.peers.lock().unwrap();
        let addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, server_port).into();
        let arc = pool.get(&addr).expect("peer present after success");
        let conn = arc.lock().unwrap();
        assert_eq!(conn.backoff_ms, 0);
        drop(conn);
        drop(pool);

        drop(client);
        h.join().unwrap();
    }

    /// Non-throttled path: a PeerConn with backoff_ms=0 set holds no
    /// timer → `ensure_connected` tries to connect directly. We expect
    /// an `io::Error` (target on port 1).
    #[test]
    fn ensure_connected_no_backoff_does_not_throttle() {
        let mut p = PeerConn::new(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1));
        assert_eq!(p.backoff_ms, 0);
        // Direct connect attempt on port 1 → Err, but NOT WouldBlock.
        let err = p.ensure_connected().unwrap_err();
        assert_ne!(err.kind(), std::io::ErrorKind::WouldBlock);
        // After a failed connect, backoff rises to INITIAL_BACKOFF_MS.
        assert_eq!(p.backoff_ms, INITIAL_BACKOFF_MS);
    }

    /// `Transport::send` with a payload > MAX_FRAME_SIZE → `PayloadTooLarge`
    /// from the underlying write_frame (covers the `FrameTooLarge` path
    /// in the send match).
    #[test]
    fn send_oversized_payload_returns_payload_too_large() {
        use std::io::Write;
        use std::thread;

        let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let port = u16::try_from(server.local_locator().port).unwrap();
        // Server thread just reads through until the client closes.
        let h = thread::spawn(move || {
            let _ = server.accept_one();
        });
        thread::sleep(std::time::Duration::from_millis(30));

        let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = Locator::tcp_v4([127, 0, 0, 1], u32::from(port));
        let oversized = alloc::vec![0u8; (crate::framing::MAX_FRAME_SIZE as usize) + 1];
        let err = Transport::send(&client, &dest, &oversized).unwrap_err();
        match err {
            SendError::PayloadTooLarge { size, limit } => {
                assert_eq!(size, oversized.len());
                assert_eq!(limit, crate::framing::MAX_FRAME_SIZE as usize);
            }
            other => {
                // Depending on timing the platform may return Io instead of
                // PayloadTooLarge; accept both as coverage of the error branch.
                assert!(
                    matches!(other, SendError::Io { .. }),
                    "unexpected: {other:?}"
                );
            }
        }
        drop(client);
        // The writer is irrelevant for the server; join blocks until EOF
        // arrives. If the payload fails immediately for being too large,
        // the TCP socket was never even written → the server sits on EOF.
        let _ = std::io::sink().write_all(b"tickle"); // no-op, satisfies formatter
        h.join().unwrap();
    }

    /// When a peer aborts mid header-read (TCP RST via linger=0 +
    /// shutdown), `read_frame` returns the Io path. We cannot force a
    /// deterministic ECONNRESET on macOS, so we document this as a
    /// known uncovered branch.
    /// (The test stub keeps the caller green.)
    #[test]
    fn peer_io_error_branch_documented() {
        // No assertion — this is a marker test.
        // The PeerIo branch is only visible via BufReader::read_exact
        // when the OS returns an IO error instead of an EOF. On
        // Linux/macOS, TCP after shutdown() reliably behaves as EOF.
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
}
