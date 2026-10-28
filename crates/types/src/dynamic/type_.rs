// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DynamicType` and `DynamicTypeMember`. XTypes 1.3 §7.5.2.8.
//!
//! A [`DynamicType`] is a **view on [`zerodds_ast::Ast`]** — a shared
//! model and one node id — and not a representation of its own.
//!
//! ## Why a view and not a tree
//!
//! The module this replaces held a `DynamicType` as an `Arc` over an
//! owned descriptor and an owned member list, built by a builder and
//! bridged to and from a `TypeObject`. That made three representations of
//! one type: the model, the wire form, and this. Every property a type
//! has — its extensibility when the source said nothing, a member's
//! effective id, an enumerator's value, a bit bound's default — then had
//! an answer in each, and the answers agreed only until one of the three
//! changed. Every defect this rebuild has found so far has that shape.
//!
//! So there is one representation, and it is the model.
//! [`crate::from_ast`] derives the wire form from it and
//! [`super::to_ast`] derives it back from a peer's wire form; this module
//! reads it. None of the three decides anything.
//!
//! ## What a handle costs
//!
//! An `Arc` clone and a copy of an id. Walking a hundred-member structure
//! allocates one `Vec` of handles and reads no node twice.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_ast::{
    Ast, BitValueNode, Bound, CaseNode, CharWidth, EnumeratorNode, MemberNode,
    PrimitiveKind as AstPrimitive, TypeKind as AstKind, TypeNode,
};
use zerodds_ast_core::{Id, ScopedName};

use super::annotation::{AnnotationDescriptor, VerbatimTextDescriptor};
use super::descriptor::{
    ExtensibilityKind, MemberDescriptor, MemberId, TryConstructKind, TypeDescriptor, TypeKind,
};
use super::error::DynamicError;

/// How deep [`DynamicType::equals`] follows a type before it gives up.
///
/// A model whose references form a cycle is compared co-inductively — see
/// [`DynamicType::equals`] — so this is not what stops a cycle. It stops
/// a model that is *deep* rather than cyclic from using the stack up, and
/// is the same bound `zerodds_wire_codec::default` uses for the same
/// reason.
const MAX_DEPTH: usize = 64;

/// One type in a model. §7.5.2.8.
///
/// Read-only. The model behind it is shared and immutable; building one
/// out of a peer's `TypeObject` is [`super::to_ast`]'s business.
#[derive(Clone)]
pub struct DynamicType {
    ast: Arc<Ast>,
    id: Id<TypeNode>,
}

impl DynamicType {
    /// A view on one type of a model.
    ///
    /// The id is not checked here — [`Self::descriptor`] and every
    /// accessor report a stale handle as
    /// [`DynamicError::Unresolved`] — because a constructor that
    /// validated would make the cheap operation the expensive one.
    #[must_use]
    pub const fn new(ast: Arc<Ast>, id: Id<TypeNode>) -> Self {
        Self { ast, id }
    }

