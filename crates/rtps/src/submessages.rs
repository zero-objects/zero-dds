// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTPS-Submessages — DDSI-RTPS 2.5 §8.3.7.
//!
//! Implementierte Submessages:
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
//! ParameterList (Inline-QoS) lebt im separaten Modul
//! [`crate::parameter_list`]; SecuredPayload-Wrapping liegt im
//! `zerodds-security`-Crate (DDS-Security 1.2 §7.4).
//!
//! # Endianness
//!
//! Submessage-Bodies werden in der Endianness des Submessage-Headers
//! geschrieben (E-Flag). Die hier gegebenen `to_bytes_*`/`from_bytes_*`-
//! Funktionen nehmen explizite Endianness als Parameter — der Caller
//! muss sie konsistent zum Header waehlen.

extern crate alloc;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::submessage_header::FLAG_E_LITTLE_ENDIAN;
use crate::wire_types::{EntityId, FragmentNumber, SequenceNumber};

/// Hard-Cap fuer `numBits` in `SequenceNumberSet` und
/// `FragmentNumberSet`. DDSI-RTPS gibt kein spezifisches Limit vor,
/// aber sowohl Cyclone DDS (`ddsi_radmin.c`) als auch Fast-DDS
/// (`BitmapRange<..., 256>`) capen auf 256. Wir folgen — verhindert
/// DoS via `numBits=2^32-1`-Bitmap-Alloc.
pub const RTPS_BITMAP_MAX_BITS: u32 = 256;

// ============================================================================
// SequenceNumberSet (§9.4.2.6)
// ============================================================================

/// Bitset von Sequence-Numbers ab `bitmap_base`. Wird in HEARTBEAT/
/// ACKNACK/GAP genutzt, um Mengen verlorener oder bekannter Pakete zu
/// signalisieren.
///
/// Wire-Layout (variable Laenge):
///   bitmapBase: 8 Byte (SequenceNumber, big oder little gemaess Header)
///   numBits:    4 Byte (u32)
///   bitmap:     ceil(numBits/32) * 4 Byte (u32-Words)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceNumberSet {
    /// Erste Sequence-Number, fuer die das erste Bit zustaendig ist.
    pub bitmap_base: SequenceNumber,
    /// Anzahl gueltiger Bits.
    pub num_bits: u32,
    /// `ceil(num_bits/32)` u32-Worte.
    pub bitmap: Vec<u32>,
}

impl SequenceNumberSet {
    /// Berechnet die Wire-Size in Bytes basierend auf `num_bits`.
    #[must_use]
    pub fn wire_size(num_bits: u32) -> usize {
        let words = (num_bits as usize).div_ceil(32);
        8 + 4 + words * 4
    }

    /// Baut ein `SequenceNumberSet` aus einer sortierten Liste fehlender SNs.
    ///
    /// `base` ist die kleinste noch nicht acked SN (der AckNack-Base).
    /// `missing` muss aufsteigend sortiert und alle SNs ≥ `base` sein.
    /// Bits werden in RTPS-Konvention gesetzt: Bit 0 ist das hoechstwertige
    /// Bit (MSB) von `bitmap[0]`.
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

    /// Iteriert ueber alle SNs, deren Bit gesetzt ist.
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

    /// Encoded das Set in `out` mit der gegebenen Endianness.
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

    /// Decoded ein Set aus `bytes` ab `offset`. Liefert (Set, neue Position).
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

/// Bitset von `FragmentNumber`-Werten ab `bitmap_base`. Analog zu
/// [`SequenceNumberSet`], aber mit `FragmentNumber` (u32) als Base
/// statt `SequenceNumber`.
///
/// Wire-Layout:
///   bitmapBase: 4 Byte (FragmentNumber, LE oder BE gemaess Header)
///   numBits:    4 Byte (u32)
///   bitmap:     ceil(numBits/32) * 4 Byte
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentNumberSet {
    /// Erstes Fragment, fuer das das erste Bit zustaendig ist.
    pub bitmap_base: FragmentNumber,
    /// Anzahl gueltiger Bits.
    pub num_bits: u32,
    /// `ceil(num_bits/32)` u32-Worte.
    pub bitmap: Vec<u32>,
}

impl FragmentNumberSet {
    /// Wire-Size in Bytes.
    #[must_use]
    pub fn wire_size(num_bits: u32) -> usize {
        let words = (num_bits as usize).div_ceil(32);
        4 + 4 + words * 4
    }

