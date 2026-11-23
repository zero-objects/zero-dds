// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Federation of agents (DDS-XRCE 1.0 §10.4): "XRCE Agents appear as Clients
//! to other XRCE Agents. In order to support these deployments the XRCE Agent
//! implementation must implement the client-side of the XRCE Protocol in
//! addition to the server part." Figure 26: "The Agents enable federations
//! and store-and-forward dataflow. This type of deployment is transparent to
//! the XRCE Client applications and the DDS applications."
//!
//! An agent with an `upstream:` opens, for each of its clients it routes
//! there, an XRCE session at the upstream agent with the client's
//! `ClientKey`, `session_id` and `properties`, and relays the client's object
//! operations (`CREATE`, `DELETE`, `WRITE_DATA`, `READ_DATA`, `GET_INFO`)
//! unchanged, with the client's `request_id` and `ObjectId`, on the stream
//! they came on. The answers of the upstream agent (`STATUS`, `INFO`,
//! `DATA`) go back to the client on the stream they came on. Each hop runs
//! its own reliability and `FRAGMENT` reassembly: the relay holds a message
//! until the next hop has it (store and forward). A request on a reliable
//! stream counts as the upstream agent's once it acknowledged every message
//! that carries it (§8.4.14.1) or answered it; a `FRAGMENT` run goes out whole
//! or not at all, the part the sender window has no room for following as the
//! upstream agent's ACKNACKs free it.
//!
//! The answers of the upstream agent on a reliable stream reach the client
//! only as far as the client's sender window on that stream has room
//! (§8.4.14): the link takes no more of them from its reliable input stream
//! ([`XrceClient::poll_held`]) and leaves the rest there unacknowledged, so
//! the upstream agent's sender window fills and the upstream agent takes no
//! more samples out of its DataReaders for the client. When the client's
//! ACKNACK frees its window, [`Upstream::release`] hands over what waited and
//! acknowledges it upstream at once. Of the answers on a reliable stream the
//! relay holds no more than the client's window: 16 messages, and the
//! `FRAGMENT`s of the last answer it took when that answer is larger than the
//! client's MTU.
//!
//! While the upstream agent cannot be reached, the requests wait in a bounded
//! queue (`max_queued_messages`); a request that finds the queue full is
//! answered `STATUS_ERR_RESOURCES`, never dropped, and so is a request the
//! upstream MTU cannot carry. A link that hears nothing of the upstream agent
//! for `link_timeout_ms` (keepalive `GET_INFO` of `OBJECTID_AGENT` every
//! `keepalive_ms`), or whose transport fails, is opened again: a
//! `CREATE_CLIENT` with the other of two `session_id`s (the client's, and the
//! next one in its half), never the one of the session that was open, so it
//! replaces a session the upstream agent still holds with its objects and
//! cached messages (§7.8.2.1); then the `CREATE`s of the objects that exist,
//! in their order, the running `READ_DATA`s, the requests the upstream agent
//! had neither acknowledged nor answered, in their order, and then the queue.
//! A request the upstream agent got but whose acknowledgement was lost with
//! the link is thus carried out again (at least once, never lost). The
//! `STATUS` of a `CREATE` the client already had answered stays with the
//! relay. A `CREATE_CLIENT` the upstream agent refuses answers the waiting
//! requests, and the later ones, with its `ResultStatus`. A link opened
//! again after `link_timeout_ms` gets a new transport: a TCP connection that
//! hears nothing may be dead without the kernel having noticed.
//!
//! The first session of a link replaces any session the upstream agent may
//! still hold for the client under either `session_id`: one an earlier
//! process of this agent left there, or one whose `DELETE` was lost. Its
//! `CREATE_CLIENT` carries the other `session_id` and, once the upstream
//! agent answered it, a second one the client's own; two `session_id`s in a
//! row always end in a new ProxyClient whose streams start at sequence
//! number 0 (§7.8.2.1).
//!
//! A `RESET` of a routed client (§8.3.5.13) ends its reads at the upstream
//! agent: each running `READ_DATA`, and each one the upstream agent may have
//! started, gets a `READ_DATA` with `max_samples` `MAX_SAMPLES_ZERO` on its
//! stream, which "cancel[s] the currently active read operation without
//! sending any more samples" (§7.8.5.1 Table 9, §8.3.5.9.2). The reads leave
//! the journal, the `READ_DATA`s that wait leave the queue, and a `DATA` of
//! an ended read that is still on its way does not reach the client.
//!
//! The relay runs under the lock of the session engine, so no call of a
//! link may block. A UDP link sends and reads without blocking. A TCP link
//! ([`TcpLink`]) is connected, written and read by threads of its own, as
//! the agent's own TCP connections are written by one thread each: an
//! upstream agent that drops the connection attempt or stops reading delays
//! no client of this agent. What it does not take waits in the link up to
//! [`TCP_LINK_SEND_LIMIT`] octets beyond the socket's send buffer; past
//! that the link takes no more and the requests wait in the relay's queue,
//! and when the upstream agent is silent for `link_timeout_ms` the link is
//! opened again on a new connection. A dropped TCP link writes what it was
//! handed (the `DELETE` of a closed session) before it ends the connection,
//! within [`TCP_LINK_CLOSE_LIMIT`].

#![allow(clippy::print_stderr)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use zerodds_xrce::header::{ClientKey, MessageHeader, StreamId};
use zerodds_xrce::object_id::{OBJECTID_AGENT, ObjectId};
use zerodds_xrce::object_info::{BaseObjectReply, BaseObjectRequest, InfoMask, ResultStatusCode};
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::submessages::{Submessage, SubmessageHeader, SubmessageId};
use zerodds_xrce_client::discovery::UdpTransport;
use zerodds_xrce_client::{ClientError, ClientEvent, ClientState, ClientTransport, XrceClient};

use crate::daemon::handshake::ClientRepresentation;
use crate::daemon::session::ClientKeyOrd;

/// The relay of a [`crate::daemon::session::SessionEngine`] to an upstream
/// agent (DDS-XRCE 1.0 §10.4).
pub trait Upstream: Send {
    /// Whether the operations of `client` go to the upstream agent.
    fn routes(&self, client: ClientKeyOrd) -> bool;
    /// The agent accepted the `CREATE_CLIENT` of `client`: open its session
    /// at the upstream agent.
    fn open(&mut self, client: ClientKeyOrd, representation: &ClientRepresentation, now: Duration);
    /// The session of `client` ended at this agent: end it upstream.
    fn close(&mut self, client: ClientKeyOrd);
    /// `client` sent a `RESET` (§8.3.5.13): its reads end at the upstream
    /// agent, and no `DATA` of them reaches the client any more.
    fn reset(&mut self, client: ClientKeyOrd);
    /// Relays one request of `client` that came on `stream_id`.
    ///
    /// # Errors
    /// [`UpstreamRefusal`]: the request is not taken and must be answered.
    fn forward(
        &mut self,
        client: ClientKeyOrd,
        stream_id: u8,
        submessage: Submessage,
        now: Duration,
    ) -> Result<(), UpstreamRefusal>;
    /// Reads what the upstream agent sent, keeps the links alive and opens
    /// lost ones again. `room(client, stream_id)` is how many more answers
    /// the client's output stream `stream_id` takes now; an answer on a
    /// reliable stream beyond it stays unacknowledged at the upstream agent
    /// ([`Reply`](UpstreamEvent::Reply) comes for it in a later call).
    fn poll(
        &mut self,
        now: Duration,
        room: &mut dyn FnMut(ClientKeyOrd, u8) -> usize,
    ) -> Vec<UpstreamEvent>;
    /// `client` acknowledged messages and its output streams have room again:
    /// polls its link alone and hands over the answers that waited, as
    /// [`Self::poll`] does.
    fn release(
        &mut self,
        client: ClientKeyOrd,
        now: Duration,
        room: &mut dyn FnMut(ClientKeyOrd, u8) -> usize,
    ) -> Vec<UpstreamEvent>;
}

