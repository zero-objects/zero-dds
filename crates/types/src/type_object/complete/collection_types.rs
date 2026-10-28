// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteSequence/Array/MapType (XTypes §7.3.4.4).
//!
//! The framing is Annex B's, member for member:
//!
//! ```idl
//! @final      struct CompleteElementDetail     { @optional AppliedBuiltinMemberAnnotations ann_builtin;
//!                                                @optional AppliedAnnotationSeq ann_custom; };
//! @appendable struct CompleteCollectionElement { CommonCollectionElement common;
//!                                                CompleteElementDetail detail; };
//! @appendable struct CompleteCollectionHeader  { CommonCollectionHeader common;
//!                                                @optional CompleteTypeDetail detail; };
//! @final      struct CompleteSequenceType      { CollectionTypeFlag collection_flag;
//!                                                CompleteCollectionHeader header;
//!                                                CompleteCollectionElement element; };
//! @appendable struct CompleteArrayHeader       { CommonArrayHeader common; CompleteTypeDetail detail; };
//! @appendable struct CompleteArrayType         { CollectionTypeFlag collection_flag;
//!                                                CompleteArrayHeader header;
//!                                                CompleteCollectionElement element; };
//! @final      struct CompleteMapType           { CollectionTypeFlag collection_flag;
//!                                                CompleteCollectionHeader header;
//!                                                CompleteCollectionElement key;
//!                                                CompleteCollectionElement element; };
//! ```
//!
//! Under XCDR2 each appendable one is a `DHEADER` and a body; the header's
//! `detail` is "not present for anonymous" (the comment in Annex B), which
//! every sequence and map of IDL is. Byte-verified against the serializer of
//! Cyclone DDS 11.0.1 (`tests/typeobject_collections_annex_b.rs`).

use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use crate::error::TypeCodecError;
use crate::framing::{decode_appendable, decode_optional, encode_appendable, encode_optional};
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{
    AppliedBuiltinMemberAnnotations, CompleteTypeDetail, OptionalAppliedAnnotationSeq, decode_seq,
    encode_seq,
};
use crate::type_object::flags::{CollectionElementFlag, CollectionTypeFlag};
use crate::type_object::minimal::CommonCollectionElement;

/// CompleteCollectionElement: the element's flags and type, and the
/// `CompleteElementDetail` with its two `@optional` annotation members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteCollectionElement {
    /// Common (flags + type).
    pub common: CommonCollectionElement,
    /// Builtin annotations on the element (`CompleteElementDetail::ann_builtin`).
    pub ann_builtin: AppliedBuiltinMemberAnnotations,
    /// Custom (`CompleteElementDetail::ann_custom`).
    pub ann_custom: OptionalAppliedAnnotationSeq,
}

impl CompleteCollectionElement {
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        encode_appendable(w, |w| {
            w.write_u16(self.common.element_flags.0);
            self.common.type_id.encode_into(w)?;
            // CompleteElementDetail: `ann_builtin` is member 0, `ann_custom` 1.
            self.ann_builtin.encode_as_member(w, 0)?;
            self.ann_custom.encode_as_member(w, 1)
        })
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        Ok(decode_appendable(r, |r| {
            let element_flags = CollectionElementFlag(r.read_u16()?);
            let type_id = TypeIdentifier::decode_from(r)?;
            let ann_builtin = AppliedBuiltinMemberAnnotations::decode_as_member(r, 0)?;
            let ann_custom = OptionalAppliedAnnotationSeq::decode_as_member(r, 1)?;
            Ok(Self {
                common: CommonCollectionElement {
                    element_flags,
                    type_id,
                },
                ann_builtin,
                ann_custom,
            })
        })?)
    }
}

/// CompleteCollectionHeader: the bound and the `@optional` detail
/// (member 1), as a sequence and a map carry it.
fn encode_collection_header(
    w: &mut Writer,
    bound: u32,
    detail: Option<&CompleteTypeDetail>,
) -> Result<(), WireError> {
    encode_appendable(w, |w| {
        w.write_u32(bound);
        encode_optional(w, 1, detail, |w, d| d.encode_into(w))
    })
}

