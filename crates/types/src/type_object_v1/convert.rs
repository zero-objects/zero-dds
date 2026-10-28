// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `TypeObjectV1` and the Complete TypeObjects of XTypes 1.3.
//!
//! [`from_complete`] describes a topic type by an Annex B library of XTypes
//! 1.1, [`to_complete`] reads such a library as Complete TypeObjects, which
//! the rest of the stack (assignability, the type registry, the TypeLookup
//! service) takes.
//!
//! # What Annex B of 1.1 can carry
//!
//! The library has the names, the member ids, the flags `IS_KEY`,
//! `IS_OPTIONAL` and `IS_SHAREABLE`, the extensibility and the nesting of a
//! type, the bounds, the labels, the enumerators, the bits, the aliases, the
//! annotation types and the applications of annotations by type id. What 1.3
//! added to the type model has no member in it and is not carried:
//! `try_construct` (every member is read back as `DISCARD`, the default),
//! `@must_understand` (other than the one a key member has), `@autoid(HASH)`, `@verbatim`, `@unit`, `@min`, `@max`,
//! `@hashid`, the default value of a member, `@ignore_literal_names`, the
//! `@default_literal` of an enumeration (the first literal is the default
//! again) and the bit sets of 1.3 (a bit set with bit fields has no form at
//! all and is refused). Two kinds of 1.3 have a form that is not their own:
//! `int8` is refused (1.1 has no 8-bit signed integer) and `uint8` is
//! `BYTE_TYPE`; `char16` is `CHAR_32_TYPE`.
//!
//! A type that refers to itself, directly or through others, has no form in
//! Annex B: the id of a type is the hash of a serialization that contains
//! the ids of the types it uses, so a cycle has none (the text of 1.1 has no
//! strongly connected components, which 1.3 introduced for that). Both
//! directions refuse it by name.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::hash::compute_complete_hash;
use crate::resolve::TypeRegistry;
use crate::type_identifier::{
    CollectionElementFlag, EquivalenceHash, PlainCollectionHeader, PrimitiveKind, TypeIdentifier,
    kinds::{EK_BOTH, EK_COMPLETE},
};
use crate::type_object::common::{
    AnnotationParameterValue, AppliedAnnotation, AppliedAnnotationParameter,
    AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CommonStructMember,
    CommonUnionMember, CompleteMemberDetail, CompleteTypeDetail, NameHash,
    OptionalAppliedAnnotationSeq,
};
use crate::type_object::complete::{
    CompleteAliasBody, CompleteAliasHeader, CompleteAliasType, CompleteAnnotationHeader,
    CompleteAnnotationParameter, CompleteAnnotationType, CompleteBitflag, CompleteBitmaskType,
    CompleteDiscriminatorMember, CompleteEnumeratedHeader, CompleteEnumeratedLiteral,
    CompleteEnumeratedType, CompleteStructHeader, CompleteStructMember, CompleteStructType,
    CompleteTypeObject, CompleteUnionHeader, CompleteUnionMember, CompleteUnionType,
};
use crate::type_object::flags::{
    AliasMemberFlag, AliasTypeFlag, AnnotationParameterFlag, AnnotationTypeFlag, BitflagFlag,
    BitmaskTypeFlag, EnumLiteralFlag, EnumTypeFlag, StructMemberFlag, StructTypeFlag,
    UnionDiscriminatorFlag, UnionMemberFlag, UnionTypeFlag,
};
use crate::type_object::minimal::{
    CommonAnnotationParameter, CommonBitflag, CommonDiscriminatorMember, CommonEnumeratedHeader,
    CommonEnumeratedLiteral,
};

use super::{
    AliasType, AnnotationMember, AnnotationMemberValue, AnnotationType, AnnotationUsage,
    AnnotationUsageMember, ArrayType, Bit, BitSetType, CollectionElement, EnumeratedConstant,
    EnumerationType, LibraryElement, MapType, Member, MemberProperty, Module, SequenceType,
    StringType, StructureType, TypeBase, TypeId, TypeLibrary, TypeObjectV1, TypeProperty,
    UnionMember, UnionType, V1Error, kind, member_flag, type_flag,
};

type R<T> = Result<T, V1Error>;

/// How deep the types of a model nest in one another. Plain collections of
/// collections are bounded by the decoder of the identifiers; named types
/// reach each other through the registry and are bounded by a repeat
/// check.
const MAX_DEPTH: usize = 64;

// -- names ---------------------------------------------------------------

fn split_name(qualified: &str) -> (Vec<String>, String) {
    let trimmed = qualified.strip_prefix("::").unwrap_or(qualified);
    let mut parts: Vec<String> = trimmed.split("::").map(ToString::to_string).collect();
    let name = parts.pop().unwrap_or_default();
    (parts, name)
}

fn primitive_name(k: i16) -> &'static str {
    match k {
        kind::BOOLEAN_TYPE => "boolean",
        kind::BYTE_TYPE => "octet",
        kind::INT_16_TYPE => "short",
        kind::UINT_16_TYPE => "unsigned short",
        kind::INT_32_TYPE => "long",
        kind::UINT_32_TYPE => "unsigned long",
        kind::INT_64_TYPE => "long long",
        kind::UINT_64_TYPE => "unsigned long long",
        kind::FLOAT_32_TYPE => "float",
        kind::FLOAT_64_TYPE => "double",
        kind::FLOAT_128_TYPE => "long double",
        kind::CHAR_8_TYPE => "char",
        kind::CHAR_32_TYPE => "wchar",
        _ => "?",
    }
}

fn primitive_v1(p: PrimitiveKind) -> R<i16> {
    Ok(match p {
        PrimitiveKind::Boolean => kind::BOOLEAN_TYPE,
        PrimitiveKind::Byte | PrimitiveKind::UInt8 => kind::BYTE_TYPE,
        PrimitiveKind::Int16 => kind::INT_16_TYPE,
        PrimitiveKind::UInt16 => kind::UINT_16_TYPE,
        PrimitiveKind::Int32 => kind::INT_32_TYPE,
        PrimitiveKind::UInt32 => kind::UINT_32_TYPE,
        PrimitiveKind::Int64 => kind::INT_64_TYPE,
        PrimitiveKind::UInt64 => kind::UINT_64_TYPE,
        PrimitiveKind::Float32 => kind::FLOAT_32_TYPE,
        PrimitiveKind::Float64 => kind::FLOAT_64_TYPE,
        PrimitiveKind::Float128 => kind::FLOAT_128_TYPE,
        PrimitiveKind::Char8 => kind::CHAR_8_TYPE,
        PrimitiveKind::Char16 => kind::CHAR_32_TYPE,
        PrimitiveKind::Int8 => {
            return Err(V1Error::Unrepresentable(String::from(
                "int8: XTypes 1.1 has no 8-bit signed integer",
            )));
        }
    })
}

