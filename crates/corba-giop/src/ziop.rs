// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ZIOP — GIOP compression, CORBA 3.3 Part 2 §12.
//!
//! A ZIOP message is a GIOP message (1.2 or later, fragments included)
//! whose magic is `"ZIOP"` and whose body is replaced by
//!
//! ```text
//! module ZIOP {
//!     struct CompressionData {
//!         Compression::CompressorId compressor;       // unsigned short
//!         unsigned long             original_length;  // GIOP body length
//!         Compression::Buffer       data;             // sequence<octet>
//!     };
//! };
//! ```
//!
//! The struct is CDR in the byte order of the header, aligned from the
//! first octet of the message like any GIOP body: `compressor` at offset
//! 12, `original_length` at 16, the length of `data` at 20, the data from
//! 24. The `message_size` of the header is the length of this body; the
//! other header fields are those of the GIOP message (§12.1).
//!
//! The four ZIOP policies (§12.3) decide whether and how a message is
//! compressed. `CompressionEnablingPolicy` and
//! `CompressorIdLevelListPolicy` are client-exposed: they travel in the
//! `TAG_POLICIES` component of an object reference and in the
//! `INVOCATION_POLICIES` service context of requests and replies
//! ([`crate::messaging`], §12.2, §12.4). `CompressionLowValuePolicy` and
//! `CompressionMinRatioPolicy` stay with the ORB that set them.
//!
//! The compressors are not part of this crate: [`ZiopCodec`] is what an
//! ORB hands in (the `Compression::CompressionManager` of Part 1 §18 in
//! `zerodds-corba-ccm`).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, Writer};

use crate::error::{GiopError, GiopResult};
use crate::header::{HEADER_SIZE, Magic, MessageHeader, ZIOP_MAGIC_BYTES};
use crate::messaging::{PolicyValue, begin_encapsulation, open_encapsulation};

/// `Compression::CompressorId` (Part 1 §18.5).
pub type CompressorId = u16;

/// `Compression::CompressionLevel` (Part 1 §18.5).
pub type CompressionLevel = u16;

/// `Compression::CompressionRatio` (Part 1 §18.5).
pub type CompressionRatio = f32;

/// `Compression::CompressorIdLevel` (Part 1 §18.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompressorIdLevel {
    /// The compressor.
    pub compressor_id: CompressorId,
    /// Its level.
    pub compression_level: CompressionLevel,
}

/// `Compression::CompressorIdLevelList` (Part 1 §18.5).
pub type CompressorIdLevelList = Vec<CompressorIdLevel>;

/// `Compression::COMPRESSORID_ZLIB` (Part 1 §18.5), the compressor of
/// the ORB default list.
pub const COMPRESSORID_ZLIB: CompressorId = 4;

/// `ZIOP::COMPRESSION_ENABLING_POLICY_ID` (§12.5).
pub const COMPRESSION_ENABLING_POLICY_ID: u32 = 64;
/// `ZIOP::COMPRESSOR_ID_LEVEL_LIST_POLICY_ID` (§12.5).
pub const COMPRESSOR_ID_LEVEL_LIST_POLICY_ID: u32 = 65;
/// `ZIOP::COMPRESSION_LOW_VALUE_POLICY_ID` (§12.5).
pub const COMPRESSION_LOW_VALUE_POLICY_ID: u32 = 66;
/// `ZIOP::COMPRESSION_MIN_RATIO_POLICY_ID` (§12.5).
pub const COMPRESSION_MIN_RATIO_POLICY_ID: u32 = 67;

/// The ORB default of `CompressionLowValuePolicy`: bodies shorter than
/// this many octets are sent as GIOP.
pub const DEFAULT_LOW_VALUE: u32 = 1024;

/// The ORB default of `CompressorIdLevelListPolicy`: zlib at level 6.
pub const DEFAULT_COMPRESSOR: CompressorIdLevel = CompressorIdLevel {
    compressor_id: COMPRESSORID_ZLIB,
    compression_level: 6,
};

/// `ZIOP::CompressionData`, the body of a ZIOP message (§12.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionData {
    /// The compressor that produced `data`.
    pub compressor: CompressorId,
    /// Length of the uncompressed GIOP body.
    pub original_length: u32,
    /// The compressed GIOP body.
    pub data: Vec<u8>,
}

