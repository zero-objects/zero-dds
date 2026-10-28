// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Building a type at run time. XTypes 1.3 §7.5.2.
//!
//! A schema that arrives at run time — a `.proto` descriptor set, an XML
//! type description, a `.msg` file — becomes a type here, and the type it
//! becomes is a node in a [`zerodds_ast::Ast`]. Not a parallel structure
//! that is later bridged into one: the model **is** the representation,
//! so a type built here is the same kind of thing as a type lowered from
//! IDL and reaches the wire through the same planner.
//!
//! ## Why this is a factory over one model and not a builder per type
//!
//! §7.5.2 shapes the API as `create_type(descriptor) -> builder`,
//! `builder.add_member(descriptor)`, `builder.build() -> DynamicType`. A
//! [`super::DynamicType`] here is a view on a **shared, finished** model,
//! and a model that is still being written to cannot hand one out: either
//! every accessor takes a lock, or `build()` copies the model and two
//! types built in sequence end up in two different models — where a
//! member of one cannot name the other.
//!
//! So the shape is: one factory owning one model, every creation
//! answering a [`zerodds_ast_core::Id`], and [`DynamicTypeBuilder::finish`]
//! handing over the whole model at the end. Types built in one factory
//! can name each other, which is the thing a schema translation needs and
//! the thing a per-type `build()` cannot give.
//!
//! ## Member ids
//!
//! A member gets the id the caller states. One with none gets "one more
//! than the current maximum member ID in the type" (§7.5.2.7.5) — unless
//! the type is `@autoid(HASH)`, whose ids are the model's to derive from
//! the names (§7.3.1.2.1.1).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_ast::{
    Ast, AutoId, Bound, CaseLabel, CharWidth, ConstExprNode, MemberIdAssignment, MemberIdOrigin,
    MemberSpec, ModuleNode, TryConstruct, TypeKind as AstKind, TypeNode,
};

/// `LENGTH_UNLIMITED` as the `UInt32` bound of `create_string_type`,
/// `create_sequence_type` and `create_map_type` (§7.5.2.2): "If this
/// argument is equal to LENGTH_UNLIMITED, the \[type\] shall be considered to
/// be unbounded." DCPS has the constant as `-1`, which is this value as an
/// unsigned integer.
pub const LENGTH_UNLIMITED: u32 = u32::MAX;
use zerodds_ast_core::{Id, Ref};

use super::descriptor::{ExtensibilityKind, MemberId, TryConstructKind, TypeKind};
use super::error::DynamicError;
use super::type_::DynamicType;

/// What a member is declared with. §7.5.2.7 (Table 53), as an argument.
///
/// A [`super::MemberDescriptor`] is what comes *out* of a type and holds
/// a [`DynamicType`] — a view on a finished model, which is exactly what
/// does not exist yet while one is being built. This is what goes *in*,
/// and names the member's type by the handle the factory just answered
/// with.
#[derive(Debug, Clone)]
pub struct NewMember {
    /// The member's name.
    pub name: String,
    /// The member's type.
    pub type_: Id<TypeNode>,
    /// `@id(n)`, where the caller states one.
    ///
    /// `None` leaves the member unnumbered, and
    /// [`zerodds_ast::Ast`]'s rule — the previous member plus one —
    /// applies as it does to a member lowered from IDL. A builder that
    /// filled this in itself would be a second answer to §7.3.1.2.1.1.
    pub id: Option<MemberId>,
    /// `@key`.
    pub is_key: bool,
    /// `@optional`.
    pub is_optional: bool,
    /// `@must_understand`.
    pub is_must_understand: bool,
    /// `@external`.
    pub is_external: bool,
    /// `@try_construct`, where the caller states one. `None` leaves the
    /// annotation absent, which §7.2.2.7 reads as DISCARD.
    pub try_construct: Option<TryConstructKind>,
    /// `@default("text")`, where the caller states one. `None` leaves the
    /// member with no explicit default, and
    /// [`zerodds_wire_codec::default::member`] falls through to the
    /// type's own — Table 9.
    ///
    /// Parsed against the member's own type by
    /// [`DynamicTypeBuilder::add_member`] or
    /// [`DynamicTypeBuilder::add_case`] — not here, and not eagerly:
    /// building the model's expression node needs the model itself,
    /// which a member declared in isolation does not have.
    pub default: Option<String>,
}

impl NewMember {
    /// A member of that name and type, with no annotation.
    #[must_use]
    pub fn new(name: &str, type_: Id<TypeNode>) -> Self {
        Self {
            name: name.to_string(),
            type_,
            id: None,
            is_key: false,
            is_optional: false,
            is_must_understand: false,
            is_external: false,
            try_construct: None,
            default: None,
        }
    }

    /// The same member with an explicit `@id(n)`.
    #[must_use]
    pub const fn with_id(mut self, id: MemberId) -> Self {
        self.id = Some(id);
        self
    }

