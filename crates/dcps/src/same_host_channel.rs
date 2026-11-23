// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Same-host SHM channel per participant pair (Spec
//! `docs/specs/zerodds-zero-copy-1.0.md` §6, "Same-Host-SHM-Hot-Path").
//!
//! Two ZeroDDS participants on one host (equal `host_id`, see
//! [`GuidPrefix::is_same_host`]) hold two rings between them, one per
//! direction. Every unicast datagram a participant sends to a locator of
//! the other goes through the ring instead of UDP: user data, HEARTBEAT,
//! ACKNACK, GAP, NACK_FRAG, repairs, SEDP, TypeLookup, WLP. A frame starts
//! with one byte that names the socket it was for ([`Port`]), so the
//! receiver hands it to the handler of that socket.
//!
//! The channel is made when SPDP reports the peer and goes with it. A
//! sender routes only once the peer has opened its ring
//! ([`PosixShmTransport::peer_attached`]); a frame the ring refuses goes
//! over UDP. After each frame the sender rings the receiver's doorbell
//! ([`ShmDoorbell`]); one thread per participant parks on its own bell and
//! drains all its inbound rings, so the threads do not grow with the
//! number of peers. A ring holds four datagrams of `max_datagram`. Before this, the same-host path kept one segment and one
//! receive thread per writer/reader pair and carried user DATA only: a
//! chain of 21 processes on one host held 920 segments and about 1 030
//! threads, and 14 to 22 % of its datagrams still crossed UDP loopback
//! (measured in a multi-process test chain, 2026-10-02).

#![cfg(feature = "same-host-shm")]

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

use zerodds_foundation::observability::{Component, Event, Level, SharedSink};
use zerodds_rtps::wire_types::{GuidPrefix, Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};
use zerodds_transport_shm::{PosixShmTransport, ShmConfig, ShmDoorbell};

/// Which socket of the receiving participant a frame is for: the first
/// byte of every frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Port {
    /// The metatraffic unicast socket: SPDP unicast, SEDP, TypeLookup, WLP.
    Metatraffic,
    /// The user unicast socket: the endpoints' own traffic.
    User,
}

impl Port {
    fn tag(self) -> u8 {
        match self {
            Self::Metatraffic => 0,
            Self::User => 1,
        }
    }

    /// The port a frame's first byte names.
    #[must_use]
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Metatraffic),
            1 => Some(Self::User),
            _ => None,
        }
    }

    fn index(self) -> usize {
        usize::from(self.tag())
    }
}

/// The two rings to one peer participant.
pub struct PeerChannel {
    /// This participant → the peer; this side is the owner.
    pub outbound: Arc<PosixShmTransport>,
    /// The peer's doorbell, rung after every frame sent to it.
    pub peer_bell: Arc<ShmDoorbell>,
    /// The peer → this participant; this side is the consumer.
    pub inbound: Arc<PosixShmTransport>,
    /// The peer's metatraffic unicast locator: the source the receive path
    /// names for a metatraffic frame, where a reply goes.
    pub metatraffic_source: Locator,
    /// The peer's user unicast locator, likewise for a user frame.
    pub user_source: Locator,
    /// What went to this peer, kept across a rebind of `inbound`.
    pub counts: Arc<PeerCounts>,
}

/// What one push into a channel's ring did.
enum Push {
    Sent,
    /// The peer has not opened its end of the ring; counted in the
    /// channel's `unattached`.
    Unattached,
    /// The ring refused the frame.
    Refused(SendError),
}

/// How long after a channel opens a peer that has not opened its end is
/// still the normal course of discovery, not a fault.
pub const ATTACH_GRACE: Duration = Duration::from_secs(5);

/// How long a ring may stay too full for a frame without its reader
/// taking anything before the channel is closed. A reader that ended
/// without closing its end (killed) still counts as attached, and its
/// ring stays full: every frame to it was refused, after a spin, until
/// SPDP gave the peer up (measured in a multi-process test chain on a MacBook Air:
/// 262 124 of 262 144 bytes waiting, 25 040 frames refused).
pub const STALL_LIMIT: Duration = Duration::from_secs(2);

/// Datagrams for one peer, by the way they went.
#[derive(Debug)]
pub struct PeerCounts {
    /// When the channel opened.
    opened_at: Instant,
    /// Through the ring.
    pub sent: AtomicU64,
    /// Over UDP: the ring refused the frame (full, or the frame too large).
    pub refused: AtomicU64,
    /// Over UDP: the peer had not opened its end of the ring.
    pub unattached: AtomicU64,
    /// The part of `unattached` sent later than [`ATTACH_GRACE`] after the
    /// channel opened.
    pub unattached_late: AtomicU64,
    /// `refused` and `unattached_late` at the last report.
    reported: Mutex<(u64, u64)>,
    /// Since when, and at which reader position, frames were refused
    /// without the reader moving on.
    stalled: Mutex<Option<(Instant, u64)>>,
}

impl PeerCounts {
    fn new() -> Self {
        Self {
            opened_at: Instant::now(),
            sent: AtomicU64::new(0),
            refused: AtomicU64::new(0),
            unattached: AtomicU64::new(0),
            unattached_late: AtomicU64::new(0),
            reported: Mutex::new((0, 0)),
            stalled: Mutex::new(None),
        }
    }
}

/// The channels of one participant to the ZeroDDS participants on its host.
pub struct SameHostChannels {
    local: GuidPrefix,
    domain_id: u32,
    config: ShmConfig,
    sink: SharedSink,
    peers: RwLock<BTreeMap<GuidPrefix, Arc<PeerChannel>>>,
    /// A peer's UDP unicast locators → the peer.
    routes: RwLock<BTreeMap<Locator, GuidPrefix>>,
    /// This participant's doorbell, opened with the first channel.
    own_bell: RwLock<Option<Arc<ShmDoorbell>>>,
    /// Goes up with every [`Self::wake_receiver`] and when the doorbell
    /// opens: what the receive thread waits on while it has no doorbell
    /// to park on ([`Self::wait_for_receive_bell`]).
    receiver_wakes: Mutex<u64>,
    receiver_woken: Condvar,
    /// Goes up whenever the set of inbound rings changes.
    generation: AtomicU64,
    frames_sent: [AtomicU64; 2],
    fallbacks: AtomicU64,
    /// Datagrams to a loopback locator no channel routes: a same-host
    /// participant without a channel, or a peer of another implementation.
    unrouted_loopback: AtomicU64,
    open_failures: AtomicU64,
    unknown_ports: AtomicU64,
    /// Frames an inbound ring dropped as corrupt
    /// (`zerodds-shm-transport-1.0` §3.3).
    corrupt_frames: AtomicU64,
    /// When `report_udp_sends` last looked, and `unrouted_loopback` then.
    last_report: Mutex<(Option<Instant>, u64)>,
}

/// Whether the reader of `channel`'s outbound ring has taken nothing for
/// [`STALL_LIMIT`] while frames were refused: the first refusal at a
/// reader position starts the clock, a refusal at a later position starts
/// it again.
fn reader_stalled(channel: &PeerChannel) -> bool {
    let position = channel.outbound.consumed_bytes();
    let Ok(mut stalled) = channel.counts.stalled.lock() else {
        return false;
    };
    match *stalled {
        Some((since, at)) if at == position => since.elapsed() >= STALL_LIMIT,
        _ => {
            *stalled = Some((Instant::now(), position));
            false
        }
    }
}

