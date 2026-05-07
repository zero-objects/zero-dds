// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalSequenceType / MinimalArrayType / MinimalMapType
//! (XTypes §7.3.4.4.6 / .7 / .8).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{decode_seq, encode_seq};
use crate::type_object::flags::{CollectionElementFlag, CollectionTypeFlag};

// ============================================================================
// Shared: CollectionElement (Common + Detail)
// ============================================================================

/// CommonCollectionElement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonCollectionElement {
    /// Flags.
    pub element_flags: CollectionElementFlag,
    /// Element-Typ.
    pub type_id: TypeIdentifier,
}

impl CommonCollectionElement {
    fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.element_flags.0)?;
        self.type_id.encode_into(w)
    }

    fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let element_flags = CollectionElementFlag(r.read_u16()?);
        let type_id = TypeIdentifier::decode_from(r)?;
        Ok(Self {
            element_flags,
            type_id,
        })
    }
}

/// MinimalCollectionElement = Common + (kein Detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalCollectionElement {
    /// Common.
    pub common: CommonCollectionElement,
}

impl MinimalCollectionElement {
    fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.common.encode_into(w)
    }

    fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            common: CommonCollectionElement::decode_from(r)?,
        })
    }
}

// ============================================================================
// MinimalSequenceType
// ============================================================================

/// MinimalSequenceType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalSequenceType {
    /// Flags.
    pub collection_flag: CollectionTypeFlag,
    /// Max-Groesse (0 = unbounded).
    pub bound: u32,
    /// Element.
    pub element: MinimalCollectionElement,
}

impl MinimalSequenceType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.collection_flag.0)?;
        w.write_u32(self.bound)?;
        self.element.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let bound = r.read_u32()?;
        let element = MinimalCollectionElement::decode_from(r)?;
        Ok(Self {
            collection_flag,
            bound,
            element,
        })
    }
}

// ============================================================================
// MinimalArrayType
// ============================================================================

/// MinimalArrayType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalArrayType {
    /// Flags.
    pub collection_flag: CollectionTypeFlag,
    /// Dimensionen (mindestens 1).
    pub bound_seq: Vec<u32>,
    /// Element.
    pub element: MinimalCollectionElement,
}

impl MinimalArrayType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.collection_flag.0)?;
        encode_seq(w, &self.bound_seq, |w, dim| w.write_u32(*dim))?;
        self.element.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let bound_seq = decode_seq(r, |r| r.read_u32())?;
        let element = MinimalCollectionElement::decode_from(r)?;
        Ok(Self {
            collection_flag,
            bound_seq,
            element,
        })
    }
}

// ============================================================================
// MinimalMapType
// ============================================================================

/// MinimalMapType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalMapType {
    /// Flags.
    pub collection_flag: CollectionTypeFlag,
    /// Max-Groesse.
    pub bound: u32,
    /// Key-Element.
    pub key: MinimalCollectionElement,
    /// Value-Element.
    pub element: MinimalCollectionElement,
}

impl MinimalMapType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.collection_flag.0)?;
        w.write_u32(self.bound)?;
        self.key.encode_into(w)?;
        self.element.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let bound = r.read_u32()?;
        let key = MinimalCollectionElement::decode_from(r)?;
        let element = MinimalCollectionElement::decode_from(r)?;
        Ok(Self {
            collection_flag,
            bound,
            key,
            element,
        })
    }
}