    /// Baut das Set aus einer sortierten Liste fehlender Fragmente.
    /// `base` = kleinste noch nicht bestaetigte FragmentNumber.
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
        let num_bits = last.0.saturating_sub(base.0).saturating_add(1);
        let num_words = (num_bits as usize).div_ceil(32);
        let mut bitmap = alloc::vec![0u32; num_words];
        for fnum in missing {
            if *fnum < base {
                continue;
            }
            let offset = (fnum.0 - base.0) as usize;
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

    /// Iteriert ueber alle gesetzten FragmentNumbers.
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

    /// Encoded das Set in `out`.
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

    /// Decoded ein Set aus `bytes` ab `offset`.
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

/// DATA-Submessage. Phase 0 unterstuetzt nur D-Flag (Daten), kein Q
/// (kein Inline-QoS), kein K, kein N.
///
/// `serialized_payload` ist `Arc<[u8]>` (WP 2.0a Zero-Copy-Spike).
/// Writer teilen sich die Payload-Allocation mit `CacheChange` und
/// allen DATA/DATA_FRAG-Datagrammen — `clone()` auf dieser Struct ist
/// ein reiner Refcount-Bump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSubmessage {
    /// Reserved Extra-Flags (uint16, meist 0).
    pub extra_flags: u16,
    /// Empfaenger-EntityId.
    pub reader_id: EntityId,
    /// Sender-EntityId.
    pub writer_id: EntityId,
    /// Sequence-Number dieser DATA.
    pub writer_sn: SequenceNumber,
    /// Inline-QoS-ParameterList (Q-Flag, §9.4.5.3.2). `None` = kein
    /// Q-Flag, kein Inline-QoS-Block. Trager fuer PID_KEY_HASH (WP 1.B),
    /// PID_STATUS_INFO, PID_COHERENT_SET etc.
    pub inline_qos: Option<crate::parameter_list::ParameterList>,
    /// K-Flag (Spec §8.3.8.2 Tab. 8.43). `true`: `serialized_payload`
    /// enthaelt nur die @key-Felder (key-only Sample, z.B. dispose-
    /// Marker). Der D-Flag kann gleichzeitig false sein, wenn nur Key
    /// gesendet wird; in dem Fall ist `serialized_payload` ein
    /// XCDR-codierter Key-Holder.
    pub key_flag: bool,
    /// N-Flag (Spec §8.3.8.2 Tab. 8.43, NonStandardPayloadFlag).
    /// `true`: `serialized_payload` ist NICHT in der durch
    /// `representation_identifier` impliziten CDR-Variante codiert
    /// (z.B. fuer DDS-Security-encrypted Payloads).
    pub non_standard_flag: bool,
    /// Serialisierter Payload (XCDR2-codiert oder vendor-spezifisch).
    pub serialized_payload: Arc<[u8]>,
}

impl DataSubmessage {
    /// Encoded den DATA-Body (ohne Submessage-Header) in einen Vec.
    /// Setzt automatisch das D-Flag und ggf. das Q-Flag im
    /// `flags`-Output (rueckgegeben), damit der Caller den
    /// Submessage-Header korrekt fuellen kann.
    ///
    /// Layout: extraFlags(2) + octetsToInlineQos(2) + readerId(4) +
    /// writerId(4) + writerSN(8) + [optional InlineQoS-PL] + payload.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::new();
        // extraFlags (2 byte)
        let extra = if little_endian {
            self.extra_flags.to_le_bytes()
        } else {
            self.extra_flags.to_be_bytes()
        };
        out.extend_from_slice(&extra);
        // octetsToInlineQos (2 byte) — Distanz vom Ende dieses Felds bis
        // Anfang von readerId. Konstant 16 (4 readerId + 4 writerId
        // + 8 writerSN), unabhaengig vom Q-Flag.
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
        // Inline-QoS-ParameterList (Q-Flag) — wenn vorhanden.
        if let Some(pl) = &self.inline_qos {
            out.extend_from_slice(&pl.to_bytes(little_endian));
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

    /// Decoded den DATA-Body aus einem Slice. Backward-Compat-Wrapper
    /// fuer Caller, die kein Q-Flag tragen — Inline-QoS wird ignoriert.
    ///
    /// # Errors
    /// `UnexpectedEof` bei zu kurzem Body.
    pub fn read_body(body: &[u8], little_endian: bool) -> Result<Self, WireError> {
        Self::read_body_with_flags(body, little_endian, 0)
    }

    /// Decoded den DATA-Body unter Beruecksichtigung der Submessage-Flags.
    /// Wenn `flags & DATA_FLAG_INLINE_QOS != 0` parst der Decoder die
    /// ParameterList nach dem writerSN, vor dem Payload.
    ///
    /// # Errors
    /// `UnexpectedEof` bei zu kurzem Body. ParameterList-Fehler werden
    /// als `WireError` durchgereicht.
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
        // octetsToInlineQos lesen — wir nutzen es nicht direkt (immer 16),
        // aber wir konsumieren das Feld.
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

        // Inline-QoS — Q-Flag (DATA_FLAG_INLINE_QOS = 0x02).
        let inline_qos = if flags & DATA_FLAG_INLINE_QOS != 0 {
            // ParameterList::from_bytes parst bis zum Sentinel und gibt
            // den Rest des Buffers an uns zurueck via konsumierte Bytes.
            // Da from_bytes nur die Liste liefert, muessen wir die
            // Konsum-Laenge selbst tracken — wir berechnen sie aus dem
            // Encode-Output der List.
            let pl = crate::parameter_list::ParameterList::from_bytes(&body[pos..], little_endian)?;
            // Re-Encode um konsumierte Byte-Laenge zu ermitteln. Das ist
            // robust und vermeidet Drift mit dem from_bytes-Parser.
            let consumed = pl.to_bytes(little_endian).len();
            pos += consumed;
            Some(pl)
        } else {
            None
        };

        // Rest ist serializedPayload.
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
/// HEARTBEAT Flag: G (GroupInfo present). Vendor-Extension fuer
/// Group-Ordered-Access (§8.3.8.6.2). Trailer enthaelt currentGSN,
/// firstGSN, lastGSN und einen `writerSet` (Liste der GUID-Prefixe der
/// Gruppen-Member).
pub const HEARTBEAT_FLAG_GROUP_INFO: u8 = 0x08;

/// Optionaler GroupInfo-Trailer einer HEARTBEAT-Submessage (§8.3.8.6.2).
///
/// Wire-Layout:
/// - currentGSN: i64
/// - firstGSN:   i64
/// - lastGSN:    i64
/// - writerSet:  u32-Length + length × GuidPrefix(12 byte)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatGroupInfo {
    /// Aktuelle Group-SN (hoechste vom Gruppen-Coordinator vergebene).
    pub current_gsn: SequenceNumber,
    /// Erste relevante Group-SN (cache_min der Gruppe).
    pub first_gsn: SequenceNumber,
    /// Letzte verfuegbare Group-SN (= currentGSN minus pending, in der
    /// Praxis identisch zu currentGSN bei Steady-State).
    pub last_gsn: SequenceNumber,
    /// GuidPrefix-Set der teilnehmenden Writer dieser Gruppe.
    pub writer_set: Vec<crate::wire_types::GuidPrefix>,
}

/// HEARTBEAT-Submessage.
///
/// `final_flag`, `liveliness_flag` und `group_info_flag` (via `Some` von
/// `group_info`) entsprechen den F-/L-/G-Bits im Submessage-Header
/// (Spec §8.3.7.5.1, §8.3.8.6.2) — sie liegen **nicht** im Body, werden
/// aber als semantischer Teil der Nachricht hier gefuehrt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatSubmessage {
    /// Reader-EntityId (Ziel).
    pub reader_id: EntityId,
    /// Writer-EntityId (Quelle).
    pub writer_id: EntityId,
    /// Erste verfuegbare Sequence-Number im History-Cache.
    pub first_sn: SequenceNumber,
    /// Letzte gesendete Sequence-Number.
    pub last_sn: SequenceNumber,
    /// Count_t (i32) — Heartbeat-Sequenznummer (zur ACK-Korrelation).
    pub count: i32,
    /// F-Flag: `true` = Reader muss keine Response senden wenn vollstaendig.
    pub final_flag: bool,
    /// L-Flag: Liveliness-Announce (ohne History-Semantik).
    pub liveliness_flag: bool,
    /// G-Flag (§8.3.8.6.2): optionaler GroupInfo-Trailer.
    pub group_info: Option<HeartbeatGroupInfo>,
}

impl HeartbeatSubmessage {
    /// Minimal-Wire-Size (Body ohne GroupInfo): 28 Bytes (4+4+8+8+4).
    /// Flags sind im Submessage-Header.
    pub const WIRE_SIZE: usize = 28;

