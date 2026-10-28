// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! HeaderExtension Submessage (DDSI-RTPS 2.5 §8.3.3.2 / §9.4.5.2 /
//! §9.4.2.15).
//!
//! The HeaderExtension (HE) is a submessage class newly introduced in
//! DDSI-RTPS 2.5, which appends optional header
//! fields to an RTPS message:
//!
//! - **`messageLength`**: u32 — "the length of the complete RTPS Message,
//!   starting from the beginning of the RTPS Header" (§8.3.3.2). Allows
//!   reader pre-allocation and is a mandatory hint for non-UDP transports
//!   without an implicit datagram boundary.
//! - **`rtpsSendTimestamp`**: 8 bytes (Time_t) — sender wallclock at the
//!   time of the `send()` call. Feeds `clockSkewDetected` in the
//!   receiver state (§8.3.7.4).
//! - **`uextension4`**: 4 bytes vendor-specific.
//! - **`wextension8`**: 8 bytes vendor-specific (Spec §8.3.3.2).
//! - **`messageChecksum`**: CRC-32C (4 bytes), CRC-64-XZ (8 bytes) or
//!   MD5-128 (16 bytes) over the range `[RtpsHeader || all
//!   submessages || HE submessage header || HE body up to before the
//!   checksum field]`. The C flag (2 bits) selects the variant.
//! - **`parameters`**: ParameterList (TLV) — so far reserved only for
//!   vendor extensions; a `must_understand` within it triggers a
//!   whole-message reject (§9.4.2.11.2).
//!
//! # SubmessageId and flag layout (commissioned specification)
//!
//! - `SubmessageId = 0x00` — `RTPS_HE` (§9.4.5.1.1). A pre-2.5
//!   implementation takes it for an unknown submessage and skips it by
//!   `octets_to_next_header` (§8.3.3.2).
//!
//! Flag bits in submessage header:
//!
//! | Bit | Symbol | Meaning |
//! |-----|--------|-----------|
//! | 0 | E | Endianness (1 = LE) |
//! | 1 | L | `messageLength` present |
//! | 2 | T | `rtpsSendTimestamp` present |
//! | 3 | U | `uextension4` present |
//! | 4 | W | `wextension8` present |
//! | 5+6 | C | checksum variant (0=none, 1=CRC-32C, 2=CRC-64, 3=MD5) |
//! | 7 | P | `parameters` (ParameterList) present |
//!
//! # Receiver state update (§8.3.7.4)
//!
//! On receipt of an HE the receiver updates:
//!
//! - `Receiver.messageLength` from the L field (if present). A value too
//!   small to fit the RTPS Header and the HeaderExtension makes the
//!   HeaderExtension invalid (§8.3.7.3).
//! - `Receiver.rtpsSendTimestamp = …` from the T field (if present); not
//!   `haveTimestamp`/`timestamp`, which only an InfoTimestamp sets.
//! - `Receiver.messageChecksum = …` from the C field (if variant ≠ 0).
//! - `Receiver.parameters = …` from the P field (if present).
//! - `clockSkewDetected` heuristic: if `|Receiver.timestamp - now|` >
//!   threshold. (This heuristic stays in the receiver code; the
//!   decode module only provides the input.)
//!
//! # DoS-Cap
//!
//! `MAX_HE_LENGTH = 16 KiB`: the HE body must not be longer than 16 KiB
//! (covers a typical ParameterList + all optional fields).
//! Prevents amplification via maliciously large `parameters`.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::parameter_list::ParameterList;

/// SubmessageId for HeaderExtension: DDSI-RTPS 2.5 §9.4.5.1.1,
/// "`@value(0x00) RTPS_HE, /* HeaderExtension */`". 0x80 and above are
/// vendor-specific ("Submessages with ID's 0x80 to 0xff (inclusive) are
/// vendor-specific").
pub const SUBMESSAGE_ID_HEADER_EXTENSION: u8 = 0x00;

/// Hard cap for the HE body length (DoS protection).
pub const MAX_HE_LENGTH: usize = 16 * 1024;

// ----- Flag-Bits -----

/// E flag (bit 0) — endianness of the body (1 = LE).
pub const HE_FLAG_E: u8 = 1 << 0;
/// L flag (bit 1) — `messageLength` present.
pub const HE_FLAG_L: u8 = 1 << 1;
/// T flag (bit 2, TimestampFlag, DDSI-RTPS 2.5 §9.4.5.2) —
/// `rtpsSendTimestamp` present.
pub const HE_FLAG_T: u8 = 1 << 2;
/// U flag (bit 3) — `uextension4` present.
pub const HE_FLAG_U: u8 = 1 << 3;
/// W flag (bit 4, WExtension8Flag, DDSI-RTPS 2.5 §9.4.5.2) — `wextension8`
/// present.
pub const HE_FLAG_W: u8 = 1 << 4;
/// Lower bit of the C mask (bit 5).
pub const HE_FLAG_C0: u8 = 1 << 5;
/// Higher bit of the C mask (bit 6).
pub const HE_FLAG_C1: u8 = 1 << 6;
/// Mask of the two checksum selector bits (bits 5+6).
pub const HE_FLAG_C_MASK: u8 = HE_FLAG_C0 | HE_FLAG_C1;
/// P flag (bit 7) — `parameters` (ParameterList) present.
pub const HE_FLAG_P: u8 = 1 << 7;

// =====================================================================
// Wire types
// =====================================================================

/// Spec §9.4.2.15.2 — selection of the `messageChecksum` variant. The value
/// is in the C bits (5+6) of the flag byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumKind {
    /// `C = 00` — no checksum.
    None,
    /// `C = 01` — CRC-32C (RFC 4960 App. B), 4 Byte.
    Crc32c,
    /// `C = 10` — CRC-64-XZ (ECMA-182, XZ utils), 8 Byte.
    Crc64,
    /// `C = 11` — MD5-128 (RFC 1321), 16 Byte.
    Md5,
}

impl ChecksumKind {
    /// Wire size in bytes (0/4/8/16).
    #[must_use]
    pub fn wire_size(self) -> usize {
        match self {
            Self::None => 0,
            Self::Crc32c => 4,
            Self::Crc64 => 8,
            Self::Md5 => 16,
        }
    }

