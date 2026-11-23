// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Standalone MQTT broker server — the conformance target *MQTT Server* of
//! MQTT 5.0 §7.1.1 (chapters 3, 4 and 6), MQTT 3.1.1 clients beside it.
//!
//! One accept loop per listener gives each Network Connection a reader
//! thread (the protocol, `conn`) and a writer thread (the sole socket
//! writer). The listeners are a [`Transport`] each: TCP always, and with
//! the features `websocket` and `tls` the WebSocket of §6
//! ([`MqttBrokerServer::with_websocket`]), TLS
//! ([`MqttBrokerServer::with_tls`]) and the WebSocket on TLS
//! ([`MqttBrokerServer::with_secure_websocket`]); the handshakes of TLS
//! and WebSocket run on the connection's thread. The session state lives
//! in the [`crate::broker::Broker`] engine behind one lock; a housekeeping
//! thread publishes delayed Wills, ends expired sessions and deletes
//! expired messages.
//!
//! What a server decides itself is in [`ServerConfig`]: Receive Maximum,
//! Maximum Packet Size, Topic Alias Maximum, Maximum QoS, the available
//! features, Server Keep Alive, the longest Session Expiry Interval,
//! Response Information, redirection (§4.11) and the [`Authenticator`] for
//! simple and enhanced authentication (§4.12). A transport of the caller
//! hands its connections to [`ServerHandle::serve`] through
//! [`ClientStream`]. `std`-only.

mod conn;
pub mod inbound;
#[cfg(feature = "tls")]
mod tls;
#[cfg(feature = "websocket")]
mod websocket;

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::string::String;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::vec::Vec;

use crate::broker::{Broker, QoS};
use crate::net::MAX_MQTT_PACKET_SIZE;
use crate::property_set::PropertySet;
use crate::version::ProtocolVersion;

pub use conn::Outbound;
#[cfg(feature = "websocket")]
pub use websocket::CLOSE_WAIT;

/// The byte stream of one client connection (§4.2: ordered, lossless,
/// both directions).
pub trait ClientStream: Read + Send + 'static {
    /// A second handle the writer thread writes through.
    ///
    /// # Errors
    /// The transport cannot hand out one.
    fn writer(&self) -> io::Result<Box<dyn ClientWriter>>;

    /// How long one blocking read may wait; `None` waits without limit.
    ///
    /// # Errors
    /// The transport refuses the timeout.
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()>;
}

/// The writing side of a [`ClientStream`].
pub trait ClientWriter: Write + Send {
    /// Closes the Network Connection in both directions.
    fn close(&mut self);

    /// Ends the server's direction of the Network Connection; the client
    /// can still send until it closes. A transport above it, such as the
    /// WebSocket of §6, reads the client's last frames with it. The
    /// default closes both directions.
    fn close_write(&mut self) {
        self.close();
    }
}

impl ClientStream for TcpStream {
    fn writer(&self) -> io::Result<Box<dyn ClientWriter>> {
        Ok(Box::new(self.try_clone()?))
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }
}

impl ClientWriter for TcpStream {
    fn close(&mut self) {
        // The peer may have closed already; there is nothing left to tell it.
        let _ = self.shutdown(Shutdown::Both);
    }

    fn close_write(&mut self) {
        // As in `close`.
        let _ = self.shutdown(Shutdown::Write);
    }
}

/// One step of an enhanced authentication exchange (§4.12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStep {
    /// Send AUTH 0x18 Continue authentication with this Authentication
    /// Data and wait for the client's answer.
    Continue(Vec<u8>),
    /// Authenticated; the CONNACK (or the AUTH 0x00 of a
    /// re-authentication) carries this Authentication Data.
    Success(Option<Vec<u8>>),
    /// Refused with this Reason Code (0x87 Not authorized, 0x8C Bad
    /// authentication method, …).
    Failure(u8),
}

/// One enhanced authentication exchange, from the CONNECT (or the AUTH
/// 0x19 of a re-authentication) to its end.
pub trait AuthExchange: Send {
    /// The client's Authentication Data (absent in a CONNECT without it).
    fn step(&mut self, data: Option<&[u8]>) -> AuthStep;
}

/// Authentication of the server (§4.12, §5.4.1).
pub trait Authenticator: Send + Sync {
    /// Simple authentication: the User Name and Password of a CONNECT
    /// without an Authentication Method. `Err` carries the Reason Code of
    /// the CONNACK (0x86 Bad User Name or Password, 0x87 Not authorized).
    ///
    /// # Errors
    /// The Reason Code that refuses the connection.
    fn authenticate(
        &self,
        client_id: &str,
        user_name: Option<&str>,
        password: Option<&[u8]>,
    ) -> Result<(), u8>;

