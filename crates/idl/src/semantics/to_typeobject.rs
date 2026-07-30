// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AST → TypeObject Mapper (WP 1.5 T-IDL2).
//!
//! Converts IDL `StructDef` → XTypes `TypeObject`. Primary focus
//! phase 1: struct with primitives, strings, bounded sequences.
//! Nested composite types (struct-in-struct, unions, enums as members)
//! are returned as `TypeIdentifier::EquivalenceHashMinimal(ZERO)` placeholders
//! — the caller must resolve them in the registry.

use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::type_object::minimal::MinimalStructType;
use zerodds_types::{PrimitiveKind, TypeIdentifier};

use crate::ast::{
    FloatingType, IntegerType, MapType, PrimitiveType, SequenceType, StringType, StructDef,
    TypeSpec,
};

use super::annotations::{ExtensibilityKind, lower_annotations};

/// Error during mapping.
#[derive(Debug, Clone, PartialEq)]
pub enum MapError {
    /// Non-primitive types are currently mapped as an opaque hash.
    /// If the caller does not allow that, this variant returns a
    /// hint that a real type mapper for composite types is needed.
    UnsupportedTypeSpec(&'static str),
    /// Annotation-lowering error.
    Annotation(String),
    /// Scoped-name reference to another named type (e.g.
    /// `struct S { Foo a; };`). The isolated single-type mapper cannot
    /// resolve forward refs. [`build_type_registry`] resolves them
    /// via a two-stage, topologically sorted pass.
    UnresolvedScoped(alloc::string::String),
    /// Recursive type definition (cycle in the dependency graph). The
    /// named types form a strongly-connected component that
    /// needs XTypes 1.3 §7.3.4.9.2 SCC identifiers.
    RecursiveType(alloc::string::String),
}

/// Maps an IDL `TypeSpec` to a `TypeIdentifier`.
///
/// Primitives + strings + sequences-of-primitives map directly. Scoped
/// refs + other composite types are mapped as null-hash placeholders.
///
/// zerodds-lint: recursion-depth 8
///
/// Recursion on `Sequence<...>` with a nested element TypeSpec. Realistic
/// IDL constructs nest at most 2-3 levels. The cap is not explicitly
/// enforced — the AST comes from the IDL parser (WP 0.3), which
/// itself has grammar-based limits (LR(1) stack).
///
/// # Errors
/// `UnsupportedTypeSpec` for currently non-mappable constructions.
pub fn map_type_spec(spec: &TypeSpec) -> Result<TypeIdentifier, MapError> {
    Ok(match spec {
        TypeSpec::Primitive(p) => TypeIdentifier::Primitive(map_primitive(*p)),
        TypeSpec::String(StringType { wide, bound, .. }) => {
            let bound_u32 = bound
                .as_ref()
                .and_then(|e| {
                    if let crate::ast::ConstExpr::Literal(l) = e {
                        l.raw.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if *wide {
                if bound_u32 <= 255 {
                    TypeIdentifier::String16Small {
                        bound: bound_u32 as u8,
                    }
                } else {
                    TypeIdentifier::String16Large { bound: bound_u32 }
                }
            } else if bound_u32 <= 255 {
                TypeIdentifier::String8Small {
                    bound: bound_u32 as u8,
                }
            } else {
                TypeIdentifier::String8Large { bound: bound_u32 }
            }
        }
        TypeSpec::Sequence(SequenceType { elem, bound, .. }) => {
            let element = alloc::boxed::Box::new(map_type_spec(elem)?);
            let bound_u32 = bound
                .as_ref()
                .and_then(|e| {
                    if let crate::ast::ConstExpr::Literal(l) = e {
                        l.raw.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if bound_u32 <= 255 {
                TypeIdentifier::PlainSequenceSmall {
                    header: zerodds_types::PlainCollectionHeader::for_element(
                        collection_equiv_kind(&element),
                    ),
                    bound: bound_u32 as u8,
                    element,
                }
            } else {
                TypeIdentifier::PlainSequenceLarge {
                    header: zerodds_types::PlainCollectionHeader::for_element(
                        collection_equiv_kind(&element),
                    ),
                    bound: bound_u32,
                    element,
                }
            }
        }
        TypeSpec::Scoped(s) => {
            let path = s
                .parts
                .iter()
                .map(|p| p.text.clone())
                .collect::<alloc::vec::Vec<_>>()
                .join("::");
            return Err(MapError::UnresolvedScoped(path));
        }
        TypeSpec::Map(MapType {
            key, value, bound, ..
        }) => {
            let key_ti = map_type_spec(key)?;
            let value_ti = map_type_spec(value)?;
            make_map_ti(key_ti, value_ti, literal_bound(bound))
        }
        TypeSpec::Fixed(_) => return Err(MapError::UnsupportedTypeSpec("fixed")),
        TypeSpec::Any => return Err(MapError::UnsupportedTypeSpec("any")),
    })
}

/// Builds a `PlainMap` `TypeIdentifier` (XTypes 1.3 §7.3.4.6, TK_MAP) from
/// the resolved key + value `TypeIdentifier`s and the declared bound
/// (`0` = unbounded). The map's `PlainCollectionHeader.equiv_kind` follows
/// the value element (`EK_MINIMAL` when the value is a named-type minimal
/// hash, else plain), mirroring the sequence/array convention.
fn make_map_ti(key: TypeIdentifier, value: TypeIdentifier, bound_u32: u32) -> TypeIdentifier {
    let header = zerodds_types::PlainCollectionHeader::for_element(collection_equiv_kind(&value));
    // `key_flags` is `type_identifier::CollectionElementFlag` (distinct from
    // the `type_object::flags` flag type and not re-exported at the crate
    // root); let the struct-literal infer it via `Default`.
    if bound_u32 <= u32::from(u8::MAX) {
        TypeIdentifier::PlainMapSmall {
            header,
            bound: bound_u32 as u8,
            element: alloc::boxed::Box::new(value),
            key_flags: Default::default(),
            key: alloc::boxed::Box::new(key),
        }
    } else {
        TypeIdentifier::PlainMapLarge {
            header,
            bound: bound_u32,
            element: alloc::boxed::Box::new(value),
            key_flags: Default::default(),
            key: alloc::boxed::Box::new(key),
        }
    }
}

fn map_primitive(p: PrimitiveType) -> PrimitiveKind {
    use FloatingType::*;
    use IntegerType::*;
    match p {
        PrimitiveType::Boolean => PrimitiveKind::Boolean,
        PrimitiveType::Octet => PrimitiveKind::Byte,
        PrimitiveType::Char => PrimitiveKind::Char8,
        PrimitiveType::WideChar => PrimitiveKind::Char16,
        PrimitiveType::Integer(i) => match i {
            Short | Int16 => PrimitiveKind::Int16,
            Long | Int32 => PrimitiveKind::Int32,
            LongLong | Int64 => PrimitiveKind::Int64,
            UShort | UInt16 => PrimitiveKind::UInt16,
            ULong | UInt32 => PrimitiveKind::UInt32,
            ULongLong | UInt64 => PrimitiveKind::UInt64,
            Int8 => PrimitiveKind::Int8,
            UInt8 => PrimitiveKind::UInt8,
        },
        PrimitiveType::Floating(f) => match f {
            Float => PrimitiveKind::Float32,
            Double => PrimitiveKind::Float64,
            LongDouble => PrimitiveKind::Float128,
        },
    }
}

/// Maps the frontend `@try_construct(...)` kind (XTypes 1.3 §7.2.4.2) to the
/// `zerodds_types` builder enum, which materializes the two
/// `TRY_CONSTRUCT1`/`TRY_CONSTRUCT2` member-flag bits (§7.3.1.2.1.1). This is
/// the single point where the IDL frontend value flows into the TypeObject
/// member flags; the default (no annotation) stays DISCARD at the builder.
fn map_try_construct(
    kind: super::annotations::TryConstructKind,
) -> zerodds_types::builder::TryConstruct {
    use super::annotations::TryConstructKind as K;
    use zerodds_types::builder::TryConstruct as T;
    match kind {
        K::Discard => T::Discard,
        K::UseDefault => T::UseDefault,
        K::Trim => T::Trim,
    }
}

/// Maps an IDL `StructDef` → XTypes `MinimalStructType`.
///
/// Recognizes `@key`, `@id(n)`, `@optional`, `@must_understand`, `@external`,
/// `@try_construct(...)` on members and `@final`/`@appendable`/`@mutable`/
/// `@nested`/`@extensibility(...)` on the struct.
///
/// # Errors
/// `MapError` for non-mappable constructions.
pub fn lower_struct_to_minimal(s: &StructDef) -> Result<MinimalStructType, MapError> {
    let type_annotations = lower_annotations(&s.annotations)
        .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
    let extensibility = match type_annotations.extensibility() {
        Some(ExtensibilityKind::Final) => Extensibility::Final,
        Some(ExtensibilityKind::Mutable) => Extensibility::Mutable,
        _ => Extensibility::Appendable,
    };
    let nested = type_annotations
        .builtins
        .iter()
        .any(|a| matches!(a, super::annotations::BuiltinAnnotation::Nested));
    let autoid_hash = type_annotations.builtins.iter().any(|a| {
        matches!(
            a,
            super::annotations::BuiltinAnnotation::Autoid(super::annotations::AutoidKind::Hash)
        )
    });

    let mut builder =
        TypeObjectBuilder::struct_type(s.name.text.clone()).extensibility(extensibility);
    if nested {
        builder = builder.nested();
    }
    if autoid_hash {
        builder = builder.autoid_hash();
    }

    for m in &s.members {
        // An IDL member can have multiple declarators (e.g. `long a, b;`).
        // We treat each declarator as a separate TypeObject member.
        for decl in &m.declarators {
            let ti = map_type_spec(&m.type_spec)?;
            let member_anns = lower_annotations(&m.annotations)
                .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
            // §7.2.4.4.2 — @non_serialized members are not included in the
            // TypeObject (ABI-only, no wire form, hence also
            // not in the assignability comparison).
            let is_non_serialized = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::NonSerialized));
            if is_non_serialized {
                continue;
            }
            let explicit_id = member_anns.explicit_id();
            let is_key = member_anns.has_key();
            let is_optional = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::Optional));
            let is_must_understand = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::MustUnderstand));
            let is_external = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::External));
            let hashid = member_anns.builtins.iter().find_map(|a| match a {
                super::annotations::BuiltinAnnotation::HashId(h) => Some(h.clone()),
                _ => None,
            });
            // `@try_construct(...)` (§7.2.4.2) → TRY_CONSTRUCT1/2 bits.
            let try_construct = member_anns.try_construct();
            let member_name = decl.name().text.clone();
            // `@hashid` hint (A31): explicit argument, else the member name.
            let hashid_hint = hashid.map(|h| h.unwrap_or_else(|| member_name.clone()));
            builder = builder.member(member_name, ti, |mut mb| {
                if is_key {
                    mb = mb.key();
                }
                if is_optional {
                    mb = mb.optional();
                }
                if is_must_understand {
                    mb = mb.must_understand();
                }
                if is_external {
                    mb = mb.external();
                }
                if let Some(id) = explicit_id {
                    mb = mb.id(id);
                }
                if let Some(h) = hashid_hint {
                    mb = mb.hash_id(h);
                }
                if let Some(tc) = try_construct {
                    mb = mb.try_construct(map_try_construct(tc));
                }
                mb
            });
        }
    }

    Ok(builder.build_minimal())
}

// ============================================================================
// Complete spec → TypeRegistry mapper (two-stage, topological).
// ============================================================================
//
// `map_type_spec` above fails on any scoped reference to a
// named type. The following block resolves all named types of a
// `Specification` together: first all types are collected along with their
// module scope and topologically sorted by dependency,
// then lowered in order — when a type's turn comes,
// the minimal hashes of all its dependencies are already in the
// `NameMap`, so that `map_type_spec_resolved` can insert them as
// `EquivalenceHashMinimal`.

use alloc::collections::{BTreeMap, BTreeSet};

use zerodds_types::MinimalTypeObject;
use zerodds_types::compute_minimal_hash;
use zerodds_types::resolve::TypeRegistry;

use crate::ast::{
    BitmaskDecl, BitsetDecl, ConstExpr, ConstrTypeDecl, Declarator, Definition, EnumDef,
    ScopedName, Specification, StructDcl, SwitchTypeSpec, TypeDecl, UnionDcl, UnionDef,
};

use super::annotations::BuiltinAnnotation;
use super::const_eval::{SymbolTable, evaluate};

/// Enum FQN → (literal name → ordinal value). Precomputed, so that
/// union case labels (`case Color::RED:`) can be resolved.
type EnumValues = BTreeMap<alloc::string::String, BTreeMap<alloc::string::String, i32>>;