fn primitive_complete(k: i16) -> R<PrimitiveKind> {
    Ok(match k {
        kind::BOOLEAN_TYPE => PrimitiveKind::Boolean,
        kind::BYTE_TYPE => PrimitiveKind::Byte,
        kind::INT_16_TYPE => PrimitiveKind::Int16,
        kind::UINT_16_TYPE => PrimitiveKind::UInt16,
        kind::INT_32_TYPE => PrimitiveKind::Int32,
        kind::UINT_32_TYPE => PrimitiveKind::UInt32,
        kind::INT_64_TYPE => PrimitiveKind::Int64,
        kind::UINT_64_TYPE => PrimitiveKind::UInt64,
        kind::FLOAT_32_TYPE => PrimitiveKind::Float32,
        kind::FLOAT_64_TYPE => PrimitiveKind::Float64,
        kind::FLOAT_128_TYPE => PrimitiveKind::Float128,
        kind::CHAR_8_TYPE => PrimitiveKind::Char8,
        kind::CHAR_32_TYPE => PrimitiveKind::Char16,
        other => {
            return Err(V1Error::Invalid(format!(
                "{other} is no primitive type kind"
            )));
        }
    })
}

fn unrepresentable<T>(what: impl Into<String>) -> R<T> {
    Err(V1Error::Unrepresentable(what.into()))
}

// ======================================================================
// Complete -> V1
// ======================================================================

/// The library that describes `root`, a Complete hash identifier or a
/// member of a strongly connected component, and the types it depends on.
///
/// The registry has to hold the objects behind every hash the type
/// reaches, as it does for a type an endpoint announces.
///
/// # Errors
/// [`V1Error::Unresolved`] for an object the registry does not hold,
/// [`V1Error::Unrepresentable`] for what Annex B has no form for (see the
/// module documentation), a type that refers to itself among it.
pub fn from_complete(root: &TypeIdentifier, registry: &TypeRegistry) -> R<TypeObjectV1> {
    let mut out = FromComplete {
        registry,
        items: Vec::new(),
        memo: BTreeMap::new(),
        visiting: BTreeMap::new(),
    };
    let id = out.type_id(root, 0)?;
    let mut library = TypeLibrary::default();
    for (path, element) in out.items {
        insert_at(&mut library, &path, element);
    }
    Ok(TypeObjectV1 {
        library,
        the_type: alloc::vec![id],
    })
}

/// Puts `element` into the module `path` of `library`, creating the modules.
///
/// zerodds-lint: recursion-depth 64 (the segments of one qualified name; a
/// name has at most `TYPE_NAME_MAX_LENGTH` octets, so far fewer)
fn insert_at(library: &mut TypeLibrary, path: &[String], element: LibraryElement) {
    let Some((first, rest)) = path.split_first() else {
        library.element.push(element);
        return;
    };
    let existing = library
        .element
        .iter()
        .position(|e| matches!(e, LibraryElement::Module(m) if m.name == *first));
    let at = existing.unwrap_or_else(|| {
        library.element.push(LibraryElement::Module(Module {
            name: first.clone(),
            library: TypeLibrary::default(),
        }));
        library.element.len() - 1
    });
    if let LibraryElement::Module(m) = &mut library.element[at] {
        insert_at(&mut m.library, rest, element);
    }
}

struct FromComplete<'a> {
    registry: &'a TypeRegistry,
    items: Vec<(Vec<String>, LibraryElement)>,
    memo: BTreeMap<Vec<u8>, TypeId>,
    /// The objects being converted, by key, with the kind of each: a
    /// reference back to one of them is a cycle.
    visiting: BTreeMap<Vec<u8>, i16>,
}

fn key_of_hash(hash: &EquivalenceHash) -> Vec<u8> {
    let mut key = alloc::vec![1u8];
    key.extend_from_slice(&hash.0);
    key
}

fn key_of_component(hash: &EquivalenceHash, index: i32) -> Vec<u8> {
    let mut key = alloc::vec![2u8];
    key.extend_from_slice(&hash.0);
    key.extend_from_slice(&index.to_be_bytes());
    key
}

fn bound_of(b: u32) -> String {
    if b == 0 {
        String::new()
    } else {
        format!(",{b}")
    }
}