/// Why a compressor of a [`ZiopCodec`] did not deliver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZiopError {
    /// The codec has no compressor with this id.
    UnknownCompressor(CompressorId),
    /// The compressor failed (`Compression::CompressionException`).
    Failed {
        /// `reason` of the exception.
        reason: i32,
        /// `description` of the exception.
        description: String,
    },
}

impl core::fmt::Display for ZiopError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownCompressor(id) => write!(f, "no compressor with id {id}"),
            Self::Failed {
                reason,
                description,
            } => write!(f, "compressor failed ({reason}): {description}"),
        }
    }
}

/// The compressors an ORB uses for ZIOP.
pub trait ZiopCodec: Send + Sync {
    /// `true` when the codec has a compressor with this id.
    fn supports(&self, compressor: CompressorId) -> bool;

    /// Compresses `body` with the compressor and level of `compressor`.
    ///
    /// # Errors
    /// [`ZiopError`] when there is no such compressor or it fails.
    fn compress(&self, compressor: CompressorIdLevel, body: &[u8]) -> Result<Vec<u8>, ZiopError>;

    /// Decompresses `data` written by `compressor`; the result is expected
    /// to be `original_length` octets.
    ///
    /// # Errors
    /// [`ZiopError`] when there is no such compressor or it fails.
    fn decompress(
        &self,
        compressor: CompressorId,
        data: &[u8],
        original_length: u32,
    ) -> Result<Vec<u8>, ZiopError>;
}

/// The ZIOP policies of one side (§12.3). A policy that is not set takes
/// the ORB default of [`Default`].
#[derive(Debug, Clone, PartialEq)]
pub struct ZiopPolicies {
    /// `CompressionEnablingPolicy` (§12.3.1, client-exposed): ZIOP is used
    /// only when both sides set it to `true`. Default `false`.
    pub compression_enabled: bool,
    /// `CompressorIdLevelListPolicy` (§12.3.2, client-exposed): the
    /// compressors this side may use, by preference. Default
    /// [`DEFAULT_COMPRESSOR`].
    pub compressor_ids: CompressorIdLevelList,
    /// `CompressionLowValuePolicy` (§12.3.3): bodies shorter than this go
    /// out as GIOP. Default [`DEFAULT_LOW_VALUE`].
    pub low_value: u32,
    /// `CompressionMinRatioPolicy` (§12.3.4): a compressed message is sent
    /// only when `compressed_length / original_length` is at most this
    /// ratio. Without it (the default) a message is sent compressed only
    /// when the ZIOP message is shorter than the GIOP message.
    pub min_ratio: Option<CompressionRatio>,
}

impl Default for ZiopPolicies {
    fn default() -> Self {
        Self {
            compression_enabled: false,
            compressor_ids: alloc::vec![DEFAULT_COMPRESSOR],
            low_value: DEFAULT_LOW_VALUE,
            min_ratio: None,
        }
    }
}

/// The client-exposed ZIOP policies of the other side, as its
/// `TAG_POLICIES` component or `INVOCATION_POLICIES` service context
/// carried them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeerZiop {
    /// Its `CompressionEnablingPolicy`; `false` when it sent none.
    pub compression_enabled: bool,
    /// Its `CompressorIdLevelListPolicy`; empty when it sent none.
    pub compressor_ids: CompressorIdLevelList,
}

/// How one message is compressed: the compressor both sides agreed on
/// and the two policies of the sender that are not client-exposed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZiopSend {
    /// Compressor and level.
    pub compressor: CompressorIdLevel,
    /// `CompressionLowValuePolicy` of the sender.
    pub low_value: u32,
    /// `CompressionMinRatioPolicy` of the sender.
    pub min_ratio: Option<CompressionRatio>,
}

impl ZiopPolicies {
    /// ZIOP enabled with `compressor_ids` and the ORB defaults for the
    /// other two policies.
    #[must_use]
    pub fn enabled(compressor_ids: CompressorIdLevelList) -> Self {
        Self {
            compression_enabled: true,
            compressor_ids,
            ..Self::default()
        }
    }

