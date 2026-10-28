// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The XCDR1 serialization of the `TypeObject` of XTypes 1.1 Annex B.
//!
//! Plain CDR for the extensible structures and unions, parameter lists for the
//! mutable ones (XTypes 1.1 §7.4.1, XTypes 1.3 §7.4.3.5.3 rules (23) to
//! (28)); the module documentation of [`super`] names the readings chosen
//! where the text leaves a choice. The parameter headers, the alignment
//! origin of a parameter value and the sentinel are those of `zerodds-wire`,
//! the same that carry the other XCDR1 parameter lists of this project.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_float128::Float128;
use zerodds_wire::writer::Pid;
use zerodds_wire::{Endianness, ParameterRole, Reader, WireError, Writer};

use super::{
    AliasType, AnnotationMember, AnnotationMemberValue, AnnotationType, AnnotationUsage,
    AnnotationUsageMember, ArrayType, Bit, BitSetType, CollectionElement, ELEMENT_NAME_MAX_LENGTH,
    EnumeratedConstant, EnumerationType, LibraryElement, MapType, Member, MemberProperty, Module,
    SequenceType, StringType, StructureType, TypeBase, TypeId, TypeLibrary, TypeObjectV1,
    TypeProperty, UnionMember, UnionType, V1Error, kind,
};

/// XCDR1: 64-bit primitives align to eight.
const MAX_ALIGN: usize = 8;

/// How deep modules may nest in a decoded object. A module holds a library
/// that holds modules; every level is a parameter list inside a parameter
/// list, and the stream ends before this is much deeper than a real type
/// hierarchy ever is.
pub const MAX_MODULE_DEPTH: usize = 32;

type R<T> = Result<T, V1Error>;

fn endianness(big: bool) -> Endianness {
    if big {
        Endianness::Big
    } else {
        Endianness::Little
    }
}

fn writer(big: bool) -> Writer {
    Writer::with_max_align(endianness(big), MAX_ALIGN)
}

fn reader(bytes: &[u8], big: bool) -> Reader<'_> {
    Reader::with_max_align(bytes, endianness(big), MAX_ALIGN)
}

// -- member ids ----------------------------------------------------------

const ID_PROPERTY: u16 = 0;
const ID_ANNOTATION: u16 = 1;
const ID_BASE_TYPE: u16 = 100;
const ID_MEMBER_STRUCT: u16 = 101;
const ID_LABEL: u16 = 100;
const ID_UNION_MEMBER: u16 = 100;
const ID_DEFAULT_VALUE: u16 = 100;
const ID_ELEMENT_TYPE: u16 = 100;
const ID_ELEMENT_SHARED: u16 = 101;
const ID_BOUND_ARRAY: u16 = 200;
const ID_BIT_BOUND: u16 = 100;
const ID_BIT_OR_CONSTANT: u16 = 101;
const ID_LIBRARY: u16 = 0;
const ID_THE_TYPE: u16 = 1;

/// The member id of the selected member of `TypeLibraryElement`: the
/// position of the member in the union, from one (the discriminator is 0).
fn element_member_id(element: &LibraryElement) -> u16 {
    match element {
        LibraryElement::Alias(_) => 1,
        LibraryElement::Annotation(_) => 2,
        LibraryElement::Array(_) => 3,
        LibraryElement::BitSet(_) => 4,
        LibraryElement::Enumeration(_) => 5,
        LibraryElement::Map(_) => 6,
        LibraryElement::Sequence(_) => 7,
        LibraryElement::String(_) => 8,
        LibraryElement::Structure(_) => 9,
        LibraryElement::Union(_) => 10,
        LibraryElement::Module(_) => 11,
    }
}

/// `TypeLibraryElementKind`: the `TypeKind` of the type, or the module value.
fn element_kind(element: &LibraryElement) -> i16 {
    element.type_kind().unwrap_or(kind::MODULE_ELEMENT)
}

// -- writing -------------------------------------------------------------

fn param(
    w: &mut Writer,
    id: u16,
    must_understand: bool,
    body: impl FnOnce(&mut Writer) -> R<()>,
) -> R<()> {
    let header: Pid = w.write_pid(id, ParameterRole::Member, must_understand)?;
    body(w)?;
    w.finish_pid(header);
    Ok(())
}