/// EK_MINIMAL — equivalence-kind discriminator for minimal hashes
/// (XTypes 1.3 §7.3.4.5). Identical to `zerodds_types` `kinds::EK_MINIMAL`.
const EK_MINIMAL: u8 = 0xF1;
/// EK_COMPLETE — equivalence-kind discriminator for complete hashes.
const EK_COMPLETE: u8 = 0xF2;
/// EK_BOTH — the element's minimal and complete TypeIdentifiers are
/// identical (true for primitives / strings / nested plain collections).
const EK_BOTH: u8 = 0xF3;

/// Fully-qualified name → `EquivalenceHashMinimal` identifier of a
/// lowered named type.
pub type NameMap = BTreeMap<alloc::string::String, TypeIdentifier>;

/// Result of [`build_type_registry`]: all named types of a
/// `Specification` as minimal `TypeObject`s plus a resolution index.
#[derive(Debug)]
pub struct LoweredSpec {
    /// All lowered minimal `TypeObject`s, addressable by hash.
    pub registry: TypeRegistry,
    /// Fully-qualified type name → minimal-hash identifier. Contains ONLY
    /// the types that lowered successfully; a skipped type (see `skipped`)
    /// is absent.
    pub names: NameMap,
    /// Topological emission order (dependencies first). Contains only the
    /// successfully-lowered types.
    pub order: alloc::vec::Vec<alloc::string::String>,
    /// Types that could NOT be lowered, isolated per strongly-connected
    /// component (SCC): each entry is `(fqn, reason)`. A recursive cycle or
    /// an unsupported construct drops only its own SCC — plus any type that
    /// transitively depends on it — never the whole spec. Empty on a clean
    /// mapping. Consumers that require a total mapping (e.g. `dump-typeobject`)
    /// treat a non-empty `skipped` as a hard error.
    pub skipped: alloc::vec::Vec<(alloc::string::String, MapError)>,
}

