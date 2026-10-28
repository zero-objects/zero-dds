// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalSequenceType / MinimalArrayType / MinimalMapType
//! (XTypes §7.3.4.4.6 / .7 / .8).
//!
//! Annex B makes `MinimalCollectionElement`, `MinimalCollectionHeader` and
//! `MinimalArrayHeader` appendable and the three types around them final:
//! under XCDR2 the header and each element are a `DHEADER` and a body.
//! Byte-verified against the serializer of Cyclone DDS 11.0.1 under both
//! encoding versions (`tests/typeobject_collections_annex_b.rs`).

use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use crate::framing::{decode_appendable, encode_appendable};
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
    /// Element type.
    pub type_id: TypeIdentifier,
}

impl CommonCollectionElement {
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.element_flags.0);
        self.type_id.encode_into(w)
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let element_flags = CollectionElementFlag(r.read_u16()?);
        let type_id = TypeIdentifier::decode_from(r)?;
        Ok(Self {
            element_flags,
            type_id,
        })
    }
}

/// MinimalCollectionElement = Common + (no detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalCollectionElement {
    /// Common.
    pub common: CommonCollectionElement,
}

impl MinimalCollectionElement {
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_appendable(w, |w| self.common.encode_into(w))
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            common: decode_appendable(r, CommonCollectionElement::decode_from)?,
        })
    }
}

/// `MinimalCollectionHeader`: appendable around `CommonCollectionHeader`,
/// which is the bound.
fn encode_collection_header(w: &mut Writer, bound: u32) -> Result<(), WireError> {
    encode_appendable::<WireError>(w, |w| {
        w.write_u32(bound);
        Ok(())
    })
}

fn decode_collection_header(r: &mut Reader<'_>) -> Result<u32, WireError> {
    decode_appendable(r, |r| r.read_u32())
}

// ============================================================================
// MinimalSequenceType
// ============================================================================

/// MinimalSequenceType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalSequenceType {
    /// Flags.
    pub collection_flag: CollectionTypeFlag,
    /// Max size (0 = unbounded).
    pub bound: u32,
    /// Element.
    pub element: MinimalCollectionElement,
}

impl MinimalSequenceType {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.collection_flag.0);
        encode_collection_header(w, self.bound)?;
        self.element.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let bound = decode_collection_header(r)?;
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
    /// Dimensions (at least 1).
    pub bound_seq: Vec<u32>,
    /// Element.
    pub element: MinimalCollectionElement,
}

impl MinimalArrayType {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.collection_flag.0);
        // MinimalArrayHeader, appendable around `CommonArrayHeader`.
        encode_appendable(w, |w| {
            encode_seq(w, &self.bound_seq, |w, dim| {
                w.write_u32(*dim);
                Ok(())
            })
        })?;
        self.element.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let bound_seq = decode_appendable(r, |r| decode_seq(r, |r| r.read_u32()))?;
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
    /// Max size.
    pub bound: u32,
    /// Key element.
    pub key: MinimalCollectionElement,
    /// Value element.
    pub element: MinimalCollectionElement,
}

impl MinimalMapType {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.collection_flag.0);
        encode_collection_header(w, self.bound)?;
        self.key.encode_into(w)?;
        self.element.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let bound = decode_collection_header(r)?;
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
