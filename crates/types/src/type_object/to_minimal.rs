// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Projection of a `CompleteTypeObject` onto its `MinimalTypeObject`
//! (XTypes 1.3 §7.3.4.5).
//!
//! The specification derives the two equivalence forms of a type from the
//! same model: the complete form keeps every name and annotation, the
//! minimal form keeps only what assignability is defined on — the member
//! ids, the member flags, the member type ids, and a 4-byte hash of each
//! member name. Everything the minimal form holds is therefore already
//! present in the complete form, with a single exception: a member whose
//! type is an aggregated type is named by an `EquivalenceHash`, and the
//! complete form names it by the hash of the *complete* object while the
//! minimal form names it by the hash of the *minimal* object. A hash
//! cannot be inverted, so that one substitution is the whole difficulty
//! of the projection.
//!
//! This module resolves it the only way that states nothing untrue: it
//! looks the dependency's complete object up in a [`TypeRegistry`],
//! projects that object in turn, and hashes the result. When the
//! dependency is not in the registry the projection fails and reports it,
//! rather than inventing a hash for a type it has never seen — a wrong
//! `EquivalenceHash` on the wire is a type identity a peer resolves to
//! the wrong type or to nothing at all, and both are worse than the
//! honest absence of the identifier.
//!
//! A `TI_STRONGLY_CONNECTED_COMPONENT` identifier (§7.3.4.9.3) is looked
//! up, not derived. Its hash covers a whole component in the order
//! §7.3.4.9.2 step 4b fixes, so its minimal counterpart is not a function
//! of any single object in the component; the registry files the minimal
//! component when it builds both, and an identifier of a component it does
//! not hold is reported as unprojectable.

use alloc::vec::Vec;

use crate::hash::compute_minimal_hash_with;
use crate::resolve::TypeRegistry;
use crate::type_identifier::TypeIdentifier;
use crate::type_identifier::kinds::{EK_BOTH, EK_COMPLETE, EK_MINIMAL};
use crate::type_object::common::NameHash;
use crate::type_object::complete::{
    CompleteAliasType, CompleteAnnotationType, CompleteArrayType, CompleteBitmaskType,
    CompleteBitsetType, CompleteCollectionElement, CompleteEnumeratedType, CompleteMapType,
    CompleteSequenceType, CompleteStructType, CompleteTypeObject, CompleteUnionType,
};
use crate::type_object::flags::{
    AnnotationParameterFlag, StructMemberFlag, StructTypeFlag, UnionDiscriminatorFlag,
    UnionMemberFlag, UnionTypeFlag, minimal_member_flags, minimal_type_flags,
    minimal_union_member_flags,
};
use crate::type_object::minimal::{
    CommonAliasBody, CommonAnnotationParameter, MinimalAliasBody, MinimalAliasType,
    MinimalAnnotationParameter, MinimalAnnotationType, MinimalArrayType, MinimalBitfield,
    MinimalBitflag, MinimalBitmaskType, MinimalBitsetType, MinimalCollectionElement,
    MinimalEnumeratedHeader, MinimalEnumeratedLiteral, MinimalEnumeratedType, MinimalMapType,
    MinimalSequenceType, MinimalStructHeader, MinimalStructMember, MinimalStructType,
    MinimalTypeObject, MinimalUnionMember, MinimalUnionType, ordered_by_name_hash,
};

