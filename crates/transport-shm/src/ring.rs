// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The ring of §2 to §4 over a region of memory, in `core` alone: what
//! [`crate::posix::PosixShmTransport`] maps from a named segment, and what
//! [`crate::region::RegionShmTransport`] takes from the integrator where
//! there is no operating system (§6.4). One implementation for both: the
//! header, the frame format, the padding and the checks of §3.3.
//!
//! # Positions
//!
//! `head` (written by the producer alone) and `tail` (written by the
//! consumer alone) are 64-bit positions in the header. Each side reads the
//! other's word only for its low 32 bits and takes the distance to its own
//! position from them: `head - tail` is at most the capacity, and the
//! capacity is at most [`MAX_CAPACITY`] (2³¹), so the low 32 bits of both
//! give it exactly. The other side's high half is never needed. That is
//! what lets a core without 64-bit atomics (Cortex-M, `thumbv7em`) take
//! part: it writes its position as two 32-bit words, the low one last with
//! `Release`, and a 64-bit peer that loads the whole word sees, in the low
//! half, what the 32-bit side published. A target with 64-bit atomics
//! keeps storing and loading the whole word.
//!
//! A consumer that drops a corrupt frame (§3.3) resumes at the producer's
//! whole `head`; with a 32-bit producer that read can catch the two halves
//! between their stores once every 4 GiB, and the next read then counts one
//! more drop.

use core::ptr;
#[cfg(target_has_atomic = "64")]
use core::sync::atomic::AtomicU64;
use core::sync::atomic::{AtomicU32, Ordering};

/// Magic prefix of a ring, `ZSHM` big-endian at offset 0.
pub const SHM_MAGIC: u32 = u32::from_be_bytes(*b"ZSHM");

/// Layout version at offset 4. Bump on a change of the layout or of the
/// protocol around it; attachers reject unknown versions. 2: every user of
/// a named segment holds a shared lock on it (§5.5); 3: the pair lock and,
/// where a segment is no file, the hold lie in the pair directory under
/// the segment's name (§5.4). The session word at offset 48 (§6.4) is
/// written only by an owner over a caller's region; a version-3 reader of
/// a named segment ignores it, and it is 0 in a named segment.
pub const SHM_VERSION: u32 = 3;

/// Bytes of the header in front of the data region.
pub const HEADER_BYTES: usize = 64;

/// The largest data region: positions are compared by their low 32 bits.
pub const MAX_CAPACITY: usize = 1 << 31;

/// The smallest data region an attacher accepts.
pub const MIN_CAPACITY: usize = 32;

/// Frame length that marks a padding frame: the rest of the ring up to its
/// end is skipped, and the writer goes on at the ring's start.
pub const PADDING_FRAME_LEN: u32 = 0xFFFF_FFFE;

const MAGIC: usize = 0;
const VERSION: usize = 4;
const CAPACITY: usize = 8;
const HEAD: usize = 16;
const TAIL: usize = 24;
const SHUTDOWN: usize = 32;
const CONSUMER_ATTACHED: usize = 36;
const OWNER_EPOCH: usize = 40;
const SESSION_HEAD: usize = 48;

/// Role in the SHM pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShmRole {
    /// This side writes the ring (sole writer); it makes the ring where
    /// there is none.
    Owner,
    /// This side reads the ring (sole reader).
    Consumer,
}

/// Why a region holds no ring this side can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RingError {
    /// The region is not aligned to 8 bytes.
    Misaligned,
    /// The region is shorter than the header and [`MIN_CAPACITY`] bytes, or
    /// than the capacity its header names.
    TooSmall,
    /// No ring is there yet: the magic is missing.
    NotReady,
    /// The magic is there, the layout version or the capacity is not one
    /// this side reads.
    InvalidHeader,
}

impl core::fmt::Display for RingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Misaligned => "shm region is not aligned to 8 bytes",
            Self::TooSmall => "shm region is too small for the ring",
            Self::NotReady => "shm region holds no ring yet",
            Self::InvalidHeader => "shm ring has an unknown version or capacity",
        })
    }
}

