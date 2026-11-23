// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The SHM transport over memory the integrator gives, for targets without
//! an operating system (§6.4): RAM two cores of one chip share, a page a
//! hypervisor shares between two partitions. The ring is
//! [`crate::ring`]'s, byte for byte the one [`crate::posix`] maps from a
//! named segment.
//!
//! What an operating system does for the named segments (§5.1, §5.2, §5.4,
//! §5.5: names, flinks, the unlink before a create, the pair lock, the
//! liveness lock) has no counterpart here. The integrator places one region
//! per (owner, consumer) pair at an address both sides map, and starts at
//! most one owner and one consumer on it. In their place:
//!
//! - The owner, at its start, makes the ring anew where the region holds
//!   no ring of this layout and capacity (first start, other contents).
//!   Where it holds one, the owner goes on with it and marks where its
//!   session begins (`session_head`, offset 48): at `head` when the
//!   previous owner ended without its drop (its `shutdown` is 0, a core
//!   that was reset), so that its frames are not delivered (as §5.5 has it
//!   for a named segment); at the consumer's `tail` when the previous owner
//!   dropped its side (`shutdown` 1), so that its frames still arrive (as
//!   §5.3 has it). Then it counts `owner_epoch` up.
//! - A consumer skips to the session's start when it attaches and whenever
//!   it sees `owner_epoch` change.
//! - The shutdown flag of §5.3 holds as it is: the owner's drop sets it
//!   and wakes the consumer, whose `recv` returns the targeted error, also
//!   from a park. An owner that stops without its drop leaves it 0; a
//!   consumer cannot tell that from an owner that has nothing to send.
//! - The owner makes the ring (§4.3 does not hold here): a consumer that
//!   comes first gets [`RingError::NotReady`] and opens again later.
//!   Without an operating system there is no lock that would order two
//!   sides that write the header at the same time.
//!
//! How a side waits for the other is the integrator's ([`RingWait`]): an
//! event between cores (`WFE`/`SEV` on Arm), an interrupt a hypervisor
//! routes, or [`SpinWait`], which looks at the ring in a loop. A park ends
//! on a frame and on the owner's drop ([`Parked`]).

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use core::time::Duration;

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

pub use crate::ring::ShmRole;
use crate::ring::{HEADER_BYTES, MAX_CAPACITY, MIN_CAPACITY, Next, Push, Ring, RingError};

/// What a consumer parked in `recv` waits for: a frame, which moves the low
/// 32 bits of `head` away from what the consumer saw, or the owner's drop,
/// which sets the shutdown flag (§5.3) and leaves `head` as it is.
#[derive(Debug)]
pub struct Parked<'a> {
    word: &'a AtomicU32,
    seen: u32,
    shutdown: &'a AtomicU32,
    /// [`Self::is_over`] returned `true` during the park: what `recv` asks
    /// after a park that returned `true`.
    found_over: AtomicBool,
}

impl<'a> Parked<'a> {
    fn new(word: &'a AtomicU32, seen: u32, shutdown: &'a AtomicU32) -> Self {
        Self {
            word,
            seen,
            shutdown,
            found_over: AtomicBool::new(false),
        }
    }

    /// The word the owner wakes after each frame and in its drop: the low
    /// 32 bits of `head`. A wait that sleeps on an address (a futex, a
    /// monitor on the cache line) sleeps on this one.
    #[must_use]
    pub fn word(&self) -> &'a AtomicU32 {
        self.word
    }

    /// What [`Self::word`] held when the consumer last looked at the ring.
    #[must_use]
    pub fn seen(&self) -> u32 {
        self.seen
    }

    /// Whether the wait is over: [`Self::word`] no longer holds
    /// [`Self::seen`], or the owner has set the shutdown flag.
    #[must_use]
    pub fn is_over(&self) -> bool {
        let over = self.word.load(Ordering::Acquire) != self.seen
            || self.shutdown.load(Ordering::Acquire) != 0;
        if over {
            self.found_over.store(true, Ordering::Release);
        }
        over
    }

    /// Whether [`Self::is_over`] returned `true` since the park began. The
    /// flag stays when the shutdown flag goes back to 0 (a next owner).
    fn found_over(&self) -> bool {
        self.found_over.load(Ordering::Acquire)
    }
}

