// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Shared wire types for TypeObject (Minimal + Complete).
//!
//! XTypes §7.3.4.5 (CommonStructMember, NameHash, MemberId).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_float128::Float128;
use zerodds_wire::{
    Reader, WcharPosition, WireError, WirePosition, Writer, WstringLengthPolicy, WstringUnitOrder,
};

use crate::framing::{decode_optional, encode_optional};
use crate::type_identifier::TypeIdentifier;

use super::flags::{StructMemberFlag, UnionMemberFlag};

/// 32-bit member ID (§7.3.4.5). Either assigned explicitly via `@id(n)`
/// or hashed from the member name (`@autoid(HASH)`).
pub type MemberId = u32;

/// 4-byte name hash (§7.3.4.5 — "MD5(name)[0..4]").
///
/// Stored in the MinimalTypeObject instead of the full name, to
/// keep the payload small. In the CompleteTypeObject the full
/// name is additionally present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NameHash(pub [u8; 4]);

impl NameHash {
    /// Computes the 4-byte NameHash from a member/literal name.
    ///
    /// Spec §7.3.4.5: "the `name_hash` is computed as the first 4
    /// octets of the MD5 hash of the name, interpreted as ASCII/UTF-8".
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        let digest = zerodds_wire::md5(name.as_bytes());
        let out: [u8; 4] = [digest[0], digest[1], digest[2], digest[3]];
        Self(out)
    }

    /// Hash-derived member ID for `@autoid(HASH)` / `@hashid("hint")`
    /// (XTypes 1.3 §7.3.1.2.1.1 + §7.3.1.2.1.4).
    ///
    /// The member ID is the first 4 octets of `MD5(name)` interpreted as a
    /// little-endian `u32`, masked to the 28-bit member-ID space
    /// (`& 0x0FFFFFFF`). The top 4 bits are outside the member-ID range
    /// (the EMHEADER reserves them for the length-code on the wire), so the
    /// mask — not a shift — yields the canonical ID. `@autoid(HASH)` hashes
    /// the member name; `@hashid("hint")` hashes the given hint string.
    ///
    /// This is the single, central derivation the TypeObject builder and the
    /// codegen backends consume — never a per-backend re-implementation.
    #[must_use]
    pub fn member_id_from_name(name: &str) -> u32 {
        u32::from_le_bytes(Self::from_name(name).0) & 0x0FFF_FFFF
    }

    /// Encoded as `octet[4]` (4 bytes, no length, no padding).
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        {
            w.write_bytes(&self.0);
            Ok(())
        }
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let bytes = r.read_bytes(4)?;
        // `read_bytes(n)` is guaranteed to return exactly n bytes, otherwise
        // UnexpectedEof. The try_into to [u8; 4] is therefore infallible.
        let Ok(out): Result<[u8; 4], _> = bytes.try_into() else {
            return Err(WireError::Truncated {
                at: r.position(),
                needed: 4,
                available: 0,
            });
        };
        Ok(Self(out))
    }
}

/// CommonStructMember (Annex B).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonStructMember {
    /// Member-ID (4 byte).
    pub member_id: MemberId,
    /// Flags (IS_KEY, IS_OPTIONAL, etc.).
    pub member_flags: StructMemberFlag,
    /// Type of the member (may be recursive).
    pub member_type_id: TypeIdentifier,
}

impl CommonStructMember {
    /// Encoded as `{ u32 member_id; u16 member_flags; TypeIdentifier member_type_id; }`.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u32(self.member_id);
        w.write_u16(self.member_flags.0);
        self.member_type_id.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let member_id = r.read_u32()?;
        let member_flags = StructMemberFlag(r.read_u16()?);
        let member_type_id = TypeIdentifier::decode_from(r)?;
        Ok(Self {
            member_id,
            member_flags,
            member_type_id,
        })
    }
}

/// CommonUnionMember (§7.3.4.5.3). Additionally contains the label list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonUnionMember {
    /// Member ID.
    pub member_id: MemberId,
    /// Flags (IS_DEFAULT for the default case).
    pub member_flags: UnionMemberFlag,
    /// Type of the member.
    pub type_id: TypeIdentifier,
    /// Case labels as an `i32` sequence (Spec §7.3.4.5.3.2: `long[]`).
    pub label_seq: alloc::vec::Vec<i32>,
}

impl CommonUnionMember {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u32(self.member_id);
        w.write_u16(self.member_flags.0);
        self.type_id.encode_into(w)?;
        // sequence<long>: u32 length + N*i32
        let len = u32::try_from(self.label_seq.len()).map_err(|_| WireError::OutOfRange {
            at: w.position(),
            expected: alloc::string::String::from("union label sequence length exceeds u32::MAX"),
        })?;
        w.write_u32(len);
        for l in &self.label_seq {
            w.write_u32(*l as u32);
        }
        Ok(())
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let member_id = r.read_u32()?;
        let member_flags = UnionMemberFlag(r.read_u16()?);
        let type_id = TypeIdentifier::decode_from(r)?;
        let len = r.read_u32()? as usize;
        let cap = safe_capacity(len, 4, r.remaining());
        let mut label_seq = alloc::vec::Vec::with_capacity(cap);
        for _ in 0..len {
            label_seq.push(r.read_u32()? as i32);
        }
        Ok(Self {
            member_id,
            member_flags,
            type_id,
            label_seq,
        })
    }
}