    /// Encoded den Body. Liefert (bytes, flags), wobei `flags` das
    /// Submessage-Header-Flag-Byte inkl. E/F/L/G enthaelt.
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

    /// Decoded den Body. `final_flag`, `liveliness_flag`, `group_info_flag`
    /// werden vom Caller aus dem Submessage-Header extrahiert.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` (writerSet-Laenge bizarr gross).
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
            // Cap: writer_set darf nicht groesser sein als der Body uebrig
            // hat. Schutz gegen DoS via riesigem length-Feld.
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

/// ACKNACK-Submessage.
///
/// `final_flag` entspricht dem F-Bit im Submessage-Header (Spec
/// §8.3.7.1.1). `final=false` verlangt vom Writer eine zeitnahe
/// HEARTBEAT-Response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckNackSubmessage {
    /// Reader-EntityId (Quelle).
    pub reader_id: EntityId,
    /// Writer-EntityId (Ziel).
    pub writer_id: EntityId,
    /// Bitset der noch nicht empfangenen Sequence-Numbers.
    pub reader_sn_state: SequenceNumberSet,
    /// Count_t (zur Korrelation mit HEARTBEAT.count).
    pub count: i32,
    /// F-Flag: `false` = Writer soll zeitnah mit HEARTBEAT antworten.
    pub final_flag: bool,
}

impl AckNackSubmessage {
    /// Encoded den Body. Liefert (bytes, flags).
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::new();
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

