// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `UdpTransport`: concrete `Transport` implementation over
//! `std::net::UdpSocket`.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};
use zerodds_monitor::{Counter, Labels, default_registry, metric_names};
use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

fn udp_counters() -> &'static UdpCounters {
    static C: OnceLock<UdpCounters> = OnceLock::new();
    C.get_or_init(|| {
        let r = default_registry();
        r.set_help(
            metric_names::DDS_TRANSPORT_PACKETS_SENT_TOTAL,
            "RTPS packets sent (zerodds-monitor-1.1 §2.1)",
        );
        r.set_help(
            metric_names::DDS_TRANSPORT_PACKETS_RECEIVED_TOTAL,
            "RTPS packets received (zerodds-monitor-1.1 §2.1)",
        );
        r.set_help(
            metric_names::DDS_TRANSPORT_BYTES_SENT_TOTAL,
            "Bytes sent (zerodds-monitor-1.1 §2.1)",
        );
        r.set_help(
            metric_names::DDS_TRANSPORT_BYTES_RECEIVED_TOTAL,
            "Bytes received (zerodds-monitor-1.1 §2.1)",
        );
        r.set_help(
            metric_names::DDS_TRANSPORT_SEND_ERRORS_TOTAL,
            "Send errors (zerodds-monitor-1.1 §2.1)",
        );
        let labels = || Labels::new().with("transport", "udp");
        UdpCounters {
            packets_sent: r.counter(metric_names::DDS_TRANSPORT_PACKETS_SENT_TOTAL, labels()),
            packets_received: r
                .counter(metric_names::DDS_TRANSPORT_PACKETS_RECEIVED_TOTAL, labels()),
            bytes_sent: r.counter(metric_names::DDS_TRANSPORT_BYTES_SENT_TOTAL, labels()),
            bytes_received: r.counter(metric_names::DDS_TRANSPORT_BYTES_RECEIVED_TOTAL, labels()),
            send_errors: r.counter(
                metric_names::DDS_TRANSPORT_SEND_ERRORS_TOTAL,
                Labels::new()
                    .with("transport", "udp")
                    .with("error_kind", "io"),
            ),
            // Connected-cache telemetry (Option E in the spread audit
            // 2026-05-25): enables cache-hit-rate monitoring for
            // informed tuning decisions.
            cache_hits: r.counter("dds_transport_udp_connected_cache_hits_total", labels()),
            cache_misses: r.counter("dds_transport_udp_connected_cache_misses_total", labels()),
            cache_evictions: r.counter(
                "dds_transport_udp_connected_cache_evictions_total",
                labels(),
            ),
        }
    })
}

struct UdpCounters {
    packets_sent: Arc<Counter>,
    packets_received: Arc<Counter>,
    bytes_sent: Arc<Counter>,
    bytes_received: Arc<Counter>,
    send_errors: Arc<Counter>,
    cache_hits: Arc<Counter>,
    cache_misses: Arc<Counter>,
    cache_evictions: Arc<Counter>,
}

/// Maximum datagram size for a UDP recv. Bounded to the classic IP
/// datagram limit without fragmentation (safe for phase 0).
pub const MAX_DATAGRAM_SIZE: usize = 65_507;

/// Construction error.
#[derive(Debug)]
pub enum UdpTransportError {
    /// Bind failed.
    Bind(std::io::Error),
    /// `set_read_timeout` failed.
    SetTimeout(std::io::Error),
}

impl core::fmt::Display for UdpTransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bind(e) => write!(f, "udp bind failed: {e}"),
            Self::SetTimeout(e) => write!(f, "udp set_read_timeout failed: {e}"),
        }
    }
}

impl std::error::Error for UdpTransportError {}

/// Configuration for the connected UDP cache. Default values are the
/// optima measured on M1 (cache on, 16 entries FIFO eviction,
/// kernel-default SO_SNDBUF).
///
/// **Trade-off**: the connected cache reduces the per-send median by
/// ~5-8 µs on macOS/Linux, but for long run sessions (many transient
/// locators over time) it can increase jitter — the Linux test host
/// bench 2026-05-25 showed zerodds-self median CV rise from 4.8% to
/// 13.6%, while Cyclone/RTI stayed stable. Callers that want to
/// optimize latency over jitter can fall back to the classic `send_to`
/// path via `disabled()`.
#[derive(Debug, Clone)]
pub struct UdpTransportConfig {
    /// Enable the connected UDP cache. `false` → classic
    /// `send_to(data, addr)` without a cache. Default `true`.
    pub connected_cache_enabled: bool,
    /// Max number of ephemeral sockets in the cache. On overflow the
    /// oldest entry (FIFO) is dropped. 16 is typically enough for
    /// bridge daemons with 1-5 readers + discovery overhead. Default 16.
    pub connected_cache_max_entries: usize,
    /// Optional: SO_SNDBUF on ephemeral sockets in bytes. `None` =
    /// kernel default. Default `Some(256 * 1024)` so the ephemeral
    /// sockets do not work with a smaller default buffer than the
    /// bound listener socket (jitter mitigation).
    pub connected_cache_sndbuf: Option<usize>,
    /// SO_RCVBUF on the listener socket in bytes. `None` = kernel
    /// default (typically 208 KB on Linux, smaller on macOS). Default
    /// `Some(1 * 1024 * 1024)` = 1 MB analogous to Cyclone DDS — better
    /// burst absorption on loopback traffic, fewer packets dropped in
    /// the kernel queue under load → p99 mitigation.
    pub recv_buffer_size: Option<usize>,
    /// SO_DONTROUTE on the listener socket. Saves the per-send kernel
    /// routing-table lookup when the target is directly reachable
    /// (loopback, local LAN). Cyclone DDS enables this by default on
    /// all UDP sockets. With cross-subnet routing it must be disabled —
    /// then enable it manually caller-side only for loopback
    /// optimization. Default `false` for spec conformance, override
    /// via bench/config.
    pub dont_route: bool,
    /// The largest RTPS message, in octets, the transport is told to send in
    /// one datagram: what the network path carries without IP
    /// fragmentation. [`Transport::max_message_size`] reports it, and the
    /// fragment size of a writer follows the smallest of its transports
    /// (DDSI-RTPS 2.5 §8.4.14.1). Default `DEFAULT_MAX_MESSAGE_SIZE`,
    /// 1472: a UDP datagram that fits an Ethernet frame of 1500 octets.
    /// At most [`MAX_DATAGRAM_SIZE`], which a datagram cannot exceed.
    pub max_message_size: usize,
}

/// Default of [`UdpTransportConfig::max_message_size`]: 1500 octets of an
/// Ethernet payload less the IPv4 header (20) and the UDP header (8).
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = zerodds_transport::DEFAULT_MAX_MESSAGE_SIZE;

impl Default for UdpTransportConfig {
    /// The compiled defaults. A DDS participant reads its values from the
    /// runtime configuration (`transport.udp.*` in `zerodds.yaml`, the
    /// `ZERODDS_UDP_*` variables, `--set`) and passes them to the
    /// `_with_config` binds; nothing here reads the environment.
    fn default() -> Self {
        Self {
            connected_cache_enabled: true,
            connected_cache_max_entries: 16,
            connected_cache_sndbuf: Some(256 * 1024),
            // Cyclone DDS sets SO_RCVBUF=1 MB by default. We adopt that
            // — better burst absorption on loopback traffic.
            recv_buffer_size: Some(1024 * 1024),
            dont_route: false,
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
        }
    }
}

impl UdpTransportConfig {
    /// Classic `send_to(data, addr)` without a cache. Gives up the ~5-8 µs
    /// connected-cache latency win, in exchange for tighter jitter (the
    /// Linux test host 2026-05-25 showed zerodds-self median CV 4.8%
    /// instead of 13.6%).
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            connected_cache_enabled: false,
            ..Self::default()
        }
    }
}

/// FIFO cache for connected ephemeral UDP sockets.
///
/// Eviction policy: first-in-first-out (not LRU). For the typical DCPS
/// workload with few, equally-frequented readers, FIFO is equivalent
/// to LRU but simpler — no touch-on-read writes.
#[derive(Debug)]
struct ConnectedSocketCache {
    entries: std::collections::HashMap<SocketAddr, UdpSocket>,
    insertion_order: std::collections::VecDeque<SocketAddr>,
    max_entries: usize,
    sndbuf_bytes: Option<usize>,
}

impl ConnectedSocketCache {
    fn new(max_entries: usize, sndbuf_bytes: Option<usize>) -> Self {
        Self {
            entries: std::collections::HashMap::with_capacity(max_entries),
            insertion_order: std::collections::VecDeque::with_capacity(max_entries),
            max_entries,
            sndbuf_bytes,
        }
    }