/// What [`Ring::try_push`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Push {
    /// The frame is in the ring and published.
    Pushed,
    /// Not enough room: the consumer has to read first.
    Full,
    /// Not enough room for the padding to the ring's end and the frame.
    FullAtWrap,
    /// A position computed lies outside the data region: not reached while
    /// `head` holds what this producer stored.
    OutsideTheRing,
    /// The frame is longer than half the ring, which a consumer drops as
    /// corrupt (§3.3).
    TooLarge,
}

/// The next frame, as [`Ring::next`] found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Nothing published.
    Empty,
    /// A frame of `len` bytes at `offset` of the data region; reading it
    /// ends at position `end`, which [`Ring::release`] takes.
    Frame {
        /// Offset of the frame's bytes in the data region.
        offset: usize,
        /// Length of the frame.
        len: usize,
        /// The position behind the frame.
        end: u64,
    },
    /// The frame was corrupt (§3.3): everything published was dropped, and
    /// `tail` stands at `head`.
    Corrupt,
}

/// A view of a ring in memory the two sides share. It holds a pointer, not
/// the memory: whoever makes it keeps the memory mapped and in place for as
/// long as the view is used.
///
/// # Safety of the accesses
///
/// The producer writes the bytes of a frame before it stores `head` with
/// `Release`; the consumer loads `head` with `Acquire` before it reads
/// them, and stores `tail` with `Release` after it has read them, which the
/// producer loads with `Acquire` before it reuses the room. The plain
/// copies in and out of the data region are therefore ordered by these
/// edges. Every offset and length read from the ring is checked against the
/// data region before a byte is copied: the other side can write anything
/// there.
#[derive(Debug)]
pub struct Ring {
    base: *mut u8,
    capacity: usize,
}

// SAFETY: the pointer is only dereferenced through the atomics and the
// bounded copies above; the memory is shared by design.
unsafe impl Send for Ring {}
// SAFETY: as above; the single-producer, single-consumer discipline is the
// caller's (one sending side, one receiving side).
unsafe impl Sync for Ring {}

impl Ring {
    /// Writes a new ring into the region: header and positions, the magic
    /// last, so that an attacher that sees the magic sees the rest.
    /// `capacity` is the data region's size.
    ///
    /// # Errors
    /// [`RingError::Misaligned`], [`RingError::TooSmall`].
    ///
    /// # Safety
    /// `base` points to `len` bytes that stay mapped while the ring is used,
    /// and nobody reads the ring while it is made.
    pub unsafe fn create(base: *mut u8, len: usize, capacity: usize) -> Result<Self, RingError> {
        check_region(base, len, capacity)?;
        let ring = Self { base, capacity };
        // SAFETY: the header lies in the region (checked above).
        unsafe {
            ring.word(MAGIC).store(0, Ordering::Relaxed);
            for offset in (VERSION..HEADER_BYTES).step_by(4) {
                ring.word(offset).store(0, Ordering::Relaxed);
            }
            ring.word(VERSION)
                .store(SHM_VERSION.to_le(), Ordering::Relaxed);
            ptr::write_unaligned(base.add(CAPACITY).cast::<u64>(), (capacity as u64).to_le());
        }
        ring.word(MAGIC).store(SHM_MAGIC.to_be(), Ordering::Release);
        Ok(ring)
    }

    /// Writes the header of a new ring into a region that is zero, as a
    /// named segment is when it is made: magic, version and capacity. The
    /// positions and flags stay zero.
    ///
    /// # Errors
    /// [`RingError::Misaligned`], [`RingError::TooSmall`].
    ///
    /// # Safety
    /// As [`Self::create`]; the region is zero.
    pub unsafe fn create_in_zeroed(
        base: *mut u8,
        len: usize,
        capacity: usize,
    ) -> Result<Self, RingError> {
        check_region(base, len, capacity)?;
        // SAFETY: the header lies in the region (checked above).
        unsafe {
            ptr::write_unaligned(base.add(VERSION).cast::<u32>(), SHM_VERSION.to_le());
            ptr::write_unaligned(base.add(CAPACITY).cast::<u64>(), (capacity as u64).to_le());
        }
        let ring = Self { base, capacity };
        ring.word(MAGIC).store(SHM_MAGIC.to_be(), Ordering::Release);
        Ok(ring)
    }

