// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTPS submessages — DDSI-RTPS 2.5 §8.3.8 (the HeaderExtension is §8.3.7,
//! see [`crate::header_extension`]).
//!
//! Implemented submessages:
//! - **DATA** (`DataSubmessage`) — §8.3.8.2.
//! - **HEARTBEAT** (`HeartbeatSubmessage`) — §8.3.8.6.
//! - **ACKNACK** (`AckNackSubmessage`) — §8.3.8.1.
//! - **GAP** (`GapSubmessage`) — §8.3.8.4.
//! - **DATA_FRAG** (`DataFragSubmessage`) — §8.3.8.3.
//! - **HEARTBEAT_FRAG** (`HeartbeatFragSubmessage`) — §8.3.8.7.
//! - **NACK_FRAG** (`NackFragSubmessage`) — §8.3.8.12.
//! - **INFO_SRC** (`InfoSourceSubmessage`) — §8.3.8.10.
//! - **INFO_TS** (`InfoTimestampSubmessage`) — §8.3.8.11.
//! - **INFO_REPLY** (`InfoReplySubmessage`) — §8.3.8.9.
//! - **INFO_REPLY_IP4** (`InfoReplyIp4Submessage`) — PSM §9.4.5.11 (IPv4-only
//!   predecessor of INFO_REPLY).
//!
//! ParameterList (inline QoS) lives in the separate module
//! [`crate::parameter_list`]; SecuredPayload wrapping is in the
//! `zerodds-security` crate (DDS-Security 1.2 §7.4).
//!
//! # Endianness
//!
//! Submessage bodies are written in the endianness of the submessage
//! header (E-flag). The `to_bytes_*`/`from_bytes_*` functions given here
//! take explicit endianness as a parameter — the caller must choose it
//! consistently with the header.

extern crate alloc;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::submessage_header::FLAG_E_LITTLE_ENDIAN;
use crate::wire_types::{EntityId, FragmentNumber, SequenceNumber};

/// Hard cap for `numBits` in `SequenceNumberSet` and
/// `FragmentNumberSet`. DDSI-RTPS gives no specific limit, but both
/// Cyclone DDS (`ddsi_radmin.c`) and Fast-DDS (`BitmapRange<..., 256>`)
/// cap at 256. We follow — prevents DoS via a `numBits=2^32-1` bitmap
/// alloc.
pub const RTPS_BITMAP_MAX_BITS: u32 = 256;

// ============================================================================
// SequenceNumberSet (§9.4.2.6)
// ============================================================================

/// Locators of a kind this stack does not know that an INFO_REPLY listed and
/// that were left out of the list. `portable_atomic`, as in
/// `history_cache.rs`: a 64-bit atomic the no_std targets without one
/// (thumbv7em) also have.
static SKIPPED_UNKNOWN_LOCATORS: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);

/// How many locators of an unknown kind INFO_REPLY submessages have listed
/// and were left out of the list (DDSI-RTPS 2.5 §9.3.2.4: vendor kinds).
#[must_use]
pub fn skipped_unknown_locators() -> u64 {
    SKIPPED_UNKNOWN_LOCATORS.load(portable_atomic::Ordering::Relaxed)
}

/// Whether an ACKNACK may carry an empty `readerSNState` with `bitmapBase` 0.
/// See [`set_accept_empty_acknack_base_zero`].
static ACCEPT_EMPTY_ACKNACK_BASE_ZERO: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Receive setting for the ACKNACK of OpenDDS 3.34.0 and RTI Connext 7.7.0,
/// which send `readerSNState` `0/0:` (bitmapBase 0, numBits 0) as the answer
/// of a reader that has received nothing; DDSI-RTPS 2.5 §9.4.2.6 requires
/// `bitmapBase >= 1`. Off by default, so such an ACKNACK is invalid and the
/// rest of its message with it. It is one setting for the process, over every
/// participant: the decoders have no participant.
pub fn set_accept_empty_acknack_base_zero(accept: bool) {
    ACCEPT_EMPTY_ACKNACK_BASE_ZERO.store(accept, core::sync::atomic::Ordering::Relaxed);
}

/// The value of [`set_accept_empty_acknack_base_zero`].
#[must_use]
pub fn accepts_empty_acknack_base_zero() -> bool {
    ACCEPT_EMPTY_ACKNACK_BASE_ZERO.load(core::sync::atomic::Ordering::Relaxed)
}

/// Bitset of sequence numbers from `bitmap_base`. Used in HEARTBEAT/
/// ACKNACK/GAP to signal sets of lost or known packets.
///
/// Wire layout (variable length):
///   bitmapBase: 8 byte (SequenceNumber, big or little per header)
///   numBits:    4 byte (u32)
///   bitmap:     ceil(numBits/32) * 4 byte (u32 words)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceNumberSet {
    /// First sequence number that the first bit is responsible for.
    pub bitmap_base: SequenceNumber,
    /// Number of valid bits.
    pub num_bits: u32,
    /// `ceil(num_bits/32)` u32 words.
    pub bitmap: Vec<u32>,
}

impl SequenceNumberSet {
    /// Computes the wire size in bytes based on `num_bits`.
    #[must_use]
    pub fn wire_size(num_bits: u32) -> usize {
        let words = (num_bits as usize).div_ceil(32);
        8 + 4 + words * 4
    }

    /// Builds a `SequenceNumberSet` from a sorted list of missing SNs.
    ///
    /// `base` is the smallest not-yet-acked SN (the AckNack base).
    /// `missing` must be sorted ascending and all SNs ≥ `base`. Bits are
    /// set in RTPS convention: bit 0 is the most-significant bit (MSB) of
    /// `bitmap[0]`.
    #[must_use]
    pub fn from_missing(base: SequenceNumber, missing: &[SequenceNumber]) -> Self {
        let Some(last) = missing.last().copied() else {
            return Self {
                bitmap_base: base,
                num_bits: 0,
                bitmap: Vec::new(),
            };
        };
        if last < base {
            return Self {
                bitmap_base: base,
                num_bits: 0,
                bitmap: Vec::new(),
            };
        }
        // numBits is at most 256 (RTPS_BITMAP_MAX_BITS), and the decoder below
        // refuses more, as Cyclone DDS and Fast DDS do. `missing` holds at
        // most 256 entries but they need not be consecutive: received
        // samples between them spread the set over a wider window, and a
        // set of 1168 bits was an ACKNACK every receiver discarded — the
        // writer never learned what was missing and the repair stopped
        // (measured: 96 393 samples never delivered). The window is cut at
        // 256; the rest follows in the next ACKNACK once the base has moved,
        // as FragmentNumberSet::from_missing does for fragments.
        let span = u64::try_from(last.0 - base.0).unwrap_or(u64::MAX);
        let num_bits = u32::try_from(span.saturating_add(1))
            .unwrap_or(u32::MAX)
            .min(RTPS_BITMAP_MAX_BITS);
        let num_words = (num_bits as usize).div_ceil(32);
        let mut bitmap = alloc::vec![0u32; num_words];
        for sn in missing {
            if *sn < base {
                continue;
            }
            let offset = (sn.0 - base.0) as usize;
            // Beyond the window: the next ACKNACK asks for it.
            if offset >= num_bits as usize {
                continue;
            }
            let word_idx = offset / 32;
            let bit = 31 - (offset % 32);
            if word_idx < bitmap.len() {
                bitmap[word_idx] |= 1u32 << bit;
            }
        }
        Self {
            bitmap_base: base,
            num_bits,
            bitmap,
        }
    }

    /// Iterates over all SNs whose bit is set.
    pub fn iter_set(&self) -> impl Iterator<Item = SequenceNumber> + '_ {
        (0..self.num_bits).filter_map(move |i| {
            let word_idx = (i / 32) as usize;
            let bit = 31 - (i as usize % 32);
            if word_idx < self.bitmap.len() && (self.bitmap[word_idx] >> bit) & 1 == 1 {
                Some(SequenceNumber(self.bitmap_base.0 + i64::from(i)))
            } else {
                None
            }
        })
    }

    /// Actual wire size of this set.
    #[must_use]
    pub fn encoded_size(&self) -> usize {
        Self::wire_size(self.num_bits)
    }

    /// Encodes the set into `out` with the given endianness.
    pub fn write_to(&self, out: &mut Vec<u8>, little_endian: bool) {
        if little_endian {
            out.extend_from_slice(&self.bitmap_base.to_bytes_le());
            out.extend_from_slice(&self.num_bits.to_le_bytes());
            for w in &self.bitmap {
                out.extend_from_slice(&w.to_le_bytes());
            }
        } else {
            out.extend_from_slice(&self.bitmap_base.to_bytes_be());
            out.extend_from_slice(&self.num_bits.to_be_bytes());
            for w in &self.bitmap {
                out.extend_from_slice(&w.to_be_bytes());
            }
        }
    }

    /// Decodes a set from `bytes` at `offset`. Returns (set, new position).
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_from(
        bytes: &[u8],
        offset: usize,
        little_endian: bool,
    ) -> Result<(Self, usize), WireError> {
        Self::read_from_with(bytes, offset, little_endian, false)
    }

    /// [`Self::read_from`], accepting with `accept_empty_base_zero` an empty
    /// set (`numBits` 0) whose `bitmapBase` is 0. DDSI-RTPS 2.5 §9.4.2.6
    /// makes that set invalid ("bitmapBase >= 1"); OpenDDS 3.34.0 and RTI
    /// Connext 7.7.0 send it as the ACKNACK of a reader that has received
    /// nothing (recorded sessions under `tools/wire-capture/rtps-sessions`).
    ///
    /// # Errors
    /// As [`Self::read_from`].
    pub fn read_from_with(
        bytes: &[u8],
        offset: usize,
        little_endian: bool,
        accept_empty_base_zero: bool,
    ) -> Result<(Self, usize), WireError> {
        let mut pos = offset;
        if bytes.len() < pos + 8 {
            return Err(WireError::UnexpectedEof {
                needed: 8,
                offset: pos,
            });
        }
        let mut sn_bytes = [0u8; 8];
        sn_bytes.copy_from_slice(&bytes[pos..pos + 8]);
        let bitmap_base = if little_endian {
            SequenceNumber::from_bytes_le(sn_bytes)
        } else {
            SequenceNumber::from_bytes_be(sn_bytes)
        };
        pos += 8;
        if bytes.len() < pos + 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: pos,
            });
        }
        let mut num_bytes = [0u8; 4];
        num_bytes.copy_from_slice(&bytes[pos..pos + 4]);
        let num_bits = if little_endian {
            u32::from_le_bytes(num_bytes)
        } else {
            u32::from_be_bytes(num_bytes)
        };
        pos += 4;
        if num_bits > RTPS_BITMAP_MAX_BITS {
            return Err(WireError::ValueOutOfRange {
                message: "SequenceNumberSet.numBits exceeds RTPS_BITMAP_MAX_BITS (256)",
            });
        }
        // §9.4.2.6: "A valid SequenceNumberSet must satisfy the following
        // conditions: bitmapBase >= 1" (§8.3.5.5: minimum >= 1).
        if bitmap_base.0 < 1 && !(accept_empty_base_zero && bitmap_base.0 == 0 && num_bits == 0) {
            return Err(WireError::ValueOutOfRange {
                message: "SequenceNumberSet.bitmapBase below 1 (DDSI-RTPS 2.5 §9.4.2.6)",
            });
        }
        let words = (num_bits as usize).div_ceil(32);
        let bitmap_bytes = words * 4;
        if bytes.len() < pos + bitmap_bytes {
            return Err(WireError::UnexpectedEof {
                needed: bitmap_bytes,
                offset: pos,
            });
        }
        let mut bitmap = Vec::with_capacity(words);
        for _ in 0..words {
            let mut w = [0u8; 4];
            w.copy_from_slice(&bytes[pos..pos + 4]);
            bitmap.push(if little_endian {
                u32::from_le_bytes(w)
            } else {
                u32::from_be_bytes(w)
            });
            pos += 4;
        }
        Ok((
            Self {
                bitmap_base,
                num_bits,
                bitmap,
            },
            pos,
        ))
    }
}

// ============================================================================
// FragmentNumberSet (§9.4.2.8)
// ============================================================================

/// Bitset of `FragmentNumber` values from `bitmap_base`. Analogous to
/// [`SequenceNumberSet`], but with `FragmentNumber` (u32) as the base
/// instead of `SequenceNumber`.
///
/// Wire layout:
///   bitmapBase: 4 byte (FragmentNumber, LE or BE per header)
///   numBits:    4 byte (u32)
///   bitmap:     ceil(numBits/32) * 4 byte
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentNumberSet {
    /// First fragment that the first bit is responsible for.
    pub bitmap_base: FragmentNumber,
    /// Number of valid bits.
    pub num_bits: u32,
    /// `ceil(num_bits/32)` u32 words.
    pub bitmap: Vec<u32>,
}

impl FragmentNumberSet {
    /// Wire size in bytes.
    #[must_use]
    pub fn wire_size(num_bits: u32) -> usize {
        let words = (num_bits as usize).div_ceil(32);
        4 + 4 + words * 4
    }

    /// Builds the set from a sorted list of missing fragments.
    /// `base` = smallest not-yet-acknowledged FragmentNumber.
    #[must_use]
    pub fn from_missing(base: FragmentNumber, missing: &[FragmentNumber]) -> Self {
        let Some(last) = missing.last().copied() else {
            return Self {
                bitmap_base: base,
                num_bits: 0,
                bitmap: Vec::new(),
            };
        };
        if last < base {
            return Self {
                bitmap_base: base,
                num_bits: 0,
                bitmap: Vec::new(),
            };
        }
        // DDSI-RTPS §8.3.5.4: numBits MUST be <= 256. A gap over more
        // than 256 fragments (large samples under packet loss) would
        // otherwise produce a num_bits > 256, which every spec-conformant
        // receiver discards as malformed → the NACK_FRAG is lost →
        // fragment stall. We cover only the first 256; the rest follows
        // in the next NACK_FRAG once bitmap_base has advanced.
        let num_bits = last.0.saturating_sub(base.0).saturating_add(1).min(256);
        let num_words = (num_bits as usize).div_ceil(32);
        let mut bitmap = alloc::vec![0u32; num_words];
        for fnum in missing {
            if *fnum < base {
                continue;
            }
            let offset = (fnum.0 - base.0) as usize;
            // Skip fragments beyond the 256-bit window (follow-up NACK_FRAG).
            if offset >= num_bits as usize {
                continue;
            }
            let word_idx = offset / 32;
            let bit = 31 - (offset % 32);
            if word_idx < bitmap.len() {
                bitmap[word_idx] |= 1u32 << bit;
            }
        }
        Self {
            bitmap_base: base,
            num_bits,
            bitmap,
        }
    }

    /// Iterates over all set FragmentNumbers.
    pub fn iter_set(&self) -> impl Iterator<Item = FragmentNumber> + '_ {
        (0..self.num_bits).filter_map(move |i| {
            let word_idx = (i / 32) as usize;
            let bit = 31 - (i as usize % 32);
            if word_idx < self.bitmap.len() && (self.bitmap[word_idx] >> bit) & 1 == 1 {
                Some(FragmentNumber(self.bitmap_base.0.wrapping_add(i)))
            } else {
                None
            }
        })
    }

    /// Actual wire size of this set.
    #[must_use]
    pub fn encoded_size(&self) -> usize {
        Self::wire_size(self.num_bits)
    }

    /// Encodes the set into `out`.
    pub fn write_to(&self, out: &mut Vec<u8>, little_endian: bool) {
        if little_endian {
            out.extend_from_slice(&self.bitmap_base.to_bytes_le());
            out.extend_from_slice(&self.num_bits.to_le_bytes());
            for w in &self.bitmap {
                out.extend_from_slice(&w.to_le_bytes());
            }
        } else {
            out.extend_from_slice(&self.bitmap_base.to_bytes_be());
            out.extend_from_slice(&self.num_bits.to_be_bytes());
            for w in &self.bitmap {
                out.extend_from_slice(&w.to_be_bytes());
            }
        }
    }

    /// Decodes a set from `bytes` at `offset`.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_from(
        bytes: &[u8],
        offset: usize,
        little_endian: bool,
    ) -> Result<(Self, usize), WireError> {
        let mut pos = offset;
        if bytes.len() < pos + 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: pos,
            });
        }
        let mut bb = [0u8; 4];
        bb.copy_from_slice(&bytes[pos..pos + 4]);
        let bitmap_base = if little_endian {
            FragmentNumber::from_bytes_le(bb)
        } else {
            FragmentNumber::from_bytes_be(bb)
        };
        pos += 4;
        if bytes.len() < pos + 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: pos,
            });
        }
        let mut nb = [0u8; 4];
        nb.copy_from_slice(&bytes[pos..pos + 4]);
        let num_bits = if little_endian {
            u32::from_le_bytes(nb)
        } else {
            u32::from_be_bytes(nb)
        };
        pos += 4;
        if num_bits > RTPS_BITMAP_MAX_BITS {
            return Err(WireError::ValueOutOfRange {
                message: "FragmentNumberSet.numBits exceeds RTPS_BITMAP_MAX_BITS (256)",
            });
        }
        // §9.4.2.8: the interpretation matches that of a SequenceNumberSet,
        // whose bitmapBase is at least 1 (§9.4.2.6); fragment numbers start
        // at 1 (§8.3.5.6).
        if bitmap_base.0 < 1 {
            return Err(WireError::ValueOutOfRange {
                message: "FragmentNumberSet.bitmapBase below 1 (DDSI-RTPS 2.5 §9.4.2.8)",
            });
        }
        let words = (num_bits as usize).div_ceil(32);
        let need = words * 4;
        if bytes.len() < pos + need {
            return Err(WireError::UnexpectedEof {
                needed: need,
                offset: pos,
            });
        }
        let mut bitmap = Vec::with_capacity(words);
        for _ in 0..words {
            let mut w = [0u8; 4];
            w.copy_from_slice(&bytes[pos..pos + 4]);
            bitmap.push(if little_endian {
                u32::from_le_bytes(w)
            } else {
                u32::from_be_bytes(w)
            });
            pos += 4;
        }
        Ok((
            Self {
                bitmap_base,
                num_bits,
                bitmap,
            },
            pos,
        ))
    }
}

// ============================================================================
// DATA Submessage (§8.3.8.2)
// ============================================================================

/// DATA-Submessage Flag: Q (Inline-QoS present).
pub const DATA_FLAG_INLINE_QOS: u8 = 0x02;
/// DATA-Submessage Flag: D (data payload present).
pub const DATA_FLAG_DATA: u8 = 0x04;
/// DATA-Submessage Flag: K (key payload present, Q-flag mutually exclusive with D).
pub const DATA_FLAG_KEY: u8 = 0x08;
/// DATA-Submessage Flag: N (non-standard payload).
pub const DATA_FLAG_NON_STANDARD: u8 = 0x10;