    fn get(&self, addr: &SocketAddr) -> Option<&UdpSocket> {
        self.entries.get(addr)
    }

    /// Drops the socket for `addr`, if there is one.
    fn remove(&mut self, addr: &SocketAddr) {
        if self.entries.remove(addr).is_some() {
            self.insertion_order.retain(|a| a != addr);
        }
    }

    /// Insert + on overflow drop the FIFO-oldest entry.
    /// Returns `true` if an eviction took place.
    fn insert(&mut self, addr: SocketAddr, sock: UdpSocket) -> bool {
        let mut evicted = false;
        if !self.entries.contains_key(&addr) && self.entries.len() >= self.max_entries {
            if let Some(oldest) = self.insertion_order.pop_front() {
                self.entries.remove(&oldest);
                evicted = true;
            }
        }
        if self.entries.insert(addr, sock).is_none() {
            self.insertion_order.push_back(addr);
        }
        evicted
    }
}

/// UDP-based transport.
///
/// Construction binds a socket to a local port and remembers the local
/// locator for `local_locator()` calls. The receive timeout defaults to
/// `None` (blocks until a datagram arrives); settable via
/// `with_timeout`.
///
/// **Connected UDP cache** (see [`UdpTransportConfig`]): `send(dest, …)`
/// caches a separate ephemeral-bound `UdpSocket` per destination
/// address, bound via `connect()` to exactly that target. Follow-up
/// sends to the same address use `socket.send(data)` instead of
/// `send_to(data, addr)` — saving the kernel route lookup per send.
/// The cache has FIFO eviction on overflow (default 16 entries) and an
/// optional explicit `SO_SNDBUF` so the ephemeral sockets do not get a
/// smaller buffer than the listener socket.
///
/// The source port of the connected sockets is ephemeral
/// (kernel-assigned). Reader reverse replies (ACKNACK) go to the writer
/// locator announced by the PublicationData, not back to the source
/// port of the individual datagram — so the connected cache is
/// transparent to the reliable protocol.
///
/// **Telemetry**: `cache_hits`/`cache_misses`/`cache_evictions` are
/// exposed via `zerodds-monitor` (see
/// `udp_counters`). Callers can use them to
/// check whether the cache hit rate is high enough to justify the win —
/// with many short-lived locators (e.g. SPDP multicast discovery)
/// misses dominate and the cache becomes overhead.
#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
    local_locator: Locator,
    /// `None` if `connected_cache_enabled=false` — send then goes
    /// classically via `socket.send_to(data, addr)`.
    connected_cache: Option<std::sync::RwLock<ConnectedSocketCache>>,
    /// One send socket per TRANSPORT_PRIORITY value a writer used, its
    /// IPv4 TOS or IPv6 Traffic Class set to the value
    /// ([`Transport::send_with_priority`]); bound to this transport's
    /// address, made on first use.
    priority_sockets: std::sync::Mutex<alloc::collections::BTreeMap<(bool, u8), UdpSocket>>,
    /// [`UdpTransportConfig::max_message_size`].
    max_message_size: usize,
}

impl UdpTransport {
    /// Access to the underlying `UdpSocket`.
    ///
    /// Mainly for Linux-specific `recvmmsg` batch-recv paths (Opt-2,
    /// feature `recvmmsg-batch`). General users should use the
    /// `Transport` trait methods.
    #[must_use]
    pub fn std_socket(&self) -> &UdpSocket {
        &self.socket
    }

    /// Opt-5 (spec `zerodds-zero-copy-1.0` §9): scatter-gather send
    /// via `sendmsg(iovec)`. Sends multiple buffer segments as ONE
    /// datagram, without first copying them into a single vec. Useful
    /// for the encap-header + payload constellation (W2 optimization).
    ///
    /// Active only with the `sendmsg-iovec` variant of the
    /// `recvmmsg-batch` feature (re-uses the libc dep). On non-Linux the
    /// method falls back to a `chain-collect + send_to` loop —
    /// functionally identical, without the syscall win.
    ///
    /// # Errors
    /// [`SendError`] analogous to `send`.
    pub fn send_iovec(&self, dest: &Locator, segments: &[&[u8]]) -> Result<(), SendError> {
        if dest.kind != LocatorKind::UdpV4 {
            return Err(SendError::UnsupportedLocator);
        }
        let total: usize = segments.iter().map(|s| s.len()).sum();
        if total > MAX_DATAGRAM_SIZE {
            return Err(SendError::PayloadTooLarge {
                size: total,
                limit: MAX_DATAGRAM_SIZE,
            });
        }
        #[cfg(all(feature = "recvmmsg-batch", target_os = "linux"))]
        {
            self.send_iovec_linux(dest, segments)
        }
        // Fallback (non-Linux or feature off): copy into a single vec
        // and send classically. Functionally identical.
        #[cfg(not(all(feature = "recvmmsg-batch", target_os = "linux")))]
        {
            let mut combined: Vec<u8> = Vec::with_capacity(total);
            for s in segments {
                combined.extend_from_slice(s);
            }
            self.send(dest, &combined)
        }
    }

    /// Linux specifics for `send_iovec` via the `sendmsg(2)` syscall.
    ///
    /// Feature-gated unsafe island; the crate-level `deny(unsafe_code)`
    /// (with `recvmmsg-batch` active) is locally bridged with a SAFETY
    /// comment per block.
    #[cfg(all(feature = "recvmmsg-batch", target_os = "linux"))]
    #[allow(unsafe_code)]
    fn send_iovec_linux(&self, dest: &Locator, segments: &[&[u8]]) -> Result<(), SendError> {
        use std::os::fd::AsRawFd;
        let ip = [
            dest.address[12],
            dest.address[13],
            dest.address[14],
            dest.address[15],
        ];
        let port = u16::try_from(dest.port).map_err(|_| SendError::Io {
            message: "udp port overflow",
        })?;
        // SAFETY: `sockaddr_in` is POD; zeroed() yields a valid
        // all-zero AF_UNSPEC; all fields are set directly afterwards.
        let mut sa: libc::sockaddr_in = unsafe { core::mem::zeroed() };
        sa.sin_family = libc::AF_INET as libc::sa_family_t;
        sa.sin_port = port.to_be();
        sa.sin_addr.s_addr = u32::from_be_bytes(ip).to_be();
        let iovecs: Vec<libc::iovec> = segments
            .iter()
            .map(|s| libc::iovec {
                iov_base: s.as_ptr() as *mut libc::c_void,
                iov_len: s.len(),
            })
            .collect();
        // SAFETY: `msghdr` is POD; all-zero is a header without control
        // data and flags. It is filled field by field: musl's `msghdr` has
        // private padding fields and declares `msg_iovlen` as `c_int`,
        // glibc's as `size_t`.
        let mut hdr: libc::msghdr = unsafe { core::mem::zeroed() };
        hdr.msg_name = &mut sa as *mut _ as *mut libc::c_void;
        hdr.msg_namelen = core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
        hdr.msg_iov = iovecs.as_ptr() as *mut libc::iovec;
        hdr.msg_iovlen = iovecs.len() as _;
        // SAFETY: the socket fd is a valid Linux fd; hdr references
        // sa (lives in the stack frame), iovecs (lives in the vec) and
        // the referenced segments (caller lifetime).
        let sent = unsafe { libc::sendmsg(self.socket.as_raw_fd(), &hdr, 0) };
        if sent < 0 {
            udp_counters().send_errors.inc();
            return Err(SendError::Io {
                message: "udp sendmsg failed",
            });
        }
        let counters = udp_counters();
        counters.packets_sent.inc();
        counters.bytes_sent.add(sent as u64);
        Ok(())
    }
}

/// Send buffer a bound socket gets at least: room for the largest datagram.
/// macOS sizes a UDP socket's send buffer to `net.inet.udp.maxdgram`
/// (9 216 bytes) and refuses a larger datagram with EMSGSIZE; the connected
/// cache's sockets had 256 KiB, the bound socket, which sends when the cache
/// is off, had the default, and every datagram over 9 216 bytes failed.
const MIN_SEND_BUFFER: usize = 256 * 1024;

