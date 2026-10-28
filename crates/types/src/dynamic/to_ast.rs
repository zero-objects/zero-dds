// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A peer's `TypeObject`, back into the model. XTypes 1.3 §7.6.3.
//!
//! The inverse of [`crate::from_ast`], and the half of the old
//! `bridge.rs` that has no counterpart anywhere else in this crate.
//! `from_ast` answers "what do I tell a peer about this type"; this
//! answers "what is the type this peer just told me about", and the
//! answer is a [`zerodds_ast::Ast`] — the same model an IDL file lowers
//! to, so a type that arrived over discovery is read by exactly the code
//! that reads a type that was compiled in.
//!
//! ## Why this is the proof that matters
//!
//! A reflective codec measured against types it built itself is a codec
//! asking its own encoder to agree with its own decoder. Cyclone, RTI,
//! Fast DDS and OpenDDS all publish a `CompleteTypeObject` over the
//! TypeLookup service (§7.6.3.3), and `crate::type_lookup` is what
//! fetches one. A `DynamicType` built here out of *that* object, decoding
//! the bytes *that* peer wrote, is a measurement neither side of this
//! tree could have arranged.
//!
//! ## What it refuses, and why each refusal is loud
//!
//! - **A `MinimalTypeObject`.** §7.3.4.1 replaces every name with a
//!   four-byte hash, and a hash is not invertible. A `DynamicType` built
//!   from one would have to invent every member name it then reports.
//!   §7.6.3.2.2 says the complete form is the one that "provides all the
//!   information needed to … construct a `DynamicType`".
//! - **A `TypeIdentifier` the registry cannot resolve.** A member whose
//!   type is unknown moves every member after it, so a placeholder would
//!   put the failure at the last member rather than at the one that
//!   caused it.
//! - **A bitset whose fields overlap.** IDL 4.2 §7.4.13.4.3.2 packs
//!   bitfields consecutively; a `CommonBitfield::position` that is behind
//!   the running position describes a packing no declaration produces.
//!
//! ## What is reconstructed and what is lost
//!
//! A `CompleteTypeObject` does not carry everything a declaration said.
//! What comes back is what §7.3.4.5 writes down: names, member ids and
//! their flags, extensibility, `@nested`, `@autoid(HASH)`, bit bounds,
//! enumerator values, bit positions, bitfield widths and holders, case
//! labels, bounds and dimensions, and the five builtin member
//! annotations of `AppliedBuiltinMemberAnnotations` — `@unit`, `@min`,
//! `@max`, the `@hashid` source name and the `@default` text
//! (`tests/type_object_annotations.rs` is the measurement).
//!
//! **Custom annotations come back too.** §7.3.4.5's
//! `@optional AppliedAnnotationSeq ann_custom` is read on a
//! `CompleteTypeDetail`, a `CompleteMemberDetail`, a
//! `CompleteDiscriminatorMember`, a `CompleteCollectionElement`, a
//! `CompleteAliasBody`, an enumeration literal, a bitmask flag and a
//! bitfield, and each application lands in
//! `Facets::annotations` — the list `zerodds_idl_lower` writes for the
//! same annotation in IDL
//! (`tests/type_object_custom_annotations.rs` is the measurement).
//! A `CompleteCollectionElement`'s comes back onto the collection's
//! `XTypesFacet::element_annotations` (a map key's onto
//! `key_annotations`), and a `CompleteDiscriminatorMember`'s onto the
//! union's `discriminator_annotations`.
//!
//! One thing does not survive, and it is the specification's shape rather
//! than a gap: `AppliedAnnotation::annotation_typeid` is a hash and
//! nothing else, so the *declaration* an application names is looked up
//! in the registry and rebuilt from the `CompleteAnnotationType` it holds
//! (`annotation_declaration`). That object carries the qualified name and
//! each parameter's name and default, so the declaration comes back
//! whole; what does not come back is the **application's written form** —
//! §7.3.4.5 has one encoding for `@Unit("m")` and
//! `@Unit(symbol = "m")` alike, and what comes back is always the named
//! form, because a parameter is named by its hash and the two forms write
//! the same hash.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_float128::Float128;

use zerodds_ast::{
    AnnotationDefNode, AppliedAnnotation as AstAppliedAnnotation, AppliedArgument, Ast, AutoId,
    Bound, CaseLabel, CharWidth, ConstExpr, Extensibility, Literal, MemberIdAssignment,
    MemberIdOrigin, MemberSpec, ModuleNode, PrimitiveKind as AstPrimitive, RangeConstraint,
    TryConstruct, TypeKind as AstKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

use crate::resolve::TypeRegistry;
use crate::type_identifier::{
    CollectionElementFlag, EquivalenceHash, PrimitiveKind, TypeIdentifier,
};
use crate::type_object::common::{
    AnnotationParameterValue, CompleteMemberDetail, CompleteTypeDetail, NameHash,
    OptionalAppliedAnnotationSeq,
};
use crate::type_object::complete::{
    CompleteArrayType, CompleteBitmaskType, CompleteBitsetType, CompleteEnumeratedType,
    CompleteMapType, CompleteSequenceType, CompleteStructType, CompleteUnionType,
};
use crate::type_object::flags::{
    EnumLiteralFlag, StructMemberFlag, StructTypeFlag, UnionDiscriminatorFlag, UnionMemberFlag,
};
use crate::type_object::{CompleteTypeObject, TypeObject};

use super::error::DynamicError;
use super::type_::DynamicType;

/// How deep a chain of `TypeObject`s is followed before it is taken to be
/// a cycle.
///
/// The stack of hashes below is what actually detects a cycle; this stops
/// a registry whose types are merely deeply nested from using the stack
/// up. The same bound `zerodds_wire_codec::default` uses.
const MAX_DEPTH: usize = 64;

/// A model built out of one or more peers' `TypeObject`s.
///
/// Several objects go into **one** model on purpose: two types that name
/// the same `sequence<long>` must reach the same node, or a caller
/// comparing them gets two types that are equal and not identical. The
/// cache below is what makes that so.
pub struct ModelFromTypeObjects<'a> {
    registry: &'a TypeRegistry,
    ast: Ast,
    /// Every identifier already materialised, and the node it became.
    ///
    /// A `Vec` and not a map because [`TypeIdentifier`] is neither `Ord`
    /// nor `Hash`, and a lookup over the handful of types one topic names
    /// is not where the time goes.
    cache: Vec<(TypeIdentifier, Id<TypeNode>)>,
    /// Every annotation declaration already rebuilt, by the hash its
    /// `AppliedAnnotation::annotation_typeid` named.
    ///
    /// §7.3.4.5 gives an applied annotation the hash of its declaration's
    /// `TypeObject` and nothing else, so the hash is all that two
    /// applications of one annotation have in common. Two `@Unit`
    /// applications on two members of the same structure are one
    /// declaration in the model and one node here — a second
    /// `AnnotationDefNode` would make `Ref::Resolved` on the two point
    /// at declarations that are equal in every field and not the same
    /// node, which is what `Ast::find_by_scoped_name` reports as
    /// ambiguous.
    ///
    /// The third field is the declaration's parameter names in the order
    /// `CompleteAnnotationParameterSeq` holds them, which is what an
    /// `AppliedAnnotationParameter`'s `paramname_hash` is looked up in.
    annotations: Vec<(EquivalenceHash, Id<AnnotationDefNode>, Vec<String>)>,
    /// The hashes currently being built, innermost last.
    stack: Vec<EquivalenceHash>,
    /// Every module already created, by its path.
    modules: BTreeMap<String, Id<ModuleNode>>,
}

impl<'a> ModelFromTypeObjects<'a> {
    /// An empty model, resolving dependent identifiers against
    /// `registry`.
    #[must_use]
    pub fn new(registry: &'a TypeRegistry) -> Self {
        Self {
            registry,
            ast: Ast::new(),
            cache: Vec::new(),
            annotations: Vec::new(),
            stack: Vec::new(),
            modules: BTreeMap::new(),
        }
    }

    /// Materialise one `CompleteTypeObject` and everything it names.
    ///
    /// # Errors
    /// [`DynamicError`], naming the type or the member that stopped it.
    pub fn add(&mut self, object: &CompleteTypeObject) -> Result<Id<TypeNode>, DynamicError> {
        self.object(object, 0, None)
    }

    /// Materialise a `TypeObject`, refusing the minimal form.
    ///
    /// # Errors
    /// [`DynamicError::MinimalHasNoNames`] for a minimal object; other
    /// variants as [`Self::add`].
    pub fn add_type_object(&mut self, object: &TypeObject) -> Result<Id<TypeNode>, DynamicError> {
        match object {
            TypeObject::Complete(complete) => self.add(complete),
            TypeObject::Minimal(_) => Err(DynamicError::MinimalHasNoNames {
                at: String::from("the type object offered"),
            }),
        }
    }

