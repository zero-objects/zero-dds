// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `TypeObject` of DDS-XTypes 1.1 Annex B ("`TypeObjectV1`") and its type
//! identifier `_TypeId` ("`TypeIdV1`").
//!
//! DDS-XTypes 1.3 Annex D keeps two optional members of XTYPES 1.1 in the
//! built-in topic data of Topic, Publication and Subscription:
//!
//! ```text
//! @id(0x0069) @optional TypeIdV1 type_id;     // XTYPES 1.1
//! @id(0x0072) @optional TypeObjectV1 type;    // XTYPES 1.1
//! ```
//!
//! §7.6.3.2.2: "TypeObjectV1 corresponds to the TypeObject data type
//! specified in Annex B: Representing Types with TypeObject of DDS-XTYPES
//! Version 1.1 [...]. Likewise, the type member shall be set as specified in
//! Clause 7.3.4 of [DDS-XTYPES11]." The text of 1.3 defines no type named
//! `TypeIdV1`; the only type identifier of 1.1 is the union `_TypeId` of its
//! Annex B, which is what [`TypeId`] is.
//!
//! This module is that Annex B: the model ([`TypeObjectV1`] and its parts),
//! the XCDR1 serialization of §7.4.1 of 1.1 ([`TypeObjectV1::to_bytes_le`],
//! [`TypeObjectV1::from_bytes`]), the type identifiers of §7.3.4.1.2
//! ([`TypeLibrary::type_id_of`]) and the conversion from and to the
//! Complete TypeObjects of 1.3 ([`convert`]).
//!
//! # Readings of the text chosen here
//!
//! - **Member ids.** The IDL gives the mutable structures their ids in the
//!   `...MemberId` enumerations (0 and 1 for `Type` and `Member`, 100 and 101
//!   for the first derived level, 200 for the second). `TypeObject` and the
//!   union `TypeLibraryElement` have none: their members are numbered by
//!   declaration order as §7.3.1.3.1 says, from 0 (the first member of a union
//!   is its discriminator), so `library` is 0 and `the_type` 1, and the
//!   members of `TypeLibraryElement` are 1 (`alias_type`) to 11 (`mod`).
//! - **Serialization.** Extensible structures and unions are plain CDR
//!   (§7.4.1.1). Mutable structures are parameter lists (§7.4.1.2), a mutable
//!   union is its discriminator as member 0 and its selected member
//!   (XTypes 1.3 §7.4.3.5.3 rule (28), the one rule that gives an XCDR1
//!   mutable union a form). A structure that derives from a mutable one and
//!   is itself extensible (`MapType`, `SequenceType`, `StringType`) writes
//!   the members of its base before its own, without parameter headers, as
//!   §7.4.1.1.5.1 has it: "exactly as if all of the members had been
//!   defined, in the same order, in the most-derived type".
//! - **`MODULE_ELEMENT`.** The enumerator is declared without a value ("auto
//!   assigned"), which would be 22 and collide with `STRUCTURE_TYPE_ELEMENT`;
//!   it is [`kind::MODULE_ELEMENT`] = 24, the next value no kind uses.
//! - **Type ids.** §7.3.4.1.2: the id is "the less-significant 64 bits" of the
//!   MD5 of the big-endian serialization of the type, which is read as the
//!   last eight octets of the digest as a big-endian number. The
//!   `TypeProperty::type_id` of the type itself is [`TypeId::NONE`] while the
//!   hash is taken, as it cannot be the hash it is the input of. The kind of
//!   a constructed id is the kind of the type (14 to 23).

pub mod codec;
pub mod convert;

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::WireError;

