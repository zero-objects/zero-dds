// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stream mode (`zerodds-uds-transport-1.0` §3.5): RTPS messages over
//! `AF_UNIX` `SOCK_STREAM` connections, each message a frame of a
//! four-octet little-endian length and its octets.
//!
//! Windows has no `SOCK_DGRAM` for `AF_UNIX`, only `SOCK_STREAM` (§6.2), so
//! this is the UDS transport there; on Unix it is the alternative to the
//! datagram transport of §3.1, with the same path resolution (§2.1), base
//! directory (§2.2), socket-file mode (§5.1) and error classification
//! (§4.3).
//!
//! - One listening socket per transport at `<base_dir>/<hex32>.sock`. An
//!   accept thread takes the connections; a thread per connection reads its
//!   frames into a bounded receive queue that [`Transport::recv`] takes from.
//!   The accept thread waits for a connection at most [`ACCEPT_POLL`] at a
//!   time and then looks whether the transport closed: a close does not
//!   reach it through the socket path, which another transport may have
//!   bound since.
//! - A connection begins with the hello: [`STREAM_MAGIC`] and the sender's
//!   16-byte id. The id is the source locator of every frame on the
//!   connection (`Locator::uds(id)`); a connection's peer address is unnamed
//!   (the sender does not bind its connecting socket), so it is the only
//!   name of the sender. A connection that has not sent the whole hello
//!   within [`HELLO_TIMEOUT`] of its accept, counted over all its reads, is
//!   closed and counted.
//! - A frame above `max_datagram` is reported by `recv` as
//!   [`RecvError::BufferTooSmall`] with its length and skipped; the frames
//!   after it on the connection arrive. A connection that ends inside a
//!   frame is counted, its partial frame is not handed on.
//! - `send` keeps one connection per destination and connects on the first
//!   send to it. A send waits at most `send_timeout` for room; then it fails
//!   with "uds: kernel send buffer full (EAGAIN)" (§4.3). A connection on
//!   which a frame went out only in part is closed, so the peer never takes
//!   the rest of a frame for the start of the next. A connection the peer
//!   has closed (the peer restarted) is replaced by a new one once, within
//!   the same send.
//! - The receive queue holds [`STREAM_QUEUE_FRAMES`] frames. When it is full
//!   the connection threads stop reading, the kernel buffers fill and the
//!   peer's send waits and fails after its `send_timeout`, as a datagram
//!   send to a full receive queue does (§4.3); nothing is dropped.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io;
use std::net::Shutdown;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use socket2::{Domain, SockAddr, Socket, Type};
use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

use crate::{UdsConfig, classify_send_error, ensure_base_dir, remove_stale_socket, socket_path};

/// The four octets a connection begins with, before the sender's id.
pub const STREAM_MAGIC: [u8; 4] = *b"ZUDS";

/// Length of the hello: [`STREAM_MAGIC`] and the 16-byte id.
pub const STREAM_HELLO_LEN: usize = 20;

/// Length of a frame's header: the message length, `u32` little endian.
pub const STREAM_FRAME_HEADER_LEN: usize = 4;

/// Frames the receive queue holds before the connection threads stop
/// reading.
pub const STREAM_QUEUE_FRAMES: usize = 128;

/// How long an accepted connection may take to send its whole hello.
pub const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the accept thread waits for a connection before it looks
/// whether the transport closed: the longest a close waits for it.
pub const ACCEPT_POLL: Duration = Duration::from_millis(50);

/// Connections the listening socket queues before they are accepted.
const LISTEN_BACKLOG: i32 = 128;

/// The shortest write timeout a socket is given. Windows counts
/// `SO_SNDTIMEO` in milliseconds, and zero means no limit there and on
/// Unix.
const MIN_SOCKET_TIMEOUT: Duration = Duration::from_millis(1);

/// The flag that keeps a write to a connection the peer closed from raising
/// `SIGPIPE`: Linux has `MSG_NOSIGNAL`; Apple systems get `SO_NOSIGPIPE`
/// on the socket instead; Windows has no `SIGPIPE`.
#[cfg(any(target_os = "linux", target_os = "android"))]
const NO_SIGPIPE: i32 = libc::MSG_NOSIGNAL;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const NO_SIGPIPE: i32 = 0;

/// The receive queue and the state the connection threads share with the
/// transport.
struct Shared {
    queue: Mutex<VecDeque<Result<ReceivedDatagram, RecvError>>>,
    /// Signalled when the queue gains an entry or the transport closes.
    not_empty: Condvar,
    /// Signalled when the queue loses an entry or the transport closes.
    not_full: Condvar,
    closed: AtomicBool,
    max_datagram: usize,
    /// Accepted connections, so that `close` can end their reads.
    connections: Mutex<Vec<(u64, Socket)>>,
    readers: Mutex<Vec<JoinHandle<()>>>,
    next_connection: AtomicU64,
    accepted: AtomicU64,
    truncated_frames: AtomicU64,
    refused_hellos: AtomicU64,
    connection_errors: AtomicU64,
}

/// A mutex's guard, also when a thread panicked while holding it: the data
/// these locks guard (a queue, a list of sockets) stays consistent.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Shared {
    /// An empty queue, no connection, the counters at zero.
    fn new(max_datagram: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            not_empty: Condvar::new(),
            not_full: Condvar::new(),
            closed: AtomicBool::new(false),
            max_datagram,
            connections: Mutex::new(Vec::new()),
            readers: Mutex::new(Vec::new()),
            next_connection: AtomicU64::new(0),
            accepted: AtomicU64::new(0),
            truncated_frames: AtomicU64::new(0),
            refused_hellos: AtomicU64::new(0),
            connection_errors: AtomicU64::new(0),
        }
    }

    /// Puts an entry on the receive queue, waiting while it is full. `false`
    /// when the transport closed first.
    fn push(&self, item: Result<ReceivedDatagram, RecvError>) -> bool {
        let mut queue = lock(&self.queue);
        while queue.len() >= STREAM_QUEUE_FRAMES {
            if self.closed.load(Ordering::Acquire) {
                return false;
            }
            queue = self
                .not_full
                .wait(queue)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if self.closed.load(Ordering::Acquire) {
            return false;
        }
        queue.push_back(item);
        self.not_empty.notify_one();
        true
    }
}