    /// Extract from the C bits of a flag byte.
    #[must_use]
    pub fn from_flags(flags: u8) -> Self {
        // Masking guarantees values 0..=3.
        match (flags & HE_FLAG_C_MASK) >> 5 {
            1 => Self::Crc32c,
            2 => Self::Crc64,
            3 => Self::Md5,
            // 0 or impossible upper bits: no checksum.
            _ => Self::None,
        }
    }

    /// Encode into the C bits of a flag byte.
    #[must_use]
    pub fn to_flag_bits(self) -> u8 {
        let raw = match self {
            Self::None => 0u8,
            Self::Crc32c => 1,
            Self::Crc64 => 2,
            Self::Md5 => 3,
        };
        (raw << 5) & HE_FLAG_C_MASK
    }
}

/// Content of the `messageChecksum` (variable per [`ChecksumKind`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ChecksumValue {
    /// No checksum field written.
    #[default]
    None,
    /// CRC-32C u32.
    Crc32c(u32),
    /// CRC-64-XZ u64.
    Crc64(u64),
    /// MD5-128 16-byte hash.
    Md5([u8; 16]),
}

impl ChecksumValue {
    /// Associated [`ChecksumKind`].
    #[must_use]
    pub fn kind(&self) -> ChecksumKind {
        match self {
            Self::None => ChecksumKind::None,
            Self::Crc32c(_) => ChecksumKind::Crc32c,
            Self::Crc64(_) => ChecksumKind::Crc64,
            Self::Md5(_) => ChecksumKind::Md5,
        }
    }

    /// Computes the checksum of `payload` with the desired algorithm
    /// (DDSI-RTPS 2.5 Table 9.8).
    ///
    /// The `messageChecksum` of a message is the checksum of the whole RTPS
    /// message with the checksum field set to zero (§8.3.7.4, §9.4.2.15):
    /// [`HeaderExtension::message_checksum`] does that.
    ///
    /// Implemented via [`zerodds_foundation::crc32c`],
    /// [`zerodds_foundation::crc64_xz`] and [`zerodds_foundation::md5`] —
    /// pure-Rust no_std hashes without an external crypto-crate
    /// dependency (Pillar 9 zero-dependency).
    #[must_use]
    pub fn compute(kind: ChecksumKind, payload: &[u8]) -> Self {
        match kind {
            ChecksumKind::None => Self::None,
            ChecksumKind::Crc32c => Self::Crc32c(zerodds_foundation::crc32c(payload)),
            ChecksumKind::Crc64 => Self::Crc64(zerodds_foundation::crc64_xz(payload)),
            ChecksumKind::Md5 => Self::Md5(zerodds_foundation::md5(payload)),
        }
    }

    /// Verifies that the checksum held in this value matches the one
    /// computed over `payload`.
    ///
    /// Returns `true` if the algorithm is active AND the value
    /// matches; `true` also if `kind() == None` (no checksum
    /// declared, nothing to verify); `false` on mismatch.
    ///
    /// Spec §9.4.2.15.2: the receiver discards the message on mismatch.
    #[must_use]
    pub fn verify(&self, payload: &[u8]) -> bool {
        let computed = Self::compute(self.kind(), payload);
        computed == *self
    }
}

/// `MESSAGE_LENGTH_INVALID` (DDSI-RTPS 2.5 §9.3.2.7): the value of a
/// `MessageLength_t` that names no length, 0. It is the initial
/// `messageLength` of the receiver (Table 8.17; [`crate::receiver_state::ReceiverState::message_length`]
/// is `None` for it), and a HeaderExtension that announces it is invalid,
/// as any `messageLength` below the RTPS header and the HeaderExtension
/// itself is (§8.3.7.3).
pub const MESSAGE_LENGTH_INVALID: u32 = 0;

/// The RTPS `Time_t` (DDSI-RTPS 2.5 §9.3.2.1): an unsigned 32-bit second
/// count and a fraction in units of 2⁻³² s, each written in the byte order
/// of its submessage. Octet for octet the same as the signed `long seconds`
/// of the wire diagram of §9.4.2.9; read as unsigned, as the IDL of
/// §9.3.2.1 types it, the seconds reach the year 2106 and the reserved
/// values are the plain {0xffffffff, ...}.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HeTimestamp {
    /// Seconds since the Unix epoch.
    pub seconds: u32,
    /// Fraction of a second in units of 2⁻³² s (DDSI-RTPS 2.5 §9.3.2.2:
    /// `time = seconds + fraction / 2^32`).
    pub fraction: u32,
}

impl HeTimestamp {
    /// `TIME_INVALID` of DDSI-RTPS 2.5 §9.3.2.1 ({0xffffffff, 0xffffffff}),
    /// the initial `timestamp` of the receiver (Table 8.17).
    pub const TIME_INVALID: Self = Self {
        seconds: 0xFFFF_FFFF,
        fraction: 0xFFFF_FFFF,
    };

    /// `TIME_INFINITE` of DDSI-RTPS 2.5 §9.3.2.1 ({0xffffffff, 0xfffffffe}).
    pub const TIME_INFINITE: Self = Self {
        seconds: 0xFFFF_FFFF,
        fraction: 0xFFFF_FFFE,
    };
}

/// Parsed HeaderExtension submessage.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeaderExtension {
    /// Endianness of the body (`true` = little-endian).
    pub little_endian: bool,
    /// If the L flag is set: remaining length of the message.
    pub message_length: Option<u32>,
    /// If the T flag is set: sender timestamp.
    pub timestamp: Option<HeTimestamp>,
    /// If the U flag is set: 4-byte vendor-specific value.
    pub uextension4: Option<[u8; 4]>,
    /// If the W flag is set: 8-octet `wExtension8`.
    pub wextension8: Option<[u8; 8]>,
    /// If the C flag != 0: checksum value.
    pub checksum: ChecksumValue,
    /// If the P flag is set: ParameterList.
    pub parameters: Option<ParameterList>,
}

impl HeaderExtension {
    /// Empty HE (all optional fields absent). Manipulate the fields
    /// directly in builder style.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Computes the flag byte (incl. E bit) from the set fields.
    #[must_use]
    pub fn flag_byte(&self) -> u8 {
        let mut f = 0u8;
        if self.little_endian {
            f |= HE_FLAG_E;
        }
        if self.message_length.is_some() {
            f |= HE_FLAG_L;
        }
        if self.timestamp.is_some() {
            f |= HE_FLAG_T;
        }
        if self.uextension4.is_some() {
            f |= HE_FLAG_U;
        }
        if self.wextension8.is_some() {
            f |= HE_FLAG_W;
        }
        f |= self.checksum.kind().to_flag_bits();
        if self.parameters.is_some() {
            f |= HE_FLAG_P;
        }
        f
    }

