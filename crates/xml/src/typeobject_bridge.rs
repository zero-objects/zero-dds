// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XML → TypeObject Bridge.
//!
//! Converts the internal XML data model from `xtypes_def` into the
//! XTypes-1.3 TypeObject format from `zerodds-types`. Wire-/hash-compatible
//! with the IDL lowering from `zerodds-idl::semantics::to_typeobject`.
//!
//! # Spec sources
//!
//! - OMG XTypes 1.3 §7.3.4.x — TypeObject + Minimal/Complete variants.
//! - OMG XTypes 1.3 §7.3.4.5 — MinimalStructType / EnumType / UnionType.
//! - OMG XTypes 1.3 Annex A — XML mapping.
//!
//! # Scope C4.5-b (this stage)
//!
//! - `MinimalTypeObject` only. `CompleteTypeObject` not implemented.
//! - Top-level mapping (struct, enum, union, typedef, bitmask, bitset).
//! - Member types via [`TypeRef::Primitive`] (direct) or
//!   [`TypeRef::Named`] (lookup in [`TypeLibrary`] with hash precomputation).
//! - The modifiers `arrayDimensions`/`sequenceMaxLength`/`stringMaxLength`
//!   wrap the member TypeIdentifier in PlainArray-/PlainSequence-/
//!   string-bound variants.
//! - Member IDs: `@id` from XSD if set, otherwise sequential from 1
//!   (AUTOID_SEQUENTIAL, Spec §7.3.1.2.1.1).
//! - Extensibility: map `final`/`appendable`/`mutable` from
//!   [`Extensibility`] onto `StructTypeFlag`/`UnionTypeFlag`.
//!
//! # Deliberately not in the crate
//!
//! - `CompleteTypeObject` (comes in phase 6).
//! - Forward declarations / cross-library refs.
//! - Inheritance via `baseType` — the bridge does set `base_type` as
//!   `EquivalenceHashMinimal`, but cycle detection remains the task of the
//!   existing [`crate::inheritance`] module.
//! - `@autoid(HASH)` — XML schema has no corresponding annotation.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use zerodds_types::builder::{Extensibility as TypeExt, TypeObjectBuilder};
use zerodds_types::hash::{compute_complete_hash, compute_minimal_hash};
use zerodds_types::type_object::flags::{
    EnumLiteralFlag, EnumTypeFlag, StructTypeFlag, UnionDiscriminatorFlag, UnionMemberFlag,
    UnionTypeFlag,
};
use zerodds_types::type_object::minimal::{
    CommonDiscriminatorMember, CommonEnumeratedHeader, CommonEnumeratedLiteral,
    MinimalDiscriminatorMember, MinimalEnumeratedHeader, MinimalEnumeratedLiteral,
    MinimalEnumeratedType, MinimalUnionMember, MinimalUnionType,
};
use zerodds_types::{
    CompleteTypeObject, EquivalenceHash, MinimalTypeObject, PrimitiveKind, TypeIdentifier,
    TypeObject,
};

use crate::xtypes_def::{
    AutoId, BitField, BitValue, BitmaskType, BitsetType, EnumLiteral, EnumType, Extensibility,
    PrimitiveType as XmlPrimitive, StructMember, StructType, TryConstruct, TypeDef, TypeLibrary,
    TypeRef, TypedefType, UnionDiscriminator, UnionType,
};

/// Error during the XML→TypeObject mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// An XSD construct is (in C4.5-b) not yet mappable — e.g.
    /// inline maps or self-recursion.
    UnsupportedXsdConstruct(String),
    /// A `TypeRef::Named` reference could not be resolved,
    /// because it does not appear in the supplied [`TypeLibrary`] (or via
    /// `bridge_with_resolver`).
    UnresolvedReference(String),
    /// A numeric value (e.g. a union discriminator label) is
    /// not parseable.
    InvalidLiteral(String),
    /// The EquivalenceHash computation failed (buffer overflow
    /// in the internal encoder).
    HashFailed(String),
    /// Module entries are not a top-level type — the XML has a
    /// `<module>` at a position where only a type may appear.
    ModuleAtTopLevel(String),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedXsdConstruct(s) => {
                write!(f, "unsupported XSD construct: {s}")
            }
            Self::UnresolvedReference(s) => write!(f, "unresolved reference: {s}"),
            Self::InvalidLiteral(s) => write!(f, "invalid literal: {s}"),
            Self::HashFailed(s) => write!(f, "TypeIdentifier hashing failed: {s}"),
            Self::ModuleAtTopLevel(s) => {
                write!(f, "<module> not allowed at this position: {s}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BridgeError {}

// ============================================================================
// Public API
// ============================================================================

/// Converts a single XmlType-[`TypeDef`] into a [`TypeObject`]
/// (minimal variant).
///
/// Named member refs are stored as a null-hash placeholder
/// (`TypeIdentifier::EquivalenceHashMinimal([0; 14])`) — a
/// caller wanting to resolve a whole library at once should
/// use [`bridge_library`] instead.
///
/// # Errors
/// `BridgeError::UnsupportedXsdConstruct` for constructs not mapped in this
/// stage; `BridgeError::ModuleAtTopLevel` if
/// a `<module>` was passed directly (modules are not types).
pub fn xml_type_to_typeobject(xml: &TypeDef) -> Result<TypeObject, BridgeError> {
    let mto = xml_type_to_minimal_typeobject(xml)?;
    Ok(TypeObject::Minimal(mto))
}

/// Like [`xml_type_to_typeobject`], but directly as a
/// [`MinimalTypeObject`] (without the discriminator wrapper).
///
/// # Errors
/// See [`xml_type_to_typeobject`].
pub fn xml_type_to_minimal_typeobject(xml: &TypeDef) -> Result<MinimalTypeObject, BridgeError> {
    let resolver = NullResolver;
    bridge_typedef(xml, &resolver)
}

/// Converts a whole [`TypeLibrary`] into a map
/// `Name → MinimalTypeObject`.
///
/// A reference between types of the library becomes the
/// `EquivalenceHashMinimal` identifier of the referenced type, computed
/// from the type as it is finally built: the types are built in dependency
/// order, so a reference at any depth carries the identifier its target has
/// in the result.
///
/// `<module>` entries are flattened recursively. The map key for
/// nested types is the scoped name (`Module::Inner`). Names are looked up
/// like IDL scoped names. `<const>`, `<forward_dcl>` and `<annotation>`
/// entries define no type and are skipped.
///
/// # Errors
/// [`BridgeError::UnresolvedReference`] for a name that is no type of the
/// library; [`BridgeError::UnsupportedXsdConstruct`] for types that refer
/// to each other in a cycle (a recursive type needs a strongly connected
/// component identifier, which this mapping does not build); and the errors
/// of the individual type mappings.
pub fn bridge_library(
    lib: &TypeLibrary,
) -> Result<BTreeMap<String, MinimalTypeObject>, BridgeError> {
    bridge_in_dependency_order(
        lib,
        &BTreeMap::new(),
        TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([0; 14])),
        |td, res| bridge_typedef(td, res),
        |mto| {
            compute_minimal_hash(mto)
                .map(TypeIdentifier::EquivalenceHashMinimal)
                .map_err(|e| BridgeError::HashFailed(alloc::format!("{e:?}")))
        },
    )
}

// ============================================================================
// Complete-TypeObject side (XTypes 1.3 §7.3.4.5/§7.3.4.6, Annex A XML
// mapping) — a `CompleteTypeObject` carries the full member/field layout
// (names, member IDs, `@key`, extensibility) a *Minimal* one does not,
// which is what a consumer that decodes/encodes samples by structure
// (`zerodds_dcps::serialized_type::SerializedTypeSupport`) needs to build
// a codec from. Every `bridge_*_complete` function below mirrors its
// `bridge_*` sibling's field mapping exactly (extensibility, `@key`/
// `@optional`/`@must_understand`, auto-numbering, the first-literal-is-
// default rule for an enum with none marked), routed through the same
// [`TypeObjectBuilder`] the Minimal side uses where that builder exists
// (struct/enum/union/alias/bitmask/bitset all provide `build_complete()`
// beside `build_minimal()` from the one set of setter calls) — so the two
// forms cannot drift apart on where a flag comes from.
// ============================================================================

/// Converts a single [`TypeDef`] into a [`TypeObject::Complete`].
///
/// Named member refs are stored as a null-hash placeholder
/// (`TypeIdentifier::EquivalenceHashComplete([0; 14])`) — a
/// caller wanting to resolve a whole library at once should use
/// [`bridge_library_complete`] instead.
///
/// # Errors
/// See [`xml_type_to_typeobject`].
pub fn xml_type_to_complete_typeobject(xml: &TypeDef) -> Result<TypeObject, BridgeError> {
    Ok(TypeObject::Complete(xml_type_to_complete_type(xml)?))
}

/// Like [`xml_type_to_complete_typeobject`], but directly as a
/// [`CompleteTypeObject`] (without the discriminator wrapper).
///
/// # Errors
/// See [`xml_type_to_typeobject`].
pub fn xml_type_to_complete_type(xml: &TypeDef) -> Result<CompleteTypeObject, BridgeError> {
    let resolver = NullResolverComplete;
    bridge_typedef_complete(xml, &resolver)
}

/// Converts a whole [`TypeLibrary`] into a map `Name -> CompleteTypeObject`.
///
/// References between the types of the library are resolved as in
/// [`bridge_library`], to `EquivalenceHashComplete` identifiers: a type is
/// built after the types it refers to, and its identifier is the
/// [`compute_complete_hash`] of the object in the result — at any depth of
/// reference.
///
/// # Errors
/// As [`bridge_library`].
pub fn bridge_library_complete(
    lib: &TypeLibrary,
) -> Result<BTreeMap<String, CompleteTypeObject>, BridgeError> {
    bridge_library_complete_with_known(lib, &BTreeMap::new())
}

/// Like [`bridge_library_complete`], for a library whose types may refer to
/// types that are not part of it but already known: DDS-XRCE 1.0
/// §7.7.3.3.2 lets a type created from one `<types>` string "reference
/// other Types already known to the Agent".
///
/// `known` maps the name of each known type to its [`CompleteTypeObject`]
/// — the map [`bridge_library_complete`] and this function return fits
/// directly. A reference from the library to a known type becomes
/// `TypeIdentifier::EquivalenceHashComplete` of that type's
/// [`compute_complete_hash`], which is the identifier the known type has
/// itself. The result holds the types of `lib` only.
///
/// * A name that is neither a type of `lib` nor a known type is
///   [`BridgeError::UnresolvedReference`] naming it.
/// * The identifier of a type is computed from the type as it is
///   finally built, with its own references resolved, in dependency
///   order. A reference two levels down therefore carries the same
///   identifier the referenced type has in the result.
/// * References between types of `lib` that form a cycle (a type that
///   contains itself directly or through others) are refused with
///   [`BridgeError::UnsupportedXsdConstruct`] naming the types: a cycle
///   needs the strongly-connected-component identifier, which this
///   mapping does not build.
/// * `<const>`, `<forward_dcl>` and `<annotation>` entries define no type
///   of their own (the parser has folded the value of a constant into the
///   bounds that name it) and are skipped; `<include>` is refused.
/// * A name is looked up like an IDL scoped name: from the module of the
///   referring type outwards (`A::B::n`, `A::n`, `n`), a leading `::`
///   makes it absolute; within one scope a type of `lib` takes
///   precedence over a known type of the same name.
///
/// # Errors
/// [`BridgeError::UnresolvedReference`] for a name that resolves to
/// nothing; [`BridgeError::UnsupportedXsdConstruct`] as above.
pub fn bridge_library_complete_with_known(
    lib: &TypeLibrary,
    known: &BTreeMap<String, CompleteTypeObject>,
) -> Result<BTreeMap<String, CompleteTypeObject>, BridgeError> {
    let mut known_ids: BTreeMap<String, TypeIdentifier> = BTreeMap::new();
    for (name, cto) in known {
        let h = compute_complete_hash(cto)
            .map_err(|e| BridgeError::HashFailed(alloc::format!("`{name}`: {e:?}")))?;
        known_ids.insert(name.clone(), TypeIdentifier::EquivalenceHashComplete(h));
    }
    bridge_in_dependency_order(
        lib,
        &known_ids,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([0; 14])),
        |td, res| bridge_typedef_complete(td, res),
        |cto| {
            compute_complete_hash(cto)
                .map(TypeIdentifier::EquivalenceHashComplete)
                .map_err(|e| BridgeError::HashFailed(alloc::format!("{e:?}")))
        },
    )
}

