// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP message header — 12 bytes, spec §15.4.1.
//!
//! ```text
//! struct MessageHeader {
//!     char           magic[4];        // "GIOP" = 47 49 4F 50
//!     octet          GIOP_version_major;
//!     octet          GIOP_version_minor;
//!     octet          flags;           // GIOP 1.0: byte_order
//!     octet          message_type;
//!     unsigned long  message_size;    // body size, in bytes
//! };
//! ```
//!
//! Total: 12 bytes. `message_size` is encoded with the header
//! endianness (bit 0 of `flags`).

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

use crate::error::{GiopError, GiopResult};
use crate::flags::Flags;
use crate::message_type::MessageType;
use crate::version::Version;

/// `MAGIC` — 4-byte ASCII `"GIOP"` (spec §15.4.1).
pub const MAGIC_BYTES: [u8; 4] = *b"GIOP";

/// `MAGIC` as a constant for tests.
pub const MAGIC: u32 = u32::from_be_bytes(MAGIC_BYTES);

/// GIOP header size in bytes (always 12).
pub const HEADER_SIZE: usize = 12;

/// GIOP message header (spec §15.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    /// GIOP version (spec §15.4.1).
    pub version: Version,
    /// Flags octet (in GIOP 1.0 only `byte_order`).
    pub flags: Flags,
    /// Message-type discriminant.
    pub message_type: MessageType,
    /// `message_size` — body size in bytes (excluding the header).
    pub message_size: u32,
}

impl MessageHeader {
    /// Constructor.
    #[must_use]
    pub const fn new(
        version: Version,
        flags: Flags,
        message_type: MessageType,
        message_size: u32,
    ) -> Self {
        Self {
            version,
            flags,
            message_type,
            message_size,
        }
    }

    /// Returns the endianness from the `flags` octet (bit 0).
    #[must_use]
    pub const fn endianness(&self) -> Endianness {
        self.flags.endianness()
    }

    /// Encodes the 12-byte header.
    ///
    /// # Errors
    /// Buffer write error from the CDR layer.
    pub fn encode(&self, w: &mut BufferWriter) -> GiopResult<()> {
        // Spec §15.4.1: the first four bytes are the `magic` as a sequence
        // of `char` — so no CDR length prefix, just raw bytes.
        // We write here without alignment, because the header sits at the
        // start of the stream (pos=0, alignment irrelevant).
        w.write_bytes(&MAGIC_BYTES)?;
        w.write_u8(self.version.major)?;
        w.write_u8(self.version.minor)?;
        w.write_u8(self.flags.0)?;
        w.write_u8(self.message_type.as_u8())?;
        // `message_size` is `unsigned long` with the header endianness.
        // The buffer was constructed with this endianness.
        w.write_u32(self.message_size)?;
        Ok(())
    }