impl FromComplete<'_> {
    /// The V1 id of the type a TypeIdentifier names, adding the definitions
    /// it needs to the library.
    ///
    /// zerodds-lint: recursion-depth 64 (a plain collection inside a plain
    /// collection is bounded by the identifier decoder; a named type
    /// returns through `object`, which stops at a repeat and at `depth`)
    fn type_id(&mut self, ti: &TypeIdentifier, depth: usize) -> R<TypeId> {
        if depth > MAX_DEPTH {
            return unrepresentable("types nested deeper than the conversion follows");
        }
        match ti {
            TypeIdentifier::None => Ok(TypeId::NONE),
            TypeIdentifier::Primitive(p) => Ok(TypeId::Primitive(primitive_v1(*p)?)),
            TypeIdentifier::String8Small { bound } => {
                self.string(kind::CHAR_8_TYPE, u32::from(*bound))
            }
            TypeIdentifier::String8Large { bound } => self.string(kind::CHAR_8_TYPE, *bound),
            TypeIdentifier::String16Small { bound } => {
                self.string(kind::CHAR_32_TYPE, u32::from(*bound))
            }
            TypeIdentifier::String16Large { bound } => self.string(kind::CHAR_32_TYPE, *bound),
            TypeIdentifier::PlainSequenceSmall {
                header,
                bound,
                element,
            } => self.sequence(header, u32::from(*bound), element, depth),
            TypeIdentifier::PlainSequenceLarge {
                header,
                bound,
                element,
            } => self.sequence(header, *bound, element, depth),
            TypeIdentifier::PlainArraySmall {
                header,
                array_bounds,
                element,
            } => {
                let bounds: Vec<u32> = array_bounds.iter().map(|b| u32::from(*b)).collect();
                self.array(header, bounds, element, depth)
            }
            TypeIdentifier::PlainArrayLarge {
                header,
                array_bounds,
                element,
            } => self.array(header, array_bounds.clone(), element, depth),
            TypeIdentifier::PlainMapSmall {
                header,
                bound,
                element,
                key_flags,
                key,
            } => self.map(header, u32::from(*bound), element, *key_flags, key, depth),
            TypeIdentifier::PlainMapLarge {
                header,
                bound,
                element,
                key_flags,
                key,
            } => self.map(header, *bound, element, *key_flags, key, depth),
            TypeIdentifier::EquivalenceHashComplete(hash) => {
                let key = key_of_hash(hash);
                let object = self.registry.get_complete(hash).ok_or_else(|| {
                    V1Error::Unresolved(format!("the complete object of {:02x?}", hash.0))
                })?;
                self.object(key, object, depth)
            }
            TypeIdentifier::StronglyConnectedComponent(scc) => {
                if scc.equivalence_kind != EK_COMPLETE {
                    return unrepresentable(
                        "a minimal component identifier names no complete object",
                    );
                }
                let key = key_of_component(&scc.hash, scc.scc_index);
                let object = self
                    .registry
                    .component_member(&scc.hash, scc.scc_index)
                    .ok_or_else(|| {
                        V1Error::Unresolved(format!(
                            "member {} of the component {:02x?}",
                            scc.scc_index, scc.hash.0
                        ))
                    })?;
                self.object(key, object, depth)
            }
            TypeIdentifier::EquivalenceHashMinimal(_) => unrepresentable(
                "a minimal hash identifier names no complete object, which the library is made of",
            ),
            _ => unrepresentable("a type identifier of a kind this version does not define"),
        }
    }

    fn name_of(&self, id: TypeId) -> String {
        match id {
            TypeId::Primitive(k) => String::from(primitive_name(k)),
            other => self
                .items
                .iter()
                .find_map(|(path, e)| {
                    let base = e.base()?;
                    (base.property.type_id == other).then(|| {
                        let mut name = path.join("::");
                        if !name.is_empty() {
                            name.push_str("::");
                        }
                        name.push_str(&base.property.name);
                        name
                    })
                })
                .unwrap_or_else(|| String::from("recursive")),
        }
    }

    /// Adds a definition to the root of the library under its own id.
    fn add(&mut self, path: Vec<String>, mut element: LibraryElement) -> R<TypeId> {
        let id = TypeLibrary::type_id_of(&element)?
            .ok_or_else(|| V1Error::Invalid(String::from("a module is not added as a type")))?;
        if let Some(base) = element.base_mut() {
            base.property.type_id = id;
        }
        let present = self
            .items
            .iter()
            .any(|(_, e)| e.base().is_some_and(|b| b.property.type_id == id));
        if !present {
            self.items.push((path, element));
        }
        Ok(id)
    }

    fn base(name: String, flag: u16) -> TypeBase {
        TypeBase {
            property: TypeProperty {
                flag,
                type_id: TypeId::NONE,
                name,
            },
            annotation: Vec::new(),
        }
    }

    fn string(&mut self, element_kind: i16, bound: u32) -> R<TypeId> {
        let word = if element_kind == kind::CHAR_8_TYPE {
            "string"
        } else {
            "wstring"
        };
        let name = if bound == 0 {
            String::from(word)
        } else {
            format!("{word}<{bound}>")
        };
        self.add(
            Vec::new(),
            LibraryElement::String(StringType {
                base: Self::base(name, 0),
                element: CollectionElement {
                    element_type: TypeId::Primitive(element_kind),
                    element_shared: false,
                },
                bound,
            }),
        )
    }

    fn sequence(
        &mut self,
        header: &PlainCollectionHeader,
        bound: u32,
        element: &TypeIdentifier,
        depth: usize,
    ) -> R<TypeId> {
        let element_type = self.type_id(element, depth + 1)?;
        let name = format!(
            "sequence<{}{}>",
            self.name_of(element_type),
            bound_of(bound)
        );
        self.add(
            Vec::new(),
            LibraryElement::Sequence(SequenceType {
                base: Self::base(name, 0),
                element: CollectionElement {
                    element_type,
                    element_shared: shared(header.element_flags.0),
                },
                bound,
            }),
        )
    }

    fn array(
        &mut self,
        header: &PlainCollectionHeader,
        bound: Vec<u32>,
        element: &TypeIdentifier,
        depth: usize,
    ) -> R<TypeId> {
        let element_type = self.type_id(element, depth + 1)?;
        let mut name = self.name_of(element_type);
        for b in &bound {
            name.push_str(&format!("[{b}]"));
        }
        self.add(
            Vec::new(),
            LibraryElement::Array(ArrayType {
                base: Self::base(name, 0),
                element: CollectionElement {
                    element_type,
                    element_shared: shared(header.element_flags.0),
                },
                bound,
            }),
        )
    }

    fn map(
        &mut self,
        header: &PlainCollectionHeader,
        bound: u32,
        element: &TypeIdentifier,
        _key_flags: CollectionElementFlag,
        key: &TypeIdentifier,
        depth: usize,
    ) -> R<TypeId> {
        let key_element_type = self.type_id(key, depth + 1)?;
        let element_type = self.type_id(element, depth + 1)?;
        let name = format!(
            "map<{},{}{}>",
            self.name_of(key_element_type),
            self.name_of(element_type),
            bound_of(bound)
        );
        self.add(
            Vec::new(),
            LibraryElement::Map(MapType {
                base: Self::base(name, 0),
                element: CollectionElement {
                    element_type,
                    element_shared: shared(header.element_flags.0),
                },
                key_element_type,
                bound,
            }),
        )
    }

    /// The definition of a named Complete object.
    fn object(&mut self, key: Vec<u8>, object: &CompleteTypeObject, depth: usize) -> R<TypeId> {
        if let Some(id) = self.memo.get(&key) {
            return Ok(*id);
        }
        if self.visiting.contains_key(&key) {
            return unrepresentable(
                "a type that refers to itself: its id would be the hash of a serialization \
                 that contains it",
            );
        }
        let own_kind = match object {
            CompleteTypeObject::Struct(_) => kind::STRUCTURE_TYPE,
            CompleteTypeObject::Union(_) => kind::UNION_TYPE,
            CompleteTypeObject::Enumerated(_) => kind::ENUMERATION_TYPE,
            CompleteTypeObject::Bitmask(_) => kind::BITSET_TYPE,
            CompleteTypeObject::Alias(_) => kind::ALIAS_TYPE,
            CompleteTypeObject::Annotation(_) => kind::ANNOTATION_TYPE,
            CompleteTypeObject::Sequence(_) => kind::SEQUENCE_TYPE,
            CompleteTypeObject::Array(_) => kind::ARRAY_TYPE,
            CompleteTypeObject::Map(_) => kind::MAP_TYPE,
            CompleteTypeObject::Bitset(_) => {
                return unrepresentable(
                    "a bit set with bit fields: XTypes 1.1 has bit sets of flags only",
                );
            }
            _ => return unrepresentable("a TypeObject of a kind this version does not define"),
        };
        self.visiting.insert(key.clone(), own_kind);
        let outcome = self.convert_object(object, depth);
        self.visiting.remove(&key);
        let id = outcome?;
        self.memo.insert(key, id);
        Ok(id)
    }

    fn convert_object(&mut self, object: &CompleteTypeObject, depth: usize) -> R<TypeId> {
        match object {
            CompleteTypeObject::Struct(s) => {
                let (path, name) = split_name(&s.header.detail.type_name);
                let mut flag = 0;
                if s.struct_flags.has(StructTypeFlag::IS_FINAL) {
                    flag |= type_flag::IS_FINAL;
                }
                if s.struct_flags.has(StructTypeFlag::IS_MUTABLE) {
                    flag |= type_flag::IS_MUTABLE;
                }
                if s.struct_flags.has(StructTypeFlag::IS_NESTED) {
                    flag |= type_flag::IS_NESTED;
                }
                let base_type = self.type_id(&s.header.base_type, depth + 1)?;
                let mut base = Self::base(name, flag);
                base.annotation = self.usages(&s.header.detail.ann_custom, depth)?;
                let mut member = Vec::new();
                for m in &s.member_seq {
                    let mut flags = 0;
                    if m.common.member_flags.has(StructMemberFlag::IS_KEY) {
                        flags |= member_flag::IS_KEY;
                    }
                    if m.common.member_flags.has(StructMemberFlag::IS_OPTIONAL) {
                        flags |= member_flag::IS_OPTIONAL;
                    }
                    if m.common.member_flags.has(StructMemberFlag::IS_EXTERNAL) {
                        flags |= member_flag::IS_SHAREABLE;
                    }
                    member.push(Member {
                        property: MemberProperty {
                            flag: flags,
                            member_id: m.common.member_id,
                            type_id: self.type_id(&m.common.member_type_id, depth + 1)?,
                            name: m.detail.name.clone(),
                        },
                        annotation: self.usages(&m.detail.ann_custom, depth)?,
                    });
                }
                self.add(
                    path,
                    LibraryElement::Structure(StructureType {
                        base,
                        base_type,
                        member,
                    }),
                )
            }
            CompleteTypeObject::Union(u) => {
                let (path, name) = split_name(&u.header.detail.type_name);
                let mut flag = 0;
                if u.union_flags.0 & StructTypeFlag::IS_FINAL != 0 {
                    flag |= type_flag::IS_FINAL;
                }
                if u.union_flags.0 & StructTypeFlag::IS_MUTABLE != 0 {
                    flag |= type_flag::IS_MUTABLE;
                }
                if u.union_flags.0 & StructTypeFlag::IS_NESTED != 0 {
                    flag |= type_flag::IS_NESTED;
                }
                let mut base = Self::base(name, flag);
                base.annotation = self.usages(&u.header.detail.ann_custom, depth)?;
                let mut member = Vec::new();
                // The discriminator is the first member (member id 0).
                let mut disc_flags = 0;
                if u.discriminator
                    .common
                    .member_flags
                    .has(UnionDiscriminatorFlag::IS_KEY)
                {
                    disc_flags |= member_flag::IS_KEY;
                }
                member.push(UnionMember {
                    member: Member {
                        property: MemberProperty {
                            flag: disc_flags,
                            member_id: 0,
                            type_id: self.type_id(&u.discriminator.common.type_id, depth + 1)?,
                            name: String::from("discriminator"),
                        },
                        annotation: self.usages(&u.discriminator.ann_custom, depth)?,
                    },
                    label: Vec::new(),
                });
                for m in &u.member_seq {
                    let mut flags = 0;
                    if m.common.member_flags.0 & UnionMemberFlag::IS_DEFAULT != 0 {
                        flags |= member_flag::IS_UNION_DEFAULT;
                    }
                    member.push(UnionMember {
                        member: Member {
                            property: MemberProperty {
                                flag: flags,
                                member_id: m.common.member_id,
                                type_id: self.type_id(&m.common.type_id, depth + 1)?,
                                name: m.detail.name.clone(),
                            },
                            annotation: self.usages(&m.detail.ann_custom, depth)?,
                        },
                        label: m.common.label_seq.clone(),
                    });
                }
                self.add(path, LibraryElement::Union(UnionType { base, member }))
            }
            CompleteTypeObject::Enumerated(e) => {
                let (path, name) = split_name(&e.header.detail.type_name);
                let mut flag = 0;
                if e.enum_flags.has(EnumTypeFlag::IS_FINAL) {
                    flag |= type_flag::IS_FINAL;
                }
                if e.enum_flags.0 & StructTypeFlag::IS_MUTABLE != 0 {
                    flag |= type_flag::IS_MUTABLE;
                }
                let mut base = Self::base(name, flag);
                base.annotation = self.usages(&e.header.detail.ann_custom, depth)?;
                let constant = e
                    .literal_seq
                    .iter()
                    .map(|l| EnumeratedConstant {
                        value: l.common.value,
                        name: l.detail.name.clone(),
                    })
                    .collect();
                self.add(
                    path,
                    LibraryElement::Enumeration(EnumerationType {
                        base,
                        bit_bound: u32::from(e.header.common.bit_bound),
                        constant,
                    }),
                )
            }
            CompleteTypeObject::Bitmask(b) => {
                let (path, name) = split_name(&b.detail.type_name);
                let mut base = Self::base(name, 0);
                base.annotation = self.usages(&b.detail.ann_custom, depth)?;
                let bit = b
                    .flag_seq
                    .iter()
                    .map(|f| Bit {
                        index: i32::from(f.common.position),
                        name: f.detail.name.clone(),
                    })
                    .collect();
                self.add(
                    path,
                    LibraryElement::BitSet(BitSetType {
                        base,
                        bit_bound: u32::from(b.bit_bound),
                        bit,
                    }),
                )
            }
            CompleteTypeObject::Alias(a) => {
                let (path, name) = split_name(&a.header.detail.type_name);
                let mut base = Self::base(name, 0);
                base.annotation = self.usages(&a.header.detail.ann_custom, depth)?;
                let base_type = self.type_id(&a.body.related_type, depth + 1)?;
                self.add(path, LibraryElement::Alias(AliasType { base, base_type }))
            }
            CompleteTypeObject::Annotation(a) => {
                let (path, name) = split_name(&a.header.annotation_name);
                let base = Self::base(name, 0);
                let mut member = Vec::new();
                for (index, p) in a.member_seq.iter().enumerate() {
                    member.push(AnnotationMember {
                        member: Member {
                            property: MemberProperty {
                                flag: 0,
                                member_id: u32::try_from(index).map_err(|_| {
                                    V1Error::Invalid(String::from("too many annotation members"))
                                })?,
                                type_id: self.type_id(&p.common.member_type_id, depth + 1)?,
                                name: p.name.clone(),
                            },
                            annotation: Vec::new(),
                        },
                        default_value: Some(value_v1(&p.default_value)?),
                    });
                }
                self.add(
                    path,
                    LibraryElement::Annotation(AnnotationType {
                        base,
                        base_type: TypeId::NONE,
                        member,
                    }),
                )
            }
            CompleteTypeObject::Sequence(c) => {
                if c.detail.as_ref().is_some_and(|d| d.ann_custom.0.is_some())
                    || c.element.ann_custom.0.is_some()
                {
                    return unrepresentable("annotations on a sequence type");
                }
                let element_type = self.type_id(&c.element.common.type_id, depth + 1)?;
                // Annex B of 1.3: an anonymous sequence has no detail, and
                // the 1.1 element takes no name then.
                let (path, name) =
                    split_name(c.detail.as_ref().map_or("", |d| d.type_name.as_str()));
                self.add(
                    path,
                    LibraryElement::Sequence(SequenceType {
                        base: Self::base(name, 0),
                        element: CollectionElement {
                            element_type,
                            element_shared: shared(c.element.common.element_flags.0),
                        },
                        bound: c.bound,
                    }),
                )
            }
            CompleteTypeObject::Array(c) => {
                if c.detail.ann_custom.0.is_some() || c.element.ann_custom.0.is_some() {
                    return unrepresentable("annotations on an array type");
                }
                let element_type = self.type_id(&c.element.common.type_id, depth + 1)?;
                let (path, name) = split_name(&c.detail.type_name);
                self.add(
                    path,
                    LibraryElement::Array(ArrayType {
                        base: Self::base(name, 0),
                        element: CollectionElement {
                            element_type,
                            element_shared: shared(c.element.common.element_flags.0),
                        },
                        bound: c.bound_seq.clone(),
                    }),
                )
            }
            CompleteTypeObject::Map(c) => {
                if c.detail.as_ref().is_some_and(|d| d.ann_custom.0.is_some())
                    || c.element.ann_custom.0.is_some()
                    || c.key.ann_custom.0.is_some()
                {
                    return unrepresentable("annotations on a map type");
                }
                let key_element_type = self.type_id(&c.key.common.type_id, depth + 1)?;
                let element_type = self.type_id(&c.element.common.type_id, depth + 1)?;
                let (path, name) =
                    split_name(c.detail.as_ref().map_or("", |d| d.type_name.as_str()));
                self.add(
                    path,
                    LibraryElement::Map(MapType {
                        base: Self::base(name, 0),
                        element: CollectionElement {
                            element_type,
                            element_shared: shared(c.element.common.element_flags.0),
                        },
                        key_element_type,
                        bound: c.bound,
                    }),
                )
            }
            _ => unrepresentable("a TypeObject of a kind this version does not define"),
        }
    }

    /// The annotations applied to a type or a member.
    fn usages(
        &mut self,
        applied: &OptionalAppliedAnnotationSeq,
        depth: usize,
    ) -> R<Vec<AnnotationUsage>> {
        let mut out = Vec::new();
        let Some(list) = applied.0.as_ref() else {
            return Ok(out);
        };
        for a in list {
            let type_id = self.type_id(&a.annotation_typeid, depth + 1)?;
            let TypeIdentifier::EquivalenceHashComplete(hash) = &a.annotation_typeid else {
                return unrepresentable(
                    "an annotation whose type is not a complete hash identifier",
                );
            };
            let Some(CompleteTypeObject::Annotation(annotation)) = self.registry.get_complete(hash)
            else {
                return Err(V1Error::Unresolved(String::from(
                    "the annotation type of an applied annotation",
                )));
            };
            let mut member = Vec::new();
            for p in &a.param_seq {
                let index = annotation
                    .member_seq
                    .iter()
                    .position(|m| NameHash::from_name(&m.name) == p.paramname_hash)
                    .ok_or_else(|| {
                        V1Error::Invalid(String::from(
                            "an annotation parameter that its annotation type does not have",
                        ))
                    })?;
                member.push(AnnotationUsageMember {
                    member_id: u32::try_from(index).map_err(|_| {
                        V1Error::Invalid(String::from("too many annotation members"))
                    })?,
                    value: value_v1(&p.value)?,
                });
            }
            out.push(AnnotationUsage { type_id, member });
        }
        Ok(out)
    }
}

