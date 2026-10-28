// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteAnnotationType (XTypes 1.3 §7.3.4.4, Annex B).

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU8, Ordering};

use zerodds_wire::{Reader, WireError, Writer};

use crate::error::TypeCodecError;
use crate::type_object::common::{
    AnnotationParameterValue, MEMBER_NAME_MAX_LENGTH, QualifiedTypeName, TYPE_NAME_MAX_LENGTH,
    decode_seq_appendable, encode_seq_appendable, read_bounded_string, write_bounded_string,
};
use crate::type_object::flags::AnnotationTypeFlag;
use crate::type_object::minimal::annotation_type::CommonAnnotationParameter;

/// CompleteAnnotationHeader (DDS-XTypes 1.3, Annex B).
///
/// ```idl
/// @extensibility(APPENDABLE) @nested
/// struct CompleteAnnotationHeader {
///      QualifiedTypeName           annotation_name;
/// };
/// ```
///
/// One member. It is *not* the `CompleteTypeDetail` of a structure, a
/// union or a collection: an annotation type has no `ann_builtin` and no
/// `ann_custom` of its own, and the fully qualified name is all Annex B
/// gives here. `@extensibility(APPENDABLE)` means a `DHEADER` and a body
/// (XTypes 1.3 §7.4.3.4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAnnotationHeader {
    /// The fully qualified name of the annotation type.
    pub annotation_name: QualifiedTypeName,
}

impl CompleteAnnotationHeader {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            write_bounded_string(
                w,
                &self.annotation_name,
                TYPE_NAME_MAX_LENGTH,
                "QualifiedTypeName",
            )
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let annotation_name = crate::framing::decode_appendable(r, |r| {
            read_bounded_string(r, TYPE_NAME_MAX_LENGTH, "QualifiedTypeName")
        })?;
        Ok(Self { annotation_name })
    }
}

/// CompleteAnnotationParameter (DDS-XTypes 1.3, Annex B).
///
/// ```idl
/// @extensibility(APPENDABLE) @nested
/// struct CompleteAnnotationParameter {
///      CommonAnnotationParameter   common;
///      MemberName                  name;
///      AnnotationParameterValue    default_value;
/// };
/// ```
///
/// The same three members as the minimal form, with the parameter name in
/// full where the minimal one has its `NameHash`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAnnotationParameter {
    /// Flags and type.
    pub common: CommonAnnotationParameter,
    /// `MemberName` of the parameter.
    pub name: String,
    /// The parameter's default value, as Annex B's `union
    /// AnnotationParameterValue switch (octet)` writes it.
    pub default_value: AnnotationParameterValue,
}

impl CompleteAnnotationParameter {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            self.common.encode_into(w)?;
            write_bounded_string(w, &self.name, MEMBER_NAME_MAX_LENGTH, "MemberName")?;
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
            let name = read_bounded_string(r, MEMBER_NAME_MAX_LENGTH, "MemberName")?;
            let default_value = AnnotationParameterValue::decode_from(r)?;
            Ok(Self {
                common,
                name,
                default_value,
            })
        })
    }
}

/// CompleteAnnotationType (DDS-XTypes 1.3, Annex B).
///
/// ```idl
/// @extensibility(FINAL) @nested
/// struct CompleteAnnotationType {
///      AnnotationTypeFlag               annotation_flag;
///      CompleteAnnotationHeader         header;
///      CompleteAnnotationParameterSeq member_seq;
/// };
/// ```
///
/// `member_seq` is `sequence<CompleteAnnotationParameter>`, and XTypes 1.3
/// §7.4.3.5.3 rule (12) puts a `DHEADER` in front of a sequence's length.
/// Its order: see [`AnnotationParameterOrder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAnnotationType {
    /// `typedef TypeFlag`; Annex B's comment is "Unused. No flags apply".
    pub annotation_flag: AnnotationTypeFlag,
    /// Header: the annotation's fully qualified name.
    pub header: CompleteAnnotationHeader,
    /// The parameters, in the order [`AnnotationParameterOrder`] names.
    pub member_seq: Vec<CompleteAnnotationParameter>,
}