/// UDS transport in stream mode, bound to a 16-byte locator id
/// (`zerodds-uds-transport-1.0` §3.5).
pub struct UdsStreamTransport {
    local_id: [u8; 16],
    config: UdsConfig,
    path: PathBuf,
    shared: Arc<Shared>,
    accept: Mutex<Option<JoinHandle<()>>>,
    /// One connection per destination id.
    outbound: Mutex<HashMap<[u8; 16], Arc<Mutex<Socket>>>>,
}

impl UdsStreamTransport {
    /// Binds a listening socket at `<base_dir>/<hex32>.sock` for
    /// `local_id` and starts its accept thread. A file left at the path by
    /// a crashed instance is removed first.
    ///
    /// # Errors
    /// - `InvalidInput` for a mode outside the permitted bits, a `base_dir`
    ///   that is a symlink (on Windows any reparse point) or not a
    ///   directory, a socket path longer than `sun_path` (108 octets on
    ///   Linux and Windows, 104 on macOS), or a socket path that is no
    ///   longer the socket this process bound when its mode is set.
    /// - `Unsupported` on Windows for a `socket_mode` and for a
    ///   `base_dir_mode` other than [`crate::DEFAULT_BASE_DIR_MODE`] (§6.2).
    /// - `PermissionDenied` for a created `base_dir` whose mode the kernel
    ///   did not set as `base_dir_mode` asks.
    /// - I/O error creating `base_dir`, binding or listening.
    pub fn bind(local_id: [u8; 16], config: UdsConfig) -> io::Result<Self> {
        crate::check_modes(config.socket_mode, config.base_dir_mode)?;
        ensure_base_dir(&config.base_dir, config.base_dir_mode)?;
        let path = socket_path(&config.base_dir, local_id);
        let addr = SockAddr::unix(&path)?;
        remove_stale_socket(&path)?;
        let listener = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        listener.bind(&addr)?;
        // From here on a failure removes the socket file again.
        let bound = RemoveOnFailure(Some(path.clone()));
        #[cfg(unix)]
        crate::apply_socket_mode(&path, config.socket_mode)?;
        listener.listen(LISTEN_BACKLOG)?;
        // The accept thread waits in `wait_readable`; an `accept` after a
        // connection went away again returns instead of blocking.
        listener.set_nonblocking(true)?;
        let shared = Arc::new(Shared::new(config.max_datagram));
        let accept_shared = Arc::clone(&shared);
        let accept = std::thread::Builder::new()
            .name("zdds-uds-accept".into())
            .spawn(move || accept_loop(&listener, &accept_shared))?;
        bound.keep();
        Ok(Self {
            local_id,
            config,
            path,
            shared,
            accept: Mutex::new(Some(accept)),
            outbound: Mutex::new(HashMap::new()),
        })
    }

    /// Connections accepted since the bind.
    #[must_use]
    pub fn accepted_connections(&self) -> u64 {
        self.shared.accepted.load(Ordering::Relaxed)
    }

    /// Frames a peer began and did not finish: the connection ended inside
    /// the frame or its header.
    #[must_use]
    pub fn truncated_frames(&self) -> u64 {
        self.shared.truncated_frames.load(Ordering::Relaxed)
    }

    /// Connections closed because they did not begin with a hello or did
    /// not send the whole hello within [`HELLO_TIMEOUT`].
    #[must_use]
    pub fn refused_hellos(&self) -> u64 {
        self.shared.refused_hellos.load(Ordering::Relaxed)
    }

    /// Connections whose reading ended in an error other than their end.
    #[must_use]
    pub fn connection_errors(&self) -> u64 {
        self.shared.connection_errors.load(Ordering::Relaxed)
    }

    /// The path of the listening socket.
    #[must_use]
    pub fn socket_path(&self) -> &std::path::Path {
        &self.path
    }

    /// The connection to `id`, connected now when there is none.
    fn connection(&self, id: [u8; 16]) -> Result<Arc<Mutex<Socket>>, SendError> {
        let mut outbound = lock(&self.outbound);
        if let Some(conn) = outbound.get(&id) {
            return Ok(Arc::clone(conn));
        }
        let socket = self.connect(id).map_err(|e| classify_send_error(&e))?;
        let conn = Arc::new(Mutex::new(socket));
        outbound.insert(id, Arc::clone(&conn));
        Ok(conn)
    }

    /// Opens a connection to the listening socket of `id` and sends the
    /// hello. The connect and the hello wait at most `send_timeout`.
    fn connect(&self, id: [u8; 16]) -> io::Result<Socket> {
        let addr = SockAddr::unix(socket_path(&self.config.base_dir, id))?;
        let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos"
        ))]
        socket.set_nosigpipe(true)?;
        socket.set_write_timeout(Some(self.config.send_timeout.max(MIN_SOCKET_TIMEOUT)))?;
        socket.connect(&addr)?;
        let mut hello = [0u8; STREAM_HELLO_LEN];
        hello[..4].copy_from_slice(&STREAM_MAGIC);
        hello[4..].copy_from_slice(&self.local_id);
        write_frame(&socket, &hello, self.config.send_timeout).map_err(|(e, _)| e)?;
        if self.config.send_timeout.is_zero() {
            socket.set_nonblocking(true)?;
        }
        Ok(socket)
    }

    /// Drops the connection to `id` if it is still `conn`.
    fn forget(&self, id: [u8; 16], conn: &Arc<Mutex<Socket>>) {
        let mut outbound = lock(&self.outbound);
        if outbound.get(&id).is_some_and(|c| Arc::ptr_eq(c, conn)) {
            outbound.remove(&id);
        }
    }

    /// Stops the accept thread and the connection threads, wakes a waiting
    /// `recv`, closes every connection. Idempotent.
    fn shut_down(&self) {
        if self.shared.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        {
            // Under the queue's lock, so that no waiter misses the flag
            // between its check and its wait.
            let _queue = lock(&self.shared.queue);
            self.shared.not_empty.notify_all();
            self.shared.not_full.notify_all();
        }
        // The accept thread sees the flag within `ACCEPT_POLL`.
        if let Some(handle) = lock(&self.accept).take() {
            let _ = handle.join();
        }
        for (_, socket) in lock(&self.shared.connections).drain(..) {
            let _ = socket.shutdown(Shutdown::Both);
        }
        let readers: Vec<JoinHandle<()>> = lock(&self.shared.readers).drain(..).collect();
        for handle in readers {
            let _ = handle.join();
        }
        lock(&self.outbound).clear();
    }
}