    /// Attaches to the ring in the region: magic, version and a capacity
    /// between [`MIN_CAPACITY`] and [`MAX_CAPACITY`] that fits the region.
    ///
    /// # Errors
    /// [`RingError`].
    ///
    /// # Safety
    /// `base` points to `len` bytes that stay mapped while the ring is used.
    pub unsafe fn attach(base: *mut u8, len: usize) -> Result<Self, RingError> {
        check_region(base, len, MIN_CAPACITY)?;
        let probe = Self {
            base,
            capacity: MIN_CAPACITY,
        };
        if u32::from_be(probe.word(MAGIC).load(Ordering::Acquire)) != SHM_MAGIC {
            return Err(RingError::NotReady);
        }
        if u32::from_le(probe.word(VERSION).load(Ordering::Relaxed)) != SHM_VERSION {
            return Err(RingError::InvalidHeader);
        }
        let capacity = probe.capacity_in_header();
        if !(MIN_CAPACITY..=MAX_CAPACITY).contains(&capacity) {
            return Err(RingError::InvalidHeader);
        }
        if len < HEADER_BYTES.saturating_add(capacity) {
            return Err(RingError::TooSmall);
        }
        Ok(Self { base, capacity })
    }

    /// The capacity the header names now.
    fn capacity_in_header(&self) -> usize {
        // SAFETY: offset 8 lies in the header.
        let raw = unsafe { ptr::read_unaligned(self.base.add(CAPACITY).cast::<u64>()) };
        usize::try_from(u64::from_le(raw)).unwrap_or(usize::MAX)
    }

    /// Start of the region (the header).
    #[must_use]
    pub fn base(&self) -> *mut u8 {
        self.base
    }

    /// Size of the data region.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The 32-bit word at `offset` of the header.
    fn word(&self, offset: usize) -> &AtomicU32 {
        // SAFETY: every offset passed lies in the 64-byte header, 4-aligned
        // in a region aligned to 8 (checked at construction).
        unsafe { &*(self.base.add(offset).cast::<AtomicU32>()) }
    }

    /// `shutdown` (§5.3): the owner sets it to 1 in its drop.
    #[must_use]
    pub fn shutdown(&self) -> &AtomicU32 {
        self.word(SHUTDOWN)
    }

    /// `consumer_attached`: consumers that have the ring open.
    #[must_use]
    pub fn consumer_attached(&self) -> &AtomicU32 {
        self.word(CONSUMER_ATTACHED)
    }

    /// `owner_epoch`: every owner that opens the ring counts it up.
    #[must_use]
    pub fn owner_epoch(&self) -> &AtomicU32 {
        self.word(OWNER_EPOCH)
    }

    /// The low 32 bits of `head`, the word a consumer parks on: it changes
    /// with every frame. Byte 16 on a little-endian target, byte 20 on a
    /// big-endian one.
    #[must_use]
    pub fn head_word(&self) -> &AtomicU32 {
        self.word(HEAD + low_half())
    }

    /// `head` as its writer left it: exact on the producer's side; on the
    /// consumer's side see the module docs.
    #[must_use]
    pub fn head_position(&self) -> u64 {
        load_position(self.base, HEAD)
    }

    /// `tail` as its writer left it: exact on the consumer's side.
    #[must_use]
    pub fn tail_position(&self) -> u64 {
        load_position(self.base, TAIL)
    }

    /// Stores `head` (the producer, or a test).
    pub fn store_head(&self, head: u64) {
        store_position(self.base, HEAD, head);
    }