/// Why [`Upstream::forward`] did not take a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamRefusal {
    /// The queue of the link is full (`STATUS_ERR_RESOURCES`).
    QueueFull,
    /// The upstream agent refused the session with this status.
    Refused(ResultStatusCode),
    /// No link for the client.
    NoLink,
    /// The upstream link cannot carry the request (`STATUS_ERR_RESOURCES`):
    /// its MTU leaves no room for the `FRAGMENT`s it needs.
    Unsendable,
    /// A request on a best-effort stream or on `STREAMID_NONE` that is
    /// larger than one message of the upstream link (`STATUS_ERR_RESOURCES`):
    /// only a reliable stream splits a message into `FRAGMENT`s (§8.3.5.14).
    TooLarge {
        /// The octets of the message that would carry it.
        size: usize,
        /// The largest message of the link
        /// ([`UpstreamLocator::max_message_size`]).
        limit: usize,
    },
}

/// What [`Upstream::poll`] observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpstreamEvent {
    /// An answer of the upstream agent for `client` on `stream_id`.
    Reply {
        /// The client.
        client: ClientKeyOrd,
        /// The stream the answer came on.
        stream_id: u8,
        /// `STATUS`, `INFO` or `DATA`, unchanged.
        submessage: Submessage,
    },
    /// The session of `client` at the upstream agent is open.
    Connected {
        /// The client.
        client: ClientKeyOrd,
    },
    /// The link of `client` was lost; it is being opened again.
    Lost {
        /// The client.
        client: ClientKeyOrd,
        /// Why.
        reason: String,
    },
    /// The upstream agent refused the `CREATE_CLIENT` of `client`; the
    /// requests that waited are in `pending`.
    Refused {
        /// The client.
        client: ClientKeyOrd,
        /// The status of the `STATUS_AGENT`.
        code: ResultStatusCode,
        /// The requests that waited, with their streams.
        pending: Vec<(u8, Submessage)>,
    },
    /// A `FRAGMENT` run of the upstream agent could not be reassembled; what
    /// it carried is lost.
    FragmentDiscarded {
        /// The client.
        client: ClientKeyOrd,
        /// The stream.
        stream_id: u8,
    },
    /// A request that waited cannot be carried upstream; it is answered
    /// `STATUS_ERR_RESOURCES`.
    Unsendable {
        /// The client.
        client: ClientKeyOrd,
        /// The stream the request came on.
        stream_id: u8,
        /// The request.
        submessage: Submessage,
        /// Why.
        reason: String,
    },
}

/// The smallest `upstream.mtu`: a message header with a `ClientKey` (8
/// octets, §8.3.2) and a `FRAGMENT` submessage header (4 octets, §8.3.5.14)
/// leave one octet of `FRAGMENT` data.
#[allow(clippy::cast_possible_truncation)]
pub const MIN_UPSTREAM_MTU: u16 =
    (MessageHeader::WIRE_SIZE_WITH_KEY + SubmessageHeader::WIRE_SIZE + 1) as u16;

/// Where the upstream agent is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamLocator {
    /// `udp://host:port` (DDS-XRCE 1.0 §11.2).
    Udp(SocketAddr),
    /// `tcp://host:port` (DDS-XRCE 1.0 §11.3).
    Tcp(SocketAddr),
}

impl UpstreamLocator {
    /// The largest message the link carries in one piece: a UDP datagram
    /// (65 507 octets over IPv4, 65 527 over IPv6) or a TCP frame, whose
    /// 2-octet length (§11.3.3) counts up to 65 535 octets.
    #[must_use]
    pub fn max_message_size(&self) -> usize {
        match self {
            Self::Udp(SocketAddr::V4(_)) => 65_507,
            Self::Udp(SocketAddr::V6(_)) => 65_527,
            Self::Tcp(_) => usize::from(u16::MAX),
        }
    }

    /// Reads `udp://host:port` or `tcp://host:port`.
    ///
    /// # Errors
    /// A text that is neither, naming what is expected.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (scheme, rest) = text
            .split_once("://")
            .ok_or_else(|| format!("`{text}`: expected udp://host:port or tcp://host:port"))?;
        let address = std::net::ToSocketAddrs::to_socket_addrs(rest)
            .map_err(|e| format!("`{text}`: {e}"))?
            .next()
            .ok_or_else(|| format!("`{text}`: the host has no address"))?;
        match scheme {
            "udp" => Ok(Self::Udp(address)),
            "tcp" => Ok(Self::Tcp(address)),
            other => Err(format!(
                "`{text}`: the scheme `{other}` is not one of the upstream link (udp, tcp)"
            )),
        }
    }
}

/// The `upstream:` block of the daemon configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamConfig {
    /// `locator:`.
    pub locator: UpstreamLocator,
    /// `mtu:` — the MTU the relay asks for at the upstream agent.
    pub mtu: u16,
    /// `keepalive_ms:` — how often the relay asks the upstream agent about
    /// itself while a session is open.
    pub keepalive: Duration,
    /// `link_timeout_ms:` — after this long without a message of the
    /// upstream agent, the link counts as lost and is opened again.
    pub link_timeout: Duration,
    /// `reconnect_ms:` — the wait before a new attempt after a failed one.
    pub reconnect: Duration,
    /// `max_queued_messages:` — requests a link holds while the upstream
    /// agent cannot take them.
    pub max_queued_messages: usize,
    /// `clients:` — the ClientKeys routed upstream; `None` routes all.
    pub clients: Option<Vec<ClientKeyOrd>>,
}

impl UpstreamConfig {
    /// The defaults for `locator`.
    #[must_use]
    pub fn new(locator: UpstreamLocator) -> Self {
        Self {
            locator,
            mtu: crate::daemon::handshake::DEFAULT_MTU,
            keepalive: Duration::from_millis(1000),
            link_timeout: Duration::from_millis(5000),
            reconnect: Duration::from_millis(1000),
            max_queued_messages: 256,
            clients: None,
        }
    }

    /// Whether the operations of `client` go to the upstream agent.
    #[must_use]
    pub fn routes(&self, client: ClientKeyOrd) -> bool {
        self.clients
            .as_ref()
            .is_none_or(|keys| keys.contains(&client))
    }

    /// Whether a setting for `client` covers a client routed upstream;
    /// `None` stands for every client, which covers the routed ones as soon
    /// as one is routed.
    #[must_use]
    pub fn covers_routed(&self, client: Option<ClientKeyOrd>) -> bool {
        match client {
            Some(key) => self.routes(key),
            None => self.clients.as_ref().is_none_or(|keys| !keys.is_empty()),
        }
    }
}

/// The transport of a link.
#[derive(Debug)]
pub enum LinkTransport {
    /// UDP.
    Udp(UdpTransport),
    /// TCP.
    Tcp(TcpLink),
}

impl ClientTransport for LinkTransport {
    fn send(&mut self, payload: &[u8]) -> Result<(), ClientError> {
        match self {
            Self::Udp(t) => t.send(payload),
            Self::Tcp(t) => t.send(payload),
        }
    }
    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
        match self {
            Self::Udp(t) => t.try_recv(),
            Self::Tcp(t) => t.try_recv(),
        }
    }
}

impl LinkTransport {
    /// The transport to `locator`; a TCP connection gives up after
    /// `connect_timeout`. Neither blocks.
    fn open(locator: UpstreamLocator, connect_timeout: Duration) -> std::io::Result<Self> {
        match locator {
            UpstreamLocator::Udp(address) => UdpTransport::connect(address).map(Self::Udp),
            UpstreamLocator::Tcp(address) => TcpLink::open(address, connect_timeout).map(Self::Tcp),
        }
    }
}

/// The octets a [`TcpLink`] holds for the upstream agent beyond what the
/// socket's send buffer took; a message beyond it is not taken
/// ([`ClientError::QueueFull`]).
pub const TCP_LINK_SEND_LIMIT: usize = 1 << 20;

/// How long a dropped [`TcpLink`] keeps its connection to write what it was
/// handed and to see the upstream agent end the connection; then the
/// connection is shut down, also while a write still waits for an upstream
/// agent that does not read.
pub const TCP_LINK_CLOSE_LIMIT: Duration = Duration::from_millis(1000);