    /// Decoded den Body. `final_flag` wird vom Caller aus dem
    /// Submessage-Header extrahiert.
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
/// Vendor-Extension fuer Group-Ordered-Access (§8.3.8.4.2). ZeroDDS-Encoder setzt das nicht; der Decoder akzeptiert es lesend.
pub const GAP_FLAG_GROUP_INFO: u8 = 0x04;

/// GAP Flag: K (FilteredCount present). Spec §8.3.8.4.2 fuehrt ein
/// optionales `Count_t filteredCount`-Trailer-Feld ein, das dem Reader
/// erlaubt, "via Content-Filter weggeworfen" von "echt removed" zu
/// unterscheiden — Voraussetzung fuer korrekte Instance-State-Transitionen
/// nach §8.7.4 (NOT_ALIVE_FILTERED vs. NOT_ALIVE_DISPOSED).
pub const GAP_FLAG_FILTERED_COUNT: u8 = 0x08;

/// Optionaler Trailer einer GAP-Submessage mit GroupInfo (G-Flag, §8.3.8.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GapGroupInfo {
    /// Group-SN des ersten ueberprungenen Samples in der Gruppe.
    pub gap_start_gsn: SequenceNumber,
    /// Group-SN des letzten ueberprungenen Samples in der Gruppe.
    pub gap_end_gsn: SequenceNumber,
}

/// GAP-Submessage. Signalisiert Reader, dass Writer Sequence-Numbers
/// `[gap_start, gap_list.bitmap_base)` nie senden wird (alle vor
/// `gap_list.bitmap_base` Lücken; die Bits in `gap_list` markieren
/// individuelle weitere Lücken ab `bitmap_base`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapSubmessage {
    /// Reader-EntityId (Ziel).
    pub reader_id: EntityId,
    /// Writer-EntityId (Quelle).
    pub writer_id: EntityId,
    /// Erste irreversible Lücken-SN.
    pub gap_start: SequenceNumber,
    /// Bitset der weiteren Lücken ab `gap_list.bitmap_base`.
    pub gap_list: SequenceNumberSet,
    /// Optionale GroupInfo (§8.3.8.4.2). `Some` ⇒ G-Flag im Header gesetzt.
    pub group_info: Option<GapGroupInfo>,
    /// Optionaler `filteredCount`-Trailer (§8.3.8.4.2). `Some` ⇒
    /// K-Flag im Header gesetzt. `0` ist explizit "nichts gefiltert,
    /// alles wirklich removed"; `1+` heisst "n Samples via Content-
    /// Filter verworfen".
    pub filtered_count: Option<u32>,
}

impl GapSubmessage {
    /// Encoded den Body. Liefert (bytes, flags) inkl. evtl. G-/K-Bit.
    #[must_use]
    pub fn write_body(&self, little_endian: bool) -> (Vec<u8>, u8) {
        let mut out = Vec::new();
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

    /// Decoded den Body. Flags G/K werden aus dem Submessage-Header vom
    /// Caller uebergeben (siehe `decode_datagram`).
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
/// DATA_FRAG Flag: K (key flag — serialized_payload ist Key statt Data).
pub const DATA_FRAG_FLAG_KEY: u8 = 0x08;
/// DATA_FRAG Flag: N (non-standard payload).
pub const DATA_FRAG_FLAG_NON_STANDARD: u8 = 0x10;

/// DATA_FRAG-Submessage. Traegt einen Ausschnitt (Fragmente) eines
/// Samples, dessen Gesamtgroesse in `sample_size` steht.
///
/// Flags (Q/H/K/N) werden aus dem Submessage-Header gespiegelt. Encoder setzt diese Flags aktuell nicht; Decoder akzeptiert sie lesend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFragSubmessage {
    /// octetsToInlineQos-Analog zu DATA (§8.3.7.2 spricht von extraFlags+
    /// octetsToInlineQos; Variante traegt 0).
    pub extra_flags: u16,
    /// Reader-EntityId (Ziel).
    pub reader_id: EntityId,
    /// Writer-EntityId (Quelle).
    pub writer_id: EntityId,
    /// Sequence-Number des Samples, dessen Fragmente dies traegt.
    pub writer_sn: SequenceNumber,
    /// Erstes Fragment in dieser Submessage (1-basiert).
    pub fragment_starting_num: FragmentNumber,
    /// Anzahl der Fragmente in dieser Submessage. Writer: immer 1.
    pub fragments_in_submessage: u16,
    /// Groesse eines einzelnen Fragments (das letzte darf kuerzer sein).
    pub fragment_size: u16,
    /// Gesamtgroesse des Samples in Bytes.
    pub sample_size: u32,
    /// Fragmentierter Payload-Ausschnitt. Arc-shared:
    /// Writer-Re-Sends bauen nur Refcount-Bumps, keine Kopie.
    pub serialized_payload: Arc<[u8]>,
    /// Q-Flag aus dem Submessage-Header (inline_qos present).
    pub inline_qos_flag: bool,
    /// H-Flag aus dem Submessage-Header (hash_key).
    pub hash_key_flag: bool,
    /// K-Flag aus dem Submessage-Header (serialized_payload = Key).
    pub key_flag: bool,
    /// N-Flag aus dem Submessage-Header (non-standard payload).
    pub non_standard_flag: bool,
}

impl DataFragSubmessage {
    /// Minimal-Body-Size ohne Payload: extraFlags(2) + octetsToInlineQos(2)
    /// + readerId(4) + writerId(4) + writerSN(8) + fragmentStartingNum(4)
    /// + fragmentsInSubmessage(2) + fragmentSize(2) + sampleSize(4) = 32.
    pub const HEADER_WIRE_SIZE: usize = 32;