    /// Stores `tail` (the consumer, or a test).
    pub fn store_tail(&self, tail: u64) {
        store_position(self.base, TAIL, tail);
    }

    /// The position at which the current owner's session began (§6.4):
    /// what was published before it belongs to an owner that ended.
    #[must_use]
    pub fn session_head(&self) -> u64 {
        load_position(self.base, SESSION_HEAD)
    }

    /// Stores the session's first position; the owner does this before it
    /// counts `owner_epoch` up.
    pub fn store_session_head(&self, at: u64) {
        store_position(self.base, SESSION_HEAD, at);
    }

    /// Bytes published and not yet read, from the low halves of both
    /// positions: exact from either side.
    #[must_use]
    pub fn occupied(&self) -> u64 {
        let h = low(self.base, HEAD, Ordering::Acquire);
        let t = low(self.base, TAIL, Ordering::Acquire);
        u64::from(h.wrapping_sub(t))
    }

    /// Whether `len` bytes from `offset` lie inside the data region.
    fn in_ring(&self, offset: usize, len: usize) -> bool {
        offset
            .checked_add(len)
            .is_some_and(|end| end <= self.capacity)
    }

    fn data(&self) -> *mut u8 {
        // SAFETY: the data region starts behind the header.
        unsafe { self.base.add(HEADER_BYTES) }
    }

    /// Copies `out.len()` bytes from `offset` of the data region; `false`,
    /// with nothing copied, when they would leave it.
    #[must_use]
    pub fn read(&self, offset: usize, out: &mut [u8]) -> bool {
        if !self.in_ring(offset, out.len()) {
            return false;
        }
        // SAFETY: inside the data region (checked above).
        unsafe { ptr::copy_nonoverlapping(self.data().add(offset), out.as_mut_ptr(), out.len()) };
        true
    }

    /// Copies `src` to `offset` of the data region; `false`, with nothing
    /// written, when it would leave it.
    #[must_use]
    pub fn write(&self, offset: usize, src: &[u8]) -> bool {
        if !self.in_ring(offset, src.len()) {
            return false;
        }
        // SAFETY: inside the data region (checked above).
        unsafe { ptr::copy_nonoverlapping(src.as_ptr(), self.data().add(offset), src.len()) };
        true
    }

    /// Writes a little-endian `u32` at `offset` of the data region.
    #[must_use]
    pub fn write_u32(&self, offset: usize, v: u32) -> bool {
        self.write(offset, &v.to_le_bytes())
    }

    /// Reads a little-endian `u32` at `offset` of the data region.
    #[must_use]
    pub fn read_u32(&self, offset: usize) -> Option<u32> {
        let mut b = [0u8; 4];
        self.read(offset, &mut b).then(|| u32::from_le_bytes(b))
    }

    /// One attempt to put `prefix` and `data` into the ring as one frame
    /// (the producer). A frame is never split at the ring's end: when the
    /// rest up to the end is too short, it is padded (a marker where 4 bytes
    /// or more are left, nothing where 1 to 3 are) and the frame goes to the
    /// start, if the room for both is there.
    #[must_use]
    pub fn try_push(&self, prefix: &[u8], data: &[u8]) -> Push {
        let len = prefix.len().saturating_add(data.len());
        if len > self.capacity / 2 {
            return Push::TooLarge;
        }
        let len_u32 = len as u32;
        let needed = len as u64 + 4;
        let cap = self.capacity as u64;
        loop {
            let h = self.head_position();
            let t_low = low(self.base, TAIL, Ordering::Acquire);
            // A reader never takes more than was published, so `used` is at
            // most `cap`; a broken or hostile consumer can store any `tail`,
            // and that reads as a full ring, not as room.
            let used = u64::from((h as u32).wrapping_sub(t_low));
            let free = cap.saturating_sub(used);
            let h_mod = (h % cap) as usize;
            let tail_space = self.capacity - h_mod;
            if needed > free {
                return Push::Full;
            }
            if needed > tail_space as u64 {
                // After the padding `used` rises by `tail_space`; the frame
                // needs its room on top of that.
                if needed + tail_space as u64 > free {
                    return Push::FullAtWrap;
                }
                if tail_space >= 4 && !self.write_u32(h_mod, PADDING_FRAME_LEN) {
                    return Push::OutsideTheRing;
                }
                store_position(self.base, HEAD, h + tail_space as u64);
                continue;
            }
            if !(self.write_u32(h_mod, len_u32)
                && self.write(h_mod + 4, prefix)
                && self.write(h_mod + 4 + prefix.len(), data))
            {
                return Push::OutsideTheRing;
            }
            store_position(self.base, HEAD, h + needed);
            return Push::Pushed;
        }
    }