fn put_seq<T>(w: &mut Writer, items: &[T], each: impl Fn(&mut Writer, &T) -> R<()>) -> R<()> {
    let count = u32::try_from(items.len()).map_err(|_| {
        V1Error::Wire(WireError::OutOfRange {
            at: w.position(),
            expected: String::from("a sequence length a 32-bit count holds"),
        })
    })?;
    w.write_u32(count);
    for item in items {
        each(w, item)?;
    }
    Ok(())
}

fn put_name(w: &mut Writer, name: &str) -> R<()> {
    if name.len() > ELEMENT_NAME_MAX_LENGTH {
        return Err(V1Error::Invalid(alloc::format!(
            "the name `{name}` is longer than ObjectName's {ELEMENT_NAME_MAX_LENGTH}"
        )));
    }
    w.write_string(name);
    Ok(())
}

fn put_type_id(w: &mut Writer, id: TypeId) {
    w.write_i16(id.kind());
    match id {
        TypeId::Primitive(k) => w.write_i16(k),
        TypeId::Constructed { id, .. } => w.write_u64(id),
    }
}

fn put_wstring(w: &mut Writer, text: &str) {
    // UTF-32, the length counting the terminator (XTypes 1.1 §7.4.1.1.1).
    let chars: Vec<char> = text.chars().collect();
    #[allow(clippy::cast_possible_truncation)]
    w.write_u32(chars.len() as u32 + 1);
    for c in chars {
        w.write_u32(u32::from(c));
    }
    w.write_u32(0);
}

fn put_value(w: &mut Writer, value: &AnnotationMemberValue) {
    use AnnotationMemberValue as V;
    match value {
        V::Boolean(v) => {
            w.write_i16(kind::BOOLEAN_TYPE);
            w.write_bool(*v);
        }
        V::Byte(v) => {
            w.write_i16(kind::BYTE_TYPE);
            w.write_u8(*v);
        }
        V::Int16(v) => {
            w.write_i16(kind::INT_16_TYPE);
            w.write_i16(*v);
        }
        V::UInt16(v) => {
            w.write_i16(kind::UINT_16_TYPE);
            w.write_u16(*v);
        }
        V::Int32(v) => {
            w.write_i16(kind::INT_32_TYPE);
            w.write_i32(*v);
        }
        V::UInt32(v) => {
            w.write_i16(kind::UINT_32_TYPE);
            w.write_u32(*v);
        }
        V::Int64(v) => {
            w.write_i16(kind::INT_64_TYPE);
            w.write_i64(*v);
        }
        V::UInt64(v) => {
            w.write_i16(kind::UINT_64_TYPE);
            w.write_u64(*v);
        }
        V::Float32Bits(v) => {
            w.write_i16(kind::FLOAT_32_TYPE);
            w.write_u32(*v);
        }
        V::Float64Bits(v) => {
            w.write_i16(kind::FLOAT_64_TYPE);
            w.write_u64(*v);
        }
        V::Float128(v) => {
            w.write_i16(kind::FLOAT_128_TYPE);
            w.write_f128(Float128::from_be_bytes(*v));
        }
        V::Char8(v) => {
            w.write_i16(kind::CHAR_8_TYPE);
            w.write_u8(*v);
        }
        V::Char32(v) => {
            w.write_i16(kind::CHAR_32_TYPE);
            w.write_u32(*v);
        }
        V::Enumeration(v) => {
            w.write_i16(kind::ENUMERATION_TYPE);
            w.write_i32(*v);
        }
        V::String(v) => {
            w.write_i16(kind::STRING_TYPE);
            put_wstring(w, v);
        }
    }
}

fn put_usage(w: &mut Writer, usage: &AnnotationUsage) -> R<()> {
    put_type_id(w, usage.type_id);
    put_seq(w, &usage.member, |w, m: &AnnotationUsageMember| {
        w.write_u32(m.member_id);
        put_value(w, &m.value);
        Ok(())
    })
}

fn put_usages(w: &mut Writer, usages: &[AnnotationUsage]) -> R<()> {
    put_seq(w, usages, put_usage)
}

fn put_type_property(w: &mut Writer, p: &TypeProperty) -> R<()> {
    w.write_u16(p.flag);
    put_type_id(w, p.type_id);
    put_name(w, &p.name)
}

fn put_member_property(w: &mut Writer, p: &MemberProperty) -> R<()> {
    w.write_u16(p.flag);
    w.write_u32(p.member_id);
    put_type_id(w, p.type_id);
    put_name(w, &p.name)
}