/// Builds the types of `lib` in dependency order.
///
/// `bridge` maps a definition to its object given a resolver for the names
/// it uses; `identify` gives an object's TypeIdentifier (its equivalence
/// hash). `known_ids` are the identifiers of types outside `lib` that may
/// be referred to.
fn bridge_in_dependency_order<O>(
    lib: &TypeLibrary,
    known_ids: &BTreeMap<String, TypeIdentifier>,
    null_id: TypeIdentifier,
    bridge: impl Fn(&TypeDef, &ScopedResolver<'_>) -> Result<O, BridgeError>,
    identify: impl Fn(&O) -> Result<TypeIdentifier, BridgeError>,
) -> Result<BTreeMap<String, O>, BridgeError> {
    let mut types: BTreeMap<String, &TypeDef> = BTreeMap::new();
    for (scoped, td) in flatten(&lib.types, "") {
        match td {
            TypeDef::Const(_) | TypeDef::ForwardDcl(_) | TypeDef::Annotation(_) => {}
            other => {
                types.insert(scoped, other);
            }
        }
    }

    // Edges: a type depends on the types of `lib` its references name.
    let mut pending: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (scoped, td) in &types {
        let mut needs: Vec<String> = Vec::new();
        for name in referenced_names(td) {
            match lookup_scoped(scope_of(scoped), &name, &types, known_ids) {
                Some(Resolved::Local(target)) => {
                    if !needs.contains(&target) {
                        needs.push(target);
                    }
                }
                Some(Resolved::Known(_)) => {}
                None => {
                    return Err(BridgeError::UnresolvedReference(alloc::format!(
                        "type `{name}` referenced from `{scoped}`"
                    )));
                }
            }
        }
        for target in &needs {
            dependents
                .entry(target.clone())
                .or_default()
                .push(scoped.clone());
        }
        pending.insert(scoped.clone(), needs);
    }

    // Dependency order: a type is built once every type it needs is.
    let mut local_ids: BTreeMap<String, TypeIdentifier> = BTreeMap::new();
    let mut out: BTreeMap<String, O> = BTreeMap::new();
    let mut ready: Vec<String> = pending
        .iter()
        .filter(|(_, needs)| needs.is_empty())
        .map(|(name, _)| name.clone())
        .collect();
    while let Some(scoped) = ready.pop() {
        let td = types[&scoped];
        let resolver = ScopedResolver {
            scope: scope_of(&scoped),
            types: &types,
            local_ids: &local_ids,
            known_ids,
            null_id: &null_id,
            unresolved: core::cell::RefCell::new(None),
        };
        let object = bridge(td, &resolver)?;
        if let Some(name) = resolver.unresolved.into_inner() {
            return Err(BridgeError::UnresolvedReference(alloc::format!(
                "type `{name}` referenced from `{scoped}`"
            )));
        }
        local_ids.insert(scoped.clone(), identify(&object)?);
        out.insert(scoped.clone(), object);
        for dependent in dependents.get(&scoped).into_iter().flatten() {
            if let Some(needs) = pending.get_mut(dependent) {
                needs.retain(|n| *n != scoped);
                if needs.is_empty() && !out.contains_key(dependent) && !ready.contains(dependent) {
                    ready.push(dependent.clone());
                }
            }
        }
    }
    if out.len() < types.len() {
        let stuck: Vec<&str> = types
            .keys()
            .filter(|n| !out.contains_key(*n))
            .map(String::as_str)
            .collect();
        return Err(BridgeError::UnsupportedXsdConstruct(alloc::format!(
            "types refer to each other in a cycle ({}); a recursive type needs a \
             strongly connected component identifier",
            stuck.join(", ")
        )));
    }
    Ok(out)
}

/// What a name resolves to for [`bridge_library_complete_with_known`].
enum Resolved {
    /// A type of the library being bridged, by its scoped name.
    Local(String),
    /// A known type, by its name.
    Known(String),
}

/// The enclosing module of a scoped name: `A::B::T` is in `A::B`.
fn scope_of(scoped: &str) -> &str {
    scoped.rsplit_once("::").map_or("", |(scope, _)| scope)
}

/// IDL name lookup: the candidates are `scope::name` for the scope and
/// every enclosing scope, then `name`; a leading `::` names the global
/// scope only.
fn lookup_scoped(
    scope: &str,
    name: &str,
    types: &BTreeMap<String, &TypeDef>,
    known: &BTreeMap<String, TypeIdentifier>,
) -> Option<Resolved> {
    let find = |candidate: &str| -> Option<Resolved> {
        if types.contains_key(candidate) {
            Some(Resolved::Local(candidate.to_string()))
        } else if known.contains_key(candidate) {
            Some(Resolved::Known(candidate.to_string()))
        } else {
            None
        }
    };
    if let Some(absolute) = name.strip_prefix("::") {
        return find(absolute);
    }
    let mut scope = scope;
    loop {
        let candidate = if scope.is_empty() {
            name.to_string()
        } else {
            alloc::format!("{scope}::{name}")
        };
        if let Some(found) = find(&candidate) {
            return Some(found);
        }
        if scope.is_empty() {
            return None;
        }
        scope = scope_of(scope);
    }
}

/// The type names a definition refers to: member and discriminator types,
/// map keys, the type an alias names, and the base of a structure.
fn referenced_names(td: &TypeDef) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut add = |t: &TypeRef| {
        if let TypeRef::Named(n) = t {
            names.push(n.clone());
        }
    };
    match td {
        TypeDef::Struct(s) => {
            if let Some(base) = &s.base_type {
                add(&TypeRef::Named(base.clone()));
            }
            for m in &s.members {
                add(&m.type_ref);
                if let Some(k) = &m.map_key {
                    add(k);
                }
            }
        }
        TypeDef::Union(u) => {
            add(&u.discriminator);
            for c in &u.cases {
                add(&c.member.type_ref);
                if let Some(k) = &c.member.map_key {
                    add(k);
                }
            }
        }
        TypeDef::Typedef(t) => {
            add(&t.type_ref);
            if let Some(k) = &t.map_key {
                add(k);
            }
        }
        _ => {}
    }
    names
}

/// Resolver of [`bridge_library_complete_with_known`]: scoped lookup into
/// the identifiers of the library's types built so far and of the known
/// types. [`NameResolver::resolve`] has no way to fail, so a name that
/// resolves to nothing is recorded and the caller turns it into an error.
struct ScopedResolver<'a> {
    scope: &'a str,
    types: &'a BTreeMap<String, &'a TypeDef>,
    local_ids: &'a BTreeMap<String, TypeIdentifier>,
    known_ids: &'a BTreeMap<String, TypeIdentifier>,
    /// What a name that resolves to nothing is stood in by while the error
    /// is recorded.
    null_id: &'a TypeIdentifier,
    unresolved: core::cell::RefCell<Option<String>>,
}

impl NameResolver for ScopedResolver<'_> {
    fn resolve(&self, name: &str) -> TypeIdentifier {
        let found = match lookup_scoped(self.scope, name, self.types, self.known_ids) {
            Some(Resolved::Local(target)) => self.local_ids.get(&target),
            Some(Resolved::Known(target)) => self.known_ids.get(&target),
            None => None,
        };
        if let Some(id) = found {
            return id.clone();
        }
        let mut slot = self.unresolved.borrow_mut();
        if slot.is_none() {
            *slot = Some(name.to_string());
        }
        self.null_id.clone()
    }
}

/// Bitmask annotations and `external` on an alias have no place in a
/// TypeObject; they are refused by name.
fn check_bitmask_and_typedef(xml: &TypeDef) -> Result<(), BridgeError> {
    match xml {
        TypeDef::Bitmask(b) => {
            if let Some(a) = b.annotations.first() {
                return Err(unmapped(
                    &b.name,
                    &alloc::format!("annotation `{}`", a.name),
                ));
            }
            if let Some(v) = b.bit_values.iter().find(|v| !v.annotations.is_empty()) {
                return Err(unmapped(
                    &alloc::format!("{}::{}", b.name, v.name),
                    &alloc::format!("annotation `{}`", v.annotations[0].name),
                ));
            }
        }
        TypeDef::Typedef(t) if t.external == Some(true) => {
            return Err(unmapped(&t.name, "external"));
        }
        _ => {}
    }
    Ok(())
}

fn bridge_typedef_complete<R: NameResolver>(
    xml: &TypeDef,
    res: &R,
) -> Result<CompleteTypeObject, BridgeError> {
    check_bitmask_and_typedef(xml)?;
    match xml {
        TypeDef::Struct(s) => Ok(CompleteTypeObject::Struct(bridge_struct_complete(s, res)?)),
        TypeDef::Enum(e) => Ok(CompleteTypeObject::Enumerated(bridge_enum_complete(e)?)),
        TypeDef::Union(u) => Ok(CompleteTypeObject::Union(bridge_union_complete(u, res)?)),
        TypeDef::Typedef(t) => bridge_typedef_alias_complete(t, res),
        TypeDef::Bitmask(b) => Ok(CompleteTypeObject::Bitmask(bridge_bitmask_complete(b))),
        TypeDef::Bitset(b) => Ok(CompleteTypeObject::Bitset(bridge_bitset_complete(b)?)),
        TypeDef::Module(m) => Err(BridgeError::ModuleAtTopLevel(m.name.clone())),
        TypeDef::Include(_)
        | TypeDef::ForwardDcl(_)
        | TypeDef::Const(_)
        | TypeDef::Annotation(_) => Err(BridgeError::UnsupportedXsdConstruct(alloc::format!(
            "non-bridgeable XML element kind: {}",
            xml.name()
        ))),
    }
}

fn bridge_struct_complete<R: NameResolver>(
    s: &StructType,
    res: &R,
) -> Result<zerodds_types::type_object::complete::CompleteStructType, BridgeError> {
    Ok(struct_builder(s, res)?.build_complete())
}

/// Mirrors [`bridge_enum`]'s auto-numbering and first-literal-default
/// rule, via [`TypeObjectBuilder::enum_type`] instead of a hand-built
/// [`MinimalEnumeratedType`] — the builder's `build_complete()` supplies
/// `CompleteMemberDetail`'s name/annotations from the same `.literal()`
/// calls.
fn bridge_enum_complete(
    e: &EnumType,
) -> Result<zerodds_types::type_object::complete::CompleteEnumeratedType, BridgeError> {
    refuse_enum_extras(e)?;
    let bit_bound: u16 = e.bit_bound.unwrap_or(32).min(64) as u16;
    let mut prev: i32 = -1;
    let default_index = default_literal_index(e);
    let mut builder = TypeObjectBuilder::enum_type(e.name.clone())
        .bit_bound(bit_bound)
        .extensibility(map_extensibility(e.extensibility.unwrap_or_default()));
    for (index, l) in e.enumerators.iter().enumerate() {
        let value = l.value.unwrap_or(prev.saturating_add(1));
        prev = value;
        builder = if index == default_index {
            builder.default_literal(l.name.clone(), value)
        } else {
            builder.literal(l.name.clone(), value)
        };
    }
    Ok(builder.build_complete())
}

/// The enumerator that carries `IS_DEFAULT`: the one marked
/// `defaultLiteral="true"`, else the first (§7.2.2.4.1.1).
fn default_literal_index(e: &EnumType) -> usize {
    e.enumerators
        .iter()
        .position(|l| l.default_literal == Some(true))
        .unwrap_or(0)
}

/// `<annotate>` and `<verbatim>` of an enumeration have no place in a
/// TypeObject.
fn refuse_enum_extras(e: &EnumType) -> Result<(), BridgeError> {
    if let Some(a) = e.annotations.first() {
        return Err(unmapped(
            &e.name,
            &alloc::format!("annotation `{}`", a.name),
        ));
    }
    if !e.verbatims.is_empty() {
        return Err(unmapped(&e.name, "a <verbatim>"));
    }
    if let Some(l) = e.enumerators.iter().find(|l| !l.annotations.is_empty()) {
        return Err(unmapped(
            &alloc::format!("{}::{}", e.name, l.name),
            &alloc::format!("annotation `{}`", l.annotations[0].name),
        ));
    }
    Ok(())
}

