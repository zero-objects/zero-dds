// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteSequence/Array/MapType (XTypes §7.3.4.4).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{
    AppliedBuiltinMemberAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq, decode_seq,
    encode_seq,
};
use crate::type_object::flags::{CollectionElementFlag, CollectionTypeFlag};
use crate::type_object::minimal::CommonCollectionElement;

/// CompleteCollectionElement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteCollectionElement {
    /// Common (flags + type).
    pub common: CommonCollectionElement,
    /// Builtin-Annotations auf dem Element.
    pub ann_builtin: AppliedBuiltinMemberAnnotations,
    /// Custom.
    pub ann_custom: OptionalAppliedAnnotationSeq,
}

fn encode_complete_collection_element(
    w: &mut BufferWriter,
    e: &CompleteCollectionElement,
) -> Result<(), EncodeError> {
    w.write_u16(e.common.element_flags.0)?;
    e.common.type_id.encode_into(w)?;
    e.ann_builtin.encode_into(w)?;
    e.ann_custom.encode_into(w)
}

fn decode_complete_collection_element(
    r: &mut BufferReader<'_>,
) -> Result<CompleteCollectionElement, TypeCodecError> {
    let element_flags = CollectionElementFlag(r.read_u16()?);
    let type_id = TypeIdentifier::decode_from(r)?;
    let ann_builtin = AppliedBuiltinMemberAnnotations::decode_from(r)?;
    let ann_custom = OptionalAppliedAnnotationSeq::decode_from(r)?;
    Ok(CompleteCollectionElement {
        common: CommonCollectionElement {
            element_flags,
            type_id,
        },
        ann_builtin,
        ann_custom,
    })
}

/// CompleteSequenceType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteSequenceType {
    /// Flags.
    pub collection_flag: CollectionTypeFlag,
    /// Max size.
    pub bound: u32,
    /// Header detail.
    pub detail: CompleteTypeDetail,
    /// Element.
    pub element: CompleteCollectionElement,
}

impl CompleteSequenceType {
    pub(super) fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.collection_flag.0)?;
        w.write_u32(self.bound)?;
        self.detail.encode_into(w)?;
        encode_complete_collection_element(w, &self.element)
    }

    pub(super) fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let bound = r.read_u32()?;
        let detail = CompleteTypeDetail::decode_from(r)?;
        let element = decode_complete_collection_element(r)?;
        Ok(Self {
            collection_flag,
            bound,
            detail,
            element,
        })
    }
}

/// CompleteArrayType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteArrayType {
    /// Flags.
    pub collection_flag: CollectionTypeFlag,
    /// Dimensionen.
    pub bound_seq: Vec<u32>,
    /// Detail.
    pub detail: CompleteTypeDetail,
    /// Element.
    pub element: CompleteCollectionElement,
}

impl CompleteArrayType {
    pub(super) fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.collection_flag.0)?;
        encode_seq(w, &self.bound_seq, |w, dim| w.write_u32(*dim))?;
        self.detail.encode_into(w)?;
        encode_complete_collection_element(w, &self.element)
    }

    pub(super) fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let bound_seq = decode_seq(r, |r| r.read_u32())?;
        let detail = CompleteTypeDetail::decode_from(r)?;
        let element = decode_complete_collection_element(r)?;
        Ok(Self {
            collection_flag,
            bound_seq,
            detail,
            element,
        })
    }
}

/// CompleteMapType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteMapType {
    /// Flags.
    pub collection_flag: CollectionTypeFlag,
    /// Max size.
    pub bound: u32,
    /// Detail.
    pub detail: CompleteTypeDetail,
    /// Key.
    pub key: CompleteCollectionElement,
    /// Value.
    pub element: CompleteCollectionElement,
}

impl CompleteMapType {
    pub(super) fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.collection_flag.0)?;
        w.write_u32(self.bound)?;
        self.detail.encode_into(w)?;
        encode_complete_collection_element(w, &self.key)?;
        encode_complete_collection_element(w, &self.element)
    }

    pub(super) fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let bound = r.read_u32()?;
        let detail = CompleteTypeDetail::decode_from(r)?;
        let key = decode_complete_collection_element(r)?;
        let element = decode_complete_collection_element(r)?;
        Ok(Self {
            collection_flag,
            bound,
            detail,
            key,
            element,
        })
    }
}