    /// The client-exposed policies as `Messaging::PolicyValue`s (§12.2):
    /// `CompressionEnablingPolicy` and `CompressorIdLevelListPolicy`, each
    /// `pvalue` a CDR encapsulation of the policy value.
    ///
    /// # Errors
    /// `Malformed` for a compressor list longer than a CDR sequence holds.
    pub fn client_exposed(&self, endianness: Endianness) -> GiopResult<Vec<PolicyValue>> {
        Ok(alloc::vec![
            enabling_value(self.compression_enabled, endianness),
            id_level_list_value(&self.compressor_ids, endianness)?,
        ])
    }

    /// The compressor for messages to `peer` (§12.3.2): none unless both
    /// sides enable compression; else the first entry of this side's list
    /// that `codec` has and `peer` lists, at the lower of the two levels.
    #[must_use]
    pub fn select_compressor(
        &self,
        peer: &PeerZiop,
        codec: &dyn ZiopCodec,
    ) -> Option<CompressorIdLevel> {
        if !self.compression_enabled || !peer.compression_enabled {
            return None;
        }
        self.compressor_ids
            .iter()
            .filter(|own| codec.supports(own.compressor_id))
            .find_map(|own| {
                peer.compressor_ids
                    .iter()
                    .find(|theirs| theirs.compressor_id == own.compressor_id)
                    .map(|theirs| CompressorIdLevel {
                        compressor_id: own.compressor_id,
                        compression_level: own.compression_level.min(theirs.compression_level),
                    })
            })
    }

    /// The [`ZiopSend`] for messages to `peer`, `None` when they go out
    /// as GIOP.
    #[must_use]
    pub fn send_policy(&self, peer: &PeerZiop, codec: &dyn ZiopCodec) -> Option<ZiopSend> {
        self.select_compressor(peer, codec)
            .map(|compressor| ZiopSend {
                compressor,
                low_value: self.low_value,
                min_ratio: self.min_ratio,
            })
    }
}

impl PeerZiop {
    /// Reads the ZIOP policies from the `PolicyValue`s of a `TAG_POLICIES`
    /// component or an `INVOCATION_POLICIES` service context. Values of
    /// other policy types belong to other policies and are not read here.
    ///
    /// # Errors
    /// `Malformed`/`Wire` for a ZIOP `pvalue` that does not decode.
    pub fn from_policy_values(values: &[PolicyValue]) -> GiopResult<Self> {
        let mut peer = Self::default();
        for v in values {
            match v.ptype {
                COMPRESSION_ENABLING_POLICY_ID => {
                    let mut r = open_encapsulation(&v.pvalue)?;
                    peer.compression_enabled = r.read_bool()?;
                }
                COMPRESSOR_ID_LEVEL_LIST_POLICY_ID => {
                    peer.compressor_ids = decode_id_level_list(&v.pvalue)?;
                }
                _ => {}
            }
        }
        Ok(peer)
    }
}

/// The `PolicyValue` of a `CompressionEnablingPolicy`.
#[must_use]
pub fn enabling_value(enabled: bool, endianness: Endianness) -> PolicyValue {
    let mut w = begin_encapsulation(endianness);
    w.write_bool(enabled);
    PolicyValue {
        ptype: COMPRESSION_ENABLING_POLICY_ID,
        pvalue: w.finish(),
    }
}

/// The `PolicyValue` of a `CompressorIdLevelListPolicy`.
///
/// # Errors
/// `Malformed` for a list longer than a CDR sequence holds.
pub fn id_level_list_value(
    list: &[CompressorIdLevel],
    endianness: Endianness,
) -> GiopResult<PolicyValue> {
    let mut w = begin_encapsulation(endianness);
    w.write_u32(
        u32::try_from(list.len())
            .map_err(|_| GiopError::Malformed("compressor list longer than u32".into()))?,
    );
    for e in list {
        w.write_u16(e.compressor_id);
        w.write_u16(e.compression_level);
    }
    Ok(PolicyValue {
        ptype: COMPRESSOR_ID_LEVEL_LIST_POLICY_ID,
        pvalue: w.finish(),
    })
}

/// The `PolicyValue` of a `CompressionLowValuePolicy`.
#[must_use]
pub fn low_value_value(low_value: u32, endianness: Endianness) -> PolicyValue {
    let mut w = begin_encapsulation(endianness);
    w.write_u32(low_value);
    PolicyValue {
        ptype: COMPRESSION_LOW_VALUE_POLICY_ID,
        pvalue: w.finish(),
    }
}