    /// The finished model.
    ///
    /// Shared, because that is what a [`DynamicType`] holds. Nothing more
    /// can be added afterwards, which is what makes every handle handed
    /// out before this call still valid: the arenas are append-only and
    /// the ids are stable.
    #[must_use]
    pub fn finish(self) -> Arc<Ast> {
        Arc::new(self.ast)
    }

    /// The finished model, owned: for a caller that goes on building in
    /// it (`DynamicTypeBuilderFactory::create_type_w_type_object`).
    #[must_use]
    pub fn into_model(self) -> Ast {
        self.ast
    }

    /// The model as it stands, borrowed.
    #[must_use]
    pub const fn model(&self) -> &Ast {
        &self.ast
    }

    // -- types ---------------------------------------------------------

    /// `announce` is the identifier the node is cached under **before**
    /// its members are built.
    ///
    /// Only a strongly connected component needs it, and needs it: a
    /// member pointing back into the component is followed while the
    /// component member is half-built, so the entry has to exist by
    /// then. Every other type is cached by `identifier` once it is
    /// finished, which is where a cycle would otherwise be a hang.
    fn object(
        &mut self,
        object: &CompleteTypeObject,
        depth: usize,
        announce: Option<&TypeIdentifier>,
    ) -> Result<Id<TypeNode>, DynamicError> {
        if depth > MAX_DEPTH {
            return Err(DynamicError::Recursive {
                what: qualified_name(object).to_string(),
            });
        }
        match object {
            CompleteTypeObject::Struct(node) => self.structure(node, depth, announce),
            CompleteTypeObject::Union(node) => self.union(node, depth, announce),
            CompleteTypeObject::Enumerated(node) => self.enumeration(node),
            CompleteTypeObject::Bitmask(node) => self.bitmask(node),
            CompleteTypeObject::Bitset(node) => self.bitset(node, depth),
            // Annex B's `default:` arm. The octets survive a decode and
            // an encode, but a model needs the shape of the type and a
            // `CompleteExtendedType` carries none.
            CompleteTypeObject::Extended { type_kind, .. } => Err(DynamicError::Unrepresentable {
                what: alloc::format!("TypeKind 0x{type_kind:02x}"),
                kind: "a TypeKind this build has no shape for",
            }),
            CompleteTypeObject::Alias(node) => {
                let name = node.header.detail.type_name.clone();
                let target = self.identifier(&node.body.related_type, &name, depth + 1)?;
                let id = self.declare(
                    &name,
                    AstKind::Alias {
                        aliased: Ref::Resolved(target),
                    },
                )?;
                // An alias has two `ann_custom` slots in Annex B — the
                // type's in `CompleteAliasHeader::detail`, and
                // §7.3.1.2.1.2's `@external` in `CompleteAliasBody` — and
                // the model has one list, on the typedef's node. Both are
                // read there.
                let mut applied = self.apply_type_annotations(&node.header.detail, &name, depth)?;
                self.apply_type_verbatim(id, &node.header.detail)?;
                self.apply_applied_annotations(
                    &node.body.ann_custom,
                    &mut applied,
                    depth,
                    &format!("{name} (body)"),
                )?;
                if !applied.is_empty() {
                    if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
                        facets.annotations.extend(applied);
                    }
                }
                Ok(id)
            }
            CompleteTypeObject::Sequence(node) => self.named_sequence(node, depth),
            CompleteTypeObject::Array(node) => self.named_array(node, depth),
            CompleteTypeObject::Map(node) => self.named_map(node, depth),
            // §7.3.4.4 carries an annotation declaration as a type object
            // and the model carries it as a declaration with a node of
            // its own — `annotation_declaration` builds one, for an
            // `AppliedAnnotation` that names it. This arm is the *entry*
            // point: an annotation declaration offered as the type a peer
            // published. Nothing sends one, an annotation is applied to a
            // type and is not itself a topic type, and the model has no
            // `TypeNode` for a declaration to be (`zerodds_idl_lower`
            // resolves `@Name` to the declaration precisely so that it
            // does not), so it is refused by name.
            CompleteTypeObject::Annotation(node) => Err(DynamicError::Unrepresentable {
                what: node.header.annotation_name.clone(),
                kind: "an annotation declaration offered as the type to rebuild",
            }),
        }
    }

    /// Create a named type in the module its qualified name puts it in.
    fn declare(&mut self, qualified: &str, kind: AstKind) -> Result<Id<TypeNode>, DynamicError> {
        Ok(self.declare_once(qualified, kind)?.0)
    }

    /// As [`Self::declare`], and says whether the type was already there.
    ///
    /// A qualified name denotes one type. Two nodes under one name are
    /// indistinguishable to everything downstream — `find_by_scoped_name`
    /// answers `Ambiguous` and a peer asking for the type gets neither.
    ///
    /// This matters for a strongly connected component and only for one:
    /// the entry point builds a component member, and the member that
    /// closes the cycle arrives at the same type from the other side. The
    /// caller uses the flag to stop rather than fill the aggregate twice.
    fn declare_once(
        &mut self,
        qualified: &str,
        kind: AstKind,
    ) -> Result<(Id<TypeNode>, bool), DynamicError> {
        let (module, name) = self.module_for(qualified)?;
        if let Ok(existing) = self
            .ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(
                qualified
                    .split("::")
                    .filter(|segment| !segment.is_empty())
                    .map(alloc::string::ToString::to_string)
                    .collect::<Vec<_>>(),
            ))
        {
            if let Some(id) = existing.as_type() {
                return Ok((id, true));
            }
        }
        self.ast
            .add_type(module, &name, kind)
            .map(|id| (id, false))
            .map_err(|error| DynamicError::model(&error))
    }

    /// The module a qualified name puts a type in, and the simple name.
    ///
    /// `a::b::C` is the type `C` in the module `a::b`. The separator is
    /// `::` and not `.`: §7.3.4.5.4's `QualifiedTypeName` is IDL's
    /// scoped name, and `crate::from_ast::qualified_name` writes it that
    /// way.
    fn module_for(&mut self, qualified: &str) -> Result<(Id<ModuleNode>, String), DynamicError> {
        let mut segments: Vec<&str> = qualified
            .split("::")
            .filter(|segment| !segment.is_empty())
            .collect();
        let name = segments
            .pop()
            .ok_or_else(|| {
                DynamicError::inconsistent(format!(
                    "a TypeObject whose qualified name is `{qualified}`, which names nothing"
                ))
            })?
            .to_string();
        let mut module = self.ast.root_module();
        let mut path = String::new();
        for segment in segments {
            path.push_str("::");
            path.push_str(segment);
            module = match self.modules.get(&path) {
                Some(known) => *known,
                None => {
                    let created = self
                        .ast
                        .ensure_module(module, segment)
                        .map_err(|error| DynamicError::model(&error))?;
                    self.modules.insert(path.clone(), created);
                    created
                }
            };
        }
        Ok((module, name))
    }

    fn cached(&self, identifier: &TypeIdentifier) -> Option<Id<TypeNode>> {
        self.cache
            .iter()
            .find(|(known, _)| known == identifier)
            .map(|(_, id)| *id)
    }

    /// The node one `TypeIdentifier` stands for, materialising it once.
    fn identifier(
        &mut self,
        identifier: &TypeIdentifier,
        at: &str,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        if let Some(known) = self.cached(identifier) {
            return Ok(known);
        }
        if depth > MAX_DEPTH {
            return Err(DynamicError::Recursive {
                what: at.to_string(),
            });
        }
        let id = self.materialise(identifier, at, depth)?;
        // A component member announced itself before its members were
        // built, so it is already here; pushing again would leave two
        // entries for one identifier.
        if self.cached(identifier).is_none() {
            self.cache.push((identifier.clone(), id));
        }
        Ok(id)
    }

    fn materialise(
        &mut self,
        identifier: &TypeIdentifier,
        at: &str,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        match identifier {
            TypeIdentifier::Primitive(kind) => self.primitive(*kind, at),
            TypeIdentifier::String8Small { bound } => {
                self.string(CharWidth::Char, u32::from(*bound), at)
            }
            TypeIdentifier::String8Large { bound } => self.string(CharWidth::Char, *bound, at),
            TypeIdentifier::String16Small { bound } => {
                self.string(CharWidth::WChar, u32::from(*bound), at)
            }
            TypeIdentifier::String16Large { bound } => self.string(CharWidth::WChar, *bound, at),
            TypeIdentifier::PlainSequenceSmall {
                header,
                bound,
                element,
            } => self.sequence(element, header.element_flags, u32::from(*bound), at, depth),
            TypeIdentifier::PlainSequenceLarge {
                header,
                bound,
                element,
            } => self.sequence(element, header.element_flags, *bound, at, depth),
            TypeIdentifier::PlainArraySmall {
                header,
                array_bounds,
                element,
            } => self.array(
                element,
                header.element_flags,
                &array_bounds
                    .iter()
                    .map(|d| u32::from(*d))
                    .collect::<Vec<_>>(),
                at,
                depth,
            ),
            TypeIdentifier::PlainArrayLarge {
                header,
                array_bounds,
                element,
            } => self.array(element, header.element_flags, array_bounds, at, depth),
            TypeIdentifier::PlainMapSmall {
                header,
                bound,
                element,
                key_flags,
                key,
            } => self.map(
                key,
                *key_flags,
                element,
                header.element_flags,
                u32::from(*bound),
                at,
                depth,
            ),
            TypeIdentifier::PlainMapLarge {
                header,
                bound,
                element,
                key_flags,
                key,
            } => self.map(
                key,
                *key_flags,
                element,
                header.element_flags,
                *bound,
                at,
                depth,
            ),
            TypeIdentifier::EquivalenceHashComplete(hash) => self.hashed(*hash, at, depth),
            TypeIdentifier::EquivalenceHashMinimal(_) => {
                Err(DynamicError::MinimalHasNoNames { at: at.to_string() })
            }
            TypeIdentifier::StronglyConnectedComponent(scc) => self.component(scc, at, depth),
            TypeIdentifier::None => Err(DynamicError::Unrepresentable {
                what: at.to_string(),
                kind: "`TK_NONE`, which names no type",
            }),
            // §7.3.4.2 has discriminator bytes this crate does not
            // decode, and a byte nobody defined. Both are refused the
            // same way: a member whose type is unknown moves every
            // member after it, and an identifier read as "some type" is
            // the silent form of that.
            TypeIdentifier::Extended { .. } => Err(DynamicError::Unrepresentable {
                what: at.to_string(),
                kind: "a TypeIdentifier discriminator this crate does not decode",
            }),
        }
    }

    /// The member of a strongly connected component an identifier names.
    ///
    /// XTypes 1.3 §7.3.4.9.2: the hash belongs to the component and the
    /// index to a type within it, so this needs the registry's component
    /// list and not its hash-to-object map — the component's hash is not
    /// any single type's.
    ///
    /// The node is announced under this identifier before its members
    /// are built, which is what lets the member that closes the cycle
    /// resolve to it.
    fn component(
        &mut self,
        scc: &crate::type_identifier::StronglyConnectedComponentId,
        at: &str,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let object = self
            .registry
            .component_member(&scc.hash, scc.scc_index)
            .ok_or_else(|| DynamicError::UnknownTypeIdentifier { at: at.to_string() })?
            .clone();
        let identifier = TypeIdentifier::StronglyConnectedComponent(*scc);
        self.object(&object, depth + 1, Some(&identifier))
    }

    fn hashed(
        &mut self,
        hash: EquivalenceHash,
        at: &str,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        if self.stack.contains(&hash) {
            return Err(DynamicError::Recursive {
                what: at.to_string(),
            });
        }
        let object = self
            .registry
            .get_complete(&hash)
            .ok_or_else(|| DynamicError::UnknownTypeIdentifier { at: at.to_string() })?
            .clone();
        self.stack.push(hash);
        let outcome = self.object(&object, depth + 1, None);
        self.stack.pop();
        outcome
    }

    // -- fully descriptive types --------------------------------------

    fn primitive(&mut self, kind: PrimitiveKind, at: &str) -> Result<Id<TypeNode>, DynamicError> {
        let ast_kind = match kind {
            PrimitiveKind::Boolean => AstPrimitive::Boolean,
            // §7.3.4.2 gives `TK_BYTE` and `TK_UINT8` distinct kinds
            // and `crate::from_ast` announces them apart, so the way
            // back has one answer per kind.
            PrimitiveKind::Byte => AstPrimitive::Octet,
            PrimitiveKind::UInt8 => AstPrimitive::UInt8,
            PrimitiveKind::Char8 => AstPrimitive::Char,
            PrimitiveKind::Char16 => AstPrimitive::WChar,
            PrimitiveKind::Int8 => AstPrimitive::Int8,
            PrimitiveKind::Int16 => AstPrimitive::Int16,
            PrimitiveKind::UInt16 => AstPrimitive::UInt16,
            PrimitiveKind::Int32 => AstPrimitive::Int32,
            PrimitiveKind::UInt32 => AstPrimitive::UInt32,
            PrimitiveKind::Int64 => AstPrimitive::Int64,
            PrimitiveKind::UInt64 => AstPrimitive::UInt64,
            PrimitiveKind::Float32 => AstPrimitive::Float32,
            PrimitiveKind::Float64 => AstPrimitive::Float64,
            PrimitiveKind::Float128 => AstPrimitive::Float128,
        };
        let _ = at;
        self.anonymous(ast_kind.as_str(), AstKind::Primitive(ast_kind))
    }

    fn string(
        &mut self,
        width: CharWidth,
        bound: u32,
        at: &str,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let _ = at;
        let bound = self.bound_of(bound)?;
        let name = match (width, bound) {
            (CharWidth::Char, Bound::Unbounded) => String::from("string"),
            (CharWidth::WChar, Bound::Unbounded) => String::from("wstring"),
            (CharWidth::Char, Bound::Const(_)) => String::from("bounded string"),
            (CharWidth::WChar, Bound::Const(_)) => String::from("bounded wstring"),
        };
        self.anonymous(&name, AstKind::String { width, bound })
    }

    fn sequence(
        &mut self,
        element: &TypeIdentifier,
        flags: CollectionElementFlag,
        bound: u32,
        at: &str,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let element = self.identifier(element, at, depth + 1)?;
        let bound = self.bound_of(bound)?;
        let id = self.anonymous(
            "sequence",
            AstKind::Sequence {
                element: Ref::Resolved(element),
                bound,
            },
        )?;
        self.set_element_try_construct(id, flags.0);
        Ok(id)
    }

    fn array(
        &mut self,
        element: &TypeIdentifier,
        flags: CollectionElementFlag,
        bounds: &[u32],
        at: &str,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let element = self.identifier(element, at, depth + 1)?;
        let mut dims = Vec::with_capacity(bounds.len());
        for bound in bounds {
            dims.push(
                self.ast
                    .add_int_expr(i128::from(*bound))
                    .map_err(|error| DynamicError::model(&error))?,
            );
        }
        let id = self.anonymous(
            "array",
            AstKind::Array {
                element: Ref::Resolved(element),
                dims,
            },
        )?;
        self.set_element_try_construct(id, flags.0);
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    fn map(
        &mut self,
        key: &TypeIdentifier,
        key_flags: CollectionElementFlag,
        value: &TypeIdentifier,
        value_flags: CollectionElementFlag,
        bound: u32,
        at: &str,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let key_id = self.identifier(key, at, depth + 1)?;
        let value_id = self.identifier(value, at, depth + 1)?;
        let bound = self.bound_of(bound)?;
        let id = self.anonymous(
            "map",
            AstKind::Map {
                key: Ref::Resolved(key_id),
                value: Ref::Resolved(value_id),
                bound,
            },
        )?;
        self.set_element_try_construct(id, value_flags.0);
        if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
            let x = facets.xtypes_mut();
            if let Some(behaviour) = try_construct_of(key_flags.0) {
                x.key_try_construct = Some(behaviour);
            }
            x.key_is_external = key_flags.0 & CollectionElementFlag::IS_EXTERNAL != 0;
        }
        Ok(id)
    }

    /// A `Bound` for a wire bound, where zero is "unbounded".
    ///
    /// §7.3.4.6 writes zero for an unbounded collection, which is why a
    /// declared bound of zero cannot be told from none — and why
    /// `TypeDescriptor::bound` reports an empty list rather than `[0]`.
    fn bound_of(&mut self, bound: u32) -> Result<Bound, DynamicError> {
        if bound == 0 {
            return Ok(Bound::Unbounded);
        }
        self.ast
            .add_int_expr(i128::from(bound))
            .map(Bound::Const)
            .map_err(|error| DynamicError::model(&error))
    }

    /// A type with no declaration of its own, in the root module.
    ///
    /// §7.3.4.6's fully descriptive types are anonymous: a
    /// `sequence<long>` is written where it is used and has no name a
    /// peer sent. The name here is a spelling for a diagnostic and
    /// nothing reads it back; the cache is what keeps two uses of one
    /// `sequence<long>` on one node.
    fn anonymous(&mut self, name: &str, kind: AstKind) -> Result<Id<TypeNode>, DynamicError> {
        let root = self.ast.root_module();
        self.ast
            .add_type(root, name, kind)
            .map_err(|error| DynamicError::model(&error))
    }

    fn any_type(&self, id: Id<TypeNode>) -> Result<zerodds_ast::AnyId, DynamicError> {
        self.ast
            .any_of_type(id)
            .map_err(|error| DynamicError::model(&error))
    }

    /// Takes the raw flag word and not either `CollectionElementFlag`.
    ///
    /// There are two types of that name in this crate — one in
    /// `type_identifier` for a plain collection's header, one in
    /// `type_object::flags` for a `CompleteCollectionElement` — and both
    /// reach here. Naming one of the two would have made the other
    /// convert at the call, which is where a wrong bit word gets built.
    fn set_element_try_construct(&mut self, id: Id<TypeNode>, flags: u16) {
        let behaviour = try_construct_of(flags);
        let external = flags & CollectionElementFlag::IS_EXTERNAL != 0;
        if behaviour.is_none() && !external {
            return;
        }
        let Ok(any) = self.any_type(id) else { return };
        if let Some(facets) = self.ast.facets_mut(any) {
            let x = facets.xtypes_mut();
            if behaviour.is_some() {
                x.element_try_construct = behaviour;
            }
            // `IS_EXTERNAL` of the element flags: the `@external` of
            // `sequence<@external T>` (§7.3.1.2.1.4).
            x.element_is_external = external;
        }
    }

    // -- aggregates ----------------------------------------------------

    fn structure(
        &mut self,
        node: &CompleteStructType,
        depth: usize,
        announce: Option<&TypeIdentifier>,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let name = node.header.detail.type_name.clone();
        let base = match &node.header.base_type {
            TypeIdentifier::None => None,
            other => Some(Ref::Resolved(self.identifier(other, &name, depth + 1)?)),
        };
        let (id, existed) = self.declare_once(
            &name,
            AstKind::Structure {
                base,
                members: Vec::new(),
            },
        )?;
        if existed {
            // Reached from the other side of a cycle; it is this type and
            // it is already being built.
            return Ok(id);
        }
        self.apply_aggregate_flags(id, node.struct_flags.0)?;
        // The declaration's own applications. A structure's
        // `CompleteTypeDetail::ann_custom` is the same slot a member's
        // is, so it goes onto the type node here.
        let applied = self.apply_type_annotations(&node.header.detail, &name, depth)?;
        self.apply_type_verbatim(id, &node.header.detail)?;
        if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
            facets.annotations.extend(applied);
        }
        // Announced before the members, so a member naming this
        // component finds the node instead of building it again.
        if let Some(identifier) = announce {
            self.cache.push((identifier.clone(), id));
        }
        for member in &node.member_seq {
            let at = format!("{name}::{}", member.detail.name);
            let member_type = self.identifier(&member.common.member_type_id, &at, depth + 1)?;
            let mut spec = MemberSpec::new(&member.detail.name, Ref::Resolved(member_type));
            apply_member_flags(
                spec.facets.xtypes_mut(),
                member.common.member_flags.0,
                member.common.member_id,
            );
            self.apply_member_annotations(
                spec.facets.xtypes_mut(),
                &member.detail,
                member_type,
                &at,
            )?;
            self.apply_applied_annotations(
                &member.detail.ann_custom,
                &mut spec.facets.annotations,
                depth,
                &at,
            )?;
            self.ast
                .add_member(id, spec)
                .map_err(|error| DynamicError::model(&error))?;
        }
        Ok(id)
    }

    fn union(
        &mut self,
        node: &CompleteUnionType,
        depth: usize,
        announce: Option<&TypeIdentifier>,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let name = node.header.detail.type_name.clone();
        let discriminator =
            self.identifier(&node.discriminator.common.type_id, &name, depth + 1)?;
        let (id, existed) = self.declare_once(
            &name,
            AstKind::Union {
                discriminator: Ref::Resolved(discriminator),
                cases: Vec::new(),
                default_case: None,
            },
        )?;
        if existed {
            return Ok(id);
        }
        self.apply_aggregate_flags(id, node.union_flags.0)?;
        // As in `structure`: announced before the members, so a branch
        // naming this component resolves to the node being built.
        if let Some(identifier) = announce {
            self.cache.push((identifier.clone(), id));
        }
        if let Some(behaviour) = try_construct_of(node.discriminator.common.member_flags.0) {
            if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
                facets.xtypes_mut().discriminator_try_construct = Some(behaviour);
            }
        }
        // The union's own applications and the discriminator's. Annex B
        // has an `ann_custom` on both `CompleteUnionHeader::detail` and
        // on `CompleteDiscriminatorMember`, and the model keeps them
        // apart: the union's in `Facets::annotations`, the
        // discriminator's in `XTypesFacet::discriminator_annotations`,
        // where `zerodds_idl_lower` reads `union U switch (@a K)`.
        let applied = self.apply_type_annotations(&node.header.detail, &name, depth)?;
        self.apply_type_verbatim(id, &node.header.detail)?;
        if !applied.is_empty() {
            if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
                facets.annotations.extend(applied);
            }
        }
        let mut on_discriminator = Vec::new();
        self.apply_applied_annotations(
            &node.discriminator.ann_custom,
            &mut on_discriminator,
            depth,
            &format!("{name} (discriminator)"),
        )?;
        if !on_discriminator.is_empty() {
            if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
                facets
                    .xtypes_mut()
                    .discriminator_annotations
                    .extend(on_discriminator);
            }
        }
        // `@key` on the discriminator, Table 21's second position for
        // the annotation. Dropping it here would turn a keyed union that
        // arrived over TypeLookup into an unkeyed one, and the model
        // would then hash to a `TypeIdentifier` that is not the one it
        // came in under.
        if node.discriminator.common.member_flags.0 & UnionDiscriminatorFlag::IS_KEY != 0 {
            if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
                facets.xtypes_mut().discriminator_is_key = true;
            }
        }
        for member in &node.member_seq {
            let at = format!("{name}::{}", member.detail.name);
            let member_type = self.identifier(&member.common.type_id, &at, depth + 1)?;
            let mut labels = Vec::with_capacity(member.common.label_seq.len() + 1);
            for label in &member.common.label_seq {
                labels.push(CaseLabel::Value(
                    self.ast
                        .add_int_expr(i128::from(*label))
                        .map_err(|error| DynamicError::model(&error))?,
                ));
            }
            if member.common.member_flags.0 & UnionMemberFlag::IS_DEFAULT != 0 {
                labels.push(CaseLabel::Default);
            }
            let mut spec = MemberSpec::new(&member.detail.name, Ref::Resolved(member_type));
            apply_member_flags(
                spec.facets.xtypes_mut(),
                member.common.member_flags.0,
                member.common.member_id,
            );
            self.apply_member_annotations(
                spec.facets.xtypes_mut(),
                &member.detail,
                member_type,
                &at,
            )?;
            self.apply_applied_annotations(
                &member.detail.ann_custom,
                &mut spec.facets.annotations,
                depth,
                &at,
            )?;
            self.ast
                .add_case(id, labels, spec)
                .map_err(|error| DynamicError::model(&error))?;
        }
        Ok(id)
    }

    fn enumeration(&mut self, node: &CompleteEnumeratedType) -> Result<Id<TypeNode>, DynamicError> {
        let name = node.header.detail.type_name.clone();
        let id = self.declare(
            &name,
            AstKind::Enumeration {
                enumerators: Vec::new(),
            },
        )?;
        if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
            facets.xtypes_mut().bit_bound = Some(node.header.common.bit_bound);
        }
        let applied = self.apply_type_annotations(&node.header.detail, &name, 0)?;
        self.apply_type_verbatim(id, &node.header.detail)?;
        if !applied.is_empty() {
            if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
                facets.annotations.extend(applied);
            }
        }
        for literal in &node.literal_seq {
            // Written as an explicit `@value`, always: the wire form
            // carries the number and nothing says whether the
            // declaration wrote it. A positional enumerator whose value
            // happened to match would serialise the same and would say
            // something about the source that the peer did not.
            let value = self
                .ast
                .add_int_expr(i128::from(literal.common.value))
                .map_err(|error| DynamicError::model(&error))?;
            let enumerator = self
                .ast
                .add_enumerator(id, &literal.detail.name, Some(value))
                .map_err(|error| DynamicError::model(&error))?;
            if literal.common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL != 0 {
                let node_id = self
                    .ast
                    .enumerator(enumerator)
                    .map_err(|error| DynamicError::model(&error))?
                    .node_id;
                let any = zerodds_ast::AnyId::Enumerator(enumerator, node_id);
                if let Some(facets) = self.ast.facets_mut(any) {
                    facets.xtypes_mut().is_default_literal = true;
                }
                let at = format!("{name}::{}", literal.detail.name);
                let mut applications = Vec::new();
                self.apply_applied_annotations(
                    &literal.detail.ann_custom,
                    &mut applications,
                    0,
                    &at,
                )?;
                if let Some(facets) = self.ast.facets_mut(any) {
                    facets.annotations.extend(applications);
                }
            }
        }
        Ok(id)
    }

    fn bitmask(&mut self, node: &CompleteBitmaskType) -> Result<Id<TypeNode>, DynamicError> {
        let name = node.detail.type_name.clone();
        let id = self.declare(&name, AstKind::Bitmask { bits: Vec::new() })?;
        if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
            facets.xtypes_mut().bit_bound = Some(node.bit_bound);
        }
        let applied = self.apply_type_annotations(&node.detail, &name, 0)?;
        self.apply_type_verbatim(id, &node.detail)?;
        if !applied.is_empty() {
            if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
                facets.annotations.extend(applied);
            }
        }
        for flag in &node.flag_seq {
            // As with an enumerator's value: the position is explicit
            // because the wire carried it.
            let bit = self
                .ast
                .add_bit_value(id, &flag.detail.name, Some(flag.common.position))
                .map_err(|error| DynamicError::model(&error))?;
            let at = format!("{name}::{}", flag.detail.name);
            let mut applications = Vec::new();
            self.apply_applied_annotations(&flag.detail.ann_custom, &mut applications, 0, &at)?;
            let Ok(bit_node) = self.ast.bit_value(bit) else {
                continue;
            };
            let any = zerodds_ast::AnyId::BitValue(bit, bit_node.node_id);
            if let Some(facets) = self.ast.facets_mut(any) {
                facets.annotations.extend(applications);
            }
        }
        Ok(id)
    }

    fn bitset(
        &mut self,
        node: &CompleteBitsetType,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let name = node.detail.type_name.clone();
        // A `CompleteBitsetType` carries no base: §7.3.4.5 writes the
        // derived bitset's own fields *and* its base's into one
        // `field_seq`, at the positions the packing gives them, which is
        // what `crate::from_ast::bitset` writes with `all_bitfields_of`.
        // So what comes back is one flat bitset, and the inheritance is
        // the part §7.3.4.5 does not carry.
        let id = self.declare(
            &name,
            AstKind::Bitset {
                base: None,
                fields: Vec::new(),
            },
        )?;
        let applied = self.apply_type_annotations(&node.detail, &name, depth)?;
        self.apply_type_verbatim(id, &node.detail)?;
        if !applied.is_empty() {
            if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
                facets.annotations.extend(applied);
            }
        }
        let mut position: u16 = 0;
        for field in &node.field_seq {
            let start = field.common.position;
            if start < position {
                return Err(DynamicError::inconsistent(format!(
                    "`{name}::{}` starts at bit {start}, behind bit {position} where the \
                     field before it ended — IDL 4.2 §7.4.13.4.3.2 packs bitfields \
                     consecutively",
                    field.detail.name
                )));
            }
            if start > position {
                // The gap is anonymous padding: §7.4.13.4.3.2 counts an
                // unnamed field in the packing and gives it no name, so
                // the only way to put the next field at the position the
                // wire says is to declare the padding it was declared
                // with.
                self.ast
                    .add_bitfield(id, None, start - position, None)
                    .map_err(|error| DynamicError::model(&error))?;
                position = start;
            }
            let holder = self.holder_type(field.common.holder_type, &name, depth)?;
            let bitfield = self
                .ast
                .add_bitfield(
                    id,
                    Some(&field.detail.name),
                    u16::from(field.common.bitcount),
                    Some(Ref::Resolved(holder)),
                )
                .map_err(|error| DynamicError::model(&error))?;
            let at = format!("{name}::{}", field.detail.name);
            let mut applications = Vec::new();
            self.apply_applied_annotations(
                &field.detail.ann_custom,
                &mut applications,
                depth,
                &at,
            )?;
            if !applications.is_empty() {
                let Ok(node_id) = self.ast.bitfield(bitfield).map(|field| field.node_id) else {
                    continue;
                };
                let any = zerodds_ast::AnyId::Bitfield(bitfield, node_id);
                if let Some(facets) = self.ast.facets_mut(any) {
                    facets.annotations.extend(applications);
                }
            }
            position = position
                .checked_add(u16::from(field.common.bitcount))
                .ok_or_else(|| {
                    DynamicError::inconsistent(format!(
                        "`{name}` is wider than 65535 bits, which no bitset is"
                    ))
                })?;
        }
        Ok(id)
    }

    fn holder_type(
        &mut self,
        holder: u8,
        at: &str,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let kind = PrimitiveKind::from_u8(holder).ok_or_else(|| DynamicError::Unrepresentable {
            what: at.to_string(),
            kind: "a bitfield holder that is not a primitive kind",
        })?;
        self.identifier(&TypeIdentifier::Primitive(kind), at, depth + 1)
    }

    // -- named collections ---------------------------------------------

    fn named_sequence(
        &mut self,
        node: &CompleteSequenceType,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        // Annex B: the detail is "not present for anonymous"; such a
        // sequence is an anonymous node, as a plain one is.
        let name = node
            .detail
            .as_ref()
            .map_or_else(|| String::from("sequence"), |d| d.type_name.clone());
        let element = self.identifier(&node.element.common.type_id, &name, depth + 1)?;
        let bound = self.bound_of(node.bound)?;
        let kind = AstKind::Sequence {
            element: Ref::Resolved(element),
            bound,
        };
        let id = if node.detail.is_some() {
            self.declare(&name, kind)?
        } else {
            self.anonymous("sequence", kind)?
        };
        self.set_element_try_construct(id, node.element.common.element_flags.0);
        self.apply_collection_annotations(
            id,
            node.detail.as_ref(),
            &node.element.ann_custom,
            &OptionalAppliedAnnotationSeq(None),
            &name,
            depth,
        )?;
        Ok(id)
    }

    fn named_array(
        &mut self,
        node: &CompleteArrayType,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let name = node.detail.type_name.clone();
        let element = self.identifier(&node.element.common.type_id, &name, depth + 1)?;
        let mut dims = Vec::with_capacity(node.bound_seq.len());
        for bound in &node.bound_seq {
            dims.push(
                self.ast
                    .add_int_expr(i128::from(*bound))
                    .map_err(|error| DynamicError::model(&error))?,
            );
        }
        let id = self.declare(
            &name,
            AstKind::Array {
                element: Ref::Resolved(element),
                dims,
            },
        )?;
        self.set_element_try_construct(id, node.element.common.element_flags.0);
        self.apply_collection_annotations(
            id,
            Some(&node.detail),
            &node.element.ann_custom,
            &OptionalAppliedAnnotationSeq(None),
            &name,
            depth,
        )?;
        Ok(id)
    }

    fn named_map(
        &mut self,
        node: &CompleteMapType,
        depth: usize,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let name = node
            .detail
            .as_ref()
            .map_or_else(|| String::from("map"), |d| d.type_name.clone());
        let key = self.identifier(&node.key.common.type_id, &name, depth + 1)?;
        let value = self.identifier(&node.element.common.type_id, &name, depth + 1)?;
        let bound = self.bound_of(node.bound)?;
        let kind = AstKind::Map {
            key: Ref::Resolved(key),
            value: Ref::Resolved(value),
            bound,
        };
        let id = if node.detail.is_some() {
            self.declare(&name, kind)?
        } else {
            self.anonymous("map", kind)?
        };
        self.set_element_try_construct(id, node.element.common.element_flags.0);
        if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
            let x = facets.xtypes_mut();
            if let Some(behaviour) = try_construct_of(node.key.common.element_flags.0) {
                x.key_try_construct = Some(behaviour);
            }
            x.key_is_external =
                node.key.common.element_flags.0 & CollectionElementFlag::IS_EXTERNAL != 0;
        }
        self.apply_collection_annotations(
            id,
            node.detail.as_ref(),
            &node.element.ann_custom,
            &node.key.ann_custom,
            &name,
            depth,
        )?;
        Ok(id)
    }

    // -- flags ---------------------------------------------------------

    fn apply_aggregate_flags(&mut self, id: Id<TypeNode>, bits: u16) -> Result<(), DynamicError> {
        let any = self.any_type(id)?;
        let Some(facets) = self.ast.facets_mut(any) else {
            return Ok(());
        };
        let xtypes = facets.xtypes_mut();
        // Exactly one of the three is set by a writer. Where none is —
        // which no `crate::from_ast` output has and a foreign one might —
        // the extensibility stays **absent** rather than defaulted, so
        // that `Ast::effective_extensibility` gives it the same answer an
        // unannotated declaration gets and the dialect decides, in one
        // place.
        xtypes.extensibility = if bits & StructTypeFlag::IS_FINAL != 0 {
            Some(Extensibility::Final)
        } else if bits & StructTypeFlag::IS_APPENDABLE != 0 {
            Some(Extensibility::Appendable)
        } else if bits & StructTypeFlag::IS_MUTABLE != 0 {
            Some(Extensibility::Mutable)
        } else {
            None
        };
        xtypes.is_nested = bits & StructTypeFlag::IS_NESTED != 0;
        if bits & StructTypeFlag::IS_AUTOID_HASH != 0 {
            xtypes.autoid = Some(AutoId::Hash);
        }
        Ok(())
    }

    /// The five builtin member annotations of §7.3.4.5.4, back onto the
    /// member's facet — the read half of what
    /// `crate::from_ast::builtin_member_annotations` writes.
    ///
    /// `@min`/`@max` come back as constant expressions of the value the
    /// wire carried, `@hashid` as the member id's origin — the id itself
    /// was already applied from `CommonStructMember` and does not move —
    /// and `@default` as the text parsed against the member's own type,
    /// which is the only type the clause offers to read it in.
    fn apply_member_annotations(
        &mut self,
        facet: &mut zerodds_ast::XTypesFacet,
        detail: &CompleteMemberDetail,
        member_type: Id<TypeNode>,
        at: &str,
    ) -> Result<(), DynamicError> {
        if let Some(unit) = &detail.ann_builtin.unit {
            facet.unit = Some(unit.clone());
        }
        if let Some(hash_id) = &detail.ann_builtin.hash_id {
            if let Some(assignment) = facet.member_id.as_mut() {
                assignment.origin = MemberIdOrigin::Hashed {
                    source_name: hash_id.clone(),
                };
            }
        }
        if detail.ann_builtin.min.is_some() || detail.ann_builtin.max.is_some() {
            let mut range = RangeConstraint {
                min: None,
                max: None,
            };
            if let Some(value) = &detail.ann_builtin.min {
                range.min = Some(self.annotation_expr(value)?);
            }
            if let Some(value) = &detail.ann_builtin.max {
                range.max = Some(self.annotation_expr(value)?);
            }
            facet.range = Some(range);
        }
        if let Some(text) = &detail.ann_builtin.default_value {
            facet.default_value = Some(super::default_literal::default_expr(
                &mut self.ast,
                text,
                member_type,
                at,
            )?);
        }
        Ok(())
    }

    // -- applied annotations ---------------------------------------------

    /// Annex B's `@optional AppliedAnnotationSeq ann_custom`, back onto the
    /// node's `Facets::annotations` — the read half of
    /// [`crate::from_ast::applied_annotations`], and the same list
    /// `zerodds_idl_lower` writes for an `@annotation` applied in IDL.
    ///
    /// `ann_custom` is the same member on a `CompleteTypeDetail` and on a
    /// `CompleteMemberDetail`, so one function reads both; `at` says which
    /// node an application sits on for the diagnostic.
    ///
    /// Each `AppliedAnnotation` names its annotation by
    /// `annotation_typeid` — the hash of the declaration's `TypeObject`
    /// (§7.3.4.2, Table 29) — so reading one means looking that hash up in
    /// the registry and rebuilding the declaration from the object it holds:
    /// [`Self::annotation_declaration`]. The parameter values then need the
    /// declaration's parameter names, because `AppliedAnnotationParameter`
    /// carries a `paramname_hash` where the model holds a name.
    fn apply_applied_annotations(
        &mut self,
        ann_custom: &OptionalAppliedAnnotationSeq,
        into: &mut Vec<AstAppliedAnnotation>,
        depth: usize,
        at: &str,
    ) -> Result<(), DynamicError> {
        let Some(values) = &ann_custom.0 else {
            return Ok(());
        };
        for applied in values {
            let TypeIdentifier::EquivalenceHashComplete(hash) = applied.annotation_typeid else {
                // §7.3.4.5 writes `annotation_typeid` as the hash of the
                // declaration's complete `TypeObject`, which
                // `TypeIdentifier` carries as `EK_COMPLETE` +
                // `EquivalenceHash`. Any other shape is either a minimal
                // hash, which carries no names at all, or a discriminator
                // this build does not decode — §7.3.4.4 and Table 29 have
                // no other arm for an annotation declaration, which
                // `docs/omg-xtypes-issues.md` §B9 writes up.
                return Err(DynamicError::Unrepresentable {
                    what: at.to_string(),
                    kind: "an `annotation_typeid` that is not a complete EquivalenceHash, which \
                           is the only shape Annex B gives an annotation declaration's identifier",
                });
            };
            let (name, definition, parameters) = self.annotation_declaration(hash, at, depth)?;
            let mut arguments = Vec::with_capacity(applied.param_seq.len());
            for parameter in &applied.param_seq {
                // §7.3.4.5 orders the parameters by `paramname_hash`, and
                // that order is kept because the wire has no other.
                let Some(parameter_name) = parameters
                    .iter()
                    .find(|candidate| NameHash::from_name(candidate) == parameter.paramname_hash)
                else {
                    return Err(DynamicError::Unrepresentable {
                        what: at.to_string(),
                        kind: "an `AppliedAnnotationParameter` whose `paramname_hash` names no \
                               parameter of the annotation declaration",
                    });
                };
                arguments.push(AppliedArgument {
                    name: Some(parameter_name.clone()),
                    value: self.annotation_expr(&parameter.value)?,
                });
            }
            into.push(AstAppliedAnnotation {
                name,
                definition: Ref::Resolved(definition),
                arguments,
            });
        }
        Ok(())
    }

    /// The declaration one `annotation_typeid` hash names, rebuilt as a
    /// node of this model.
    ///
    /// The registry is asked by hash, because that is the whole of what
    /// the wire carried: §7.3.4.5's `AppliedAnnotation::annotation_typeid`
    /// is §7.3.4.2's Table 29 hash of the annotation's `TypeObject`, and
    /// nothing in the applied form names the declaration. What comes back
    /// is `CompleteAnnotationType` — the qualified name in
    /// `CompleteAnnotationHeader`, and one `CompleteAnnotationParameter`
    /// per parameter with its `MemberName` and its default — so the
    /// declaration is rebuilt from exactly what a peer sent.
    ///
    /// The name is the simple one, so that `@ann::mark` and `@mark` are
    /// the same model: `zerodds_idl_lower` writes the simple name for
    /// both, and the qualified name lives in the module the declaration is
    /// created in.
    ///
    /// Built once per hash and cached, so two applications of one
    /// annotation on two members of the same structure are one declaration
    /// in the model. An object the registry does not hold, or one that is
    /// not a `TK_ANNOTATION` declaration, is refused by name rather than
    /// skipped: an applied annotation whose declaration cannot be read
    /// would otherwise vanish from the rebuilt model while the node it
    /// sits on kept every other annotation, and a regenerated IDL would
    /// come back without it.
    ///
    /// The return is the name a writer would write it under, the node, and
    /// the parameter names in the object's order — which is what an
    /// `AppliedAnnotationParameter`'s `paramname_hash` is matched against.
    fn annotation_declaration(
        &mut self,
        hash: EquivalenceHash,
        at: &str,
        depth: usize,
    ) -> Result<(String, Id<AnnotationDefNode>, Vec<String>), DynamicError> {
        if let Some((_, definition, parameters)) =
            self.annotations.iter().find(|(known, _, _)| *known == hash)
        {
            let Ok(declaration) = self.ast.annotation_def(*definition) else {
                return Err(DynamicError::Unrepresentable {
                    what: at.to_string(),
                    kind: "an annotation declaration rebuilt earlier and gone from the model",
                });
            };
            return Ok((declaration.name.clone(), *definition, parameters.clone()));
        }
        let object = self
            .registry
            .get_complete(&hash)
            .ok_or_else(|| DynamicError::UnknownTypeIdentifier {
                at: alloc::format!(
                    "{at}: the annotation declaration this `annotation_typeid` names, whose \
                     TypeObject the registry does not hold"
                ),
            })?
            .clone();
        let CompleteTypeObject::Annotation(annotation) = &object else {
            return Err(DynamicError::Unrepresentable {
                what: at.to_string(),
                kind: "an `annotation_typeid` whose TypeObject is not a TK_ANNOTATION declaration",
            });
        };
        let qualified = annotation.header.annotation_name.clone();
        let (module, name) = self.module_for(&qualified)?;
        let definition = self
            .ast
            .add_annotation_def(module, &name)
            .map_err(|error| DynamicError::model(&error))?;
        let mut parameters = Vec::with_capacity(annotation.member_seq.len());
        for parameter in &annotation.member_seq {
            // The parameter's own type, read from its `TypeIdentifier`:
            // a primitive identifier names itself and an enumeration is a
            // hash the registry holds, which is the same resolution every
            // other member type on this path goes through.
            let parameter_type =
                self.identifier(&parameter.common.member_type_id, at, depth + 1)?;
            // `CompleteAnnotationParameter::default_value` is not
            // `@optional` in Annex B, so there is always a value to put
            // in the declaration's own `default`.
            let default = self.annotation_expr(&parameter.default_value)?;
            self.ast
                .add_annotation_member(
                    definition,
                    &parameter.name,
                    Ref::Resolved(parameter_type),
                    Some(default),
                )
                .map_err(|error| DynamicError::model(&error))?;
            parameters.push(parameter.name.clone());
        }
        self.annotations
            .push((hash, definition, parameters.clone()));
        Ok((name, definition, parameters))
    }

    /// A type's `@verbatim`, off the `AppliedBuiltinTypeAnnotations` of its
    /// `CompleteTypeDetail` (Annex B), onto the node's `XTypesFacet`.
    ///
    /// A placement the model has no name for (§7.2.2.4.8.2: "individual
    /// compiler implementations may recognize others") cannot be kept, and
    /// is refused by name rather than left out.
    fn apply_type_verbatim(
        &mut self,
        id: Id<TypeNode>,
        detail: &CompleteTypeDetail,
    ) -> Result<(), DynamicError> {
        use crate::type_object::common::VerbatimPlacement as Wire;
        use zerodds_ast::VerbatimPlacement as Model;
        let Some(verbatim) = &detail.ann_builtin.verbatim else {
            return Ok(());
        };
        let placement = match &verbatim.placement {
            Wire::BeginFile => Model::BeginFile,
            Wire::Before => Model::BeforeDeclaration,
            Wire::BeginDeclaration => Model::BeginDeclaration,
            Wire::EndDeclaration => Model::EndDeclaration,
            Wire::After => Model::AfterDeclaration,
            Wire::EndFile => Model::EndFile,
            Wire::Other(written) => {
                return Err(DynamicError::Unrepresentable {
                    what: detail.type_name.clone(),
                    kind: if written.is_empty() {
                        "a @verbatim with an empty placement"
                    } else {
                        "a @verbatim with a placement the model has no name for"
                    },
                });
            }
        };
        if let Some(facets) = self.ast.facets_mut(self.any_type(id)?) {
            facets
                .xtypes_mut()
                .verbatim
                .push(zerodds_ast::VerbatimDirective {
                    language: verbatim.language.clone(),
                    placement,
                    text: verbatim.text.clone(),
                });
        }
        Ok(())
    }

    /// A type's own applied annotations, off its `CompleteTypeDetail`.
    fn apply_type_annotations(
        &mut self,
        detail: &CompleteTypeDetail,
        at: &str,
        depth: usize,
    ) -> Result<Vec<AstAppliedAnnotation>, DynamicError> {
        let mut applications = Vec::new();
        self.apply_applied_annotations(&detail.ann_custom, &mut applications, depth, at)?;
        Ok(applications)
    }

    /// A named collection's own applied annotations, and its element's.
    ///
    /// Annex B gives a named `CompleteSequenceType`, `CompleteArrayType`
    /// and `CompleteMapType` two `ann_custom` members — the type's, in its
    /// `CompleteTypeDetail`, and the element's, in the
    /// `CompleteCollectionElement` — and `CompleteMapType` a third for
    /// its key. The type's go on the node's `Facets::annotations`; the
    /// element's and the key's on `XTypesFacet::element_annotations` and
    /// `key_annotations`, which is where `zerodds_idl_lower` puts the
    /// annotations written in front of `sequence<@a Foo, 4>`.
    fn apply_collection_annotations(
        &mut self,
        collection: Id<TypeNode>,
        detail: Option<&CompleteTypeDetail>,
        element: &OptionalAppliedAnnotationSeq,
        key: &OptionalAppliedAnnotationSeq,
        at: &str,
        depth: usize,
    ) -> Result<(), DynamicError> {
        let applied = match detail {
            Some(detail) => self.apply_type_annotations(detail, at, depth)?,
            None => Vec::new(),
        };
        if !applied.is_empty() {
            if let Some(facets) = self.ast.facets_mut(self.any_type(collection)?) {
                facets.annotations.extend(applied);
            }
        }
        let mut on_element = Vec::new();
        self.apply_applied_annotations(
            element,
            &mut on_element,
            depth,
            &format!("{at} (element)"),
        )?;
        let mut on_key = Vec::new();
        self.apply_applied_annotations(key, &mut on_key, depth, &format!("{at} (key)"))?;
        if !on_element.is_empty() || !on_key.is_empty() {
            if let Some(facets) = self.ast.facets_mut(self.any_type(collection)?) {
                let x = facets.xtypes_mut();
                x.element_annotations.extend(on_element);
                x.key_annotations.extend(on_key);
            }
        }
        Ok(())
    }

    /// An `AnnotationParameterValue`, as the literal expression the model
    /// holds a constant in. The variant already names its kind, so no
    /// context is needed to read it.
    fn annotation_expr(
        &mut self,
        value: &AnnotationParameterValue,
    ) -> Result<Id<zerodds_ast::ConstExprNode>, DynamicError> {
        let literal = match value {
            AnnotationParameterValue::Boolean(v) => Literal::Boolean(*v),
            AnnotationParameterValue::Byte(v) | AnnotationParameterValue::UInt8(v) => {
                Literal::Integer(i128::from(*v))
            }
            AnnotationParameterValue::Int8(v) => Literal::Integer(i128::from(*v)),
            AnnotationParameterValue::Int16(v) => Literal::Integer(i128::from(*v)),
            AnnotationParameterValue::UInt16(v) => Literal::Integer(i128::from(*v)),
            AnnotationParameterValue::Int32(v) | AnnotationParameterValue::Enum(v) => {
                Literal::Integer(i128::from(*v))
            }
            AnnotationParameterValue::UInt32(v) => Literal::Integer(i128::from(*v)),
            AnnotationParameterValue::Int64(v) => Literal::Integer(i128::from(*v)),
            AnnotationParameterValue::UInt64(v) => Literal::Integer(i128::from(*v)),
            // `{}` on a float is the shortest text that parses back to
            // the same value, so the bits survive the round trip.
            AnnotationParameterValue::Float32Bits(bits) => Literal::Float {
                mantissa: format!("{}", f32::from_bits(*bits)),
                exponent: None,
            },
            AnnotationParameterValue::Float64Bits(bits) => Literal::Float {
                mantissa: format!("{}", f64::from_bits(*bits)),
                exponent: None,
            },
            AnnotationParameterValue::Float128Bits(bits) => {
                let value = Float128::from_bits(*bits);
                if !value.is_finite() {
                    return Err(DynamicError::Unrepresentable {
                        what: String::from("a `long double` annotation value"),
                        kind: "a NaN or an infinity, which IDL has no literal for",
                    });
                }
                let text = value.to_shortest_decimal();
                match text.split_once(['e', 'E']) {
                    Some((mantissa, exponent)) => Literal::Float {
                        mantissa: String::from(mantissa),
                        exponent: Some(String::from(exponent)),
                    },
                    None => Literal::Float {
                        mantissa: text,
                        exponent: None,
                    },
                }
            }
            AnnotationParameterValue::Char8(v) => Literal::Char(*v),
            AnnotationParameterValue::Char16(v) => Literal::WChar(u32::from(*v)),
            AnnotationParameterValue::String8(v) => Literal::String(v.clone()),
            AnnotationParameterValue::String16(v) => {
                Literal::WString(v.chars().map(u32::from).collect())
            }
            AnnotationParameterValue::Extended { kind, .. } => {
                return Err(DynamicError::Unrepresentable {
                    what: alloc::format!("an annotation value with discriminator 0x{kind:02x}"),
                    kind: "an ExtendedAnnotationParameterValue, which has no case in the model",
                });
            }
        };
        self.ast
            .add_expr(ConstExpr::Literal(literal))
            .map_err(|error| DynamicError::model(&error))
    }
}