/// DATA submessage. Phase 0 supports only the D-flag (data), no Q
/// (no inline QoS), no K, no N.
///
/// `serialized_payload` is `Arc<[u8]>` (WP 2.0a zero-copy spike).
/// Writers share the payload allocation with `CacheChange` and all
/// DATA/DATA_FRAG datagrams — `clone()` on this struct is a pure
/// refcount bump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSubmessage {
    /// Reserved extra flags (uint16, mostly 0).
    pub extra_flags: u16,
    /// Receiver EntityId.
    pub reader_id: EntityId,
    /// Sender EntityId.
    pub writer_id: EntityId,
    /// Sequence number of this DATA.
    pub writer_sn: SequenceNumber,
    /// Inline-QoS ParameterList (Q-flag, §9.4.5.3.2). `None` = no
    /// Q-flag, no inline-QoS block. Carrier for PID_KEY_HASH (WP 1.B),
    /// PID_STATUS_INFO, PID_COHERENT_SET etc.
    pub inline_qos: Option<crate::parameter_list::ParameterList>,
    /// D-flag (spec §9.4.5.3, `DataFlag`). `true`:
    /// `serialized_payload` holds a full sample. D and K are mutually
    /// exclusive — a DATA that carries only the key clears D and sets K,
    /// and a DATA that carries neither clears both.
    ///
    /// The flag is a field and not a constant because a peer sends all
    /// three combinations. OpenDDS 3.34.0 announces every instance with
    /// a `D=0, K=1` DATA before the first sample; a decoder that dropped
    /// the flag and an encoder that always set it turned that
    /// announcement into a sample with an empty payload, and the
    /// round-trip against our own encoder could not see it.
    pub data_flag: bool,
    /// K-flag (spec §8.3.8.2 Tab. 8.43). `true`: `serialized_payload`
    /// contains only the @key fields (key-only sample, e.g. a dispose
    /// marker). The D-flag can be false at the same time when only the
    /// key is sent; in that case `serialized_payload` is an
    /// XCDR-encoded key holder.
    pub key_flag: bool,
    /// N-flag (spec §8.3.8.2 Tab. 8.43, NonStandardPayloadFlag).
    /// `true`: `serialized_payload` is NOT encoded in the CDR variant
    /// implied by `representation_identifier` (e.g. for
    /// DDS-Security-encrypted payloads).
    pub non_standard_flag: bool,
    /// Serialized payload (XCDR2-encoded or vendor-specific). A range of
    /// the received datagram on the receive path; see
    /// [`crate::payload::Payload`].
    pub serialized_payload: crate::payload::Payload,
}

impl DataSubmessage {
    /// Encodes the DATA body (without the submessage header) into a Vec.
    /// Automatically sets the D-flag and, if applicable, the Q-flag in
    /// the `flags` output (returned), so the caller can fill the
    /// submessage header correctly.
    ///
    /// Layout: extraFlags(2) + octetsToInlineQos(2) + readerId(4) +
    /// writerId(4) + writerSN(8) + [optional InlineQoS PL] + payload.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        // Encode inline QoS once (instead of amortizing it with
        // `out.extend_from_slice`) and thereby set the Vec capacity
        // *exactly*. This eliminates the reallocation cascade with
        // `extend_from_slice` — visible in the macOS recv-thread profile
        // (`finish_grow`/`reserve` ~17% of samples before this refactor).
        let inline_qos_buf = self
            .inline_qos
            .as_ref()
            .map(|pl| pl.to_bytes(little_endian));
        let inline_qos_len = inline_qos_buf.as_ref().map_or(0, |v| v.len());
        // 20 = extraFlags(2)+octetsToInlineQos(2)+readerId(4)+writerId(4)
        // +writerSN(8).
        let mut out = Vec::with_capacity(20 + inline_qos_len + self.serialized_payload.len());
        // extraFlags (2 byte)
        let extra = if little_endian {
            self.extra_flags.to_le_bytes()
        } else {
            self.extra_flags.to_be_bytes()
        };
        out.extend_from_slice(&extra);
        // octetsToInlineQos (2 byte) — distance from the end of this field to
        // the start of readerId. Constant 16 (4 readerId + 4 writerId
        // + 8 writerSN), independent of the Q flag.
        let octets_to_inline_qos: u16 = 16;
        let oti = if little_endian {
            octets_to_inline_qos.to_le_bytes()
        } else {
            octets_to_inline_qos.to_be_bytes()
        };
        out.extend_from_slice(&oti);
        // readerId, writerId
        out.extend_from_slice(&self.reader_id.to_bytes());
        out.extend_from_slice(&self.writer_id.to_bytes());
        // writerSN
        out.extend_from_slice(&if little_endian {
            self.writer_sn.to_bytes_le()
        } else {
            self.writer_sn.to_bytes_be()
        });
        // Inline-QoS ParameterList (Q-flag) — if present.
        if let Some(qos_bytes) = inline_qos_buf {
            out.extend_from_slice(&qos_bytes);
        }
        // serializedPayload
        out.extend_from_slice(&self.serialized_payload);

        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        if self.data_flag {
            flags |= DATA_FLAG_DATA;
        }
        if self.key_flag {
            flags |= DATA_FLAG_KEY;
        }
        if self.non_standard_flag {
            flags |= DATA_FLAG_NON_STANDARD;
        }
        if self.inline_qos.is_some() {
            flags |= DATA_FLAG_INLINE_QOS;
        }
        (out, flags)
    }

    /// Decodes the DATA body from a slice with **no flags set**: no
    /// inline QoS, and `data_flag`, `key_flag` and `non_standard_flag`
    /// all false. A caller that has the submessage header's flag byte
    /// must pass it to [`Self::read_body_with_flags`] instead — the
    /// flags decide how `serializedPayload` is read.
    ///
    /// # Errors
    /// `UnexpectedEof` on a too-short body.
    pub fn read_body(body: &[u8], little_endian: bool) -> Result<Self, WireError> {
        Self::read_body_with_flags(body, little_endian, 0)
    }

    /// Decodes the DATA body taking the submessage flags into account.
    /// If `flags & DATA_FLAG_INLINE_QOS != 0` the decoder parses the
    /// ParameterList after the writerSN, before the payload.
    ///
    /// # Errors
    /// `UnexpectedEof` on a too-short body. ParameterList errors are
    /// passed through as `WireError`.
    pub fn read_body_with_flags(
        body: &[u8],
        little_endian: bool,
        flags: u8,
    ) -> Result<Self, WireError> {
        Self::read_body_into(body, little_endian, flags, |payload| {
            crate::payload::Payload::from(&body[payload])
        })
    }

    /// [`Self::read_body_with_flags`] for a body that is `body` of
    /// `buffer`: the serializedPayload is that range of `buffer`, shared,
    /// and not copied out of it.
    ///
    /// # Errors
    /// As [`Self::read_body_with_flags`].
    pub fn read_body_shared(
        buffer: &Arc<[u8]>,
        body: core::ops::Range<usize>,
        little_endian: bool,
        flags: u8,
    ) -> Result<Self, WireError> {
        let start = body.start;
        Self::read_body_into(&buffer[body], little_endian, flags, |payload| {
            crate::payload::Payload::slice(buffer, start + payload.start..start + payload.end)
        })
    }

    /// The decoder behind both, with the payload made by `payload_of` from
    /// its range within `body`.
    fn read_body_into(
        body: &[u8],
        little_endian: bool,
        flags: u8,
        payload_of: impl FnOnce(core::ops::Range<usize>) -> crate::payload::Payload,
    ) -> Result<Self, WireError> {
        if body.len() < 4 + 4 + 4 + 8 {
            return Err(WireError::UnexpectedEof {
                needed: 20,
                offset: 0,
            });
        }
        let mut pos = 0usize;
        let mut ef = [0u8; 2];
        ef.copy_from_slice(&body[pos..pos + 2]);
        let extra_flags = if little_endian {
            u16::from_le_bytes(ef)
        } else {
            u16::from_be_bytes(ef)
        };
        pos += 2;
        // octetsToInlineQos: "the number of octets starting from the first
        // octet immediately following this field until the first octet of
        // the inlineQos SubmessageElement"; a receiver "should always use
        // the octetsToInlineQos to skip any submessage header elements it
        // does not expect or understand" (DDSI-RTPS 2.5 §9.4.5.4). This
        // version's elements up to the inline QoS take 16 octets.
        let mut oq = [0u8; 2];
        oq.copy_from_slice(&body[pos..pos + 2]);
        let octets_to_inline_qos = usize::from(if little_endian {
            u16::from_le_bytes(oq)
        } else {
            u16::from_be_bytes(oq)
        });
        pos += 2;
        let after_header = pos + octets_to_inline_qos;
        if octets_to_inline_qos < 16 || after_header > body.len() {
            return Err(WireError::ValueOutOfRange {
                message: "DATA octetsToInlineQos below 16 or beyond the submessage",
            });
        }
        let mut rid = [0u8; 4];
        rid.copy_from_slice(&body[pos..pos + 4]);
        let reader_id = EntityId::from_bytes(rid);
        pos += 4;
        let mut wid = [0u8; 4];
        wid.copy_from_slice(&body[pos..pos + 4]);
        let writer_id = EntityId::from_bytes(wid);
        pos += 4;
        let mut sn = [0u8; 8];
        sn.copy_from_slice(&body[pos..pos + 8]);
        let writer_sn = if little_endian {
            SequenceNumber::from_bytes_le(sn)
        } else {
            SequenceNumber::from_bytes_be(sn)
        };
        // §8.3.8.2: invalid when "writerSN.value is not strictly positive
        // (1, 2, ...) or is SEQUENCENUMBER_UNKNOWN". UNKNOWN is {-1, 0}, a
        // negative value.
        if writer_sn.0 < 1 {
            return Err(WireError::ValueOutOfRange {
                message: "DATA writerSN not strictly positive (DDSI-RTPS 2.5 §8.3.8.2)",
            });
        }
        // §9.4.5.3: "D=1 and K=1 is an invalid combination in this version
        // of the protocol."
        if flags & DATA_FLAG_DATA != 0 && flags & DATA_FLAG_KEY != 0 {
            return Err(WireError::ValueOutOfRange {
                message: "DATA with both the D and the K flag (DDSI-RTPS 2.5 §9.4.5.3)",
            });
        }
        // Header elements of a later version: skipped.
        pos = after_header;

        // Inline QoS — Q-flag (DATA_FLAG_INLINE_QOS = 0x02).
        let inline_qos = if flags & DATA_FLAG_INLINE_QOS != 0 {
            // The parser's own count of the octets it read, PID_IGNORE
            // parameters and the sentinel included. The length used to be
            // that of the parsed list encoded again, which leaves out a
            // skipped PID_IGNORE: a DATA whose inline QoS held one had its
            // serializedPayload start that many octets early, and the
            // sample decoded from the wrong octets. Encoding the list again
            // was also three allocations per received sample.
            let (pl, consumed) = crate::parameter_list::ParameterList::from_bytes_with_consumed(
                &body[pos..],
                little_endian,
            )?;
            pos += consumed;
            Some(pl)
        } else {
            None
        };

        // The rest is serializedPayload.
        let serialized_payload = payload_of(pos..body.len());
        let data_flag = (flags & DATA_FLAG_DATA) != 0;
        let key_flag = (flags & DATA_FLAG_KEY) != 0;
        let non_standard_flag = (flags & DATA_FLAG_NON_STANDARD) != 0;
        Ok(Self {
            extra_flags,
            reader_id,
            writer_id,
            writer_sn,
            inline_qos,
            data_flag,
            key_flag,
            non_standard_flag,
            serialized_payload,
        })
    }
}

// ============================================================================
// HEARTBEAT Submessage (§8.3.8.6)
// ============================================================================

/// HEARTBEAT Flag: F (Final).
pub const HEARTBEAT_FLAG_FINAL: u8 = 0x02;
/// HEARTBEAT Flag: L (Liveliness).
pub const HEARTBEAT_FLAG_LIVELINESS: u8 = 0x04;
/// HEARTBEAT flag G, the GroupInfoFlag: `currentGSN`, `firstGSN` and
/// `lastGSN` follow the `count` (DDSI-RTPS 2.5 §8.3.8.6.2, §8.7.5).
/// §9.4.5.7: "G = SubmessageHeader.flags & 0x08".
pub const HEARTBEAT_FLAG_GROUP_INFO: u8 = 0x08;

/// The group sequence numbers of a HEARTBEAT with the GroupInfoFlag
/// (§8.3.8.6 Table 8.44, §9.4.5.7).
///
/// The wire form is the three `SequenceNumber`s, 24 octets, and the two
/// `GroupDigest`s `writerSet` and `secureWriterSet` of Table 8.44, four
/// octets each (§9.4.2.19). The figure of §9.4.5.7 shows only the three
/// numbers; the PIM and §8.7.5 ask for the sets. A writer sends all five
/// (unless the peer's behaviour omits the sets,
/// [`crate::dialect::HeartbeatGroupDigests`]); a reader reads the sets when
/// the submessage leaves room and accepts their absence. See "Widersprüche
/// innerhalb der Spec" in the audit findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatGroupInfo {
    /// "the last (highest) group sequence number written by any
    /// DataWriter in the Writer's Group at the time that the HeartBeat
    /// was sent".
    pub current_gsn: SequenceNumber,
    /// The group sequence number of the sample `firstSN`.
    pub first_gsn: SequenceNumber,
    /// The group sequence number of the sample `lastSN`.
    pub last_gsn: SequenceNumber,
    /// `writerSet`: the writers of the group the publisher had announced
    /// when the HEARTBEAT was sent. `None`: not on the wire (the peer left
    /// it out, or the submessage had no room for it).
    pub writer_set: Option<crate::group_digest::GroupDigest>,
    /// `secureWriterSet`: "Reserved for use by the DDS-Security
    /// Specification". ZeroDDS writes the empty group. `None`: not on the
    /// wire. Written and read together with `writer_set`.
    pub secure_writer_set: Option<crate::group_digest::GroupDigest>,
}

impl HeartbeatGroupInfo {
    /// Wire size of the three `SequenceNumber`s, the part every
    /// HEARTBEAT with the GroupInfoFlag has.
    pub const WIRE_SIZE: usize = 24;
    /// Wire size of `writerSet` and `secureWriterSet` (two `GroupDigest`s).
    pub const DIGESTS_WIRE_SIZE: usize = 8;

    /// §8.3.8.6 Validity: the submessage is invalid when `currentGSN` or
    /// `firstGSN` is zero or negative, `lastGSN` is negative, `lastGSN <
    /// firstGSN - 1` or `currentGSN < firstGSN`.
    ///
    /// Not enforced: "currentGSN.value > lastGSN.value". `currentGSN` is
    /// the highest number of the whole group and `lastGSN` the one of
    /// this writer's last sample, so whenever another writer of the group
    /// wrote last the rule would reject a correct HEARTBEAT; §8.7.5's
    /// commit rule reads `currentGSN` by its definition.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let (c, f, l) = (self.current_gsn.0, self.first_gsn.0, self.last_gsn.0);
        c > 0 && f > 0 && l >= 0 && l >= f - 1 && c >= f
    }
}

/// HEARTBEAT submessage.
///
/// `final_flag`, `liveliness_flag` and `group_info_flag` (via `Some` of
/// `group_info`) correspond to the F-/L-/G bits in the submessage header
/// (spec §8.3.8.6, §9.4.5.7) — they are **not** in the body, but are
/// carried here as a semantic part of the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatSubmessage {
    /// Reader EntityId (target).
    pub reader_id: EntityId,
    /// Writer EntityId (source).
    pub writer_id: EntityId,
    /// First available sequence number in the history cache.
    pub first_sn: SequenceNumber,
    /// Last sent sequence number.
    pub last_sn: SequenceNumber,
    /// Count_t (i32) — heartbeat sequence number (for ACK correlation).
    pub count: i32,
    /// F-flag: `true` = the reader need not send a response when complete.
    pub final_flag: bool,
    /// L-flag: liveliness announce (without history semantics).
    pub liveliness_flag: bool,
    /// G-flag (§8.3.8.6.2): optional GroupInfo trailer.
    pub group_info: Option<HeartbeatGroupInfo>,
}

impl HeartbeatSubmessage {
    /// Minimal wire size (body without GroupInfo): 28 bytes (4+4+8+8+4).
    /// Flags are in the submessage header.
    pub const WIRE_SIZE: usize = 28;

    /// Encodes the body. Returns (bytes, flags), where `flags` is the
    /// submessage-header flag byte incl. E/F/L/G.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::with_capacity(Self::WIRE_SIZE);
        out.extend_from_slice(&self.reader_id.to_bytes());
        out.extend_from_slice(&self.writer_id.to_bytes());
        out.extend_from_slice(&if little_endian {
            self.first_sn.to_bytes_le()
        } else {
            self.first_sn.to_bytes_be()
        });
        out.extend_from_slice(&if little_endian {
            self.last_sn.to_bytes_le()
        } else {
            self.last_sn.to_bytes_be()
        });
        out.extend_from_slice(&if little_endian {
            self.count.to_le_bytes()
        } else {
            self.count.to_be_bytes()
        });
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        if self.final_flag {
            flags |= HEARTBEAT_FLAG_FINAL;
        }
        if self.liveliness_flag {
            flags |= HEARTBEAT_FLAG_LIVELINESS;
        }
        if let Some(gi) = &self.group_info {
            flags |= HEARTBEAT_FLAG_GROUP_INFO;
            for sn in [gi.current_gsn, gi.first_gsn, gi.last_gsn] {
                out.extend_from_slice(&if little_endian {
                    sn.to_bytes_le()
                } else {
                    sn.to_bytes_be()
                });
            }
            // §9.4.2.19: a GroupDigest is four octets (`octet value[4]`),
            // which have no byte order.
            if let Some(writer_set) = gi.writer_set {
                out.extend_from_slice(&writer_set.to_bytes());
                out.extend_from_slice(&gi.secure_writer_set.unwrap_or_default().to_bytes());
            }
        }
        (out, flags)
    }

    /// Decodes the body. `final_flag`, `liveliness_flag`,
    /// `group_info_flag` are extracted by the caller from the submessage header.
    ///
    /// # Errors
    /// `UnexpectedEof`; `ValueOutOfRange` for group sequence numbers that
    /// make the submessage invalid ([`HeartbeatGroupInfo::is_valid`]).
    pub fn read_body(
        body: &[u8],
        little_endian: bool,
        final_flag: bool,
        liveliness_flag: bool,
        group_info_flag: bool,
    ) -> Result<Self, WireError> {
        if body.len() < Self::WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        let mut pos = 0usize;
        let mut rid = [0u8; 4];
        rid.copy_from_slice(&body[pos..pos + 4]);
        let reader_id = EntityId::from_bytes(rid);
        pos += 4;
        let mut wid = [0u8; 4];
        wid.copy_from_slice(&body[pos..pos + 4]);
        let writer_id = EntityId::from_bytes(wid);
        pos += 4;
        let mut sn = [0u8; 8];
        sn.copy_from_slice(&body[pos..pos + 8]);
        let first_sn = if little_endian {
            SequenceNumber::from_bytes_le(sn)
        } else {
            SequenceNumber::from_bytes_be(sn)
        };
        pos += 8;
        sn.copy_from_slice(&body[pos..pos + 8]);
        let last_sn = if little_endian {
            SequenceNumber::from_bytes_le(sn)
        } else {
            SequenceNumber::from_bytes_be(sn)
        };
        pos += 8;
        let mut cnt = [0u8; 4];
        cnt.copy_from_slice(&body[pos..pos + 4]);
        let count = if little_endian {
            i32::from_le_bytes(cnt)
        } else {
            i32::from_be_bytes(cnt)
        };
        pos += 4;
        // §8.3.8.6: invalid when "firstSN.value is zero or negative",
        // "lastSN.value is negative" or "lastSN.value < firstSN.value - 1".
        if first_sn.0 < 1 || last_sn.0 < 0 || last_sn.0 < first_sn.0 - 1 {
            return Err(WireError::ValueOutOfRange {
                message: "HEARTBEAT firstSN/lastSN invalid (DDSI-RTPS 2.5 §8.3.8.6)",
            });
        }
        let group_info = if group_info_flag {
            if body.len() < pos + HeartbeatGroupInfo::WIRE_SIZE {
                return Err(WireError::UnexpectedEof {
                    needed: HeartbeatGroupInfo::WIRE_SIZE,
                    offset: pos,
                });
            }
            let mut s = [0u8; 8];
            s.copy_from_slice(&body[pos..pos + 8]);
            let current_gsn = if little_endian {
                SequenceNumber::from_bytes_le(s)
            } else {
                SequenceNumber::from_bytes_be(s)
            };
            pos += 8;
            s.copy_from_slice(&body[pos..pos + 8]);
            let first_gsn = if little_endian {
                SequenceNumber::from_bytes_le(s)
            } else {
                SequenceNumber::from_bytes_be(s)
            };
            pos += 8;
            s.copy_from_slice(&body[pos..pos + 8]);
            let last_gsn = if little_endian {
                SequenceNumber::from_bytes_le(s)
            } else {
                SequenceNumber::from_bytes_be(s)
            };
            pos += 8;
            // writerSet and secureWriterSet follow where the submessage
            // leaves room for both; a HEARTBEAT in the form of the figure
            // of §9.4.5.7 has neither.
            let (writer_set, secure_writer_set) =
                if body.len() >= pos + HeartbeatGroupInfo::DIGESTS_WIRE_SIZE {
                    let mut w = [0u8; 4];
                    w.copy_from_slice(&body[pos..pos + 4]);
                    let mut x = [0u8; 4];
                    x.copy_from_slice(&body[pos + 4..pos + 8]);
                    (
                        Some(crate::group_digest::GroupDigest::from_bytes(w)),
                        Some(crate::group_digest::GroupDigest::from_bytes(x)),
                    )
                } else {
                    (None, None)
                };
            let gi = HeartbeatGroupInfo {
                current_gsn,
                first_gsn,
                last_gsn,
                writer_set,
                secure_writer_set,
            };
            if !gi.is_valid() {
                return Err(WireError::ValueOutOfRange {
                    message: "HEARTBEAT group sequence numbers invalid (DDSI-RTPS 2.5 §8.3.8.6)",
                });
            }
            Some(gi)
        } else {
            None
        };
        Ok(Self {
            reader_id,
            writer_id,
            first_sn,
            last_sn,
            count,
            final_flag,
            liveliness_flag,
            group_info,
        })
    }
}