    /// Decodes the 12-byte header.
    ///
    /// **Important:** the caller must create the `BufferReader` with
    /// [`Endianness::Big`] — the first 8 bytes
    /// (magic+version+flags+msg_type) are endian-free, but the
    /// `message_size` `u32` must be re-interpreted based on the `flags`
    /// byte. This helper reads the `flags` byte and flips the endianness
    /// if necessary.
    ///
    /// # Errors
    /// * `InvalidMagic` if bytes 0..4 are not `"GIOP"`.
    /// * `UnsupportedVersion` if major/minor is none of 1.0/1.1/1.2.
    /// * `UnknownMessageType` if the octet is not in 0..=7.
    /// * Buffer read error.
    pub fn decode(bytes: &[u8]) -> GiopResult<(Self, &[u8])> {
        if bytes.len() < HEADER_SIZE {
            return Err(GiopError::Malformed(alloc::format!(
                "header needs {HEADER_SIZE} bytes, got {}",
                bytes.len()
            )));
        }
        // Magic + version + flags + message_type are endian-free
        // (bytes 0..8). We start with BigEndian, then adjust.
        let mut tmp = BufferReader::new(&bytes[..8], Endianness::Big);
        let magic = tmp.read_bytes(4)?;
        if magic != MAGIC_BYTES {
            let mut m = [0u8; 4];
            m.copy_from_slice(magic);
            return Err(GiopError::InvalidMagic(m));
        }
        let major = tmp.read_u8()?;
        let minor = tmp.read_u8()?;
        let version = Version::new(major, minor);
        // Spec §15.4.1: only 1.0, 1.1, 1.2 are currently standardized.
        if !matches!((major, minor), (1, 0) | (1, 1) | (1, 2)) {
            return Err(GiopError::UnsupportedVersion { major, minor });
        }
        let flags_byte = tmp.read_u8()?;
        let flags = Flags(flags_byte);
        // Fragment-bit constraint (spec §15.4.9).
        if flags.has_more_fragments() && !version.supports_fragments() {
            return Err(GiopError::FragmentNotSupported { major, minor });
        }
        let mt_byte = tmp.read_u8()?;
        let message_type = MessageType::from_u8(mt_byte)?;

        // Now read the `message_size` u32 with the determined endianness.
        let mut size_reader = BufferReader::new(&bytes[8..12], flags.endianness());
        let message_size = size_reader.read_u32()?;
        Ok((
            Self {
                version,
                flags,
                message_type,
                message_size,
            },
            &bytes[HEADER_SIZE..],
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn magic_bytes_match_spec() {
        // Spec §15.4.1: "GIOP" = 0x47 0x49 0x4F 0x50.
        assert_eq!(MAGIC_BYTES, [0x47, 0x49, 0x4F, 0x50]);
    }

    #[test]
    fn header_size_is_12_bytes() {
        // Spec §15.4.1: the header is always 12 bytes.
        assert_eq!(HEADER_SIZE, 12);
    }

    #[test]
    fn round_trip_big_endian_request_header() {
        let h = MessageHeader::new(
            Version::V1_2,
            Flags::from_endianness(Endianness::Big),
            MessageType::Request,
            42,
        );
        let mut w = BufferWriter::new(Endianness::Big);
        h.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), HEADER_SIZE);
        // Check magic + version + flags + msg_type + size (BE).
        assert_eq!(&bytes[0..4], b"GIOP");
        assert_eq!(bytes[4], 1); // major
        assert_eq!(bytes[5], 2); // minor
        assert_eq!(bytes[6], 0); // flags=BE
        assert_eq!(bytes[7], MessageType::Request.as_u8());
        assert_eq!(&bytes[8..12], &[0, 0, 0, 42]); // BE u32

        let (decoded, rest) = MessageHeader::decode(&bytes).unwrap();
        assert_eq!(decoded, h);
        assert!(rest.is_empty());
    }

    #[test]
    fn round_trip_little_endian_reply_header() {
        let h = MessageHeader::new(
            Version::V1_1,
            Flags::from_endianness(Endianness::Little),
            MessageType::Reply,
            0x1234_5678,
        );
        let mut w = BufferWriter::new(Endianness::Little);
        h.encode(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[0..4], b"GIOP");
        assert_eq!(bytes[4], 1);
        assert_eq!(bytes[5], 1);
        assert_eq!(bytes[6], Flags::BYTE_ORDER_BIT);
        assert_eq!(bytes[7], MessageType::Reply.as_u8());
        // LE u32 bytes of 0x12345678 = 78 56 34 12.
        assert_eq!(&bytes[8..12], &[0x78, 0x56, 0x34, 0x12]);
        let (decoded, _) = MessageHeader::decode(&bytes).unwrap();
        assert_eq!(decoded, h);
    }

    #[test]
    fn invalid_magic_yields_diagnostic() {
        let bytes = [
            b'X', b'X', b'X', b'X', // bad magic
            1, 2, 0, 0, 0, 0, 0, 42,
        ];
        let err = MessageHeader::decode(&bytes).unwrap_err();
        assert!(matches!(
            err,
            GiopError::InvalidMagic([b'X', b'X', b'X', b'X'])
        ));
    }

    #[test]
    fn unsupported_version_is_diagnostic() {
        let bytes = [
            b'G', b'I', b'O', b'P', 2, 0, // 2.0 not standardized
            0, 0, 0, 0, 0, 0,
        ];
        let err = MessageHeader::decode(&bytes).unwrap_err();
        assert!(matches!(
            err,
            GiopError::UnsupportedVersion { major: 2, minor: 0 }
        ));
    }

    #[test]
    fn fragment_bit_in_giop_1_0_is_rejected() {
        // Spec §15.4.9: the fragment bit exists only from GIOP 1.1 onward.
        let bytes = [
            b'G',
            b'I',
            b'O',
            b'P',
            1,
            0,                   // GIOP 1.0
            Flags::FRAGMENT_BIT, // fragment bit set — illegal
            MessageType::Request.as_u8(),
            0,
            0,
            0,
            0,
        ];
        let err = MessageHeader::decode(&bytes).unwrap_err();
        assert!(matches!(
            err,
            GiopError::FragmentNotSupported { major: 1, minor: 0 }
        ));
    }

    #[test]
    fn truncated_input_is_malformed() {
        let bytes = [b'G', b'I', b'O', b'P'];
        let err = MessageHeader::decode(&bytes).unwrap_err();
        assert!(matches!(err, GiopError::Malformed(_)));
    }

    #[test]
    fn unknown_message_type_propagates() {
        let bytes = [
            b'G', b'I', b'O', b'P', 1, 2, 0, 99, // out of range
            0, 0, 0, 0,
        ];
        let err = MessageHeader::decode(&bytes).unwrap_err();
        assert!(matches!(err, GiopError::UnknownMessageType(99)));
    }
}