fn shared(bits: u16) -> bool {
    bits & CollectionElementFlag::IS_EXTERNAL != 0
}

fn value_v1(v: &AnnotationParameterValue) -> R<AnnotationMemberValue> {
    use AnnotationMemberValue as V;
    use AnnotationParameterValue as P;
    Ok(match v {
        P::Boolean(b) => V::Boolean(*b),
        P::Byte(b) | P::UInt8(b) => V::Byte(*b),
        P::Int16(x) => V::Int16(*x),
        P::UInt16(x) => V::UInt16(*x),
        P::Int32(x) => V::Int32(*x),
        P::UInt32(x) => V::UInt32(*x),
        P::Int64(x) => V::Int64(*x),
        P::UInt64(x) => V::UInt64(*x),
        P::Float32Bits(x) => V::Float32Bits(*x),
        P::Float64Bits(x) => V::Float64Bits(*x),
        P::Float128Bits(x) => V::Float128(x.to_be_bytes()),
        P::Char8(c) => V::Char8(
            u8::try_from(u32::from(*c))
                .map_err(|_| V1Error::Unrepresentable(String::from("a char outside ISO-8859-1")))?,
        ),
        P::Char16(c) => V::Char32(u32::from(*c)),
        P::Enum(x) => V::Enumeration(*x),
        P::String8(s) | P::String16(s) => V::String(s.clone()),
        P::Int8(_) => return unrepresentable("an int8 annotation value"),
        _ => return unrepresentable("an annotation value of a kind this version does not define"),
    })
}