// ============================================================================
// ACKNACK Submessage (§8.3.8.1)
// ============================================================================

/// ACKNACK Flag: F (Final).
pub const ACKNACK_FLAG_FINAL: u8 = 0x02;

/// ACKNACK submessage.
///
/// `final_flag` corresponds to the F-bit in the submessage header (spec
/// §8.3.8.1). `final=false` requires a timely HEARTBEAT response from
/// the writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckNackSubmessage {
    /// Reader EntityId (source).
    pub reader_id: EntityId,
    /// Writer EntityId (target).
    pub writer_id: EntityId,
    /// Bitset of the not-yet-received sequence numbers.
    pub reader_sn_state: SequenceNumberSet,
    /// Count_t (for correlation with HEARTBEAT.count).
    pub count: i32,
    /// F-flag: `false` = the writer should answer with a timely HEARTBEAT.
    pub final_flag: bool,
}

impl AckNackSubmessage {
    /// Encodes the body. Returns (bytes, flags).
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        // 4 readerId + 4 writerId + 12 SN base + 4*num_words bitmap +
        // 4 count. The SN set can be variable; 12 + words*4 is the upper
        // bound. Pre-allocating saves the realloc step on the recv-thread
        // hot path (HEARTBEAT response).
        let snset_words = self.reader_sn_state.bitmap.len();
        let mut out = Vec::with_capacity(4 + 4 + 12 + snset_words * 4 + 4);
        out.extend_from_slice(&self.reader_id.to_bytes());
        out.extend_from_slice(&self.writer_id.to_bytes());
        self.reader_sn_state.write_to(&mut out, little_endian);
        out.extend_from_slice(&if little_endian {
            self.count.to_le_bytes()
        } else {
            self.count.to_be_bytes()
        });
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        if self.final_flag {
            flags |= ACKNACK_FLAG_FINAL;
        }
        (out, flags)
    }

    /// Decodes the body. `final_flag` is extracted by the caller from
    /// the submessage header.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_body(
        body: &[u8],
        little_endian: bool,
        final_flag: bool,
    ) -> Result<Self, WireError> {
        if body.len() < 8 {
            return Err(WireError::UnexpectedEof {
                needed: 8,
                offset: 0,
            });
        }
        let mut pos = 0usize;
        let mut rid = [0u8; 4];
        rid.copy_from_slice(&body[pos..pos + 4]);
        let reader_id = EntityId::from_bytes(rid);
        pos += 4;
        let mut wid = [0u8; 4];
        wid.copy_from_slice(&body[pos..pos + 4]);
        let writer_id = EntityId::from_bytes(wid);
        pos += 4;
        let (reader_sn_state, new_pos) = SequenceNumberSet::read_from_with(
            body,
            pos,
            little_endian,
            accepts_empty_acknack_base_zero(),
        )?;
        pos = new_pos;
        if body.len() < pos + 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: pos,
            });
        }
        let mut cnt = [0u8; 4];
        cnt.copy_from_slice(&body[pos..pos + 4]);
        let count = if little_endian {
            i32::from_le_bytes(cnt)
        } else {
            i32::from_be_bytes(cnt)
        };
        Ok(Self {
            reader_id,
            writer_id,
            reader_sn_state,
            count,
            final_flag,
        })
    }
}

// ============================================================================
// GAP Submessage (§8.3.8.4)
// ============================================================================

/// GAP flag G, the GroupInfoFlag: `gapStartGSN` and `gapEndGSN` follow
/// the `gapList` (DDSI-RTPS 2.5 §8.3.8.4.2, §8.7.5). §9.4.5.6:
/// "G = SubmessageHeader.flags & 0x02".
pub const GAP_FLAG_GROUP_INFO: u8 = 0x02;

/// GAP flag F, the FilteredCountFlag: a `ChangeCount filteredCount`
/// follows, which lets the reader tell a change filtered for it from one
/// removed (§8.3.8.4.2, §8.7.4: NOT_ALIVE_FILTERED vs.
/// NOT_ALIVE_DISPOSED). §9.4.5.6: "F = SubmessageHeader.flags & 0x04".
pub const GAP_FLAG_FILTERED_COUNT: u8 = 0x04;

/// The group sequence numbers of a GAP with the GroupInfoFlag (§8.3.8.4
/// Table 8.43, §9.4.5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GapGroupInfo {
    /// The group sequence number of the sample `gapStart`.
    pub gap_start_gsn: SequenceNumber,
    /// "the end of a continuous range of GSNs starting at gapStartGSN
    /// that are not available to the Reader"; it may reach past the GSN
    /// of `gapList.bitmapBase` where other writers of the group wrote
    /// the numbers between.
    pub gap_end_gsn: SequenceNumber,
}

impl GapGroupInfo {
    /// §8.3.8.4 Validity: the submessage is invalid when `gapStartGSN` or
    /// `gapEndGSN` is zero or negative, or `gapEndGSN < gapStartGSN - 1`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let (start, end) = (self.gap_start_gsn.0, self.gap_end_gsn.0);
        start > 0 && end > 0 && end >= start - 1
    }
}

/// GAP submessage. Signals the reader that the writer will never send
/// sequence numbers `[gap_start, gap_list.bitmap_base)` (all before
/// `gap_list.bitmap_base` are gaps; the bits in `gap_list` mark
/// individual further gaps from `bitmap_base`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapSubmessage {
    /// Reader EntityId (target).
    pub reader_id: EntityId,
    /// Writer EntityId (source).
    pub writer_id: EntityId,
    /// First irreversible gap SN.
    pub gap_start: SequenceNumber,
    /// Bitset of the further gaps from `gap_list.bitmap_base`.
    pub gap_list: SequenceNumberSet,
    /// Optional GroupInfo (§8.3.8.4.2). `Some` ⇒ G flag set in the header.
    pub group_info: Option<GapGroupInfo>,
    /// Optional `filteredCount` (§8.3.8.4.2, §9.4.5.6). `Some` ⇒ the
    /// FilteredCountFlag in the header. A `ChangeCount_t` (§9.3.2.11):
    /// "a 64-bit count … a structure that contains two 32-bit integers:
    /// high and low", eight octets on the wire (§9.4.2.14). `0` is
    /// "nothing filtered, everything really removed"; `n` means "n of the
    /// changes discarded by a content filter".
    pub filtered_count: Option<u64>,
}

impl GapSubmessage {
    /// Encodes the body. Returns (bytes, flags) incl. possible G/F bit.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        // 4 readerId + 4 writerId + 8 gap_start + 12 SN-Set-Base +
        // 4*words. Optional GroupInfo (24) + filteredCount (8) on top.
        let snset_words = self.gap_list.bitmap.len();
        let extra =
            self.group_info.as_ref().map_or(0, |_| 24) + self.filtered_count.map_or(0, |_| 8);
        let mut out = Vec::with_capacity(4 + 4 + 8 + 12 + snset_words * 4 + extra);
        out.extend_from_slice(&self.reader_id.to_bytes());
        out.extend_from_slice(&self.writer_id.to_bytes());
        out.extend_from_slice(&if little_endian {
            self.gap_start.to_bytes_le()
        } else {
            self.gap_start.to_bytes_be()
        });
        self.gap_list.write_to(&mut out, little_endian);
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        if let Some(gi) = self.group_info {
            flags |= GAP_FLAG_GROUP_INFO;
            out.extend_from_slice(&if little_endian {
                gi.gap_start_gsn.to_bytes_le()
            } else {
                gi.gap_start_gsn.to_bytes_be()
            });
            out.extend_from_slice(&if little_endian {
                gi.gap_end_gsn.to_bytes_le()
            } else {
                gi.gap_end_gsn.to_bytes_be()
            });
        }
        if let Some(fc) = self.filtered_count {
            flags |= GAP_FLAG_FILTERED_COUNT;
            // `long high; unsigned long low;` (§9.3.2.11).
            #[allow(clippy::cast_possible_truncation)]
            let (high, low) = ((fc >> 32) as u32, fc as u32);
            for word in [high, low] {
                out.extend_from_slice(&if little_endian {
                    word.to_le_bytes()
                } else {
                    word.to_be_bytes()
                });
            }
        }
        (out, flags)
    }

    /// Decodes the body. Flags G/K are passed by the caller from the
    /// submessage header (see `decode_datagram`).
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_body(
        body: &[u8],
        little_endian: bool,
        group_info_flag: bool,
        filtered_count_flag: bool,
    ) -> Result<Self, WireError> {
        if body.len() < 4 + 4 + 8 {
            return Err(WireError::UnexpectedEof {
                needed: 16,
                offset: 0,
            });
        }
        let mut pos = 0usize;
        let mut rid = [0u8; 4];
        rid.copy_from_slice(&body[pos..pos + 4]);
        let reader_id = EntityId::from_bytes(rid);
        pos += 4;
        let mut wid = [0u8; 4];
        wid.copy_from_slice(&body[pos..pos + 4]);
        let writer_id = EntityId::from_bytes(wid);
        pos += 4;
        let mut sn = [0u8; 8];
        sn.copy_from_slice(&body[pos..pos + 8]);
        let gap_start = if little_endian {
            SequenceNumber::from_bytes_le(sn)
        } else {
            SequenceNumber::from_bytes_be(sn)
        };
        pos += 8;
        // §8.3.8.4: invalid when "gapStart is zero or negative".
        if gap_start.0 < 1 {
            return Err(WireError::ValueOutOfRange {
                message: "GAP gapStart zero or negative (DDSI-RTPS 2.5 §8.3.8.4)",
            });
        }
        let (gap_list, new_pos) = SequenceNumberSet::read_from(body, pos, little_endian)?;
        pos = new_pos;
        let group_info = if group_info_flag {
            if body.len() < pos + 16 {
                return Err(WireError::UnexpectedEof {
                    needed: 16,
                    offset: pos,
                });
            }
            let mut s = [0u8; 8];
            s.copy_from_slice(&body[pos..pos + 8]);
            let gap_start_gsn = if little_endian {
                SequenceNumber::from_bytes_le(s)
            } else {
                SequenceNumber::from_bytes_be(s)
            };
            pos += 8;
            s.copy_from_slice(&body[pos..pos + 8]);
            let gap_end_gsn = if little_endian {
                SequenceNumber::from_bytes_le(s)
            } else {
                SequenceNumber::from_bytes_be(s)
            };
            pos += 8;
            let gi = GapGroupInfo {
                gap_start_gsn,
                gap_end_gsn,
            };
            if !gi.is_valid() {
                return Err(WireError::ValueOutOfRange {
                    message: "GAP group sequence numbers invalid (DDSI-RTPS 2.5 §8.3.8.4)",
                });
            }
            Some(gi)
        } else {
            None
        };
        let filtered_count = if filtered_count_flag {
            if body.len() < pos + 8 {
                return Err(WireError::UnexpectedEof {
                    needed: 8,
                    offset: pos,
                });
            }
            let word = |at: usize| {
                let w = [body[at], body[at + 1], body[at + 2], body[at + 3]];
                if little_endian {
                    u32::from_le_bytes(w)
                } else {
                    u32::from_be_bytes(w)
                }
            };
            // "change_count = low + high * 2^(32)" (§9.3.2.11); `high` is a
            // `long`, and a negative count is none.
            let (high, low) = (word(pos), word(pos + 4));
            if high > i32::MAX as u32 {
                return Err(WireError::ValueOutOfRange {
                    message: "GAP filteredCount negative (DDSI-RTPS 2.5 §9.3.2.11)",
                });
            }
            Some((u64::from(high) << 32) | u64::from(low))
        } else {
            None
        };
        Ok(Self {
            reader_id,
            writer_id,
            gap_start,
            gap_list,
            group_info,
            filtered_count,
        })
    }
}

// ============================================================================
// DATA_FRAG Submessage (§8.3.8.3)
// ============================================================================

/// DATA_FRAG Flag: Q (Inline-QoS present).
pub const DATA_FRAG_FLAG_INLINE_QOS: u8 = 0x02;
/// DATA_FRAG flag: K (key flag — serialized_payload is key instead of data).
pub const DATA_FRAG_FLAG_KEY: u8 = 0x04;
/// DATA_FRAG Flag: N (non-standard payload).
pub const DATA_FRAG_FLAG_NON_STANDARD: u8 = 0x08;

/// DATA_FRAG submessage. Carries a section (fragments) of a sample
/// whose total size is in `sample_size`.
///
/// Flags Q/K/N follow DDSI-RTPS 2.5 §9.4.5.5. DATA_FRAG has no separate H flag; key hashes are carried by PID_KEY_HASH in inline QoS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFragSubmessage {
    /// Parsed inline QoS, retained for fragmented keyed samples.
    pub inline_qos: Option<crate::parameter_list::ParameterList>,
    /// octetsToInlineQos analogue to DATA (§8.3.8.2 speaks of extraFlags+
    /// octetsToInlineQos; this variant carries 0).
    pub extra_flags: u16,
    /// Reader EntityId (target).
    pub reader_id: EntityId,
    /// Writer EntityId (source).
    pub writer_id: EntityId,
    /// Sequence number of the sample whose fragments this carries.
    pub writer_sn: SequenceNumber,
    /// First fragment in this submessage (1-based).
    pub fragment_starting_num: FragmentNumber,
    /// Number of fragments in this submessage. Writer: always 1.
    pub fragments_in_submessage: u16,
    /// Size of a single fragment (the last may be shorter).
    pub fragment_size: u16,
    /// Total size of the sample in bytes.
    pub sample_size: u32,
    /// Fragmented payload section. Arc-shared:
    /// writer re-sends are just refcount bumps, no copy.
    pub serialized_payload: Arc<[u8]>,
    /// Q-flag from the submessage header (inline_qos present).
    pub inline_qos_flag: bool,
    /// K-flag from the submessage header (serialized_payload = key).
    pub key_flag: bool,
    /// N-flag from the submessage header (non-standard payload).
    pub non_standard_flag: bool,
}

impl DataFragSubmessage {
    /// Minimal body size without payload: extraFlags(2) + octetsToInlineQos(2)
    /// + readerId(4) + writerId(4) + writerSN(8) + fragmentStartingNum(4)
    /// + fragmentsInSubmessage(2) + fragmentSize(2) + sampleSize(4) = 32.
    pub const HEADER_WIRE_SIZE: usize = 32;

    /// octetsToInlineQos: offset from the end of this field to the start
    /// of inlineQos or serializedPayload. Variant with
    /// Q=false: offset = 28 (readerId..sampleSize).
    pub const OCTETS_TO_INLINE_QOS: u16 = 28;