    /// The next frame (the consumer). Padding is skipped and counted in
    /// `paddings`. Every value read from the ring (`head`, a length, a
    /// padding marker) is checked against what a producer of this protocol
    /// can publish (§3.3); a corrupt one drops everything published, and
    /// `tail` goes to `head`.
    pub fn next(&self, paddings: &mut u32) -> Next {
        let cap = self.capacity as u64;
        loop {
            let t = self.tail_position();
            // What the producer has published past `tail`. A producer of this
            // protocol never gets more than the ring ahead of its reader.
            let published =
                u64::from(low(self.base, HEAD, Ordering::Acquire).wrapping_sub(t as u32));
            if published == 0 {
                return Next::Empty;
            }
            if published > cap {
                return self.drop_published();
            }
            let t_mod = (t % cap) as usize;
            let tail_space = self.capacity - t_mod;
            // A remainder of 1 to 3 bytes at the ring's end holds no length.
            if tail_space < 4 {
                if published < tail_space as u64 {
                    return self.drop_published();
                }
                self.store_tail(t + tail_space as u64);
                continue;
            }
            // Every frame and every padding marker is at least 4 bytes.
            if published < 4 {
                return self.drop_published();
            }
            let Some(len) = self.read_u32(t_mod) else {
                return self.drop_published();
            };
            if len == PADDING_FRAME_LEN {
                if published < tail_space as u64 {
                    return self.drop_published();
                }
                *paddings = paddings.saturating_add(1);
                self.store_tail(t + tail_space as u64);
                continue;
            }
            // The bound is the ring's: frames are whole up to half the ring,
            // never split at its end, and published whole.
            let frame = 4 + u64::from(len);
            if len as usize > self.capacity / 2 || frame > tail_space as u64 || frame > published {
                return self.drop_published();
            }
            return Next::Frame {
                offset: t_mod + 4,
                len: len as usize,
                end: t + frame,
            };
        }
    }

    /// Releases the frame [`Self::next`] returned: `tail` goes behind it.
    pub fn release(&self, end: u64) {
        self.store_tail(end);
    }

    /// Discards everything published: behind a corrupt length the next frame
    /// boundary is unknown. `tail` takes `head`'s value, so the ring reads as
    /// empty and the producer's next frame starts where the consumer looks.
    pub fn drop_published(&self) -> Next {
        self.store_tail(self.head_position());
        Next::Corrupt
    }

    /// Skips to the start of the current owner's session (§6.4) when `tail`
    /// is behind it and the session start is no more than a ring ahead:
    /// what lies before it is an ended owner's. Returns whether it skipped.
    pub fn skip_to_session(&self) -> bool {
        let t = self.tail_position();
        let behind = (self.session_head() as u32).wrapping_sub(t as u32);
        if behind == 0 || u64::from(behind) > self.capacity as u64 {
            return false;
        }
        self.store_tail(t + u64::from(behind));
        true
    }
}

/// Checks a region for a ring of `capacity` bytes.
fn check_region(base: *mut u8, len: usize, capacity: usize) -> Result<(), RingError> {
    if base.align_offset(8) != 0 {
        return Err(RingError::Misaligned);
    }
    if !(MIN_CAPACITY..=MAX_CAPACITY).contains(&capacity)
        || len < HEADER_BYTES.saturating_add(capacity)
    {
        return Err(RingError::TooSmall);
    }
    Ok(())
}