/// The TCP link to the upstream agent (DDS-XRCE 1.0 §11.3; the framing of
/// §11.3.3, a 2-octet little-endian length in front of each message). One
/// thread connects and then writes, one reads: [`ClientTransport::send`]
/// hands the message over and [`ClientTransport::try_recv`] takes what was
/// read, neither waits for the socket. While [`TCP_LINK_SEND_LIMIT`] octets
/// wait for the upstream agent, `send` takes nothing more and returns
/// [`ClientError::QueueFull`]: nothing of the message was sent, and the
/// relay keeps it. A failed connect, write or read and a closed connection
/// make the link fail: every later call returns [`ClientError::Transport`]
/// and the relay opens a new link. Dropping the link returns at once: the
/// writer writes what the link was handed and ends its half of the
/// connection, and the connection is shut down when the upstream agent has
/// ended its half or [`TCP_LINK_CLOSE_LIMIT`] has passed, which ends both
/// threads. A link dropped before its connection was made sends nothing.
#[derive(Debug)]
pub struct TcpLink {
    to_writer: Sender<Vec<u8>>,
    from_reader: Receiver<Vec<u8>>,
    shared: Arc<TcpLinkShared>,
}

#[derive(Debug)]
struct TcpLinkShared {
    /// The upstream agent.
    address: SocketAddr,
    /// Why the link failed; `None` while it works.
    failure: Mutex<Option<String>>,
    /// Octets handed to the writer and not yet in the socket.
    waiting: AtomicUsize,
    /// The connection, for the shutdown after the link was dropped, and
    /// whether it was dropped.
    socket: Mutex<(Option<TcpStream>, bool)>,
    /// The threads of the link that still run, and the signal of each end.
    running: Mutex<usize>,
    ended: Condvar,
}

/// Counts a thread of a [`TcpLink`] as ended when it returns.
struct LinkThread<'a>(&'a TcpLinkShared);

impl Drop for LinkThread<'_> {
    fn drop(&mut self) {
        if let Ok(mut running) = self.0.running.lock() {
            *running = running.saturating_sub(1);
        }
        self.0.ended.notify_all();
    }
}

impl TcpLinkShared {
    /// Shuts the connection down, which ends a write or read that waits.
    fn shut_down(&self) {
        if let Ok(mut socket) = self.socket.lock()
            && let Some(stream) = socket.0.take()
        {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }

    /// After the link was dropped: waits until its threads have ended, at
    /// most `limit`, and shuts the connection down.
    fn close_within(&self, limit: Duration) {
        if let Ok(running) = self.running.lock() {
            let _ = self.ended.wait_timeout_while(running, limit, |n| *n > 0);
        }
        self.shut_down();
    }

    /// Notes the first failure; later ones, and those after the link was
    /// dropped, change nothing.
    fn fail(&self, reason: String) {
        let dropped = self.socket.lock().map_or(true, |s| s.1);
        if let Ok(mut failure) = self.failure.lock()
            && failure.is_none()
        {
            if !dropped {
                eprintln!(
                    "[zerodds-xrce-agentd] upstream link tcp://{}: {reason}",
                    self.address
                );
            }
            *failure = Some(reason);
        }
    }

    fn failed(&self) -> bool {
        self.failure.lock().map_or(true, |f| f.is_some())
    }
}

impl TcpLink {
    /// Starts the connection to `address` and returns at once; the connect
    /// gives up after `connect_timeout`. Messages sent before the
    /// connection is up go out when it is.
    ///
    /// # Errors
    /// The `std::io::Error` of starting the thread.
    pub fn open(address: SocketAddr, connect_timeout: Duration) -> std::io::Result<Self> {
        let (to_writer, writer_rx) = mpsc::channel();
        let (reader_tx, from_reader) = mpsc::channel();
        let shared = Arc::new(TcpLinkShared {
            address,
            failure: Mutex::new(None),
            waiting: AtomicUsize::new(0),
            socket: Mutex::new((None, false)),
            // The writer thread.
            running: Mutex::new(1),
            ended: Condvar::new(),
        });
        let s = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("xrce-upstream-tcp".to_string())
            .spawn(move || {
                let _counted = LinkThread(&s);
                run_tcp_link(connect_timeout, &s, &writer_rx, reader_tx);
            })?;
        Ok(Self {
            to_writer,
            from_reader,
            shared,
        })
    }

    /// Why the link failed, or `None` while it works.
    #[must_use]
    pub fn failure(&self) -> Option<String> {
        self.shared.failure.lock().ok().and_then(|f| f.clone())
    }
}

impl ClientTransport for TcpLink {
    fn send(&mut self, payload: &[u8]) -> Result<(), ClientError> {
        if self.shared.failed() {
            return Err(ClientError::Transport);
        }
        // The same answer as the client's own TCP transport: the 2-octet
        // length cannot state it.
        let len = u16::try_from(payload.len()).map_err(|_| ClientError::QueueFull)?;
        let frame_len = payload.len() + 2;
        let waiting = self.shared.waiting.load(Ordering::Acquire);
        if waiting + frame_len > TCP_LINK_SEND_LIMIT {
            return Err(ClientError::QueueFull);
        }
        let mut frame = Vec::with_capacity(frame_len);
        frame.extend_from_slice(&len.to_le_bytes());
        frame.extend_from_slice(payload);
        self.shared.waiting.fetch_add(frame_len, Ordering::AcqRel);
        self.to_writer
            .send(frame)
            .map_err(|_| ClientError::Transport)
    }

    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
        match self.from_reader.try_recv() {
            Ok(frame) => Ok(Some(frame)),
            Err(TryRecvError::Empty) if !self.shared.failed() => Ok(None),
            Err(_) => Err(ClientError::Transport),
        }
    }
}

impl Drop for TcpLink {
    fn drop(&mut self) {
        if let Ok(mut socket) = self.shared.socket.lock() {
            socket.1 = true;
        }
        // Without its sender, the writer ends after the last frame it was
        // handed.
        drop(std::mem::replace(&mut self.to_writer, mpsc::channel().0));
        let shared = Arc::clone(&self.shared);
        let closer = std::thread::Builder::new()
            .name("xrce-upstream-tcp-close".to_string())
            .spawn(move || shared.close_within(TCP_LINK_CLOSE_LIMIT));
        if closer.is_err() {
            self.shared.shut_down();
        }
    }
}

/// The thread of a [`TcpLink`]: connects, starts the reader, writes what
/// the link hands over until a write fails, or until the link is dropped
/// and the last frame it was handed is written; then it ends its half of
/// the connection.
fn run_tcp_link(
    connect_timeout: Duration,
    shared: &Arc<TcpLinkShared>,
    frames: &Receiver<Vec<u8>>,
    to_link: Sender<Vec<u8>>,
) {
    let stream = match TcpStream::connect_timeout(&shared.address, connect_timeout) {
        Ok(stream) => stream,
        Err(e) => {
            shared.fail(format!("no connection: {e}"));
            return;
        }
    };
    let _ = stream.set_nodelay(true);
    let (reader, kept) = match (stream.try_clone(), stream.try_clone()) {
        (Ok(reader), Ok(kept)) => (reader, kept),
        (Err(e), _) | (_, Err(e)) => {
            shared.fail(format!("the connection cannot be shared: {e}"));
            return;
        }
    };
    {
        let Ok(mut socket) = shared.socket.lock() else {
            return;
        };
        if socket.1 {
            let _ = stream.shutdown(Shutdown::Both);
            return; // the link was dropped while it connected
        }
        socket.0 = Some(kept);
    }
    let s = Arc::clone(shared);
    if let Ok(mut running) = shared.running.lock() {
        *running += 1;
    }
    if let Err(e) = std::thread::Builder::new()
        .name("xrce-upstream-tcp-read".to_string())
        .spawn(move || {
            let _counted = LinkThread(&s);
            read_tcp_link(reader, &s, &to_link);
        })
    {
        if let Ok(mut running) = shared.running.lock() {
            *running = running.saturating_sub(1);
        }
        let _ = stream.shutdown(Shutdown::Both);
        shared.fail(format!("no reader thread: {e}"));
        return;
    }
    let mut writer = stream;
    for frame in frames {
        if let Err(e) = writer.write_all(&frame) {
            shared.fail(format!("write: {e}"));
            let _ = writer.shutdown(Shutdown::Both);
            return;
        }
        shared.waiting.fetch_sub(frame.len(), Ordering::AcqRel);
    }
    // The link was dropped and what it was handed is written: the upstream
    // agent reads it and then the end of the connection.
    let _ = writer.shutdown(Shutdown::Write);
}