/// How one side of a region waits for the other.
pub trait RingWait {
    /// Waits until `on` [is over](Parked::is_over) and returns `true`, or
    /// until `timeout` has passed and returns `false` (`None`: no limit). It
    /// returns at no other time: where what it waits on wakes it before
    /// either (an event or an interrupt sent for something else, a
    /// [`Self::wake`] of another park), it asks [`Parked::is_over`] again
    /// and waits for the rest of `timeout`, which only it can measure; the
    /// region has no clock. It returns `true` only after
    /// [`Parked::is_over`] returned `true` during this park.
    ///
    /// A park that returns `true` before that, against this contract,
    /// ends a `recv` that has a timeout with [`RecvError::Timeout`], since
    /// the `recv` cannot tell how much of the timeout has passed; a `recv`
    /// without a timeout parks again.
    ///
    /// The owner's drop sets the shutdown flag and then wakes, without a
    /// change of [`Parked::word`]: a wait that looks only at that word for
    /// a change waits out its timeout there.
    fn park(&self, on: &Parked<'_>, timeout: Option<Duration>) -> bool;

    /// Wakes the side parked on `word`. The owner calls it after each frame
    /// it publishes and in its drop, after it set the shutdown flag.
    fn wake(&self, word: &AtomicU32);
}

/// Waits by looking at the ring in a loop ([`Parked::is_over`]). It has no
/// clock: a timeout is `spins_per_ms` looks per millisecond, which the
/// integrator measures for its core. A wake is a no-op: what ends the wait
/// is a change the loop sees, a frame's `head` or the owner's shutdown
/// flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpinWait {
    /// Looks at the ring per millisecond of a timeout.
    pub spins_per_ms: u32,
}

impl RingWait for SpinWait {
    fn park(&self, on: &Parked<'_>, timeout: Option<Duration>) -> bool {
        let limit = timeout.map(|t| {
            let ms = u64::try_from(t.as_millis()).unwrap_or(u64::MAX);
            ms.saturating_mul(u64::from(self.spins_per_ms)).max(1)
        });
        let mut looks: u64 = 0;
        loop {
            if on.is_over() {
                return true;
            }
            if limit.is_some_and(|limit| looks >= limit) {
                return false;
            }
            looks += 1;
            core::hint::spin_loop();
        }
    }

    fn wake(&self, _word: &AtomicU32) {}
}

/// Configuration of one side of a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionConfig {
    /// The largest datagram; the ring must hold two and their headers
    /// (`2 * max_datagram + 16 <= capacity`). A consumer lowers it to what
    /// the ring holds.
    pub max_datagram: usize,
    /// How long a `recv` waits for a frame (`None`: no limit).
    pub recv_timeout: Option<Duration>,
}

/// Why a side cannot use a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegionShmError {
    /// The region holds no ring this side can use, or cannot hold one.
    Ring(RingError),
    /// The configuration does not fit the region.
    InvalidConfig {
        /// Why.
        reason: &'static str,
    },
}

impl core::fmt::Display for RegionShmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ring(e) => write!(f, "{e}"),
            Self::InvalidConfig { reason } => write!(f, "shm region config invalid: {reason}"),
        }
    }
}

impl From<RingError> for RegionShmError {
    fn from(e: RingError) -> Self {
        Self::Ring(e)
    }
}

/// Sends that may spin on a full ring before the send fails.
const SPIN_LIMIT: u32 = 1024;

/// One side of a ring in a caller's region.
pub struct RegionShmTransport<W: RingWait> {
    ring: Ring,
    role: ShmRole,
    local_locator: Locator,
    peer_locator: Locator,
    max_datagram: usize,
    recv_timeout: Option<Duration>,
    wait: W,
    /// The owner epoch this owner opened under; for a consumer the last one
    /// it saw.
    epoch: AtomicU32,
    /// A send is running (the owner): a second one at the same time fails
    /// instead of racing the first on `head`.
    sending: AtomicBool,
    padding_frames: AtomicU32,
    corrupt_frames: AtomicU32,
}

impl<W: RingWait> core::fmt::Debug for RegionShmTransport<W> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RegionShmTransport")
            .field("role", &self.role)
            .field("local_locator", &self.local_locator)
            .field("peer_locator", &self.peer_locator)
            .field("capacity", &self.ring.capacity())
            .finish()
    }
}

/// The data region a region of `len` bytes holds.
fn capacity_of(len: usize) -> Result<usize, RegionShmError> {
    let capacity = len.saturating_sub(HEADER_BYTES).min(MAX_CAPACITY);
    if capacity < MIN_CAPACITY {
        return Err(RegionShmError::Ring(RingError::TooSmall));
    }
    Ok(capacity)
}

