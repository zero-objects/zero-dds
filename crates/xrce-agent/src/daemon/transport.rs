// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! UDP/TCP/serial listeners driving a [`SessionEngine`].
//!
//! Each listener resolves "which client does this datagram belong to"
//! itself (spec §8.4.5: "If the `clientKey` is not present, the XRCE
//! Agent shall be able to derive the `ClientKey` [...] from the source
//! address of the message") and otherwise just moves bytes: decode →
//! `SessionEngine::handle_message` → encode → send. One background thread
//! ([`run_ticker`]) calls [`SessionEngine::tick`] on a fixed period for
//! `HEARTBEAT`/continuous-`READ_DATA` pumping and session-timeout
//! eviction, and sends its output to each client over the transport the
//! client registered on ([`Route`]).

#![allow(clippy::print_stderr)]

use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_xrce::header::MessageHeader;
use zerodds_xrce::submessages::CreateClientPayload;
use zerodds_xrce::{Message, Submessage, SubmessageId};

use crate::daemon::dds_port::DdsPort;
use crate::daemon::session::{ClientKeyOrd, Outbound, SessionEngine, SessionEvent};

/// Resolves an inbound datagram's `ClientKeyOrd` from its `MessageHeader`
/// and the peer address it arrived from, and remembers new bindings from
/// `SessionEvent::ClientRegistered`.
#[derive(Default)]
pub(crate) struct PeerTable {
    by_addr: HashMap<SocketAddr, ClientKeyOrd>,
    /// The client registered from an address on a serial line: the line
    /// and the `sourceAddress` of its frames (DDS-XRCE 1.0 Annex C.1.1.2).
    by_line: HashMap<(u64, u8), ClientKeyOrd>,
    /// The way to each registered client.
    routes: HashMap<ClientKeyOrd, Route>,
    /// How many messages of a peer named a ClientKey without a ProxyClient
    /// (DDS-XRCE 1.0 §7.8.1).
    no_client: HashMap<SocketAddr, u32>,
    /// The peers whose connection was closed at the threshold: a UDP
    /// datagram from one is not read.
    blocked: std::collections::HashSet<SocketAddr>,
    /// The count after which the connection of a peer is closed (§7.8.1: "once
    /// a certain threshold is crossed it shall close the connection"); 0
    /// never closes.
    no_client_threshold: u32,
}

/// Where the messages of the agent to one client go: the transport the
/// client registered on (DDS-XRCE 1.0 §11.2, §11.3, Annex C). A reply, a
/// HEARTBEAT and the samples of a running READ_DATA take the same way.
#[derive(Clone)]
pub(crate) enum Route {
    /// A UDP listener's socket and the client's address.
    Udp {
        /// The socket of the listener the client spoke to.
        socket: Arc<UdpSocket>,
        /// The client's address.
        addr: SocketAddr,
    },
    /// A TCP connection: the queue its writer drains.
    Stream {
        /// The connection, to drop its routes when it closes.
        connection: u64,
        /// The queue of its writer.
        tx: std::sync::mpsc::Sender<Message>,
    },
    /// A serial line: the queue its writer drains, and the client's address
    /// on the line, the `remoteAddress` of the frames to it (Annex C.1.1.3).
    /// Without the feature `serial` only the tests open a line.
    #[cfg_attr(not(feature = "serial"), allow(dead_code))]
    Serial {
        /// The line, to drop its routes when it closes.
        connection: u64,
        /// The client's address on the line.
        address: u8,
        /// The queue of the line's writer: the recipient's address and the
        /// message.
        tx: std::sync::mpsc::Sender<(u8, Message)>,
    },
}

impl Route {
    fn connection(&self) -> Option<u64> {
        match self {
            Self::Udp { .. } => None,
            Self::Stream { connection, .. } | Self::Serial { connection, .. } => Some(*connection),
        }
    }
}

/// The client a message names: the `clientKey` of its header, or the one a
/// CREATE_CLIENT without a header `clientKey` asks for in its payload, even
/// from an address another client used before.
fn requested_client(header: &MessageHeader, submessages: &[Submessage]) -> Option<ClientKeyOrd> {
    if let Some(ck) = header.client_key {
        return Some(ck.0);
    }
    submessages.iter().find_map(|sm| {
        if sm.header.submessage_id != SubmessageId::CreateClient {
            return None;
        }
        let payload = CreateClientPayload::try_from_submessage(sm).ok()?;
        Some(payload.representation.client_key.0)
    })
}