// ============================================================================
// Complete-TypeObject-Annotations (§7.3.4.5.4)
// ============================================================================

/// Longest `MemberName` (DDS-XTypes 1.3 Annex B, p. 279:
/// `typedef string<MEMBER_NAME_MAX_LENGTH> MemberName`), in octets.
pub const MEMBER_NAME_MAX_LENGTH: usize = 256;

/// Longest `QualifiedTypeName` (Annex B, p. 279:
/// `typedef string<TYPE_NAME_MAX_LENGTH> QualifiedTypeName`), in octets.
pub const TYPE_NAME_MAX_LENGTH: usize = 256;

/// Longest string value of an `AnnotationParameterValue` (Annex B,
/// p. 286): `string<ANNOTATION_STR_VALUE_MAX_LEN>` and
/// `wstring<ANNOTATION_STR_VALUE_MAX_LEN>`, in octets and in UTF-16 code
/// units respectively.
pub const ANNOTATION_STR_VALUE_MAX_LEN: usize = 128;

/// Bound Annex B (p. 286) declares for an octet sequence value of an
/// annotation parameter. The union of Annex B has no case that uses it;
/// the constant is part of the declared interface.
pub const ANNOTATION_OCTETSEC_VALUE_MAX_LEN: usize = 128;

/// A bounded string in a `TypeObject`: refused above `bound` octets, on
/// writing and on reading, because `string<N>` holds at most `N`
/// characters (Annex B).
pub(crate) fn write_bounded_string(
    w: &mut Writer,
    value: &str,
    bound: usize,
    what: &'static str,
) -> Result<(), WireError> {
    if value.len() > bound {
        return Err(WireError::OutOfRange {
            at: w.position(),
            expected: alloc::format!(
                "a {what} of at most {bound} octets (Annex B), and this one has {}",
                value.len()
            ),
        });
    }
    w.write_string(value);
    Ok(())
}

/// The read side of [`write_bounded_string`].
pub(crate) fn read_bounded_string(
    r: &mut Reader<'_>,
    bound: usize,
    what: &'static str,
) -> Result<String, WireError> {
    let at = r.position();
    let value = r.read_string()?;
    if value.len() > bound {
        return Err(WireError::OutOfRange {
            at,
            expected: alloc::format!(
                "a {what} of at most {bound} octets (Annex B), and this one has {}",
                value.len()
            ),
        });
    }
    Ok(value)
}

/// Full qualified type name, e.g. "::sensors::Chatter". Alias for
/// `String` — on the wire as a CDR string (u32 length + UTF-8 + null-term)
/// of at most [`TYPE_NAME_MAX_LENGTH`] octets.
pub type QualifiedTypeName = String;

/// Placement of a `@verbatim` text (DDS-XTypes 1.3 §7.2.2.4.8.2).
///
/// On the wire the placement is the `string<32> placement` of Annex B's
/// `AppliedVerbatimAnnotation`. The six strings §7.2.2.4.8.2 names are
/// `begin-declaration-file`, `before-declaration`, `begin-declaration`,
/// `end-declaration`, `after-declaration` and `end-declaration-file`; they
/// are "interpreted in a case-insensitive manner", and "individual compiler
/// implementations may recognize others in addition" ([`Self::Other`]).
/// IDL 4.2 spells the same six as the enumerators `BEGIN_FILE`,
/// `BEFORE_DECLARATION`, `BEGIN_DECLARATION`, `END_DECLARATION`,
/// `AFTER_DECLARATION` and `END_FILE`; [`Self::parse`] reads those too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerbatimPlacement {
    /// `begin-declaration-file`: the start of the file with the declaration.
    BeginFile,
    /// `before-declaration`: immediately before the declaration.
    Before,
    /// `begin-declaration`: in the body, before the members.
    BeginDeclaration,
    /// `end-declaration`: in the body, after the members.
    EndDeclaration,
    /// `after-declaration`: immediately after the declaration.
    After,
    /// `end-declaration-file`: the end of the file with the declaration.
    EndFile,
    /// A placement an implementation recognizes in addition, as written.
    Other(String),
}

impl VerbatimPlacement {
    /// The string written on the wire.
    #[must_use]
    pub fn as_wire_str(&self) -> &str {
        match self {
            Self::BeginFile => "begin-declaration-file",
            Self::Before => "before-declaration",
            Self::BeginDeclaration => "begin-declaration",
            Self::EndDeclaration => "end-declaration",
            Self::After => "after-declaration",
            Self::EndFile => "end-declaration-file",
            Self::Other(written) => written.as_str(),
        }
    }