/// SO_RCVBUF, SO_SNDBUF and SO_DONTROUTE from `cfg` on a freshly bound
/// socket. Best-effort: the kernel may clamp a buffer (Linux halves it
/// against `rmem_max`), which is not a bind error.
fn apply_socket_options(socket: &UdpSocket, cfg: &UdpTransportConfig) {
    let sock_ref = socket2::SockRef::from(socket);
    if let Some(rcv) = cfg.recv_buffer_size {
        let _ = sock_ref.set_recv_buffer_size(rcv);
    }
    if sock_ref
        .send_buffer_size()
        .is_ok_and(|current| current < MIN_SEND_BUFFER)
    {
        let _ = sock_ref.set_send_buffer_size(MIN_SEND_BUFFER);
    }
    // SO_DONTROUTE (Cyclone sets this by default, ddsi_udp.c line 406):
    // saves the per-send kernel routing-table lookup when the target is
    // directly reachable (loopback, local LAN). With cross-subnet routing
    // it must stay off.
    #[cfg(unix)]
    if cfg.dont_route {
        use std::os::fd::AsRawFd;
        let one: libc::c_int = 1;
        #[allow(unsafe_code)]
        // SAFETY: setsockopt(SO_DONTROUTE) on a valid owned socket
        // (`socket.as_raw_fd()`); `&one` is valid for the call duration,
        // size_of_val is consistent with the int type. Errno is discarded
        // (best-effort; on failure the routing default stays active).
        let _ = unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_DONTROUTE,
                &one as *const _ as *const libc::c_void,
                core::mem::size_of_val(&one) as libc::socklen_t,
            )
        };
    }
}

/// The connected-socket cache `cfg` asks for.
fn connected_cache_for(
    cfg: &UdpTransportConfig,
) -> Option<std::sync::RwLock<ConnectedSocketCache>> {
    cfg.connected_cache_enabled.then(|| {
        std::sync::RwLock::new(ConnectedSocketCache::new(
            cfg.connected_cache_max_entries,
            cfg.connected_cache_sndbuf,
        ))
    })
}

impl UdpTransport {
    /// Binds to the given IPv4 address + port with the default config
    /// (connected cache on, 16 entries, 256 KB SO_SNDBUF).
    /// `port = 0` lets the OS choose a free port.
    ///
    /// # Errors
    /// `UdpTransportError::Bind`.
    pub fn bind_v4(addr: Ipv4Addr, port: u16) -> Result<Self, UdpTransportError> {
        Self::bind_v4_with_config(addr, port, UdpTransportConfig::default())
    }