/// A new connection number for [`Route::Stream`] and [`Route::Serial`].
fn next_connection() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl PeerTable {
    /// Counts a message of `peer` to an unknown ClientKey; `true` when the
    /// threshold is crossed and the connection is to be closed.
    pub(crate) fn strike(&mut self, peer: SocketAddr) -> bool {
        let n = self.no_client.entry(peer).or_insert(0);
        *n += 1;
        let crossed = self.no_client_threshold != 0 && *n >= self.no_client_threshold;
        if crossed {
            self.blocked.insert(peer);
        }
        crossed
    }

    /// Whether the connection of `peer` was closed at the threshold.
    pub(crate) fn is_blocked(&self, peer: &SocketAddr) -> bool {
        self.blocked.contains(peer)
    }

    /// Resolves `header`'s client, peeking the `CREATE_CLIENT` payload
    /// directly when the header carries no `client_key` and the address
    /// is not yet bound (the very first datagram of a session).
    fn resolve(
        &self,
        header: &MessageHeader,
        peer: SocketAddr,
        submessages: &[Submessage],
    ) -> Option<ClientKeyOrd> {
        requested_client(header, submessages).or_else(|| self.by_addr.get(&peer).copied())
    }

    /// The client of a frame from `address` on serial line `connection`
    /// (Annex C.1.1.2): the one the message names, else the one registered
    /// from that address.
    #[cfg_attr(not(feature = "serial"), allow(dead_code))]
    fn resolve_on_line(
        &self,
        header: &MessageHeader,
        connection: u64,
        address: u8,
        submessages: &[Submessage],
    ) -> Option<ClientKeyOrd> {
        requested_client(header, submessages)
            .or_else(|| self.by_line.get(&(connection, address)).copied())
    }

    /// Binds `client` to the address on a serial line its CREATE_CLIENT
    /// came from; as for [`Self::bind`], a client has one.
    fn bind_line(&mut self, connection: u64, address: u8, client: ClientKeyOrd) {
        self.by_line
            .retain(|at, c| *c != client || *at == (connection, address));
        self.by_line.insert((connection, address), client);
    }

    /// Binds `client` to the address its CREATE_CLIENT came from. A client
    /// has one address: one that registers again from another address — a
    /// restarted process reusing its ClientKey, as every eProsima example
    /// does — is answered there, not at the address of its previous run.
    fn bind(&mut self, peer: SocketAddr, client: ClientKeyOrd) {
        self.by_addr.retain(|a, c| *c != client || *a == peer);
        self.by_addr.insert(peer, client);
    }

    /// Drops `client`'s address and route when its session ends.
    fn unbind(&mut self, client: ClientKeyOrd) {
        self.by_addr.retain(|_, c| *c != client);
        self.by_line.retain(|_, c| *c != client);
        self.routes.remove(&client);
    }

    /// The way to `client` from its registration on.
    fn set_route(&mut self, client: ClientKeyOrd, route: Route) {
        self.routes.insert(client, route);
    }

    fn route_of(&self, client: ClientKeyOrd) -> Option<Route> {
        self.routes.get(&client).cloned()
    }

    /// Drops the routes over `connection`, which closed; its writer then
    /// ends.
    fn close_connection(&mut self, connection: u64) {
        self.routes
            .retain(|_, r| r.connection() != Some(connection));
        self.by_line.retain(|(line, _), _| *line != connection);
    }

    fn addr_of(&self, client: ClientKeyOrd) -> Option<SocketAddr> {
        self.by_addr
            .iter()
            .find(|(_, c)| **c == client)
            .map(|(a, _)| *a)
    }
}

/// Handles the engine's events; a registered client gets `route`, and its
/// address `peer` where it has one.
fn apply_events<P: DdsPort>(
    engine: &mut SessionEngine<P>,
    peers: &Arc<Mutex<PeerTable>>,
    peer: Option<SocketAddr>,
    route: &Route,
) -> bool {
    let mut close = false;
    for ev in engine.drain_events() {
        match ev {
            SessionEvent::NoClient { client } => {
                if let (Some(peer), Ok(mut t)) = (peer, peers.lock()) {
                    if t.strike(peer) {
                        eprintln!(
                            "[zerodds-xrce-agentd] {peer}: too many messages to a ClientKey                              without a ProxyClient ({client:02x?}), connection closed"
                        );
                        close = true;
                    }
                }
            }
            SessionEvent::ClientRegistered { client, .. } => {
                if let Ok(mut t) = peers.lock() {
                    if let Some(peer) = peer {
                        t.bind(peer, client);
                    }
                    if let Route::Serial {
                        connection,
                        address,
                        ..
                    } = route
                    {
                        t.bind_line(*connection, *address, client);
                    }
                    t.set_route(client, route.clone());
                }
            }
            SessionEvent::ClientUnregistered { client } => {
                if let Ok(mut t) = peers.lock() {
                    t.unbind(client);
                }
            }
            SessionEvent::RequestUnreadable {
                client,
                submessage,
                body_len,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] client {client:02x?}: a {submessage:?} of {body_len} \
                     bytes cannot be read, answered STATUS_ERR_INVALID_DATA"
                );
            }
            SessionEvent::ClockSynchronised {
                client,
                offset_nanos,
                round_trip_nanos,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] client {client:02x?}: clock {offset_nanos} ns from the \
                     agent's, round trip {round_trip_nanos} ns"
                );
            }
            SessionEvent::UnknownTypeForwarding {
                client,
                topic,
                type_name,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] topic {:04x} (client {client:02x?}): type `{type_name}` \
                     unknown, forwarding samples opaque (no key tracking, no TypeInformation)",
                    topic.raw()
                );
            }
            SessionEvent::DdsPortFailed {
                client,
                object,
                error,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] DDS operation on {:04x} (client {client:02x?}) failed: {error}",
                    object.raw()
                );
            }
            SessionEvent::CreateRefused {
                client,
                object,
                reason,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] CREATE {:04x} (client {client:02x?}) refused: {reason}",
                    object.raw()
                );
            }
            SessionEvent::RepresentationNotApplied {
                client,
                object,
                elements,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] CREATE {:04x} (client {client:02x?}): created without \
                     {}",
                    object.raw(),
                    elements.join(", ")
                );
            }
            SessionEvent::FragmentsDiscarded { client, runs } => {
                eprintln!(
                    "[zerodds-xrce-agentd] client {client:02x?}: {runs} FRAGMENT run(s) incomplete \
                     past the reassembly TTL, discarded -- their messages are lost"
                );
            }
            SessionEvent::ReliableSendOverflowed {
                client,
                stream_id,
                reason,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] reliable stream {stream_id:02x} (client {client:02x?}): \
                     could not sequence an outbound message ({reason}) -- sent unsequenced"
                );
            }
            SessionEvent::SampleTooLarge {
                client,
                reader,
                len,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] READ_DATA {:04x} (client {client:02x?}): a sample of \
                     {len} bytes does not fit a DATA submessage -- not delivered, STATUS \
                     STATUS_ERR_RESOURCES sent",
                    reader.raw()
                );
            }
            SessionEvent::MessageRefused {
                client,
                session_id,
                stream_id,
                reason,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] client {client:02x?}: a message (session_id \
                     {session_id:#04x}, stream {stream_id:#04x}) is not processed: {reason}"
                );
            }
            SessionEvent::BestEffortMessageDiscarded {
                client,
                stream_id,
                sequence_nr,
                newest,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] client {client:02x?}: stream {stream_id:#04x}: \
                     message {sequence_nr} is not newer than {newest}, discarded"
                );
            }
            SessionEvent::OperationDenied {
                client,
                object,
                what,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] client {client:02x?}, object {:04x}: {what} -- \
                     STATUS_ERR_DENIED sent",
                    object.raw()
                );
            }
            SessionEvent::ReadEndedUndelivered {
                client,
                reader,
                samples,
            } => {
                eprintln!(
                    "[zerodds-xrce-agentd] READ_DATA {:04x} (client {client:02x?}): \
                     max_elapsed_time ended the read with {samples} sample(s) taken and not \
                     delivered",
                    reader.raw()
                );
            }
        }
    }
    close
}