// ======================================================================
// V1 -> Complete
// ======================================================================

/// What a `TypeObjectV1` is as Complete TypeObjects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    /// Every named type of the library with the hash of its Complete
    /// object, the types a type needs before it.
    pub types: Vec<(EquivalenceHash, CompleteTypeObject)>,
    /// The identifiers of the types of `the_type`.
    pub roots: Vec<TypeIdentifier>,
}

/// Reads a `TypeObjectV1` as Complete TypeObjects.
///
/// # Errors
/// [`V1Error::Unresolved`] for an id the library does not define,
/// [`V1Error::Unrepresentable`] for a type that refers to itself (a Complete
/// object names its dependencies by hash) and for annotations on a
/// collection, [`V1Error::Invalid`] for what Annex B does not admit.
pub fn to_complete(object: &TypeObjectV1) -> R<Converted> {
    let mut index: BTreeMap<TypeId, (Vec<&str>, &LibraryElement)> = BTreeMap::new();
    index_library(&object.library, &mut Vec::new(), &mut index, 0)?;
    let mut input = ToComplete {
        index,
        memo: BTreeMap::new(),
        visiting: BTreeSet::new(),
        out: Vec::new(),
        annotation_members: BTreeMap::new(),
    };
    let mut roots = Vec::new();
    for id in &object.the_type {
        roots.push(input.identifier(*id, 0)?);
    }
    Ok(Converted {
        types: input.out,
        roots,
    })
}

/// zerodds-lint: recursion-depth 32 (module nesting, which the decoder
/// stops at `codec::MAX_MODULE_DEPTH`; the check below guards a library
/// built by hand)
fn index_library<'a>(
    library: &'a TypeLibrary,
    path: &mut Vec<&'a str>,
    index: &mut BTreeMap<TypeId, (Vec<&'a str>, &'a LibraryElement)>,
    depth: usize,
) -> R<()> {
    if depth > super::codec::MAX_MODULE_DEPTH {
        return Err(V1Error::Invalid(String::from(
            "modules nest deeper than this conversion follows",
        )));
    }
    for element in &library.element {
        match element {
            LibraryElement::Module(m) => {
                path.push(m.name.as_str());
                index_library(&m.library, path, index, depth + 1)?;
                path.pop();
            }
            other => {
                if let Some(base) = other.base() {
                    index.insert(base.property.type_id, (path.clone(), other));
                }
            }
        }
    }
    Ok(())
}

struct ToComplete<'a> {
    index: BTreeMap<TypeId, (Vec<&'a str>, &'a LibraryElement)>,
    memo: BTreeMap<TypeId, TypeIdentifier>,
    visiting: BTreeSet<TypeId>,
    out: Vec<(EquivalenceHash, CompleteTypeObject)>,
    /// The type identifiers of the members of the annotation types read so
    /// far, for the string width of an applied value.
    annotation_members: BTreeMap<TypeId, Vec<TypeIdentifier>>,
}

fn qualified(path: &[&str], name: &str) -> String {
    let mut out = String::new();
    for p in path {
        out.push_str(p);
        out.push_str("::");
    }
    out.push_str(name);
    out
}

fn collection_flags(shared: bool) -> CollectionElementFlag {
    let mut bits = StructMemberFlag::TRY_CONSTRUCT1;
    if shared {
        bits |= CollectionElementFlag::IS_EXTERNAL;
    }
    CollectionElementFlag(bits)
}