/// The `@try_construct` the two `TRY_CONSTRUCT` bits name. §7.3.1.2.1.1.
///
/// `01` is DISCARD, `10` is USE_DEFAULT, `11` is TRIM. `00` is undefined
/// and is never written, so it answers `None` — which leaves the
/// annotation **absent** on the way back, and an absent annotation is
/// §7.2.2.7's DISCARD by way of `TryConstruct::ABSENT`. Writing DISCARD
/// out instead would say the peer had annotated something it did not.
const fn try_construct_of(bits: u16) -> Option<TryConstruct> {
    match bits & (StructMemberFlag::TRY_CONSTRUCT1 | StructMemberFlag::TRY_CONSTRUCT2) {
        StructMemberFlag::TRY_CONSTRUCT1 => Some(TryConstruct::Discard),
        StructMemberFlag::TRY_CONSTRUCT2 => Some(TryConstruct::UseDefault),
        3 => Some(TryConstruct::Trim),
        _ => None,
    }
}

/// Every member flag §7.3.4.5 carries, onto the member's facet.
fn apply_member_flags(facet: &mut zerodds_ast::XTypesFacet, bits: u16, member_id: u32) {
    facet.try_construct = try_construct_of(bits);
    facet.is_external = bits & StructMemberFlag::IS_EXTERNAL != 0;
    facet.is_optional = bits & StructMemberFlag::IS_OPTIONAL != 0;
    facet.is_must_understand = bits & StructMemberFlag::IS_MUST_UNDERSTAND != 0;
    facet.is_key = bits & StructMemberFlag::IS_KEY != 0;
    // Declared, because it was: the number came over the wire as this
    // member's id, whatever rule the peer's frontend used to arrive at
    // it. Recording it as `PreviousPlusOne` would claim a rule the peer
    // never stated, and a later member would then be numbered from it.
    facet.member_id = Some(MemberIdAssignment {
        effective: member_id,
        origin: MemberIdOrigin::Declared,
    });
}