    /// octetsToInlineQos: Offset vom Ende dieses Felds bis zum Beginn
    /// von inlineQos bzw. serializedPayload. Variante mit
    /// Q=false: Offset = 28 (readerId..sampleSize).
    pub const OCTETS_TO_INLINE_QOS: u16 = 28;

    /// Encoded den Body. Liefert (bytes, flags).
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

    /// Decoded den Body. Flags werden aus dem Submessage-Header vom
    /// Caller uebergeben.
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
        // extra_flags: siehe DATA. 2.1-Reader muessen ignorieren
        // (Cyclone + Fast-DDS machen das), wir ebenfalls — nur lesen
        // und weiterreichen.
        // octetsToInlineQos (2 Byte): offset von Anfang readerId bis zum
        // Inline-QoS bzw. (bei Q=false) bis zum serializedPayload. Spec
        // fordert 28 = Header-Size − 2 (extra_flags) − 2 (dieses Feld).
        // Abweichung bei Q=false fangen wir hier ab — haeufiger Interop-
        // Bug, den wir spec-treu ablehnen.
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
        // Q-Flag = false, daher kein Inline-QoS-Block.
        // Bei Q=true wuerden hier ParameterList-Bytes folgen — aktuell
        // akzeptieren wir das nicht.
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
// table) bzw. 0x0A im 2.5-PSM. Wir folgen 2.5: id=0x0A.
// ============================================================================

/// InfoSource-Submessage (§8.3.8.9.4). Setzt im ReceiverState
/// `sourceProtocolVersion`, `sourceVendorId`, `sourceGuidPrefix` neu —
/// alle nachfolgenden Submessages werden dieser Quelle zugeordnet (nicht
/// dem Datagram-Header).
///
/// Wire-Layout (Body, 20 byte):
/// - unused (4 byte, in der Spec "Long unused")
/// - ProtocolVersion (2 byte)
/// - VendorId (2 byte)
/// - GuidPrefix (12 byte)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoSourceSubmessage {
    /// Reservierte 4 Byte (Spec sagt: muss vom Sender 0 sein, vom
    /// Receiver ignoriert).
    pub unused: u32,
    /// Source-ProtocolVersion (z.B. 2.5).
    pub protocol_version: crate::wire_types::ProtocolVersion,
    /// Source-VendorId (Hersteller-Kennung).
    pub vendor_id: crate::wire_types::VendorId,
    /// Source-GuidPrefix (12 byte).
    pub guid_prefix: crate::wire_types::GuidPrefix,
}

impl InfoSourceSubmessage {
    /// Wire-Size: 20 Bytes (4+2+2+12).
    pub const WIRE_SIZE: usize = 20;