/// The reader thread of a [`TcpLink`]: one message per frame, until the
/// connection ends or the link is dropped.
fn read_tcp_link(mut stream: TcpStream, shared: &TcpLinkShared, to_link: &Sender<Vec<u8>>) {
    let mut len = [0u8; 2];
    loop {
        if let Err(e) = stream.read_exact(&mut len) {
            shared.fail(format!("read: {e}"));
            return;
        }
        let mut frame = vec![0u8; usize::from(u16::from_le_bytes(len))];
        if let Err(e) = stream.read_exact(&mut frame) {
            shared.fail(format!("read: {e}"));
            return;
        }
        if to_link.send(frame).is_err() {
            return; // the link was dropped
        }
    }
}

/// Opens the transport of a link; replaceable for tests.
pub type TransportFactory<T> = Box<dyn FnMut() -> std::io::Result<T> + Send>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkState {
    /// No session; a new attempt at `retry_at`.
    Down { retry_at: Duration },
    /// `CREATE_CLIENT` sent at `since`.
    Connecting { since: Duration },
    /// The session is open.
    Up,
    /// The upstream agent refused the session; a new attempt at `retry_at`.
    Refused {
        code: ResultStatusCode,
        retry_at: Duration,
    },
}

/// A request waiting for the upstream agent.
#[derive(Clone)]
struct Pending {
    stream_id: u8,
    submessage: Submessage,
    /// A request of the relay's own: a `CREATE` or `READ_DATA` sent again
    /// from the journal after a new session, or the cancel of a read after a
    /// `RESET`. It is not held as in flight, and a new session drops it: the
    /// journal sends its requests again, and the new session has no read to
    /// cancel.
    replay: bool,
}

/// A `CREATE` the upstream agent got, and whether its `STATUS` reached the
/// client.
struct Journaled {
    object: ObjectId,
    stream_id: u8,
    submessage: Submessage,
    answered: bool,
}

struct Link<T: ClientTransport> {
    representation: ClientRepresentation,
    /// The `session_id` of the session at the upstream agent: the client's
    /// once the first session is open; after a session was open, the other
    /// of the two ([`other_session_id`] of the client's), so the upstream
    /// agent replaces the session it may still hold (§7.8.2.1).
    session_id: u8,
    /// The `session_id` of the last session that was open at the upstream
    /// agent; `Some` means a new session replays the objects.
    held: Option<u8>,
    /// The first session of the link is being opened with the other
    /// `session_id`; once it is answered, a `CREATE_CLIENT` with the
    /// client's own replaces it, and with it whatever session the upstream
    /// agent held for the client before.
    replacing: bool,
    client: Option<XrceClient<T>>,
    state: LinkState,
    /// The `CREATE`s of the objects that exist upstream, in their order: a
    /// `CREATE` the upstream agent acknowledged or answered.
    journal: Vec<Journaled>,
    /// The running `READ_DATA` of each DataReader.
    reads: BTreeMap<u16, (u8, Submessage)>,
    /// The requests sent on reliable streams that the upstream agent has
    /// neither acknowledged nor answered, in the order they were sent; a new
    /// session sends them again.
    in_flight: VecDeque<(u8, Submessage)>,
    queue: VecDeque<Pending>,
    /// `(request_id, object)` of the requests the relay sent again; their
    /// `STATUS` is not the client's.
    replayed: BTreeSet<([u8; 2], u16)>,
    /// The `request_id`s of the reads of each DataReader that a `RESET`
    /// ended, the newest last, at most [`ENDED_READS_KEPT`]: a `DATA` of
    /// them is not the client's.
    ended_reads: BTreeMap<u16, VecDeque<[u8; 2]>>,
    /// Requests of the queue the upstream link cannot carry, to be answered.
    unsendable: Vec<(u8, Submessage, String)>,
    last_heard: Duration,
    last_keepalive: Duration,
}

/// The relay over XRCE sessions, one per routed client.
pub struct XrceUpstream<T: ClientTransport + Send + 'static = LinkTransport> {
    config: UpstreamConfig,
    factory: TransportFactory<T>,
    links: BTreeMap<ClientKeyOrd, Link<T>>,
}

impl XrceUpstream<LinkTransport> {
    /// A relay to the agent of `config.locator`.
    #[must_use]
    pub fn new(config: UpstreamConfig) -> Self {
        let (locator, connect_timeout) = (config.locator, config.link_timeout);
        Self::with_transport(
            config,
            Box::new(move || LinkTransport::open(locator, connect_timeout)),
        )
    }
}

/// The `request_id` and `ObjectId` of a request or reply submessage.
fn request_of(sm: &Submessage) -> Option<BaseObjectRequest> {
    BaseObjectRequest::decode(&sm.body).ok().map(|(b, _)| b)
}