impl<W: RingWait> RegionShmTransport<W> {
    /// The owner (sole writer) of the ring in `region`. `local_id` names
    /// this side, `peer_id` the consumer, as the GUID prefixes of the
    /// participants do for a named segment.
    ///
    /// # Errors
    /// The region is misaligned or too small, or `max_datagram` does not
    /// fit it.
    ///
    /// # Safety
    /// `region` points to `len` bytes, aligned to 8, that the consumer maps
    /// as well, that stay mapped while the transport lives, and that only
    /// this pair's two sides use; no other owner uses the region at the same
    /// time.
    pub unsafe fn open_owner(
        region: *mut u8,
        len: usize,
        local_id: [u8; 16],
        peer_id: [u8; 16],
        config: RegionConfig,
        wait: W,
    ) -> Result<Self, RegionShmError> {
        let capacity = capacity_of(len)?;
        if config.max_datagram.saturating_mul(2).saturating_add(16) > capacity {
            return Err(RegionShmError::InvalidConfig {
                reason: "the region must hold 2 * max_datagram + 16 bytes behind its header",
            });
        }
        // SAFETY: as the caller guarantees.
        let ring = match unsafe { Ring::attach(region, len) } {
            Ok(ring) if ring.capacity() == capacity => {
                let ended_cleanly = ring.shutdown().load(Ordering::Acquire) != 0;
                let start = if ended_cleanly {
                    ring.head_position().wrapping_sub(ring.occupied())
                } else {
                    ring.head_position()
                };
                ring.store_session_head(start);
                ring.shutdown().store(0, Ordering::Release);
                ring
            }
            Ok(_) | Err(RingError::NotReady | RingError::InvalidHeader) => {
                // SAFETY: as the caller guarantees; no owner but this one.
                unsafe { Ring::create(region, len, capacity) }?
            }
            Err(e) => return Err(e.into()),
        };
        // After `session_head`: a consumer that sees the new epoch sees it.
        let epoch = ring
            .owner_epoch()
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        Ok(Self::new(
            ring,
            ShmRole::Owner,
            local_id,
            peer_id,
            config,
            wait,
            epoch,
        ))
    }

    /// The consumer (sole reader) of the ring in `region`. `peer_id` names
    /// the owner.
    ///
    /// # Errors
    /// [`RingError::NotReady`] while the owner has not made the ring; the
    /// region is misaligned or too small, or holds a ring of another
    /// layout.
    ///
    /// # Safety
    /// As [`Self::open_owner`], with no other consumer at the same time.
    pub unsafe fn open_consumer(
        region: *mut u8,
        len: usize,
        local_id: [u8; 16],
        peer_id: [u8; 16],
        mut config: RegionConfig,
        wait: W,
    ) -> Result<Self, RegionShmError> {
        // SAFETY: as the caller guarantees.
        let ring = unsafe { Ring::attach(region, len) }?;
        let capacity = ring.capacity();
        if config.max_datagram.saturating_mul(2).saturating_add(16) > capacity {
            config.max_datagram = (capacity - 16) / 2;
        }
        let epoch = ring.owner_epoch().load(Ordering::Acquire);
        ring.skip_to_session();
        ring.consumer_attached().fetch_add(1, Ordering::AcqRel);
        Ok(Self::new(
            ring,
            ShmRole::Consumer,
            local_id,
            peer_id,
            config,
            wait,
            epoch,
        ))
    }

    fn new(
        ring: Ring,
        role: ShmRole,
        local_id: [u8; 16],
        peer_id: [u8; 16],
        config: RegionConfig,
        wait: W,
        epoch: u32,
    ) -> Self {
        Self {
            ring,
            role,
            local_locator: Locator::shm(local_id),
            peer_locator: Locator::shm(peer_id),
            max_datagram: config.max_datagram,
            recv_timeout: config.recv_timeout,
            wait,
            epoch: AtomicU32::new(epoch),
            sending: AtomicBool::new(false),
            padding_frames: AtomicU32::new(0),
            corrupt_frames: AtomicU32::new(0),
        }
    }

    /// The peer's locator.
    #[must_use]
    pub fn peer_locator(&self) -> Locator {
        self.peer_locator
    }

    /// Whether a consumer has the ring open, from the owner's side.
    #[must_use]
    pub fn peer_attached(&self) -> bool {
        self.ring.consumer_attached().load(Ordering::Acquire) > 0
    }

    /// Whether another owner opened the region since this one (§5.3): its
    /// sends fail.
    #[must_use]
    pub fn is_superseded(&self) -> bool {
        self.role == ShmRole::Owner
            && self.ring.owner_epoch().load(Ordering::Acquire) != self.epoch.load(Ordering::Relaxed)
    }