    /// Encoded den Body. Liefert (bytes, flags) inkl. E-Bit.
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

/// InfoTimestamp Flag: I (Invalidate). Wenn gesetzt: Body ist leer und
/// `haveTimestamp` wird im ReceiverState auf `false` gesetzt.
pub const INFO_TIMESTAMP_FLAG_INVALIDATE: u8 = 0x02;

/// InfoTimestamp-Submessage (§8.3.7.5 / §8.3.8.5). Setzt im
/// ReceiverState das `timestamp`-Feld + `haveTimestamp` Flag.
/// Wird via `INFO_TIMESTAMP_FLAG_INVALIDATE` invertiert (I-Flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InfoTimestampSubmessage {
    /// Timestamp (8 byte: i32 sec + u32 fraction). Wenn `invalidate=true`
    /// ignoriert.
    pub timestamp: crate::header_extension::HeTimestamp,
    /// `true` = I-Flag in der Submessage gesetzt → Body leer und der
    /// Receiver setzt `haveTimestamp = false`.
    pub invalidate: bool,
}

impl InfoTimestampSubmessage {
    /// Encoded den Body. Wenn `invalidate=true`: Body leer (0 byte).
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

    /// Decoded den Body. Wenn `invalidate_flag=true`: erwartet einen
    /// leeren Body und liefert `timestamp = default()`.
    ///
    /// # Errors
    /// `UnexpectedEof` wenn `invalidate_flag=false` und Body < 8 byte.
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

/// InfoReply Flag: M (Multicast). Wenn gesetzt: zweite LocatorList
/// (multicastReplyLocatorList) folgt im Body.
pub const INFO_REPLY_FLAG_MULTICAST: u8 = 0x02;

/// InfoReply-Submessage (§8.3.8.10.4). Setzt im ReceiverState
/// `unicastReplyLocatorList` (Pflicht) und ggf.
/// `multicastReplyLocatorList` (wenn M-Flag).
///
/// Wire-Layout (Body):
/// - unicastLocatorList: u32 length + N × 24 byte Locator
/// - (M-Flag) multicastLocatorList: u32 length + N × 24 byte Locator
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoReplySubmessage {
    /// Unicast-Reply-Locators (mind. 1 sinnvoll, leere Liste erlaubt).
    pub unicast_locators: Vec<crate::wire_types::Locator>,
    /// Multicast-Reply-Locators (`Some` ⇒ M-Flag im Header gesetzt).
    pub multicast_locators: Option<Vec<crate::wire_types::Locator>>,
}

impl InfoReplySubmessage {
    /// Encoded den Body. Liefert (bytes, flags) inkl. E- und ggf. M-Bit.
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
            // Locator hat ein eigenes Wire-Format (24 byte). Wir nehmen
            // hier den LE-Pfad — Locator ist im RTPS immer LE in den
            // ParameterList-Pfaden, fuer Submessage-Body folgen wir der
            // Submessage-Endianness.
            if little_endian {
                out.extend_from_slice(&loc.to_bytes_le());
            } else {
                // BE-Variante: kind (4 byte BE), port (4 byte BE), addr (16 byte raw)
                out.extend_from_slice(&(loc.kind.as_i32()).to_be_bytes());
                out.extend_from_slice(&loc.port.to_be_bytes());
                out.extend_from_slice(&loc.address);
            }
        }
    }