/// The two members of `Type`, as parameters of the list of a mutable type.
fn put_base_params(w: &mut Writer, base: &TypeBase) -> R<()> {
    param(w, ID_PROPERTY, false, |w| {
        put_type_property(w, &base.property)
    })?;
    param(w, ID_ANNOTATION, false, |w| put_usages(w, &base.annotation))
}

/// A mutable `Member` (the parameters of the list, without its sentinel).
fn put_member_params(w: &mut Writer, m: &Member) -> R<()> {
    param(w, ID_PROPERTY, false, |w| {
        put_member_property(w, &m.property)
    })?;
    param(w, ID_ANNOTATION, false, |w| put_usages(w, &m.annotation))
}

fn put_member(w: &mut Writer, m: &Member) -> R<()> {
    put_member_params(w, m)?;
    w.write_sentinel();
    Ok(())
}

fn put_union_member(w: &mut Writer, m: &UnionMember) -> R<()> {
    put_member_params(w, &m.member)?;
    param(w, ID_LABEL, false, |w| {
        put_seq(w, &m.label, |w, l: &i32| {
            w.write_i32(*l);
            Ok(())
        })
    })?;
    w.write_sentinel();
    Ok(())
}

fn put_annotation_member(w: &mut Writer, m: &AnnotationMember) -> R<()> {
    put_member_params(w, &m.member)?;
    if let Some(value) = &m.default_value {
        param(w, ID_DEFAULT_VALUE, false, |w| {
            put_value(w, value);
            Ok(())
        })?;
    }
    w.write_sentinel();
    Ok(())
}

fn put_collection_flat(w: &mut Writer, base: &TypeBase, element: &CollectionElement) -> R<()> {
    // Extensible below a mutable base: the members of the base first, plain
    // (XTypes 1.1 §7.4.1.1.5.1).
    put_type_property(w, &base.property)?;
    put_usages(w, &base.annotation)?;
    put_type_id(w, element.element_type);
    w.write_bool(element.element_shared);
    Ok(())
}

fn put_structure(w: &mut Writer, t: &StructureType) -> R<()> {
    put_base_params(w, &t.base)?;
    param(w, ID_BASE_TYPE, false, |w| {
        put_type_id(w, t.base_type);
        Ok(())
    })?;
    param(w, ID_MEMBER_STRUCT, false, |w| {
        put_seq(w, &t.member, put_member)
    })?;
    w.write_sentinel();
    Ok(())
}

fn put_union(w: &mut Writer, t: &UnionType) -> R<()> {
    put_base_params(w, &t.base)?;
    param(w, ID_UNION_MEMBER, false, |w| {
        put_seq(w, &t.member, put_union_member)
    })?;
    w.write_sentinel();
    Ok(())
}

fn put_annotation_type(w: &mut Writer, t: &AnnotationType) -> R<()> {
    put_base_params(w, &t.base)?;
    param(w, ID_BASE_TYPE, false, |w| {
        put_type_id(w, t.base_type);
        Ok(())
    })?;
    param(w, ID_MEMBER_STRUCT, false, |w| {
        put_seq(w, &t.member, put_annotation_member)
    })?;
    w.write_sentinel();
    Ok(())
}

fn put_alias(w: &mut Writer, t: &AliasType) -> R<()> {
    put_base_params(w, &t.base)?;
    param(w, ID_BASE_TYPE, false, |w| {
        put_type_id(w, t.base_type);
        Ok(())
    })?;
    w.write_sentinel();
    Ok(())
}

fn put_array(w: &mut Writer, t: &ArrayType) -> R<()> {
    put_base_params(w, &t.base)?;
    param(w, ID_ELEMENT_TYPE, false, |w| {
        put_type_id(w, t.element.element_type);
        Ok(())
    })?;
    param(w, ID_ELEMENT_SHARED, false, |w| {
        w.write_bool(t.element.element_shared);
        Ok(())
    })?;
    param(w, ID_BOUND_ARRAY, false, |w| {
        put_seq(w, &t.bound, |w, b: &u32| {
            w.write_u32(*b);
            Ok(())
        })
    })?;
    w.write_sentinel();
    Ok(())
}