    /// Encodes the body **without** the submessage header. The body length is
    /// by construction ≤ [`MAX_HE_LENGTH`].
    ///
    /// Order: L → W → U → V → C → P, per Spec §9.4.5.2.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the body would be > `u16::MAX` or
    /// > `MAX_HE_LENGTH`.
    pub fn encode_body(&self) -> Result<Vec<u8>, WireError> {
        let le = self.little_endian;
        let mut body = Vec::with_capacity(64);

        if let Some(len) = self.message_length {
            write_u32(&mut body, len, le);
        }
        if let Some(ts) = self.timestamp {
            write_u32(&mut body, ts.seconds, le);
            write_u32(&mut body, ts.fraction, le);
        }
        if let Some(u) = self.uextension4 {
            body.extend_from_slice(&u);
        }
        if let Some(v) = self.wextension8 {
            body.extend_from_slice(&v);
        }
        match self.checksum {
            ChecksumValue::None => {}
            // Table 9.8: the result "shall be serialized … using CDR big
            // endian encoding", whatever the byte order of the submessage.
            ChecksumValue::Crc32c(c) => body.extend_from_slice(&c.to_be_bytes()),
            ChecksumValue::Crc64(c) => body.extend_from_slice(&c.to_be_bytes()),
            ChecksumValue::Md5(m) => body.extend_from_slice(&m),
        }
        if let Some(pl) = &self.parameters {
            // Append ParameterList in submessage endianness.
            body.extend_from_slice(&pl.to_bytes(le));
        }
        if body.len() > MAX_HE_LENGTH {
            return Err(WireError::ValueOutOfRange {
                message: "HeaderExtension body exceeds MAX_HE_LENGTH",
            });
        }
        Ok(body)
    }