fn send_outbound(socket: &UdpSocket, addr: SocketAddr, ob: &Outbound) {
    let msg = Message {
        header: ob.header,
        submessages: ob.submessages.clone(),
    };
    if let Ok(bytes) = msg.encode() {
        let _ = socket.send_to(&bytes, addr);
    }
}

/// Sends `out` to the clients by their routes. A message for a client
/// without a route, or whose stream writer is gone, is reported; the route
/// of a gone writer is removed.
pub(crate) fn deliver(peers: &Arc<Mutex<PeerTable>>, out: &[Outbound]) {
    for ob in out {
        let route = peers.lock().ok().and_then(|t| t.route_of(ob.client));
        let msg = || Message {
            header: ob.header,
            submessages: ob.submessages.clone(),
        };
        let queued = match route {
            Some(Route::Udp { socket, addr }) => {
                send_outbound(&socket, addr, ob);
                continue;
            }
            Some(Route::Stream { tx, .. }) => tx.send(msg()).is_ok(),
            Some(Route::Serial { address, tx, .. }) => tx.send((address, msg())).is_ok(),
            None => {
                eprintln!(
                    "[zerodds-xrce-agentd] client {:02x?}: no transport to it -- a message of \
                     the agent is not delivered",
                    ob.client
                );
                continue;
            }
        };
        if !queued {
            eprintln!(
                "[zerodds-xrce-agentd] client {:02x?}: its connection is closed -- a message of \
                 the agent is not delivered",
                ob.client
            );
            if let Ok(mut t) = peers.lock() {
                t.routes.remove(&ob.client);
            }
        }
    }
}

/// Runs the session engine's clock until `stop` is set: every 100 ms
/// `tick` makes the HEARTBEATs of the reliable streams, the samples of a
/// running READ_DATA and the FRAGMENTs that follow (DDS-XRCE 1.0 §8.4.14.1,
/// §8.3.5.9), and they go to each client over its own transport. Only the
/// UDP listener ticked, and it sent the output for a TCP client over UDP to
/// the TCP address; over TCP and serial none of it arrived. Blocking; call
/// from its own thread.
pub(crate) fn run_ticker<P: DdsPort + Send>(
    engine: Arc<Mutex<SessionEngine<P>>>,
    peers: Arc<Mutex<PeerTable>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    while !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
        let out = match engine.lock() {
            Ok(mut e) => e.tick(tick_now()),
            Err(_) => continue,
        };
        deliver(&peers, &out);
    }
}

/// Runs the UDP listener loop until `stop` is set. Blocking; call from its
/// own thread.
pub(crate) fn run_udp_listener<P: DdsPort + Send>(
    socket: UdpSocket,
    engine: Arc<Mutex<SessionEngine<P>>>,
    peers: Arc<Mutex<PeerTable>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    let _ = socket.set_read_timeout(Some(Duration::from_millis(200)));
    let socket = Arc::new(socket);
    let mut buf = vec![0u8; 65_507];
    while !stop.load(Ordering::Relaxed) {
        let (n, peer) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(_) => continue,
        };
        if peers.lock().is_ok_and(|t| t.is_blocked(&peer)) {
            continue; // the connection of this peer was closed (§7.8.1)
        }
        let Ok(msg) = Message::decode(&buf[..n]) else {
            // A corrupt/foreign UDP datagram on the agent port — logged,
            // not answered (there is no header to reply through).
            eprintln!(
                "[zerodds-xrce-agentd] udp: dropped {n} bytes from {peer} — not a valid XRCE message"
            );
            continue;
        };
        let resolved = {
            let Ok(t) = peers.lock() else { continue };
            t.resolve(&msg.header, peer, &msg.submessages)
        };
        let Ok(mut e) = engine.lock() else { continue };
        let now = tick_now();
        let out = e.handle_message(resolved, &msg.header, &msg.submessages, now);
        let route = Route::Udp {
            socket: Arc::clone(&socket),
            addr: peer,
        };
        apply_events(&mut e, &peers, Some(peer), &route);
        drop(e);
        for ob in &out {
            let addr = {
                let Ok(t) = peers.lock() else { continue };
                t.addr_of(ob.client).unwrap_or(peer)
            };
            send_outbound(&socket, addr, ob);
        }
    }
}