    /// Encodes the body. Returns (bytes, flags).
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::with_capacity(Self::HEADER_WIRE_SIZE + self.serialized_payload.len());
        if little_endian {
            out.extend_from_slice(&self.extra_flags.to_le_bytes());
            out.extend_from_slice(&Self::OCTETS_TO_INLINE_QOS.to_le_bytes());
        } else {
            out.extend_from_slice(&self.extra_flags.to_be_bytes());
            out.extend_from_slice(&Self::OCTETS_TO_INLINE_QOS.to_be_bytes());
        }
        out.extend_from_slice(&self.reader_id.to_bytes());
        out.extend_from_slice(&self.writer_id.to_bytes());
        out.extend_from_slice(&if little_endian {
            self.writer_sn.to_bytes_le()
        } else {
            self.writer_sn.to_bytes_be()
        });
        out.extend_from_slice(&if little_endian {
            self.fragment_starting_num.to_bytes_le()
        } else {
            self.fragment_starting_num.to_bytes_be()
        });
        if little_endian {
            out.extend_from_slice(&self.fragments_in_submessage.to_le_bytes());
            out.extend_from_slice(&self.fragment_size.to_le_bytes());
            out.extend_from_slice(&self.sample_size.to_le_bytes());
        } else {
            out.extend_from_slice(&self.fragments_in_submessage.to_be_bytes());
            out.extend_from_slice(&self.fragment_size.to_be_bytes());
            out.extend_from_slice(&self.sample_size.to_be_bytes());
        }
        if let Some(qos) = &self.inline_qos {
            out.extend_from_slice(&qos.to_bytes(little_endian));
        }
        out.extend_from_slice(&self.serialized_payload);
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        if self.inline_qos_flag || self.inline_qos.is_some() {
            flags |= DATA_FRAG_FLAG_INLINE_QOS;
        }
        if self.key_flag {
            flags |= DATA_FRAG_FLAG_KEY;
        }
        if self.non_standard_flag {
            flags |= DATA_FRAG_FLAG_NON_STANDARD;
        }
        (out, flags)
    }

    /// Decodes the body. Flags are passed by the caller from the
    /// submessage header.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_body(
        body: &[u8],
        little_endian: bool,
        inline_qos_flag: bool,
        key_flag: bool,
        non_standard_flag: bool,
    ) -> Result<Self, WireError> {
        if body.len() < Self::HEADER_WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: Self::HEADER_WIRE_SIZE,
                offset: 0,
            });
        }
        let mut pos = 0usize;
        let mut ef = [0u8; 2];
        ef.copy_from_slice(&body[pos..pos + 2]);
        let extra_flags = if little_endian {
            u16::from_le_bytes(ef)
        } else {
            u16::from_be_bytes(ef)
        };
        pos += 2;
        // extra_flags: see DATA. 2.1 readers must ignore them
        // (Cyclone + Fast-DDS do), we do too — only read and pass on.
        // octetsToInlineQos (2 byte): offset from the start of readerId
        // to the inline QoS, or (with Q=false) to the serializedPayload.
        // This version's header elements take 28 octets; a receiver
        // "should always use the octetsToInlineQos to skip any submessage
        // header elements it does not expect or understand" (DDSI-RTPS
        // 2.5 §9.4.5.4), so a larger value is skipped, a smaller one is
        // no DATA_FRAG.
        let mut otq = [0u8; 2];
        otq.copy_from_slice(&body[pos..pos + 2]);
        let octets_to_inline_qos = if little_endian {
            u16::from_le_bytes(otq)
        } else {
            u16::from_be_bytes(otq)
        };
        pos += 2;
        if octets_to_inline_qos < Self::OCTETS_TO_INLINE_QOS {
            return Err(WireError::ValueOutOfRange {
                message: "DATA_FRAG.octetsToInlineQos below 28",
            });
        }
        let mut rid = [0u8; 4];
        rid.copy_from_slice(&body[pos..pos + 4]);
        let reader_id = EntityId::from_bytes(rid);
        pos += 4;
        let mut wid = [0u8; 4];
        wid.copy_from_slice(&body[pos..pos + 4]);
        let writer_id = EntityId::from_bytes(wid);
        pos += 4;
        let mut sn = [0u8; 8];
        sn.copy_from_slice(&body[pos..pos + 8]);
        let writer_sn = if little_endian {
            SequenceNumber::from_bytes_le(sn)
        } else {
            SequenceNumber::from_bytes_be(sn)
        };
        pos += 8;
        let mut fsn = [0u8; 4];
        fsn.copy_from_slice(&body[pos..pos + 4]);
        let fragment_starting_num = if little_endian {
            FragmentNumber::from_bytes_le(fsn)
        } else {
            FragmentNumber::from_bytes_be(fsn)
        };
        pos += 4;
        let mut fis = [0u8; 2];
        fis.copy_from_slice(&body[pos..pos + 2]);
        let fragments_in_submessage = if little_endian {
            u16::from_le_bytes(fis)
        } else {
            u16::from_be_bytes(fis)
        };
        pos += 2;
        let mut fs = [0u8; 2];
        fs.copy_from_slice(&body[pos..pos + 2]);
        let fragment_size = if little_endian {
            u16::from_le_bytes(fs)
        } else {
            u16::from_be_bytes(fs)
        };
        pos += 2;
        let mut ss = [0u8; 4];
        ss.copy_from_slice(&body[pos..pos + 4]);
        let sample_size = if little_endian {
            u32::from_le_bytes(ss)
        } else {
            u32::from_be_bytes(ss)
        };
        pos += 4;
        // §8.3.8.3: invalid when "writerSN.value is not strictly positive
        // (1, 2, ...) or is SEQUENCENUMBER_UNKNOWN", "fragmentStartingNum.value
        // is not strictly positive (1, 2, ...) or exceeds the total number of
        // fragments", or "fragmentSize exceeds dataSize". The total number
        // of fragments is dataSize / fragmentSize, rounded up.
        if writer_sn.0 < 1 {
            return Err(WireError::ValueOutOfRange {
                message: "DATA_FRAG writerSN not strictly positive (DDSI-RTPS 2.5 §8.3.8.3)",
            });
        }
        if fragment_size == 0 || u32::from(fragment_size) > sample_size {
            return Err(WireError::ValueOutOfRange {
                message: "DATA_FRAG fragmentSize zero or exceeding dataSize (DDSI-RTPS 2.5 §8.3.8.3)",
            });
        }
        if fragment_starting_num.0 < 1
            || fragment_starting_num.0 > sample_size.div_ceil(u32::from(fragment_size))
        {
            return Err(WireError::ValueOutOfRange {
                message: "DATA_FRAG fragmentStartingNum below 1 or beyond the fragments (DDSI-RTPS 2.5 §8.3.8.3)",
            });
        }
        let inline_start = 4 + usize::from(octets_to_inline_qos);
        if inline_start < pos || inline_start > body.len() {
            return Err(WireError::ValueOutOfRange {
                message: "DATA_FRAG inline QoS offset outside body",
            });
        }
        pos = inline_start;
        let inline_qos = if inline_qos_flag {
            let (qos, consumed) = crate::parameter_list::ParameterList::from_bytes_with_consumed(
                &body[pos..],
                little_endian,
            )?;
            pos += consumed;
            if pos > body.len() {
                return Err(WireError::ValueOutOfRange {
                    message: "DATA_FRAG inline QoS exceeds body",
                });
            }
            Some(qos)
        } else {
            None
        };
        // §8.3.8.3: invalid when "The size of serializedData exceeds
        // fragmentsInSubmessage * fragmentSize". The last fragment of the
        // sample is as long as the sample leaves, and Cyclone DDS and Fast
        // DDS pad it to the 4-octet alignment of the submessage (measured:
        // 72 octets for 71); that padding is allowed beyond the bound.
        let fragment_size_u64 = u64::from(fragment_size);
        let total_fragments = u64::from(sample_size).div_ceil(fragment_size_u64);
        let first = u64::from(fragment_starting_num.0);
        let count = u64::from(fragments_in_submessage);
        let max_len = if count == 0 {
            0
        } else if first + count > total_fragments {
            let tail = u64::from(sample_size) - (total_fragments - 1) * fragment_size_u64;
            ((total_fragments - first) * fragment_size_u64 + tail).next_multiple_of(4)
        } else {
            count * fragment_size_u64
        };
        if (body.len() - pos) as u64 > max_len {
            return Err(WireError::ValueOutOfRange {
                message: "DATA_FRAG serializedData larger than fragmentsInSubmessage * fragmentSize (DDSI-RTPS 2.5 §8.3.8.3)",
            });
        }
        let serialized_payload: Arc<[u8]> = Arc::from(&body[pos..]);
        Ok(Self {
            extra_flags,
            reader_id,
            writer_id,
            writer_sn,
            fragment_starting_num,
            fragments_in_submessage,
            fragment_size,
            sample_size,
            serialized_payload,
            inline_qos_flag,
            inline_qos,
            key_flag,
            non_standard_flag,
        })
    }
}

// ============================================================================
// InfoSource Submessage (§8.3.8.10) — submessageId 0x0c (legacy
// table) or 0x0A in the 2.5 PSM. We follow 2.5: id=0x0A.
// ============================================================================

/// InfoSource submessage (§8.3.8.9.4). Resets `sourceProtocolVersion`,
/// `sourceVendorId`, `sourceGuidPrefix` in the ReceiverState — all
/// subsequent submessages are attributed to this source (not the
/// datagram header).
///
/// Wire layout (body, 20 byte):
/// - unused (4 byte, "Long unused" in the spec)
/// - ProtocolVersion (2 byte)
/// - VendorId (2 byte)
/// - GuidPrefix (12 byte)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoSourceSubmessage {
    /// Reserved 4 byte (spec says: must be 0 from the sender, ignored by
    /// the receiver).
    pub unused: u32,
    /// Source ProtocolVersion (e.g. 2.5).
    pub protocol_version: crate::wire_types::ProtocolVersion,
    /// Source-VendorId (Hersteller-Kennung).
    pub vendor_id: crate::wire_types::VendorId,
    /// Source-GuidPrefix (12 byte).
    pub guid_prefix: crate::wire_types::GuidPrefix,
}

impl InfoSourceSubmessage {
    /// Wire-Size: 20 Bytes (4+2+2+12).
    pub const WIRE_SIZE: usize = 20;

    /// Encodes the body. Returns (bytes, flags) incl. E bit.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::with_capacity(Self::WIRE_SIZE);
        out.extend_from_slice(&if little_endian {
            self.unused.to_le_bytes()
        } else {
            self.unused.to_be_bytes()
        });
        out.extend_from_slice(&self.protocol_version.to_bytes());
        out.extend_from_slice(&self.vendor_id.to_bytes());
        out.extend_from_slice(&self.guid_prefix.to_bytes());
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        (out, flags)
    }

    /// Decodes the body.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_body(body: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if body.len() < Self::WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        let mut pos = 0usize;
        let mut u = [0u8; 4];
        u.copy_from_slice(&body[pos..pos + 4]);
        let unused = if little_endian {
            u32::from_le_bytes(u)
        } else {
            u32::from_be_bytes(u)
        };
        pos += 4;
        let mut pv = [0u8; 2];
        pv.copy_from_slice(&body[pos..pos + 2]);
        let protocol_version = crate::wire_types::ProtocolVersion::from_bytes(pv);
        pos += 2;
        let mut vid = [0u8; 2];
        vid.copy_from_slice(&body[pos..pos + 2]);
        let vendor_id = crate::wire_types::VendorId::from_bytes(vid);
        pos += 2;
        let mut gp = [0u8; 12];
        gp.copy_from_slice(&body[pos..pos + 12]);
        let guid_prefix = crate::wire_types::GuidPrefix::from_bytes(gp);
        Ok(Self {
            unused,
            protocol_version,
            vendor_id,
            guid_prefix,
        })
    }
}

// ============================================================================
// InfoTimestamp Submessage (§8.3.8.11) — submessageId 0x09
// ============================================================================

/// InfoTimestamp flag: I (Invalidate). When set: the body is empty and
/// `haveTimestamp` is set to `false` in the ReceiverState.
pub const INFO_TIMESTAMP_FLAG_INVALIDATE: u8 = 0x02;

/// InfoTimestamp submessage (§8.3.8.11). Sets the `timestamp`
/// field + `haveTimestamp` flag in the ReceiverState.
/// Inverted via `INFO_TIMESTAMP_FLAG_INVALIDATE` (I-flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InfoTimestampSubmessage {
    /// Timestamp (8 byte: i32 sec + u32 fraction). Ignored when
    /// `invalidate=true`.
    pub timestamp: crate::header_extension::HeTimestamp,
    /// `true` = the I-flag is set in the submessage → the body is empty
    /// and the receiver sets `haveTimestamp = false`.
    pub invalidate: bool,
}

impl InfoTimestampSubmessage {
    /// Encodes the body. If `invalidate=true`: body empty (0 byte).
    /// Sonst: 8 byte Time_t.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        if self.invalidate {
            flags |= INFO_TIMESTAMP_FLAG_INVALIDATE;
            return (Vec::new(), flags);
        }
        (
            Self::timestamp_body(self.timestamp, little_endian).to_vec(),
            flags,
        )
    }

    /// The eight-octet body of a valid INFO_TS: `Time_t` seconds, then
    /// fraction (DDSI-RTPS 2.5 §9.4.5.9). The writer's hot path puts it
    /// into the message without a heap buffer.
    #[must_use]
    pub fn timestamp_body(
        timestamp: crate::header_extension::HeTimestamp,
        little_endian: bool,
    ) -> [u8; 8] {
        let (s, f) = if little_endian {
            (
                timestamp.seconds.to_le_bytes(),
                timestamp.fraction.to_le_bytes(),
            )
        } else {
            (
                timestamp.seconds.to_be_bytes(),
                timestamp.fraction.to_be_bytes(),
            )
        };
        let mut body = [0u8; 8];
        body[..4].copy_from_slice(&s);
        body[4..].copy_from_slice(&f);
        body
    }

    /// Decodes the body. When `invalidate_flag=true`: expects an empty
    /// body and returns `timestamp = default()`.
    ///
    /// # Errors
    /// `UnexpectedEof` if `invalidate_flag=false` and the body is < 8 byte.
    pub fn read_body(
        body: &[u8],
        little_endian: bool,
        invalidate_flag: bool,
    ) -> Result<Self, WireError> {
        if invalidate_flag {
            return Ok(Self {
                timestamp: crate::header_extension::HeTimestamp::default(),
                invalidate: true,
            });
        }
        if body.len() < 8 {
            return Err(WireError::UnexpectedEof {
                needed: 8,
                offset: 0,
            });
        }
        let mut s = [0u8; 4];
        s.copy_from_slice(&body[0..4]);
        let mut f = [0u8; 4];
        f.copy_from_slice(&body[4..8]);
        let seconds = if little_endian {
            u32::from_le_bytes(s)
        } else {
            u32::from_be_bytes(s)
        };
        let fraction = if little_endian {
            u32::from_le_bytes(f)
        } else {
            u32::from_be_bytes(f)
        };
        Ok(Self {
            timestamp: crate::header_extension::HeTimestamp { seconds, fraction },
            invalidate: false,
        })
    }
}

// ============================================================================
// InfoReply Submessage (§8.3.8.9) — submessageId 0x0F
// ============================================================================

/// InfoReply flag: M (multicast). If set: a second LocatorList
/// (multicastReplyLocatorList) follows in body.
pub const INFO_REPLY_FLAG_MULTICAST: u8 = 0x02;

/// InfoReply submessage (§8.3.8.10.4). Sets `unicastReplyLocatorList`
/// (mandatory) and, if applicable, `multicastReplyLocatorList` (with the
/// M-flag) in the ReceiverState.
///
/// Wire layout (body):
/// - unicastLocatorList: u32 length + N × 24 byte locator
/// - (M-flag) multicastLocatorList: u32 length + N × 24 byte locator
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoReplySubmessage {
    /// Unicast reply locators (at least 1 sensible, empty list allowed).
    pub unicast_locators: Vec<crate::wire_types::Locator>,
    /// Multicast reply locators (`Some` ⇒ M flag set in the header).
    pub multicast_locators: Option<Vec<crate::wire_types::Locator>>,
}

impl InfoReplySubmessage {
    /// Encodes the body. Returns (bytes, flags) incl. E and possibly M bit.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::new();
        Self::write_locator_list(&mut out, &self.unicast_locators, little_endian);
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        if let Some(mcast) = &self.multicast_locators {
            flags |= INFO_REPLY_FLAG_MULTICAST;
            Self::write_locator_list(&mut out, mcast, little_endian);
        }
        (out, flags)
    }

    fn write_locator_list(
        out: &mut Vec<u8>,
        list: &[crate::wire_types::Locator],
        little_endian: bool,
    ) {
        let len = u32::try_from(list.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&if little_endian {
            len.to_le_bytes()
        } else {
            len.to_be_bytes()
        });
        for loc in list {
            // The locator has its own wire format (24 byte). We take the
            // LE path here — the locator is always LE in RTPS in the
            // ParameterList paths; for the submessage body we follow the
            // submessage endianness.
            if little_endian {
                out.extend_from_slice(&loc.to_bytes_le());
            } else {
                // BE variant: kind (4 byte BE), port (4 byte BE), addr (16 byte raw)
                out.extend_from_slice(&(loc.kind.as_i32()).to_be_bytes());
                out.extend_from_slice(&loc.port.to_be_bytes());
                out.extend_from_slice(&loc.address);
            }
        }
    }

    /// Decodes the body. The M-flag is extracted by the caller from the
    /// submessage header.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` (locator length bizarrely large).
    pub fn read_body(
        body: &[u8],
        little_endian: bool,
        multicast_flag: bool,
    ) -> Result<Self, WireError> {
        let mut pos = 0usize;
        let unicast_locators = Self::read_locator_list(body, &mut pos, little_endian)?;
        let multicast_locators = if multicast_flag {
            Some(Self::read_locator_list(body, &mut pos, little_endian)?)
        } else {
            None
        };
        Ok(Self {
            unicast_locators,
            multicast_locators,
        })
    }

    fn read_locator_list(
        body: &[u8],
        pos: &mut usize,
        little_endian: bool,
    ) -> Result<Vec<crate::wire_types::Locator>, WireError> {
        if body.len() < *pos + 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: *pos,
            });
        }
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&body[*pos..*pos + 4]);
        let len = if little_endian {
            u32::from_le_bytes(len_bytes)
        } else {
            u32::from_be_bytes(len_bytes)
        } as usize;
        *pos += 4;
        // §9.4.2.10: "typedef sequence<Locator_t, 8> LocatorList;".
        if len > 8 {
            return Err(WireError::ValueOutOfRange {
                message: "InfoReply.locatorList holds more than 8 locators (DDSI-RTPS 2.5 §9.4.2.10)",
            });
        }
        let remaining = body.len().saturating_sub(*pos);
        if len.saturating_mul(24) > remaining {
            return Err(WireError::ValueOutOfRange {
                message: "InfoReply.locatorList length exceeds body",
            });
        }
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            let mut buf = [0u8; 24];
            buf.copy_from_slice(&body[*pos..*pos + 24]);
            *pos += 24;
            // DDSI-RTPS 2.5 §9.3.2.4: kinds from 3 on are the vendors' and
            // third parties'. A locator of a kind this stack does not know
            // is a transport it does not speak: it is left out of the list,
            // and the locators behind it are read.
            let decoded = if little_endian {
                crate::wire_types::Locator::from_bytes_le(buf)
            } else {
                crate::wire_types::Locator::from_bytes_be(buf)
            };
            match decoded {
                Ok(loc) => out.push(loc),
                // Counted (`skipped_unknown_locators`).
                Err(WireError::InvalidLocatorKind { .. }) => {
                    SKIPPED_UNKNOWN_LOCATORS.fetch_add(1, portable_atomic::Ordering::Relaxed);
                }
                Err(other) => return Err(other),
            }
        }
        Ok(out)
    }
}

// ============================================================================
// InfoReplyIp4 Submessage (PSM §9.4.5.11) — submessageId 0x0D. The IPv4-only
// predecessor of InfoReply, still emitted by some pre-2.5 vendors instead of
// the general form.
// ============================================================================

/// InfoReplyIp4 submessage: the deprecated IPv4-only predecessor of
/// [`InfoReplySubmessage`] (PSM §9.4.5.11). Carries exactly one unicast
/// reply locator and, with the M flag, exactly one multicast reply
/// locator — both as `LocatorUDPv4_t` (4-byte address + 4-byte port; no
/// `LocatorKind` field, it is always UDPv4). Updates the same
/// `unicastReplyLocatorList`/`multicastReplyLocatorList` receiver-state
/// fields InfoReply does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoReplyIp4Submessage {
    /// Unicast reply locator (UDPv4).
    pub unicast_locator: crate::wire_types::Locator,
    /// Multicast reply locator (`Some` ⇒ M flag set).
    pub multicast_locator: Option<crate::wire_types::Locator>,
}

impl InfoReplyIp4Submessage {
    /// One `LocatorUDPv4_t`: 4-byte address + 4-byte port.
    const LOCATOR_UDPV4_WIRE_SIZE: usize = 8;