/// Byte offset of the low half of a 64-bit word.
const fn low_half() -> usize {
    if cfg!(target_endian = "big") { 4 } else { 0 }
}

/// The low 32 bits of the position at `offset`.
fn low(base: *mut u8, offset: usize, order: Ordering) -> u32 {
    #[cfg(target_has_atomic = "64")]
    {
        // SAFETY: the position lies in the header, 8-aligned.
        let word = unsafe { &*(base.add(offset).cast::<AtomicU64>()) };
        word.load(order) as u32
    }
    #[cfg(not(target_has_atomic = "64"))]
    {
        split::low(base, offset, order)
    }
}

fn load_position(base: *mut u8, offset: usize) -> u64 {
    #[cfg(target_has_atomic = "64")]
    {
        // SAFETY: the position lies in the header, 8-aligned.
        let word = unsafe { &*(base.add(offset).cast::<AtomicU64>()) };
        word.load(Ordering::Acquire)
    }
    #[cfg(not(target_has_atomic = "64"))]
    {
        split::load(base, offset)
    }
}

fn store_position(base: *mut u8, offset: usize, v: u64) {
    #[cfg(target_has_atomic = "64")]
    {
        // SAFETY: the position lies in the header, 8-aligned.
        let word = unsafe { &*(base.add(offset).cast::<AtomicU64>()) };
        word.store(v, Ordering::Release);
    }
    #[cfg(not(target_has_atomic = "64"))]
    {
        split::store(base, offset, v);
    }
}

/// A 64-bit position as two 32-bit words, for a target without 64-bit
/// atomics; the bytes are those of the whole word on the same target.
#[cfg_attr(target_has_atomic = "64", allow(dead_code))]
pub(crate) mod split {
    use super::low_half;
    use core::sync::atomic::{AtomicU32, Ordering};

    /// The half of the position at `offset`.
    ///
    /// # Safety
    /// `offset` names a position in the header of a region `base` that is
    /// mapped and 8-aligned for as long as the returned reference is used.
    unsafe fn half<'a>(base: *mut u8, offset: usize, high: bool) -> &'a AtomicU32 {
        let at = if high { 4 - low_half() } else { low_half() };
        // SAFETY: as the caller guarantees.
        unsafe { &*(base.add(offset + at).cast::<AtomicU32>()) }
    }

    /// The low half, with `order`.
    pub(crate) fn low(base: *mut u8, offset: usize, order: Ordering) -> u32 {
        // SAFETY: the ring's positions lie in its mapped header.
        unsafe { half(base, offset, false) }.load(order)
    }

    /// Both halves; exact for the position's own writer.
    pub(crate) fn load(base: *mut u8, offset: usize) -> u64 {
        // SAFETY: as above.
        let (high, low) = unsafe { (half(base, offset, true), half(base, offset, false)) };
        let high = high.load(Ordering::Acquire);
        let low = low.load(Ordering::Acquire);
        (u64::from(high) << 32) | u64::from(low)
    }

    /// The high half first, then the low half with `Release`: the low half
    /// is what the other side reads, and its store publishes.
    pub(crate) fn store(base: *mut u8, offset: usize, v: u64) {
        // SAFETY: as above.
        let (high, low) = unsafe { (half(base, offset, true), half(base, offset, false)) };
        high.store((v >> 32) as u32, Ordering::Relaxed);
        low.store(v as u32, Ordering::Release);
    }
}