    /// The type of that scoped name, `a::B::C` or `::a::B::C`.
    ///
    /// # Errors
    /// [`DynamicError::NoSuchType`] when the model declares no such type.
    /// A name that resolves to something that is not a type — a module,
    /// an interface, a constant — is the same refusal: what was asked for
    /// is a type and there is none.
    pub fn find(ast: &Arc<Ast>, name: &str) -> Result<Self, DynamicError> {
        let segments: Vec<String> = name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(ToString::to_string)
            .collect();
        ast.find_by_scoped_name(&ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .map(|id| Self::new(Arc::clone(ast), id))
            .ok_or_else(|| DynamicError::NoSuchType {
                name: name.to_string(),
            })
    }

    /// The model this view is into.
    #[must_use]
    pub fn ast(&self) -> &Arc<Ast> {
        &self.ast
    }

    /// The node this view is of.
    #[must_use]
    pub const fn id(&self) -> Id<TypeNode> {
        self.id
    }

    fn node(&self) -> Result<&TypeNode, DynamicError> {
        self.ast
            .type_(self.id)
            .map_err(|_| DynamicError::Unresolved {
                at: String::from("a type handle"),
            })
    }

    /// §7.5.2.8 (Table 54) `name` — the simple name.
    ///
    /// `<unknown type>` for a stale handle rather than a panic: a name is
    /// asked for in a diagnostic, and a diagnostic that panics loses the
    /// diagnostic. [`Self::descriptor`] is the accessor that reports it.
    #[must_use]
    pub fn name(&self) -> &str {
        self.node()
            .map_or("<unknown type>", |node| node.name.as_str())
    }

    /// The fully qualified name, without a leading `::`.
    ///
    /// The spelling `CompleteTypeDetail::type_name` carries, which is
    /// what Cyclone, Fast DDS and RTI all write — see
    /// `crate::from_ast::qualified_name`, which reads it the same way for
    /// the wire form.
    #[must_use]
    pub fn qualified_name(&self) -> String {
        self.node()
            .ok()
            .map(|node| zerodds_ast::AnyId::Type(self.id, node.node_id))
            .and_then(|any| self.ast.scoped_name_of(any))
            .map(|scoped| {
                let text = scoped.to_string();
                text.strip_prefix("::").unwrap_or(&text).to_string()
            })
            .unwrap_or_else(|| self.name().to_string())
    }

    /// §7.5.2.8 (Table 54) `kind`.
    ///
    /// [`TypeKind::NoType`] for a construct XTypes does not tabulate and
    /// for a stale handle. Which of the two it is, is what
    /// [`Self::descriptor`] answers.
    #[must_use]
    pub fn kind(&self) -> TypeKind {
        self.node()
            .map_or(TypeKind::NoType, |node| match &node.kind {
                AstKind::Primitive(primitive) => TypeKind::of_primitive(*primitive),
                AstKind::String { width, .. } => match width {
                    CharWidth::Char => TypeKind::String8,
                    CharWidth::WChar => TypeKind::String16,
                },
                AstKind::Sequence { .. } => TypeKind::Sequence,
                AstKind::Array { .. } => TypeKind::Array,
                AstKind::Map { .. } => TypeKind::Map,
                AstKind::Structure { .. } => TypeKind::Structure,
                AstKind::Union { .. } => TypeKind::Union,
                AstKind::Enumeration { .. } => TypeKind::Enumeration,
                AstKind::Bitmask { .. } => TypeKind::Bitmask,
                AstKind::Bitset { .. } => TypeKind::Bitset,
                AstKind::Alias { .. } => TypeKind::Alias,
                AstKind::AnnotationType { .. } => TypeKind::Annotation,
                // Every kind IDL 4.2 declares and Annex C's `TypeKind` has no
                // row for. Named rather than swept into a catch-all so that
                // a kind added to the model is a compile error here.
                AstKind::Fixed { .. }
                | AstKind::Valuetype { .. }
                | AstKind::ValueBox { .. }
                | AstKind::Native
                | AstKind::ValueBase
                | AstKind::Any
                | AstKind::ObjectRef { .. } => TypeKind::NoType,
            })
    }

    /// §7.5.2.8 `get_descriptor()`.
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`] for a stale handle or for a reference
    /// the resolver did not close.
    pub fn descriptor(&self) -> Result<TypeDescriptor, DynamicError> {
        let node = self.node()?;
        let name = self.qualified_name();
        let mut out = TypeDescriptor {
            kind: self.kind(),
            name: name.clone(),
            base_type: None,
            discriminator_type: None,
            bound: Vec::new(),
            element_type: None,
            key_element_type: None,
            extensibility_kind: None,
            is_nested: node
                .facets
                .xtypes
                .as_ref()
                .is_some_and(|facet| facet.is_nested),
            bit_bound: None,
        };
        match &node.kind {
            AstKind::String { bound, width } => {
                out.bound = self.one_bound(*bound, &name)?;
                // §7.5.2.4.5: the element type of a string is Char8, and
                // Char16 for a wstring (§7.5.2.2.5).
                out.element_type = Some(primitive_view(match width {
                    CharWidth::Char => AstPrimitive::Char,
                    CharWidth::WChar => AstPrimitive::WChar,
                })?);
            }
            AstKind::Sequence { element, bound } => {
                out.element_type = Some(self.follow(element, &name)?);
                out.bound = self.one_bound(*bound, &name)?;
            }
            AstKind::Array { element, dims } => {
                out.element_type = Some(self.follow(element, &name)?);
                let mut bounds = Vec::with_capacity(dims.len());
                for dim in dims {
                    let value =
                        self.ast
                            .eval_integral(*dim)
                            .map_err(|_| DynamicError::Unresolved {
                                at: alloc::format!("{name}: an array dimension"),
                            })?;
                    bounds.push(u32::try_from(value).map_err(|_| DynamicError::Unresolved {
                        at: alloc::format!("{name}: an array dimension past a u32"),
                    })?);
                }
                out.bound = bounds;
            }
            AstKind::Map { key, value, bound } => {
                out.key_element_type = Some(self.follow(key, &name)?);
                out.element_type = Some(self.follow(value, &name)?);
                out.bound = self.one_bound(*bound, &name)?;
            }
            // §7.5.2.4.1: "If this descriptor represents an alias type,
            // base_type indicates the type being aliased."
            AstKind::Alias { aliased } => {
                out.base_type = Some(self.follow(aliased, &name)?);
            }
            AstKind::Structure { base, .. } => {
                if let Some(base) = base {
                    out.base_type = Some(self.follow(base, &name)?);
                }
                out.extensibility_kind = Some(self.ast.effective_extensibility(self.id).into());
            }
            AstKind::Union { discriminator, .. } => {
                out.discriminator_type = Some(self.follow(discriminator, &name)?);
                out.extensibility_kind = Some(self.ast.effective_extensibility(self.id).into());
            }
            AstKind::Bitset {
                base: Some(base), ..
            } => {
                out.base_type = Some(self.follow(base, &name)?);
            }
            // §7.5.2.4.2: the bound of a bitmask is its `bound`, and §7.5.2.4.5
            // gives it a Boolean element type.
            AstKind::Bitmask { .. } => {
                out.bound = alloc::vec![u32::from(self.ast.effective_bit_bound(self.id))];
                out.element_type = Some(primitive_view(AstPrimitive::Boolean)?);
            }
            AstKind::Enumeration { .. } => {
                out.bit_bound = Some(self.ast.effective_bit_bound(self.id));
            }
            _ => {}
        }
        Ok(out)
    }

    fn one_bound(&self, bound: Bound, at: &str) -> Result<Vec<u32>, DynamicError> {
        let Some(expr) = bound.expr() else {
            // Unbounded: one bound, `LENGTH_UNLIMITED` (Table 50: "the
            // length of the property value is one"; §7.5.2.2.3 to
            // §7.5.2.2.5: that value "shall be considered to be
            // unbounded"). See `TypeDescriptor::bound`.
            return Ok(alloc::vec![super::builder::LENGTH_UNLIMITED]);
        };
        let value = self
            .ast
            .eval_integral(expr)
            .map_err(|_| DynamicError::Unresolved {
                at: alloc::format!("{at}: a bound"),
            })?;
        Ok(alloc::vec![u32::try_from(value).map_err(|_| {
            DynamicError::Unresolved {
                at: alloc::format!("{at}: a bound past a u32"),
            }
        })?])
    }

    fn follow(
        &self,
        reference: &zerodds_ast_core::Ref<TypeNode>,
        at: &str,
    ) -> Result<Self, DynamicError> {
        let id = reference
            .resolved()
            .ok_or_else(|| DynamicError::Unresolved { at: at.to_string() })?;
        if self.ast.type_(id).is_err() {
            return Err(DynamicError::Unresolved { at: at.to_string() });
        }
        Ok(Self::new(Arc::clone(&self.ast), id))
    }

    /// The type an alias chain ends at, or this type where it is not an
    /// alias.
    ///
    /// §7.2.4.4.3: an alias is its target for every purpose the wire has.
    /// The chain is followed at most `MAX_DEPTH` steps; a longer one is
    /// a cycle of typedefs, which the frontend refuses and which this
    /// reports rather than loops on.
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`] for a broken link,
    /// [`DynamicError::Recursive`] for a chain that does not end.
    pub fn resolve_alias(&self) -> Result<Self, DynamicError> {
        let mut current = self.clone();
        for _ in 0..MAX_DEPTH {
            let node = current.node()?;
            let AstKind::Alias { aliased } = &node.kind else {
                return Ok(current);
            };
            let name = node.name.clone();
            current = current.follow(aliased, &name)?;
        }
        Err(DynamicError::Recursive {
            what: self.name().to_string(),
        })
    }

    /// §7.5.2.4 `base_type` — what a `struct` or a `bitset` extends, and
    /// the type an alias names (§7.5.2.4.1).
    #[must_use]
    pub fn base_type(&self) -> Option<Self> {
        if let Ok(AstKind::Alias { aliased }) = self.node().map(|node| &node.kind) {
            return self.follow(aliased, self.name()).ok();
        }
        self.ast
            .base_of(self.id)
            .map(|id| Self::new(Arc::clone(&self.ast), id))
    }