impl Drop for UdsStreamTransport {
    fn drop(&mut self) {
        self.shut_down();
        // Best effort, as the datagram transport: a detached volume may
        // already be gone.
        let _ = fs::remove_file(&self.path);
    }
}

/// Removes the socket file when the bind fails after creating it.
struct RemoveOnFailure(Option<PathBuf>);

impl RemoveOnFailure {
    fn keep(mut self) {
        self.0 = None;
    }
}

impl Drop for RemoveOnFailure {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}

/// Takes connections until the transport closes; one reader thread each.
/// The listener is non-blocking: the thread waits for a connection at most
/// [`ACCEPT_POLL`] at a time and looks at the closed flag in between.
fn accept_loop(listener: &Socket, shared: &Arc<Shared>) {
    loop {
        if shared.closed.load(Ordering::Acquire) {
            return;
        }
        match wait_readable(listener, ACCEPT_POLL) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => {
                shared.connection_errors.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
        }
        let socket = match accept(listener) {
            Ok(socket) => socket,
            // The connection went away between the wait and the accept.
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) =>
            {
                continue;
            }
            Err(_) => {
                // Out of descriptors or memory: wait before the next
                // attempt instead of spinning on the same error.
                shared.connection_errors.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
        };
        let accepted_at = Instant::now();
        if shared.closed.load(Ordering::Acquire) {
            return;
        }
        shared.accepted.fetch_add(1, Ordering::Relaxed);
        // On macOS, the BSDs and Windows an accepted socket inherits the
        // listener's non-blocking mode; the reader blocks in its reads.
        if socket.set_nonblocking(false).is_err() {
            shared.connection_errors.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        let Ok(handle) = socket.try_clone() else {
            shared.connection_errors.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let id = shared.next_connection.fetch_add(1, Ordering::Relaxed);
        lock(&shared.connections).push((id, handle));
        let reader_shared = Arc::clone(shared);
        let spawned = std::thread::Builder::new()
            .name("zdds-uds-conn".into())
            .spawn(move || {
                read_connection(&socket, &reader_shared, accepted_at + HELLO_TIMEOUT);
                lock(&reader_shared.connections).retain(|(c, _)| *c != id);
            });
        let mut readers = lock(&shared.readers);
        readers.retain(|h| !h.is_finished());
        match spawned {
            Ok(handle) => readers.push(handle),
            Err(_) => {
                shared.connection_errors.fetch_add(1, Ordering::Relaxed);
                lock(&shared.connections).retain(|(c, _)| *c != id);
            }
        }
    }
}

/// Accepts a connection and sets no socket option on it. macOS refuses
/// every socket option with `EINVAL` on a socket whose peer has closed, and
/// `Socket::accept` sets `SO_NOSIGPIPE` there: a peer that connected, wrote
/// its frames and closed before the accept thread took the connection lost
/// the connection with all it wrote, counted only as a connection error.
/// Close-on-exec is set with `fcntl`, which a closed peer does not affect;
/// the accepted socket is only read, so `SIGPIPE` does not concern it. The
/// other systems accept with `accept4` and its flags, or without an option
/// that a closed peer refuses.
#[cfg(target_vendor = "apple")]
fn accept(listener: &Socket) -> io::Result<Socket> {
    let (socket, _) = listener.accept_raw()?;
    socket.set_cloexec(true)?;
    Ok(socket)
}

/// See the Apple variant above.
#[cfg(not(target_vendor = "apple"))]
fn accept(listener: &Socket) -> io::Result<Socket> {
    listener.accept().map(|(socket, _)| socket)
}

/// The milliseconds `poll` waits for `timeout`, rounded up: a wait shorter
/// than a millisecond is not a wait of zero, which would return at once.
fn poll_millis(timeout: Duration) -> i32 {
    i32::try_from(timeout.as_micros().div_ceil(1000)).unwrap_or(i32::MAX)
}

/// Waits until `socket` has a connection to accept (a listening socket) or
/// octets or its end to read (a connection), at most `timeout`.
/// `Ok(false)` when the time ran out.
#[cfg(unix)]
fn wait_readable(socket: &Socket, timeout: Duration) -> io::Result<bool> {
    use std::os::unix::io::AsRawFd;
    let mut fd = libc::pollfd {
        fd: socket.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ms: libc::c_int = poll_millis(timeout);
    // SAFETY: `fd` is one valid `pollfd` that lives for the call, and the
    // count passed is 1.
    let n = unsafe { libc::poll(&mut fd, 1, ms) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(n > 0)
}

/// See the Unix variant above.
#[cfg(windows)]
fn wait_readable(socket: &Socket, timeout: Duration) -> io::Result<bool> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{
        POLLRDNORM, SOCKET, SOCKET_ERROR, WSAGetLastError, WSAPOLLFD, WSAPoll,
    };
    let handle = SOCKET::try_from(socket.as_raw_socket())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "socket handle out of range"))?;
    let mut fd = WSAPOLLFD {
        fd: handle,
        events: POLLRDNORM,
        revents: 0,
    };
    let ms = poll_millis(timeout);
    // SAFETY: `fd` is one valid `WSAPOLLFD` that lives for the call, and
    // the count passed is 1.
    let n = unsafe { WSAPoll(&mut fd, 1, ms) };
    if n == SOCKET_ERROR {
        // SAFETY: no precondition; it reads the calling thread's last
        // Winsock error.
        return Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }));
    }
    Ok(n > 0)
}