/// Via [`TypeObjectBuilder::union_type`] rather than mirroring
/// [`bridge_union`]'s hand-built [`MinimalUnionType`] literally: the
/// builder additionally sets `UnionMemberFlag::TRY_CONSTRUCT1` on every
/// case, which `crates/types/src/builder.rs` documents as byte-verified
/// against Cyclone/FastDDS — a correction the Minimal hand-built path
/// does not carry, not a behavior this function reproduces.
fn bridge_union_complete<R: NameResolver>(
    u: &UnionType,
    res: &R,
) -> Result<zerodds_types::type_object::complete::CompleteUnionType, BridgeError> {
    check_union(u)?;
    let disc_ti = type_ref_to_identifier(&u.discriminator, res);
    let mut builder = TypeObjectBuilder::union_type(u.name.clone(), disc_ti)
        .extensibility(map_extensibility(u.extensibility.unwrap_or_default()));
    for case in &u.cases {
        let m = &case.member;
        let mut labels: Vec<i32> = Vec::with_capacity(case.discriminators.len());
        let mut is_default = false;
        for d in &case.discriminators {
            match d {
                UnionDiscriminator::Default => is_default = true,
                UnionDiscriminator::Value(s) => labels.push(parse_label(s)?),
            }
        }
        let ti = wrap_member_type(m, res)?;
        builder = if is_default {
            builder.default_case(m.name.clone(), ti)
        } else {
            builder.case(m.name.clone(), ti, labels)
        };
    }
    Ok(builder.build_complete())
}

/// Complete counterpart of [`bridge_typedef_alias`]. XTypes 1.3 §7.3.4.6:
/// an alias is a `CompleteAliasType` whose related type is the identifier
/// of its target; for a typedef of a sequence, an array or a map the
/// target is the plain collection identifier (`PlainSequence*`,
/// `PlainArray*`, `PlainMap*`) of the element type, with the bounds.
///
/// # Errors
/// As [`bridge_typedef_alias`].
fn bridge_typedef_alias_complete<R: NameResolver>(
    t: &TypedefType,
    res: &R,
) -> Result<CompleteTypeObject, BridgeError> {
    let base = type_ref_to_identifier_with_string_bound(&t.type_ref, t.string_max_length, res);
    let key = t
        .map_key
        .as_ref()
        .map(|k| type_ref_to_identifier_with_string_bound(k, t.map_key_string_max_length, res));
    let related = collection_identifier(
        base,
        t.sequence_max_length,
        &t.array_dimensions,
        key.map(|k| (k, t.map_max_length.unwrap_or(0))),
    );
    let alias = TypeObjectBuilder::alias(t.name.clone(), related).build_complete();
    Ok(CompleteTypeObject::Alias(alias))
}

fn bridge_bitmask_complete(
    b: &BitmaskType,
) -> zerodds_types::type_object::complete::CompleteBitmaskType {
    let bit_bound: u16 = b.bit_bound.unwrap_or(32).min(64) as u16;
    let mut prev: i32 = -1;
    let mut builder = TypeObjectBuilder::bitmask(b.name.clone()).bit_bound(bit_bound);
    for v in &b.bit_values {
        let pos = match v.position {
            Some(p) => {
                prev = p as i32;
                p
            }
            None => {
                let p = (prev.saturating_add(1)).max(0) as u32;
                prev = p as i32;
                p
            }
        };
        builder = builder.flag(v.name.clone(), pos as u16);
    }
    builder.build_complete()
}

fn bridge_bitset_complete(
    b: &BitsetType,
) -> Result<zerodds_types::type_object::complete::CompleteBitsetType, BridgeError> {
    let mut next_pos: u16 = 0;
    let mut builder = TypeObjectBuilder::bitset(b.name.clone());
    for f in &b.bit_fields {
        let bitcount = parse_bitset_mask_bits(&f.mask)?;
        let holder = match &f.type_ref {
            TypeRef::Primitive(p) => holder_kind_byte(*p).ok_or_else(|| {
                BridgeError::UnsupportedXsdConstruct(alloc::format!(
                    "bitset holder type: {}",
                    p.as_xml()
                ))
            })?,
            TypeRef::Named(_) => {
                return Err(BridgeError::UnsupportedXsdConstruct(
                    "bitset holder must be a primitive integer".to_string(),
                ));
            }
        };
        let pos = next_pos;
        next_pos = next_pos.saturating_add(u16::from(bitcount));
        builder = builder.field(f.name.clone(), pos, bitcount, holder);
    }
    Ok(builder.build_complete())
}

/// Resolver — Complete side.
struct NullResolverComplete;

impl NameResolver for NullResolverComplete {
    fn resolve(&self, _name: &str) -> TypeIdentifier {
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([0; 14]))
    }
}

// ============================================================================
// Resolver — strategy for named references
// ============================================================================

/// Strategy for mapping `TypeRef::Named(...)` onto a [`TypeIdentifier`].
/// Implementations decide how unresolvable refs
/// are handled (null hash vs. error).
trait NameResolver {
    fn resolve(&self, name: &str) -> TypeIdentifier;
}

/// Always returns `EquivalenceHashMinimal([0; 14])`. Used in the pre-pass
/// and for the single-type bridge when no library is known.
struct NullResolver;

impl NameResolver for NullResolver {
    fn resolve(&self, _name: &str) -> TypeIdentifier {
        TypeIdentifier::EquivalenceHashMinimal(zerodds_types::EquivalenceHash([0; 14]))
    }
}

// ============================================================================
// Flattening
// ============================================================================

/// zerodds-lint: recursion-depth 32
///
/// XML module hierarchies are rarely deeper than ~8 levels
/// (`org::omg::dds::core::policy` style). A cap of 32 also covers
/// pathologically nested test fixtures.
fn flatten<'a>(types: &'a [TypeDef], prefix: &str) -> Vec<(String, &'a TypeDef)> {
    let mut out: Vec<(String, &TypeDef)> = Vec::new();
    for t in types {
        match t {
            TypeDef::Module(m) => {
                let new_prefix = if prefix.is_empty() {
                    m.name.clone()
                } else {
                    alloc::format!("{prefix}::{}", m.name)
                };
                out.extend(flatten(&m.types, &new_prefix));
            }
            other => {
                let scoped = if prefix.is_empty() {
                    other.name().to_string()
                } else {
                    alloc::format!("{prefix}::{}", other.name())
                };
                out.push((scoped, other));
            }
        }
    }
    out
}

// ============================================================================
// Top-Level Dispatcher
// ============================================================================

fn bridge_typedef<R: NameResolver>(
    xml: &TypeDef,
    res: &R,
) -> Result<MinimalTypeObject, BridgeError> {
    check_bitmask_and_typedef(xml)?;
    match xml {
        TypeDef::Struct(s) => bridge_struct(s, res),
        TypeDef::Enum(e) => Ok(MinimalTypeObject::Enumerated(bridge_enum(e)?)),
        TypeDef::Union(u) => bridge_union(u, res),
        TypeDef::Typedef(t) => bridge_typedef_alias(t, res),
        TypeDef::Bitmask(b) => Ok(MinimalTypeObject::Bitmask(bridge_bitmask(b))),
        TypeDef::Bitset(b) => bridge_bitset(b),
        TypeDef::Module(m) => Err(BridgeError::ModuleAtTopLevel(m.name.clone())),
        TypeDef::Include(_)
        | TypeDef::ForwardDcl(_)
        | TypeDef::Const(_)
        | TypeDef::Annotation(_) => Err(BridgeError::UnsupportedXsdConstruct(alloc::format!(
            "non-bridgeable XML element kind: {}",
            xml.name()
        ))),
    }
}

// ============================================================================
// Struct
// ============================================================================

fn bridge_struct<R: NameResolver>(
    s: &StructType,
    res: &R,
) -> Result<MinimalTypeObject, BridgeError> {
    Ok(MinimalTypeObject::Struct(
        struct_builder(s, res)?.build_minimal(),
    ))
}

/// The error for a feature of the XML model the TypeObject cannot carry.
fn unmapped(owner: &str, what: &str) -> BridgeError {
    BridgeError::UnsupportedXsdConstruct(alloc::format!(
        "{owner}: {what} has no place in a TypeObject"
    ))
}

/// The builder of a structure, with everything the model carries that a
/// TypeObject can: extensibility, `nested`, `autoid="hash"`, the base, and
/// per member `key`, `optional`, `mustUnderstand`, `id`, `external` and
/// `tryConstruct`. `<annotate>`, `<verbatim>` and `nonSerialized="true"`
/// are refused by name rather than left out.
fn struct_builder<R: NameResolver>(
    s: &StructType,
    res: &R,
) -> Result<zerodds_types::builder::StructBuilder, BridgeError> {
    if let Some(a) = s.annotations.first() {
        return Err(unmapped(
            &s.name,
            &alloc::format!("annotation `{}`", a.name),
        ));
    }
    if !s.verbatims.is_empty() {
        return Err(unmapped(&s.name, "a <verbatim>"));
    }
    let ext = map_extensibility(s.extensibility.unwrap_or_default());
    let mut builder = TypeObjectBuilder::struct_type(s.name.clone()).extensibility(ext);
    if s.nested == Some(true) {
        builder = builder.nested();
    }
    if s.auto_id == Some(AutoId::Hash) {
        builder = builder.autoid_hash();
    }
    if let Some(base) = &s.base_type {
        builder = builder.base(res.resolve(base));
    }

    for m in &s.members {
        let owner = alloc::format!("{}::{}", s.name, m.name);
        if let Some(a) = m.annotations.first() {
            return Err(unmapped(&owner, &alloc::format!("annotation `{}`", a.name)));
        }
        if m.non_serialized == Some(true) {
            return Err(unmapped(&owner, "nonSerialized"));
        }
        let ti = wrap_member_type(m, res)?;
        let key = m.key;
        let optional = m.optional;
        let must_understand = m.must_understand;
        let explicit_id = m.id;
        let external = m.external == Some(true);
        let try_construct = m.try_construct;
        let name = m.name.clone();
        builder = builder.member(name, ti, |mut mb| {
            if key {
                mb = mb.key();
            }
            if optional {
                mb = mb.optional();
            }
            if must_understand {
                mb = mb.must_understand();
            }
            if external {
                mb = mb.external();
            }
            if let Some(kind) = try_construct {
                mb = mb.try_construct(match kind {
                    TryConstruct::Discard => zerodds_types::builder::TryConstruct::Discard,
                    TryConstruct::UseDefault => zerodds_types::builder::TryConstruct::UseDefault,
                    TryConstruct::Trim => zerodds_types::builder::TryConstruct::Trim,
                });
            }
            if let Some(id) = explicit_id {
                mb = mb.id(id);
            }
            mb
        });
    }
    Ok(builder)
}

fn map_extensibility(e: Extensibility) -> TypeExt {
    match e {
        Extensibility::Final => TypeExt::Final,
        Extensibility::Appendable => TypeExt::Appendable,
        Extensibility::Mutable => TypeExt::Mutable,
    }
}

// ============================================================================
// Enum
// ============================================================================

fn bridge_enum(e: &EnumType) -> Result<MinimalEnumeratedType, BridgeError> {
    refuse_enum_extras(e)?;
    let default_index = default_literal_index(e);
    let bit_bound: u16 = e.bit_bound.unwrap_or(32).min(64) as u16;
    // Auto-numbering: Spec §7.3.1.2.4.1 — if `value` is missing,
    // uses `prev + 1`, starting at 0.
    let mut prev: i32 = -1;
    // The default is the enumerator marked `defaultLiteral="true"`, else
    // the first one (§7.2.2.4.1.1); it carries `IS_DEFAULT`, as `from_ast`
    // writes it for an unannotated IDL enumeration.
    let literal_seq: Vec<MinimalEnumeratedLiteral> = e
        .enumerators
        .iter()
        .enumerate()
        .map(|(index, l): (usize, &EnumLiteral)| {
            let value = l.value.unwrap_or(prev.saturating_add(1));
            prev = value;
            MinimalEnumeratedLiteral {
                common: CommonEnumeratedLiteral {
                    value,
                    flags: EnumLiteralFlag(if index == default_index {
                        EnumLiteralFlag::IS_DEFAULT_LITERAL
                    } else {
                        0
                    }),
                },
                detail: zerodds_types::type_object::common::NameHash::from_name(&l.name),
            }
        })
        .collect();
    let enum_flags = EnumTypeFlag(match e.extensibility.unwrap_or_default() {
        Extensibility::Final => EnumTypeFlag::IS_FINAL,
        Extensibility::Appendable => EnumTypeFlag::IS_APPENDABLE,
        Extensibility::Mutable => StructTypeFlag::IS_MUTABLE,
    });
    Ok(MinimalEnumeratedType {
        enum_flags,
        header: MinimalEnumeratedHeader {
            common: CommonEnumeratedHeader { bit_bound },
        },
        literal_seq,
    })
}

// ============================================================================
// Union
// ============================================================================