    /// Reads a placement string: the six of §7.2.2.4.8.2 in any case, the
    /// IDL 4.2 enumerators for them, anything else as [`Self::Other`].
    #[must_use]
    pub fn parse(written: &str) -> Self {
        let normalised = written.to_ascii_lowercase().replace('_', "-");
        match normalised.as_str() {
            "begin-declaration-file" | "begin-file" => Self::BeginFile,
            "before-declaration" => Self::Before,
            "begin-declaration" => Self::BeginDeclaration,
            "end-declaration" => Self::EndDeclaration,
            "after-declaration" => Self::After,
            "end-declaration-file" | "end-file" => Self::EndFile,
            _ => Self::Other(String::from(written)),
        }
    }
}

/// `@verbatim(language, text, placement)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedVerbatimAnnotation {
    /// Placement in code-gen output.
    pub placement: VerbatimPlacement,
    /// Target language (e.g., "c++").
    pub language: String,
    /// Literal text.
    pub text: String,
}

impl AppliedVerbatimAnnotation {
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        write_bounded_string(w, self.placement.as_wire_str(), 32, "placement")?;
        write_bounded_string(w, &self.language, 32, "language")?;
        w.write_string(&self.text);
        Ok(())
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let placement = VerbatimPlacement::parse(&read_bounded_string(r, 32, "placement")?);
        let language = read_bounded_string(r, 32, "language")?;
        let text = r.read_string()?;
        Ok(Self {
            placement,
            language,
            text,
        })
    }
}

/// AppliedBuiltinTypeAnnotations (DDS-XTypes 1.3, Annex B): `@verbatim`
/// at type level.
///
/// `@extensibility(APPENDABLE)` with one `@optional` member, and itself
/// `@optional` in `CompleteTypeDetail` and `CompleteDiscriminatorMember`.
/// [`Self::encode_as_member`] writes both optionals: under the encoding
/// version 2 absent is a single `0` (byte-verified against Cyclone DDS
/// and Fast DDS) and present is `1`, a `DHEADER`, `1`, then the
/// `AppliedVerbatimAnnotation`; under version 1 each is a parameter
/// header (see [`crate::framing::encode_optional`]). Further builtin
/// annotations (`@unit`, `@min`, `@max`, `@hashid`) belong to the member
/// scope, not the type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedBuiltinTypeAnnotations {
    /// Optional `@verbatim` directive.
    pub verbatim: Option<AppliedVerbatimAnnotation>,
}

impl AppliedBuiltinTypeAnnotations {
    /// The `@optional` member `member_id` of the enclosing structure
    /// that holds this record, absent when no annotation is set.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_as_member(&self, w: &mut Writer, member_id: u32) -> Result<(), WireError> {
        encode_optional(w, member_id, self.verbatim.as_ref(), |w, v| {
            crate::framing::encode_appendable(w, |w| {
                encode_optional(w, 0, Some(v), |w, v| v.encode_into(w))
            })
        })
    }

    /// Decode the form [`Self::encode_as_member`] writes.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_as_member(r: &mut Reader<'_>, member_id: u32) -> Result<Self, WireError> {
        let verbatim = decode_optional(r, member_id, |r| {
            crate::framing::decode_appendable(r, |r| {
                decode_optional(r, 0, AppliedVerbatimAnnotation::decode_from)
            })
        })?
        .flatten();
        Ok(Self { verbatim })
    }
}

/// AppliedAnnotationParameter (DDS-XTypes 1.3, Annex B): one parameter
/// of an applied custom annotation, named by the hash of its name.
///
/// `@extensibility(APPENDABLE)`: a `DHEADER`, the 4-octet `NameHash`,
/// then the value as the final union [`AnnotationParameterValue`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedAnnotationParameter {
    /// Hash of the parameter name.
    pub paramname_hash: NameHash,
    /// The value the parameter was applied with.
    pub value: AnnotationParameterValue,
}

impl AppliedAnnotationParameter {
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            self.paramname_hash.encode_into(w)?;
            self.value.encode_into(w)
        })
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let paramname_hash = NameHash::decode_from(r)?;
            let value = AnnotationParameterValue::decode_from(r)?;
            Ok(Self {
                paramname_hash,
                value,
            })
        })
    }
}

/// AppliedAnnotation (DDS-XTypes 1.3, Annex B): one applied custom
/// annotation.
///
/// `@extensibility(APPENDABLE)`: a `DHEADER`, the annotation type's
/// `TypeIdentifier`, then `@optional` `param_seq`. An empty `param_seq` is
/// written absent. No vendor capture of a custom annotation is recorded;
/// the form follows the IDL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedAnnotation {
    /// Type of the annotation (TypeIdentifier to the annotation definition).
    pub annotation_typeid: TypeIdentifier,
    /// Set parameters.
    pub param_seq: Vec<AppliedAnnotationParameter>,
}

impl AppliedAnnotation {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            self.annotation_typeid.encode_into(w)?;
            let param_seq = (!self.param_seq.is_empty()).then_some(self.param_seq.as_slice());
            encode_optional(w, 1, param_seq, |w, seq| {
                encode_seq_appendable(w, seq, |w, p| p.encode_into(w))
            })
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let annotation_typeid = TypeIdentifier::decode_from(r)?;
            let param_seq = decode_optional(r, 1, |r| {
                decode_seq_appendable(r, AppliedAnnotationParameter::decode_from)
            })?
            .unwrap_or_default();
            Ok(Self {
                annotation_typeid,
                param_seq,
            })
        })
    }
}