    /// Padding frames the consumer skipped.
    #[must_use]
    pub fn padding_frames_seen(&self) -> u64 {
        u64::from(self.padding_frames.load(Ordering::Relaxed))
    }

    /// Frames the consumer dropped as corrupt (§3.3).
    #[must_use]
    pub fn corrupt_frames_seen(&self) -> u64 {
        u64::from(self.corrupt_frames.load(Ordering::Relaxed))
    }

    /// Bytes published and not yet read.
    #[must_use]
    pub fn occupied_bytes(&self) -> u64 {
        self.ring.occupied()
    }

    /// Sends `prefix` and `data` as one frame.
    ///
    /// # Errors
    /// As [`Transport::send`]; a send while another one of this side runs
    /// fails as well.
    pub fn send_with_prefix(
        &self,
        dest: &Locator,
        prefix: &[u8],
        data: &[u8],
    ) -> Result<(), SendError> {
        if self.role != ShmRole::Owner {
            return Err(SendError::Io {
                message: "shm consumer cannot send on this region",
            });
        }
        if dest.kind != LocatorKind::Shm || *dest != self.peer_locator {
            return Err(SendError::UnsupportedLocator);
        }
        let size = prefix.len().saturating_add(data.len());
        if size > self.max_datagram {
            return Err(SendError::PayloadTooLarge {
                size,
                limit: self.max_datagram,
            });
        }
        if self
            .sending
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Err(SendError::Io {
                message: "shm region: another send of this owner is running",
            });
        }
        let result = self.push(prefix, data);
        self.sending.store(false, Ordering::Release);
        if result.is_ok() {
            self.wait.wake(self.ring.head_word());
        }
        result
    }

    fn push(&self, prefix: &[u8], data: &[u8]) -> Result<(), SendError> {
        if self.is_superseded() {
            return Err(SendError::Io {
                message: "shm owner superseded: a later owner sends on this region",
            });
        }
        let mut spins: u32 = 0;
        loop {
            let full = match self.ring.try_push(prefix, data) {
                Push::Pushed => return Ok(()),
                Push::Full => "shm ring full, reader too slow",
                Push::FullAtWrap => "shm ring full near wraparound",
                Push::OutsideTheRing => {
                    return Err(SendError::Io {
                        message: "shm ring position outside the ring",
                    });
                }
                Push::TooLarge => {
                    return Err(SendError::PayloadTooLarge {
                        size: prefix.len().saturating_add(data.len()),
                        limit: self.max_datagram,
                    });
                }
            };
            spins = spins.saturating_add(1);
            if spins > SPIN_LIMIT {
                return Err(SendError::Io { message: full });
            }
            core::hint::spin_loop();
        }
    }

    /// Takes the next frame off the ring; first, when the owner's epoch
    /// changed, skips to the new session's start.
    fn pop(&self) -> Option<Vec<u8>> {
        let epoch = self.ring.owner_epoch().load(Ordering::Acquire);
        if epoch != self.epoch.load(Ordering::Relaxed) {
            self.epoch.store(epoch, Ordering::Relaxed);
            self.ring.skip_to_session();
        }
        let mut paddings = 0;
        let next = self.ring.next(&mut paddings);
        if paddings > 0 {
            self.padding_frames.fetch_add(paddings, Ordering::Relaxed);
        }
        match next {
            Next::Empty => None,
            Next::Corrupt => {
                self.corrupt_frames.fetch_add(1, Ordering::Relaxed);
                None
            }
            Next::Frame { offset, len, end } => {
                let mut out = alloc::vec![0u8; len];
                if !self.ring.read(offset, &mut out) {
                    let _ = self.ring.drop_published();
                    self.corrupt_frames.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
                self.ring.release(end);
                Some(out)
            }
        }
    }

    /// The next frame, without waiting: `Ok(None)` on an empty ring.
    ///
    /// # Errors
    /// The owner dropped its side (`Io`), the frame was corrupt (`Io`), or
    /// this is the owner's side.
    pub fn try_recv(&self) -> Result<Option<ReceivedDatagram>, RecvError> {
        if self.role != ShmRole::Consumer {
            return Err(RecvError::Io {
                message: "shm owner cannot recv on this region",
            });
        }
        let corrupt = self.corrupt_frames_seen();
        if let Some(data) = self.pop() {
            return Ok(Some(ReceivedDatagram {
                source: self.peer_locator,
                data: data.into(),
            }));
        }
        if self.corrupt_frames_seen() != corrupt {
            return Err(RecvError::Io {
                message: "shm corrupt frame dropped",
            });
        }
        if self.ring.shutdown().load(Ordering::Acquire) != 0 {
            return Err(RecvError::Io {
                message: "shm owner terminated",
            });
        }
        Ok(None)
    }
}