    /// Like [`Self::bind_v4`] with an explicit [`UdpTransportConfig`].
    ///
    /// # Errors
    /// `UdpTransportError::Bind`.
    pub fn bind_v4_with_config(
        addr: Ipv4Addr,
        port: u16,
        cfg: UdpTransportConfig,
    ) -> Result<Self, UdpTransportError> {
        let bind_addr = SocketAddrV4::new(addr, port);
        let socket = UdpSocket::bind(bind_addr).map_err(UdpTransportError::Bind)?;
        // IP_MULTICAST_LOOP (sender-side option): one's own multicast
        // sends must also be seen on the local host so intra-process
        // discovery + self-match works. Linux default 1, but
        // containerized CI runners may have it at 0. Setting it
        // explicitly makes the behavior reproducible.
        let _ = socket.set_multicast_loop_v4(true);
        // SO_RCVBUF + SO_DONTROUTE analogous to Cyclone DDS (see ddsi_udp.c
        // line 517 default 1 MB RCVBUF + line 406 SO_DONTROUTE).
        apply_socket_options(&socket, &cfg);
        let local = match socket.local_addr().map_err(UdpTransportError::Bind)? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                // IPv6 should not occur on an IPv4 bind — defensive.
                return Err(UdpTransportError::Bind(std::io::Error::other(
                    "got V6 address on V4 bind",
                )));
            }
        };
        let local_locator = Locator::udp_v4(local.ip().octets(), u32::from(local.port()));
        let connected_cache = connected_cache_for(&cfg);
        Ok(Self {
            socket,
            local_locator,
            connected_cache,
            priority_sockets: std::sync::Mutex::new(alloc::collections::BTreeMap::new()),
            max_message_size: cfg.max_message_size,
        })
    }

    /// Opt-3 (spec `zerodds-zero-copy-1.0` §9): binds to `addr:port`
    /// with `SO_REUSEADDR + SO_REUSEPORT` so multiple sockets can share
    /// the same port. The kernel distributes incoming datagrams across
    /// all bound sockets by flow hash (src IP/src port) — load
    /// balancing for multi-thread recv pools.
    ///
    /// `port` must be explicit (not 0), otherwise each socket gets a
    /// different one from the kernel; that is not a pool.
    ///
    /// # Errors
    /// [`UdpTransportError::Bind`] on bind or setsockopt error.
    pub fn bind_v4_reuse(addr: Ipv4Addr, port: u16) -> Result<Self, UdpTransportError> {
        Self::bind_v4_reuse_with_config(addr, port, UdpTransportConfig::default())
    }

    /// Like [`Self::bind_v4_reuse`] with an explicit [`UdpTransportConfig`].
    ///
    /// # Errors
    /// [`UdpTransportError::Bind`] on bind or setsockopt error.
    pub fn bind_v4_reuse_with_config(
        addr: Ipv4Addr,
        port: u16,
        cfg: UdpTransportConfig,
    ) -> Result<Self, UdpTransportError> {
        use socket2::{Domain, Protocol, SockAddr, Socket, Type};
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .map_err(UdpTransportError::Bind)?;
        socket
            .set_reuse_address(true)
            .map_err(UdpTransportError::Bind)?;
        // SO_REUSEPORT is Linux/macOS/BSD; on Windows only SO_REUSEADDR
        // exists (the above is enough there for multi-bind).
        #[cfg(unix)]
        socket
            .set_reuse_port(true)
            .map_err(UdpTransportError::Bind)?;
        let bind_addr: SockAddr = SocketAddrV4::new(addr, port).into();
        socket.bind(&bind_addr).map_err(UdpTransportError::Bind)?;
        let std_sock: UdpSocket = socket.into();
        apply_socket_options(&std_sock, &cfg);
        let local = match std_sock.local_addr().map_err(UdpTransportError::Bind)? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                return Err(UdpTransportError::Bind(std::io::Error::other(
                    "got V6 address on V4 reuse-bind",
                )));
            }
        };
        let local_locator = Locator::udp_v4(local.ip().octets(), u32::from(local.port()));
        let connected_cache = connected_cache_for(&cfg);
        Ok(Self {
            socket: std_sock,
            local_locator,
            connected_cache,
            priority_sockets: std::sync::Mutex::new(alloc::collections::BTreeMap::new()),
            max_message_size: cfg.max_message_size,
        })
    }

    /// Binds a UDPv6 socket to `addr:port` with the default config.
    ///
    /// `addr = ::` (UNSPECIFIED) for all-interface, `::1` for loopback.
    /// `port = 0` lets the OS choose a free port.
    ///
    /// # Errors
    /// `UdpTransportError::Bind`.
    pub fn bind_v6(addr: Ipv6Addr, port: u16) -> Result<Self, UdpTransportError> {
        Self::bind_v6_with_config(addr, port, UdpTransportConfig::default())
    }

    /// Like [`Self::bind_v6`] with an explicit [`UdpTransportConfig`].
    ///
    /// # Errors
    /// `UdpTransportError::Bind`.
    pub fn bind_v6_with_config(
        addr: Ipv6Addr,
        port: u16,
        cfg: UdpTransportConfig,
    ) -> Result<Self, UdpTransportError> {
        let bind_addr = SocketAddrV6::new(addr, port, 0, 0);
        let socket = UdpSocket::bind(bind_addr).map_err(UdpTransportError::Bind)?;
        // IPv6 multicast: hopcount default 1 (link-local). Reproducible.
        let _ = socket.set_multicast_loop_v6(true);
        apply_socket_options(&socket, &cfg);
        let local = match socket.local_addr().map_err(UdpTransportError::Bind)? {
            SocketAddr::V6(v6) => v6,
            SocketAddr::V4(_) => {
                return Err(UdpTransportError::Bind(std::io::Error::other(
                    "got V4 address on V6 bind",
                )));
            }
        };
        let local_locator = Locator::udp_v6(local.ip().octets(), u32::from(local.port()));
        let connected_cache = connected_cache_for(&cfg);
        Ok(Self {
            socket,
            local_locator,
            connected_cache,
            priority_sockets: std::sync::Mutex::new(alloc::collections::BTreeMap::new()),
            max_message_size: cfg.max_message_size,
        })
    }

    /// Sets the receive timeout. `None` means blocking until a datagram
    /// arrives.
    ///
    /// # Errors
    /// `UdpTransportError::SetTimeout`.
    pub fn with_timeout(self, timeout: Option<Duration>) -> Result<Self, UdpTransportError> {
        self.socket
            .set_read_timeout(timeout)
            .map_err(UdpTransportError::SetTimeout)?;
        Ok(self)
    }

    /// Configures the socket as a multicast receiver: binds to
    /// `0.0.0.0:port` with `SO_REUSEADDR`+`SO_REUSEPORT` and joins the
    /// multicast group. SO_REUSE_* allows multiple processes on the
    /// same multicast port (e.g. ZeroDDS + Cyclone in parallel).
    ///
    /// `interface = 0.0.0.0` lets the kernel choose the default
    /// interface (often loopback). For real discovery between processes
    /// on the same host you must give the concrete IP of the network
    /// interface (e.g. `192.168.1.10`).
    ///
    /// # Errors
    /// `UdpTransportError::Bind` on bind/multicast-join error.
    ///
    /// On `EADDRINUSE`/`EADDRNOTAVAIL` the method retries up to three
    /// times with backoff (100/300/700 ms). Background: in CI tests
    /// that sequentially create and drop DomainParticipants in the same
    /// domain, the multicast-membership cleanup latency in the kernel
    /// is non-deterministic — the subsequent bind can transiently get
    /// EADDRINUSE even when `SO_REUSEADDR` is set (especially near
    /// `IP_MAX_MEMBERSHIPS` utilization). The retry loop makes the
    /// clearly transient race disappear, without pulling in permanent
    /// sleep pauses.
    pub fn bind_multicast_v4(
        group: Ipv4Addr,
        port: u16,
        interface: Ipv4Addr,
    ) -> Result<Self, UdpTransportError> {
        Self::bind_multicast_v4_with_config(group, port, interface, UdpTransportConfig::default())
    }

    /// Like [`Self::bind_multicast_v4`] with an explicit
    /// [`UdpTransportConfig`].
    ///
    /// # Errors
    /// As [`Self::bind_multicast_v4`].
    pub fn bind_multicast_v4_with_config(
        group: Ipv4Addr,
        port: u16,
        interface: Ipv4Addr,
        cfg: UdpTransportConfig,
    ) -> Result<Self, UdpTransportError> {
        Self::bind_multicast_v4_on_with_config(group, port, &[interface], cfg)
            .map(|(transport, _)| transport)
    }

    /// Like [`Self::bind_multicast_v4_with_config`], with `group` joined on
    /// every interface address of `interfaces`: one membership per
    /// interface, so a datagram to the group is received whichever of them
    /// it arrives on. With one interface the kernel would pick by route,
    /// and on a host with a LAN and a container bridge it picked the LAN:
    /// a participant in a container on the bridge was never heard.
    ///
    /// A join that fails is skipped, and the returned list names the
    /// interface addresses whose join succeeded, in the order given.
    ///
    /// # Errors
    /// `UdpTransportError::Bind` on the bind, or when no join succeeded
    /// (the last join error). The bind is retried as in
    /// [`Self::bind_multicast_v4`].
    pub fn bind_multicast_v4_on_with_config(
        group: Ipv4Addr,
        port: u16,
        interfaces: &[Ipv4Addr],
        cfg: UdpTransportConfig,
    ) -> Result<(Self, Vec<Ipv4Addr>), UdpTransportError> {
        const RETRY_BACKOFF_MS: &[u64] = &[100, 300, 700];

        let mut last_err: Option<std::io::Error> = None;
        for (attempt, backoff_ms) in core::iter::once(0)
            .chain(RETRY_BACKOFF_MS.iter().copied())
            .enumerate()
        {
            if attempt > 0 {
                std::thread::sleep(core::time::Duration::from_millis(backoff_ms));
            }
            match Self::try_bind_multicast_v4(group, port, interfaces, &cfg) {
                Ok(t) => return Ok(t),
                Err(UdpTransportError::Bind(e)) => {
                    let kind = e.kind();
                    let retryable = matches!(
                        kind,
                        std::io::ErrorKind::AddrInUse | std::io::ErrorKind::AddrNotAvailable,
                    );
                    if !retryable {
                        return Err(UdpTransportError::Bind(e));
                    }
                    last_err = Some(e);
                }
                // try_bind_multicast_v4 only produces bind errors —
                // SetTimeout comes later via with_timeout(). Pass other
                // variants straight through.
                Err(other) => return Err(other),
            }
        }
        // All retries exhausted.
        Err(UdpTransportError::Bind(last_err.unwrap_or_else(|| {
            std::io::Error::other("multicast bind retries exhausted")
        })))
    }

    fn try_bind_multicast_v4(
        group: Ipv4Addr,
        port: u16,
        interfaces: &[Ipv4Addr],
        cfg: &UdpTransportConfig,
    ) -> Result<(Self, Vec<Ipv4Addr>), UdpTransportError> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .map_err(UdpTransportError::Bind)?;
        socket
            .set_reuse_address(true)
            .map_err(UdpTransportError::Bind)?;
        #[cfg(unix)]
        socket
            .set_reuse_port(true)
            .map_err(UdpTransportError::Bind)?;
        // Set IP_MULTICAST_LOOP explicitly: intra-process discovery
        // needs one's own multicast sends to see themselves and other
        // sockets on the same host. The Linux default is 1, but
        // containerized CI runners (GitHub Actions VMs) may have it at
        // 0 — then intra-process DCPS setup fails with discovery
        // timeouts. Default 1 here makes it deterministic.
        socket
            .set_multicast_loop_v4(true)
            .map_err(UdpTransportError::Bind)?;
        let bind_addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into();
        socket
            .bind(&bind_addr.into())
            .map_err(UdpTransportError::Bind)?;
        let mut joined = Vec::with_capacity(interfaces.len());
        let mut join_err: Option<std::io::Error> = None;
        for interface in interfaces {
            match socket.join_multicast_v4(&group, interface) {
                Ok(()) => joined.push(*interface),
                Err(e) => join_err = Some(e),
            }
        }
        if joined.is_empty() {
            return Err(UdpTransportError::Bind(join_err.unwrap_or_else(|| {
                std::io::Error::other("no interface to join the multicast group on")
            })));
        }
        // ip(7), IP_MULTICAST_ALL: at its default of 1, Linux gives a
        // socket bound to INADDR_ANY:port every datagram for that port of
        // every group any socket on the host joined. A participant with a
        // group of its own would still hear, and discover, the
        // participants of another group on the same port.
        #[cfg(target_os = "linux")]
        socket
            .set_multicast_all_v4(false)
            .map_err(UdpTransportError::Bind)?;
        let socket: UdpSocket = socket.into();
        apply_socket_options(&socket, cfg);
        let local = match socket.local_addr().map_err(UdpTransportError::Bind)? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                return Err(UdpTransportError::Bind(std::io::Error::other(
                    "got V6 address on V4 multicast bind",
                )));
            }
        };
        // The local locator points to the multicast group, not to
        // 0.0.0.0 — the caller's view is "I receive on the group".
        let local_locator = Locator::udp_v4(group.octets(), u32::from(local.port()));
        // Multicast receiver sockets normally do not use the send path,
        // but take the same config for consistency.
        let connected_cache = connected_cache_for(cfg);
        Ok((
            Self {
                socket,
                local_locator,
                connected_cache,
                priority_sockets: std::sync::Mutex::new(alloc::collections::BTreeMap::new()),
                max_message_size: cfg.max_message_size,
            },
            joined,
        ))
    }

    /// A sender of multicast through one interface: bound to
    /// `interface:0`, with `IP_MULTICAST_IF` set to it, so that the kernel
    /// does not choose the outgoing interface by route (ip(7)). It sends
    /// without the connected-socket cache, whose sockets would not carry
    /// the option. `IP_MULTICAST_LOOP` is on, as for every sender here.
    ///
    /// # Errors
    /// `UdpTransportError::Bind` on the socket, an option or the bind.
    pub fn bind_multicast_sender_v4_with_config(
        interface: Ipv4Addr,
        cfg: UdpTransportConfig,
    ) -> Result<Self, UdpTransportError> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .map_err(UdpTransportError::Bind)?;
        socket
            .set_multicast_if_v4(&interface)
            .map_err(UdpTransportError::Bind)?;
        socket
            .set_multicast_loop_v4(true)
            .map_err(UdpTransportError::Bind)?;
        let bind_addr: SocketAddr = SocketAddrV4::new(interface, 0).into();
        socket
            .bind(&bind_addr.into())
            .map_err(UdpTransportError::Bind)?;
        let socket: UdpSocket = socket.into();
        apply_socket_options(&socket, &cfg);
        let local = match socket.local_addr().map_err(UdpTransportError::Bind)? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                return Err(UdpTransportError::Bind(std::io::Error::other(
                    "got V6 address on V4 bind",
                )));
            }
        };
        Ok(Self {
            socket,
            local_locator: Locator::udp_v4(local.ip().octets(), u32::from(local.port())),
            connected_cache: None,
            priority_sockets: std::sync::Mutex::new(alloc::collections::BTreeMap::new()),
            max_message_size: cfg.max_message_size,
        })
    }

    /// Sets the multicast TTL for outgoing multicast packets.
    /// The default is usually 1 (local subnet only). 32 = local site
    /// (RFC-2365); 255 = global.
    ///
    /// # Errors
    /// `UdpTransportError::SetTimeout` (re-used).
    pub fn set_multicast_ttl(self, ttl: u32) -> Result<Self, UdpTransportError> {
        self.socket
            .set_multicast_ttl_v4(ttl)
            .map_err(UdpTransportError::SetTimeout)?;
        Ok(self)
    }
}