    /// The same member with `@key`.
    ///
    /// §7.2.2.4.4.4.8 makes a key member must-understand, so this sets
    /// both. Setting only the key bit would put a type on the wire whose
    /// `TypeObject` says something the descriptor does not.
    #[must_use]
    pub const fn keyed(mut self) -> Self {
        self.is_key = true;
        self.is_must_understand = true;
        self
    }

    /// The same member with `@optional`.
    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.is_optional = true;
        self
    }

    /// The same member with an explicit `@default("text")`.
    ///
    /// `text` is read the same way [`super::to_ast`] reads one off a
    /// peer's `TypeObject` — parsed against this member's own type when
    /// it is added to a `struct` or a `union`, and refused rather than
    /// dropped where it does not read in that type. §7.2.2.4.4.4.9 does
    /// not allow one on an `@optional` member; [`Self::optional`] and
    /// this can both be called, but the combination is refused when the
    /// member is added, not here.
    #[must_use]
    pub fn with_default(mut self, text: &str) -> Self {
        self.default = Some(text.to_string());
        self
    }

    /// The member's [`MemberSpec`], with its `@default` — if it carries
    /// one — parsed against `self.type_` and added to `ast`.
    ///
    /// `at` names the member for a refusal's detail; the caller composes
    /// it, because only the caller knows the aggregate the member is
    /// joining.
    ///
    /// # Errors
    /// [`DynamicError::Inconsistent`] where the member is `@optional` and
    /// carries a `@default` (§7.2.2.4.4.4.9), or where the default text
    /// does not read in the member's type.
    fn spec(&self, ast: &mut Ast, at: &str) -> Result<MemberSpec, DynamicError> {
        let mut spec = MemberSpec::new(&self.name, Ref::Resolved(self.type_));
        let facet = spec.facets.xtypes_mut();
        facet.is_key = self.is_key;
        facet.is_optional = self.is_optional;
        facet.is_must_understand = self.is_must_understand;
        facet.is_external = self.is_external;
        facet.try_construct = self.try_construct.map(TryConstruct::from);
        if let Some(id) = self.id {
            facet.member_id = Some(MemberIdAssignment {
                effective: id,
                origin: MemberIdOrigin::Declared,
            });
        }
        if let Some(text) = &self.default {
            if self.is_optional {
                return Err(DynamicError::inconsistent(format!(
                    "`{at}` is @optional and carries a @default, which §7.2.2.4.4.4.9 \
                     does not allow"
                )));
            }
            facet.default_value = Some(super::default_literal::default_expr(
                ast, text, self.type_, at,
            )?);
        }
        Ok(spec)
    }
}

/// One model, and the types being built in it. §7.5.2.
///
/// Every creation answers a handle into this model; [`Self::finish`]
/// hands the model over as the shared, immutable thing a
/// [`DynamicType`] is a view on.
pub struct DynamicTypeBuilder {
    pub(super) ast: Ast,
}

impl Default for DynamicTypeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicTypeBuilder {
    /// An empty model.
    #[must_use]
    pub fn new() -> Self {
        Self { ast: Ast::new() }
    }

    /// Build into a model that already holds types.
    ///
    /// For a caller that lowered some IDL and wants to add to it — a
    /// bridge that maps a foreign schema onto types a `.idl` file already
    /// declared, for instance.
    #[must_use]
    pub const fn over(ast: Ast) -> Self {
        Self { ast }
    }

    /// The model as it stands.
    #[must_use]
    pub const fn model(&self) -> &Ast {
        &self.ast
    }

    /// The finished model, shared.
    ///
    /// Every handle taken before this call is still valid: the arenas are
    /// append-only and the ids are stable.
    #[must_use]
    pub fn finish(self) -> Arc<Ast> {
        Arc::new(self.ast)
    }