    /// §7.5.2.4 `element_type` — of an array, a sequence, a map's value;
    /// Char8 or Char16 for a string and Boolean for a bitmask
    /// (§7.5.2.4.5). An alias has none: its target is
    /// [`Self::base_type`].
    #[must_use]
    pub fn element_type(&self) -> Option<Self> {
        let node = self.node().ok()?;
        let reference = match &node.kind {
            AstKind::Sequence { element, .. } | AstKind::Array { element, .. } => element,
            AstKind::Map { value, .. } => value,
            AstKind::String { width, .. } => {
                return primitive_view(match width {
                    CharWidth::Char => AstPrimitive::Char,
                    CharWidth::WChar => AstPrimitive::WChar,
                })
                .ok();
            }
            AstKind::Bitmask { .. } => return primitive_view(AstPrimitive::Boolean).ok(),
            _ => return None,
        };
        self.follow(reference, node.name.as_str()).ok()
    }

    /// §7.5.2.4 `key_element_type` — a map's key.
    #[must_use]
    pub fn key_element_type(&self) -> Option<Self> {
        let node = self.node().ok()?;
        let AstKind::Map { key, .. } = &node.kind else {
            return None;
        };
        self.follow(key, node.name.as_str()).ok()
    }

    /// §7.5.2.4 `discriminator_type` — a union's discriminator.
    #[must_use]
    pub fn discriminator_type(&self) -> Option<Self> {
        let node = self.node().ok()?;
        let AstKind::Union { discriminator, .. } = &node.kind else {
            return None;
        };
        self.follow(discriminator, node.name.as_str()).ok()
    }

    /// The type's effective extensibility, for the kinds that have one.
    ///
    /// `None` where the kind has none — a primitive, a collection, an
    /// alias — and never a substituted value. What an unannotated
    /// aggregate is taken to be is
    /// [`zerodds_ast::Ast::effective_extensibility`]'s answer and is not
    /// decided again here.
    #[must_use]
    pub fn extensibility(&self) -> Option<ExtensibilityKind> {
        match self.kind() {
            TypeKind::Structure | TypeKind::Union | TypeKind::Annotation => {
                Some(self.ast.effective_extensibility(self.id).into())
            }
            _ => None,
        }
    }

    /// The `@bit_bound` of an enumeration or a bitmask.
    #[must_use]
    pub fn bit_bound(&self) -> Option<u16> {
        match self.kind() {
            TypeKind::Enumeration | TypeKind::Bitmask => {
                Some(self.ast.effective_bit_bound(self.id))
            }
            _ => None,
        }
    }

    /// Whether this type carries members a walk can enumerate.
    #[must_use]
    pub fn is_aggregable(&self) -> bool {
        self.kind().is_aggregable()
    }

    /// §7.5.2.8 `get_member_count()`.
    #[must_use]
    pub fn member_count(&self) -> u32 {
        u32::try_from(self.members().len()).unwrap_or(u32::MAX)
    }

    /// Every member, in declaration order. §7.5.2.8.
    ///
    /// A `struct`'s members, a `union`'s case members, an enumeration's
    /// enumerators, a bitmask's bits, a bitset's bitfields — §7.5.2.8's
    /// `get_member_by_index` walks all six, and so does this.
    ///
    /// A `struct` reports its **own** members and not its base's:
    /// §7.5.2.8 gives the base its own `DynamicType`, which is where an
    /// inherited member is reached. The wire walk expands the base
    /// (§7.2.4.4.8) and reads `all_members_of` for that; the reflective
    /// walk does not, because a caller that wants both can ask for both
    /// and a caller that wants one cannot un-ask.
    #[must_use]
    pub fn members(&self) -> Vec<DynamicTypeMember> {
        // An alias has the members of the type it ends at (Table 51), and
        // they belong to that type: an enumerator's own type is the
        // enumeration and not the alias.
        let owner = match self.resolve_alias() {
            Ok(target) => target,
            Err(_) => return Vec::new(),
        };
        owner
            .parts()
            .into_iter()
            .enumerate()
            .map(|(index, part)| DynamicTypeMember {
                ast: Arc::clone(&owner.ast),
                parent: owner.id,
                part,
                index: u32::try_from(index).unwrap_or(u32::MAX),
            })
            .collect()
    }

    fn parts(&self) -> Vec<Part> {
        let Ok(node) = self.node() else {
            return Vec::new();
        };
        match &node.kind {
            AstKind::Structure { members, .. } | AstKind::Valuetype { state: members, .. } => {
                members.iter().copied().map(Part::Member).collect()
            }
            AstKind::Union { cases, .. } => cases.iter().copied().map(Part::Case).collect(),
            AstKind::Enumeration { enumerators } => {
                enumerators.iter().copied().map(Part::Enumerator).collect()
            }
            AstKind::Bitmask { bits } => bits.iter().copied().map(Part::Bit).collect(),
            // Table 51: "No other type kinds are considered to have
            // members." An alias is [`Self::members`]'s business, which
            // asks the type it ends at. A bitset's bitfields are reached
            // through the model, where the wire walk reads them
            // (`zerodds_ast::Ast::all_bitfields_of`).
            _ => Vec::new(),
        }
    }

    /// §7.5.2.8 `get_member_by_index(index)`.
    #[must_use]
    pub fn member_by_index(&self, index: u32) -> Option<DynamicTypeMember> {
        self.members().into_iter().nth(index as usize)
    }

    /// §7.5.2.8 `get_member_by_name(name)`.
    #[must_use]
    pub fn member_by_name(&self, name: &str) -> Option<DynamicTypeMember> {
        self.members()
            .into_iter()
            .find(|member| member.name() == name)
    }

    /// §7.5.2.8 `get_member(MemberId)`.
    #[must_use]
    pub fn member_by_id(&self, id: MemberId) -> Option<DynamicTypeMember> {
        // §7.5.2.9.2: the `member` property maps the ids of an aggregated
        // type; for any other type it is empty. An enumerator or a bit has
        // `MEMBER_ID_INVALID` (§7.5.2.7.5) and is not found by it.
        if id == super::MEMBER_ID_INVALID {
            return None;
        }
        self.members().into_iter().find(|member| member.id() == id)
    }

    /// §7.5.2.8.9 `get_member(id)`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] — "If there is no member with the
    /// specified member ID, the operation shall fail with
    /// RETCODE_BAD_PARAMETER".
    pub fn get_member(&self, id: MemberId) -> Result<DynamicTypeMember, DynamicError> {
        self.member_by_id(id).ok_or_else(|| {
            DynamicError::bad_parameter(alloc::format!(
                "`{}` has no member with the id {id}",
                self.name()
            ))
        })
    }