impl ToComplete<'_> {
    /// The TypeIdentifier of the type `id` names, reading its definition and
    /// those it depends on into `out`.
    ///
    /// zerodds-lint: recursion-depth 64 (every step goes through a type the
    /// `visiting` set has not seen, or to a plain collection that is no
    /// deeper than `depth` allows)
    fn identifier(&mut self, id: TypeId, depth: usize) -> R<TypeIdentifier> {
        if depth > MAX_DEPTH {
            return unrepresentable("types nested deeper than the conversion follows");
        }
        if id.is_none() {
            return Ok(TypeIdentifier::None);
        }
        if let TypeId::Primitive(k) = id {
            return Ok(TypeIdentifier::Primitive(primitive_complete(k)?));
        }
        if let Some(known) = self.memo.get(&id) {
            return Ok(known.clone());
        }
        if !self.visiting.insert(id) {
            return unrepresentable(
                "a type that refers to itself: a Complete TypeObject names its dependencies by hash",
            );
        }
        let element = match self.index.get(&id) {
            Some((path, element)) => (path.clone(), *element),
            None => {
                self.visiting.remove(&id);
                return Err(V1Error::Unresolved(format!(
                    "the type id {:#x} of kind {}",
                    match id {
                        TypeId::Constructed { id, .. } => id,
                        TypeId::Primitive(_) => 0,
                    },
                    id.kind()
                )));
            }
        };
        let outcome = self.build(id, &element.0, element.1, depth);
        self.visiting.remove(&id);
        let ti = outcome?;
        self.memo.insert(id, ti.clone());
        Ok(ti)
    }

    fn build(
        &mut self,
        id: TypeId,
        path: &[&str],
        element: &LibraryElement,
        depth: usize,
    ) -> R<TypeIdentifier> {
        match element {
            LibraryElement::String(s) => {
                let narrow = match s.element.element_type {
                    TypeId::Primitive(kind::CHAR_8_TYPE) => true,
                    TypeId::Primitive(kind::CHAR_32_TYPE) => false,
                    _ => {
                        return Err(V1Error::Invalid(String::from(
                            "a string whose element is neither CHAR_8_TYPE nor CHAR_32_TYPE",
                        )));
                    }
                };
                Ok(match (narrow, u8::try_from(s.bound)) {
                    (true, Ok(b)) => TypeIdentifier::String8Small { bound: b },
                    (true, Err(_)) => TypeIdentifier::String8Large { bound: s.bound },
                    (false, Ok(b)) => TypeIdentifier::String16Small { bound: b },
                    (false, Err(_)) => TypeIdentifier::String16Large { bound: s.bound },
                })
            }
            LibraryElement::Sequence(s) => {
                no_annotations(&s.base, "a sequence")?;
                let element_ti = self.identifier(s.element.element_type, depth + 1)?;
                let header = header_for(&element_ti, s.element.element_shared);
                Ok(match u8::try_from(s.bound) {
                    Ok(b) => TypeIdentifier::PlainSequenceSmall {
                        header,
                        bound: b,
                        element: alloc::boxed::Box::new(element_ti),
                    },
                    Err(_) => TypeIdentifier::PlainSequenceLarge {
                        header,
                        bound: s.bound,
                        element: alloc::boxed::Box::new(element_ti),
                    },
                })
            }
            LibraryElement::Array(a) => {
                no_annotations(&a.base, "an array")?;
                let element_ti = self.identifier(a.element.element_type, depth + 1)?;
                let header = header_for(&element_ti, a.element.element_shared);
                Ok(if a.bound.iter().all(|b| u8::try_from(*b).is_ok()) {
                    TypeIdentifier::PlainArraySmall {
                        header,
                        #[allow(clippy::cast_possible_truncation)]
                        array_bounds: a.bound.iter().map(|b| *b as u8).collect(),
                        element: alloc::boxed::Box::new(element_ti),
                    }
                } else {
                    TypeIdentifier::PlainArrayLarge {
                        header,
                        array_bounds: a.bound.clone(),
                        element: alloc::boxed::Box::new(element_ti),
                    }
                })
            }
            LibraryElement::Map(m) => {
                no_annotations(&m.base, "a map")?;
                let key_ti = self.identifier(m.key_element_type, depth + 1)?;
                let element_ti = self.identifier(m.element.element_type, depth + 1)?;
                let header = header_for(&element_ti, m.element.element_shared);
                let key_flags = CollectionElementFlag(StructMemberFlag::TRY_CONSTRUCT1);
                Ok(match u8::try_from(m.bound) {
                    Ok(b) => TypeIdentifier::PlainMapSmall {
                        header,
                        bound: b,
                        element: alloc::boxed::Box::new(element_ti),
                        key_flags,
                        key: alloc::boxed::Box::new(key_ti),
                    },
                    Err(_) => TypeIdentifier::PlainMapLarge {
                        header,
                        bound: m.bound,
                        element: alloc::boxed::Box::new(element_ti),
                        key_flags,
                        key: alloc::boxed::Box::new(key_ti),
                    },
                })
            }
            LibraryElement::Structure(s) => {
                let object = self.structure(path, s, depth)?;
                self.register(CompleteTypeObject::Struct(object))
            }
            LibraryElement::Union(u) => {
                let object = self.union(path, u, depth)?;
                self.register(CompleteTypeObject::Union(object))
            }
            LibraryElement::Enumeration(e) => {
                let object = Self::enumeration(path, e)?;
                self.register(CompleteTypeObject::Enumerated(object))
            }
            LibraryElement::BitSet(b) => {
                let object = self.bitmask(path, b, depth)?;
                self.register(CompleteTypeObject::Bitmask(object))
            }
            LibraryElement::Alias(a) => {
                let related_type = self.identifier(a.base_type, depth + 1)?;
                let detail = self.detail(path, &a.base, depth)?;
                self.register(CompleteTypeObject::Alias(CompleteAliasType {
                    alias_flags: AliasTypeFlag(0),
                    header: CompleteAliasHeader { detail },
                    body: CompleteAliasBody {
                        related_flags: AliasMemberFlag::default(),
                        related_type,
                        ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                        ann_custom: OptionalAppliedAnnotationSeq::default(),
                    },
                }))
            }
            LibraryElement::Annotation(a) => {
                let object = self.annotation_type(id, path, a, depth)?;
                self.register(CompleteTypeObject::Annotation(object))
            }
            LibraryElement::Module(_) => {
                Err(V1Error::Invalid(String::from("a module has no type id")))
            }
        }
    }

    fn register(&mut self, object: CompleteTypeObject) -> R<TypeIdentifier> {
        let hash = compute_complete_hash(&object).map_err(|e| {
            V1Error::Invalid(format!("the Complete object does not serialize: {e}"))
        })?;
        self.out.push((hash, object));
        Ok(TypeIdentifier::EquivalenceHashComplete(hash))
    }

    fn detail(&mut self, path: &[&str], base: &TypeBase, depth: usize) -> R<CompleteTypeDetail> {
        Ok(CompleteTypeDetail {
            ann_builtin: AppliedBuiltinTypeAnnotations::default(),
            ann_custom: self.applied(&base.annotation, depth)?,
            type_name: qualified(path, &base.property.name),
        })
    }

    fn member_detail(&mut self, m: &Member, depth: usize) -> R<CompleteMemberDetail> {
        Ok(CompleteMemberDetail {
            name: m.property.name.clone(),
            ann_builtin: AppliedBuiltinMemberAnnotations::default(),
            ann_custom: self.applied(&m.annotation, depth)?,
        })
    }

    fn structure(
        &mut self,
        path: &[&str],
        s: &StructureType,
        depth: usize,
    ) -> R<CompleteStructType> {
        let flags = s.base.property.flag;
        let mut bits = if flags & type_flag::IS_FINAL != 0 {
            StructTypeFlag::IS_FINAL
        } else if flags & type_flag::IS_MUTABLE != 0 {
            StructTypeFlag::IS_MUTABLE
        } else {
            StructTypeFlag::IS_APPENDABLE
        };
        if flags & type_flag::IS_NESTED != 0 {
            bits |= StructTypeFlag::IS_NESTED;
        }
        let base_type = self.identifier(s.base_type, depth + 1)?;
        let detail = self.detail(path, &s.base, depth)?;
        let mut member_seq = Vec::new();
        for m in &s.member {
            let mut member_flags = StructMemberFlag::TRY_CONSTRUCT1;
            if m.property.flag & member_flag::IS_KEY != 0 {
                // A key member is a member that must be understood
                // (XTypes 1.3 §7.2.2.4.4.4.8), which is what the objects of
                // 1.3 write for it.
                member_flags |= StructMemberFlag::IS_KEY | StructMemberFlag::IS_MUST_UNDERSTAND;
            }
            if m.property.flag & member_flag::IS_OPTIONAL != 0 {
                member_flags |= StructMemberFlag::IS_OPTIONAL;
            }
            if m.property.flag & member_flag::IS_SHAREABLE != 0 {
                member_flags |= StructMemberFlag::IS_EXTERNAL;
            }
            member_seq.push(CompleteStructMember {
                common: CommonStructMember {
                    member_id: m.property.member_id,
                    member_flags: StructMemberFlag(member_flags),
                    member_type_id: self.identifier(m.property.type_id, depth + 1)?,
                },
                detail: self.member_detail(m, depth)?,
            });
        }
        Ok(CompleteStructType {
            struct_flags: StructTypeFlag(bits),
            header: CompleteStructHeader { base_type, detail },
            member_seq,
        })
    }

    fn union(&mut self, path: &[&str], u: &UnionType, depth: usize) -> R<CompleteUnionType> {
        let Some((disc, cases)) = u.member.split_first() else {
            return Err(V1Error::Invalid(String::from(
                "a union without its discriminator, which is its first member",
            )));
        };
        let flags = u.base.property.flag;
        let mut bits = if flags & type_flag::IS_FINAL != 0 {
            StructTypeFlag::IS_FINAL
        } else if flags & type_flag::IS_MUTABLE != 0 {
            StructTypeFlag::IS_MUTABLE
        } else {
            StructTypeFlag::IS_APPENDABLE
        };
        if flags & type_flag::IS_NESTED != 0 {
            bits |= StructTypeFlag::IS_NESTED;
        }
        let detail = self.detail(path, &u.base, depth)?;
        let mut disc_flags = UnionDiscriminatorFlag::discriminator_default().0;
        if disc.member.property.flag & member_flag::IS_KEY != 0 {
            disc_flags |= UnionDiscriminatorFlag::IS_KEY;
        }
        let discriminator = CompleteDiscriminatorMember {
            common: CommonDiscriminatorMember {
                member_flags: UnionDiscriminatorFlag(disc_flags),
                type_id: self.identifier(disc.member.property.type_id, depth + 1)?,
            },
            ann_builtin: AppliedBuiltinTypeAnnotations::default(),
            ann_custom: self.applied(&disc.member.annotation, depth)?,
        };
        let mut member_seq = Vec::new();
        for c in cases {
            let mut member_flags = UnionMemberFlag::TRY_CONSTRUCT1;
            if c.member.property.flag & member_flag::IS_UNION_DEFAULT != 0 {
                member_flags |= UnionMemberFlag::IS_DEFAULT;
            }
            member_seq.push(CompleteUnionMember {
                common: CommonUnionMember {
                    member_id: c.member.property.member_id,
                    member_flags: UnionMemberFlag(member_flags),
                    type_id: self.identifier(c.member.property.type_id, depth + 1)?,
                    label_seq: c.label.clone(),
                },
                detail: self.member_detail(&c.member, depth)?,
            });
        }
        Ok(CompleteUnionType {
            union_flags: UnionTypeFlag(bits),
            header: CompleteUnionHeader { detail },
            discriminator,
            member_seq,
        })
    }

    fn enumeration(path: &[&str], e: &EnumerationType) -> R<CompleteEnumeratedType> {
        if !e.base.annotation.is_empty() {
            return unrepresentable("annotations on an enumeration (as Complete objects)");
        }
        let flags = e.base.property.flag;
        let bits = if flags & type_flag::IS_FINAL != 0 {
            EnumTypeFlag::IS_FINAL
        } else if flags & type_flag::IS_MUTABLE != 0 {
            StructTypeFlag::IS_MUTABLE
        } else {
            EnumTypeFlag::IS_APPENDABLE
        };
        let bit_bound = u16::try_from(e.bit_bound).map_err(|_| {
            V1Error::Invalid(format!("an enumeration with the bit bound {}", e.bit_bound))
        })?;
        let mut literal_seq: Vec<CompleteEnumeratedLiteral> = e
            .constant
            .iter()
            .enumerate()
            .map(|(i, c)| CompleteEnumeratedLiteral {
                common: CommonEnumeratedLiteral {
                    value: c.value,
                    flags: EnumLiteralFlag(if i == 0 {
                        EnumLiteralFlag::IS_DEFAULT_LITERAL
                    } else {
                        0
                    }),
                },
                detail: CompleteMemberDetail {
                    name: c.name.clone(),
                    ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            })
            .collect();
        literal_seq.sort_by_key(|l| l.common.value);
        Ok(CompleteEnumeratedType {
            enum_flags: EnumTypeFlag(bits),
            header: CompleteEnumeratedHeader {
                common: CommonEnumeratedHeader { bit_bound },
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                    type_name: qualified(path, &e.base.property.name),
                },
            },
            literal_seq,
        })
    }

    fn bitmask(&mut self, path: &[&str], b: &BitSetType, depth: usize) -> R<CompleteBitmaskType> {
        let bit_bound = u16::try_from(b.bit_bound).map_err(|_| {
            V1Error::Invalid(format!("a bit set with the bit bound {}", b.bit_bound))
        })?;
        let mut flag_seq = Vec::new();
        for bit in &b.bit {
            flag_seq.push(CompleteBitflag {
                common: CommonBitflag {
                    position: u16::try_from(bit.index)
                        .map_err(|_| V1Error::Invalid(format!("the bit index {}", bit.index)))?,
                    flags: BitflagFlag(0),
                },
                detail: CompleteMemberDetail {
                    name: bit.name.clone(),
                    ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            });
        }
        flag_seq.sort_by_key(|f| f.common.position);
        Ok(CompleteBitmaskType {
            bitmask_flags: BitmaskTypeFlag(0),
            bit_bound,
            detail: self.detail(path, &b.base, depth)?,
            flag_seq,
        })
    }

    fn annotation_type(
        &mut self,
        id: TypeId,
        path: &[&str],
        a: &AnnotationType,
        depth: usize,
    ) -> R<CompleteAnnotationType> {
        if !a.base_type.is_none() {
            return unrepresentable("an annotation type with a base type");
        }
        let mut member_seq = Vec::new();
        let mut types = Vec::new();
        for m in &a.member {
            let member_type_id = self.identifier(m.member.property.type_id, depth + 1)?;
            let default_value = match &m.default_value {
                Some(v) => value_complete(v, &member_type_id)?,
                None => default_of(&member_type_id)?,
            };
            types.push(member_type_id.clone());
            member_seq.push(CompleteAnnotationParameter {
                common: CommonAnnotationParameter {
                    member_flags: AnnotationParameterFlag(0),
                    member_type_id,
                },
                name: m.member.property.name.clone(),
                default_value,
            });
        }
        self.annotation_members.insert(id, types);
        Ok(CompleteAnnotationType {
            annotation_flag: AnnotationTypeFlag(0),
            header: CompleteAnnotationHeader {
                annotation_name: qualified(path, &a.base.property.name),
            },
            member_seq,
        })
    }

    /// The annotations applied to a type or a member, as Complete objects
    /// apply them.
    fn applied(
        &mut self,
        usages: &[AnnotationUsage],
        depth: usize,
    ) -> R<OptionalAppliedAnnotationSeq> {
        if usages.is_empty() {
            return Ok(OptionalAppliedAnnotationSeq::default());
        }
        let mut out = Vec::new();
        for u in usages {
            let annotation_typeid = self.identifier(u.type_id, depth + 1)?;
            let Some(TypeIdentifier::EquivalenceHashComplete(hash)) =
                self.memo.get(&u.type_id).cloned()
            else {
                return Err(V1Error::Invalid(String::from(
                    "an annotation whose type is not an annotation type of the library",
                )));
            };
            let Some(CompleteTypeObject::Annotation(annotation)) = self
                .out
                .iter()
                .find(|(h, _)| *h == hash)
                .map(|(_, o)| o.clone())
            else {
                return Err(V1Error::Invalid(String::from(
                    "an annotation whose type is not an annotation type",
                )));
            };
            let mut param_seq = Vec::new();
            for m in &u.member {
                let index = m.member_id as usize;
                let Some(parameter) = annotation.member_seq.get(index) else {
                    return Err(V1Error::Invalid(format!(
                        "member {index} of an annotation type that has {}",
                        annotation.member_seq.len()
                    )));
                };
                param_seq.push(AppliedAnnotationParameter {
                    paramname_hash: NameHash::from_name(&parameter.name),
                    value: value_complete(&m.value, &parameter.common.member_type_id)?,
                });
            }
            param_seq.sort_by_key(|p| p.paramname_hash);
            out.push(AppliedAnnotation {
                annotation_typeid,
                param_seq,
            });
        }
        Ok(OptionalAppliedAnnotationSeq(Some(out)))
    }
}

