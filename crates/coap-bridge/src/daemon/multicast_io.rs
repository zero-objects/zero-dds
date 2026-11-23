// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Multicast CoAP in the daemon (RFC 7252 §8, §11.3): joining the groups,
//! learning the destination address of a received datagram
//! (`IP_PKTINFO`/`IPV6_RECVPKTINFO`, RFC 3542) and sending the responses
//! to multicast requests at a random point of a leisure period (§8.2).

use std::collections::{BTreeMap, BinaryHeap};
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::config::MulticastSettings;

/// The groups of `settings` for a socket bound to `local`: the configured
/// ones, or the all-CoAP-nodes groups of RFC 7252 §12.8 of the bind
/// address's family.
#[must_use]
pub fn groups_of(settings: &MulticastSettings, local: SocketAddr) -> Vec<IpAddr> {
    if !settings.groups.is_empty() {
        return settings.groups.clone();
    }
    if local.is_ipv4() {
        vec![IpAddr::V4(Ipv4Addr::new(224, 0, 1, 187))]
    } else {
        vec![
            IpAddr::V6(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0xfd)),
            IpAddr::V6(Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 0, 0xfd)),
        ]
    }
}

/// Joins the groups on the daemon's socket `main` (bound to `local`). A
/// group of the other family is joined on a second socket, bound to the
/// unspecified address of its family and the same port, which this
/// returns; IPv4 groups need an IPv4 bind address.
///
/// # Errors
/// A group the system does not let the socket join, an IPv4 group with an
/// IPv6 bind address, or the second socket that cannot be bound.
pub fn join(
    settings: &MulticastSettings,
    main: &UdpSocket,
    local: SocketAddr,
) -> io::Result<Option<UdpSocket>> {
    let groups = groups_of(settings, local);
    let mut extra: Option<UdpSocket> = None;
    for group in groups {
        match (group, local) {
            (IpAddr::V4(g), SocketAddr::V4(_)) => {
                main.join_multicast_v4(&g, &settings.interface_v4)?;
            }
            (IpAddr::V6(g), SocketAddr::V6(_)) => {
                main.join_multicast_v6(&g, settings.interface_v6)?;
            }
            (IpAddr::V6(g), SocketAddr::V4(_)) => {
                if extra.is_none() {
                    extra = Some(v6_only_socket(local.port())?);
                }
                if let Some(socket) = &extra {
                    socket.join_multicast_v6(&g, settings.interface_v6)?;
                }
            }
            (IpAddr::V4(g), SocketAddr::V6(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("the IPv4 group {g} needs an IPv4 coap.bind address"),
                ));
            }
        }
    }
    if let Some(socket) = &extra {
        enable_destination_info(socket)?;
        socket.set_read_timeout(Some(Duration::from_millis(500)))?;
    }
    enable_destination_info(main)?;
    Ok(extra)
}

/// An IPv6 socket that takes IPv6 only, bound to `[::]:port`.
fn v6_only_socket(port: u16) -> io::Result<UdpSocket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV6,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_only_v6(true)?;
    socket.bind(&SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port).into())?;
    Ok(socket.into())
}

/// Lets `recv_with_destination` learn the destination address of a
/// datagram on `socket` (RFC 7252 §8.1: "A server SHOULD be aware that a
/// request arrived via multicast, e.g., by making use of modern APIs such
/// as IPV6_RECVPKTINFO").
///
/// # Errors
/// The socket option is refused.
#[cfg(unix)]
pub fn enable_destination_info(socket: &UdpSocket) -> io::Result<()> {
    use nix::sys::socket::{setsockopt, sockopt};
    let ipv4 = socket.local_addr()?.is_ipv4();
    let result = if ipv4 {
        setsockopt(socket, sockopt::Ipv4PacketInfo, &true)
    } else {
        setsockopt(socket, sockopt::Ipv6RecvPacketInfo, &true)
    };
    result.map_err(io::Error::from)
}

/// Without the socket options of RFC 3542 the destination is not known.
///
/// # Errors
/// Never.
#[cfg(not(unix))]
pub fn enable_destination_info(_socket: &UdpSocket) -> io::Result<()> {
    Ok(())
}