fn decode_collection_header(
    r: &mut Reader<'_>,
) -> Result<(u32, Option<CompleteTypeDetail>), WireError> {
    decode_appendable(r, |r| {
        let bound = r.read_u32()?;
        let detail = decode_optional(r, 1, CompleteTypeDetail::decode_from)?;
        Ok((bound, detail))
    })
}

/// CompleteSequenceType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteSequenceType {
    /// Flags.
    pub collection_flag: CollectionTypeFlag,
    /// Max size (`header.common.bound`).
    pub bound: u32,
    /// `header.detail`: absent for an anonymous sequence, which every IDL
    /// sequence is.
    pub detail: Option<CompleteTypeDetail>,
    /// Element.
    pub element: CompleteCollectionElement,
}

impl CompleteSequenceType {
    pub(super) fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.collection_flag.0);
        encode_collection_header(w, self.bound, self.detail.as_ref())?;
        self.element.encode_into(w)
    }

    pub(super) fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let (bound, detail) = decode_collection_header(r)?;
        let element = CompleteCollectionElement::decode_from(r)?;
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
    /// Dimensions (`header.common.bound_seq`).
    pub bound_seq: Vec<u32>,
    /// `header.detail`, which an array header always has.
    pub detail: CompleteTypeDetail,
    /// Element.
    pub element: CompleteCollectionElement,
}

impl CompleteArrayType {
    pub(super) fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        // CompleteArrayType is appendable, unlike the sequence and the map.
        encode_appendable(w, |w| {
            w.write_u16(self.collection_flag.0);
            encode_appendable(w, |w| {
                encode_seq(w, &self.bound_seq, |w, dim| {
                    w.write_u32(*dim);
                    Ok(())
                })?;
                self.detail.encode_into(w)
            })?;
            self.element.encode_into(w)
        })
    }

    pub(super) fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        decode_appendable_with_codec(r, |r| {
            let collection_flag = CollectionTypeFlag(r.read_u16()?);
            let (bound_seq, detail) = decode_appendable(r, |r| {
                let bound_seq = decode_seq(r, |r| r.read_u32())?;
                let detail = CompleteTypeDetail::decode_from(r)?;
                Ok((bound_seq, detail))
            })?;
            let element = CompleteCollectionElement::decode_from(r)?;
            Ok(Self {
                collection_flag,
                bound_seq,
                detail,
                element,
            })
        })
    }
}

/// CompleteMapType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteMapType {
    /// Flags.
    pub collection_flag: CollectionTypeFlag,
    /// Max size (`header.common.bound`).
    pub bound: u32,
    /// `header.detail`: absent for an anonymous map, which every IDL map is.
    pub detail: Option<CompleteTypeDetail>,
    /// Key.
    pub key: CompleteCollectionElement,
    /// Value.
    pub element: CompleteCollectionElement,
}

impl CompleteMapType {
    pub(super) fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.collection_flag.0);
        encode_collection_header(w, self.bound, self.detail.as_ref())?;
        self.key.encode_into(w)?;
        self.element.encode_into(w)
    }

    pub(super) fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let collection_flag = CollectionTypeFlag(r.read_u16()?);
        let (bound, detail) = decode_collection_header(r)?;
        let key = CompleteCollectionElement::decode_from(r)?;
        let element = CompleteCollectionElement::decode_from(r)?;
        Ok(Self {
            collection_flag,
            bound,
            detail,
            key,
            element,
        })
    }
}

/// [`decode_appendable`] for a body that reports [`TypeCodecError`].
fn decode_appendable_with_codec<T>(
    r: &mut Reader<'_>,
    body: impl FnOnce(&mut Reader<'_>) -> Result<T, TypeCodecError>,
) -> Result<T, TypeCodecError> {
    crate::framing::decode_appendable_with(r, body)
}