/// What a union can carry into a TypeObject: its extensibility. The rest
/// the model reads (`nested`, `autoid`, annotations, `<verbatim>`, the
/// discriminator's `key`, a member's `external`/`tryConstruct`/
/// `nonSerialized`) is refused by name.
fn check_union(u: &UnionType) -> Result<(), BridgeError> {
    if let Some(a) = u.annotations.first() {
        return Err(unmapped(
            &u.name,
            &alloc::format!("annotation `{}`", a.name),
        ));
    }
    if !u.verbatims.is_empty() {
        return Err(unmapped(&u.name, "a <verbatim>"));
    }
    if u.nested == Some(true) {
        return Err(unmapped(&u.name, "nested"));
    }
    if u.auto_id == Some(AutoId::Hash) {
        return Err(unmapped(&u.name, "autoid=\"hash\""));
    }
    if u.discriminator_key || !u.discriminator_annotations.is_empty() {
        return Err(unmapped(&u.name, "key or annotation on the discriminator"));
    }
    for c in &u.cases {
        let m = &c.member;
        let owner = alloc::format!("{}::{}", u.name, m.name);
        if let Some(a) = m.annotations.first() {
            return Err(unmapped(&owner, &alloc::format!("annotation `{}`", a.name)));
        }
        if m.external == Some(true) {
            return Err(unmapped(&owner, "external"));
        }
        if m.try_construct.is_some() {
            return Err(unmapped(&owner, "tryConstruct"));
        }
        if m.non_serialized == Some(true) {
            return Err(unmapped(&owner, "nonSerialized"));
        }
    }
    Ok(())
}

fn bridge_union<R: NameResolver>(u: &UnionType, res: &R) -> Result<MinimalTypeObject, BridgeError> {
    check_union(u)?;
    let disc_ti = type_ref_to_identifier(&u.discriminator, res);
    let mut next_seq: u32 = 1;
    let mut member_seq: Vec<MinimalUnionMember> = Vec::with_capacity(u.cases.len());
    for case in &u.cases {
        let m = &case.member;
        let mut labels: Vec<i32> = Vec::with_capacity(case.discriminators.len());
        let mut is_default = false;
        for d in &case.discriminators {
            match d {
                UnionDiscriminator::Default => {
                    is_default = true;
                }
                UnionDiscriminator::Value(s) => {
                    let v = parse_label(s)?;
                    labels.push(v);
                }
            }
        }
        let id = m.id.unwrap_or_else(|| {
            let v = next_seq;
            next_seq += 1;
            v
        });
        let ti = wrap_member_type(m, res)?;
        let mut flags: u16 = 0;
        if is_default {
            flags |= UnionMemberFlag::IS_DEFAULT;
        }
        member_seq.push(MinimalUnionMember {
            common: zerodds_types::type_object::common::CommonUnionMember {
                member_id: id,
                member_flags: UnionMemberFlag(flags),
                type_id: ti,
                label_seq: labels,
            },
            detail: zerodds_types::type_object::common::NameHash::from_name(&m.name),
        });
    }
    // The extensibility of the union, appendable unless it says otherwise
    // (Spec §7.2.2.4). The struct flag bit positions are the union's.
    let union_flags_bits = match u.extensibility.unwrap_or_default() {
        Extensibility::Final => StructTypeFlag::IS_FINAL,
        Extensibility::Appendable => StructTypeFlag::IS_APPENDABLE,
        Extensibility::Mutable => StructTypeFlag::IS_MUTABLE,
    };
    Ok(MinimalTypeObject::Union(MinimalUnionType {
        union_flags: UnionTypeFlag(union_flags_bits),
        discriminator: MinimalDiscriminatorMember {
            common: CommonDiscriminatorMember {
                member_flags: UnionDiscriminatorFlag::default(),
                type_id: disc_ti,
            },
        },
        member_seq,
    }))
}

fn parse_label(s: &str) -> Result<i32, BridgeError> {
    let trimmed = s.trim();
    if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return i64::from_str_radix(rest, 16)
            .ok()
            .and_then(|v| i32::try_from(v).ok())
            .ok_or_else(|| BridgeError::InvalidLiteral(s.to_string()));
    }
    trimmed
        .parse::<i32>()
        .map_err(|_| BridgeError::InvalidLiteral(s.to_string()))
}

// ============================================================================
// Typedef → Alias (of a plain collection identifier where it names one)
// ============================================================================

/// XTypes 1.3 §7.3.4.6 and §7.3.4.8: an alias is an alias type whose
/// related type is the identifier of its target. For a typedef of a
/// sequence, an array or a map the target is the plain collection
/// identifier (`PlainSequence*`, `PlainArray*`, `PlainMap*`) of the element
/// type with the bounds; the element identifier is the one `res` gives,
/// the minimal hash of a named type. XTypes 1.3 §7.3.2.4.4 binds sequence
/// tighter than array and array tighter than map, so a typedef naming
/// several is built from the inside out ([`collection_identifier`]).
///
/// # Errors
/// None for the typedef itself; the signature matches the other bridges.
fn bridge_typedef_alias<R: NameResolver>(
    t: &TypedefType,
    res: &R,
) -> Result<MinimalTypeObject, BridgeError> {
    let base = type_ref_to_identifier_with_string_bound(&t.type_ref, t.string_max_length, res);
    let key = t
        .map_key
        .as_ref()
        .map(|k| type_ref_to_identifier_with_string_bound(k, t.map_key_string_max_length, res));
    let related = collection_identifier(
        base,
        t.sequence_max_length,
        &t.array_dimensions,
        key.map(|k| (k, t.map_max_length.unwrap_or(0))),
    );
    let alias = TypeObjectBuilder::alias(t.name.clone(), related).build_minimal();
    Ok(MinimalTypeObject::Alias(alias))
}

// ============================================================================
// Bitmask
// ============================================================================

fn bridge_bitmask(b: &BitmaskType) -> zerodds_types::type_object::minimal::MinimalBitmaskType {
    let bit_bound: u16 = b.bit_bound.unwrap_or(32).min(64) as u16;
    let mut prev: i32 = -1;
    let mut builder = TypeObjectBuilder::bitmask(b.name.clone()).bit_bound(bit_bound);
    for v in &b.bit_values {
        let pos = match v.position {
            Some(p) => {
                prev = p as i32;
                p
            }
            None => {
                let p = (prev.saturating_add(1)).max(0) as u32;
                prev = p as i32;
                p
            }
        };
        let p_u16 = pos as u16;
        let _ = v as &BitValue;
        builder = builder.flag(v.name.clone(), p_u16);
    }
    builder.build_minimal()
}

// ============================================================================
// Bitset
// ============================================================================

fn bridge_bitset(b: &BitsetType) -> Result<MinimalTypeObject, BridgeError> {
    use zerodds_types::type_identifier::kinds::{
        TK_INT8, TK_INT16, TK_INT32, TK_INT64, TK_UINT8, TK_UINT16, TK_UINT32, TK_UINT64,
    };
    // Bit position runs cumulatively: each field sets prev + bitcount.
    let mut next_pos: u16 = 0;
    let mut builder = TypeObjectBuilder::bitset(b.name.clone());
    for f in &b.bit_fields {
        let bitcount = parse_bitset_mask_bits(&f.mask)?;
        let holder = match &f.type_ref {
            TypeRef::Primitive(p) => holder_kind_byte(*p).ok_or_else(|| {
                BridgeError::UnsupportedXsdConstruct(alloc::format!(
                    "bitset holder type: {}",
                    p.as_xml()
                ))
            })?,
            TypeRef::Named(_) => {
                return Err(BridgeError::UnsupportedXsdConstruct(
                    "bitset holder must be a primitive integer".to_string(),
                ));
            }
        };
        // Defensive: that the TK_ constants exist.
        debug_assert!(matches!(
            holder,
            TK_INT8 | TK_INT16 | TK_INT32 | TK_INT64 | TK_UINT8 | TK_UINT16 | TK_UINT32 | TK_UINT64
        ));
        let pos = next_pos;
        next_pos = next_pos.saturating_add(u16::from(bitcount));
        builder = builder.field(f.name.clone(), pos, bitcount, holder);
        let _ = f as &BitField;
    }
    Ok(MinimalTypeObject::Bitset(builder.build_minimal()))
}

fn holder_kind_byte(p: XmlPrimitive) -> Option<u8> {
    use zerodds_types::type_identifier::kinds::*;
    Some(match p {
        XmlPrimitive::Octet => TK_BYTE,
        XmlPrimitive::Int8 => TK_INT8,
        XmlPrimitive::UInt8 => TK_UINT8,
        XmlPrimitive::Short => TK_INT16,
        XmlPrimitive::UShort => TK_UINT16,
        XmlPrimitive::Long => TK_INT32,
        XmlPrimitive::ULong => TK_UINT32,
        XmlPrimitive::LongLong => TK_INT64,
        XmlPrimitive::ULongLong => TK_UINT64,
        XmlPrimitive::Boolean => TK_BOOLEAN,
        XmlPrimitive::Char => TK_CHAR8,
        XmlPrimitive::WChar => TK_CHAR16,
        _ => return None,
    })
}

/// Parses a bitmask such as `0x0F` into the number of set bits.
fn parse_bitset_mask_bits(mask: &str) -> Result<u8, BridgeError> {
    let trimmed = mask.trim();
    let (rest, radix) = if let Some(r) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        (r, 16u32)
    } else if let Some(r) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        (r, 2u32)
    } else {
        (trimmed, 10u32)
    };
    let v = u64::from_str_radix(rest, radix)
        .map_err(|_| BridgeError::InvalidLiteral(alloc::format!("bitset mask {mask}")))?;
    let bits = u8::try_from(v.count_ones())
        .map_err(|_| BridgeError::InvalidLiteral(alloc::format!("bitset mask too wide: {mask}")))?;
    if bits == 0 {
        return Err(BridgeError::InvalidLiteral(alloc::format!(
            "bitset mask must have >= 1 bit: {mask}"
        )));
    }
    Ok(bits)
}

// ============================================================================
// TypeRef → TypeIdentifier
// ============================================================================

/// Wraps a struct member into its final TypeIdentifier — incl.
/// `sequenceMaxLength`, `arrayDimensions`, `mapMaxLength` and
/// `stringMaxLength`.
///
/// XTypes 1.3 §7.3.2.4.4 orders the designations from most to least
/// tightly binding: sequence, array, map. A member naming several is
/// a map of arrays of sequences of the element type.
fn wrap_member_type<R: NameResolver>(
    m: &StructMember,
    res: &R,
) -> Result<TypeIdentifier, BridgeError> {
    let inner = type_ref_to_identifier_with_string_bound(&m.type_ref, m.string_max_length, res);
    let key = m
        .map_key
        .as_ref()
        .map(|k| type_ref_to_identifier_with_string_bound(k, m.map_key_string_max_length, res));
    Ok(collection_identifier(
        inner,
        m.sequence_max_length,
        &m.array_dimensions,
        key.map(|k| (k, m.map_max_length.unwrap_or(0))),
    ))
}

/// The identifier of `element` wrapped in the collections its declaration
/// names, tightest first: a sequence (`Some(bound)`, 0 unbounded), then an
/// array (`dims`), then a map (key identifier and bound).
fn collection_identifier(
    element: TypeIdentifier,
    sequence_bound: Option<u32>,
    dims: &[u32],
    map: Option<(TypeIdentifier, u32)>,
) -> TypeIdentifier {
    let mut id = element;
    if let Some(bound) = sequence_bound {
        id = wrap_sequence(id, bound);
    }
    if !dims.is_empty() {
        id = wrap_array(id, dims);
    }
    if let Some((key, bound)) = map {
        id = wrap_map(id, key, bound);
    }
    id
}

/// `map<K, V, N>` (XTypes 1.3 §7.3.2.4.3). A bound of 0 is the unbounded
/// map, as for sequences.
fn wrap_map(element: TypeIdentifier, key: TypeIdentifier, bound: u32) -> TypeIdentifier {
    if bound <= 255 {
        TypeIdentifier::PlainMapSmall {
            header: zerodds_types::PlainCollectionHeader::default(),
            bound: bound as u8,
            element: alloc::boxed::Box::new(element),
            key_flags: zerodds_types::type_identifier::CollectionElementFlag::default(),
            key: alloc::boxed::Box::new(key),
        }
    } else {
        TypeIdentifier::PlainMapLarge {
            header: zerodds_types::PlainCollectionHeader::default(),
            bound,
            element: alloc::boxed::Box::new(element),
            key_flags: zerodds_types::type_identifier::CollectionElementFlag::default(),
            key: alloc::boxed::Box::new(key),
        }
    }
}