impl<T: ClientTransport + Send + 'static> XrceUpstream<T> {
    /// A relay whose links open their transport with `factory`.
    #[must_use]
    pub fn with_transport(config: UpstreamConfig, factory: TransportFactory<T>) -> Self {
        Self {
            config,
            factory,
            links: BTreeMap::new(),
        }
    }

    /// The number of requests the link of `client` holds.
    #[must_use]
    pub fn queued(&self, client: ClientKeyOrd) -> usize {
        self.links.get(&client).map_or(0, |l| l.queue.len())
    }

    /// Whether the session of `client` at the upstream agent is open.
    #[must_use]
    pub fn is_up(&self, client: ClientKeyOrd) -> bool {
        self.links
            .get(&client)
            .is_some_and(|l| l.state == LinkState::Up)
    }

    fn connect(&mut self, client: ClientKeyOrd, now: Duration, events: &mut Vec<UpstreamEvent>) {
        let reconnect = self.config.reconnect;
        let mtu = self.config.mtu;
        let keepalive = self.config.keepalive;
        let Some(link) = self.links.get_mut(&client) else {
            return;
        };
        link.replayed.clear();
        // What the last session had neither acknowledged nor answered goes
        // first, then what waits; the journal's CREATEs and READ_DATAs are put
        // in front of both when the session is open.
        for (stream_id, submessage) in link.in_flight.drain(..).rev() {
            link.queue.push_front(Pending {
                stream_id,
                submessage,
                replay: false,
            });
        }
        link.queue.retain(|p| !p.replay);
        // Never the session_id of a session the upstream agent may hold: it
        // replaces a session it still holds, with its objects and cached
        // messages, only for another session_id (§7.8.2.1); for the same one
        // its streams would wait for the old sequence numbers and take the
        // new ones for duplicates. After a session was open, the other one
        // of the two. The first session of the link cannot know which of the
        // two the upstream agent holds (one an earlier process of this agent
        // left there, or one whose DELETE was lost): it goes first with the
        // other one and then with the client's (`replacing`).
        let own = link.representation.session_id;
        link.replacing = link.held.is_none();
        link.session_id = match link.held {
            Some(held) if held != own => own,
            _ => other_session_id(own),
        };
        if link.client.is_none() {
            match (self.factory)() {
                Ok(transport) => {
                    let mut c = XrceClient::new(ClientKey(client), link.session_id, transport);
                    c.set_properties(link.representation.properties.clone());
                    c.set_requested_mtu(mtu);
                    c.set_forward_replies(true);
                    // HEARTBEATs of waiting messages as often as keepalives:
                    // their ACKNACKs show the session is there.
                    c.set_heartbeat_period(keepalive);
                    link.client = Some(c);
                }
                Err(e) => {
                    link.state = LinkState::Down {
                        retry_at: now + reconnect,
                    };
                    events.push(UpstreamEvent::Lost {
                        client,
                        reason: format!("the upstream agent cannot be reached: {e}"),
                    });
                    return;
                }
            }
        }
        let Some(c) = link.client.as_mut() else {
            return;
        };
        if c.state() != ClientState::Disconnected {
            c.reset_session();
        }
        let _ = c.set_session_id(link.session_id);
        match c.connect(link.representation.version) {
            Ok(()) => link.state = LinkState::Connecting { since: now },
            Err(e) => {
                link.client = None;
                link.state = LinkState::Down {
                    retry_at: now + reconnect,
                };
                events.push(UpstreamEvent::Lost {
                    client,
                    reason: format!("CREATE_CLIENT not sent: {e:?}"),
                });
            }
        }
    }

    /// The session is open: the objects and reads again, before what waits.
    fn replay(link: &mut Link<T>) {
        if link.held.is_some() {
            let mut resend: Vec<Pending> = Vec::new();
            for entry in &link.journal {
                if entry.answered
                    && let Some(base) = request_of(&entry.submessage)
                {
                    link.replayed.insert((base.request_id, entry.object.raw()));
                }
                resend.push(Pending {
                    stream_id: entry.stream_id,
                    submessage: entry.submessage.clone(),
                    replay: true,
                });
            }
            resend.extend(link.reads.values().map(|(stream_id, sm)| Pending {
                stream_id: *stream_id,
                submessage: sm.clone(),
                replay: true,
            }));
            for item in resend.into_iter().rev() {
                link.queue.push_front(item);
            }
        }
        link.held = Some(link.session_id);
    }

    /// Sends what waits, in its order, as far as the upstream link takes it.
    fn flush(link: &mut Link<T>) {
        let Some(c) = link.client.as_mut() else {
            return;
        };
        while let Some(item) = link.queue.front().cloned() {
            match c.forward(item.stream_id, item.submessage.clone()) {
                Ok(()) => {
                    link.queue.pop_front();
                    Self::taken(
                        &mut link.in_flight,
                        &mut link.journal,
                        &mut link.reads,
                        &mut link.ended_reads,
                        item,
                    );
                }
                Err(e @ (ClientError::MtuTooSmall | ClientError::Protocol)) => {
                    link.queue.pop_front();
                    if !item.replay {
                        link.unsendable.push((
                            item.stream_id,
                            item.submessage,
                            unsendable_reason(e),
                        ));
                    }
                }
                // The window is full, a FRAGMENT run still goes out, the TCP
                // link holds its limit, or the transport failed: nothing of
                // it was sent; it waits.
                Err(_) => break,
            }
        }
    }

    /// The upstream link took `item`: a request on a reliable stream is in
    /// flight until the upstream agent acknowledges or answers it; one on
    /// another stream has no acknowledgement and counts as the upstream
    /// agent's now.
    fn taken(
        in_flight: &mut VecDeque<(u8, Submessage)>,
        journal: &mut Vec<Journaled>,
        reads: &mut BTreeMap<u16, (u8, Submessage)>,
        ended_reads: &mut BTreeMap<u16, VecDeque<[u8; 2]>>,
        item: Pending,
    ) {
        if item.replay {
            return;
        }
        if StreamId(item.stream_id).is_reliable() {
            in_flight.push_back((item.stream_id, item.submessage));
        } else {
            Self::settled(
                journal,
                reads,
                ended_reads,
                item.stream_id,
                &item.submessage,
                true,
            );
        }
    }

    /// Notes what a request the upstream agent has does to its objects;
    /// `success` is false for one it answered with an error.
    fn settled(
        journal: &mut Vec<Journaled>,
        reads: &mut BTreeMap<u16, (u8, Submessage)>,
        ended_reads: &mut BTreeMap<u16, VecDeque<[u8; 2]>>,
        stream_id: u8,
        sm: &Submessage,
        success: bool,
    ) {
        let Some(base) = request_of(sm) else {
            return;
        };
        if !success {
            return; // refused upstream: nothing to create or delete again
        }
        let oid = base.object_id;
        match sm.header.submessage_id {
            SubmessageId::Create => {
                let same_request = |e: &Journaled| {
                    e.object == oid
                        && request_of(&e.submessage)
                            .is_some_and(|b| b.request_id == base.request_id)
                };
                if journal.iter().any(same_request) {
                    return; // sent again after a new session
                }
                // An older entry of the object goes when the STATUS of this
                // one says it took effect (`answer_journal`).
                journal.push(Journaled {
                    object: oid,
                    stream_id,
                    submessage: sm.clone(),
                    answered: false,
                });
            }
            SubmessageId::Delete => {
                journal.retain(|e| e.object != oid);
                reads.remove(&oid.raw());
                ended_reads.remove(&oid.raw());
            }
            SubmessageId::ReadData => {
                reads.insert(oid.raw(), (stream_id, sm.clone()));
            }
            _ => {}
        }
    }

    /// The `STATUS` with `success` for the request `b` reached the client: a
    /// `CREATE` of the journal that took effect replaces the older entries of
    /// its object and is not answered again; one the upstream agent refused
    /// is not created again; a `READ_DATA` it refused is not read again.
    fn answer_journal(link: &mut Link<T>, b: BaseObjectRequest, success: bool) {
        let this = |e: &Journaled| {
            e.object == b.object_id
                && request_of(&e.submessage).is_some_and(|r| r.request_id == b.request_id)
        };
        if success {
            if link.journal.iter().any(this) {
                link.journal.retain(|e| e.object != b.object_id || this(e));
                for entry in link.journal.iter_mut().filter(|e| this(e)) {
                    entry.answered = true;
                }
            }
        } else {
            link.journal.retain(|e| !this(e));
            if link
                .reads
                .get(&b.object_id.raw())
                .and_then(|(_, sm)| request_of(sm))
                .is_some_and(|r| r.request_id == b.request_id)
            {
                link.reads.remove(&b.object_id.raw());
            }
        }
    }

    /// The request in flight that `matches`, the oldest first: the upstream
    /// agent has it now (`success` false: it answered with an error).
    fn settle_in_flight(
        link: &mut Link<T>,
        success: bool,
        matches: impl Fn(u8, &Submessage) -> bool,
    ) {
        if let Some(at) = link
            .in_flight
            .iter()
            .position(|(stream_id, sm)| matches(*stream_id, sm))
            && let Some((stream_id, sm)) = link.in_flight.remove(at)
        {
            Self::settled(
                &mut link.journal,
                &mut link.reads,
                &mut link.ended_reads,
                stream_id,
                &sm,
                success,
            );
        }
    }
}

/// Whether the upstream agent answers every `sm` (`CREATE` and `DELETE` with
/// a `STATUS`, `GET_INFO` with an `INFO`): such a request stays in flight
/// until its answer, so that a new session sends it again when the answer
/// was lost with the old one. `WRITE_DATA` (no `STATUS` on success,
/// §8.3.5.8) and `READ_DATA` (`DATA` only when there are samples) count as
/// the upstream agent's when it acknowledged them.
fn awaits_answer(sm: &Submessage) -> bool {
    matches!(
        sm.header.submessage_id,
        SubmessageId::Create | SubmessageId::Delete | SubmessageId::GetInfo
    )
}

/// The `request_id`s of ended reads a link keeps per DataReader; an older
/// one is forgotten, and a `DATA` of it that still arrived would reach the
/// client.
pub const ENDED_READS_KEPT: usize = 8;

/// The `READ_DATA` that cancels the read `read` started (§7.8.5.1 Table 9,
/// §8.3.5.9.2: `MAX_SAMPLES_ZERO` "shall be used to cancel the currently
/// active read operation without sending any more samples"): the same
/// `request_id`, DataReader, stream and format, in the byte order of `read`
/// and with its `DataDeliveryControl` written in the form `read` has (the
/// specification's, with its DHEADER, when `read` has none). `None` for a
/// `read` that cannot be decoded, which no agent started.
fn cancel_of(read: &Submessage) -> Option<Submessage> {
    use zerodds_xrce::submessages::read_data::{
        DataDeliveryControl, DeliveryControlForm, ReadDataPayload,
    };
    let payload = ReadDataPayload::try_from_submessage(read).ok()?;
    let e = read.header.body_endianness();
    let form = [DeliveryControlForm::Appendable, DeliveryControlForm::Bare]
        .into_iter()
        .find(|form| {
            payload
                .encode_body_with(e, *form)
                .is_ok_and(|body| body == read.body)
        })
        .unwrap_or(DeliveryControlForm::Appendable);
    let mut cancel = payload;
    cancel.read_specification.content_filter_expression = None;
    cancel.read_specification.delivery_control = Some(DataDeliveryControl {
        max_samples: zerodds_xrce::MAX_SAMPLES_ZERO,
        max_elapsed_time: 0,
        max_bytes_per_second: 0,
        min_pace_period: 0,
    });
    cancel.into_submessage_with(e, form).ok()
}

/// Whether `sm` is a `READ_DATA` with `max_samples` `MAX_SAMPLES_ZERO`.
fn is_cancel(sm: &Submessage) -> bool {
    zerodds_xrce::submessages::read_data::ReadDataPayload::try_from_submessage(sm).is_ok_and(|p| {
        p.read_specification
            .delivery_control
            .is_some_and(|d| d.max_samples == zerodds_xrce::MAX_SAMPLES_ZERO)
    })
}