/// Receives one datagram: its length, its source and, where the system
/// tells it, the address it was sent to.
///
/// # Errors
/// As `UdpSocket::recv_from` (a timeout is `WouldBlock`).
#[cfg(unix)]
pub fn recv_with_destination(
    socket: &UdpSocket,
    buf: &mut [u8],
) -> io::Result<(usize, SocketAddr, Option<IpAddr>)> {
    use nix::sys::socket::{ControlMessageOwned, MsgFlags, SockaddrStorage, recvmsg};
    use std::os::fd::AsRawFd;
    let mut iov = [io::IoSliceMut::new(buf)];
    let mut space = nix::cmsg_space!(nix::libc::in_pktinfo, nix::libc::in6_pktinfo);
    let msg = recvmsg::<SockaddrStorage>(
        socket.as_raw_fd(),
        &mut iov,
        Some(&mut space),
        MsgFlags::empty(),
    )
    .map_err(io::Error::from)?;
    let peer = msg
        .address
        .and_then(|a| {
            a.as_sockaddr_in()
                .map(|v4| SocketAddr::V4((*v4).into()))
                .or_else(|| a.as_sockaddr_in6().map(|v6| SocketAddr::V6((*v6).into())))
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no source address"))?;
    let mut destination = None;
    for cmsg in msg.cmsgs().map_err(io::Error::from)? {
        match cmsg {
            ControlMessageOwned::Ipv4PacketInfo(info) => {
                destination = Some(IpAddr::V4(Ipv4Addr::from(u32::from_be(
                    info.ipi_addr.s_addr,
                ))));
            }
            ControlMessageOwned::Ipv6PacketInfo(info) => {
                destination = Some(IpAddr::V6(Ipv6Addr::from(info.ipi6_addr.s6_addr)));
            }
            _ => {}
        }
    }
    Ok((msg.bytes, peer, destination))
}

/// Without RFC 3542 the destination is not known; RFC 7252 §8.1 then lets
/// the server respond as to a unicast request.
///
/// # Errors
/// As `UdpSocket::recv_from`.
#[cfg(not(unix))]
pub fn recv_with_destination(
    socket: &UdpSocket,
    buf: &mut [u8],
) -> io::Result<(usize, SocketAddr, Option<IpAddr>)> {
    let (n, peer) = socket.recv_from(buf)?;
    Ok((n, peer, None))
}

/// The most responses to multicast requests waiting for their time; more
/// are not answered (RFC 7252 §8.2: the server "MAY always ignore the
/// request").
const MAX_PENDING: usize = 1024;

/// A response waiting for its point in a leisure period.
struct Pending {
    at: Instant,
    bytes: Vec<u8>,
    peer: SocketAddr,
    socket: Arc<UdpSocket>,
}

impl PartialEq for Pending {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at
    }
}
impl Eq for Pending {}
impl PartialOrd for Pending {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Pending {
    // The earliest first out of a max-heap.
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        other.at.cmp(&self.at)
    }
}

/// Sends the responses to multicast requests at a random point of a
/// leisure period (RFC 7252 §8.2); a further response for the same group
/// takes the next period at the earliest after the previous one.
pub struct Leisure {
    leisure: Duration,
    periods: Mutex<BTreeMap<IpAddr, Instant>>,
    tx: SyncSender<Pending>,
    hasher: std::collections::hash_map::RandomState,
}

impl Leisure {
    /// The scheduler and the thread that sends; it runs until `stop`.
    #[must_use]
    pub fn start(leisure: Duration, stop: Arc<AtomicBool>) -> (Arc<Self>, JoinHandle<()>) {
        let (tx, rx) = mpsc::sync_channel(MAX_PENDING);
        let thread = std::thread::spawn(move || send_loop(&rx, &stop));
        (
            Arc::new(Self {
                leisure,
                periods: Mutex::new(BTreeMap::new()),
                tx,
                hasher: std::collections::hash_map::RandomState::new(),
            }),
            thread,
        )
    }

    /// Schedules `bytes` to `peer` over `socket` within the leisure period
    /// for `group`; `false` when the queue is full and the response is
    /// not sent.
    pub fn schedule(
        &self,
        group: IpAddr,
        token: &[u8],
        bytes: Vec<u8>,
        peer: SocketAddr,
        socket: Arc<UdpSocket>,
    ) -> bool {
        use std::hash::BuildHasher;
        let now = Instant::now();
        let start = match self.periods.lock() {
            Ok(mut periods) => {
                let end = periods.get(&group).copied().unwrap_or(now);
                let start = end.max(now);
                periods.insert(group, start + self.leisure);
                start
            }
            Err(_) => now,
        };
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let seed = self.hasher.hash_one((token, nanos, peer));
        let offset =
            crate::multicast::leisure_delay(self.leisure, (seed >> 32) as u32 ^ seed as u32);
        self.tx
            .try_send(Pending {
                at: start + offset,
                bytes,
                peer,
                socket,
            })
            .is_ok()
    }
}

fn send_loop(rx: &Receiver<Pending>, stop: &AtomicBool) {
    let mut queue: BinaryHeap<Pending> = BinaryHeap::new();
    while !stop.load(Ordering::SeqCst) {
        let wait = queue
            .peek()
            .map_or(Duration::from_millis(200), |p| {
                p.at.saturating_duration_since(Instant::now())
            })
            .min(Duration::from_millis(200));
        match rx.recv_timeout(wait) {
            Ok(pending) => queue.push(pending),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let now = Instant::now();
        while queue.peek().is_some_and(|p| p.at <= now) {
            if let Some(p) = queue.pop() {
                let _ = p.socket.send_to(&p.bytes, p.peer);
            }
        }
    }
}