    /// Encodes the body. Returns (bytes, flags) incl. E and possibly M bit.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::with_capacity(Self::LOCATOR_UDPV4_WIRE_SIZE * 2);
        Self::write_locator_udpv4(&mut out, &self.unicast_locator, little_endian);
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        if let Some(mcast) = &self.multicast_locator {
            flags |= INFO_REPLY_FLAG_MULTICAST;
            Self::write_locator_udpv4(&mut out, mcast, little_endian);
        }
        (out, flags)
    }

    fn write_locator_udpv4(
        out: &mut Vec<u8>,
        loc: &crate::wire_types::Locator,
        little_endian: bool,
    ) {
        // DDSI-RTPS 2.5 §9.4.3.1: `unsigned long address; unsigned long
        // port;` in the CDR encoding of the submessage, so both in its byte
        // order. 192.168.1.10 is the number 0xC0A8010A (the address as
        // `inet_addr` reads it, first octet the most significant).
        let address = u32::from_be_bytes([
            loc.address[12],
            loc.address[13],
            loc.address[14],
            loc.address[15],
        ]);
        if little_endian {
            out.extend_from_slice(&address.to_le_bytes());
            out.extend_from_slice(&loc.port.to_le_bytes());
        } else {
            out.extend_from_slice(&address.to_be_bytes());
            out.extend_from_slice(&loc.port.to_be_bytes());
        }
    }

    /// Decodes the body. The M-flag is extracted by the caller from the
    /// submessage header.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_body(
        body: &[u8],
        little_endian: bool,
        multicast_flag: bool,
    ) -> Result<Self, WireError> {
        let needed = if multicast_flag {
            Self::LOCATOR_UDPV4_WIRE_SIZE * 2
        } else {
            Self::LOCATOR_UDPV4_WIRE_SIZE
        };
        if body.len() < needed {
            return Err(WireError::UnexpectedEof { needed, offset: 0 });
        }
        let unicast_locator = Self::read_locator_udpv4(&body[0..8], little_endian);
        let multicast_locator = if multicast_flag {
            Some(Self::read_locator_udpv4(&body[8..16], little_endian))
        } else {
            None
        };
        Ok(Self {
            unicast_locator,
            multicast_locator,
        })
    }

    fn read_locator_udpv4(chunk: &[u8], little_endian: bool) -> crate::wire_types::Locator {
        let word = |at: usize| {
            let w = [chunk[at], chunk[at + 1], chunk[at + 2], chunk[at + 3]];
            if little_endian {
                u32::from_le_bytes(w)
            } else {
                u32::from_be_bytes(w)
            }
        };
        crate::wire_types::Locator::udp_v4(word(0).to_be_bytes(), word(4))
    }
}

// ============================================================================
// HEARTBEAT_FRAG Submessage (§8.3.8.7)
// ============================================================================

/// HEARTBEAT_FRAG submessage. Sent by the writer to inform the reader
/// that fragments up to `last_fragment_num` are available for
/// `writer_sn`. The writer does not send these; the decoder is kept
/// ready anyway for interop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatFragSubmessage {
    /// Reader EntityId (target).
    pub reader_id: EntityId,
    /// Writer EntityId (source).
    pub writer_id: EntityId,
    /// Associated sample.
    pub writer_sn: SequenceNumber,
    /// Highest available FragmentNumber.
    pub last_fragment_num: FragmentNumber,
    /// Count_t (for correlation with NACK_FRAG).
    pub count: i32,
}

impl HeartbeatFragSubmessage {
    /// Wire-Size: 24 Bytes (4+4+8+4+4).
    pub const WIRE_SIZE: usize = 24;

    /// Encodes the body.
    #[must_use]
    pub fn write_body(self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::with_capacity(Self::WIRE_SIZE);
        out.extend_from_slice(&self.reader_id.to_bytes());
        out.extend_from_slice(&self.writer_id.to_bytes());
        out.extend_from_slice(&if little_endian {
            self.writer_sn.to_bytes_le()
        } else {
            self.writer_sn.to_bytes_be()
        });
        out.extend_from_slice(&if little_endian {
            self.last_fragment_num.to_bytes_le()
        } else {
            self.last_fragment_num.to_bytes_be()
        });
        out.extend_from_slice(&if little_endian {
            self.count.to_le_bytes()
        } else {
            self.count.to_be_bytes()
        });
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        (out, flags)
    }

    /// Decodes the body.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_body(body: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if body.len() < Self::WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: Self::WIRE_SIZE,
                offset: 0,
            });
        }
        let mut pos = 0usize;
        let mut rid = [0u8; 4];
        rid.copy_from_slice(&body[pos..pos + 4]);
        let reader_id = EntityId::from_bytes(rid);
        pos += 4;
        let mut wid = [0u8; 4];
        wid.copy_from_slice(&body[pos..pos + 4]);
        let writer_id = EntityId::from_bytes(wid);
        pos += 4;
        let mut sn = [0u8; 8];
        sn.copy_from_slice(&body[pos..pos + 8]);
        let writer_sn = if little_endian {
            SequenceNumber::from_bytes_le(sn)
        } else {
            SequenceNumber::from_bytes_be(sn)
        };
        pos += 8;
        let mut lf = [0u8; 4];
        lf.copy_from_slice(&body[pos..pos + 4]);
        let last_fragment_num = if little_endian {
            FragmentNumber::from_bytes_le(lf)
        } else {
            FragmentNumber::from_bytes_be(lf)
        };
        pos += 4;
        // §8.3.8.7: invalid when "writerSN.value is zero or negative" or
        // "lastFragmentNum.value is zero or negative".
        if writer_sn.0 < 1 || last_fragment_num.0 < 1 {
            return Err(WireError::ValueOutOfRange {
                message: "HEARTBEAT_FRAG writerSN or lastFragmentNum not positive (DDSI-RTPS 2.5 §8.3.8.7)",
            });
        }
        let mut cnt = [0u8; 4];
        cnt.copy_from_slice(&body[pos..pos + 4]);
        let count = if little_endian {
            i32::from_le_bytes(cnt)
        } else {
            i32::from_be_bytes(cnt)
        };
        Ok(Self {
            reader_id,
            writer_id,
            writer_sn,
            last_fragment_num,
            count,
        })
    }
}

// ============================================================================
// NACK_FRAG Submessage (§8.3.8.12)
// ============================================================================

/// NACK_FRAG submessage. The reader reports missing fragments for a
/// specific `writer_sn`. No flags on the wire except E.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NackFragSubmessage {
    /// Reader EntityId (source).
    pub reader_id: EntityId,
    /// Writer EntityId (target).
    pub writer_id: EntityId,
    /// Associated sample.
    pub writer_sn: SequenceNumber,
    /// Bitset of missing fragments.
    pub fragment_number_state: FragmentNumberSet,
    /// Count_t (for correlation).
    pub count: i32,
}