impl<W: RingWait> Transport for RegionShmTransport<W> {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        self.send_with_prefix(dest, &[], data)
    }

    /// The datagram limit of the ring ([`RegionConfig::max_datagram`]).
    fn max_message_size(&self) -> usize {
        self.max_datagram
    }

    /// Waits for the next frame with the [`RingWait`] up to
    /// [`RegionConfig::recv_timeout`]; the owner's drop ends the wait with
    /// the targeted error (§5.3). A park that returns before its wait is
    /// over and before its timeout ran out (against the contract of
    /// [`RingWait::park`]) ends a `recv` with a timeout with `Timeout`; a
    /// `recv` without one parks again.
    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        loop {
            // Read before the look at the ring: a frame published between the
            // look and the park changes the word, and the park returns. A
            // drop between them has set the flag, which the park sees.
            let seen = self.ring.head_word().load(Ordering::Acquire);
            if let Some(datagram) = self.try_recv()? {
                return Ok(datagram);
            }
            let on = Parked::new(self.ring.head_word(), seen, self.ring.shutdown());
            let returned_true = self.wait.park(&on, self.recv_timeout);
            // The ring changed (a frame, the owner's drop, a next owner):
            // look at it again, with the whole timeout. A park that returned
            // `true` without that would start the timeout anew on every
            // turn, so a `recv` with a timeout ends there.
            if returned_true && (on.found_over() || self.recv_timeout.is_none()) {
                continue;
            }
            return self.try_recv()?.ok_or(RecvError::Timeout);
        }
    }

    fn local_locator(&self) -> Locator {
        self.local_locator
    }
}

impl<W: RingWait> Drop for RegionShmTransport<W> {
    fn drop(&mut self) {
        match self.role {
            ShmRole::Owner => {
                if !self.is_superseded() {
                    self.ring.shutdown().store(1, Ordering::Release);
                    self.wait.wake(self.ring.head_word());
                }
            }
            ShmRole::Consumer => {
                let _ = self.ring.consumer_attached().fetch_update(
                    Ordering::AcqRel,
                    Ordering::Acquire,
                    |n| n.checked_sub(1),
                );
            }
        }
    }
}