fn wrap_sequence(element: TypeIdentifier, bound: u32) -> TypeIdentifier {
    if bound <= 255 {
        TypeIdentifier::PlainSequenceSmall {
            header: zerodds_types::PlainCollectionHeader::default(),
            bound: bound as u8,
            element: alloc::boxed::Box::new(element),
        }
    } else {
        TypeIdentifier::PlainSequenceLarge {
            header: zerodds_types::PlainCollectionHeader::default(),
            bound,
            element: alloc::boxed::Box::new(element),
        }
    }
}

fn wrap_array(element: TypeIdentifier, dims: &[u32]) -> TypeIdentifier {
    let any_large = dims.iter().any(|d| *d > 255);
    if any_large {
        TypeIdentifier::PlainArrayLarge {
            header: zerodds_types::PlainCollectionHeader::default(),
            array_bounds: dims.to_vec(),
            element: alloc::boxed::Box::new(element),
        }
    } else {
        TypeIdentifier::PlainArraySmall {
            header: zerodds_types::PlainCollectionHeader::default(),
            array_bounds: dims.iter().map(|d| *d as u8).collect(),
            element: alloc::boxed::Box::new(element),
        }
    }
}

fn type_ref_to_identifier<R: NameResolver>(t: &TypeRef, res: &R) -> TypeIdentifier {
    type_ref_to_identifier_with_string_bound(t, None, res)
}

fn type_ref_to_identifier_with_string_bound<R: NameResolver>(
    t: &TypeRef,
    string_max_length: Option<u32>,
    res: &R,
) -> TypeIdentifier {
    match t {
        TypeRef::Primitive(p) => primitive_to_identifier(*p, string_max_length),
        TypeRef::Named(n) => res.resolve(n),
    }
}

fn primitive_to_identifier(p: XmlPrimitive, bound: Option<u32>) -> TypeIdentifier {
    match p {
        XmlPrimitive::Boolean => TypeIdentifier::Primitive(PrimitiveKind::Boolean),
        XmlPrimitive::Octet => TypeIdentifier::Primitive(PrimitiveKind::Byte),
        XmlPrimitive::Int8 => TypeIdentifier::Primitive(PrimitiveKind::Int8),
        XmlPrimitive::UInt8 => TypeIdentifier::Primitive(PrimitiveKind::UInt8),
        XmlPrimitive::Char => TypeIdentifier::Primitive(PrimitiveKind::Char8),
        XmlPrimitive::WChar => TypeIdentifier::Primitive(PrimitiveKind::Char16),
        XmlPrimitive::Short => TypeIdentifier::Primitive(PrimitiveKind::Int16),
        XmlPrimitive::UShort => TypeIdentifier::Primitive(PrimitiveKind::UInt16),
        XmlPrimitive::Long => TypeIdentifier::Primitive(PrimitiveKind::Int32),
        XmlPrimitive::ULong => TypeIdentifier::Primitive(PrimitiveKind::UInt32),
        XmlPrimitive::LongLong => TypeIdentifier::Primitive(PrimitiveKind::Int64),
        XmlPrimitive::ULongLong => TypeIdentifier::Primitive(PrimitiveKind::UInt64),
        XmlPrimitive::Float => TypeIdentifier::Primitive(PrimitiveKind::Float32),
        XmlPrimitive::Double => TypeIdentifier::Primitive(PrimitiveKind::Float64),
        XmlPrimitive::LongDouble => TypeIdentifier::Primitive(PrimitiveKind::Float128),
        XmlPrimitive::String => string_id(false, bound.unwrap_or(0)),
        XmlPrimitive::WString => string_id(true, bound.unwrap_or(0)),
    }
}