fn put_bit_set(w: &mut Writer, t: &BitSetType) -> R<()> {
    put_base_params(w, &t.base)?;
    param(w, ID_BIT_BOUND, false, |w| {
        w.write_u32(t.bit_bound);
        Ok(())
    })?;
    param(w, ID_BIT_OR_CONSTANT, false, |w| {
        put_seq(w, &t.bit, |w, b: &Bit| {
            w.write_i32(b.index);
            put_name(w, &b.name)
        })
    })?;
    w.write_sentinel();
    Ok(())
}

fn put_enumeration(w: &mut Writer, t: &EnumerationType) -> R<()> {
    put_base_params(w, &t.base)?;
    param(w, ID_BIT_BOUND, false, |w| {
        w.write_u32(t.bit_bound);
        Ok(())
    })?;
    param(w, ID_BIT_OR_CONSTANT, false, |w| {
        put_seq(w, &t.constant, |w, c: &EnumeratedConstant| {
            w.write_i32(c.value);
            put_name(w, &c.name)
        })
    })?;
    w.write_sentinel();
    Ok(())
}

fn put_library(w: &mut Writer, library: &TypeLibrary) -> R<()> {
    put_seq(w, &library.element, put_element)
}

/// The body of an element as it is serialized inside the union: what the
/// selected member's value is.
pub(super) fn put_element_body(w: &mut Writer, element: &LibraryElement) -> R<()> {
    match element {
        LibraryElement::Alias(t) => put_alias(w, t),
        LibraryElement::Annotation(t) => put_annotation_type(w, t),
        LibraryElement::Array(t) => put_array(w, t),
        LibraryElement::BitSet(t) => put_bit_set(w, t),
        LibraryElement::Enumeration(t) => put_enumeration(w, t),
        LibraryElement::Map(t) => {
            put_collection_flat(w, &t.base, &t.element)?;
            put_type_id(w, t.key_element_type);
            w.write_u32(t.bound);
            Ok(())
        }
        LibraryElement::Sequence(t) => {
            put_collection_flat(w, &t.base, &t.element)?;
            w.write_u32(t.bound);
            Ok(())
        }
        LibraryElement::String(t) => {
            put_collection_flat(w, &t.base, &t.element)?;
            w.write_u32(t.bound);
            Ok(())
        }
        LibraryElement::Structure(t) => put_structure(w, t),
        LibraryElement::Union(t) => put_union(w, t),
        LibraryElement::Module(m) => {
            put_name(w, &m.name)?;
            put_library(w, &m.library)
        }
    }
}

/// A `TypeLibraryElement`: a mutable union, serialized as its discriminator
/// (member 0, which must be understood) and its selected member.
fn put_element(w: &mut Writer, element: &LibraryElement) -> R<()> {
    param(w, 0, true, |w| {
        w.write_i16(element_kind(element));
        Ok(())
    })?;
    param(w, element_member_id(element), false, |w| {
        put_element_body(w, element)
    })?;
    w.write_sentinel();
    Ok(())
}

/// The serialization of one element's selected member, for the hash of its
/// type id (§7.3.4.1.2 step 1: "Serialize the type (as an ArrayType,
/// StructureType, etc. as appropriate)").
pub(super) fn element_bytes(element: &LibraryElement, big: bool) -> R<Vec<u8>> {
    let mut w = writer(big);
    put_element_body(&mut w, element)?;
    Ok(w.finish())
}

pub(super) fn encode_object(object: &TypeObjectV1, big: bool) -> R<Vec<u8>> {
    let mut w = writer(big);
    param(&mut w, ID_LIBRARY, false, |w| {
        put_library(w, &object.library)
    })?;
    param(&mut w, ID_THE_TYPE, false, |w| {
        put_seq(w, &object.the_type, |w, id: &TypeId| {
            put_type_id(w, *id);
            Ok(())
        })
    })?;
    w.write_sentinel();
    Ok(w.finish())
}

pub(super) fn encode_type_id(id: TypeId, big: bool) -> R<Vec<u8>> {
    let mut w = writer(big);
    put_type_id(&mut w, id);
    Ok(w.finish())
}

// -- reading -------------------------------------------------------------

/// Reads a parameter list: `on` takes the member id and the reader at the
/// value and says whether it knew the member.
fn read_list(r: &mut Reader<'_>, mut on: impl FnMut(u32, &mut Reader<'_>) -> R<bool>) -> R<()> {
    loop {
        let header = r.read_pid()?;
        if header.end {
            return Ok(());
        }
        if header.ignore || header.impl_extension || header.reserved {
            r.skip_parameter(header)?;
            continue;
        }
        if on(header.pid, r)? {
            r.finish_pid(header)?;
        } else if header.must_understand {
            return Err(V1Error::Invalid(alloc::format!(
                "the member {} is marked must-understand and this reader does not know it",
                header.pid
            )));
        } else {
            r.skip_parameter(header)?;
        }
    }
}