/// `TypeKind` values and the primitive type ids of Annex B.
pub mod kind {
    /// The "null" value.
    pub const NO_TYPE: i16 = 0;
    /// `boolean`.
    pub const BOOLEAN_TYPE: i16 = 1;
    /// `octet`.
    pub const BYTE_TYPE: i16 = 2;
    /// `short`.
    pub const INT_16_TYPE: i16 = 3;
    /// `unsigned short`.
    pub const UINT_16_TYPE: i16 = 4;
    /// `long`.
    pub const INT_32_TYPE: i16 = 5;
    /// `unsigned long`.
    pub const UINT_32_TYPE: i16 = 6;
    /// `long long`.
    pub const INT_64_TYPE: i16 = 7;
    /// `unsigned long long`.
    pub const UINT_64_TYPE: i16 = 8;
    /// `float`.
    pub const FLOAT_32_TYPE: i16 = 9;
    /// `double`.
    pub const FLOAT_64_TYPE: i16 = 10;
    /// `long double`.
    pub const FLOAT_128_TYPE: i16 = 11;
    /// `char`.
    pub const CHAR_8_TYPE: i16 = 12;
    /// `wchar` (UTF-32).
    pub const CHAR_32_TYPE: i16 = 13;
    /// An enumeration.
    pub const ENUMERATION_TYPE: i16 = 14;
    /// A bit set.
    pub const BITSET_TYPE: i16 = 15;
    /// An alias.
    pub const ALIAS_TYPE: i16 = 16;
    /// An array.
    pub const ARRAY_TYPE: i16 = 17;
    /// A sequence.
    pub const SEQUENCE_TYPE: i16 = 18;
    /// A string.
    pub const STRING_TYPE: i16 = 19;
    /// A map.
    pub const MAP_TYPE: i16 = 20;
    /// A union.
    pub const UNION_TYPE: i16 = 21;
    /// A structure.
    pub const STRUCTURE_TYPE: i16 = 22;
    /// An annotation type.
    pub const ANNOTATION_TYPE: i16 = 23;
    /// The `TypeLibraryElementKind` of a module (see the module documentation).
    pub const MODULE_ELEMENT: i16 = 24;
}

/// `TypeFlag` (Annex B): `IS_FINAL` is bit 0, `IS_MUTABLE` bit 1, `IS_NESTED`
/// bit 2. Neither of the first two is an extensible type.
pub mod type_flag {
    /// A final type.
    pub const IS_FINAL: u16 = 1 << 0;
    /// A mutable type.
    pub const IS_MUTABLE: u16 = 1 << 1;
    /// A nested type.
    pub const IS_NESTED: u16 = 1 << 2;
}

/// `MemberFlag` (Annex B).
pub mod member_flag {
    /// A key member.
    pub const IS_KEY: u16 = 1 << 0;
    /// An optional member.
    pub const IS_OPTIONAL: u16 = 1 << 1;
    /// A shareable member.
    pub const IS_SHAREABLE: u16 = 1 << 2;
    /// The default case of a union.
    pub const IS_UNION_DEFAULT: u16 = 1 << 3;
}

/// `MEMBER_ID_INVALID` of Annex B.
pub const MEMBER_ID_INVALID: u32 = 0x0FFF_FFFF;

/// Longest `ObjectName` (`ELEMENT_NAME_MAX_LENGTH`).
pub const ELEMENT_NAME_MAX_LENGTH: usize = 256;

/// What the conversion or the serialization of a `TypeObjectV1` refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum V1Error {
    /// The byte runtime refused a read or a write.
    Wire(WireError),
    /// The object is not what Annex B admits: a discriminator it does not
    /// define, a name past its bound.
    Invalid(String),
    /// A type or a feature of it has no form in Annex B of XTypes 1.1.
    Unrepresentable(String),
    /// A type the conversion needs is not known to the registry.
    Unresolved(String),
}