/// Runs one TCP connection's read/reply loop (spec §11.3: 2-byte
/// length-prefix framing, via `zerodds_xrce::transport_tcp::XrceTcpClient`
/// wrapping the accepted stream).
pub(crate) fn run_tcp_connection<P: DdsPort + Send>(
    stream: TcpStream,
    peer: SocketAddr,
    engine: Arc<Mutex<SessionEngine<P>>>,
    peers: Arc<Mutex<PeerTable>>,
) {
    // One writer per connection: the replies and what the ticker has for
    // this client go through its queue, in order (DDS-XRCE 1.0 §11.3.2: "The
    // TCP Server can send and receive messages to and from the Client").
    let (tx, rx) = std::sync::mpsc::channel::<Message>();
    let writer = match stream.try_clone() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[zerodds-xrce-agentd] tcp {peer}: no writer for the connection: {e}");
            return;
        }
    };
    let writer = thread::spawn(move || {
        let mut out = zerodds_xrce::transport_tcp::XrceTcpClient::from_stream(writer);
        for msg in rx {
            if out.send_message(&msg).is_err() {
                break;
            }
        }
    });
    let connection = next_connection();
    let route = Route::Stream {
        connection,
        tx: tx.clone(),
    };
    let mut client = zerodds_xrce::transport_tcp::XrceTcpClient::from_stream(stream);
    // `recv_message()` returning `Err` (connection closed or a malformed
    // frame) ends the loop.
    while let Ok(msg) = client.recv_message() {
        let resolved = {
            let Ok(t) = peers.lock() else { break };
            t.resolve(&msg.header, peer, &msg.submessages)
        };
        let Ok(mut e) = engine.lock() else { break };
        let now = tick_now();
        let out = e.handle_message(resolved, &msg.header, &msg.submessages, now);
        let close = apply_events(&mut e, &peers, Some(peer), &route);
        drop(e);
        if close {
            break; // §7.8.1: the threshold of messages to an unknown ClientKey
        }
        for ob in &out {
            let reply = Message {
                header: ob.header,
                submessages: ob.submessages.clone(),
            };
            if tx.send(reply).is_err() {
                break;
            }
        }
    }
    // The routes of this connection's clients hold senders too; with them
    // gone the writer ends.
    if let Ok(mut t) = peers.lock() {
        t.close_connection(connection);
    }
    drop(route);
    drop(tx);
    let _ = writer.join();
}