impl UdpTransport {
    /// The send socket for TRANSPORT_PRIORITY `tos` towards an IPv4 or
    /// IPv6 destination, made on first use: bound to this transport's
    /// address (a peer sees the same source address as for the transport's
    /// other datagrams) with `IP_TOS`, or `IPV6_TCLASS`, set to `tos`.
    fn priority_socket(&self, v6: bool, tos: u8) -> std::io::Result<UdpSocket> {
        let mut sockets = self
            .priority_sockets
            .lock()
            .map_err(|_| std::io::Error::other("priority sockets poisoned"))?;
        if let Some(socket) = sockets.get(&(v6, tos)) {
            return socket.try_clone();
        }
        let local_ip = self.socket.local_addr()?.ip();
        let socket = if v6 {
            let socket = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?;
            set_traffic_class(&socket, tos)?;
            let bind_ip = if local_ip.is_ipv6() {
                local_ip
            } else {
                std::net::IpAddr::V6(Ipv6Addr::UNSPECIFIED)
            };
            socket.bind(&SocketAddr::new(bind_ip, 0).into())?;
            socket
        } else {
            let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
            socket.set_tos(u32::from(tos))?;
            let bind_ip = if local_ip.is_ipv4() {
                local_ip
            } else {
                std::net::IpAddr::V4(Ipv4Addr::UNSPECIFIED)
            };
            socket.bind(&SocketAddr::new(bind_ip, 0).into())?;
            socket
        };
        let socket: UdpSocket = socket.into();
        let handle = socket.try_clone()?;
        sockets.insert((v6, tos), socket);
        Ok(handle)
    }
}

/// `IPV6_TCLASS` of `socket`. socket2 offers it on Unix only.
#[cfg(unix)]
fn set_traffic_class(socket: &Socket, tclass: u8) -> std::io::Result<()> {
    socket.set_tclass_v6(u32::from(tclass))
}

/// See the Unix variant; elsewhere there is no traffic class to set.
#[cfg(not(unix))]
fn set_traffic_class(_socket: &Socket, _tclass: u8) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "no IPv6 traffic class on this platform",
    ))
}

impl Transport for UdpTransport {
    /// [`UdpTransportConfig::max_message_size`], at most
    /// [`MAX_DATAGRAM_SIZE`].
    fn max_message_size(&self) -> usize {
        self.max_message_size.min(MAX_DATAGRAM_SIZE)
    }

    /// Closes the connected socket kept for `dest`. Entries otherwise left
    /// only by FIFO eviction, so a process kept a socket, and a file
    /// descriptor, per destination it ever sent to, up to the cache size:
    /// 105 to 187 per process in a multi-process test chain at a cache size of 128.
    fn forget_destination(&self, dest: &Locator) {
        let Some(cache_lock) = &self.connected_cache else {
            return;
        };
        let Ok(port) = u16::try_from(dest.port) else {
            return;
        };
        let addr = match dest.kind {
            LocatorKind::UdpV4 => {
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(dest.ipv4()), port))
            }
            LocatorKind::UdpV6 => {
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(dest.address), port, 0, 0))
            }
            _ => return,
        };
        if let Ok(mut cache) = cache_lock.write() {
            cache.remove(&addr);
        }
    }

    /// TRANSPORT_PRIORITY (DDS 1.4 §2.2.3.15) as the IP header's
    /// priority: the value, 1 to 255, is written as the IPv4 TOS octet
    /// or the IPv6 Traffic Class, as RTI Connext's UDP transport does;
    /// a value above 255 is 255, `0` and below is no priority. The
    /// datagram goes out of a socket of its own for that value
    /// (`Self::priority_socket`); where the platform cannot set one,
    /// it goes out as [`Transport::send`] sends it.
    fn send_with_priority(
        &self,
        dest: &Locator,
        data: &[u8],
        priority: i32,
    ) -> Result<(), SendError> {
        if priority <= 0 {
            return self.send(dest, data);
        }
        let tos = u8::try_from(priority).unwrap_or(u8::MAX);
        let port = u16::try_from(dest.port).map_err(|_| SendError::UnsupportedLocator)?;
        let (v6, addr): (bool, SocketAddr) = match dest.kind {
            LocatorKind::UdpV4 => (
                false,
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(dest.ipv4()), port)),
            ),
            LocatorKind::UdpV6 => (
                true,
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(dest.address), port, 0, 0)),
            ),
            _ => return Err(SendError::UnsupportedLocator),
        };
        if data.len() > MAX_DATAGRAM_SIZE {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: MAX_DATAGRAM_SIZE,
            });
        }
        let Ok(socket) = self.priority_socket(v6, tos) else {
            return self.send(dest, data);
        };
        let counters = udp_counters();
        socket
            .send_to(data, addr)
            .map(|_| {
                counters.packets_sent.inc();
                counters.bytes_sent.add(data.len() as u64);
                #[cfg(feature = "inspect")]
                dispatch_transport_tap("udp:send", data);
            })
            .map_err(|_| {
                counters.send_errors.inc();
                SendError::Io {
                    message: "udp send_to (priority socket) failed",
                }
            })
    }

    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        let port = u16::try_from(dest.port).map_err(|_| SendError::UnsupportedLocator)?;
        let addr: SocketAddr = match dest.kind {
            LocatorKind::UdpV4 => {
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(dest.ipv4()), port))
            }
            LocatorKind::UdpV6 => {
                // Locator::address is [u8; 16] in network byte order (BE).
                let octets: [u8; 16] = dest.address;
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(octets), port, 0, 0))
            }
            _ => return Err(SendError::UnsupportedLocator),
        };
        if data.len() > MAX_DATAGRAM_SIZE {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: MAX_DATAGRAM_SIZE,
            });
        }
        let counters = udp_counters();

        // Option-D: cache fully off → classic send_to(data, addr).
        // No cache lookup, no ephemeral socket. Safe fallback path
        // when the caller prioritizes jitter over latency.
        let Some(cache_lock) = &self.connected_cache else {
            return self
                .socket
                .send_to(data, addr)
                .map(|_| {
                    counters.packets_sent.inc();
                    counters.bytes_sent.add(data.len() as u64);
                    #[cfg(feature = "inspect")]
                    dispatch_transport_tap("udp:send", data);
                })
                .map_err(|_| {
                    counters.send_errors.inc();
                    SendError::Io {
                        message: "udp send_to (cache disabled) failed",
                    }
                });
        };

        // Fast path: connected-cache hit. `send()` on a connected UDP
        // socket saves the per-send kernel route lookup (~1.18 µs
        // measured on macOS loopback, ~0.5 µs on Linux).
        if let Ok(cache) = cache_lock.read() {
            if let Some(sock) = cache.get(&addr) {
                return match sock.send(data) {
                    Ok(_) => {
                        counters.packets_sent.inc();
                        counters.bytes_sent.add(data.len() as u64);
                        counters.cache_hits.inc();
                        #[cfg(feature = "inspect")]
                        dispatch_transport_tap("udp:send", data);
                        Ok(())
                    }
                    Err(_) => {
                        counters.send_errors.inc();
                        Err(SendError::Io {
                            message: "udp send (connected) failed",
                        })
                    }
                };
            }
        }

        // Slow path: first send to this target. Bind+connect+send,
        // then cache insert with FIFO eviction on overflow.
        // Race-tolerant: two threads that miss in parallel each create
        // a socket, the second wins in the cache.
        counters.cache_misses.inc();
        // Bind an ephemeral source socket: the family must match the
        // dest (v4 or v6), else connect() cannot reach the target.
        let bind_addr: SocketAddr = match addr {
            SocketAddr::V4(_) => SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
            SocketAddr::V6(_) => SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0)),
        };
        let new_sock = UdpSocket::bind(bind_addr).map_err(|_| SendError::Io {
            message: "udp connect-cache bind failed",
        })?;
        // Set SO_SNDBUF from the cache config (Option-B): if `None`,
        // the kernel default stays. If `Some(n)`, the send buffer is
        // set to n bytes — jitter mitigation against the small
        // ephemeral-socket default-buffer problem (the Linux test host
        // bench 2026-05-25). Errors here are non-fatal (best effort).
        if let Ok(cache_ro) = cache_lock.read() {
            if let Some(sndbuf) = cache_ro.sndbuf_bytes {
                let sock_ref = socket2::SockRef::from(&new_sock);
                let _ = sock_ref.set_send_buffer_size(sndbuf);
            }
        }
        new_sock.connect(addr).map_err(|_| SendError::Io {
            message: "udp connect failed",
        })?;
        new_sock.send(data).map_err(|_| {
            counters.send_errors.inc();
            SendError::Io {
                message: "udp send (connect-cache miss) failed",
            }
        })?;
        if let Ok(mut cache) = cache_lock.write() {
            if cache.insert(addr, new_sock) {
                counters.cache_evictions.inc();
            }
        }
        counters.packets_sent.inc();
        counters.bytes_sent.add(data.len() as u64);
        #[cfg(feature = "inspect")]
        dispatch_transport_tap("udp:send", data);
        Ok(())
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        let (len, peer) = self.socket.recv_from(&mut buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut
            {
                RecvError::Timeout
            } else {
                RecvError::Io {
                    message: "udp recv_from failed",
                }
            }
        })?;
        let counters = udp_counters();
        counters.packets_received.inc();
        counters.bytes_received.add(len as u64);
        let source = match peer {
            SocketAddr::V4(v4) => Locator::udp_v4(v4.ip().octets(), u32::from(v4.port())),
            SocketAddr::V6(v6) => Locator::udp_v6(v6.ip().octets(), u32::from(v6.port())),
        };
        // Zero-copy path: Arc::from(&[u8]) creates a refcounted slice
        // that downstream consumers can share without further copies.
        let data: Arc<[u8]> = Arc::from(&buf[..len]);
        #[cfg(feature = "inspect")]
        dispatch_transport_tap("udp:recv", &data);
        Ok(ReceivedDatagram { source, data })
    }

    /// Linux `recvmmsg` batch recv (Opt-2, spec `zerodds-zero-copy-1.0` §9):
    /// up to [`crate::recv_batch::DEFAULT_BATCH_SIZE`] datagrams per syscall.
    ///
    /// This override is what makes the batch path reachable at all. The DCPS
    /// receive loop holds an `Arc<dyn Transport>`, so it can never call an
    /// inherent method on the concrete socket — for a while the loop carried a
    /// comment claiming it used the batch path and a `socket.recv()` call that
    /// did not, and enabling the feature changed nothing.
    ///
    /// Without the feature, or off Linux, the trait default applies: one
    /// blocking [`Transport::recv`].
    #[cfg(all(feature = "recvmmsg-batch", target_os = "linux"))]
    fn recv_batch(&self, out: &mut Vec<ReceivedDatagram>, max: usize) -> Result<usize, RecvError> {
        if max == 0 {
            return Ok(0);
        }
        let n = crate::recv_batch::recv_batch_linux(&self.socket, out, max)?;
        if n == 0 {
            // `recvmmsg` ran with `MSG_DONTWAIT`, so an empty queue answers
            // immediately. Handing that back to the caller turns its loop into
            // a busy spin; the blocking single `recv()` is what makes this
            // method wait the way `Transport::recv` promises, honouring the
            // socket timeout the caller configured.
            return match self.recv() {
                Ok(dg) => {
                    out.push(dg);
                    Ok(1)
                }
                Err(RecvError::Timeout) => Ok(0),
                Err(e) => Err(e),
            };
        }
        let counters = udp_counters();
        counters.packets_received.add(n as u64);
        for dg in out.iter().rev().take(n) {
            counters.bytes_received.add(dg.data.len() as u64);
            #[cfg(feature = "inspect")]
            dispatch_transport_tap("udp:recv", &dg.data);
        }
        Ok(n)
    }

    fn local_locator(&self) -> Locator {
        self.local_locator
    }
}