/// The `PolicyValue` of a `CompressionMinRatioPolicy`.
#[must_use]
pub fn min_ratio_value(ratio: CompressionRatio, endianness: Endianness) -> PolicyValue {
    let mut w = begin_encapsulation(endianness);
    w.write_f32(ratio);
    PolicyValue {
        ptype: COMPRESSION_MIN_RATIO_POLICY_ID,
        pvalue: w.finish(),
    }
}

fn decode_id_level_list(encap: &[u8]) -> GiopResult<CompressorIdLevelList> {
    let mut r = open_encapsulation(encap)?;
    let count = r.read_u32()? as usize;
    if count > r.remaining() / 4 {
        return Err(GiopError::Malformed(alloc::format!(
            "CompressorIdLevelList announces {count} entries in {} octets",
            r.remaining()
        )));
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let compressor_id = r.read_u16()?;
        let compression_level = r.read_u16()?;
        out.push(CompressorIdLevel {
            compressor_id,
            compression_level,
        });
    }
    Ok(out)
}

impl CompressionData {
    /// Writes the struct as the body of a message whose 12 header octets
    /// `w` already holds, so the alignment counts from the message start.
    ///
    /// # Errors
    /// `Malformed` for data longer than a CDR sequence holds.
    pub fn encode_body(&self, w: &mut Writer) -> GiopResult<()> {
        w.write_u16(self.compressor);
        w.write_u32(self.original_length);
        w.write_u32(
            u32::try_from(self.data.len())
                .map_err(|_| GiopError::Malformed("compressed data longer than u32".into()))?,
        );
        w.write_bytes(&self.data);
        Ok(())
    }

    /// Reads the struct from the body of the ZIOP message `frame` (header
    /// included) described by `header`. The struct fills the body: octets
    /// behind `data` are refused.
    ///
    /// # Errors
    /// `BodyTooLarge` when `frame` is shorter than the header says,
    /// `Malformed`/`Wire` for a body that is not one `CompressionData`.
    pub fn decode_body(frame: &[u8], header: &MessageHeader) -> GiopResult<Self> {
        let end = HEADER_SIZE + header.message_size as usize;
        if frame.len() < end {
            return Err(GiopError::BodyTooLarge {
                body_size: header.message_size as usize,
                message_size_field: header.message_size,
            });
        }
        let mut r = zerodds_wire::Reader::new(&frame[..end], header.endianness());
        r.skip_to(HEADER_SIZE)?;
        let compressor = r.read_u16()?;
        let original_length = r.read_u32()?;
        let len = r.read_u32()? as usize;
        if len != r.remaining() {
            return Err(GiopError::Malformed(alloc::format!(
                "CompressionData of {len} octets in a body with {} octets left",
                r.remaining()
            )));
        }
        let data = r.read_bytes(len)?.to_vec();
        Ok(Self {
            compressor,
            original_length,
            data,
        })
    }
}

fn ziop_failure(e: &ZiopError) -> GiopError {
    GiopError::Malformed(alloc::format!("ZIOP: {e}"))
}

/// Compresses one GIOP message (`frame`: header and body) into a ZIOP
/// message (§12.1), or `Ok(None)` when it stays a GIOP message: a version
/// below 1.2, an empty body, a body shorter than the low value
/// (§12.3.3), a ratio above the minimum ratio (§12.3.4) or, without a
/// minimum ratio, a ZIOP message that would not be shorter.
///
/// # Errors
/// The header error of a `frame` that is not a GIOP message; `Malformed`
/// when the compressor fails.
pub fn compress_message(
    frame: &[u8],
    send: &ZiopSend,
    codec: &dyn ZiopCodec,
) -> GiopResult<Option<Vec<u8>>> {
    let (header, rest) = MessageHeader::decode(frame)?;
    let size = header.message_size as usize;
    if rest.len() < size {
        return Err(GiopError::BodyTooLarge {
            body_size: size,
            message_size_field: header.message_size,
        });
    }
    if !header.version.supports_ziop() || size == 0 || (size as u64) < u64::from(send.low_value) {
        return Ok(None);
    }
    let body = &rest[..size];
    let data = codec
        .compress(send.compressor, body)
        .map_err(|e| ziop_failure(&e))?;
    if let Some(min) = send.min_ratio {
        #[allow(clippy::cast_precision_loss)]
        let ratio = data.len() as f64 / size as f64;
        if ratio > f64::from(min) {
            return Ok(None);
        }
    }
    let cd = CompressionData {
        compressor: send.compressor.compressor_id,
        original_length: header.message_size,
        data,
    };
    let mut w = Writer::new(header.endianness());
    for _ in 0..HEADER_SIZE {
        w.write_u8(0);
    }
    cd.encode_body(&mut w)?;
    let mut out = w.finish();
    if send.min_ratio.is_none() && out.len() >= HEADER_SIZE + size {
        return Ok(None);
    }
    let body_size = u32::try_from(out.len() - HEADER_SIZE)
        .map_err(|_| GiopError::Malformed("ZIOP body exceeds u32".into()))?;
    let z = MessageHeader::new(header.version, header.flags, header.message_type, body_size);
    let mut hw = Writer::new(header.endianness());
    z.encode(&mut hw)?;
    let hb = hw.finish();
    out[..HEADER_SIZE].copy_from_slice(&hb);
    out[..4].copy_from_slice(&ZIOP_MAGIC_BYTES);
    Ok(Some(out))
}