    /// §7.5.2.8.11 `get_member_by_name(name)`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] when there is no member of that
    /// name.
    pub fn get_member_by_name(&self, name: &str) -> Result<DynamicTypeMember, DynamicError> {
        self.member_by_name(name).ok_or_else(|| {
            DynamicError::bad_parameter(alloc::format!(
                "`{}` has no member named `{name}`",
                self.name()
            ))
        })
    }

    /// §7.5.2.8.10 `get_member_by_index(index)`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] when `index` is not below the
    /// member count.
    pub fn get_member_by_index(&self, index: u32) -> Result<DynamicTypeMember, DynamicError> {
        self.member_by_index(index).ok_or_else(|| {
            DynamicError::bad_parameter(alloc::format!(
                "`{}` has {} members, none at index {index}",
                self.name(),
                self.member_count()
            ))
        })
    }

    /// Annex C `get_all_members_by_name`: the §7.5.2.8.3
    /// `member_by_name` property whole.
    #[must_use]
    pub fn all_members_by_name(&self) -> BTreeMap<String, DynamicTypeMember> {
        self.members()
            .into_iter()
            .map(|member| (member.name().to_string(), member))
            .collect()
    }

    /// Annex C `get_all_members` (named by id here, since `all_members`
    /// is the private walk of the equality): the §7.5.2.8.2 `member` property
    /// whole. Empty for a type that is not an aggregated type, whose
    /// members have no ids (§7.5.2.7.5).
    #[must_use]
    pub fn all_members_by_id(&self) -> BTreeMap<MemberId, DynamicTypeMember> {
        self.members()
            .into_iter()
            .filter(|member| member.id() != super::MEMBER_ID_INVALID)
            .map(|member| (member.id(), member))
            .collect()
    }

    /// §7.5.2.8.6 `get_annotation_count()`: the annotations applied to
    /// the type.
    #[must_use]
    pub fn annotation_count(&self) -> u32 {
        u32::try_from(self.annotations().len()).unwrap_or(u32::MAX)
    }

    /// §7.5.2.8.1 `annotation`: every annotation applied to the type —
    /// the built-in ones (`@final`, `@nested`, …) and the declared ones.
    #[must_use]
    pub fn annotations(&self) -> Vec<AnnotationDescriptor> {
        self.node().map_or_else(
            |_| Vec::new(),
            |node| super::annotation::annotations_of(&self.ast, &node.facets, &node.name),
        )
    }

    /// §7.5.2.8.5 `get_annotation(index)`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] when `index` is not below the
    /// annotation count.
    pub fn annotation(&self, index: u32) -> Result<AnnotationDescriptor, DynamicError> {
        self.annotations()
            .into_iter()
            .nth(index as usize)
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "`{}` has {} annotations, none at index {index}",
                    self.name(),
                    self.annotation_count()
                ))
            })
    }

    /// Annex C `get_verbatim_text_count()`.
    #[must_use]
    pub fn verbatim_text_count(&self) -> u32 {
        u32::try_from(self.verbatim_texts().len()).unwrap_or(u32::MAX)
    }

    /// Every `@verbatim` text of the type.
    #[must_use]
    pub fn verbatim_texts(&self) -> Vec<VerbatimTextDescriptor> {
        self.node().map_or_else(
            |_| Vec::new(),
            |node| super::annotation::verbatim_texts_of(&node.facets),
        )
    }

    /// Annex C `get_verbatim_text(inout descriptor, in idx)`.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] when `index` is not below the
    /// count.
    pub fn verbatim_text(&self, index: u32) -> Result<VerbatimTextDescriptor, DynamicError> {
        self.verbatim_texts()
            .into_iter()
            .nth(index as usize)
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "`{}` has {} verbatim texts, none at index {index}",
                    self.name(),
                    self.verbatim_text_count()
                ))
            })
    }

    /// Annex C `DynamicType::get_descriptor`.
    ///
    /// # Errors
    /// As [`Self::descriptor`].
    pub fn get_descriptor(&self) -> Result<TypeDescriptor, DynamicError> {
        self.descriptor()
    }

    /// Annex C `DynamicType::get_name`: "identical to the name that is a
    /// member of the descriptor property" (§7.5.2.8), the fully
    /// qualified name.
    #[must_use]
    pub fn get_name(&self) -> String {
        self.qualified_name()
    }

    /// Annex C `DynamicType::get_kind`.
    #[must_use]
    pub fn get_kind(&self) -> TypeKind {
        self.kind()
    }

    /// Annex C `DynamicType::get_all_members_by_name`.
    #[must_use]
    pub fn get_all_members_by_name(&self) -> BTreeMap<String, DynamicTypeMember> {
        self.all_members_by_name()
    }

    /// Annex C `DynamicType::get_all_members`.
    #[must_use]
    pub fn get_all_members(&self) -> BTreeMap<MemberId, DynamicTypeMember> {
        self.all_members_by_id()
    }

    /// Annex C `DynamicType::get_member_count`.
    #[must_use]
    pub fn get_member_count(&self) -> u32 {
        self.member_count()
    }

    /// Annex C `DynamicType::get_annotation_count`.
    #[must_use]
    pub fn get_annotation_count(&self) -> u32 {
        self.annotation_count()
    }

    /// Annex C `DynamicType::get_annotation`.
    ///
    /// # Errors
    /// As [`Self::annotation`].
    pub fn get_annotation(&self, index: u32) -> Result<AnnotationDescriptor, DynamicError> {
        self.annotation(index)
    }

    /// Annex C `DynamicType::get_verbatim_text_count`.
    #[must_use]
    pub fn get_verbatim_text_count(&self) -> u32 {
        self.verbatim_text_count()
    }

    /// Annex C `DynamicType::get_verbatim_text`.
    ///
    /// # Errors
    /// As [`Self::verbatim_text`].
    pub fn get_verbatim_text(&self, index: u32) -> Result<VerbatimTextDescriptor, DynamicError> {
        self.verbatim_text(index)
    }

    /// §7.5.2.8 `equals(other)` — deep equality.
    ///
    /// Structural, and terminating on a model whose references form a
    /// cycle: a pair already being compared compares equal, which is
    /// bisimulation and is the only answer that both terminates and does
    /// not call two different types equal. A pair that is *not* on the
    /// stack is compared in full.
    ///
    /// "Two types shall be considered equal if and only if all of their
    /// respective properties, as identified in Table 54 above, are
    /// equal" (§7.5.2.8.4), and the annotations applied to the type and to
    /// each member are among them (Tables 52 and 54). Two annotations are
    /// equal as [`AnnotationDescriptor::equals`] has it, and the lists are
    /// compared without regard to their order. The built-in annotations
    /// that only restate a property compared anyway (`@key`, `@id`,
    /// `@final`, `@optional`, `@bit_bound`, …) are not compared twice:
    /// a type that came through a `TypeObject` has those, spelled by what
    /// the object carries, and has no declared annotation at all, which
    /// is why [`Self::equals_ignoring_annotations`] exists.
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        let mut stack = Vec::new();
        self.equals_at(other, &mut stack, 0, true)
    }

    /// [`Self::equals`] without the annotations: the types have the same
    /// shape, names, bounds, extensibility and member properties. What a
    /// check that two types are *compatible* means, as the
    /// `TypeSupport` operations of Tables 63 to 65 have it.
    #[must_use]
    pub fn equals_ignoring_annotations(&self, other: &Self) -> bool {
        let mut stack = Vec::new();
        self.equals_at(other, &mut stack, 0, false)
    }

    fn equals_at(
        &self,
        other: &Self,
        stack: &mut Vec<(Id<TypeNode>, Id<TypeNode>)>,
        depth: usize,
        annotations: bool,
    ) -> bool {
        if Arc::ptr_eq(&self.ast, &other.ast) && self.id == other.id {
            return true;
        }
        if depth > MAX_DEPTH {
            return false;
        }
        if stack.contains(&(self.id, other.id)) {
            return true;
        }
        let (Ok(mine), Ok(theirs)) = (self.descriptor(), other.descriptor()) else {
            return false;
        };
        if mine.kind != theirs.kind
            || mine.name != theirs.name
            || mine.bound != theirs.bound
            || mine.extensibility_kind != theirs.extensibility_kind
            || mine.is_nested != theirs.is_nested
            || mine.bit_bound != theirs.bit_bound
        {
            return false;
        }
        stack.push((self.id, other.id));
        let outcome = (!annotations
            || (annotations_equal(&self.annotations(), &other.annotations())
                && self.verbatim_texts() == other.verbatim_texts()))
            && pair_equals(
                mine.base_type.as_ref(),
                theirs.base_type.as_ref(),
                stack,
                depth,
                annotations,
            )
            && pair_equals(
                mine.discriminator_type.as_ref(),
                theirs.discriminator_type.as_ref(),
                stack,
                depth,
                annotations,
            )
            && pair_equals(
                mine.element_type.as_ref(),
                theirs.element_type.as_ref(),
                stack,
                depth,
                annotations,
            )
            && pair_equals(
                mine.key_element_type.as_ref(),
                theirs.key_element_type.as_ref(),
                stack,
                depth,
                annotations,
            )
            && self.members_equal(other, stack, depth, annotations);
        stack.pop();
        outcome
    }

    fn members_equal(
        &self,
        other: &Self,
        stack: &mut Vec<(Id<TypeNode>, Id<TypeNode>)>,
        depth: usize,
        annotations: bool,
    ) -> bool {
        let (mine, theirs) = (self.members(), other.members());
        if mine.len() != theirs.len() {
            return false;
        }
        mine.iter()
            .zip(theirs.iter())
            .all(|(a, b)| a.equals_at(b, stack, depth + 1, annotations))
    }

    /// Whether the type is consistent, itself and in every member.
    /// §7.5.2.8.
    ///
    /// # Errors
    /// [`DynamicError::Inconsistent`], naming what is wrong.
    pub fn is_consistent(&self) -> Result<(), DynamicError> {
        self.descriptor()?.is_consistent()?;
        for member in self.members() {
            member.descriptor()?.is_consistent()?;
        }
        Ok(())
    }
}

