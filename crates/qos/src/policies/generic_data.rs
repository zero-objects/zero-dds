// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! User/Group/Topic DataQosPolicy (DDS 1.4 §2.2.3.1–3).
//!
//! All three share the same wire form: opaque `sequence<octet>` =
//! u32 length + N × octet (plus CDR alignment padding to 4 bytes).

use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

/// DoS cap per opaque data block. 64 KiB covers realistic
/// UserData/TopicData/GroupData payloads (usually <1 KiB); a larger
/// one is refused. The spec sets no hard limit, but Cyclone DDS uses
/// ~1 MiB as "reasonable".
pub const MAX_OPAQUE_LEN: usize = 64 * 1024;

/// Shared wire form: opaque sequence<octet>.
///
/// # Errors
/// [`WireError::OutOfRange`] for a value whose length no `u32` holds.
/// Refused rather than truncated: a cast would put a length on the wire
/// that does not describe the bytes behind it.
fn encode_opaque(w: &mut Writer, bytes: &[u8]) -> Result<(), WireError> {
    let len = u32::try_from(bytes.len())
        .map_err(|_| WireError::refused(w.position(), "an opaque length a u32 can carry"))?;
    w.write_u32(len);
    w.write_array(bytes, |w, byte| {
        w.write_u8(*byte);
        Ok(())
    })
}

/// # Errors
/// [`WireError::OutOfRange`] above [`MAX_OPAQUE_LEN`], and whatever the
/// element read returns for a stream that ends early.
fn decode_opaque(r: &mut Reader<'_>) -> Result<Vec<u8>, WireError> {
    let at = r.position();
    let len = r.read_u32()? as usize;
    // Hard DoS cap: MAX_OPAQUE_LEN.
    if len > MAX_OPAQUE_LEN {
        return Err(WireError::refused(
            at,
            "an opaque block of at most MAX_OPAQUE_LEN octets",
        ));
    }
    // A length larger than the stream is a malformed stream, not a
    // short one, and `read_array` reserves against what is left rather
    // than against what the length claims.
    if len > r.remaining() {
        return Err(WireError::ImpossibleLength {
            at,
            claimed: len,
            available: r.remaining(),
        });
    }
    r.read_array(Reader::read_u8, len)
}

/// UserDataQosPolicy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserDataQosPolicy {
    /// Opaque Bytes.
    pub value: Vec<u8>,
}

impl UserDataQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// As `encode_opaque`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_opaque(w, &self.value)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            value: decode_opaque(r)?,
        })
    }
}

/// TopicDataQosPolicy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicDataQosPolicy {
    /// Opaque Bytes.
    pub value: Vec<u8>,
}

impl TopicDataQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// As `encode_opaque`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_opaque(w, &self.value)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            value: decode_opaque(r)?,
        })
    }
}

/// GroupDataQosPolicy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GroupDataQosPolicy {
    /// Opaque Bytes.
    pub value: Vec<u8>,
}

impl GroupDataQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// As `encode_opaque`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_opaque(w, &self.value)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            value: decode_opaque(r)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

    #[test]
    fn user_data_roundtrip() {
        let p = UserDataQosPolicy {
            value: alloc::vec![1, 2, 3, 4],
        };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(UserDataQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn empty_data_roundtrip() {
        let p = TopicDataQosPolicy::default();
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(TopicDataQosPolicy::decode_from(&mut r).unwrap(), p);
    }

    #[test]
    fn decoder_rejects_oversized_opaque() {
        let mut bytes = alloc::vec::Vec::new();
        bytes.extend_from_slice(&(MAX_OPAQUE_LEN as u32 + 1).to_le_bytes());
        let mut r = Reader::new(&bytes, Endianness::Little);
        let err = UserDataQosPolicy::decode_from(&mut r).unwrap_err();
        assert!(
            matches!(&err, WireError::OutOfRange { expected, .. }
                if expected.contains("MAX_OPAQUE_LEN")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn group_data_large_payload() {
        let p = GroupDataQosPolicy {
            value: (0..200u8).collect(),
        };
        let mut w = Writer::new(Endianness::Little);
        p.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(GroupDataQosPolicy::decode_from(&mut r).unwrap(), p);
    }
}