    /// The finished model and a view on one of its types.
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`] where the handle is not this model's.
    pub fn finish_as(self, type_: Id<TypeNode>) -> Result<DynamicType, DynamicError> {
        if self.ast.type_(type_).is_err() {
            return Err(DynamicError::Unresolved {
                at: String::from("a type handle that is not this model's"),
            });
        }
        Ok(DynamicType::new(self.finish(), type_))
    }

    // -- fully descriptive types --------------------------------------

    /// A primitive.
    ///
    /// # Errors
    /// [`DynamicError::Unrepresentable`] for a [`TypeKind`] that is not a
    /// primitive — `TypeKind::Structure` here is a caller asking the
    /// wrong question, and answering it with `long` would be the silent
    /// part.
    pub fn primitive(&mut self, kind: TypeKind) -> Result<Id<TypeNode>, DynamicError> {
        let primitive = kind.as_primitive().ok_or(DynamicError::Unrepresentable {
            what: kind.as_str().to_string(),
            kind: "a primitive kind, which this is not",
        })?;
        self.anonymous(primitive.as_str(), AstKind::Primitive(primitive))
    }

    /// A `string<N>` or a `wstring<N>`. `None`, and [`LENGTH_UNLIMITED`],
    /// are unbounded.
    ///
    /// # Errors
    /// [`DynamicError::Model`] where the model rejects the edit.
    pub fn string(&mut self, wide: bool, bound: Option<u32>) -> Result<Id<TypeNode>, DynamicError> {
        let width = if wide {
            CharWidth::WChar
        } else {
            CharWidth::Char
        };
        let bound = self.bound(bound)?;
        let name = match (wide, bound) {
            (false, Bound::Unbounded) => "string",
            (true, Bound::Unbounded) => "wstring",
            (false, Bound::Const(_)) => "bounded string",
            (true, Bound::Const(_)) => "bounded wstring",
        };
        self.anonymous(name, AstKind::String { width, bound })
    }

    /// A `sequence<T, N>`. `None`, and [`LENGTH_UNLIMITED`], are unbounded.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn sequence(
        &mut self,
        element: Id<TypeNode>,
        bound: Option<u32>,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let bound = self.bound(bound)?;
        self.anonymous(
            "sequence",
            AstKind::Sequence {
                element: Ref::Resolved(element),
                bound,
            },
        )
    }

    /// A `T[d1][d2]…`, outermost dimension first.
    ///
    /// # Errors
    /// [`DynamicError::Inconsistent`] for an array with no dimensions,
    /// which §7.5.2.4 does not admit; [`DynamicError::Model`].
    pub fn array(
        &mut self,
        element: Id<TypeNode>,
        dimensions: &[u32],
    ) -> Result<Id<TypeNode>, DynamicError> {
        if dimensions.is_empty() {
            return Err(DynamicError::inconsistent(
                "an array with no dimensions, which §7.5.2.4 does not admit",
            ));
        }
        let mut dims = Vec::with_capacity(dimensions.len());
        for dimension in dimensions {
            dims.push(self.int_expr(i128::from(*dimension))?);
        }
        self.anonymous(
            "array",
            AstKind::Array {
                element: Ref::Resolved(element),
                dims,
            },
        )
    }

    /// A `map<K, V, N>`. `None`, and [`LENGTH_UNLIMITED`], are unbounded.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn map(
        &mut self,
        key: Id<TypeNode>,
        value: Id<TypeNode>,
        bound: Option<u32>,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let bound = self.bound(bound)?;
        self.anonymous(
            "map",
            AstKind::Map {
                key: Ref::Resolved(key),
                value: Ref::Resolved(value),
                bound,
            },
        )
    }

    // -- named types ---------------------------------------------------

    /// A `typedef`.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn alias(
        &mut self,
        name: &str,
        target: Id<TypeNode>,
    ) -> Result<Id<TypeNode>, DynamicError> {
        self.declare(
            name,
            AstKind::Alias {
                aliased: Ref::Resolved(target),
            },
        )
    }

    /// A `struct`.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn structure(
        &mut self,
        name: &str,
        extensibility: ExtensibilityKind,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let id = self.declare(
            name,
            AstKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )?;
        self.set_extensibility(id, extensibility)?;
        Ok(id)
    }

    /// A `struct` that extends another.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn derived_structure(
        &mut self,
        name: &str,
        base: Id<TypeNode>,
        extensibility: ExtensibilityKind,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let id = self.declare(
            name,
            AstKind::Structure {
                base: Some(Ref::Resolved(base)),
                members: Vec::new(),
            },
        )?;
        self.set_extensibility(id, extensibility)?;
        Ok(id)
    }

    /// A `union`.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn union(
        &mut self,
        name: &str,
        discriminator: Id<TypeNode>,
        extensibility: ExtensibilityKind,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let id = self.declare(
            name,
            AstKind::Union {
                discriminator: Ref::Resolved(discriminator),
                cases: Vec::new(),
                default_case: None,
            },
        )?;
        self.set_extensibility(id, extensibility)?;
        Ok(id)
    }

    /// An `enum`, with the `@bit_bound` §7.3.1.2.1.9 gives it.
    ///
    /// `None` leaves the annotation absent, and
    /// [`zerodds_ast::DEFAULT_BIT_BOUND`] applies — in one place, rather
    /// than written out here as well.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn enumeration(
        &mut self,
        name: &str,
        bit_bound: Option<u16>,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let id = self.declare(
            name,
            AstKind::Enumeration {
                enumerators: Vec::new(),
            },
        )?;
        self.set_bit_bound(id, bit_bound)?;
        Ok(id)
    }

    /// A `bitmask`.
    ///
    /// # Errors
    /// [`DynamicError::Inconsistent`] for a bound outside 1 to 64, the
    /// range `@bit_bound` takes on a bitmask (§7.5.2.2.2: "If this value
    /// is out of range, the operation shall fail with
    /// RETCODE_BAD_PARAMETER"); [`DynamicError::Model`].
    pub fn bitmask(
        &mut self,
        name: &str,
        bit_bound: Option<u16>,
    ) -> Result<Id<TypeNode>, DynamicError> {
        if let Some(bound) = bit_bound {
            if !(1..=64).contains(&bound) {
                return Err(DynamicError::inconsistent(format!(
                    "`{name}`: a bitmask bound of {bound} is outside 1 to 64"
                )));
            }
        }
        let id = self.declare(name, AstKind::Bitmask { bits: Vec::new() })?;
        self.set_bit_bound(id, bit_bound)?;
        Ok(id)
    }

    /// A `bitset`.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn bitset(&mut self, name: &str) -> Result<Id<TypeNode>, DynamicError> {
        self.declare(
            name,
            AstKind::Bitset {
                base: None,
                fields: Vec::new(),
            },
        )
    }

    // -- parts ---------------------------------------------------------

    /// Add a member to a `struct`, as its last member.
    ///
    /// An alias is added to as its base type is (Table 55): the member goes
    /// to the type the alias chain ends at.
    ///
    /// # Errors
    /// [`DynamicError::Model`] where the target is not an aggregate that
    /// holds members; [`DynamicError::Inconsistent`] where `member`
    /// carries a `@default` §7.2.2.4.4.4.9 does not allow, or one that
    /// does not read in the member's own type.
    pub fn add_member(
        &mut self,
        aggregate: Id<TypeNode>,
        member: &NewMember,
    ) -> Result<(), DynamicError> {
        self.add_member_at(aggregate, usize::MAX, member)
    }

    /// [`Self::add_member`] at `index` among the members.
    ///
    /// §7.5.2.7.6: an `index` beyond the last "shall be taken to indicate
    /// that the new member will become the last member"; the member at
    /// `index` and those after it "shall be shifted up by a single index
    /// value".
    ///
    /// # Errors
    /// As [`Self::add_member`].
    pub fn add_member_at(
        &mut self,
        aggregate: Id<TypeNode>,
        index: usize,
        member: &NewMember,
    ) -> Result<(), DynamicError> {
        let aggregate = self.alias_target(aggregate)?;
        let at = self.member_at(aggregate, &member.name);
        let member = self.numbered(aggregate, member);
        let spec = member.spec(&mut self.ast, &at)?;
        self.ast
            .add_member_at(aggregate, spec, index)
            .map(|_| ())
            .map_err(|error| DynamicError::model(&error))
    }

    /// Add a case to a `union`.
    ///
    /// `labels` are the case's values; `is_default` adds the `default:`
    /// label. A case with neither is a member no discriminator selects,
    /// which the model admits and §7.4.1.1.2's serialisation never
    /// reaches — it is the caller's to avoid, and reporting it here would
    /// be a validation rule this crate does not own.
    ///
    /// # Errors
    /// [`DynamicError::Model`] where the target is not a union;
    /// [`DynamicError::Inconsistent`] where `member` carries a `@default`
    /// §7.2.2.4.4.4.9 does not allow, or one that does not read in the
    /// member's own type.
    pub fn add_case(
        &mut self,
        union: Id<TypeNode>,
        labels: &[i64],
        is_default: bool,
        member: &NewMember,
    ) -> Result<(), DynamicError> {
        self.add_case_at(union, usize::MAX, labels, is_default, member)
    }

    /// [`Self::add_case`] at `index` among the cases, with the rule of
    /// [`Self::add_member_at`].
    ///
    /// # Errors
    /// As [`Self::add_case`].
    pub fn add_case_at(
        &mut self,
        union: Id<TypeNode>,
        index: usize,
        labels: &[i64],
        is_default: bool,
        member: &NewMember,
    ) -> Result<(), DynamicError> {
        let union = self.alias_target(union)?;
        let member = &self.numbered(union, member);
        let mut out = Vec::with_capacity(labels.len() + usize::from(is_default));
        for label in labels {
            out.push(CaseLabel::Value(self.int_expr(i128::from(*label))?));
        }
        if is_default {
            out.push(CaseLabel::Default);
        }
        let at = self.member_at(union, &member.name);
        let spec = member.spec(&mut self.ast, &at)?;
        self.ast
            .add_case_at(union, out, spec, index)
            .map(|_| ())
            .map_err(|error| DynamicError::model(&error))
    }

    /// Add an enumerator.
    ///
    /// `value` is `None` for a positional enumerator and `Some` for an
    /// explicit `@value(n)`; §7.3.1.2.1.11 makes the two different
    /// declarations, and the model keeps them apart.
    ///
    /// `default_literal` is `@default_literal`, §7.3.1.2.1.10 — which
    /// enumerator a value of this type takes when nothing selects one.
    /// It is a parameter and not a later call because Table 9's rule
    /// otherwise applies ("the first literal in the enumeration") and a
    /// caller that meant to override it has to say so while the
    /// enumerator is being declared, not after the type is read back.
    ///
    /// # Errors
    /// [`DynamicError::Model`] where the target is not an enumeration.
    pub fn add_enumerator(
        &mut self,
        enumeration: Id<TypeNode>,
        name: &str,
        value: Option<i32>,
        default_literal: bool,
    ) -> Result<(), DynamicError> {
        let value = match value {
            Some(value) => Some(self.int_expr(i128::from(value))?),
            None => None,
        };
        let id = self
            .ast
            .add_enumerator(enumeration, name, value)
            .map_err(|error| DynamicError::model(&error))?;
        if !default_literal {
            return Ok(());
        }
        let node_id = self
            .ast
            .enumerator(id)
            .map_err(|error| DynamicError::model(&error))?
            .node_id;
        if let Some(facets) = self
            .ast
            .facets_mut(zerodds_ast::AnyId::Enumerator(id, node_id))
        {
            facets.xtypes_mut().is_default_literal = true;
        }
        Ok(())
    }

    /// Add a bit to a `bitmask`.
    ///
    /// `position` is `None` for a positional bit and `Some` for an
    /// explicit `@position(k)`.
    ///
    /// # Errors
    /// [`DynamicError::Model`] where the target is not a bitmask.
    pub fn add_bit(
        &mut self,
        bitmask: Id<TypeNode>,
        name: &str,
        position: Option<u16>,
    ) -> Result<(), DynamicError> {
        self.ast
            .add_bit_value(bitmask, name, position)
            .map(|_| ())
            .map_err(|error| DynamicError::model(&error))
    }

    /// Add a field to a `bitset`. `name` is `None` for anonymous padding.
    ///
    /// # Errors
    /// [`DynamicError::Model`] where the target is not a bitset.
    pub fn add_bitfield(
        &mut self,
        bitset: Id<TypeNode>,
        name: Option<&str>,
        bits: u16,
        holder: Option<Id<TypeNode>>,
    ) -> Result<(), DynamicError> {
        self.ast
            .add_bitfield(bitset, name, bits, holder.map(Ref::Resolved))
            .map(|_| ())
            .map_err(|error| DynamicError::model(&error))
    }

    /// Put a type in a module, creating the module if it is new.
    ///
    /// Types created by the methods above sit in the root module.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn module(&mut self, path: &[&str]) -> Result<Id<ModuleNode>, DynamicError> {
        let mut module = self.ast.root_module();
        for segment in path {
            module = self
                .ast
                .ensure_module(module, segment)
                .map_err(|error| DynamicError::model(&error))?;
        }
        Ok(module)
    }

    /// A `struct` in a module rather than at the root.
    ///
    /// # Errors
    /// [`DynamicError::Model`].
    pub fn structure_in(
        &mut self,
        module: Id<ModuleNode>,
        name: &str,
        extensibility: ExtensibilityKind,
    ) -> Result<Id<TypeNode>, DynamicError> {
        let id = self
            .ast
            .add_type(
                module,
                name,
                AstKind::Structure {
                    base: None,
                    members: Vec::new(),
                },
            )
            .map_err(|error| DynamicError::model(&error))?;
        self.set_extensibility(id, extensibility)?;
        Ok(id)
    }

    /// `@nested` on a type, or `@autoid(HASH)`, or both.
    ///
    /// # Errors
    /// [`DynamicError::Unresolved`] for a stale handle.
    pub fn annotate(
        &mut self,
        type_: Id<TypeNode>,
        nested: bool,
        autoid_hash: bool,
    ) -> Result<(), DynamicError> {
        let any = self
            .ast
            .any_of_type(type_)
            .map_err(|error| DynamicError::model(&error))?;
        if let Some(facets) = self.ast.facets_mut(any) {
            let xtypes = facets.xtypes_mut();
            xtypes.is_nested = nested;
            if autoid_hash {
                xtypes.autoid = Some(AutoId::Hash);
            }
        }
        Ok(())
    }

    // -- internals -----------------------------------------------------

    /// A member's name, qualified with its aggregate's where the model
    /// still has one — for a `@default` refusal's detail, matching the
    /// `"{type}::{member}"` spelling [`super::to_ast`] composes for the
    /// same purpose.
    /// The type an alias chain ends at; the type itself where it is not an
    /// alias. Table 55: adding a member to an alias "is as it would be for
    /// the alias's base type".
    fn alias_target(&self, type_: Id<TypeNode>) -> Result<Id<TypeNode>, DynamicError> {
        let mut current = type_;
        for _ in 0..64 {
            let node = self
                .ast
                .type_(current)
                .map_err(|error| DynamicError::model(&error))?;
            let AstKind::Alias { aliased } = &node.kind else {
                return Ok(current);
            };
            current = aliased.resolved().ok_or_else(|| DynamicError::Unresolved {
                at: node.name.clone(),
            })?;
        }
        Err(DynamicError::Recursive {
            what: self.member_at(type_, ""),
        })
    }

    /// `member` with the id §7.5.2.7.5 gives one that has none: "one more
    /// than the current maximum member ID in the type". A type whose ids
    /// are hashed from the names keeps that rule instead.
    fn numbered(&self, aggregate: Id<TypeNode>, member: &NewMember) -> NewMember {
        let mut out = member.clone();
        if out.id.is_some() || matches!(self.ast.effective_autoid(aggregate), Some(AutoId::Hash)) {
            return out;
        }
        let existing: Vec<Id<zerodds_ast::MemberNode>> = match self.ast.type_(aggregate) {
            Ok(node) => match &node.kind {
                AstKind::Union { cases, .. } => cases
                    .iter()
                    .filter_map(|case| self.ast.case(*case).ok().map(|case| case.member))
                    .collect(),
                _ => self.ast.all_members_of(aggregate),
            },
            Err(_) => Vec::new(),
        };
        let mut highest: Option<MemberId> = None;
        for (index, id) in existing.iter().enumerate() {
            let own = self
                .ast
                .member(*id)
                .ok()
                .and_then(|node| node.facets.xtypes.as_ref())
                .and_then(zerodds_ast::XTypesFacet::effective_member_id)
                .unwrap_or_else(|| u32::try_from(index).unwrap_or(u32::MAX));
            highest = Some(highest.map_or(own, |current| current.max(own)));
        }
        out.id = Some(highest.map_or(0, |highest| highest.saturating_add(1)));
        out
    }

    fn member_at(&self, aggregate: Id<TypeNode>, member_name: &str) -> String {
        match self.ast.type_(aggregate) {
            Ok(node) => format!("{}::{member_name}", node.name),
            Err(_) => member_name.to_string(),
        }
    }

    fn declare(&mut self, name: &str, kind: AstKind) -> Result<Id<TypeNode>, DynamicError> {
        let root = self.ast.root_module();
        self.ast
            .add_type(root, name, kind)
            .map_err(|error| DynamicError::model(&error))
    }

    /// A type with no declaration of its own. See
    /// [`super::to_ast`]'s note: §7.3.4.6's fully descriptive types are
    /// anonymous and the name is a spelling for a diagnostic.
    fn anonymous(&mut self, name: &str, kind: AstKind) -> Result<Id<TypeNode>, DynamicError> {
        self.declare(name, kind)
    }

    fn bound(&mut self, bound: Option<u32>) -> Result<Bound, DynamicError> {
        match bound {
            None | Some(LENGTH_UNLIMITED) => Ok(Bound::Unbounded),
            Some(value) => self.int_expr(i128::from(value)).map(Bound::Const),
        }
    }

    fn int_expr(&mut self, value: i128) -> Result<Id<ConstExprNode>, DynamicError> {
        self.ast
            .add_int_expr(value)
            .map_err(|error| DynamicError::model(&error))
    }

    fn set_extensibility(
        &mut self,
        type_: Id<TypeNode>,
        extensibility: ExtensibilityKind,
    ) -> Result<(), DynamicError> {
        let any = self
            .ast
            .any_of_type(type_)
            .map_err(|error| DynamicError::model(&error))?;
        if let Some(facets) = self.ast.facets_mut(any) {
            facets.xtypes_mut().extensibility = Some(extensibility.into());
        }
        Ok(())
    }

    fn set_bit_bound(
        &mut self,
        type_: Id<TypeNode>,
        bit_bound: Option<u16>,
    ) -> Result<(), DynamicError> {
        let Some(bound) = bit_bound else {
            return Ok(());
        };
        let any = self
            .ast
            .any_of_type(type_)
            .map_err(|error| DynamicError::model(&error))?;
        if let Some(facets) = self.ast.facets_mut(any) {
            facets.xtypes_mut().bit_bound = Some(bound);
        }
        Ok(())
    }
}

/// The four built-in types of DDS-XTypes 1.3 §7.6.5 and Annex E, in one model.
///
/// `DDS::String`, `DDS::KeyedString`, `DDS::Bytes` and `DDS::KeyedBytes`
/// — the types a `DataWriter<String>` publishes when the application
/// declares no type of its own. They are declared here rather than
/// written out as IDL because they are the same four nodes in every
/// process and a caller needs them before any file is read.
///
/// # Errors
/// [`DynamicError::Model`] where the model rejects an edit, which no
/// declaration below should produce.
pub fn builtin_topic_types() -> Result<Arc<Ast>, DynamicError> {
    let mut builder = DynamicTypeBuilder::new();
    let module = builder.module(&["DDS"])?;
    let string = builder.string(false, None)?;
    let octet = builder.primitive(TypeKind::Byte)?;
    let bytes = builder.sequence(octet, None)?;

    // Annex E: all four `@extensibility(APPENDABLE)`, no `@nested`, the
    // payload member `value` (`key` first for the keyed ones).
    let plain_string = builder.structure_in(module, "String", ExtensibilityKind::Appendable)?;
    builder.add_member(plain_string, &NewMember::new("value", string))?;

    let keyed_string =
        builder.structure_in(module, "KeyedString", ExtensibilityKind::Appendable)?;
    builder.add_member(keyed_string, &NewMember::new("key", string).keyed())?;
    builder.add_member(keyed_string, &NewMember::new("value", string))?;

    let plain_bytes = builder.structure_in(module, "Bytes", ExtensibilityKind::Appendable)?;
    builder.add_member(plain_bytes, &NewMember::new("value", bytes))?;

    let keyed_bytes = builder.structure_in(module, "KeyedBytes", ExtensibilityKind::Appendable)?;
    builder.add_member(keyed_bytes, &NewMember::new("key", string).keyed())?;
    builder.add_member(keyed_bytes, &NewMember::new("value", bytes))?;

    Ok(builder.finish())
}

/// The names the four built-in topic types are declared under.
pub const BUILTIN_TOPIC_TYPE_NAMES: [&str; 4] = [
    "DDS::String",
    "DDS::KeyedString",
    "DDS::Bytes",
    "DDS::KeyedBytes",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use zerodds_ast::ConstValue;

    /// The `@default` expression a member of that name carries, evaluated
    /// — matching the style `tests/type_object_annotations.rs` uses for
    /// the same field read off the wire form.
    fn default_of(ast: &Ast, type_: Id<TypeNode>, name: &str) -> ConstValue {
        let member_id = *ast
            .members_of(type_)
            .iter()
            .find(|id| ast.member(**id).is_ok_and(|node| node.name == name))
            .unwrap_or_else(|| panic!("no member `{name}`"));
        let node = ast.member(member_id).expect("the member resolves");
        let expr = node
            .facets
            .xtypes
            .as_ref()
            .and_then(|facet| facet.default_value)
            .unwrap_or_else(|| panic!("`{name}` carries no @default"));
        ast.eval_const(expr).expect("the default evaluates")
    }

    /// Annex E defines the four built-in types `@extensibility(APPENDABLE)`,
    /// without `@nested`, with the member `value` (and `key` first for the
    /// keyed ones).
    #[test]
    fn the_builtin_types_are_those_of_annex_e() {
        let ast = builtin_topic_types().expect("the model declares");
        for (name, members) in [
            ("String", &["value"][..]),
            ("KeyedString", &["key", "value"][..]),
            ("Bytes", &["value"][..]),
            ("KeyedBytes", &["key", "value"][..]),
        ] {
            let type_ = ast
                .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(alloc::vec![
                    "DDS".to_string(),
                    name.to_string()
                ]))
                .ok()
                .and_then(zerodds_ast::AnyId::as_type)
                .unwrap_or_else(|| panic!("DDS::{name} is not declared"));
            assert_eq!(
                ast.effective_extensibility(type_),
                zerodds_ast::Extensibility::Appendable,
                "DDS::{name}"
            );
            assert!(!ast.effective_nested(type_), "DDS::{name} is non-nested");
            let names: Vec<String> = ast
                .members_of(type_)
                .iter()
                .map(|id| ast.member(*id).expect("member").name.clone())
                .collect();
            assert_eq!(names, members, "DDS::{name}");
        }
    }

    #[test]
    fn an_integer_default_is_stored_and_reads_back() {
        let mut builder = DynamicTypeBuilder::new();
        let int32 = builder.primitive(TypeKind::Int32).expect("int32");
        let sample = builder
            .structure("Sample", ExtensibilityKind::Final)
            .expect("the struct declares");
        builder
            .add_member(sample, &NewMember::new("count", int32).with_default("42"))
            .expect("the member adds");
        let ast = builder.model();
        assert_eq!(default_of(ast, sample, "count"), ConstValue::Integer(42));
    }

    #[test]
    fn a_float_default_is_stored_and_reads_back() {
        let mut builder = DynamicTypeBuilder::new();
        let float64 = builder.primitive(TypeKind::Float64).expect("float64");
        let sample = builder
            .structure("Sample", ExtensibilityKind::Final)
            .expect("the struct declares");
        builder
            .add_member(
                sample,
                &NewMember::new("factor", float64).with_default("2.5"),
            )
            .expect("the member adds");
        let ast = builder.model();
        assert_eq!(
            default_of(ast, sample, "factor"),
            ConstValue::Float {
                mantissa: "2.5".to_string(),
                exponent: None,
            }
        );
    }

    #[test]
    fn a_boolean_default_is_stored_and_reads_back() {
        let mut builder = DynamicTypeBuilder::new();
        let boolean = builder.primitive(TypeKind::Boolean).expect("boolean");
        let sample = builder
            .structure("Sample", ExtensibilityKind::Final)
            .expect("the struct declares");
        builder
            .add_member(
                sample,
                &NewMember::new("armed", boolean).with_default("TRUE"),
            )
            .expect("the member adds");
        let ast = builder.model();
        assert_eq!(default_of(ast, sample, "armed"), ConstValue::Boolean(true));
    }

    #[test]
    fn an_enum_default_is_stored_and_reads_back() {
        let mut builder = DynamicTypeBuilder::new();
        let colour = builder.enumeration("Colour", None).expect("enumeration");
        builder
            .add_enumerator(colour, "RED", None, false)
            .expect("RED");
        builder
            .add_enumerator(colour, "GREEN", None, false)
            .expect("GREEN");
        let sample = builder
            .structure("Sample", ExtensibilityKind::Final)
            .expect("the struct declares");
        builder
            .add_member(sample, &NewMember::new("hue", colour).with_default("1"))
            .expect("the member adds");
        let ast = builder.model();
        assert_eq!(default_of(ast, sample, "hue"), ConstValue::Integer(1));
    }

    #[test]
    fn a_string_default_is_stored_and_reads_back() {
        let mut builder = DynamicTypeBuilder::new();
        let string = builder.string(false, None).expect("string");
        let sample = builder
            .structure("Sample", ExtensibilityKind::Final)
            .expect("the struct declares");
        builder
            .add_member(
                sample,
                &NewMember::new("label", string).with_default("none"),
            )
            .expect("the member adds");
        let ast = builder.model();
        assert_eq!(
            default_of(ast, sample, "label"),
            ConstValue::String("none".to_string())
        );
    }

    /// A text that does not read in the member's type is refused, never
    /// silently turned into a zero.
    #[test]
    fn a_default_that_does_not_read_in_the_type_is_refused() {
        let mut builder = DynamicTypeBuilder::new();
        let int32 = builder.primitive(TypeKind::Int32).expect("int32");
        let sample = builder
            .structure("Sample", ExtensibilityKind::Final)
            .expect("the struct declares");
        let err = builder
            .add_member(sample, &NewMember::new("count", int32).with_default("abc"))
            .expect_err("`abc` does not read as an int32");
        assert!(matches!(err, DynamicError::Inconsistent { .. }));
    }

    /// §7.2.2.4.4.4.9: an `@optional` member may not carry a `@default`.
    #[test]
    fn optional_with_a_default_is_refused() {
        let mut builder = DynamicTypeBuilder::new();
        let int32 = builder.primitive(TypeKind::Int32).expect("int32");
        let sample = builder
            .structure("Sample", ExtensibilityKind::Final)
            .expect("the struct declares");
        let err = builder
            .add_member(
                sample,
                &NewMember::new("count", int32).optional().with_default("1"),
            )
            .expect_err("optional + default is inconsistent");
        assert!(matches!(err, DynamicError::Inconsistent { .. }));
    }

    /// The same refusal on a union case member, through [`add_case`].
    #[test]
    fn optional_with_a_default_is_refused_on_a_union_case() {
        let mut builder = DynamicTypeBuilder::new();
        let int32 = builder.primitive(TypeKind::Int32).expect("int32");
        let choice = builder
            .union("Choice", int32, ExtensibilityKind::Final)
            .expect("the union declares");
        let err = builder
            .add_case(
                choice,
                &[1],
                false,
                &NewMember::new("branch", int32).optional().with_default("1"),
            )
            .expect_err("optional + default is inconsistent");
        assert!(matches!(err, DynamicError::Inconsistent { .. }));
    }

    /// §7.5.2.11: a fresh value of the type starts a defaulted member at
    /// its own `@default`, not its type's Table 9 zero.
    /// `zerodds_wire_codec::default::of_type` — what
    /// `DynamicData`-from-a-type goes through — walks a structure member
    /// by member through `zerodds_wire_codec::default::member`, which
    /// this exercises end to end from the model side; the FFI side is
    /// `rmw-zerodds-shim::dynamic::tests::
    /// a_defaulted_member_reads_back_its_default_after_init_and_clear`.
    #[test]
    fn a_fresh_value_of_the_type_starts_a_defaulted_member_at_its_default() {
        use super::super::data::DynamicData;

        let mut builder = DynamicTypeBuilder::new();
        let int32 = builder.primitive(TypeKind::Int32).expect("int32");
        let sample = builder
            .structure("Sample", ExtensibilityKind::Final)
            .expect("the struct declares");
        builder
            .add_member(sample, &NewMember::new("count", int32).with_default("42"))
            .expect("the member adds");
        builder
            .add_member(sample, &NewMember::new("plain", int32))
            .expect("the member adds");
        let ast = builder.model();
        let value: DynamicData = zerodds_wire_codec::default::of_type(ast, sample, "")
            .expect("the type's default value builds");
        let DynamicData::Aggregate(members) = value else {
            panic!("a structure's default is an aggregate");
        };
        let count = members
            .iter()
            .find(|(name, _)| name == "count")
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| panic!("`count` is a member"));
        assert_eq!(
            count,
            DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 42),
            "`count` starts at its own @default"
        );
        let plain = members
            .iter()
            .find(|(name, _)| name == "plain")
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| panic!("`plain` is a member"));
        assert_eq!(
            plain,
            DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 0),
            "a member with no @default still starts at Table 9's zero"
        );
    }
}