/// `@optional AppliedAnnotationSeq` (DDS-XTypes 1.3, Annex B): a presence
/// octet, then, when present, the sequence. Its elements are appendable,
/// so the sequence carries a `DHEADER` in front of its length (XTypes 1.3
/// §7.4.3.5.3). Absent is a single `0` (byte-verified against Cyclone DDS
/// and Fast DDS).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionalAppliedAnnotationSeq(pub Option<Vec<AppliedAnnotation>>);

impl OptionalAppliedAnnotationSeq {
    /// Encode as the `@optional` member `member_id` of the enclosing
    /// structure.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_as_member(&self, w: &mut Writer, member_id: u32) -> Result<(), WireError> {
        encode_optional(w, member_id, self.0.as_deref(), |w, seq| {
            encode_seq_appendable(w, seq, |w, a| a.encode_into(w))
        })
    }

    /// Decode the `@optional` member `member_id`.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_as_member(r: &mut Reader<'_>, member_id: u32) -> Result<Self, WireError> {
        decode_optional(r, member_id, |r| {
            decode_seq_appendable(r, AppliedAnnotation::decode_from)
        })
        .map(Self)
    }
}

/// CompleteTypeDetail (§7.3.4.5.4): ann_builtin + ann_custom + type_name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTypeDetail {
    /// Built-in annotations, `@verbatim` among them.
    pub ann_builtin: AppliedBuiltinTypeAnnotations,
    /// Custom annotations (optional).
    pub ann_custom: OptionalAppliedAnnotationSeq,
    /// Fully qualified type name (e.g. "::sensors::Chatter").
    pub type_name: QualifiedTypeName,
}

impl CompleteTypeDetail {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        self.ann_builtin.encode_as_member(w, 0)?;
        self.ann_custom.encode_as_member(w, 1)?;
        write_bounded_string(
            w,
            &self.type_name,
            TYPE_NAME_MAX_LENGTH,
            "QualifiedTypeName",
        )
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let ann_builtin = AppliedBuiltinTypeAnnotations::decode_as_member(r, 0)?;
        let ann_custom = OptionalAppliedAnnotationSeq::decode_as_member(r, 1)?;
        let type_name = read_bounded_string(r, TYPE_NAME_MAX_LENGTH, "QualifiedTypeName")?;
        Ok(Self {
            ann_builtin,
            ann_custom,
            type_name,
        })
    }
}

/// AnnotationParameterValue (DDS-XTypes 1.3 Annex B): the value an
/// annotation was applied with, discriminated by a `TypeKind` octet.
///
/// One variant per case of the union: the numeric primitives, `boolean`,
/// `char`, `wchar` (`TK_CHAR16`), `long double` (`TK_FLOAT128`), an
/// enumerator (`TK_ENUM`, as its `int32` value), a narrow string
/// (`TK_STRING8`), a wide string (`TK_STRING16`), and the `default` case
/// of the union, `ExtendedAnnotationParameterValue`, which Annex B
/// reserves for future extension.
///
/// Floats are held as IEEE-754 bits rather than as `f32`/`f64` so the
/// type can be `Eq`: two annotation values are the same annotation
/// value exactly when their octets are, which is also the only equality
/// the wire form can check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationParameterValue {
    /// `TK_BOOLEAN`.
    Boolean(bool),
    /// `TK_BYTE` (`octet`).
    Byte(u8),
    /// `TK_INT8`.
    Int8(i8),
    /// `TK_UINT8`.
    UInt8(u8),
    /// `TK_INT16`.
    Int16(i16),
    /// `TK_UINT16`.
    UInt16(u16),
    /// `TK_INT32`.
    Int32(i32),
    /// `TK_UINT32`.
    UInt32(u32),
    /// `TK_INT64`.
    Int64(i64),
    /// `TK_UINT64`.
    UInt64(u64),
    /// `TK_FLOAT32`, as its IEEE-754 bits.
    Float32Bits(u32),
    /// `TK_FLOAT64`, as its IEEE-754 bits.
    Float64Bits(u64),
    /// `TK_FLOAT128` (`long double`), as its IEEE-754 binary128 bits.
    Float128Bits(u128),
    /// `TK_CHAR8`.
    Char8(char),
    /// `TK_CHAR16` (`wchar`), a code point of the Basic Multilingual
    /// Plane (§7.2.2.2.1.2.5).
    Char16(char),
    /// `TK_ENUM`: an enumerator, as the `int32` §7.2.2.4.1.2 stores it.
    Enum(i32),
    /// `TK_STRING8`.
    String8(String),
    /// `TK_STRING16` (`wstring`).
    String16(String),
    /// The `default` case of the union: a discriminator that has no case
    /// of its own, with the `ExtendedAnnotationParameterValue` that
    /// follows it. That type is `@extensibility(MUTABLE)` and empty in
    /// this version of the specification, so `body` is what lies inside
    /// its `DHEADER` and is empty for every value written today; a
    /// later version's members are kept as the octets they were read as.
    Extended {
        /// The discriminator octet.
        kind: u8,
        /// The octets of the `ExtendedAnnotationParameterValue` after its
        /// `DHEADER`.
        body: Vec<u8>,
    },
}