    /// Decoded den Body. M-Flag wird vom Caller aus dem Submessage-Header
    /// extrahiert.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` (Locator-Length bizarr gross).
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
            // BE-Decode: bauen das Locator manuell, da from_bytes_le
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

/// HEARTBEAT_FRAG-Submessage. Schickt der Writer, um den Reader zu
/// informieren, dass Fragmente bis `last_fragment_num` fuer `writer_sn`
/// verfuegbar sind. Writer sendet diese nicht; der Decoder wird
/// dennoch bereitgehalten fuer Interop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatFragSubmessage {
    /// Reader-EntityId (Ziel).
    pub reader_id: EntityId,
    /// Writer-EntityId (Quelle).
    pub writer_id: EntityId,
    /// Zugehoeriges Sample.
    pub writer_sn: SequenceNumber,
    /// Hoechste verfuegbare FragmentNumber.
    pub last_fragment_num: FragmentNumber,
    /// Count_t (zur Korrelation mit NACK_FRAG).
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

/// NACK_FRAG-Submessage. Reader meldet fehlende Fragmente fuer einen
/// bestimmten `writer_sn`. Auf der Wire keine Flags ausser E.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NackFragSubmessage {
    /// Reader-EntityId (Quelle).
    pub reader_id: EntityId,
    /// Writer-EntityId (Ziel).
    pub writer_id: EntityId,
    /// Zugehoeriges Sample.
    pub writer_sn: SequenceNumber,
    /// Bitset fehlender Fragmente.
    pub fragment_number_state: FragmentNumberSet,
    /// Count_t (zur Korrelation).
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
        // numBits=64 → 8 byte bitmap erwartet; nur 4 vorhanden.
        let mut buf = Vec::new();
        buf.extend_from_slice(&SequenceNumber(0).to_bytes_le());
        buf.extend_from_slice(&64_u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 4]); // nur 4 statt 8
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
        // Spec §8.3.8.2 K-Flag: serialized_payload enthaelt nur Key.
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
        // E + Q + D + K + N alle gesetzt — der volle 5-Flag-Roundtrip.
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