/// The qualified name a `CompleteTypeObject` carries, for a diagnostic.
fn qualified_name(object: &CompleteTypeObject) -> &str {
    match object {
        CompleteTypeObject::Alias(node) => &node.header.detail.type_name,
        CompleteTypeObject::Annotation(node) => &node.header.annotation_name,
        CompleteTypeObject::Struct(node) => &node.header.detail.type_name,
        CompleteTypeObject::Union(node) => &node.header.detail.type_name,
        CompleteTypeObject::Bitset(node) => &node.detail.type_name,
        // Annex B: a sequence's and a map's detail is "not present for
        // anonymous".
        CompleteTypeObject::Sequence(node) => node
            .detail
            .as_ref()
            .map_or("<anonymous sequence>", |d| d.type_name.as_str()),
        CompleteTypeObject::Array(node) => &node.detail.type_name,
        CompleteTypeObject::Map(node) => node
            .detail
            .as_ref()
            .map_or("<anonymous map>", |d| d.type_name.as_str()),
        CompleteTypeObject::Enumerated(node) => &node.header.detail.type_name,
        CompleteTypeObject::Bitmask(node) => &node.detail.type_name,
        // Annex B's `default: CompleteExtendedType extended_type;` is an
        // empty structure; a TypeKind this revision has no shape for has
        // no name in the stream either.
        CompleteTypeObject::Extended { .. } => "<extended TypeKind>",
    }
}