impl AnnotationParameterValue {
    /// The `TypeKind` octet that discriminates this value.
    #[must_use]
    pub const fn kind(&self) -> u8 {
        use crate::type_identifier::kinds as tk;
        match self {
            Self::Boolean(_) => tk::TK_BOOLEAN,
            Self::Byte(_) => tk::TK_BYTE,
            Self::Int8(_) => tk::TK_INT8,
            Self::UInt8(_) => tk::TK_UINT8,
            Self::Int16(_) => tk::TK_INT16,
            Self::UInt16(_) => tk::TK_UINT16,
            Self::Int32(_) => tk::TK_INT32,
            Self::UInt32(_) => tk::TK_UINT32,
            Self::Int64(_) => tk::TK_INT64,
            Self::UInt64(_) => tk::TK_UINT64,
            Self::Float32Bits(_) => tk::TK_FLOAT32,
            Self::Float64Bits(_) => tk::TK_FLOAT64,
            Self::Float128Bits(_) => tk::TK_FLOAT128,
            Self::Char8(_) => tk::TK_CHAR8,
            Self::Char16(_) => tk::TK_CHAR16,
            Self::Enum(_) => super::kinds::TK_ENUM,
            Self::String8(_) => tk::TK_STRING8,
            Self::String16(_) => tk::TK_STRING16,
            Self::Extended { kind, .. } => *kind,
        }
    }

    /// Encode as the `@extensibility(FINAL)` union `switch (octet)` of
    /// DDS-XTypes 1.3 Annex B: the `TypeKind` octet, then the value,
    /// aligned within the enclosing stream. A final union has no
    /// `DHEADER`; its `default` case carries the `DHEADER` of the
    /// `@extensibility(MUTABLE)` `ExtendedAnnotationParameterValue`.
    ///
    /// # Errors
    /// A `char` outside the 8-bit range, a `wchar` outside the Basic
    /// Multilingual Plane, or an `Extended` whose discriminator is a case
    /// of its own.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u8(self.kind());
        match self {
            Self::Boolean(v) => w.write_bool(*v),
            Self::Byte(v) | Self::UInt8(v) => w.write_u8(*v),
            Self::Int8(v) => w.write_i8(*v),
            Self::Int16(v) => w.write_i16(*v),
            Self::UInt16(v) => w.write_u16(*v),
            Self::Int32(v) | Self::Enum(v) => w.write_i32(*v),
            Self::UInt32(v) => w.write_u32(*v),
            Self::Int64(v) => w.write_i64(*v),
            Self::UInt64(v) => w.write_u64(*v),
            Self::Float32Bits(bits) => w.write_f32(f32::from_bits(*bits)),
            Self::Float64Bits(bits) => w.write_f64(f64::from_bits(*bits)),
            Self::Float128Bits(bits) => w.write_f128(Float128::from_bits(*bits)),
            Self::Char8(v) => w.write_char(*v)?,
            Self::Char16(v) => w.write_wchar(*v, WcharPosition::Member)?,
            Self::String8(v) => write_bounded_string(
                w,
                v,
                ANNOTATION_STR_VALUE_MAX_LEN,
                "annotation string value",
            )?,
            Self::String16(v) => w.write_wstring_bounded(
                v,
                ANNOTATION_STR_VALUE_MAX_LEN,
                WstringLengthPolicy::SPECIFICATION,
                WstringUnitOrder::Stream,
                WirePosition::Member,
            )?,
            Self::Extended { kind, body } => {
                if Self::has_case(*kind) {
                    return Err(WireError::OutOfRange {
                        at: w.position(),
                        expected: alloc::format!(
                            "an ExtendedAnnotationParameterValue discriminator that is no case                              of the union, and 0x{kind:02x} is one"
                        ),
                    });
                }
                crate::framing::encode_extended_members(w, body)?;
            }
        }
        Ok(())
    }

    /// `true` if the union of Annex B has a case of its own for `kind`.
    #[must_use]
    pub const fn has_case(kind: u8) -> bool {
        use crate::type_identifier::kinds as tk;
        matches!(
            kind,
            tk::TK_BOOLEAN
                | tk::TK_BYTE
                | tk::TK_INT8
                | tk::TK_UINT8
                | tk::TK_INT16
                | tk::TK_UINT16
                | tk::TK_INT32
                | tk::TK_UINT32
                | tk::TK_INT64
                | tk::TK_UINT64
                | tk::TK_FLOAT32
                | tk::TK_FLOAT64
                | tk::TK_FLOAT128
                | tk::TK_CHAR8
                | tk::TK_CHAR16
                | super::kinds::TK_ENUM
                | tk::TK_STRING8
                | tk::TK_STRING16
        )
    }

    /// Read the form [`Self::encode_into`] writes.
    ///
    /// A discriminator without a case of its own is the `default` case of
    /// the union, an `ExtendedAnnotationParameterValue`: its `DHEADER` is
    /// read and its members are kept as [`Self::Extended`].
    ///
    /// # Errors
    /// A truncated value, or a `wchar` that is not a BMP code point.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        use crate::type_identifier::kinds as tk;
        let kind = r.read_u8()?;
        Ok(match kind {
            tk::TK_BOOLEAN => Self::Boolean(r.read_bool()?),
            tk::TK_BYTE => Self::Byte(r.read_u8()?),
            tk::TK_INT8 => Self::Int8(r.read_i8()?),
            tk::TK_UINT8 => Self::UInt8(r.read_u8()?),
            tk::TK_INT16 => Self::Int16(r.read_i16()?),
            tk::TK_UINT16 => Self::UInt16(r.read_u16()?),
            tk::TK_INT32 => Self::Int32(r.read_i32()?),
            tk::TK_UINT32 => Self::UInt32(r.read_u32()?),
            tk::TK_INT64 => Self::Int64(r.read_i64()?),
            tk::TK_UINT64 => Self::UInt64(r.read_u64()?),
            tk::TK_FLOAT32 => Self::Float32Bits(r.read_f32()?.to_bits()),
            tk::TK_FLOAT64 => Self::Float64Bits(r.read_f64()?.to_bits()),
            tk::TK_FLOAT128 => Self::Float128Bits(r.read_f128()?.to_bits()),
            tk::TK_CHAR8 => Self::Char8(r.read_char()?),
            tk::TK_CHAR16 => Self::Char16(r.read_wchar(WcharPosition::Member)?),
            super::kinds::TK_ENUM => Self::Enum(r.read_i32()?),
            tk::TK_STRING8 => Self::String8(read_bounded_string(
                r,
                ANNOTATION_STR_VALUE_MAX_LEN,
                "annotation string value",
            )?),
            tk::TK_STRING16 => {
                let at = r.position();
                let value = r.read_wstring(
                    WstringLengthPolicy::SPECIFICATION,
                    WstringUnitOrder::Stream,
                    WirePosition::Member,
                )?;
                let units = value.encode_utf16().count();
                if units > ANNOTATION_STR_VALUE_MAX_LEN {
                    return Err(WireError::OutOfRange {
                        at,
                        expected: alloc::format!(
                            "an annotation wstring value of at most \
                             {ANNOTATION_STR_VALUE_MAX_LEN} code units (Annex B), and this \
                             one has {units}"
                        ),
                    });
                }
                Self::String16(value)
            }
            other => Self::Extended {
                kind: other,
                body: crate::framing::decode_extended_members(r)?,
            },
        })
    }

    /// The value on its own, as the stream [`Self::encode_into`] writes
    /// from offset zero.
    ///
    /// # Errors
    /// As [`Self::encode_into`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, WireError> {
        let mut w = crate::framing::xcdr2_writer();
        self.encode_into(&mut w)?;
        Ok(w.finish())
    }

    /// Read the form [`Self::to_bytes`] writes.
    ///
    /// # Errors
    /// As [`Self::decode_from`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        Self::decode_from(&mut crate::framing::xcdr2_reader(bytes))
    }
}