/// Why the upstream link cannot carry a request.
fn unsendable_reason(e: ClientError) -> String {
    match e {
        ClientError::MtuTooSmall => {
            "the upstream mtu leaves no room for the FRAGMENTs it needs".to_string()
        }
        other => format!("the upstream link refused it: {other:?}"),
    }
}

/// The other of the two `session_id`s of an upstream session: in the same
/// half (with or without a `ClientKey` in the header, §8.3.2.1) and never a
/// `SESSION_ID_NONE_*`.
#[must_use]
pub fn other_session_id(session_id: u8) -> u8 {
    if session_id & 0x7F == 0x7F {
        session_id - 1
    } else {
        session_id + 1
    }
}

impl<T: ClientTransport + Send + 'static> Upstream for XrceUpstream<T> {
    fn routes(&self, client: ClientKeyOrd) -> bool {
        self.config.routes(client)
    }

    fn open(&mut self, client: ClientKeyOrd, representation: &ClientRepresentation, now: Duration) {
        if self.links.contains_key(&client) {
            return;
        }
        self.links.insert(
            client,
            Link {
                representation: representation.clone(),
                session_id: representation.session_id,
                held: None,
                replacing: false,
                client: None,
                state: LinkState::Down { retry_at: now },
                journal: Vec::new(),
                reads: BTreeMap::new(),
                in_flight: VecDeque::new(),
                queue: VecDeque::new(),
                replayed: BTreeSet::new(),
                ended_reads: BTreeMap::new(),
                unsendable: Vec::new(),
                last_heard: now,
                last_keepalive: now,
            },
        );
        let mut events = Vec::new();
        self.connect(client, now, &mut events);
        for e in events {
            if let UpstreamEvent::Lost { reason, .. } = e {
                eprintln!("[zerodds-xrce-agentd] upstream link of client {client:02x?}: {reason}");
            }
        }
    }

    fn close(&mut self, client: ClientKeyOrd) {
        if let Some(mut link) = self.links.remove(&client)
            && let Some(c) = link.client.as_mut()
        {
            c.disconnect();
        }
    }

    fn reset(&mut self, client: ClientKeyOrd) {
        let Some(link) = self.links.get_mut(&client) else {
            return;
        };
        // The reads the upstream agent runs or may have started, the newest
        // READ_DATA of each DataReader on each stream: the journal's, then
        // those in flight, in the order they were sent.
        let mut running: BTreeMap<(u8, u16), Submessage> = BTreeMap::new();
        for (reader, (stream_id, sm)) in core::mem::take(&mut link.reads) {
            running.insert((stream_id, reader), sm);
        }
        link.in_flight.retain(|(stream_id, sm)| {
            if sm.header.submessage_id != SubmessageId::ReadData {
                return true;
            }
            if let Some(b) = request_of(sm) {
                running.insert((*stream_id, b.object_id.raw()), sm.clone());
            }
            false
        });
        // A READ_DATA that has not gone out yet does not go, unless it
        // cancels a read.
        link.queue.retain(|p| {
            p.submessage.header.submessage_id != SubmessageId::ReadData || is_cancel(&p.submessage)
        });
        for ((_, reader), sm) in &running {
            if let Some(b) = request_of(sm) {
                let ended = link.ended_reads.entry(*reader).or_default();
                ended.retain(|r| *r != b.request_id);
                ended.push_back(b.request_id);
                while ended.len() > ENDED_READS_KEPT {
                    ended.pop_front();
                }
            }
        }
        // A session that is not open is replaced by the next one, which has
        // no reads; the open one gets the cancel of each read, on its stream
        // and behind what that stream carried before.
        if link.state != LinkState::Up {
            return;
        }
        for ((stream_id, _), sm) in running {
            if let Some(cancel) = cancel_of(&sm) {
                link.queue.push_back(Pending {
                    stream_id,
                    submessage: cancel,
                    replay: true,
                });
            }
        }
        Self::flush(link);
    }

    fn forward(
        &mut self,
        client: ClientKeyOrd,
        stream_id: u8,
        submessage: Submessage,
        _now: Duration,
    ) -> Result<(), UpstreamRefusal> {
        let max = self.config.max_queued_messages;
        let Some(link) = self.links.get_mut(&client) else {
            return Err(UpstreamRefusal::NoLink);
        };
        if let LinkState::Refused { code, .. } = link.state {
            return Err(UpstreamRefusal::Refused(code));
        }
        // A best-effort or STREAMID_NONE message goes whole (no FRAGMENT
        // there, §8.3.5.14): one the link cannot carry would fail at every
        // attempt and hold up the requests behind it.
        if !StreamId(stream_id).is_reliable() {
            let header = if link.session_id < 0x80 {
                MessageHeader::WIRE_SIZE_WITH_KEY
            } else {
                MessageHeader::WIRE_SIZE_NO_KEY
            };
            let size = header + submessage.wire_size_unpadded();
            let limit = self.config.locator.max_message_size();
            if size > limit {
                return Err(UpstreamRefusal::TooLarge { size, limit });
            }
        }
        // The client uses the request_id of a read a RESET ended again: what
        // answers it is the client's.
        if submessage.header.submessage_id == SubmessageId::ReadData
            && let Some(b) = request_of(&submessage)
            && let Some(ended) = link.ended_reads.get_mut(&b.object_id.raw())
        {
            ended.retain(|r| *r != b.request_id);
        }
        let item = Pending {
            stream_id,
            submessage,
            replay: false,
        };
        if link.state == LinkState::Up
            && link.queue.is_empty()
            && let Some(c) = link.client.as_mut()
        {
            match c.forward(stream_id, item.submessage.clone()) {
                Ok(()) => {
                    Self::taken(
                        &mut link.in_flight,
                        &mut link.journal,
                        &mut link.reads,
                        &mut link.ended_reads,
                        item,
                    );
                    return Ok(());
                }
                Err(ClientError::MtuTooSmall | ClientError::Protocol) => {
                    return Err(UpstreamRefusal::Unsendable);
                }
                // Nothing of it was sent; it waits.
                Err(_) => {}
            }
        }
        if link.queue.len() >= max {
            return Err(UpstreamRefusal::QueueFull);
        }
        link.queue.push_back(item);
        if link.state == LinkState::Up {
            Self::flush(link);
        }
        Ok(())
    }

    fn poll(
        &mut self,
        now: Duration,
        room: &mut dyn FnMut(ClientKeyOrd, u8) -> usize,
    ) -> Vec<UpstreamEvent> {
        let mut events = Vec::new();
        let clients: Vec<ClientKeyOrd> = self.links.keys().copied().collect();
        for client in clients {
            self.poll_link(client, now, room, &mut events);
        }
        events
    }

    fn release(
        &mut self,
        client: ClientKeyOrd,
        now: Duration,
        room: &mut dyn FnMut(ClientKeyOrd, u8) -> usize,
    ) -> Vec<UpstreamEvent> {
        let mut events = Vec::new();
        if self.links.contains_key(&client) {
            self.poll_link(client, now, room, &mut events);
        }
        events
    }
}