/// The GIOP message of the ZIOP message `frame` (§12.1): the body is
/// decompressed, the magic set back to `"GIOP"` and `message_size` to
/// `original_length`; the other header fields stay. A GIOP `frame` is
/// returned as it is.
///
/// # Errors
/// The header error of a `frame` that is neither; `Malformed` when
/// `original_length` exceeds `max_body`, the compressor fails or does not
/// deliver `original_length` octets.
pub fn decompress_message(
    frame: &[u8],
    codec: &dyn ZiopCodec,
    max_body: usize,
) -> GiopResult<Vec<u8>> {
    let (magic, header, _) = MessageHeader::decode_frame(frame)?;
    if magic == Magic::Giop {
        return Ok(frame.to_vec());
    }
    let cd = CompressionData::decode_body(frame, &header)?;
    if cd.original_length as usize > max_body {
        return Err(GiopError::Malformed(alloc::format!(
            "ZIOP original_length {} exceeds {max_body}",
            cd.original_length
        )));
    }
    let body = codec
        .decompress(cd.compressor, &cd.data, cd.original_length)
        .map_err(|e| ziop_failure(&e))?;
    if body.len() != cd.original_length as usize {
        return Err(GiopError::Malformed(alloc::format!(
            "ZIOP body decompressed to {} octets, original_length {}",
            body.len(),
            cd.original_length
        )));
    }
    let giop = MessageHeader::new(
        header.version,
        header.flags,
        header.message_type,
        cd.original_length,
    );
    let mut w = Writer::new(header.endianness());
    giop.encode(&mut w)?;
    let mut out = w.finish();
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::codec::{Message, decode_message, encode_message};
    use crate::flags::Flags;
    use crate::message_type::MessageType;
    use crate::reply::{Reply, ReplyStatusType};
    use crate::service_context::ServiceContextList;
    use crate::version::Version;

    /// A run-length "compressor" with id 4 (zlib's id) for the codec
    /// tests: pairs of (count, byte). Only the framing is under test here;
    /// the zlib of Part 1 §18 is tested in zerodds-corba-ccm.
    struct Rle;

    impl ZiopCodec for Rle {
        fn supports(&self, compressor: CompressorId) -> bool {
            compressor == COMPRESSORID_ZLIB
        }
        fn compress(&self, c: CompressorIdLevel, body: &[u8]) -> Result<Vec<u8>, ZiopError> {
            if c.compressor_id != COMPRESSORID_ZLIB {
                return Err(ZiopError::UnknownCompressor(c.compressor_id));
            }
            let mut out = Vec::new();
            let mut i = 0;
            while i < body.len() {
                let b = body[i];
                let mut n = 1usize;
                while i + n < body.len() && body[i + n] == b && n < 255 {
                    n += 1;
                }
                out.push(n as u8);
                out.push(b);
                i += n;
            }
            Ok(out)
        }
        fn decompress(&self, c: CompressorId, data: &[u8], _: u32) -> Result<Vec<u8>, ZiopError> {
            if c != COMPRESSORID_ZLIB {
                return Err(ZiopError::UnknownCompressor(c));
            }
            let mut out = Vec::new();
            for pair in data.chunks(2) {
                if pair.len() != 2 {
                    return Err(ZiopError::Failed {
                        reason: -3,
                        description: "odd".into(),
                    });
                }
                out.extend(core::iter::repeat_n(pair[1], pair[0] as usize));
            }
            Ok(out)
        }
    }

    fn reply_frame(endianness: Endianness, body_len: usize) -> Vec<u8> {
        encode_message(
            Version::V1_2,
            endianness,
            false,
            &Message::Reply(Reply {
                request_id: 7,
                reply_status: ReplyStatusType::NoException,
                service_context: ServiceContextList::default(),
                body: alloc::vec![0x41; body_len],
            }),
        )
        .unwrap()
    }

    fn send() -> ZiopSend {
        ZiopSend {
            compressor: DEFAULT_COMPRESSOR,
            low_value: 0,
            min_ratio: None,
        }
    }

    #[test]
    fn corba_p2_12_1_ziop_message_layout() {
        for endianness in [Endianness::Big, Endianness::Little] {
            let frame = reply_frame(endianness, 400);
            let z = compress_message(&frame, &send(), &Rle).unwrap().unwrap();
            assert_eq!(&z[..4], b"ZIOP");
            // Version, flags and message type are those of the GIOP message.
            assert_eq!(&z[4..8], &frame[4..8]);
            let u32_at = |b: &[u8], o: usize| {
                let a = [b[o], b[o + 1], b[o + 2], b[o + 3]];
                match endianness {
                    Endianness::Big => u32::from_be_bytes(a),
                    Endianness::Little => u32::from_le_bytes(a),
                }
            };
            let giop_size = u32_at(&frame, 8);
            // message_size is the length of the CompressionData body.
            assert_eq!(u32_at(&z, 8) as usize, z.len() - 12);
            // compressor (ushort) at 12, two padding octets, then
            // original_length at 16 and the data length at 20.
            let id = match endianness {
                Endianness::Big => u16::from_be_bytes([z[12], z[13]]),
                Endianness::Little => u16::from_le_bytes([z[12], z[13]]),
            };
            assert_eq!(id, COMPRESSORID_ZLIB);
            assert_eq!(u32_at(&z, 16), giop_size);
            assert_eq!(u32_at(&z, 20) as usize, z.len() - 24);
            let expected = Rle.compress(DEFAULT_COMPRESSOR, &frame[12..]).unwrap();
            assert_eq!(&z[24..], expected.as_slice());
            // Back to the same GIOP message.
            let back = decompress_message(&z, &Rle, 1 << 20).unwrap();
            assert_eq!(back, frame);
        }
    }

    #[test]
    fn corba_p2_12_1_the_header_accepts_ziop_from_giop_1_2_only() {
        let frame = reply_frame(Endianness::Big, 400);
        let z = compress_message(&frame, &send(), &Rle).unwrap().unwrap();
        let (magic, h, _) = MessageHeader::decode_frame(&z).unwrap();
        assert_eq!(magic, Magic::Ziop);
        assert_eq!(h.message_type, MessageType::Reply);
        // The GIOP-only decoder refuses the ZIOP magic.
        assert!(matches!(
            MessageHeader::decode(&z),
            Err(GiopError::InvalidMagic(m)) if m == *b"ZIOP"
        ));
        let mut v11 = z.clone();
        v11[5] = 1;
        assert!(matches!(
            MessageHeader::decode_frame(&v11),
            Err(GiopError::UnsupportedVersion { major: 1, minor: 1 })
        ));
        // A GIOP 1.1 message is not compressed.
        let giop11 = encode_message(
            Version::V1_1,
            Endianness::Big,
            false,
            &Message::Reply(Reply {
                request_id: 1,
                reply_status: ReplyStatusType::NoException,
                service_context: ServiceContextList::default(),
                body: alloc::vec![0; 400],
            }),
        )
        .unwrap();
        assert_eq!(compress_message(&giop11, &send(), &Rle).unwrap(), None);
    }

    #[test]
    fn corba_p2_12_1_a_fragment_message_is_compressed_with_its_flags() {
        let frag = MessageHeader::new(
            Version::V1_2,
            Flags::from_endianness(Endianness::Big).with_fragment(true),
            MessageType::Fragment,
            404,
        );
        let mut w = Writer::new(Endianness::Big);
        frag.encode(&mut w).unwrap();
        let mut frame = w.finish();
        frame.extend_from_slice(&[0, 0, 0, 9]);
        frame.extend_from_slice(&[0x55; 400]);
        let z = compress_message(&frame, &send(), &Rle).unwrap().unwrap();
        let (magic, h, _) = MessageHeader::decode_frame(&z).unwrap();
        assert_eq!(magic, Magic::Ziop);
        assert!(h.flags.has_more_fragments());
        assert_eq!(h.message_type, MessageType::Fragment);
        assert_eq!(decompress_message(&z, &Rle, 1 << 20).unwrap(), frame);
    }

    #[test]
    fn corba_p2_12_3_low_value_and_min_ratio_keep_a_message_uncompressed() {
        let frame = reply_frame(Endianness::Big, 400);
        let body = frame.len() - 12;
        let low = ZiopSend {
            low_value: u32::try_from(body + 1).unwrap(),
            ..send()
        };
        assert_eq!(compress_message(&frame, &low, &Rle).unwrap(), None);
        let at_low = ZiopSend {
            low_value: u32::try_from(body).unwrap(),
            ..send()
        };
        assert!(compress_message(&frame, &at_low, &Rle).unwrap().is_some());
        // The run-length data of this body is 2 octets per run: the body
        // of 400 equal octets plus the reply header compresses to a small
        // ratio; a minimum ratio below it keeps the message GIOP.
        let data = Rle.compress(DEFAULT_COMPRESSOR, &frame[12..]).unwrap();
        #[allow(clippy::cast_precision_loss)]
        let ratio = (data.len() as f64 / body as f64) as f32;
        let strict = ZiopSend {
            min_ratio: Some(ratio * 0.9),
            ..send()
        };
        assert_eq!(compress_message(&frame, &strict, &Rle).unwrap(), None);
        let loose = ZiopSend {
            min_ratio: Some(ratio * 1.1),
            ..send()
        };
        assert!(compress_message(&frame, &loose, &Rle).unwrap().is_some());
        // Without a minimum ratio, a message the compressor makes longer
        // stays GIOP: alternating octets double under run-length coding.
        let mut noisy = reply_frame(Endianness::Big, 400);
        let n = noisy.len();
        for (i, b) in noisy[40..n].iter_mut().enumerate() {
            *b = (i % 2) as u8;
        }
        assert_eq!(compress_message(&noisy, &send(), &Rle).unwrap(), None);
    }

    #[test]
    fn corba_p2_12_1_a_ziop_body_is_checked_before_and_after_decompression() {
        let frame = reply_frame(Endianness::Big, 400);
        let z = compress_message(&frame, &send(), &Rle).unwrap().unwrap();
        // original_length above the limit.
        assert!(decompress_message(&z, &Rle, 100).is_err());
        // original_length that the data does not decompress to.
        let mut wrong = z.clone();
        wrong[19] ^= 1;
        assert!(decompress_message(&wrong, &Rle, 1 << 20).is_err());
        // An unknown compressor id.
        let mut unknown = z.clone();
        unknown[13] = 9;
        assert!(decompress_message(&unknown, &Rle, 1 << 20).is_err());
        // Octets behind the data inside message_size.
        let mut trailing = z.clone();
        trailing.push(0);
        let size = u32::from_be_bytes([z[8], z[9], z[10], z[11]]) + 1;
        trailing[8..12].copy_from_slice(&size.to_be_bytes());
        assert!(decompress_message(&trailing, &Rle, 1 << 20).is_err());
        // A GIOP frame passes unchanged.
        assert_eq!(decompress_message(&frame, &Rle, 1 << 20).unwrap(), frame);
        let (msg, _) = decode_message(&decompress_message(&z, &Rle, 1 << 20).unwrap()).unwrap();
        assert!(matches!(msg, Message::Reply(r) if r.request_id == 7));
    }

    #[test]
    fn corba_p2_12_2_no_ziop_to_a_peer_without_the_policies() {
        let own = ZiopPolicies::enabled(alloc::vec![DEFAULT_COMPRESSOR]);
        // The peer sent nothing.
        assert_eq!(own.send_policy(&PeerZiop::default(), &Rle), None);
        // The peer lists zlib but did not enable compression.
        let disabled = PeerZiop {
            compression_enabled: false,
            compressor_ids: alloc::vec![DEFAULT_COMPRESSOR],
        };
        assert_eq!(own.send_policy(&disabled, &Rle), None);
        // This side did not enable compression.
        let enabled_peer = PeerZiop {
            compression_enabled: true,
            compressor_ids: alloc::vec![DEFAULT_COMPRESSOR],
        };
        assert_eq!(
            ZiopPolicies::default().send_policy(&enabled_peer, &Rle),
            None
        );
        // No compressor in common.
        let gzip_only = PeerZiop {
            compression_enabled: true,
            compressor_ids: alloc::vec![CompressorIdLevel {
                compressor_id: 1,
                compression_level: 9,
            }],
        };
        assert_eq!(own.send_policy(&gzip_only, &Rle), None);
        assert!(own.send_policy(&enabled_peer, &Rle).is_some());
    }

    #[test]
    fn corba_p2_12_3_2_the_first_common_compressor_at_the_lowest_level() {
        let own = ZiopPolicies::enabled(alloc::vec![
            CompressorIdLevel {
                compressor_id: 9,
                compression_level: 3,
            },
            CompressorIdLevel {
                compressor_id: 1,
                compression_level: 8,
            },
            CompressorIdLevel {
                compressor_id: COMPRESSORID_ZLIB,
                compression_level: 7,
            },
        ]);
        let peer = PeerZiop {
            compression_enabled: true,
            compressor_ids: alloc::vec![
                CompressorIdLevel {
                    compressor_id: COMPRESSORID_ZLIB,
                    compression_level: 2,
                },
                CompressorIdLevel {
                    compressor_id: 9,
                    compression_level: 1,
                },
            ],
        };
        // 9 is listed by both but the codec has no compressor 9; 1 is not
        // listed by the peer; zlib at min(7, 2).
        assert_eq!(
            own.select_compressor(&peer, &Rle),
            Some(CompressorIdLevel {
                compressor_id: COMPRESSORID_ZLIB,
                compression_level: 2,
            })
        );
    }

    #[test]
    fn corba_p2_12_4_client_exposed_policies_round_trip_as_policy_values() {
        let own = ZiopPolicies {
            compression_enabled: true,
            compressor_ids: alloc::vec![
                DEFAULT_COMPRESSOR,
                CompressorIdLevel {
                    compressor_id: 1,
                    compression_level: 9,
                },
            ],
            low_value: 10,
            min_ratio: Some(0.5),
        };
        let values = own.client_exposed(Endianness::Big).unwrap();
        // Only the two client-exposed policies travel.
        assert_eq!(
            values.iter().map(|v| v.ptype).collect::<Vec<_>>(),
            alloc::vec![64, 65]
        );
        assert_eq!(values[0].pvalue, alloc::vec![0, 1]);
        assert_eq!(
            values[1].pvalue,
            alloc::vec![0, 0, 0, 0, 0, 0, 0, 2, 0, 4, 0, 6, 0, 1, 0, 9]
        );
        let peer = PeerZiop::from_policy_values(&values).unwrap();
        assert!(peer.compression_enabled);
        assert_eq!(peer.compressor_ids, own.compressor_ids);
        // Little endian, mixed with a policy of another type.
        let mut le = own.client_exposed(Endianness::Little).unwrap();
        le.push(PolicyValue {
            ptype: 33,
            pvalue: alloc::vec![0, 0],
        });
        assert_eq!(PeerZiop::from_policy_values(&le).unwrap(), peer);
        // The two policies that are not client-exposed have encodings too.
        assert_eq!(
            low_value_value(10, Endianness::Big).pvalue,
            alloc::vec![0, 0, 0, 0, 0, 0, 0, 10]
        );
        assert_eq!(
            min_ratio_value(0.5, Endianness::Big).pvalue,
            alloc::vec![0, 0, 0, 0, 0x3f, 0, 0, 0]
        );
        // A list that announces more entries than it holds.
        let bad = PolicyValue {
            ptype: 65,
            pvalue: alloc::vec![0, 0, 0, 0, 0, 0, 0, 9, 0, 4],
        };
        assert!(PeerZiop::from_policy_values(&[bad]).is_err());
    }
}