impl core::fmt::Display for V1Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Wire(e) => write!(f, "TypeObjectV1 wire error: {e}"),
            Self::Invalid(s) => write!(f, "TypeObjectV1 invalid: {s}"),
            Self::Unrepresentable(s) => write!(f, "no TypeObjectV1 form: {s}"),
            Self::Unresolved(s) => write!(f, "type not known: {s}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for V1Error {}

impl From<WireError> for V1Error {
    fn from(e: WireError) -> Self {
        Self::Wire(e)
    }
}

/// `_TypeId`: the identifier of a type inside a library.
///
/// The union switches on a `TypeKind`: the thirteen primitive kinds select
/// the `PrimitiveTypeId`, which has the same value as the kind; every other
/// value, `NO_TYPE` included, selects the 64-bit `constructed_type_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TypeId {
    /// A primitive type, by its `TypeKind` (`BOOLEAN_TYPE` to
    /// `CHAR_32_TYPE`).
    Primitive(i16),
    /// A constructed type: its kind and the hash of its serialization.
    Constructed {
        /// `TypeKind` of the type (the discriminator).
        kind: i16,
        /// `constructed_type_id`.
        id: u64,
    },
}

impl TypeId {
    /// `NO_TYPE`: the absence of a type (a structure without base type).
    pub const NONE: Self = Self::Constructed {
        kind: kind::NO_TYPE,
        id: 0,
    };

    /// Whether this is `NO_TYPE`.
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Self::NONE
    }

    /// The discriminator.
    #[must_use]
    pub fn kind(&self) -> i16 {
        match self {
            Self::Primitive(k) => *k,
            Self::Constructed { kind, .. } => *kind,
        }
    }
}

/// `TypeProperty`: the fundamental properties of a type definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeProperty {
    /// `TypeFlag` bits ([`type_flag`]).
    pub flag: u16,
    /// The type's own id.
    pub type_id: TypeId,
    /// The simple name of the type (its module is the enclosing
    /// [`Module`]).
    pub name: String,
}

/// `Type`: the base of every type definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeBase {
    /// `property`, member id 0.
    pub property: TypeProperty,
    /// `annotation`, member id 1.
    pub annotation: Vec<AnnotationUsage>,
}

/// `AnnotationMemberValue`: a literal value of an annotation member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationMemberValue {
    /// `BOOLEAN_TYPE`.
    Boolean(bool),
    /// `BYTE_TYPE`.
    Byte(u8),
    /// `INT_16_TYPE`.
    Int16(i16),
    /// `UINT_16_TYPE`.
    UInt16(u16),
    /// `INT_32_TYPE`.
    Int32(i32),
    /// `UINT_32_TYPE`.
    UInt32(u32),
    /// `INT_64_TYPE`.
    Int64(i64),
    /// `UINT_64_TYPE`.
    UInt64(u64),
    /// `FLOAT_32_TYPE`, as its IEEE-754 bits.
    Float32Bits(u32),
    /// `FLOAT_64_TYPE`, as its IEEE-754 bits.
    Float64Bits(u64),
    /// `FLOAT_128_TYPE`, as its IEEE-754 binary128 octets (big-endian).
    Float128([u8; 16]),
    /// `CHAR_8_TYPE`.
    Char8(u8),
    /// `CHAR_32_TYPE`.
    Char32(u32),
    /// `ENUMERATION_TYPE`.
    Enumeration(i32),
    /// `STRING_TYPE`: a `wstring` "regardless of char width".
    String(String),
}

/// `AnnotationUsageMember`: the value one member of an applied annotation
/// takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationUsageMember {
    /// The member of the annotation type.
    pub member_id: u32,
    /// Its value.
    pub value: AnnotationMemberValue,
}

/// `AnnotationUsage`: the application of an annotation to a type or a member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationUsage {
    /// The annotation type.
    pub type_id: TypeId,
    /// The values given.
    pub member: Vec<AnnotationUsageMember>,
}

/// `MemberProperty`: the fundamental properties of an aggregation member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberProperty {
    /// `MemberFlag` bits ([`member_flag`]).
    pub flag: u16,
    /// The member id.
    pub member_id: u32,
    /// The type of the member.
    pub type_id: TypeId,
    /// The name of the member.
    pub name: String,
}

/// `Member`: a member of an aggregation type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// `property`, member id 0.
    pub property: MemberProperty,
    /// `annotation`, member id 1.
    pub annotation: Vec<AnnotationUsage>,
}