fn no_annotations(base: &TypeBase, what: &str) -> R<()> {
    if base.annotation.is_empty() {
        Ok(())
    } else {
        unrepresentable(format!("annotations on {what}"))
    }
}

/// The header of a plain collection for its element.
fn header_for(element: &TypeIdentifier, shared: bool) -> PlainCollectionHeader {
    let plain = !matches!(
        element,
        TypeIdentifier::EquivalenceHashComplete(_)
            | TypeIdentifier::EquivalenceHashMinimal(_)
            | TypeIdentifier::StronglyConnectedComponent(_)
    );
    PlainCollectionHeader {
        equiv_kind: if plain { EK_BOTH } else { EK_COMPLETE },
        element_flags: collection_flags(shared),
    }
}

fn is_narrow_string(ti: &TypeIdentifier) -> bool {
    matches!(
        ti,
        TypeIdentifier::String8Small { .. } | TypeIdentifier::String8Large { .. }
    )
}

fn value_complete(
    v: &AnnotationMemberValue,
    member_type: &TypeIdentifier,
) -> R<AnnotationParameterValue> {
    use AnnotationMemberValue as V;
    use AnnotationParameterValue as P;
    Ok(match v {
        V::Boolean(b) => P::Boolean(*b),
        V::Byte(b) => match member_type {
            TypeIdentifier::Primitive(PrimitiveKind::UInt8) => P::UInt8(*b),
            _ => P::Byte(*b),
        },
        V::Int16(x) => P::Int16(*x),
        V::UInt16(x) => P::UInt16(*x),
        V::Int32(x) => P::Int32(*x),
        V::UInt32(x) => P::UInt32(*x),
        V::Int64(x) => P::Int64(*x),
        V::UInt64(x) => P::UInt64(*x),
        V::Float32Bits(x) => P::Float32Bits(*x),
        V::Float64Bits(x) => P::Float64Bits(*x),
        V::Float128(x) => P::Float128Bits(u128::from_be_bytes(*x)),
        V::Char8(c) => P::Char8(char::from(*c)),
        V::Char32(c) => P::Char16(match char::from_u32(*c) {
            Some(ch) if *c <= 0xFFFF => ch,
            _ => {
                return unrepresentable("a CHAR_32 value outside the Basic Multilingual Plane");
            }
        }),
        V::Enumeration(x) => P::Enum(*x),
        V::String(s) => {
            if is_narrow_string(member_type) {
                P::String8(s.clone())
            } else {
                P::String16(s.clone())
            }
        }
    })
}