    /// A new exchange for the Authentication Method `method`, `None` when
    /// the method is not supported ([MQTT-4.12.0-1]).
    fn start(&self, client_id: &str, method: &str) -> Option<Box<dyn AuthExchange>>;
}

/// Server redirection (§4.11): a CONNACK with Reason Code 0x9C Use another
/// server or 0x9D Server moved and the Server Reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    /// `true`: 0x9D Server moved; `false`: 0x9C Use another server.
    pub permanent: bool,
    /// The Server Reference (§4.11).
    pub server_reference: String,
}

/// What the server decides itself.
#[derive(Clone)]
pub struct ServerConfig {
    /// Receive Maximum (§3.2.2.3.3): QoS 2 messages of a client held at
    /// once before 0x93 Receive Maximum exceeded.
    pub receive_maximum: u16,
    /// Maximum Packet Size (§3.2.2.3.6) the server accepts, whole packet.
    pub maximum_packet_size: u32,
    /// Topic Alias Maximum (§3.2.2.3.8) the server accepts from a client.
    pub topic_alias_maximum: u16,
    /// Maximum QoS (§3.2.2.3.4) of PUBLISH packets the server accepts and
    /// grants.
    pub maximum_qos: QoS,
    /// Retain Available (§3.2.2.3.5).
    pub retain_available: bool,
    /// Wildcard Subscription Available (§3.2.2.3.11).
    pub wildcard_subscription_available: bool,
    /// Subscription Identifiers Available (§3.2.2.3.12).
    pub subscription_identifiers_available: bool,
    /// Shared Subscription Available (§3.2.2.3.13).
    pub shared_subscription_available: bool,
    /// Server Keep Alive (§3.2.2.3.14): replaces the Keep Alive of an
    /// MQTT 5.0 client, which gets it in the CONNACK; a 3.1.1 client keeps
    /// its own.
    pub server_keep_alive: Option<u16>,
    /// The longest Session Expiry Interval granted; a longer request is
    /// answered with this value in the CONNACK (§3.2.2.3.2).
    pub maximum_session_expiry: Option<u32>,
    /// Response Information (§3.2.2.3.15) for a client that requests it.
    pub response_information: Option<String>,
    /// Redirect every client (§4.11).
    pub redirect: Option<Redirect>,
    /// Assign a ClientID to a client that sends an empty one
    /// ([MQTT-3.1.3-6]); `false` refuses it with 0x85.
    pub allow_empty_client_id: bool,
    /// The most messages a session holds pending transmission; `None` (the
    /// default) without limit. At the limit a QoS 0 message is not kept
    /// for the session (§4.1 makes that state optional); a QoS 1 or QoS 2
    /// message ends the session (§4.1.1): its connection gets DISCONNECT
    /// 0x97 Quota exceeded, then the session state is discarded and its
    /// Will published. Both count in [`ServerHandle::queue_overflows`].
    pub max_queued_messages: Option<usize>,
    /// Use Topic Aliases towards a client that announced a Topic Alias
    /// Maximum (§3.3.2.3.4).
    pub outbound_topic_aliases: bool,
    /// How long the server waits for a CONNECT (and each AUTH of the
    /// exchange before the CONNACK).
    pub connect_timeout: Duration,
    /// Authentication; `None` accepts every client and supports no
    /// Authentication Method.
    pub authenticator: Option<Arc<dyn Authenticator>>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            receive_maximum: u16::MAX,
            maximum_packet_size: MAX_MQTT_PACKET_SIZE,
            topic_alias_maximum: 64,
            maximum_qos: QoS::ExactlyOnce,
            retain_available: true,
            wildcard_subscription_available: true,
            subscription_identifiers_available: true,
            shared_subscription_available: true,
            server_keep_alive: None,
            maximum_session_expiry: None,
            response_information: None,
            redirect: None,
            allow_empty_client_id: true,
            max_queued_messages: None,
            outbound_topic_aliases: true,
            connect_timeout: Duration::from_secs(10),
            authenticator: None,
        }
    }
}

impl core::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("receive_maximum", &self.receive_maximum)
            .field("maximum_packet_size", &self.maximum_packet_size)
            .field("topic_alias_maximum", &self.topic_alias_maximum)
            .field("maximum_qos", &self.maximum_qos)
            .field("authenticator", &self.authenticator.is_some())
            .finish_non_exhaustive()
    }
}