/// `StructureType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureType {
    /// The base `Type`.
    pub base: TypeBase,
    /// `base_type`, member id 100: [`TypeId::NONE`] without one.
    pub base_type: TypeId,
    /// `member`, member id 101.
    pub member: Vec<Member>,
}

/// `UnionMember`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionMember {
    /// The base `Member`.
    pub member: Member,
    /// `label`, member id 100.
    pub label: Vec<i32>,
}

/// `UnionType`: the discriminator is the first of the members (Annex B has
/// no member for it, and §7.2.2 of 1.1 says the discriminator "is always
/// considered to be the first member of a union").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionType {
    /// The base `Type`.
    pub base: TypeBase,
    /// `member`, member id 100.
    pub member: Vec<UnionMember>,
}

/// `AnnotationMember`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationMember {
    /// The base `Member`.
    pub member: Member,
    /// `default_value`, member id 100; `None` where the parameter list
    /// leaves it out.
    pub default_value: Option<AnnotationMemberValue>,
}

/// `AnnotationType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationType {
    /// The base `Type`.
    pub base: TypeBase,
    /// `base_type`, member id 100.
    pub base_type: TypeId,
    /// `member`, member id 101.
    pub member: Vec<AnnotationMember>,
}

/// `AliasType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasType {
    /// The base `Type`.
    pub base: TypeBase,
    /// `base_type`, member id 100.
    pub base_type: TypeId,
}

/// The members `CollectionType` adds to `Type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionElement {
    /// `element_type`, member id 100.
    pub element_type: TypeId,
    /// `element_shared`, member id 101.
    pub element_shared: bool,
}

/// `ArrayType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayType {
    /// The base `Type`.
    pub base: TypeBase,
    /// The element.
    pub element: CollectionElement,
    /// `bound`, member id 200: one per dimension.
    pub bound: Vec<u32>,
}

/// `MapType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapType {
    /// The base `Type`.
    pub base: TypeBase,
    /// The value element.
    pub element: CollectionElement,
    /// `key_element_type`.
    pub key_element_type: TypeId,
    /// `bound`; 0 is unbounded.
    pub bound: u32,
}

/// `SequenceType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceType {
    /// The base `Type`.
    pub base: TypeBase,
    /// The element.
    pub element: CollectionElement,
    /// `bound`; 0 is unbounded.
    pub bound: u32,
}

/// `StringType`: the element type says narrow or wide characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringType {
    /// The base `Type`.
    pub base: TypeBase,
    /// The element (`CHAR_8_TYPE` or `CHAR_32_TYPE`).
    pub element: CollectionElement,
    /// `bound`; 0 is unbounded.
    pub bound: u32,
}

/// `Bit`: a bit of a bit set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bit {
    /// The index of the bit.
    pub index: i32,
    /// Its name.
    pub name: String,
}

/// `BitSetType`: the bit sets of XTypes 1.1 are the flag sets 1.3 calls
/// bitmasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitSetType {
    /// The base `Type`.
    pub base: TypeBase,
    /// `bit_bound`, member id 100.
    pub bit_bound: u32,
    /// `bit`, member id 101.
    pub bit: Vec<Bit>,
}

/// `EnumeratedConstant`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumeratedConstant {
    /// Its value.
    pub value: i32,
    /// Its name.
    pub name: String,
}

/// `EnumerationType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumerationType {
    /// The base `Type`.
    pub base: TypeBase,
    /// `bit_bound`, member id 100.
    pub bit_bound: u32,
    /// `constant`, member id 101.
    pub constant: Vec<EnumeratedConstant>,
}

/// `_Module`: a name and the types inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// The name of the module.
    pub name: String,
    /// What it contains.
    pub library: TypeLibrary,
}