/// The value an annotation member takes when its default is left out: the
/// zero of its type.
fn default_of(member_type: &TypeIdentifier) -> R<AnnotationParameterValue> {
    use AnnotationParameterValue as P;
    Ok(match member_type {
        TypeIdentifier::Primitive(p) => match p {
            PrimitiveKind::Boolean => P::Boolean(false),
            PrimitiveKind::Byte => P::Byte(0),
            PrimitiveKind::UInt8 => P::UInt8(0),
            PrimitiveKind::Int8 => P::Int8(0),
            PrimitiveKind::Int16 => P::Int16(0),
            PrimitiveKind::UInt16 => P::UInt16(0),
            PrimitiveKind::Int32 => P::Int32(0),
            PrimitiveKind::UInt32 => P::UInt32(0),
            PrimitiveKind::Int64 => P::Int64(0),
            PrimitiveKind::UInt64 => P::UInt64(0),
            PrimitiveKind::Float32 => P::Float32Bits(0),
            PrimitiveKind::Float64 => P::Float64Bits(0),
            PrimitiveKind::Float128 => P::Float128Bits(0),
            PrimitiveKind::Char8 => P::Char8('\0'),
            PrimitiveKind::Char16 => P::Char16('\0'),
        },
        TypeIdentifier::String8Small { .. } | TypeIdentifier::String8Large { .. } => {
            P::String8(String::new())
        }
        TypeIdentifier::String16Small { .. } | TypeIdentifier::String16Large { .. } => {
            P::String16(String::new())
        }
        _ => P::Enum(0),
    })
}