    /// Encodes the HE as a complete submessage = 4-byte SubmessageHeader
    /// + body. `octets_to_next_header` is set to the body length.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the body length > `u16::MAX`.
    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        let body = self.encode_body()?;
        let body_len = u16::try_from(body.len()).map_err(|_| WireError::ValueOutOfRange {
            message: "HE body exceeds u16::MAX",
        })?;
        let mut out = Vec::with_capacity(4 + body.len());
        out.push(SUBMESSAGE_ID_HEADER_EXTENSION);
        out.push(self.flag_byte());
        let len_bytes = if self.little_endian {
            body_len.to_le_bytes()
        } else {
            body_len.to_be_bytes()
        };
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(&body);
        Ok(out)
    }

    /// Decodes the HE body from a slice of length
    /// `octets_to_next_header`, given the flag byte from the
    /// submessage header.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange` (body larger than
    /// `MAX_HE_LENGTH`).
    pub fn decode_body(body: &[u8], flags: u8) -> Result<Self, WireError> {
        if body.len() > MAX_HE_LENGTH {
            return Err(WireError::ValueOutOfRange {
                message: "HE body exceeds MAX_HE_LENGTH",
            });
        }
        let le = (flags & HE_FLAG_E) != 0;
        let mut pos = 0usize;
        let need = |n: usize, p: usize| -> Result<(), WireError> {
            if body.len() < p + n {
                Err(WireError::UnexpectedEof {
                    needed: n,
                    offset: p,
                })
            } else {
                Ok(())
            }
        };

        let message_length = if (flags & HE_FLAG_L) != 0 {
            need(4, pos)?;
            let v = read_u32(&body[pos..pos + 4], le);
            pos += 4;
            Some(v)
        } else {
            None
        };
        let timestamp = if (flags & HE_FLAG_T) != 0 {
            need(8, pos)?;
            let s = read_u32(&body[pos..pos + 4], le);
            let f = read_u32(&body[pos + 4..pos + 8], le);
            pos += 8;
            Some(HeTimestamp {
                seconds: s,
                fraction: f,
            })
        } else {
            None
        };
        let uextension4 = if (flags & HE_FLAG_U) != 0 {
            need(4, pos)?;
            let mut u = [0u8; 4];
            u.copy_from_slice(&body[pos..pos + 4]);
            pos += 4;
            Some(u)
        } else {
            None
        };
        let wextension8 = if (flags & HE_FLAG_W) != 0 {
            need(8, pos)?;
            let mut v = [0u8; 8];
            v.copy_from_slice(&body[pos..pos + 8]);
            pos += 8;
            Some(v)
        } else {
            None
        };

        let kind = ChecksumKind::from_flags(flags);
        let checksum = match kind {
            ChecksumKind::None => ChecksumValue::None,
            ChecksumKind::Crc32c => {
                need(4, pos)?;
                let v = read_u32(&body[pos..pos + 4], false);
                pos += 4;
                ChecksumValue::Crc32c(v)
            }
            ChecksumKind::Crc64 => {
                need(8, pos)?;
                let v = read_u64(&body[pos..pos + 8], false);
                pos += 8;
                ChecksumValue::Crc64(v)
            }
            ChecksumKind::Md5 => {
                need(16, pos)?;
                let mut m = [0u8; 16];
                m.copy_from_slice(&body[pos..pos + 16]);
                pos += 16;
                ChecksumValue::Md5(m)
            }
        };

        // The ParameterList takes the rest of the body and need not end in
        // a PID_SENTINEL (§9.4.2.11). Without one, what follows the known
        // fields is skipped: "Additional fields can be added to the end of
        // a Submessage … An implementation should skip over additional
        // fields using the submessageLength field" (§8.6.1).
        let parameters = if (flags & HE_FLAG_P) != 0 {
            Some(ParameterList::from_bytes_sentinel_optional(
                &body[pos..],
                le,
            )?)
        } else {
            None
        };

        Ok(Self {
            little_endian: le,
            message_length,
            timestamp,
            uextension4,
            wextension8,
            checksum,
            parameters,
        })
    }

    /// Where the `messageChecksum` of the message `message` sits, and its
    /// kind: the HeaderExtension must follow the RTPS Header at once
    /// (§8.3.7.3); the checksum comes after `messageLength`, the timestamp
    /// and the extensions that the flags announce.
    ///
    /// # Errors
    /// `ValueOutOfRange` if no HeaderExtension follows the header or its
    /// fields do not fit the submessage; `UnexpectedEof` if the message is
    /// too short.
    fn checksum_field(message: &[u8]) -> Result<(usize, ChecksumKind), WireError> {
        const HE_AT: usize = crate::header::RtpsHeader::WIRE_SIZE;
        if message.len() < HE_AT + 4 {
            return Err(WireError::UnexpectedEof {
                needed: HE_AT + 4,
                offset: message.len(),
            });
        }
        if message[HE_AT] != SUBMESSAGE_ID_HEADER_EXTENSION {
            return Err(WireError::ValueOutOfRange {
                message: "no HeaderExtension after the RTPS header",
            });
        }
        let flags = message[HE_AT + 1];
        let kind = ChecksumKind::from_flags(flags);
        let mut at = HE_AT + 4;
        for (flag, len) in [
            (HE_FLAG_L, 4),
            (HE_FLAG_T, 8),
            (HE_FLAG_U, 4),
            (HE_FLAG_W, 8),
        ] {
            if flags & flag != 0 {
                at += len;
            }
        }
        if message.len() < at + kind.wire_size() {
            return Err(WireError::UnexpectedEof {
                needed: at + kind.wire_size() - message.len(),
                offset: at,
            });
        }
        Ok((at, kind))
    }

    /// The `messageChecksum` that `message` should carry (DDSI-RTPS 2.5
    /// §8.3.7.4): the checksum "computed over the entire RTPS Message", the
    /// bytes of the `messageChecksum` field "set to zero". `ChecksumValue::None`
    /// where the HeaderExtension announces none.
    ///
    /// # Errors
    /// As for the location of the field: no HeaderExtension after the header,
    /// or a message too short for it.
    pub fn message_checksum(message: &[u8]) -> Result<ChecksumValue, WireError> {
        let (at, kind) = Self::checksum_field(message)?;
        if kind == ChecksumKind::None {
            return Ok(ChecksumValue::None);
        }
        let mut zeroed = message.to_vec();
        zeroed[at..at + kind.wire_size()].fill(0);
        Ok(ChecksumValue::compute(kind, &zeroed))
    }

    /// Writes the checksum of `message` into its `messageChecksum` field, in
    /// the big-endian form of Table 9.8.
    ///
    /// # Errors
    /// As [`Self::message_checksum`].
    pub fn seal_message(message: &mut [u8]) -> Result<(), WireError> {
        let (at, _) = Self::checksum_field(message)?;
        let value = Self::message_checksum(message)?;
        match value {
            ChecksumValue::None => {}
            ChecksumValue::Crc32c(c) => message[at..at + 4].copy_from_slice(&c.to_be_bytes()),
            ChecksumValue::Crc64(c) => message[at..at + 8].copy_from_slice(&c.to_be_bytes()),
            ChecksumValue::Md5(m) => message[at..at + 16].copy_from_slice(&m),
        }
        Ok(())
    }

    /// Whether the `messageChecksum` in `message` is the one its content
    /// gives. `true` where the HeaderExtension announces none.
    ///
    /// # Errors
    /// As [`Self::message_checksum`].
    pub fn verify_message(message: &[u8]) -> Result<bool, WireError> {
        let (at, kind) = Self::checksum_field(message)?;
        let carried = match kind {
            ChecksumKind::None => return Ok(true),
            ChecksumKind::Crc32c => ChecksumValue::Crc32c(u32::from_be_bytes(
                message[at..at + 4].try_into().unwrap_or([0; 4]),
            )),
            ChecksumKind::Crc64 => ChecksumValue::Crc64(u64::from_be_bytes(
                message[at..at + 8].try_into().unwrap_or([0; 8]),
            )),
            ChecksumKind::Md5 => {
                let mut m = [0u8; 16];
                m.copy_from_slice(&message[at..at + 16]);
                ChecksumValue::Md5(m)
            }
        };
        Ok(Self::message_checksum(message)? == carried)
    }

    /// The HeaderExtension of a message that a receiver rebuilt from the
    /// message it received, `received`, for instance after it opened the
    /// protected submessages (DDS-Security 1.2 §7.3.6.3): `rebuilt` starts
    /// with the same RTPS header and HeaderExtension. The extension's
    /// messageLength is "the length of the complete RTPS Message" (§8.3.3.2)
    /// and its messageChecksum is "computed over the entire RTPS Message"
    /// (§8.3.7.4), both of the received message. They are checked here
    /// against `received`; `rebuilt` gets the extension with the
    /// messageLength of its own length and without the checksum, which its
    /// content no longer gives. A message without the extension, or one
    /// whose extension carries neither field, is left as it is.
    ///
    /// # Errors
    /// `ValueOutOfRange` when `received` fails its messageLength or its
    /// checksum, `UnexpectedEof` when `rebuilt` is shorter than its
    /// extension; `rebuilt` is then left as it is.
    pub fn restate_for_rebuilt_message(
        received: &[u8],
        rebuilt: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        const HEADER: usize = 20;
        let carries = |m: &[u8]| {
            m.len() >= HEADER + 4
                && m[HEADER] == SUBMESSAGE_ID_HEADER_EXTENSION
                && (m[4], m[5]) >= (2, 5)
        };
        if !carries(received) || !carries(rebuilt) {
            return Ok(());
        }
        let he = Self::decode(&received[HEADER..])?;
        if he.message_length.is_none() && he.checksum.kind() == ChecksumKind::None {
            return Ok(());
        }
        he.validate_message_length(u32::try_from(received.len()).unwrap_or(u32::MAX))?;
        if !Self::verify_message(received)? {
            return Err(WireError::ValueOutOfRange {
                message: "HeaderExtension messageChecksum does not match the message",
            });
        }
        let le = rebuilt[HEADER + 1] & HE_FLAG_E != 0;
        let len_bytes = [rebuilt[HEADER + 2], rebuilt[HEADER + 3]];
        let octets = if le {
            u16::from_le_bytes(len_bytes)
        } else {
            u16::from_be_bytes(len_bytes)
        } as usize;
        let old_end = HEADER + 4 + octets;
        if rebuilt.len() < old_end {
            return Err(WireError::UnexpectedEof {
                needed: old_end,
                offset: 0,
            });
        }
        let mut restated = he;
        restated.checksum = ChecksumValue::None;
        if restated.message_length.is_some() {
            // The encoded size does not depend on the value.
            let size = restated.encode()?.len();
            let total = rebuilt.len() - (old_end - HEADER) + size;
            restated.message_length =
                Some(
                    u32::try_from(total).map_err(|_| WireError::ValueOutOfRange {
                        message: "rebuilt message longer than u32::MAX",
                    })?,
                );
        }
        let encoded = restated.encode()?;
        rebuilt.splice(HEADER..old_end, encoded);
        Ok(())
    }

    /// Decodes a complete HE submessage (header + body) from
    /// `bytes`. Checks the SubmessageId.
    ///
    /// # Errors
    /// `UnexpectedEof`, `UnknownSubmessageId`, `ValueOutOfRange`.
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: 0,
            });
        }
        if bytes[0] != SUBMESSAGE_ID_HEADER_EXTENSION {
            return Err(WireError::UnknownSubmessageId { id: bytes[0] });
        }
        let flags = bytes[1];
        let le = (flags & HE_FLAG_E) != 0;
        let mut len_bytes = [0u8; 2];
        len_bytes.copy_from_slice(&bytes[2..4]);
        let octets = if le {
            u16::from_le_bytes(len_bytes)
        } else {
            u16::from_be_bytes(len_bytes)
        } as usize;
        if bytes.len() < 4 + octets {
            return Err(WireError::UnexpectedEof {
                needed: octets,
                offset: 4,
            });
        }
        Self::decode_body(&bytes[4..4 + octets], flags)
    }

    /// Cross-check: if `message_length` is set, it equals
    /// `actual_message_length`, the length of the whole RTPS message from
    /// the beginning of its header (§8.3.3.2), as the receiver measured it.
    ///
    /// # Errors
    /// `ValueOutOfRange` on mismatch.
    pub fn validate_message_length(&self, actual_message_length: u32) -> Result<(), WireError> {
        if let Some(declared) = self.message_length {
            if declared != actual_message_length {
                return Err(WireError::ValueOutOfRange {
                    message: "HeaderExtension messageLength mismatch",
                });
            }
        }
        Ok(())
    }
}

