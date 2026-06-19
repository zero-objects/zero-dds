// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTPS submessages — DDSI-RTPS 2.5 §8.3.7.
//!
//! Implemented submessages:
//! - **DATA** (`DataSubmessage`) — §8.3.7.2.
//! - **HEARTBEAT** (`HeartbeatSubmessage`) — §8.3.7.5.
//! - **ACKNACK** (`AckNackSubmessage`) — §8.3.7.1.
//! - **GAP** (`GapSubmessage`) — §8.3.7.4.
//! - **DATA_FRAG** (`DataFragSubmessage`) — §8.3.7.3.
//! - **HEARTBEAT_FRAG** (`HeartbeatFragSubmessage`) — §8.3.7.6.
//! - **NACK_FRAG** (`NackFragSubmessage`) — §8.3.7.10.
//! - **INFO_SRC** (`InfoSourceSubmessage`) — §8.3.7.9.
//! - **INFO_TS** (`InfoTimestampSubmessage`) — §8.3.7.5/§8.3.8.5.
//! - **INFO_REPLY** (`InfoReplySubmessage`) — §8.3.7.8.
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
        let num_bits = u32::try_from(last.0 - base.0 + 1).unwrap_or(u32::MAX);
        let num_words = (num_bits as usize).div_ceil(32);
        let mut bitmap = alloc::vec![0u32; num_words];
        for sn in missing {
            if *sn < base {
                continue;
            }
            let offset = (sn.0 - base.0) as usize;
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

    /// Tatsaechliche Wire-Size dieses Sets.
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

    /// Tatsaechliche Wire-Size dieses Sets.
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
// DATA Submessage (§8.3.7.2)
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
    /// Serialized payload (XCDR2-encoded or vendor-specific).
    pub serialized_payload: Arc<[u8]>,
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
        flags |= DATA_FLAG_DATA;
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

    /// Decodes the DATA body from a slice. A backward-compat wrapper for
    /// callers that carry no Q-flag — inline QoS is ignored.
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
        // Read octetsToInlineQos — we do not use it directly (always 16),
        // but we consume the field.
        pos += 2;
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

        // Inline QoS — Q-flag (DATA_FLAG_INLINE_QOS = 0x02).
        let inline_qos = if flags & DATA_FLAG_INLINE_QOS != 0 {
            // ParameterList::from_bytes parses up to the sentinel and
            // returns the rest of the buffer to us via consumed bytes.
            // Since from_bytes only returns the list, we must track the
            // consumed length ourselves — we compute it from the list's
            // encode output.
            let pl = crate::parameter_list::ParameterList::from_bytes(&body[pos..], little_endian)?;
            // Re-encode to determine the consumed byte length. This is
            // robust and avoids drift with the from_bytes parser.
            let consumed = pl.to_bytes(little_endian).len();
            pos += consumed;
            Some(pl)
        } else {
            None
        };

        // The rest is serializedPayload.
        let serialized_payload: Arc<[u8]> = Arc::from(&body[pos..]);
        let key_flag = (flags & DATA_FLAG_KEY) != 0;
        let non_standard_flag = (flags & DATA_FLAG_NON_STANDARD) != 0;
        Ok(Self {
            extra_flags,
            reader_id,
            writer_id,
            writer_sn,
            inline_qos,
            key_flag,
            non_standard_flag,
            serialized_payload,
        })
    }
}

// ============================================================================
// HEARTBEAT Submessage (§8.3.7.5)
// ============================================================================

/// HEARTBEAT Flag: F (Final).
pub const HEARTBEAT_FLAG_FINAL: u8 = 0x02;
/// HEARTBEAT Flag: L (Liveliness).
pub const HEARTBEAT_FLAG_LIVELINESS: u8 = 0x04;
/// HEARTBEAT flag: G (GroupInfo present). A vendor extension for
/// group-ordered access (§8.3.8.6.2). The trailer contains currentGSN,
/// firstGSN, lastGSN and a `writerSet` (list of the GUID prefixes of the
/// group members).
pub const HEARTBEAT_FLAG_GROUP_INFO: u8 = 0x08;