/// AppliedBuiltinMemberAnnotations (DDS-XTypes 1.3, Annex B): the
/// builtin annotations of one member.
///
/// `@extensibility(APPENDABLE)`: on the wire a `DHEADER`, then the four
/// `@optional` members `unit`, `min`, `max`, `hash_id`, each a presence
/// octet and, when present, its value. Byte-verified against Cyclone DDS
/// 11.0.1 and RTI Connext 7.7.0 on a struct with `@min`, `@max` and
/// `@unit` (`tests/typeobject_member_annotations.rs`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedBuiltinMemberAnnotations {
    /// `@unit("...")`.
    pub unit: Option<String>,
    /// `@min(val)`, or the lower bound of `@range`.
    pub min: Option<AnnotationParameterValue>,
    /// `@max(val)`, or the upper bound of `@range`.
    pub max: Option<AnnotationParameterValue>,
    /// `@hashid("...")`.
    pub hash_id: Option<String>,
    /// `@default(val)`, as the text of its IDL literal.
    ///
    /// DDS-XTypes 1.3 has no slot for it in this struct (DDSXTY14-44 asks
    /// for one), so it does not go on the wire: Cyclone DDS and Fast DDS
    /// write the four members and nothing after them, and a type's
    /// `COMPLETE` hash is the one they compute. RTI Connext 7.7.0 appends a
    /// fifth member, `@default` as an [`AnnotationParameterValue`];
    /// [`Self::decode_from`] skips it with the rest of the `DHEADER`. The
    /// field carries the value within one process, from the type builders
    /// to `dynamic::to_ast`.
    pub default_value: Option<String>,
}

impl AppliedBuiltinMemberAnnotations {
    // `@optional` members of an appendable struct, numbered by
    // declaration from zero (§7.3.1.3.1): `unit` 0, `min` 1, `max` 2,
    // `hash_id` 3. A presence octet under XCDR2 (§7.4.3.5.3), a parameter
    // header under XCDR1 (§7.4.1.1.5.2).
    fn write_opt_string(
        w: &mut Writer,
        member_id: u32,
        s: &Option<String>,
    ) -> Result<(), WireError> {
        encode_optional(w, member_id, s.as_deref(), |w, v| {
            w.write_string(v);
            Ok(())
        })
    }