/// Why a `CompleteTypeObject` could not be projected onto its minimal form.
///
/// The variants are kept apart because they call for different answers: an
/// unresolved dependency is fixed by registering the dependency, and a
/// strongly connected component is not fixable at all by this path.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToMinimalError {
    /// A member names an aggregated type by its complete `EquivalenceHash`,
    /// and no `CompleteTypeObject` under that hash is in the registry. The
    /// minimal hash of that dependency is therefore unknown.
    UnresolvedDependency {
        /// The complete hash that could not be resolved.
        hash: crate::EquivalenceHash,
    },
    /// A member is named by a `TI_STRONGLY_CONNECTED_COMPONENT` identifier
    /// (XTypes 1.3 §7.3.4.9.3), whose minimal counterpart is a property of
    /// the whole component and not of this object.
    StronglyConnectedComponent,
    /// A member is named by a discriminator this build does not define
    /// (`TypeIdentifier::Extended`). Its equivalence form is unknown, so it
    /// cannot be carried over.
    UnknownIdentifier {
        /// The undefined discriminator byte.
        discriminator: u8,
    },
    /// The object itself is Annex B's `default:` arm —
    /// `CompleteExtendedType`, a TypeKind this revision has no shape for.
    /// The stream can be carried and written back, but the minimal
    /// projection of §7.3.4.5 is a per-kind rule and there is none.
    ExtendedTypeKind {
        /// The TypeKind the stream carried.
        type_kind: u8,
    },
    /// The dependency chain is deeper than [`MAX_DEPENDENCY_DEPTH`], which
    /// a hash cycle among registered complete objects would produce.
    TooDeep,
    /// Hashing a projected dependency failed.
    Hash,
}

/// How far the projection follows member type ids into the registry.
///
/// A complete object may name a dependency that names it back. The pair is
/// legal input to a registry (nothing rejects it on insertion) and it would
/// make the projection recurse without end, so the descent is bounded. The
/// bound is generous against real IDL: XTypes 1.3 §7.3.4.9 turns genuine
/// recursion into a strongly connected component, which this projection
/// rejects outright, so only a hand-assembled registry reaches this depth.
pub const MAX_DEPENDENCY_DEPTH: u32 = 64;

impl CompleteTypeObject {
    /// Projects this object onto the `MinimalTypeObject` of the same type
    /// (XTypes 1.3 §7.3.4.5).
    ///
    /// `registry` supplies the complete objects of aggregated dependencies;
    /// see the module documentation for why they are needed. A type whose
    /// members are all primitives, strings, or plain collections of those
    /// projects without consulting the registry at all.
    ///
    /// # Errors
    /// [`ToMinimalError`] when a dependency cannot be resolved to its
    /// minimal hash. The projection then yields nothing rather than an
    /// object carrying an identifier it cannot justify.
    pub fn to_minimal(&self, registry: &TypeRegistry) -> Result<MinimalTypeObject, ToMinimalError> {
        self.to_minimal_at(registry, 0)
    }

    fn to_minimal_at(
        &self,
        registry: &TypeRegistry,
        depth: u32,
    ) -> Result<MinimalTypeObject, ToMinimalError> {
        if depth > MAX_DEPENDENCY_DEPTH {
            return Err(ToMinimalError::TooDeep);
        }
        Ok(match self {
            Self::Alias(a) => MinimalTypeObject::Alias(alias(a, registry, depth)?),
            Self::Annotation(a) => MinimalTypeObject::Annotation(annotation(a, registry, depth)?),
            Self::Struct(s) => MinimalTypeObject::Struct(structure(s, registry, depth)?),
            Self::Union(u) => MinimalTypeObject::Union(union(u, registry, depth)?),
            Self::Bitset(b) => MinimalTypeObject::Bitset(bitset(b)),
            Self::Sequence(s) => MinimalTypeObject::Sequence(sequence(s, registry, depth)?),
            Self::Array(a) => MinimalTypeObject::Array(array(a, registry, depth)?),
            Self::Map(m) => MinimalTypeObject::Map(map(m, registry, depth)?),
            Self::Enumerated(e) => MinimalTypeObject::Enumerated(enumerated(e)),
            Self::Bitmask(b) => MinimalTypeObject::Bitmask(bitmask(b)),
            Self::Extended { type_kind, .. } => {
                return Err(ToMinimalError::ExtendedTypeKind {
                    type_kind: *type_kind,
                });
            }
        })
    }
}