#[cfg(all(test, feature = "alloc"))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn region(capacity: usize) -> Vec<u64> {
        vec![0u64; (HEADER_BYTES + capacity) / 8]
    }

    /// §6.4: positions go past 2³² — where a 32-bit core's low word wraps —
    /// and frames cross that point whole and in order; a position stored as
    /// two 32-bit words, as a core without 64-bit atomics stores it, has the
    /// bytes of the whole word, and the whole word read in halves is the
    /// same value.
    #[test]
    fn shm_6_4_positions_cross_the_low_word_wrap_and_split_words_match_whole_ones() {
        let mut memory = region(4096);
        let base = memory.as_mut_ptr().cast::<u8>();
        let len = memory.len() * 8;
        // SAFETY: `memory` is `len` bytes, 8-aligned, and outlives `ring`.
        let ring = unsafe { Ring::create(base, len, 4096) }.unwrap();
        let start = (1u64 << 32) - 1500;
        ring.store_head(start);
        ring.store_tail(start);
        let mut sent = 0u32;
        let mut received = 0u32;
        let mut paddings = 0;
        while received < 400 {
            while sent < received + 20 {
                let frame = vec![sent as u8; 1 + (sent as usize * 7) % 300];
                if ring.try_push(&sent.to_le_bytes(), &frame) != Push::Pushed {
                    break;
                }
                sent += 1;
            }
            let Next::Frame { offset, len, end } = ring.next(&mut paddings) else {
                panic!("frame {received} missing");
            };
            let mut out = vec![0u8; len];
            assert!(ring.read(offset, &mut out));
            ring.release(end);
            assert_eq!(&out[..4], &received.to_le_bytes(), "in order");
            assert_eq!(out.len(), 4 + 1 + (received as usize * 7) % 300);
            assert!(out[4..].iter().all(|&b| b == received as u8), "whole");
            received += 1;
        }
        assert!(
            ring.tail_position() > 1u64 << 32,
            "past the low word's wrap"
        );
        assert!(paddings > 0, "and across the ring's end");

        for v in [0u64, 1, 0xFFFF_FFFF, 1 << 32, (1 << 32) + 7, u64::MAX - 3] {
            split::store(base, TAIL, v);
            assert_eq!(ring.tail_position(), v, "split store, whole load");
            assert_eq!(split::load(base, TAIL), v);
            assert_eq!(split::low(base, TAIL, Ordering::Acquire), v as u32);
            ring.store_tail(v);
            assert_eq!(split::load(base, TAIL), v, "whole store, split load");
            // SAFETY: offset 24 lies in the header.
            let bytes = unsafe { core::ptr::read_unaligned(base.add(TAIL).cast::<[u8; 8]>()) };
            assert_eq!(bytes, v.to_ne_bytes());
        }
    }

    /// The header a ring is made with, and what an attacher refuses: no
    /// magic, another version, a capacity the region cannot hold, a region
    /// out of alignment or too small.
    #[test]
    fn shm_6_4_create_and_attach_check_the_header_and_the_region() {
        let mut memory = region(4096);
        let base = memory.as_mut_ptr().cast::<u8>();
        let len = memory.len() * 8;
        // SAFETY: `memory` is `len` bytes of 8-aligned words that live to
        // the end of the test and that only this thread touches, through
        // `base`. Every `create` and `attach` gets a range inside them (the
        // one at `base + 4` spans `len - 8` bytes), no ring is read while
        // one is made, and the write at `base + 4` is the `u32` of the
        // header's version field.
        unsafe {
            assert_eq!(Ring::attach(base, len).unwrap_err(), RingError::NotReady);
            assert_eq!(
                Ring::create(base.add(4), len - 8, 4096).unwrap_err(),
                RingError::Misaligned
            );
            assert_eq!(
                Ring::create(base, len, 8192).unwrap_err(),
                RingError::TooSmall
            );
            let made = Ring::create(base, len, 4096).unwrap();
            assert_eq!(made.capacity(), 4096);
            let attached = Ring::attach(base, len).unwrap();
            assert_eq!(attached.capacity(), 4096);
            assert_eq!(
                Ring::attach(base, len - 8).unwrap_err(),
                RingError::TooSmall
            );
            core::ptr::write_unaligned(base.add(4).cast::<u32>(), 2u32.to_le());
            assert_eq!(
                Ring::attach(base, len).unwrap_err(),
                RingError::InvalidHeader
            );
        }
    }
}