/// A `DynamicType` over one peer's `CompleteTypeObject`.
///
/// The model it is a view on holds that type and every type it names, and
/// nothing else.
///
/// # Errors
/// [`DynamicError`], naming the type or the member that stopped it.
pub fn dynamic_type(
    object: &CompleteTypeObject,
    registry: &TypeRegistry,
) -> Result<DynamicType, DynamicError> {
    let mut builder = ModelFromTypeObjects::new(registry);
    let id = builder.add(object)?;
    Ok(DynamicType::new(builder.finish(), id))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::type_object::minimal::{MinimalStructHeader, MinimalStructType, MinimalTypeObject};

    /// A `MinimalTypeObject` carries name hashes and not names, so no
    /// `DynamicType` comes out of one — and the refusal says which form
    /// was offered rather than producing a type with invented member
    /// names.
    #[test]
    fn a_minimal_type_object_is_refused_by_name() {
        let registry = TypeRegistry::new();
        let mut builder = ModelFromTypeObjects::new(&registry);
        let minimal = TypeObject::Minimal(MinimalTypeObject::Struct(MinimalStructType {
            struct_flags: crate::type_object::flags::StructTypeFlag(StructTypeFlag::IS_FINAL),
            header: MinimalStructHeader {
                base_type: TypeIdentifier::None,
            },
            member_seq: Vec::new(),
        }));
        assert!(matches!(
            builder.add_type_object(&minimal),
            Err(DynamicError::MinimalHasNoNames { .. })
        ));
    }

    /// A member whose type the registry does not hold stops the rebuild
    /// at that member, and not at the last one.
    #[test]
    fn an_unresolvable_member_type_is_named_where_it_sits() {
        use crate::type_object::common::{CommonStructMember, CompleteMemberDetail};
        use crate::type_object::complete::{
            CompleteStructHeader, CompleteStructMember, CompleteStructType,
        };

        let registry = TypeRegistry::new();
        let mut builder = ModelFromTypeObjects::new(&registry);
        let object = CompleteTypeObject::Struct(CompleteStructType {
            struct_flags: crate::type_object::flags::StructTypeFlag(StructTypeFlag::IS_FINAL),
            header: CompleteStructHeader {
                base_type: TypeIdentifier::None,
                detail: crate::type_object::common::CompleteTypeDetail {
                    type_name: String::from("a::Sample"),
                    ann_builtin: Default::default(),
                    ann_custom: crate::type_object::common::OptionalAppliedAnnotationSeq(None),
                },
            },
            member_seq: alloc::vec![CompleteStructMember {
                common: CommonStructMember {
                    member_id: 0,
                    member_flags: crate::type_object::flags::StructMemberFlag(0),
                    // A hash of something nobody registered.
                    member_type_id: TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(
                        [0x11; 14],
                    )),
                },
                detail: CompleteMemberDetail {
                    name: String::from("nested"),
                    ann_builtin: Default::default(),
                    ann_custom: crate::type_object::common::OptionalAppliedAnnotationSeq(None),
                },
            }],
        });
        match builder.add(&object) {
            Err(DynamicError::UnknownTypeIdentifier { at }) => {
                assert_eq!(at, "a::Sample::nested");
            }
            other => panic!("expected the member to be named, got {other:?}"),
        }
    }

    /// A qualified name puts the type in the module it names.
    #[test]
    fn a_qualified_name_becomes_modules() {
        use crate::type_object::complete::{CompleteEnumeratedHeader, CompleteEnumeratedType};
        use crate::type_object::minimal::{CommonEnumeratedHeader, CommonEnumeratedLiteral};

        let registry = TypeRegistry::new();
        let mut builder = ModelFromTypeObjects::new(&registry);
        let object = CompleteTypeObject::Enumerated(CompleteEnumeratedType {
            enum_flags: crate::type_object::flags::EnumTypeFlag(0),
            header: CompleteEnumeratedHeader {
                common: CommonEnumeratedHeader { bit_bound: 16 },
                detail: crate::type_object::common::CompleteTypeDetail {
                    type_name: String::from("a::b::Colour"),
                    ann_builtin: Default::default(),
                    ann_custom: crate::type_object::common::OptionalAppliedAnnotationSeq(None),
                },
            },
            literal_seq: alloc::vec![crate::type_object::complete::CompleteEnumeratedLiteral {
                common: CommonEnumeratedLiteral {
                    value: 7,
                    flags: EnumLiteralFlag(EnumLiteralFlag::IS_DEFAULT_LITERAL),
                },
                detail: crate::type_object::common::CompleteMemberDetail {
                    name: String::from("RED"),
                    ann_builtin: Default::default(),
                    ann_custom: crate::type_object::common::OptionalAppliedAnnotationSeq(None),
                },
            }],
        });
        let id = builder.add(&object).expect("the enumeration rebuilds");
        let view = DynamicType::new(builder.finish(), id);
        assert_eq!(view.qualified_name(), "a::b::Colour");
        assert_eq!(view.bit_bound(), Some(16));
        let literal = view.member_by_name("RED").expect("the literal");
        // The enumerator's value is what `CommonEnumeratedLiteral::value`
        // carries; its member id is MEMBER_ID_INVALID (§7.5.2.7.5).
        assert_eq!(literal.literal_value(), Some(7));
        assert_eq!(literal.id(), crate::dynamic::MEMBER_ID_INVALID);
    }
}