/// Accepts TCP connections and spawns [`run_tcp_connection`] for each.
/// Blocking; call from its own thread.
pub(crate) fn run_tcp_listener<P: DdsPort + Send + 'static>(
    listener: TcpListener,
    engine: Arc<Mutex<SessionEngine<P>>>,
    peers: Arc<Mutex<PeerTable>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    let _ = listener.set_nonblocking(true);
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, peer)) => {
                let _ = stream.set_nonblocking(false);
                let engine = Arc::clone(&engine);
                let peers = Arc::clone(&peers);
                thread::spawn(move || run_tcp_connection(stream, peer, engine, peers));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// Process-uptime clock the session engine's `now: Duration` is measured
/// against (relative time only — nothing here compares across restarts).
fn tick_now() -> Duration {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

/// Constructs a fresh, empty [`PeerTable`] guarded by a mutex, shared
/// across every listener thread the daemon starts.
#[cfg(test)]
#[must_use]
pub(crate) fn new_peer_table() -> Arc<Mutex<PeerTable>> {
    new_peer_table_with(DEFAULT_NO_CLIENT_THRESHOLD)
}

/// The count of messages to an unknown ClientKey after which a connection is
/// closed in the tests that name none.
#[cfg(test)]
pub(crate) const DEFAULT_NO_CLIENT_THRESHOLD: u32 = 16;

/// As [`new_peer_table`], with the threshold of §7.8.1.
#[must_use]
pub(crate) fn new_peer_table_with(no_client_threshold: u32) -> Arc<Mutex<PeerTable>> {
    Arc::new(Mutex::new(PeerTable {
        no_client_threshold,
        ..PeerTable::default()
    }))
}

/// A serial line of the agent (DDS-XRCE 1.0 Annex C): the agent's address
/// on it and the queue of its writer. Without the feature `serial` only the
/// tests open one.
#[cfg_attr(not(feature = "serial"), allow(dead_code))]
pub(crate) struct SerialLine {
    connection: u64,
    address: u8,
    tx: std::sync::mpsc::Sender<(u8, Message)>,
}

#[cfg_attr(not(feature = "serial"), allow(dead_code))]
impl SerialLine {
    /// A line on which the agent has `address`; what goes out is queued on
    /// `tx` with the recipient's address.
    pub(crate) fn new(address: u8, tx: std::sync::mpsc::Sender<(u8, Message)>) -> Self {
        Self {
            connection: next_connection(),
            address,
            tx,
        }
    }

    /// Takes a frame of the line. A frame whose `remoteAddress` is another
    /// station's is not for the agent (Annex C.1.1.3). The client is the one
    /// the message names, else the one registered from the frame's
    /// `sourceAddress`; its replies go back to that address. Every frame was
    /// taken as one client's, whatever its address, and the replies had no
    /// address.
    pub(crate) fn take_frame<P: DdsPort>(
        &self,
        engine: &Arc<Mutex<SessionEngine<P>>>,
        peers: &Arc<Mutex<PeerTable>>,
        frame: &zerodds_xrce::transport_serial::SerialFrame,
        msg: &Message,
    ) {
        if frame.remote != self.address {
            return;
        }
        let resolved = {
            let Ok(t) = peers.lock() else { return };
            t.resolve_on_line(&msg.header, self.connection, frame.source, &msg.submessages)
        };
        let Ok(mut e) = engine.lock() else { return };
        let out = e.handle_message(resolved, &msg.header, &msg.submessages, tick_now());
        let route = Route::Serial {
            connection: self.connection,
            address: frame.source,
            tx: self.tx.clone(),
        };
        apply_events(&mut e, peers, None, &route);
        drop(e);
        for ob in &out {
            let reply = Message {
                header: ob.header,
                submessages: ob.submessages.clone(),
            };
            if self.tx.send((frame.source, reply)).is_err() {
                eprintln!(
                    "[zerodds-xrce-agentd] serial: the writer of the line is gone -- a reply \
                     to address {} is not delivered",
                    frame.source
                );
            }
        }
    }

    /// Drops the routes and addresses of the line, which closed.
    pub(crate) fn close(&self, peers: &Arc<Mutex<PeerTable>>) {
        if let Ok(mut t) = peers.lock() {
            t.close_connection(self.connection);
        }
    }
}

#[cfg(any(feature = "tls", feature = "dtls"))]
pub mod secure;

#[cfg(feature = "serial")]
pub mod serial {
    //! Real serial-port I/O for the daemon (feature `serial`) — a
    //! [`serialport::SerialPort`] wrapped in
    //! `zerodds_xrce::transport_serial`'s Annex C framing.

    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use zerodds_xrce::Message;
    use zerodds_xrce::transport_serial::{FrameCheck, SerialFramer, encode_message_with};

    use crate::daemon::dds_port::DdsPort;
    use crate::daemon::session::SessionEngine;

    use super::{PeerTable, SerialLine};

    /// Opens `device` at `baud_rate` and runs the framed read/reply loop
    /// until `stop` is set. The agent has `address` on the line; the frames
    /// are checked with `frame_check`. What the ticker has for a client of
    /// the line (HEARTBEAT, the samples of a running READ_DATA) goes out
    /// between two reads.
    ///
    /// # Errors
    /// [`serialport::Error`] if the device cannot be opened, or a read fails
    /// other than by its timeout.
    pub(crate) fn run_serial_listener<P: DdsPort + Send>(
        device: &str,
        baud_rate: u32,
        address: u8,
        frame_check: FrameCheck,
        engine: Arc<Mutex<SessionEngine<P>>>,
        peers: Arc<Mutex<PeerTable>>,
        stop: Arc<AtomicBool>,
    ) -> Result<(), serialport::Error> {
        let mut port = serialport::new(device, baud_rate)
            .timeout(Duration::from_millis(200))
            .open()?;
        let mut framer = SerialFramer::with_frame_check(frame_check);
        let mut read_buf = [0u8; 256];
        let (tx, rx) = std::sync::mpsc::channel::<(u8, Message)>();
        let line = SerialLine::new(address, tx);
        let write_queued = |port: &mut Box<dyn serialport::SerialPort>| {
            while let Ok((remote, msg)) = rx.try_recv() {
                let written = encode_message_with(frame_check, &msg, address, remote)
                    .map_err(|e| e.to_string())
                    .and_then(|framed| port.write_all(&framed).map_err(|e| e.to_string()));
                if let Err(e) = written {
                    eprintln!(
                        "[zerodds-xrce-agentd] serial {device}: a message to address {remote} \
                         is not delivered: {e}"
                    );
                }
            }
        };
        let result = loop {
            if stop.load(Ordering::Relaxed) {
                break Ok(());
            }
            write_queued(&mut port);
            let n = match port.read(&mut read_buf) {
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(e) => break Err(serialport::Error::from(e)),
            };
            for result in framer.push_bytes(&read_buf[..n]) {
                match result {
                    Ok((frame, msg)) => line.take_frame(&engine, &peers, &frame, &msg),
                    Err(e) => {
                        eprintln!("[zerodds-xrce-agentd] serial {device}: frame dropped: {e}");
                    }
                }
            }
        };
        line.close(&peers);
        result
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::daemon::dds_port::NullDdsPort;
    use crate::daemon::handshake::ClientRepresentation;
    use crate::daemon::session::SessionEngineConfig;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use zerodds_xrce::header::{ClientKey, SessionId, StreamId};
    use zerodds_xrce::serial_number::SerialNumber16;
    use zerodds_xrce::submessages::FLAG_E_LITTLE_ENDIAN;

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    /// DDS-XRCE 1.0 §7.8.1: "The Agent shall maintain a counter on the number
    /// of times the STATUS_ERR_NOCLIENT was sent on an established
    /// connection, and once a certain threshold is crossed it shall close the
    /// connection." The agent drops such a message (StatusValue has no
    /// NOCLIENT) and counts it per peer; at the threshold the peer is blocked.
    #[test]
    fn messages_to_an_unknown_client_key_close_the_connection_at_the_threshold() {
        let peer: SocketAddr = "127.0.0.1:40000".parse().unwrap();
        let other: SocketAddr = "127.0.0.1:40001".parse().unwrap();
        let peers = new_peer_table_with(3);
        let mut t = peers.lock().unwrap();
        assert!(!t.strike(peer));
        assert!(!t.strike(peer));
        assert!(!t.is_blocked(&peer));
        assert!(t.strike(peer), "the third message crosses the threshold");
        assert!(t.is_blocked(&peer));
        assert!(!t.is_blocked(&other), "another connection is untouched");
        // A threshold of 0 never closes.
        drop(t);
        let never = new_peer_table_with(0);
        let mut t = never.lock().unwrap();
        for _ in 0..100 {
            assert!(!t.strike(peer));
        }
    }

    #[test]
    fn peer_table_resolves_bound_address() {
        let mut t = PeerTable::default();
        t.bind(addr(1), [1, 2, 3, 4]);
        let header = MessageHeader::without_client_key(
            SessionId(0x81),
            StreamId::BUILTIN_RELIABLE,
            SerialNumber16::new(0),
        )
        .unwrap();
        assert_eq!(t.resolve(&header, addr(1), &[]), Some([1, 2, 3, 4]));
        assert_eq!(t.resolve(&header, addr(2), &[]), None);
    }

    #[test]
    fn peer_table_resolves_header_client_key_directly() {
        let t = PeerTable::default();
        let header = MessageHeader::with_client_key(
            SessionId(0),
            StreamId::NONE,
            SerialNumber16::new(0),
            ClientKey([9, 9, 9, 9]),
        )
        .unwrap();
        assert_eq!(t.resolve(&header, addr(1), &[]), Some([9, 9, 9, 9]));
    }

    #[test]
    fn peer_table_bootstraps_from_create_client_payload() {
        let t = PeerTable::default();
        let header = MessageHeader::without_client_key(
            SessionId(0x80),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        let rep = ClientRepresentation {
            timestamp: None,
            version: crate::daemon::handshake::XRCE_VERSION,
            vendor_id: [0, 0],
            client_key: ClientKey([7, 7, 7, 7]),
            session_id: 0x81,
            properties: None,
            requested_mtu: None,
        };
        let sm = CreateClientPayload {
            representation: rep,
        }
        .into_submessage()
        .unwrap();
        assert_eq!(t.resolve(&header, addr(1), &[sm]), Some([7, 7, 7, 7]));
    }

    /// Every eProsima example process uses the same ClientKey; the second
    /// run registers from a new port. Replies must go to that port — with
    /// both addresses bound, `addr_of` picked either, and the second run's
    /// `uxr_create_session` saw no STATUS_AGENT ("Error at create
    /// session", measured on the Linux host).
    #[test]
    fn a_client_registering_again_is_answered_at_its_new_address() {
        let mut t = PeerTable::default();
        t.bind(addr(40_000), [0xAA, 0xAA, 0xBB, 0xBB]);
        t.bind(addr(30_000), [0xAA, 0xAA, 0xBB, 0xBB]);
        assert_eq!(t.addr_of([0xAA, 0xAA, 0xBB, 0xBB]), Some(addr(30_000)));
        assert_eq!(t.by_addr.len(), 1);
        t.unbind([0xAA, 0xAA, 0xBB, 0xBB]);
        assert!(t.by_addr.is_empty());
    }

    #[test]
    fn addr_of_finds_bound_client() {
        let mut t = PeerTable::default();
        t.bind(addr(5), [1; 4]);
        assert_eq!(t.addr_of([1; 4]), Some(addr(5)));
        assert_eq!(t.addr_of([2; 4]), None);
    }

    /// DDS-XRCE 1.0 §11.3.2: "The TCP Server can send and receive messages
    /// to and from the Client". A client registered over TCP gets what the
    /// ticker has for it (here one message handed to `deliver`) over its
    /// connection; when the connection closes its route goes. The ticker's
    /// output for a TCP client went over UDP to the TCP address.
    #[test]
    fn a_tcp_client_gets_the_messages_of_the_agent_over_its_connection() {
        let listener = TcpListener::bind(addr(0)).expect("bind tcp");
        let server_addr = listener.local_addr().unwrap();
        let engine = Arc::new(Mutex::new(SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig::default(),
        )));
        let peers = new_peer_table();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (engine_c, peers_c, stop_c) =
            (Arc::clone(&engine), Arc::clone(&peers), Arc::clone(&stop));
        let handle = thread::spawn(move || run_tcp_listener(listener, engine_c, peers_c, stop_c));

        let mut client =
            zerodds_xrce::transport_tcp::XrceTcpClient::connect(server_addr).expect("connect");
        // A read that gets no answer fails the test; it does not wait for ever.
        client
            .stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let rep = ClientRepresentation {
            timestamp: None,
            version: crate::daemon::handshake::XRCE_VERSION,
            vendor_id: [0, 0],
            client_key: ClientKey([4, 4, 4, 4]),
            session_id: 0x81,
            properties: None,
            requested_mtu: None,
        };
        let sm = CreateClientPayload {
            representation: rep,
        }
        .into_submessage()
        .unwrap();
        let header = MessageHeader::without_client_key(
            SessionId(0x80),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        client
            .send_message(&Message {
                header,
                submessages: vec![sm.clone()],
            })
            .unwrap();
        let reply = client.recv_message().expect("STATUS_AGENT");
        assert_eq!(
            reply.submessages[0].header.submessage_id,
            SubmessageId::StatusAgent
        );

        deliver(
            &peers,
            &[Outbound {
                client: [4, 4, 4, 4],
                header,
                submessages: vec![sm],
            }],
        );
        let unsolicited = client.recv_message().expect("the message of the agent");
        assert_eq!(
            unsolicited.submessages[0].header.submessage_id,
            SubmessageId::CreateClient
        );

        drop(client);
        let deadline = Instant::now() + Duration::from_secs(5);
        while peers.lock().unwrap().route_of([4, 4, 4, 4]).is_some() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            peers.lock().unwrap().route_of([4, 4, 4, 4]).is_none(),
            "the route of a closed connection goes"
        );
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        handle.join().unwrap();
    }

    fn create_client_from(key: [u8; 4]) -> Message {
        let rep = ClientRepresentation {
            timestamp: None,
            version: crate::daemon::handshake::XRCE_VERSION,
            vendor_id: [0, 0],
            client_key: ClientKey(key),
            session_id: 0x81,
            properties: None,
            requested_mtu: None,
        };
        let sm = CreateClientPayload {
            representation: rep,
        }
        .into_submessage()
        .unwrap();
        Message {
            header: MessageHeader::without_client_key(
                SessionId(0x80),
                StreamId::NONE,
                SerialNumber16::new(0),
            )
            .unwrap(),
            submessages: vec![sm],
        }
    }

    fn frame(source: u8, remote: u8) -> zerodds_xrce::transport_serial::SerialFrame {
        zerodds_xrce::transport_serial::SerialFrame {
            source,
            remote,
            payload: Vec::new(),
        }
    }

    /// DDS-XRCE 1.0 Annex C.1.1.2/.3: two clients on one line, at addresses
    /// 1 and 2. Each is answered at its own address; a message without a
    /// `clientKey` from address 2 is the client registered there; a frame
    /// to another station is not taken; the line's close drops both.
    #[test]
    fn the_clients_of_a_serial_line_are_told_apart_by_their_address() {
        let engine = Arc::new(Mutex::new(SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig::default(),
        )));
        let peers = new_peer_table();
        let (tx, rx) = std::sync::mpsc::channel();
        let line = SerialLine::new(0, tx);

        line.take_frame(&engine, &peers, &frame(1, 0), &create_client_from([1; 4]));
        line.take_frame(&engine, &peers, &frame(2, 0), &create_client_from([2; 4]));
        let replies: Vec<(u8, Message)> = rx.try_iter().collect();
        assert_eq!(
            replies
                .iter()
                .map(|(to, m)| (*to, m.submessages[0].header.submessage_id))
                .collect::<Vec<_>>(),
            vec![
                (1, SubmessageId::StatusAgent),
                (2, SubmessageId::StatusAgent)
            ]
        );

        let keyless = MessageHeader::without_client_key(
            SessionId(0x81),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        {
            let t = peers.lock().unwrap();
            assert_eq!(
                t.resolve_on_line(&keyless, line.connection, 2, &[]),
                Some([2; 4])
            );
            assert_eq!(
                t.resolve_on_line(&keyless, line.connection, 1, &[]),
                Some([1; 4])
            );
            assert_eq!(t.resolve_on_line(&keyless, line.connection, 3, &[]), None);
        }

        line.take_frame(&engine, &peers, &frame(3, 9), &create_client_from([3; 4]));
        assert!(
            rx.try_recv().is_err(),
            "a frame to address 9 is not the agent's"
        );

        deliver(
            &peers,
            &[Outbound {
                client: [2; 4],
                header: keyless,
                submessages: Vec::new(),
            }],
        );
        assert_eq!(rx.try_recv().map(|(to, _)| to), Ok(2));

        line.close(&peers);
        let t = peers.lock().unwrap();
        assert!(t.route_of([1; 4]).is_none() && t.route_of([2; 4]).is_none());
        assert_eq!(t.resolve_on_line(&keyless, line.connection, 2, &[]), None);
    }

    /// End-to-end over a real UDP loopback socket: a hand-built
    /// CREATE_CLIENT arrives, the listener answers STATUS_AGENT, and the
    /// peer table now resolves that address to the registered client.
    #[test]
    fn udp_listener_answers_create_client_over_loopback() {
        let server_socket = UdpSocket::bind(addr(0)).expect("bind server");
        let server_addr = server_socket.local_addr().unwrap();
        let engine = Arc::new(Mutex::new(SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig::default(),
        )));
        let peers = new_peer_table();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let engine_c = Arc::clone(&engine);
        let peers_c = Arc::clone(&peers);
        let stop_c = Arc::clone(&stop);
        let handle =
            thread::spawn(move || run_udp_listener(server_socket, engine_c, peers_c, stop_c));

        let client_socket = UdpSocket::bind(addr(0)).expect("bind client");
        client_socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let rep = ClientRepresentation {
            timestamp: None,
            version: crate::daemon::handshake::XRCE_VERSION,
            vendor_id: [0, 0],
            client_key: ClientKey([3, 3, 3, 3]),
            session_id: 0x81,
            properties: None,
            requested_mtu: None,
        };
        let sm = CreateClientPayload {
            representation: rep,
        }
        .into_submessage()
        .unwrap();
        let header = MessageHeader::without_client_key(
            SessionId(0x80),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        let msg = Message {
            header,
            submessages: vec![sm],
        };
        client_socket
            .send_to(&msg.encode().unwrap(), server_addr)
            .unwrap();

        let mut buf = [0u8; 4096];
        let (n, _) = client_socket
            .recv_from(&mut buf)
            .expect("recv STATUS_AGENT");
        let reply = Message::decode(&buf[..n]).expect("decode reply");
        assert_eq!(reply.submessages.len(), 1);
        assert_eq!(
            reply.submessages[0].header.submessage_id,
            SubmessageId::StatusAgent
        );
        assert!((reply.submessages[0].header.flags & FLAG_E_LITTLE_ENDIAN) != 0);

        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        handle.join().unwrap();
    }

    /// The GET_INFO of the agent, in a message without a session and a
    /// ClientKey, as a client sends it before CREATE_CLIENT.
    fn get_info_of_the_agent(object_id: zerodds_xrce::object_id::ObjectId) -> Message {
        use zerodds_xrce::encoding::Endianness;
        use zerodds_xrce::object_info::BaseObjectRequest;
        use zerodds_xrce::object_info::InfoMask;
        use zerodds_xrce::submessages::get_info::GetInfoPayload;
        let sm = GetInfoPayload::new(
            BaseObjectRequest {
                request_id: [0, 5],
                object_id,
            },
            InfoMask::ALL,
            Endianness::Little,
        )
        .into_submessage()
        .unwrap();
        Message {
            header: MessageHeader::without_client_key(
                SessionId(0x80),
                StreamId::NONE,
                SerialNumber16::new(0),
            )
            .unwrap(),
            submessages: vec![sm],
        }
    }

    /// Reads the INFO a listener answered GET_INFO with: status OK, the
    /// request_id and object_id of the request echoed.
    fn assert_info_ok(reply: &Message, object_id: zerodds_xrce::object_id::ObjectId) {
        use zerodds_xrce::object_info::ResultStatusCode;
        use zerodds_xrce::submessages::info::InfoPayload;
        assert_eq!(reply.submessages.len(), 1);
        let sm = &reply.submessages[0];
        assert_eq!(sm.header.submessage_id, SubmessageId::Info);
        let (reply, _) = InfoPayload::try_from_submessage(sm)
            .unwrap()
            .parse(sm.header.body_endianness())
            .unwrap();
        assert_eq!(reply.result.status_code, ResultStatusCode::Ok);
        assert_eq!(reply.related_request.request_id, [0, 5]);
        assert_eq!(reply.related_request.object_id, object_id);
    }

    /// DDS-XRCE 1.0 §7.8.2.2: get_info "may be used prior to establishing a
    /// Session"; §8.3.5.3: the ObjectKind in the low bits of the object_id
    /// selects the Root, not the whole id 0xFFFD. Over a real UDP socket,
    /// without a CREATE_CLIENT before.
    #[test]
    fn the_udp_listener_answers_get_info_before_a_session() {
        use zerodds_xrce::object_id::{OBJECTID_AGENT, ObjectId};
        use zerodds_xrce::object_kind::ObjectKind;
        let server_socket = UdpSocket::bind(addr(0)).expect("bind server");
        let server_addr = server_socket.local_addr().unwrap();
        let engine = Arc::new(Mutex::new(SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig::default(),
        )));
        let peers = new_peer_table();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (engine_c, peers_c, stop_c) =
            (Arc::clone(&engine), Arc::clone(&peers), Arc::clone(&stop));
        let handle =
            thread::spawn(move || run_udp_listener(server_socket, engine_c, peers_c, stop_c));
        let client_socket = UdpSocket::bind(addr(0)).expect("bind client");
        client_socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        for object_id in [
            OBJECTID_AGENT,
            ObjectId::new(0x010, ObjectKind::Agent).unwrap(),
        ] {
            client_socket
                .send_to(
                    &get_info_of_the_agent(object_id).encode().unwrap(),
                    server_addr,
                )
                .unwrap();
            let mut buf = [0u8; 4096];
            let (n, _) = client_socket.recv_from(&mut buf).expect("recv INFO");
            assert_info_ok(&Message::decode(&buf[..n]).unwrap(), object_id);
        }
        assert_eq!(engine.lock().unwrap().client_count(), 0);
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        handle.join().unwrap();
    }

    /// The same over TCP (§11.3).
    #[test]
    fn the_tcp_listener_answers_get_info_before_a_session() {
        use zerodds_xrce::object_id::OBJECTID_AGENT;
        let listener = TcpListener::bind(addr(0)).expect("bind tcp");
        let server_addr = listener.local_addr().unwrap();
        let engine = Arc::new(Mutex::new(SessionEngine::new(
            NullDdsPort::new(),
            SessionEngineConfig::default(),
        )));
        let peers = new_peer_table();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (engine_c, peers_c, stop_c) =
            (Arc::clone(&engine), Arc::clone(&peers), Arc::clone(&stop));
        let handle = thread::spawn(move || run_tcp_listener(listener, engine_c, peers_c, stop_c));
        let mut client =
            zerodds_xrce::transport_tcp::XrceTcpClient::connect(server_addr).expect("connect");
        // A read that gets no answer fails the test; it does not wait for ever.
        client
            .stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        client
            .send_message(&get_info_of_the_agent(OBJECTID_AGENT))
            .unwrap();
        let reply = client.recv_message().expect("INFO");
        assert_info_ok(&reply, OBJECTID_AGENT);
        assert_eq!(engine.lock().unwrap().client_count(), 0);
        drop(client);
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        handle.join().unwrap();
    }
}