impl<T: ClientTransport + Send + 'static> XrceUpstream<T> {
    /// Polls the link of `client`: opens it again when it is due, reads what
    /// the upstream agent sent as far as `room` takes it (the answers the
    /// client has no room for stay unacknowledged in the link's reliable
    /// input stream, so the upstream agent's sender window fills and it
    /// stops taking samples for the client, §8.4.14), sends the keepalive.
    #[allow(clippy::too_many_lines)]
    fn poll_link(
        &mut self,
        client: ClientKeyOrd,
        now: Duration,
        room: &mut dyn FnMut(ClientKeyOrd, u8) -> usize,
        events: &mut Vec<UpstreamEvent>,
    ) {
        {
            let (keepalive, link_timeout, reconnect) = (
                self.config.keepalive,
                self.config.link_timeout,
                self.config.reconnect,
            );
            let state = self.links[&client].state;
            match state {
                LinkState::Down { retry_at } | LinkState::Refused { retry_at, .. }
                    if now >= retry_at =>
                {
                    self.connect(client, now, events);
                }
                LinkState::Connecting { since } if now.saturating_sub(since) > link_timeout => {
                    // A new transport: the connection may be dead without
                    // the kernel having noticed.
                    if let Some(link) = self.links.get_mut(&client) {
                        link.client = None;
                    }
                    self.connect(client, now, events);
                }
                _ => {}
            }
            let Some(link) = self.links.get_mut(&client) else {
                return;
            };
            let Some(c) = link.client.as_mut() else {
                return;
            };
            let heard_before = c.received_messages();
            let polled = c
                .poll_held(now, &mut |stream_id| room(client, stream_id))
                .and_then(|ev| c.tick(now).map(|()| ev));
            if c.received_messages() != heard_before {
                link.last_heard = now;
            }
            let polled = match polled {
                Ok(ev) => ev,
                Err(e) => {
                    link.client = None;
                    link.state = LinkState::Down {
                        retry_at: now + reconnect,
                    };
                    events.push(UpstreamEvent::Lost {
                        client,
                        reason: format!("the transport to the upstream agent failed: {e:?}"),
                    });
                    return;
                }
            };
            for ev in polled {
                match ev {
                    ClientEvent::Connected { .. } if link.replacing => {
                        // The upstream agent holds the session of the other
                        // session_id now, and nothing went on its streams:
                        // the CREATE_CLIENT with the client's own replaces
                        // it (§7.8.2.1).
                        link.replacing = false;
                        link.last_heard = now;
                        let (own, version) =
                            (link.representation.session_id, link.representation.version);
                        link.session_id = own;
                        let sent = match link.client.as_mut() {
                            Some(c) => {
                                c.reset_session();
                                let _ = c.set_session_id(own);
                                c.connect(version)
                                    .map_err(|e| format!("CREATE_CLIENT not sent: {e:?}"))
                            }
                            None => Err("the link has no transport".to_string()),
                        };
                        match sent {
                            Ok(()) => link.state = LinkState::Connecting { since: now },
                            Err(reason) => {
                                link.client = None;
                                link.state = LinkState::Down {
                                    retry_at: now + reconnect,
                                };
                                events.push(UpstreamEvent::Lost { client, reason });
                            }
                        }
                    }
                    ClientEvent::Connected { .. } => {
                        link.state = LinkState::Up;
                        link.last_heard = now;
                        link.last_keepalive = now;
                        Self::replay(link);
                        Self::flush(link);
                        events.push(UpstreamEvent::Connected { client });
                    }
                    ClientEvent::ConnectRejected { code } => {
                        link.state = LinkState::Refused {
                            code,
                            retry_at: now + reconnect,
                        };
                        events.push(UpstreamEvent::Refused {
                            client,
                            code,
                            pending: link
                                .queue
                                .drain(..)
                                .filter(|p| !p.replay)
                                .map(|p| (p.stream_id, p.submessage))
                                .collect(),
                        });
                    }
                    ClientEvent::Forwarded {
                        stream_id,
                        submessage,
                    } => {
                        link.last_heard = now;
                        let base = request_of(&submessage);
                        let id = submessage.header.submessage_id;
                        if id == SubmessageId::Data
                            && let Some(b) = base
                            && link
                                .ended_reads
                                .get(&b.object_id.raw())
                                .is_some_and(|ended| ended.contains(&b.request_id))
                        {
                            // A sample of a read a RESET of the client ended:
                            // the read sends "no more samples" (§8.3.5.13,
                            // §8.3.5.9.2).
                            continue;
                        }
                        if id == SubmessageId::Info
                            && base.is_some_and(|b| b.object_id == OBJECTID_AGENT)
                        {
                            continue; // the answer to the keepalive
                        }
                        if id == SubmessageId::Status
                            && let Some(b) = base
                            && link.replayed.remove(&(b.request_id, b.object_id.raw()))
                        {
                            continue; // the answer to a request sent again
                        }
                        // An answer shows the upstream agent has the request.
                        if let Some(b) = base {
                            let success = id != SubmessageId::Status
                                || zerodds_xrce::submessages::StatusPayload::try_from_submessage(
                                    &submessage,
                                )
                                .ok()
                                .and_then(|p| {
                                    BaseObjectReply::decode_with(
                                        &p.representation,
                                        submessage.header.body_endianness(),
                                    )
                                    .ok()
                                })
                                .is_some_and(|(r, _)| r.result.status_code.is_success());
                            Self::settle_in_flight(link, success, |_, sm| {
                                request_of(sm).is_some_and(|r| {
                                    r.request_id == b.request_id && r.object_id == b.object_id
                                })
                            });
                            if id == SubmessageId::Status {
                                Self::answer_journal(link, b, success);
                            }
                        }
                        events.push(UpstreamEvent::Reply {
                            client,
                            stream_id,
                            submessage,
                        });
                    }
                    ClientEvent::ForwardAcknowledged {
                        stream_id,
                        submessage,
                    } => {
                        if !awaits_answer(&submessage) {
                            Self::settle_in_flight(link, true, |s, sm| {
                                s == stream_id && *sm == submessage
                            });
                        }
                    }
                    ClientEvent::FragmentDiscarded { stream_id } => {
                        events.push(UpstreamEvent::FragmentDiscarded { client, stream_id });
                    }
                    _ => {}
                }
            }
            if link.state == LinkState::Up {
                Self::flush(link);
            }
            for (stream_id, submessage, reason) in link.unsendable.drain(..) {
                events.push(UpstreamEvent::Unsendable {
                    client,
                    stream_id,
                    submessage,
                    reason,
                });
            }
            if link.state == LinkState::Up {
                if now.saturating_sub(link.last_heard) > link_timeout {
                    events.push(UpstreamEvent::Lost {
                        client,
                        reason: format!(
                            "no message of the upstream agent for {} ms",
                            link_timeout.as_millis()
                        ),
                    });
                    link.client = None;
                    self.connect(client, now, events);
                } else if now.saturating_sub(link.last_keepalive) >= keepalive
                    && let Some(c) = link.client.as_mut()
                    && !c.holds_input()
                {
                    link.last_keepalive = now;
                    // On the reliable stream: an agent that does not hold the
                    // session (restarted, or replaced it) does not take its
                    // sequence numbers and does not answer. While the window
                    // has no room, the ACKNACKs to the HEARTBEATs of the
                    // waiting messages keep the link heard. While the link
                    // holds answers its client has no room for, no keepalive
                    // goes: its answer would wait at the upstream agent behind
                    // them, one more per keepalive, and the upstream agent's
                    // HEARTBEATs of the held messages keep the link heard.
                    let mask = InfoMask(InfoMask::INFO_ACTIVITY);
                    let _ = c.request_get_info(OBJECTID_AGENT, mask, true);
                }
            }
        }
    }
}

/// The answer to a request the relay could not take: a `STATUS` with
/// `code` for the request in `sm` (an `INFO` for a `GET_INFO` is built by the
/// engine). `None` for a submessage without a request.
#[must_use]
pub fn refusal_reply(sm: &Submessage, code: ResultStatusCode) -> Option<BaseObjectReply> {
    let base = request_of(sm)?;
    Some(BaseObjectReply {
        related_request: base,
        result: zerodds_xrce::object_info::ResultStatus {
            status_code: code,
            implementation_status: 0,
        },
    })
}