// =====================================================================
// PID helpers — must-understand bit (Spec §9.4.2.11.2)
// =====================================================================

/// Bit mask: set -> the reader MUST understand this PID, otherwise discard
/// the whole message (HE carrier).
pub const PID_MUST_UNDERSTAND: u16 = 0x4000;

/// Bit mask: set -> vendor-specific PID (spec reservation 2.5
/// Tab 9.13). Readers that do not know this PID may skip it
/// **if** the `must_understand` bit is not set.
pub const PID_VENDOR_SPECIFIC: u16 = 0x8000;

/// Returns `true` if the given PID has the must-understand bit
/// set.
#[must_use]
pub fn pid_must_understand(pid: u16) -> bool {
    (pid & PID_MUST_UNDERSTAND) != 0
}

/// Raw PID without the must-understand and vendor bits.
#[must_use]
pub fn pid_strip(pid: u16) -> u16 {
    pid & !(PID_MUST_UNDERSTAND | PID_VENDOR_SPECIFIC)
}

// =====================================================================
// Bit helpers (private)
// =====================================================================

fn write_u32(out: &mut Vec<u8>, v: u32, le: bool) {
    let bytes = if le { v.to_le_bytes() } else { v.to_be_bytes() };
    out.extend_from_slice(&bytes);
}

fn read_u32(bytes: &[u8], le: bool) -> u32 {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&bytes[..4]);
    if le {
        u32::from_le_bytes(buf)
    } else {
        u32::from_be_bytes(buf)
    }
}

