// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DTLS over UDP, blocking, one thread per session.
//!
//! The server reads its socket on one thread and hands each datagram to the
//! session of its source; a datagram from a new source starts a handshake
//! (at most [`MAX_HANDSHAKES`] at a time, each within
//! [`HANDSHAKE_TIMEOUT`]). A session reads its datagrams from a channel and
//! writes to the socket; [`Waker::wake`] makes a waiting
//! [`Session::recv`] return, so that a thread can send what others queued.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use openssl::ssl::{ErrorCode, Ssl, SslContext, SslStream};

use crate::config::{ClientConfig, ConfigError, ServerConfig, peer_index};
use crate::ffi;

/// The most handshakes a server runs at a time; further new sources are
/// ignored until one ends.
pub const MAX_HANDSHAKES: usize = 64;
/// How long a handshake may take.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// The DTLS MTU: the IPv6 minimum of 1280 octets less the IPv6 and UDP
/// headers (RFC 7252 §4.6; RFC 6347 §4.1.1.1).
pub const MTU: u32 = 1232;
/// The datagrams queued for one session before further ones are dropped.
const QUEUE: usize = 256;
/// How often a waiting read looks again (OpenSSL's retransmission timer
/// runs when the read returns).
const POLL: Duration = Duration::from_millis(50);

/// Who the peer proved to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerIdentity {
    /// A pre-shared key, its identity.
    Psk(Vec<u8>),
    /// A certificate, its leaf in DER.
    Certificate(Vec<u8>),
    /// A raw public key (RFC 7250), its SubjectPublicKeyInfo in DER.
    RawPublicKey(Vec<u8>),
}

enum Event {
    Datagram(Vec<u8>),
    Wake,
    /// The peer's address refused a datagram (ICMP port unreachable).
    Refused,
}

/// Makes a [`Session::recv`] waiting in another thread return.
#[derive(Clone)]
pub struct Waker(WakeTo);

#[derive(Clone)]
enum WakeTo {
    Server(SyncSender<Event>),
    Client(Sender<Event>),
}

impl Waker {
    /// Wakes the session; `false` once it is gone.
    pub fn wake(&self) -> bool {
        match &self.0 {
            WakeTo::Server(s) => {
                !matches!(s.try_send(Event::Wake), Err(TrySendError::Disconnected(_)))
            }
            WakeTo::Client(s) => s.send(Event::Wake).is_ok(),
        }
    }
}

/// The datagram I/O under OpenSSL: a read is one datagram from the
/// channel, a write one datagram to the peer.
struct Datagrams {
    events: Receiver<Event>,
    socket: Arc<UdpSocket>,
    peer: SocketAddr,
    connected: bool,
    wait: Duration,
    /// Set by a [`Waker`], taken by [`Session::recv`].
    woken: bool,
}

impl Read for Datagrams {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.events.recv_timeout(self.wait) {
            Ok(Event::Datagram(d)) => {
                let n = d.len().min(buf.len());
                buf[..n].copy_from_slice(&d[..n]);
                Ok(n)
            }
            Ok(Event::Wake) => {
                self.woken = true;
                Err(io::Error::from(io::ErrorKind::WouldBlock))
            }
            Err(RecvTimeoutError::Timeout) => Err(io::Error::from(io::ErrorKind::WouldBlock)),
            Ok(Event::Refused) => Err(io::Error::from(io::ErrorKind::ConnectionRefused)),
            Err(RecvTimeoutError::Disconnected) => Ok(0),
        }
    }
}