impl NackFragSubmessage {
    /// Encodes the body.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::new();
        out.extend_from_slice(&self.reader_id.to_bytes());
        out.extend_from_slice(&self.writer_id.to_bytes());
        out.extend_from_slice(&if little_endian {
            self.writer_sn.to_bytes_le()
        } else {
            self.writer_sn.to_bytes_be()
        });
        self.fragment_number_state.write_to(&mut out, little_endian);
        out.extend_from_slice(&if little_endian {
            self.count.to_le_bytes()
        } else {
            self.count.to_be_bytes()
        });
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        (out, flags)
    }

    /// Decodes the body.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_body(body: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if body.len() < 4 + 4 + 8 + 4 + 4 + 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4 + 4 + 8 + 4 + 4 + 4,
                offset: 0,
            });
        }
        let mut pos = 0usize;
        let mut rid = [0u8; 4];
        rid.copy_from_slice(&body[pos..pos + 4]);
        let reader_id = EntityId::from_bytes(rid);
        pos += 4;
        let mut wid = [0u8; 4];
        wid.copy_from_slice(&body[pos..pos + 4]);
        let writer_id = EntityId::from_bytes(wid);
        pos += 4;
        let mut sn = [0u8; 8];
        sn.copy_from_slice(&body[pos..pos + 8]);
        let writer_sn = if little_endian {
            SequenceNumber::from_bytes_le(sn)
        } else {
            SequenceNumber::from_bytes_be(sn)
        };
        pos += 8;
        // §8.3.8.12: invalid when "writerSN.value is zero or negative".
        if writer_sn.0 < 1 {
            return Err(WireError::ValueOutOfRange {
                message: "NACK_FRAG writerSN zero or negative (DDSI-RTPS 2.5 §8.3.8.12)",
            });
        }
        let (fragment_number_state, new_pos) =
            FragmentNumberSet::read_from(body, pos, little_endian)?;
        pos = new_pos;
        if body.len() < pos + 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: pos,
            });
        }
        let mut cnt = [0u8; 4];
        cnt.copy_from_slice(&body[pos..pos + 4]);
        let count = if little_endian {
            i32::from_le_bytes(cnt)
        } else {
            i32::from_be_bytes(cnt)
        };
        Ok(Self {
            reader_id,
            writer_id,
            writer_sn,
            fragment_number_state,
            count,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;

    fn writer_id() -> EntityId {
        EntityId::user_writer_with_key([0x10, 0x20, 0x30])
    }
    fn reader_id() -> EntityId {
        EntityId::user_reader_with_key([0x40, 0x50, 0x60])
    }

    // ---- SequenceNumberSet ----

    /// Missing samples spread over more than 256 sequence numbers give a
    /// set of 256 bits that our own decoder reads back; the ones beyond the
    /// window wait for the next ACKNACK. A set of 1168 bits was refused by
    /// every receiver and stopped the repair.
    #[test]
    fn snset_from_missing_spread_wide_stays_within_256_bits() {
        let base = SequenceNumber(24_604);
        let missing = [
            SequenceNumber(24_604),
            SequenceNumber(24_700),
            SequenceNumber(24_859),
            SequenceNumber(24_860),
            SequenceNumber(25_771),
        ];
        let set = SequenceNumberSet::from_missing(base, &missing);
        assert_eq!(set.num_bits, RTPS_BITMAP_MAX_BITS);
        let wanted: Vec<SequenceNumber> = set.iter_set().collect();
        assert_eq!(
            wanted,
            vec![
                SequenceNumber(24_604),
                SequenceNumber(24_700),
                SequenceNumber(24_859)
            ]
        );
        let mut bytes = Vec::new();
        set.write_to(&mut bytes, true);
        let (read, _) =
            SequenceNumberSet::read_from(&bytes, 0, true).expect("a set the decoder reads");
        assert_eq!(read, set);
    }

    #[test]
    fn snset_wire_size_zero_bits_is_12_bytes() {
        assert_eq!(SequenceNumberSet::wire_size(0), 12);
    }

    #[test]
    fn snset_wire_size_32_bits_is_16_bytes() {
        assert_eq!(SequenceNumberSet::wire_size(32), 16);
    }

    #[test]
    fn snset_wire_size_33_bits_is_20_bytes() {
        assert_eq!(SequenceNumberSet::wire_size(33), 20);
    }

    #[test]
    fn snset_roundtrip_le() {
        let s = SequenceNumberSet {
            bitmap_base: SequenceNumber(100),
            num_bits: 5,
            bitmap: vec![0b0000_1010_0000_0000_0000_0000_0000_0000],
        };
        let mut buf = Vec::new();
        s.write_to(&mut buf, true);
        let (decoded, end) = SequenceNumberSet::read_from(&buf, 0, true).unwrap();
        assert_eq!(decoded, s);
        assert_eq!(end, buf.len());
    }

    #[test]
    fn snset_roundtrip_be() {
        let s = SequenceNumberSet {
            bitmap_base: SequenceNumber(0xDEAD_BEEF),
            num_bits: 64,
            bitmap: vec![0x1234_5678, 0x9ABC_DEF0],
        };
        let mut buf = Vec::new();
        s.write_to(&mut buf, false);
        let (decoded, _) = SequenceNumberSet::read_from(&buf, 0, false).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn snset_decode_rejects_truncated_bitmap() {
        // numBits=64 → 8 byte bitmap expected; only 4 present.
        let mut buf = Vec::new();
        buf.extend_from_slice(&SequenceNumber(1).to_bytes_le());
        buf.extend_from_slice(&64_u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 4]); // only 4 instead of 8
        let res = SequenceNumberSet::read_from(&buf, 0, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- DATA Submessage ----

    #[test]
    fn data_submessage_roundtrip_le() {
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(42),
            inline_qos: None,
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from([1u8, 2, 3, 4, 5].as_slice()),
        };
        let (bytes, flags) = d.write_body(true);
        assert!(flags & FLAG_E_LITTLE_ENDIAN != 0);
        assert!(flags & DATA_FLAG_DATA != 0);
        let decoded = DataSubmessage::read_body_with_flags(&bytes, true, flags).unwrap();
        assert_eq!(decoded, d);
    }

    /// A PID_IGNORE in the inline QoS is skipped by the parser and its
    /// octets are still the inline QoS's (DDSI-RTPS 2.5 §9.6.2.2.1): the
    /// serializedPayload starts behind the sentinel, not where the parsed
    /// list, encoded again without the ignored parameter, would end.
    #[test]
    fn a_pid_ignore_in_the_inline_qos_does_not_move_the_payload() {
        let payload = [0x00u8, 0x01, 0x00, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];
        let mut body = alloc::vec![0x00, 0x00, 16, 0x00];
        body.extend_from_slice(&reader_id().to_bytes());
        body.extend_from_slice(&writer_id().to_bytes());
        body.extend_from_slice(&SequenceNumber(7).to_bytes_le());
        // PID_IGNORE with four octets, PID_KEY_HASH, PID_SENTINEL.
        body.extend_from_slice(&crate::parameter_list::pid::IGNORE.to_le_bytes());
        body.extend_from_slice(&4u16.to_le_bytes());
        body.extend_from_slice(&[0xEE; 4]);
        body.extend_from_slice(&crate::parameter_list::pid::KEY_HASH.to_le_bytes());
        body.extend_from_slice(&16u16.to_le_bytes());
        body.extend_from_slice(&[0x11; 16]);
        body.extend_from_slice(&crate::parameter_list::pid::SENTINEL.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&payload);
        let flags = FLAG_E_LITTLE_ENDIAN | DATA_FLAG_DATA | DATA_FLAG_INLINE_QOS;
        let decoded = DataSubmessage::read_body_with_flags(&body, true, flags).unwrap();
        assert_eq!(&decoded.serialized_payload[..], &payload[..]);
        let qos = decoded.inline_qos.expect("inline QoS");
        assert_eq!(qos.parameters.len(), 1, "PID_IGNORE is not kept");
        assert_eq!(qos.parameters[0].id, crate::parameter_list::pid::KEY_HASH);
    }

    #[test]
    fn data_submessage_roundtrip_be_with_empty_payload() {
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(0xDEAD_BEEF),
            inline_qos: None,
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from([].as_slice()),
        };
        let (bytes, flags) = d.write_body(false);
        assert_eq!(flags & FLAG_E_LITTLE_ENDIAN, 0);
        let decoded = DataSubmessage::read_body_with_flags(&bytes, false, flags).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_submessage_key_flag_roundtrip() {
        // Spec §8.3.8.2 K-flag: serialized_payload contains only the key.
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(7),
            inline_qos: None,
            // D and K are mutually exclusive (§9.4.5.3): a
            // DATA carrying only the key clears D.
            data_flag: false,
            key_flag: true,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from([0xAA, 0xBB].as_slice()),
        };
        let (bytes, flags) = d.write_body(true);
        assert!(flags & DATA_FLAG_KEY != 0, "K-Flag must be set");
        assert_eq!(flags & DATA_FLAG_DATA, 0, "D-Flag must be clear");
        let decoded = DataSubmessage::read_body_with_flags(&bytes, true, flags).unwrap();
        assert!(decoded.key_flag);
        assert!(!decoded.non_standard_flag);
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_submessage_non_standard_flag_roundtrip() {
        // Spec §8.3.8.2 N-Flag: NonStandardPayload (e.g. Encrypted).
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(8),
            inline_qos: None,
            data_flag: true,
            key_flag: false,
            non_standard_flag: true,
            serialized_payload: crate::payload::Payload::from([0xCC, 0xDD].as_slice()),
        };
        let (bytes, flags) = d.write_body(true);
        assert!(flags & DATA_FLAG_NON_STANDARD != 0, "N-Flag must be set");
        let decoded = DataSubmessage::read_body_with_flags(&bytes, true, flags).unwrap();
        assert!(!decoded.key_flag);
        assert!(decoded.non_standard_flag);
        assert_eq!(decoded, d);
    }

    /// E, Q and N set, with D (a value) and with K (a key) in turn: the
    /// valid combinations (§9.4.5.3: "D=1 and K=1 is an invalid combination
    /// in this version of the protocol", see
    /// `validity_tests::data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid`).
    #[test]
    fn data_submessage_all_valid_flags_combined_roundtrip() {
        for (data_flag, key_flag) in [(true, false), (false, true)] {
            let mut pl = crate::parameter_list::ParameterList::new();
            pl.push(crate::parameter_list::Parameter::new(0x0070, vec![1; 4]));
            let d = DataSubmessage {
                extra_flags: 0xABCD,
                reader_id: reader_id(),
                writer_id: writer_id(),
                writer_sn: SequenceNumber(9),
                inline_qos: Some(pl),
                data_flag,
                key_flag,
                non_standard_flag: true,
                serialized_payload: crate::payload::Payload::from([0xEE; 8].as_slice()),
            };
            let (bytes, flags) = d.write_body(true);
            assert!(flags & FLAG_E_LITTLE_ENDIAN != 0);
            assert!(flags & DATA_FLAG_INLINE_QOS != 0);
            assert_eq!(flags & DATA_FLAG_DATA != 0, data_flag);
            assert_eq!(flags & DATA_FLAG_KEY != 0, key_flag);
            assert!(flags & DATA_FLAG_NON_STANDARD != 0);
            let decoded = DataSubmessage::read_body_with_flags(&bytes, true, flags).unwrap();
            assert_eq!(decoded, d);
        }
    }

    #[test]
    fn data_submessage_octets_to_inline_qos_is_16() {
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from([].as_slice()),
        };
        let (bytes, _) = d.write_body(true);
        // bytes[2..4] = octetsToInlineQos LE
        assert_eq!(&bytes[2..4], &[16, 0]);
    }

    #[test]
    fn data_submessage_decode_rejects_truncated() {
        let res = DataSubmessage::read_body(&[1, 2, 3], true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- HEARTBEAT Submessage ----

    #[test]
    fn heartbeat_submessage_roundtrip_le() {
        let h = HeartbeatSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(10),
            count: 7,
            final_flag: true,
            liveliness_flag: false,
            group_info: None,
        };
        let (bytes, flags) = h.write_body(true);
        assert!(flags & HEARTBEAT_FLAG_FINAL != 0);
        assert_eq!(flags & HEARTBEAT_FLAG_LIVELINESS, 0);
        assert_eq!(bytes.len(), HeartbeatSubmessage::WIRE_SIZE);
        let decoded = HeartbeatSubmessage::read_body(&bytes, true, true, false, false).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn heartbeat_submessage_no_final_flag_when_disabled() {
        let h = HeartbeatSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(1),
            count: 0,
            final_flag: false,
            liveliness_flag: false,
            group_info: None,
        };
        let (_, flags) = h.write_body(true);
        assert_eq!(flags & HEARTBEAT_FLAG_FINAL, 0);
    }

    #[test]
    fn heartbeat_submessage_liveliness_flag_roundtrip() {
        let h = HeartbeatSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(1),
            count: 0,
            final_flag: false,
            liveliness_flag: true,
            group_info: None,
        };
        let (bytes, flags) = h.write_body(true);
        assert!(flags & HEARTBEAT_FLAG_LIVELINESS != 0);
        let decoded = HeartbeatSubmessage::read_body(&bytes, true, false, true, false).unwrap();
        assert_eq!(decoded, h);
        assert!(decoded.liveliness_flag);
    }

    #[test]
    fn heartbeat_decode_rejects_truncated() {
        let res = HeartbeatSubmessage::read_body(&[0u8; 27], true, false, false, false);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- WP 1.E stage D: HEARTBEAT GroupInfo ----

    fn heartbeat_with(group_info: HeartbeatGroupInfo) -> HeartbeatSubmessage {
        HeartbeatSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(5),
            count: 3,
            final_flag: false,
            liveliness_flag: false,
            group_info: Some(group_info),
        }
    }

    fn gsns(current: i64, first: i64, last: i64) -> HeartbeatGroupInfo {
        HeartbeatGroupInfo {
            current_gsn: SequenceNumber(current),
            first_gsn: SequenceNumber(first),
            last_gsn: SequenceNumber(last),
            writer_set: None,
            secure_writer_set: None,
        }
    }

    #[test]
    fn heartbeat_with_group_info_roundtrip_le() {
        let h = heartbeat_with(gsns(100, 50, 99));
        let (bytes, flags) = h.write_body(true);
        assert!(flags & HEARTBEAT_FLAG_GROUP_INFO != 0);
        let decoded = HeartbeatSubmessage::read_body(&bytes, true, false, false, true).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn the_group_info_is_the_three_sequence_numbers_of_9_4_5_7() {
        // §9.4.5.7: after `count`, currentGSN, firstGSN and lastGSN, each
        // a SequenceNumber (high long, low unsigned long), nothing else.
        let (bytes, _) = heartbeat_with(gsns(7, 1, 7)).write_body(false);
        assert_eq!(bytes.len(), HeartbeatSubmessage::WIRE_SIZE + 24);
        assert_eq!(
            &bytes[HeartbeatSubmessage::WIRE_SIZE..],
            &[
                0, 0, 0, 0, 0, 0, 0, 7, // currentGSN
                0, 0, 0, 0, 0, 0, 0, 1, // firstGSN
                0, 0, 0, 0, 0, 0, 0, 7, // lastGSN
            ]
        );
    }

    /// Table 8.44 and §9.4.2.19: `writerSet` and `secureWriterSet` are two
    /// `GroupDigest`s of four octets each, after lastGSN; a reader reads them
    /// where the submessage leaves room for both and accepts their absence (the
    /// figure of §9.4.5.7).
    #[test]
    fn the_group_digests_follow_last_gsn_and_are_optional_on_read() {
        use crate::group_digest::GroupDigest;
        let mut with = gsns(7, 1, 7);
        with.writer_set = Some(GroupDigest([1, 2, 3, 4]));
        with.secure_writer_set = Some(GroupDigest([5, 6, 7, 8]));
        for little_endian in [true, false] {
            let (bytes, flags) = heartbeat_with(with).write_body(little_endian);
            assert!(flags & HEARTBEAT_FLAG_GROUP_INFO != 0);
            assert_eq!(
                bytes.len(),
                HeartbeatSubmessage::WIRE_SIZE
                    + HeartbeatGroupInfo::WIRE_SIZE
                    + HeartbeatGroupInfo::DIGESTS_WIRE_SIZE
            );
            assert_eq!(
                &bytes[bytes.len() - 8..],
                &[1, 2, 3, 4, 5, 6, 7, 8],
                "two GroupDigests after lastGSN, no byte order"
            );
            let back =
                HeartbeatSubmessage::read_body(&bytes, little_endian, false, false, true).unwrap();
            assert_eq!(back.group_info, Some(with));
            // The figure of §9.4.5.7: the three numbers and nothing after.
            let short = &bytes[..bytes.len() - 8];
            let back =
                HeartbeatSubmessage::read_body(short, little_endian, false, false, true).unwrap();
            let gi = back.group_info.unwrap();
            assert_eq!((gi.writer_set, gi.secure_writer_set), (None, None));
            assert_eq!(gi.current_gsn.0, 7);
            // Octets after lastGSN that do not make two GroupDigests are skipped.
            let odd = &bytes[..bytes.len() - 4];
            let back =
                HeartbeatSubmessage::read_body(odd, little_endian, false, false, true).unwrap();
            assert_eq!(back.group_info.unwrap().writer_set, None);
        }
        // A sender without the sets writes the 24 octets only.
        let (bytes, _) = heartbeat_with(gsns(7, 1, 7)).write_body(true);
        assert_eq!(
            bytes.len(),
            HeartbeatSubmessage::WIRE_SIZE + HeartbeatGroupInfo::WIRE_SIZE
        );
    }

    #[test]
    fn a_current_gsn_past_the_writers_last_is_accepted() {
        // Another writer of the group wrote GSN 9 after this writer's
        // last sample (GSN 7). §8.3.8.6's "currentGSN.value >
        // lastGSN.value" rule would reject it; the definition of
        // currentGSN makes it the normal case.
        let h = heartbeat_with(gsns(9, 1, 7));
        let (bytes, _) = h.write_body(true);
        assert_eq!(
            HeartbeatSubmessage::read_body(&bytes, true, false, false, true).unwrap(),
            h
        );
    }

    #[test]
    fn heartbeat_decode_rejects_invalid_group_sequence_numbers() {
        // §8.3.8.6 Validity.
        for (current, first, last) in [
            (0, 1, 1),  // currentGSN zero
            (5, 0, 4),  // firstGSN zero
            (5, 1, -1), // lastGSN negative
            (5, 4, 2),  // lastGSN < firstGSN - 1
            (3, 4, 6),  // currentGSN < firstGSN
        ] {
            let (bytes, _) = heartbeat_with(gsns(current, first, last)).write_body(true);
            assert!(
                matches!(
                    HeartbeatSubmessage::read_body(&bytes, true, false, false, true),
                    Err(WireError::ValueOutOfRange { .. })
                ),
                "({current}, {first}, {last})"
            );
        }
    }

    #[test]
    fn heartbeat_decode_rejects_truncated_group_info() {
        // body ends before the 3 GSN fields
        let mut body = Vec::new();
        body.extend_from_slice(&reader_id().to_bytes());
        body.extend_from_slice(&writer_id().to_bytes());
        body.extend_from_slice(&SequenceNumber(1).to_bytes_le());
        body.extend_from_slice(&SequenceNumber(1).to_bytes_le());
        body.extend_from_slice(&1i32.to_le_bytes());
        // GroupInfo trailer missing → UnexpectedEof
        let res = HeartbeatSubmessage::read_body(&body, true, false, false, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- ACKNACK Submessage ----

    #[test]
    fn acknack_submessage_roundtrip_le() {
        let a = AckNackSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            reader_sn_state: SequenceNumberSet {
                bitmap_base: SequenceNumber(5),
                num_bits: 3,
                bitmap: vec![0b1010_0000_0000_0000_0000_0000_0000_0000],
            },
            count: 1,
            final_flag: false,
        };
        let (bytes, flags) = a.write_body(true);
        assert_eq!(flags & ACKNACK_FLAG_FINAL, 0);
        let decoded = AckNackSubmessage::read_body(&bytes, true, false).unwrap();
        assert_eq!(decoded, a);
    }

    #[test]
    fn acknack_submessage_with_final_flag() {
        let a = AckNackSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            reader_sn_state: SequenceNumberSet {
                bitmap_base: SequenceNumber(1),
                num_bits: 0,
                bitmap: vec![],
            },
            count: 0,
            final_flag: true,
        };
        let (bytes, flags) = a.write_body(true);
        assert!(flags & ACKNACK_FLAG_FINAL != 0);
        let decoded = AckNackSubmessage::read_body(&bytes, true, true).unwrap();
        assert!(decoded.final_flag);
    }

    // ---- GAP Submessage ----

    #[test]
    fn gap_submessage_roundtrip_le() {
        let g = GapSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            gap_start: SequenceNumber(1),
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(5),
                num_bits: 8,
                bitmap: vec![0xFF000000],
            },
            group_info: None,
            filtered_count: None,
        };
        let (bytes, flags) = g.write_body(true);
        assert!(flags & FLAG_E_LITTLE_ENDIAN != 0);
        assert_eq!(flags & GAP_FLAG_GROUP_INFO, 0);
        assert_eq!(flags & GAP_FLAG_FILTERED_COUNT, 0);
        let decoded = GapSubmessage::read_body(&bytes, true, false, false).unwrap();
        assert_eq!(decoded, g);
    }

    #[test]
    fn the_gap_flags_are_those_of_9_4_5_6() {
        // DDSI-RTPS 2.5 §9.4.5.6. G stood at 0x04 and F at 0x08: a
        // spec sender's GAP with a filteredCount read as one with group
        // information, sixteen octets long, and was dropped.
        assert_eq!(GAP_FLAG_GROUP_INFO, 0x02);
        assert_eq!(GAP_FLAG_FILTERED_COUNT, 0x04);
    }

    #[test]
    fn gap_decode_rejects_truncated_header() {
        let res = GapSubmessage::read_body(&[0u8; 10], true, false, false);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- WP 1.E stage C: GAP filteredCount + GroupInfo ----

    #[test]
    fn gap_with_filtered_count_roundtrip_le() {
        let g = GapSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            gap_start: SequenceNumber(1),
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(2),
                num_bits: 0,
                bitmap: vec![],
            },
            group_info: None,
            filtered_count: Some(3),
        };
        let (bytes, flags) = g.write_body(true);
        assert!(flags & GAP_FLAG_FILTERED_COUNT != 0);
        let decoded = GapSubmessage::read_body(&bytes, true, false, true).unwrap();
        assert_eq!(decoded, g);
        assert_eq!(decoded.filtered_count, Some(3));
    }

    /// DDSI-RTPS 2.5 §9.4.2.14: `filteredCount` is a `ChangeCount_t`,
    /// `long high` then `unsigned long low`, eight octets in the
    /// submessage's byte order; a negative `high` is no count.
    #[test]
    fn gap_filtered_count_is_eight_octets_high_then_low() {
        let g = GapSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            gap_start: SequenceNumber(1),
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(2),
                num_bits: 0,
                bitmap: vec![],
            },
            group_info: None,
            filtered_count: Some((1 << 32) | 2),
        };
        let plain = GapSubmessage {
            filtered_count: None,
            ..g.clone()
        };
        let (le, _) = g.write_body(true);
        let (be, _) = g.write_body(false);
        let base = plain.write_body(true).0.len();
        assert_eq!(le.len(), base + 8);
        assert_eq!(&le[base..], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&be[base..], &[0, 0, 0, 1, 0, 0, 0, 2]);
        assert_eq!(
            GapSubmessage::read_body(&be, false, false, true).unwrap(),
            g
        );
        let mut negative = le.clone();
        negative[base + 3] = 0x80;
        assert!(GapSubmessage::read_body(&negative, true, false, true).is_err());
        assert!(GapSubmessage::read_body(&le[..base + 4], true, false, true).is_err());
    }

    #[test]
    fn gap_with_group_info_roundtrip_be() {
        let g = GapSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            gap_start: SequenceNumber(10),
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(11),
                num_bits: 0,
                bitmap: vec![],
            },
            group_info: Some(GapGroupInfo {
                gap_start_gsn: SequenceNumber(100),
                gap_end_gsn: SequenceNumber(110),
            }),
            filtered_count: None,
        };
        let (bytes, flags) = g.write_body(false);
        assert!(flags & GAP_FLAG_GROUP_INFO != 0);
        let decoded = GapSubmessage::read_body(&bytes, false, true, false).unwrap();
        assert_eq!(decoded, g);
    }

    #[test]
    fn gap_decode_rejects_invalid_group_sequence_numbers() {
        // §8.3.8.4 Validity.
        for (start, end) in [(0, 5), (5, 0), (5, 3)] {
            let g = GapSubmessage {
                reader_id: reader_id(),
                writer_id: writer_id(),
                gap_start: SequenceNumber(10),
                gap_list: SequenceNumberSet {
                    bitmap_base: SequenceNumber(11),
                    num_bits: 0,
                    bitmap: vec![],
                },
                group_info: Some(GapGroupInfo {
                    gap_start_gsn: SequenceNumber(start),
                    gap_end_gsn: SequenceNumber(end),
                }),
                filtered_count: None,
            };
            let (bytes, _) = g.write_body(true);
            assert!(
                matches!(
                    GapSubmessage::read_body(&bytes, true, true, false),
                    Err(WireError::ValueOutOfRange { .. })
                ),
                "({start}, {end})"
            );
        }
    }

    #[test]
    fn a_gap_end_gsn_one_below_its_start_is_an_empty_range_and_valid() {
        let g = GapGroupInfo {
            gap_start_gsn: SequenceNumber(5),
            gap_end_gsn: SequenceNumber(4),
        };
        assert!(g.is_valid());
    }

    #[test]
    fn gap_with_group_info_and_filtered_count_combined() {
        let g = GapSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            gap_start: SequenceNumber(5),
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(6),
                num_bits: 0,
                bitmap: vec![],
            },
            group_info: Some(GapGroupInfo {
                gap_start_gsn: SequenceNumber(50),
                gap_end_gsn: SequenceNumber(55),
            }),
            filtered_count: Some(7),
        };
        let (bytes, flags) = g.write_body(true);
        assert!(flags & GAP_FLAG_GROUP_INFO != 0);
        assert!(flags & GAP_FLAG_FILTERED_COUNT != 0);
        let decoded = GapSubmessage::read_body(&bytes, true, true, true).unwrap();
        assert_eq!(decoded, g);
    }

    #[test]
    fn gap_filtered_count_zero_is_distinct_from_none() {
        // filtered_count=Some(0) means "K flag set, but 0 filtered"
        // (= everything really removed). None = trailer completely missing.
        // Both must round-trip.
        let zero = GapSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            gap_start: SequenceNumber(1),
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(2),
                num_bits: 0,
                bitmap: vec![],
            },
            group_info: None,
            filtered_count: Some(0),
        };
        let (bytes, flags) = zero.write_body(true);
        assert!(flags & GAP_FLAG_FILTERED_COUNT != 0);
        let decoded = GapSubmessage::read_body(&bytes, true, false, true).unwrap();
        assert_eq!(decoded.filtered_count, Some(0));
    }

    #[test]
    fn gap_decode_rejects_truncated_filtered_count() {
        // body ends before filtered_count → UnexpectedEof
        let g = GapSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            gap_start: SequenceNumber(1),
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(2),
                num_bits: 0,
                bitmap: vec![],
            },
            group_info: None,
            filtered_count: None,
        };
        let (bytes, _) = g.write_body(true);
        // Decoder with filtered_count_flag=true expects a 4-byte trailer
        let res = GapSubmessage::read_body(&bytes, true, false, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn gap_decode_rejects_truncated_group_info() {
        let g = GapSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            gap_start: SequenceNumber(1),
            gap_list: SequenceNumberSet {
                bitmap_base: SequenceNumber(2),
                num_bits: 0,
                bitmap: vec![],
            },
            group_info: None,
            filtered_count: None,
        };
        let (bytes, _) = g.write_body(true);
        let res = GapSubmessage::read_body(&bytes, true, true, false);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- FragmentNumberSet ----

    #[test]
    fn fnset_wire_size_formula() {
        assert_eq!(FragmentNumberSet::wire_size(0), 8);
        assert_eq!(FragmentNumberSet::wire_size(1), 12);
        assert_eq!(FragmentNumberSet::wire_size(32), 12);
        assert_eq!(FragmentNumberSet::wire_size(33), 16);
    }

    #[test]
    fn fnset_from_missing_single() {
        let s = FragmentNumberSet::from_missing(
            FragmentNumber(1),
            &[FragmentNumber(1), FragmentNumber(3)],
        );
        assert_eq!(s.bitmap_base, FragmentNumber(1));
        assert_eq!(s.num_bits, 3);
        let set: Vec<_> = s.iter_set().collect();
        assert_eq!(set, vec![FragmentNumber(1), FragmentNumber(3)]);
    }

    #[test]
    fn fnset_from_missing_empty() {
        let s = FragmentNumberSet::from_missing(FragmentNumber(5), &[]);
        assert_eq!(s.num_bits, 0);
        assert!(s.iter_set().next().is_none());
    }

    #[test]
    fn fnset_from_missing_caps_num_bits_at_256() {
        // Regression M-4 / DDSI-RTPS §8.3.5.4: numBits MUST be <= 256. A
        // gap over > 256 fragments (e.g. fragment 1 AND 300 missing at
        // ~781 fragments under packet loss) must not build the set with
        // num_bits=300 — a spec-conformant receiver discards that as
        // malformed → the NACK_FRAG is lost → fragments are never resent
        // → sample stall. The set covers only the first 256; the rest
        // follows in the next NACK_FRAG once bitmap_base has advanced.
        let missing = [FragmentNumber(1), FragmentNumber(300)];
        let s = FragmentNumberSet::from_missing(FragmentNumber(1), &missing);
        assert!(
            s.num_bits <= 256,
            "num_bits {} > 256 (malformed)",
            s.num_bits
        );
        assert_eq!(s.bitmap_base, FragmentNumber(1));
        // Fragment 1 (within the 256 window) is set, 300 (outside) is
        // not — it is re-requested in a follow-up NACK_FRAG.
        let set: Vec<_> = s.iter_set().collect();
        assert!(set.contains(&FragmentNumber(1)));
        assert!(!set.contains(&FragmentNumber(300)));
    }

    #[test]
    fn fnset_missing_below_base_is_ignored() {
        let s = FragmentNumberSet::from_missing(
            FragmentNumber(10),
            &[FragmentNumber(5), FragmentNumber(11)],
        );
        assert_eq!(s.bitmap_base, FragmentNumber(10));
        let set: Vec<_> = s.iter_set().collect();
        assert_eq!(set, vec![FragmentNumber(11)]);
    }

    #[test]
    fn fnset_roundtrip_le() {
        let s = FragmentNumberSet {
            bitmap_base: FragmentNumber(100),
            num_bits: 35,
            bitmap: vec![0xDEAD_BEEF, 0xC000_0000],
        };
        let mut buf = Vec::new();
        s.write_to(&mut buf, true);
        assert_eq!(buf.len(), s.encoded_size());
        let (decoded, end) = FragmentNumberSet::read_from(&buf, 0, true).unwrap();
        assert_eq!(decoded, s);
        assert_eq!(end, buf.len());
    }

    #[test]
    fn fnset_roundtrip_be() {
        let s = FragmentNumberSet {
            bitmap_base: FragmentNumber(1),
            num_bits: 8,
            bitmap: vec![0xFF00_0000],
        };
        let mut buf = Vec::new();
        s.write_to(&mut buf, false);
        let (decoded, _) = FragmentNumberSet::read_from(&buf, 0, false).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn fnset_decode_rejects_truncated() {
        let buf = [0u8; 4];
        let res = FragmentNumberSet::read_from(&buf, 0, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- DATA_FRAG Submessage ----

    fn dataflag_frag(
        writer_sn: i64,
        starting: u32,
        count: u16,
        frag_size: u16,
        sample_size: u32,
        payload: Vec<u8>,
    ) -> DataFragSubmessage {
        DataFragSubmessage {
            inline_qos: None,
            extra_flags: 0,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(writer_sn),
            fragment_starting_num: FragmentNumber(starting),
            fragments_in_submessage: count,
            fragment_size: frag_size,
            sample_size,
            serialized_payload: Arc::from(payload),
            inline_qos_flag: false,
            key_flag: false,
            non_standard_flag: false,
        }
    }

    #[test]
    fn data_frag_roundtrip_le() {
        let d = dataflag_frag(1, 1, 1, 4, 12, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let (bytes, flags) = d.write_body(true);
        assert!(flags & FLAG_E_LITTLE_ENDIAN != 0);
        assert_eq!(bytes.len(), DataFragSubmessage::HEADER_WIRE_SIZE + 4);
        let decoded = DataFragSubmessage::read_body(&bytes, true, false, false, false).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_frag_roundtrip_be() {
        let d = dataflag_frag(7, 2, 1, 8, 16, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let (bytes, flags) = d.write_body(false);
        assert_eq!(flags & FLAG_E_LITTLE_ENDIAN, 0);
        let decoded = DataFragSubmessage::read_body(&bytes, false, false, false, false).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_frag_last_fragment_shorter_than_fragment_size() {
        // sample_size=10, fragment_size=4, fragment 3 carries only 2 bytes
        let d = dataflag_frag(1, 3, 1, 4, 10, vec![0xAA, 0xBB]);
        let (bytes, _) = d.write_body(true);
        let decoded = DataFragSubmessage::read_body(&bytes, true, false, false, false).unwrap();
        assert_eq!(decoded.serialized_payload.as_ref(), &[0xAA, 0xBB][..]);
        assert_eq!(decoded.sample_size, 10);
        assert_eq!(decoded.fragment_size, 4);
    }

    #[test]
    fn data_frag_decode_rejects_truncated() {
        let res = DataFragSubmessage::read_body(&[0u8; 20], true, false, false, false);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn data_frag_decode_accepts_nonzero_extra_flags_silently() {
        // B3 research: Cyclone/Fast-DDS ignore non-zero extra_flags.
        // So do we.
        let d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        let (mut bytes, _) = d.write_body(true);
        bytes[0..2].copy_from_slice(&0x0042u16.to_le_bytes()); // extra_flags nonzero
        let decoded = DataFragSubmessage::read_body(&bytes, true, false, false, false).unwrap();
        assert_eq!(decoded.extra_flags, 0x0042);
    }

    #[test]
    fn seqnumset_rejects_num_bits_above_256() {
        // B7: hard cap against DoS via a huge bitmap.
        let mut buf = Vec::new();
        buf.extend_from_slice(&SequenceNumber(1).to_bytes_le());
        buf.extend_from_slice(&257u32.to_le_bytes()); // num_bits > 256
        let res = SequenceNumberSet::read_from(&buf, 0, true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn seqnumset_accepts_exactly_256_bits() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&SequenceNumber(1).to_bytes_le());
        buf.extend_from_slice(&256u32.to_le_bytes());
        // 256 bits = 8 words = 32 byte bitmap
        buf.extend_from_slice(&[0u8; 32]);
        let res = SequenceNumberSet::read_from(&buf, 0, true);
        assert!(res.is_ok());
    }

    #[test]
    fn fnset_rejects_num_bits_above_256() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&FragmentNumber(1).to_bytes_le());
        buf.extend_from_slice(&1000u32.to_le_bytes()); // num_bits far above 256
        let res = FragmentNumberSet::read_from(&buf, 0, true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn fnset_dos_giant_num_bits_rejected_before_alloc() {
        // Pathological: num_bits = u32::MAX would allocate ~512 MB if we
        // do not cap beforehand.
        let mut buf = Vec::new();
        buf.extend_from_slice(&FragmentNumber(1).to_bytes_le());
        buf.extend_from_slice(&u32::MAX.to_le_bytes());
        let res = FragmentNumberSet::read_from(&buf, 0, true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn data_frag_decode_rejects_wrong_octets_to_inline_qos_when_q_false() {
        // We craft a DATA_FRAG body with a wrong octetsToInlineQos=99 and
        // Q=false. The decoder must reject it.
        let d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        let (mut bytes, _) = d.write_body(true);
        // octetsToInlineQos sits in bytes [2..4] (after extra_flags).
        bytes[2..4].copy_from_slice(&99u16.to_le_bytes());
        let res = DataFragSubmessage::read_body(&bytes, true, false, false, false);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    /// DDSI-RTPS 2.5 §9.4.5.4: header elements a later version adds in
    /// front of the inline QoS are skipped by octetsToInlineQos — four extra
    /// octets in a DATA (16 + 4) and a DATA_FRAG (28 + 4).
    #[test]
    fn octets_to_inline_qos_skips_header_elements_of_a_later_version() {
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: EntityId::user_writer_with_key([4, 5, 6]),
            writer_sn: SequenceNumber(7),
            inline_qos: None,
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: crate::payload::Payload::from(vec![0xAA, 0xBB, 0xCC, 0xDD]),
        };
        let (bytes, flags) = d.write_body(true);
        let mut later = bytes[..20].to_vec();
        later[2..4].copy_from_slice(&20u16.to_le_bytes());
        later.extend_from_slice(&[0xEE; 4]);
        later.extend_from_slice(&bytes[20..]);
        let read = DataSubmessage::read_body_with_flags(&later, true, flags).unwrap();
        assert_eq!(read.writer_sn, SequenceNumber(7));
        assert_eq!(&read.serialized_payload[..], &[0xAA, 0xBB, 0xCC, 0xDD]);
        let mut short = bytes.clone();
        short[2..4].copy_from_slice(&12u16.to_le_bytes());
        assert!(DataSubmessage::read_body_with_flags(&short, true, flags).is_err());

        let f = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        let (bytes, _) = f.write_body(true);
        let mut later = bytes[..32].to_vec();
        later[2..4].copy_from_slice(&32u16.to_le_bytes());
        later.extend_from_slice(&[0xEE; 4]);
        later.extend_from_slice(&bytes[32..]);
        let read = DataFragSubmessage::read_body(&later, true, false, false, false).unwrap();
        assert_eq!(&read.serialized_payload[..], &[1, 2, 3, 4]);
    }

    #[test]
    fn data_frag_decode_rejects_missing_inline_qos() {
        // A Q flag without an actual ParameterList is still malformed.
        let d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        let (bytes, _) = d.write_body(true);
        let res = DataFragSubmessage::read_body(&bytes, true, true, false, false);
        assert!(res.is_err());
    }

    #[test]
    fn data_frag_key_hash_roundtrips_both_byte_orders() {
        for little_endian in [false, true] {
            let mut d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
            let mut qos = crate::parameter_list::ParameterList::with_byte_order(little_endian);
            qos.push(crate::parameter_list::Parameter::new(
                crate::parameter_list::pid::KEY_HASH,
                vec![0; 16],
            ));
            d.inline_qos = Some(qos);
            d.inline_qos_flag = true;
            let (bytes, flags) = d.write_body(little_endian);
            assert_ne!(flags & DATA_FRAG_FLAG_INLINE_QOS, 0);
            let decoded =
                DataFragSubmessage::read_body(&bytes, little_endian, true, false, false).unwrap();
            assert_eq!(decoded, d);
            assert_eq!(
                decoded
                    .inline_qos
                    .as_ref()
                    .and_then(crate::inline_qos::find_key_hash),
                Some([0; 16])
            );
        }
    }

    #[test]
    fn data_frag_ignored_parameter_does_not_become_payload() {
        let mut d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        let mut qos = crate::parameter_list::ParameterList::new();
        qos.push(crate::parameter_list::Parameter::new(
            crate::parameter_list::pid::IGNORE,
            vec![9; 4],
        ));
        qos.push(crate::parameter_list::Parameter::new(
            crate::parameter_list::pid::KEY_HASH,
            vec![7; 16],
        ));
        d.inline_qos = Some(qos);
        for endian in [false, true] {
            let (body, _) = d.write_body(endian);
            let decoded = DataFragSubmessage::read_body(&body, endian, true, false, false).unwrap();
            assert_eq!(decoded.serialized_payload, d.serialized_payload);
            assert_eq!(
                decoded
                    .inline_qos
                    .as_ref()
                    .and_then(crate::inline_qos::find_key_hash),
                Some([7; 16])
            );
        }
    }

    #[test]
    fn data_frag_flags_survive_roundtrip() {
        let mut d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        d.key_flag = true;
        d.non_standard_flag = true;
        let (bytes, flags) = d.write_body(true);
        assert!(flags & DATA_FRAG_FLAG_KEY != 0);
        assert!(flags & DATA_FRAG_FLAG_NON_STANDARD != 0);
        let decoded = DataFragSubmessage::read_body(&bytes, true, false, true, true).unwrap();
        assert!(decoded.key_flag);
        assert!(decoded.non_standard_flag);
    }

    // ---- HEARTBEAT_FRAG Submessage ----

    #[test]
    fn heartbeat_frag_roundtrip_le() {
        let h = HeartbeatFragSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(42),
            last_fragment_num: FragmentNumber(8),
            count: 3,
        };
        let (bytes, flags) = h.write_body(true);
        assert!(flags & FLAG_E_LITTLE_ENDIAN != 0);
        assert_eq!(bytes.len(), HeartbeatFragSubmessage::WIRE_SIZE);
        let decoded = HeartbeatFragSubmessage::read_body(&bytes, true).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn heartbeat_frag_roundtrip_be() {
        let h = HeartbeatFragSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(1),
            last_fragment_num: FragmentNumber(1),
            count: 1,
        };
        let (bytes, _) = h.write_body(false);
        let decoded = HeartbeatFragSubmessage::read_body(&bytes, false).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn heartbeat_frag_decode_rejects_truncated() {
        let res = HeartbeatFragSubmessage::read_body(&[0u8; 20], true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- NACK_FRAG Submessage ----

    #[test]
    fn nack_frag_roundtrip_le() {
        let n = NackFragSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(5),
            fragment_number_state: FragmentNumberSet {
                bitmap_base: FragmentNumber(1),
                num_bits: 4,
                bitmap: vec![0b1010_0000_0000_0000_0000_0000_0000_0000],
            },
            count: 2,
        };
        let (bytes, flags) = n.write_body(true);
        assert!(flags & FLAG_E_LITTLE_ENDIAN != 0);
        let decoded = NackFragSubmessage::read_body(&bytes, true).unwrap();
        assert_eq!(decoded, n);
    }

    #[test]
    fn nack_frag_roundtrip_be() {
        let n = NackFragSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(100),
            fragment_number_state: FragmentNumberSet {
                bitmap_base: FragmentNumber(10),
                num_bits: 0,
                bitmap: vec![],
            },
            count: 0,
        };
        let (bytes, _) = n.write_body(false);
        let decoded = NackFragSubmessage::read_body(&bytes, false).unwrap();
        assert_eq!(decoded, n);
    }

    #[test]
    fn nack_frag_decode_rejects_truncated() {
        let res = NackFragSubmessage::read_body(&[0u8; 20], true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- WP 1.E stage E: InfoSource ----

    fn make_info_source() -> InfoSourceSubmessage {
        InfoSourceSubmessage {
            unused: 0,
            protocol_version: crate::wire_types::ProtocolVersion::V2_5,
            vendor_id: crate::wire_types::VendorId([0xAB, 0xCD]),
            guid_prefix: crate::wire_types::GuidPrefix::from_bytes([0xEE; 12]),
        }
    }

    #[test]
    fn info_source_roundtrip_le() {
        let i = make_info_source();
        let (bytes, flags) = i.write_body(true);
        assert!(flags & FLAG_E_LITTLE_ENDIAN != 0);
        assert_eq!(bytes.len(), InfoSourceSubmessage::WIRE_SIZE);
        let decoded = InfoSourceSubmessage::read_body(&bytes, true).unwrap();
        assert_eq!(decoded, i);
    }

    #[test]
    fn info_source_roundtrip_be() {
        let i = make_info_source();
        let (bytes, flags) = i.write_body(false);
        assert_eq!(flags & FLAG_E_LITTLE_ENDIAN, 0);
        let decoded = InfoSourceSubmessage::read_body(&bytes, false).unwrap();
        assert_eq!(decoded, i);
    }

    #[test]
    fn info_source_wire_size_is_20() {
        let i = make_info_source();
        let (bytes, _) = i.write_body(true);
        assert_eq!(bytes.len(), 20);
    }

    #[test]
    fn info_source_decode_rejects_truncated() {
        let res = InfoSourceSubmessage::read_body(&[0u8; 19], true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn info_source_unused_field_roundtrips() {
        // unused MUST roundtrip byte-identisch — manche Vendor-Implementations
        // enter diagnostic data there.
        let mut i = make_info_source();
        i.unused = 0xDEAD_BEEF;
        let (bytes, _) = i.write_body(true);
        let decoded = InfoSourceSubmessage::read_body(&bytes, true).unwrap();
        assert_eq!(decoded.unused, 0xDEAD_BEEF);
    }

    // ---- WP 1.E stage F: InfoReply ----

    #[test]
    fn info_reply_unicast_only_roundtrip_le() {
        use crate::wire_types::Locator;
        let i = InfoReplySubmessage {
            unicast_locators: vec![
                Locator::udp_v4([10, 0, 0, 1], 7411),
                Locator::udp_v4([10, 0, 0, 2], 7411),
            ],
            multicast_locators: None,
        };
        let (bytes, flags) = i.write_body(true);
        assert_eq!(flags & INFO_REPLY_FLAG_MULTICAST, 0);
        let decoded = InfoReplySubmessage::read_body(&bytes, true, false).unwrap();
        assert_eq!(decoded, i);
    }

    #[test]
    fn info_reply_with_multicast_roundtrip_le() {
        use crate::wire_types::Locator;
        let i = InfoReplySubmessage {
            unicast_locators: vec![Locator::udp_v4([10, 0, 0, 1], 7411)],
            multicast_locators: Some(vec![Locator::udp_v4([239, 255, 0, 1], 7400)]),
        };
        let (bytes, flags) = i.write_body(true);
        assert!(flags & INFO_REPLY_FLAG_MULTICAST != 0);
        let decoded = InfoReplySubmessage::read_body(&bytes, true, true).unwrap();
        assert_eq!(decoded, i);
    }

    #[test]
    fn info_reply_with_multicast_roundtrip_be() {
        use crate::wire_types::Locator;
        let i = InfoReplySubmessage {
            unicast_locators: vec![Locator::udp_v4([10, 0, 0, 5], 7420)],
            multicast_locators: Some(vec![Locator::udp_v4([239, 255, 0, 9], 7400)]),
        };
        let (bytes, _) = i.write_body(false);
        let decoded = InfoReplySubmessage::read_body(&bytes, false, true).unwrap();
        assert_eq!(decoded, i);
    }

    #[test]
    fn info_reply_empty_unicast_list_is_valid() {
        // The spec allows an empty list (e.g. "forget all previous
        // reply locators"). The decoder must accept it.
        let i = InfoReplySubmessage {
            unicast_locators: vec![],
            multicast_locators: None,
        };
        let (bytes, _) = i.write_body(true);
        let decoded = InfoReplySubmessage::read_body(&bytes, true, false).unwrap();
        assert_eq!(decoded, i);
        assert!(decoded.unicast_locators.is_empty());
    }

    /// DDSI-RTPS 2.5 §9.3.2.4: a locator of a kind from the vendors' ranges
    /// (here 0x01000000 and 0x03000001) is left out of the list, and the
    /// locators behind it are read, in both byte orders.
    #[test]
    fn info_reply_leaves_out_a_locator_of_a_kind_it_does_not_know() {
        use crate::wire_types::Locator;
        let udp = Locator::udp_v4([10, 1, 2, 3], 7411);
        for little_endian in [true, false] {
            let foreign = |kind: i32| {
                let mut raw = [0u8; 24];
                let k = if little_endian {
                    kind.to_le_bytes()
                } else {
                    kind.to_be_bytes()
                };
                raw[..4].copy_from_slice(&k);
                raw[4..8].copy_from_slice(&[0, 0, 0x1C, 0xF3]);
                raw
            };
            let mut body = Vec::new();
            let count = 3u32;
            body.extend_from_slice(&if little_endian {
                count.to_le_bytes()
            } else {
                count.to_be_bytes()
            });
            body.extend_from_slice(&foreign(0x0100_0000));
            body.extend_from_slice(&if little_endian {
                udp.to_bytes_le()
            } else {
                udp.to_bytes_be()
            });
            body.extend_from_slice(&foreign(0x0300_0001));
            let decoded = InfoReplySubmessage::read_body(&body, little_endian, false).unwrap();
            assert_eq!(
                decoded.unicast_locators,
                vec![udp],
                "little_endian {little_endian}"
            );
        }
    }

    #[test]
    fn info_reply_decode_rejects_oversized_locator_list_length() {
        // length=u32::MAX → DoS-Schutz greift
        let mut body = Vec::new();
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        // no locator body
        let res = InfoReplySubmessage::read_body(&body, true, false);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn info_reply_decode_rejects_truncated_length_field() {
        let res = InfoReplySubmessage::read_body(&[0u8; 3], true, false);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    // ---- PSM §9.4.5.11 InfoReplyIp4 ----

    #[test]
    fn info_reply_ip4_unicast_only_roundtrip_le() {
        use crate::wire_types::Locator;
        let i = InfoReplyIp4Submessage {
            unicast_locator: Locator::udp_v4([10, 0, 0, 1], 7411),
            multicast_locator: None,
        };
        let (bytes, flags) = i.write_body(true);
        assert_eq!(bytes.len(), 8);
        assert_eq!(flags & INFO_REPLY_FLAG_MULTICAST, 0);
        let decoded = InfoReplyIp4Submessage::read_body(&bytes, true, false).unwrap();
        assert_eq!(decoded, i);
    }

    #[test]
    fn info_reply_ip4_with_multicast_roundtrip_be() {
        use crate::wire_types::Locator;
        let i = InfoReplyIp4Submessage {
            unicast_locator: Locator::udp_v4([10, 0, 0, 5], 7420),
            multicast_locator: Some(Locator::udp_v4([239, 255, 0, 1], 7400)),
        };
        let (bytes, flags) = i.write_body(false);
        assert_eq!(bytes.len(), 16);
        assert!(flags & INFO_REPLY_FLAG_MULTICAST != 0);
        let decoded = InfoReplyIp4Submessage::read_body(&bytes, false, true).unwrap();
        assert_eq!(decoded, i);
    }

    #[test]
    fn info_reply_ip4_decode_rejects_truncated_body() {
        let res = InfoReplyIp4Submessage::read_body(&[0u8; 7], true, false);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
        let res = InfoReplyIp4Submessage::read_body(&[0u8; 15], true, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn info_reply_ip4_locator_kind_is_always_udpv4() {
        use crate::wire_types::{Locator, LocatorKind};
        let i = InfoReplyIp4Submessage {
            unicast_locator: Locator::udp_v4([192, 168, 0, 1], 7400),
            multicast_locator: None,
        };
        let (bytes, _) = i.write_body(true);
        let decoded = InfoReplyIp4Submessage::read_body(&bytes, true, false).unwrap();
        assert_eq!(decoded.unicast_locator.kind, LocatorKind::UdpV4);
    }

    /// DDSI-RTPS 2.5 §9.4.3.1 and §9.4.5.15: a LocatorUDPv4 is `unsigned
    /// long address` and `unsigned long port`, each in the byte order of the
    /// submessage; 192.168.1.10 is the number 0xC0A8010A. INFO_REPLY_IP4
    /// carries the unicast locator and, with the M flag, the multicast one;
    /// no other flag is set.
    #[test]
    fn locator_udpv4_is_two_unsigned_longs_in_the_byte_order_of_the_submessage() {
        use crate::wire_types::Locator;
        let i = InfoReplyIp4Submessage {
            unicast_locator: Locator::udp_v4([192, 168, 1, 10], 7411),
            multicast_locator: Some(Locator::udp_v4([239, 255, 0, 1], 7400)),
        };
        let (le, flags) = i.write_body(true);
        assert_eq!(flags, FLAG_E_LITTLE_ENDIAN | INFO_REPLY_FLAG_MULTICAST);
        assert_eq!(
            le,
            [
                0x0A, 0x01, 0xA8, 0xC0, 0xF3, 0x1C, 0x00, 0x00, // 0xC0A8010A, 7411
                0x01, 0x00, 0xFF, 0xEF, 0xE8, 0x1C, 0x00, 0x00, // 0xEFFF0001, 7400
            ]
        );
        let (be, flags) = i.write_body(false);
        assert_eq!(flags, INFO_REPLY_FLAG_MULTICAST);
        assert_eq!(
            be,
            [
                0xC0, 0xA8, 0x01, 0x0A, 0x00, 0x00, 0x1C, 0xF3, //
                0xEF, 0xFF, 0x00, 0x01, 0x00, 0x00, 0x1C, 0xE8,
            ]
        );
        for (bytes, little_endian) in [(&le, true), (&be, false)] {
            assert_eq!(
                InfoReplyIp4Submessage::read_body(bytes, little_endian, true).unwrap(),
                i
            );
        }
        // Without the M flag: the unicast locator alone, 8 octets.
        let unicast_only = InfoReplyIp4Submessage {
            multicast_locator: None,
            ..i
        };
        assert_eq!(unicast_only.write_body(true).0.len(), 8);
        assert_eq!(unicast_only.write_body(true).1, FLAG_E_LITTLE_ENDIAN);
    }

    // ---- §8.3.7.5 / §8.3.8.5 InfoTimestamp ----

    #[test]
    fn info_timestamp_roundtrip_le() {
        let i = InfoTimestampSubmessage {
            timestamp: crate::header_extension::HeTimestamp {
                seconds: 0x1234_5678,
                fraction: 0x9ABC_DEF0,
            },
            invalidate: false,
        };
        let (bytes, flags) = i.write_body(true);
        assert_eq!(flags & INFO_TIMESTAMP_FLAG_INVALIDATE, 0);
        assert_eq!(bytes.len(), 8);
        let decoded = InfoTimestampSubmessage::read_body(&bytes, true, false).unwrap();
        assert_eq!(decoded, i);
    }

    #[test]
    fn info_timestamp_roundtrip_be() {
        let i = InfoTimestampSubmessage {
            timestamp: crate::header_extension::HeTimestamp {
                seconds: 1_700_000_000,
                fraction: 12345,
            },
            invalidate: false,
        };
        let (bytes, flags) = i.write_body(false);
        assert_eq!(flags & FLAG_E_LITTLE_ENDIAN, 0);
        let decoded = InfoTimestampSubmessage::read_body(&bytes, false, false).unwrap();
        assert_eq!(decoded, i);
    }

    #[test]
    fn info_timestamp_invalidate_flag_yields_empty_body() {
        let i = InfoTimestampSubmessage {
            timestamp: crate::header_extension::HeTimestamp::default(),
            invalidate: true,
        };
        let (bytes, flags) = i.write_body(true);
        assert!(flags & INFO_TIMESTAMP_FLAG_INVALIDATE != 0);
        assert!(bytes.is_empty(), "I-Flag → empty body");
        let decoded = InfoTimestampSubmessage::read_body(&bytes, true, true).unwrap();
        assert!(decoded.invalidate);
    }

    #[test]
    fn info_timestamp_decode_rejects_truncated_when_no_invalidate() {
        let res = InfoTimestampSubmessage::read_body(&[0u8; 4], true, false);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn info_timestamp_decode_with_invalidate_ignores_body() {
        // I-flag → the body is ignored; even when it is full,
        // invalidate=true holds.
        let res = InfoTimestampSubmessage::read_body(&[0u8; 8], true, true).unwrap();
        assert!(res.invalidate);
        assert_eq!(
            res.timestamp,
            crate::header_extension::HeTimestamp::default()
        );
    }

    /// DDSI-RTPS 2.5 §9.4.2.6 Table 9.5: in the bitmap "1234/12:00110"
    /// bitmapBase is 1234 and numBits 12; 1234 and 1235 are 0, 1236 and 1237
    /// are 1, 1238 to 1245 are 0. The bit for bitmapBase is the leftmost.
    #[test]
    fn the_bitmap_1234_12_00110_of_table_9_5_names_1236_and_1237() {
        let set = SequenceNumberSet {
            bitmap_base: SequenceNumber(1234),
            num_bits: 12,
            // 0011 0000 ...: bit 0 is the most significant bit.
            bitmap: alloc::vec![0b0011_0000 << 24],
        };
        assert_eq!(
            set.iter_set().collect::<alloc::vec::Vec<_>>(),
            alloc::vec![SequenceNumber(1236), SequenceNumber(1237)]
        );
        for sn in [1234, 1235, 1238, 1245] {
            assert!(
                !set.iter_set().any(|s| s.0 == sn),
                "{sn} is a 0 bit of the example"
            );
        }
        // The same set from the sequence numbers it names, as written to the
        // wire and read back, in both byte orders.
        for little_endian in [true, false] {
            let mut out = alloc::vec::Vec::new();
            set.write_to(&mut out, little_endian);
            assert_eq!(out.len(), 8 + 4 + 4);
            let (back, end) = SequenceNumberSet::read_from(&out, 0, little_endian).unwrap();
            assert_eq!(end, out.len());
            assert_eq!(back, set);
        }
        let mut le = alloc::vec::Vec::new();
        set.write_to(&mut le, true);
        assert_eq!(&le[8..12], &12u32.to_le_bytes(), "numBits");
        // The bitmap is a sequence of 32-bit words, bit 0 the high bit.
        assert_eq!(&le[12..16], &0x3000_0000u32.to_le_bytes());
    }
}

/// The validity rules of the submessages (DDSI-RTPS 2.5 §8.3.8 "Validity",
/// §9.4.2.6, §9.4.2.8, §9.4.2.10, §9.4.5.3). The bodies are written octet by
/// octet, little-endian, from the wire layouts of §9.4.5.
#[cfg(test)]
mod validity_tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    fn sn(v: i64) -> [u8; 8] {
        SequenceNumber(v).to_bytes_le()
    }

    /// readerId, writerId of a user reader and writer.
    const IDS: [u8; 8] = [0x40, 0x50, 0x60, 0x04, 0x10, 0x20, 0x30, 0x02];

    fn body(parts: &[&[u8]]) -> Vec<u8> {
        parts.iter().flat_map(|p| p.iter().copied()).collect()
    }

    fn rejected<T: core::fmt::Debug>(res: Result<T, WireError>) {
        assert!(
            matches!(res, Err(WireError::ValueOutOfRange { .. })),
            "{res:?}"
        );
    }

    /// §8.3.8.2: "writerSN.value is not strictly positive (1, 2, ...) or is
    /// SEQUENCENUMBER_UNKNOWN" ({-1, 0}); §9.4.5.3: "D=1 and K=1 is an invalid
    /// combination in this version of the protocol."
    #[test]
    fn data_with_writer_sn_not_positive_or_with_d_and_k_is_invalid() {
        let mk = |writer_sn: i64| {
            // extraFlags 0, octetsToInlineQos 16, ids, writerSN.
            body(&[&[0, 0, 16, 0], &IDS, &sn(writer_sn)])
        };
        let flags_d = DATA_FLAG_DATA;
        assert!(DataSubmessage::read_body_with_flags(&mk(1), true, flags_d).is_ok());
        for bad in [0, -1, -(1 << 32)] {
            rejected(DataSubmessage::read_body_with_flags(
                &mk(bad),
                true,
                flags_d,
            ));
        }
        rejected(DataSubmessage::read_body_with_flags(
            &mk(1),
            true,
            DATA_FLAG_DATA | DATA_FLAG_KEY,
        ));
        assert!(DataSubmessage::read_body_with_flags(&mk(1), true, DATA_FLAG_KEY).is_ok());
    }

    /// §8.3.8.3: invalid when writerSN is not strictly positive,
    /// fragmentStartingNum is not strictly positive or exceeds the total
    /// number of fragments, fragmentSize exceeds dataSize, or the size of
    /// serializedData exceeds fragmentsInSubmessage * fragmentSize.
    #[test]
    fn data_frag_validity_rules() {
        // writerSN, fragmentStartingNum, fragmentsInSubmessage, fragmentSize,
        // sampleSize, payload octets.
        let mk = |wsn: i64, start: u32, n: u16, fsize: u16, dsize: u32, len: usize| {
            body(&[
                &[0, 0, 28, 0],
                &IDS,
                &sn(wsn),
                &start.to_le_bytes(),
                &n.to_le_bytes(),
                &fsize.to_le_bytes(),
                &dsize.to_le_bytes(),
                &alloc::vec![0xAB; len],
            ])
        };
        let read = |b: &[u8]| DataFragSubmessage::read_body(b, true, false, false, false);
        // 10 octets in fragments of 4: 3 fragments; the third holds 2.
        assert!(read(&mk(1, 3, 1, 4, 10, 2)).is_ok());
        rejected(read(&mk(0, 1, 1, 4, 10, 4)));
        rejected(read(&mk(-1, 1, 1, 4, 10, 4)));
        rejected(read(&mk(1, 0, 1, 4, 10, 4)));
        rejected(read(&mk(1, 4, 1, 4, 10, 2)));
        rejected(read(&mk(1, 1, 1, 12, 10, 4)));
        rejected(read(&mk(1, 1, 1, 4, 10, 5)));
        // Cyclone DDS and Fast DDS pad the last fragment to the alignment of
        // the submessage: 10 octets in fragments of 7 end with 3 octets, sent
        // as 4.
        assert!(read(&mk(1, 2, 1, 7, 10, 4)).is_ok());
        rejected(read(&mk(1, 2, 1, 7, 10, 5)));
        // Two fragments of 4 in one submessage carry up to 8 octets.
        assert!(read(&mk(1, 1, 2, 4, 10, 8)).is_ok());
        rejected(read(&mk(1, 1, 2, 4, 10, 9)));
    }

    /// §9.3.2.1: `Time_t.seconds` is an `unsigned long`: a second count past
    /// 2^31 is the instant it says, not a negative one, in either byte order;
    /// the reserved TIME_INVALID and TIME_INFINITE are {0xffffffff, ...}.
    #[test]
    fn time_t_seconds_are_unsigned() {
        use crate::header_extension::HeTimestamp;
        for little_endian in [true, false] {
            for seconds in [0x7FFF_FFFFu32, 0x8000_0000, 0xF000_0001, 0xFFFF_FFFD] {
                let ts = HeTimestamp {
                    seconds,
                    fraction: 0x8000_0001,
                };
                let sub = InfoTimestampSubmessage {
                    timestamp: ts,
                    invalidate: false,
                };
                let (body, _flags) = sub.write_body(little_endian);
                let back = InfoTimestampSubmessage::read_body(&body, little_endian, false).unwrap();
                assert_eq!(back.timestamp, ts, "{seconds:#x} le={little_endian}");
                assert!(back.timestamp.seconds >= 0x7FFF_FFFF);
            }
        }
        assert_eq!(
            (
                HeTimestamp::TIME_INVALID.seconds,
                HeTimestamp::TIME_INVALID.fraction
            ),
            (0xFFFF_FFFF, 0xFFFF_FFFF)
        );
        assert_eq!(
            (
                HeTimestamp::TIME_INFINITE.seconds,
                HeTimestamp::TIME_INFINITE.fraction
            ),
            (0xFFFF_FFFF, 0xFFFF_FFFE)
        );
    }

    /// §8.3.8.3: a DATA_FRAG is invalid when its "inlineQos is invalid": a
    /// parameter list that runs past the body, or has no sentinel.
    #[test]
    fn data_frag_with_an_invalid_inline_qos_is_invalid() {
        let mk = |inline: &[u8]| {
            body(&[
                &[0, 0, 28, 0],
                &IDS,
                &sn(1),
                &1u32.to_le_bytes(),
                &1u16.to_le_bytes(),
                &4u16.to_le_bytes(),
                &4u32.to_le_bytes(),
                inline,
                &[0xAB; 4],
            ])
        };
        let read = |b: &[u8]| DataFragSubmessage::read_body(b, true, true, false, false);
        // A list of one parameter and the sentinel.
        let valid: &[u8] = &[0x05, 0x00, 0x04, 0x00, 1, 2, 3, 4, 0x01, 0x00, 0x00, 0x00];
        assert!(read(&mk(valid)).is_ok());
        // A parameter whose length runs past the body.
        let past_the_end: &[u8] = &[0x05, 0x00, 0x40, 0x00, 1, 2, 3, 4, 0x01, 0x00, 0x00, 0x00];
        assert!(read(&mk(past_the_end)).is_err());
        // No sentinel: the list runs into the payload and ends with the body.
        let no_sentinel: &[u8] = &[0x05, 0x00, 0x04, 0x00, 1, 2, 3, 4];
        assert!(read(&mk(no_sentinel)).is_err());
    }

    /// §8.3.8.6: invalid when "firstSN.value is zero or negative", "lastSN.value
    /// is negative" or "lastSN.value < firstSN.value - 1".
    #[test]
    fn heartbeat_validity_rules() {
        let mk = |first: i64, last: i64| body(&[&IDS, &sn(first), &sn(last), &1i32.to_le_bytes()]);
        let read = |b: &[u8]| HeartbeatSubmessage::read_body(b, true, false, false, false);
        assert!(read(&mk(1, 5)).is_ok());
        // An empty writer: lastSN = firstSN - 1.
        assert!(read(&mk(1, 0)).is_ok());
        assert!(read(&mk(7, 6)).is_ok());
        rejected(read(&mk(0, 0)));
        rejected(read(&mk(-3, 2)));
        rejected(read(&mk(1, -1)));
        rejected(read(&mk(7, 5)));
    }

    /// §8.3.8.4: invalid when "gapStart is zero or negative"; §9.4.2.6: a
    /// valid SequenceNumberSet has "bitmapBase >= 1".
    #[test]
    fn gap_and_sequence_number_set_validity_rules() {
        let set = |base: i64| body(&[&sn(base), &0u32.to_le_bytes()]);
        let gap = |start: i64, base: i64| body(&[&IDS, &sn(start), &set(base)]);
        let read = |b: &[u8]| GapSubmessage::read_body(b, true, false, false);
        assert!(read(&gap(1, 1)).is_ok());
        rejected(read(&gap(0, 1)));
        rejected(read(&gap(-2, 1)));
        rejected(read(&gap(1, 0)));
        rejected(SequenceNumberSet::read_from(&set(0), 0, true));
        rejected(SequenceNumberSet::read_from(&set(-5), 0, true));
        assert!(SequenceNumberSet::read_from(&set(1), 0, true).is_ok());
        // ACKNACK: readerSNState is the set.
        let ack = |base: i64| body(&[&IDS, &set(base), &1i32.to_le_bytes()]);
        assert!(AckNackSubmessage::read_body(&ack(1), true, false).is_ok());
        rejected(AckNackSubmessage::read_body(&ack(0), true, false));
    }

    /// The receive setting for OpenDDS and RTI (recorded sessions): only the
    /// empty set with bitmapBase 0 is accepted, only where the caller says so,
    /// and a non-empty set with bitmapBase 0 stays invalid.
    #[test]
    fn the_empty_set_with_base_zero_is_accepted_only_on_request() {
        let empty = body(&[&sn(0), &0u32.to_le_bytes()]);
        rejected(SequenceNumberSet::read_from(&empty, 0, true));
        assert!(SequenceNumberSet::read_from_with(&empty, 0, true, true).is_ok());
        let one_bit = body(&[&sn(0), &1u32.to_le_bytes(), &0x8000_0000u32.to_le_bytes()]);
        rejected(SequenceNumberSet::read_from_with(&one_bit, 0, true, true));
        let negative = body(&[&sn(-1), &0u32.to_le_bytes()]);
        rejected(SequenceNumberSet::read_from_with(&negative, 0, true, true));
    }

    /// §8.3.8.7: HEARTBEAT_FRAG is invalid when "writerSN.value is zero or
    /// negative" or "lastFragmentNum.value is zero or negative"; §8.3.8.12:
    /// NACK_FRAG when "writerSN.value is zero or negative" or its
    /// fragmentNumberState is invalid (§9.4.2.8: bitmapBase >= 1).
    #[test]
    fn fragment_submessages_validity_rules() {
        let hb =
            |wsn: i64, last: u32| body(&[&IDS, &sn(wsn), &last.to_le_bytes(), &1i32.to_le_bytes()]);
        assert!(HeartbeatFragSubmessage::read_body(&hb(1, 1), true).is_ok());
        rejected(HeartbeatFragSubmessage::read_body(&hb(0, 1), true));
        rejected(HeartbeatFragSubmessage::read_body(&hb(1, 0), true));
        let nf = |wsn: i64, base: u32| {
            body(&[
                &IDS,
                &sn(wsn),
                &base.to_le_bytes(),
                &0u32.to_le_bytes(),
                &1i32.to_le_bytes(),
            ])
        };
        assert!(NackFragSubmessage::read_body(&nf(1, 1), true).is_ok());
        rejected(NackFragSubmessage::read_body(&nf(0, 1), true));
        rejected(NackFragSubmessage::read_body(&nf(1, 0), true));
    }

    /// §9.3.2.4: a locator of a kind outside the ones this stack knows (a
    /// vendor's) in an INFO_REPLY's list does not make the submessage
    /// invalid: it is left out, counted, and the others stand, in either byte
    /// order.
    #[test]
    fn an_info_reply_locator_of_an_unknown_kind_is_left_out_and_counted() {
        let known = |port: u32| crate::wire_types::Locator::udp_v4([10, 0, 0, 1], port);
        let vendor_kind = |le: bool| {
            let mut b = [0u8; 24];
            let kind = 0x01F0_0099i32;
            b[..4].copy_from_slice(&if le {
                kind.to_le_bytes()
            } else {
                kind.to_be_bytes()
            });
            b[4..8].copy_from_slice(&if le {
                7000u32.to_le_bytes()
            } else {
                7000u32.to_be_bytes()
            });
            b
        };
        for le in [true, false] {
            let enc = |l: crate::wire_types::Locator| {
                if le { l.to_bytes_le() } else { l.to_bytes_be() }
            };
            let mut b = if le {
                3u32.to_le_bytes()
            } else {
                3u32.to_be_bytes()
            }
            .to_vec();
            b.extend_from_slice(&enc(known(7400)));
            b.extend_from_slice(&vendor_kind(le));
            b.extend_from_slice(&enc(known(7401)));
            let before = skipped_unknown_locators();
            let got = InfoReplySubmessage::read_body(&b, le, false).unwrap();
            assert_eq!(
                got.unicast_locators,
                alloc::vec![known(7400), known(7401)],
                "le={le}"
            );
            assert!(skipped_unknown_locators() > before, "counted, le={le}");
        }
    }

    /// §9.4.2.10: "typedef sequence<Locator_t, 8> LocatorList;".
    #[test]
    fn info_reply_locator_list_holds_at_most_8_locators() {
        let list = |n: u32| {
            let loc = crate::wire_types::Locator::udp_v4([10, 0, 0, 1], 7400).to_bytes_le();
            let mut b = n.to_le_bytes().to_vec();
            for _ in 0..n {
                b.extend_from_slice(&loc);
            }
            b
        };
        assert!(InfoReplySubmessage::read_body(&list(8), true, false).is_ok());
        rejected(InfoReplySubmessage::read_body(&list(9), true, false));
    }
}
