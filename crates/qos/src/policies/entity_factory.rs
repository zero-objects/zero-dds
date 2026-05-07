// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! EntityFactoryQosPolicy (DDS 1.4 §2.2.3.27).
//!
//! Wire-Format: bool (1 byte + 3 pad = 4 byte). Lokale Policy; wird
//! normalerweise nicht ueber Wire propagiert, nur der Vollstaendigkeit
//! halber implementiert.

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::wire_helpers::{read_bool_padded, write_bool_padded};

/// EntityFactoryQosPolicy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityFactoryQosPolicy {
    /// Entities werden bei Erzeugung automatisch enabled. Default: `true`.
    pub autoenable_created_entities: bool,
}

impl Default for EntityFactoryQosPolicy {
    fn default() -> Self {
        Self {
            autoenable_created_entities: true,
        }
    }
}

impl EntityFactoryQosPolicy {
    /// Wire-Encoding.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        write_bool_padded(w, self.autoenable_created_entities)
    }

    /// Wire-Decoding.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            autoenable_created_entities: read_bool_padded(r)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn default_is_true() {
        assert!(EntityFactoryQosPolicy::default().autoenable_created_entities);
    }

    #[test]
    fn roundtrip() {
        for v in [true, false] {
            let p = EntityFactoryQosPolicy {
                autoenable_created_entities: v,
            };
            let mut w = BufferWriter::new(Endianness::Little);
            p.encode_into(&mut w).unwrap();
            let bytes = w.into_bytes();
            let mut r = BufferReader::new(&bytes, Endianness::Little);
            assert_eq!(EntityFactoryQosPolicy::decode_from(&mut r).unwrap(), p);
        }
    }
}