/// `TypeLibraryElement`: a type or a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryElement {
    /// `alias_type`.
    Alias(AliasType),
    /// `annotation_type`.
    Annotation(AnnotationType),
    /// `array_type`.
    Array(ArrayType),
    /// `bitset_type`.
    BitSet(BitSetType),
    /// `enumeration_type`.
    Enumeration(EnumerationType),
    /// `map_type`.
    Map(MapType),
    /// `sequence_type`.
    Sequence(SequenceType),
    /// `string_type`.
    String(StringType),
    /// `structure_type`.
    Structure(StructureType),
    /// `union_type`.
    Union(UnionType),
    /// `mod`.
    Module(Module),
}

impl LibraryElement {
    /// The `TypeBase` of a type; `None` for a module.
    #[must_use]
    pub fn base(&self) -> Option<&TypeBase> {
        Some(match self {
            Self::Alias(t) => &t.base,
            Self::Annotation(t) => &t.base,
            Self::Array(t) => &t.base,
            Self::BitSet(t) => &t.base,
            Self::Enumeration(t) => &t.base,
            Self::Map(t) => &t.base,
            Self::Sequence(t) => &t.base,
            Self::String(t) => &t.base,
            Self::Structure(t) => &t.base,
            Self::Union(t) => &t.base,
            Self::Module(_) => return None,
        })
    }

    /// The mutable `TypeBase` of a type; `None` for a module.
    pub fn base_mut(&mut self) -> Option<&mut TypeBase> {
        Some(match self {
            Self::Alias(t) => &mut t.base,
            Self::Annotation(t) => &mut t.base,
            Self::Array(t) => &mut t.base,
            Self::BitSet(t) => &mut t.base,
            Self::Enumeration(t) => &mut t.base,
            Self::Map(t) => &mut t.base,
            Self::Sequence(t) => &mut t.base,
            Self::String(t) => &mut t.base,
            Self::Structure(t) => &mut t.base,
            Self::Union(t) => &mut t.base,
            Self::Module(_) => return None,
        })
    }

    /// The `TypeKind` of the type; `None` for a module.
    #[must_use]
    pub fn type_kind(&self) -> Option<i16> {
        Some(match self {
            Self::Alias(_) => kind::ALIAS_TYPE,
            Self::Annotation(_) => kind::ANNOTATION_TYPE,
            Self::Array(_) => kind::ARRAY_TYPE,
            Self::BitSet(_) => kind::BITSET_TYPE,
            Self::Enumeration(_) => kind::ENUMERATION_TYPE,
            Self::Map(_) => kind::MAP_TYPE,
            Self::Sequence(_) => kind::SEQUENCE_TYPE,
            Self::String(_) => kind::STRING_TYPE,
            Self::Structure(_) => kind::STRUCTURE_TYPE,
            Self::Union(_) => kind::UNION_TYPE,
            Self::Module(_) => return None,
        })
    }
}

/// `TypeLibrary`: the definitions of a set of types and modules.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeLibrary {
    /// `element`.
    pub element: Vec<LibraryElement>,
}

impl TypeLibrary {
    /// The element with the id `id`, looked up in this library and the
    /// modules inside it (§7.3.4.1.1: an id is unique "within the
    /// TypeLibrary contained by a TypeObject object and in any other Type
    /// Libraries contains recursively therein"), with the names of the
    /// modules it is in.
    ///
    /// zerodds-lint: recursion-depth 64 (module nesting; a decoded object
    /// is as deep as the nesting of parameter lists the reader allows)
    #[must_use]
    pub fn find(&self, id: TypeId) -> Option<(Vec<&str>, &LibraryElement)> {
        let mut path: Vec<&str> = Vec::new();
        self.find_in(id, &mut path, 0)
    }