    // ---- WP 1.E Stufe-D: HEARTBEAT GroupInfo ----

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
        // kein Body fuer prefixes
        let res = HeartbeatSubmessage::read_body(&body, true, false, false, true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn heartbeat_decode_rejects_truncated_group_info() {
        // Body endet vor den 3 GSN-Feldern
        let mut body = Vec::new();
        body.extend_from_slice(&reader_id().to_bytes());
        body.extend_from_slice(&writer_id().to_bytes());
        body.extend_from_slice(&SequenceNumber(1).to_bytes_le());
        body.extend_from_slice(&SequenceNumber(1).to_bytes_le());
        body.extend_from_slice(&1i32.to_le_bytes());
        // GroupInfo-Trailer fehlt → UnexpectedEof
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

    // ---- WP 1.E Stufe-C: GAP filteredCount + GroupInfo ----

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
        // filtered_count=Some(0) heisst "K-Flag gesetzt, aber 0 gefiltert"
        // (= alles wirklich removed). None = Trailer fehlt komplett.
        // Beide muessen sich roundtripen lassen.
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
        // Body endet vor filtered_count → UnexpectedEof
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
        // Decoder mit filtered_count_flag=true erwartet 4 byte Trailer
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
        // sample_size=10, fragment_size=4, fragment 3 traegt nur 2 Byte
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
        // B3-Recherche: Cyclone/Fast-DDS ignorieren non-zero extra_flags.
        // Wir auch.
        let d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        let (mut bytes, _) = d.write_body(true);
        bytes[0..2].copy_from_slice(&0x0042u16.to_le_bytes()); // extra_flags nonzero
        let decoded =
            DataFragSubmessage::read_body(&bytes, true, false, false, false, false).unwrap();
        assert_eq!(decoded.extra_flags, 0x0042);
    }

    #[test]
    fn seqnumset_rejects_num_bits_above_256() {
        // B7: Hard-Cap gegen DoS via riesiger Bitmap.
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
        buf.extend_from_slice(&1000u32.to_le_bytes()); // num_bits weit ueber 256
        let res = FragmentNumberSet::read_from(&buf, 0, true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn fnset_dos_giant_num_bits_rejected_before_alloc() {
        // Pathologisch: num_bits = u32::MAX wuerde ~512 MB allokieren
        // wenn wir nicht vorher cappen.
        let mut buf = Vec::new();
        buf.extend_from_slice(&FragmentNumber(1).to_bytes_le());
        buf.extend_from_slice(&u32::MAX.to_le_bytes());
        let res = FragmentNumberSet::read_from(&buf, 0, true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn data_frag_decode_rejects_wrong_octets_to_inline_qos_when_q_false() {
        // Wir basteln einen DATA_FRAG-Body mit falschem
        // octetsToInlineQos=99 und Q=false. Decoder muss das ablehnen.
        let d = dataflag_frag(1, 1, 1, 4, 4, vec![1, 2, 3, 4]);
        let (mut bytes, _) = d.write_body(true);
        // octetsToInlineQos sitzt in Bytes [2..4] (nach extra_flags).
        bytes[2..4].copy_from_slice(&99u16.to_le_bytes());
        let res = DataFragSubmessage::read_body(&bytes, true, false, false, false, false);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn data_frag_decode_rejects_inline_qos() {
        // Q-Flag true wird abgelehnt (Feature nicht implementiert).
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

    // ---- WP 1.E Stufe-E: InfoSource ----

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
        // tragen dort Diagnose-Daten ein.
        let mut i = make_info_source();
        i.unused = 0xDEAD_BEEF;
        let (bytes, _) = i.write_body(true);
        let decoded = InfoSourceSubmessage::read_body(&bytes, true).unwrap();
        assert_eq!(decoded.unused, 0xDEAD_BEEF);
    }

    // ---- WP 1.E Stufe-F: InfoReply ----

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
        // Spec erlaubt leere Liste (z.B. "vergiss alle bisherigen
        // Reply-Locators"). Decoder muss das akzeptieren.
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
        // kein Locator-Body
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
        assert!(bytes.is_empty(), "I-Flag → leerer Body");
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
        // I-Flag → Body wird ignoriert; auch wenn er voll ist, gilt
        // invalidate=true.
        let res = InfoTimestampSubmessage::read_body(&[0u8; 8], true, true).unwrap();
        assert!(res.invalidate);
        assert_eq!(
            res.timestamp,
            crate::header_extension::HeTimestamp::default()
        );
    }
}