/// A primitive type in a model of its own.
///
/// The element type of a string (Char8, Char16) and of a bitmask
/// (Boolean), and the type of a bitmask's flags, are types the model the
/// string or bitmask was declared in need not hold a node for. The view
/// is on a one-node model, which compares equal to the same primitive
/// anywhere ([`DynamicType::equals`] is structural).
fn primitive_view(kind: AstPrimitive) -> Result<DynamicType, DynamicError> {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let id = ast
        .add_type(root, kind.as_str(), AstKind::Primitive(kind))
        .map_err(|error| DynamicError::model(&error))?;
    Ok(DynamicType::new(Arc::new(ast), id))
}

fn pair_equals(
    left: Option<&DynamicType>,
    right: Option<&DynamicType>,
    stack: &mut Vec<(Id<TypeNode>, Id<TypeNode>)>,
    depth: usize,
    annotations: bool,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(a), Some(b)) => a.equals_at(b, stack, depth + 1, annotations),
        _ => false,
    }
}

/// The built-in annotations that restate a property the type and member
/// comparisons look at in any case.
fn restates_a_property(descriptor: &AnnotationDescriptor) -> bool {
    descriptor.annotation_type().is_some_and(|type_| {
        type_.is_builtin()
            && matches!(
                type_.name(),
                "key"
                    | "id"
                    | "hashid"
                    | "autoid"
                    | "optional"
                    | "external"
                    | "must_understand"
                    | "try_construct"
                    | "default"
                    | "final"
                    | "appendable"
                    | "mutable"
                    | "extensibility"
                    | "nested"
                    | "bit_bound"
            )
    })
}

/// Whether two lists of annotations hold equal ones, in any order.
fn annotations_equal(a: &[AnnotationDescriptor], b: &[AnnotationDescriptor]) -> bool {
    let a: Vec<&AnnotationDescriptor> = a.iter().filter(|d| !restates_a_property(d)).collect();
    let b: Vec<&AnnotationDescriptor> = b.iter().filter(|d| !restates_a_property(d)).collect();
    if a.len() != b.len() {
        return false;
    }
    let mut taken = alloc::vec![false; b.len()];
    a.iter().all(|left| {
        let found = b
            .iter()
            .enumerate()
            .find(|(index, right)| !taken[*index] && left.equals(right));
        match found {
            Some((index, _)) => {
                taken[index] = true;
                true
            }
            None => false,
        }
    })
}

/// The type's own name and kind, and nothing of the model behind it.
///
/// Written by hand because [`Ast`] is not `Debug` — it is an arena of
/// several hundred nodes and printing one would bury whatever was being
/// debugged. What a reader of a `{:?}` wants here is which type this is.
impl core::fmt::Debug for DynamicType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "DynamicType({} {})",
            self.kind().as_str(),
            self.qualified_name()
        )
    }
}

impl PartialEq for DynamicType {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl Eq for DynamicType {}

/// Which part of an aggregate a member view stands for.
///
/// A `union`'s members are its cases, an enumeration's are its
/// enumerators and a bitmask's are its bits (Table 51). §7.5.2.8 walks
/// all of them through one accessor, so one member type covers all of
/// them — and the four are kept apart here rather than flattened,
/// because what a member's id and type *are* differs between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Member(Id<MemberNode>),
    Case(Id<CaseNode>),
    Enumerator(Id<EnumeratorNode>),
    Bit(Id<BitValueNode>),
}