    fn find_in<'a>(
        &'a self,
        id: TypeId,
        path: &mut Vec<&'a str>,
        depth: usize,
    ) -> Option<(Vec<&'a str>, &'a LibraryElement)> {
        if depth > codec::MAX_MODULE_DEPTH {
            return None;
        }
        for element in &self.element {
            match element {
                LibraryElement::Module(m) => {
                    path.push(m.name.as_str());
                    let found = m.library.find_in(id, path, depth + 1);
                    path.pop();
                    if found.is_some() {
                        return found;
                    }
                }
                other => {
                    if other.base().map(|b| b.property.type_id) == Some(id) {
                        return Some((path.clone(), other));
                    }
                }
            }
        }
        None
    }

    /// The serialization of an element's selected member in the given byte
    /// order: what the hash of its type id is taken over.
    ///
    /// # Errors
    /// [`V1Error::Invalid`] for a name past [`ELEMENT_NAME_MAX_LENGTH`].
    pub fn element_bytes(element: &LibraryElement, big_endian: bool) -> Result<Vec<u8>, V1Error> {
        codec::element_bytes(element, big_endian)
    }

    /// The id of an element: §7.3.4.1.2 steps 1 to 3 over the element as it
    /// is, with the id of the type itself taken as [`TypeId::NONE`].
    ///
    /// `None` for a module, which has no id.
    ///
    /// # Errors
    /// [`V1Error::Wire`] when the element does not serialize.
    pub fn type_id_of(element: &LibraryElement) -> Result<Option<TypeId>, V1Error> {
        let Some(type_kind) = element.type_kind() else {
            return Ok(None);
        };
        let mut copy = element.clone();
        if let Some(base) = copy.base_mut() {
            base.property.type_id = TypeId::NONE;
        }
        let bytes = codec::element_bytes(&copy, true)?;
        let digest = zerodds_wire::md5(&bytes);
        let mut low = [0u8; 8];
        low.copy_from_slice(&digest[8..16]);
        Ok(Some(TypeId::Constructed {
            kind: type_kind,
            id: u64::from_be_bytes(low),
        }))
    }
}

/// `TypeObject`: one library and the ids of the types that describe the
/// endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeObjectV1 {
    /// `library`, member id 0.
    pub library: TypeLibrary,
    /// `the_type`, member id 1.
    pub the_type: Vec<TypeId>,
}

impl TypeObjectV1 {
    /// The XCDR1 serialization, little-endian (the form of the built-in
    /// topic data this project writes).
    ///
    /// # Errors
    /// [`V1Error::Invalid`] for a name past [`ELEMENT_NAME_MAX_LENGTH`];
    /// [`V1Error::Wire`] for a length that does not fit.
    pub fn to_bytes_le(&self) -> Result<Vec<u8>, V1Error> {
        codec::encode_object(self, false)
    }

    /// The XCDR1 serialization, big-endian.
    ///
    /// # Errors
    /// As [`Self::to_bytes_le`].
    pub fn to_bytes_be(&self) -> Result<Vec<u8>, V1Error> {
        codec::encode_object(self, true)
    }

    /// Reads a serialization of the given byte order.
    ///
    /// # Errors
    /// [`V1Error::Wire`] for a truncated or ill-formed stream;
    /// [`V1Error::Invalid`] for a discriminator Annex B does not define.
    pub fn from_bytes(bytes: &[u8], big_endian: bool) -> Result<Self, V1Error> {
        codec::decode_object(bytes, big_endian)
    }
}

impl TypeId {
    /// The XCDR1 serialization of the `_TypeId` as the value of the
    /// parameter `PID_TYPE_ID` (0x0069), little-endian.
    ///
    /// # Errors
    /// [`V1Error::Wire`] when the writer refuses.
    pub fn to_bytes_le(&self) -> Result<Vec<u8>, V1Error> {
        codec::encode_type_id(*self, false)
    }

    /// As [`Self::to_bytes_le`], big-endian.
    ///
    /// # Errors
    /// As [`Self::to_bytes_le`].
    pub fn to_bytes_be(&self) -> Result<Vec<u8>, V1Error> {
        codec::encode_type_id(*self, true)
    }

    /// Reads a serialized `_TypeId`.
    ///
    /// # Errors
    /// [`V1Error::Wire`] for a truncated stream.
    pub fn from_bytes(bytes: &[u8], big_endian: bool) -> Result<Self, V1Error> {
        codec::decode_type_id(bytes, big_endian)
    }
}