// Also without the feature `std`: the tests of the crate built for a core
// without 64-bit atomics (tools/shm-ring-armv5te.sh) take threads from the
// test harness's `std`.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    extern crate std;

    use super::*;
    use std::sync::Arc;

    /// A region the test owns: 8-aligned words, shared by the two sides.
    struct Region(Vec<u64>);

    impl Region {
        fn new(capacity: usize) -> Arc<Self> {
            Arc::new(Self(alloc::vec![0u64; (HEADER_BYTES + capacity) / 8]))
        }
        fn base(&self) -> *mut u8 {
            self.0.as_ptr().cast::<u8>().cast_mut()
        }
        fn len(&self) -> usize {
            self.0.len() * 8
        }
    }

    const OWNER: [u8; 16] = [1; 16];
    const CONSUMER: [u8; 16] = [2; 16];
    const WAIT: SpinWait = SpinWait {
        spins_per_ms: 100_000,
    };

    fn config() -> RegionConfig {
        RegionConfig {
            max_datagram: 1024,
            recv_timeout: Some(Duration::from_secs(5)),
        }
    }

    fn owner(region: &Region) -> RegionShmTransport<SpinWait> {
        // SAFETY: the region outlives the transport in every test, and each
        // test opens one owner and one consumer at a time.
        unsafe {
            RegionShmTransport::open_owner(
                region.base(),
                region.len(),
                OWNER,
                CONSUMER,
                config(),
                WAIT,
            )
        }
        .unwrap()
    }

    fn consumer(region: &Region) -> RegionShmTransport<SpinWait> {
        // SAFETY: as in `owner`.
        unsafe {
            RegionShmTransport::open_consumer(
                region.base(),
                region.len(),
                CONSUMER,
                OWNER,
                config(),
                WAIT,
            )
        }
        .unwrap()
    }

    fn frame(i: u32) -> Vec<u8> {
        let mut f = i.to_le_bytes().to_vec();
        f.resize(4 + (i as usize * 13) % 600, i as u8);
        f
    }

    /// §6.4: an owner and a consumer on two threads (two cores of the host)
    /// carry 5 000 frames of 4 to 603 bytes through a 4 KiB ring in a
    /// region the test gives, in order and whole, across the ring's end and
    /// across 2³², where the low word of a position wraps (on a core
    /// without 64-bit atomics the positions are two words, `ring::split`);
    /// the consumer waits with the integrator's wait, here [`SpinWait`].
    #[test]
    fn shm_6_4_a_ring_over_a_caller_supplied_region_carries_frames() {
        let region = Region::new(4096);
        // A ring whose positions stand 600 000 bytes below 2³²: the owner
        // goes on with it (same layout and capacity) from there.
        // SAFETY: the region is the test's, 8-aligned, `len` bytes.
        let made = unsafe { Ring::create(region.base(), region.len(), 4096) }.unwrap();
        let start = (1u64 << 32) - 600_000;
        made.store_head(start);
        made.store_tail(start);
        let sender = owner(&region);
        assert_eq!(sender.ring.head_position(), start);
        let receiver = consumer(&region);
        assert!(sender.peer_attached());
        let reader = {
            let region = Arc::clone(&region);
            std::thread::spawn(move || {
                let _keep = region;
                for i in 0..5000u32 {
                    let got = receiver.recv().expect("a frame");
                    assert_eq!(&got.data[..], &frame(i)[..], "frame {i}");
                    assert_eq!(got.source, Locator::shm(OWNER));
                }
                assert!(receiver.padding_frames_seen() > 0, "across the ring's end");
                assert_eq!(receiver.corrupt_frames_seen(), 0);
                assert!(
                    receiver.ring.tail_position() > 1 << 32,
                    "across the low word's wrap: {}",
                    receiver.ring.tail_position()
                );
                receiver
            })
        };
        for i in 0..5000u32 {
            loop {
                match sender.send(&Locator::shm(CONSUMER), &frame(i)) {
                    Ok(()) => break,
                    Err(SendError::Io { message }) if message.starts_with("shm ring full") => {
                        std::thread::yield_now();
                    }
                    Err(e) => panic!("frame {i}: {e:?}"),
                }
            }
        }
        let receiver = reader.join().unwrap();
        drop(sender);
        assert!(matches!(
            receiver.recv(),
            Err(RecvError::Io {
                message: "shm owner terminated"
            })
        ));
    }

    /// §6.4: an owner that ends without its drop (its core reset) leaves
    /// frames in the ring; the next owner's session starts behind them, and
    /// the consumer receives only the new session's frames. An owner that
    /// drops its side leaves its frames to the consumer (§5.3), which
    /// receives them before the next owner's.
    #[test]
    fn shm_6_4_a_new_owner_session_discards_the_frames_of_an_ended_one() {
        let region = Region::new(4096);
        let reader = consumer_after_owner(&region);
        let crashed = owner(&region);
        crashed.send(&Locator::shm(CONSUMER), b"crashed 1").unwrap();
        crashed.send(&Locator::shm(CONSUMER), b"crashed 2").unwrap();
        core::mem::forget(crashed);
        let next = owner(&region);
        next.send(&Locator::shm(CONSUMER), b"next 1").unwrap();
        assert_eq!(&reader.recv().unwrap().data[..], b"next 1");
        assert!(reader.try_recv().unwrap().is_none());

        next.send(&Locator::shm(CONSUMER), b"next 2").unwrap();
        next.send(&Locator::shm(CONSUMER), b"next 3").unwrap();
        drop(next);
        let third = owner(&region);
        third.send(&Locator::shm(CONSUMER), b"third").unwrap();
        for want in [&b"next 2"[..], b"next 3", b"third"] {
            assert_eq!(&reader.recv().unwrap().data[..], want);
        }
        assert!(reader.try_recv().unwrap().is_none());
        // The superseded owner's sends fail, its drop leaves the ring.
        let fourth = owner(&region);
        assert!(third.is_superseded());
        assert!(third.send(&Locator::shm(CONSUMER), b"late").is_err());
        drop(third);
        fourth.send(&Locator::shm(CONSUMER), b"fourth").unwrap();
        assert_eq!(&reader.recv().unwrap().data[..], b"fourth");
    }

    /// The consumer of [`shm_6_4_a_new_owner_session_discards_the_frames_of_an_ended_one`]:
    /// before any owner the region holds no ring, and the consumer is
    /// refused until one made it.
    fn consumer_after_owner(region: &Region) -> RegionShmTransport<SpinWait> {
        // SAFETY: as in `owner`.
        let early = unsafe {
            RegionShmTransport::<SpinWait>::open_consumer(
                region.base(),
                region.len(),
                CONSUMER,
                OWNER,
                config(),
                WAIT,
            )
        };
        assert_eq!(
            early.unwrap_err(),
            RegionShmError::Ring(RingError::NotReady)
        );
        drop(owner(region));
        consumer(region)
    }

    /// §6.4: a region with other contents (a ring of another capacity,
    /// bytes that are no ring) is made anew by the owner; an owner refuses a
    /// `max_datagram` the region cannot hold twice, and a consumer lowers
    /// its own to what the ring holds.
    #[test]
    fn shm_6_4_the_owner_makes_the_ring_anew_where_the_region_holds_none() {
        let region = Region::new(4096);
        // SAFETY: the region is the test's and 8-aligned.
        unsafe {
            Ring::create(region.base(), region.len(), 1024).unwrap();
        }
        let sender = owner(&region);
        assert_eq!(
            sender.ring.capacity(),
            4096,
            "made anew at the region's size"
        );
        // SAFETY: as above.
        unsafe { core::ptr::write_bytes(region.base(), 0xA5, 64) };
        drop(sender);
        let sender = owner(&region);
        let receiver = consumer(&region);
        sender.send(&Locator::shm(CONSUMER), b"made anew").unwrap();
        assert_eq!(&receiver.recv().unwrap().data[..], b"made anew");

        let big = RegionConfig {
            max_datagram: 4096,
            recv_timeout: None,
        };
        // SAFETY: as in `owner`.
        let refused = unsafe {
            RegionShmTransport::open_owner(region.base(), region.len(), OWNER, CONSUMER, big, WAIT)
        };
        assert!(matches!(refused, Err(RegionShmError::InvalidConfig { .. })));
        // SAFETY: as in `owner`.
        let lowered = unsafe {
            RegionShmTransport::open_consumer(
                region.base(),
                region.len(),
                CONSUMER,
                OWNER,
                big,
                WAIT,
            )
        }
        .unwrap();
        assert_eq!(lowered.max_message_size(), (4096 - 16) / 2);
    }

    /// [`WAIT`], counting its parks.
    #[derive(Default)]
    struct Counting(AtomicU32);

    impl Counting {
        fn parks(&self) -> u32 {
            self.0.load(Ordering::Acquire)
        }
    }

    impl RingWait for Counting {
        fn park(&self, on: &Parked<'_>, timeout: Option<Duration>) -> bool {
            self.0.fetch_add(1, Ordering::AcqRel);
            WAIT.park(on, timeout)
        }
        fn wake(&self, word: &AtomicU32) {
            WAIT.wake(word);
        }
    }

    /// §6.4 with §5.3: the owner's drop ends a `recv` parked in
    /// [`SpinWait`] at once, with the targeted error, without a timeout and
    /// with a long one. The drop sets the shutdown flag and leaves `head` as
    /// it is.
    #[test]
    fn shm_6_4_the_owners_drop_ends_a_parked_recv() {
        for recv_timeout in [None, Some(Duration::from_secs(30))] {
            let region = Region::new(4096);
            let sender = owner(&region);
            let parked = RegionConfig {
                max_datagram: 1024,
                recv_timeout,
            };
            let receiver = Arc::new(
                // SAFETY: `region` is `region.len()` bytes of 8-aligned
                // words, and `sender` the one owner of its ring. This
                // consumer is the only one: the region is new in each turn.
                // The words stay allocated while the consumer lives: this
                // thread joins the reader before it drops `receiver`, and
                // `receiver` before `region`; the reader holds a clone of
                // `region` and drops its clone of `receiver` first.
                unsafe {
                    RegionShmTransport::open_consumer(
                        region.base(),
                        region.len(),
                        CONSUMER,
                        OWNER,
                        parked,
                        Counting::default(),
                    )
                }
                .unwrap(),
            );
            let (done, ended) = std::sync::mpsc::channel();
            let reader = {
                let receiver = Arc::clone(&receiver);
                let region = Arc::clone(&region);
                std::thread::spawn(move || {
                    let _keep = region;
                    // Dropped before `_keep`, on unwinding too.
                    let receiver = receiver;
                    let _ = done.send(receiver.recv());
                })
            };
            while receiver.wait.parks() == 0 {
                std::thread::yield_now();
            }
            std::thread::sleep(Duration::from_millis(50));
            let dropped = std::time::Instant::now();
            drop(sender);
            let got = ended
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| {
                    panic!("recv_timeout {recv_timeout:?}: still parked 10 s after the drop")
                });
            assert_eq!(
                got.unwrap_err(),
                RecvError::Io {
                    message: "shm owner terminated"
                },
                "recv_timeout {recv_timeout:?}"
            );
            assert!(
                dropped.elapsed() < Duration::from_secs(5),
                "recv_timeout {recv_timeout:?}: {:?}",
                dropped.elapsed()
            );
            reader.join().unwrap();
        }
    }

    /// The consumer waits with the integrator's wait up to its timeout, and
    /// a frame published meanwhile ends the wait.
    #[test]
    fn shm_6_4_recv_waits_with_the_integrators_wait_up_to_its_timeout() {
        let region = Region::new(4096);
        let sender = owner(&region);
        let short = RegionConfig {
            max_datagram: 1024,
            recv_timeout: Some(Duration::from_millis(1)),
        };
        // SAFETY: as in `owner`.
        let receiver = unsafe {
            RegionShmTransport::open_consumer(
                region.base(),
                region.len(),
                CONSUMER,
                OWNER,
                short,
                Counting::default(),
            )
        }
        .unwrap();
        assert_eq!(receiver.recv().unwrap_err(), RecvError::Timeout);
        assert_eq!(receiver.wait.parks(), 1, "one park");
        sender.send(&Locator::shm(CONSUMER), b"now").unwrap();
        assert_eq!(&receiver.recv().unwrap().data[..], b"now");
    }

    /// A wait that breaks the contract of [`RingWait::park`]: it returns
    /// `true` after 1 ms whether or not the park is over, as a wait on an
    /// event without a loop around it does when something else sends the
    /// event.
    #[derive(Default)]
    struct Early(AtomicU32);

    impl RingWait for Early {
        fn park(&self, _on: &Parked<'_>, _timeout: Option<Duration>) -> bool {
            self.0.fetch_add(1, Ordering::AcqRel);
            std::thread::sleep(Duration::from_millis(1));
            true
        }
        fn wake(&self, _word: &AtomicU32) {}
    }

    /// A park that returns before its wait is over and before its timeout
    /// ran out does not start the timeout anew on every turn: a `recv` with
    /// a timeout (50 ms) returns `Timeout` after that park, since the region
    /// has no clock to wait out the rest; a `recv` without one parks again
    /// until a frame comes. Before, the `recv` with 50 ms had not returned
    /// after 3 s.
    #[test]
    fn shm_6_4_a_park_that_returns_early_does_not_restart_the_timeout_without_end() {
        let region = Region::new(4096);
        let sender = owner(&region);
        for recv_timeout in [Some(Duration::from_millis(50)), None] {
            let config = RegionConfig {
                max_datagram: 1024,
                recv_timeout,
            };
            let receiver = Arc::new(
                // SAFETY: `region` is `region.len()` bytes of 8-aligned
                // words, and `sender` the one owner of its ring. This
                // consumer is the only one: the consumer of the first turn
                // is dropped at the end of that turn. The words stay
                // allocated while the consumer lives: this thread joins the
                // reader before it drops `receiver`, and `region` lives to
                // the end of the test; the reader holds a clone of `region`
                // and drops its clone of `receiver` first.
                unsafe {
                    RegionShmTransport::open_consumer(
                        region.base(),
                        region.len(),
                        CONSUMER,
                        OWNER,
                        config,
                        Early::default(),
                    )
                }
                .unwrap(),
            );
            let (done, ended) = std::sync::mpsc::channel();
            let started = std::time::Instant::now();
            let reader = {
                let receiver = Arc::clone(&receiver);
                let region = Arc::clone(&region);
                std::thread::spawn(move || {
                    let _keep = region;
                    // Dropped before `_keep`, on unwinding too.
                    let receiver = receiver;
                    let _ = done.send(receiver.recv());
                })
            };
            if recv_timeout.is_none() {
                // Parks again and again, and the frame ends it.
                while receiver.wait.0.load(Ordering::Acquire) < 5 {
                    std::thread::yield_now();
                }
                sender.send(&Locator::shm(CONSUMER), b"late").unwrap();
            }
            let got = ended
                .recv_timeout(Duration::from_secs(3))
                .unwrap_or_else(|_| {
                    panic!("recv_timeout {recv_timeout:?}: recv had not returned after 3 s")
                });
            match recv_timeout {
                Some(_) => {
                    assert_eq!(got.unwrap_err(), RecvError::Timeout);
                    assert_eq!(receiver.wait.0.load(Ordering::Acquire), 1, "one park");
                    assert!(started.elapsed() < Duration::from_secs(2));
                }
                None => assert_eq!(&got.unwrap().data[..], b"late"),
            }
            reader.join().unwrap();
        }
    }
}