/// Whether the peer of the outgoing connection `socket` has closed it: the
/// connection is readable (the peer writes nothing on it, so only its end
/// makes it so) and a look at the receive side finds no octet.
///
/// On Linux and macOS the write after the peer closed fails (`EPIPE`) and
/// the send connects anew (§3.5). On Windows that write succeeds, and the
/// frame is lost: after the peer's restart the second message never
/// arrived (`uds_3_5_stream_reconnects_after_the_peer_restarts` on the Windows CI runner,
/// pipeline 10055, job 239493). So there a send first looks, as the TCP
/// transport does before each write.
#[cfg(windows)]
fn peer_closed(socket: &Socket) -> bool {
    match wait_readable(socket, Duration::ZERO) {
        Ok(false) => false,
        Ok(true) => {
            let mut probe = [std::mem::MaybeUninit::<u8>::uninit(); 1];
            !matches!(socket.peek(&mut probe), Ok(n) if n > 0)
        }
        Err(_) => true,
    }
}

/// How a read of a fixed number of octets ended.
enum Fill {
    Done,
    /// The connection ended before the first octet.
    EndBefore,
    /// The connection ended after some octets.
    EndInside,
    /// A read failed (not interrupted), or the deadline passed.
    Failed,
}

/// Reads exactly `buf.len()` octets, with a `deadline` over all its reads
/// when one is given: before each read it waits with `poll` at most the
/// time left. The deadline is no `SO_RCVTIMEO`: macOS refuses that option
/// with `EINVAL` on a connection whose peer has closed, while the octets
/// the peer wrote before are still there to read. Once `closed` is set
/// the read fails.
///
/// Without a deadline the read blocks, and the close of the transport
/// ends it by shutting the connection down: on Linux and macOS a `recv`
/// that waits returns then. On Windows it does not, while the peer holds
/// the connection open, and the close waited on the reading thread
/// (measured on the Windows CI runner: past 25 s, pipeline 10050, job 239479); after a
/// timed-out `SO_RCVTIMEO` the connection is in an indeterminate state
/// (Microsoft, `setsockopt`). So on Windows a read without a deadline
/// first waits with `WSAPoll`, at most [`ACCEPT_POLL`] at a time, and
/// looks at `closed` in between.
fn fill(socket: &Socket, buf: &mut [u8], deadline: Option<Instant>, closed: &AtomicBool) -> Fill {
    let mut got = 0;
    while got < buf.len() {
        if closed.load(Ordering::Acquire) {
            return Fill::Failed;
        }
        #[cfg(windows)]
        if deadline.is_none() {
            match wait_readable(socket, ACCEPT_POLL) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Fill::Failed,
            }
        }
        if let Some(deadline) = deadline {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Fill::Failed;
            }
            match wait_readable(socket, left) {
                Ok(true) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Ok(false) | Err(_) => return Fill::Failed,
            }
        }
        match io::Read::read(&mut &*socket, &mut buf[got..]) {
            Ok(0) if got == 0 => return Fill::EndBefore,
            Ok(0) => return Fill::EndInside,
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Fill::Failed,
        }
    }
    Fill::Done
}

/// Reads and discards `len` octets.
fn skip(socket: &Socket, mut len: usize, closed: &AtomicBool) -> Fill {
    let mut buf = [0u8; 8192];
    while len > 0 {
        let take = len.min(buf.len());
        match fill(socket, &mut buf[..take], None, closed) {
            Fill::Done => len -= take,
            Fill::EndBefore => return Fill::EndInside,
            other => return other,
        }
    }
    Fill::Done
}

/// Reads one connection: its hello, which must be whole by
/// `hello_deadline`, then frames until it ends or the transport closes.
fn read_connection(socket: &Socket, shared: &Shared, hello_deadline: Instant) {
    let mut hello = [0u8; STREAM_HELLO_LEN];
    match fill(socket, &mut hello, Some(hello_deadline), &shared.closed) {
        Fill::Done if hello[..4] == STREAM_MAGIC => {}
        // `close` shut the connection down.
        _ if shared.closed.load(Ordering::Acquire) => return,
        _ => {
            shared.refused_hellos.fetch_add(1, Ordering::Relaxed);
            return;
        }
    }
    let mut id = [0u8; 16];
    id.copy_from_slice(&hello[4..]);
    let source = Locator::uds(id);
    loop {
        let mut header = [0u8; STREAM_FRAME_HEADER_LEN];
        match fill(socket, &mut header, None, &shared.closed) {
            Fill::Done => {}
            Fill::EndBefore => return,
            Fill::EndInside => {
                shared.truncated_frames.fetch_add(1, Ordering::Relaxed);
                return;
            }
            Fill::Failed => {
                if !shared.closed.load(Ordering::Acquire) {
                    shared.connection_errors.fetch_add(1, Ordering::Relaxed);
                }
                return;
            }
        }
        let len = u32::from_le_bytes(header) as usize;
        let item = if len > shared.max_datagram {
            // The frame is skipped and reported; the stream stays in step
            // and the next frame is read (§3.5).
            match skip(socket, len, &shared.closed) {
                Fill::Done => Err(RecvError::BufferTooSmall {
                    datagram_size: len,
                    buffer_size: shared.max_datagram,
                }),
                end => {
                    count_end(shared, &end);
                    return;
                }
            }
        } else {
            let mut data = vec![0u8; len];
            match fill(socket, &mut data, None, &shared.closed) {
                Fill::Done => Ok(ReceivedDatagram {
                    source,
                    data: Arc::<[u8]>::from(data.into_boxed_slice()).into(),
                }),
                end => {
                    count_end(shared, &end);
                    return;
                }
            }
        };
        if !shared.push(item) {
            return;
        }
    }
}