fn get_seq<T>(r: &mut Reader<'_>, each: impl Fn(&mut Reader<'_>) -> R<T>) -> R<Vec<T>> {
    let count = r.read_u32()? as usize;
    if count > r.remaining() {
        return Err(V1Error::Wire(WireError::ImpossibleLength {
            at: 0,
            claimed: count,
            available: r.remaining(),
        }));
    }
    let mut out = Vec::new();
    for _ in 0..count {
        out.push(each(r)?);
    }
    Ok(out)
}

fn get_name(r: &mut Reader<'_>) -> R<String> {
    let name = r.read_string()?;
    if name.len() > ELEMENT_NAME_MAX_LENGTH {
        return Err(V1Error::Invalid(alloc::format!(
            "a name of {} octets, ObjectName holds {ELEMENT_NAME_MAX_LENGTH}",
            name.len()
        )));
    }
    Ok(name)
}

fn get_type_id(r: &mut Reader<'_>) -> R<TypeId> {
    let discriminator = r.read_i16()?;
    if (kind::BOOLEAN_TYPE..=kind::CHAR_32_TYPE).contains(&discriminator) {
        let primitive = r.read_i16()?;
        if primitive != discriminator {
            return Err(V1Error::Invalid(alloc::format!(
                "primitive type id {primitive} under the kind {discriminator}"
            )));
        }
        Ok(TypeId::Primitive(primitive))
    } else {
        Ok(TypeId::Constructed {
            kind: discriminator,
            id: r.read_u64()?,
        })
    }
}

fn get_wstring(r: &mut Reader<'_>) -> R<String> {
    let length = r.read_u32()? as usize;
    if length == 0 || length > r.remaining() / 4 + 1 {
        return Err(V1Error::Wire(WireError::ImpossibleLength {
            at: 0,
            claimed: length,
            available: r.remaining(),
        }));
    }
    let mut out = String::new();
    for _ in 0..length - 1 {
        let c = r.read_u32()?;
        out.push(char::from_u32(c).ok_or_else(|| {
            V1Error::Invalid(alloc::format!("{c:#x} is no Unicode scalar value"))
        })?);
    }
    if r.read_u32()? != 0 {
        return Err(V1Error::Invalid(String::from(
            "a wide string whose last character is not a NUL",
        )));
    }
    Ok(out)
}

fn get_value(r: &mut Reader<'_>) -> R<AnnotationMemberValue> {
    use AnnotationMemberValue as V;
    let discriminator = r.read_i16()?;
    Ok(match discriminator {
        kind::BOOLEAN_TYPE => V::Boolean(r.read_bool()?),
        kind::BYTE_TYPE => V::Byte(r.read_u8()?),
        kind::INT_16_TYPE => V::Int16(r.read_i16()?),
        kind::UINT_16_TYPE => V::UInt16(r.read_u16()?),
        kind::INT_32_TYPE => V::Int32(r.read_i32()?),
        kind::UINT_32_TYPE => V::UInt32(r.read_u32()?),
        kind::INT_64_TYPE => V::Int64(r.read_i64()?),
        kind::UINT_64_TYPE => V::UInt64(r.read_u64()?),
        kind::FLOAT_32_TYPE => V::Float32Bits(r.read_u32()?),
        kind::FLOAT_64_TYPE => V::Float64Bits(r.read_u64()?),
        kind::FLOAT_128_TYPE => V::Float128(r.read_f128()?.to_be_bytes()),
        kind::CHAR_8_TYPE => V::Char8(r.read_u8()?),
        kind::CHAR_32_TYPE => V::Char32(r.read_u32()?),
        kind::ENUMERATION_TYPE => V::Enumeration(r.read_i32()?),
        kind::STRING_TYPE => V::String(get_wstring(r)?),
        other => {
            return Err(V1Error::Invalid(alloc::format!(
                "AnnotationMemberValue has no member for the kind {other}"
            )));
        }
    })
}