/// An open Network Connection of a session, as the routing sees it.
pub(crate) struct ConnEntry {
    /// The connection's number (a takeover replaces the entry).
    pub(crate) id: u64,
    pub(crate) tx: std::sync::mpsc::Sender<Outbound>,
    pub(crate) version: ProtocolVersion,
    /// The client's Receive Maximum.
    pub(crate) receive_maximum: u16,
    /// The client's Maximum Packet Size.
    pub(crate) maximum_packet_size: u32,
    /// The client's Topic Alias Maximum.
    pub(crate) topic_alias_maximum: u16,
    /// Topic Aliases the server assigned on this connection.
    pub(crate) aliases_out: BTreeMap<String, u16>,
}

pub(crate) struct State {
    pub(crate) broker: Broker,
    pub(crate) conns: BTreeMap<String, ConnEntry>,
    pub(crate) shutdown: bool,
}

pub(crate) struct Shared {
    pub(crate) config: ServerConfig,
    state: Mutex<State>,
    wake: Condvar,
    epoch: Instant,
    next_conn: AtomicU64,
    next_assigned: AtomicU64,
    /// Connections whose TLS or WebSocket handshake failed.
    handshake_failures: AtomicU64,
}

impl Shared {
    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Milliseconds since the server started: the engine's clock.
    pub(crate) fn now_ms(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub(crate) fn next_conn_id(&self) -> u64 {
        self.next_conn.fetch_add(1, Ordering::Relaxed)
    }

    /// A ClientID no session uses ([MQTT-3.2.2-16]).
    pub(crate) fn assign_client_id(&self, st: &State) -> String {
        loop {
            let n = self.next_assigned.fetch_add(1, Ordering::Relaxed);
            let id = std::format!(
                "zerodds-{:x}-{n}",
                self.epoch.elapsed().as_nanos() & 0xFFFF_FFFF
            );
            if !st.broker.has_session(&id) && !st.conns.contains_key(&id) {
                return id;
            }
        }
    }

    pub(crate) fn notify(&self) {
        self.wake.notify_all();
    }
}

/// The transport of a listener of the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// MQTT on TCP (§4.2).
    Tcp,
    /// MQTT on TLS (§4.2, `mqtts`).
    Tls,
    /// MQTT on WebSocket (§6, `ws`).
    WebSocket,
    /// MQTT on WebSocket on TLS (§6, `wss`).
    SecureWebSocket,
}

/// How the connections of a listener are taken: the handshakes before the
/// MQTT protocol.
#[derive(Clone)]
enum Accept {
    Tcp,
    #[cfg(feature = "tls")]
    Tls(Arc<rustls::ServerConfig>),
    #[cfg(feature = "websocket")]
    WebSocket,
    #[cfg(all(feature = "tls", feature = "websocket"))]
    SecureWebSocket(Arc<rustls::ServerConfig>),
}

impl Accept {
    fn transport(&self) -> Transport {
        match self {
            Self::Tcp => Transport::Tcp,
            #[cfg(feature = "tls")]
            Self::Tls(_) => Transport::Tls,
            #[cfg(feature = "websocket")]
            Self::WebSocket => Transport::WebSocket,
            #[cfg(all(feature = "tls", feature = "websocket"))]
            Self::SecureWebSocket(_) => Transport::SecureWebSocket,
        }
    }