    fn read_opt_string(r: &mut Reader<'_>, member_id: u32) -> Result<Option<String>, WireError> {
        decode_optional(r, member_id, |r| r.read_string())
    }

    fn write_opt_value(
        w: &mut Writer,
        member_id: u32,
        value: &Option<AnnotationParameterValue>,
    ) -> Result<(), WireError> {
        encode_optional(w, member_id, value.as_ref(), |w, v| v.encode_into(w))
    }

    fn read_opt_value(
        r: &mut Reader<'_>,
        member_id: u32,
    ) -> Result<Option<AnnotationParameterValue>, WireError> {
        decode_optional(r, member_id, AnnotationParameterValue::decode_from)
    }

    /// `true` if none of the four members that go on the wire is set. The
    /// record is then absent: the `@optional` in front of it is a single
    /// `0`. [`Self::default_value`] does not count, since it is not written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.unit.is_none() && self.min.is_none() && self.max.is_none() && self.hash_id.is_none()
    }

    /// The record as the `@optional` member `member_id` of the structure
    /// that holds it, absent when [`Self::is_empty`].
    ///
    /// # Errors
    /// As [`Self::encode_into`].
    pub fn encode_as_member(&self, w: &mut Writer, member_id: u32) -> Result<(), WireError> {
        let present = (!self.is_empty()).then_some(self);
        encode_optional(w, member_id, present, |w, a| a.encode_into(w))
    }

    /// Read what [`Self::encode_as_member`] writes; absent is the empty
    /// record.
    ///
    /// # Errors
    /// As [`Self::decode_from`].
    pub fn decode_as_member(r: &mut Reader<'_>, member_id: u32) -> Result<Self, WireError> {
        Ok(decode_optional(r, member_id, Self::decode_from)?.unwrap_or_default())
    }

    /// Encode: `DHEADER`, then `unit`, `min`, `max`, `hash_id`.
    ///
    /// # Errors
    /// A `char` value outside the 8-bit range.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            Self::write_opt_string(w, 0, &self.unit)?;
            Self::write_opt_value(w, 1, &self.min)?;
            Self::write_opt_value(w, 2, &self.max)?;
            Self::write_opt_string(w, 3, &self.hash_id)
        })
    }

    /// Decode the four members inside the `DHEADER`. Bytes the header
    /// announces after `hash_id` belong to a later version of the struct
    /// (RTI Connext's `@default` slot) and are skipped.
    ///
    /// # Errors
    /// The stream ends inside this value, or a value's kind octet is
    /// unknown.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let unit = Self::read_opt_string(r, 0)?;
            let min = Self::read_opt_value(r, 1)?;
            let max = Self::read_opt_value(r, 2)?;
            let hash_id = Self::read_opt_string(r, 3)?;
            Ok(Self {
                unit,
                min,
                max,
                hash_id,
                default_value: None,
            })
        })
    }
}

/// CompleteMemberDetail: `name` + `ann_builtin` + `ann_custom`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteMemberDetail {
    /// Voller Member-Name.
    pub name: String,
    /// Builtin-Member-Annotations.
    pub ann_builtin: AppliedBuiltinMemberAnnotations,
    /// Custom-Annotations.
    pub ann_custom: OptionalAppliedAnnotationSeq,
}

impl CompleteMemberDetail {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        write_bounded_string(w, &self.name, MEMBER_NAME_MAX_LENGTH, "MemberName")?;
        // ann_builtin is `@optional` (§7.3.4.5), absent when no builtin
        // member annotation is set (byte-verified vs Cyclone/FastDDS under
        // XCDR2 — the whole record collapses to a single `0`).
        self.ann_builtin.encode_as_member(w, 1)?;
        self.ann_custom.encode_as_member(w, 2)
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let name = read_bounded_string(r, MEMBER_NAME_MAX_LENGTH, "MemberName")?;
        let ann_builtin = AppliedBuiltinMemberAnnotations::decode_as_member(r, 1)?;
        let ann_custom = OptionalAppliedAnnotationSeq::decode_as_member(r, 2)?;
        Ok(Self {
            name,
            ann_builtin,
            ann_custom,
        })
    }
}

/// Hilfsroutine: sequence<T> encode via Callback.
pub(crate) fn encode_seq<T, F>(w: &mut Writer, items: &[T], mut f: F) -> Result<(), WireError>
where
    F: FnMut(&mut Writer, &T) -> Result<(), WireError>,
{
    let len = u32::try_from(items.len()).map_err(|_| WireError::OutOfRange {
        at: w.position(),
        expected: alloc::string::String::from("sequence length exceeds u32::MAX"),
    })?;
    w.write_u32(len);
    for it in items {
        f(w, it)?;
    }
    Ok(())
}

/// DoS cap for Vec pre-allocation during decode. The value is the
/// upper bound in elements that we allocate initially. Large sequences
/// are grown incrementally via `push()`.
///
/// 16 MiB / min_elem_size is a generous heuristic — no one
/// legitimately sends 16M TypeIdentifiers in a getTypes reply, but
/// Vec::with_capacity(u32_wire as usize) would reserve ~16 GB at u32::MAX
/// = an OOM vector.
pub const DECODE_PREALLOC_CAP: usize = 4096;