fn get_usage(r: &mut Reader<'_>) -> R<AnnotationUsage> {
    let type_id = get_type_id(r)?;
    let member = get_seq(r, |r| {
        Ok(AnnotationUsageMember {
            member_id: r.read_u32()?,
            value: get_value(r)?,
        })
    })?;
    Ok(AnnotationUsage { type_id, member })
}

fn get_usages(r: &mut Reader<'_>) -> R<Vec<AnnotationUsage>> {
    get_seq(r, get_usage)
}

fn get_type_property(r: &mut Reader<'_>) -> R<TypeProperty> {
    Ok(TypeProperty {
        flag: r.read_u16()?,
        type_id: get_type_id(r)?,
        name: get_name(r)?,
    })
}

fn get_member_property(r: &mut Reader<'_>) -> R<MemberProperty> {
    Ok(MemberProperty {
        flag: r.read_u16()?,
        member_id: r.read_u32()?,
        type_id: get_type_id(r)?,
        name: get_name(r)?,
    })
}

fn empty_base() -> TypeBase {
    TypeBase {
        property: TypeProperty {
            flag: 0,
            type_id: TypeId::NONE,
            name: String::new(),
        },
        annotation: Vec::new(),
    }
}

fn empty_member() -> Member {
    Member {
        property: MemberProperty {
            flag: 0,
            member_id: 0,
            type_id: TypeId::NONE,
            name: String::new(),
        },
        annotation: Vec::new(),
    }
}

/// Reads the members of `Type`; true when `id` was one of them.
fn get_base_param(id: u32, r: &mut Reader<'_>, base: &mut TypeBase) -> R<bool> {
    match id {
        x if x == u32::from(ID_PROPERTY) => base.property = get_type_property(r)?,
        x if x == u32::from(ID_ANNOTATION) => base.annotation = get_usages(r)?,
        _ => return Ok(false),
    }
    Ok(true)
}