/// Optional GroupInfo trailer of a HEARTBEAT submessage (§8.3.8.6.2).
///
/// Wire layout:
/// - currentGSN: i64
/// - firstGSN:   i64
/// - lastGSN:    i64
/// - writerSet:  u32 length + length × GuidPrefix(12 byte)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatGroupInfo {
    /// Current group SN (highest assigned by the group coordinator).
    pub current_gsn: SequenceNumber,
    /// First relevant group SN (cache_min of the group).
    pub first_gsn: SequenceNumber,
    /// Last available group SN (= currentGSN minus pending, in practice
    /// identical to currentGSN at steady state).
    pub last_gsn: SequenceNumber,
    /// GuidPrefix set of the participating writers of this group.
    pub writer_set: Vec<crate::wire_types::GuidPrefix>,
}

/// HEARTBEAT submessage.
///
/// `final_flag`, `liveliness_flag` and `group_info_flag` (via `Some` of
/// `group_info`) correspond to the F-/L-/G bits in the submessage header
/// (spec §8.3.7.5.1, §8.3.8.6.2) — they are **not** in the body, but are
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
            let len = u32::try_from(gi.writer_set.len()).unwrap_or(u32::MAX);
            out.extend_from_slice(&if little_endian {
                len.to_le_bytes()
            } else {
                len.to_be_bytes()
            });
            for prefix in &gi.writer_set {
                out.extend_from_slice(&prefix.to_bytes());
            }
        }
        (out, flags)
    }

    /// Decodes the body. `final_flag`, `liveliness_flag`,
    /// `group_info_flag` are extracted by the caller from the submessage header.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` (writerSet length bizarrely large).
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
        let group_info = if group_info_flag {
            // 3 × i64 + u32 = 28 byte minimum
            if body.len() < pos + 28 {
                return Err(WireError::UnexpectedEof {
                    needed: 28,
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
            let mut len_bytes = [0u8; 4];
            len_bytes.copy_from_slice(&body[pos..pos + 4]);
            let len = if little_endian {
                u32::from_le_bytes(len_bytes)
            } else {
                u32::from_be_bytes(len_bytes)
            } as usize;
            pos += 4;
            // Cap: writer_set must not be larger than what the body has
            // left. Protection against DoS via a huge length field.
            let remaining = body.len().saturating_sub(pos);
            if len.saturating_mul(12) > remaining {
                return Err(WireError::ValueOutOfRange {
                    message: "HEARTBEAT.groupInfo.writerSet length exceeds body",
                });
            }
            let mut writer_set = Vec::with_capacity(len);
            for _ in 0..len {
                let mut p = [0u8; 12];
                p.copy_from_slice(&body[pos..pos + 12]);
                writer_set.push(crate::wire_types::GuidPrefix::from_bytes(p));
                pos += 12;
            }
            Some(HeartbeatGroupInfo {
                current_gsn,
                first_gsn,
                last_gsn,
                writer_set,
            })
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
// ACKNACK Submessage (§8.3.7.1)
// ============================================================================

/// ACKNACK Flag: F (Final).
pub const ACKNACK_FLAG_FINAL: u8 = 0x02;

/// ACKNACK submessage.
///
/// `final_flag` corresponds to the F-bit in the submessage header (spec
/// §8.3.7.1.1). `final=false` requires a timely HEARTBEAT response from
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
        let (reader_sn_state, new_pos) = SequenceNumberSet::read_from(body, pos, little_endian)?;
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
// GAP Submessage (§8.3.7.4 / §8.3.8.4.2)
// ============================================================================

/// GAP Flag: G (GroupInfo present — `gapStartGSN`/`gapEndGSN` Trailer).
/// A vendor extension for group-ordered access (§8.3.8.4.2). The ZeroDDS encoder does not set it; the decoder accepts it on read.
pub const GAP_FLAG_GROUP_INFO: u8 = 0x04;

/// GAP flag: K (FilteredCount present). Spec §8.3.8.4.2 introduces an
/// optional `Count_t filteredCount` trailer field that lets the reader
/// distinguish "discarded via content filter" from "really removed" — a
/// prerequisite for correct instance-state transitions per §8.7.4
/// (NOT_ALIVE_FILTERED vs. NOT_ALIVE_DISPOSED).
pub const GAP_FLAG_FILTERED_COUNT: u8 = 0x08;

/// Optional trailer of a GAP submessage with GroupInfo (G-flag, §8.3.8.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GapGroupInfo {
    /// Group SN of the first skipped sample in the group.
    pub gap_start_gsn: SequenceNumber,
    /// Group SN of the last skipped sample in the group.
    pub gap_end_gsn: SequenceNumber,
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
    /// Optional `filteredCount` trailer (§8.3.8.4.2). `Some` ⇒
    /// K flag set in the header. `0` is explicitly "nothing filtered,
    /// everything really removed"; `1+` means "n samples discarded via
    /// content filter".
    pub filtered_count: Option<u32>,
}

impl GapSubmessage {
    /// Encodes the body. Returns (bytes, flags) incl. possible G/K bit.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        // 4 readerId + 4 writerId + 8 gap_start + 12 SN-Set-Base +
        // 4*words. Optional GroupInfo (24) + filteredCount (4) on top.
        let snset_words = self.gap_list.bitmap.len();
        let extra =
            self.group_info.as_ref().map_or(0, |_| 24) + self.filtered_count.map_or(0, |_| 4);
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
            out.extend_from_slice(&if little_endian {
                fc.to_le_bytes()
            } else {
                fc.to_be_bytes()
            });
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
            Some(GapGroupInfo {
                gap_start_gsn,
                gap_end_gsn,
            })
        } else {
            None
        };
        let filtered_count = if filtered_count_flag {
            if body.len() < pos + 4 {
                return Err(WireError::UnexpectedEof {
                    needed: 4,
                    offset: pos,
                });
            }
            let mut c = [0u8; 4];
            c.copy_from_slice(&body[pos..pos + 4]);
            let fc = if little_endian {
                u32::from_le_bytes(c)
            } else {
                u32::from_be_bytes(c)
            };
            Some(fc)
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
// DATA_FRAG Submessage (§8.3.7.3)
// ============================================================================

/// DATA_FRAG Flag: Q (Inline-QoS present).
pub const DATA_FRAG_FLAG_INLINE_QOS: u8 = 0x02;
/// DATA_FRAG Flag: H (hash key).
pub const DATA_FRAG_FLAG_HASH_KEY: u8 = 0x04;
/// DATA_FRAG flag: K (key flag — serialized_payload is key instead of data).
pub const DATA_FRAG_FLAG_KEY: u8 = 0x08;
/// DATA_FRAG Flag: N (non-standard payload).
pub const DATA_FRAG_FLAG_NON_STANDARD: u8 = 0x10;

/// DATA_FRAG submessage. Carries a section (fragments) of a sample
/// whose total size is in `sample_size`.
///
/// Flags (Q/H/K/N) are mirrored from the submessage header. The encoder does not currently set these flags; the decoder accepts them on read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFragSubmessage {
    /// octetsToInlineQos analogue to DATA (§8.3.7.2 speaks of extraFlags+
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
    /// H-flag from the submessage header (hash_key).
    pub hash_key_flag: bool,
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
        out.extend_from_slice(&self.serialized_payload);
        let mut flags = 0u8;
        if little_endian {
            flags |= FLAG_E_LITTLE_ENDIAN;
        }
        if self.inline_qos_flag {
            flags |= DATA_FRAG_FLAG_INLINE_QOS;
        }
        if self.hash_key_flag {
            flags |= DATA_FRAG_FLAG_HASH_KEY;
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
        hash_key_flag: bool,
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
        // The spec requires 28 = header size − 2 (extra_flags) − 2 (this
        // field). A deviation at Q=false we catch here — a frequent
        // interop bug that we reject spec-faithfully.
        let mut otq = [0u8; 2];
        otq.copy_from_slice(&body[pos..pos + 2]);
        let octets_to_inline_qos = if little_endian {
            u16::from_le_bytes(otq)
        } else {
            u16::from_be_bytes(otq)
        };
        pos += 2;
        if !inline_qos_flag && octets_to_inline_qos != Self::OCTETS_TO_INLINE_QOS {
            return Err(WireError::ValueOutOfRange {
                message: "DATA_FRAG.octetsToInlineQos must equal 28 when Q=false",
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
        // Q-flag = false, so no inline-QoS block.
        // With Q=true, ParameterList bytes would follow here — we do not
        // currently accept that.
        if inline_qos_flag {
            return Err(WireError::UnsupportedFeature {
                what: "DATA_FRAG with inline_qos",
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
            hash_key_flag,
            key_flag,
            non_standard_flag,
        })
    }
}

// ============================================================================
// InfoSource Submessage (§8.3.7.9 / §8.3.8.9.4) — submessageId 0x0c (legacy
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

    /// Decoded den Body.
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
// InfoTimestamp Submessage (§8.3.8.5 / §8.3.7.5) — submessageId 0x09
// ============================================================================

/// InfoTimestamp flag: I (Invalidate). When set: the body is empty and
/// `haveTimestamp` is set to `false` in the ReceiverState.
pub const INFO_TIMESTAMP_FLAG_INVALIDATE: u8 = 0x02;

/// InfoTimestamp submessage (§8.3.7.5 / §8.3.8.5). Sets the `timestamp`
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
        let mut out = Vec::with_capacity(8);
        let s = if little_endian {
            self.timestamp.seconds.to_le_bytes()
        } else {
            self.timestamp.seconds.to_be_bytes()
        };
        let f = if little_endian {
            self.timestamp.fraction.to_le_bytes()
        } else {
            self.timestamp.fraction.to_be_bytes()
        };
        out.extend_from_slice(&s);
        out.extend_from_slice(&f);
        (out, flags)
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
            i32::from_le_bytes(s)
        } else {
            i32::from_be_bytes(s)
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
// InfoReply Submessage (§8.3.7.10 / §8.3.8.10.4) — submessageId 0x0F
// ============================================================================

/// InfoReply flag: M (multicast). If set: a second LocatorList
/// (multicastReplyLocatorList) folgt im Body.
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
            // BE decode: build the locator manually, since from_bytes_le
            // strikt LE annimmt.
            let loc = if little_endian {
                crate::wire_types::Locator::from_bytes_le(buf)?
            } else {
                let mut k = [0u8; 4];
                k.copy_from_slice(&buf[0..4]);
                let kind_raw = i32::from_be_bytes(k);
                let kind = crate::wire_types::LocatorKind::from_i32(kind_raw)?;
                let mut p = [0u8; 4];
                p.copy_from_slice(&buf[4..8]);
                let port = u32::from_be_bytes(p);
                let mut address = [0u8; 16];
                address.copy_from_slice(&buf[8..24]);
                crate::wire_types::Locator {
                    kind,
                    port,
                    address,
                }
            };
            out.push(loc);
            *pos += 24;
        }
        Ok(out)
    }
}

// ============================================================================
// HEARTBEAT_FRAG Submessage (§8.3.7.7)
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

    /// Encoded den Body.
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

    /// Decoded den Body.
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
// NACK_FRAG Submessage (§8.3.7.6)
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
    /// Encoded den Body.
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

    /// Decoded den Body.
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
        buf.extend_from_slice(&SequenceNumber(0).to_bytes_le());
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
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: Arc::<[u8]>::from([1u8, 2, 3, 4, 5].as_slice()),
        };
        let (bytes, flags) = d.write_body(true);
        assert!(flags & FLAG_E_LITTLE_ENDIAN != 0);
        assert!(flags & DATA_FLAG_DATA != 0);
        let decoded = DataSubmessage::read_body(&bytes, true).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_submessage_roundtrip_be_with_empty_payload() {
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(0xDEAD_BEEF),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: Arc::<[u8]>::from([].as_slice()),
        };
        let (bytes, flags) = d.write_body(false);
        assert_eq!(flags & FLAG_E_LITTLE_ENDIAN, 0);
        let decoded = DataSubmessage::read_body(&bytes, false).unwrap();
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
            key_flag: true,
            non_standard_flag: false,
            serialized_payload: Arc::<[u8]>::from([0xAA, 0xBB].as_slice()),
        };
        let (bytes, flags) = d.write_body(true);
        assert!(flags & DATA_FLAG_KEY != 0, "K-Flag must be set");
        let decoded = DataSubmessage::read_body_with_flags(&bytes, true, flags).unwrap();
        assert!(decoded.key_flag);
        assert!(!decoded.non_standard_flag);
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_submessage_non_standard_flag_roundtrip() {
        // Spec §8.3.8.2 N-Flag: NonStandardPayload (z.B. Encrypted).
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(8),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: true,
            serialized_payload: Arc::<[u8]>::from([0xCC, 0xDD].as_slice()),
        };
        let (bytes, flags) = d.write_body(true);
        assert!(flags & DATA_FLAG_NON_STANDARD != 0, "N-Flag must be set");
        let decoded = DataSubmessage::read_body_with_flags(&bytes, true, flags).unwrap();
        assert!(!decoded.key_flag);
        assert!(decoded.non_standard_flag);
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_submessage_all_flags_combined_roundtrip() {
        // E + Q + D + K + N all set — the full 5-flag roundtrip.
        let mut pl = crate::parameter_list::ParameterList::new();
        pl.push(crate::parameter_list::Parameter::new(0x0070, vec![1; 4]));
        let d = DataSubmessage {
            extra_flags: 0xABCD,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(9),
            inline_qos: Some(pl),
            key_flag: true,
            non_standard_flag: true,
            serialized_payload: Arc::<[u8]>::from([0xEE; 8].as_slice()),
        };
        let (bytes, flags) = d.write_body(true);
        assert!(flags & FLAG_E_LITTLE_ENDIAN != 0);
        assert!(flags & DATA_FLAG_INLINE_QOS != 0);
        assert!(flags & DATA_FLAG_DATA != 0);
        assert!(flags & DATA_FLAG_KEY != 0);
        assert!(flags & DATA_FLAG_NON_STANDARD != 0);
        let decoded = DataSubmessage::read_body_with_flags(&bytes, true, flags).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_submessage_octets_to_inline_qos_is_16() {
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: reader_id(),
            writer_id: writer_id(),
            writer_sn: SequenceNumber(1),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: Arc::<[u8]>::from([].as_slice()),
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

    #[test]
    fn heartbeat_with_empty_group_info_roundtrip_le() {
        let h = HeartbeatSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(5),
            count: 3,
            final_flag: false,
            liveliness_flag: false,
            group_info: Some(HeartbeatGroupInfo {
                current_gsn: SequenceNumber(100),
                first_gsn: SequenceNumber(50),
                last_gsn: SequenceNumber(99),
                writer_set: vec![],
            }),
        };
        let (bytes, flags) = h.write_body(true);
        assert!(flags & HEARTBEAT_FLAG_GROUP_INFO != 0);
        let decoded = HeartbeatSubmessage::read_body(&bytes, true, false, false, true).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn heartbeat_with_writer_set_roundtrip_be() {
        use crate::wire_types::GuidPrefix;
        let h = HeartbeatSubmessage {
            reader_id: reader_id(),
            writer_id: writer_id(),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(2),
            count: 1,
            final_flag: false,
            liveliness_flag: false,
            group_info: Some(HeartbeatGroupInfo {
                current_gsn: SequenceNumber(7),
                first_gsn: SequenceNumber(1),
                last_gsn: SequenceNumber(7),
                writer_set: vec![
                    GuidPrefix::from_bytes([1; 12]),
                    GuidPrefix::from_bytes([2; 12]),
                    GuidPrefix::from_bytes([3; 12]),
                ],
            }),
        };
        let (bytes, flags) = h.write_body(false);
        assert!(flags & HEARTBEAT_FLAG_GROUP_INFO != 0);
        let decoded = HeartbeatSubmessage::read_body(&bytes, false, false, false, true).unwrap();
        assert_eq!(decoded, h);
        let gi = decoded.group_info.unwrap();
        assert_eq!(gi.writer_set.len(), 3);
    }

    #[test]
    fn heartbeat_decode_rejects_oversized_writer_set_length() {
        // length=u32::MAX waere 12 × MAX byte → DoS-Schutz.
        let mut body = Vec::new();
        body.extend_from_slice(&reader_id().to_bytes());
        body.extend_from_slice(&writer_id().to_bytes());
        body.extend_from_slice(&SequenceNumber(1).to_bytes_le());
        body.extend_from_slice(&SequenceNumber(1).to_bytes_le());
        body.extend_from_slice(&1i32.to_le_bytes());
        // 3 × i64 GSN
        body.extend_from_slice(&SequenceNumber(0).to_bytes_le());
        body.extend_from_slice(&SequenceNumber(0).to_bytes_le());
        body.extend_from_slice(&SequenceNumber(0).to_bytes_le());
        // bizarre length
        body.extend_from_slice(&u32::MAX.to_le_bytes());
        // no body for prefixes
        let res = HeartbeatSubmessage::read_body(&body, true, false, false, true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
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
            hash_key_flag: false,
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
        let decoded =
            DataFragSubmessage::read_body(&bytes, true, false, false, false, false).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_frag_roundtrip_be() {
        let d = dataflag_frag(7, 2, 1, 8, 16, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let (bytes, flags) = d.write_body(false);
        assert_eq!(flags & FLAG_E_LITTLE_ENDIAN, 0);
        let decoded =
            DataFragSubmessage::read_body(&bytes, false, false, false, false, false).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn data_frag_last_fragment_shorter_than_fragment_size() {
        // sample_size=10, fragment_size=4, fragment 3 carries only 2 bytes
        let d = dataflag_frag(1, 3, 1, 4, 10, vec![0xAA, 0xBB]);
        let (bytes, _) = d.write_body(true);
        let decoded =
            DataFragSubmessage::read_body(&bytes, true, false, false, false, false).unwrap();
        assert_eq!(decoded.serialized_payload.as_ref(), &[0xAA, 0xBB][..]);
        assert_eq!(decoded.sample_size, 10);
        assert_eq!(decoded.fragment_size, 4);
    }

    #[test]
    fn data_frag_decode_rejects_truncated() {
        let res = DataFragSubmessage::read_body(&[0u8; 20], true, false, false, false, false);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn data_frag_decode_accepts_nonzero_extra_flags_silently() {
        // B3 research: Cyclone/Fast-DDS ignore non-zero extra_flags.
        // So do we.
        let d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        let (mut bytes, _) = d.write_body(true);
        bytes[0..2].copy_from_slice(&0x0042u16.to_le_bytes()); // extra_flags nonzero
        let decoded =
            DataFragSubmessage::read_body(&bytes, true, false, false, false, false).unwrap();
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
        let res = DataFragSubmessage::read_body(&bytes, true, false, false, false, false);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn data_frag_decode_rejects_inline_qos() {
        // Q-flag true is rejected (feature not implemented).
        let d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        let (bytes, _) = d.write_body(true);
        let res = DataFragSubmessage::read_body(&bytes, true, true, false, false, false);
        assert!(matches!(res, Err(WireError::UnsupportedFeature { .. })));
    }

    #[test]
    fn data_frag_flags_survive_roundtrip() {
        let mut d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        d.hash_key_flag = true;
        d.key_flag = true;
        d.non_standard_flag = true;
        let (bytes, flags) = d.write_body(true);
        assert!(flags & DATA_FRAG_FLAG_HASH_KEY != 0);
        assert!(flags & DATA_FRAG_FLAG_KEY != 0);
        assert!(flags & DATA_FRAG_FLAG_NON_STANDARD != 0);
        let decoded = DataFragSubmessage::read_body(&bytes, true, false, true, true, true).unwrap();
        assert!(decoded.hash_key_flag);
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
}
