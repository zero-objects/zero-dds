// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ObjectId — Spec §11.3.4.
//!
//! `typedef sequence<octet> ObjectId;` — opaque Bytes, vom POA
//! generiert (SYSTEM_ID) oder vom Caller geliefert (USER_ID).

use alloc::vec::Vec;

/// ObjectId — opaque Bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct ObjectId(pub Vec<u8>);

impl ObjectId {
    /// Konstruktor.
    #[must_use]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Konstruiert eine SYSTEM_ID aus einer monotonen u64. Das
    /// Format ist Implementation-Defined; wir nutzen 16 Bytes
    /// `[0xCAFE_F00D, 8 random bytes, 4 padding]`-aehnlich.
    /// In Praxis wird die u64-Sequenz als Big-Endian eingebettet,
    /// damit Nesting-Reihenfolge stabil ist.
    #[must_use]
    pub fn system_id(seq: u64) -> Self {
        let mut bytes = Vec::with_capacity(8);
        bytes.extend_from_slice(&seq.to_be_bytes());
        Self(bytes)
    }

    /// Bytes-Slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Laenge in Bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` wenn leer.
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