/// Reads the members of `Member`; true when `id` was one of them.
fn get_member_param(id: u32, r: &mut Reader<'_>, member: &mut Member) -> R<bool> {
    match id {
        x if x == u32::from(ID_PROPERTY) => member.property = get_member_property(r)?,
        x if x == u32::from(ID_ANNOTATION) => member.annotation = get_usages(r)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn get_member(r: &mut Reader<'_>) -> R<Member> {
    let mut member = empty_member();
    read_list(r, |id, r| get_member_param(id, r, &mut member))?;
    Ok(member)
}

fn get_union_member(r: &mut Reader<'_>) -> R<UnionMember> {
    let mut member = empty_member();
    let mut label = Vec::new();
    read_list(r, |id, r| {
        if get_member_param(id, r, &mut member)? {
            return Ok(true);
        }
        if id == u32::from(ID_LABEL) {
            label = get_seq(r, |r| Ok(r.read_i32()?))?;
            return Ok(true);
        }
        Ok(false)
    })?;
    Ok(UnionMember { member, label })
}

fn get_annotation_member(r: &mut Reader<'_>) -> R<AnnotationMember> {
    let mut member = empty_member();
    let mut default_value = None;
    read_list(r, |id, r| {
        if get_member_param(id, r, &mut member)? {
            return Ok(true);
        }
        if id == u32::from(ID_DEFAULT_VALUE) {
            default_value = Some(get_value(r)?);
            return Ok(true);
        }
        Ok(false)
    })?;
    Ok(AnnotationMember {
        member,
        default_value,
    })
}

fn get_collection_flat(r: &mut Reader<'_>) -> R<(TypeBase, CollectionElement)> {
    let property = get_type_property(r)?;
    let annotation = get_usages(r)?;
    let element_type = get_type_id(r)?;
    let element_shared = r.read_bool()?;
    Ok((
        TypeBase {
            property,
            annotation,
        },
        CollectionElement {
            element_type,
            element_shared,
        },
    ))
}

fn get_structure(r: &mut Reader<'_>) -> R<StructureType> {
    let mut base = empty_base();
    let mut base_type = TypeId::NONE;
    let mut member = Vec::new();
    read_list(r, |id, r| {
        if get_base_param(id, r, &mut base)? {
            return Ok(true);
        }
        match id {
            x if x == u32::from(ID_BASE_TYPE) => base_type = get_type_id(r)?,
            x if x == u32::from(ID_MEMBER_STRUCT) => member = get_seq(r, get_member)?,
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok(StructureType {
        base,
        base_type,
        member,
    })
}

fn get_union(r: &mut Reader<'_>) -> R<UnionType> {
    let mut base = empty_base();
    let mut member = Vec::new();
    read_list(r, |id, r| {
        if get_base_param(id, r, &mut base)? {
            return Ok(true);
        }
        if id == u32::from(ID_UNION_MEMBER) {
            member = get_seq(r, get_union_member)?;
            return Ok(true);
        }
        Ok(false)
    })?;
    Ok(UnionType { base, member })
}

fn get_annotation_type(r: &mut Reader<'_>) -> R<AnnotationType> {
    let mut base = empty_base();
    let mut base_type = TypeId::NONE;
    let mut member = Vec::new();
    read_list(r, |id, r| {
        if get_base_param(id, r, &mut base)? {
            return Ok(true);
        }
        match id {
            x if x == u32::from(ID_BASE_TYPE) => base_type = get_type_id(r)?,
            x if x == u32::from(ID_MEMBER_STRUCT) => member = get_seq(r, get_annotation_member)?,
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok(AnnotationType {
        base,
        base_type,
        member,
    })
}

fn get_alias(r: &mut Reader<'_>) -> R<AliasType> {
    let mut base = empty_base();
    let mut base_type = TypeId::NONE;
    read_list(r, |id, r| {
        if get_base_param(id, r, &mut base)? {
            return Ok(true);
        }
        if id == u32::from(ID_BASE_TYPE) {
            base_type = get_type_id(r)?;
            return Ok(true);
        }
        Ok(false)
    })?;
    Ok(AliasType { base, base_type })
}

fn get_array(r: &mut Reader<'_>) -> R<ArrayType> {
    let mut base = empty_base();
    let mut element = CollectionElement {
        element_type: TypeId::NONE,
        element_shared: false,
    };
    let mut bound = Vec::new();
    read_list(r, |id, r| {
        if get_base_param(id, r, &mut base)? {
            return Ok(true);
        }
        match id {
            x if x == u32::from(ID_ELEMENT_TYPE) => element.element_type = get_type_id(r)?,
            x if x == u32::from(ID_ELEMENT_SHARED) => element.element_shared = r.read_bool()?,
            x if x == u32::from(ID_BOUND_ARRAY) => bound = get_seq(r, |r| Ok(r.read_u32()?))?,
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok(ArrayType {
        base,
        element,
        bound,
    })
}

fn get_bit_set(r: &mut Reader<'_>) -> R<BitSetType> {
    let mut base = empty_base();
    let mut bit_bound = 0;
    let mut bit = Vec::new();
    read_list(r, |id, r| {
        if get_base_param(id, r, &mut base)? {
            return Ok(true);
        }
        match id {
            x if x == u32::from(ID_BIT_BOUND) => bit_bound = r.read_u32()?,
            x if x == u32::from(ID_BIT_OR_CONSTANT) => {
                bit = get_seq(r, |r| {
                    Ok(Bit {
                        index: r.read_i32()?,
                        name: get_name(r)?,
                    })
                })?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok(BitSetType {
        base,
        bit_bound,
        bit,
    })
}

fn get_enumeration(r: &mut Reader<'_>) -> R<EnumerationType> {
    let mut base = empty_base();
    let mut bit_bound = 0;
    let mut constant = Vec::new();
    read_list(r, |id, r| {
        if get_base_param(id, r, &mut base)? {
            return Ok(true);
        }
        match id {
            x if x == u32::from(ID_BIT_BOUND) => bit_bound = r.read_u32()?,
            x if x == u32::from(ID_BIT_OR_CONSTANT) => {
                constant = get_seq(r, |r| {
                    Ok(EnumeratedConstant {
                        value: r.read_i32()?,
                        name: get_name(r)?,
                    })
                })?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok(EnumerationType {
        base,
        bit_bound,
        constant,
    })
}

fn get_library(r: &mut Reader<'_>, depth: usize) -> R<TypeLibrary> {
    if depth > MAX_MODULE_DEPTH {
        return Err(V1Error::Invalid(String::from(
            "modules nest deeper than this reader follows",
        )));
    }
    Ok(TypeLibrary {
        element: get_seq(r, |r| get_element(r, depth))?,
    })
}

/// The selected member of a `TypeLibraryElement` of the kind `selected`.
fn get_element_body(r: &mut Reader<'_>, selected: i16, depth: usize) -> R<LibraryElement> {
    Ok(match selected {
        kind::ALIAS_TYPE => LibraryElement::Alias(get_alias(r)?),
        kind::ANNOTATION_TYPE => LibraryElement::Annotation(get_annotation_type(r)?),
        kind::ARRAY_TYPE => LibraryElement::Array(get_array(r)?),
        kind::BITSET_TYPE => LibraryElement::BitSet(get_bit_set(r)?),
        kind::ENUMERATION_TYPE => LibraryElement::Enumeration(get_enumeration(r)?),
        kind::MAP_TYPE => {
            let (base, element) = get_collection_flat(r)?;
            let key_element_type = get_type_id(r)?;
            let bound = r.read_u32()?;
            LibraryElement::Map(MapType {
                base,
                element,
                key_element_type,
                bound,
            })
        }
        kind::SEQUENCE_TYPE => {
            let (base, element) = get_collection_flat(r)?;
            let bound = r.read_u32()?;
            LibraryElement::Sequence(SequenceType {
                base,
                element,
                bound,
            })
        }
        kind::STRING_TYPE => {
            let (base, element) = get_collection_flat(r)?;
            let bound = r.read_u32()?;
            LibraryElement::String(StringType {
                base,
                element,
                bound,
            })
        }
        kind::STRUCTURE_TYPE => LibraryElement::Structure(get_structure(r)?),
        kind::UNION_TYPE => LibraryElement::Union(get_union(r)?),
        kind::MODULE_ELEMENT => {
            let name = get_name(r)?;
            let library = get_library(r, depth + 1)?;
            LibraryElement::Module(Module { name, library })
        }
        other => {
            return Err(V1Error::Invalid(alloc::format!(
                "TypeLibraryElement has no member for the kind {other}"
            )));
        }
    })
}

/// A `TypeLibraryElement`: the discriminator, then the member it selects.
///
/// zerodds-lint: recursion-depth = [`MAX_MODULE_DEPTH`] (a module's library
/// is read through `get_library`, which stops at that depth)
fn get_element(r: &mut Reader<'_>, depth: usize) -> R<LibraryElement> {
    let mut selected: Option<i16> = None;
    let mut element: Option<LibraryElement> = None;
    read_list(r, |id, r| {
        if id == 0 {
            selected = Some(r.read_i16()?);
            return Ok(true);
        }
        let Some(kind_selected) = selected else {
            return Err(V1Error::Invalid(String::from(
                "the member of a TypeLibraryElement comes before its discriminator",
            )));
        };
        let expected = match kind_selected {
            kind::ALIAS_TYPE => 1,
            kind::ANNOTATION_TYPE => 2,
            kind::ARRAY_TYPE => 3,
            kind::BITSET_TYPE => 4,
            kind::ENUMERATION_TYPE => 5,
            kind::MAP_TYPE => 6,
            kind::SEQUENCE_TYPE => 7,
            kind::STRING_TYPE => 8,
            kind::STRUCTURE_TYPE => 9,
            kind::UNION_TYPE => 10,
            kind::MODULE_ELEMENT => 11,
            other => {
                return Err(V1Error::Invalid(alloc::format!(
                    "TypeLibraryElement has no member for the kind {other}"
                )));
            }
        };
        if id != expected {
            return Err(V1Error::Invalid(alloc::format!(
                "member {id} of a TypeLibraryElement whose discriminator selects member {expected}"
            )));
        }
        element = Some(get_element_body(r, kind_selected, depth)?);
        Ok(true)
    })?;
    element.ok_or_else(|| {
        V1Error::Invalid(String::from(
            "a TypeLibraryElement without the member its discriminator selects",
        ))
    })
}

pub(super) fn decode_object(bytes: &[u8], big: bool) -> R<TypeObjectV1> {
    let mut r = reader(bytes, big);
    let mut library = TypeLibrary::default();
    let mut the_type = Vec::new();
    read_list(&mut r, |id, r| {
        match id {
            x if x == u32::from(ID_LIBRARY) => library = get_library(r, 0)?,
            x if x == u32::from(ID_THE_TYPE) => the_type = get_seq(r, get_type_id)?,
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok(TypeObjectV1 { library, the_type })
}

pub(super) fn decode_type_id(bytes: &[u8], big: bool) -> R<TypeId> {
    let mut r = reader(bytes, big);
    get_type_id(&mut r)
}
