// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalAnnotationType (XTypes 1.3 §7.3.4.4, Annex B).

use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{
    AnnotationParameterValue, NameHash, decode_seq_appendable, encode_seq_appendable,
};
use crate::type_object::flags::{AnnotationParameterFlag, AnnotationTypeFlag};

/// CommonAnnotationParameter (DDS-XTypes 1.3, Annex B).
///
/// ```idl
/// @extensibility(FINAL) @nested
/// struct CommonAnnotationParameter {
///      AnnotationParameterFlag       member_flags;
///      TypeIdentifier                member_type_id;
/// };
/// ```
///
/// Annex B gives it two members and no `member_id`: an annotation
/// parameter is named, not numbered, and its name is what
/// [`MinimalAnnotationParameter::name_hash`] or
/// [`crate::type_object::complete::CompleteAnnotationParameter::name`]
/// carries. `@extensibility(FINAL)` means the two members go out as they
/// stand — no `DHEADER` (XTypes 1.3 §7.4.3.4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonAnnotationParameter {
    /// `typedef MemberFlag`; Annex B's comment is "Unused. No flags apply".
    pub member_flags: AnnotationParameterFlag,
    /// `TypeIdentifier` of the parameter.
    pub member_type_id: TypeIdentifier,
}

impl CommonAnnotationParameter {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.member_flags.0);
        self.member_type_id.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let member_flags = AnnotationParameterFlag(r.read_u16()?);
        let member_type_id = TypeIdentifier::decode_from(r)?;
        Ok(Self {
            member_flags,
            member_type_id,
        })
    }
}

/// MinimalAnnotationParameter (DDS-XTypes 1.3, Annex B).
///
/// ```idl
/// @extensibility(APPENDABLE) @nested
/// struct MinimalAnnotationParameter {
///      CommonAnnotationParameter   common;
///      NameHash                    name_hash;
///      AnnotationParameterValue    default_value;
/// };
/// ```
///
/// The parameter's name is a `NameHash` here and the full `MemberName` in
/// the complete form. `@extensibility(APPENDABLE)` means a `DHEADER` and a
/// body (XTypes 1.3 §7.4.3.4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalAnnotationParameter {
    /// Flags and type.
    pub common: CommonAnnotationParameter,
    /// `NameHash` of the parameter name: the first four octets of
    /// `MD5(name)` over its UTF-8 bytes (XTypes 1.3 §7.3.4.5).
    pub name_hash: NameHash,
    /// The parameter's default value, as Annex B's `union
    /// AnnotationParameterValue switch (octet)` writes it.
    pub default_value: AnnotationParameterValue,
}

impl MinimalAnnotationParameter {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            self.common.encode_into(w)?;
            self.name_hash.encode_into(w)?;
            self.default_value.encode_into(w)
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let common = CommonAnnotationParameter::decode_from(r)?;
            let name_hash = NameHash::decode_from(r)?;
            let default_value = AnnotationParameterValue::decode_from(r)?;
            Ok(Self {
                common,
                name_hash,
                default_value,
            })
        })
    }
}

/// MinimalAnnotationType (DDS-XTypes 1.3, Annex B).
///
/// ```idl
/// @extensibility(FINAL) @nested
/// struct MinimalAnnotationType {
///      AnnotationTypeFlag               annotation_flag;
///      MinimalAnnotationHeader          header;
///      MinimalAnnotationParameterSeq    member_seq;
/// };
/// ```
///
/// `MinimalAnnotationHeader` is `@extensibility(APPENDABLE)` and empty
/// ("// Empty. Available for future extension"), so it is a `DHEADER` of
/// zero and carries no field here — the same treatment
/// [`crate::type_object::minimal::MinimalUnionType`] gives
/// `MinimalUnionHeader`, whose bare `DHEADER` Cyclone and Fast DDS emit.
///
/// `member_seq` is `sequence<MinimalAnnotationParameter>`, and XTypes 1.3
/// §7.4.3.5.3 rule (12) puts a `DHEADER` in front of a sequence's length.
/// Its order is not this crate's: see [`ordered_by_name_hash`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalAnnotationType {
    /// `typedef TypeFlag`; Annex B's comment is "Unused. No flags apply".
    pub annotation_flag: AnnotationTypeFlag,
    /// The parameters, in the order [`ordered_by_name_hash`] puts them in.
    pub member_seq: Vec<MinimalAnnotationParameter>,
}

impl MinimalAnnotationType {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.annotation_flag.0);
        // MinimalAnnotationHeader: @appendable and empty → a bare DHEADER.
        crate::framing::encode_appendable(w, |_w| Ok(()))?;
        encode_seq_appendable(w, &self.member_seq, |w, p| p.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let annotation_flag = AnnotationTypeFlag(r.read_u16()?);
        crate::framing::decode_appendable(r, |_r| Ok(()))?;
        let member_seq = decode_seq_appendable(r, MinimalAnnotationParameter::decode_from)?;
        Ok(Self {
            annotation_flag,
            member_seq,
        })
    }
}

/// `member_seq` in the order DDS-XTypes 1.3 requires of it.
///
/// §7.3.4.5 (TypeObject serialization): "The elements in
/// MinimalAnnotationMemberSeq shall be ordered in increasing values of the
/// member_name hash", and Annex B's comment on `MinimalAnnotationParameterSeq`
/// reads `// Ordered by MinimalAnnotationParameter.name_hash`. `NameHash` is
/// `octet[4]` (XTypes 1.3 §7.3.4.5), so "increasing values" is read
/// lexicographically over the four octets — the hash as a big-endian number.
///
/// Every builder of a [`MinimalAnnotationType`] runs its `member_seq`
/// through this. The order is part of the serialized form, and the
/// `EquivalenceHash` is taken over exactly those octets (§7.3.1.2.1), so a
/// declaration order that the rule does not allow would give a hash no peer
/// can reproduce.
#[must_use]
pub fn ordered_by_name_hash(
    parameters: Vec<MinimalAnnotationParameter>,
) -> Vec<MinimalAnnotationParameter> {
    let mut parameters = parameters;
    parameters.sort_by(|a, b| a.name_hash.cmp(&b.name_hash));
    parameters
}