fn string_id(wide: bool, bound: u32) -> TypeIdentifier {
    if wide {
        if bound <= 255 {
            TypeIdentifier::String16Small { bound: bound as u8 }
        } else {
            TypeIdentifier::String16Large { bound }
        }
    } else if bound <= 255 {
        TypeIdentifier::String8Small { bound: bound as u8 }
    } else {
        TypeIdentifier::String8Large { bound }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::xtypes_def::{
        BitField, BitValue, BitmaskType, BitsetType, EnumLiteral, EnumType, ModuleEntry,
        StructMember, StructType, TypedefType, UnionCase, UnionType,
    };

    fn primitive(p: XmlPrimitive) -> TypeRef {
        TypeRef::Primitive(p)
    }

    fn make_struct() -> StructType {
        StructType {
            name: "Position".into(),
            extensibility: Some(Extensibility::Final),
            base_type: None,
            members: alloc::vec![
                StructMember {
                    name: "x".into(),
                    type_ref: primitive(XmlPrimitive::Float),
                    key: true,
                    ..Default::default()
                },
                StructMember {
                    name: "y".into(),
                    type_ref: primitive(XmlPrimitive::Float),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    // ---- Struct ----------------------------------------------------------

    #[test]
    fn struct_two_members_lower_to_minimal_struct() {
        let s = make_struct();
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        match mto {
            MinimalTypeObject::Struct(st) => {
                assert_eq!(st.member_seq.len(), 2);
                assert!(st.struct_flags.has(StructTypeFlag::IS_FINAL));
                assert!(matches!(
                    st.member_seq[0].common.member_type_id,
                    TypeIdentifier::Primitive(PrimitiveKind::Float32)
                ));
            }
            other => panic!("expected struct, got {other:?}"),
        }
    }

    #[test]
    fn struct_explicit_id_is_preserved() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                id: Some(42),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert_eq!(st.member_seq[0].common.member_id, 42);
        } else {
            panic!("not a struct");
        }
    }

    #[test]
    fn struct_autoid_sequential_starts_at_0() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![
                StructMember {
                    name: "a".into(),
                    type_ref: primitive(XmlPrimitive::Long),
                    ..Default::default()
                },
                StructMember {
                    name: "b".into(),
                    type_ref: primitive(XmlPrimitive::Long),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            // Sequential @autoid is 0-based (XTypes §7.2.2.4.9).
            assert_eq!(st.member_seq[0].common.member_id, 0);
            assert_eq!(st.member_seq[1].common.member_id, 1);
        } else {
            panic!("not a struct");
        }
    }

    #[test]
    fn struct_member_flags_key_optional_must_understand() {
        use zerodds_types::type_object::flags::StructMemberFlag;
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                key: true,
                optional: true,
                must_understand: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            let f = st.member_seq[0].common.member_flags;
            assert!(f.has(StructMemberFlag::IS_KEY));
            assert!(f.has(StructMemberFlag::IS_OPTIONAL));
            assert!(f.has(StructMemberFlag::IS_MUST_UNDERSTAND));
        } else {
            panic!("not a struct");
        }
    }

    #[test]
    fn struct_string_member_with_bound_uses_string_small() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "name".into(),
                type_ref: primitive(XmlPrimitive::String),
                string_max_length: Some(64),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert_eq!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::String8Small { bound: 64 }
            );
        } else {
            panic!();
        }
    }

    #[test]
    fn struct_member_with_array_dimensions_wraps_into_plain_array() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                array_dimensions: alloc::vec![3, 4],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert!(matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::PlainArraySmall { .. }
            ));
        } else {
            panic!();
        }
    }

    #[test]
    fn struct_member_with_sequence_bound_wraps_into_plain_sequence() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                sequence_max_length: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert!(matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::PlainSequenceSmall { bound: 100, .. }
            ));
        } else {
            panic!();
        }
    }

    #[test]
    fn struct_member_with_large_array_uses_plain_array_large() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                array_dimensions: alloc::vec![300, 4],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert!(matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::PlainArrayLarge { .. }
            ));
        } else {
            panic!();
        }
    }

    #[test]
    fn struct_extensibility_mutable_sets_flag() {
        let mut s = make_struct();
        s.extensibility = Some(Extensibility::Mutable);
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert!(st.struct_flags.has(StructTypeFlag::IS_MUTABLE));
        } else {
            panic!();
        }
    }

    // ---- Enum ------------------------------------------------------------

    #[test]
    fn enum_with_explicit_values() {
        let e = EnumType {
            name: "Color".into(),
            bit_bound: Some(16),
            extensibility: None,
            enumerators: alloc::vec![
                EnumLiteral {
                    name: "RED".into(),
                    value: Some(0),
                    ..Default::default()
                },
                EnumLiteral {
                    name: "GREEN".into(),
                    value: Some(1),
                    ..Default::default()
                },
                EnumLiteral {
                    name: "BLUE".into(),
                    value: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Enum(e)).unwrap();
        if let MinimalTypeObject::Enumerated(en) = mto {
            assert_eq!(en.header.common.bit_bound, 16);
            assert_eq!(en.literal_seq.len(), 3);
            assert_eq!(en.literal_seq[2].common.value, 2);
        } else {
            panic!();
        }
    }

    #[test]
    fn enum_auto_numbering_starts_at_zero() {
        let e = EnumType {
            name: "X".into(),
            bit_bound: None,
            extensibility: None,
            enumerators: alloc::vec![
                EnumLiteral {
                    name: "A".into(),
                    value: None,
                    ..Default::default()
                },
                EnumLiteral {
                    name: "B".into(),
                    value: None,
                    ..Default::default()
                },
                EnumLiteral {
                    name: "C".into(),
                    value: Some(10),
                    ..Default::default()
                },
                EnumLiteral {
                    name: "D".into(),
                    value: None,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Enum(e)).unwrap();
        if let MinimalTypeObject::Enumerated(en) = mto {
            assert_eq!(en.literal_seq[0].common.value, 0);
            assert_eq!(en.literal_seq[1].common.value, 1);
            assert_eq!(en.literal_seq[2].common.value, 10);
            assert_eq!(en.literal_seq[3].common.value, 11);
            // Default bit_bound = 32.
            assert_eq!(en.header.common.bit_bound, 32);
        } else {
            panic!();
        }
    }

    // ---- Union -----------------------------------------------------------

    #[test]
    fn union_with_int_disc_and_default_branch() {
        let u = UnionType {
            name: "U".into(),
            discriminator: primitive(XmlPrimitive::Long),
            cases: alloc::vec![
                UnionCase {
                    discriminators: alloc::vec![
                        UnionDiscriminator::Value("1".into()),
                        UnionDiscriminator::Value("2".into()),
                    ],
                    member: StructMember {
                        name: "a".into(),
                        type_ref: primitive(XmlPrimitive::LongLong),
                        ..Default::default()
                    },
                },
                UnionCase {
                    discriminators: alloc::vec![UnionDiscriminator::Default],
                    member: StructMember {
                        name: "b".into(),
                        type_ref: primitive(XmlPrimitive::String),
                        string_max_length: Some(64),
                        ..Default::default()
                    },
                },
            ],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Union(u)).unwrap();
        if let MinimalTypeObject::Union(un) = mto {
            assert_eq!(un.member_seq.len(), 2);
            assert_eq!(un.member_seq[0].common.label_seq, alloc::vec![1_i32, 2]);
            assert!(un.member_seq[1].common.member_flags.0 & UnionMemberFlag::IS_DEFAULT != 0);
        } else {
            panic!();
        }
    }

    #[test]
    fn union_label_hex_parsed() {
        let u = UnionType {
            name: "U".into(),
            discriminator: primitive(XmlPrimitive::Long),
            cases: alloc::vec![UnionCase {
                discriminators: alloc::vec![UnionDiscriminator::Value("0x10".into())],
                member: StructMember {
                    name: "a".into(),
                    type_ref: primitive(XmlPrimitive::Long),
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Union(u)).unwrap();
        if let MinimalTypeObject::Union(un) = mto {
            assert_eq!(un.member_seq[0].common.label_seq, alloc::vec![16_i32]);
        } else {
            panic!();
        }
    }

    #[test]
    fn union_label_invalid_returns_invalid_literal() {
        let u = UnionType {
            name: "U".into(),
            discriminator: primitive(XmlPrimitive::Long),
            cases: alloc::vec![UnionCase {
                discriminators: alloc::vec![UnionDiscriminator::Value("not-an-int".into())],
                member: StructMember {
                    name: "a".into(),
                    type_ref: primitive(XmlPrimitive::Long),
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        let err = xml_type_to_minimal_typeobject(&TypeDef::Union(u)).unwrap_err();
        assert!(matches!(err, BridgeError::InvalidLiteral(_)));
    }

    // ---- Typedef ---------------------------------------------------------

    #[test]
    fn typedef_to_alias() {
        let t = TypedefType {
            name: "Count".into(),
            type_ref: primitive(XmlPrimitive::ULongLong),
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Typedef(t)).unwrap();
        if let MinimalTypeObject::Alias(a) = mto {
            assert_eq!(
                a.body.common.related_type,
                TypeIdentifier::Primitive(PrimitiveKind::UInt64)
            );
        } else {
            panic!("not an alias");
        }
    }

    #[test]
    fn typedef_with_array_dims_lowers_to_array_type() {
        let t = TypedefType {
            name: "Vec3".into(),
            type_ref: primitive(XmlPrimitive::Float),
            array_dimensions: alloc::vec![3],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Typedef(t)).unwrap();
        let MinimalTypeObject::Alias(a) = mto else {
            panic!("expected an alias (XTypes 1.3 §7.3.4.8)");
        };
        let TypeIdentifier::PlainArraySmall {
            array_bounds,
            element,
            ..
        } = a.body.common.related_type
        else {
            panic!("related type is not a plain array");
        };
        assert_eq!(array_bounds, alloc::vec![3u8]);
        assert_eq!(*element, TypeIdentifier::Primitive(PrimitiveKind::Float32));
    }

    #[test]
    fn typedef_with_sequence_bound_lowers_to_sequence_type() {
        let t = TypedefType {
            name: "Bytes".into(),
            type_ref: primitive(XmlPrimitive::Octet),
            sequence_max_length: Some(1024),
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Typedef(t)).unwrap();
        let MinimalTypeObject::Alias(a) = mto else {
            panic!("expected an alias (XTypes 1.3 §7.3.4.6)");
        };
        assert!(matches!(
            a.body.common.related_type,
            TypeIdentifier::PlainSequenceLarge { bound: 1024, .. }
        ));
    }

    // ---- Bitmask ---------------------------------------------------------

    #[test]
    fn bitmask_three_flags() {
        let b = BitmaskType {
            name: "Perms".into(),
            bit_bound: Some(8),
            bit_values: alloc::vec![
                BitValue {
                    name: "READ".into(),
                    position: Some(0),
                    ..Default::default()
                },
                BitValue {
                    name: "WRITE".into(),
                    position: Some(1),
                    ..Default::default()
                },
                BitValue {
                    name: "EXEC".into(),
                    position: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Bitmask(b)).unwrap();
        if let MinimalTypeObject::Bitmask(bm) = mto {
            assert_eq!(bm.bit_bound, 8);
            assert_eq!(bm.flag_seq.len(), 3);
            assert_eq!(bm.flag_seq[0].common.position, 0);
            assert_eq!(bm.flag_seq[2].common.position, 2);
        } else {
            panic!();
        }
    }

    #[test]
    fn bitmask_auto_position() {
        let b = BitmaskType {
            name: "X".into(),
            bit_bound: None,
            bit_values: alloc::vec![
                BitValue {
                    name: "A".into(),
                    position: None,
                    ..Default::default()
                },
                BitValue {
                    name: "B".into(),
                    position: None,
                    ..Default::default()
                },
                BitValue {
                    name: "C".into(),
                    position: Some(10),
                    ..Default::default()
                },
                BitValue {
                    name: "D".into(),
                    position: None,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Bitmask(b)).unwrap();
        if let MinimalTypeObject::Bitmask(bm) = mto {
            assert_eq!(bm.flag_seq[0].common.position, 0);
            assert_eq!(bm.flag_seq[1].common.position, 1);
            assert_eq!(bm.flag_seq[2].common.position, 10);
            assert_eq!(bm.flag_seq[3].common.position, 11);
            assert_eq!(bm.bit_bound, 32);
        } else {
            panic!();
        }
    }

    // ---- Bitset ----------------------------------------------------------

    #[test]
    fn bitset_with_two_fields() {
        let b = BitsetType {
            name: "Packed".into(),
            bit_fields: alloc::vec![
                BitField {
                    name: "header".into(),
                    type_ref: primitive(XmlPrimitive::ULong),
                    mask: "0x0F".into(),
                },
                BitField {
                    name: "body".into(),
                    type_ref: primitive(XmlPrimitive::ULong),
                    mask: "0xFFFFFFF0".into(),
                },
            ],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Bitset(b)).unwrap();
        if let MinimalTypeObject::Bitset(bs) = mto {
            assert_eq!(bs.field_seq.len(), 2);
            assert_eq!(bs.field_seq[0].common.bitcount, 4);
            assert_eq!(bs.field_seq[0].common.position, 0);
            assert_eq!(bs.field_seq[1].common.bitcount, 28);
            assert_eq!(bs.field_seq[1].common.position, 4);
        } else {
            panic!();
        }
    }

    #[test]
    fn bitset_invalid_mask_rejected() {
        let b = BitsetType {
            name: "X".into(),
            bit_fields: alloc::vec![BitField {
                name: "f".into(),
                type_ref: primitive(XmlPrimitive::ULong),
                mask: "garbage".into(),
            }],
        };
        let err = xml_type_to_minimal_typeobject(&TypeDef::Bitset(b)).unwrap_err();
        assert!(matches!(err, BridgeError::InvalidLiteral(_)));
    }

    #[test]
    fn bitset_zero_mask_rejected() {
        let b = BitsetType {
            name: "X".into(),
            bit_fields: alloc::vec![BitField {
                name: "f".into(),
                type_ref: primitive(XmlPrimitive::ULong),
                mask: "0x00".into(),
            }],
        };
        let err = xml_type_to_minimal_typeobject(&TypeDef::Bitset(b)).unwrap_err();
        assert!(matches!(err, BridgeError::InvalidLiteral(_)));
    }

    // ---- Module / Library -----------------------------------------------

    #[test]
    fn module_at_top_level_is_error() {
        let m = TypeDef::Module(ModuleEntry {
            name: "M".into(),
            types: alloc::vec![],
            ..Default::default()
        });
        let err = xml_type_to_minimal_typeobject(&m).unwrap_err();
        assert!(matches!(err, BridgeError::ModuleAtTopLevel(_)));
    }

    #[test]
    fn library_resolves_named_refs_between_types() {
        // struct Point { float x; float y; };
        // struct Line { Point a; Point b; };
        let lib = TypeLibrary {
            name: "Lib".into(),
            types: alloc::vec![
                TypeDef::Struct(StructType {
                    name: "Point".into(),
                    extensibility: None,
                    base_type: None,
                    members: alloc::vec![
                        StructMember {
                            name: "x".into(),
                            type_ref: primitive(XmlPrimitive::Float),
                            ..Default::default()
                        },
                        StructMember {
                            name: "y".into(),
                            type_ref: primitive(XmlPrimitive::Float),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
                TypeDef::Struct(StructType {
                    name: "Line".into(),
                    extensibility: None,
                    base_type: None,
                    members: alloc::vec![
                        StructMember {
                            name: "a".into(),
                            type_ref: TypeRef::Named("Point".into()),
                            ..Default::default()
                        },
                        StructMember {
                            name: "b".into(),
                            type_ref: TypeRef::Named("Point".into()),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
            ],
        };
        let map = bridge_library(&lib).unwrap();
        let line = map.get("Line").expect("Line registered");
        if let MinimalTypeObject::Struct(st) = line {
            // Both members should point to the same hash.
            assert_eq!(
                st.member_seq[0].common.member_type_id,
                st.member_seq[1].common.member_type_id
            );
            assert!(matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::EquivalenceHashMinimal(_)
            ));
        } else {
            panic!("Line not a struct");
        }
        // Both types in the map.
        assert!(map.contains_key("Point"));
    }

    #[test]
    fn library_module_flattens_to_scoped_name() {
        let lib = TypeLibrary {
            name: "L".into(),
            types: alloc::vec![TypeDef::Module(ModuleEntry {
                name: "Inner".into(),
                types: alloc::vec![TypeDef::Struct(StructType {
                    name: "S".into(),
                    extensibility: None,
                    base_type: None,
                    members: alloc::vec![],
                    ..Default::default()
                })],
                ..Default::default()
            })],
        };
        let map = bridge_library(&lib).unwrap();
        assert!(map.contains_key("Inner::S"));
    }

    /// A name that is no type of the library is an error naming it, not a
    /// zero hash.
    #[test]
    fn library_unknown_named_ref_is_an_error() {
        let lib = TypeLibrary {
            name: "L".into(),
            types: alloc::vec![TypeDef::Struct(StructType {
                name: "S".into(),
                members: alloc::vec![StructMember {
                    name: "missing".into(),
                    type_ref: TypeRef::Named("DoesNotExist".into()),
                    ..Default::default()
                }],
                ..Default::default()
            })],
        };
        for err in [
            bridge_library(&lib).unwrap_err(),
            bridge_library_complete(&lib).unwrap_err(),
        ] {
            match err {
                BridgeError::UnresolvedReference(m) => {
                    assert!(m.contains("DoesNotExist") && m.contains('S'), "{m}");
                }
                other => panic!("expected UnresolvedReference, got {other:?}"),
            }
        }
    }

    fn chain_library(depth_names: &[&str]) -> TypeLibrary {
        // T0 { long v }, T1 { T0 a }, T2 { T1 a }, … each referring to the
        // one before. Declared in reverse, so document order is not the
        // dependency order.
        let mut types = alloc::vec::Vec::new();
        for (i, name) in depth_names.iter().enumerate().rev() {
            let member = if i == 0 {
                StructMember {
                    name: "v".into(),
                    type_ref: TypeRef::Primitive(XmlPrimitive::Long),
                    ..Default::default()
                }
            } else {
                StructMember {
                    name: "a".into(),
                    type_ref: TypeRef::Named(depth_names[i - 1].into()),
                    ..Default::default()
                }
            };
            types.push(TypeDef::Struct(StructType {
                name: (*name).into(),
                members: alloc::vec![member],
                ..Default::default()
            }));
        }
        TypeLibrary {
            name: "L".into(),
            types,
        }
    }

    /// A reference two and three levels down carries the identifier the
    /// referenced type has in the result (the old pre-pass hashed a type
    /// before its own references were resolved).
    #[test]
    fn identifiers_equal_the_targets_hash_at_any_depth() {
        let lib = chain_library(&["T0", "T1", "T2", "T3"]);

        let minimal = bridge_library(&lib).unwrap();
        for (outer, inner) in [("T1", "T0"), ("T2", "T1"), ("T3", "T2")] {
            let MinimalTypeObject::Struct(st) = &minimal[outer] else {
                panic!("not a struct");
            };
            assert_eq!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::EquivalenceHashMinimal(
                    compute_minimal_hash(&minimal[inner]).unwrap()
                ),
                "{outer} -> {inner} (minimal)"
            );
        }

        let complete = bridge_library_complete(&lib).unwrap();
        for (outer, inner) in [("T1", "T0"), ("T2", "T1"), ("T3", "T2")] {
            let CompleteTypeObject::Struct(st) = &complete[outer] else {
                panic!("not a struct");
            };
            assert_eq!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::EquivalenceHashComplete(
                    compute_complete_hash(&complete[inner]).unwrap()
                ),
                "{outer} -> {inner} (complete)"
            );
        }
    }

    /// `<const>`, `<forward_dcl>` and `<annotation>` entries in a `<types>`
    /// string define no type; they no longer make the library fail.
    #[test]
    fn constants_and_declarations_do_not_fail_the_library() {
        let xml = r#"<types>
            <const name="N" type="int32" value="4"/>
            <annotation name="A"/>
            <forward_dcl name="S" kind="struct"/>
            <struct name="S"><member name="s" type="string" stringMaxLength="N"/></struct>
        </types>"#;
        let libs = crate::xtypes_parser::parse_type_libraries(xml).unwrap();
        let minimal = bridge_library(&libs[0]).unwrap();
        let complete = bridge_library_complete(&libs[0]).unwrap();
        assert_eq!(minimal.keys().collect::<alloc::vec::Vec<_>>(), ["S"]);
        assert_eq!(complete.keys().collect::<alloc::vec::Vec<_>>(), ["S"]);
        let CompleteTypeObject::Struct(st) = &complete["S"] else {
            panic!("not a struct");
        };
        assert_eq!(
            st.member_seq[0].common.member_type_id,
            TypeIdentifier::String8Small { bound: 4 }
        );
    }

    #[test]
    fn library_cycle_is_refused_by_name() {
        let lib = TypeLibrary {
            name: "L".into(),
            types: alloc::vec![TypeDef::Struct(StructType {
                name: "Node".into(),
                members: alloc::vec![StructMember {
                    name: "next".into(),
                    type_ref: TypeRef::Named("Node".into()),
                    ..Default::default()
                }],
                ..Default::default()
            })],
        };
        for err in [
            bridge_library(&lib).unwrap_err(),
            bridge_library_complete(&lib).unwrap_err(),
        ] {
            assert!(
                matches!(&err, BridgeError::UnsupportedXsdConstruct(m) if m.contains("Node")),
                "{err:?}"
            );
        }
    }

    // ---- TypeIdentifier-Hash --------------------------------------------

    #[test]
    fn typeobject_can_be_hashed() {
        let s = make_struct();
        let to = xml_type_to_typeobject(&TypeDef::Struct(s)).unwrap();
        let h = zerodds_types::compute_hash(&to).expect("hash");
        // Deterministic: the same result twice.
        let h2 = zerodds_types::compute_hash(&to).unwrap();
        assert_eq!(h, h2);
    }

    #[test]
    fn xml_struct_matches_idl_struct_hash() {
        // Cross-check: an XML struct and an equivalent definition built
        // directly in the builder should have the same hash
        // (provided member IDs/flags/types are identical).
        let xml_struct = StructType {
            name: "Sensor".into(),
            extensibility: Some(Extensibility::Appendable),
            base_type: None,
            members: alloc::vec![
                StructMember {
                    name: "id".into(),
                    type_ref: primitive(XmlPrimitive::LongLong),
                    key: true,
                    ..Default::default()
                },
                StructMember {
                    name: "temp".into(),
                    type_ref: primitive(XmlPrimitive::Float),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let xml_mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(xml_struct)).unwrap();
        let xml_hash = compute_minimal_hash(&xml_mto).unwrap();

        // Aequivalentes via Builder direkt:
        let builder_struct = TypeObjectBuilder::struct_type("Sensor")
            .extensibility(TypeExt::Appendable)
            .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                m.key()
            })
            .member(
                "temp",
                TypeIdentifier::Primitive(PrimitiveKind::Float32),
                |m| m,
            )
            .build_minimal();
        let builder_mto = MinimalTypeObject::Struct(builder_struct);
        let builder_hash = compute_minimal_hash(&builder_mto).unwrap();
        assert_eq!(
            xml_hash, builder_hash,
            "the XML bridge and builder must produce byte-identical TypeObjects"
        );
    }

    // ---- Display impl of BridgeError -------------------------------------

    #[test]
    fn bridge_error_display() {
        let e = BridgeError::UnsupportedXsdConstruct("foo".into());
        assert!(e.to_string().contains("foo"));
        let e = BridgeError::UnresolvedReference("Bar".into());
        assert!(e.to_string().contains("Bar"));
        let e = BridgeError::InvalidLiteral("x".into());
        assert!(e.to_string().contains("x"));
        let e = BridgeError::HashFailed("oom".into());
        assert!(e.to_string().contains("oom"));
        let e = BridgeError::ModuleAtTopLevel("M".into());
        assert!(e.to_string().contains("M"));
    }

    // ========================================================================
    // Complete-TypeObject side — against DDS-XML 1.0 Annex A / DDS-XTYPES
    // 1.3 style `<types>` XML documents, the same syntax
    // `xtypes_parser::tests` already exercises for parsing.
    // ========================================================================

    use zerodds_types::type_object::flags::StructMemberFlag;

    fn one_type(xml: &str) -> TypeDef {
        let libs = crate::xtypes_parser::parse_type_libraries(xml).expect("parse");
        libs.into_iter()
            .next()
            .expect("one library")
            .types
            .into_iter()
            .next()
            .expect("one type")
    }

    #[test]
    fn complete_struct_carries_the_key_flag_and_member_names() {
        let td = one_type(
            r#"<types>
                <struct name="State">
                    <member name="id" type="int32" key="true"/>
                    <member name="temperature" type="float64"/>
                </struct>
            </types>"#,
        );
        let TypeDef::Struct(s) = &td else {
            panic!("expected struct")
        };
        let cto = bridge_struct_complete(s, &NullResolverComplete).expect("bridge");

        assert_eq!(cto.header.detail.type_name, "State");
        assert_eq!(cto.member_seq.len(), 2);
        assert_eq!(cto.member_seq[0].detail.name, "id");
        assert!(
            cto.member_seq[0]
                .common
                .member_flags
                .has(StructMemberFlag::IS_KEY)
        );
        assert_eq!(cto.member_seq[1].detail.name, "temperature");
        assert!(
            !cto.member_seq[1]
                .common
                .member_flags
                .has(StructMemberFlag::IS_KEY)
        );
        // Default extensibility (no `extensibility=` attribute) is
        // APPENDABLE (DDS-XML 1.0 §7.2.2.4) — the same default
        // `bridge_struct`'s Minimal side reads from `Extensibility::default()`.
        assert!(cto.struct_flags.has(StructTypeFlag::IS_APPENDABLE));
    }

    #[test]
    fn complete_struct_final_extensibility_is_read_from_the_xml() {
        let td = one_type(
            r#"<types>
                <struct name="Fixed" extensibility="final">
                    <member name="x" type="int32"/>
                </struct>
            </types>"#,
        );
        let TypeDef::Struct(s) = &td else {
            panic!("expected struct")
        };
        let cto = bridge_struct_complete(s, &NullResolverComplete).expect("bridge");
        assert!(cto.struct_flags.has(StructTypeFlag::IS_FINAL));
        assert!(!cto.struct_flags.has(StructTypeFlag::IS_APPENDABLE));
    }

    #[test]
    fn complete_enum_auto_numbers_and_marks_the_first_literal_default() {
        let td = one_type(
            r#"<types>
                <enum name="Color">
                    <enumerator name="RED"/>
                    <enumerator name="GREEN"/>
                    <enumerator name="BLUE" value="9"/>
                </enum>
            </types>"#,
        );
        let TypeDef::Enum(e) = &td else {
            panic!("expected enum")
        };
        let cto = bridge_enum_complete(e).expect("enum");
        assert_eq!(cto.literal_seq.len(), 3);
        assert_eq!(cto.literal_seq[0].common.value, 0);
        assert_eq!(cto.literal_seq[1].common.value, 1);
        assert_eq!(cto.literal_seq[2].common.value, 9);
        assert_eq!(cto.literal_seq[0].detail.name, "RED");
        assert_ne!(
            cto.literal_seq[0].common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL,
            0
        );
        assert_eq!(
            cto.literal_seq[1].common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL,
            0
        );
    }

    /// A struct referencing a named enum member — the dependency-closure
    /// case `bridge_library_complete`'s two-pass resolution exists for.
    #[test]
    fn complete_library_resolves_a_struct_referencing_an_enum() {
        let xml = r#"<types>
            <enum name="Color">
                <enumerator name="RED"/>
                <enumerator name="GREEN"/>
            </enum>
            <struct name="Pixel">
                <member name="id" type="int32" key="true"/>
                <member name="color" type="nonBasic" nonBasicTypeName="Color"/>
            </struct>
        </types>"#;
        let libs = crate::xtypes_parser::parse_type_libraries(xml).expect("parse");
        let objects = bridge_library_complete(&libs[0]).expect("bridge library");

        assert_eq!(objects.len(), 2);
        let CompleteTypeObject::Enumerated(color) = &objects["Color"] else {
            panic!("expected enum")
        };
        assert_eq!(color.literal_seq.len(), 2);

        let CompleteTypeObject::Struct(pixel) = &objects["Pixel"] else {
            panic!("expected struct")
        };
        let color_member = &pixel.member_seq[1];
        assert_eq!(color_member.detail.name, "color");
        // Resolved to the real EquivalenceHashComplete of `Color`, not the
        // null-hash placeholder the pre-pass used.
        let TypeIdentifier::EquivalenceHashComplete(color_hash) =
            &color_member.common.member_type_id
        else {
            panic!("expected a resolved EquivalenceHashComplete ref")
        };
        assert_ne!(color_hash.0, [0u8; 14]);
        let expected_hash = compute_complete_hash(&objects["Color"]).expect("hash");
        assert_eq!(*color_hash, expected_hash);
    }

    #[test]
    fn complete_union_has_a_default_case_and_try_construct_set() {
        let td = one_type(
            r#"<types>
                <union name="Shape">
<discriminator type="int32"/>
                    <case>
                        <caseDiscriminator value="1"/>
                        <member name="circle_radius" type="float64"/>
                    </case>
                    <case>
                        <caseDiscriminator value="default"/>
                        <member name="none" type="int32"/>
                    </case>
                </union>
            </types>"#,
        );
        let TypeDef::Union(u) = &td else {
            panic!("expected union")
        };
        let cto = bridge_union_complete(u, &NullResolverComplete).expect("bridge");
        assert_eq!(cto.member_seq.len(), 2);
        assert_eq!(cto.member_seq[0].detail.name, "circle_radius");
        assert_eq!(cto.member_seq[0].common.label_seq, alloc::vec![1]);
        assert_ne!(
            cto.member_seq[1].common.member_flags.0 & UnionMemberFlag::IS_DEFAULT,
            0
        );
        for m in &cto.member_seq {
            assert_ne!(m.common.member_flags.0 & UnionMemberFlag::TRY_CONSTRUCT1, 0);
        }
    }

    #[test]
    fn complete_bitmask_and_bitset_build() {
        let bm_td = one_type(
            r#"<types>
                <bitmask name="Flags">
                    <flag name="A" position="0"/>
                    <flag name="B" position="1"/>
                </bitmask>
            </types>"#,
        );
        let TypeDef::Bitmask(b) = &bm_td else {
            panic!("expected bitmask")
        };
        let cto = bridge_bitmask_complete(b);
        assert_eq!(cto.detail.type_name, "Flags");

        // `<bitset>` is not in the types schema: a vendor form.
        let bs_td = crate::xtypes_parser::parse_type_libraries_with(
            r#"<types>
                <bitset name="Bits">
                    <bitfield name="a" type="byte" mask="0x0F"/>
                </bitset>
            </types>"#,
            &crate::options::ReadOptions::accept_vendor_forms(),
        )
        .expect("parse")
        .value
        .remove(0)
        .types
        .remove(0);
        let TypeDef::Bitset(b) = &bs_td else {
            panic!("expected bitset")
        };
        let cto = bridge_bitset_complete(b).expect("bridge");
        assert_eq!(cto.detail.type_name, "Bits");
    }

    /// XTypes 1.3 §7.3.4.6 and §7.3.4.8, Minimal form: a typedef of a
    /// sequence, an array or a map is an alias whose related type is the
    /// plain collection identifier; a named element is identified by its
    /// minimal hash.
    #[test]
    fn minimal_typedef_of_a_collection_is_an_alias_of_the_plain_collection() {
        let types = crate::xtypes_parser::parse_type_libraries(
            r#"<types>
                <struct name="Point"><member name="x" type="int32"/></struct>
                <typedef name="Ints" type="int32" sequenceMaxLength="10"/>
                <typedef name="Points" type="nonBasic" nonBasicTypeName="Point"
                         sequenceMaxLength="-1"/>
                <typedef name="Grid" type="float32" arrayDimensions="3,4"/>
                <typedef name="Table" type="float64" mapKeyType="string"
                         mapKeyStringMaxLength="16" mapMaxLength="300"/>
                <typedef name="Rows" type="int32" sequenceMaxLength="5" arrayDimensions="2"/>
            </types>"#,
        )
        .unwrap();
        let minimal = bridge_library(&types[0]).unwrap();
        let related = |n: &str| {
            let MinimalTypeObject::Alias(a) = &minimal[n] else {
                panic!("`{n}` is not an alias");
            };
            a.body.common.related_type.clone()
        };
        assert_eq!(
            related("Ints"),
            TypeIdentifier::PlainSequenceSmall {
                header: zerodds_types::PlainCollectionHeader::default(),
                bound: 10,
                element: alloc::boxed::Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            }
        );
        // The element of `Points` is the minimal hash of `Point`.
        let point_hash = compute_minimal_hash(&minimal["Point"]).unwrap();
        let TypeIdentifier::PlainSequenceSmall {
            bound: 0, element, ..
        } = related("Points")
        else {
            panic!("Points is not an unbounded sequence");
        };
        assert_eq!(*element, TypeIdentifier::EquivalenceHashMinimal(point_hash));
        let TypeIdentifier::PlainArraySmall { array_bounds, .. } = related("Grid") else {
            panic!("Grid is not a small array");
        };
        assert_eq!(array_bounds, alloc::vec![3u8, 4u8]);
        let TypeIdentifier::PlainMapLarge { bound, key, .. } = related("Table") else {
            panic!("Table is not a large map");
        };
        assert_eq!(bound, 300);
        assert_eq!(*key, TypeIdentifier::String8Small { bound: 16 });
        let TypeIdentifier::PlainArraySmall { element, .. } = related("Rows") else {
            panic!("Rows is not an array");
        };
        assert!(matches!(
            *element,
            TypeIdentifier::PlainSequenceSmall { bound: 5, .. }
        ));
    }

    /// XTypes 1.3 §7.3.4.6: a typedef of a sequence, an array or a map is an
    /// alias whose related type is the plain collection identifier.
    #[test]
    fn complete_typedef_of_a_collection_is_an_alias_of_the_plain_collection() {
        let types = crate::xtypes_parser::parse_type_libraries(
            r#"<types>
                <typedef name="Ints" type="int32" sequenceMaxLength="10"/>
                <typedef name="Unbounded" type="string" sequenceMaxLength="-1"/>
                <typedef name="Grid" type="float32" arrayDimensions="3,4"/>
                <typedef name="Table" type="float64" mapKeyType="string"
                         mapKeyStringMaxLength="16" mapMaxLength="300"/>
                <typedef name="Rows" type="int32" sequenceMaxLength="5" arrayDimensions="2"/>
            </types>"#,
        )
        .unwrap();
        let complete = bridge_library_complete(&types[0]).unwrap();
        let related = |n: &str| {
            let CompleteTypeObject::Alias(a) = &complete[n] else {
                panic!("`{n}` is not an alias");
            };
            a.body.related_type.clone()
        };
        let i32_id = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        assert_eq!(
            related("Ints"),
            TypeIdentifier::PlainSequenceSmall {
                header: zerodds_types::PlainCollectionHeader::default(),
                bound: 10,
                element: alloc::boxed::Box::new(i32_id.clone()),
            }
        );
        assert!(matches!(
            related("Unbounded"),
            TypeIdentifier::PlainSequenceSmall { bound: 0, .. }
        ));
        let TypeIdentifier::PlainArraySmall {
            array_bounds,
            element,
            ..
        } = related("Grid")
        else {
            panic!("Grid is not a small array");
        };
        assert_eq!(array_bounds, alloc::vec![3u8, 4u8]);
        assert_eq!(*element, TypeIdentifier::Primitive(PrimitiveKind::Float32));
        let TypeIdentifier::PlainMapLarge {
            bound,
            key,
            element,
            ..
        } = related("Table")
        else {
            panic!("Table is not a large map");
        };
        assert_eq!(bound, 300);
        assert_eq!(*key, TypeIdentifier::String8Small { bound: 16 });
        assert_eq!(*element, TypeIdentifier::Primitive(PrimitiveKind::Float64));
        // Sequence binds tighter than array: an array of sequences.
        let TypeIdentifier::PlainArraySmall { element, .. } = related("Rows") else {
            panic!("Rows is not an array");
        };
        assert!(matches!(
            *element,
            TypeIdentifier::PlainSequenceSmall { bound: 5, .. }
        ));
    }

    /// A struct member typed by such an alias refers to it by its hash, at
    /// the depth of the alias.
    #[test]
    fn complete_alias_of_a_collection_is_referable() {
        let types = crate::xtypes_parser::parse_type_libraries(
            r#"<types>
                <typedef name="Ints" type="int32" sequenceMaxLength="10"/>
                <struct name="S"><member name="v" type="nonBasic" nonBasicTypeName="Ints"/></struct>
            </types>"#,
        )
        .unwrap();
        let complete = bridge_library_complete(&types[0]).unwrap();
        let CompleteTypeObject::Struct(st) = &complete["S"] else {
            panic!("not a struct");
        };
        assert_eq!(
            st.member_seq[0].common.member_type_id,
            TypeIdentifier::EquivalenceHashComplete(
                compute_complete_hash(&complete["Ints"]).unwrap()
            )
        );
    }

    #[test]
    fn unmapped_annotations_and_flags_are_refused_by_name() {
        let cases = [
            (
                r#"<types><struct name="S"><annotate name="Doc"/><member name="a" type="int32"/></struct></types>"#,
                "Doc",
            ),
            (
                r#"<types><struct name="S"><member name="a" type="int32" nonSerialized="true"/></struct></types>"#,
                "nonSerialized",
            ),
            (
                r#"<types><enum name="E"><enumerator name="A"><annotate name="LAnn"/></enumerator></enum></types>"#,
                "LAnn",
            ),
            (
                r#"<types><bitmask name="B"><flag name="F" position="0"><annotate name="FAnn"/></flag></bitmask></types>"#,
                "FAnn",
            ),
        ];
        for (xml, needle) in cases {
            let libs = crate::xtypes_parser::parse_type_libraries(xml).unwrap();
            for err in [
                bridge_library(&libs[0]).unwrap_err(),
                bridge_library_complete(&libs[0]).unwrap_err(),
            ] {
                assert!(
                    matches!(&err, BridgeError::UnsupportedXsdConstruct(m) if m.contains(needle)),
                    "{needle}: {err:?}"
                );
            }
        }
    }

    /// `external`, `tryConstruct`, `nested`, `autoid` and `defaultLiteral`
    /// reach the flags of the TypeObject.
    #[test]
    fn external_try_construct_nested_autoid_and_default_literal_reach_the_flags() {
        let libs = crate::xtypes_parser::parse_type_libraries(
            r#"<types>
                <struct name="S" nested="true" autoid="hash">
                    <member name="a" type="int32" external="true" tryConstruct="trim"/>
                </struct>
                <enum name="E">
                    <enumerator name="A"/>
                    <enumerator name="B" defaultLiteral="true"/>
                </enum>
            </types>"#,
        )
        .unwrap();
        let complete = bridge_library_complete(&libs[0]).unwrap();
        let CompleteTypeObject::Struct(s) = &complete["S"] else {
            panic!("not a struct");
        };
        assert!(s.struct_flags.has(StructTypeFlag::IS_NESTED));
        assert!(s.struct_flags.has(StructTypeFlag::IS_AUTOID_HASH));
        let f = s.member_seq[0].common.member_flags;
        assert!(f.has(StructMemberFlag::IS_EXTERNAL));
        assert!(f.has(StructMemberFlag::TRY_CONSTRUCT1));
        assert!(f.has(StructMemberFlag::TRY_CONSTRUCT2));
        let CompleteTypeObject::Enumerated(e) = &complete["E"] else {
            panic!("not an enum");
        };
        assert!(e.literal_seq[0].common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL == 0);
        assert!(e.literal_seq[1].common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL != 0);
        let minimal = bridge_library(&libs[0]).unwrap();
        let MinimalTypeObject::Enumerated(me) = &minimal["E"] else {
            panic!("not an enum");
        };
        assert!(me.literal_seq[1].common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL != 0);
    }

    /// The extensibility of a union is its own (it was always appendable).
    #[test]
    fn union_extensibility_reaches_the_flags() {
        let libs = crate::xtypes_parser::parse_type_libraries(
            r#"<types><union name="U" extensibility="final">
                <discriminator type="int32"/>
                <case><caseDiscriminator value="0"/><member name="a" type="int32"/></case>
            </union></types>"#,
        )
        .unwrap();
        let minimal = bridge_library(&libs[0]).unwrap();
        let MinimalTypeObject::Union(u) = &minimal["U"] else {
            panic!("not a union");
        };
        assert!(u.union_flags.0 & StructTypeFlag::IS_FINAL != 0);
        let complete = bridge_library_complete(&libs[0]).unwrap();
        let CompleteTypeObject::Union(u) = &complete["U"] else {
            panic!("not a union");
        };
        assert!(u.union_flags.0 & StructTypeFlag::IS_FINAL != 0);
    }

    #[test]
    fn complete_and_minimal_agree_on_which_members_are_keyed() {
        let td = one_type(
            r#"<types>
                <struct name="Both">
                    <member name="id" type="int32" key="true"/>
                    <member name="v" type="float64"/>
                </struct>
            </types>"#,
        );
        let TypeDef::Struct(s) = &td else {
            panic!("expected struct")
        };
        let minimal = bridge_struct(s, &NullResolver).expect("minimal");
        let MinimalTypeObject::Struct(minimal) = minimal else {
            panic!("expected MinimalTypeObject::Struct")
        };
        let complete = bridge_struct_complete(s, &NullResolverComplete).expect("complete");
        assert_eq!(minimal.member_seq.len(), complete.member_seq.len());
        for (m, c) in minimal.member_seq.iter().zip(complete.member_seq.iter()) {
            assert_eq!(
                m.common.member_flags.has(StructMemberFlag::IS_KEY),
                c.common.member_flags.has(StructMemberFlag::IS_KEY)
            );
            assert_eq!(m.common.member_id, c.common.member_id);
        }
    }

    /// XTypes 1.3 §7.3.2.4.3: a member with `mapKeyType` and `mapMaxLength`
    /// is a map of the key type to the member's type.
    #[test]
    fn struct_member_with_map_wraps_into_plain_map() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "m".into(),
                type_ref: primitive(XmlPrimitive::Float),
                map_key: Some(primitive(XmlPrimitive::Long)),
                map_max_length: Some(18),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        let MinimalTypeObject::Struct(st) = mto else {
            panic!("not a struct");
        };
        let TypeIdentifier::PlainMapSmall {
            bound,
            element,
            key,
            ..
        } = &st.member_seq[0].common.member_type_id
        else {
            panic!("expected PlainMapSmall");
        };
        assert_eq!(*bound, 18);
        assert_eq!(**element, TypeIdentifier::Primitive(PrimitiveKind::Float32));
        assert_eq!(**key, TypeIdentifier::Primitive(PrimitiveKind::Int32));
    }

    /// An unbounded map (`mapMaxLength="-1"`) has bound 0, and a string key
    /// takes `mapKeyStringMaxLength`.
    #[test]
    fn struct_member_with_unbounded_map_and_bounded_string_key() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "m".into(),
                type_ref: primitive(XmlPrimitive::Long),
                map_key: Some(primitive(XmlPrimitive::String)),
                map_key_string_max_length: Some(128),
                map_max_length: None,
                ..Default::default()
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        let MinimalTypeObject::Struct(st) = mto else {
            panic!("not a struct");
        };
        let TypeIdentifier::PlainMapSmall { bound, key, .. } =
            &st.member_seq[0].common.member_type_id
        else {
            panic!("expected PlainMapSmall");
        };
        assert_eq!(*bound, 0);
        assert_eq!(**key, TypeIdentifier::String8Small { bound: 128 });
    }

    /// XTypes 1.3 §7.3.2.4.4: sequence binds tighter than array, so a
    /// member naming both is an array of sequences, neither dropped.
    #[test]
    fn struct_member_with_sequence_and_array_is_array_of_sequences() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                sequence_max_length: Some(5),
                array_dimensions: alloc::vec![3],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        let MinimalTypeObject::Struct(st) = mto else {
            panic!("not a struct");
        };
        let TypeIdentifier::PlainArraySmall {
            array_bounds,
            element,
            ..
        } = &st.member_seq[0].common.member_type_id
        else {
            panic!("expected PlainArraySmall");
        };
        assert_eq!(array_bounds, &alloc::vec![3u8]);
        assert!(matches!(
            **element,
            TypeIdentifier::PlainSequenceSmall { bound: 5, .. }
        ));
    }

    /// A map alias lowers to an alias of the map identifier.
    #[test]
    fn typedef_map_lowers_to_alias_of_plain_map() {
        let t = TypedefType {
            name: "Lookup".into(),
            type_ref: primitive(XmlPrimitive::Double),
            map_key: Some(primitive(XmlPrimitive::Long)),
            map_max_length: Some(300),
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Typedef(t)).unwrap();
        let MinimalTypeObject::Alias(a) = mto else {
            panic!("expected alias");
        };
        assert!(matches!(
            a.body.common.related_type,
            TypeIdentifier::PlainMapLarge { bound: 300, .. }
        ));
    }

    /// `int8` and `uint8` are their own primitive kinds (XTypes 1.3
    /// §7.3.4.2), not `byte`.
    #[test]
    fn int8_and_uint8_members_use_their_own_primitive_kinds() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![
                StructMember {
                    name: "s".into(),
                    type_ref: primitive(XmlPrimitive::Int8),
                    ..Default::default()
                },
                StructMember {
                    name: "u".into(),
                    type_ref: primitive(XmlPrimitive::UInt8),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        let MinimalTypeObject::Struct(st) = mto else {
            panic!("not a struct");
        };
        assert_eq!(
            st.member_seq[0].common.member_type_id,
            TypeIdentifier::Primitive(PrimitiveKind::Int8)
        );
        assert_eq!(
            st.member_seq[1].common.member_type_id,
            TypeIdentifier::Primitive(PrimitiveKind::UInt8)
        );
    }
}
