// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ObjectId — Spec §11.3.4.
//!
//! `typedef sequence<octet> ObjectId;` — opaque bytes, generated
//! by the POA (SYSTEM_ID) or supplied by the caller (USER_ID).

use alloc::vec::Vec;

/// ObjectId — opaque bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct ObjectId(pub Vec<u8>);

impl ObjectId {
    /// Constructor.
    #[must_use]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Constructs a SYSTEM_ID from a monotonic u64. The
    /// format is implementation-defined; we use something like 16
    /// bytes `[0xCAFE_F00D, 8 random bytes, 4 padding]`.
    /// In practice the u64 sequence is embedded big-endian
    /// so that nesting order stays stable.
    #[must_use]
    pub fn system_id(seq: u64) -> Self {
        let mut bytes = Vec::with_capacity(8);
        bytes.extend_from_slice(&seq.to_be_bytes());
        Self(bytes)
    }

    /// Byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<u8>> for ObjectId {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl From<&[u8]> for ObjectId {
    fn from(s: &[u8]) -> Self {
        Self(s.to_vec())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn system_id_is_8_bytes_big_endian() {
        let id = ObjectId::system_id(0x0102_0304_0506_0708);
        assert_eq!(
            id.as_bytes(),
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
    }

    #[test]
    fn user_id_round_trip() {
        let bytes = alloc::vec![b'U', b'S', b'E', b'R'];
        let id = ObjectId::new(bytes.clone());
        assert_eq!(id.as_bytes(), &bytes);
    }

    #[test]
    fn equality_uses_byte_content() {
        let a = ObjectId::from(b"abc".as_slice());
        let b = ObjectId::from(alloc::vec![b'a', b'b', b'c']);
        assert_eq!(a, b);
    }
}