fn read_u64(bytes: &[u8], le: bool) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[..8]);
    if le {
        u64::from_le_bytes(buf)
    } else {
        u64::from_be_bytes(buf)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use crate::parameter_list::Parameter;
    use alloc::vec;

    fn roundtrip(he: HeaderExtension) {
        let bytes = he.encode().unwrap();
        let decoded = HeaderExtension::decode(&bytes).unwrap();
        assert_eq!(decoded, he);
    }

    #[test]
    fn empty_he_le_roundtrip() {
        let he = HeaderExtension {
            little_endian: true,
            ..HeaderExtension::default()
        };
        roundtrip(he);
    }

    #[test]
    fn empty_he_be_roundtrip() {
        let he = HeaderExtension::default();
        // little_endian = false
        roundtrip(he);
    }

    #[test]
    fn flag_byte_empty_le() {
        let he = HeaderExtension {
            little_endian: true,
            ..HeaderExtension::default()
        };
        assert_eq!(he.flag_byte(), HE_FLAG_E);
    }

    #[test]
    fn flag_byte_with_message_length() {
        let mut he = HeaderExtension {
            little_endian: true,
            message_length: Some(4242),
            ..HeaderExtension::default()
        };
        assert_eq!(he.flag_byte(), HE_FLAG_E | HE_FLAG_L);
        he.message_length = None;
        assert_eq!(he.flag_byte(), HE_FLAG_E);
    }

    #[test]
    fn flag_byte_all_optional_set_le() {
        let he = HeaderExtension {
            little_endian: true,
            message_length: Some(0x0102_0304),
            timestamp: Some(HeTimestamp {
                seconds: 7,
                fraction: 8,
            }),
            uextension4: Some([0xAA, 0xBB, 0xCC, 0xDD]),
            wextension8: Some([0x11; 8]),
            checksum: ChecksumValue::Md5([0x22; 16]),
            parameters: Some(ParameterList::new()),
        };
        let f = he.flag_byte();
        assert_eq!(
            f,
            HE_FLAG_E
                | HE_FLAG_L
                | HE_FLAG_T
                | HE_FLAG_U
                | HE_FLAG_W
                | HE_FLAG_P
                | ChecksumKind::Md5.to_flag_bits()
        );
    }

    #[test]
    fn checksum_kind_roundtrip() {
        for kind in [
            ChecksumKind::None,
            ChecksumKind::Crc32c,
            ChecksumKind::Crc64,
            ChecksumKind::Md5,
        ] {
            let bits = kind.to_flag_bits();
            assert_eq!(ChecksumKind::from_flags(HE_FLAG_E | bits), kind);
        }
    }

    #[test]
    fn checksum_kind_wire_size() {
        assert_eq!(ChecksumKind::None.wire_size(), 0);
        assert_eq!(ChecksumKind::Crc32c.wire_size(), 4);
        assert_eq!(ChecksumKind::Crc64.wire_size(), 8);
        assert_eq!(ChecksumKind::Md5.wire_size(), 16);
    }

    #[test]
    fn roundtrip_with_message_length_only_le() {
        let he = HeaderExtension {
            little_endian: true,
            message_length: Some(0xDEAD_BEEF),
            ..HeaderExtension::default()
        };
        roundtrip(he);
    }

    #[test]
    fn roundtrip_with_timestamp_le() {
        let he = HeaderExtension {
            little_endian: true,
            timestamp: Some(HeTimestamp {
                seconds: 1_710_000_000,
                fraction: 500_000_000,
            }),
            ..HeaderExtension::default()
        };
        roundtrip(he);
    }

    #[test]
    fn roundtrip_with_uextension4_be() {
        let he = HeaderExtension {
            little_endian: false,
            uextension4: Some([1, 2, 3, 4]),
            ..HeaderExtension::default()
        };
        roundtrip(he);
    }

    #[test]
    fn roundtrip_with_wextension8_le() {
        let he = HeaderExtension {
            little_endian: true,
            wextension8: Some([0xA5; 8]),
            ..HeaderExtension::default()
        };
        roundtrip(he);
    }

    #[test]
    fn roundtrip_with_crc32c_le() {
        let he = HeaderExtension {
            little_endian: true,
            checksum: ChecksumValue::Crc32c(0xCAFE_BABE),
            ..HeaderExtension::default()
        };
        roundtrip(he);
    }

    #[test]
    fn roundtrip_with_crc64_be() {
        let he = HeaderExtension {
            little_endian: false,
            checksum: ChecksumValue::Crc64(0x0123_4567_89AB_CDEF),
            ..HeaderExtension::default()
        };
        roundtrip(he);
    }

    #[test]
    fn roundtrip_with_md5_le() {
        let he = HeaderExtension {
            little_endian: true,
            checksum: ChecksumValue::Md5([
                0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
                0x42, 0x7e,
            ]),
            ..HeaderExtension::default()
        };
        roundtrip(he);
    }

    #[test]
    fn roundtrip_with_parameter_list() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(0x0015, vec![2, 5, 0, 0]));
        let he = HeaderExtension {
            little_endian: true,
            parameters: Some(pl),
            ..HeaderExtension::default()
        };
        roundtrip(he);
    }

    #[test]
    fn decode_rejects_malformed_parameter_list_when_p_flag_set() {
        // Spec §8.3.7.3: with the P flag the rest of the body must be a
        // valid ParameterList. A body containing half a
        // parameter header (4-byte header + 4 bytes missing)
        // must be rejected with an error.
        let mut pl_bytes = Vec::<u8>::new();
        // PID=0x0002, length=0x0010 (16 bytes announced), but only 2
        // bytes of body delivered. (PID=0x0001 is the sentinel — avoid.)
        pl_bytes.extend_from_slice(&0x0002u16.to_le_bytes());
        pl_bytes.extend_from_slice(&0x0010u16.to_le_bytes());
        pl_bytes.extend_from_slice(&[0xAA, 0xBB]); // truncated body
        // Body = E flag in flags + P flag → no L/W/U/V/C, then PL.
        let flags = HE_FLAG_E | HE_FLAG_P;
        let r = HeaderExtension::decode_body(&pl_bytes, flags);
        assert!(r.is_err(), "malformed PL must be rejected");
    }

    #[test]
    fn decode_rejects_truncated_uextension4_body() {
        // Spec §8.3.7.3 length mismatch: U flag announced, but the
        // body contains only 2 bytes instead of 4.
        let body: [u8; 2] = [0xAA, 0xBB];
        let flags = HE_FLAG_E | HE_FLAG_U;
        let r = HeaderExtension::decode_body(&body, flags);
        assert!(matches!(r, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn decode_rejects_truncated_wextension8_body() {
        // Spec §8.3.7.3: V flag announced, body shorter than 8 bytes.
        let body: [u8; 4] = [0; 4];
        let flags = HE_FLAG_E | HE_FLAG_W;
        let r = HeaderExtension::decode_body(&body, flags);
        assert!(matches!(r, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn decode_rejects_truncated_md5_checksum_body() {
        // C flag = MD5 (=3), body < 16 bytes → reject.
        let body: [u8; 4] = [0; 4];
        let flags = HE_FLAG_E | HE_FLAG_C0 | HE_FLAG_C1;
        let r = HeaderExtension::decode_body(&body, flags);
        assert!(matches!(r, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn flag_byte_includes_all_seven_logical_flags() {
        // Spec §8.3.3.2: 7 flags E/L/W/U/V/C/P.
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(0x0001, vec![1, 2, 3, 4]));
        let he = HeaderExtension {
            little_endian: true,
            message_length: Some(99),
            timestamp: Some(HeTimestamp::default()),
            uextension4: Some([1; 4]),
            wextension8: Some([2; 8]),
            checksum: ChecksumValue::Crc64(0xABCD),
            parameters: Some(pl),
        };
        let f = he.flag_byte();
        assert!(f & HE_FLAG_E != 0, "E");
        assert!(f & HE_FLAG_L != 0, "L");
        assert!(f & HE_FLAG_T != 0, "T");
        assert!(f & HE_FLAG_U != 0, "U");
        assert!(f & HE_FLAG_W != 0, "W");
        assert!(f & HE_FLAG_C_MASK != 0, "C");
        assert!(f & HE_FLAG_P != 0, "P");
    }

    #[test]
    fn roundtrip_all_fields_set_le() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(0x0015, vec![2, 5, 0, 0]));
        let he = HeaderExtension {
            little_endian: true,
            message_length: Some(1234),
            timestamp: Some(HeTimestamp {
                seconds: 1,
                fraction: 2,
            }),
            uextension4: Some([0xDE, 0xAD, 0xBE, 0xEF]),
            wextension8: Some([0xC0; 8]),
            checksum: ChecksumValue::Crc64(0x1122_3344_5566_7788),
            parameters: Some(pl),
        };
        roundtrip(he);
    }

    #[test]
    fn roundtrip_all_fields_set_be() {
        let mut pl = ParameterList::with_byte_order(false);
        pl.push(Parameter::new(0x0050, vec![0xAA; 16]));
        let he = HeaderExtension {
            little_endian: false,
            message_length: Some(99),
            timestamp: Some(HeTimestamp {
                seconds: 0xFFFF_FFFD,
                fraction: 9,
            }),
            uextension4: Some([0; 4]),
            wextension8: Some([0xFF; 8]),
            checksum: ChecksumValue::Md5([0x77; 16]),
            parameters: Some(pl),
        };
        roundtrip(he);
    }

    #[test]
    fn decode_rejects_wrong_submessage_id() {
        // Wrong SubmessageId (0x15 = DATA).
        let bytes = [0x15u8, HE_FLAG_E, 0, 0];
        let res = HeaderExtension::decode(&bytes);
        assert!(matches!(
            res,
            Err(WireError::UnknownSubmessageId { id: 0x15 })
        ));
    }

    #[test]
    fn decode_rejects_truncated_header() {
        let bytes = [0x80u8, HE_FLAG_E];
        let res = HeaderExtension::decode(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn decode_rejects_truncated_body_before_message_length() {
        // L flag but only 2 bytes of body.
        let bytes = [
            SUBMESSAGE_ID_HEADER_EXTENSION,
            HE_FLAG_E | HE_FLAG_L,
            2,
            0,
            0xAA,
            0xBB,
        ];
        let res = HeaderExtension::decode(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn decode_rejects_oversized_body() {
        // Construct a header with a large body — we can
        // only address up to u16::MAX, but we can
        // feed `decode_body` directly with a > MAX_HE_LENGTH body.
        let big = vec![0u8; MAX_HE_LENGTH + 1];
        let res = HeaderExtension::decode_body(&big, HE_FLAG_E);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn decode_skips_trailing_bytes_without_p_flag() {
        // §8.6.1: additional fields at the end are skipped.
        let body = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let he = HeaderExtension::decode_body(&body, HE_FLAG_E).unwrap();
        assert_eq!(
            he,
            HeaderExtension {
                little_endian: true,
                ..HeaderExtension::default()
            }
        );
    }

    /// §9.4.2.11: "The presence of the PID_SENTINEL is not required in the
    /// ParameterList that appears in the HeaderExtension."
    #[test]
    fn decode_takes_a_parameter_list_without_sentinel() {
        // PID 0x0042, length 4, value 1 2 3 4 — and the body ends.
        let body = vec![0x42, 0x00, 0x04, 0x00, 1, 2, 3, 4];
        let he = HeaderExtension::decode_body(&body, HE_FLAG_E | HE_FLAG_P).unwrap();
        let pl = he.parameters.unwrap();
        assert_eq!(pl.parameters.len(), 1);
        assert_eq!(pl.parameters[0].id, 0x0042);
        assert_eq!(pl.parameters[0].value, [1, 2, 3, 4]);
    }

    #[test]
    fn validate_message_length_matches() {
        let he = HeaderExtension {
            little_endian: true,
            message_length: Some(42),
            ..HeaderExtension::default()
        };
        assert!(he.validate_message_length(42).is_ok());
    }

    #[test]
    fn validate_message_length_mismatch() {
        let he = HeaderExtension {
            little_endian: true,
            message_length: Some(42),
            ..HeaderExtension::default()
        };
        let res = he.validate_message_length(43);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn validate_message_length_no_l_flag_succeeds() {
        let he = HeaderExtension::default();
        assert!(he.validate_message_length(123).is_ok());
    }

    #[test]
    fn checksum_value_kind_matches() {
        assert_eq!(ChecksumValue::None.kind(), ChecksumKind::None);
        assert_eq!(ChecksumValue::Crc32c(1).kind(), ChecksumKind::Crc32c);
        assert_eq!(ChecksumValue::Crc64(1).kind(), ChecksumKind::Crc64);
        assert_eq!(ChecksumValue::Md5([0; 16]).kind(), ChecksumKind::Md5);
    }

    /// DDSI-RTPS 2.5 Table 9.10: CRC-32C of the input bytes and the
    /// Checksum32_t bytes it is serialized to (big endian); Table 9.12 for
    /// CRC-64/XZ.
    #[test]
    fn the_checksums_of_tables_9_10_and_9_12() {
        let crc32 = |input: &[u8]| match ChecksumValue::compute(ChecksumKind::Crc32c, input) {
            ChecksumValue::Crc32c(c) => c,
            other => panic!("{other:?}"),
        };
        assert_eq!(crc32(&[0, 0, 0, 0]), 0x4867_4BC7);
        assert_eq!(crc32(&[0xFF; 4]), 0xFFFF_FFFF);
        assert_eq!(
            crc32(&[0x33, 0x22, 0x55, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
            0xB59C_A09B
        );
        assert_eq!(
            ChecksumValue::compute(ChecksumKind::Crc64, &[0, 0, 0, 0]),
            ChecksumValue::Crc64(0xF4A5_8635_1E1B_9F4B)
        );
        assert_eq!(
            ChecksumValue::compute(ChecksumKind::Crc64, &[0xFF; 4]),
            ChecksumValue::Crc64(0xFFFF_FFFF_0000_0000)
        );
        assert_eq!(
            ChecksumValue::compute(
                ChecksumKind::Crc64,
                &[0x33, 0x22, 0x55, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]
            ),
            ChecksumValue::Crc64(0x701E_CEB2_19A8_E5D5)
        );
    }

    /// Table 9.8: the checksum "shall be serialized … using CDR big endian
    /// encoding", also in a little-endian HeaderExtension.
    #[test]
    fn the_checksum_is_big_endian_in_a_little_endian_header_extension() {
        let he = HeaderExtension {
            little_endian: true,
            checksum: ChecksumValue::Crc32c(0x4867_4BC7),
            ..HeaderExtension::default()
        };
        assert_eq!(he.encode_body().unwrap(), vec![0x48, 0x67, 0x4B, 0xC7]);
        let he64 = HeaderExtension {
            little_endian: true,
            checksum: ChecksumValue::Crc64(0xF4A5_8635_1E1B_9F4B),
            ..HeaderExtension::default()
        };
        assert_eq!(
            he64.encode_body().unwrap(),
            vec![0xF4, 0xA5, 0x86, 0x35, 0x1E, 0x1B, 0x9F, 0x4B]
        );
        let back =
            HeaderExtension::decode_body(&he64.encode_body().unwrap(), he64.flag_byte()).unwrap();
        assert_eq!(back, he64);
    }

    /// §8.3.7.4: "computed over the entire RTPS Message", the field "set to
    /// zero" for the computation. A sealed message verifies; one octet of it
    /// changed does not.
    #[test]
    fn a_sealed_message_verifies_and_a_changed_one_does_not() {
        for (kind, placeholder) in [
            (ChecksumKind::Crc32c, ChecksumValue::Crc32c(0)),
            (ChecksumKind::Crc64, ChecksumValue::Crc64(0)),
            (ChecksumKind::Md5, ChecksumValue::Md5([0; 16])),
        ] {
            let he = HeaderExtension {
                little_endian: true,
                message_length: Some(64),
                timestamp: Some(HeTimestamp {
                    seconds: 5,
                    fraction: 6,
                }),
                checksum: placeholder,
                ..HeaderExtension::default()
            };
            let mut message = vec![b'R', b'T', b'P', b'S', 2, 5, 1, 0xF0];
            message.extend_from_slice(&[7u8; 12]);
            message.extend_from_slice(&he.encode().unwrap());
            // An INFO_TS after it, so that the checksum covers more than the
            // HeaderExtension.
            message.extend_from_slice(&[0x09, 0x01, 8, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
            assert!(
                !HeaderExtension::verify_message(&message).unwrap(),
                "{kind:?}: zero"
            );
            HeaderExtension::seal_message(&mut message).unwrap();
            assert!(
                HeaderExtension::verify_message(&message).unwrap(),
                "{kind:?}"
            );
            let last = message.len() - 1;
            message[last] ^= 0x01;
            assert!(
                !HeaderExtension::verify_message(&message).unwrap(),
                "{kind:?}: changed"
            );
        }
    }

    #[test]
    fn pid_must_understand_helpers() {
        assert!(pid_must_understand(0x4000));
        assert!(pid_must_understand(0x4015));
        assert!(!pid_must_understand(0x0015));
        assert_eq!(pid_strip(0xC015), 0x0015);
        assert_eq!(pid_strip(0x4015), 0x0015);
        assert_eq!(pid_strip(0x8015), 0x0015);
        assert_eq!(pid_strip(0x0015), 0x0015);
    }

    #[test]
    fn wire_layout_le_message_length_correct() {
        // L flag, 4 bytes of body = 0xDEADBEEF LE.
        let he = HeaderExtension {
            little_endian: true,
            message_length: Some(0xDEAD_BEEF),
            ..HeaderExtension::default()
        };
        let bytes = he.encode().unwrap();
        assert_eq!(bytes[0], SUBMESSAGE_ID_HEADER_EXTENSION);
        assert_eq!(bytes[1], HE_FLAG_E | HE_FLAG_L);
        // octets_to_next_header LE = 4
        assert_eq!(&bytes[2..4], &[0x04, 0x00]);
        // body LE
        assert_eq!(&bytes[4..8], &[0xEF, 0xBE, 0xAD, 0xDE]);
    }

    #[test]
    fn wire_layout_be_message_length_correct() {
        let he = HeaderExtension {
            little_endian: false,
            message_length: Some(0xDEAD_BEEF),
            ..HeaderExtension::default()
        };
        let bytes = he.encode().unwrap();
        assert_eq!(bytes[1], HE_FLAG_L); // E flag NOT set
        // octets_to_next_header BE = 4
        assert_eq!(&bytes[2..4], &[0x00, 0x04]);
        assert_eq!(&bytes[4..8], &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn empty_he_has_zero_octets_to_next_header() {
        let he = HeaderExtension {
            little_endian: true,
            ..HeaderExtension::default()
        };
        let bytes = he.encode().unwrap();
        assert_eq!(bytes.len(), 4);
        assert_eq!(&bytes[2..4], &[0, 0]);
    }

    // ========================================================================
    // ChecksumValue::compute / verify (Spec §9.4.2.15.2)
    // ========================================================================

    #[test]
    fn checksum_compute_none_yields_none() {
        let cs = ChecksumValue::compute(ChecksumKind::None, b"any payload");
        assert_eq!(cs, ChecksumValue::None);
    }

    #[test]
    fn checksum_compute_crc32c_matches_rfc4960_vector() {
        // RFC 4960 Appendix B.4: CRC32C("123456789") = 0xE3069283
        let cs = ChecksumValue::compute(ChecksumKind::Crc32c, b"123456789");
        assert_eq!(cs, ChecksumValue::Crc32c(0xE306_9283));
    }

    #[test]
    fn checksum_compute_crc64_xz_matches_known_vector() {
        // XZ utils CRC-64-XZ Standard-Test-Vector:
        // CRC-64("123456789") = 0x995DC9BBDF1939FA
        let cs = ChecksumValue::compute(ChecksumKind::Crc64, b"123456789");
        assert_eq!(cs, ChecksumValue::Crc64(0x995D_C9BB_DF19_39FA));
    }

    #[test]
    fn checksum_compute_md5_matches_rfc1321_vector() {
        // RFC 1321 §A.5: MD5("abc") = 900150983cd24fb0 d6963f7d28e17f72
        let cs = ChecksumValue::compute(ChecksumKind::Md5, b"abc");
        let expected: [u8; 16] = [
            0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
            0x7f, 0x72,
        ];
        assert_eq!(cs, ChecksumValue::Md5(expected));
    }

    #[test]
    fn checksum_verify_round_trip_crc32c_succeeds() {
        let payload = b"the quick brown fox jumps over the lazy dog";
        let cs = ChecksumValue::compute(ChecksumKind::Crc32c, payload);
        assert!(cs.verify(payload));
    }

    #[test]
    fn checksum_verify_detects_tampered_payload() {
        let original = b"original message";
        let cs = ChecksumValue::compute(ChecksumKind::Crc32c, original);
        let tampered = b"original Message";
        assert!(!cs.verify(tampered));
    }

    #[test]
    fn checksum_verify_none_kind_always_succeeds() {
        // Kind=None means "no checksum declared" — verify is
        // then a no-op and MUST return true (otherwise all
        // messages without a declared checksum would be discarded).
        let cs = ChecksumValue::None;
        assert!(cs.verify(b"any payload"));
        assert!(cs.verify(b""));
    }

    #[test]
    fn checksum_verify_crc64_round_trip() {
        let payload = b"\x52\x54\x50\x53\x02\x05\x01\x0F";
        let cs = ChecksumValue::compute(ChecksumKind::Crc64, payload);
        assert!(cs.verify(payload));
    }

    #[test]
    fn checksum_verify_md5_round_trip() {
        let payload = b"DDSI-RTPS 2.5 HEADER_EXTENSION";
        let cs = ChecksumValue::compute(ChecksumKind::Md5, payload);
        assert!(cs.verify(payload));
    }
}