#[cfg(feature = "inspect")]
fn dispatch_transport_tap(label: &str, data: &[u8]) {
    let ts_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    let mut frame = zerodds_inspect_endpoint::Frame::transport(ts_ns, 0, data.to_vec());
    frame.topic = label.to_owned();
    zerodds_inspect_endpoint::tap::dispatch(&frame);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stderr)]
    use super::*;

    fn make_loopback_pair() -> (UdpTransport, UdpTransport) {
        let a = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind a");
        let b = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind b");
        let a = a
            .with_timeout(Some(Duration::from_secs(2)))
            .expect("timeout a");
        let b = b
            .with_timeout(Some(Duration::from_secs(2)))
            .expect("timeout b");
        (a, b)
    }

    #[test]
    fn bind_v4_returns_local_locator_with_assigned_port() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        let loc = t.local_locator();
        assert_eq!(loc.kind, LocatorKind::UdpV4);
        assert_eq!(loc.ipv4(), [127, 0, 0, 1]);
        assert!(loc.port > 0, "OS should assign a non-zero port");
    }

    #[test]
    fn loopback_send_and_recv_delivers_datagram() {
        let (sender, receiver) = make_loopback_pair();
        let dest = receiver.local_locator();
        let payload = b"hello rtps";
        sender.send(&dest, payload).expect("send");
        let received = receiver.recv().expect("recv");
        assert_eq!(&received.data[..], payload);
        // Source locator: IP/family check; with the connected cache the
        // port is an ephemeral kernel-assigned source port, not the
        // sender's listener port. The spec is neutral here — reverse
        // routing (ACKNACK) goes to the SEDP-announced locators, not to
        // the source port of the datagram.
        assert_eq!(received.source.kind, LocatorKind::UdpV4);
        assert_eq!(received.source.ipv4(), [127, 0, 0, 1]);
        assert!(
            received.source.port > 0,
            "ephemeral source-port should be non-zero"
        );
    }

    /// TRANSPORT_PRIORITY (DDS 1.4 §2.2.3.15): a datagram sent with one
    /// arrives, from the sender's address, out of a socket whose IP TOS is
    /// the priority; the socket is made once per value; priority 0 is a
    /// plain send.
    #[test]
    fn a_priority_send_goes_out_of_a_socket_with_that_tos() {
        let (sender, receiver) = make_loopback_pair();
        let dest = receiver.local_locator();
        sender
            .send_with_priority(&dest, b"prioritised", 0x28)
            .expect("send");
        let received = receiver.recv().expect("recv");
        assert_eq!(&received.data[..], b"prioritised");
        assert_eq!(received.source.ipv4(), [127, 0, 0, 1]);
        let socket = sender.priority_socket(false, 0x28).expect("the socket");
        assert_eq!(socket2::SockRef::from(&socket).tos().expect("tos"), 0x28);
        sender
            .send_with_priority(&dest, b"again", 0x28)
            .expect("send");
        assert_eq!(&receiver.recv().expect("recv").data[..], b"again");
        assert_eq!(sender.priority_sockets.lock().expect("lock").len(), 1);
        sender.send_with_priority(&dest, b"none", 0).expect("send");
        assert_eq!(&receiver.recv().expect("recv").data[..], b"none");
        assert_eq!(sender.priority_sockets.lock().expect("lock").len(), 1);
    }

    #[test]
    fn disabled_cache_uses_listener_source_port() {
        // With cache_disabled() send() runs over socket.send_to() —
        // the source port is the sender's LISTENER port (not an
        // ephemeral cache-socket port). The test verifies that
        // (a) the disabled path stays functional and (b) the source-
        // port visibility is clear for tooling/PCAP diff.
        let sender = UdpTransport::bind_v4_with_config(
            Ipv4Addr::LOCALHOST,
            0,
            UdpTransportConfig::disabled(),
        )
        .expect("bind sender");
        let receiver = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind receiver");
        sender
            .send(&receiver.local_locator(), b"no-cache")
            .expect("send");
        let received = receiver.recv().expect("recv");
        assert_eq!(&received.data[..], b"no-cache");
        // With the cache disabled, source port MUST == sender listener port.
        assert_eq!(received.source.port, sender.local_locator().port);
    }

    /// With the connected cache off the bound socket sends, and a datagram
    /// of the largest size arrives whole: macOS sizes a UDP socket's send
    /// buffer to `net.inet.udp.maxdgram` (9 216 bytes) and refuses anything
    /// larger with EMSGSIZE, so every larger datagram failed (a multi-process test chain:
    /// the TRANSIENT_LOCAL contracts never arrived).
    #[test]
    fn without_the_cache_a_largest_datagram_arrives() {
        let cfg = UdpTransportConfig {
            connected_cache_enabled: false,
            ..UdpTransportConfig::default()
        };
        let sender =
            UdpTransport::bind_v4_with_config(Ipv4Addr::LOCALHOST, 0, cfg).expect("bind sender");
        let receiver = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .expect("bind receiver")
            .with_timeout(Some(core::time::Duration::from_secs(2)))
            .expect("timeout");
        let big = vec![0x5Au8; MAX_DATAGRAM_SIZE];
        sender
            .send(&receiver.local_locator(), &big)
            .expect("the largest datagram is sent");
        let got = receiver.recv().expect("and received");
        assert_eq!(got.data.len(), MAX_DATAGRAM_SIZE);
    }

    /// A destination whose peer is gone loses its connected socket; a
    /// later send to it makes one again.
    #[test]
    fn forget_destination_closes_its_cached_socket() {
        let sender = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind sender");
        let r1 = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("r1");
        let r2 = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("r2");
        sender.send(&r1.local_locator(), b"a").expect("send r1");
        sender.send(&r2.local_locator(), b"b").expect("send r2");
        let entries = || {
            sender
                .connected_cache
                .as_ref()
                .expect("cache enabled")
                .read()
                .expect("read")
                .entries
                .len()
        };
        assert_eq!(entries(), 2);
        sender.forget_destination(&r1.local_locator());
        assert_eq!(entries(), 1, "r1's socket is closed");
        sender.forget_destination(&r1.local_locator());
        assert_eq!(entries(), 1, "forgetting twice is harmless");
        sender
            .send(&r1.local_locator(), b"c")
            .expect("send r1 again");
        assert_eq!(entries(), 2);
    }

    #[test]
    fn cache_fifo_evicts_oldest_at_overflow() {
        // FIFO eviction verification: cache with max_entries=2,
        // then serve 3 different targets, the first must fly out
        // of the cache.
        let cfg = UdpTransportConfig {
            connected_cache_enabled: true,
            connected_cache_max_entries: 2,
            connected_cache_sndbuf: None,
            recv_buffer_size: None,
            dont_route: false,
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
        };
        let sender =
            UdpTransport::bind_v4_with_config(Ipv4Addr::LOCALHOST, 0, cfg).expect("bind sender");
        let r1 = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("r1");
        let r2 = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("r2");
        let r3 = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("r3");
        sender.send(&r1.local_locator(), b"a").expect("send r1");
        sender.send(&r2.local_locator(), b"b").expect("send r2");
        sender.send(&r3.local_locator(), b"c").expect("send r3");
        // Check cache state: r1 gone, r2+r3 present.
        let cache_lock = sender.connected_cache.as_ref().expect("cache enabled");
        let cache = cache_lock.read().expect("read");
        assert_eq!(cache.entries.len(), 2);
        let r1_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::from(r1.local_locator().ipv4()),
            u16::try_from(r1.local_locator().port).expect("port"),
        ));
        let r2_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::from(r2.local_locator().ipv4()),
            u16::try_from(r2.local_locator().port).expect("port"),
        ));
        let r3_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::from(r3.local_locator().ipv4()),
            u16::try_from(r3.local_locator().port).expect("port"),
        ));
        assert!(
            !cache.entries.contains_key(&r1_addr),
            "r1 should be evicted"
        );
        assert!(cache.entries.contains_key(&r2_addr), "r2 should be cached");
        assert!(cache.entries.contains_key(&r3_addr), "r3 should be cached");
    }

    #[test]
    fn send_rejects_non_udp_locator() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        let res = t.send(&Locator::INVALID, b"x");
        assert!(matches!(res, Err(SendError::UnsupportedLocator)));
    }

    // --- IPv6 ---

    #[test]
    fn bind_v6_loopback_returns_v6_locator() {
        let t = UdpTransport::bind_v6(Ipv6Addr::LOCALHOST, 0).expect("bind v6");
        let loc = t.local_locator();
        assert_eq!(loc.kind, LocatorKind::UdpV6);
        // ::1 = [0; 15] + [1]
        assert_eq!(loc.address[15], 1);
        assert!(loc.port > 0);
    }

    #[test]
    fn send_recv_v6_loopback() {
        let receiver = UdpTransport::bind_v6(Ipv6Addr::LOCALHOST, 0).expect("recv bind");
        let sender = UdpTransport::bind_v6(Ipv6Addr::LOCALHOST, 0).expect("send bind");
        receiver
            .std_socket()
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        sender
            .send(&receiver.local_locator(), b"hello-v6")
            .expect("send");
        let dg = receiver.recv().expect("recv");
        assert_eq!(&dg.data[..], b"hello-v6");
    }

    #[test]
    fn send_rejects_payload_above_max() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        let huge = vec![0u8; MAX_DATAGRAM_SIZE + 1];
        let res = t.send(&t.local_locator(), &huge);
        assert!(matches!(res, Err(SendError::PayloadTooLarge { .. })));
    }

    /// The decisive test for the batch path: it calls `recv_batch`
    /// **through the trait**, which is the only way the DCPS receive
    /// loop can reach it — that loop holds an `Arc<dyn Transport>`, and
    /// an inherent `recv_batch_linux` is invisible from there. That is
    /// how the loop ended up with a comment claiming the batch path and
    /// a `socket.recv()` that did not take it.
    ///
    /// Written `<UdpTransport as Transport>::recv_batch(&receiver, …)`
    /// and not `receiver.recv_batch(…)`. Method syntax prefers an
    /// inherent method, which is the shape of the defect; the qualified
    /// form resolves to the trait impl — the same function a vtable
    /// entry holds — so an override that never reached the trait falls
    /// back to the default here and the assertion below fails.
    #[test]
    fn recv_batch_through_the_trait_object_takes_several_datagrams_at_once() {
        let receiver = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .expect("bind")
            .with_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        let sender = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        let target = receiver.local_locator();
        for i in 0u8..8 {
            sender.send(&target, &[i; 16]).expect("send");
        }
        // Give the kernel a moment to queue all eight on the receive socket.
        std::thread::sleep(Duration::from_millis(100));

        let mut out: Vec<ReceivedDatagram> = Vec::new();
        let n =
            <UdpTransport as Transport>::recv_batch(&receiver, &mut out, 8).expect("recv_batch");
        assert_eq!(n, out.len(), "the count and the appended datagrams agree");
        assert!(n >= 1, "at least the blocking first datagram comes back");
        if cfg!(all(feature = "recvmmsg-batch", target_os = "linux")) {
            assert!(
                n > 1,
                "the batch override must hand over more than one queued datagram, got {n}"
            );
        }
        for dg in &out {
            assert_eq!(dg.data.len(), 16);
        }
    }

    #[test]
    fn recv_batch_appends_instead_of_clearing() {
        // A caller keeps its capacity across iterations, so the method must
        // never drop what is already in the vec.
        let receiver = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .expect("bind")
            .with_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        let sender = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        sender
            .send(&receiver.local_locator(), b"keep-me")
            .expect("send");
        let mut out: Vec<ReceivedDatagram> = alloc_one_placeholder(&receiver);
        let before = out.len();
        let n =
            <UdpTransport as Transport>::recv_batch(&receiver, &mut out, 4).expect("recv_batch");
        assert_eq!(out.len(), before + n, "the placeholder entry survived");
    }

    /// One already-received datagram, so
    /// [`recv_batch_appends_instead_of_clearing`] has something in the vec that
    /// must survive the call.
    fn alloc_one_placeholder(t: &UdpTransport) -> Vec<ReceivedDatagram> {
        alloc::vec![ReceivedDatagram {
            source: t.local_locator(),
            data: Arc::from(&b"placeholder"[..]),
        }]
    }

    #[test]
    fn recv_batch_with_zero_max_reads_nothing() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .expect("bind")
            .with_timeout(Some(Duration::from_millis(50)))
            .expect("timeout");
        let mut out: Vec<ReceivedDatagram> = Vec::new();
        assert_eq!(
            <UdpTransport as Transport>::recv_batch(&t, &mut out, 0).expect("no read"),
            0
        );
        assert!(out.is_empty());
    }

    #[test]
    fn recv_timeout_returns_timeout_error() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .expect("bind")
            .with_timeout(Some(Duration::from_millis(50)))
            .expect("timeout");
        let res = t.recv();
        assert!(matches!(res, Err(RecvError::Timeout)), "got {res:?}");
    }

    #[test]
    fn bind_multicast_v4_joins_group_and_returns_locator() {
        // We use 239.0.0.1 as the test group (not the reserved SPDP
        // group, so parallel test runs do not disturb each other).
        let group = Ipv4Addr::new(239, 0, 0, 1);
        // Port=0 is not allowed for a multicast bind — use an
        // ephemeral port via a bind attempt with a default port. If the
        // OS is reserved, we switch the test variant to
        // "no multicast".
        let res = UdpTransport::bind_multicast_v4(group, 0, Ipv4Addr::LOCALHOST);
        // Some CI environments do not allow multicast — then a bind
        // error is acceptable and the test is skipped.
        let Ok(t) = res else {
            eprintln!("Multicast not available in environment; skipping");
            return;
        };
        let loc = t.local_locator();
        assert_eq!(loc.kind, LocatorKind::UdpV4);
        // The locator points to the group, not to 0.0.0.0.
        assert_eq!(loc.ipv4(), [239, 0, 0, 1]);
        assert!(loc.port > 0);
    }

    /// A multicast receiver gets the group it joined and not another
    /// group sent to the same port (see `try_bind_multicast_v4`).
    #[cfg(target_os = "linux")]
    #[test]
    fn a_multicast_receiver_gets_only_the_group_it_joined() {
        let own = Ipv4Addr::new(239, 255, 88, 2);
        let other = Ipv4Addr::new(239, 255, 88, 3);
        let wait = Some(Duration::from_millis(300));
        let mine = UdpTransport::bind_multicast_v4(own, 0, Ipv4Addr::UNSPECIFIED)
            .expect("bind")
            .with_timeout(wait)
            .expect("timeout");
        let port = u16::try_from(mine.local_locator().port).expect("port");
        let theirs = UdpTransport::bind_multicast_v4(other, port, Ipv4Addr::UNSPECIFIED)
            .expect("bind the same port")
            .with_timeout(wait)
            .expect("timeout");
        let tx = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).expect("sender");
        tx.set_multicast_loop_v4(true).expect("loop");
        tx.send_to(b"other group", (other, port)).expect("send");
        assert_eq!(
            &theirs.recv().expect("the group it joined").data[..],
            b"other group"
        );
        let res = mine.recv();
        assert!(matches!(res, Err(RecvError::Timeout)), "got {res:?}");
    }

    /// A join that fails is skipped, and one that succeeds is enough;
    /// with none there is no transport. 192.0.2.1 (TEST-NET-1, RFC 5737)
    /// is the address of no local interface.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_join_on_several_interfaces_skips_the_ones_that_fail() {
        let group = Ipv4Addr::new(239, 255, 88, 4);
        let absent = Ipv4Addr::new(192, 0, 2, 1);
        let (_t, joined) = UdpTransport::bind_multicast_v4_on_with_config(
            group,
            0,
            &[absent, Ipv4Addr::UNSPECIFIED],
            UdpTransportConfig::default(),
        )
        .expect("the second join succeeds");
        assert_eq!(joined, [Ipv4Addr::UNSPECIFIED]);
        assert!(
            UdpTransport::bind_multicast_v4_on_with_config(
                group,
                0,
                &[absent],
                UdpTransportConfig::default()
            )
            .is_err(),
            "no join, no transport"
        );
    }

    /// A receiver joined on every eligible interface hears a sender on
    /// each of them: what goes out through one interface arrives through
    /// the membership on that interface, whichever the route would pick.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_group_joined_on_every_interface_hears_a_sender_on_each() {
        let addresses: Vec<Ipv4Addr> = crate::eligible_ipv4_interfaces()
            .iter()
            .map(|e| e.ipv4)
            .collect();
        if addresses.is_empty() {
            eprintln!("no eligible interface in this environment; skipping");
            return;
        }
        let group = Ipv4Addr::new(239, 255, 88, 5);
        let (rx, joined) = UdpTransport::bind_multicast_v4_on_with_config(
            group,
            0,
            &addresses,
            UdpTransportConfig::default(),
        )
        .expect("bind");
        let rx = rx
            .with_timeout(Some(Duration::from_millis(500)))
            .expect("timeout");
        let to = Locator::udp_v4(group.octets(), rx.local_locator().port);
        let mut sent = Vec::new();
        for interface in &joined {
            let tx = UdpTransport::bind_multicast_sender_v4_with_config(
                *interface,
                UdpTransportConfig::default(),
            )
            .expect("sender");
            assert_eq!(tx.local_locator().ipv4(), interface.octets());
            if tx.send(&to, &interface.octets()).is_ok() {
                sent.push(interface.octets().to_vec());
            }
        }
        assert!(
            !sent.is_empty(),
            "no interface took the datagram: {joined:?}"
        );
        let mut heard = Vec::new();
        while let Ok(d) = rx.recv() {
            heard.push(d.data.to_vec());
        }
        // Two interfaces on one segment hear each other's copy too, so
        // the count can be higher; every interface's own must be there.
        for payload in &sent {
            assert!(heard.contains(payload), "{payload:?} not heard: {heard:?}");
        }
    }

    #[test]
    fn set_multicast_ttl_does_not_error() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        let res = t.set_multicast_ttl(32);
        assert!(res.is_ok());
    }

    #[test]
    fn loopback_multiple_datagrams_in_order() {
        let (sender, receiver) = make_loopback_pair();
        let dest = receiver.local_locator();
        for i in 0u8..5 {
            sender.send(&dest, &[i, i, i]).expect("send");
        }
        for i in 0u8..5 {
            let r = receiver.recv().expect("recv");
            assert_eq!(&r.data[..], &[i, i, i]);
        }
    }

    /// Opt-5 — send_iovec sends multiple segments as 1 datagram.
    /// On macOS / non-Linux: the fallback path copy-merges + send().
    /// On Linux with feature recvmmsg-batch: sendmsg(iovec).
    /// The test covers the non-feature fallback (locally on macOS).
    #[test]
    fn send_iovec_combines_segments_into_one_datagram() {
        let (sender, receiver) = make_loopback_pair();
        let dest = receiver.local_locator();
        let head = b"HEAD-";
        let body = b"PAYLOAD";
        sender.send_iovec(&dest, &[head, body]).expect("send_iovec");
        let r = receiver.recv().expect("recv");
        assert_eq!(&r.data[..], b"HEAD-PAYLOAD");
    }

    /// Opt-3 — bind_v4_reuse allows two sockets on the same port.
    /// The kernel distributes incoming datagrams by flow hash. We build
    /// 2 reuse sockets, send 10 datagrams and verify that the sum of
    /// receives across both sockets is >= 1 (no bind error, both
    /// sockets are functional). Exact load balancing is a kernel
    /// heuristic and not deterministically testable.
    #[test]
    fn bind_v4_reuse_allows_two_sockets_on_same_port() {
        // Unique test port — otherwise it conflicts with parallel tests.
        let probe = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("probe");
        let port: u16 = probe.local_locator.port as u16;
        drop(probe);
        let a = UdpTransport::bind_v4_reuse(Ipv4Addr::LOCALHOST, port).expect("bind reuse a");
        let b = UdpTransport::bind_v4_reuse(Ipv4Addr::LOCALHOST, port).expect("bind reuse b");
        let a = a.with_timeout(Some(Duration::from_millis(200))).unwrap();
        let b = b.with_timeout(Some(Duration::from_millis(200))).unwrap();

        let sender = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).unwrap();
        let dest = a.local_locator();
        assert_eq!(a.local_locator().port, b.local_locator().port);
        for i in 0u8..10 {
            sender.send(&dest, &[i; 4]).expect("send");
        }
        // Drain both sockets for whatever they get.
        let mut total = 0;
        for _ in 0..20 {
            if a.recv().is_ok() {
                total += 1;
            }
            if b.recv().is_ok() {
                total += 1;
            }
            if total >= 10 {
                break;
            }
        }
        assert!(total >= 1, "at least one reuse-socket got a datagram");
    }

    /// DDSI-RTPS 2.5 §8.4.14.1: the transport reports the largest message it
    /// sends in one datagram: the configured one, 1472 by default, and never
    /// more than a UDP datagram holds.
    #[test]
    fn max_message_size_is_the_configured_one_up_to_the_datagram_limit() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        assert_eq!(t.max_message_size(), 1472);
        let cfg = UdpTransportConfig {
            max_message_size: 9000,
            ..UdpTransportConfig::default()
        };
        let t = UdpTransport::bind_v4_with_config(Ipv4Addr::LOCALHOST, 0, cfg).expect("bind");
        assert_eq!(t.max_message_size(), 9000);
        let cfg = UdpTransportConfig {
            max_message_size: 1_000_000,
            ..UdpTransportConfig::default()
        };
        let t = UdpTransport::bind_v4_with_config(Ipv4Addr::LOCALHOST, 0, cfg).expect("bind");
        assert_eq!(t.max_message_size(), MAX_DATAGRAM_SIZE);
    }
}