impl Write for Datagrams {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.connected {
            self.socket.send(buf)
        } else {
            self.socket.send_to(buf, self.peer)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// An established DTLS session.
pub struct Session {
    stream: SslStream<Datagrams>,
    peer: SocketAddr,
    identity: Option<PeerIdentity>,
    waker: Waker,
    /// Removes the session from the server's table when it ends.
    _entry: Option<Entry>,
    /// Ends the client's reading thread when the session ends.
    _reader: Option<Reader>,
}

struct Entry {
    table: Arc<Mutex<HashMap<SocketAddr, SyncSender<Event>>>>,
    peer: SocketAddr,
}

impl Drop for Entry {
    fn drop(&mut self) {
        if let Ok(mut t) = self.table.lock() {
            t.remove(&self.peer);
        }
    }
}

struct Reader {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn would_block(e: &openssl::ssl::Error) -> bool {
    matches!(e.code(), ErrorCode::WANT_READ | ErrorCode::WANT_WRITE)
}

fn to_io(e: openssl::ssl::Error) -> io::Error {
    if e.code() == ErrorCode::ZERO_RETURN {
        return io::Error::from(io::ErrorKind::ConnectionAborted);
    }
    match e.into_io_error() {
        Ok(io) => io,
        Err(e) => io::Error::other(e.to_string()),
    }
}

fn identity_of(ssl: &openssl::ssl::SslRef, server_side: bool) -> Option<PeerIdentity> {
    let (client_type, server_type) = ffi::negotiated_cert_types(ssl);
    let peer_type = if server_side {
        client_type
    } else {
        server_type
    };
    if peer_type == ffi::CERT_TYPE_RPK
        && let Some(spki) = ffi::peer_rpk_spki(ssl)
    {
        return Some(PeerIdentity::RawPublicKey(spki));
    }
    if let Some(der) = ssl.peer_certificate().and_then(|c| c.to_der().ok()) {
        return Some(PeerIdentity::Certificate(der));
    }
    ssl.psk_identity().map(|id| PeerIdentity::Psk(id.to_vec()))
}

/// Drives a handshake to its end or the deadline.
fn handshake(
    stream: &mut SslStream<Datagrams>,
    server_side: bool,
    deadline: Instant,
) -> io::Result<()> {
    loop {
        let step = if server_side {
            stream.accept()
        } else {
            stream.connect()
        };
        match step {
            Ok(()) => return Ok(()),
            Err(e) if would_block(&e) => {
                if Instant::now() >= deadline {
                    return Err(io::Error::from(io::ErrorKind::TimedOut));
                }
            }
            Err(e) => return Err(to_io(e)),
        }
    }
}

impl Session {
    /// The peer's address.
    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    /// Who the peer proved to be in the handshake.
    #[must_use]
    pub fn peer_identity(&self) -> Option<&PeerIdentity> {
        self.identity.as_ref()
    }

    /// The cipher suite of the session, OpenSSL's name.
    #[must_use]
    pub fn cipher(&self) -> Option<&'static str> {
        self.stream.ssl().current_cipher().map(|c| c.name())
    }

    /// The handle that interrupts a waiting [`Self::recv`].
    #[must_use]
    pub fn waker(&self) -> Waker {
        self.waker.clone()
    }

    /// Sends one record of application data.
    ///
    /// # Errors
    /// The session is closed or the socket refuses the datagram.
    pub fn send(&mut self, data: &[u8]) -> io::Result<()> {
        // A write does not block on a datagram socket; WANT_READ comes only
        // while OpenSSL waits for a handshake message, a bounded wait.
        for _ in 0..100 {
            match self.stream.ssl_write(data) {
                Ok(_) => return Ok(()),
                Err(e) if would_block(&e) => {}
                Err(e) => return Err(to_io(e)),
            }
        }
        Err(io::Error::from(io::ErrorKind::TimedOut))
    }

    /// The next record of application data, or `None` when none came
    /// within `timeout` or the [`Waker`] was used.
    ///
    /// # Errors
    /// `ConnectionAborted` once the peer closed the session, another error
    /// when the session broke.
    pub fn recv(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let deadline = Instant::now() + timeout;
        let mut buf = vec![0u8; 65_536];
        loop {
            self.stream.get_mut().wait = deadline
                .saturating_duration_since(Instant::now())
                .min(POLL)
                .max(Duration::from_millis(1));
            match self.stream.ssl_read(&mut buf) {
                Ok(0) => return Err(io::Error::from(io::ErrorKind::ConnectionAborted)),
                Ok(n) => {
                    buf.truncate(n);
                    return Ok(Some(buf));
                }
                Err(e) if would_block(&e) => {
                    let woken = std::mem::take(&mut self.stream.get_mut().woken);
                    if woken || Instant::now() >= deadline {
                        return Ok(None);
                    }
                }
                Err(e) => return Err(to_io(e)),
            }
        }
    }

    /// Sends close_notify (RFC 5246 §7.2.1).
    pub fn close(&mut self) {
        let _ = self.stream.shutdown();
    }
}

/// A DTLS server on one UDP socket.
pub struct Server {
    socket: Arc<UdpSocket>,
    accepted: Mutex<Receiver<Session>>,
    context: Arc<RwLock<SslContext>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    failed: Arc<AtomicU64>,
}

impl Server {
    /// Binds `addr` and serves with `config`.
    ///
    /// # Errors
    /// The configuration makes no context, or the socket cannot be bound.
    pub fn bind(addr: SocketAddr, config: &ServerConfig) -> Result<Self, ConfigError> {
        let context = Arc::new(RwLock::new(config.context()?));
        let socket = Arc::new(UdpSocket::bind(addr).map_err(ConfigError::Io)?);
        let _ = socket.set_read_timeout(Some(Duration::from_millis(200)));
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel();
        let thread = {
            let (socket, context, stop, failed) = (
                Arc::clone(&socket),
                Arc::clone(&context),
                Arc::clone(&stop),
                Arc::clone(&failed),
            );
            std::thread::Builder::new()
                .name("dtls-recv".into())
                .spawn(move || receive(&socket, &context, &stop, &tx, &failed))
                .ok()
        };
        Ok(Self {
            socket,
            accepted: Mutex::new(rx),
            context,
            stop,
            thread,
            failed,
        })
    }

    /// The bound address.
    ///
    /// # Errors
    /// As `UdpSocket::local_addr`.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// How many handshakes failed since the bind: a peer that did not
    /// authenticate, offered no suite in common, or did not finish in time.
    #[must_use]
    pub fn failed_handshakes(&self) -> u64 {
        self.failed.load(Ordering::Relaxed)
    }

    /// The next session whose handshake completed, waiting up to
    /// `timeout`.
    #[must_use]
    pub fn accept(&self, timeout: Duration) -> Option<Session> {
        self.accepted.lock().ok()?.recv_timeout(timeout).ok()
    }

    /// Takes `config` for the handshakes from now on; established sessions
    /// keep their keys.
    ///
    /// # Errors
    /// The configuration makes no context; the previous one stays.
    pub fn rotate(&self, config: &ServerConfig) -> Result<(), ConfigError> {
        let fresh = config.context()?;
        if let Ok(mut c) = self.context.write() {
            *c = fresh;
        }
        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The server's reading thread: datagrams to their sessions, a new source
/// to a handshake.
fn receive(
    socket: &Arc<UdpSocket>,
    context: &Arc<RwLock<SslContext>>,
    stop: &Arc<AtomicBool>,
    accepted: &Sender<Session>,
    failed: &Arc<AtomicU64>,
) {
    let table: Arc<Mutex<HashMap<SocketAddr, SyncSender<Event>>>> = Arc::default();
    let handshakes = Arc::new(AtomicUsize::new(0));
    let mut buf = vec![0u8; 65_536];
    while !stop.load(Ordering::SeqCst) {
        let Ok((n, peer)) = socket.recv_from(&mut buf) else {
            continue;
        };
        let datagram = buf[..n].to_vec();
        let known = table.lock().ok().and_then(|t| t.get(&peer).cloned());
        if let Some(tx) = known {
            // A full queue drops the datagram, as the network may.
            let _ = tx.try_send(Event::Datagram(datagram));
            continue;
        }
        // A new source: only a ClientHello record (content type 22,
        // epoch 0) starts a handshake (RFC 6347 §4.1).
        if datagram.len() < 13 || datagram[0] != 22 || datagram[3..5] != [0, 0] {
            continue;
        }
        if handshakes.load(Ordering::SeqCst) >= MAX_HANDSHAKES {
            continue;
        }
        let Ok(ctx) = context.read().map(|c| c.clone()) else {
            continue;
        };
        let (tx, rx) = mpsc::sync_channel(QUEUE);
        let _ = tx.try_send(Event::Datagram(datagram));
        if let Ok(mut t) = table.lock() {
            t.insert(peer, tx.clone());
        }
        let entry = Entry {
            table: Arc::clone(&table),
            peer,
        };
        handshakes.fetch_add(1, Ordering::SeqCst);
        let (socket, running, accepted, failed) = (
            Arc::clone(socket),
            Arc::clone(&handshakes),
            accepted.clone(),
            Arc::clone(failed),
        );
        let spawned = std::thread::Builder::new()
            .name("dtls-session".into())
            .spawn(move || {
                let result = start_server_session(&ctx, socket, peer, rx, tx);
                running.fetch_sub(1, Ordering::SeqCst);
                match result {
                    Ok(mut session) => {
                        session._entry = Some(entry);
                        let _ = accepted.send(session);
                    }
                    Err(_) => {
                        failed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        if spawned.is_err() {
            handshakes.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn start_server_session(
    ctx: &SslContext,
    socket: Arc<UdpSocket>,
    peer: SocketAddr,
    events: Receiver<Event>,
    tx: SyncSender<Event>,
) -> io::Result<Session> {
    let mut ssl = Ssl::new(ctx).map_err(io::Error::other)?;
    ssl.set_mtu(MTU).map_err(io::Error::other)?;
    ssl.set_ex_data(peer_index().map_err(io::Error::other)?, peer);
    let io = Datagrams {
        events,
        socket,
        peer,
        connected: false,
        wait: POLL,
        woken: false,
    };
    let mut stream = SslStream::new(ssl, io).map_err(io::Error::other)?;
    handshake(&mut stream, true, Instant::now() + HANDSHAKE_TIMEOUT)?;
    let identity = identity_of(stream.ssl(), true);
    Ok(Session {
        stream,
        peer,
        identity,
        waker: Waker(WakeTo::Server(tx)),
        _entry: None,
        _reader: None,
    })
}

/// Opens a DTLS session to `server` with `config`, the handshake within
/// `timeout`.
///
/// # Errors
/// The configuration makes no context, the socket fails, the handshake
/// fails (among others: a certificate or raw public key not trusted) or
/// takes longer.
pub fn connect(
    server: SocketAddr,
    config: &ClientConfig,
    timeout: Duration,
) -> io::Result<Session> {
    let ctx = config.context().map_err(io::Error::other)?;
    let mut ssl = config.ssl(&ctx).map_err(io::Error::other)?;
    ssl.set_mtu(MTU).map_err(io::Error::other)?;
    let local: SocketAddr = if server.is_ipv4() {
        "0.0.0.0:0".parse().map_err(io::Error::other)?
    } else {
        "[::]:0".parse().map_err(io::Error::other)?
    };
    let socket = Arc::new(UdpSocket::bind(local)?);
    socket.connect(server)?;
    socket.set_read_timeout(Some(Duration::from_millis(200)))?;
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let (socket, stop, tx) = (Arc::clone(&socket), Arc::clone(&stop), tx.clone());
        std::thread::Builder::new()
            .name("dtls-client-recv".into())
            .spawn(move || {
                let mut buf = vec![0u8; 65_536];
                while !stop.load(Ordering::SeqCst) {
                    match socket.recv(&mut buf) {
                        Ok(n) => {
                            if tx.send(Event::Datagram(buf[..n].to_vec())).is_err() {
                                break;
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                            // ICMP port unreachable: the session ends.
                            let _ = tx.send(Event::Refused);
                            break;
                        }
                        Err(_) => {}
                    }
                }
            })?
    };
    let reader = Reader {
        stop,
        thread: Some(thread),
    };
    let io = Datagrams {
        events: rx,
        socket,
        peer: server,
        connected: true,
        wait: POLL,
        woken: false,
    };
    let mut stream = SslStream::new(ssl, io).map_err(io::Error::other)?;
    handshake(&mut stream, false, Instant::now() + timeout)?;
    let identity = identity_of(stream.ssl(), false);
    Ok(Session {
        stream,
        peer: server,
        identity,
        waker: Waker(WakeTo::Client(tx)),
        _entry: None,
        _reader: Some(reader),
    })
}