/// Whether a request of a routed client goes to the upstream agent: the
/// object operations, except those on the agent and on the client itself,
/// which this agent answers (§7.8.2).
#[must_use]
pub fn is_relayed(sm: &Submessage) -> bool {
    let object_kind = || request_of(sm).map(|b| b.object_id.kind());
    match sm.header.submessage_id {
        SubmessageId::Create | SubmessageId::WriteData | SubmessageId::ReadData => true,
        SubmessageId::Delete => !matches!(object_kind(), Some(Ok(ObjectKind::Client))),
        SubmessageId::GetInfo => !matches!(
            object_kind(),
            Some(Ok(ObjectKind::Client | ObjectKind::Agent))
        ),
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Instant;

    /// A loopback listener that accepts every connection and hands it over.
    fn listener() -> (SocketAddr, Receiver<TcpStream>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                if tx.send(stream).is_err() {
                    return;
                }
            }
        });
        (address, rx)
    }

    /// DDS-XRCE 1.0 §11.3.3: a message goes out with its 2-octet
    /// little-endian length, and a frame of the agent comes back as one
    /// message.
    #[test]
    fn xrce_11_3_3_a_tcp_link_frames_each_message_with_its_length() {
        let (address, accepted) = listener();
        let mut link = TcpLink::open(address, Duration::from_secs(2)).unwrap();
        link.send(&[1, 2, 3]).unwrap();
        let mut agent = accepted.recv_timeout(Duration::from_secs(2)).unwrap();
        let mut frame = [0u8; 5];
        agent.read_exact(&mut frame).unwrap();
        assert_eq!(frame, [3, 0, 1, 2, 3]);
        agent.write_all(&[2, 0, 9, 8, 1, 0, 7]).unwrap();
        let start = Instant::now();
        let mut got = Vec::new();
        while got.len() < 2 && start.elapsed() < Duration::from_secs(2) {
            match link.try_recv().unwrap() {
                Some(m) => got.push(m),
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        }
        assert_eq!(got, vec![vec![9, 8], vec![7]]);
    }

    /// DDS-XRCE 1.0 §10.4: an upstream agent that reads nothing makes no
    /// call of the link wait. Once the socket's buffers and
    /// [`TCP_LINK_SEND_LIMIT`] octets of the link hold what it did not read,
    /// `send` takes no more message (`QueueFull`, nothing sent) and the link
    /// has not failed. Dropping it returns at once and ends the connection
    /// within [`TCP_LINK_CLOSE_LIMIT`], although the writer still waits for
    /// the agent.
    #[test]
    fn xrce_10_4_a_tcp_link_to_an_agent_that_reads_nothing_never_blocks() {
        let (address, accepted) = listener();
        let mut link = TcpLink::open(address, Duration::from_secs(2)).unwrap();
        let mut agent = accepted.recv_timeout(Duration::from_secs(2)).unwrap();
        let message = vec![0x5A; 60_000];
        let start = Instant::now();
        let mut taken = 0usize;
        let mut slowest = Duration::ZERO;
        let refusal = loop {
            let call = Instant::now();
            let result = link.send(&message);
            slowest = slowest.max(call.elapsed());
            match result {
                Ok(()) => taken += message.len() + 2,
                Err(e) => break e,
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "{taken} octets taken"
            );
            // Time for the writer to hand what it holds to the socket.
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(refusal, ClientError::QueueFull);
        assert!(link.failure().is_none(), "{:?}", link.failure());
        assert!(
            taken > TCP_LINK_SEND_LIMIT,
            "the socket took a part too: {taken} octets"
        );
        assert!(slowest < Duration::from_millis(100), "{slowest:?}");
        let call = Instant::now();
        assert_eq!(link.try_recv(), Ok(None));
        assert!(call.elapsed() < Duration::from_millis(100));
        // Dropped, the link ends the connection: the agent, which reads
        // only after the limit, reads what the socket holds and then the
        // end of the stream.
        let call = Instant::now();
        drop(link);
        assert!(call.elapsed() < Duration::from_millis(100));
        std::thread::sleep(TCP_LINK_CLOSE_LIMIT + Duration::from_millis(500));
        agent
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut sink = Vec::new();
        agent.read_to_end(&mut sink).unwrap();
        assert!(sink.len() < taken, "{} of {taken} octets", sink.len());
    }

    /// DDS-XRCE 1.0 §10.4 with §7.8.2.3: a link dropped right after it was
    /// handed messages (the `DELETE` of a session that ended) writes them
    /// all before it ends the connection; dropping it does not wait.
    #[test]
    fn xrce_10_4_a_dropped_tcp_link_writes_what_it_was_handed_before_it_ends_the_connection() {
        let (address, accepted) = listener();
        let mut link = TcpLink::open(address, Duration::from_secs(2)).unwrap();
        let mut agent = accepted.recv_timeout(Duration::from_secs(2)).unwrap();
        // The writer is past the connect: the first message has arrived.
        link.send(&[7]).unwrap();
        let mut first = [0u8; 3];
        agent.read_exact(&mut first).unwrap();
        assert_eq!(first, [1, 0, 7]);
        let message = vec![0x3C; 1000];
        for _ in 0..200 {
            link.send(&message).unwrap();
        }
        let call = Instant::now();
        drop(link);
        assert!(call.elapsed() < Duration::from_millis(100));
        agent
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut sink = Vec::new();
        agent.read_to_end(&mut sink).unwrap();
        assert_eq!(sink.len(), 200 * 1002, "every message, then the end");
        assert!(
            sink.chunks(1002)
                .all(|frame| frame[..2] == [0xE8, 0x03] && frame[2..] == message[..]),
            "each message whole and framed"
        );
    }

    /// DDS-XRCE 1.0 §8.3.5.9.2 with §10.4: the cancel of a read is a
    /// READ_DATA with `max_samples` `MAX_SAMPLES_ZERO` for the same
    /// `request_id`, DataReader, stream and format, in the byte order of the
    /// read and with its `DataDeliveryControl` in the form the read has: with
    /// the DHEADER of the specification, or without it as the eProsima
    /// client writes it; with the DHEADER when the read has none.
    #[test]
    fn xrce_8_3_5_9_the_cancel_of_a_read_keeps_its_form() {
        use zerodds_xrce::encoding::Endianness;
        use zerodds_xrce::submessages::read_data::{
            DataDeliveryControl, DeliveryControlForm, ReadDataPayload, ReadSpecification,
        };
        use zerodds_xrce::submessages::write_data::DataFormat;
        let reader = ObjectId::new(0x002, ObjectKind::DataReader).unwrap();
        let read = |control: Option<DataDeliveryControl>| ReadDataPayload {
            base: BaseObjectRequest {
                request_id: [0x12, 0x34],
                object_id: reader,
            },
            read_specification: ReadSpecification {
                preferred_stream_id: 0x81,
                data_format: DataFormat::DataSeq,
                content_filter_expression: Some("x > 1".to_string()),
                delivery_control: control,
            },
        };
        let unlimited = Some(DataDeliveryControl {
            max_samples: zerodds_xrce::MAX_SAMPLES_UNLIMITED,
            max_elapsed_time: 0,
            max_bytes_per_second: 0,
            min_pace_period: 0,
        });
        let cancel_payload = ReadDataPayload {
            read_specification: ReadSpecification {
                content_filter_expression: None,
                delivery_control: Some(DataDeliveryControl {
                    max_samples: zerodds_xrce::MAX_SAMPLES_ZERO,
                    max_elapsed_time: 0,
                    max_bytes_per_second: 0,
                    min_pace_period: 0,
                }),
                ..read(None).read_specification
            },
            ..read(None)
        };
        for (control, e, form, expected) in [
            (
                unlimited,
                Endianness::Little,
                DeliveryControlForm::Appendable,
                DeliveryControlForm::Appendable,
            ),
            (
                unlimited,
                Endianness::Little,
                DeliveryControlForm::Bare,
                DeliveryControlForm::Bare,
            ),
            (
                unlimited,
                Endianness::Big,
                DeliveryControlForm::Bare,
                DeliveryControlForm::Bare,
            ),
            (
                None,
                Endianness::Big,
                DeliveryControlForm::Bare,
                DeliveryControlForm::Appendable,
            ),
        ] {
            let sm = read(control).into_submessage_with(e, form).unwrap();
            let cancel = cancel_of(&sm).unwrap();
            assert_eq!(
                cancel,
                cancel_payload
                    .clone()
                    .into_submessage_with(e, expected)
                    .unwrap(),
                "{control:?} {e:?} {form:?}"
            );
            assert!(is_cancel(&cancel));
            assert!(!is_cancel(&sm));
        }
    }

    /// A connection that cannot be made fails the link without making
    /// `open` wait.
    #[test]
    fn xrce_10_4_a_tcp_link_that_cannot_connect_fails_without_waiting() {
        let address = {
            let probe = TcpListener::bind("127.0.0.1:0").unwrap();
            probe.local_addr().unwrap()
        };
        let call = Instant::now();
        let mut link = TcpLink::open(address, Duration::from_secs(2)).unwrap();
        assert!(call.elapsed() < Duration::from_millis(100));
        let start = Instant::now();
        let result = loop {
            match link.try_recv() {
                Ok(None) if start.elapsed() < Duration::from_secs(3) => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                other => break other,
            }
        };
        assert_eq!(result, Err(ClientError::Transport));
        assert!(
            link.failure()
                .is_some_and(|f| f.starts_with("no connection")),
            "{:?}",
            link.failure()
        );
        assert_eq!(link.send(&[1]), Err(ClientError::Transport));
    }
}