/// One member of a type. §7.5.2.8.
#[derive(Clone)]
pub struct DynamicTypeMember {
    ast: Arc<Ast>,
    parent: Id<TypeNode>,
    part: Part,
    index: u32,
}

impl DynamicTypeMember {
    /// The member node behind a struct member or a union case.
    fn member_node(&self) -> Option<&MemberNode> {
        match self.part {
            Part::Member(id) => self.ast.member(id).ok(),
            Part::Case(id) => self
                .ast
                .case(id)
                .ok()
                .and_then(|case| self.ast.member(case.member).ok()),
            _ => None,
        }
    }

    /// §7.5.2.1 `name`.
    #[must_use]
    pub fn name(&self) -> &str {
        match self.part {
            Part::Member(_) | Part::Case(_) => self
                .member_node()
                .map_or("<unknown member>", |node| node.name.as_str()),
            Part::Enumerator(id) => self
                .ast
                .enumerator(id)
                .map_or("<unknown enumerator>", |node| node.name.as_str()),
            Part::Bit(id) => self
                .ast
                .bit_value(id)
                .map_or("<unknown bit>", |node| node.name.as_str()),
        }
    }

    /// §7.5.2.6.4 `get_id` — "identical to the ID value that is a member
    /// of the descriptor property".
    ///
    /// A `struct` member or a `union` case member has the **effective**
    /// member id the frontend assigned — `@id`, `@hashid`, `@autoid` or the
    /// previous member plus one — which is what
    /// `CommonStructMember::member_id` writes; a member with none falls back
    /// to its index, as [`crate::from_ast`] does. For an enumerator or a
    /// bit, which do not belong to an aggregated type, §7.5.2.7.5 requires
    /// [`super::MEMBER_ID_INVALID`]; the value or the position they carry
    /// is [`Self::literal_value`].
    #[must_use]
    pub fn id(&self) -> MemberId {
        match self.part {
            Part::Member(_) | Part::Case(_) => self
                .member_node()
                .and_then(|node| {
                    node.facets
                        .xtypes
                        .as_ref()
                        .and_then(zerodds_ast::XTypesFacet::effective_member_id)
                })
                .unwrap_or(self.index),
            Part::Enumerator(_) | Part::Bit(_) => super::MEMBER_ID_INVALID,
        }
    }

    /// What a literal stands for: an enumerator's effective value (what
    /// `CommonEnumeratedLiteral::value` writes) or a bit's position (what
    /// `CommonBitflag::position` writes). `None` for a member of a
    /// `struct` or a `union`, and for a value the model cannot give.
    ///
    /// Not a property of §7.5.2.7's `MemberDescriptor`, whose `id` is
    /// `MEMBER_ID_INVALID` there; it is what a caller walking an
    /// enumeration or a bitmask needs to know which value a literal is.
    #[must_use]
    pub fn literal_value(&self) -> Option<u32> {
        match self.part {
            Part::Member(_) | Part::Case(_) => None,
            Part::Enumerator(id) => self
                .ast
                .effective_enumerator_value(id)
                .and_then(|value| u32::try_from(value).ok()),
            Part::Bit(id) => self.ast.effective_bit_position(id).map(u32::from),
        }
    }

    /// The facets of the node behind this member.
    fn facets(&self) -> Option<&zerodds_ast::Facets> {
        match self.part {
            Part::Member(_) | Part::Case(_) => self.member_node().map(|node| &node.facets),
            Part::Enumerator(id) => self.ast.enumerator(id).ok().map(|node| &node.facets),
            Part::Bit(id) => self.ast.bit_value(id).ok().map(|node| &node.facets),
        }
    }

    /// §7.5.2.6.1 `annotation`: every annotation applied to the member —
    /// the built-in ones (`@key`, `@optional`, `@id`, …) and the declared
    /// ones.
    #[must_use]
    pub fn annotations(&self) -> Vec<AnnotationDescriptor> {
        self.facets().map_or_else(Vec::new, |facets| {
            super::annotation::annotations_of(&self.ast, facets, self.name())
        })
    }

    /// Annex C `get_annotation_count()` of a member.
    #[must_use]
    pub fn annotation_count(&self) -> u32 {
        u32::try_from(self.annotations().len()).unwrap_or(u32::MAX)
    }