/// A UDP locator on the loopback interface: 127.0.0.0/8 or ::1.
fn is_loopback(locator: &Locator) -> bool {
    match locator.kind {
        LocatorKind::UdpV4 => locator.address[12] == 127,
        LocatorKind::UdpV6 => locator.address == core::net::Ipv6Addr::LOCALHOST.octets(),
        _ => false,
    }
}

/// The 16-byte id of a participant's end of a channel: its prefix, a
/// marker that keeps it apart from the ids of the `SHM` user transport
/// (prefix + domain), and the domain.
fn channel_id(prefix: GuidPrefix, domain_id: u32) -> [u8; 16] {
    tagged_id(prefix, domain_id, 0xC5)
}

/// The id of a participant's doorbell.
fn bell_id(prefix: GuidPrefix, domain_id: u32) -> [u8; 16] {
    tagged_id(prefix, domain_id, 0xB5)
}

fn tagged_id(prefix: GuidPrefix, domain_id: u32, tag: u8) -> [u8; 16] {
    let mut id = [0u8; 16];
    id[..12].copy_from_slice(&prefix.to_bytes());
    id[12] = tag;
    id[13..].copy_from_slice(&domain_id.to_be_bytes()[1..]);
    id
}

impl SameHostChannels {
    /// No channels yet. `max_datagram` bounds a frame, its port byte
    /// included.
    ///
    /// # Errors
    /// A `max_datagram` the ring cannot be sized for.
    pub fn new(
        local: GuidPrefix,
        domain_id: u32,
        max_datagram: usize,
        sink: SharedSink,
    ) -> Result<Self, &'static str> {
        let mut config =
            crate::same_host_shm::shm_config_for_pair_with_limit(local, domain_id, max_datagram)?;
        // Four datagrams, not the 2 MiB of a writer/reader pair's ring: a
        // participant holds a ring to every peer on the host, and a
        // datagram the ring cannot take goes over UDP.
        config.capacity = max_datagram
            .checked_mul(4)
            .ok_or("shm: invalid max_datagram")?;
        Ok(Self {
            local,
            domain_id,
            config,
            sink,
            peers: RwLock::new(BTreeMap::new()),
            routes: RwLock::new(BTreeMap::new()),
            own_bell: RwLock::new(None),
            receiver_wakes: Mutex::new(0),
            receiver_woken: Condvar::new(),
            generation: AtomicU64::new(0),
            frames_sent: [AtomicU64::new(0), AtomicU64::new(0)],
            fallbacks: AtomicU64::new(0),
            unrouted_loopback: AtomicU64::new(0),
            open_failures: AtomicU64::new(0),
            unknown_ports: AtomicU64::new(0),
            corrupt_frames: AtomicU64::new(0),
            last_report: Mutex::new((None, 0)),
        })
    }

    /// Opens the channel to `peer`, or brings its routes up to date when
    /// it is open. `metatraffic` and `user` are the peer's announced
    /// unicast locators; only UDP ones are routed, since those are the ones
    /// a send would otherwise take.
    pub fn add_peer(&self, peer: GuidPrefix, metatraffic: &[Locator], user: &[Locator]) {
        let routable = |l: &&Locator| matches!(l.kind, LocatorKind::UdpV4 | LocatorKind::UdpV6);
        let mut locators: Vec<Locator> = metatraffic
            .iter()
            .chain(user)
            .filter(routable)
            .copied()
            .collect();
        locators.sort();
        locators.dedup();
        let set_routes = |locators: Vec<Locator>| {
            if let Ok(mut routes) = self.routes.write() {
                routes.retain(|_, p| *p != peer);
                for l in locators {
                    routes.insert(l, peer);
                }
            }
        };
        if !self.peers.read().is_ok_and(|p| p.contains_key(&peer)) {
            // Under the write lock: SPDP for one peer arrives on several
            // receive threads at once, and two opens of one channel would
            // put two owners on its ring.
            let Ok(mut peers) = self.peers.write() else {
                return;
            };
            if let alloc::collections::btree_map::Entry::Vacant(slot) = peers.entry(peer) {
                let Some(channel) = self.open(peer, metatraffic, user) else {
                    return;
                };
                // The routes before the channel: whoever sees the channel
                // finds its routes. The other way round a thread preempted
                // between the two left an open, attached channel that no
                // send was routed to (measured on the Linux test host: 4
                // of 2 000 parallel runs of
                // `same_host_e2e::flatdata_10_3_e2e_two_runtimes_shm_roundtrip`
                // sent everything over UDP, `routes []`).
                set_routes(locators);
                slot.insert(channel);
                self.generation.fetch_add(1, Ordering::AcqRel);
                return;
            }
        }
        set_routes(locators);
    }

    fn open(
        &self,
        peer: GuidPrefix,
        metatraffic: &[Locator],
        user: &[Locator],
    ) -> Option<Arc<PeerChannel>> {
        let (me, them) = (
            channel_id(self.local, self.domain_id),
            channel_id(peer, self.domain_id),
        );
        let opened = self.own_bell().and_then(|_| {
            let peer_bell =
                ShmDoorbell::open(bell_id(peer, self.domain_id), false, &self.config.flink_dir)?;
            let outbound = PosixShmTransport::open_owner(me, them, self.config.clone())?;
            let inbound = PosixShmTransport::open_consumer(me, them, self.config.clone())?;
            Ok((peer_bell, outbound, inbound))
        });
        match opened {
            Ok((peer_bell, outbound, inbound)) => Some(Arc::new(PeerChannel {
                outbound: {
                    // The peer parks on its doorbell, not on this ring.
                    outbound.set_wake_on_push(false);
                    Arc::new(outbound)
                },
                peer_bell: Arc::new(peer_bell),
                inbound: Arc::new(inbound),
                metatraffic_source: metatraffic.first().copied().unwrap_or(Locator::INVALID),
                user_source: user.first().copied().unwrap_or(Locator::INVALID),
                counts: Arc::new(PeerCounts::new()),
            })),
            Err(e) => {
                if self.open_failures.fetch_add(1, Ordering::Relaxed) == 0 {
                    self.sink.record(
                        &Event::new(Level::Warn, Component::Transport, "shm.channel_open_failed")
                            .with_attr("peer", alloc::format!("{peer:?}"))
                            .with_attr("error", alloc::format!("{e}")),
                    );
                }
                None
            }
        }
    }

    /// This participant's doorbell, opened on first use.
    ///
    /// # Errors
    /// The doorbell segment could not be opened.
    fn own_bell(&self) -> Result<Arc<ShmDoorbell>, zerodds_transport_shm::PosixShmError> {
        if let Some(bell) = self.own_bell.read().ok().and_then(|b| b.clone()) {
            return Ok(bell);
        }
        let mut slot = self
            .own_bell
            .write()
            .map_err(|_| zerodds_transport_shm::PosixShmError::InvalidHeader)?;
        if let Some(bell) = slot.as_ref() {
            return Ok(Arc::clone(bell));
        }
        let bell = Arc::new(ShmDoorbell::open(
            bell_id(self.local, self.domain_id),
            true,
            &self.config.flink_dir,
        )?);
        *slot = Some(Arc::clone(&bell));
        drop(slot);
        // A receive thread without a doorbell waits for this one.
        self.signal_receiver();
        Ok(bell)
    }

    /// This participant's doorbell, once a channel opened it: what the
    /// receive thread parks on.
    #[must_use]
    pub fn receive_bell(&self) -> Option<Arc<ShmDoorbell>> {
        self.own_bell.read().ok().and_then(|b| b.clone())
    }

    /// Wakes the receive thread, so it sees a stop flag at once: rings the
    /// doorbell it parks on, and ends the wait of a thread that has no
    /// doorbell yet ([`Self::wait_for_receive_bell`]).
    pub fn wake_receiver(&self) {
        if let Some(bell) = self.receive_bell() {
            bell.ring();
        }
        self.signal_receiver();
    }

    /// The count [`Self::wait_for_receive_bell`] waits to change. The
    /// receive thread reads it before it looks at its stop flag, so a wake
    /// that follows the flag is not lost between the look and the wait.
    #[must_use]
    pub fn receiver_wakes(&self) -> u64 {
        self.receiver_wakes.lock().map_or(0, |wakes| *wakes)
    }

    /// Waits until [`Self::wake_receiver`] ran or the doorbell opened after
    /// `seen` was read from [`Self::receiver_wakes`], or until `timeout`.
    /// What the receive thread does while there is no channel: before, it
    /// slept 100 ms, and a shutdown of a participant without a same-host
    /// peer waited out the rest of that sleep in the join of the thread
    /// (92 to 119 ms, measured on the Linux test host).
    pub fn wait_for_receive_bell(&self, seen: u64, timeout: Duration) {
        let Ok(wakes) = self.receiver_wakes.lock() else {
            // Poisoned: nothing to wait on, and no busy loop either.
            std::thread::sleep(timeout);
            return;
        };
        let _ = self
            .receiver_woken
            .wait_timeout_while(wakes, timeout, |wakes| *wakes == seen);
    }

    fn signal_receiver(&self) {
        if let Ok(mut wakes) = self.receiver_wakes.lock() {
            *wakes = wakes.wrapping_add(1);
            self.receiver_woken.notify_all();
        }
    }

    /// Goes up whenever the set of inbound rings changes.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Closes the channel to `peer`: its routes go, and its rings with the
    /// last handle. The outbound owner's drop tells the peer it is gone.
    pub fn remove_peer(&self, peer: GuidPrefix) {
        // Routes and channel under the channels' lock together: a channel
        // is never visible without its routes (see `add_peer`).
        let Ok(mut peers) = self.peers.write() else {
            return;
        };
        if let Ok(mut routes) = self.routes.write() {
            routes.retain(|_, p| *p != peer);
        }
        if peers.remove(&peer).is_some() {
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Closes every channel.
    pub fn clear(&self) {
        if let Ok(mut peers) = self.peers.write() {
            if let Ok(mut routes) = self.routes.write() {
                routes.clear();
            }
            peers.clear();
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
        // The doorbell went only with the channels' last handle, which a
        // runtime its threads still reference never drops.
        if let Ok(mut bell) = self.own_bell.write() {
            bell.take();
        }
    }

    /// Sends `bytes` for `port` through the channel to the participant
    /// behind `dest`. `false` when there is none, its peer has not opened
    /// its ring yet, or the ring refused the frame: the caller sends over
    /// UDP then.
    pub fn send(&self, dest: &Locator, port: Port, bytes: &[u8]) -> bool {
        let Some(peer) = self.routes.read().ok().and_then(|r| r.get(dest).copied()) else {
            if is_loopback(dest) {
                self.unrouted_loopback.fetch_add(1, Ordering::Relaxed);
            }
            return false;
        };
        let Some(channel) = self.peers.read().ok().and_then(|p| p.get(&peer).cloned()) else {
            return false;
        };
        self.send_on(peer, channel, port, bytes)
    }

    /// [`Self::send`] through `channel`, the channel to `peer` as the send
    /// found it. A channel closed and opened again since then
    /// ([`Self::remove_peer`], [`Self::add_peer`]) has an owner that took
    /// the ring over (`zerodds-shm-transport-1.0` §5.3), and `channel`'s
    /// owner refuses the frame: it goes through the channel there is now.
    /// Before, the two owners pushed into the ring at once.
    fn send_on(
        &self,
        peer: GuidPrefix,
        mut channel: Arc<PeerChannel>,
        port: Port,
        bytes: &[u8],
    ) -> bool {
        let mut result = self.push_to(&channel, port, bytes);
        // Each turn moves to a newer owner; more than a couple of closes
        // and opens during one send do not occur.
        for _ in 0..4 {
            if !matches!(result, Push::Refused(_)) || !channel.outbound.is_superseded() {
                break;
            }
            let Some(current) = self
                .peers
                .read()
                .ok()
                .and_then(|p| p.get(&peer).cloned())
                .filter(|c| !Arc::ptr_eq(&c.outbound, &channel.outbound))
            else {
                break;
            };
            channel = current;
            result = self.push_to(&channel, port, bytes);
        }
        match result {
            Push::Unattached => false,
            Push::Sent => {
                channel.peer_bell.ring();
                self.frames_sent[port.index()].fetch_add(1, Ordering::Relaxed);
                channel.counts.sent.fetch_add(1, Ordering::Relaxed);
                true
            }
            Push::Refused(e) => {
                channel.counts.refused.fetch_add(1, Ordering::Relaxed);
                self.note_fallback(peer, &e);
                if !matches!(e, SendError::PayloadTooLarge { .. })
                    && !channel.outbound.is_superseded()
                    && reader_stalled(&channel)
                {
                    self.close_stalled(peer, &channel);
                }
                false
            }
        }
    }

    /// Pushes one frame into `channel`'s ring.
    fn push_to(&self, channel: &PeerChannel, port: Port, bytes: &[u8]) -> Push {
        if !channel.outbound.peer_attached() {
            let counts = &channel.counts;
            counts.unattached.fetch_add(1, Ordering::Relaxed);
            if counts.opened_at.elapsed() > ATTACH_GRACE {
                counts.unattached_late.fetch_add(1, Ordering::Relaxed);
            }
            return Push::Unattached;
        }
        match channel.outbound.send_with_prefix(
            &channel.outbound.peer_locator(),
            &[port.tag()],
            bytes,
        ) {
            Ok(()) => Push::Sent,
            Err(e) => Push::Refused(e),
        }
    }

    /// Closes the channel to `peer`, whose reader took nothing from a full
    /// ring for [`STALL_LIMIT`]; its traffic goes over UDP. A reader that
    /// is still there follows the owner's new segment when the peer is
    /// reported again, as after any reopening.
    fn close_stalled(&self, peer: GuidPrefix, channel: &PeerChannel) {
        self.sink.record(
            &Event::new(
                Level::Warn,
                Component::Transport,
                "shm.channel_closed_stalled_reader",
            )
            .with_attr("peer", alloc::format!("{peer:?}"))
            .with_attr(
                "outbound_bytes_waiting",
                alloc::format!("{}", channel.outbound.occupied_bytes()),
            )
            .with_attr(
                "refused",
                alloc::format!("{}", channel.counts.refused.load(Ordering::Relaxed)),
            ),
        );
        self.remove_peer(peer);
    }

    /// Files what went over UDP although a channel could have carried it,
    /// at most once per `every`. A warning `shm.channel_udp_sends` per peer
    /// whose ring refused frames since the last report, or who still had
    /// not opened its end [`ATTACH_GRACE`] after the channel opened; sends
    /// before that are discovery's normal course and only counted. An info
    /// `shm.unrouted_loopback_sends` when datagrams went to a loopback
    /// locator no channel routes, which a peer of another implementation on
    /// this host does as well. Nothing when nothing went over UDP. The
    /// counts are cumulative, with the change since the last report beside
    /// them.
    pub fn report_udp_sends(&self, every: Duration) {
        let unrouted = self.unrouted_loopback.load(Ordering::Relaxed);
        let unrouted_before = {
            let Ok(mut last) = self.last_report.lock() else {
                return;
            };
            let now = Instant::now();
            if last.0.is_some_and(|at| now.duration_since(at) < every) {
                return;
            }
            let before = last.1;
            *last = (Some(now), unrouted);
            before
        };
        if unrouted > unrouted_before {
            self.sink.record(
                &Event::new(
                    Level::Info,
                    Component::Transport,
                    "shm.unrouted_loopback_sends",
                )
                .with_attr("total", alloc::format!("{unrouted}"))
                .with_attr(
                    "since_last",
                    alloc::format!("{}", unrouted - unrouted_before),
                )
                .with_attr("channels", alloc::format!("{}", self.len())),
            );
        }
        for (peer, channel) in self.snapshot() {
            let counts = &channel.counts;
            let refused = counts.refused.load(Ordering::Relaxed);
            let late = counts.unattached_late.load(Ordering::Relaxed);
            let Ok(mut reported) = counts.reported.lock() else {
                continue;
            };
            if (refused, late) == *reported {
                continue;
            }
            let (refused_before, late_before) = *reported;
            *reported = (refused, late);
            drop(reported);
            self.sink.record(
                &Event::new(Level::Warn, Component::Transport, "shm.channel_udp_sends")
                    .with_attr("peer", alloc::format!("{peer:?}"))
                    .with_attr(
                        "attached",
                        alloc::format!("{}", channel.outbound.peer_attached()),
                    )
                    .with_attr(
                        "sent",
                        alloc::format!("{}", counts.sent.load(Ordering::Relaxed)),
                    )
                    .with_attr("refused", alloc::format!("{refused}"))
                    .with_attr(
                        "refused_since_last",
                        alloc::format!("{}", refused - refused_before),
                    )
                    .with_attr(
                        "unattached",
                        alloc::format!("{}", counts.unattached.load(Ordering::Relaxed)),
                    )
                    .with_attr("unattached_late", alloc::format!("{late}"))
                    .with_attr(
                        "unattached_late_since_last",
                        alloc::format!("{}", late - late_before),
                    )
                    .with_attr(
                        "outbound_bytes_waiting",
                        alloc::format!("{}", channel.outbound.occupied_bytes()),
                    )
                    .with_attr(
                        "inbound_bytes_waiting",
                        alloc::format!("{}", channel.inbound.occupied_bytes()),
                    ),
            );
        }
    }

    /// Datagrams to a loopback locator no channel routes.
    #[must_use]
    pub fn unrouted_loopback(&self) -> u64 {
        self.unrouted_loopback.load(Ordering::Relaxed)
    }

    fn note_fallback(&self, peer: GuidPrefix, error: &SendError) {
        if self.fallbacks.fetch_add(1, Ordering::Relaxed) == 0 {
            self.sink.record(
                &Event::new(
                    Level::Warn,
                    Component::Transport,
                    "shm.send_failed_udp_fallback",
                )
                .with_attr("peer", alloc::format!("{peer:?}"))
                .with_attr("error", alloc::format!("{error}")),
            );
        }
    }

    /// Counts a received frame whose port byte names no port, and files the
    /// first: the frame is dropped, since there is no handler to give it to.
    pub fn note_unknown_port(&self, tag: u8) {
        if self.unknown_ports.fetch_add(1, Ordering::Relaxed) == 0 {
            self.sink.record(
                &Event::new(
                    Level::Warn,
                    Component::Transport,
                    "shm.channel_unknown_port",
                )
                .with_attr("tag", alloc::format!("{tag}")),
            );
        }
    }

    /// Frames received with a port byte that names no port.
    #[must_use]
    pub fn unknown_ports(&self) -> u64 {
        self.unknown_ports.load(Ordering::Relaxed)
    }

    /// The next frame of `peer`'s ring `inbound`, without waiting, as
    /// [`PosixShmTransport::try_recv`]. A frame the ring drops on the way
    /// because its length or position cannot be one a writer of the
    /// protocol published (`zerodds-shm-transport-1.0` §3.3) is counted
    /// here and filed as `shm.corrupt_frame` with the peer and the number
    /// dropped: the ring resumes at its `head`, and what the peer wrote up
    /// to there is lost.
    ///
    /// # Errors
    /// As [`PosixShmTransport::try_recv`].
    pub fn receive(
        &self,
        peer: GuidPrefix,
        inbound: &PosixShmTransport,
    ) -> Result<Option<ReceivedDatagram>, RecvError> {
        let before = inbound.corrupt_frames_seen();
        let got = inbound.try_recv();
        let dropped = inbound.corrupt_frames_seen().wrapping_sub(before);
        if dropped > 0 {
            self.corrupt_frames.fetch_add(dropped, Ordering::Relaxed);
            self.sink.record(
                &Event::new(Level::Error, Component::Transport, "shm.corrupt_frame")
                    .with_attr("peer", alloc::format!("{peer:?}"))
                    .with_attr("count", alloc::format!("{dropped}")),
            );
        }
        got
    }

    /// Whether the channel to `peer` is open and how many locators route
    /// to it, read together under the channels' lock.
    #[cfg(test)]
    fn channel_and_routes(&self, peer: GuidPrefix) -> (bool, usize) {
        let Ok(peers) = self.peers.read() else {
            return (false, 0);
        };
        let routes = self
            .routes
            .read()
            .map(|r| r.values().filter(|p| **p == peer).count())
            .unwrap_or(0);
        (peers.contains_key(&peer), routes)
    }

    /// The locators a send routes to the channel of `peer`.
    #[must_use]
    pub fn routes_of(&self, peer: GuidPrefix) -> Vec<Locator> {
        self.routes
            .read()
            .map(|r| {
                r.iter()
                    .filter(|(_, p)| **p == peer)
                    .map(|(l, _)| *l)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Frames the inbound rings dropped as corrupt.
    #[must_use]
    pub fn corrupt_frames(&self) -> u64 {
        self.corrupt_frames.load(Ordering::Relaxed)
    }

    /// Re-opens the inbound ring of `peer` when it is still `old`: the
    /// peer dropped it (its shutdown flag) while it stays discovered, as
    /// after a lease that expired on the peer's side only, and made it
    /// anew. Returns whether it replaced.
    pub fn rebind_inbound(&self, peer: GuidPrefix, old: &Arc<PosixShmTransport>) -> bool {
        let (me, them) = (
            channel_id(self.local, self.domain_id),
            channel_id(peer, self.domain_id),
        );
        let Ok(fresh) = PosixShmTransport::open_consumer(me, them, self.config.clone()) else {
            return false;
        };
        let Ok(mut peers) = self.peers.write() else {
            return false;
        };
        let Some(current) = peers.get(&peer) else {
            return false;
        };
        if !Arc::ptr_eq(&current.inbound, old) {
            return false;
        }
        let replaced = Arc::new(PeerChannel {
            outbound: Arc::clone(&current.outbound),
            peer_bell: Arc::clone(&current.peer_bell),
            inbound: Arc::new(fresh),
            metatraffic_source: current.metatraffic_source,
            user_source: current.user_source,
            counts: Arc::clone(&current.counts),
        });
        peers.insert(peer, replaced);
        self.generation.fetch_add(1, Ordering::AcqRel);
        true
    }

    /// The open channels.
    #[must_use]
    pub fn snapshot(&self) -> Vec<(GuidPrefix, Arc<PeerChannel>)> {
        self.peers
            .read()
            .map(|p| p.iter().map(|(k, v)| (*k, Arc::clone(v))).collect())
            .unwrap_or_default()
    }

    /// Number of open channels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.peers.read().map(|p| p.len()).unwrap_or(0)
    }

    /// `true` without an open channel.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Frames sent through a channel for `port`.
    #[must_use]
    pub fn frames_sent(&self, port: Port) -> u64 {
        self.frames_sent[port.index()].load(Ordering::Relaxed)
    }

    /// Datagrams a ring refused and that went over UDP instead.
    #[must_use]
    pub fn fallbacks(&self) -> u64 {
        self.fallbacks.load(Ordering::Relaxed)
    }
}

impl core::fmt::Debug for SameHostChannels {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SameHostChannels")
            .field("peers", &self.len())
            .finish()
    }
}

/// The user unicast transport with the channels in front: a send to a
/// locator of a same-host peer with an open channel goes through it, every
/// other send and every receive is the wrapped transport's.
pub struct RoutedUserTransport {
    inner: Arc<dyn Transport + Send + Sync>,
    channels: Arc<SameHostChannels>,
}

impl RoutedUserTransport {
    /// Puts `channels` in front of `inner`.
    #[must_use]
    pub fn new(inner: Arc<dyn Transport + Send + Sync>, channels: Arc<SameHostChannels>) -> Self {
        Self { inner, channels }
    }
}

impl Transport for RoutedUserTransport {
    /// That of the network transport behind the same-host channels.
    fn max_message_size(&self) -> usize {
        self.inner.max_message_size()
    }

    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if self.channels.send(dest, Port::User, data) {
            return Ok(());
        }
        self.inner.send(dest, data)
    }

    /// As [`Self::send`]: a same-host channel has no IP header to carry
    /// a priority in; the network path gets it.
    fn send_with_priority(
        &self,
        dest: &Locator,
        data: &[u8],
        priority: i32,
    ) -> Result<(), SendError> {
        if self.channels.send(dest, Port::User, data) {
            return Ok(());
        }
        self.inner.send_with_priority(dest, data, priority)
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        self.inner.recv()
    }

    fn add_destination(&self, dest: &Locator) {
        self.inner.add_destination(dest);
    }

    fn forget_destination(&self, dest: &Locator) {
        self.inner.forget_destination(dest);
    }

    fn close(&self) {
        self.inner.close();
    }

    fn local_locator(&self) -> Locator {
        self.inner.local_locator()
    }

    fn local_locators(&self) -> Vec<Locator> {
        self.inner.local_locators()
    }

    fn recv_batch(&self, out: &mut Vec<ReceivedDatagram>, max: usize) -> Result<usize, RecvError> {
        self.inner.recv_batch(out, max)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn port_tags_round_trip_and_unknown_tags_are_refused() {
        for port in [Port::Metatraffic, Port::User] {
            assert_eq!(Port::from_tag(port.tag()), Some(port));
        }
        assert_eq!(Port::from_tag(2), None);
    }

    /// A domain no other test of this process uses, so two runs on one
    /// host do not share a channel name.
    fn private_domain(n: u32) -> u32 {
        (std::process::id() % 100_000) * 10 + n
    }

    /// A ZeroDDS prefix on `host`: the vendor id, the host id in octets
    /// 2..6 (DDSI-RTPS 2.5 §9.3.1.5), the rest `fill`.
    fn prefix(host: [u8; 4], fill: u8) -> GuidPrefix {
        GuidPrefix::zerodds(host, [fill; 6])
    }

    /// The split between the channel and UDP, on any host: a send to a
    /// locator the peer announced goes through its ring once the peer has
    /// opened it, framed with the port byte; a send to any other locator,
    /// or before the peer is there, or after the channel closed, does not.
    #[test]
    fn flatdata_3_2_a_send_to_a_peer_locator_goes_through_the_channel_and_any_other_does_not() {
        let domain = private_domain(1);
        let host = [0x61, 0x62, 0x63, 0x64];
        let (pa, pb) = (prefix(host, 0xA1), prefix(host, 0xB2));
        let sink = zerodds_foundation::observability::null_sink();
        let a = SameHostChannels::new(pa, domain, 4096, sink.clone()).unwrap();
        let b = SameHostChannels::new(pb, domain, 4096, sink).unwrap();
        let (pa_meta, pa_user) = (
            Locator::udp_v4([127, 0, 0, 1], 41001),
            Locator::udp_v4([127, 0, 0, 1], 41002),
        );
        let (pb_meta, pb_user) = (
            Locator::udp_v4([127, 0, 0, 1], 41003),
            Locator::udp_v4([127, 0, 0, 1], 41004),
        );

        a.add_peer(pb, &[pb_meta], &[pb_user]);
        assert!(
            !a.send(&pb_user, Port::User, b"early"),
            "b has not opened its ring"
        );
        b.add_peer(pa, &[pa_meta], &[pa_user]);
        assert!(a.send(&pb_user, Port::User, b"to b"));
        assert!(a.send(&pb_meta, Port::Metatraffic, b"sedp"));
        assert!(
            !a.send(
                &Locator::udp_v4([127, 0, 0, 1], 41009),
                Port::User,
                b"elsewhere"
            ),
            "a locator nobody on this host announced"
        );
        let inbound = Arc::clone(&b.snapshot()[0].1.inbound);
        assert_eq!(&inbound.recv().unwrap().data[..], b"\x01to b");
        assert_eq!(&inbound.recv().unwrap().data[..], b"\x00sedp");
        assert_eq!(
            (a.frames_sent(Port::User), a.frames_sent(Port::Metatraffic)),
            (1, 1)
        );

        a.remove_peer(pb);
        assert!(!a.send(&pb_user, Port::User, b"closed"));
        assert!(a.is_empty());
    }

    /// A frame the ring refuses (larger than `max_datagram`) is not sent
    /// and is counted, so the caller sends it over UDP.
    #[test]
    fn flatdata_4_2_a_frame_the_ring_refuses_is_counted_and_left_to_udp() {
        let domain = private_domain(2);
        let host = [0x71, 0x72, 0x73, 0x74];
        let (pa, pb) = (prefix(host, 0xA3), prefix(host, 0xB4));
        let sink = zerodds_foundation::observability::null_sink();
        let a = SameHostChannels::new(pa, domain, 1024, sink.clone()).unwrap();
        let b = SameHostChannels::new(pb, domain, 1024, sink).unwrap();
        let pb_user = Locator::udp_v4([127, 0, 0, 1], 41014);
        a.add_peer(pb, &[], &[pb_user]);
        b.add_peer(pa, &[], &[Locator::udp_v4([127, 0, 0, 1], 41013)]);
        assert!(a.send(&pb_user, Port::User, &[0u8; 1000]));
        assert!(
            !a.send(&pb_user, Port::User, &[0u8; 1024]),
            "1 port byte over the limit"
        );
        assert_eq!(a.fallbacks(), 1);
    }

    /// What went over UDP is reported per peer, with the reason: before
    /// the peer opened its ring (`unattached`), and when the ring refused a
    /// frame (`refused`); a loopback locator no channel routes is counted
    /// apart. A report without anything new since the last one files
    /// nothing.
    #[test]
    fn udp_sends_are_reported_per_peer_with_their_reason_and_only_when_new() {
        use zerodds_foundation::observability::VecSink;
        let domain = private_domain(3);
        let host = [0x81, 0x82, 0x83, 0x84];
        let (pa, pb) = (prefix(host, 0xA5), prefix(host, 0xB6));
        let events = Arc::new(VecSink::new());
        let sink: SharedSink = events.clone();
        let a = SameHostChannels::new(pa, domain, 1024, sink).unwrap();
        let b = SameHostChannels::new(
            pb,
            domain,
            1024,
            zerodds_foundation::observability::null_sink(),
        )
        .unwrap();
        let pb_user = Locator::udp_v4([127, 0, 0, 1], 41024);
        a.add_peer(pb, &[], &[pb_user]);
        assert!(!a.send(&pb_user, Port::User, b"before b opened"));
        b.add_peer(pa, &[], &[Locator::udp_v4([127, 0, 0, 1], 41023)]);
        assert!(a.send(&pb_user, Port::User, b"through the ring"));
        assert!(!a.send(&pb_user, Port::User, &[0u8; 1024]), "too large");
        assert!(!a.send(
            &Locator::udp_v4([127, 0, 0, 1], 41029),
            Port::User,
            b"nobody routes this"
        ));
        assert!(!a.send(
            &Locator::udp_v4([10, 0, 0, 1], 41029),
            Port::User,
            b"another host"
        ));
        assert_eq!(a.unrouted_loopback(), 1);

        a.report_udp_sends(Duration::ZERO);
        let filed = events.snapshot();
        let attr = |name: &str, key: &str| {
            filed
                .iter()
                .find(|e| e.name == name)
                .and_then(|e| e.attrs.iter().find(|a| a.key == key))
                .map(|a| a.value.clone())
                .unwrap_or_default()
        };
        assert_eq!(attr("shm.channel_udp_sends", "sent"), "1");
        assert_eq!(attr("shm.channel_udp_sends", "refused"), "1");
        assert_eq!(attr("shm.channel_udp_sends", "unattached"), "1");
        assert_eq!(
            attr("shm.channel_udp_sends", "unattached_late"),
            "0",
            "within the grace of a fresh channel"
        );
        assert_eq!(attr("shm.channel_udp_sends", "attached"), "true");
        assert_eq!(attr("shm.unrouted_loopback_sends", "total"), "1");

        let before = events.len();
        assert!(a.send(&pb_user, Port::User, b"through the ring again"));
        a.report_udp_sends(Duration::ZERO);
        assert_eq!(events.len(), before, "nothing new went over UDP");
    }

    /// A peer that has not opened its end when the grace is over is
    /// reported, sends before that are not.
    #[test]
    fn a_peer_that_never_opens_its_end_is_reported_after_the_grace() {
        use zerodds_foundation::observability::VecSink;
        let domain = private_domain(4);
        let host = [0x91, 0x92, 0x93, 0x94];
        let (pa, pb) = (prefix(host, 0xA7), prefix(host, 0xB8));
        let events = Arc::new(VecSink::new());
        let sink: SharedSink = events.clone();
        let a = SameHostChannels::new(pa, domain, 1024, sink).unwrap();
        let pb_user = Locator::udp_v4([127, 0, 0, 1], 41034);
        a.add_peer(pb, &[], &[pb_user]);
        assert!(!a.send(&pb_user, Port::User, b"during discovery"));
        a.report_udp_sends(Duration::ZERO);
        assert!(
            !events
                .snapshot()
                .iter()
                .any(|e| e.name == "shm.channel_udp_sends"),
            "a send within the grace is discovery's normal course"
        );
        std::thread::sleep(ATTACH_GRACE + Duration::from_millis(100));
        assert!(!a.send(&pb_user, Port::User, b"b never opened"));
        a.report_udp_sends(Duration::ZERO);
        let late = events
            .snapshot()
            .iter()
            .filter(|e| e.name == "shm.channel_udp_sends")
            .flat_map(|e| e.attrs.clone())
            .find(|a| a.key == "unattached_late")
            .map(|a| a.value);
        assert_eq!(late.as_deref(), Some("1"));
    }

    /// A peer that opened its end and then takes nothing (a killed
    /// process still counts as attached) gets its channel closed once its
    /// full ring has not moved for `STALL_LIMIT`; its traffic goes over UDP.
    #[test]
    fn a_reader_that_takes_nothing_from_a_full_ring_loses_its_channel() {
        use zerodds_foundation::observability::VecSink;
        let domain = private_domain(5);
        let host = [0xA1, 0xA2, 0xA3, 0xA4];
        let (pa, pb) = (prefix(host, 0xA9), prefix(host, 0xBA));
        let events = Arc::new(VecSink::new());
        let sink: SharedSink = events.clone();
        let a = SameHostChannels::new(pa, domain, 1024, sink).unwrap();
        let b = SameHostChannels::new(
            pb,
            domain,
            1024,
            zerodds_foundation::observability::null_sink(),
        )
        .unwrap();
        let pb_user = Locator::udp_v4([127, 0, 0, 1], 41044);
        a.add_peer(pb, &[], &[pb_user]);
        b.add_peer(pa, &[], &[Locator::udp_v4([127, 0, 0, 1], 41043)]);
        // b never reads: the ring of four datagrams fills.
        let mut sent = 0;
        while a.send(&pb_user, Port::User, &[0u8; 1000]) {
            sent += 1;
            assert!(sent < 10, "the ring never filled");
        }
        assert_eq!(a.len(), 1, "one refusal starts the clock, no more");
        std::thread::sleep(STALL_LIMIT + Duration::from_millis(100));
        assert!(!a.send(&pb_user, Port::User, &[0u8; 1000]));
        assert!(a.is_empty(), "the channel to the stalled reader is closed");
        assert!(
            events
                .snapshot()
                .iter()
                .any(|e| e.name == "shm.channel_closed_stalled_reader")
        );
        assert!(
            !a.send(&pb_user, Port::User, b"over UDP now"),
            "no route any more"
        );
    }

    /// A reader that keeps taking frames, however slowly, keeps its channel.
    #[test]
    fn a_slow_reader_that_moves_on_keeps_its_channel() {
        let domain = private_domain(6);
        let host = [0xB1, 0xB2, 0xB3, 0xB4];
        let (pa, pb) = (prefix(host, 0xAB), prefix(host, 0xBC));
        let sink = zerodds_foundation::observability::null_sink();
        let a = SameHostChannels::new(pa, domain, 1024, sink.clone()).unwrap();
        let b = SameHostChannels::new(pb, domain, 1024, sink).unwrap();
        let pb_user = Locator::udp_v4([127, 0, 0, 1], 41054);
        a.add_peer(pb, &[], &[pb_user]);
        b.add_peer(pa, &[], &[Locator::udp_v4([127, 0, 0, 1], 41053)]);
        let inbound = Arc::clone(&b.snapshot()[0].1.inbound);
        let start = Instant::now();
        while start.elapsed() < STALL_LIMIT * 2 {
            if !a.send(&pb_user, Port::User, &[0u8; 1000]) {
                // Full: the reader takes one frame now and then.
                std::thread::sleep(Duration::from_millis(500));
                let _ = inbound.try_recv().unwrap();
            }
        }
        assert_eq!(a.len(), 1, "a reader that moves on is not stalled");
    }

    /// Closing the channels removes what this side made: the name, flink
    /// and `.lock` of each ring it owns, and its doorbell. Whatever a
    /// process leaves after its participant's `shutdown` is not this.
    #[test]
    fn clear_removes_the_owned_rings_files_and_the_doorbell() {
        let domain = private_domain(7);
        let host = [0xC1, 0xC2, 0xC3, 0xC4];
        let (pa, pb) = (prefix(host, 0xAD), prefix(host, 0xBE));
        let sink = zerodds_foundation::observability::null_sink();
        let a = SameHostChannels::new(pa, domain, 1024, sink.clone()).unwrap();
        let b = SameHostChannels::new(pb, domain, 1024, sink).unwrap();
        a.add_peer(pb, &[], &[Locator::udp_v4([127, 0, 0, 1], 41064)]);
        b.add_peer(pa, &[], &[Locator::udp_v4([127, 0, 0, 1], 41063)]);
        let dir = crate::same_host_shm::shm_config_for_pair(pa, domain).flink_dir;
        let hex = |id: [u8; 16]| {
            id.iter()
                .map(|b| alloc::format!("{b:02x}"))
                .collect::<String>()
        };
        let flink = dir.join(alloc::format!(
            "{}-{}.shm",
            hex(channel_id(pa, domain)),
            hex(channel_id(pb, domain))
        ));
        let os_id = std::fs::read_to_string(&flink).expect("a's ring has a flink");
        // The pair lock lies in the pair directory under the segment's name
        // (zerodds-shm-transport-1.0 §5.4).
        let lock = zerodds_transport_shm::posix::pair_dir()
            .unwrap()
            .join(alloc::format!("{}.lock", &os_id[1..]));
        assert!(lock.exists());
        let bell_weak = Arc::downgrade(&a.receive_bell().expect("a opened its doorbell"));

        a.clear();
        assert!(!flink.exists(), "the flink of a's ring goes");
        assert!(!lock.exists(), "and its lock file");
        #[cfg(target_os = "linux")]
        assert!(
            !std::path::Path::new("/dev/shm").join(&os_id[1..]).exists(),
            "and the segment"
        );
        let _ = os_id;
        assert!(bell_weak.upgrade().is_none(), "a's doorbell is released");
        drop(b);
    }

    /// The receive thread of a participant without a channel waits on
    /// [`SameHostChannels::wait_for_receive_bell`]: `wake_receiver`, which
    /// `shutdown` calls, ends the wait, and so does a wake that came between
    /// the thread's read of the count and its wait.
    #[test]
    fn wake_receiver_ends_the_wait_of_a_receiver_without_a_doorbell() {
        let host = [0xD1, 0xD2, 0xD3, 0xD4];
        let sink = zerodds_foundation::observability::null_sink();
        let a = Arc::new(
            SameHostChannels::new(prefix(host, 0xA8), private_domain(8), 1024, sink).unwrap(),
        );
        assert!(a.receive_bell().is_none(), "no channel, no doorbell");

        let seen = a.receiver_wakes();
        let waiting = Arc::clone(&a);
        let waiter = std::thread::spawn(move || {
            let start = Instant::now();
            waiting.wait_for_receive_bell(seen, Duration::from_secs(10));
            start.elapsed()
        });
        std::thread::sleep(Duration::from_millis(50));
        a.wake_receiver();
        let took = waiter.join().unwrap();
        assert!(
            took >= Duration::from_millis(40),
            "it waited for the wake: {took:?}"
        );
        assert!(
            took < Duration::from_secs(5),
            "the wake ended the wait: {took:?}"
        );

        let seen = a.receiver_wakes();
        a.wake_receiver();
        let start = Instant::now();
        a.wait_for_receive_bell(seen, Duration::from_secs(10));
        let took = start.elapsed();
        assert!(
            took < Duration::from_secs(1),
            "a wake before the wait is kept: {took:?}"
        );
    }

    /// The first channel opens the doorbell, and the receive thread that
    /// waited for one goes on to park on it at once.
    #[test]
    fn the_first_doorbell_ends_the_wait_of_the_receiver() {
        let domain = private_domain(9);
        let host = [0xE1, 0xE2, 0xE3, 0xE4];
        let (pa, pb) = (prefix(host, 0xA9), prefix(host, 0xB9));
        let sink = zerodds_foundation::observability::null_sink();
        let a = Arc::new(SameHostChannels::new(pa, domain, 1024, sink.clone()).unwrap());
        let b = SameHostChannels::new(pb, domain, 1024, sink).unwrap();

        let seen = a.receiver_wakes();
        let waiting = Arc::clone(&a);
        let waiter = std::thread::spawn(move || {
            let start = Instant::now();
            waiting.wait_for_receive_bell(seen, Duration::from_secs(10));
            start.elapsed()
        });
        std::thread::sleep(Duration::from_millis(50));
        b.add_peer(pa, &[], &[Locator::udp_v4([127, 0, 0, 1], 41091)]);
        a.add_peer(pb, &[], &[Locator::udp_v4([127, 0, 0, 1], 41092)]);
        let took = waiter.join().unwrap();
        assert!(
            a.receive_bell().is_some(),
            "the channel opened a's doorbell"
        );
        assert!(
            took < Duration::from_secs(5),
            "the doorbell ended the wait: {took:?}"
        );
        a.clear();
        b.clear();
    }

    /// What a broken or hostile owner leaves in `ring`: a length above half
    /// the ring at its `head`, published with 8 bytes. Written through the
    /// segment's file in `/dev/shm`, as another process would write it.
    #[cfg(target_os = "linux")]
    pub(crate) fn publish_corrupt_length(ring: &PosixShmTransport) {
        use std::os::unix::fs::FileExt;
        let path =
            std::path::Path::new("/dev/shm").join(ring.segment_name().trim_start_matches('/'));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        let mut word = [0u8; 8];
        file.read_exact_at(&mut word, 8).unwrap();
        let capacity = u64::from_le_bytes(word);
        file.read_exact_at(&mut word, 16).unwrap();
        let head = u64::from_ne_bytes(word);
        let length = u32::try_from(capacity).unwrap();
        file.write_all_at(&length.to_le_bytes(), 64 + head % capacity)
            .unwrap();
        file.write_all_at(&(head + 8).to_ne_bytes(), 16).unwrap();
    }

    /// zerodds-shm-transport-1.0 §3.3: a frame an inbound ring drops as
    /// corrupt is counted and filed as `shm.corrupt_frame`, at level
    /// error, with the peer and the number dropped; the peer's next frame
    /// arrives. Before, the ring counted it and nobody read the count.
    #[cfg(target_os = "linux")]
    #[test]
    fn shm_3_3_a_corrupt_frame_is_reported_as_an_event() {
        use zerodds_foundation::observability::VecSink;
        let domain = private_domain(0);
        let host = [0x91, 0x92, 0x93, 0x94];
        let (pa, pb) = (prefix(host, 0xA9), prefix(host, 0xBA));
        let events = Arc::new(VecSink::new());
        let sink: SharedSink = events.clone();
        let a = SameHostChannels::new(
            pa,
            domain,
            1024,
            zerodds_foundation::observability::null_sink(),
        )
        .unwrap();
        let b = SameHostChannels::new(pb, domain, 1024, sink).unwrap();
        let pb_user = Locator::udp_v4([127, 0, 0, 1], 41102);
        a.add_peer(pb, &[], &[pb_user]);
        b.add_peer(pa, &[], &[Locator::udp_v4([127, 0, 0, 1], 41101)]);
        let inbound = Arc::clone(&b.snapshot()[0].1.inbound);

        publish_corrupt_length(&inbound);
        assert!(
            matches!(b.receive(pa, &inbound), Ok(None)),
            "nothing handed up"
        );
        assert_eq!(b.corrupt_frames(), 1);
        let filed: Vec<(Level, Vec<(&str, String)>)> = events
            .snapshot()
            .iter()
            .filter(|e| e.name == "shm.corrupt_frame")
            .map(|e| {
                (
                    e.level,
                    e.attrs.iter().map(|a| (a.key, a.value.clone())).collect(),
                )
            })
            .collect();
        assert_eq!(
            filed,
            vec![(
                Level::Error,
                vec![("peer", alloc::format!("{pa:?}")), ("count", "1".into())]
            )]
        );

        assert!(a.send(&pb_user, Port::User, b"after"));
        let got = b.receive(pa, &inbound).unwrap().expect("the next frame");
        assert_eq!(&got.data[..], b"\x01after");
        assert_eq!(b.corrupt_frames(), 1, "counted once");
        a.clear();
        b.clear();
    }

    /// A channel is never visible without the routes to it: `add_peer`
    /// sets them before it puts the channel in, `remove_peer` and `clear`
    /// take both under one lock. Before, a thread between the two steps
    /// left an open, attached channel no send was routed to; under load
    /// `same_host_e2e::flatdata_10_3_e2e_two_runtimes_shm_roundtrip` then
    /// sent its SEDP and its sample over UDP (17 of about 15 000 parallel
    /// runs on the Linux test host, `routes []` in the report; OPEN-ITEMS
    /// 132).
    #[test]
    fn a_channel_is_never_visible_without_its_routes() {
        use core::sync::atomic::AtomicBool;
        let domain = private_domain(0);
        let host = [0xD1, 0xD2, 0xD3, 0xD4];
        let (pa, pb) = (prefix(host, 0xAB), prefix(host, 0xBC));
        let sink = zerodds_foundation::observability::null_sink();
        let a = Arc::new(SameHostChannels::new(pa, domain, 1024, sink).unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let observer = {
            let (a, stop) = (Arc::clone(&a), Arc::clone(&stop));
            std::thread::spawn(move || {
                let (mut looks, mut without) = (0u64, 0u64);
                while !stop.load(Ordering::Relaxed) {
                    let (open, routes) = a.channel_and_routes(pb);
                    looks += u64::from(open);
                    without += u64::from(open && routes == 0);
                }
                (looks, without)
            })
        };
        let (meta, user) = (
            Locator::udp_v4([127, 0, 0, 1], 41111),
            Locator::udp_v4([127, 0, 0, 1], 41112),
        );
        for _ in 0..2000 {
            a.add_peer(pb, &[meta], &[user]);
            a.add_peer(pb, &[meta], &[user]);
            a.remove_peer(pb);
        }
        a.add_peer(pb, &[meta], &[user]);
        a.clear();
        stop.store(true, Ordering::Relaxed);
        let (looks, without) = observer.join().unwrap();
        assert!(looks > 0, "the observer saw the channel open");
        assert_eq!(
            without, 0,
            "{without} of {looks} looks saw it without routes"
        );
    }

    /// `zerodds-shm-transport-1.0` §5.3: a participant that closes its
    /// channel to a peer and opens it again, with nobody else holding the
    /// old one, makes the ring anew. The peer's inbound ring ends on the
    /// old owner's shutdown flag, the first send goes over UDP (the peer is
    /// not on the new ring yet), and after the peer's rebind the frames go
    /// through the new ring.
    #[test]
    fn shm_5_3_the_peer_follows_a_channel_made_anew() {
        let domain = private_domain(10);
        let host = [0xE1, 0xE2, 0xE3, 0xE4];
        let (pa, pb) = (prefix(host, 0xA5), prefix(host, 0xB6));
        let sink = zerodds_foundation::observability::null_sink();
        let a = SameHostChannels::new(pa, domain, 1024, sink.clone()).unwrap();
        let b = SameHostChannels::new(pb, domain, 1024, sink).unwrap();
        let pb_user = Locator::udp_v4([127, 0, 0, 1], 41102);
        a.add_peer(pb, &[], &[pb_user]);
        b.add_peer(pa, &[], &[Locator::udp_v4([127, 0, 0, 1], 41101)]);
        assert!(a.send(&pb_user, Port::User, b"first"));
        let old = Arc::clone(&b.snapshot()[0].1.inbound);
        assert_eq!(
            &b.receive(pa, &old).unwrap().expect("first").data[..],
            b"\x01first"
        );

        a.remove_peer(pb);
        assert!(
            b.receive(pa, &old).is_err(),
            "the old owner's drop ended the ring"
        );
        a.add_peer(pb, &[], &[pb_user]);
        assert!(
            !a.send(&pb_user, Port::User, b"early"),
            "b is not on the new ring yet"
        );
        assert!(b.rebind_inbound(pa, &old));
        let new = Arc::clone(&b.snapshot()[0].1.inbound);
        assert!(!Arc::ptr_eq(&new, &old));
        assert!(a.send(&pb_user, Port::User, b"second"));
        assert_eq!(
            &b.receive(pa, &new).unwrap().expect("second").data[..],
            b"\x01second"
        );
        a.clear();
        b.clear();
    }

    /// `zerodds-shm-transport-1.0` §5.3: a channel opened again while a
    /// send still holds the old one takes the old ring over. The peer keeps
    /// its inbound ring; the frame of the send that held the old channel
    /// goes through the new one, since the old owner refuses it; and the
    /// old owner's drop leaves the ring to the new one. Before, the two
    /// owners pushed into one ring at once.
    #[test]
    fn shm_5_3_a_send_on_a_superseded_channel_goes_through_the_new_one() {
        let domain = private_domain(11);
        let host = [0xE5, 0xE6, 0xE7, 0xE8];
        let (pa, pb) = (prefix(host, 0xA7), prefix(host, 0xB8));
        let sink = zerodds_foundation::observability::null_sink();
        let a = SameHostChannels::new(pa, domain, 1024, sink.clone()).unwrap();
        let b = SameHostChannels::new(pb, domain, 1024, sink).unwrap();
        let pb_user = Locator::udp_v4([127, 0, 0, 1], 41112);
        a.add_peer(pb, &[], &[pb_user]);
        b.add_peer(pa, &[], &[Locator::udp_v4([127, 0, 0, 1], 41111)]);
        let inbound = Arc::clone(&b.snapshot()[0].1.inbound);

        // A send in flight holds the channel.
        let held = Arc::clone(&a.snapshot()[0].1);
        a.remove_peer(pb);
        a.add_peer(pb, &[], &[pb_user]);
        assert!(held.outbound.is_superseded());
        assert!(
            matches!(b.receive(pa, &inbound), Ok(None)),
            "the ring goes on"
        );
        assert!(a.send_on(pb, Arc::clone(&held), Port::User, b"held"));
        assert!(a.send(&pb_user, Port::User, b"new"));
        assert_eq!((a.frames_sent(Port::User), a.fallbacks()), (2, 0));
        drop(held);
        assert!(a.send(&pb_user, Port::User, b"after"));
        for want in [&b"\x01held"[..], b"\x01new", b"\x01after"] {
            assert_eq!(
                &b.receive(pa, &inbound).unwrap().expect("a frame").data[..],
                want
            );
        }
        a.clear();
        b.clear();
    }

    #[test]
    fn channel_ids_differ_by_prefix_and_domain_and_from_the_shm_user_transport() {
        let a = GuidPrefix::from_bytes([1; 12]);
        let b = GuidPrefix::from_bytes([2; 12]);
        assert_ne!(channel_id(a, 7), channel_id(b, 7));
        assert_ne!(channel_id(a, 7), channel_id(a, 8));
        let mut shm_user = [0u8; 16];
        shm_user[..12].copy_from_slice(&a.to_bytes());
        shm_user[12..].copy_from_slice(&7u32.to_be_bytes());
        assert_ne!(channel_id(a, 7), shm_user);
    }
}