    /// Takes the handshakes of the transport on `stream`, then serves the
    /// MQTT protocol on it. A failed handshake has been answered — a TLS
    /// alert, an HTTP error response — and counts in
    /// [`ServerHandle::handshake_failures`].
    fn serve(&self, shared: &Shared, stream: TcpStream) {
        let served: io::Result<()> = match self {
            Self::Tcp => {
                conn::serve(shared, stream);
                Ok(())
            }
            #[cfg(feature = "tls")]
            Self::Tls(cfg) => tls::accept(Arc::clone(cfg), stream, shared.config.connect_timeout)
                .map(|s| conn::serve(shared, s)),
            #[cfg(feature = "websocket")]
            Self::WebSocket => websocket::accept(stream, shared.config.connect_timeout)
                .map(|s| conn::serve(shared, s)),
            #[cfg(all(feature = "tls", feature = "websocket"))]
            Self::SecureWebSocket(cfg) => {
                let timeout = shared.config.connect_timeout;
                tls::accept(Arc::clone(cfg), stream, timeout)
                    .and_then(|s| websocket::accept(s, timeout))
                    .map(|s| conn::serve(shared, s))
            }
        };
        if served.is_err() {
            shared.handshake_failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// A running MQTT broker server.
pub struct MqttBrokerServer {
    /// The TCP listener first, then those of the other transports.
    listeners: Vec<(Accept, TcpListener)>,
    shared: Arc<Shared>,
}

/// Handle to a server running on background threads.
pub struct ServerHandle {
    /// The address of each listener, the TCP one first.
    addrs: Vec<(Transport, std::net::SocketAddr)>,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    accept: Vec<JoinHandle<()>>,
    housekeeping: Option<JoinHandle<()>>,
}

impl ServerHandle {
    /// The bound local address of the TCP listener (useful when binding to
    /// port 0).
    #[must_use]
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.addrs[0].1
    }

    /// The bound local address of the listener of `transport`, `None`
    /// without one.
    #[must_use]
    pub fn transport_addr(&self, transport: Transport) -> Option<std::net::SocketAddr> {
        self.addrs
            .iter()
            .find(|(t, _)| *t == transport)
            .map(|(_, a)| *a)
    }

    /// Connections whose TLS or WebSocket handshake failed: a TLS
    /// handshake error, an invalid opening handshake, or one that does not
    /// offer the subprotocol `mqtt` (§6).
    #[must_use]
    pub fn handshake_failures(&self) -> u64 {
        self.shared.handshake_failures.load(Ordering::Relaxed)
    }

    /// Serves a connection that came in over another transport.
    pub fn serve<S: ClientStream>(&self, stream: S) {
        let shared = Arc::clone(&self.shared);
        thread::spawn(move || conn::serve(&shared, stream));
    }

    /// Deliveries that found a session's queue full
    /// ([`ServerConfig::max_queued_messages`]): QoS 0 messages not kept and
    /// QoS 1 or QoS 2 messages that ended their session.
    #[must_use]
    pub fn queue_overflows(&self) -> u64 {
        self.shared.lock().broker.queue_overflows()
    }

    /// Redirects every connected client (§4.11): MQTT 5.0 clients get a
    /// DISCONNECT with 0x9C or 0x9D and the Server Reference; every
    /// connection is closed.
    pub fn redirect_clients(&self, redirect: &Redirect) {
        let props = PropertySet {
            server_reference: Some(redirect.server_reference.clone()),
            ..PropertySet::default()
        };
        let code = if redirect.permanent { 0x9D } else { 0x9C };
        let st = self.shared.lock();
        for entry in st.conns.values() {
            conn::send_disconnect(entry, code, &props);
        }
    }

    /// Stops accepting, tells every MQTT 5.0 client 0x8B Server shutting
    /// down, closes every connection and waits for the server threads. A
    /// connection accepted before whose CONNECT comes after it gets CONNACK
    /// 0x88 Server unavailable (MQTT 3.1.1: Return Code 0x03) and is closed.
    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        {
            let mut st = self.shared.lock();
            st.shutdown = true;
            for entry in st.conns.values() {
                conn::send_disconnect(entry, 0x8B, &PropertySet::default());
            }
        }
        self.shared.notify();
        // Unblock each blocking accept() with a throwaway connection.
        for (_, addr) in &self.addrs {
            let _ = TcpStream::connect(addr);
        }
        for j in self.accept.drain(..).chain(self.housekeeping.take()) {
            let _ = j.join();
        }
    }
}

impl MqttBrokerServer {
    /// Binds the broker to `addr` (use `127.0.0.1:0` for an ephemeral port)
    /// with the default [`ServerConfig`].
    ///
    /// # Errors
    /// I/O error binding the listener.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Self::bind_with(addr, ServerConfig::default())
    }