    /// Annex C `get_annotation(inout descriptor, in idx)` of a member.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] when `index` is not below the
    /// annotation count.
    pub fn annotation(&self, index: u32) -> Result<AnnotationDescriptor, DynamicError> {
        self.annotations()
            .into_iter()
            .nth(index as usize)
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "`{}` has {} annotations, none at index {index}",
                    self.name(),
                    self.annotation_count()
                ))
            })
    }

    /// Every `@verbatim` text of the member.
    #[must_use]
    pub fn verbatim_texts(&self) -> Vec<VerbatimTextDescriptor> {
        self.facets()
            .map_or_else(Vec::new, super::annotation::verbatim_texts_of)
    }

    /// Annex C `get_verbatim_text_count()` of a member.
    #[must_use]
    pub fn verbatim_text_count(&self) -> u32 {
        u32::try_from(self.verbatim_texts().len()).unwrap_or(u32::MAX)
    }

    /// Annex C `get_verbatim_text(inout descriptor, in idx)` of a member.
    ///
    /// # Errors
    /// [`DynamicError::BadParameter`] when `index` is not below the
    /// count.
    pub fn verbatim_text(&self, index: u32) -> Result<VerbatimTextDescriptor, DynamicError> {
        self.verbatim_texts()
            .into_iter()
            .nth(index as usize)
            .ok_or_else(|| {
                DynamicError::bad_parameter(alloc::format!(
                    "`{}` has {} verbatim texts, none at index {index}",
                    self.name(),
                    self.verbatim_text_count()
                ))
            })
    }

    /// Annex C `DynamicTypeMember::get_id`.
    #[must_use]
    pub fn get_id(&self) -> MemberId {
        self.id()
    }

    /// Annex C `DynamicTypeMember::get_name`.
    #[must_use]
    pub fn get_name(&self) -> String {
        self.name().to_string()
    }

    /// Annex C `DynamicTypeMember::get_descriptor`.
    ///
    /// # Errors
    /// As [`Self::descriptor`].
    pub fn get_descriptor(&self) -> Result<MemberDescriptor, DynamicError> {
        self.descriptor()
    }

    /// Annex C `DynamicTypeMember::get_annotation_count`.
    #[must_use]
    pub fn get_annotation_count(&self) -> u32 {
        self.annotation_count()
    }

    /// Annex C `DynamicTypeMember::get_annotation`.
    ///
    /// # Errors
    /// As [`Self::annotation`].
    pub fn get_annotation(&self, index: u32) -> Result<AnnotationDescriptor, DynamicError> {
        self.annotation(index)
    }

    /// Annex C `DynamicTypeMember::get_verbatim_text_count`.
    #[must_use]
    pub fn get_verbatim_text_count(&self) -> u32 {
        self.verbatim_text_count()
    }

    /// Annex C `DynamicTypeMember::get_verbatim_text`.
    ///
    /// # Errors
    /// As [`Self::verbatim_text`].
    pub fn get_verbatim_text(&self, index: u32) -> Result<VerbatimTextDescriptor, DynamicError> {
        self.verbatim_text(index)
    }

    /// §7.5.2.1 `index` — the position in declaration order.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// The member's own type. §7.5.2.8.
    ///
    /// For a `struct` member or a `union` case member it is the declared
    /// type. For an enumerator it is the **containing type**: a literal
    /// of an enumeration is a value of that enumeration (Table 51). For a
    /// bitmask bit it is Boolean (Table 51).
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`] for a reference the resolver did not
    /// close, and for a bitset field whose default holder is not in the
    /// model.
    pub fn dynamic_type(&self) -> Result<DynamicType, DynamicError> {
        let containing = DynamicType::new(Arc::clone(&self.ast), self.parent);
        match self.part {
            Part::Member(_) | Part::Case(_) => {
                let node = self.member_node().ok_or_else(|| DynamicError::Unresolved {
                    at: self.name().to_string(),
                })?;
                let name = node.name.clone();
                let id = node
                    .type_
                    .resolved()
                    .ok_or(DynamicError::Unresolved { at: name.clone() })?;
                if self.ast.type_(id).is_err() {
                    return Err(DynamicError::Unresolved { at: name });
                }
                Ok(DynamicType::new(Arc::clone(&self.ast), id))
            }
            Part::Enumerator(_) => Ok(containing),
            // Table 51: "Each named flag in a bitmask shall be considered
            // to be a 'member' of that bitmask with Boolean type."
            Part::Bit(_) => primitive_view(AstPrimitive::Boolean),
        }
    }

    /// The value this member takes when nothing sets it. §7.2.2.4.4.4.9:
    /// its own `@default` where the declaration carries one, or its
    /// type's Table 9 default (`zerodds_wire_codec::default::of_type`)
    /// otherwise — the same two-tier rule
    /// [`zerodds_wire_codec::default::member`] applies, through it.
    ///
    /// A struct member and a union case member are the two kinds
    /// §7.3.4.5.4 gives a `@default` to. An enumerator, a bitmask bit or
    /// a bitset field carries none of its own, so this falls straight
    /// through to the type's default for those.
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`] for a reference the resolver did not
    /// close; [`DynamicError::Inconsistent`] where the value this member
    /// would take is one [`zerodds_wire_codec`]'s own walk cannot name —
    /// wrapped, since the two crates read one model but do not share an
    /// error type.
    pub fn default_value<V>(&self) -> Result<V, DynamicError>
    where
        V: zerodds_wire_codec::Source + zerodds_wire_codec::Sink,
    {
        let type_ = self.dynamic_type()?;
        let wrap = |error: zerodds_wire_codec::ProbeError| DynamicError::Inconsistent {
            detail: error.to_string(),
        };
        match self.member_node() {
            Some(node) => {
                zerodds_wire_codec::default::member(&self.ast, node, type_.id(), self.name())
                    .map_err(wrap)
            }
            None => zerodds_wire_codec::default::of_type(&self.ast, type_.id(), self.name())
                .map_err(wrap),
        }
    }

    /// §7.5.2 `get_descriptor()`.
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`].
    pub fn descriptor(&self) -> Result<MemberDescriptor, DynamicError> {
        let type_ = self.dynamic_type()?;
        let node = self.member_node();
        let facet = node.and_then(|node| node.facets.xtypes.as_ref());
        let (label, is_default_label) = self.labels()?;
        Ok(MemberDescriptor {
            name: self.name().to_string(),
            id: self.id(),
            type_,
            index: self.index,
            label,
            is_default_label,
            try_construct_kind: node.map_or(TryConstructKind::Discard, |node| {
                node.facets
                    .xtypes
                    .as_ref()
                    .and_then(|x| x.try_construct)
                    .unwrap_or(zerodds_ast::TryConstruct::ABSENT)
                    .into()
            }),
            is_key: facet.is_some_and(|x| x.is_key),
            is_optional: facet.is_some_and(|x| x.is_optional),
            // §7.2.2.4.4.4.8 makes a key member must-understand whatever
            // the declaration said, and `crate::from_ast::member_bits`
            // sets the bit for that reason. Reported the same way here,
            // so that the descriptor and the `TypeObject` a peer receives
            // say the same thing about the same member.
            is_must_understand: facet.is_some_and(|x| x.is_must_understand || x.is_key),
            is_shared: facet.is_some_and(|x| x.is_external),
            has_default_value: facet.is_some_and(|x| x.default_value.is_some()),
            default_value: match self.part {
                // Not a default: the value of an enumerator and the
                // position of a flag, where `TypeBuilder::add_member`
                // reads them back (the descriptor of a literal has no
                // other place for them, its id being MEMBER_ID_INVALID).
                Part::Enumerator(_) | Part::Bit(_) => self
                    .literal_value()
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                Part::Member(_) | Part::Case(_) => facet
                    .and_then(|x| x.default_value)
                    .map(|expr| {
                        super::default_literal::default_expr_text(&self.ast, expr, self.name())
                    })
                    .transpose()?
                    .unwrap_or_default(),
            },
        })
    }

    /// The case labels that select this member, and whether `default:`
    /// does.
    ///
    /// Table 53 types a label `Int64`, so a `long long` discriminator's
    /// labels are kept. A label the model cannot evaluate is an error
    /// rather than a label that is missing from the descriptor.
    fn labels(&self) -> Result<(Vec<i64>, bool), DynamicError> {
        let Part::Case(id) = self.part else {
            return Ok((Vec::new(), false));
        };
        let case = self.ast.case(id).map_err(|_| DynamicError::Unresolved {
            at: self.name().to_string(),
        })?;
        let mut out = Vec::new();
        let mut is_default = false;
        for label in &case.labels {
            match label.expr() {
                Some(expr) => {
                    let value =
                        self.ast
                            .eval_integral(expr)
                            .map_err(|_| DynamicError::Unresolved {
                                at: alloc::format!(
                                    "{}: a case label that does not evaluate",
                                    self.name()
                                ),
                            })?;
                    out.push(i64::try_from(value).map_err(|_| DynamicError::Unresolved {
                        at: alloc::format!("{}: a case label past an Int64", self.name()),
                    })?);
                }
                None => is_default = true,
            }
        }
        Ok((out, is_default))
    }

    /// §7.5.2.8 `equals(other)` — deep equality.
    ///
    /// Equal members have equal annotations (Table 52, as for types).
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        let mut stack = Vec::new();
        self.equals_at(other, &mut stack, 0, true)
    }

    fn equals_at(
        &self,
        other: &Self,
        stack: &mut Vec<(Id<TypeNode>, Id<TypeNode>)>,
        depth: usize,
        annotations: bool,
    ) -> bool {
        // §7.5.2.6.3: "Two members shall be considered equal if and only
        // if they belong to the same type". The owners are compared as
        // types are (`DynamicType::equals_at`); when this runs inside the
        // comparison of those very types the pair is on `stack` and
        // compares equal, which ends the recursion.
        let owner = DynamicType::new(Arc::clone(&self.ast), self.parent);
        let other_owner = DynamicType::new(Arc::clone(&other.ast), other.parent);
        if !owner.equals_at(&other_owner, stack, depth + 1, annotations) {
            return false;
        }
        if annotations
            && !(annotations_equal(&self.annotations(), &other.annotations())
                && self.verbatim_texts() == other.verbatim_texts())
        {
            return false;
        }
        if self.name() != other.name()
            || self.id() != other.id()
            || self.index != other.index
            || self.labels().ok() != other.labels().ok()
        {
            return false;
        }
        let (Ok(mine), Ok(theirs)) = (self.descriptor(), other.descriptor()) else {
            return false;
        };
        if mine.try_construct_kind != theirs.try_construct_kind
            || mine.is_key != theirs.is_key
            || mine.is_optional != theirs.is_optional
            || mine.is_must_understand != theirs.is_must_understand
            || mine.is_shared != theirs.is_shared
            || mine.has_default_value != theirs.has_default_value
            || mine.default_value != theirs.default_value
        {
            return false;
        }
        mine.type_
            .equals_at(&theirs.type_, stack, depth, annotations)
    }
}

/// As [`DynamicType`]'s: the member's name and id, not the model.
impl core::fmt::Debug for DynamicTypeMember {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "DynamicTypeMember({} id {} index {})",
            self.name(),
            self.id(),
            self.index
        )
    }
}

impl PartialEq for DynamicTypeMember {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl Eq for DynamicTypeMember {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::dynamic::builder::{DynamicTypeBuilder, NewMember};
    use crate::dynamic::descriptor::ExtensibilityKind;

    /// A model with one `struct` of two members, the second keyed.
    fn sample() -> (Arc<Ast>, Id<TypeNode>) {
        let mut builder = DynamicTypeBuilder::new();
        let long = builder.primitive(TypeKind::Int32).expect("a long");
        let text = builder.string(false, None).expect("a string");
        let structure = builder
            .structure("Sample", ExtensibilityKind::Mutable)
            .expect("a struct");
        builder
            .add_member(structure, &NewMember::new("id", long).with_id(7).keyed())
            .expect("a member");
        builder
            .add_member(structure, &NewMember::new("name", text).optional())
            .expect("a member");
        (builder.finish(), structure)
    }

    #[test]
    fn a_view_reports_what_the_model_holds() {
        let (ast, id) = sample();
        let type_ = DynamicType::new(ast, id);
        assert_eq!(type_.kind(), TypeKind::Structure);
        assert_eq!(type_.name(), "Sample");
        assert_eq!(type_.member_count(), 2);
        assert_eq!(type_.extensibility(), Some(ExtensibilityKind::Mutable));

        let keyed = type_.member_by_name("id").expect("the member");
        assert_eq!(keyed.id(), 7);
        assert_eq!(keyed.index(), 0);
        let descriptor = keyed.descriptor().expect("a descriptor");
        assert!(descriptor.is_key);
        // §7.2.2.4.4.4.8: a key member is must-understand whatever the
        // declaration said, and the descriptor says the same thing the
        // `TypeObject` does.
        assert!(descriptor.is_must_understand);
        assert_eq!(descriptor.type_.kind(), TypeKind::Int32);

        // §7.5.2.7.5: a member added with no id gets one more than the
        // maximum in the type, which is 7 + 1.
        let optional = type_.member_by_id(8).expect("the second member");
        assert_eq!(optional.name(), "name");
        assert!(optional.descriptor().expect("a descriptor").is_optional);
    }

    /// The same model twice is the same type; a different member id is
    /// not.
    #[test]
    fn equality_is_structural_and_not_by_pointer() {
        let (one, id) = sample();
        let (other, other_id) = sample();
        assert!(DynamicType::new(Arc::clone(&one), id).equals(&DynamicType::new(other, other_id)));

        let mut builder = DynamicTypeBuilder::new();
        let long = builder.primitive(TypeKind::Int32).expect("a long");
        let text = builder.string(false, None).expect("a string");
        let structure = builder
            .structure("Sample", ExtensibilityKind::Mutable)
            .expect("a struct");
        builder
            .add_member(structure, &NewMember::new("id", long).with_id(8).keyed())
            .expect("a member");
        builder
            .add_member(structure, &NewMember::new("name", text).optional())
            .expect("a member");
        let renumbered = DynamicType::new(builder.finish(), structure);
        assert!(!DynamicType::new(one, id).equals(&renumbered));
    }

    /// An alias chain ends at the type it names, and a `sequence`'s
    /// element is reachable without touching the model.
    #[test]
    fn a_view_follows_an_alias_and_an_element() {
        let mut builder = DynamicTypeBuilder::new();
        let long = builder.primitive(TypeKind::Int32).expect("a long");
        let sequence = builder.sequence(long, Some(4)).expect("a sequence");
        let first = builder.alias("Longs", sequence).expect("an alias");
        let second = builder.alias("MoreLongs", first).expect("an alias");
        let ast = builder.finish();

        let view = DynamicType::new(Arc::clone(&ast), second);
        assert_eq!(view.kind(), TypeKind::Alias);
        let target = view.resolve_alias().expect("the chain ends");
        assert_eq!(target.kind(), TypeKind::Sequence);
        assert_eq!(
            target.element_type().expect("an element").kind(),
            TypeKind::Int32
        );
        assert_eq!(
            target.descriptor().expect("a descriptor").bound,
            alloc::vec![4]
        );
    }
}