impl CompleteAnnotationType {
    pub(super) fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.annotation_flag.0);
        self.header.encode_into(w)?;
        encode_seq_appendable(w, &self.member_seq, |w, p| p.encode_into(w))
    }

    pub(super) fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let annotation_flag = AnnotationTypeFlag(r.read_u16()?);
        let header = CompleteAnnotationHeader::decode_from(r)?;
        let member_seq = decode_seq_appendable(r, CompleteAnnotationParameter::decode_from)?;
        Ok(Self {
            annotation_flag,
            header,
            member_seq,
        })
    }
}

/// The order of the elements of a `CompleteAnnotationParameterSeq`.
///
/// DDS-XTypes 1.3 gives two statements that disagree. §7.3.4.5 (S. 127):
/// "The elements in CompleteAnnotationMemberSeq shall be ordered in
/// increasing values of the member_index." Annex B, the comment behind
/// `typedef sequence<CompleteAnnotationParameter>
/// CompleteAnnotationParameterSeq`: "Ordered by
/// CompleteAnnotationParameter.name". The normative text is §7.3.4.5, so
/// [`Self::MemberIndex`] is the default; the Annex B comment is the named
/// setting [`Self::Name`] (`xtypes.annotation_parameter_order: name`).
///
/// `member_index` is the position of the parameter in the declaration of
/// the annotation type; `CommonAnnotationParameter` has no member id, so
/// the position is the only index a parameter has.
///
/// The order is part of the serialized object and therefore of its
/// `EquivalenceHash` and of the `annotation_typeid` of every application
/// of the annotation: two peers that differ in the setting compute
/// different identifiers for an annotation type whose declaration order is
/// not the order of the names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotationParameterOrder {
    /// §7.3.4.5: increasing `member_index`, the declaration order.
    #[default]
    MemberIndex,
    /// Annex B's comment: increasing `name`, lexicographic over its UTF-8
    /// octets.
    Name,
}

impl AnnotationParameterOrder {
    /// The spelling of the setting `xtypes.annotation_parameter_order`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MemberIndex => "member-index",
            Self::Name => "name",
        }
    }

    /// Reads the spelling of the setting.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "member-index" | "member_index" => Some(Self::MemberIndex),
            "name" => Some(Self::Name),
            _ => None,
        }
    }

    /// `parameters`, given in declaration order, in this order.
    #[must_use]
    pub fn ordered(
        self,
        parameters: Vec<CompleteAnnotationParameter>,
    ) -> Vec<CompleteAnnotationParameter> {
        match self {
            Self::MemberIndex => parameters,
            Self::Name => ordered_by_name(parameters),
        }
    }

    const fn to_byte(self) -> u8 {
        match self {
            Self::MemberIndex => 0,
            Self::Name => 1,
        }
    }

    const fn from_byte(byte: u8) -> Self {
        match byte {
            1 => Self::Name,
            _ => Self::MemberIndex,
        }
    }
}

static ANNOTATION_PARAMETER_ORDER: AtomicU8 = AtomicU8::new(0);

/// Chooses the order [`annotation_parameter_order`] answers, for every
/// `CompleteAnnotationType` this process builds from a model afterwards.
///
/// A process-wide setting, because the object is built deep inside the walk
/// of a type (`from_ast`), from every place an annotation is applied. Set it
/// once at start-up, before any `TypeObject` is built; a `TypeObject` built
/// earlier keeps the order it was built with.
pub fn set_annotation_parameter_order(order: AnnotationParameterOrder) {
    ANNOTATION_PARAMETER_ORDER.store(order.to_byte(), Ordering::Relaxed);
}

/// The order `from_ast` gives the parameters of a `CompleteAnnotationType`.
#[must_use]
pub fn annotation_parameter_order() -> AnnotationParameterOrder {
    AnnotationParameterOrder::from_byte(ANNOTATION_PARAMETER_ORDER.load(Ordering::Relaxed))
}

/// `member_seq` ordered by the parameter name, the reading of Annex B's
/// comment `// Ordered by CompleteAnnotationParameter.name`
/// ([`AnnotationParameterOrder::Name`]). `MemberName` is a `string`, so
/// "increasing values" is lexicographic over its UTF-8 octets.
#[must_use]
pub fn ordered_by_name(
    parameters: Vec<CompleteAnnotationParameter>,
) -> Vec<CompleteAnnotationParameter> {
    let mut parameters = parameters;
    parameters.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    parameters
}