/// The minimal-form counterpart of a complete-form `TypeIdentifier`.
///
/// Everything that is *fully descriptive* in the sense of XTypes 1.3
/// §7.3.4.6.1 — the primitives, the strings, and the plain collections
/// built over them — is the same identifier in both forms and is returned
/// unchanged. A complete `EquivalenceHash` is replaced by the minimal hash
/// of the same type, obtained by projecting the registered complete object.
///
/// zerodds-lint: recursion-depth 64 (plain-collection element nesting
/// and registered dependencies; bounded by [`MAX_DEPENDENCY_DEPTH`],
/// which returns [`ToMinimalError::TooDeep`])
pub(crate) fn minimal_identifier(
    id: &TypeIdentifier,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<TypeIdentifier, ToMinimalError> {
    if depth > MAX_DEPENDENCY_DEPTH {
        return Err(ToMinimalError::TooDeep);
    }
    Ok(match id {
        TypeIdentifier::None
        | TypeIdentifier::Primitive(_)
        | TypeIdentifier::String8Small { .. }
        | TypeIdentifier::String8Large { .. }
        | TypeIdentifier::String16Small { .. }
        | TypeIdentifier::String16Large { .. }
        // A minimal hash inside a complete object is already the form this
        // function produces; carrying it over unchanged is the identity.
        | TypeIdentifier::EquivalenceHashMinimal(_) => id.clone(),

        TypeIdentifier::PlainSequenceSmall {
            header,
            bound,
            element,
        } => {
            let element = minimal_identifier(element, registry, depth + 1)?;
            TypeIdentifier::PlainSequenceSmall {
                header: crate::type_identifier::PlainCollectionHeader {
                    equiv_kind: element_equiv_kind(header.equiv_kind, &element),
                    element_flags: header.element_flags,
                },
                bound: *bound,
                element: alloc::boxed::Box::new(element),
            }
        }
        TypeIdentifier::PlainSequenceLarge {
            header,
            bound,
            element,
        } => {
            let element = minimal_identifier(element, registry, depth + 1)?;
            TypeIdentifier::PlainSequenceLarge {
                header: crate::type_identifier::PlainCollectionHeader {
                    equiv_kind: element_equiv_kind(header.equiv_kind, &element),
                    element_flags: header.element_flags,
                },
                bound: *bound,
                element: alloc::boxed::Box::new(element),
            }
        }
        TypeIdentifier::PlainArraySmall {
            header,
            array_bounds,
            element,
        } => {
            let element = minimal_identifier(element, registry, depth + 1)?;
            TypeIdentifier::PlainArraySmall {
                header: crate::type_identifier::PlainCollectionHeader {
                    equiv_kind: element_equiv_kind(header.equiv_kind, &element),
                    element_flags: header.element_flags,
                },
                array_bounds: array_bounds.clone(),
                element: alloc::boxed::Box::new(element),
            }
        }
        TypeIdentifier::PlainArrayLarge {
            header,
            array_bounds,
            element,
        } => {
            let element = minimal_identifier(element, registry, depth + 1)?;
            TypeIdentifier::PlainArrayLarge {
                header: crate::type_identifier::PlainCollectionHeader {
                    equiv_kind: element_equiv_kind(header.equiv_kind, &element),
                    element_flags: header.element_flags,
                },
                array_bounds: array_bounds.clone(),
                element: alloc::boxed::Box::new(element),
            }
        }
        TypeIdentifier::PlainMapSmall {
            header,
            bound,
            element,
            key_flags,
            key,
        } => {
            let element = minimal_identifier(element, registry, depth + 1)?;
            let key = minimal_identifier(key, registry, depth + 1)?;
            TypeIdentifier::PlainMapSmall {
                header: crate::type_identifier::PlainCollectionHeader {
                    equiv_kind: element_equiv_kind(header.equiv_kind, &element),
                    element_flags: header.element_flags,
                },
                bound: *bound,
                element: alloc::boxed::Box::new(element),
                key_flags: *key_flags,
                key: alloc::boxed::Box::new(key),
            }
        }
        TypeIdentifier::PlainMapLarge {
            header,
            bound,
            element,
            key_flags,
            key,
        } => {
            let element = minimal_identifier(element, registry, depth + 1)?;
            let key = minimal_identifier(key, registry, depth + 1)?;
            TypeIdentifier::PlainMapLarge {
                header: crate::type_identifier::PlainCollectionHeader {
                    equiv_kind: element_equiv_kind(header.equiv_kind, &element),
                    element_flags: header.element_flags,
                },
                bound: *bound,
                element: alloc::boxed::Box::new(element),
                key_flags: *key_flags,
                key: alloc::boxed::Box::new(key),
            }
        }

        TypeIdentifier::EquivalenceHashComplete(hash) => {
            let complete = registry
                .get_complete(hash)
                .ok_or(ToMinimalError::UnresolvedDependency { hash: *hash })?;
            // A collection with an annotated element is a non-plain type
            // only in the COMPLETE form (§7.3.4.1); the MINIMAL form keeps
            // "only the information relevant to assignability" and the
            // collection is the plain type it is without the annotation.
            if let Some(plain) = plain_collection(complete, registry, depth)? {
                return Ok(plain);
            }
            let minimal = complete.to_minimal_at(registry, depth + 1)?;
            let minimal_hash =
                compute_minimal_hash_with(&minimal, registry.minimal_member_trailing_octet())
                    .map_err(|_| ToMinimalError::Hash)?;
            TypeIdentifier::EquivalenceHashMinimal(minimal_hash)
        }
        TypeIdentifier::StronglyConnectedComponent(scc) => {
            // A MINIMAL component identifier is already in this form. A
            // COMPLETE one names a component whose MINIMAL counterpart the
            // registry filed when it built both (`insert_minimal_component`):
            // the minimal hash covers the minimal members and is not a
            // function of any one complete object, which is why this
            // projection cannot derive it and has to look it up.
            if scc.equivalence_kind != EK_COMPLETE {
                return Ok(id.clone());
            }
            let Some(minimal_hash) = registry.minimal_component_hash(&scc.hash) else {
                return Err(ToMinimalError::StronglyConnectedComponent);
            };
            TypeIdentifier::StronglyConnectedComponent(
                crate::type_identifier::StronglyConnectedComponentId {
                    equivalence_kind: EK_MINIMAL,
                    hash: minimal_hash,
                    scc_length: scc.scc_length,
                    scc_index: scc.scc_index,
                },
            )
        }
        TypeIdentifier::Extended { discriminator, .. } => {
            return Err(ToMinimalError::UnknownIdentifier {
                discriminator: *discriminator,
            });
        }
    })
}

/// The plain `TypeIdentifier` of a complete collection object, in its
/// minimal form, or `None` when the object is not a collection.
///
/// The inverse of what makes a collection non-plain: the object carries
/// the element's annotations in `ann_custom`, and the plain identifier
/// has no place for them. Everything else — bound or dimensions, the
/// element's and the key's identifiers and flags — is carried over, the
/// element and key projected to their minimal form.
///
/// zerodds-lint: recursion-depth 64 (through `minimal_identifier`, which
/// returns [`ToMinimalError::TooDeep`] past [`MAX_DEPENDENCY_DEPTH`])
fn plain_collection(
    object: &CompleteTypeObject,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<Option<TypeIdentifier>, ToMinimalError> {
    use crate::type_identifier::{CollectionElementFlag, PlainCollectionHeader};
    use alloc::boxed::Box;
    // The object's flags and the identifier's are two types over the same
    // sixteen bits.
    let flags = |written: crate::type_object::flags::CollectionElementFlag| {
        CollectionElementFlag(written.0)
    };
    let header = |element: &TypeIdentifier, written| PlainCollectionHeader {
        equiv_kind: element_equiv_kind(EK_COMPLETE, element),
        element_flags: flags(written),
    };
    Ok(Some(match object {
        CompleteTypeObject::Sequence(s) => {
            let element = minimal_identifier(&s.element.common.type_id, registry, depth + 1)?;
            let header = header(&element, s.element.common.element_flags);
            match u8::try_from(s.bound) {
                Ok(small) => TypeIdentifier::PlainSequenceSmall {
                    header,
                    bound: small,
                    element: Box::new(element),
                },
                Err(_) => TypeIdentifier::PlainSequenceLarge {
                    header,
                    bound: s.bound,
                    element: Box::new(element),
                },
            }
        }
        CompleteTypeObject::Array(a) => {
            let element = minimal_identifier(&a.element.common.type_id, registry, depth + 1)?;
            let header = header(&element, a.element.common.element_flags);
            let small: Option<Vec<u8>> = a
                .bound_seq
                .iter()
                .map(|bound| u8::try_from(*bound).ok())
                .collect();
            match small {
                Some(array_bounds) => TypeIdentifier::PlainArraySmall {
                    header,
                    array_bounds,
                    element: Box::new(element),
                },
                None => TypeIdentifier::PlainArrayLarge {
                    header,
                    array_bounds: a.bound_seq.clone(),
                    element: Box::new(element),
                },
            }
        }
        CompleteTypeObject::Map(m) => {
            let element = minimal_identifier(&m.element.common.type_id, registry, depth + 1)?;
            let key = minimal_identifier(&m.key.common.type_id, registry, depth + 1)?;
            let header = header(&element, m.element.common.element_flags);
            let key_flags = flags(m.key.common.element_flags);
            match u8::try_from(m.bound) {
                Ok(small) => TypeIdentifier::PlainMapSmall {
                    header,
                    bound: small,
                    element: Box::new(element),
                    key_flags,
                    key: Box::new(key),
                },
                Err(_) => TypeIdentifier::PlainMapLarge {
                    header,
                    bound: m.bound,
                    element: Box::new(element),
                    key_flags,
                    key: Box::new(key),
                },
            }
        }
        _ => return Ok(None),
    }))
}

/// The `equiv_kind` a plain collection header carries once its element has
/// been projected.
///
/// §7.3.4.6.2 lets the header say `EK_BOTH` when the element identifier is
/// the same in both forms, and names the element's own equivalence kind
/// otherwise. A complete element becomes a minimal one under this
/// projection, so an `EK_COMPLETE` header becomes `EK_MINIMAL`; a header
/// that already said `EK_BOTH` is left alone, because a fully descriptive
/// element is unchanged.
fn element_equiv_kind(source: u8, projected: &TypeIdentifier) -> u8 {
    match projected {
        TypeIdentifier::EquivalenceHashMinimal(_) => EK_MINIMAL,
        _ if source == EK_COMPLETE => EK_BOTH,
        _ => source,
    }
}

fn alias(
    a: &CompleteAliasType,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<MinimalAliasType, ToMinimalError> {
    Ok(MinimalAliasType {
        alias_flags: a.alias_flags,
        body: MinimalAliasBody {
            common: CommonAliasBody {
                related_flags: a.body.related_flags,
                related_type: minimal_identifier(&a.body.related_type, registry, depth + 1)?,
            },
        },
    })
}

fn annotation(
    a: &CompleteAnnotationType,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<MinimalAnnotationType, ToMinimalError> {
    let mut member_seq = Vec::with_capacity(a.member_seq.len());
    for p in &a.member_seq {
        member_seq.push(MinimalAnnotationParameter {
            common: CommonAnnotationParameter {
                member_flags: AnnotationParameterFlag(minimal_member_flags(
                    p.common.member_flags.0,
                )),
                member_type_id: minimal_identifier(&p.common.member_type_id, registry, depth + 1)?,
            },
            // Annex B: `MinimalAnnotationParameter` names its parameter by
            // `NameHash`, where the complete one has the `MemberName`.
            name_hash: NameHash::from_name(&p.name),
            default_value: p.default_value.clone(),
        });
    }
    Ok(MinimalAnnotationType {
        annotation_flag: a.annotation_flag,
        // §7.3.4.5 orders the minimal sequence by name hash and the
        // complete one by name, so the two orders are not the same
        // sequence: this projection has to sort for itself.
        member_seq: ordered_by_name_hash(member_seq),
    })
}

fn structure(
    s: &CompleteStructType,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<MinimalStructType, ToMinimalError> {
    let mut member_seq = Vec::with_capacity(s.member_seq.len());
    for m in &s.member_seq {
        let mut common = m.common.clone();
        common.member_type_id = minimal_identifier(&common.member_type_id, registry, depth + 1)?;
        common.member_flags = StructMemberFlag(minimal_member_flags(common.member_flags.0));
        member_seq.push(MinimalStructMember {
            common,
            detail: NameHash::from_name(&m.detail.name),
        });
    }
    Ok(MinimalStructType {
        struct_flags: StructTypeFlag(minimal_type_flags(s.struct_flags.0)),
        header: MinimalStructHeader {
            base_type: minimal_identifier(&s.header.base_type, registry, depth + 1)?,
        },
        member_seq,
    })
}

fn union(
    u: &CompleteUnionType,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<MinimalUnionType, ToMinimalError> {
    let mut discriminator = u.discriminator.common.clone();
    discriminator.type_id = minimal_identifier(&discriminator.type_id, registry, depth + 1)?;
    discriminator.member_flags =
        UnionDiscriminatorFlag(minimal_member_flags(discriminator.member_flags.0));
    let mut member_seq = Vec::with_capacity(u.member_seq.len());
    for m in &u.member_seq {
        let mut common = m.common.clone();
        common.type_id = minimal_identifier(&common.type_id, registry, depth + 1)?;
        common.member_flags = UnionMemberFlag(minimal_union_member_flags(common.member_flags.0));
        member_seq.push(MinimalUnionMember {
            common,
            detail: NameHash::from_name(&m.detail.name),
        });
    }
    Ok(MinimalUnionType {
        union_flags: UnionTypeFlag(minimal_type_flags(u.union_flags.0)),
        discriminator: crate::type_object::minimal::MinimalDiscriminatorMember {
            common: discriminator,
        },
        member_seq,
    })
}

/// A bitset holds bit fields, not typed members, so nothing in it names
/// another type and the registry is not consulted.
fn bitset(b: &CompleteBitsetType) -> MinimalBitsetType {
    MinimalBitsetType {
        bitset_flags: b.bitset_flags,
        field_seq: b
            .field_seq
            .iter()
            .map(|f| MinimalBitfield {
                common: f.common,
                name_hash: NameHash::from_name(&f.detail.name),
            })
            .collect(),
    }
}

fn sequence(
    s: &CompleteSequenceType,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<MinimalSequenceType, ToMinimalError> {
    Ok(MinimalSequenceType {
        collection_flag: s.collection_flag,
        bound: s.bound,
        element: collection_element(&s.element, registry, depth)?,
    })
}

fn array(
    a: &CompleteArrayType,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<MinimalArrayType, ToMinimalError> {
    Ok(MinimalArrayType {
        collection_flag: a.collection_flag,
        bound_seq: a.bound_seq.clone(),
        element: collection_element(&a.element, registry, depth)?,
    })
}

fn map(
    m: &CompleteMapType,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<MinimalMapType, ToMinimalError> {
    Ok(MinimalMapType {
        collection_flag: m.collection_flag,
        bound: m.bound,
        key: collection_element(&m.key, registry, depth)?,
        element: collection_element(&m.element, registry, depth)?,
    })
}

fn collection_element(
    e: &CompleteCollectionElement,
    registry: &TypeRegistry,
    depth: u32,
) -> Result<MinimalCollectionElement, ToMinimalError> {
    let mut common = e.common.clone();
    common.type_id = minimal_identifier(&common.type_id, registry, depth + 1)?;
    Ok(MinimalCollectionElement { common })
}

/// An enumeration's literals are values and names, never type references,
/// so the projection is name hashing alone.
fn enumerated(e: &CompleteEnumeratedType) -> MinimalEnumeratedType {
    MinimalEnumeratedType {
        enum_flags: e.enum_flags,
        header: MinimalEnumeratedHeader {
            common: e.header.common,
        },
        literal_seq: e
            .literal_seq
            .iter()
            .map(|l| MinimalEnumeratedLiteral {
                common: l.common,
                detail: NameHash::from_name(&l.detail.name),
            })
            .collect(),
    }
}

/// A bitmask's flags are bit positions and names; see [`enumerated`].
fn bitmask(b: &CompleteBitmaskType) -> MinimalBitmaskType {
    MinimalBitmaskType {
        bitmask_flags: b.bitmask_flags,
        bit_bound: b.bit_bound,
        flag_seq: b
            .flag_seq
            .iter()
            .map(|f| MinimalBitflag {
                common: f.common,
                detail: NameHash::from_name(&f.detail.name),
            })
            .collect(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::from_ast::{complete_type_object, minimal_type_object, register_all};
    use zerodds_ast::{Ast, TypeNode};
    use zerodds_ast_core::Id;

    fn model(idl: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "to_minimal.idl");
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let mut ast = lowered.ast;
        let report = ast.resolve_all_by_name().expect("the resolve pass");
        assert!(report.is_clean(), "{:?}", report.broken);
        ast
    }

    fn type_id(ast: &Ast, name: &str) -> Id<TypeNode> {
        ast.types()
            .find(|(_, node)| node.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("`{name}` is not in the model"))
    }

    /// The projection has to agree with the builder that derives both
    /// forms from the same model: projecting the complete object must
    /// yield exactly the object `minimal_type_object` builds. Anything
    /// else would put a minimal hash on the wire that no peer computing
    /// it from the same IDL arrives at.
    fn agrees_with_builder(idl: &str, name: &str) {
        let ast = model(idl);
        let id = type_id(&ast, name);
        let mut registry = TypeRegistry::default();
        register_all(&ast, &mut registry).expect("the model registers");
        let complete = complete_type_object(&ast, id).expect("a complete object");
        let expected = minimal_type_object(&ast, id).expect("a minimal object");
        let projected = complete
            .to_minimal(&registry)
            .expect("the projection resolves every dependency");
        assert_eq!(projected, expected, "projection of {name}");
    }

    #[test]
    fn a_flat_struct_projects_without_the_registry() {
        let ast = model("struct Robot { unsigned long id; unsigned long label; };");
        let id = type_id(&ast, "Robot");
        let complete = complete_type_object(&ast, id).expect("a complete object");
        let projected = complete
            .to_minimal(&TypeRegistry::default())
            .expect("a struct of primitives needs no dependency");
        assert_eq!(projected, minimal_type_object(&ast, id).expect("minimal"));
    }

    #[test]
    fn an_unregistered_dependency_is_reported_not_guessed() {
        let ast = model("struct Inner { unsigned long a; }; struct Outer { Inner nested; };");
        let id = type_id(&ast, "Outer");
        let complete = complete_type_object(&ast, id).expect("a complete object");
        let error = complete
            .to_minimal(&TypeRegistry::default())
            .expect_err("Inner is not in the empty registry");
        assert!(
            matches!(error, ToMinimalError::UnresolvedDependency { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn a_nested_struct_projects_through_the_registry() {
        agrees_with_builder(
            "struct Inner { unsigned long a; }; \
             struct Outer { Inner nested; unsigned long b; };",
            "Outer",
        );
    }

    #[test]
    fn an_enumeration_and_a_union_project() {
        agrees_with_builder(
            "enum Colour { RED, GREEN }; \
             union Choice switch (long) { case 0: unsigned long a; case 1: Colour c; };",
            "Choice",
        );
    }

    #[test]
    fn a_sequence_member_projects() {
        agrees_with_builder(
            "struct Inner { unsigned long a; }; \
             struct Holder { sequence<Inner> items; sequence<unsigned long> plain; };",
            "Holder",
        );
    }

    #[test]
    fn an_appendable_struct_keeps_its_extensibility_bits() {
        agrees_with_builder(
            "@appendable struct Robot { unsigned long id; unsigned long label; };",
            "Robot",
        );
    }
}