/// A named type along with its module scope: `struct`, `enum`, `union` or
/// a single `typedef` declarator (alias).
enum NamedDef<'a> {
    Struct(&'a StructDef),
    Enum(&'a EnumDef),
    Union(&'a UnionDef),
    /// A `typedef` declarator. `array_sizes` empty = scalar alias,
    /// otherwise array alias with the dimensions.
    Alias {
        underlying: &'a crate::ast::TypeSpec,
        array_sizes: &'a [ConstExpr],
    },
    Bitset(&'a BitsetDecl),
    Bitmask(&'a BitmaskDecl),
}

struct NamedItem<'a> {
    /// Fully-qualified name (`Outer::Inner::Pose`).
    fqn: alloc::string::String,
    /// Module scope in which the type lives (for relative ref resolution).
    scope: alloc::vec::Vec<alloc::string::String>,
    def: NamedDef<'a>,
}

/// Lowers all named types of a `Specification` into a `TypeRegistry`.
///
/// Lowering is isolated per strongly-connected component (SCC) of the type
/// dependency graph, processed dependency-first: an independently-lowerable
/// type always keeps its minimal `TypeObject`, even when some *other* type in
/// the same spec cannot be lowered. A type is dropped (recorded in
/// [`LoweredSpec::skipped`], absent from `names`/`order`) only when
///
/// * it forms a recursive cycle the minimal lowering cannot yet express (a
///   non-trivial SCC, or a single node that references itself — XTypes 1.3
///   §7.3.4.9.2 SCC identifiers are not implemented), OR
/// * its own lowering fails (an unsupported construct), OR
/// * it transitively depends on a type that was dropped for either reason
///   above (the scoped reference then resolves to nothing → `UnresolvedScoped`).
///
/// This replaces the previous all-or-nothing behaviour where a single
/// recursive node discarded the minimal `TypeObject`s of every unrelated type
/// in the spec.
///
/// # Errors
/// Currently infallible at the whole-spec level (per-type failures are
/// collected in [`LoweredSpec::skipped`]); the `Result` is retained for
/// forward compatibility and so existing call sites keep compiling.
pub fn build_type_registry(spec: &Specification) -> Result<LoweredSpec, MapError> {
    let mut items: alloc::vec::Vec<NamedItem<'_>> = alloc::vec::Vec::new();
    collect_named(&spec.definitions, &mut alloc::vec::Vec::new(), &mut items);

    let all_fqns: BTreeSet<alloc::string::String> = items.iter().map(|it| it.fqn.clone()).collect();
    let deps: alloc::vec::Vec<alloc::vec::Vec<alloc::string::String>> = items
        .iter()
        .map(|it| dependencies_of(it, &all_fqns))
        .collect();

    let by_fqn: BTreeMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(i, it)| (it.fqn.as_str(), i))
        .collect();

    // Dependency edges in node-index space (`i` → the indices of the types
    // `i` directly references).
    let adj: alloc::vec::Vec<alloc::vec::Vec<usize>> = deps
        .iter()
        .map(|ds| {
            ds.iter()
                .filter_map(|d| by_fqn.get(d.as_str()).copied())
                .collect()
        })
        .collect();

    // Enum literal values upfront — for union case-label resolution. Isolated
    // per enum: a malformed enum is simply absent here (and will be recorded
    // as skipped when its own lowering fails), never fatal to the whole spec.
    let mut enum_values: EnumValues = EnumValues::new();
    for item in &items {
        if let NamedDef::Enum(e) = &item.def {
            if let Ok(lits) = enum_literal_values(e) {
                let mut vals = BTreeMap::new();
                for (name, value, _) in lits {
                    vals.insert(name, value);
                }
                enum_values.insert(item.fqn.clone(), vals);
            }
        }
    }

    // Kahn topo-sort over the acyclic part. `acyclic` is the dependency-first
    // emission order (byte-identical to the pre-isolation behaviour, so no
    // snapshot churn); `stuck` are the nodes left with a dependency that never
    // emits — every node in a cycle plus every node transitively depending on
    // one.
    let (acyclic, stuck) = kahn_partition(&items, &adj);

    // Which stuck nodes are themselves part of a cycle (a non-trivial SCC or a
    // self-referential node) — used only to label the drop reason accurately;
    // it does NOT drive emission order.
    let in_cycle = cycle_membership(&adj);

    let mut registry = TypeRegistry::new();
    let mut names = NameMap::new();
    let mut order = alloc::vec::Vec::new();
    let mut skipped: alloc::vec::Vec<(alloc::string::String, MapError)> = alloc::vec::Vec::new();

    // Lower the acyclic part in dependency-first order. Each type is isolated:
    // a failure (an unsupported construct, or a dependency dropped earlier →
    // `UnresolvedScoped`) removes only this type; independent types keep their
    // `TypeObject`. Because dependencies precede dependents, a dropped type
    // cascades to its dependents naturally.
    for &i in &acyclic {
        let item = &items[i];
        let lowered = lower_named(item, &names, &enum_values).and_then(|mto| {
            let hash = compute_minimal_hash(&mto)
                .map_err(|e| MapError::Annotation(alloc::format!("hash failed: {e:?}")))?;
            Ok((mto, hash))
        });
        match lowered {
            Ok((mto, hash)) => {
                registry.insert_minimal(hash, mto);
                names.insert(
                    item.fqn.clone(),
                    TypeIdentifier::EquivalenceHashMinimal(hash),
                );
                order.push(item.fqn.clone());
            }
            Err(e) => skipped.push((item.fqn.clone(), e)),
        }
    }

    // Record the cyclic remainder. A node in a cycle cannot be expressed by the
    // minimal lowering (XTypes 1.3 §7.3.4.9.2 SCC identifiers are unimplemented)
    // → `RecursiveType`; a node merely depending on such a cycle carries the
    // genuine `UnresolvedScoped` (its missing dependency) from `lower_named`.
    for &i in &stuck {
        let item = &items[i];
        let reason = if in_cycle[i] {
            MapError::RecursiveType(item.fqn.clone())
        } else {
            lower_named(item, &names, &enum_values)
                .err()
                .unwrap_or_else(|| MapError::RecursiveType(item.fqn.clone()))
        };
        skipped.push((item.fqn.clone(), reason));
    }

    Ok(LoweredSpec {
        registry,
        names,
        order,
        skipped,
    })
}

/// Kahn topological partition of the dependency graph `adj` (`i` → the indices
/// of the types `i` depends on). Returns `(acyclic, stuck)` where `acyclic` is
/// the dependency-first order of every node that can be topologically placed —
/// identical to the historical `topo_sort` output, tie-broken by declaration
/// index — and `stuck` is every remaining node (in or transitively depending
/// on a cycle), in declaration order.
fn kahn_partition(
    items: &[NamedItem<'_>],
    adj: &[alloc::vec::Vec<usize>],
) -> (alloc::vec::Vec<usize>, alloc::vec::Vec<usize>) {
    let n = items.len();
    let mut indegree = alloc::vec![0usize; n];
    let mut dependents: alloc::vec::Vec<alloc::vec::Vec<usize>> =
        alloc::vec![alloc::vec::Vec::new(); n];
    for (i, ds) in adj.iter().enumerate() {
        for &di in ds {
            dependents[di].push(i);
            indegree[i] += 1;
        }
    }
    let mut queue: alloc::vec::Vec<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    let mut acyclic = alloc::vec::Vec::with_capacity(n);
    let mut head = 0;
    while head < queue.len() {
        let cur = queue[head];
        head += 1;
        acyclic.push(cur);
        for &dep in &dependents[cur] {
            indegree[dep] -= 1;
            if indegree[dep] == 0 {
                queue.push(dep);
            }
        }
    }
    let stuck: alloc::vec::Vec<usize> = (0..n).filter(|&i| indegree[i] > 0).collect();
    (acyclic, stuck)
}

/// `in_cycle[i]` is true iff node `i` belongs to a strongly-connected component
/// of size > 1 or references itself — i.e. it sits ON a dependency cycle (as
/// opposed to merely depending on one). Computed via
/// [`strongly_connected_components`]; used only to label drop reasons.
fn cycle_membership(adj: &[alloc::vec::Vec<usize>]) -> alloc::vec::Vec<bool> {
    let mut flags = alloc::vec![false; adj.len()];
    for scc in strongly_connected_components(adj) {
        let recursive = scc.len() > 1 || (scc.len() == 1 && adj[scc[0]].contains(&scc[0]));
        if recursive {
            for i in scc {
                flags[i] = true;
            }
        }
    }
    flags
}

impl LoweredSpec {
    /// Iterates the lowered types in topological order
    /// (dependencies first) as `(fqn, &MinimalTypeObject)`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &MinimalTypeObject)> {
        self.order.iter().filter_map(move |fqn| {
            let TypeIdentifier::EquivalenceHashMinimal(hash) = self.names.get(fqn)? else {
                return None;
            };
            self.registry
                .get_minimal(hash)
                .map(|obj| (fqn.as_str(), obj))
        })
    }
}

/// Builds the spec-complete `CompleteStructType` of an IDL struct, resolving
/// each member's type (typedef / enum / sequence / map / nested-struct / union /
/// array declarator) via the full-spec `names` index produced by
/// [`build_type_registry`].
///
/// This is the SINGLE source for the COMPLETE `TypeObject` a codegen backend
/// embeds and hashes for its `TYPE_IDENTIFIER` (F-TYPES-3 / #24): `idl-rust`
/// (Path B, `struct_type_identifier_expr`) and `idl-cpp` (`type_object()`) both
/// call it, so their emitted TypeObject bytes are byte-identical for the same
/// IDL — the cross-binding-parity requirement of #24.
///
/// Struct flags (`@final` / `@appendable` / `@mutable`, `@nested`,
/// `@autoid(HASH)`) and member flags (`@key`, `@optional`, `@must_understand`,
/// `@external`, `@id(n)`, `@unit`) plus member-id assignment (incl. the
/// `@autoid(HASH)` NameHash branch) are applied through
/// `zerodds_types::builder::StructBuilder::build_complete` — the SAME builder
/// "Path A" (`lower_struct_to_minimal`) uses — never a hand-rolled flag copy
/// that can drift from it.
///
/// `@non_serialized` members (XTypes 1.3 §7.2.4.4.2) are skipped here, exactly
/// as in the Minimal path (`lower_struct_to_minimal` /
/// `lower_struct_minimal_resolved`): the member keeps its in-memory slot in the
/// generated language type but appears in NEITHER the Minimal nor the Complete
/// TypeObject, so the two carry the identical member set and the emitted
/// `TYPE_IDENTIFIER` no longer covers it (broad-audit P0-5, #2 (a) — the changed
/// TypeIdentifier is the intended rc correction). Members that remain keep the
/// member-id assignment the builder resolves (P0-3): dropping a `@non_serialized`
/// member does not renumber the survivors, since ids come from `@id`/`@hashid`/
/// autoid, not the loop position.
///
/// # Errors
/// `MapError` if a member type cannot be resolved (`fixed`/`any`, or a scoped
/// reference absent from `names`), an array dimension is non-constant, or
/// annotation lowering fails.
pub fn build_complete_struct_type(
    s: &StructDef,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<zerodds_types::type_object::complete::CompleteStructType, MapError> {
    use super::annotations::AutoidKind;

    let type_anns = lower_annotations(&s.annotations)
        .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
    let extensibility = match type_anns.extensibility() {
        Some(ExtensibilityKind::Final) => Extensibility::Final,
        Some(ExtensibilityKind::Mutable) => Extensibility::Mutable,
        _ => Extensibility::Appendable,
    };
    let nested = type_anns
        .builtins
        .iter()
        .any(|a| matches!(a, BuiltinAnnotation::Nested));
    let autoid_hash = type_anns
        .builtins
        .iter()
        .any(|a| matches!(a, BuiltinAnnotation::Autoid(AutoidKind::Hash)));

    // The COMPLETE TypeObject serializes the qualified type name
    // (`type_name` in `CompleteTypeDetail`, §7.3.4.5.4). It must be the FQN
    // (`Alpha::SameShape`), not the simple name — otherwise two identically
    // shaped structs in different modules hash to the same TypeIdentifier.
    let fqn = join_fqn(scope, &s.name.text);
    let mut builder = TypeObjectBuilder::struct_type(fqn).extensibility(extensibility);
    if nested {
        builder = builder.nested();
    }
    if autoid_hash {
        builder = builder.autoid_hash();
    }

    for member in &s.members {
        // §7.2.4.4.2 — `@non_serialized` members are excluded from the Complete
        // TypeObject too (broad-audit P0-5, #2), so it carries the identical
        // member set as the Minimal path (`lower_struct_to_minimal`); the
        // `continue` before the builder call also keeps the sequential-autoid
        // counter from advancing, compacting the survivors' ids.
        if super::annotations::member_is_non_serialized(&member.annotations) {
            continue;
        }
        let member_anns = lower_annotations(&member.annotations)
            .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
        let is_key = member_anns.has_key();
        let is_optional = member_anns
            .builtins
            .iter()
            .any(|a| matches!(a, BuiltinAnnotation::Optional));
        let is_must_understand = member_anns
            .builtins
            .iter()
            .any(|a| matches!(a, BuiltinAnnotation::MustUnderstand));
        let is_external = member_anns
            .builtins
            .iter()
            .any(|a| matches!(a, BuiltinAnnotation::External));
        let explicit_id = member_anns.explicit_id();
        let unit = member_anns.builtins.iter().find_map(|a| match a {
            BuiltinAnnotation::Unit(u) => Some(u.clone()),
            _ => None,
        });
        // `@hashid` / `@hashid("hint")` (A31) — the outer Option is presence,
        // the inner is the explicit hint (absent = hash the member's own name).
        let hashid = member_anns.builtins.iter().find_map(|a| match a {
            BuiltinAnnotation::HashId(h) => Some(h.clone()),
            _ => None,
        });
        // `@default(value)` (A33) — carried in the complete TypeObject.
        let default_value = member_anns.builtins.iter().find_map(|a| match a {
            BuiltinAnnotation::Default(v) => Some(v.clone()),
            _ => None,
        });
        // `@try_construct(...)` (§7.2.4.2) → TRY_CONSTRUCT1/2 bits.
        let try_construct = member_anns.try_construct();
        // `@min` / `@max` / `@range(min=,max=)` (A34) — the last write wins so a
        // standalone `@min`/`@max` overrides the corresponding `@range` field.
        let (mut min_lit, mut max_lit): (Option<String>, Option<String>) = (None, None);
        for a in &member_anns.builtins {
            match a {
                BuiltinAnnotation::Min(v) => min_lit = Some(v.clone()),
                BuiltinAnnotation::Max(v) => max_lit = Some(v.clone()),
                BuiltinAnnotation::Range { min, max } => {
                    if let Some(m) = min {
                        min_lit = Some(m.clone());
                    }
                    if let Some(m) = max {
                        max_lit = Some(m.clone());
                    }
                }
                _ => {}
            }
        }
        let base_type_id = map_type_spec_resolved(&member.type_spec, scope, names)?;

        for decl in &member.declarators {
            let name = decl.name().text.clone();
            let member_type_id = match decl {
                Declarator::Simple(_) => base_type_id.clone(),
                Declarator::Array(a) => make_array_ti(base_type_id.clone(), &a.sizes)?,
            };
            let unit = unit.clone();
            // The effective hash hint: explicit argument, else the member name.
            let hashid_hint = hashid.clone().map(|h| h.unwrap_or_else(|| name.clone()));
            let default_value = default_value.clone();
            // `@min`/`@max` land in the complete TypeObject as opaque bytes
            // (§7.3.4.5.4); the frontend preserves the source literal verbatim.
            let min_bytes = min_lit.clone().map(String::into_bytes);
            let max_bytes = max_lit.clone().map(String::into_bytes);
            builder = builder.member(name, member_type_id, move |mut mb| {
                if is_key {
                    mb = mb.key();
                }
                if is_optional {
                    mb = mb.optional();
                }
                if is_must_understand {
                    mb = mb.must_understand();
                }
                if is_external {
                    mb = mb.external();
                }
                if let Some(id) = explicit_id {
                    mb = mb.id(id);
                }
                if let Some(u) = unit {
                    mb = mb.unit(u);
                }
                if let Some(h) = hashid_hint {
                    mb = mb.hash_id(h);
                }
                if let Some(d) = default_value {
                    mb = mb.set_member_default(d);
                }
                if let Some(b) = min_bytes {
                    mb = mb.min_bytes(b);
                }
                if let Some(b) = max_bytes {
                    mb = mb.max_bytes(b);
                }
                if let Some(tc) = try_construct {
                    mb = mb.try_construct(map_try_construct(tc));
                }
                mb
            });
        }
    }

    Ok(builder.build_complete())
}

/// Serializes the spec-complete `TypeObject` of an IDL struct to its XCDR-LE
/// `to_bytes_le` byte form — the exact bytes a codegen backend embeds as a
/// constant and hands to `zerodds_*_create_typed` (F-TYPES-3 / #24). Wraps
/// [`build_complete_struct_type`] in `TypeObject::Complete` and encodes it.
///
/// SINGLE source: `idl-rust`'s `TYPE_IDENTIFIER` codegen and `idl-cpp`'s
/// `type_object()` both call this, so the two bindings emit byte-identical
/// TypeObject constants (and hence the identical `TypeIdentifier`).
///
/// # Errors
/// `MapError` from [`build_complete_struct_type`], or an encode overflow.
pub fn complete_struct_type_object_bytes(
    s: &StructDef,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<alloc::vec::Vec<u8>, MapError> {
    let cs = build_complete_struct_type(s, scope, names)?;
    let to = zerodds_types::type_object::TypeObject::Complete(
        zerodds_types::type_object::CompleteTypeObject::Struct(cs),
    );
    to.to_bytes_le()
        .map_err(|e| MapError::Annotation(alloc::format!("TypeObject encode failed: {e:?}")))
}

/// Recursive module walk, collects struct/enum along with the scope.
///
/// zerodds-lint: recursion-depth 64 (parser/AST walk; bounded by IDL nesting)
fn collect_named<'a>(
    defs: &'a [Definition],
    scope: &mut alloc::vec::Vec<alloc::string::String>,
    out: &mut alloc::vec::Vec<NamedItem<'a>>,
) {
    for def in defs {
        match def {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                collect_named(&m.definitions, scope, out);
                scope.pop();
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                out.push(NamedItem {
                    fqn: join_fqn(scope, &s.name.text),
                    scope: scope.clone(),
                    def: NamedDef::Struct(s),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                out.push(NamedItem {
                    fqn: join_fqn(scope, &e.name.text),
                    scope: scope.clone(),
                    def: NamedDef::Enum(e),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                out.push(NamedItem {
                    fqn: join_fqn(scope, &u.name.text),
                    scope: scope.clone(),
                    def: NamedDef::Union(u),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
                out.push(NamedItem {
                    fqn: join_fqn(scope, &b.name.text),
                    scope: scope.clone(),
                    def: NamedDef::Bitset(b),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => {
                out.push(NamedItem {
                    fqn: join_fqn(scope, &b.name.text),
                    scope: scope.clone(),
                    def: NamedDef::Bitmask(b),
                });
            }
            Definition::Type(TypeDecl::Typedef(td)) => {
                // A typedef has 1..n declarators — each is its
                // own named alias.
                for decl in &td.declarators {
                    let (name, sizes): (&str, &[ConstExpr]) = match decl {
                        Declarator::Simple(id) => (&id.text, &[]),
                        Declarator::Array(ad) => (&ad.name.text, &ad.sizes),
                    };
                    out.push(NamedItem {
                        fqn: join_fqn(scope, name),
                        scope: scope.clone(),
                        def: NamedDef::Alias {
                            underlying: &td.type_spec,
                            array_sizes: sizes,
                        },
                    });
                }
            }
            _ => {}
        }
    }
}

/// `["Outer","Inner"] + "Pose"` → `"Outer::Inner::Pose"`.
fn join_fqn(scope: &[alloc::string::String], name: &str) -> alloc::string::String {
    if scope.is_empty() {
        name.into()
    } else {
        alloc::format!("{}::{name}", scope.join("::"))
    }
}

/// FQNs of all named types that `item` directly depends on.
fn dependencies_of(
    item: &NamedItem<'_>,
    all: &BTreeSet<alloc::string::String>,
) -> alloc::vec::Vec<alloc::string::String> {
    let mut refs: alloc::vec::Vec<&ScopedName> = alloc::vec::Vec::new();
    match &item.def {
        NamedDef::Struct(s) => {
            for m in &s.members {
                collect_scoped_refs(&m.type_spec, &mut refs);
            }
        }
        NamedDef::Union(u) => {
            if let SwitchTypeSpec::Scoped(sn) = &u.switch_type {
                refs.push(sn);
            }
            for case in &u.cases {
                collect_scoped_refs(&case.element.type_spec, &mut refs);
            }
        }
        NamedDef::Alias { underlying, .. } => {
            collect_scoped_refs(underlying, &mut refs);
        }
        NamedDef::Enum(_) => {}
        // Bitset bitfields / bitmask flags reference only primitive holder
        // types (or the width-derived default) — never another named type.
        NamedDef::Bitset(_) | NamedDef::Bitmask(_) => {}
    }
    let mut deps = alloc::vec::Vec::new();
    for sn in refs {
        if let Some(fqn) = resolve_fqn(sn, &item.scope, all) {
            if !deps.contains(&fqn) {
                deps.push(fqn);
            }
        }
    }
    deps
}

/// Collects all scoped references in a `TypeSpec` (recursively through
/// sequences + maps).
///
/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_scoped_refs<'a>(ts: &'a TypeSpec, out: &mut alloc::vec::Vec<&'a ScopedName>) {
    match ts {
        TypeSpec::Scoped(sn) => out.push(sn),
        TypeSpec::Sequence(s) => collect_scoped_refs(&s.elem, out),
        TypeSpec::Map(m) => {
            collect_scoped_refs(&m.key, out);
            collect_scoped_refs(&m.value, out);
        }
        _ => {}
    }
}

/// String-based FQN resolution: innermost scope first (IDL name-
/// lookup rule). `known` checks whether a candidate is a known name.
fn resolve_str_fqn(
    target: &str,
    absolute: bool,
    scope: &[alloc::string::String],
    known: impl Fn(&str) -> bool,
) -> Option<alloc::string::String> {
    if absolute {
        return known(target).then(|| target.to_string());
    }
    for k in (0..=scope.len()).rev() {
        let candidate = if k == 0 {
            target.to_string()
        } else {
            alloc::format!("{}::{target}", scope[..k].join("::"))
        };
        if known(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Resolves a scoped reference against the set of known FQNs.
fn resolve_fqn(
    sn: &ScopedName,
    scope: &[alloc::string::String],
    all: &BTreeSet<alloc::string::String>,
) -> Option<alloc::string::String> {
    let target = sn
        .parts
        .iter()
        .map(|p| p.text.as_str())
        .collect::<alloc::vec::Vec<_>>()
        .join("::");
    resolve_str_fqn(&target, sn.absolute, scope, |c| all.contains(c))
}

/// Resolves a scoped reference against the `NameMap` — returns the
/// minimal-hash identifier of the target type.
fn resolve_scoped(
    sn: &ScopedName,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Option<TypeIdentifier> {
    let target = sn
        .parts
        .iter()
        .map(|p| p.text.as_str())
        .collect::<alloc::vec::Vec<_>>()
        .join("::");
    if sn.absolute {
        return names.get(&target).cloned();
    }
    for k in (0..=scope.len()).rev() {
        let candidate = if k == 0 {
            target.clone()
        } else {
            alloc::format!("{}::{target}", scope[..k].join("::"))
        };
        if let Some(ti) = names.get(&candidate) {
            return Some(ti.clone());
        }
    }
    None
}

/// Tarjan's strongly-connected-components, iterative (an explicit work stack
/// instead of recursion, so a deep dependency chain cannot overflow the call
/// stack and no recursion-depth budget applies).
///
/// `adj[i]` holds the indices of the types `i` directly depends on. The result
/// is the list of SCCs in **reverse-topological order over those dependency
/// edges**: for an edge `i → dep` the component containing `dep` is emitted
/// before the component containing `i`. Because a dependency is emitted first,
/// iterating the result lowers every type after all of its dependencies —
/// exactly the order [`build_type_registry`] needs — while still grouping each
/// recursive cycle into a single component the caller can drop as a unit.
fn strongly_connected_components(
    adj: &[alloc::vec::Vec<usize>],
) -> alloc::vec::Vec<alloc::vec::Vec<usize>> {
    let n = adj.len();
    // `usize::MAX` = not yet discovered.
    let mut index = alloc::vec![usize::MAX; n];
    let mut lowlink = alloc::vec![0usize; n];
    let mut on_stack = alloc::vec![false; n];
    let mut tarjan_stack: alloc::vec::Vec<usize> = alloc::vec::Vec::new();
    let mut next_index = 0usize;
    let mut sccs: alloc::vec::Vec<alloc::vec::Vec<usize>> = alloc::vec::Vec::new();

    for start in 0..n {
        if index[start] != usize::MAX {
            continue;
        }
        // Explicit DFS: each frame is `(node, next-child cursor)`.
        let mut work: alloc::vec::Vec<(usize, usize)> = alloc::vec![(start, 0)];
        while let Some(&(v, ci)) = work.last() {
            if ci == 0 {
                // First entry into `v`.
                index[v] = next_index;
                lowlink[v] = next_index;
                next_index += 1;
                tarjan_stack.push(v);
                on_stack[v] = true;
            }
            if ci < adj[v].len() {
                if let Some(frame) = work.last_mut() {
                    frame.1 = ci + 1;
                }
                let w = adj[v][ci];
                if index[w] == usize::MAX {
                    work.push((w, 0));
                } else if on_stack[w] {
                    lowlink[v] = lowlink[v].min(index[w]);
                }
            } else {
                // `v` is fully explored. If it is the root of an SCC, pop it.
                if lowlink[v] == index[v] {
                    let mut comp = alloc::vec::Vec::new();
                    while let Some(w) = tarjan_stack.pop() {
                        on_stack[w] = false;
                        comp.push(w);
                        if w == v {
                            break;
                        }
                    }
                    sccs.push(comp);
                }
                work.pop();
                // Fold `v`'s low-link into its parent frame.
                if let Some(&(parent, _)) = work.last() {
                    lowlink[parent] = lowlink[parent].min(lowlink[v]);
                }
            }
        }
    }
    sccs
}

/// Lowers a collected type into its minimal `TypeObject`.
fn lower_named(
    item: &NamedItem<'_>,
    names: &NameMap,
    enum_values: &EnumValues,
) -> Result<MinimalTypeObject, MapError> {
    // The type name carried into every minimal builder is the fully-qualified
    // name (`Alpha::SameShape`), never the simple name — otherwise two
    // structurally identical types in different modules collide. `builder.rs`
    // treats the passed name as the qualified type name (§7.3.4.5.4).
    match &item.def {
        NamedDef::Struct(s) => Ok(MinimalTypeObject::Struct(lower_struct_minimal_resolved(
            s,
            &item.fqn,
            &item.scope,
            names,
        )?)),
        NamedDef::Enum(e) => Ok(MinimalTypeObject::Enumerated(lower_enum_minimal(
            e, &item.fqn,
        )?)),
        NamedDef::Union(u) => Ok(MinimalTypeObject::Union(lower_union_minimal(
            u,
            &item.fqn,
            &item.scope,
            names,
            enum_values,
        )?)),
        NamedDef::Alias {
            underlying,
            array_sizes,
        } => Ok(MinimalTypeObject::Alias(lower_alias_minimal(
            &item.fqn,
            underlying,
            array_sizes,
            &item.scope,
            names,
        )?)),
        NamedDef::Bitmask(b) => Ok(MinimalTypeObject::Bitmask(lower_bitmask_minimal(
            b, &item.fqn,
        )?)),
        NamedDef::Bitset(b) => Ok(MinimalTypeObject::Bitset(lower_bitset_minimal(
            b, &item.fqn,
        )?)),
    }
}

/// `map_type_spec` with resolution of scoped references via the `NameMap`.
///
/// Public (beyond this module's own [`build_type_registry`] pipeline) so
/// codegen backends with access to the full [`Specification`]'s resolved
/// [`NameMap`] can compute a spec-complete `TypeIdentifier` for an
/// individual member — e.g. `idl-rust`'s `TYPE_IDENTIFIER` codegen
/// (F-TYPES-3 / #24), which otherwise falls back to a lossy
/// primitive/string-only subset for typedef/enum/sequence/map/nested-struct
/// members.
///
/// zerodds-lint: recursion-depth 64 (parser/AST walk; bounded by IDL nesting)
///
/// # Errors
/// `UnresolvedScoped` if `ts` (or a nested element/key/value) references a
/// named type not present in `names`; `UnsupportedTypeSpec` for `fixed`/`any`.
pub fn map_type_spec_resolved(
    ts: &TypeSpec,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<TypeIdentifier, MapError> {
    match ts {
        TypeSpec::Scoped(sn) => resolve_scoped(sn, scope, names).ok_or_else(|| {
            MapError::UnresolvedScoped(
                sn.parts
                    .iter()
                    .map(|p| p.text.clone())
                    .collect::<alloc::vec::Vec<_>>()
                    .join("::"),
            )
        }),
        TypeSpec::Sequence(SequenceType { elem, bound, .. }) => {
            let element = map_type_spec_resolved(elem, scope, names)?;
            let bound_u32 = literal_bound(bound);
            let header =
                zerodds_types::PlainCollectionHeader::for_element(collection_equiv_kind(&element));
            Ok(if bound_u32 <= u32::from(u8::MAX) {
                TypeIdentifier::PlainSequenceSmall {
                    header,
                    bound: bound_u32 as u8,
                    element: alloc::boxed::Box::new(element),
                }
            } else {
                TypeIdentifier::PlainSequenceLarge {
                    header,
                    bound: bound_u32,
                    element: alloc::boxed::Box::new(element),
                }
            })
        }
        TypeSpec::Map(MapType {
            key, value, bound, ..
        }) => {
            // §7.3.4.6 TK_MAP — resolve key + value against the NameMap so a
            // map<K,V> with named-type key/value emits a proper map
            // TypeIdentifier instead of skipping the whole struct's TypeObject.
            let key_ti = map_type_spec_resolved(key, scope, names)?;
            let value_ti = map_type_spec_resolved(value, scope, names)?;
            Ok(make_map_ti(key_ti, value_ti, literal_bound(bound)))
        }
        // Primitives + strings + Fixed/Any: identical to the isolated mapper
        // (no resolution needed, or unsupported).
        other => map_type_spec(other),
    }
}

/// `bound` `ConstExpr` as `u32` (0 = unbounded), integer literals only.
fn literal_bound(bound: &Option<crate::ast::ConstExpr>) -> u32 {
    bound
        .as_ref()
        .and_then(|e| {
            if let crate::ast::ConstExpr::Literal(l) = e {
                l.raw.parse::<u32>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

/// Equiv kind for a plain-collection header — `EK_MINIMAL` if
/// the element is a minimal hash, otherwise 0 (primitive/plain).
fn collection_equiv_kind(element: &TypeIdentifier) -> u8 {
    match element {
        TypeIdentifier::EquivalenceHashMinimal(_) => EK_MINIMAL,
        TypeIdentifier::EquivalenceHashComplete(_) => EK_COMPLETE,
        // Fully-descriptive elements (primitives, strings, nested plain
        // collections) are identical in the minimal and complete graphs →
        // EK_BOTH (0xF3). Byte-verified against Cyclone + FastDDS.
        _ => EK_BOTH,
    }
}

/// Like [`lower_struct_to_minimal`], but resolves scoped member types
/// via the `NameMap`.
fn lower_struct_minimal_resolved(
    s: &StructDef,
    fqn: &str,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<MinimalStructType, MapError> {
    let type_annotations = lower_annotations(&s.annotations)
        .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
    let extensibility = match type_annotations.extensibility() {
        Some(ExtensibilityKind::Final) => Extensibility::Final,
        Some(ExtensibilityKind::Mutable) => Extensibility::Mutable,
        _ => Extensibility::Appendable,
    };
    let nested = type_annotations
        .builtins
        .iter()
        .any(|a| matches!(a, BuiltinAnnotation::Nested));
    let autoid_hash = type_annotations.builtins.iter().any(|a| {
        matches!(
            a,
            BuiltinAnnotation::Autoid(super::annotations::AutoidKind::Hash)
        )
    });

    let mut builder = TypeObjectBuilder::struct_type(fqn).extensibility(extensibility);
    if nested {
        builder = builder.nested();
    }
    if autoid_hash {
        builder = builder.autoid_hash();
    }

    for m in &s.members {
        for decl in &m.declarators {
            let ti = map_type_spec_resolved(&m.type_spec, scope, names)?;
            let member_anns = lower_annotations(&m.annotations)
                .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
            if member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::NonSerialized))
            {
                continue;
            }
            let explicit_id = member_anns.explicit_id();
            let is_key = member_anns.has_key();
            let is_optional = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::Optional));
            let is_must_understand = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::MustUnderstand));
            let is_external = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::External));
            let hashid = member_anns.builtins.iter().find_map(|a| match a {
                BuiltinAnnotation::HashId(h) => Some(h.clone()),
                _ => None,
            });
            // `@try_construct(...)` (§7.2.4.2) → TRY_CONSTRUCT1/2 bits.
            let try_construct = member_anns.try_construct();
            let member_name = decl.name().text.clone();
            // `@hashid` hint (A31): explicit argument, else the member name.
            let hashid_hint = hashid.map(|h| h.unwrap_or_else(|| member_name.clone()));
            builder = builder.member(member_name, ti, |mut mb| {
                if is_key {
                    mb = mb.key();
                }
                if is_optional {
                    mb = mb.optional();
                }
                if is_must_understand {
                    mb = mb.must_understand();
                }
                if is_external {
                    mb = mb.external();
                }
                if let Some(id) = explicit_id {
                    mb = mb.id(id);
                }
                if let Some(h) = hashid_hint {
                    mb = mb.hash_id(h);
                }
                if let Some(tc) = try_construct {
                    mb = mb.try_construct(map_try_construct(tc));
                }
                mb
            });
        }
    }
    Ok(builder.build_minimal())
}

/// Lowers an IDL `enum` into a minimal `MinimalEnumeratedType`.
/// Ordinal values sequentially from 0; `@value(n)` sets explicitly,
/// `@default_literal` marks the default; `@bit_bound(n)` the
/// bit width.
fn lower_enum_minimal(
    e: &EnumDef,
    fqn: &str,
) -> Result<zerodds_types::type_object::minimal::MinimalEnumeratedType, MapError> {
    let type_anns = lower_annotations(&e.annotations)
        .map_err(|err| MapError::Annotation(alloc::format!("{err:?}")))?;
    let bit_bound = type_anns.builtins.iter().find_map(|a| {
        if let BuiltinAnnotation::BitBound(n) = a {
            Some(*n)
        } else {
            None
        }
    });
    // `@ignore_literal_names` (XTypes 1.3 §7.2.4.4.7) → EnumTypeFlag bit.
    let ignore_literal_names = type_anns
        .builtins
        .iter()
        .any(|a| matches!(a, BuiltinAnnotation::IgnoreLiteralNames));

    let mut builder = TypeObjectBuilder::enum_type(fqn).ignore_literal_names(ignore_literal_names);
    if let Some(bits) = bit_bound {
        builder = builder.bit_bound(bits);
    }
    for (name, value, is_default) in enum_literal_values(e)? {
        builder = if is_default {
            builder.default_literal(name, value)
        } else {
            builder.literal(name, value)
        };
    }
    Ok(builder.build_minimal())
}

/// Computes the ordinal values of the enumerators: sequentially from 0,
/// `@value(n)` sets explicitly (subsequent ones count from `n+1`),
/// `@default_literal` marks the default literal.
fn enum_literal_values(
    e: &EnumDef,
) -> Result<alloc::vec::Vec<(alloc::string::String, i32, bool)>, MapError> {
    let mut out = alloc::vec::Vec::with_capacity(e.enumerators.len());
    let mut next: i32 = 0;
    for en in &e.enumerators {
        let anns = lower_annotations(&en.annotations)
            .map_err(|err| MapError::Annotation(alloc::format!("{err:?}")))?;
        let mut value = next;
        let mut is_default = false;
        for a in &anns.builtins {
            match a {
                BuiltinAnnotation::Value(s) => {
                    if let Ok(v) = s.parse::<i32>() {
                        value = v;
                    }
                }
                BuiltinAnnotation::DefaultLiteral => is_default = true,
                _ => {}
            }
        }
        out.push((en.name.text.clone(), value, is_default));
        next = value.wrapping_add(1);
    }
    Ok(out)
}

/// Lowers an IDL `bitmask` into a minimal `MinimalBitmaskType`.
///
/// `@bit_bound(n)` sets the holder width (default 32, XTypes §7.3.1.2.1.1);
/// `@position(n)` sets a flag's bit position explicitly, otherwise it runs
/// sequentially from the previous position + 1 (starting at 0) — mirroring the
/// XML `typeobject_bridge` and Cyclone/RTI wire behaviour.
fn lower_bitmask_minimal(
    b: &BitmaskDecl,
    fqn: &str,
) -> Result<zerodds_types::type_object::minimal::MinimalBitmaskType, MapError> {
    let type_anns = lower_annotations(&b.annotations)
        .map_err(|err| MapError::Annotation(alloc::format!("{err:?}")))?;
    let bit_bound = type_anns.builtins.iter().find_map(|a| {
        if let BuiltinAnnotation::BitBound(n) = a {
            Some(*n)
        } else {
            None
        }
    });

    let mut builder = TypeObjectBuilder::bitmask(fqn);
    if let Some(bits) = bit_bound {
        builder = builder.bit_bound(bits);
    }

    let mut prev: i64 = -1;
    for v in &b.values {
        let anns = lower_annotations(&v.annotations)
            .map_err(|err| MapError::Annotation(alloc::format!("{err:?}")))?;
        let explicit = anns.builtins.iter().find_map(|a| {
            if let BuiltinAnnotation::Position(p) = a {
                Some(*p)
            } else {
                None
            }
        });
        let position = match explicit {
            Some(p) => p,
            None => u32::try_from(prev + 1).unwrap_or(0),
        };
        prev = i64::from(position);
        let p_u16 = u16::try_from(position).map_err(|_| {
            MapError::Annotation(alloc::format!(
                "bitmask position {position} exceeds u16 range"
            ))
        })?;
        builder = builder.flag(v.name.text.clone(), p_u16);
    }
    Ok(builder.build_minimal())
}

/// Lowers an IDL `bitset` into a minimal `MinimalBitsetType`.
///
/// Each bitfield occupies a cumulative bit position (running offset). The
/// holder type is the bitfield's explicit destination type (`bitfield<N, T>`)
/// or — absent one — the smallest unsigned integer covering the width
/// (≤8 → uint8, ≤16 → uint16, ≤32 → uint32, else uint64), matching the
/// `bitset_storage_type` buckets used by the codegen backends. Anonymous
/// padding bitfields advance the position but emit no member (XTypes §7.3.4.4).
fn lower_bitset_minimal(
    b: &BitsetDecl,
    fqn: &str,
) -> Result<zerodds_types::type_object::minimal::MinimalBitsetType, MapError> {
    let mut builder = TypeObjectBuilder::bitset(fqn);
    let mut next_pos: u16 = 0;
    for f in &b.bitfields {
        let width = bitfield_width(&f.spec.width)?;
        let bitcount = u8::try_from(width).map_err(|_| {
            MapError::Annotation(alloc::format!("bitset bitfield width {width} exceeds 64"))
        })?;
        let holder = match f.spec.dest_type {
            Some(dt) => map_primitive(dt).to_u8(),
            None => default_bitset_holder(width).to_u8(),
        };
        let pos = next_pos;
        next_pos = next_pos.saturating_add(u16::from(bitcount));
        // Anonymous padding bitfields advance the offset but are not members.
        if let Some(name) = &f.name {
            builder = builder.field(name.text.clone(), pos, bitcount, holder);
        }
    }
    Ok(builder.build_minimal())
}

/// A bitset bitfield's width as `u32` — integer literal only (mirrors the
/// convention of [`literal_bound`] and `bitfield_validation`).
fn bitfield_width(e: &ConstExpr) -> Result<u32, MapError> {
    if let ConstExpr::Literal(l) = e {
        if let Ok(v) = l.raw.parse::<u32>() {
            return Ok(v);
        }
    }
    Err(MapError::Annotation(
        "bitset bitfield width must be an integer literal".into(),
    ))
}

/// Default holder type for an un-typed `bitfield<N>`: smallest unsigned
/// integer able to hold `width` bits.
fn default_bitset_holder(width: u32) -> PrimitiveKind {
    match width {
        0..=8 => PrimitiveKind::UInt8,
        9..=16 => PrimitiveKind::UInt16,
        17..=32 => PrimitiveKind::UInt32,
        _ => PrimitiveKind::UInt64,
    }
}

/// Discriminator `TypeIdentifier` of a `union switch (...)`.
fn switch_type_id(
    sw: &SwitchTypeSpec,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<TypeIdentifier, MapError> {
    Ok(match sw {
        SwitchTypeSpec::Integer(i) => {
            TypeIdentifier::Primitive(map_primitive(PrimitiveType::Integer(*i)))
        }
        SwitchTypeSpec::Char => TypeIdentifier::Primitive(PrimitiveKind::Char8),
        SwitchTypeSpec::Boolean => TypeIdentifier::Primitive(PrimitiveKind::Boolean),
        SwitchTypeSpec::Octet => TypeIdentifier::Primitive(PrimitiveKind::Byte),
        SwitchTypeSpec::Scoped(sn) => resolve_scoped(sn, scope, names).ok_or_else(|| {
            MapError::UnresolvedScoped(
                sn.parts
                    .iter()
                    .map(|p| p.text.clone())
                    .collect::<alloc::vec::Vec<_>>()
                    .join("::"),
            )
        })?,
    })
}

/// FQN of a union's discriminator enum, if the switch type is a
/// (resolved) enum — for resolving blank case labels.
fn switch_enum_fqn(
    sw: &SwitchTypeSpec,
    scope: &[alloc::string::String],
    enum_values: &EnumValues,
) -> Option<alloc::string::String> {
    let SwitchTypeSpec::Scoped(sn) = sw else {
        return None;
    };
    let target = sn
        .parts
        .iter()
        .map(|p| p.text.as_str())
        .collect::<alloc::vec::Vec<_>>()
        .join("::");
    resolve_str_fqn(&target, sn.absolute, scope, |c| enum_values.contains_key(c))
}

/// Lowers an IDL `union` into a `MinimalUnionType`.
fn lower_union_minimal(
    u: &UnionDef,
    fqn: &str,
    scope: &[alloc::string::String],
    names: &NameMap,
    enum_values: &EnumValues,
) -> Result<zerodds_types::type_object::minimal::MinimalUnionType, MapError> {
    let disc = switch_type_id(&u.switch_type, scope, names)?;
    let disc_enum = switch_enum_fqn(&u.switch_type, scope, enum_values);

    let type_anns = lower_annotations(&u.annotations)
        .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
    let extensibility = match type_anns.extensibility() {
        Some(ExtensibilityKind::Final) => Extensibility::Final,
        Some(ExtensibilityKind::Mutable) => Extensibility::Mutable,
        _ => Extensibility::Appendable,
    };

    let mut builder = TypeObjectBuilder::union_type(fqn, disc).extensibility(extensibility);
    for case in &u.cases {
        let elem_ti = map_type_spec_resolved(&case.element.type_spec, scope, names)?;
        let name = case.element.declarator.name().text.clone();
        if case
            .labels
            .iter()
            .any(|l| matches!(l, crate::ast::CaseLabel::Default))
        {
            builder = builder.default_case(name, elem_ti);
        } else {
            let mut labels = alloc::vec::Vec::new();
            for label in &case.labels {
                if let crate::ast::CaseLabel::Value(expr) = label {
                    labels.push(eval_case_label(
                        expr,
                        scope,
                        disc_enum.as_deref(),
                        enum_values,
                    )?);
                }
            }
            builder = builder.case(name, elem_ti, labels);
        }
    }
    Ok(builder.build_minimal())
}

/// Evaluates a union case label to an `i32`. Integer/char/
/// boolean literals via the const evaluator; enum labels (`Color::RED`
/// or blank `RED`) via the precomputed `EnumValues` map.
fn eval_case_label(
    expr: &ConstExpr,
    scope: &[alloc::string::String],
    disc_enum: Option<&str>,
    enum_values: &EnumValues,
) -> Result<i32, MapError> {
    if let ConstExpr::Scoped(sn) = expr {
        return resolve_enum_label(sn, scope, disc_enum, enum_values);
    }
    let value = evaluate(expr, &SymbolTable::new())
        .map_err(|e| MapError::Annotation(alloc::format!("union case label: {e:?}")))?;
    value
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or(MapError::UnsupportedTypeSpec(
            "union case label is not an integer",
        ))
}

/// Resolves an enum case label. Blank labels (`case RED:`) against
/// the discriminator enum, qualified ones (`case Color::RED:`) against
/// the enum determined via the prefix.
fn resolve_enum_label(
    sn: &ScopedName,
    scope: &[alloc::string::String],
    disc_enum: Option<&str>,
    enum_values: &EnumValues,
) -> Result<i32, MapError> {
    let Some((last, prefix)) = sn.parts.split_last() else {
        return Err(MapError::UnsupportedTypeSpec("empty union case label"));
    };
    let literal = &last.text;
    let enum_fqn = if prefix.is_empty() {
        disc_enum.map(alloc::string::String::from)
    } else {
        let target = prefix
            .iter()
            .map(|p| p.text.as_str())
            .collect::<alloc::vec::Vec<_>>()
            .join("::");
        resolve_str_fqn(&target, sn.absolute, scope, |c| enum_values.contains_key(c))
    };
    enum_fqn
        .as_deref()
        .and_then(|fqn| enum_values.get(fqn))
        .and_then(|m| m.get(literal))
        .copied()
        .ok_or_else(|| MapError::UnresolvedScoped(literal.clone()))
}

/// Lowers a `typedef` declarator into a `MinimalAliasType`.
fn lower_alias_minimal(
    fqn: &str,
    underlying: &crate::ast::TypeSpec,
    array_sizes: &[ConstExpr],
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<zerodds_types::type_object::minimal::MinimalAliasType, MapError> {
    let element = map_type_spec_resolved(underlying, scope, names)?;
    let target = if array_sizes.is_empty() {
        element
    } else {
        make_array_ti(element, array_sizes)?
    };
    Ok(TypeObjectBuilder::alias(fqn.to_string(), target).build_minimal())
}

/// Builds a `PlainArray` `TypeIdentifier` from element + dimensions
/// (XTypes 1.3 §7.3.4.6, TK_ARRAY). Public so codegen backends can wrap a
/// codegen-time-resolved element `TypeIdentifier` for an array declarator
/// (`T name[N][M]`) instead of discarding the dimensions — see
/// [`map_type_spec_resolved`].
///
/// # Errors
/// `UnsupportedTypeSpec` if a dimension does not evaluate to a
/// non-negative integer.
pub fn make_array_ti(
    element: TypeIdentifier,
    sizes: &[ConstExpr],
) -> Result<TypeIdentifier, MapError> {
    let mut dims = alloc::vec::Vec::with_capacity(sizes.len());
    for s in sizes {
        dims.push(eval_const_u32(s)?);
    }
    let header = zerodds_types::PlainCollectionHeader::for_element(collection_equiv_kind(&element));
    Ok(if dims.iter().all(|&d| d <= u32::from(u8::MAX)) {
        TypeIdentifier::PlainArraySmall {
            header,
            array_bounds: dims.iter().map(|&d| d as u8).collect(),
            element: alloc::boxed::Box::new(element),
        }
    } else {
        TypeIdentifier::PlainArrayLarge {
            header,
            array_bounds: dims,
            element: alloc::boxed::Box::new(element),
        }
    })
}

/// Evaluates a `ConstExpr` to a `u32` (array dimension).
fn eval_const_u32(expr: &ConstExpr) -> Result<u32, MapError> {
    let value = evaluate(expr, &SymbolTable::new())
        .map_err(|e| MapError::Annotation(alloc::format!("array bound: {e:?}")))?;
    value
        .as_i64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(MapError::UnsupportedTypeSpec(
            "array dimension is not a non-negative integer",
        ))
}

extern crate alloc;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn first_struct(src: &str) -> StructDef {
        let ast = parse(src, &ParserConfig::default()).expect("parse");
        for def in ast.definitions {
            if let crate::ast::Definition::Type(crate::ast::TypeDecl::Constr(
                crate::ast::ConstrTypeDecl::Struct(crate::ast::StructDcl::Def(s)),
            )) = def
            {
                return s;
            }
        }
        panic!("no struct");
    }

    #[test]
    fn simple_struct_lowers() {
        let s = first_struct("struct S { long id; string text; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 2);
        assert!(matches!(
            st.member_seq[0].common.member_type_id,
            TypeIdentifier::Primitive(PrimitiveKind::Int32)
        ));
        assert!(matches!(
            st.member_seq[1].common.member_type_id,
            TypeIdentifier::String8Small { .. }
        ));
    }

    #[test]
    fn key_and_id_annotations_carry_through() {
        let s = first_struct("struct S { @key @id(5) long id; long extra; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq[0].common.member_id, 5);
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_KEY)
        );
    }

    #[test]
    fn mutable_extensibility_applied() {
        let s = first_struct("@mutable struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_MUTABLE)
        );
    }

    #[test]
    fn sequence_of_ints_lowers() {
        let s = first_struct("struct S { sequence<long, 10> items; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(matches!(
            st.member_seq[0].common.member_type_id,
            TypeIdentifier::PlainSequenceSmall { bound: 10, .. }
        ));
    }

    #[test]
    fn optional_annotation_sets_flag() {
        let s = first_struct("struct S { @optional long maybe; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_OPTIONAL)
        );
    }

    #[test]
    fn non_serialized_member_is_dropped_from_typeobject() {
        // §7.2.4.4.2 — @non_serialized members are ABI-only, not in the
        // wire form and not in the assignability comparison.
        let s = first_struct(
            "struct S { long visible; @non_serialized long hidden; long also_visible; };",
        );
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 2);
        // Member order is preserved (visible, also_visible).
        // No ID annotation → IDs are allocated; just ensure
        // that the hidden member is missing.
        // Since we cannot easily grab a name list, we check
        // that the type-identifier type is correct and the length fits.
    }

    #[test]
    fn non_serialized_in_otherwise_empty_struct_yields_empty_member_seq() {
        let s = first_struct("struct S { @non_serialized long internal_only; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 0);
    }

    #[test]
    fn non_serialized_member_does_not_block_assignability() {
        // The reader has @non_serialized for a member; the writer does not
        // have it at all. In the wire comparison (assignability), both
        // member sequences should be identical → no blocking
        // mismatch.
        let writer_src = "struct S { long a; long b; };";
        let reader_src = "struct S { long a; @non_serialized long debug_only; long b; };";
        let writer = lower_struct_to_minimal(&first_struct(writer_src)).unwrap();
        let reader = lower_struct_to_minimal(&first_struct(reader_src)).unwrap();
        // Both must have the same count + the same member-identifier sequence.
        assert_eq!(writer.member_seq.len(), reader.member_seq.len());
        for (w, r) in writer.member_seq.iter().zip(&reader.member_seq) {
            assert_eq!(w.common.member_type_id, r.common.member_type_id);
        }
    }

    #[test]
    fn non_serialized_with_other_annotations_still_dropped() {
        // @key + @non_serialized — non_serialized takes precedence, the member is dropped.
        let s = first_struct("struct S { @key @non_serialized long ghost; long real; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 1);
        assert!(
            !st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_KEY)
        );
    }

    // ---- map_type_spec: every Primitive variant --------------------------

    use crate::ast::Identifier;
    use crate::ast::{
        ConstExpr, FixedPtType, FloatingType, IntegerType, Literal, LiteralKind, MapType,
        PrimitiveType, ScopedName, SequenceType, StringType, TypeSpec,
    };
    use crate::errors::Span;

    fn sp() -> Span {
        Span::SYNTHETIC
    }

    fn int_lit(raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    fn prim(p: PrimitiveType) -> TypeSpec {
        TypeSpec::Primitive(p)
    }

    #[test]
    fn map_primitive_all_integer_kinds() {
        let cases = [
            (IntegerType::Short, PrimitiveKind::Int16),
            (IntegerType::Long, PrimitiveKind::Int32),
            (IntegerType::LongLong, PrimitiveKind::Int64),
            (IntegerType::UShort, PrimitiveKind::UInt16),
            (IntegerType::ULong, PrimitiveKind::UInt32),
            (IntegerType::ULongLong, PrimitiveKind::UInt64),
            (IntegerType::Int8, PrimitiveKind::Int8),
            (IntegerType::Int16, PrimitiveKind::Int16),
            (IntegerType::Int32, PrimitiveKind::Int32),
            (IntegerType::Int64, PrimitiveKind::Int64),
            (IntegerType::UInt8, PrimitiveKind::UInt8),
            (IntegerType::UInt16, PrimitiveKind::UInt16),
            (IntegerType::UInt32, PrimitiveKind::UInt32),
            (IntegerType::UInt64, PrimitiveKind::UInt64),
        ];
        for (idl, xt) in cases {
            let ti = map_type_spec(&prim(PrimitiveType::Integer(idl))).unwrap();
            assert_eq!(ti, TypeIdentifier::Primitive(xt));
        }
    }

    #[test]
    fn map_primitive_floats_boolean_char_octet() {
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Floating(FloatingType::Float))).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Float32)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Floating(FloatingType::Double))).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Float64)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Floating(FloatingType::LongDouble))).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Float128)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Boolean)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Boolean)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Octet)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Byte)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Char)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Char8)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::WideChar)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Char16)
        );
    }

    // ---- String kinds ----------------------------------------------------

    #[test]
    fn map_string_unbounded_narrow_is_small_with_bound_zero() {
        let ti = map_type_spec(&TypeSpec::String(StringType {
            wide: false,
            bound: None,
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(ti, TypeIdentifier::String8Small { bound: 0 }));
    }

    #[test]
    fn map_string_bounded_narrow_small() {
        let ti = map_type_spec(&TypeSpec::String(StringType {
            wide: false,
            bound: Some(int_lit("128")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(ti, TypeIdentifier::String8Small { bound: 128 });
    }

    #[test]
    fn map_string_large_narrow_over_255() {
        let ti = map_type_spec(&TypeSpec::String(StringType {
            wide: false,
            bound: Some(int_lit("1000")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(ti, TypeIdentifier::String8Large { bound: 1000 });
    }

    #[test]
    fn map_string_wide_small_and_large() {
        let wide_small = map_type_spec(&TypeSpec::String(StringType {
            wide: true,
            bound: Some(int_lit("64")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(wide_small, TypeIdentifier::String16Small { bound: 64 });

        let wide_large = map_type_spec(&TypeSpec::String(StringType {
            wide: true,
            bound: Some(int_lit("70000")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(wide_large, TypeIdentifier::String16Large { bound: 70000 });
    }

    // ---- Sequence kinds --------------------------------------------------

    #[test]
    fn map_sequence_small_and_large_bounds() {
        let small = map_type_spec(&TypeSpec::Sequence(SequenceType {
            elem: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: Some(int_lit("100")),
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            small,
            TypeIdentifier::PlainSequenceSmall { bound: 100, .. }
        ));

        let large = map_type_spec(&TypeSpec::Sequence(SequenceType {
            elem: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: Some(int_lit("1000")),
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            large,
            TypeIdentifier::PlainSequenceLarge { bound: 1000, .. }
        ));
    }

    #[test]
    fn map_sequence_unbounded_is_small_with_bound_zero() {
        let ti = map_type_spec(&TypeSpec::Sequence(SequenceType {
            elem: alloc::boxed::Box::new(prim(PrimitiveType::Boolean)),
            bound: None,
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            ti,
            TypeIdentifier::PlainSequenceSmall { bound: 0, .. }
        ));
    }

    // ---- Error paths -----------------------------------------------------

    #[test]
    fn map_scoped_returns_unresolved_scoped() {
        let scoped = ScopedName {
            absolute: false,
            parts: alloc::vec![Identifier::new("Ns", sp()), Identifier::new("Inner", sp()),],
            span: sp(),
        };
        let err = map_type_spec(&TypeSpec::Scoped(scoped)).unwrap_err();
        match err {
            MapError::UnresolvedScoped(p) => assert_eq!(p, "Ns::Inner"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn map_fixed_is_unsupported() {
        let err = map_type_spec(&TypeSpec::Fixed(FixedPtType {
            digits: int_lit("10"),
            scale: int_lit("2"),
            span: sp(),
        }))
        .unwrap_err();
        assert_eq!(err, MapError::UnsupportedTypeSpec("fixed"));
    }

    #[test]
    fn map_now_lowers_to_plain_map() {
        // Bug TO (#71): inline map<K,V> previously returned
        // UnsupportedTypeSpec("map (inline IDL map)"); it now lowers to a
        // TK_MAP PlainMap TypeIdentifier (see map_of_primitives_lowers_to_plain_map).
        let ti = map_type_spec(&TypeSpec::Map(MapType {
            key: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            value: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: None,
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(ti, TypeIdentifier::PlainMapSmall { .. }));
    }

    #[test]
    fn map_any_is_unsupported() {
        let err = map_type_spec(&TypeSpec::Any).unwrap_err();
        assert_eq!(err, MapError::UnsupportedTypeSpec("any"));
    }

    // ---- Bug TO (#71): inline map<K,V> → TK_MAP TypeIdentifier -----------

    #[test]
    fn map_of_primitives_lowers_to_plain_map() {
        // map<long,long> (unbounded) → PlainMapSmall(bound=0) with the key +
        // value TypeIdentifiers populated. Previously this returned
        // UnsupportedTypeSpec("map (inline IDL map)").
        let ti = map_type_spec(&TypeSpec::Map(MapType {
            key: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            value: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: None,
            span: sp(),
        }))
        .unwrap();
        match ti {
            TypeIdentifier::PlainMapSmall {
                bound,
                element,
                key,
                ..
            } => {
                assert_eq!(bound, 0);
                assert_eq!(*key, TypeIdentifier::Primitive(PrimitiveKind::Int32));
                assert_eq!(*element, TypeIdentifier::Primitive(PrimitiveKind::Int32));
            }
            other => panic!("expected PlainMapSmall, got {other:?}"),
        }
    }

    #[test]
    fn map_string_key_long_value_lowers() {
        // map<string,long> — the exact shape of 13_maps / 20_mixed_combo.
        let ti = map_type_spec(&TypeSpec::Map(MapType {
            key: alloc::boxed::Box::new(TypeSpec::String(StringType {
                wide: false,
                bound: None,
                span: sp(),
            })),
            value: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: None,
            span: sp(),
        }))
        .unwrap();
        match ti {
            TypeIdentifier::PlainMapSmall { key, element, .. } => {
                assert!(matches!(*key, TypeIdentifier::String8Small { .. }));
                assert_eq!(*element, TypeIdentifier::Primitive(PrimitiveKind::Int32));
            }
            other => panic!("expected PlainMapSmall, got {other:?}"),
        }
    }

    #[test]
    fn map_large_bound_uses_plain_map_large() {
        let ti = map_type_spec(&TypeSpec::Map(MapType {
            key: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            value: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: Some(int_lit("1000")),
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            ti,
            TypeIdentifier::PlainMapLarge { bound: 1000, .. }
        ));
    }

    #[test]
    fn registry_struct_with_map_member_emits_typeobject() {
        // The whole-struct TypeObject must NOT be skipped just because a
        // member is a map. Mirrors 13_maps.idl: a keyed struct with a
        // map<string,long> member must lower to a real TypeObject.
        let lowered =
            registry("@appendable struct Maps { @key long id; map<string,long> counters; };");
        assert!(lowered.names.contains_key("Maps"));
        // And it serialises cleanly (this is what idlc's blob emission does).
        let (_, obj) = lowered.iter().find(|(n, _)| *n == "Maps").unwrap();
        let bytes = obj.to_bytes_le().expect("serialise map-bearing TypeObject");
        assert!(!bytes.is_empty());
    }

    #[test]
    fn registry_map_member_carries_map_type_id() {
        // The map member's TypeIdentifier inside the struct TypeObject is a
        // PlainMap (TK_MAP), not a placeholder/skip.
        let lowered = registry("struct M { map<long,long> table; };");
        let (_, obj) = lowered.iter().find(|(n, _)| *n == "M").unwrap();
        let MinimalTypeObject::Struct(st) = obj else {
            panic!("expected struct TypeObject");
        };
        assert_eq!(st.member_seq.len(), 1);
        assert!(
            matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::PlainMapSmall { .. } | TypeIdentifier::PlainMapLarge { .. }
            ),
            "map member must carry a TK_MAP TypeIdentifier, got {:?}",
            st.member_seq[0].common.member_type_id
        );
    }

    #[test]
    fn registry_map_of_named_value_resolves_and_sets_equiv_kind() {
        // map<long, Point> — the value is a named struct, so the map's
        // value element must resolve to a minimal hash and the collection
        // header's equiv_kind flips to EK_MINIMAL.
        let lowered =
            registry("struct Point { long x; long y; }; struct Grid { map<long,Point> cells; };");
        let (_, obj) = lowered.iter().find(|(n, _)| *n == "Grid").unwrap();
        let MinimalTypeObject::Struct(st) = obj else {
            panic!("expected struct TypeObject");
        };
        match &st.member_seq[0].common.member_type_id {
            TypeIdentifier::PlainMapSmall {
                header, element, ..
            }
            | TypeIdentifier::PlainMapLarge {
                header, element, ..
            } => {
                assert!(matches!(
                    **element,
                    TypeIdentifier::EquivalenceHashMinimal(_)
                ));
                assert_eq!(header.equiv_kind, EK_MINIMAL);
            }
            other => panic!("expected PlainMap, got {other:?}"),
        }
    }

    // ---- lower_struct_to_minimal struct-level annotations ----------------

    #[test]
    fn struct_autoid_hash_sets_flag() {
        let s = first_struct("@autoid(HASH) struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_AUTOID_HASH)
        );
    }

    #[test]
    fn struct_nested_sets_flag() {
        let s = first_struct("@nested struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_NESTED)
        );
    }

    fn first_enum(src: &str) -> EnumDef {
        let ast = parse(src, &ParserConfig::default()).expect("parse");
        for def in ast.definitions {
            if let crate::ast::Definition::Type(crate::ast::TypeDecl::Constr(
                crate::ast::ConstrTypeDecl::Enum(e),
            )) = def
            {
                return e;
            }
        }
        panic!("no enum");
    }

    // ---- A31 @hashid: member-id derived from MD5(hint) -------------------

    #[test]
    fn hashid_with_hint_derives_member_id_minimal() {
        use zerodds_types::type_object::common::NameHash;
        // MD5("my_hint")[0..4] LE & 0x0FFFFFFF == 0x026C50E0.
        let s = first_struct("struct S { @hashid(\"my_hint\") long v; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq[0].common.member_id, 0x026C_50E0);
        assert_eq!(
            st.member_seq[0].common.member_id,
            NameHash::member_id_from_name("my_hint")
        );
    }

    #[test]
    fn bare_hashid_hashes_member_name_minimal() {
        use zerodds_types::type_object::common::NameHash;
        // A bare @hashid hashes the member's own name ("color" → 0x0FA5DD70).
        let s = first_struct("struct S { @hashid long color; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq[0].common.member_id, 0x0FA5_DD70);
        assert_eq!(
            st.member_seq[0].common.member_id,
            NameHash::member_id_from_name("color")
        );
    }

    #[test]
    fn hashid_derives_member_id_and_detail_complete() {
        use zerodds_types::type_object::common::NameHash;
        let s = first_struct("struct S { @hashid(\"my_hint\") long v; };");
        let cs = build_complete_struct_type(&s, &[], &NameMap::new()).unwrap();
        assert_eq!(
            cs.member_seq[0].common.member_id,
            NameHash::member_id_from_name("my_hint")
        );
        assert_eq!(
            cs.member_seq[0].detail.ann_builtin.hash_id.as_deref(),
            Some("my_hint")
        );
    }

    // ---- A33 @default: carried into the complete TypeObject --------------

    #[test]
    fn default_value_lands_in_complete_typeobject() {
        let s = first_struct("struct S { @default(7) long v; };");
        let cs = build_complete_struct_type(&s, &[], &NameMap::new()).unwrap();
        assert_eq!(
            cs.member_seq[0].detail.ann_builtin.default_value.as_deref(),
            Some("7")
        );
    }

    // ---- A34 @min/@max/@range: carried into the complete TypeObject ------

    #[test]
    fn min_max_land_in_complete_typeobject() {
        let s = first_struct("struct S { @min(0) @max(100) long v; };");
        let cs = build_complete_struct_type(&s, &[], &NameMap::new()).unwrap();
        assert_eq!(
            cs.member_seq[0].detail.ann_builtin.min.as_deref(),
            Some(b"0".as_slice())
        );
        assert_eq!(
            cs.member_seq[0].detail.ann_builtin.max.as_deref(),
            Some(b"100".as_slice())
        );
    }

    #[test]
    fn range_lands_in_complete_typeobject() {
        let s = first_struct("struct S { @range(min=1, max=9) long v; };");
        let cs = build_complete_struct_type(&s, &[], &NameMap::new()).unwrap();
        assert_eq!(
            cs.member_seq[0].detail.ann_builtin.min.as_deref(),
            Some(b"1".as_slice())
        );
        assert_eq!(
            cs.member_seq[0].detail.ann_builtin.max.as_deref(),
            Some(b"9".as_slice())
        );
    }

    // ---- A9 @default_literal: marks the annotated literal, not first() ---

    #[test]
    fn enum_default_literal_marks_selected_not_first() {
        use zerodds_types::type_object::flags::EnumLiteralFlag;
        let e = first_enum("enum E { A, @default_literal B, C };");
        let et = lower_enum_minimal(&e, "E").unwrap();
        let is_default =
            |i: usize| et.literal_seq[i].common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL != 0;
        assert!(!is_default(0), "A must not be default");
        assert!(is_default(1), "B (@default_literal) must be default");
        assert!(!is_default(2), "C must not be default");
    }

    // ---- @ignore_literal_names: sets EnumTypeFlag::IGNORE_LITERAL_NAMES ----

    #[test]
    fn enum_ignore_literal_names_sets_flag() {
        use zerodds_types::type_object::flags::EnumTypeFlag;
        let with = first_enum("@ignore_literal_names enum E { A, B, C };");
        let et = lower_enum_minimal(&with, "E").unwrap();
        assert!(
            et.enum_flags.has(EnumTypeFlag::IGNORE_LITERAL_NAMES),
            "@ignore_literal_names must set IGNORE_LITERAL_NAMES"
        );

        let without = first_enum("enum E { A, B, C };");
        let et2 = lower_enum_minimal(&without, "E").unwrap();
        assert!(
            !et2.enum_flags.has(EnumTypeFlag::IGNORE_LITERAL_NAMES),
            "un-annotated enum must not set IGNORE_LITERAL_NAMES"
        );
    }

    #[test]
    fn struct_explicit_final_extensibility_sets_flag() {
        let s = first_struct("@extensibility(FINAL) struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_FINAL)
        );
    }

    #[test]
    fn struct_multi_declarator_expands_to_separate_members() {
        let s = first_struct("struct S { long a, b, c; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 3);
        for m in &st.member_seq {
            assert!(matches!(
                m.common.member_type_id,
                TypeIdentifier::Primitive(PrimitiveKind::Int32)
            ));
        }
    }

    #[test]
    fn struct_with_must_understand_flag() {
        let s = first_struct("struct S { @must_understand long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_MUST_UNDERSTAND)
        );
    }

    #[test]
    fn struct_with_external_flag() {
        let s = first_struct("struct S { @external long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_EXTERNAL)
        );
    }

    #[test]
    fn try_construct_bits_flow_from_idl_into_complete_typeobject() {
        // P1c E2E: `@try_construct(TRIM|USE_DEFAULT)` on a struct member must
        // reach the Complete TypeObject as the two TRY_CONSTRUCT member-flag
        // bits (XTypes 1.3 §7.2.4.2 / §7.3.1.2.1.1). An un-annotated member
        // stays at the DISCARD default (TRY_CONSTRUCT1 alone).
        use zerodds_types::type_object::flags::StructMemberFlag as F;
        let s = first_struct(
            "@final struct S { \
                 @try_construct(TRIM) string<4> name; \
                 @try_construct(USE_DEFAULT) long v; \
                 long d; \
             };",
        );
        let cs = build_complete_struct_type(&s, &[], &NameMap::new()).unwrap();
        let bits = |i: usize| {
            cs.member_seq[i].common.member_flags.0 & (F::TRY_CONSTRUCT1 | F::TRY_CONSTRUCT2)
        };
        // name: TRIM = both bits (0b11).
        assert_eq!(bits(0), F::TRY_CONSTRUCT1 | F::TRY_CONSTRUCT2);
        // v: USE_DEFAULT = TRY_CONSTRUCT2 alone (0b10).
        assert_eq!(bits(1), F::TRY_CONSTRUCT2);
        // d: no annotation = DISCARD default = TRY_CONSTRUCT1 alone (0b01).
        assert_eq!(bits(2), F::TRY_CONSTRUCT1);
    }

    #[test]
    fn try_construct_default_is_discard_on_minimal_path() {
        // The minimal lower path (`lower_struct_to_minimal`) must also default
        // an un-annotated member to DISCARD, so minimal and complete agree.
        use zerodds_types::type_object::flags::StructMemberFlag as F;
        let s = first_struct("struct S { @try_construct(USE_DEFAULT) long v; long d; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        let v = st.member_seq[0].common.member_flags.0 & (F::TRY_CONSTRUCT1 | F::TRY_CONSTRUCT2);
        let d = st.member_seq[1].common.member_flags.0 & (F::TRY_CONSTRUCT1 | F::TRY_CONSTRUCT2);
        assert_eq!(v, F::TRY_CONSTRUCT2);
        assert_eq!(d, F::TRY_CONSTRUCT1);
    }

    #[test]
    fn struct_with_scoped_member_returns_unresolved() {
        let s = first_struct("struct S { Foo x; };");
        let err = lower_struct_to_minimal(&s).unwrap_err();
        match err {
            MapError::UnresolvedScoped(p) => assert_eq!(p, "Foo"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    fn registry(src: &str) -> LoweredSpec {
        let ast = parse(src, &ParserConfig::default()).expect("parse");
        build_type_registry(&ast).expect("build_type_registry")
    }

    #[test]
    fn registry_resolves_struct_of_struct() {
        // The isolated mapper fails here on `Inner` — the
        // registry resolves the forward ref via the two-pass.
        let lowered = registry("struct Inner { long a; }; struct Outer { Inner i; };");
        assert!(lowered.names.contains_key("Inner"));
        assert!(lowered.names.contains_key("Outer"));
        let (min, _) = lowered.registry.len();
        assert_eq!(min, 2);
    }

    #[test]
    fn registry_topo_order_puts_dependency_first() {
        let lowered = registry("struct Inner { long a; }; struct Outer { Inner i; };");
        let inner = lowered.order.iter().position(|n| n == "Inner").unwrap();
        let outer = lowered.order.iter().position(|n| n == "Outer").unwrap();
        assert!(inner < outer, "dependency must be lowered first");
    }

    #[test]
    fn registry_lowers_enum_and_struct_referencing_it() {
        let lowered = registry("enum Color { RED, GREEN }; struct S { Color c; };");
        assert!(lowered.names.contains_key("Color"));
        assert!(lowered.names.contains_key("S"));
    }

    #[test]
    fn registry_resolves_module_scoped_reference() {
        let lowered = registry("module M { struct A { long x; }; }; struct B { M::A a; };");
        assert!(lowered.names.contains_key("M::A"));
        assert!(lowered.names.contains_key("B"));
    }

    #[test]
    fn registry_isolates_recursive_type_into_skipped() {
        // sequence<Node> is legal IDL, but creates a self-cycle Node → Node.
        // Per-SCC isolation drops just `Node` into `skipped`; the mapping as a
        // whole succeeds (no global RecursiveType error any more).
        let ast = parse(
            "struct Node { long id; sequence<Node> kids; };",
            &ParserConfig::default(),
        )
        .expect("parse");
        let lowered = build_type_registry(&ast).expect("registry must not fail globally");
        assert!(
            !lowered.names.contains_key("Node"),
            "recursive Node must not be lowered"
        );
        assert!(
            lowered
                .skipped
                .iter()
                .any(|(fqn, e)| fqn == "Node" && matches!(e, MapError::RecursiveType(_))),
            "Node must be recorded in skipped, got {:?}",
            lowered.skipped
        );
    }

    #[test]
    fn registry_keeps_independent_type_when_another_is_not_lowerable() {
        // `Node` is recursive (non-lowerable); `Flat` is entirely independent
        // and well-formed. The all-or-nothing bug dropped Flat's TypeObject
        // too — per-SCC isolation must keep it.
        let ast = parse(
            "struct Node { sequence<Node> kids; }; struct Flat { long x; double y; };",
            &ParserConfig::default(),
        )
        .expect("parse");
        let lowered = build_type_registry(&ast).expect("registry");
        assert!(
            lowered.names.contains_key("Flat"),
            "independent Flat must keep its TypeObject"
        );
        assert!(
            lowered.order.iter().any(|n| n == "Flat"),
            "Flat must appear in the emission order"
        );
        assert!(
            !lowered.names.contains_key("Node"),
            "recursive Node must be dropped"
        );
        assert_eq!(lowered.skipped.len(), 1, "only Node is skipped");
        assert_eq!(lowered.skipped[0].0, "Node");
    }

    #[test]
    fn registry_drops_dependents_of_a_skipped_type() {
        // `User` references the recursive `Node`; with `Node` dropped, `User`
        // can no longer resolve it and is dropped too (UnresolvedScoped), while
        // the unrelated `Flat` survives.
        let ast = parse(
            "struct Node { sequence<Node> kids; }; \
             struct User { Node root; }; \
             struct Flat { long x; };",
            &ParserConfig::default(),
        )
        .expect("parse");
        let lowered = build_type_registry(&ast).expect("registry");
        assert!(lowered.names.contains_key("Flat"), "Flat survives");
        assert!(!lowered.names.contains_key("Node"), "Node dropped");
        assert!(
            !lowered.names.contains_key("User"),
            "User depends on dropped Node → dropped"
        );
        assert!(
            lowered.skipped.iter().any(|(fqn, _)| fqn == "User"),
            "User must be recorded as skipped, got {:?}",
            lowered.skipped
        );
    }

    #[test]
    fn registry_distinct_types_get_distinct_hashes() {
        let lowered = registry("struct A { long a; }; struct B { double b; };");
        let a = lowered.names.get("A").unwrap();
        let b = lowered.names.get("B").unwrap();
        assert_ne!(a, b, "structurally different types must hash differently");
    }

    /// P0-9: two modules each declare `struct SameShape { long x; };`. Their
    /// COMPLETE TypeObjects must differ, because the qualified type name
    /// (`Alpha::SameShape` vs `Beta::SameShape`) is serialized into
    /// `CompleteTypeDetail.type_name` (§7.3.4.5.4). Before the FQN fix both
    /// carried the simple name `SameShape` and hashed identically, so
    /// `Alpha::SameShape` and `Beta::SameShape` collided on one COMPLETE
    /// TypeIdentifier.
    #[test]
    fn identical_shapes_in_distinct_modules_get_distinct_complete_type_objects() {
        let src = "module Alpha { struct SameShape { long x; }; }; \
                   module Beta  { struct SameShape { long x; }; };";
        let ast = parse(src, &ParserConfig::default()).expect("parse");
        let lowered = build_type_registry(&ast).expect("registry");

        fn struct_in_module<'a>(
            ast: &'a Specification,
            module: &str,
        ) -> (&'a StructDef, alloc::vec::Vec<alloc::string::String>) {
            for def in &ast.definitions {
                if let Definition::Module(m) = def {
                    if m.name.text == module {
                        for inner in &m.definitions {
                            if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(
                                StructDcl::Def(s),
                            ))) = inner
                            {
                                return (s, alloc::vec![module.into()]);
                            }
                        }
                    }
                }
            }
            panic!("struct in module {module} not found");
        }

        let (alpha_s, alpha_scope) = struct_in_module(&ast, "Alpha");
        let (beta_s, beta_scope) = struct_in_module(&ast, "Beta");

        // COMPLETE: the qualified type name is serialized → must differ.
        let alpha_c = build_complete_struct_type(alpha_s, &alpha_scope, &lowered.names).unwrap();
        let beta_c = build_complete_struct_type(beta_s, &beta_scope, &lowered.names).unwrap();
        assert_eq!(alpha_c.header.detail.type_name, "Alpha::SameShape");
        assert_eq!(beta_c.header.detail.type_name, "Beta::SameShape");

        let alpha_bytes =
            complete_struct_type_object_bytes(alpha_s, &alpha_scope, &lowered.names).unwrap();
        let beta_bytes =
            complete_struct_type_object_bytes(beta_s, &beta_scope, &lowered.names).unwrap();
        assert_ne!(
            alpha_bytes, beta_bytes,
            "COMPLETE TypeObjects of identically shaped structs in distinct \
             modules must differ once the FQN is serialized"
        );

        // MINIMAL: by XTypes 1.3 (§7.3.4.5) the minimal TypeObject header carries
        // NO type name — only structure plus member-name hashes — so two
        // identically shaped structs share one minimal TypeIdentifier. That is
        // unchanged by this fix and correct per spec; the FQN we now feed the
        // minimal builder is inert there (`build_minimal` never reads it). The
        // Minimal-vs-Complete name policy is a separate architectural decision.
        let alpha_min = lowered.names.get("Alpha::SameShape").unwrap();
        let beta_min = lowered.names.get("Beta::SameShape").unwrap();
        assert_eq!(
            alpha_min, beta_min,
            "minimal TypeIdentifiers stay equal (minimal omits the type name)"
        );
    }

    #[test]
    fn registry_lowers_union() {
        let lowered = registry(
            "union U switch (long) { case 1: long x; case 2: double y; default: boolean z; };",
        );
        assert!(lowered.names.contains_key("U"));
    }

    #[test]
    fn registry_union_over_enum_resolves_bare_labels() {
        // Bare case labels (`case RED:`) against the discriminator enum.
        let lowered = registry(
            "enum Color { RED, GREEN, BLUE }; \
             union U switch (Color) { case RED: long r; default: double d; };",
        );
        assert!(lowered.names.contains_key("Color"));
        assert!(lowered.names.contains_key("U"));
    }

    #[test]
    fn registry_struct_with_union_member() {
        let lowered =
            registry("union U switch (long) { case 1: long x; }; struct S { U u; long n; };");
        assert!(lowered.names.contains_key("U"));
        assert!(lowered.names.contains_key("S"));
        let u = lowered.order.iter().position(|n| n == "U").unwrap();
        let s = lowered.order.iter().position(|n| n == "S").unwrap();
        assert!(u < s, "union must be lowered before the struct using it");
    }

    #[test]
    fn registry_lowers_scalar_typedef() {
        let lowered = registry("typedef long MyLong; struct S { MyLong v; };");
        assert!(lowered.names.contains_key("MyLong"));
        assert!(lowered.names.contains_key("S"));
    }

    #[test]
    fn registry_lowers_array_typedef() {
        let lowered = registry("typedef long Matrix[3][3];");
        assert!(lowered.names.contains_key("Matrix"));
    }

    #[test]
    fn registry_multi_declarator_typedef_yields_separate_aliases() {
        let lowered = registry("typedef double Lat, Lon;");
        assert!(lowered.names.contains_key("Lat"));
        assert!(lowered.names.contains_key("Lon"));
    }

    // ---- Bitset / Bitmask (#24 Gap 3) ------------------------------------

    const BITMASK_BITSET_SRC: &str = "bitmask Flags { @position(0) A, @position(3) B }; \
         bitset Coord { bitfield<3> x; bitfield<10> y; };";

    /// Unit: an IDL bitmask + bitset both land in the registry with a
    /// non-zero minimal hash and the correct `MinimalTypeObject` variant —
    /// i.e. they are no longer silently dropped by the old `_ => {}` arm.
    #[test]
    fn registry_lowers_bitmask_and_bitset() {
        use zerodds_types::type_identifier::EquivalenceHash;

        let lowered = registry(BITMASK_BITSET_SRC);

        // Both FQNs are resolvable to a non-zero EquivalenceHashMinimal.
        for fqn in ["Flags", "Coord"] {
            match lowered.names.get(fqn) {
                Some(TypeIdentifier::EquivalenceHashMinimal(h)) => {
                    assert_ne!(*h, EquivalenceHash::ZERO, "{fqn} has an all-zero hash");
                }
                other => panic!("{fqn} not an EquivalenceHashMinimal: {other:?}"),
            }
        }

        // The stored objects are the Bitmask / Bitset variants.
        let by_name: alloc::collections::BTreeMap<&str, &MinimalTypeObject> =
            lowered.iter().collect();
        assert!(
            matches!(by_name.get("Flags"), Some(MinimalTypeObject::Bitmask(_))),
            "Flags must be a Bitmask MinimalTypeObject"
        );
        assert!(
            matches!(by_name.get("Coord"), Some(MinimalTypeObject::Bitset(_))),
            "Coord must be a Bitset MinimalTypeObject"
        );
    }

    /// Fetches the single `MinimalTypeObject` lowered for `fqn`.
    fn lowered_object(lowered: &LoweredSpec, fqn: &str) -> MinimalTypeObject {
        lowered
            .iter()
            .find(|(n, _)| *n == fqn)
            .map(|(_, o)| o.clone())
            .unwrap_or_else(|| panic!("no lowered object for {fqn}"))
    }

    /// Golden: the minimal bitmask `TypeObject` the IDL path emits is
    /// byte-identical to a direct `BitmaskBuilder` reference (Path A).
    /// Mirrors idl-rust `complete_type_bytes_are_byte_identical_to_path_a_struct_builder`.
    #[test]
    fn bitmask_minimal_bytes_byte_identical_to_builder() {
        use zerodds_types::type_object::TypeObject;

        let lowered = registry(BITMASK_BITSET_SRC);
        let actual = TypeObject::Minimal(lowered_object(&lowered, "Flags"))
            .to_bytes_le()
            .expect("encode actual");

        // Path A: no @bit_bound in the IDL → the builder's default 32.
        let expected_obj = TypeObjectBuilder::bitmask("Flags")
            .flag("A", 0)
            .flag("B", 3)
            .build_minimal();
        let expected = TypeObject::Minimal(MinimalTypeObject::Bitmask(expected_obj))
            .to_bytes_le()
            .expect("encode expected");

        assert_eq!(
            actual, expected,
            "IDL bitmask TypeObject must be byte-identical to BitmaskBuilder"
        );
    }

    /// Golden: the minimal bitset `TypeObject` the IDL path emits is
    /// byte-identical to a direct `BitsetBuilder` reference (Path A). The
    /// un-typed `bitfield<3>`/`bitfield<10>` pick uint8/uint16 holders and
    /// cumulative positions 0 / 3.
    #[test]
    fn bitset_minimal_bytes_byte_identical_to_builder() {
        use zerodds_types::type_object::TypeObject;

        let lowered = registry(BITMASK_BITSET_SRC);
        let actual = TypeObject::Minimal(lowered_object(&lowered, "Coord"))
            .to_bytes_le()
            .expect("encode actual");

        let expected_obj = TypeObjectBuilder::bitset("Coord")
            .field("x", 0, 3, PrimitiveKind::UInt8.to_u8())
            .field("y", 3, 10, PrimitiveKind::UInt16.to_u8())
            .build_minimal();
        let expected = TypeObject::Minimal(MinimalTypeObject::Bitset(expected_obj))
            .to_bytes_le()
            .expect("encode expected");

        assert_eq!(
            actual, expected,
            "IDL bitset TypeObject must be byte-identical to BitsetBuilder"
        );
    }

    /// E2E: IDL → compose-path `TypeObject` (serialise like
    /// `zerodds-idl-compose::type_object_blobs`) → wire bytes → decode →
    /// `DynamicType` (bridge `create_type_w_type_object_in` / `resolve_minimal`)
    /// → back to a Complete `TypeObject` (bridge `to_complete_bitset` /
    /// `to_complete_bitmask`). The recovered field/flag geometry must equal
    /// the source IDL.
    #[test]
    fn bitset_bitmask_roundtrip_through_bridge() {
        use zerodds_types::dynamic::DynamicTypeBuilderFactory;
        use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

        let lowered = registry(BITMASK_BITSET_SRC);

        // --- bitset Coord ---
        let coord_bytes = TypeObject::Minimal(lowered_object(&lowered, "Coord"))
            .to_bytes_le()
            .expect("serialise Coord");
        let coord_to = TypeObject::from_bytes_le(&coord_bytes).expect("decode Coord");
        let coord_dyn =
            DynamicTypeBuilderFactory::create_type_w_type_object_in(&coord_to, &lowered.registry)
                .expect("bridge Coord → DynamicType");
        let TypeObject::Complete(CompleteTypeObject::Bitset(bs)) =
            coord_dyn.to_type_object().expect("Coord → Complete")
        else {
            panic!("recovered Coord is not a Complete Bitset");
        };
        let fields: alloc::vec::Vec<(u16, u8, u8)> = bs
            .field_seq
            .iter()
            .map(|f| (f.common.position, f.common.bitcount, f.common.holder_type))
            .collect();
        assert_eq!(
            fields,
            alloc::vec![
                (0u16, 3u8, PrimitiveKind::UInt8.to_u8()),
                (3u16, 10u8, PrimitiveKind::UInt16.to_u8()),
            ],
            "recovered bitset geometry must equal the source IDL"
        );

        // --- bitmask Flags ---
        let flags_bytes = TypeObject::Minimal(lowered_object(&lowered, "Flags"))
            .to_bytes_le()
            .expect("serialise Flags");
        let flags_to = TypeObject::from_bytes_le(&flags_bytes).expect("decode Flags");
        let flags_dyn =
            DynamicTypeBuilderFactory::create_type_w_type_object_in(&flags_to, &lowered.registry)
                .expect("bridge Flags → DynamicType");
        let TypeObject::Complete(CompleteTypeObject::Bitmask(bm)) =
            flags_dyn.to_type_object().expect("Flags → Complete")
        else {
            panic!("recovered Flags is not a Complete Bitmask");
        };
        assert_eq!(
            bm.bit_bound, 32,
            "default bit_bound survives the round-trip"
        );
        let positions: alloc::vec::Vec<u16> =
            bm.flag_seq.iter().map(|f| f.common.position).collect();
        assert_eq!(
            positions,
            alloc::vec![0u16, 3u16],
            "recovered bitmask flag positions must equal the source IDL"
        );
    }
}