/// Counts a connection that ended inside a frame's octets.
fn count_end(shared: &Shared, end: &Fill) {
    match end {
        Fill::Failed if shared.closed.load(Ordering::Acquire) => {}
        Fill::Failed => {
            shared.connection_errors.fetch_add(1, Ordering::Relaxed);
        }
        _ => {
            shared.truncated_frames.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Writes `buf` whole. The error says whether some octets went out before
/// it (`true`): the connection then ends inside a frame and must be closed.
/// After a write that went out in part the remaining octets get no more
/// than what is left of `send_timeout`.
fn write_frame(
    socket: &Socket,
    buf: &[u8],
    send_timeout: Duration,
) -> Result<(), (io::Error, bool)> {
    let start = Instant::now();
    let mut written = 0;
    while written < buf.len() {
        if written > 0 && start.elapsed() >= send_timeout {
            return Err((io::Error::from(io::ErrorKind::WouldBlock), true));
        }
        match socket.send_with_flags(&buf[written..], NO_SIGPIPE) {
            Ok(0) => return Err((io::Error::from(io::ErrorKind::WriteZero), written > 0)),
            Ok(n) => written += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err((e, written > 0)),
        }
    }
    Ok(())
}

/// An error that says the peer closed the connection: a new connection may
/// reach the peer's new listening socket.
fn closed_by_peer(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::NotConnected
            | io::ErrorKind::WriteZero
    )
}

impl Transport for UdsStreamTransport {
    /// The frame limit (`max_datagram`).
    fn max_message_size(&self) -> usize {
        self.config.max_datagram
    }

    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if dest.kind != LocatorKind::Uds {
            return Err(SendError::UnsupportedLocator);
        }
        if data.len() > self.config.max_datagram {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: self.config.max_datagram,
            });
        }
        if self.shared.closed.load(Ordering::Acquire) {
            return Err(SendError::Io {
                message: "uds stream transport closed",
            });
        }
        let len = u32::try_from(data.len()).map_err(|_| SendError::PayloadTooLarge {
            size: data.len(),
            limit: u32::MAX as usize,
        })?;
        let mut frame = Vec::with_capacity(STREAM_FRAME_HEADER_LEN + data.len());
        frame.extend_from_slice(&len.to_le_bytes());
        frame.extend_from_slice(data);
        let mut reconnected = false;
        loop {
            let conn = self.connection(dest.address)?;
            let socket = lock(&conn);
            #[cfg(windows)]
            if !reconnected && peer_closed(&socket) {
                drop(socket);
                self.forget(dest.address, &conn);
                reconnected = true;
                continue;
            }
            let written = write_frame(&socket, &frame, self.config.send_timeout);
            drop(socket);
            match written {
                Ok(()) => return Ok(()),
                Err((e, in_part)) => {
                    let replace = closed_by_peer(&e) && !reconnected;
                    // A frame that went out in part leaves the peer inside
                    // it; a closed connection carries nothing more; on
                    // Windows a socket whose send timed out is in an
                    // indeterminate state and is not used again
                    // (Microsoft, `SO_SNDTIMEO`).
                    let timed_out = matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    );
                    if in_part || closed_by_peer(&e) || (cfg!(windows) && timed_out) {
                        self.forget(dest.address, &conn);
                    }
                    if replace {
                        reconnected = true;
                        continue;
                    }
                    return Err(classify_send_error(&e));
                }
            }
        }
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let deadline = self.config.recv_timeout.map(|t| Instant::now() + t);
        let mut queue = lock(&self.shared.queue);
        loop {
            if let Some(item) = queue.pop_front() {
                self.shared.not_full.notify_one();
                return item;
            }
            if self.shared.closed.load(Ordering::Acquire) {
                return Err(RecvError::Io {
                    message: "uds stream transport closed",
                });
            }
            queue = match deadline {
                None => self
                    .shared
                    .not_empty
                    .wait(queue)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return Err(RecvError::Timeout);
                    }
                    self.shared
                        .not_empty
                        .wait_timeout(queue, deadline - now)
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .0
                }
            };
        }
    }

    /// Takes the first entry as [`Transport::recv`] does and every message
    /// queued behind it, up to `max`; an error entry behind the first one
    /// stays for the next call.
    fn recv_batch(&self, out: &mut Vec<ReceivedDatagram>, max: usize) -> Result<usize, RecvError> {
        if max == 0 {
            return Ok(0);
        }
        out.push(self.recv()?);
        let mut n = 1;
        let mut queue = lock(&self.shared.queue);
        while n < max && matches!(queue.front(), Some(Ok(_))) {
            if let Some(Ok(dg)) = queue.pop_front() {
                out.push(dg);
                n += 1;
            }
        }
        self.shared.not_full.notify_all();
        Ok(n)
    }

    fn local_locator(&self) -> Locator {
        Locator::uds(self.local_id)
    }

    fn forget_destination(&self, dest: &Locator) {
        if dest.kind == LocatorKind::Uds {
            lock(&self.outbound).remove(&dest.address);
        }
    }

    fn close(&self) {
        self.shut_down();
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Write;

    fn id_for(n: u8) -> [u8; 16] {
        let mut a = [0u8; 16];
        a[15] = n;
        a
    }

    /// A temporary directory with a short path: `sun_path` holds 104
    /// octets on macOS, whose per-user temporary directory is long.
    fn short_tempdir() -> tempfile::TempDir {
        #[cfg(unix)]
        {
            let dir = tempfile::Builder::new().tempdir_in("/tmp").unwrap();
            // SAFETY: getegid has no preconditions and cannot fail.
            let egid = unsafe { libc::getegid() };
            std::os::unix::fs::chown(dir.path(), None, Some(egid)).unwrap();
            dir
        }
        #[cfg(not(unix))]
        {
            tempfile::tempdir().unwrap()
        }
    }

    fn cfg(dir: &std::path::Path) -> UdsConfig {
        UdsConfig {
            base_dir: dir.join("uds"),
            max_datagram: 8192,
            recv_timeout: Some(Duration::from_secs(2)),
            ..UdsConfig::default()
        }
    }

    /// A raw client of the listening socket of `id`.
    fn raw_client(dir: &std::path::Path, id: [u8; 16]) -> Socket {
        let s = Socket::new(Domain::UNIX, Type::STREAM, None).unwrap();
        s.connect(&SockAddr::unix(socket_path(&dir.join("uds"), id)).unwrap())
            .unwrap();
        s
    }

    fn hello(id: [u8; 16]) -> Vec<u8> {
        let mut h = STREAM_MAGIC.to_vec();
        h.extend_from_slice(&id);
        h
    }

    fn frame(data: &[u8]) -> Vec<u8> {
        let mut f = (data.len() as u32).to_le_bytes().to_vec();
        f.extend_from_slice(data);
        f
    }

    fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
        let start = Instant::now();
        while !cond() {
            assert!(start.elapsed() < Duration::from_secs(5), "{what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// §3.5: three messages of 0, 1 and 8192 octets (the limit) arrive
    /// whole, in order, each as one datagram: the frames keep the message
    /// boundaries a stream does not have.
    #[test]
    fn uds_3_5_stream_frames_keep_message_boundaries() {
        let tmp = short_tempdir();
        let a = UdsStreamTransport::bind(id_for(1), cfg(tmp.path())).unwrap();
        let b = UdsStreamTransport::bind(id_for(2), cfg(tmp.path())).unwrap();
        let big: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        for msg in [&b""[..], &b"x"[..], &big[..]] {
            a.send(&b.local_locator(), msg).unwrap();
        }
        for msg in [&b""[..], &b"x"[..], &big[..]] {
            let dg = b.recv().unwrap();
            assert_eq!(&dg.data[..], msg);
        }
    }

    /// §3.5: the source locator of a message is the id of the sender's
    /// hello, and a reply to it reaches the sender.
    #[test]
    fn uds_3_5_stream_source_locator_is_the_senders_id_and_reaches_it() {
        let tmp = short_tempdir();
        let a = UdsStreamTransport::bind(id_for(3), cfg(tmp.path())).unwrap();
        let b = UdsStreamTransport::bind(id_for(4), cfg(tmp.path())).unwrap();
        a.send(&b.local_locator(), b"ping").unwrap();
        let got = b.recv().unwrap();
        assert_eq!(got.source, Locator::uds(id_for(3)));
        b.send(&got.source, b"pong").unwrap();
        let back = a.recv().unwrap();
        assert_eq!(&back.data[..], b"pong");
        assert_eq!(back.source, Locator::uds(id_for(4)));
    }

    /// §3.5: a frame above `max_datagram` is reported as
    /// `BufferTooSmall` with its length, and the frame after it on the same
    /// connection arrives.
    #[test]
    fn uds_3_5_stream_a_frame_above_max_datagram_is_reported_and_the_next_arrives() {
        let tmp = short_tempdir();
        let b = UdsStreamTransport::bind(id_for(5), cfg(tmp.path())).unwrap();
        let mut c = raw_client(tmp.path(), id_for(5));
        c.write_all(&hello(id_for(6))).unwrap();
        c.write_all(&frame(&vec![7u8; 8193])).unwrap();
        c.write_all(&frame(b"after")).unwrap();
        match b.recv() {
            Err(RecvError::BufferTooSmall {
                datagram_size,
                buffer_size,
            }) => {
                assert_eq!((datagram_size, buffer_size), (8193, 8192));
            }
            other => panic!("{other:?}"),
        }
        let dg = b.recv().unwrap();
        assert_eq!(&dg.data[..], b"after");
        assert_eq!(dg.source, Locator::uds(id_for(6)));
    }

    /// §3.5: `send` refuses a payload above `max_datagram`.
    #[test]
    fn uds_3_5_stream_send_refuses_a_payload_above_max_datagram() {
        let tmp = short_tempdir();
        let a = UdsStreamTransport::bind(id_for(7), cfg(tmp.path())).unwrap();
        let b = UdsStreamTransport::bind(id_for(8), cfg(tmp.path())).unwrap();
        assert_eq!(
            a.send(&b.local_locator(), &vec![0u8; 8193]),
            Err(SendError::PayloadTooLarge {
                size: 8193,
                limit: 8192
            })
        );
        assert_eq!(
            a.send(&Locator::udp_v4([127, 0, 0, 1], 7400), b"x"),
            Err(SendError::UnsupportedLocator)
        );
    }

    /// §3.5: a connection that does not begin with the hello is closed and
    /// counted, nothing of it is handed on; a connection that ends inside a
    /// frame is counted as a truncated frame.
    #[test]
    fn uds_3_5_stream_a_connection_without_the_hello_or_ending_inside_a_frame_is_counted() {
        let tmp = short_tempdir();
        let b = UdsStreamTransport::bind(id_for(9), cfg(tmp.path())).unwrap();
        let mut bad = raw_client(tmp.path(), id_for(9));
        bad.write_all(b"GET / HTTP/1.1\r\n\r\n.....").unwrap();
        wait_for("the bad hello is counted", || b.refused_hellos() == 1);
        let mut cut = raw_client(tmp.path(), id_for(9));
        cut.write_all(&hello(id_for(10))).unwrap();
        cut.write_all(&100u32.to_le_bytes()).unwrap();
        cut.write_all(&[1u8; 40]).unwrap();
        drop(cut);
        wait_for("the cut frame is counted", || b.truncated_frames() == 1);
        assert_eq!(b.recv().err(), Some(RecvError::Timeout));
        assert_eq!(b.connection_errors(), 0);
    }

    /// §3.5: a peer that connected, wrote its hello and frames and closed
    /// before the connection was accepted is read like one that is still
    /// there: its whole frame arrives with its id as the source, the frame
    /// it ended inside is counted. macOS refuses every socket option on
    /// such a connection; the accept set `SO_NOSIGPIPE` and the hello
    /// deadline was `SO_RCVTIMEO`, so the connection was dropped with what
    /// it carried and counted as a connection error.
    #[cfg(unix)]
    #[test]
    fn uds_3_5_stream_a_peer_that_closed_before_the_accept_is_read_whole() {
        let tmp = short_tempdir();
        let path = tmp.path().join("listener.sock");
        let listener = Socket::new(Domain::UNIX, Type::STREAM, None).unwrap();
        listener.bind(&SockAddr::unix(&path).unwrap()).unwrap();
        listener.listen(LISTEN_BACKLOG).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut peer = Socket::new(Domain::UNIX, Type::STREAM, None).unwrap();
        peer.connect(&SockAddr::unix(&path).unwrap()).unwrap();
        peer.write_all(&hello(id_for(30))).unwrap();
        peer.write_all(&frame(b"before the accept")).unwrap();
        peer.write_all(&100u32.to_le_bytes()).unwrap();
        peer.write_all(&[7u8; 10]).unwrap();
        drop(peer);
        assert!(wait_readable(&listener, Duration::from_secs(2)).unwrap());
        let socket = accept(&listener).expect("the connection of a closed peer is accepted");
        socket.set_nonblocking(false).unwrap();
        let shared = Shared::new(8192);
        read_connection(&socket, &shared, Instant::now() + HELLO_TIMEOUT);
        let queue = lock(&shared.queue);
        assert_eq!(queue.len(), 1);
        let dg = queue.front().unwrap().as_ref().unwrap();
        assert_eq!(dg.source, Locator::uds(id_for(30)));
        assert_eq!(&dg.data[..], b"before the accept");
        assert_eq!(shared.truncated_frames.load(Ordering::Relaxed), 1);
        assert_eq!(shared.refused_hellos.load(Ordering::Relaxed), 0);
        assert_eq!(shared.connection_errors.load(Ordering::Relaxed), 0);
    }

    /// §3.5: the 5 s of the hello hold for the whole hello, not for each
    /// read. A connection that sends nothing and a connection that sends
    /// the hello in pieces 1.5 s apart, the rest of it after 6 s, are both
    /// open at 4.5 s, closed and counted after 5 s; the frame the second
    /// sends after its late hello is not received.
    #[test]
    fn uds_3_5_stream_a_connection_without_a_hello_within_5_s_is_refused() {
        let tmp = short_tempdir();
        let mut config = cfg(tmp.path());
        config.recv_timeout = Some(Duration::from_millis(500));
        let b = UdsStreamTransport::bind(id_for(23), config).unwrap();
        let start = Instant::now();
        let _silent = raw_client(tmp.path(), id_for(23));
        let mut pieces = raw_client(tmp.path(), id_for(23));
        let whole = hello(id_for(24));
        // `Z` at 0 s, `U` at 1.5 s, `D` at 3 s, `S` at 4.5 s.
        for (i, octet) in whole[..4].iter().enumerate() {
            if i > 0 {
                std::thread::sleep(Duration::from_millis(1500));
            }
            pieces.write_all(&[*octet]).unwrap();
        }
        assert!(start.elapsed() < HELLO_TIMEOUT);
        assert_eq!(b.refused_hellos(), 0, "refused before 5 s");
        std::thread::sleep(Duration::from_secs(6).saturating_sub(start.elapsed()));
        // The transport has closed the connection: these writes may fail.
        let _ = pieces.write_all(&whole[4..]);
        let _ = pieces.write_all(&frame(b"late"));
        wait_for("both connections are refused", || b.refused_hellos() == 2);
        assert_eq!(b.recv().err(), Some(RecvError::Timeout));
        assert_eq!(b.accepted_connections(), 2);
        assert_eq!(b.truncated_frames(), 0);
    }

    /// §4.1 in stream mode: a transport whose socket path another
    /// transport bound since (the bind at the same id replaces the file) is
    /// dropped within a second, and its close sends nothing to the other
    /// transport.
    #[test]
    fn uds_4_1_stream_a_transport_whose_path_another_took_is_dropped() {
        let tmp = short_tempdir();
        let first = UdsStreamTransport::bind(id_for(25), cfg(tmp.path())).unwrap();
        let second = UdsStreamTransport::bind(id_for(25), cfg(tmp.path())).unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let start = Instant::now();
        std::thread::spawn(move || {
            drop(first);
            let _ = done_tx.send(());
        });
        done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the drop of the first transport returned within 5 s");
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "the drop took {:?}",
            start.elapsed()
        );
        assert_eq!(second.accepted_connections(), 0);
        assert_eq!(second.refused_hellos(), 0);
    }

    /// §3.5: when the peer restarts at the same id, the next send replaces
    /// the closed connection and arrives at the new listening socket.
    #[test]
    fn uds_3_5_stream_reconnects_after_the_peer_restarts() {
        let tmp = short_tempdir();
        let a = UdsStreamTransport::bind(id_for(11), cfg(tmp.path())).unwrap();
        let b = UdsStreamTransport::bind(id_for(12), cfg(tmp.path())).unwrap();
        a.send(&b.local_locator(), b"one").unwrap();
        assert_eq!(&b.recv().unwrap().data[..], b"one");
        drop(b);
        let b = UdsStreamTransport::bind(id_for(12), cfg(tmp.path())).unwrap();
        a.send(&b.local_locator(), b"two").unwrap();
        let dg = b.recv().unwrap();
        assert_eq!(&dg.data[..], b"two");
        assert_eq!(b.accepted_connections(), 1);
    }

    /// §4.1 in stream mode: the bind replaces a file left at its path and
    /// the drop removes the socket file.
    #[test]
    fn uds_4_1_stream_bind_replaces_a_leftover_file_and_drop_removes_it() {
        let tmp = short_tempdir();
        let config = cfg(tmp.path());
        fs::create_dir_all(&config.base_dir).unwrap();
        let path = socket_path(&config.base_dir, id_for(13));
        fs::write(&path, b"left over").unwrap();
        let t = UdsStreamTransport::bind(id_for(13), config).unwrap();
        assert_eq!(t.socket_path(), path.as_path());
        assert!(fs::symlink_metadata(&path).is_ok());
        drop(t);
        assert!(fs::symlink_metadata(&path).is_err(), "{path:?}");
    }

    /// §4.4 in stream mode: `recv` returns `Timeout` after `recv_timeout`
    /// without a message; after `close` a `recv` and a `send` fail with
    /// "uds stream transport closed".
    #[test]
    fn uds_4_4_stream_recv_times_out_when_idle_and_close_ends_it() {
        let tmp = short_tempdir();
        let mut config = cfg(tmp.path());
        config.recv_timeout = Some(Duration::from_millis(100));
        let t = UdsStreamTransport::bind(id_for(14), config).unwrap();
        let start = Instant::now();
        assert_eq!(t.recv().err(), Some(RecvError::Timeout));
        assert!(start.elapsed() >= Duration::from_millis(100));
        t.close();
        assert_eq!(
            t.recv().err(),
            Some(RecvError::Io {
                message: "uds stream transport closed"
            })
        );
        assert_eq!(
            t.send(&t.local_locator(), b"x"),
            Err(SendError::Io {
                message: "uds stream transport closed"
            })
        );
    }

    /// §4.3 in stream mode: a send to an id without a socket file names
    /// the missing socket; to a socket file nobody listens on, the refusal.
    /// Which of the two errors Windows reports for a missing file is not
    /// documented; there either is accepted for it.
    #[test]
    fn uds_4_3_stream_send_names_a_missing_socket_and_a_refusal() {
        const MISSING: SendError = SendError::Io {
            message: "uds: destination socket missing",
        };
        const REFUSED: SendError = SendError::Io {
            message: "uds: peer socket exists but refused (ECONNREFUSED)",
        };
        let tmp = short_tempdir();
        let a = UdsStreamTransport::bind(id_for(15), cfg(tmp.path())).unwrap();
        let missing = a.send(&Locator::uds(id_for(16)), b"x");
        if cfg!(unix) {
            assert_eq!(missing, Err(MISSING));
        } else {
            assert!(
                missing == Err(MISSING) || missing == Err(REFUSED),
                "{missing:?}"
            );
        }
        // A socket bound at the path that does not listen.
        let leftover = Socket::new(Domain::UNIX, Type::STREAM, None).unwrap();
        leftover
            .bind(&SockAddr::unix(socket_path(&tmp.path().join("uds"), id_for(17))).unwrap())
            .unwrap();
        assert_eq!(a.send(&Locator::uds(id_for(17)), b"x"), Err(REFUSED));
    }

    /// §4.3 in stream mode: when the peer does not take its messages, its
    /// receive queue and the kernel buffers fill, and a send fails with
    /// "uds: kernel send buffer full (EAGAIN)" after at most
    /// `send_timeout`, instead of waiting until the peer reads. Once the
    /// peer reads, sends go out again.
    #[test]
    fn uds_4_3_stream_a_peer_that_does_not_read_fails_the_send_with_the_full_buffer_error() {
        let tmp = short_tempdir();
        let mut config = cfg(tmp.path());
        config.max_datagram = 65_536;
        config.send_timeout = Duration::from_millis(50);
        let a = UdsStreamTransport::bind(id_for(18), config.clone()).unwrap();
        let b = UdsStreamTransport::bind(id_for(19), config).unwrap();
        let msg = vec![3u8; 65_536];
        let mut failed = None;
        for i in 0..(STREAM_QUEUE_FRAMES * 4) {
            let start = Instant::now();
            match a.send(&b.local_locator(), &msg) {
                Ok(()) => {}
                Err(e) => {
                    failed = Some((i, e, start.elapsed()));
                    break;
                }
            }
        }
        let (sent, error, waited) = failed.expect("a send failed");
        assert!(sent >= STREAM_QUEUE_FRAMES, "failed after {sent} sends");
        assert_eq!(
            error,
            SendError::Io {
                message: "uds: kernel send buffer full (EAGAIN)"
            }
        );
        assert!(waited < Duration::from_secs(1), "waited {waited:?}");
        let mut taken = 0;
        while b.recv().is_ok() {
            taken += 1;
            if taken == sent {
                break;
            }
        }
        assert!(taken >= STREAM_QUEUE_FRAMES, "took {taken}");
        a.send(&b.local_locator(), b"again").unwrap();
    }

    /// `recv_batch` hands over the messages queued behind the first.
    #[test]
    fn uds_3_5_stream_recv_batch_takes_the_queued_messages() {
        let tmp = short_tempdir();
        let a = UdsStreamTransport::bind(id_for(20), cfg(tmp.path())).unwrap();
        let b = UdsStreamTransport::bind(id_for(21), cfg(tmp.path())).unwrap();
        for i in 0..5u8 {
            a.send(&b.local_locator(), &[i]).unwrap();
        }
        wait_for("five queued", || lock(&b.shared.queue).len() == 5);
        let mut out = Vec::new();
        assert_eq!(b.recv_batch(&mut out, 32).unwrap(), 5);
        let got: Vec<u8> = out.iter().map(|d| d.data[0]).collect();
        assert_eq!(got, vec![0, 1, 2, 3, 4]);
    }

    /// §5.1 in stream mode: `socket_mode` is set on the listening socket's
    /// file.
    #[cfg(unix)]
    #[test]
    fn uds_5_1_stream_socket_mode_is_applied_to_the_socket_file() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = short_tempdir();
        let mut config = cfg(tmp.path());
        config.socket_mode = Some(0o660);
        let t = UdsStreamTransport::bind(id_for(22), config).unwrap();
        let mode = fs::symlink_metadata(t.socket_path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o660);
    }
}