    /// Binds the broker to `addr` with `config`.
    ///
    /// # Errors
    /// I/O error binding the listener.
    pub fn bind_with<A: ToSocketAddrs>(addr: A, config: ServerConfig) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let mut broker = Broker::new();
        broker.set_max_queued(config.max_queued_messages);
        Ok(Self {
            listeners: vec![(Accept::Tcp, listener)],
            shared: Arc::new(Shared {
                config,
                state: Mutex::new(State {
                    broker,
                    conns: BTreeMap::new(),
                    shutdown: false,
                }),
                wake: Condvar::new(),
                epoch: Instant::now(),
                next_conn: AtomicU64::new(1),
                next_assigned: AtomicU64::new(1),
                handshake_failures: AtomicU64::new(0),
            }),
        })
    }

    #[cfg(any(feature = "websocket", feature = "tls"))]
    fn listen<A: ToSocketAddrs>(mut self, addr: A, accept: Accept) -> io::Result<Self> {
        self.listeners.push((accept, TcpListener::bind(addr)?));
        Ok(self)
    }

    /// Adds a listener on `addr` for MQTT on WebSocket (§6, `ws`): the
    /// opening handshake of RFC 6455 selects the subprotocol `mqtt`, the
    /// control packets travel in binary frames.
    ///
    /// # Errors
    /// I/O error binding the listener.
    #[cfg(feature = "websocket")]
    pub fn with_websocket<A: ToSocketAddrs>(self, addr: A) -> io::Result<Self> {
        self.listen(addr, Accept::WebSocket)
    }

    /// Adds a listener on `addr` for MQTT on TLS (`mqtts`) with the
    /// certificate, key, ALPN and client verification of `tls`.
    ///
    /// # Errors
    /// I/O error binding the listener.
    #[cfg(feature = "tls")]
    pub fn with_tls<A: ToSocketAddrs>(
        self,
        addr: A,
        tls: Arc<rustls::ServerConfig>,
    ) -> io::Result<Self> {
        self.listen(addr, Accept::Tls(tls))
    }

    /// Adds a listener on `addr` for MQTT on WebSocket on TLS (§6, `wss`):
    /// the TLS handshake with `tls`, then the one of
    /// [`Self::with_websocket`]. A WebSocket client that offers ALPN offers
    /// `http/1.1` (RFC 6455 §4.1 is HTTP/1.1); `tls` names it in its ALPN
    /// protocols or none.
    ///
    /// # Errors
    /// I/O error binding the listener.
    #[cfg(all(feature = "tls", feature = "websocket"))]
    pub fn with_secure_websocket<A: ToSocketAddrs>(
        self,
        addr: A,
        tls: Arc<rustls::ServerConfig>,
    ) -> io::Result<Self> {
        self.listen(addr, Accept::SecureWebSocket(tls))
    }

    /// The bound local address of the TCP listener.
    ///
    /// # Errors
    /// I/O error querying the socket.
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listeners[0].1.local_addr()
    }

    /// Spawns one accept loop per listener and the housekeeping on
    /// background threads, returning a handle that can shut them down.
    ///
    /// # Errors
    /// I/O error querying a listener.
    pub fn spawn(self) -> io::Result<ServerHandle> {
        let stop = Arc::new(AtomicBool::new(false));
        let mut addrs = Vec::with_capacity(self.listeners.len());
        for (accept, listener) in &self.listeners {
            addrs.push((accept.transport(), listener.local_addr()?));
        }
        let accept = self
            .listeners
            .into_iter()
            .map(|(accept, listener)| {
                let shared = Arc::clone(&self.shared);
                let flag = Arc::clone(&stop);
                thread::spawn(move || accept_loop(&listener, &accept, &shared, &flag))
            })
            .collect();
        let hk_shared = Arc::clone(&self.shared);
        let housekeeping = thread::spawn(move || housekeeping(&hk_shared));
        Ok(ServerHandle {
            addrs,
            shared: self.shared,
            stop,
            accept,
            housekeeping: Some(housekeeping),
        })
    }
}

/// Accepts the connections of `listener` until `stop`; each gets a thread
/// of its own for the handshakes and the protocol.
fn accept_loop(listener: &TcpListener, accept: &Accept, shared: &Arc<Shared>, stop: &AtomicBool) {
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        // Latency over throughput for the small control packets.
        let _ = stream.set_nodelay(true);
        let shared = Arc::clone(shared);
        let accept = accept.clone();
        thread::spawn(move || accept.serve(&shared, stream));
    }
}

/// Publishes delayed Wills, ends expired sessions and deletes expired
/// messages when they are due ([`Broker::tick`]).
fn housekeeping(shared: &Shared) {
    let mut st = shared.lock();
    loop {
        if st.shutdown {
            return;
        }
        let now = shared.now_ms();
        let envs = st.broker.tick(now);
        conn::route(&mut st, envs, now, &shared.config);
        let wait = st
            .broker
            .next_deadline()
            .map_or(1000, |d| d.saturating_sub(now))
            .clamp(1, 1000);
        st = shared
            .wake
            .wait_timeout(st, Duration::from_millis(wait))
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
}

impl ServerHandle {
    /// The ClientIDs with an open Network Connection.
    #[must_use]
    pub fn connected_clients(&self) -> Vec<String> {
        self.shared.lock().conns.keys().cloned().collect()
    }

    /// The protocol of the open Network Connection of `client_id`, as its
    /// CONNECT named it (protocol level 5 or 4); `None` without one.
    #[must_use]
    pub fn protocol_version_of(&self, client_id: &str) -> Option<ProtocolVersion> {
        self.shared.lock().conns.get(client_id).map(|c| c.version)
    }

    /// Whether the server holds a session for `client_id`.
    #[must_use]
    pub fn has_session(&self, client_id: &str) -> bool {
        self.shared.lock().broker.has_session(client_id)
    }
}