/// Safe allocation: `Vec::with_capacity(len.min(remaining_bytes /
/// min_elem_size).min(CAP))`. Prevents "30-byte PID forces 4 GB RAM".
#[must_use]
pub(crate) fn safe_capacity(len: usize, min_elem_size: usize, remaining_bytes: usize) -> usize {
    let by_bytes = if min_elem_size == 0 {
        DECODE_PREALLOC_CAP
    } else {
        remaining_bytes.saturating_div(min_elem_size)
    };
    len.min(by_bytes).min(DECODE_PREALLOC_CAP)
}

/// Helper routine: sequence<T> decode via a callback.
///
/// Uses [`safe_capacity`] for DoS protection: even if the
/// wire length is `u32::MAX`, we allocate at most
/// `DECODE_PREALLOC_CAP` entries initially. The actual loop still builds
/// up to `len`, but aborts at the latest when `read_*` reads beyond the
/// available buffer.
pub(crate) fn decode_seq<T, F>(
    r: &mut Reader<'_>,
    mut f: F,
) -> Result<alloc::vec::Vec<T>, WireError>
where
    F: FnMut(&mut Reader<'_>) -> Result<T, WireError>,
{
    let len = r.read_u32()? as usize;
    let cap = safe_capacity(len, 1, r.remaining());
    let mut out = alloc::vec::Vec::with_capacity(cap);
    for _ in 0..len {
        out.push(f(r)?);
    }
    Ok(out)
}

/// Like [`encode_seq`], but with XCDR2 `@appendable` framing: the whole
/// sequence (length + elements) is wrapped in a 4-byte DHEADER. XTypes 1.3
/// §7.3.4.5 — the TypeObject member/literal sequences are `@appendable`, so a
/// peer (and the vendors) prefix the member-seq with its byte length. Each
/// element additionally carries its own DHEADER via its `encode_into`.
pub(crate) fn encode_seq_appendable<T, F>(
    w: &mut Writer,
    items: &[T],
    f: F,
) -> Result<(), WireError>
where
    F: FnMut(&mut Writer, &T) -> Result<(), WireError>,
{
    crate::framing::encode_appendable(w, move |w| encode_seq(w, items, f))
}

/// Decode counterpart to [`encode_seq_appendable`]: strips the member-seq
/// DHEADER, then decodes `length + elements`.
pub(crate) fn decode_seq_appendable<T, F>(
    r: &mut Reader<'_>,
    f: F,
) -> Result<alloc::vec::Vec<T>, WireError>
where
    F: FnMut(&mut Reader<'_>) -> Result<T, WireError>,
{
    crate::framing::decode_appendable(r, move |r| decode_seq(r, f))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod safe_capacity_tests {
    use super::*;

    #[test]
    fn safe_capacity_clamps_by_remaining_bytes() {
        assert_eq!(safe_capacity(1_000_000_000, 4, 100), 25);
    }

    #[test]
    fn safe_capacity_caps_at_prealloc_cap() {
        let cap = safe_capacity(usize::MAX, 1, usize::MAX);
        assert_eq!(cap, DECODE_PREALLOC_CAP);
    }

    #[test]
    fn safe_capacity_returns_len_when_small() {
        assert_eq!(safe_capacity(10, 4, 1000), 10);
    }

    #[test]
    fn safe_capacity_handles_zero_elem_size() {
        assert_eq!(safe_capacity(usize::MAX, 0, 100), DECODE_PREALLOC_CAP);
    }

    #[test]
    fn decode_seq_truncates_preallocation_for_large_lengths() {
        let mut bytes = alloc::vec::Vec::new();
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let res: Result<alloc::vec::Vec<u8>, _> = decode_seq(&mut r, |rr| rr.read_u8());
        assert!(res.is_err());
    }

    fn roundtrip_verbatim(placement: VerbatimPlacement) {
        let a = AppliedVerbatimAnnotation {
            placement: placement.clone(),
            language: alloc::string::String::from("c++"),
            text: alloc::string::String::from("// example"),
        };
        let mut w = crate::framing::xcdr2_writer();
        a.encode_into(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = crate::framing::xcdr2_reader(&bytes);
        let decoded = AppliedVerbatimAnnotation::decode_from(&mut r).unwrap();
        assert_eq!(decoded, a);
    }

    #[test]
    fn verbatim_placement_roundtrip_before() {
        roundtrip_verbatim(VerbatimPlacement::Before);
    }

    #[test]
    fn verbatim_placement_roundtrip_after() {
        roundtrip_verbatim(VerbatimPlacement::After);
    }

    #[test]
    fn verbatim_placement_roundtrip_begin_file() {
        roundtrip_verbatim(VerbatimPlacement::BeginFile);
    }

    #[test]
    fn verbatim_placement_roundtrip_end_file() {
        roundtrip_verbatim(VerbatimPlacement::EndFile);
    }

    #[test]
    fn verbatim_placement_roundtrip_other_forward_compat() {
        roundtrip_verbatim(VerbatimPlacement::Other(alloc::string::String::from(
            "CUSTOM_PLACEMENT",
        )));
    }
}
